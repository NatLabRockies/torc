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
    // Relative output paths resolve against the workflow's submission directory (falling
    // back to the current directory), and the current run_id identifies already-accepted
    // completions whose output metadata may still need restoring.
    let workflow = apis::workflows_api::get_workflow(config, workflow_id)
        .map_err(|e| format!("Failed to fetch workflow {}: {}", workflow_id, e))?;
    let submission_dir = workflow
        .submission_directory
        .clone()
        .map(std::path::PathBuf::from);
    let current_run_id = workflow.run_id.unwrap_or(0);
    let server_stores_mtimes =
        crate::client::version_check::check_version(config).server_stores_completion_metadata();
    let mut metadata_failures = 0usize;

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
                    // it per batch so a later failed request cannot skip it. Also restore it
                    // for completions an earlier reconcile already replayed (now rejected as
                    // duplicates) so a failed restore can be retried by re-running reconcile.
                    let (completed, lookup_failures) =
                        completions_to_restore(config, chunk, &response.completed, current_run_id);
                    metadata_failures += lookup_failures;
                    metadata_failures += restore_offline_output_metadata(
                        config,
                        workflow_id,
                        &completed,
                        submission_dir.as_deref(),
                        server_stores_mtimes,
                    );
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

    if metadata_failures > 0 {
        warn!(
            "Could not restore output metadata for {} job(s); see the warnings above. Fix the \
             cause (e.g. run from the directory the output paths are relative to) and re-run \
             reconcile to retry.",
            metadata_failures
        );
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

/// Restore the output metadata the server could not record from the journal entry itself:
/// the on-disk mtime of output files missing from `entry.output_files` (journals written by
/// older runners, or files an offline runner could not check) and, for RO-Crate workflows,
/// the CreateAction and output File provenance entities. Mtimes present in
/// `entry.output_files` were applied by the server with the completion and are not restatted;
/// if `server_stores_mtimes` is false (a server that ignores `output_files`), they are written
/// here instead. Call it with journal entries the server accepted as Completed.
///
/// Relative output paths resolve against `base_dir` when given, else the current directory.
/// Returns the number of jobs whose metadata could not be fully restored.
pub(crate) fn restore_offline_output_metadata(
    config: &Configuration,
    workflow_id: i64,
    entries: &[&JobCompletionEntry],
    base_dir: Option<&std::path::Path>,
    server_stores_mtimes: bool,
) -> usize {
    if entries.is_empty() {
        return 0;
    }
    let mut failures = 0;
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
                failures += 1;
                continue;
            }
        };
        let mut updated = Vec::with_capacity(files.len());
        let mut job_failed = false;
        for mut file in files {
            let Some(file_id) = file.id else { continue };
            let journaled = entry
                .output_files
                .iter()
                .flatten()
                .find(|f| f.file_id == file_id);
            if journaled.is_some() && server_stores_mtimes {
                updated.push(file);
                continue;
            }
            let path = match base_dir {
                Some(base) if std::path::Path::new(&file.path).is_relative() => {
                    base.join(&file.path)
                }
                _ => std::path::PathBuf::from(&file.path),
            };
            let stat = match journaled {
                Some(f) => Ok(f.st_mtime),
                None => std::fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .map(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs_f64())
                            .unwrap_or(0.0)
                    }),
            };
            let st_mtime = match stat {
                Ok(t) => t,
                Err(e) => {
                    warn!(
                        "Cannot stat output file {} (file_id={}) for job_id={}: {}",
                        path.display(),
                        file_id,
                        job_id,
                        e
                    );
                    job_failed = true;
                    continue;
                }
            };
            file.st_mtime = Some(st_mtime);
            match apis::files_api::update_file(config, file_id, file.clone()) {
                Ok(_) => updated.push(file),
                Err(e) => {
                    warn!("Failed to update st_mtime for file_id={}: {}", file_id, e);
                    job_failed = true;
                }
            }
        }
        if ro_crate && !updated.is_empty() {
            restore_output_provenance(config, workflow_id, entry, &updated, base_dir);
        }
        if job_failed {
            failures += 1;
        }
    }
    failures
}

/// True if the server already has this journaled completion: the job is Completed in the
/// current run. Used to retry metadata restoration for completions an earlier reconcile
/// replayed.
/// Select the Completed entries in `chunk` whose output metadata should be restored: those
/// the server just accepted, plus those an earlier replay already delivered in the current
/// run (now rejected as duplicates). `current_run_id` must come from the server, not a cached
/// value, so a stale completion from before a reset is never treated as a duplicate.
/// Returns the entries and the number whose status could not be looked up; callers count
/// those as metadata failures so the journal is kept for a retry.
pub(crate) fn completions_to_restore<'a>(
    config: &Configuration,
    chunk: &'a [JobCompletionEntry],
    accepted: &[i64],
    current_run_id: i64,
) -> (Vec<&'a JobCompletionEntry>, usize) {
    let mut entries = Vec::new();
    let mut lookup_failures = 0;
    for entry in chunk.iter().filter(|e| e.status == JobStatus::Completed) {
        if accepted.contains(&entry.job_id) {
            entries.push(entry);
        } else if entry.run_id == current_run_id {
            match apis::jobs_api::get_job(config, entry.job_id) {
                Ok(job) if job.status == Some(JobStatus::Completed) => entries.push(entry),
                Ok(_) => {}
                Err(e) => {
                    warn!("Could not look up job_id={}: {}", entry.job_id, e);
                    lookup_failures += 1;
                }
            }
        }
    }
    (entries, lookup_failures)
}

/// Create the RO-Crate entities the runner would have created at completion, using the
/// run and attempt recorded in the journal entry.
fn restore_output_provenance(
    config: &Configuration,
    workflow_id: i64,
    entry: &JobCompletionEntry,
    output_files: &[FileModel],
    base_dir: Option<&std::path::Path>,
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
        let path = match base_dir {
            Some(base) if std::path::Path::new(&file.path).is_relative() => base.join(&file.path),
            _ => std::path::PathBuf::from(&file.path),
        };
        let content_size = std::fs::metadata(&path).ok().map(|m| m.len());
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
