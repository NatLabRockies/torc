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
use crate::client::commands::pagination::files::{FileListParams, paginate_files};
use crate::client::offline_journal::{FLUSH_BATCH_SIZE, OfflineJournal};
use crate::client::ro_crate_utils;
use crate::models::{BatchCompleteJobsRequest, FileModel, JobCompletionEntry, JobStatus};

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
    if files.is_empty() && format != "json" {
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
                    // Output metadata was not recorded while the runner was offline. Restore
                    // it per batch so a later failed request cannot skip it.
                    let completed: Vec<&JobCompletionEntry> = chunk
                        .iter()
                        .filter(|entry| {
                            entry.status == JobStatus::Completed
                                && response.completed.contains(&entry.job_id)
                        })
                        .collect();
                    restore_offline_output_metadata(config, workflow_id, &completed);
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
            eprintln!(
                "Rejected completions are usually from a superseded run (run_id no longer \
                 current after a manual retry/reset); this is expected and safe."
            );
        }
    }

    Ok(summary)
}

/// Restore the output metadata a runner skips while offline: each output file's on-disk
/// mtime and, for RO-Crate workflows, the CreateAction and output File provenance entities.
/// Call it with journal entries the server accepted as Completed. Paths resolve against the
/// current directory, as they do for the runner.
pub(crate) fn restore_offline_output_metadata(
    config: &Configuration,
    workflow_id: i64,
    entries: &[&JobCompletionEntry],
) {
    if entries.is_empty() {
        return;
    }
    let ro_crate = match apis::workflows_api::get_workflow(config, workflow_id) {
        Ok(w) => w.enable_ro_crate == Some(true),
        Err(e) => {
            warn!("Failed to get workflow_id={}: {}", workflow_id, e);
            false
        }
    };
    for entry in entries {
        let job_id = entry.job_id;
        let files = match paginate_files(
            config,
            workflow_id,
            FileListParams::new().with_produced_by_job_id(job_id),
        ) {
            Ok(files) => files,
            Err(e) => {
                warn!(
                    "Failed to list output files workflow_id={} job_id={}: {}",
                    workflow_id, job_id, e
                );
                continue;
            }
        };
        let mut updated = Vec::with_capacity(files.len());
        for mut file in files {
            let Some(file_id) = file.id else { continue };
            let st_mtime = match std::fs::metadata(&file.path).and_then(|m| m.modified()) {
                Ok(t) => t
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0),
                Err(e) => {
                    warn!(
                        "Cannot stat output file {} (file_id={}) for job_id={}: {}",
                        file.path, file_id, job_id, e
                    );
                    continue;
                }
            };
            file.st_mtime = Some(st_mtime);
            match apis::files_api::update_file(config, file_id, file.clone()) {
                Ok(_) => updated.push(file),
                Err(e) => warn!("Failed to update st_mtime for file_id={}: {}", file_id, e),
            }
        }
        if ro_crate && !updated.is_empty() {
            restore_output_provenance(config, workflow_id, entry, &updated);
        }
    }
}

/// Create the RO-Crate entities the runner would have created at completion, using the
/// run and attempt recorded in the journal entry.
fn restore_output_provenance(
    config: &Configuration,
    workflow_id: i64,
    entry: &JobCompletionEntry,
    output_files: &[FileModel],
) {
    let job = match apis::jobs_api::get_job(config, entry.job_id) {
        Ok(job) => job,
        Err(e) => {
            warn!(
                "Could not fetch job_id={} for RO-Crate provenance: {}",
                entry.job_id, e
            );
            return;
        }
    };
    let attempt_id = entry.result.attempt_id.unwrap_or(1);
    let input_file_paths: Vec<String> = job
        .input_file_ids
        .iter()
        .flatten()
        .filter_map(|id| apis::files_api::get_file(config, *id).ok())
        .map(|f| f.path)
        .collect();
    let output_file_paths: Vec<String> = output_files.iter().map(|f| f.path.clone()).collect();
    ro_crate_utils::create_create_action_entity(
        config,
        workflow_id,
        entry.run_id,
        &job,
        attempt_id,
        &input_file_paths,
        &output_file_paths,
    );
    for file in output_files {
        let content_size = std::fs::metadata(&file.path).ok().map(|m| m.len());
        ro_crate_utils::create_ro_crate_entity_for_output_file(
            config,
            workflow_id,
            entry.run_id,
            file,
            content_size,
            entry.job_id,
            attempt_id,
            &input_file_paths,
        );
    }
}
