//! Job Runner - Local parallel job execution engine for Torc workflows.
//!
//! This module provides the [`JobRunner`] struct which manages the execution of workflow jobs
//! on a compute node. It handles job scheduling based on available resources, process lifecycle
//! management, and graceful termination via signal handling.
//!
//! # Signal Handling (SIGTERM)
//!
//! The JobRunner supports graceful termination when running in HPC environments like Slurm.
//! When Slurm is about to reach walltime, it sends SIGTERM to the job runner process. The
//! JobRunner handles this by:
//!
//! 1. **Signal Registration**: External code (e.g., `torc-slurm-job-runner`) registers a signal
//!    handler that sets the termination flag via [`JobRunner::get_termination_flag()`].
//!
//! 2. **Graceful Shutdown**: When the flag is set, the main loop detects it and calls
//!    [`JobRunner::terminate_jobs()`], which:
//!    - Sends SIGTERM to jobs with `supports_termination = true`, allowing them to clean up
//!    - Sends SIGKILL to jobs with `supports_termination = false` (immediate termination)
//!    - Waits for all processes to exit and collects their exit codes
//!    - Sets job status to `JobStatus::Terminated`
//!
//! # Example: Signal Handler Registration
//!
//! ```ignore
//! use signal_hook::consts::SIGTERM;
//! use signal_hook::iterator::Signals;
//! use std::sync::atomic::Ordering;
//! use std::thread;
//!
//! let mut job_runner = JobRunner::new(/* ... */);
//!
//! // Get the termination flag to share with the signal handler
//! let termination_flag = job_runner.get_termination_flag();
//!
//! // Register SIGTERM handler in a background thread
//! let mut signals = Signals::new([SIGTERM]).expect("Failed to register signals");
//! thread::spawn(move || {
//!     for sig in signals.forever() {
//!         if sig == SIGTERM {
//!             termination_flag.store(true, Ordering::SeqCst);
//!             break;
//!         }
//!     }
//! });
//!
//! // Run the job runner - it will check the flag in its main loop
//! job_runner.run_worker()?;
//! ```
//!
//! # Job Termination Behavior
//!
//! Jobs can opt-in to graceful termination by setting `supports_termination = true` in their
//! job specification. This is useful for jobs that need to:
//! - Save checkpoints before exiting
//! - Clean up temporary files
//! - Flush output buffers
//! - Release external resources (database connections, locks, etc.)
//!
//! Jobs without this flag (or with `supports_termination = false`) will be killed immediately
//! with SIGKILL, which doesn't allow cleanup but ensures rapid shutdown.

use chrono::{DateTime, Utc};
use log::{self, debug, error, info, warn};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::client::apis::configuration::Configuration;
use crate::client::apis::default_api;
use crate::client::async_cli_command::AsyncCliCommand;
use crate::client::resource_correction::format_duration_iso8601;
use crate::client::resource_monitor::{ResourceMonitor, ResourceMonitorConfig};
use crate::client::utils;
use crate::config::TorcConfig;
use crate::memory_utils::memory_string_to_gb;
use crate::models::{
    ClaimJobsSortMethod, ComputeNodesResources, JobStatus, ResourceRequirementsModel, ResultModel,
    WorkflowModel,
};

/// Rule definition for failure handler (parsed from JSON stored in database)
#[derive(Debug, Clone, serde::Deserialize)]
pub struct FailureHandlerRule {
    #[serde(default)]
    pub exit_codes: Vec<i32>,
    /// If true, this rule matches any non-zero exit code
    #[serde(default)]
    pub match_all_exit_codes: bool,
    pub recovery_script: Option<String>,
    #[serde(default = "default_max_retries")]
    pub max_retries: i32,
}

fn default_max_retries() -> i32 {
    3
}

/// Result of running the job worker, indicating whether any jobs failed or were terminated.
#[derive(Debug, Default, Clone)]
pub struct WorkerResult {
    /// True if any job failed during execution
    pub had_failures: bool,
    /// True if any job was terminated (e.g., due to SIGTERM or time limit)
    pub had_terminations: bool,
}

/// Outcome of attempting to recover a failed job via failure handler.
#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryOutcome {
    /// Job was successfully scheduled for retry
    Retried,
    /// No failure handler defined for this job - use PendingFailed status
    NoHandler,
    /// Failure handler exists but no rule matched the exit code - use PendingFailed status
    NoMatchingRule,
    /// Max retries exceeded - use Failed status
    MaxRetriesExceeded,
    /// API call or other error - use Failed status
    Error(String),
}

/// Manages parallel job execution on a compute node.
///
/// The JobRunner claims jobs from the server, executes them locally, and reports results.
/// It supports resource-based scheduling (CPU, memory, GPU) and graceful termination
/// via SIGTERM signal handling.
///
/// # Termination Support
///
/// The JobRunner can be gracefully terminated by setting a shared atomic flag. This is
/// typically done from a signal handler when SIGTERM is received (e.g., from Slurm
/// approaching walltime). See the module-level documentation for signal handler setup.
///
/// When termination is requested:
/// - Jobs with `supports_termination = true` receive SIGTERM (graceful shutdown)
/// - Jobs with `supports_termination = false` receive SIGKILL (immediate kill)
/// - All jobs are set to `JobStatus::Terminated`
#[allow(dead_code)]
pub struct JobRunner {
    config: Configuration,
    torc_config: TorcConfig,
    workflow: WorkflowModel,
    pub workflow_id: i64,
    pub run_id: i64,
    compute_node_id: i64,
    output_dir: PathBuf,
    job_completion_poll_interval: f64,
    max_parallel_jobs: Option<i64>,
    time_limit: Option<String>,
    end_time: Option<DateTime<Utc>>,
    resources: ComputeNodesResources,
    orig_resources: ComputeNodesResources,
    scheduler_config_id: Option<i64>,
    log_prefix: Option<String>,
    cpu_affinity_cpus_per_job: Option<i64>,
    is_subtask: bool,
    running_jobs: HashMap<i64, AsyncCliCommand>,
    job_resources: HashMap<i64, ResourceRequirementsModel>,
    rules: ComputeNodeRules,
    resource_monitor: Option<ResourceMonitor>,
    /// Flag set when SIGTERM is received. Shared with signal handler.
    termination_requested: Arc<AtomicBool>,
    /// Monotonic timestamp of when a job was last claimed. Used for idle timeout.
    /// Uses std::time::Instant instead of wall clock time to avoid issues with
    /// NTP clock adjustments that could cause premature idle timeout exits.
    last_job_claimed_time: Option<Instant>,
    /// Tracks whether any job failed during this run
    had_failures: bool,
    /// Tracks whether any job was terminated during this run
    had_terminations: bool,
    /// When this job runner started (for calculating duration_seconds)
    start_instant: Instant,
}

impl JobRunner {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: Configuration,
        workflow: WorkflowModel,
        run_id: i64,
        compute_node_id: i64,
        output_dir: PathBuf,
        job_completion_poll_interval: f64,
        max_parallel_jobs: Option<i64>,
        time_limit: Option<String>,
        end_time: Option<DateTime<Utc>>,
        resources: ComputeNodesResources,
        scheduler_config_id: Option<i64>,
        log_prefix: Option<String>,
        cpu_affinity_cpus_per_job: Option<i64>,
        is_subtask: bool,
        unique_label: String,
    ) -> Self {
        let workflow_id = workflow.id.expect("Workflow ID must be present");
        let running_jobs: HashMap<i64, AsyncCliCommand> = HashMap::new();
        let torc_config = TorcConfig::load().unwrap_or_default();
        let rules = ComputeNodeRules::new(
            workflow.compute_node_expiration_buffer_seconds,
            workflow.compute_node_wait_for_new_jobs_seconds,
            workflow.compute_node_ignore_workflow_completion,
            workflow.compute_node_wait_for_healthy_database_minutes,
            workflow.compute_node_min_time_for_new_jobs_seconds,
            workflow.jobs_sort_method,
        );
        let job_resources: HashMap<i64, ResourceRequirementsModel> = HashMap::new();
        let orig_resources = ComputeNodesResources {
            id: resources.id,
            num_cpus: resources.num_cpus,
            memory_gb: resources.memory_gb,
            num_gpus: resources.num_gpus,
            num_nodes: resources.num_nodes,
            time_limit: resources.time_limit.clone(),
            scheduler_config_id: resources.scheduler_config_id,
        };

        // Initialize resource monitoring if configured
        let resource_monitor = if let Some(ref monitor_config_json) =
            workflow.resource_monitor_config
        {
            match serde_json::from_str::<ResourceMonitorConfig>(monitor_config_json) {
                Ok(monitor_config) if monitor_config.enabled => {
                    match ResourceMonitor::new(monitor_config, output_dir.clone(), unique_label) {
                        Ok(monitor) => {
                            info!("Resource monitoring enabled");
                            Some(monitor)
                        }
                        Err(e) => {
                            error!("Failed to initialize resource monitor: {}", e);
                            None
                        }
                    }
                }
                Ok(_) => None,
                Err(e) => {
                    error!("Failed to parse resource monitor config: {}", e);
                    None
                }
            }
        } else {
            None
        };

        JobRunner {
            config,
            torc_config,
            workflow,
            workflow_id,
            run_id,
            compute_node_id,
            output_dir,
            job_completion_poll_interval,
            max_parallel_jobs,
            time_limit,
            end_time,
            resources,
            orig_resources,
            scheduler_config_id,
            log_prefix,
            cpu_affinity_cpus_per_job,
            is_subtask,
            running_jobs,
            job_resources,
            rules,
            resource_monitor,
            termination_requested: Arc::new(AtomicBool::new(false)),
            last_job_claimed_time: None,
            had_failures: false,
            had_terminations: false,
            start_instant: Instant::now(),
        }
    }

    /// Execute an API call with automatic retries for network errors.
    ///
    /// This is a convenience method that wraps [`utils::send_with_retries`] with
    /// the JobRunner's configuration and retry settings.
    fn send_with_retries<T, E, F>(&self, api_call: F) -> Result<T, E>
    where
        F: FnMut() -> Result<T, E>,
        E: std::fmt::Display,
    {
        utils::send_with_retries(
            &self.config,
            api_call,
            self.rules.compute_node_wait_for_healthy_database_minutes,
        )
    }

    /// Atomically claim a workflow action for execution.
    ///
    /// This is a convenience method that wraps [`utils::claim_action`] with
    /// the JobRunner's configuration and retry settings.
    fn claim_action(&self, action_id: i64) -> Result<bool, Box<dyn std::error::Error>> {
        utils::claim_action(
            &self.config,
            self.workflow_id,
            action_id,
            Some(self.compute_node_id),
            self.rules.compute_node_wait_for_healthy_database_minutes,
        )
    }

    /// Returns a clone of the termination flag for use with signal handlers.
    ///
    /// This method returns an `Arc<AtomicBool>` that can be shared with a signal handler
    /// running in a separate thread. When the flag is set to `true`, the JobRunner's
    /// main loop will detect this and initiate graceful termination of running jobs.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use signal_hook::consts::SIGTERM;
    /// use signal_hook::iterator::Signals;
    /// use std::sync::atomic::Ordering;
    ///
    /// let job_runner = JobRunner::new(/* ... */);
    /// let flag = job_runner.get_termination_flag();
    ///
    /// // In signal handler thread:
    /// flag.store(true, Ordering::SeqCst);
    /// ```
    pub fn get_termination_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.termination_requested)
    }

    /// Checks if termination has been requested.
    ///
    /// Returns `true` if the termination flag has been set, indicating that the
    /// JobRunner should stop accepting new jobs and gracefully terminate running ones.
    pub fn is_termination_requested(&self) -> bool {
        self.termination_requested.load(Ordering::SeqCst)
    }

    /// Requests termination programmatically.
    ///
    /// This method sets the termination flag, causing the JobRunner to initiate
    /// graceful shutdown on its next iteration. This is an alternative to setting
    /// the flag via the `Arc<AtomicBool>` returned by [`get_termination_flag()`].
    ///
    /// Typically, termination is triggered by a signal handler, but this method
    /// allows programmatic termination for testing or other use cases.
    pub fn request_termination(&self) {
        self.termination_requested.store(true, Ordering::SeqCst);
    }

    pub fn run_worker(&mut self) -> Result<WorkerResult, Box<dyn std::error::Error>> {
        use crate::client::version_check;

        let version = version_check::full_version();
        let hostname = hostname::get()
            .expect("Failed to get hostname")
            .into_string()
            .expect("Hostname is not valid UTF-8");
        let end_time = if let Some(end_time) = self.end_time {
            end_time.timestamp() - self.rules.compute_node_expiration_buffer_seconds
        } else {
            i64::MAX
        };

        // Create output directory if it doesn't exist
        if !self.output_dir.exists() {
            std::fs::create_dir_all(&self.output_dir)?;
            info!("Created output directory: {}", self.output_dir.display());
        }

        // Check and log server version
        let version_result = version_check::check_version(&self.config);
        let server_version = version_result
            .server_version
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        info!(
            "Starting torc job runner version={} server_version={} workflow_id={} hostname={} output_dir={} resources={:?} rules={:?} \
            job_completion_poll_interval={}s max_parallel_jobs={:?} end_time={:?} strict_scheduler_match={}",
            version,
            server_version,
            self.workflow_id,
            hostname,
            self.output_dir.display(),
            self.resources,
            self.rules,
            self.job_completion_poll_interval,
            self.max_parallel_jobs,
            self.end_time,
            self.torc_config.client.slurm.strict_scheduler_match
        );

        // Warn about version mismatches
        if version_result.severity.has_warning() {
            version_check::print_version_warning(&version_result);
        }

        // Check for and execute on_workflow_start and on_worker_start actions before entering main loop
        self.execute_workflow_start_actions();
        self.execute_worker_start_actions();

        loop {
            match self.send_with_retries(|| {
                default_api::is_workflow_complete(&self.config, self.workflow_id)
            }) {
                Ok(response) => {
                    if response.is_canceled {
                        info!("Workflow canceled workflow_id={}", self.workflow_id);
                        self.cancel_jobs();
                        break;
                    }
                    if response.is_complete {
                        if self.rules.compute_node_ignore_workflow_completion {
                            info!(
                                "Workflow complete (ignoring) workflow_id={}",
                                self.workflow_id
                            );
                        } else {
                            info!("Workflow complete workflow_id={}", self.workflow_id);
                            self.execute_workflow_complete_actions();
                            break;
                        }
                    }
                }
                Err(retry_err) => {
                    error!(
                        "Failed to check workflow completion after retries: {}",
                        retry_err
                    );
                    return Err(
                        format!("Unable to check workflow completion: {}", retry_err).into(),
                    );
                }
            }

            self.check_job_status();
            self.check_and_execute_actions();

            debug!("Check for new jobs");
            if let Some(max) = self.max_parallel_jobs {
                // Parallelism-based mode: skip if already at max parallel jobs
                if (self.running_jobs.len() as i64) < max {
                    self.run_ready_jobs_based_on_user_parallelism()
                } else {
                    debug!(
                        "Skipping job claim: at max parallel jobs ({}/{})",
                        self.running_jobs.len(),
                        max
                    );
                }
            } else {
                // Resource-based mode: skip if no CPUs available or memory nearly exhausted
                if self.resources.num_cpus > 0 && self.resources.memory_gb >= 0.1 {
                    self.run_ready_jobs_based_on_resources()
                } else {
                    debug!(
                        "Skipping job claim: no capacity (cpus={}, memory_gb={:.2})",
                        self.resources.num_cpus, self.resources.memory_gb
                    );
                }
            }

            thread::sleep(Duration::from_secs_f64(self.job_completion_poll_interval));

            // Check if termination was requested (e.g., via SIGTERM)
            if self.is_termination_requested() {
                info!("Termination requested (SIGTERM received). Terminating jobs.");
                self.terminate_jobs();
                break;
            }

            if Utc::now().timestamp() >= end_time {
                info!("End time reached. Terminating jobs and stopping job runner.");
                self.terminate_jobs();
                break;
            }

            // Check if we should exit due to no new jobs being claimed for too long
            if self.rules.compute_node_wait_for_new_jobs_seconds > 0 && self.running_jobs.is_empty()
            {
                // Initialize the time if this is the first check
                if self.last_job_claimed_time.is_none() {
                    self.last_job_claimed_time = Some(Instant::now());
                }

                // Use monotonic Instant to avoid issues with wall clock time going backwards
                // (e.g., due to NTP synchronization), which could cause spurious idle timeouts
                let idle_seconds = self
                    .last_job_claimed_time
                    .map(|last_time| last_time.elapsed().as_secs())
                    .unwrap_or(0);

                if idle_seconds >= self.rules.compute_node_wait_for_new_jobs_seconds {
                    // Before exiting, check if there are pending actions we can handle
                    // Actions like schedule_nodes might add more compute capacity
                    if self.has_pending_actions_we_can_handle() {
                        debug!(
                            "Idle for {} seconds but pending actions exist, continuing to wait",
                            idle_seconds
                        );
                    } else {
                        info!(
                            "No jobs claimed for {} seconds (limit: {} seconds). Exiting job runner.",
                            idle_seconds, self.rules.compute_node_wait_for_new_jobs_seconds
                        );
                        break;
                    }
                }
            }
        }

        self.execute_worker_complete_actions();

        // Shutdown resource monitor if enabled
        if let Some(monitor) = self.resource_monitor.take() {
            info!("Shutting down resource monitor");
            monitor.shutdown();
        }

        // Deactivate compute node and set duration
        self.deactivate_compute_node();

        info!(
            "Job runner completed workflow_id={} run_id={} compute_node_id={} had_failures={} had_terminations={}",
            self.workflow_id,
            self.run_id,
            self.compute_node_id,
            self.had_failures,
            self.had_terminations
        );
        Ok(WorkerResult {
            had_failures: self.had_failures,
            had_terminations: self.had_terminations,
        })
    }

    /// Deactivate the compute node and set its duration.
    fn deactivate_compute_node(&self) {
        let duration_seconds = self.start_instant.elapsed().as_secs_f64();
        info!(
            "Compute node deactivated workflow_id={} run_id={} compute_node_id={} duration_s={:.1}",
            self.workflow_id, self.run_id, self.compute_node_id, duration_seconds
        );

        // Fetch the existing compute node first to preserve all fields
        let mut update_model =
            match default_api::get_compute_node(&self.config, self.compute_node_id) {
                Ok(node) => node,
                Err(e) => {
                    error!(
                        "Failed to fetch compute node {} for deactivation: {}",
                        self.compute_node_id, e
                    );
                    return;
                }
            };

        // Only update the fields we need to change
        update_model.is_active = Some(false);
        update_model.duration_seconds = Some(duration_seconds);

        if let Err(e) =
            default_api::update_compute_node(&self.config, self.compute_node_id, update_model)
        {
            error!(
                "Failed to deactivate compute node {}: {}",
                self.compute_node_id, e
            );
        }
    }

    /// Cancel all running jobs and handle completions.
    fn cancel_jobs(&mut self) {
        let mut results = Vec::new();
        for (job_id, async_job) in self.running_jobs.iter_mut() {
            info!(
                "Job canceling workflow_id={} job_id={}",
                self.workflow_id, job_id
            );
            let _ = async_job.cancel();
        }
        for (job_id, async_job) in self.running_jobs.iter_mut() {
            let _ = match async_job.wait_for_completion() {
                Ok(_) => {
                    let attempt_id = async_job.job.attempt_id.unwrap_or(1);
                    let result = async_job.get_result(
                        self.run_id,
                        attempt_id,
                        self.compute_node_id,
                        self.resource_monitor.as_ref(),
                    );
                    results.push((*job_id, result));
                    Ok(())
                }
                Err(e) => {
                    error!("Error waiting for job {}: {}", job_id, e);
                    Err(e)
                }
            };
        }
        for (job_id, result) in results {
            self.handle_job_completion(job_id, result);
        }
    }

    /// Terminates all running jobs and reports results to the server.
    ///
    /// This method performs a three-phase termination:
    ///
    /// 1. **Signal Phase**: Send termination signals to all running jobs
    ///    - Jobs with `supports_termination = true` receive SIGTERM, allowing graceful cleanup
    ///    - Jobs with `supports_termination = false` (or unset) receive SIGKILL for immediate termination
    ///
    /// 2. **Wait Phase**: Wait for all jobs to exit and collect their exit codes
    ///    - Exit codes are captured, including negative values for signal-terminated processes
    ///
    /// 3. **Completion Phase**: Report results to the server
    ///    - All terminated jobs are set to `JobStatus::Terminated`
    ///    - Results include execution time and resource metrics (if monitoring is enabled)
    ///
    /// # Job Termination Behavior
    ///
    /// Jobs can opt-in to graceful termination by setting `supports_termination: true` in the
    /// job specification. This is useful for jobs that need to save checkpoints or clean up
    /// resources before exiting. Jobs without this flag are killed immediately to ensure
    /// rapid shutdown when the compute node is about to expire.
    ///
    /// # Note
    ///
    /// This method is called automatically by `run_worker()` when:
    /// - The termination flag is set (typically by a SIGTERM signal handler)
    /// - The compute node's end time is approaching
    fn terminate_jobs(&mut self) {
        if self.running_jobs.is_empty() {
            debug!("No running jobs to terminate");
            return;
        }

        info!(
            "Jobs terminating workflow_id={} count={}",
            self.workflow_id,
            self.running_jobs.len()
        );

        // First pass: send termination signal to all jobs
        // Jobs that support termination get SIGTERM, others get killed immediately
        for (job_id, async_job) in self.running_jobs.iter_mut() {
            let supports_termination = async_job.job.supports_termination.unwrap_or(false);
            if supports_termination {
                info!(
                    "Job SIGTERM workflow_id={} job_id={} supports_termination=true",
                    self.workflow_id, job_id
                );
                if let Err(e) = async_job.terminate() {
                    warn!(
                        "Job SIGTERM failed workflow_id={} job_id={} error={}",
                        self.workflow_id, job_id, e
                    );
                }
            } else {
                info!(
                    "Job SIGKILL workflow_id={} job_id={} supports_termination=false",
                    self.workflow_id, job_id
                );
                if let Err(e) = async_job.cancel() {
                    warn!(
                        "Job SIGKILL failed workflow_id={} job_id={} error={}",
                        self.workflow_id, job_id, e
                    );
                }
            }
        }

        // Second pass: wait for all jobs to complete and collect results
        let mut results = Vec::new();
        for (job_id, async_job) in self.running_jobs.iter_mut() {
            match async_job.wait_for_completion() {
                Ok(exit_code) => {
                    debug!(
                        "Job terminated workflow_id={} job_id={} exit_code={}",
                        self.workflow_id, job_id, exit_code
                    );
                    let attempt_id = async_job.job.attempt_id.unwrap_or(1);
                    let result = async_job.get_result(
                        self.run_id,
                        attempt_id,
                        self.compute_node_id,
                        self.resource_monitor.as_ref(),
                    );
                    results.push((*job_id, result));
                }
                Err(e) => {
                    error!(
                        "Job wait failed workflow_id={} job_id={} error={}",
                        self.workflow_id, job_id, e
                    );
                }
            }
        }

        // Third pass: handle completions (notify server)
        for (job_id, result) in results {
            self.handle_job_completion(job_id, result);
        }
    }

    /// Check the status of running jobs and remove completed ones.
    fn check_job_status(&mut self) {
        let mut completed_jobs = Vec::new();
        let mut job_results = Vec::new();

        // First pass: check status and collect completed jobs
        for (job_id, async_job) in self.running_jobs.iter_mut() {
            match async_job.check_status() {
                Ok(()) => {
                    if async_job.is_complete {
                        completed_jobs.push(*job_id);

                        let attempt_id = async_job.job.attempt_id.unwrap_or(1);
                        let result = async_job.get_result(
                            self.run_id,
                            attempt_id,
                            self.compute_node_id,
                            self.resource_monitor.as_ref(),
                        );

                        // Extract output_file_ids for validation
                        let output_file_ids = async_job.job.output_file_ids.clone();

                        job_results.push((*job_id, result, output_file_ids));
                    }
                }
                Err(e) => {
                    error!("Error checking status for job {}: {}", job_id, e);
                }
            }
        }

        // Second pass: validate output files and complete jobs
        for (job_id, mut result, output_file_ids) in job_results {
            // Validate output files if job completed successfully
            if result.return_code == 0
                && let Err(e) = self.validate_and_update_output_files(job_id, &output_file_ids)
            {
                error!("Output file validation failed for job {}: {}", job_id, e);
                result.return_code = 1;
                result.status = JobStatus::Failed;
            }

            self.handle_job_completion(job_id, result);
        }
    }

    /// Validate that all expected output files exist and update their st_mtime
    fn validate_and_update_output_files(
        &self,
        job_id: i64,
        output_file_ids: &Option<Vec<i64>>,
    ) -> Result<(), String> {
        // Get output file IDs
        let output_file_ids = match output_file_ids {
            Some(ids) if !ids.is_empty() => ids,
            _ => return Ok(()), // No output files to validate
        };

        debug!(
            "Validating {} output files for job {}",
            output_file_ids.len(),
            job_id
        );

        let mut missing_files = Vec::new();
        let mut files_to_update = Vec::new();

        // Fetch file models and check existence
        for file_id in output_file_ids {
            let file_model =
                match self.send_with_retries(|| default_api::get_file(&self.config, *file_id)) {
                    Ok(file) => file,
                    Err(e) => {
                        return Err(format!(
                            "Failed to fetch file model for file_id {}: {}",
                            file_id, e
                        ));
                    }
                };

            let file_path = Path::new(&file_model.path);

            // Check if file exists
            match fs::metadata(file_path) {
                Ok(metadata) => {
                    // File exists - get its modification time
                    match metadata.modified() {
                        Ok(modified) => {
                            let st_mtime = modified
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs_f64())
                                .unwrap_or(0.0);

                            debug!(
                                "Output file '{}' exists with mtime {}",
                                file_model.path, st_mtime
                            );
                            files_to_update.push((*file_id, st_mtime));
                        }
                        Err(e) => {
                            error!(
                                "Could not get modification time for file '{}': {}. Using current time.",
                                file_model.path, e
                            );
                            // Use current time as fallback
                            let st_mtime = Utc::now().timestamp() as f64;
                            files_to_update.push((*file_id, st_mtime));
                        }
                    }
                }
                Err(_) => {
                    // File does not exist
                    missing_files.push(file_model.path.clone());
                }
            }
        }

        // If any files are missing, return error
        if !missing_files.is_empty() {
            return Err(format!(
                "Job {} completed successfully but expected output files are missing: {}",
                job_id,
                missing_files.join(", ")
            ));
        }

        // Update st_mtime for all files
        for (file_id, st_mtime) in files_to_update {
            let mut file_model =
                match self.send_with_retries(|| default_api::get_file(&self.config, file_id)) {
                    Ok(file) => file,
                    Err(e) => {
                        error!(
                            "Failed to re-fetch file model for file_id {}: {}",
                            file_id, e
                        );
                        continue;
                    }
                };

            file_model.st_mtime = Some(st_mtime);
            match self.send_with_retries(|| {
                default_api::update_file(&self.config, file_id, file_model.clone())
            }) {
                Ok(_) => {
                    debug!("Updated st_mtime for file_id {} to {}", file_id, st_mtime);
                }
                Err(e) => {
                    error!("Failed to update st_mtime for file_id {}: {}", file_id, e);
                    // Don't fail the job for this, just log the error
                }
            }
        }

        info!(
            "Successfully validated {} output files for job {}",
            output_file_ids.len(),
            job_id
        );

        Ok(())
    }

    fn handle_job_completion(&mut self, job_id: i64, result: ResultModel) {
        // Get job info before removing from running_jobs
        let job_info = self.running_jobs.get(&job_id).map(|cmd| {
            (
                cmd.job.name.clone(),
                cmd.job.attempt_id.unwrap_or(1),
                cmd.job.failure_handler_id,
            )
        });

        // Check if we should try to recover a failed job
        let mut final_result = result;
        if final_result.status == JobStatus::Failed
            && let Some((job_name, attempt_id, failure_handler_id)) = &job_info
        {
            let return_code = final_result.return_code;
            // Try to recover the job if it has a failure handler
            let outcome = self.try_recover_job(
                job_id,
                job_name,
                return_code,
                *attempt_id,
                *failure_handler_id,
            );

            match outcome {
                RecoveryOutcome::Retried => {
                    // Job was successfully scheduled for retry - clean up but don't mark as failed
                    info!(
                        "Job retry scheduled workflow_id={} job_id={} job_name={} return_code={} attempt_id={}",
                        self.workflow_id, job_id, job_name, return_code, attempt_id
                    );
                    if let Some(job_rr) = self.job_resources.get(&job_id).cloned() {
                        self.increment_resources(&job_rr);
                    }
                    self.last_job_claimed_time = Some(Instant::now());
                    self.running_jobs.remove(&job_id);
                    self.job_resources.remove(&job_id);
                    return;
                }
                RecoveryOutcome::NoHandler | RecoveryOutcome::NoMatchingRule => {
                    // Check if workflow has use_pending_failed enabled
                    if self.workflow.use_pending_failed.unwrap_or(false) {
                        // Use PendingFailed status for AI-assisted recovery
                        info!(
                            "Job pending_failed workflow_id={} job_id={} job_name={} return_code={} reason={:?}",
                            self.workflow_id, job_id, job_name, return_code, outcome
                        );
                        final_result.status = JobStatus::PendingFailed;
                    } else {
                        // Use Failed status (default behavior)
                        debug!(
                            "Job failed workflow_id={} job_id={} job_name={} return_code={} reason={:?}",
                            self.workflow_id, job_id, job_name, return_code, outcome
                        );
                        // Keep status as Failed
                    }
                }
                RecoveryOutcome::MaxRetriesExceeded | RecoveryOutcome::Error(_) => {
                    // Max retries exceeded or error - use Failed status (no recovery possible)
                    debug!(
                        "Job failed workflow_id={} job_id={} reason={:?}",
                        self.workflow_id, job_id, outcome
                    );
                    // Keep status as Failed
                }
            }
        }

        // Track failures and terminations (if we reach here, no retry happened)
        match final_result.status {
            JobStatus::Failed | JobStatus::PendingFailed => self.had_failures = true,
            JobStatus::Terminated => self.had_terminations = true,
            _ => {}
        }

        let status_str = format!("{:?}", final_result.status).to_lowercase();
        match self.send_with_retries(|| {
            default_api::complete_job(
                &self.config,
                job_id,
                final_result.status,
                final_result.run_id,
                final_result.clone(),
            )
        }) {
            Ok(_) => {
                info!(
                    "Job completed workflow_id={} job_id={} run_id={} status={}",
                    self.workflow_id, job_id, final_result.run_id, status_str
                );
                if let Some(job_rr) = self.job_resources.get(&job_id).cloned() {
                    self.increment_resources(&job_rr);
                }
                // Reset the idle timer when a job completes, since blocked jobs may now
                // become ready. This gives dependent jobs time to be picked up before
                // the runner exits due to no jobs being claimed.
                self.last_job_claimed_time = Some(Instant::now());
            }
            Err(e) => {
                error!(
                    "Job complete failed workflow_id={} job_id={} error={}",
                    self.workflow_id, job_id, e
                );
            }
        }
        self.running_jobs.remove(&job_id);
        self.job_resources.remove(&job_id);
    }

    /// Run a recovery script with environment variables set.
    /// Returns Ok(()) if the recovery script succeeds (exit code 0).
    fn run_recovery_script(
        &self,
        job_id: i64,
        job_name: &str,
        script: &str,
        exit_code: i64,
        attempt_id: i64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Recovery script running workflow_id={} job_id={} job_name={} attempt_id={} script={}",
            self.workflow_id, job_id, job_name, attempt_id, script
        );

        // Run recovery script from the same working directory where job commands run
        // (the original working directory where `torc run` was executed), not from output_dir.
        // This ensures paths in recovery scripts are relative to the same base as job commands.
        let output = crate::client::utils::shell_command()
            .arg(script)
            .env("TORC_WORKFLOW_ID", self.workflow_id.to_string())
            .env("TORC_JOB_ID", job_id.to_string())
            .env("TORC_JOB_NAME", job_name)
            .env("TORC_API_URL", &self.config.base_path)
            .env(
                "TORC_OUTPUT_DIR",
                self.output_dir.to_string_lossy().to_string(),
            )
            .env("TORC_ATTEMPT_ID", attempt_id.to_string())
            .env("TORC_RETURN_CODE", exit_code.to_string())
            .output()?;

        if output.status.success() {
            info!(
                "Recovery script succeeded workflow_id={} job_id={} attempt_id={}",
                self.workflow_id, job_id, attempt_id
            );
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!(
                "Recovery script failed workflow_id={} job_id={} exit_code={:?} stderr={}",
                self.workflow_id,
                job_id,
                output.status.code(),
                stderr
            )
            .into())
        }
    }

    /// Try to recover and retry a failed job based on its failure handler rules.
    /// Returns a `RecoveryOutcome` indicating what happened.
    fn try_recover_job(
        &self,
        job_id: i64,
        job_name: &str,
        exit_code: i64,
        attempt_id: i64,
        failure_handler_id: Option<i64>,
    ) -> RecoveryOutcome {
        // Fetch the failure handler for this job on demand
        let fh_id = match failure_handler_id {
            Some(id) => id,
            None => return RecoveryOutcome::NoHandler,
        };

        let handler = match self
            .send_with_retries(|| default_api::get_failure_handler(&self.config, fh_id))
        {
            Ok(h) => h,
            Err(e) => {
                warn!(
                    "Failed to fetch failure handler {} for job {}: {}",
                    fh_id, job_id, e
                );
                return RecoveryOutcome::Error(format!("Failed to fetch failure handler: {}", e));
            }
        };

        // Parse the rules JSON
        let rules: Vec<FailureHandlerRule> = match serde_json::from_str(&handler.rules) {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    "Failed to parse failure handler rules for job {}: {}",
                    job_id, e
                );
                return RecoveryOutcome::Error(format!(
                    "Failed to parse failure handler rules: {}",
                    e
                ));
            }
        };

        // Find a matching rule for this exit code
        // First check for rules with specific exit_codes, then fall back to match_all_exit_codes
        let matching_rule = rules
            .iter()
            .find(|rule| rule.exit_codes.contains(&(exit_code as i32)))
            .or_else(|| rules.iter().find(|rule| rule.match_all_exit_codes));

        let rule = match matching_rule {
            Some(r) => r,
            None => {
                debug!(
                    "No matching failure handler rule for job {} with exit code {}",
                    job_id, exit_code
                );
                return RecoveryOutcome::NoMatchingRule;
            }
        };

        // Check if we've exceeded max retries
        if attempt_id >= rule.max_retries as i64 {
            info!(
                "Job max retries reached workflow_id={} job_id={} max_retries={} exit_code={}",
                self.workflow_id, job_id, rule.max_retries, exit_code
            );
            return RecoveryOutcome::MaxRetriesExceeded;
        }

        // Call retry_job API first to reserve the retry slot.
        // This ensures we don't run recovery scripts for retries that won't happen.
        // Pass max_retries for server-side validation.
        match self.send_with_retries(|| {
            default_api::retry_job(&self.config, job_id, self.run_id, rule.max_retries)
        }) {
            Ok(_) => {
                info!(
                    "Job retried workflow_id={} job_id={} run_id={} attempt_id={} new_attempt_id={}",
                    self.workflow_id,
                    job_id,
                    self.run_id,
                    attempt_id,
                    attempt_id + 1
                );
            }
            Err(e) => {
                error!(
                    "Job retry failed workflow_id={} job_id={} error={}",
                    self.workflow_id, job_id, e
                );
                return RecoveryOutcome::Error(format!("Retry API call failed: {}", e));
            }
        }

        // Run recovery script if defined (after retry is confirmed)
        // If the recovery script fails, the job will still be retried but may fail again.
        // This is safer than running recovery before retry_job, which could leave
        // external resources in an inconsistent state if the retry API call fails.
        if let Some(ref recovery_script) = rule.recovery_script
            && let Err(e) =
                self.run_recovery_script(job_id, job_name, recovery_script, exit_code, attempt_id)
        {
            warn!(
                "Recovery script failed (job will still retry) workflow_id={} job_id={} error={}",
                self.workflow_id, job_id, e
            );
            // Don't return error - the retry is already scheduled
        }

        RecoveryOutcome::Retried
    }

    fn decrement_resources(&mut self, rr: &ResourceRequirementsModel) {
        let job_memory_gb = memory_string_to_gb(&rr.memory);
        self.resources.memory_gb -= job_memory_gb;
        self.resources.num_cpus -= rr.num_cpus;
        self.resources.num_gpus -= rr.num_gpus;
        assert!(self.resources.memory_gb >= 0.0);
        assert!(self.resources.num_cpus >= 0);
        assert!(self.resources.num_gpus >= 0);
    }

    fn increment_resources(&mut self, rr: &ResourceRequirementsModel) {
        let job_memory_gb = memory_string_to_gb(&rr.memory);
        self.resources.memory_gb += job_memory_gb;
        self.resources.num_cpus += rr.num_cpus;
        self.resources.num_gpus += rr.num_gpus;
        assert!(self.resources.memory_gb <= self.orig_resources.memory_gb);
        assert!(self.resources.num_cpus <= self.orig_resources.num_cpus);
        assert!(self.resources.num_gpus <= self.orig_resources.num_gpus);
    }

    /// Update the time_limit in resources based on remaining time until end_time.
    /// This ensures the server only returns jobs whose runtime fits within the remaining allocation time.
    fn update_remaining_time_limit(&mut self) {
        if let Some(end_time) = self.end_time {
            let now = Utc::now();
            if end_time > now {
                let remaining_seconds = (end_time - now).num_seconds() as u64;
                let time_limit = format_duration_iso8601(remaining_seconds);
                debug!(
                    "Updating time_limit to {} ({} seconds remaining)",
                    time_limit, remaining_seconds
                );
                self.resources.time_limit = Some(time_limit);
            } else {
                // End time has passed - set to minimum
                debug!("End time has passed, setting time_limit to PT1M");
                self.resources.time_limit = Some("PT1M".to_string());
            }
        }
        // If end_time is None, leave time_limit as-is (unlimited)
    }

    fn run_ready_jobs_based_on_resources(&mut self) {
        self.update_remaining_time_limit();

        let limit = self.resources.num_cpus;
        let strict_scheduler_match = self.torc_config.client.slurm.strict_scheduler_match;
        match self.send_with_retries(|| {
            default_api::claim_jobs_based_on_resources(
                &self.config,
                self.workflow_id,
                &self.resources,
                limit,
                Some(self.rules.jobs_sort_method),
                Some(strict_scheduler_match),
            )
        }) {
            Ok(response) => {
                let jobs = response.jobs.unwrap_or_default();
                if jobs.is_empty() {
                    debug!("No ready jobs found");
                    return;
                }
                if jobs.len() > limit as usize {
                    panic!(
                        "Bug in server: too many jobs returned. limit: {}, returned: {}",
                        limit,
                        jobs.len()
                    );
                }
                debug!("Found {} ready jobs to execute", jobs.len());

                // Update last job claimed time since we got jobs
                self.last_job_claimed_time = Some(Instant::now());

                for job in jobs {
                    let job_id = job.id.expect("Job must have an ID");
                    let rr_id = job
                        .resource_requirements_id
                        .expect("Job must have a resource_requirements_id");
                    let mut async_job = AsyncCliCommand::new(job);

                    let job_rr = match self.send_with_retries(|| {
                        default_api::get_resource_requirements(&self.config, rr_id)
                    }) {
                        Ok(rr) => rr,
                        Err(e) => {
                            error!(
                                "Error getting resource requirements for job {}: {}",
                                job_id, e
                            );
                            panic!("Failed to get resource requirements");
                        }
                    };

                    // Mark job as started in the database before actually starting it
                    match self.send_with_retries(|| {
                        default_api::start_job(
                            &self.config,
                            job_id,
                            self.run_id,
                            self.compute_node_id,
                            None,
                        )
                    }) {
                        Ok(_) => {
                            debug!("Successfully marked job {} as started in database", job_id);
                        }
                        Err(e) => {
                            panic!(
                                "Failed to mark job {} as started in database after retries: {}",
                                job_id, e
                            );
                        }
                    }

                    let attempt_id = async_job.job.attempt_id.unwrap_or(1);
                    match async_job.start(
                        &self.output_dir,
                        self.workflow_id,
                        self.run_id,
                        attempt_id,
                        self.resource_monitor.as_ref(),
                        &self.config.base_path,
                    ) {
                        Ok(()) => {
                            info!(
                                "Job started workflow_id={} job_id={} run_id={} compute_node_id={} attempt_id={}",
                                self.workflow_id,
                                job_id,
                                self.run_id,
                                self.compute_node_id,
                                attempt_id
                            );
                            self.running_jobs.insert(job_id, async_job);
                            self.decrement_resources(&job_rr);
                            self.job_resources.insert(job_id, job_rr);
                        }
                        Err(e) => {
                            error!(
                                "Job start failed workflow_id={} job_id={} error={}",
                                self.workflow_id, job_id, e
                            );
                            continue;
                        }
                    }
                }
            }
            Err(err) => {
                error!("Failed to prepare jobs for submission: {}", err);
                match self.send_with_retries(|| {
                    default_api::claim_jobs_based_on_resources(
                        &self.config,
                        self.workflow_id,
                        &self.resources,
                        limit,
                        Some(self.rules.jobs_sort_method),
                        Some(strict_scheduler_match),
                    )
                }) {
                    Ok(_) => {
                        info!("Successfully prepared jobs after retry");
                    }
                    Err(retry_err) => {
                        error!(
                            "Failed to prepare jobs for submission after retries: {}",
                            retry_err
                        );
                    }
                }
            }
        }
    }

    fn run_ready_jobs_based_on_user_parallelism(&mut self) {
        // Check if we have enough remaining time to start new jobs
        if let Some(end_time) = self.end_time {
            let remaining_seconds = (end_time - Utc::now()).num_seconds();
            if remaining_seconds < self.rules.compute_node_min_time_for_new_jobs_seconds as i64 {
                info!(
                    "Only {} seconds remaining (min required: {}), not requesting new jobs",
                    remaining_seconds, self.rules.compute_node_min_time_for_new_jobs_seconds
                );
                return;
            }
        }

        let limit = self
            .max_parallel_jobs
            .expect("max_parallel_jobs must be set")
            - self.running_jobs.len() as i64;
        match self.send_with_retries(|| {
            default_api::claim_next_jobs(&self.config, self.workflow_id, Some(limit), None)
        }) {
            Ok(response) => {
                let jobs = response.jobs.unwrap_or_default();
                if jobs.is_empty() {
                    return;
                }
                if jobs.len() > limit as usize {
                    panic!(
                        "Bug in server: too many jobs returned. limit: {}, returned: {}",
                        limit,
                        jobs.len()
                    );
                }
                info!("Found {} ready jobs to execute", jobs.len());

                // Update last job claimed time since we got jobs
                self.last_job_claimed_time = Some(Instant::now());

                // Start each job asynchronously
                for job in jobs {
                    let job_id = job.id.expect("Job must have an ID");
                    let mut async_job = AsyncCliCommand::new(job);

                    // Mark job as started in the database before actually starting it
                    match self.send_with_retries(|| {
                        default_api::start_job(
                            &self.config,
                            job_id,
                            self.run_id,
                            self.compute_node_id,
                            None,
                        )
                    }) {
                        Ok(_) => {
                            debug!("Successfully marked job {} as started in database", job_id);
                        }
                        Err(e) => {
                            error!(
                                "Failed to mark job {} as started in database after retries: {}",
                                job_id, e
                            );
                            // Skip this job if we can't mark it as started
                            continue;
                        }
                    }

                    let attempt_id = async_job.job.attempt_id.unwrap_or(1);
                    match async_job.start(
                        &self.output_dir,
                        self.workflow_id,
                        self.run_id,
                        attempt_id,
                        self.resource_monitor.as_ref(),
                        &self.config.base_path,
                    ) {
                        Ok(()) => {
                            info!(
                                "Job started workflow_id={} job_id={} run_id={} compute_node_id={} attempt_id={}",
                                self.workflow_id,
                                job_id,
                                self.run_id,
                                self.compute_node_id,
                                attempt_id
                            );
                            self.running_jobs.insert(job_id, async_job);
                        }
                        Err(e) => {
                            error!(
                                "Job start failed workflow_id={} job_id={} error={}",
                                self.workflow_id, job_id, e
                            );
                            continue;
                        }
                    }
                }
            }
            Err(err) => {
                error!(
                    "Job preparation failed workflow_id={} error={}",
                    self.workflow_id, err
                );
                panic!("Failed to prepare jobs for submission after retries");
            }
        }
    }

    /// Helper method to execute actions of a specific trigger type.
    ///
    /// This method fetches pending actions for the given trigger type, claims them atomically,
    /// and executes them. It's used by the specific action execution methods to avoid code
    /// duplication.
    fn execute_actions_by_trigger_type(&mut self, trigger_type: &str) {
        info!(
            "Checking for {} actions workflow_id={}",
            trigger_type, self.workflow_id
        );

        // Get pending actions for the specified trigger type
        let trigger_type_owned = trigger_type.to_string();
        let pending_actions = match self.send_with_retries(
            || -> Result<Vec<crate::models::WorkflowActionModel>, Box<dyn std::error::Error>> {
                let actions = default_api::get_pending_actions(
                    &self.config,
                    self.workflow_id,
                    Some(vec![trigger_type_owned.clone()]),
                )?;
                Ok(actions)
            },
        ) {
            Ok(actions) => actions,
            Err(e) => {
                error!(
                    "Failed to get pending {} actions workflow_id={}: {}",
                    trigger_type, self.workflow_id, e
                );
                return;
            }
        };

        // Execute all actions of this trigger type
        for action in pending_actions {
            let action_id = match action.id {
                Some(id) => id,
                None => {
                    error!(
                        "Action missing id field trigger_type={} workflow_id={}",
                        trigger_type, self.workflow_id
                    );
                    continue;
                }
            };

            // Check if this job runner can handle this action before claiming
            if !self.can_handle_action(&action) {
                debug!(
                    "{} action {} cannot be handled by this job runner, skipping",
                    trigger_type, action_id
                );
                continue;
            }

            // Try to atomically claim this action
            let claimed = match self.claim_action(action_id) {
                Ok(claimed) => claimed,
                Err(e) => {
                    // Not fatal - just log and continue
                    error!(
                        "Error claiming {} action workflow_id={} action_id={}: {}",
                        trigger_type, self.workflow_id, action_id, e
                    );
                    continue;
                }
            };

            if !claimed {
                debug!(
                    "{} action {} already claimed by another runner",
                    trigger_type, action_id
                );
                continue;
            }

            // We claimed it! Execute the action
            info!(
                "Executing {} workflow_id={} action_id={}",
                trigger_type, self.workflow_id, action_id
            );
            if let Err(e) = self.execute_action(&action) {
                // Not fatal - just log and continue
                error!(
                    "Failed to execute {} workflow_id={} action_id={}: {}",
                    trigger_type, self.workflow_id, action_id, e
                );
            }
        }
    }

    /// Execute all on_workflow_start actions before the main loop begins
    fn execute_workflow_start_actions(&mut self) {
        self.execute_actions_by_trigger_type("on_workflow_start");
    }

    /// Execute all on_worker_start actions before the main loop begins
    fn execute_worker_start_actions(&mut self) {
        self.execute_actions_by_trigger_type("on_worker_start");
    }

    /// Execute all on_worker_complete actions after the main loop ends
    fn execute_worker_complete_actions(&mut self) {
        self.execute_actions_by_trigger_type("on_worker_complete");
    }

    /// Execute all on_workflow_complete actions when the workflow completes
    fn execute_workflow_complete_actions(&mut self) {
        self.execute_actions_by_trigger_type("on_workflow_complete");
    }

    /// Check for pending workflow actions and execute them if their trigger conditions are met
    fn check_and_execute_actions(&mut self) {
        // Get pending on_jobs_ready and on_jobs_complete actions
        let pending_actions = match self.send_with_retries(
            || -> Result<Vec<crate::models::WorkflowActionModel>, Box<dyn std::error::Error>> {
                let actions = default_api::get_pending_actions(
                    &self.config,
                    self.workflow_id,
                    Some(vec![
                        "on_jobs_ready".to_string(),
                        "on_jobs_complete".to_string(),
                    ]),
                )?;
                Ok(actions)
            },
        ) {
            Ok(actions) => {
                if !actions.is_empty() {
                    info!(
                        "Found {} pending action(s) (trigger_types: on_jobs_ready, on_jobs_complete)",
                        actions.len()
                    );
                    for action in &actions {
                        info!(
                            "  Action {:?}: type={}, trigger={}, trigger_count={}, required_triggers={}",
                            action.id,
                            action.action_type,
                            action.trigger_type,
                            action.trigger_count,
                            action.required_triggers
                        );
                    }
                }
                actions
            }
            Err(e) => {
                error!("Failed to get pending actions: {}", e);
                return;
            }
        };

        // Execute triggered actions
        // Note: The server now handles trigger detection server-side by setting triggered=1
        // when conditions are met, so we only need to claim and execute actions that are already triggered
        for action in pending_actions {
            let action_id = match action.id {
                Some(id) => id,
                None => {
                    error!("Action missing id field");
                    continue;
                }
            };

            let trigger_type = &action.trigger_type;

            // Check if this job runner can handle this action before claiming
            if !self.can_handle_action(&action) {
                info!(
                    "Action {} (type={}) cannot be handled by this job runner, skipping",
                    action_id, action.action_type
                );
                continue;
            }

            // Try to atomically claim this action
            let claimed = match self.claim_action(action_id) {
                Ok(claimed) => claimed,
                Err(e) => {
                    error!("Error claiming action {}: {}", action_id, e);
                    continue;
                }
            };

            if !claimed {
                debug!("Action {} already claimed by another runner", action_id);
                continue;
            }

            info!("Executing action {} (trigger: {})", action_id, trigger_type);
            if let Err(e) = self.execute_action(&action) {
                error!("Failed to execute action {}: {}", action_id, e);
            }
        }
    }

    /// Check if there are any unexecuted actions that this job runner can handle.
    /// This is used to prevent early exit when actions might still need to be executed.
    /// We check for unexecuted (not just pending) actions because the background thread
    /// might not have processed job completions yet, so actions that will become pending
    /// soon should also keep us alive.
    fn has_pending_actions_we_can_handle(&self) -> bool {
        // Get ALL actions for this workflow (not just pending ones)
        match self.send_with_retries(
            || -> Result<Vec<crate::models::WorkflowActionModel>, Box<dyn std::error::Error>> {
                let actions = default_api::get_workflow_actions(&self.config, self.workflow_id)?;
                Ok(actions)
            },
        ) {
            Ok(actions) => {
                // Check if we can handle any unexecuted on_jobs_ready or on_jobs_complete actions
                for action in &actions {
                    // Skip already executed actions
                    if action.executed {
                        continue;
                    }
                    // Only consider job-triggered actions (on_jobs_ready, on_jobs_complete)
                    // on_workflow_start and on_worker_start are handled at startup
                    if action.trigger_type != "on_jobs_ready"
                        && action.trigger_type != "on_jobs_complete"
                    {
                        continue;
                    }
                    if self.can_handle_action(action) {
                        debug!(
                            "Found unexecuted action {} (trigger={}, type={}) that we can handle",
                            action.id.unwrap_or(-1),
                            action.trigger_type,
                            action.action_type
                        );
                        return true;
                    }
                }
                false
            }
            Err(e) => {
                error!("Failed to check for unexecuted actions: {}", e);
                false
            }
        }
    }

    /// Check if this job runner can handle the given action
    /// Job runners can handle:
    /// - run_commands actions (always)
    /// - schedule_nodes actions (including slurm)
    fn can_handle_action(&self, action: &crate::models::WorkflowActionModel) -> bool {
        let action_type = &action.action_type;

        match action_type.as_str() {
            "run_commands" => true,
            "schedule_nodes" => {
                // Check scheduler_type in action_config
                let scheduler_type = action
                    .action_config
                    .get("scheduler_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Job runners can handle slurm schedule_nodes using schedule_slurm_nodes_for_action
                let can_handle = scheduler_type == "slurm";
                if !can_handle {
                    debug!(
                        "Cannot handle schedule_nodes action: scheduler_type='{}' (expected 'slurm'). action_config={:?}",
                        scheduler_type, action.action_config
                    );
                }
                can_handle
            }
            _ => {
                debug!(
                    "Cannot handle action: unknown action_type='{}'",
                    action_type
                );
                false
            }
        }
    }

    /// Execute a workflow action
    fn execute_action(
        &self,
        action: &crate::models::WorkflowActionModel,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let action_type = &action.action_type;
        let action_config = &action.action_config;

        match action_type.as_str() {
            "run_commands" => {
                let commands = action_config
                    .get("commands")
                    .and_then(|v| v.as_array())
                    .ok_or("run_commands action missing commands array")?;

                for command_value in commands {
                    let command = command_value.as_str().ok_or("Command must be a string")?;

                    info!("Executing command: {}", command);

                    // Execute the command using cross-platform shell
                    let output = crate::client::utils::shell_command()
                        .arg(command)
                        .current_dir(&self.output_dir)
                        .output()?;

                    if output.status.success() {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        if !stdout.is_empty() {
                            info!("Command output: {}", stdout.trim());
                        }
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        error!("Command failed: {}", stderr);
                        return Err(format!(
                            "Command failed with exit code: {:?}",
                            output.status.code()
                        )
                        .into());
                    }
                }

                Ok(())
            }
            "schedule_nodes" => {
                info!("schedule_nodes action triggered");

                // Extract configuration
                let scheduler_type = action_config
                    .get("scheduler_type")
                    .and_then(|v| v.as_str())
                    .ok_or("schedule_nodes action missing scheduler_type")?;

                let scheduler_id = action_config
                    .get("scheduler_id")
                    .and_then(|v| v.as_i64())
                    .ok_or("schedule_nodes action missing scheduler_id")?;

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

                info!(
                    "Scheduling {} compute nodes (scheduler_type={}, scheduler_id={})",
                    num_allocations, scheduler_type, scheduler_id
                );

                if scheduler_type == "slurm" {
                    // Use the same function as WorkflowManager for Slurm scheduling
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
                            info!("Successfully scheduled {} Slurm job(s)", num_allocations);
                            Ok(())
                        }
                        Err(err) => {
                            error!("Failed to schedule Slurm nodes: {}", err);
                            Err(format!("Failed to schedule Slurm nodes: {}", err).into())
                        }
                    }
                } else {
                    error!("scheduler_type = {} is not supported", scheduler_type);
                    Err(format!("Unsupported scheduler_type: {}", scheduler_type).into())
                }
            }
            _ => Err(format!("Unknown action type: {}", action_type).into()),
        }
    }
}

#[derive(Debug)]
struct ComputeNodeRules {
    /// Inform all compute nodes to shut down this number of seconds before the expiration time. This allows torc to send SIGTERM to all job processes and set all statuses to terminated. Increase the time in cases where the job processes handle SIGTERM and need more time to gracefully shut down. Set the value to 0 to maximize the time given to jobs. If not set, take the database's default value of 60 seconds.
    pub compute_node_expiration_buffer_seconds: i64,
    /// Inform all compute nodes to wait for new jobs for this time period before exiting.
    /// Does not apply if the workflow is complete.
    ///
    /// The default value must satisfy:
    ///   compute_node_wait_for_new_jobs_seconds >= completion_check_interval_secs + job_completion_poll_interval
    /// This ensures the worker doesn't exit before the server's background unblock task runs
    /// and the worker polls for newly-ready jobs. With defaults of 30s for each interval,
    /// the minimum safe value is 60s. We use 90s to provide a safety buffer.
    pub compute_node_wait_for_new_jobs_seconds: u64,
    /// Inform all compute nodes to ignore workflow completions and hold onto allocations indefinitely. Useful for debugging failed jobs and possibly dynamic workflows where jobs get added after starting.
    pub compute_node_ignore_workflow_completion: bool,
    /// Inform all compute nodes to wait this number of minutes if the database becomes unresponsive.
    pub compute_node_wait_for_healthy_database_minutes: u64,
    /// Minimum remaining walltime (in seconds) required before requesting new jobs.
    /// If the remaining time is less than this value, the compute node will stop requesting
    /// new jobs and wait for running jobs to complete. Default is 300 seconds (5 minutes).
    pub compute_node_min_time_for_new_jobs_seconds: u64,
    pub jobs_sort_method: ClaimJobsSortMethod,
}

impl ComputeNodeRules {
    pub fn new(
        compute_node_expiration_buffer_seconds: Option<i64>,
        compute_node_wait_for_new_jobs_seconds: Option<i64>,
        compute_node_ignore_workflow_completion: Option<bool>,
        compute_node_wait_for_healthy_database_minutes: Option<i64>,
        compute_node_min_time_for_new_jobs_seconds: Option<i64>,
        jobs_sort_method: Option<ClaimJobsSortMethod>,
    ) -> Self {
        ComputeNodeRules {
            compute_node_expiration_buffer_seconds: compute_node_expiration_buffer_seconds
                .unwrap_or(60),
            compute_node_wait_for_new_jobs_seconds: compute_node_wait_for_new_jobs_seconds
                .unwrap_or(90) as u64,
            compute_node_ignore_workflow_completion: compute_node_ignore_workflow_completion
                .unwrap_or(false),
            compute_node_wait_for_healthy_database_minutes:
                compute_node_wait_for_healthy_database_minutes.unwrap_or(20) as u64,
            compute_node_min_time_for_new_jobs_seconds: compute_node_min_time_for_new_jobs_seconds
                .unwrap_or(300) as u64,
            jobs_sort_method: jobs_sort_method.unwrap_or(ClaimJobsSortMethod::GpusRuntimeMemory),
        }
    }
}
