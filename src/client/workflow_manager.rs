use crate::client::apis;
use crate::client::apis::configuration::Configuration;
use crate::client::errors::TorcError;
use crate::config::TorcConfig;
use log::{self, debug, error, info, warn};

use crate::MAX_RECORD_TRANSFER_COUNT;
use crate::client::commands::pagination::{FileListParams, iter_files};
use crate::client::version_check;
use crate::models::{self, FileModel, TaskModel, WorkflowModel};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

#[derive(Debug, serde::Serialize)]
pub struct InitializationCheck {
    pub safe: bool,
    pub missing_input_files: Vec<String>,
    pub existing_output_files: Vec<String>,
}

/// Summary of what `WorkflowManager::start` actually did, so the caller can distinguish a real
/// submission from a no-op. A zero-allocation outcome is not a benign "deferred" state: deferred
/// `on_jobs_ready`/`on_jobs_complete` actions are only ever fired by running workers, and a worker
/// only exists because an allocation launched it. So if a submit invocation launches no
/// allocations, nothing downstream will ever fire it either — the caller should report that as an
/// error rather than printing success.
#[derive(Debug, Default, Clone, Copy)]
pub struct StartOutcome {
    /// Total Slurm allocations actually submitted across every fired schedule_nodes action.
    pub allocations_submitted: i32,
    /// Number of slurm schedule_nodes actions that were pending and due at submit time (counted
    /// before claim/dedup). Used to explain a zero-allocation outcome: zero here means nothing was
    /// due (already running/complete, or misconfigured with no bootstrap action); non-zero means
    /// actions were due but produced no allocations (claimed by a concurrent submitter, or their
    /// counts were overridden to 0).
    pub slurm_actions_seen: usize,
}

/// The workflow's files and what the client observed on disk for each.
struct FileSnapshot {
    entries: Vec<models::FileStatEntry>,
    files: HashMap<i64, FileModel>,
}

fn describe_file(files: &HashMap<i64, FileModel>, id: i64) -> String {
    match files.get(&id) {
        Some(f) => format!("'{}' (id: {}, path: {})", f.name, id, f.path),
        None => format!("id {}", id),
    }
}

pub struct WorkflowManager {
    config: Configuration,
    torc_config: TorcConfig,
    pub workflow_id: i64,
}

impl WorkflowManager {
    pub fn new(config: Configuration, torc_config: TorcConfig, workflow: WorkflowModel) -> Self {
        let workflow_id = workflow.id.expect("Workflow ID must be present");
        WorkflowManager {
            config,
            torc_config,
            workflow_id,
        }
    }

    /// Check if initialization is safe to run without executing.
    /// Returns information about missing input files and existing output files.
    pub fn check_initialization(&self) -> Result<InitializationCheck, TorcError> {
        let (response, files) = self.update_file_stats(true)?;
        let missing_input_files: Vec<String> = response
            .missing_required_file_ids
            .iter()
            .map(|id| {
                files
                    .get(id)
                    .map_or_else(|| id.to_string(), |f| f.path.clone())
            })
            .collect();
        let existing_output_files = self.get_existing_output_files()?;

        // Safe if no missing input files (existing output files are just warnings)
        let safe = missing_input_files.is_empty();

        Ok(InitializationCheck {
            safe,
            missing_input_files,
            existing_output_files,
        })
    }

    /// Get list of existing output files (file paths)
    fn get_existing_output_files(&self) -> Result<Vec<String>, TorcError> {
        let params = FileListParams::new().with_is_output(true);
        let files_iterator = iter_files(&self.config, self.workflow_id, params);

        let mut existing_files = Vec::new();
        for file_result in files_iterator {
            match file_result {
                Ok(file) => {
                    let file_path = Path::new(&file.path);
                    if file_path.exists() {
                        existing_files.push(file.path.clone());
                    }
                }
                Err(err) => {
                    warn!(
                        "Failed to fetch file from API during output file check: {}",
                        err
                    );
                }
            }
        }

        Ok(existing_files)
    }

    /// Initialize the jobs and start the workflow.
    ///
    /// If force is false:
    ///   - Return an error if required input files are missing.
    ///
    /// If force is true:
    ///   - Ignore missing input files.
    pub fn initialize(&self, force: bool) -> Result<(), TorcError> {
        self.check_workflow()?;
        let snapshot = self.snapshot_files(force)?;
        self.cleanup_output_files(false)?;
        match apis::workflows_api::reset_workflow_status(&self.config, self.workflow_id, None) {
            Ok(_) => {}
            Err(err) => {
                error!(
                    "Failed to reset status of workflow_id={}: {}",
                    self.workflow_id, err
                );
                return Err(TorcError::ApiError(err.to_string()));
            }
        }
        match apis::workflows_api::reset_job_status(
            &self.config,
            self.workflow_id,
            Some(false),
            None,
            None,
        ) {
            Ok(_) => {}
            Err(err) => {
                error!(
                    "Failed to reset job status of workflow_id={}: {}",
                    self.workflow_id, err
                );
                return Err(TorcError::ApiError(err.to_string()));
            }
        }
        // reset_workflow_status above bumps run_id — no separate bump call needed.
        self.sync_file_stats(&snapshot, force, false)?;
        self.initialize_jobs(false)?;
        // Event is now broadcast via SSE from the server
        Ok(())
    }

    /// Initialize the workflow asynchronously.
    ///
    /// Performs the same client-side pre-steps as `initialize()`, but runs the server-side
    /// initialization (`initialize_jobs`) asynchronously and returns a task record.
    ///
    /// If an async task is already in-flight for this workflow, returns that existing task
    /// without mutating state — the pre-steps are owned by whoever started that task.
    pub fn initialize_async(&self, force: bool) -> Result<TaskModel, TorcError> {
        // Probe for an active task before running check_workflow so a second caller
        // gets the existing task handle even if local validation would have failed.
        if let Some(active) = self.get_active_task()? {
            info!(
                "workflow_id={} already has an active {} task (id={}); returning it",
                self.workflow_id, active.operation, active.id
            );
            return Ok(active);
        }
        self.check_workflow()?;
        let snapshot = self.snapshot_files(force)?;
        self.cleanup_output_files(false)?;
        match apis::workflows_api::reset_workflow_status(&self.config, self.workflow_id, None) {
            Ok(_) => {}
            Err(err) => {
                error!(
                    "Failed to reset status of workflow_id={}: {}",
                    self.workflow_id, err
                );
                return Err(TorcError::ApiError(err.to_string()));
            }
        }
        match apis::workflows_api::reset_job_status(
            &self.config,
            self.workflow_id,
            Some(false),
            None,
            None,
        ) {
            Ok(_) => {}
            Err(err) => {
                error!(
                    "Failed to reset job status of workflow_id={}: {}",
                    self.workflow_id, err
                );
                return Err(TorcError::ApiError(err.to_string()));
            }
        }
        // reset_workflow_status above bumps run_id — no separate bump call needed.
        self.sync_file_stats(&snapshot, force, false)?;
        self.initialize_jobs_async(false)
    }

    /// Start the workflow: initialize if needed and schedule nodes for on_workflow_start actions
    ///
    /// `allocation_overrides` maps an action id to the number of Slurm allocations to submit for
    /// that action **on this invocation only** (it is not persisted to the action's config). It is
    /// populated by the interactive submit review (see `slurm::review_submit_pending_actions`) so a
    /// user re-submitting a workflow can right-size what `sbatch` requests without editing the
    /// action. Actions the user chose to disable are already suppressed (claimed) before `start` is
    /// called, so they do not appear in the pending set here; an empty map means "fire every pending
    /// action with its configured count".
    pub fn start(
        &self,
        force: bool,
        max_parallel_jobs_override: Option<i32>,
        output_dir: &str,
        poll_interval_override: Option<i32>,
        claim_backoff_max_secs_override: Option<f64>,
        allocation_overrides: &std::collections::HashMap<i64, i32>,
    ) -> Result<StartOutcome, TorcError> {
        // Check if workflow is uninitialized
        match apis::workflows_api::is_workflow_uninitialized(&self.config, self.workflow_id) {
            Ok(response) => {
                if let Some(is_uninitialized) =
                    response.get("is_uninitialized").and_then(|v| v.as_bool())
                {
                    if is_uninitialized {
                        info!(
                            "Workflow {} is uninitialized. Initializing...",
                            self.workflow_id
                        );
                        self.initialize(force)?;
                    } else {
                        info!("Workflow {} is already initialized", self.workflow_id);
                    }
                }
            }
            Err(err) => {
                error!("Failed to check if workflow is uninitialized: {}", err);
                return Err(TorcError::ApiError(err.to_string()));
            }
        }

        // Get every pending schedule_nodes-capable action, regardless of trigger type. "Pending"
        // already means "armed and due now" (trigger_count >= required_triggers, executed = 0), and
        // reinitialize's keep/re-arm logic guarantees that set is exactly what should be scheduled:
        //
        // - Initial run: only on_workflow_start (and any on_jobs_ready gated on root jobs that are
        //   ready at init) are pending, so this bootstraps the first allocations.
        // - Re-run (reset a subset of jobs + reinit): the reset jobs' job-gated actions re-arm and
        //   become pending while untouched stages stay suppressed, so submit re-schedules exactly
        //   the affected classes. This is what lets `reset-status <ids> --reinit` + `submit`
        //   re-submit only the jobs being re-run.
        //
        // Note: We don't create a compute node for the submission host (login node) since it's not
        // actually running jobs - it just submits to the scheduler. Downstream job-gated actions
        // whose gates are not yet ready stay pending-less here and are fired later by running
        // workers (see job_runner::check_and_execute_actions).
        let actions = match apis::workflow_actions_api::get_pending_actions(
            &self.config,
            self.workflow_id,
            Some(vec![
                "on_workflow_start".to_string(),
                "on_jobs_ready".to_string(),
                "on_jobs_complete".to_string(),
            ]),
        ) {
            Ok(actions) => actions,
            Err(err) => {
                error!("Failed to get pending actions: {}", err);
                return Err(TorcError::ApiError(err.to_string()));
            }
        };

        // Filter for schedule_nodes actions
        let mut outcome = StartOutcome::default();
        // Whether the server records action outcomes; checked on the first claimed action.
        let mut report_outcomes: Option<bool> = None;
        for action in actions {
            let action_type = &action.action_type;
            let action_id = action.id.unwrap_or(0);

            if action_type == "schedule_nodes" {
                // action_config is already a JSON Value
                let action_config = &action.action_config;

                let scheduler_type = action_config
                    .get("scheduler_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Only claim the action if we can execute it (scheduler_type == "slurm")
                if scheduler_type == "slurm" {
                    outcome.slurm_actions_seen += 1;
                    // Claim the action atomically (no compute_node_id since we're on login node)
                    let claimed = match crate::client::utils::claim_action(
                        &self.config,
                        self.workflow_id,
                        action_id,
                        None, // No compute node for login node submissions
                        20,   // wait_for_healthy_database_minutes - use a reasonable default
                    ) {
                        Ok(claimed) => claimed,
                        Err(err) => {
                            warn!("Failed to claim action {}: {}", action_id, err);
                            continue;
                        }
                    };

                    if !claimed {
                        debug!("Action {} already claimed", action_id);
                        continue;
                    }

                    // Successfully claimed, now execute
                    info!(
                        "Scheduling Slurm nodes for {} action {}",
                        action.trigger_type, action_id
                    );

                    let scheduler_id = action_config
                        .get("scheduler_id")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let start_one_worker_per_node = action_config
                        .get("start_one_worker_per_node")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    // Use the per-submission override from the interactive review if the user set
                    // one for this action; otherwise fall back to the action's configured count
                    // (missing defaults to 1, matching job_runner / list-actions display).
                    let num_allocations = allocation_overrides.get(&action_id).copied().unwrap_or(
                        action_config
                            .get("num_allocations")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(1) as i32,
                    );
                    let max_parallel_jobs = max_parallel_jobs_override.or_else(|| {
                        action_config
                            .get("max_parallel_jobs")
                            .and_then(|v| v.as_i64())
                            .map(|v| v as i32)
                    });
                    let poll_interval = poll_interval_override
                        .unwrap_or(self.torc_config.client.slurm.poll_interval);

                    let result = crate::client::commands::slurm::schedule_slurm_nodes(
                        &self.config,
                        self.workflow_id,
                        scheduler_id,
                        num_allocations,
                        start_one_worker_per_node,
                        "",
                        output_dir,
                        poll_interval,
                        max_parallel_jobs,
                        self.torc_config.client.slurm.keep_submission_scripts,
                        claim_backoff_max_secs_override,
                    );
                    if *report_outcomes.get_or_insert_with(|| {
                        version_check::check_version(&self.config).server_supports_action_outcomes()
                    }) {
                        crate::client::utils::report_action_result(
                            &self.config,
                            self.workflow_id,
                            action_id,
                            &result.as_ref().map(|_| ()).map_err(|e| e.to_string()),
                            20,
                        );
                    }
                    match result {
                        Ok(()) => {
                            outcome.allocations_submitted += num_allocations.max(0);
                            info!(
                                "Successfully scheduled {} Slurm allocation(s) for {} action {}",
                                num_allocations, action.trigger_type, action_id
                            );
                        }
                        Err(err) => {
                            error!(
                                "Failed to schedule Slurm nodes for action {}: {}",
                                action_id, err
                            );
                            return Err(TorcError::OperationNotAllowed(format!(
                                "Failed to schedule Slurm nodes: {}",
                                err
                            )));
                        }
                    }
                } else {
                    debug!(
                        "scheduler_type = {} is not 'slurm', skipping action {} in WorkflowManager (may be handled by job runner)",
                        scheduler_type, action_id
                    );
                }
            }
        }

        Ok(outcome)
    }

    /// Reinitialize the workflow. Resets workflow status (which also bumps
    /// run_id) and runs the startup script.
    pub fn reinitialize(&self, force: bool, dry_run: bool) -> Result<(), TorcError> {
        self.check_workflow()?;
        let snapshot = self.snapshot_files(force || dry_run)?;
        if !dry_run {
            match apis::workflows_api::reset_workflow_status(&self.config, self.workflow_id, None) {
                Ok(_) => {
                    info!("Reset status of workflow_id={}", self.workflow_id);
                }
                Err(err) => {
                    error!(
                        "Failed to reset status of workflow_id={}: {}",
                        self.workflow_id, err
                    );
                    return Err(TorcError::ApiError(err.to_string()));
                }
            }
        }
        self.reinitialize_jobs_from(&snapshot, force, dry_run)?;
        // Event is now broadcast via SSE from the server
        Ok(())
    }

    /// Reinitialize the workflow asynchronously.
    ///
    /// Runs client-side reinitialization checks and change detection, then starts an async
    /// server-side initialization and returns the task record. If `dry_run` is true,
    /// no task is created and `Ok(None)` is returned.
    pub fn reinitialize_async(
        &self,
        force: bool,
        dry_run: bool,
    ) -> Result<Option<TaskModel>, TorcError> {
        // Probe for an in-flight task before doing any local validation. If one is active,
        // its initial caller has already validated the workflow and owns the pre-steps —
        // running check_workflow here would be redundant and could fail spuriously, denying
        // the second caller the existing task handle.
        if !dry_run && let Some(active) = self.get_active_task()? {
            info!(
                "workflow_id={} already has an active {} task (id={}); returning it",
                self.workflow_id, active.operation, active.id
            );
            return Ok(Some(active));
        }

        self.check_workflow()?;
        let snapshot = self.snapshot_files(force || dry_run)?;
        if dry_run {
            self.reinitialize_jobs_from(&snapshot, force, true)?;
            return Ok(None);
        }

        match apis::workflows_api::reset_workflow_status(&self.config, self.workflow_id, None) {
            Ok(_) => {
                info!("Reset status of workflow_id={}", self.workflow_id);
            }
            Err(err) => {
                error!(
                    "Failed to reset status of workflow_id={}: {}",
                    self.workflow_id, err
                );
                return Err(TorcError::ApiError(err.to_string()));
            }
        }

        self.sync_file_stats(&snapshot, force, false)?;
        self.process_changed_user_data(false)?;

        let task = self.initialize_jobs_async(true)?;
        Ok(Some(task))
    }

    /// Return the single active async task for this workflow, if any.
    fn get_active_task(&self) -> Result<Option<TaskModel>, TorcError> {
        match apis::workflows_api::get_active_task_for_workflow(&self.config, self.workflow_id) {
            Ok(resp) => Ok(resp.task),
            Err(err) => Err(TorcError::ApiError(err.to_string())),
        }
    }

    fn initialize_jobs_async(&self, only_uninitialized: bool) -> Result<TaskModel, TorcError> {
        match apis::workflows_api::initialize_jobs(
            &self.config,
            self.workflow_id,
            Some(only_uninitialized),
            Some(false),
            Some(true),
        ) {
            Ok(value) => serde_json::from_value::<TaskModel>(value)
                .map_err(|e| TorcError::ApiError(format!("Failed to parse task response: {}", e))),
            Err(err) => Err(TorcError::ApiError(err.to_string())),
        }
    }

    /// Check for existing output files and optionally delete them.
    /// If delete_files is true, deletes the files. If false, logs warnings only.
    pub fn cleanup_output_files(&self, delete_files: bool) -> Result<(), TorcError> {
        info!(
            "Checking for existing output files for workflow {}",
            self.workflow_id
        );

        // Get all output files for this workflow
        let params = FileListParams::new().with_is_output(true);
        let files_iterator = iter_files(&self.config, self.workflow_id, params);

        // Collect files that exist on filesystem
        let mut existing_files = Vec::new();
        for file_result in files_iterator {
            match file_result {
                Ok(file) => {
                    let file_path = Path::new(&file.path);
                    if file_path.exists() {
                        existing_files.push(file);
                    }
                }
                Err(err) => {
                    warn!(
                        "Failed to fetch file from API during cleanup check: {}",
                        err
                    );
                }
            }
        }

        if existing_files.is_empty() {
            info!("No existing output files found");
            return Ok(());
        }

        // If not deleting, just log warnings and return
        if !delete_files {
            warn!(
                "Found {} existing output file(s) that may be overwritten:",
                existing_files.len()
            );
            for file in &existing_files {
                warn!("  - {}", file.path);
            }
            return Ok(());
        }

        // Delete the files
        let mut deleted_count = 0;
        let mut failed_deletions = Vec::new();

        for file in &existing_files {
            let file_path = Path::new(&file.path);
            match fs::remove_file(file_path) {
                Ok(_) => {
                    info!("Deleted output file: {}", file.path);
                    deleted_count += 1;

                    // Update database to reflect that file no longer exists
                    if let Some(file_id) = file.id {
                        let mut updated_file = file.clone();
                        updated_file.st_mtime = None;

                        match apis::files_api::update_file(&self.config, file_id, updated_file) {
                            Ok(_) => {
                                debug!(
                                    "Updated st_mtime to None for deleted file {} (id: {})",
                                    file.path, file_id
                                );
                            }
                            Err(err) => {
                                warn!(
                                    "Failed to update st_mtime for deleted file {} (id: {}): {}",
                                    file.path, file_id, err
                                );
                            }
                        }
                    }
                }
                Err(err) => {
                    error!("Failed to delete {}: {}", file.path, err);
                    failed_deletions.push((file.path.clone(), err.to_string()));
                }
            }
        }

        info!(
            "Deleted {} of {} output files",
            deleted_count,
            existing_files.len()
        );

        if !failed_deletions.is_empty() {
            error!("Failed to delete {} file(s)", failed_deletions.len());
            for (path, err) in failed_deletions {
                error!("  - {}: {}", path, err);
            }
        }

        Ok(())
    }

    pub fn get_run_id(&self) -> Result<i64, TorcError> {
        match apis::workflows_api::get_workflow(&self.config, self.workflow_id) {
            Ok(workflow) => Ok(workflow.run_id.unwrap_or(0)),
            Err(err) => Err(TorcError::ApiError(err.to_string())),
        }
    }

    /// Check that the workflow is not archived and has no missing user data.
    pub fn check_workflow(&self) -> Result<(), TorcError> {
        match apis::workflows_api::get_workflow(&self.config, self.workflow_id) {
            Ok(workflow) => {
                if workflow.is_archived.unwrap_or(false) {
                    return Err(TorcError::OperationNotAllowed(format!(
                        "Workflow {} is archived",
                        self.workflow_id
                    )));
                }
            }
            Err(err) => return Err(TorcError::ApiError(err.to_string())),
        }
        self.check_user_data()?;
        Ok(())
    }

    pub fn check_user_data(&self) -> Result<(), TorcError> {
        match apis::workflows_api::list_missing_user_data(&self.config, self.workflow_id) {
            Ok(response) => {
                if !response.user_data.is_empty() {
                    let missing_ids = response
                        .user_data
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    error!(
                        "Missing user data for workflow {}: IDs [{}]",
                        self.workflow_id, missing_ids
                    );
                    return Err(TorcError::OperationNotAllowed(format!(
                        "Missing user data for workflow {}: IDs [{}]",
                        self.workflow_id, missing_ids
                    )));
                }
                Ok(())
            }
            Err(err) => Err(TorcError::ApiError(err.to_string())),
        }
    }

    /// Change all uninitialized jobs to the ready state.
    pub fn initialize_jobs(&self, only_uninitialized: bool) -> Result<(), TorcError> {
        match apis::workflows_api::initialize_jobs(
            &self.config,
            self.workflow_id,
            Some(only_uninitialized),
            Some(false),
            Some(false),
        ) {
            Ok(_) => {
                info!(
                    "Changed all uninitialized jobs to ready or blocked for workflow {}",
                    self.workflow_id
                );

                // Create RO-Crate entities for input files if enabled
                self.create_ro_crate_entities_for_input_files();

                // Always create SoftwareApplication entities for torc binaries
                match self.get_run_id() {
                    Ok(run_id) => {
                        crate::client::ro_crate_utils::create_software_entities(
                            &self.config,
                            self.workflow_id,
                            run_id,
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Skipping RO-Crate software entity creation for workflow {}: failed to get run_id: {}",
                            self.workflow_id, e
                        );
                    }
                }

                Ok(())
            }
            Err(err) => Err(TorcError::ApiError(err.to_string())),
        }
    }

    /// Create RO-Crate entities for input files if `enable_ro_crate` is enabled on the workflow.
    ///
    /// Input files are identified as files with `st_mtime` set (they exist before the workflow runs).
    /// This is a non-blocking operation - warnings are logged but errors don't fail initialization.
    fn create_ro_crate_entities_for_input_files(&self) {
        // Fetch workflow to check enable_ro_crate flag
        let workflow = match apis::workflows_api::get_workflow(&self.config, self.workflow_id) {
            Ok(workflow) => workflow,
            Err(e) => {
                debug!(
                    "Could not fetch workflow {} for RO-Crate check: {}",
                    self.workflow_id, e
                );
                return;
            }
        };

        // Check if RO-Crate is enabled
        if workflow.enable_ro_crate != Some(true) {
            return;
        }

        info!(
            "Creating RO-Crate entities for input files in workflow {}",
            self.workflow_id
        );

        let run_id = match self.get_run_id() {
            Ok(run_id) => run_id,
            Err(e) => {
                warn!(
                    "Skipping RO-Crate workflow provenance creation for workflow {}: failed to get run_id: {}",
                    self.workflow_id, e
                );
                return;
            }
        };
        crate::client::ro_crate_utils::create_workflow_provenance_entities(
            &self.config,
            self.workflow_id,
            run_id,
            &workflow.name,
        );

        // st_mtime is also set on output files once their job completes, so exclude any file
        // a job produces; otherwise re-initializing after a run records outputs as inputs.
        // A partial list would misclassify outputs, so skip input entities on any error.
        let output_file_ids: std::collections::HashSet<i64> = match iter_files(
            &self.config,
            self.workflow_id,
            FileListParams::new().with_is_output(true),
        )
        .collect::<Result<Vec<_>, _>>()
        {
            Ok(files) => files.into_iter().filter_map(|f| f.id).collect(),
            Err(e) => {
                warn!(
                    "Skipping RO-Crate input entities for workflow {}: failed to list output files: {}",
                    self.workflow_id, e
                );
                return;
            }
        };

        // Collect all files with st_mtime set (input files)
        let params = FileListParams::new();
        let files_iterator = iter_files(&self.config, self.workflow_id, params);

        let mut input_files: Vec<FileModel> = Vec::new();
        for file_result in files_iterator {
            match file_result {
                Ok(file) => {
                    // Input files have st_mtime set and are not produced by a job
                    if file.st_mtime.is_some()
                        && !file.id.is_some_and(|id| output_file_ids.contains(&id))
                    {
                        input_files.push(file);
                    }
                }
                Err(e) => {
                    warn!("Error iterating files for RO-Crate creation: {}", e);
                }
            }
        }

        // Create entities for input files
        if !input_files.is_empty() {
            debug!(
                "Creating RO-Crate entities for {} input files",
                input_files.len()
            );
            crate::client::ro_crate_utils::create_entities_for_input_files(
                &self.config,
                self.workflow_id,
                &input_files,
            );
        }
    }

    /// Reinitialize jobs. Account for jobs that are new or have been reset.
    /// If force is false, return an error if required input files are missing.
    pub fn reinitialize_jobs(&self, force: bool, dry_run: bool) -> Result<(), TorcError> {
        let snapshot = self.snapshot_files(force || dry_run)?;
        self.reinitialize_jobs_from(&snapshot, force, dry_run)
    }

    fn reinitialize_jobs_from(
        &self,
        snapshot: &FileSnapshot,
        force: bool,
        dry_run: bool,
    ) -> Result<(), TorcError> {
        self.sync_file_stats(snapshot, force, dry_run)?;
        self.process_changed_user_data(dry_run)?;
        if !dry_run {
            self.initialize_jobs(true)?;
        }
        Ok(())
    }

    /// Send the workflow's file stats to the server, which stores changed mtimes and resets
    /// affected jobs and their downstream jobs. Logs what changed (or would change, for a dry
    /// run). Returns an error if required input files are missing, unless force is true.
    fn sync_file_stats(
        &self,
        snapshot: &FileSnapshot,
        force: bool,
        dry_run: bool,
    ) -> Result<(), TorcError> {
        let response = self.send_file_stats(snapshot, dry_run)?;
        let prefix = if dry_run {
            "Dry run: would reset"
        } else {
            "Reset"
        };
        for change in &response.changed_files {
            let msg = format!(
                "File {} {}",
                describe_file(&snapshot.files, change.file_id),
                change.reason
            );
            if dry_run {
                info!("Dry run: {}", msg);
            } else {
                debug!("{}", msg);
            }
        }
        for job in &response.reset_jobs {
            info!(
                "{} job_id={} name='{}' from status={} to uninitialized ({})",
                prefix, job.job_id, job.name, job.status, job.reason
            );
        }

        self.check_missing_required_files(&response, &snapshot.files, force)
    }

    /// Stat the workflow's files before anything is changed. Unless force is true, fail early
    /// if required input files are missing, using a dry run so the workflow is left untouched.
    fn snapshot_files(&self, force: bool) -> Result<FileSnapshot, TorcError> {
        let snapshot = self.stat_files()?;
        if !force {
            let response = self.send_file_stats(&snapshot, true)?;
            self.check_missing_required_files(&response, &snapshot.files, false)?;
        }
        Ok(snapshot)
    }

    /// Return an error listing missing required input files, or log them if force is true.
    fn check_missing_required_files(
        &self,
        response: &models::FileStatsResponse,
        files: &HashMap<i64, FileModel>,
        force: bool,
    ) -> Result<(), TorcError> {
        if response.missing_required_file_ids.is_empty() {
            return Ok(());
        }
        let missing: Vec<String> = response
            .missing_required_file_ids
            .iter()
            .map(|id| format!("Required file {} does not exist", describe_file(files, *id)))
            .collect();
        if !force {
            return Err(TorcError::OperationNotAllowed(format!(
                "Missing required files:\n{}",
                missing.join("\n")
            )));
        }
        for msg in &missing {
            error!("{}", msg);
        }
        error!(
            "Found {} missing required files for workflow {} (ignored due to force=true)",
            missing.len(),
            self.workflow_id
        );
        Ok(())
    }

    /// Stat the workflow's files and send the snapshot to the server's `update_file_stats`
    /// endpoint. Returns the merged responses and the files by ID.
    pub fn update_file_stats(
        &self,
        dry_run: bool,
    ) -> Result<(models::FileStatsResponse, HashMap<i64, FileModel>), TorcError> {
        let snapshot = self.stat_files()?;
        let response = self.send_file_stats(&snapshot, dry_run)?;
        Ok((response, snapshot.files))
    }

    /// List the workflow's files once and stat each path.
    fn stat_files(&self) -> Result<FileSnapshot, TorcError> {
        let mut files = HashMap::new();
        let mut entries = Vec::new();
        for file in iter_files(&self.config, self.workflow_id, FileListParams::new()) {
            let file =
                file.map_err(|e| TorcError::ApiError(format!("Failed to list files: {}", e)))?;
            let id = file
                .id
                .ok_or_else(|| TorcError::ApiError(format!("File '{}' has no ID", file.name)))?;
            // Any metadata error counts as missing, as Path::exists does.
            let entry = match fs::metadata(&file.path) {
                Ok(metadata) => {
                    let mtime = metadata
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .ok_or_else(|| {
                            TorcError::OperationNotAllowed(format!(
                                "Cannot read modification time of {}",
                                file.path
                            ))
                        })?;
                    models::FileStatEntry {
                        id,
                        exists: true,
                        st_mtime: Some(mtime.as_secs_f64()),
                    }
                }
                Err(_) => models::FileStatEntry {
                    id,
                    exists: false,
                    st_mtime: None,
                },
            };
            entries.push(entry);
            files.insert(id, file);
        }
        Ok(FileSnapshot { entries, files })
    }

    /// Send a snapshot to `update_file_stats` in batches of up to `MAX_RECORD_TRANSFER_COUNT`.
    /// Each batch is applied in its own transaction and is idempotent, so a rerun after a
    /// failure reaches the same state.
    fn send_file_stats(
        &self,
        snapshot: &FileSnapshot,
        dry_run: bool,
    ) -> Result<models::FileStatsResponse, TorcError> {
        let mut merged = models::FileStatsResponse::default();
        for batch in snapshot.entries.chunks(MAX_RECORD_TRANSFER_COUNT as usize) {
            let request = models::FileStatsRequest {
                files: batch.to_vec(),
            };
            let response = apis::files_api::update_file_stats(
                &self.config,
                self.workflow_id,
                request,
                Some(dry_run),
            )
            .map_err(|e| self.file_stats_error(e))?;
            merged.changed_files.extend(response.changed_files);
            merged.reset_jobs.extend(response.reset_jobs);
            merged
                .missing_required_file_ids
                .extend(response.missing_required_file_ids);
        }
        Ok(merged)
    }

    /// A 404 from a server that predates `update_file_stats` gets an upgrade hint; a 404
    /// for a missing workflow stays as is.
    fn file_stats_error<T: std::fmt::Debug>(&self, err: apis::Error<T>) -> TorcError {
        if let apis::Error::ResponseError(ref content) = err
            && content.status == reqwest::StatusCode::NOT_FOUND
            && !version_check::check_version(&self.config).server_supports_file_stats()
        {
            return TorcError::OperationNotAllowed(
                "The torc server is too old for this client: it has no file_stats endpoint \
                 (HTTP API 1.2.0). Upgrade torc-server."
                    .to_string(),
            );
        }
        TorcError::ApiError(format!(
            "Failed to update file stats for workflow_id={}: {}",
            self.workflow_id, err
        ))
    }

    /// Process changed user_data by detecting jobs with changed inputs.
    /// Calls the server's process_changed_job_inputs endpoint which computes
    /// input hashes and resets jobs to Uninitialized if inputs have changed.
    /// If dry_run is true, log required changes but do not apply them.
    pub fn process_changed_user_data(&self, dry_run: bool) -> Result<(), TorcError> {
        debug!(
            "Processing changed user_data for workflow {}",
            self.workflow_id
        );

        match apis::workflows_api::process_changed_job_inputs(
            &self.config,
            self.workflow_id,
            Some(dry_run),
        ) {
            Ok(response) => {
                if !response.reinitialized_jobs.is_empty() {
                    let reinitialized_jobs = &response.reinitialized_jobs;
                    if dry_run {
                        info!(
                            "Dry run: {} jobs would be reset due to changed inputs",
                            reinitialized_jobs.len()
                        );
                        for job_name in reinitialized_jobs {
                            info!("  - {}", job_name);
                        }
                    } else {
                        info!(
                            "Reset {} jobs due to changed inputs",
                            reinitialized_jobs.len()
                        );
                    }
                } else {
                    debug!("No jobs need to be reset due to changed inputs");
                }
                Ok(())
            }
            Err(err) => {
                error!(
                    "Failed to process changed job inputs for workflow {}: {}",
                    self.workflow_id, err
                );
                Err(TorcError::ApiError(err.to_string()))
            }
        }
    }
}
