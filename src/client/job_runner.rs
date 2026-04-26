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
//!    [`JobRunner::terminate_jobs()`], which sends SIGTERM to all running jobs, waits for
//!    them to exit, and sets job status to `JobStatus::Completed` (if exit code is 0) or
//!    `JobStatus::Terminated` (if exit code is non-zero).
//!
//! # Per-Step Timeout via srun
//!
//! When running under Slurm, each job step is launched with `srun --time=<runtime>`, which
//! enforces the job's configured runtime at the Slurm level. Slurm sends SIGTERM when the
//! step hits its time limit, then SIGKILL after `KillWait` seconds (typically 30s). This
//! means all jobs get a graceful termination window regardless of configuration.

use chrono::{DateTime, Utc};
use log::{self, debug, error, info, warn};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::client::apis;
use crate::client::apis::configuration::Configuration;
use crate::client::async_cli_command::AsyncCliCommand;
use crate::client::resource_correction::format_duration_iso8601;
use crate::client::resource_monitor::{
    ResourceMonitor, ResourceMonitorConfig, SystemMetricsSummary,
};
use crate::client::utils;
use crate::client::workflow_spec::{ExecutionConfig, ExecutionMode};
use crate::config::TorcConfig;
use crate::memory_utils::memory_string_to_gb;
use crate::models::{
    BatchCompleteJobsRequest, ComputeNodesResources, JobCompletionEntry, JobStatus,
    ResourceRequirementsModel, ResultModel, SlurmStatsModel, WorkflowModel,
};

/// Local-side result of preparing a job completion: the data to send to the
/// server in a `batch_complete_jobs` call. Returned by `prepare_job_completion`
/// for jobs that were not retried locally.
struct PreparedCompletion {
    job_id: i64,
    final_result: ResultModel,
    slurm_stats: Option<SlurmStatsModel>,
}

/// Condvar-backed wakeup primitive used by the runner's main loop.
///
/// `wait_with_timeout` blocks up to the given duration; `notify` wakes any
/// waiter immediately. Notifications are remembered: if `notify` is called
/// while no one is waiting, the next `wait_with_timeout` returns immediately
/// without sleeping. This makes it safe for an external thread (e.g. a
/// SIGCHLD handler thread) to call `notify` at any time without coordinating
/// with the runner's loop position.
///
/// `notify` takes a mutex and so is not async-signal-safe. Call it from a
/// normal thread that consumes signals, never from a raw signal handler.
pub struct Wakeup {
    pending: Mutex<bool>,
    cv: Condvar,
}

impl Wakeup {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            pending: Mutex::new(false),
            cv: Condvar::new(),
        })
    }

    /// Wake any waiter. If no one is waiting, the next `wait_with_timeout`
    /// returns immediately.
    pub fn notify(&self) {
        let mut pending = self.pending.lock().unwrap();
        *pending = true;
        self.cv.notify_all();
    }

    /// Wait until notified or `timeout` elapses. Returns `true` if a
    /// notification was consumed, `false` on timeout.
    pub fn wait_with_timeout(&self, timeout: Duration) -> bool {
        let mut pending = self.pending.lock().unwrap();
        if *pending {
            *pending = false;
            return true;
        }

        let deadline = Instant::now() + timeout;
        loop {
            let now = Instant::now();
            if now >= deadline {
                return false;
            }

            let remaining = deadline.saturating_duration_since(now);
            let (next_pending, wait_result) = self.cv.wait_timeout(pending, remaining).unwrap();
            pending = next_pending;

            if *pending {
                *pending = false;
                return true;
            }

            if wait_result.timed_out() {
                return false;
            }
        }
    }
}

impl Default for Wakeup {
    fn default() -> Self {
        Self {
            pending: Mutex::new(false),
            cv: Condvar::new(),
        }
    }
}

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

/// Tracks per-node resource availability for multi-node Slurm allocations.
///
/// When running across multiple nodes, the job runner needs to track each node's available
/// resources independently. Without this, dividing remaining total resources by `num_nodes`
/// gives incorrect per-node values when jobs are unevenly distributed across nodes.
///
/// # Approach
///
/// We use `srun --nodelist=<node>` to explicitly place each job step on a specific node,
/// calling `claim_jobs_based_on_resources` once per node with that node's available resources.
/// An alternative would be to let Slurm manage placement and then query `squeue --steps`
/// with the `%N` format field after launch to discover where each step landed. We chose
/// explicit placement to avoid the squeue RPC and to keep resource tracking deterministic.
pub struct PerNodeTracker {
    nodes: Vec<NodeCapacity>,
}

/// Resource capacity for a single node in a multi-node allocation.
pub(crate) struct NodeCapacity {
    name: String,
    available_cpus: i64,
    available_memory_gb: f64,
    available_gpus: i64,
}

impl PerNodeTracker {
    /// Create a new tracker with all nodes initialized to the same per-node capacity.
    pub fn new(
        node_names: Vec<String>,
        cpus_per_node: i64,
        memory_gb_per_node: f64,
        gpus_per_node: i64,
    ) -> Self {
        let nodes = node_names
            .into_iter()
            .map(|name| NodeCapacity {
                name,
                available_cpus: cpus_per_node,
                available_memory_gb: memory_gb_per_node,
                available_gpus: gpus_per_node,
            })
            .collect();
        PerNodeTracker { nodes }
    }

    /// Returns the maximum available resources across all nodes.
    ///
    /// This is sent to the server so it returns jobs that fit on at least one node.
    /// The server filters: `rr.num_cpus <= per_node_cpus`, so reporting the max
    /// ensures we can claim any job that fits on the most-available node.
    fn max_available(&self) -> (i64, f64, i64) {
        let cpus = self
            .nodes
            .iter()
            .map(|n| n.available_cpus)
            .max()
            .unwrap_or(0);
        let memory = self
            .nodes
            .iter()
            .map(|n| n.available_memory_gb)
            .fold(0.0_f64, f64::max);
        let gpus = self
            .nodes
            .iter()
            .map(|n| n.available_gpus)
            .max()
            .unwrap_or(0);
        (cpus, memory, gpus)
    }

    /// Decrement resources on a specific node after a job is placed there.
    fn decrement(&mut self, node_name: &str, cpus: i64, memory_gb: f64, gpus: i64) {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.name == node_name) {
            node.available_cpus -= cpus;
            node.available_memory_gb -= memory_gb;
            node.available_gpus -= gpus;
            debug!(
                "Per-node decrement: node={} cpus={}/{} mem={:.1}/{:.1}GB gpus={}/{}",
                node_name,
                cpus,
                node.available_cpus + cpus,
                memory_gb,
                node.available_memory_gb + memory_gb,
                gpus,
                node.available_gpus + gpus,
            );
        } else {
            warn!(
                "Per-node decrement: node {} not found in tracker, skipping",
                node_name
            );
        }
    }

    /// Increment resources on a specific node when a job completes.
    fn increment(&mut self, node_name: &str, cpus: i64, memory_gb: f64, gpus: i64) {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.name == node_name) {
            node.available_cpus += cpus;
            node.available_memory_gb += memory_gb;
            node.available_gpus += gpus;
            debug!(
                "Per-node increment: node={} cpus_now={} mem_now={:.1}GB gpus_now={}",
                node_name, node.available_cpus, node.available_memory_gb, node.available_gpus,
            );
        } else {
            warn!(
                "Per-node increment: node {} not found in tracker, skipping",
                node_name
            );
        }
    }
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

#[derive(Debug)]
struct JobRunnerApiError(String);

impl std::fmt::Display for JobRunnerApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for JobRunnerApiError {}

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
/// - Jobs that exit cleanly (exit code 0) are set to `JobStatus::Completed`
/// - Jobs that crash or are force-killed are set to `JobStatus::Terminated`
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
    /// Pool of GPU device identifiers available to this runner (e.g. `"0"`, `"1"` or UUIDs).
    ///
    /// When running in direct mode, Torc sets `CUDA_VISIBLE_DEVICES` (and friends) itself
    /// to prevent concurrent GPU jobs from all defaulting to GPU 0.
    available_gpu_devices: VecDeque<String>,
    /// Snapshot of the full GPU device pool at startup, used for modulo-based fallback
    /// when the available pool is exhausted (e.g. in user-parallelism mode).
    all_gpu_devices: Vec<String>,
    /// Counter for round-robin GPU assignment when the pool is exhausted.
    gpu_fallback_counter: usize,
    /// GPUs assigned to a running job, keyed by job_id.
    job_gpu_devices: HashMap<i64, Vec<String>>,
    /// Per-node resource tracker for multi-node Slurm allocations.
    /// None for single-node allocations where dividing total by 1 is correct.
    node_tracker: Option<PerNodeTracker>,
    /// Maps job_id to the node name where the job is running.
    /// Used to increment the correct node's resources on job completion.
    job_nodes: HashMap<i64, String>,
    execution_config: ExecutionConfig,
    rules: ComputeNodeRules,
    resource_monitor: Option<ResourceMonitor>,
    /// Flag set when SIGTERM is received. Shared with signal handler.
    termination_requested: Arc<AtomicBool>,
    /// Notified when SIGCHLD fires (or termination is requested) so the main
    /// loop can wake from its idle wait without waiting for the full
    /// `job_completion_poll_interval`. Shared with signal-handler threads.
    wakeup: Arc<Wakeup>,
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
    fn parse_visible_devices_list(value: &str) -> Vec<String> {
        value
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    }

    fn detect_gpu_devices(resources_num_gpus: i64) -> (VecDeque<String>, bool) {
        // Prefer an explicit allocation-scoped device list if present.
        // Slurm: CUDA_VISIBLE_DEVICES is commonly set at allocation scope.
        // Some clusters also set SLURM_JOB_GPUS / SLURM_STEP_GPUS.
        if let Ok(v) = std::env::var("CUDA_VISIBLE_DEVICES") {
            let parsed = Self::parse_visible_devices_list(&v);
            if !parsed.is_empty() {
                return (VecDeque::from(parsed), true);
            }
        }
        if let Ok(v) = std::env::var("SLURM_STEP_GPUS") {
            let parsed = Self::parse_visible_devices_list(&v)
                .into_iter()
                .map(|s| {
                    s.trim_start_matches("gpu:")
                        .trim_start_matches("GPU:")
                        .to_string()
                })
                .collect::<Vec<_>>();
            if !parsed.is_empty() {
                return (VecDeque::from(parsed), true);
            }
        }
        if let Ok(v) = std::env::var("SLURM_JOB_GPUS") {
            let parsed = Self::parse_visible_devices_list(&v)
                .into_iter()
                .map(|s| {
                    s.trim_start_matches("gpu:")
                        .trim_start_matches("GPU:")
                        .to_string()
                })
                .collect::<Vec<_>>();
            if !parsed.is_empty() {
                return (VecDeque::from(parsed), true);
            }
        }

        // Fall back to ordinal device indices.
        let fallback = (0..resources_num_gpus.max(0))
            .map(|i| i.to_string())
            .collect::<Vec<_>>();
        (VecDeque::from(fallback), false)
    }

    fn allocate_gpu_devices(&mut self, job_id: i64, num_gpus: i64) -> Option<String> {
        if num_gpus <= 0 {
            return None;
        }

        let requested = num_gpus as usize;
        if self.available_gpu_devices.len() >= requested {
            // Normal path: allocate from the available pool.
            let mut assigned = Vec::with_capacity(requested);
            for _ in 0..requested {
                if let Some(dev) = self.available_gpu_devices.pop_front() {
                    assigned.push(dev);
                }
            }

            let visible = assigned.join(",");
            self.job_gpu_devices.insert(job_id, assigned);
            debug!(
                "Assigned GPUs workflow_id={} job_id={} gpus={}",
                self.workflow_id, job_id, visible
            );
            return Some(visible);
        }

        // Pool exhausted — this can happen in user-parallelism mode where jobs are
        // claimed without resource filtering. Use round-robin over the full device
        // pool so behaviour is deterministic and jobs don't all default to GPU 0.
        if self.all_gpu_devices.is_empty() {
            error!(
                "No GPU devices configured but job requires GPUs \
                 workflow_id={} job_id={} requested={}",
                self.workflow_id, job_id, requested
            );
            return None;
        }

        let pool_size = self.all_gpu_devices.len();
        // Clamp to pool size to avoid duplicate device IDs in CUDA_VISIBLE_DEVICES,
        // which can cause confusing behavior with CUDA/HIP runtimes.
        let clamped = requested.min(pool_size);
        if clamped < requested {
            warn!(
                "Job requests {} GPUs but only {} devices exist, clamping \
                 workflow_id={} job_id={}",
                requested, pool_size, self.workflow_id, job_id
            );
        }
        let mut assigned = Vec::with_capacity(clamped);
        for _ in 0..clamped {
            let idx = self.gpu_fallback_counter % pool_size;
            assigned.push(self.all_gpu_devices[idx].clone());
            self.gpu_fallback_counter += 1;
        }

        let visible = assigned.join(",");
        warn!(
            "GPU pool exhausted, using round-robin fallback \
             workflow_id={} job_id={} gpus={} (oversubscribed)",
            self.workflow_id, job_id, visible
        );
        // Don't track in job_gpu_devices — these are shared, not exclusively owned.
        Some(visible)
    }

    fn release_gpu_devices(&mut self, job_id: i64) {
        if let Some(devs) = self.job_gpu_devices.remove(&job_id) {
            for dev in devs {
                self.available_gpu_devices.push_back(dev);
            }
        }
    }

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
        node_tracker: Option<PerNodeTracker>,
    ) -> Self {
        let workflow_id = workflow.id.expect("Workflow ID must be present");
        let running_jobs: HashMap<i64, AsyncCliCommand> = HashMap::new();
        let torc_config = TorcConfig::load().unwrap_or_default();
        let rules = ComputeNodeRules::new(
            workflow.compute_node_wait_for_new_jobs_seconds,
            workflow.compute_node_ignore_workflow_completion,
            workflow.compute_node_wait_for_healthy_database_minutes,
            workflow.compute_node_min_time_for_new_jobs_seconds,
        );
        let execution_config = ExecutionConfig::from_workflow_model(&workflow);
        if execution_config.effective_mode() == ExecutionMode::Slurm
            && std::env::var("SLURM_JOB_ID").is_err()
        {
            panic!(
                "Execution mode is 'slurm' but SLURM_JOB_ID is not set. \
                 Cannot run jobs with srun outside a Slurm allocation."
            );
        }
        let job_resources: HashMap<i64, ResourceRequirementsModel> = HashMap::new();

        let mut resources = resources;
        let available_gpu_devices = if execution_config.effective_mode() == ExecutionMode::Slurm {
            // In Slurm mode, `resources.num_gpus` already represents the
            // allocation-wide accounting pool. Process-local visible device
            // env vars may expose only this node's GPUs, so do not use them
            // to shrink the total pool.
            (0..resources.num_gpus.max(0))
                .map(|i| i.to_string())
                .collect::<VecDeque<_>>()
        } else {
            // In direct mode, if the environment already constrains visible
            // GPUs, use that list as the authoritative device pool and keep
            // the accounting counts aligned with it.
            let (available_gpu_devices, env_constrained) =
                Self::detect_gpu_devices(resources.num_gpus);
            if env_constrained {
                resources.num_gpus = available_gpu_devices.len() as i64;
            }
            available_gpu_devices
        };

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
                Ok(monitor_config) if monitor_config.is_enabled() => {
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
            all_gpu_devices: Vec::from(available_gpu_devices.clone()),
            gpu_fallback_counter: 0,
            available_gpu_devices,
            job_gpu_devices: HashMap::new(),
            node_tracker,
            job_nodes: HashMap::new(),
            execution_config,
            rules,
            resource_monitor,
            termination_requested: Arc::new(AtomicBool::new(false)),
            wakeup: Wakeup::new(),
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
    fn send_with_retries<T, E, F>(&self, mut api_call: F) -> Result<T, Box<dyn std::error::Error>>
    where
        F: FnMut() -> Result<T, E>,
        E: std::fmt::Display,
    {
        utils::send_with_retries(
            &self.config,
            || {
                api_call().map_err(|err| {
                    Box::new(JobRunnerApiError(err.to_string())) as Box<dyn std::error::Error>
                })
            },
            self.rules.compute_node_wait_for_healthy_database_minutes,
        )
    }

    fn box_retry_error<T, E>(result: Result<T, E>) -> Result<T, Box<dyn std::error::Error>>
    where
        E: std::fmt::Display,
    {
        result.map_err(|err| {
            Box::new(JobRunnerApiError(err.to_string())) as Box<dyn std::error::Error>
        })
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

    /// Returns a clone of the wakeup primitive for use by signal-handler
    /// threads. Calling `notify()` on the returned handle wakes the runner's
    /// main loop from its idle wait, shrinking subprocess-completion latency
    /// from up to `job_completion_poll_interval` down to the time it takes
    /// the loop to call `try_wait` on each child. Intended for SIGCHLD
    /// handlers, but safe for any thread to call.
    pub fn get_wakeup_handle(&self) -> Arc<Wakeup> {
        Arc::clone(&self.wakeup)
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
        let server_api_version = version_result
            .server_api_version
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        let exec_mode = self.execution_config.effective_mode();
        info!(
            "Starting torc job runner version={} client_api_version={} server_version={} server_api_version={} \
            workflow_id={} hostname={} output_dir={} resources={:?} rules={:?} \
            job_completion_poll_interval={}s max_parallel_jobs={:?} end_time={:?} strict_scheduler_match={} \
            execution_mode={:?} limit_resources={}",
            version,
            version_check::CLIENT_API_VERSION,
            server_version,
            server_api_version,
            self.workflow_id,
            hostname,
            self.output_dir.display(),
            self.resources,
            self.rules,
            self.job_completion_poll_interval,
            self.max_parallel_jobs,
            self.end_time,
            self.torc_config.client.slurm.strict_scheduler_match,
            exec_mode,
            self.execution_config.limit_resources(),
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
                Self::box_retry_error(apis::workflows_api::is_workflow_complete(
                    &self.config,
                    self.workflow_id,
                ))
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
                    self.kill_running_jobs();
                    return Err(
                        format!("Unable to check workflow completion: {}", retry_err).into(),
                    );
                }
            }

            let completions = match self.check_job_status() {
                Ok(count) => count,
                Err(e) => {
                    self.kill_running_jobs();
                    return Err(e);
                }
            };
            if self.execution_config.limit_resources()
                && exec_mode == ExecutionMode::Direct
                && let Err(e) = self.handle_oom_violations()
            {
                self.kill_running_jobs();
                return Err(e);
            }
            self.check_and_execute_actions();

            debug!("Check for new jobs");
            if let Some(max) = self.max_parallel_jobs {
                // Parallelism-based mode: skip if already at max parallel jobs
                if (self.running_jobs.len() as i64) < max {
                    self.run_ready_jobs_based_on_user_parallelism();
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
                    self.run_ready_jobs_based_on_resources();
                } else {
                    debug!(
                        "Skipping job claim: no capacity (cpus={}, memory_gb={:.2})",
                        self.resources.num_cpus, self.resources.memory_gb
                    );
                }
            }

            // Skip the poll-interval wait when this iteration reported one or
            // more completions. Completions free capacity and the deferred
            // unblock task may have made more jobs ready in the meantime, so
            // reacting immediately closes the idle gap between a short job
            // completing and the next job filling its slot.
            //
            // When we do wait, use the SIGCHLD-aware wakeup primitive instead
            // of a plain sleep. A subprocess that exits during the wait
            // delivers SIGCHLD to this process; the signal-handler thread
            // calls `wakeup.notify()`, and we re-enter the loop to call
            // `try_wait` on each child immediately. This eliminates the case
            // where short jobs spawned in the prior iteration finish during
            // the wait but aren't observed until the full interval elapses.
            if completions == 0 {
                self.wakeup
                    .wait_with_timeout(Duration::from_secs_f64(self.job_completion_poll_interval));
            }

            if self.is_termination_requested() {
                info!("Termination requested (SIGTERM received). Terminating jobs.");
                self.terminate_jobs();
                break;
            }

            if let Some(end_time_dt) = self.end_time {
                if exec_mode == ExecutionMode::Direct {
                    let timeout_start = self.direct_mode_timeout_start_time(end_time_dt);
                    if Utc::now() >= timeout_start {
                        info!(
                            "Direct-mode timeout window reached. Starting termination sequence \
                            workflow_id={} timeout_start={} end_time={} sigterm_lead_seconds={} \
                            sigkill_headroom_seconds={}",
                            self.workflow_id,
                            timeout_start,
                            end_time_dt,
                            self.execution_config.sigterm_lead_seconds(),
                            self.execution_config.sigkill_headroom_seconds()
                        );
                        self.terminate_jobs();
                        break;
                    }
                } else if Utc::now() >= end_time_dt {
                    info!(
                        "End time reached. Terminating jobs and stopping job runner \
                        workflow_id={} end_time={}",
                        self.workflow_id, end_time_dt
                    );
                    self.terminate_jobs();
                    break;
                }
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

        // Shutdown resource monitor if enabled. Capture the plot request before shutdown
        // consumes the monitor.
        let plot_request = self
            .resource_monitor
            .as_ref()
            .filter(|m| m.generate_plots())
            .and_then(|m| m.timeseries_db_path().map(Path::to_path_buf));
        let system_metrics_summary = if let Some(monitor) = self.resource_monitor.take() {
            info!("Shutting down resource monitor");
            monitor.shutdown()
        } else {
            None
        };

        if let Some(db_path) = plot_request {
            self.generate_resource_plots(&db_path);
        }

        // Deactivate compute node and set duration
        self.deactivate_compute_node(system_metrics_summary);

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

    /// Kill all running child processes without making any API calls.
    ///
    /// Used when the server is unreachable and we need to exit immediately.
    /// Jobs are left in their current server-side status (likely "running");
    /// the server will detect them as stale when the compute node is no longer
    /// reporting in.
    fn kill_running_jobs(&mut self) {
        if self.running_jobs.is_empty() {
            return;
        }
        error!(
            "Killing {} running job(s) due to unrecoverable API failure workflow_id={}",
            self.running_jobs.len(),
            self.workflow_id
        );
        for (job_id, async_job) in self.running_jobs.iter_mut() {
            if let Err(e) = async_job.send_sigkill() {
                warn!(
                    "Failed to SIGKILL job workflow_id={} job_id={}: {}",
                    self.workflow_id, job_id, e
                );
            }
        }
    }

    /// Generate HTML resource plots from the time-series metrics DB produced by the
    /// resource monitor. No-op (with a warning) when the binary was not built with the
    /// `plot_resources` feature.
    fn generate_resource_plots(&self, db_path: &Path) {
        #[cfg(feature = "plot_resources")]
        {
            let output_dir = db_path.parent().unwrap_or(&self.output_dir).to_path_buf();
            let args = crate::plot_resources_cmd::Args {
                db_paths: vec![db_path.to_path_buf()],
                output_dir,
                job_ids: Vec::new(),
                prefix: String::new(),
                format: "html".to_string(),
            };
            info!(
                "Generating resource plots from {} workflow_id={}",
                db_path.display(),
                self.workflow_id
            );
            if let Err(e) = crate::plot_resources_cmd::run(&args) {
                error!("Failed to generate resource plots: {}", e);
            }
        }
        #[cfg(not(feature = "plot_resources"))]
        {
            let _ = db_path;
            warn!(
                "resource_monitor.generate_plots=true but this binary was built without the \
                 'plot_resources' feature; skipping plot generation"
            );
        }
    }

    /// Deactivate the compute node and set its duration.
    fn deactivate_compute_node(&self, system_metrics_summary: Option<SystemMetricsSummary>) {
        let duration_seconds = self.start_instant.elapsed().as_secs_f64();
        info!(
            "Compute node deactivated workflow_id={} run_id={} compute_node_id={} duration_s={:.1}",
            self.workflow_id, self.run_id, self.compute_node_id, duration_seconds
        );

        // Fetch the existing compute node first to preserve all fields
        let mut update_model =
            match apis::compute_nodes_api::get_compute_node(&self.config, self.compute_node_id) {
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
        if let Some(summary) = system_metrics_summary {
            update_model.sample_count = Some(summary.sample_count);
            update_model.peak_cpu_percent = Some(summary.peak_cpu_percent);
            update_model.avg_cpu_percent = Some(summary.avg_cpu_percent);
            update_model.peak_memory_bytes = Some(summary.peak_memory_bytes as i64);
            update_model.avg_memory_bytes = Some(summary.avg_memory_bytes as i64);
        }

        if let Err(e) = apis::compute_nodes_api::update_compute_node(
            &self.config,
            self.compute_node_id,
            update_model,
        ) {
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
        if let Err(e) = self.handle_completions_batch(results) {
            error!(
                "Failed to record canceled job completions workflow_id={}: {}",
                self.workflow_id, e
            );
        }
    }

    /// Returns when direct-mode timeout handling should start for a given end time.
    ///
    /// The runner begins graceful termination at:
    /// `end_time - sigkill_headroom_seconds - sigterm_lead_seconds`
    ///
    /// This allows `terminate_jobs()` to send the configured termination signal first,
    /// then wait `sigterm_lead_seconds`, and finally send SIGKILL at the configured
    /// `sigkill_headroom_seconds` boundary.
    fn direct_mode_timeout_start_time(&self, end_time: DateTime<Utc>) -> DateTime<Utc> {
        let total_lead = self.execution_config.sigkill_headroom_seconds()
            + self.execution_config.sigterm_lead_seconds();
        end_time - chrono::Duration::seconds(total_lead)
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
    ///    - Jobs that exited cleanly (exit code 0) are set to `JobStatus::Completed`
    ///    - Jobs that crashed or were force-killed are set to `JobStatus::Terminated`
    ///    - Results include execution time and resource metrics (if monitoring is enabled)
    ///
    /// Terminates all running jobs with a graceful shutdown timeline.
    ///
    /// The termination timeline (for direct mode) is:
    /// 1. Send termination signal (configurable, default SIGTERM) to all jobs
    /// 2. Wait `sigterm_lead_seconds` (default 30) for jobs to exit gracefully
    /// 3. Send SIGKILL to any jobs still running
    /// 4. Wait for all jobs to complete
    ///
    /// In Slurm mode, srun handles the termination timeline, so we just send SIGTERM.
    ///
    /// Called automatically by `run_worker()` when:
    /// - The termination flag is set (typically by a SIGTERM signal handler)
    /// - The compute node's end time is reached
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

        // Track which jobs were force-killed (did not respond to the graceful signal).
        // These get the configured timeout_exit_code; jobs that exited on their own
        // keep their actual exit code.
        let mut force_killed: std::collections::HashSet<i64> = std::collections::HashSet::new();

        // In direct mode, we manage the termination timeline ourselves.
        // In Slurm mode, we SIGTERM the srun wrapper processes so they exit
        // promptly rather than blocking wait_for_completion() indefinitely.
        if self.execution_config.effective_mode() == ExecutionMode::Direct {
            let termination_signal = self.execution_config.termination_signal();
            let sigterm_lead_seconds = self.execution_config.sigterm_lead_seconds();

            // First pass: send termination signal to all running jobs
            for (job_id, async_job) in self.running_jobs.iter_mut() {
                info!(
                    "Job {} workflow_id={} job_id={}",
                    termination_signal, self.workflow_id, job_id
                );
                if let Err(e) = async_job.send_signal(termination_signal) {
                    warn!(
                        "Job {} failed workflow_id={} job_id={} error={}",
                        termination_signal, self.workflow_id, job_id, e
                    );
                }
            }

            // Wait for graceful termination before sending SIGKILL
            if sigterm_lead_seconds > 0 {
                info!(
                    "Waiting {}s for graceful termination before SIGKILL",
                    sigterm_lead_seconds
                );
                thread::sleep(Duration::from_secs(sigterm_lead_seconds as u64));

                // Check which jobs exited gracefully during the wait
                for async_job in self.running_jobs.values_mut() {
                    let _ = async_job.check_status();
                }

                // Send SIGKILL to any jobs still running
                for (job_id, async_job) in self.running_jobs.iter_mut() {
                    if async_job.is_running {
                        info!(
                            "Job SIGKILL workflow_id={} job_id={}",
                            self.workflow_id, job_id
                        );
                        force_killed.insert(*job_id);
                        if let Err(e) = async_job.send_sigkill() {
                            warn!(
                                "Job SIGKILL failed workflow_id={} job_id={} error={}",
                                self.workflow_id, job_id, e
                            );
                        }
                    }
                }
            }
        } else {
            // Slurm mode: send SIGTERM to srun wrapper processes so they
            // exit and don't block wait_for_completion() indefinitely.
            for (job_id, async_job) in self.running_jobs.iter_mut() {
                info!(
                    "Job SIGTERM (srun) workflow_id={} job_id={}",
                    self.workflow_id, job_id
                );
                if let Err(e) = async_job.terminate() {
                    warn!(
                        "Job SIGTERM (srun) failed workflow_id={} job_id={} error={}",
                        self.workflow_id, job_id, e
                    );
                }
            }
        }

        // Wait for all jobs to complete and collect results.
        // Jobs that responded to SIGTERM keep their own exit code (the user may
        // want to trigger off it). Jobs that were SIGKILLed get timeout_exit_code.
        let timeout_exit_code = self.execution_config.timeout_exit_code();
        let mut results = Vec::new();
        for (job_id, async_job) in self.running_jobs.iter_mut() {
            // Jobs that already exited during check_status() above are already
            // complete — get_result() works on them without wait_for_completion().
            if !async_job.is_complete {
                match async_job.wait_for_completion() {
                    Ok(exit_code) => {
                        debug!(
                            "Job terminated workflow_id={} job_id={} exit_code={}",
                            self.workflow_id, job_id, exit_code
                        );
                    }
                    Err(e) => {
                        error!(
                            "Job wait failed workflow_id={} job_id={} error={}",
                            self.workflow_id, job_id, e
                        );
                        continue;
                    }
                }
            }

            let attempt_id = async_job.job.attempt_id.unwrap_or(1);
            let mut result = async_job.get_result(
                self.run_id,
                attempt_id,
                self.compute_node_id,
                self.resource_monitor.as_ref(),
            );
            if force_killed.contains(job_id) {
                result.return_code = timeout_exit_code as i64;
            }
            // Jobs that exited cleanly (rc=0) handled termination gracefully - mark as Completed.
            // Jobs that crashed or were force-killed get Terminated status.
            result.status = if result.return_code == 0 {
                JobStatus::Completed
            } else {
                JobStatus::Terminated
            };
            results.push((*job_id, result));
        }

        // Final pass: handle completions (notify server)
        if let Err(e) = self.handle_completions_batch(results) {
            error!(
                "Failed to record terminated job completions workflow_id={}: {}",
                self.workflow_id, e
            );
        }
    }

    /// Check the status of running jobs and remove completed ones.
    /// Detect locally-completed jobs and report them to the server.
    ///
    /// Returns the number of completions handled this iteration. Callers use
    /// this to decide whether the runner should re-enter the main loop
    /// immediately rather than sleeping.
    fn check_job_status(&mut self) -> Result<usize, Box<dyn std::error::Error>> {
        let mut job_results = Vec::new();

        // First pass: check status and collect completed jobs
        for (job_id, async_job) in self.running_jobs.iter_mut() {
            match async_job.check_status() {
                Ok(()) => {
                    if async_job.is_complete {
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

        let completion_count = job_results.len();

        // Second pass: validate output files, then report all completions in one batch.
        let mut to_report = Vec::with_capacity(completion_count);
        for (job_id, mut result, output_file_ids) in job_results {
            if result.return_code == 0
                && let Err(e) = self.validate_and_update_output_files(job_id, &output_file_ids)
            {
                error!("Output file validation failed for job {}: {}", job_id, e);
                result.return_code = 1;
                result.status = JobStatus::Failed;
            }
            to_report.push((job_id, result));
        }
        self.handle_completions_batch(to_report)?;
        Ok(completion_count)
    }

    /// Handle OOM violations detected by the resource monitor.
    ///
    /// When running in direct mode with `limit_resources: true`, the resource monitor
    /// tracks memory usage for each job. If a job exceeds its configured memory limit,
    /// an OOM violation is sent. This method:
    ///
    /// 1. Polls for OOM violations from the resource monitor
    /// 2. Immediately SIGKILLs the violating job (no grace period for OOM)
    /// 3. Waits for the job to exit and collects its result
    /// 4. Reports the job as failed with the configured `oom_exit_code`
    fn handle_oom_violations(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let violations = match &self.resource_monitor {
            Some(monitor) => monitor.recv_oom_violations(),
            None => return Ok(()),
        };

        if violations.is_empty() {
            return Ok(());
        }

        let oom_exit_code = self.execution_config.oom_exit_code();

        // First pass: log and send SIGKILL to all OOM jobs
        let mut killed_job_ids = Vec::new();
        for violation in &violations {
            warn!(
                "OOM violation detected: workflow_id={} job_id={} pid={} memory={:.2}GB limit={:.2}GB",
                self.workflow_id,
                violation.job_id,
                violation.pid,
                violation.memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
                violation.limit_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
            );

            if let Some(async_job) = self.running_jobs.get_mut(&violation.job_id) {
                // Check if still running - job may have exited between OOM detection and now
                if !async_job.is_running {
                    debug!(
                        "OOM job already exited workflow_id={} job_id={}",
                        self.workflow_id, violation.job_id
                    );
                    continue;
                }
                warn!(
                    "Killing OOM job workflow_id={} job_id={}",
                    self.workflow_id, violation.job_id
                );
                if let Err(e) = async_job.send_sigkill() {
                    error!(
                        "Failed to SIGKILL OOM job workflow_id={} job_id={} error={}",
                        self.workflow_id, violation.job_id, e
                    );
                } else {
                    killed_job_ids.push(violation.job_id);
                }
            }
        }

        // Second pass: wait for completion and handle results
        let mut results = Vec::new();
        for job_id in &killed_job_ids {
            if let Some(async_job) = self.running_jobs.get_mut(job_id) {
                match async_job.wait_for_completion() {
                    Ok(_) => {
                        debug!(
                            "OOM job exited workflow_id={} job_id={}",
                            self.workflow_id, job_id
                        );
                    }
                    Err(e) => {
                        error!(
                            "OOM job wait failed workflow_id={} job_id={} error={}",
                            self.workflow_id, job_id, e
                        );
                    }
                }

                let attempt_id = async_job.job.attempt_id.unwrap_or(1);
                let mut result = async_job.get_result(
                    self.run_id,
                    attempt_id,
                    self.compute_node_id,
                    self.resource_monitor.as_ref(),
                );
                result.return_code = oom_exit_code as i64;
                result.status = JobStatus::Failed;
                results.push((*job_id, result));
            }
        }

        // Third pass: handle completions (notify server)
        self.handle_completions_batch(results)?;
        Ok(())
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
            let file_model = match self.send_with_retries(|| {
                Self::box_retry_error(apis::files_api::get_file(&self.config, *file_id))
            }) {
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

        // Update st_mtime for all files and collect file models for RO-Crate
        let mut updated_file_models: Vec<crate::models::FileModel> = Vec::new();

        for (file_id, st_mtime) in files_to_update {
            let mut file_model = match self.send_with_retries(|| {
                Self::box_retry_error(apis::files_api::get_file(&self.config, file_id))
            }) {
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
                Self::box_retry_error(apis::files_api::update_file(
                    &self.config,
                    file_id,
                    file_model.clone(),
                ))
            }) {
                Ok(_) => {
                    debug!("Updated st_mtime for file_id {} to {}", file_id, st_mtime);
                    updated_file_models.push(file_model);
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

        // Create RO-Crate entities for output files if enabled
        self.create_ro_crate_entities_for_output_files(job_id, &updated_file_models);

        Ok(())
    }

    /// Create RO-Crate entities for output files if `enable_ro_crate` is enabled on the workflow.
    ///
    /// Creates both File entities with provenance and a CreateAction entity for the job.
    /// This is a non-blocking operation - warnings are logged but errors don't fail the job.
    fn create_ro_crate_entities_for_output_files(
        &self,
        job_id: i64,
        output_files: &[crate::models::FileModel],
    ) {
        // Check if RO-Crate is enabled
        if self.workflow.enable_ro_crate != Some(true) {
            return;
        }

        if output_files.is_empty() {
            return;
        }

        debug!(
            "Creating RO-Crate entities for {} output files from job {}",
            output_files.len(),
            job_id
        );

        // Fetch the job model to get job name for CreateAction
        let job = match self.send_with_retries(|| {
            Self::box_retry_error(apis::jobs_api::get_job(&self.config, job_id))
        }) {
            Ok(job) => job,
            Err(e) => {
                warn!(
                    "Could not fetch job {} for RO-Crate creation: {}",
                    job_id, e
                );
                return;
            }
        };

        // Use run_id as the attempt_id for the CreateAction
        let attempt_id = self.run_id;

        // Collect output file paths for the CreateAction
        let output_file_paths: Vec<String> = output_files.iter().map(|f| f.path.clone()).collect();

        // Create CreateAction entity for the job
        crate::client::ro_crate_utils::create_create_action_entity(
            &self.config,
            self.workflow_id,
            self.run_id,
            &job,
            attempt_id,
            &output_file_paths,
        );

        // Create File entities for each output file with provenance
        for file in output_files {
            // Get file size if available
            let content_size = std::fs::metadata(&file.path).ok().map(|m| m.len());

            crate::client::ro_crate_utils::create_ro_crate_entity_for_output_file(
                &self.config,
                self.workflow_id,
                self.run_id,
                file,
                content_size,
                job_id,
                attempt_id,
            );
        }
    }

    /// Prepare and report a list of (job_id, result) completions in one batched
    /// server call. Each completion is run through the local-side preparation
    /// pipeline (recovery, status determination, side-effect flags); the
    /// surviving entries (those not retried locally) are then sent in a single
    /// `batch_complete_jobs` request.
    fn handle_completions_batch(
        &mut self,
        completions: Vec<(i64, ResultModel)>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let prepared: Vec<PreparedCompletion> = completions
            .into_iter()
            .filter_map(|(job_id, result)| self.prepare_job_completion(job_id, result))
            .collect();
        self.report_completions_batch(prepared)
    }

    /// Run the local-side preparation for a job completion: collect Slurm stats,
    /// run failure-handler recovery, settle on the final status, and update
    /// runner-level flags. Returns `None` when recovery scheduled a retry (in
    /// which case all local cleanup has already happened); otherwise returns
    /// the data that needs to be reported to the server.
    fn prepare_job_completion(
        &mut self,
        job_id: i64,
        result: ResultModel,
    ) -> Option<PreparedCompletion> {
        // Take sacct stats now, before the result is sent to the server, so we can backfill
        // resource fields.  For srun-wrapped jobs the sysinfo monitor only sees the srun process
        // (negligible overhead), so sacct provides the authoritative peak memory and CPU data.
        let slurm_stats = self
            .running_jobs
            .get_mut(&job_id)
            .and_then(|j| j.take_slurm_stats());

        let mut final_result = result;
        if let Some(ref stats) = slurm_stats {
            backfill_sacct_into_result(&mut final_result, stats);
        }

        let job_info = self.running_jobs.get(&job_id).map(|cmd| {
            (
                cmd.job.name.clone(),
                cmd.job.attempt_id.unwrap_or(1),
                cmd.job.failure_handler_id,
            )
        });

        if matches!(
            final_result.status,
            JobStatus::Failed | JobStatus::Terminated
        ) && let Some((job_name, attempt_id, failure_handler_id)) = &job_info
        {
            let return_code = final_result.return_code;
            let outcome = self.try_recover_job(
                job_id,
                job_name,
                return_code,
                *attempt_id,
                *failure_handler_id,
            );

            match outcome {
                RecoveryOutcome::Retried => {
                    info!(
                        "Job retry scheduled workflow_id={} job_id={} job_name={} return_code={} attempt_id={}",
                        self.workflow_id, job_id, job_name, return_code, attempt_id
                    );
                    if let Some(job_rr) = self.job_resources.get(&job_id).cloned() {
                        self.increment_node_resources(job_id, &job_rr);
                        self.increment_resources(&job_rr);
                    }
                    self.last_job_claimed_time = Some(Instant::now());
                    self.running_jobs.remove(&job_id);
                    self.job_resources.remove(&job_id);
                    return None;
                }
                RecoveryOutcome::NoHandler | RecoveryOutcome::NoMatchingRule => {
                    if self.workflow.use_pending_failed.unwrap_or(false) {
                        info!(
                            "Job pending_failed workflow_id={} job_id={} job_name={} return_code={} reason={:?}",
                            self.workflow_id, job_id, job_name, return_code, outcome
                        );
                        final_result.status = JobStatus::PendingFailed;
                    } else {
                        debug!(
                            "Job failed workflow_id={} job_id={} job_name={} return_code={} reason={:?}",
                            self.workflow_id, job_id, job_name, return_code, outcome
                        );
                    }
                }
                RecoveryOutcome::MaxRetriesExceeded | RecoveryOutcome::Error(_) => {
                    debug!(
                        "Job failed workflow_id={} job_id={} reason={:?}",
                        self.workflow_id, job_id, outcome
                    );
                }
            }
        }

        match final_result.status {
            JobStatus::Failed | JobStatus::PendingFailed => self.had_failures = true,
            JobStatus::Terminated => self.had_terminations = true,
            _ => {}
        }

        Some(PreparedCompletion {
            job_id,
            final_result,
            slurm_stats,
        })
    }

    /// Send a batch of prepared completions in a single request and run the
    /// post-success finalization for each one (slurm stats upload, resource
    /// release, stdio cleanup, removal from local state). Returns an error if
    /// the batch call itself fails after retries; per-completion errors
    /// reported by the server are logged and treated as terminal for that job.
    fn report_completions_batch(
        &mut self,
        prepared: Vec<PreparedCompletion>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if prepared.is_empty() {
            return Ok(());
        }

        let request = BatchCompleteJobsRequest {
            completions: prepared
                .iter()
                .map(|p| JobCompletionEntry {
                    job_id: p.job_id,
                    status: p.final_result.status,
                    run_id: p.final_result.run_id,
                    result: p.final_result.clone(),
                })
                .collect(),
        };

        let response = match self.send_with_retries(|| {
            Self::box_retry_error(apis::workflows_api::batch_complete_jobs(
                &self.config,
                self.workflow_id,
                request.clone(),
            ))
        }) {
            Ok(response) => response,
            Err(e) => {
                error!(
                    "batch_complete_jobs failed after retries workflow_id={} count={} error={}",
                    self.workflow_id,
                    prepared.len(),
                    e
                );
                // Clean up local state for every prepared completion before
                // propagating the batch error so finished subprocesses do not
                // keep local resources reserved.
                for p in &prepared {
                    if let Some(job_rr) = self.job_resources.get(&p.job_id).cloned() {
                        self.increment_node_resources(p.job_id, &job_rr);
                        self.increment_resources(&job_rr);
                    }
                    self.running_jobs.remove(&p.job_id);
                    self.job_resources.remove(&p.job_id);
                    self.release_gpu_devices(p.job_id);
                }
                return Err(format!("Unable to record job completions: {}", e).into());
            }
        };

        let completed: std::collections::HashSet<i64> = response.completed.into_iter().collect();
        for err in &response.errors {
            error!(
                "Job complete reported as failed by server workflow_id={} job_id={} message={}",
                self.workflow_id, err.job_id, err.message
            );
        }

        for prep in prepared {
            let PreparedCompletion {
                job_id,
                final_result,
                slurm_stats,
            } = prep;

            if !completed.contains(&job_id) {
                // Server rejected this individual completion. Clean up local
                // state so we don't leak the entry in running_jobs or keep
                // local resources reserved for a finished subprocess.
                if let Some(job_rr) = self.job_resources.get(&job_id).cloned() {
                    self.increment_node_resources(job_id, &job_rr);
                    self.increment_resources(&job_rr);
                }
                self.running_jobs.remove(&job_id);
                self.job_resources.remove(&job_id);
                self.release_gpu_devices(job_id);
                continue;
            }

            let status_str = format!("{:?}", final_result.status).to_lowercase();
            info!(
                "Job completed workflow_id={} job_id={} run_id={} status={}",
                self.workflow_id, job_id, final_result.run_id, status_str
            );

            if let Some(stats) = slurm_stats {
                match apis::slurm_stats_api::create_slurm_stats(&self.config, stats) {
                    Ok(_) => {
                        info!(
                            "Stored slurm_stats workflow_id={} job_id={}",
                            self.workflow_id, job_id
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Failed to store slurm_stats workflow_id={} job_id={}: {}",
                            self.workflow_id, job_id, e
                        );
                    }
                }
            }

            if let Some(job_rr) = self.job_resources.get(&job_id).cloned() {
                self.increment_node_resources(job_id, &job_rr);
                self.increment_resources(&job_rr);
            }
            self.last_job_claimed_time = Some(Instant::now());

            if final_result.return_code == 0
                && let Some(cmd) = self.running_jobs.get(&job_id)
            {
                let job_name = &cmd.job.name;
                if self.execution_config.delete_stdio_on_success(job_name) {
                    Self::cleanup_stdio_files(cmd);
                }
            }

            self.running_jobs.remove(&job_id);
            self.job_resources.remove(&job_id);
            self.release_gpu_devices(job_id);
        }

        Ok(())
    }

    /// Delete stdio files for a completed job.
    fn cleanup_stdio_files(cmd: &AsyncCliCommand) {
        cleanup_job_stdio_files(cmd.stdout_path.as_deref(), cmd.stderr_path.as_deref());
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

        let handler = match self.send_with_retries(|| {
            Self::box_retry_error(apis::failure_handlers_api::get_failure_handler(
                &self.config,
                fh_id,
            ))
        }) {
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
            Self::box_retry_error(apis::jobs_api::retry_job(
                &self.config,
                job_id,
                self.run_id,
                rule.max_retries,
            ))
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

    /// Convert resources to per-node values for server comparison.
    ///
    /// The server compares job resource requirements (which are per-node) against
    /// worker resources, so we must send per-node values.
    ///
    /// For multi-node allocations with a `PerNodeTracker`, we report the maximum
    /// available resources across all nodes. This ensures the server returns jobs
    /// that fit on at least one node. Without per-node tracking, we fall back to
    /// dividing the remaining total by `num_nodes` (correct for single-node
    /// allocations where `num_nodes == 1`).
    fn resources_per_node(&self) -> ComputeNodesResources {
        let (cpus, memory_gb, gpus) = if let Some(ref tracker) = self.node_tracker {
            tracker.max_available()
        } else {
            let num_nodes = self.resources.num_nodes.max(1);
            (
                self.resources.num_cpus / num_nodes,
                self.resources.memory_gb / num_nodes as f64,
                self.resources.num_gpus / num_nodes,
            )
        };
        let mut per_node =
            ComputeNodesResources::new(cpus, memory_gb, gpus, self.resources.num_nodes);
        per_node.scheduler_config_id = self.resources.scheduler_config_id;
        per_node.time_limit.clone_from(&self.resources.time_limit);
        per_node
    }

    fn reserved_node_count(rr: &ResourceRequirementsModel) -> i64 {
        rr.num_nodes.max(1)
    }

    fn is_multi_node_job(rr: &ResourceRequirementsModel) -> bool {
        Self::reserved_node_count(rr) > 1
    }

    fn decrement_resources(&mut self, rr: &ResourceRequirementsModel) {
        if Self::is_multi_node_job(rr) {
            // Resource requirements are per-node values, so multiply by the
            // number of nodes the job reserves to get the total consumption.
            let reserved_nodes = Self::reserved_node_count(rr);
            let job_memory_gb = memory_string_to_gb(&rr.memory);
            self.resources.memory_gb -= job_memory_gb * reserved_nodes as f64;
            self.resources.num_cpus -= rr.num_cpus * reserved_nodes;
            self.resources.num_gpus -= rr.num_gpus * reserved_nodes;
            self.resources.num_nodes -= reserved_nodes;
        } else {
            let job_memory_gb = memory_string_to_gb(&rr.memory);
            self.resources.memory_gb -= job_memory_gb;
            self.resources.num_cpus -= rr.num_cpus;
            self.resources.num_gpus -= rr.num_gpus;
        }
        assert!(self.resources.memory_gb >= 0.0);
        assert!(self.resources.num_cpus >= 0);
        assert!(self.resources.num_gpus >= 0);
        assert!(self.resources.num_nodes >= 0);
    }

    fn increment_resources(&mut self, rr: &ResourceRequirementsModel) {
        if Self::is_multi_node_job(rr) {
            let reserved_nodes = Self::reserved_node_count(rr);
            let job_memory_gb = memory_string_to_gb(&rr.memory);
            self.resources.memory_gb += job_memory_gb * reserved_nodes as f64;
            self.resources.num_cpus += rr.num_cpus * reserved_nodes;
            self.resources.num_gpus += rr.num_gpus * reserved_nodes;
            self.resources.num_nodes += reserved_nodes;
        } else {
            let job_memory_gb = memory_string_to_gb(&rr.memory);
            self.resources.memory_gb += job_memory_gb;
            self.resources.num_cpus += rr.num_cpus;
            self.resources.num_gpus += rr.num_gpus;
        }
        assert!(self.resources.memory_gb <= self.orig_resources.memory_gb);
        assert!(self.resources.num_cpus <= self.orig_resources.num_cpus);
        assert!(self.resources.num_gpus <= self.orig_resources.num_gpus);
        assert!(self.resources.num_nodes <= self.orig_resources.num_nodes);
    }

    /// Increment per-node resources when a job completes. Called alongside
    /// `increment_resources` which tracks the total pool.
    fn increment_node_resources(&mut self, job_id: i64, rr: &ResourceRequirementsModel) {
        if let Some(node_list) = self.job_nodes.remove(&job_id)
            && let Some(ref mut tracker) = self.node_tracker
        {
            let job_memory_gb = memory_string_to_gb(&rr.memory);
            let nodes = expand_slurm_nodelist(&node_list);
            for node in &nodes {
                tracker.increment(node, rr.num_cpus, job_memory_gb, rr.num_gpus);
            }
        }
    }

    /// Decrement per-node resources and record the job-to-node mapping.
    fn track_node_resources(
        &mut self,
        job_id: i64,
        node_name: &str,
        rr: &ResourceRequirementsModel,
    ) {
        if Self::is_multi_node_job(rr) {
            return;
        }
        if let Some(ref mut tracker) = self.node_tracker {
            let job_memory_gb = memory_string_to_gb(&rr.memory);
            tracker.decrement(node_name, rr.num_cpus, job_memory_gb, rr.num_gpus);
            self.job_nodes.insert(job_id, node_name.to_string());
        }
    }

    /// Update the time_limit in resources based on remaining time until end_time.
    /// This ensures the server only returns jobs whose runtime fits within the remaining
    /// allocation time. A startup grace period is added so that a job with runtime=PT1H
    /// can be claimed on a 1-hour allocation even if the runner started 1-2 minutes late.
    /// This is safe because srun --time enforces the actual per-step walltime.
    const STARTUP_GRACE_PERIOD_SECONDS: u64 = 120;

    fn update_remaining_time_limit(&mut self) {
        if let Some(end_time) = self.end_time {
            let now = Utc::now();
            if end_time > now {
                let remaining_seconds =
                    (end_time - now).num_seconds() as u64 + Self::STARTUP_GRACE_PERIOD_SECONDS;
                let time_limit = format_duration_iso8601(remaining_seconds);
                debug!(
                    "Updating time_limit to {} ({} seconds remaining + {}s grace period)",
                    time_limit,
                    (end_time - now).num_seconds(),
                    Self::STARTUP_GRACE_PERIOD_SECONDS
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

        if self.node_tracker.is_some() {
            // Multi-node: claim and start jobs per-node so each claim uses that
            // node's actual available resources and we can pin jobs via --nodelist.
            let node_names: Vec<String> = self
                .node_tracker
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .map(|n| n.name.clone())
                .collect();
            for node_name in node_names {
                self.claim_and_start_jobs_for_node(Some(&node_name));
            }
        } else {
            // Single-node: one claim call, no --nodelist pinning.
            self.claim_and_start_jobs_for_node(None);
        }
    }

    /// Claim ready jobs from the server and start them. When `target_node` is
    /// Some, the claim uses that node's available resources and srun is invoked
    /// with `--nodelist=<node>` to pin the step. When None, the aggregate
    /// resources are used and no node pinning is done (single-node path).
    fn claim_and_start_jobs_for_node(&mut self, target_node: Option<&str>) {
        let per_node = if let Some(node_name) = target_node {
            // Build resources from this specific node's availability
            let tracker = self.node_tracker.as_ref().unwrap();
            let node = match tracker.nodes.iter().find(|n| n.name == node_name) {
                Some(n) => n,
                None => return,
            };
            // Send num_nodes=1 because this claim represents a single node's
            // available resources. The PerNodeTracker path is only used when
            // there are no multi-node jobs, so the SQL filter rr.num_nodes <= 1
            // correctly excludes multi-node jobs.
            let mut r = ComputeNodesResources::new(
                node.available_cpus,
                node.available_memory_gb,
                node.available_gpus,
                1,
            );
            r.scheduler_config_id = self.resources.scheduler_config_id;
            r.time_limit.clone_from(&self.resources.time_limit);
            r
        } else {
            self.resources_per_node()
        };

        // Skip nodes with no available resources
        if per_node.num_cpus <= 0 {
            return;
        }

        let limit = per_node.num_cpus;
        let strict_scheduler_match = self.torc_config.client.slurm.strict_scheduler_match;
        match self.send_with_retries(|| {
            Self::box_retry_error(apis::workflows_api::claim_jobs_based_on_resources(
                &self.config,
                self.workflow_id,
                limit,
                per_node.clone(),
                Some(strict_scheduler_match),
            ))
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
                debug!(
                    "Found {} ready jobs to execute{}",
                    jobs.len(),
                    target_node.map_or(String::new(), |n| format!(" on node {}", n))
                );

                self.last_job_claimed_time = Some(Instant::now());

                for job in jobs {
                    let job_id = job.id.expect("Job must have an ID");
                    let rr_id = job
                        .resource_requirements_id
                        .expect("Job must have a resource_requirements_id");
                    let mut async_job = AsyncCliCommand::new(job);
                    let effective_job_env = async_job.job.env.clone();

                    let job_rr = match self.send_with_retries(|| {
                        Self::box_retry_error(
                            apis::resource_requirements_api::get_resource_requirements(
                                &self.config,
                                rr_id,
                            ),
                        )
                    }) {
                        Ok(rr) => rr,
                        Err(e) => {
                            error!(
                                "Failed to get resource requirements after retries \
                                 workflow_id={} job_id={} rr_id={}: {}",
                                self.workflow_id, job_id, rr_id, e
                            );
                            self.revert_job_to_ready(job_id);
                            continue;
                        }
                    };

                    match self.send_with_retries(|| {
                        Self::box_retry_error(apis::jobs_api::start_job(
                            &self.config,
                            job_id,
                            self.run_id,
                            self.compute_node_id,
                        ))
                    }) {
                        Ok(_) => {
                            debug!("Successfully marked job {} as started in database", job_id);
                        }
                        Err(e) => {
                            error!(
                                "Failed to mark job as started after retries \
                                 workflow_id={} job_id={}: {}",
                                self.workflow_id, job_id, e
                            );
                            self.revert_job_to_ready(job_id);
                            continue;
                        }
                    }

                    let attempt_id = async_job.job.attempt_id.unwrap_or(1);
                    let effective_mode = self.execution_config.effective_mode();
                    let gpu_visible_devices = if effective_mode == ExecutionMode::Slurm {
                        None
                    } else {
                        self.allocate_gpu_devices(job_id, job_rr.num_gpus)
                    };
                    let stdio_config = self.execution_config.stdio_for_job(&async_job.job.name);
                    match async_job.start(
                        &self.output_dir,
                        self.workflow_id,
                        self.run_id,
                        attempt_id,
                        self.resource_monitor.as_ref(),
                        &self.config.base_path,
                        Some(&job_rr),
                        effective_job_env.as_ref(),
                        gpu_visible_devices.as_deref(),
                        self.execution_config.limit_resources(),
                        effective_mode,
                        self.execution_config.enable_cpu_bind(),
                        self.end_time,
                        self.execution_config.srun_termination_signal.as_deref(),
                        self.execution_config.sigkill_headroom_seconds(),
                        target_node,
                        &stdio_config.mode,
                    ) {
                        Ok(()) => {
                            info!(
                                "Job started workflow_id={} job_id={} run_id={} compute_node_id={} attempt_id={}{}",
                                self.workflow_id,
                                job_id,
                                self.run_id,
                                self.compute_node_id,
                                attempt_id,
                                target_node.map_or(String::new(), |n| format!(" node={}", n))
                            );
                            if let Some(node) = target_node {
                                self.track_node_resources(job_id, node, &job_rr);
                            }
                            self.running_jobs.insert(job_id, async_job);
                            self.decrement_resources(&job_rr);
                            self.job_resources.insert(job_id, job_rr);
                        }
                        Err(e) => {
                            error!(
                                "Job start failed workflow_id={} job_id={} error={}",
                                self.workflow_id, job_id, e
                            );
                            self.revert_job_to_ready(job_id);
                            continue;
                        }
                    }
                }
            }
            Err(err) => {
                error!("Failed to prepare jobs for submission: {}", err);
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
            Self::box_retry_error(apis::workflows_api::claim_next_jobs(
                &self.config,
                self.workflow_id,
                Some(limit),
            ))
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
                    let rr_id = job
                        .resource_requirements_id
                        .expect("Job must have a resource_requirements_id");
                    let mut async_job = AsyncCliCommand::new(job);
                    let effective_job_env = async_job.job.env.clone();

                    let job_rr = match self.send_with_retries(|| {
                        Self::box_retry_error(
                            apis::resource_requirements_api::get_resource_requirements(
                                &self.config,
                                rr_id,
                            ),
                        )
                    }) {
                        Ok(rr) => rr,
                        Err(e) => {
                            error!(
                                "Failed to get resource requirements after retries \
                                 workflow_id={} job_id={} rr_id={}: {}",
                                self.workflow_id, job_id, rr_id, e
                            );
                            self.revert_job_to_ready(job_id);
                            continue;
                        }
                    };

                    // Mark job as started in the database before actually starting it
                    match self.send_with_retries(|| {
                        Self::box_retry_error(apis::jobs_api::start_job(
                            &self.config,
                            job_id,
                            self.run_id,
                            self.compute_node_id,
                        ))
                    }) {
                        Ok(_) => {
                            debug!("Successfully marked job {} as started in database", job_id);
                        }
                        Err(e) => {
                            error!(
                                "Failed to mark job as started after retries \
                                 workflow_id={} job_id={}: {}",
                                self.workflow_id, job_id, e
                            );
                            self.revert_job_to_ready(job_id);
                            continue;
                        }
                    }

                    let attempt_id = async_job.job.attempt_id.unwrap_or(1);
                    let effective_mode = self.execution_config.effective_mode();
                    let gpu_visible_devices = if effective_mode == ExecutionMode::Slurm {
                        None
                    } else {
                        self.allocate_gpu_devices(job_id, job_rr.num_gpus)
                    };
                    let stdio_config = self.execution_config.stdio_for_job(&async_job.job.name);
                    match async_job.start(
                        &self.output_dir,
                        self.workflow_id,
                        self.run_id,
                        attempt_id,
                        self.resource_monitor.as_ref(),
                        &self.config.base_path,
                        Some(&job_rr),
                        effective_job_env.as_ref(),
                        gpu_visible_devices.as_deref(),
                        self.execution_config.limit_resources(),
                        effective_mode,
                        self.execution_config.enable_cpu_bind(),
                        self.end_time,
                        self.execution_config.srun_termination_signal.as_deref(),
                        self.execution_config.sigkill_headroom_seconds(),
                        None, // target_node: user-parallelism mode doesn't use per-node placement
                        &stdio_config.mode,
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
                            self.revert_job_to_ready(job_id);
                            continue;
                        }
                    }
                }
            }
            Err(err) => {
                error!(
                    "Failed to claim jobs after retries workflow_id={}: {}",
                    self.workflow_id, err
                );
            }
        }
    }

    /// Revert a job's status back to Ready after a failed start attempt.
    ///
    /// This allows the job to be picked up by another worker. Also releases any
    /// GPU devices that were reserved for the job.
    fn revert_job_to_ready(&mut self, job_id: i64) {
        match self.send_with_retries(|| {
            Self::box_retry_error(apis::jobs_api::manage_status_change(
                &self.config,
                job_id,
                JobStatus::Ready,
                self.run_id,
            ))
        }) {
            Ok(_) => {
                info!(
                    "Reverted job to ready workflow_id={} job_id={}",
                    self.workflow_id, job_id
                );
            }
            Err(revert_err) => {
                error!(
                    "Failed to revert job to ready workflow_id={} job_id={} error={}",
                    self.workflow_id, job_id, revert_err
                );
            }
        }
        self.release_gpu_devices(job_id);
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
                let actions = apis::workflow_actions_api::get_pending_actions(
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
                let actions = apis::workflow_actions_api::get_pending_actions(
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
                let actions = apis::workflow_actions_api::get_workflow_actions(
                    &self.config,
                    self.workflow_id,
                )?;
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
                        start_one_worker_per_node,
                        "",
                        "torc_output",
                        self.torc_config.client.slurm.poll_interval,
                        max_parallel_jobs,
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
}

impl ComputeNodeRules {
    pub fn new(
        compute_node_wait_for_new_jobs_seconds: Option<i64>,
        compute_node_ignore_workflow_completion: Option<bool>,
        compute_node_wait_for_healthy_database_minutes: Option<i64>,
        compute_node_min_time_for_new_jobs_seconds: Option<i64>,
    ) -> Self {
        ComputeNodeRules {
            compute_node_wait_for_new_jobs_seconds: compute_node_wait_for_new_jobs_seconds
                .unwrap_or(90) as u64,
            compute_node_ignore_workflow_completion: compute_node_ignore_workflow_completion
                .unwrap_or(false),
            compute_node_wait_for_healthy_database_minutes:
                compute_node_wait_for_healthy_database_minutes.unwrap_or(20) as u64,
            compute_node_min_time_for_new_jobs_seconds: compute_node_min_time_for_new_jobs_seconds
                .unwrap_or(300) as u64,
        }
    }
}

/// Delete stdio files for a completed job given optional stdout and stderr paths.
///
/// Silently ignores files that don't exist (e.g., when using `NoStdout` or `NoStderr` modes).
pub fn cleanup_job_stdio_files(stdout_path: Option<&str>, stderr_path: Option<&str>) {
    for path in [stdout_path, stderr_path].iter().copied().flatten() {
        match std::fs::remove_file(path) {
            Ok(()) => {
                debug!("Deleted stdio file: {}", path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                warn!("Failed to delete stdio file {}: {}", path, e);
            }
        }
    }
}

/// Backfill Slurm sacct accounting data into a [`ResultModel`] result.
///
/// When a job runs through `srun`, torc's sysinfo-based resource monitor only sees the
/// srun launcher process (negligible overhead), not the actual job.  This function fills
/// the summary resource fields from the authoritative sacct record collected after job
/// completion.
///
/// Fields updated:
/// - `peak_memory_bytes` ← `max_rss_bytes` (sacct MaxRSS, the step's peak RSS)
/// - `avg_cpu_percent`   ← `ave_cpu_seconds / exec_time_s * 100`  (lifetime average)
/// - `peak_cpu_percent`  ← same formula, only when the sstat time-series left it at zero
///   (sacct does not provide an instantaneous CPU peak, but the avg is better than 0%)
///
/// `avg_memory_bytes` is left as-is: sacct does not provide an average RSS; that comes
/// from the sstat time-series if TimeSeries monitoring was configured.
/// This ensures that even when sstat time-series monitoring missed a spike, the sacct
/// post-mortem data fills in accurate resource usage.
fn backfill_sacct_into_result(result: &mut ResultModel, stats: &SlurmStatsModel) {
    if let Some(max_rss) = stats.max_rss_bytes {
        // sacct MaxRSS is the job-lifetime peak memory. Take the max against any
        // sstat-based value already in result (sstat may have seen a brief spike between
        // sacct samples). Also skip updating if sacct reports 0: this happens for very
        // short or failed steps where the accounting daemon never flushed real data, and
        // we do not want to clobber a meaningful sstat measurement with a zero.
        if max_rss > 0 {
            let current = result.peak_memory_bytes.unwrap_or(0);
            result.peak_memory_bytes = Some(current.max(max_rss));
        }
    }
    if let Some(ave_cpu_s) = stats.ave_cpu_seconds {
        let exec_s = result.exec_time_minutes * 60.0;
        // Skip the update when ave_cpu_s is 0: a zero usually means the step finished
        // before accounting was collected (not that the job used no CPU). Keeping any
        // sstat-derived avg_cpu_percent is more informative than replacing it with 0%.
        if exec_s > 0.0 && ave_cpu_s > 0.0 {
            let avg_pct = ave_cpu_s / exec_s * 100.0;
            // Sanity check: reject clearly garbage values (same threshold as
            // JobMetrics::add_sample).
            if avg_pct.is_finite() && avg_pct <= 100_000.0 {
                result.avg_cpu_percent = Some(avg_pct);
                // Use sacct avg as a proxy for peak when sstat gave nothing useful (0% or None).
                // This is better than displaying 0% for jobs where sstat is unavailable.
                let peak_is_zero = result.peak_cpu_percent.unwrap_or(0.0) == 0.0;
                if peak_is_zero {
                    result.peak_cpu_percent = Some(avg_pct);
                }
            }
        }
    }
}

/// Expand a Slurm compact node list into individual node names.
///
/// Uses `scontrol show hostnames` which handles all Slurm node list formats:
/// - Single node: `"node01"` → `["node01"]`
/// - Range: `"node[01-04]"` → `["node01", "node02", "node03", "node04"]`
/// - Mixed: `"node[01,03-05]"` → `["node01", "node03", "node04", "node05"]`
///
/// Falls back to treating the input as a single node name if `scontrol` fails
/// (e.g., not running in a Slurm environment).
fn expand_slurm_nodelist(compact: &str) -> Vec<String> {
    // If there are no brackets, it's already a single node name.
    if !compact.contains('[') {
        return vec![compact.to_string()];
    }

    match std::process::Command::new("scontrol")
        .args(["show", "hostnames", compact])
        .output()
    {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        }
        _ => {
            debug!(
                "scontrol show hostnames failed for '{}', treating as single node",
                compact
            );
            vec![compact.to_string()]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::apis::configuration::Configuration;
    use crate::models::{JobStatus, ResultModel, SlurmStatsModel};
    use serial_test::serial;

    #[test]
    fn wakeup_notify_before_wait_returns_immediately() {
        let w = Wakeup::new();
        w.notify();
        let start = Instant::now();
        let notified = w.wait_with_timeout(Duration::from_secs(2));
        assert!(notified, "wait should report a notification");
        assert!(
            start.elapsed() < Duration::from_millis(100),
            "wait should return immediately when a notification is already pending, took {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn wakeup_notify_during_wait_wakes_waiter() {
        let w = Wakeup::new();
        let w2 = Arc::clone(&w);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            w2.notify();
        });
        let start = Instant::now();
        let notified = w.wait_with_timeout(Duration::from_secs(5));
        let elapsed = start.elapsed();
        assert!(notified, "wait should be notified, not time out");
        assert!(
            elapsed < Duration::from_millis(500),
            "wait should wake shortly after notify, took {:?}",
            elapsed
        );
    }

    #[test]
    fn wakeup_timeout_returns_false_without_notify() {
        let w = Wakeup::new();
        let start = Instant::now();
        let notified = w.wait_with_timeout(Duration::from_millis(50));
        assert!(!notified, "wait should report timeout, not notification");
        assert!(
            start.elapsed() >= Duration::from_millis(40),
            "wait should respect the timeout, only waited {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn wakeup_ignores_spurious_condvar_notifications() {
        let w = Wakeup::new();
        let w2 = Arc::clone(&w);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            w2.cv.notify_all();
        });
        let start = Instant::now();
        let notified = w.wait_with_timeout(Duration::from_millis(60));
        assert!(
            !notified,
            "spurious condvar wake should not look like notify()"
        );
        assert!(
            start.elapsed() >= Duration::from_millis(45),
            "wait should continue until timeout after a spurious wake, only waited {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn wakeup_notification_does_not_persist_across_waits() {
        let w = Wakeup::new();
        w.notify();
        assert!(w.wait_with_timeout(Duration::from_millis(10)));
        let start = Instant::now();
        assert!(!w.wait_with_timeout(Duration::from_millis(50)));
        assert!(start.elapsed() >= Duration::from_millis(40));
    }

    #[test]
    fn wakeup_multiple_notifies_coalesce() {
        let w = Wakeup::new();
        for _ in 0..100 {
            w.notify();
        }
        assert!(w.wait_with_timeout(Duration::from_millis(10)));
        let start = Instant::now();
        assert!(!w.wait_with_timeout(Duration::from_millis(50)));
        assert!(start.elapsed() >= Duration::from_millis(40));
    }

    fn make_result(
        peak_memory_bytes: Option<i64>,
        peak_cpu_percent: Option<f64>,
        avg_cpu_percent: Option<f64>,
        exec_time_minutes: f64,
    ) -> ResultModel {
        let mut r = ResultModel::new(
            1,
            1,
            1,
            1,
            1,
            0,
            exec_time_minutes,
            "2026-01-01T00:00:00Z".to_string(),
            JobStatus::Completed,
        );
        r.peak_memory_bytes = peak_memory_bytes;
        r.peak_cpu_percent = peak_cpu_percent;
        r.avg_cpu_percent = avg_cpu_percent;
        r
    }

    fn make_stats(max_rss_bytes: Option<i64>, ave_cpu_seconds: Option<f64>) -> SlurmStatsModel {
        let mut s = SlurmStatsModel::new(1, 1, 1, 1);
        s.max_rss_bytes = max_rss_bytes;
        s.ave_cpu_seconds = ave_cpu_seconds;
        s
    }

    fn make_runner(resources: ComputeNodesResources) -> JobRunner {
        let mut workflow = WorkflowModel::new("test".to_string(), "user".to_string());
        workflow.id = Some(1);
        JobRunner::new(
            Configuration::default(),
            workflow,
            1,
            1,
            PathBuf::from("/tmp"),
            1.0,
            None,
            None,
            None,
            resources,
            None,
            None,
            None,
            false,
            "test".to_string(),
            None,
        )
    }

    #[test]
    fn test_backfill_sacct_memory_takes_max() {
        // sacct reports higher peak than sstat: use sacct value
        let mut result = make_result(Some(1_000_000), None, None, 1.0);
        let stats = make_stats(Some(2_000_000), None);
        backfill_sacct_into_result(&mut result, &stats);
        assert_eq!(result.peak_memory_bytes, Some(2_000_000));
    }

    #[test]
    fn test_backfill_sacct_memory_keeps_higher_sstat() {
        // sstat already has a higher peak: keep sstat value
        let mut result = make_result(Some(5_000_000), None, None, 1.0);
        let stats = make_stats(Some(2_000_000), None);
        backfill_sacct_into_result(&mut result, &stats);
        assert_eq!(result.peak_memory_bytes, Some(5_000_000));
    }

    #[test]
    fn test_backfill_sacct_memory_fills_none() {
        // No sstat data: sacct fills in
        let mut result = make_result(None, None, None, 1.0);
        let stats = make_stats(Some(1_000_000), None);
        backfill_sacct_into_result(&mut result, &stats);
        assert_eq!(result.peak_memory_bytes, Some(1_000_000));
    }

    #[test]
    fn test_backfill_sacct_memory_skips_zero() {
        // sacct reports 0: don't clobber sstat value
        let mut result = make_result(Some(500_000), None, None, 1.0);
        let stats = make_stats(Some(0), None);
        backfill_sacct_into_result(&mut result, &stats);
        assert_eq!(result.peak_memory_bytes, Some(500_000));
    }

    #[test]
    fn test_backfill_sacct_memory_none_unchanged() {
        // sacct has no memory data: result stays None
        let mut result = make_result(None, None, None, 1.0);
        let stats = make_stats(None, None);
        backfill_sacct_into_result(&mut result, &stats);
        assert_eq!(result.peak_memory_bytes, None);
    }

    #[test]
    fn test_direct_mode_timeout_start_time_subtracts_headroom_and_lead() {
        let mut runner = make_runner(ComputeNodesResources::new(1, 1.0, 0, 1));
        runner.execution_config = ExecutionConfig {
            mode: ExecutionMode::Direct,
            sigterm_lead_seconds: Some(30),
            sigkill_headroom_seconds: Some(60),
            ..Default::default()
        };

        let end_time = Utc::now() + chrono::Duration::hours(1);
        let timeout_start = runner.direct_mode_timeout_start_time(end_time);

        assert_eq!(timeout_start, end_time - chrono::Duration::seconds(90));
    }

    #[test]
    fn test_direct_mode_timeout_start_time_uses_default_values() {
        let mut runner = make_runner(ComputeNodesResources::new(1, 1.0, 0, 1));
        runner.execution_config = ExecutionConfig {
            mode: ExecutionMode::Direct,
            ..Default::default()
        };

        let end_time = Utc::now() + chrono::Duration::hours(1);
        let timeout_start = runner.direct_mode_timeout_start_time(end_time);

        assert_eq!(timeout_start, end_time - chrono::Duration::seconds(90));
    }

    #[test]
    fn test_backfill_sacct_cpu_sets_avg_and_peak() {
        // exec_time = 2 min = 120s, ave_cpu = 120s => 100% avg CPU
        // peak_cpu was None (or 0%) => backfill with avg
        let mut result = make_result(None, None, None, 2.0);
        let stats = make_stats(None, Some(120.0));
        backfill_sacct_into_result(&mut result, &stats);
        assert!((result.avg_cpu_percent.unwrap() - 100.0).abs() < 0.1);
        assert!((result.peak_cpu_percent.unwrap() - 100.0).abs() < 0.1);
    }

    #[test]
    fn test_backfill_sacct_cpu_preserves_nonzero_peak() {
        // peak_cpu already has a non-zero value from sstat: keep it
        let mut result = make_result(None, Some(200.0), None, 2.0);
        let stats = make_stats(None, Some(120.0));
        backfill_sacct_into_result(&mut result, &stats);
        assert!((result.avg_cpu_percent.unwrap() - 100.0).abs() < 0.1);
        assert!((result.peak_cpu_percent.unwrap() - 200.0).abs() < 0.1);
    }

    #[test]
    fn test_backfill_sacct_cpu_skips_zero_ave_cpu() {
        // ave_cpu_seconds = 0: skip (means accounting wasn't collected)
        let mut result = make_result(None, Some(50.0), Some(25.0), 2.0);
        let stats = make_stats(None, Some(0.0));
        backfill_sacct_into_result(&mut result, &stats);
        // Should be unchanged
        assert!((result.avg_cpu_percent.unwrap() - 25.0).abs() < 0.1);
        assert!((result.peak_cpu_percent.unwrap() - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_backfill_sacct_cpu_skips_zero_exec_time() {
        // exec_time = 0: skip (division by zero guard)
        let mut result = make_result(None, None, None, 0.0);
        let stats = make_stats(None, Some(10.0));
        backfill_sacct_into_result(&mut result, &stats);
        assert!(result.avg_cpu_percent.is_none());
        assert!(result.peak_cpu_percent.is_none());
    }

    #[test]
    fn test_per_node_tracker_max_available() {
        let tracker = PerNodeTracker::new(vec!["node01".into(), "node02".into()], 32, 128.0, 4);
        let (cpus, mem, gpus) = tracker.max_available();
        assert_eq!(cpus, 32);
        assert!((mem - 128.0).abs() < 0.01);
        assert_eq!(gpus, 4);
    }

    #[test]
    fn test_per_node_tracker_decrement_reports_correct_max() {
        let mut tracker = PerNodeTracker::new(vec!["node01".into(), "node02".into()], 32, 128.0, 4);
        // Use all of node01's CPUs
        tracker.decrement("node01", 32, 128.0, 4);
        let (cpus, mem, gpus) = tracker.max_available();
        // node02 is still fully available
        assert_eq!(cpus, 32);
        assert!((mem - 128.0).abs() < 0.01);
        assert_eq!(gpus, 4);
    }

    #[test]
    fn test_per_node_tracker_decrement_both_nodes() {
        let mut tracker = PerNodeTracker::new(vec!["node01".into(), "node02".into()], 32, 128.0, 4);
        tracker.decrement("node01", 8, 32.0, 1);
        tracker.decrement("node02", 16, 64.0, 2);
        let (cpus, mem, gpus) = tracker.max_available();
        // node01: 24 CPUs, 96 GB, 3 GPUs
        // node02: 16 CPUs, 64 GB, 2 GPUs
        // max is node01
        assert_eq!(cpus, 24);
        assert!((mem - 96.0).abs() < 0.01);
        assert_eq!(gpus, 3);
    }

    #[test]
    fn test_per_node_tracker_increment_after_decrement() {
        let mut tracker = PerNodeTracker::new(vec!["node01".into(), "node02".into()], 32, 128.0, 4);
        tracker.decrement("node01", 32, 128.0, 4);
        tracker.increment("node01", 32, 128.0, 4);
        let (cpus, mem, gpus) = tracker.max_available();
        assert_eq!(cpus, 32);
        assert!((mem - 128.0).abs() < 0.01);
        assert_eq!(gpus, 4);
    }

    #[test]
    fn test_per_node_tracker_unknown_node_no_panic() {
        let mut tracker = PerNodeTracker::new(vec!["node01".into()], 32, 128.0, 4);
        // Should log a warning but not panic
        tracker.decrement("unknown_node", 8, 32.0, 1);
        tracker.increment("unknown_node", 8, 32.0, 1);
        let (cpus, _, _) = tracker.max_available();
        assert_eq!(cpus, 32); // node01 unchanged
    }

    #[test]
    fn test_expand_slurm_nodelist_single_node() {
        let nodes = expand_slurm_nodelist("node01");
        assert_eq!(nodes, vec!["node01"]);
    }

    #[test]
    fn test_expand_slurm_nodelist_no_brackets_passthrough() {
        // No brackets = single node, no scontrol call needed
        let nodes = expand_slurm_nodelist("compute-node-5");
        assert_eq!(nodes, vec!["compute-node-5"]);
    }

    #[test]
    fn test_multi_node_job_reserves_per_node_resources() {
        // Allocation: 4 nodes, 64 CPUs, 256 GB, 4 GPUs total
        let resources = ComputeNodesResources::new(64, 256.0, 4, 4);
        let mut runner = make_runner(resources);
        // Job: 2 nodes, 16 CPUs/node, 0 GPUs/node, 64g/node
        let rr = ResourceRequirementsModel {
            id: Some(1),
            workflow_id: 1,
            name: "mpi".to_string(),
            num_cpus: 16,
            num_gpus: 0,
            num_nodes: 2,
            memory: "64g".to_string(),
            runtime: "PT1H".to_string(),
        };

        runner.decrement_resources(&rr);

        // Should decrement by job requirements × num_nodes, not allocation capacity
        assert_eq!(runner.resources.num_nodes, 2);
        assert_eq!(runner.resources.num_cpus, 32); // 64 - 16*2
        assert!((runner.resources.memory_gb - 128.0).abs() < 0.01); // 256 - 64*2
        assert_eq!(runner.resources.num_gpus, 4); // 4 - 0*2 (job needs no GPUs)
    }

    #[test]
    fn test_multi_node_gpu_job_reserves_correct_gpus() {
        // Allocation: 2 nodes, 16 CPUs, 64 GB, 4 GPUs total (2 per node)
        let resources = ComputeNodesResources::new(16, 64.0, 4, 2);
        let mut runner = make_runner(resources);
        // Job: 2 nodes, 8 CPUs/node, 1 GPU/node, 16g/node
        let rr = ResourceRequirementsModel {
            id: Some(1),
            workflow_id: 1,
            name: "gpu_mpi".to_string(),
            num_cpus: 8,
            num_gpus: 1,
            num_nodes: 2,
            memory: "16g".to_string(),
            runtime: "PT1H".to_string(),
        };

        runner.decrement_resources(&rr);

        assert_eq!(runner.resources.num_nodes, 0);
        assert_eq!(runner.resources.num_cpus, 0); // 16 - 8*2
        assert!((runner.resources.memory_gb - 32.0).abs() < 0.01); // 64 - 16*2
        assert_eq!(runner.resources.num_gpus, 2); // 4 - 1*2
    }

    #[test]
    fn test_multi_node_job_release_restores_full_nodes() {
        let resources = ComputeNodesResources::new(64, 256.0, 4, 4);
        let mut runner = make_runner(resources.clone());
        let rr = ResourceRequirementsModel {
            id: Some(1),
            workflow_id: 1,
            name: "mpi".to_string(),
            num_cpus: 16,
            num_gpus: 0,
            num_nodes: 2,
            memory: "64g".to_string(),
            runtime: "PT1H".to_string(),
        };

        runner.decrement_resources(&rr);
        runner.increment_resources(&rr);

        assert_eq!(runner.resources.num_nodes, resources.num_nodes);
        assert_eq!(runner.resources.num_cpus, resources.num_cpus);
        assert!((runner.resources.memory_gb - resources.memory_gb).abs() < 0.01);
        assert_eq!(runner.resources.num_gpus, resources.num_gpus);
    }

    /// The original bug: a single-node GPU job followed by a multi-node job
    /// would over-decrement GPUs and panic on the assertion.
    #[test]
    fn test_single_node_then_multi_node_no_panic() {
        // 2 nodes, 4 GPUs total (2 per node), 16 CPUs, 64 GB
        let resources = ComputeNodesResources::new(16, 64.0, 4, 2);
        let mut runner = make_runner(resources);

        // Single-node job takes 1 GPU
        let single = ResourceRequirementsModel {
            id: Some(1),
            workflow_id: 1,
            name: "single_gpu".to_string(),
            num_cpus: 4,
            num_gpus: 1,
            num_nodes: 1,
            memory: "8g".to_string(),
            runtime: "PT1H".to_string(),
        };
        runner.decrement_resources(&single);
        assert_eq!(runner.resources.num_gpus, 3);
        assert_eq!(runner.resources.num_nodes, 2);

        // 2-node job takes 1 GPU/node = 2 GPUs total
        let multi = ResourceRequirementsModel {
            id: Some(2),
            workflow_id: 1,
            name: "multi_gpu".to_string(),
            num_cpus: 4,
            num_gpus: 1,
            num_nodes: 2,
            memory: "8g".to_string(),
            runtime: "PT1H".to_string(),
        };
        runner.decrement_resources(&multi);
        assert_eq!(runner.resources.num_gpus, 1); // 3 - 1*2
        assert_eq!(runner.resources.num_nodes, 0);

        // Release both
        runner.increment_resources(&multi);
        assert_eq!(runner.resources.num_gpus, 3);
        assert_eq!(runner.resources.num_nodes, 2);

        runner.increment_resources(&single);
        assert_eq!(runner.resources.num_gpus, 4);
        assert_eq!(runner.resources.num_nodes, 2);
    }

    /// Multi-node job completes, then single-node jobs use freed resources.
    #[test]
    fn test_multi_node_then_single_node_jobs() {
        // 2 nodes, 4 GPUs total (2 per node)
        let resources = ComputeNodesResources::new(16, 64.0, 4, 2);
        let mut runner = make_runner(resources);

        // 2-node job takes all nodes but only 1 GPU/node
        let multi = ResourceRequirementsModel {
            id: Some(1),
            workflow_id: 1,
            name: "multi".to_string(),
            num_cpus: 4,
            num_gpus: 1,
            num_nodes: 2,
            memory: "8g".to_string(),
            runtime: "PT1H".to_string(),
        };
        runner.decrement_resources(&multi);
        assert_eq!(runner.resources.num_gpus, 2); // 4 - 1*2
        assert_eq!(runner.resources.num_nodes, 0);

        // resources_per_node reports 0 nodes → server won't claim any jobs
        let per_node = runner.resources_per_node();
        assert_eq!(per_node.num_nodes, 0);

        // Multi-node job finishes
        runner.increment_resources(&multi);
        assert_eq!(runner.resources.num_gpus, 4);
        assert_eq!(runner.resources.num_nodes, 2);

        // Now single-node jobs can run
        let single = ResourceRequirementsModel {
            id: Some(2),
            workflow_id: 1,
            name: "single".to_string(),
            num_cpus: 8,
            num_gpus: 2,
            num_nodes: 1,
            memory: "16g".to_string(),
            runtime: "PT1H".to_string(),
        };
        runner.decrement_resources(&single);
        assert_eq!(runner.resources.num_gpus, 2);
        assert_eq!(runner.resources.num_cpus, 8);
    }

    /// Two multi-node jobs run sequentially without resource corruption.
    #[test]
    fn test_sequential_multi_node_jobs() {
        // 4 nodes, 8 GPUs total (2 per node)
        let resources = ComputeNodesResources::new(32, 128.0, 8, 4);
        let mut runner = make_runner(resources.clone());

        let job_a = ResourceRequirementsModel {
            id: Some(1),
            workflow_id: 1,
            name: "job_a".to_string(),
            num_cpus: 8,
            num_gpus: 2,
            num_nodes: 4,
            memory: "32g".to_string(),
            runtime: "PT1H".to_string(),
        };
        runner.decrement_resources(&job_a);
        assert_eq!(runner.resources.num_gpus, 0); // 8 - 2*4
        assert_eq!(runner.resources.num_nodes, 0);

        runner.increment_resources(&job_a);

        // Second job with different resource needs
        let job_b = ResourceRequirementsModel {
            id: Some(2),
            workflow_id: 1,
            name: "job_b".to_string(),
            num_cpus: 4,
            num_gpus: 1,
            num_nodes: 2,
            memory: "16g".to_string(),
            runtime: "PT1H".to_string(),
        };
        runner.decrement_resources(&job_b);
        assert_eq!(runner.resources.num_gpus, 6); // 8 - 1*2
        assert_eq!(runner.resources.num_nodes, 2);

        runner.increment_resources(&job_b);
        assert_eq!(runner.resources.num_gpus, resources.num_gpus);
        assert_eq!(runner.resources.num_nodes, resources.num_nodes);
    }

    /// Mixed single-node and multi-node jobs with GPUs interleaved.
    #[test]
    fn test_mixed_single_and_multi_node_interleaved() {
        // 4 nodes, 16 GPUs total (4 per node), 64 CPUs
        let resources = ComputeNodesResources::new(64, 256.0, 16, 4);
        let mut runner = make_runner(resources.clone());

        // Start a single-node job: 1 GPU, 4 CPUs
        let s1 = ResourceRequirementsModel {
            id: Some(1),
            workflow_id: 1,
            name: "s1".to_string(),
            num_cpus: 4,
            num_gpus: 1,
            num_nodes: 1,
            memory: "16g".to_string(),
            runtime: "PT1H".to_string(),
        };
        runner.decrement_resources(&s1);
        assert_eq!(runner.resources.num_gpus, 15);

        // Start a 2-node job: 2 GPUs/node
        let m1 = ResourceRequirementsModel {
            id: Some(2),
            workflow_id: 1,
            name: "m1".to_string(),
            num_cpus: 8,
            num_gpus: 2,
            num_nodes: 2,
            memory: "32g".to_string(),
            runtime: "PT1H".to_string(),
        };
        runner.decrement_resources(&m1);
        assert_eq!(runner.resources.num_gpus, 11); // 15 - 2*2
        assert_eq!(runner.resources.num_nodes, 2);

        // Start another single-node job
        let s2 = ResourceRequirementsModel {
            id: Some(3),
            workflow_id: 1,
            name: "s2".to_string(),
            num_cpus: 4,
            num_gpus: 3,
            num_nodes: 1,
            memory: "16g".to_string(),
            runtime: "PT1H".to_string(),
        };
        runner.decrement_resources(&s2);
        assert_eq!(runner.resources.num_gpus, 8); // 11 - 3

        // Complete multi-node job
        runner.increment_resources(&m1);
        assert_eq!(runner.resources.num_gpus, 12); // 8 + 2*2
        assert_eq!(runner.resources.num_nodes, 4);

        // Complete both single-node jobs
        runner.increment_resources(&s1);
        runner.increment_resources(&s2);
        assert_eq!(runner.resources.num_gpus, resources.num_gpus);
        assert_eq!(runner.resources.num_cpus, resources.num_cpus);
        assert_eq!(runner.resources.num_nodes, resources.num_nodes);
    }

    /// resources_per_node divides remaining totals by remaining nodes, so the
    /// server sees accurate per-node availability for claiming.
    #[test]
    fn test_resources_per_node_after_multi_node_decrement() {
        // 4 nodes, 8 GPUs total (2 per node), 32 CPUs (8 per node)
        let resources = ComputeNodesResources::new(32, 128.0, 8, 4);
        let mut runner = make_runner(resources);

        // 2-node job takes 1 GPU/node, 4 CPUs/node
        let rr = ResourceRequirementsModel {
            id: Some(1),
            workflow_id: 1,
            name: "multi".to_string(),
            num_cpus: 4,
            num_gpus: 1,
            num_nodes: 2,
            memory: "16g".to_string(),
            runtime: "PT1H".to_string(),
        };
        runner.decrement_resources(&rr);

        let per_node = runner.resources_per_node();
        // 2 nodes remain, 6 GPUs remain → 3 GPUs/node reported
        assert_eq!(per_node.num_nodes, 2);
        assert_eq!(per_node.num_gpus, 3); // 6 / 2
        assert_eq!(per_node.num_cpus, 12); // 24 / 2
    }

    /// When all nodes are consumed, resources_per_node reports 0 nodes so the
    /// server cannot claim any more jobs.
    #[test]
    fn test_resources_per_node_all_nodes_consumed() {
        // 2 nodes, 4 GPUs total
        let resources = ComputeNodesResources::new(16, 64.0, 4, 2);
        let mut runner = make_runner(resources);

        let rr = ResourceRequirementsModel {
            id: Some(1),
            workflow_id: 1,
            name: "full".to_string(),
            num_cpus: 4,
            num_gpus: 1,
            num_nodes: 2,
            memory: "16g".to_string(),
            runtime: "PT1H".to_string(),
        };
        runner.decrement_resources(&rr);

        let per_node = runner.resources_per_node();
        assert_eq!(per_node.num_nodes, 0);
        // num_nodes.max(1) in resources_per_node prevents division by zero;
        // remaining GPUs/CPUs are still visible but 0 nodes blocks claiming.
        assert_eq!(per_node.num_gpus, 2); // 2 GPUs left but 0 nodes
    }

    // =========================================================================
    // GPU device allocation tests
    // =========================================================================

    /// Clear GPU-related env vars so `detect_gpu_devices()` falls back to ordinal
    /// indices, making tests deterministic regardless of the host environment.
    fn clear_gpu_env_vars() {
        // SAFETY: GPU tests are marked #[serial] so no concurrent env var access.
        unsafe {
            std::env::remove_var("CUDA_VISIBLE_DEVICES");
            std::env::remove_var("SLURM_STEP_GPUS");
            std::env::remove_var("SLURM_JOB_GPUS");
        }
    }

    #[test]
    #[serial]
    fn test_allocate_gpu_devices_zero_gpus_returns_none() {
        clear_gpu_env_vars();
        let resources = ComputeNodesResources::new(4, 16.0, 2, 1);
        let mut runner = make_runner(resources);
        assert_eq!(runner.allocate_gpu_devices(1, 0), None);
        assert_eq!(runner.allocate_gpu_devices(1, -1), None);
    }

    #[test]
    #[serial]
    fn test_slurm_mode_keeps_allocation_gpu_count_when_env_is_per_node() {
        clear_gpu_env_vars();
        // SAFETY: GPU tests are marked #[serial] so no concurrent env var access.
        unsafe {
            std::env::set_var("SLURM_JOB_ID", "12345");
            std::env::set_var("CUDA_VISIBLE_DEVICES", "0,1,2,3");
        }

        let resources = ComputeNodesResources::new(64, 256.0, 8, 2);
        let mut workflow = WorkflowModel::new("test".to_string(), "user".to_string());
        workflow.id = Some(1);
        workflow.execution_config = Some(
            serde_json::to_string(&ExecutionConfig {
                mode: ExecutionMode::Slurm,
                ..Default::default()
            })
            .expect("execution config should serialize"),
        );

        let mut runner = JobRunner::new(
            Configuration::default(),
            workflow,
            1,
            1,
            PathBuf::from("/tmp"),
            1.0,
            None,
            None,
            None,
            resources,
            None,
            None,
            None,
            false,
            "test".to_string(),
            None,
        );

        assert_eq!(runner.resources.num_gpus, 8);
        assert_eq!(runner.orig_resources.num_gpus, 8);
        assert_eq!(runner.available_gpu_devices.len(), 8);

        let rr = ResourceRequirementsModel {
            id: Some(1),
            workflow_id: 1,
            name: "multi_gpu".to_string(),
            num_cpus: 16,
            num_gpus: 2,
            num_nodes: 2,
            memory: "64g".to_string(),
            runtime: "PT1H".to_string(),
        };
        runner.decrement_resources(&rr);
        assert_eq!(runner.resources.num_gpus, 4);
        assert_eq!(runner.resources.num_nodes, 0);

        // SAFETY: GPU tests are marked #[serial] so no concurrent env var access.
        unsafe {
            std::env::remove_var("SLURM_JOB_ID");
        }
        clear_gpu_env_vars();
    }

    #[test]
    #[serial]
    fn test_allocate_gpu_devices_normal_allocation() {
        clear_gpu_env_vars();
        let resources = ComputeNodesResources::new(4, 16.0, 4, 1);
        let mut runner = make_runner(resources);

        // Allocate 2 GPUs for job 1
        let result = runner.allocate_gpu_devices(1, 2);
        assert_eq!(result, Some("0,1".to_string()));

        // Allocate 1 GPU for job 2
        let result = runner.allocate_gpu_devices(2, 1);
        assert_eq!(result, Some("2".to_string()));

        // Only 1 GPU left
        assert_eq!(runner.available_gpu_devices.len(), 1);
    }

    #[test]
    #[serial]
    fn test_allocate_gpu_devices_release_returns_to_pool() {
        clear_gpu_env_vars();
        let resources = ComputeNodesResources::new(4, 16.0, 2, 1);
        let mut runner = make_runner(resources);

        // Allocate all GPUs
        let result = runner.allocate_gpu_devices(1, 2);
        assert_eq!(result, Some("0,1".to_string()));
        assert!(runner.available_gpu_devices.is_empty());

        // Release them
        runner.release_gpu_devices(1);
        assert_eq!(runner.available_gpu_devices.len(), 2);

        // Can allocate again
        let result = runner.allocate_gpu_devices(2, 2);
        assert_eq!(result, Some("0,1".to_string()));
    }

    #[test]
    #[serial]
    fn test_allocate_gpu_devices_fallback_on_exhaustion() {
        clear_gpu_env_vars();
        let resources = ComputeNodesResources::new(4, 16.0, 2, 1);
        let mut runner = make_runner(resources);

        // Exhaust the pool
        let result = runner.allocate_gpu_devices(1, 2);
        assert_eq!(result, Some("0,1".to_string()));

        // Pool is empty — should get round-robin fallback
        let result = runner.allocate_gpu_devices(2, 1);
        assert_eq!(result, Some("0".to_string()));

        // Next round-robin picks device 1
        let result = runner.allocate_gpu_devices(3, 1);
        assert_eq!(result, Some("1".to_string()));

        // Wraps around
        let result = runner.allocate_gpu_devices(4, 1);
        assert_eq!(result, Some("0".to_string()));
    }

    #[test]
    #[serial]
    fn test_allocate_gpu_devices_fallback_multi_gpu() {
        clear_gpu_env_vars();
        let resources = ComputeNodesResources::new(4, 16.0, 3, 1);
        let mut runner = make_runner(resources);

        // Exhaust the pool
        runner.allocate_gpu_devices(1, 3);

        // Request 2 GPUs via fallback — should get round-robin across pool of 3
        let result = runner.allocate_gpu_devices(2, 2);
        assert_eq!(result, Some("0,1".to_string()));

        // Next fallback continues from counter=2
        let result = runner.allocate_gpu_devices(3, 2);
        assert_eq!(result, Some("2,0".to_string()));
    }

    #[test]
    #[serial]
    fn test_allocate_gpu_devices_no_pool_returns_none() {
        clear_gpu_env_vars();
        // 0 GPUs configured
        let resources = ComputeNodesResources::new(4, 16.0, 0, 1);
        let mut runner = make_runner(resources);

        // Even with fallback, no devices exist
        let result = runner.allocate_gpu_devices(1, 1);
        assert_eq!(result, None);
    }
}
