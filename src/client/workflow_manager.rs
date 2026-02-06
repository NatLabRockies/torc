use crate::client::apis::configuration::Configuration;
use crate::client::apis::default_api;
use crate::client::errors::TorcError;
use crate::config::TorcConfig;
use log::{self, debug, error, info, warn};

use crate::client::commands::pagination::{FileListParams, JobListParams, iter_files, iter_jobs};
use crate::models::{FileModel, JobStatus, WorkflowModel};
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

#[derive(Debug, serde::Serialize)]
pub struct InitializationCheck {
    pub safe: bool,
    pub missing_input_files: Vec<String>,
    pub existing_output_files: Vec<String>,
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

    /// Get the modification time of a file as seconds since Unix epoch.
    /// Panics if the modification time cannot be read.
    fn get_modified_file_time(metadata: &fs::Metadata) -> f64 {
        let system_time = metadata
            .modified()
            .expect("Failed to get modification time");
        let duration = system_time
            .duration_since(UNIX_EPOCH)
            .expect("File has modification time before Unix epoch");
        duration.as_secs_f64()
    }

    /// Check if initialization is safe to run without executing.
    /// Returns information about missing input files and existing output files.
    pub fn check_initialization(&self) -> Result<InitializationCheck, TorcError> {
        // Check for missing required input files
        let missing_input_files = self.get_missing_required_files()?;

        // Check for existing output files
        let existing_output_files = self.get_existing_output_files()?;

        // Safe if no missing input files (existing output files are just warnings)
        let safe = missing_input_files.is_empty();

        Ok(InitializationCheck {
            safe,
            missing_input_files,
            existing_output_files,
        })
    }

    /// Get list of missing required input files (file paths)
    fn get_missing_required_files(&self) -> Result<Vec<String>, TorcError> {
        let response =
            match default_api::list_required_existing_files(&self.config, self.workflow_id) {
                Ok(response) => response,
                Err(err) => {
                    return Err(TorcError::ApiError(format!(
                        "Failed to list required existing files: {}",
                        err
                    )));
                }
            };

        let mut missing_files = Vec::new();

        for file_id in response.files {
            let file = match default_api::get_file(&self.config, file_id) {
                Ok(file) => file,
                Err(err) => {
                    return Err(TorcError::ApiError(format!(
                        "Failed to get file details for ID {}: {}",
                        file_id, err
                    )));
                }
            };

            let file_path = Path::new(&file.path);
            if !file_path.exists() {
                missing_files.push(file.path.clone());
            }
        }

        Ok(missing_files)
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
        self.check_workflow(force)?;
        self.cleanup_output_files(false)?;
        match default_api::reset_workflow_status(&self.config, self.workflow_id, None, None) {
            Ok(_) => {}
            Err(err) => {
                error!(
                    "Failed to reset status of workflow_id={}: {}",
                    self.workflow_id, err
                );
                return Err(TorcError::ApiError(err.to_string()));
            }
        }
        match default_api::reset_job_status(&self.config, self.workflow_id, Some(false), None) {
            Ok(_) => {}
            Err(err) => {
                error!(
                    "Failed to reset job status of workflow_id={}: {}",
                    self.workflow_id, err
                );
                return Err(TorcError::ApiError(err.to_string()));
            }
        }
        let _run_id = self.bump_run_id()?;
        self.initialize_files()?;
        self.initialize_jobs(false)?;
        // Event is now broadcast via SSE from the server
        Ok(())
    }

    /// Start the workflow: initialize if needed and schedule nodes for on_workflow_start actions
    pub fn start(&self, force: bool) -> Result<(), TorcError> {
        // Check if workflow is uninitialized
        match default_api::is_workflow_uninitialized(&self.config, self.workflow_id) {
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

        // Get pending on_workflow_start actions
        // Note: We don't create a compute node for the submission host (login node)
        // since it's not actually running jobs - it just submits to the scheduler
        let actions = match default_api::get_pending_actions(
            &self.config,
            self.workflow_id,
            Some(vec!["on_workflow_start".to_string()]),
        ) {
            Ok(actions) => actions,
            Err(err) => {
                error!("Failed to get pending actions: {}", err);
                return Err(TorcError::ApiError(err.to_string()));
            }
        };

        // Filter for schedule_nodes actions
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
                        "Scheduling Slurm nodes for on_workflow_start action {}",
                        action_id
                    );

                    let scheduler_id = action_config
                        .get("scheduler_id")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let num_allocations = action_config
                        .get("num_allocations")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(1) as i32;
                    let start_one_worker_per_node = action_config
                        .get("start_one_worker_per_node")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let max_parallel_jobs = action_config
                        .get("max_parallel_jobs")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32);

                    match crate::client::commands::slurm::schedule_slurm_nodes(
                        &self.config,
                        self.workflow_id,
                        scheduler_id,
                        num_allocations,
                        "",
                        "output",
                        self.torc_config.client.slurm.poll_interval,
                        max_parallel_jobs,
                        start_one_worker_per_node,
                        self.torc_config.client.slurm.keep_submission_scripts,
                    ) {
                        Ok(()) => {
                            info!(
                                "Successfully scheduled {} Slurm allocation(s) for on_workflow_start",
                                num_allocations
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

        Ok(())
    }

    /// Reinitialize the workflow. Reset workflow status, bump run_id, and run startup script.
    pub fn reinitialize(&self, force: bool, dry_run: bool) -> Result<(), TorcError> {
        self.check_workflow(force)?;
        if !dry_run {
            self.bump_run_id()?;
            match default_api::reset_workflow_status(&self.config, self.workflow_id, None, None) {
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
        self.reinitialize_jobs(dry_run)?;
        // Event is now broadcast via SSE from the server
        Ok(())
    }

    /// Increment the run_id field of the workflow.
    pub fn bump_run_id(&self) -> Result<i64, TorcError> {
        match default_api::get_workflow_status(&self.config, self.workflow_id) {
            Ok(status) => {
                let mut new_status = status.clone();
                new_status.run_id += 1;
                let new_run_id = new_status.run_id;
                match default_api::update_workflow_status(
                    &self.config,
                    self.workflow_id,
                    new_status,
                ) {
                    Ok(_) => Ok(new_run_id),
                    Err(err) => Err(TorcError::ApiError(err.to_string())),
                }
            }
            Err(err) => Err(TorcError::ApiError(err.to_string())),
        }
    }

    /// Initialize the file stats in the database.
    pub fn initialize_files(&self) -> Result<(), TorcError> {
        info!("Initializing files for workflow {}", self.workflow_id);

        // Create file list parameters
        let params = FileListParams::new();

        // Iterate through all files for this workflow using iter_files
        let files_iterator = iter_files(&self.config, self.workflow_id, params);

        for file_result in files_iterator {
            match file_result {
                Ok(mut file) => {
                    let file_id = file.id.expect("File ID must be set for existing files");
                    let file_path = Path::new(&file.path);
                    if !file_path.exists() {
                        continue;
                    }
                    match fs::metadata(file_path) {
                        Ok(metadata) => {
                            let mtime = Self::get_modified_file_time(&metadata);

                            // Update the file record if the mtime has changed or is not set
                            let needs_update = match file.st_mtime {
                                Some(current_mtime) => (current_mtime - mtime).abs() > 0.01, // Allow for filesystem timestamp precision differences (10ms)
                                None => true, // Always update if no mtime is set
                            };

                            if needs_update {
                                file.st_mtime = Some(mtime);

                                match default_api::update_file(&self.config, file_id, file.clone())
                                {
                                    Ok(_) => {
                                        info!(
                                            "Updated file {} (id: {}) with mtime: {}",
                                            file.name, file_id, mtime
                                        );
                                    }
                                    Err(err) => {
                                        panic!(
                                            "Failed to update file {} (id: {}): {}",
                                            file.name, file_id, err
                                        );
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            panic!("Failed to get metadata for file {}: {}", file.path, err);
                        }
                    }
                }
                Err(err) => {
                    panic!("Failed to fetch file from API: {}", err);
                }
            }
        }
        Ok(())
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

                        match default_api::update_file(&self.config, file_id, updated_file) {
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
        match default_api::get_workflow_status(&self.config, self.workflow_id) {
            Ok(status) => Ok(status.run_id),
            Err(err) => Err(TorcError::ApiError(err.to_string())),
        }
    }

    /// Check the condtions of the workflow.
    pub fn check_workflow(&self, force: bool) -> Result<(), TorcError> {
        match default_api::get_workflow_status(&self.config, self.workflow_id) {
            Ok(status) => {
                if status.is_archived.unwrap_or(false) {
                    return Err(TorcError::OperationNotAllowed(format!(
                        "Workflow {} is archived",
                        self.workflow_id
                    )));
                }
            }
            Err(err) => return Err(TorcError::ApiError(err.to_string())),
        }
        self.check_workflow_files(force)?;
        self.check_user_data()?;
        Ok(())
    }

    pub fn check_user_data(&self) -> Result<(), TorcError> {
        match default_api::list_missing_user_data(&self.config, self.workflow_id) {
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
        match default_api::initialize_jobs(
            &self.config,
            self.workflow_id,
            Some(only_uninitialized),
            Some(false),
            None,
        ) {
            Ok(_) => {
                info!(
                    "Changed all uninitialized jobs to ready or blocked for workflow {}",
                    self.workflow_id
                );
                Ok(())
            }
            Err(err) => Err(TorcError::ApiError(err.to_string())),
        }
    }

    /// Reinitialize jobs. Account for jobs that are new or have been reset.
    pub fn reinitialize_jobs(&self, dry_run: bool) -> Result<(), TorcError> {
        self.process_changed_files(dry_run)?;
        self.update_jobs_if_output_files_are_missing(dry_run)?;
        self.process_changed_user_data(dry_run)?;
        if !dry_run {
            self.initialize_jobs(true)?;
        }
        Ok(())
    }

    /// Update files in the database that have changed.
    /// If dry_run is true, log required changes but do not apply them.
    pub fn process_changed_files(&self, dry_run: bool) -> Result<(), TorcError> {
        debug!("Processing changed files for workflow {}", self.workflow_id);

        // Create file list parameters
        let params = FileListParams::new();

        // Iterate through all files for this workflow using iter_files
        let files_iterator = iter_files(&self.config, self.workflow_id, params);

        for file_result in files_iterator {
            match file_result {
                Ok(mut file) => {
                    let file_id = file.id.expect("File ID must be set for existing files");
                    let file_path = Path::new(&file.path);

                    debug!(
                        "Processing file {} (id: {}, path: {}, current st_mtime: {:?})",
                        file.name, file_id, file.path, file.st_mtime
                    );

                    let mut file_changed = false;
                    let mut change_reason = String::new();

                    let file_exists = file_path.exists();
                    let db_has_mtime = file.st_mtime.is_some();

                    match (file_exists, db_has_mtime) {
                        (true, true) => {
                            // File exists, database has mtime - check for changes
                            match fs::metadata(file_path) {
                                Ok(metadata) => {
                                    let mtime = Self::get_modified_file_time(&metadata);
                                    let current_mtime = file.st_mtime.unwrap();

                                    if (current_mtime - mtime).abs() > 0.01 {
                                        file_changed = true;
                                        change_reason = format!(
                                            "modified time changed from {} to {}",
                                            current_mtime, mtime
                                        );
                                        file.st_mtime = Some(mtime);
                                    }
                                }
                                Err(err) => {
                                    panic!(
                                        "Failed to get metadata for file {} (id: {}): {}",
                                        file.name, file_id, err
                                    );
                                }
                            }
                        }
                        (true, false) => {
                            // File exists, database has no mtime - file appeared
                            match fs::metadata(file_path) {
                                Ok(metadata) => {
                                    let mtime = Self::get_modified_file_time(&metadata);
                                    file_changed = true;
                                    change_reason = format!("file appeared with mtime {}", mtime);
                                    file.st_mtime = Some(mtime);
                                }
                                Err(err) => {
                                    panic!(
                                        "Failed to get metadata for file {} (id: {}): {}",
                                        file.name, file_id, err
                                    );
                                }
                            }
                        }
                        (false, true) => {
                            // File doesn't exist, database has mtime - file disappeared
                            file_changed = true;
                            change_reason =
                                format!("file disappeared (was mtime {})", file.st_mtime.unwrap());
                            file.st_mtime = None;
                        }
                        (false, false) => {
                            // File doesn't exist, database has no mtime - no change
                            debug!(
                                "File {} (id: {}) doesn't exist and database has no mtime - no change needed",
                                file.name, file_id
                            );
                        }
                    }

                    if file_changed {
                        if dry_run {
                            info!(
                                "Dry run: File {} (id: {}) has changed: {}",
                                file.name, file_id, change_reason
                            );
                        } else {
                            match default_api::update_file(&self.config, file_id, file.clone()) {
                                Ok(_) => {
                                    debug!(
                                        "Updated file {} (id: {}) - {}",
                                        file.name, file_id, change_reason
                                    );
                                }
                                Err(err) => {
                                    panic!(
                                        "Failed to update file {} (id: {}): {}",
                                        file.name, file_id, err
                                    );
                                }
                            }
                        }

                        match self.update_jobs_on_file_change(file.clone(), dry_run) {
                            Ok(_) => {}
                            Err(err) => {
                                panic!(
                                    "Failed to update jobs for file {} (id: {}): {}",
                                    file.name, file_id, err
                                );
                            }
                        }
                    }
                }
                Err(err) => {
                    panic!("Failed to fetch file from API: {}", err);
                }
            }
        }

        Ok(())
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

        match default_api::process_changed_job_inputs(
            &self.config,
            self.workflow_id,
            Some(dry_run),
            None,
        ) {
            Ok(response) => {
                if let Some(ref reinitialized_jobs) = response.reinitialized_jobs {
                    if !reinitialized_jobs.is_empty() {
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

    /// Update job status in the database based on a file change.
    /// If dry_run is true, log required changes but do not apply them.
    pub fn update_jobs_on_file_change(
        &self,
        file: FileModel,
        dry_run: bool,
    ) -> Result<(), TorcError> {
        // First, find the current workflow's run_id and store in a variable
        let run_id = self.get_run_id()?;

        // Check if file.id is set, return an error if not
        let file_id = match file.id {
            Some(id) => id,
            None => {
                return Err(TorcError::OperationNotAllowed(
                    "File ID is not set, cannot update jobs on file change".to_string(),
                ));
            }
        };

        // Create job list parameters with needs_file_id filter
        let params = JobListParams::new().with_needs_file_id(file_id);

        // Iterate over the affected jobs using iter_jobs
        for job_result in iter_jobs(&self.config, self.workflow_id, params) {
            let job = match job_result {
                Ok(job) => job,
                Err(err) => {
                    error!("Failed to fetch job from API: {}", err);
                    return Err(TorcError::ApiError(format!(
                        "Failed to list jobs for file {}: {}",
                        file_id, err
                    )));
                }
            };
            let job_id = match job.id {
                Some(id) => id,
                None => {
                    warn!("Job has no ID, skipping");
                    continue;
                }
            };

            let job_status = match &job.status {
                Some(status) => status,
                None => {
                    warn!("Job {} has no status, skipping", job_id);
                    continue;
                }
            };

            // Check if job's status is Completed, Failed, or Canceled
            match job_status {
                JobStatus::Completed | JobStatus::Failed | JobStatus::Canceled => {
                    if dry_run {
                        // If dry run is true, just log the change
                        info!(
                            "Dry run: Would reset job {} (name: '{}') from {:?} to Uninitialized due to file change in {} (id: {})",
                            job_id, &job.name, job_status, file.name, file_id
                        );

                        // TODO
                        // Find and log direct downstream jobs (jobs that depend on this job)
                        // Note: This only finds direct dependencies, not transitive ones.
                        // A server-side recursive query would be more efficient for full traversal.
                        let downstream_params = JobListParams::new().with_upstream_job_id(job_id);
                        for downstream_job_result in
                            iter_jobs(&self.config, self.workflow_id, downstream_params)
                        {
                            let downstream_job = match downstream_job_result {
                                Ok(job) => job,
                                Err(err) => {
                                    panic!("Failed to fetch downstream job: {}", err);
                                }
                            };

                            let downstream_job_id = match downstream_job.id {
                                Some(id) => id,
                                None => continue,
                            };

                            info!(
                                "Dry run: Would reset downstream job {} (name: '{}' status: {:?}) to Uninitialized",
                                downstream_job_id, &downstream_job.name, downstream_job.status
                            );
                        }
                    } else {
                        match default_api::manage_status_change(
                            &self.config,
                            job_id,
                            JobStatus::Uninitialized,
                            run_id,
                            None, // body
                        ) {
                            Ok(_) => {
                                info!(
                                    "Reset job {} (name: '{}') from {:?} to Uninitialized due to file change in {} (id: {})",
                                    job_id, &job.name, job_status, file.name, file_id
                                );
                            }
                            Err(err) => {
                                panic!(
                                    "Failed to reset job {} status due to file change: {}",
                                    job_id, err
                                );
                            }
                        }
                    }
                }
                _ => {
                    // Job is not Completed, Failed, or Canceled, no action needed
                    debug!(
                        "Job {} (name: '{}') has status {:?}, no reset needed for file change in {} (id: {})",
                        job_id, &job.name, job_status, file.name, file_id
                    );
                }
            }
        }

        Ok(())
    }

    /// Update the status of "completed" jobs to "uninitialized" if their output files are now missing.
    /// If dry_run is true, log changes but don't apply them.
    pub fn update_jobs_if_output_files_are_missing(&self, dry_run: bool) -> Result<(), TorcError> {
        let run_id = self.get_run_id()?;

        let job_params = JobListParams::new().with_status(JobStatus::Completed);
        for job_result in iter_jobs(&self.config, self.workflow_id, job_params) {
            let job = match job_result {
                Ok(job) => job,
                Err(err) => {
                    panic!("Failed to fetch job from API: {}", err);
                }
            };

            let job_id = match job.id {
                Some(id) => id,
                None => {
                    panic!("Job has no ID, skipping");
                }
            };

            let file_params = FileListParams::new().with_produced_by_job_id(job_id);
            let mut any_missing_files = false;

            for file_result in iter_files(&self.config, self.workflow_id, file_params) {
                let file = match file_result {
                    Ok(file) => file,
                    Err(err) => {
                        panic!("Failed to fetch file from API: {}", err);
                    }
                };

                let file_path = Path::new(&file.path);
                if !file_path.exists() {
                    any_missing_files = true;
                    debug!(
                        "Output file {} from job {} (name: '{}') is missing",
                        file.path, job_id, &job.name
                    );
                    break; // No need to check more files for this job
                }
            }

            // If any output file is missing, handle the job status change
            if any_missing_files {
                if dry_run {
                    info!(
                        "Dry run: Would reset job {} (name: '{}') from Done to Uninitialized due to missing output files",
                        job_id, &job.name
                    );

                    // Find and log direct downstream jobs (jobs that depend on this job)
                    // Note: This only finds direct dependencies, not transitive ones.
                    // A server-side recursive query would be more efficient for full traversal.
                    let downstream_params = JobListParams::new().with_upstream_job_id(job_id);
                    for downstream_job_result in
                        iter_jobs(&self.config, self.workflow_id, downstream_params)
                    {
                        let downstream_job = match downstream_job_result {
                            Ok(job) => job,
                            Err(err) => {
                                panic!("Failed to fetch downstream job: {}", err);
                            }
                        };

                        let downstream_job_id = match downstream_job.id {
                            Some(id) => id,
                            None => continue,
                        };

                        info!(
                            "Dry run: Would reset downstream job {} (name: '{}' status: {:?}) to Uninitialized",
                            downstream_job_id, &downstream_job.name, downstream_job.status
                        );
                    }
                } else {
                    match default_api::manage_status_change(
                        &self.config,
                        job_id,
                        JobStatus::Uninitialized,
                        run_id,
                        None, // body
                    ) {
                        Ok(_) => {
                            info!(
                                "Reset job {} (name: '{}') from Done to Uninitialized due to missing output files",
                                job_id, &job.name
                            );
                        }
                        Err(err) => {
                            panic!(
                                "Failed to reset job {} status due to missing output files: {}",
                                job_id, err
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Check that all required existing files for the workflow exist on the filesystem.
    /// If force is true, log missing files as warnings but don't return an error.
    /// If force is false, return an error if any required files are missing.
    pub fn check_workflow_files(&self, force: bool) -> Result<(), TorcError> {
        // Get list of required existing file IDs
        let response =
            match default_api::list_required_existing_files(&self.config, self.workflow_id) {
                Ok(response) => response,
                Err(err) => {
                    return Err(TorcError::ApiError(format!(
                        "Failed to list required existing files: {}",
                        err
                    )));
                }
            };

        let mut missing_files = Vec::new();
        let file_count = response.files.len();

        for file_id in response.files {
            let file = match default_api::get_file(&self.config, file_id) {
                Ok(file) => file,
                Err(err) => {
                    panic!("Failed to get file details for ID {}: {}", file_id, err);
                }
            };

            let file_path = Path::new(&file.path);
            if !file_path.exists() {
                let missing_info = format!(
                    "Required file '{}' (id: {}, path: {}) does not exist on filesystem",
                    file.name, file_id, file.path
                );

                if force {
                    error!("{}", missing_info);
                } else {
                    missing_files.push(missing_info);
                }
            } else {
                debug!(
                    "Required file '{}' (id: {}) exists at path: {}",
                    file.name, file_id, file.path
                );
            }
        }

        // If we have missing files and not ignoring them, return an error
        if !missing_files.is_empty() && !force {
            return Err(TorcError::OperationNotAllowed(format!(
                "Missing required files:\n{}",
                missing_files.join("\n")
            )));
        }

        if missing_files.is_empty() {
            debug!(
                "All {} required existing files are present for workflow {}",
                file_count, self.workflow_id
            );
        } else if force {
            error!(
                "Found {} missing required files for workflow {} (ignored due to force=true)",
                missing_files.len(),
                self.workflow_id
            );
        }

        Ok(())
    }
}
