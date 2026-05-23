//! `torc workflows reconcile` — replay offline-drain journals back to the server.
//!
//! When job runners lose contact with the server they drain (run their jobs to
//! completion) and journal the results to local SQLite files (see
//! [`crate::client::offline_journal`]). Once the server is healthy again, this
//! command discovers every journal for a `(workflow_id, run_id)` under a base
//! directory and replays the completions in bulk so a user does not have to run
//! one command per compute node.
//!
//! Replay is idempotent and safe: the server validates each completion's
//! `run_id` against the workflow's current generation, so completions from a
//! superseded run (e.g. after a manual reset) are rejected rather than applied.

use log::{info, warn};

use crate::client::apis;
use crate::client::apis::configuration::Configuration;
use crate::client::offline_journal::{FLUSH_BATCH_SIZE, OfflineJournal};
use crate::models::BatchCompleteJobsRequest;

/// Outcome of a reconcile run, returned for testing and JSON output.
#[derive(Debug, Default)]
pub struct ReconcileSummary {
    pub files: usize,
    pub total_completions: usize,
    pub accepted: usize,
    pub rejected: usize,
}

/// Discover and replay all offline journals for `(workflow_id, run_id)` found
/// under `base_dir`.
pub fn reconcile(
    config: &Configuration,
    workflow_id: i64,
    run_id: i64,
    base_dir: &std::path::Path,
    format: &str,
) -> Result<ReconcileSummary, Box<dyn std::error::Error>> {
    let files = OfflineJournal::discover(base_dir, workflow_id, run_id);
    if files.is_empty() {
        println!(
            "No offline journals found for workflow_id={} run_id={} under {}",
            workflow_id,
            run_id,
            base_dir.display()
        );
        return Ok(ReconcileSummary::default());
    }

    info!(
        "Found {} offline journal file(s) for workflow_id={} run_id={}",
        files.len(),
        workflow_id,
        run_id
    );

    let mut summary = ReconcileSummary {
        files: files.len(),
        ..Default::default()
    };

    for path in &files {
        let entries = match OfflineJournal::read_file(path) {
            Ok(entries) => entries,
            Err(e) => {
                warn!("Skipping unreadable journal {}: {}", path.display(), e);
                continue;
            }
        };
        if entries.is_empty() {
            continue;
        }
        summary.total_completions += entries.len();
        info!(
            "Replaying {} completion(s) from {}",
            entries.len(),
            path.display()
        );

        for chunk in entries.chunks(FLUSH_BATCH_SIZE) {
            let request = BatchCompleteJobsRequest {
                completions: chunk.to_vec(),
            };
            match apis::workflows_api::batch_complete_jobs(config, workflow_id, request) {
                Ok(response) => {
                    summary.accepted += response.completed.len();
                    for err in &response.errors {
                        summary.rejected += 1;
                        warn!(
                            "Rejected completion job_id={} message={}",
                            err.job_id, err.message
                        );
                    }
                }
                Err(e) => {
                    return Err(format!(
                        "Failed to submit completions from {}: {}. \
                         Re-run reconcile once the server is healthy.",
                        path.display(),
                        e
                    )
                    .into());
                }
            }
        }
    }

    if format == "json" {
        println!(
            r#"{{"files": {}, "total_completions": {}, "accepted": {}, "rejected": {}}}"#,
            summary.files, summary.total_completions, summary.accepted, summary.rejected
        );
    } else {
        println!(
            "Reconciled workflow_id={} run_id={}: {} file(s), {} completion(s) \
             ({} accepted, {} rejected)",
            workflow_id,
            run_id,
            summary.files,
            summary.total_completions,
            summary.accepted,
            summary.rejected
        );
        if summary.rejected > 0 {
            println!(
                "Rejected completions are usually from a superseded run (run_id no longer \
                 current after a manual retry/reset); this is expected and safe."
            );
        }
    }

    Ok(summary)
}
