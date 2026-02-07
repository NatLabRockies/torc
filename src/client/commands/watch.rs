//! Watch command for monitoring workflows with automatic failure recovery

use env_logger::Builder;
use log::{LevelFilter, debug, error, info, warn};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use crate::client::apis::configuration::Configuration;
use crate::client::apis::default_api;
use crate::client::utils;

// Re-export shared recovery types and functions from the recover module
use super::recover::{
    RecoveryResult, apply_recovery_heuristics, diagnose_failures, regenerate_and_submit,
    reinitialize_workflow, reset_failed_jobs, run_recovery_hook,
};
use crate::client::report_models::ResourceUtilizationReport;

// Use shared orphan detection logic
use super::orphan_detection::cleanup_orphaned_jobs;
// Re-export for backwards compatibility
pub use super::orphan_detection::ORPHANED_JOB_RETURN_CODE;

/// Default wait time for database connectivity issues (in minutes)
const WAIT_FOR_HEALTHY_DATABASE_MINUTES: u64 = 20;

/// Execute an API call with automatic retries for network errors.
/// This wraps utils::send_with_retries with a default timeout.
fn send_with_retries<T, E, F>(config: &Configuration, api_call: F) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
    E: std::fmt::Display,
{
    utils::send_with_retries(config, api_call, WAIT_FOR_HEALTHY_DATABASE_MINUTES)
}
use crate::client::commands::pagination::{JobListParams, paginate_jobs};
use crate::client::hpc::common::HpcJobStatus;
use crate::client::hpc::hpc_interface::HpcInterface;
use crate::client::hpc::slurm_interface::SlurmInterface;
use crate::client::log_paths::get_watch_log_file;

// Note: ORPHANED_JOB_RETURN_CODE is now imported from orphan_detection module

/// A writer that writes to both stdout and a file
struct MultiWriter {
    stdout: std::io::Stdout,
    file: File,
}

impl Write for MultiWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.stdout.write_all(buf)?;
        self.file.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stdout.flush()?;
        self.file.flush()
    }
}

/// Arguments for the watch command
pub struct WatchArgs {
    pub workflow_id: i64,
    pub poll_interval: u64,
    pub recover: bool,
    pub max_retries: u32,
    pub memory_multiplier: f64,
    pub runtime_multiplier: f64,
    pub retry_unknown: bool,
    pub recovery_hook: Option<String>,
    pub output_dir: PathBuf,
    pub show_job_counts: bool,
    pub log_level: String,
    /// Automatically schedule new compute nodes when needed
    pub auto_schedule: bool,
    /// Minimum number of retry jobs before auto-scheduling (when schedulers exist)
    pub auto_schedule_threshold: u32,
    /// Cooldown between auto-schedule attempts (in seconds)
    pub auto_schedule_cooldown: u64,
    /// Maximum time to wait before scheduling stranded retry jobs (in seconds)
    pub auto_schedule_stranded_timeout: u64,
    /// [EXPERIMENTAL] Enable AI-assisted recovery for pending_failed jobs
    pub ai_recovery: bool,
    /// AI agent CLI to use for --ai-recovery (e.g., "claude")
    pub ai_agent: String,
}

/// Get job counts by status for a workflow
fn get_job_counts(
    config: &Configuration,
    workflow_id: i64,
) -> Result<HashMap<String, i64>, String> {
    let jobs = paginate_jobs(config, workflow_id, JobListParams::new())
        .map_err(|e| format!("Failed to list jobs: {}", e))?;
    let mut counts = HashMap::new();

    for job in &jobs {
        if let Some(status) = &job.status {
            let status_str = format!("{:?}", status);
            *counts.entry(status_str).or_insert(0) += 1;
        }
    }

    Ok(counts)
}

/// Count ready jobs that are retries (attempt_id > 1).
/// These are jobs created by failure handlers that need scheduling.
fn count_ready_retry_jobs(config: &Configuration, workflow_id: i64) -> Result<(i64, i64), String> {
    use crate::models::JobStatus;

    let ready_jobs = paginate_jobs(
        config,
        workflow_id,
        JobListParams::new().with_status(JobStatus::Ready),
    )
    .map_err(|e| format!("Failed to list ready jobs: {}", e))?;

    let total_ready = ready_jobs.len() as i64;
    let retry_count = ready_jobs
        .iter()
        .filter(|job| job.attempt_id.unwrap_or(1) > 1)
        .count() as i64;

    Ok((total_ready, retry_count))
}

// Note: fail_orphaned_slurm_jobs and cleanup_dead_pending_slurm_jobs
// are now in orphan_detection module

/// Check if there are any active workers (compute nodes or scheduled compute nodes).
/// This is used after workflow completion to wait for all workers to exit before
/// proceeding with recovery actions. Workers need to complete their cleanup routines.
fn has_active_workers(config: &Configuration, workflow_id: i64) -> bool {
    // Check for active compute nodes (is_active=true)
    if let Ok(response) = send_with_retries(config, || {
        default_api::list_compute_nodes(
            config,
            workflow_id,
            None,       // offset
            Some(1),    // limit - just need one
            None,       // sort_by
            None,       // reverse_sort
            None,       // hostname
            Some(true), // is_active = true
            None,       // scheduled_compute_node_id
        )
    }) && let Some(nodes) = response.items
        && !nodes.is_empty()
    {
        return true;
    }

    // Also check for any scheduled compute nodes (pending or active)
    // These represent Slurm allocations that haven't fully exited yet
    has_any_scheduled_compute_nodes(config, workflow_id)
}

/// Check if there are any scheduled compute nodes with status pending or active.
/// If there are none, the workflow cannot make progress.
fn has_any_scheduled_compute_nodes(config: &Configuration, workflow_id: i64) -> bool {
    // Check for pending allocations
    if let Ok(response) = send_with_retries(config, || {
        default_api::list_scheduled_compute_nodes(
            config,
            workflow_id,
            None,            // offset
            Some(1),         // limit - just need one
            None,            // sort_by
            None,            // reverse_sort
            None,            // scheduler_id
            None,            // scheduler_config_id
            Some("pending"), // status
        )
    }) && let Some(nodes) = response.items
        && !nodes.is_empty()
    {
        return true;
    }

    // Check for active allocations
    if let Ok(response) = send_with_retries(config, || {
        default_api::list_scheduled_compute_nodes(
            config,
            workflow_id,
            None,           // offset
            Some(1),        // limit - just need one
            None,           // sort_by
            None,           // reverse_sort
            None,           // scheduler_id
            None,           // scheduler_config_id
            Some("active"), // status
        )
    }) && let Some(nodes) = response.items
        && !nodes.is_empty()
    {
        return true;
    }

    false
}

/// Check if there is at least one valid Slurm allocation (pending or running in Slurm).
///
/// This is used to optimize the poll loop: if we have valid allocations, we can skip
/// the expensive per-allocation orphan detection and just sleep.
///
/// Returns true if at least one Slurm allocation is still valid (queued or running).
fn has_valid_slurm_allocation(config: &Configuration, workflow_id: i64) -> bool {
    // Get scheduled compute nodes with status="pending" or "active"
    // We'll sample one from each category to check

    // First check for active allocations
    let active_nodes = send_with_retries(config, || {
        default_api::list_scheduled_compute_nodes(
            config,
            workflow_id,
            None,           // offset
            Some(1),        // limit - just need one
            None,           // sort_by
            None,           // reverse_sort
            None,           // scheduler_id
            None,           // scheduler_config_id
            Some("active"), // status
        )
    });

    if let Ok(response) = active_nodes
        && let Some(nodes) = response.items
    {
        for node in nodes {
            if node.scheduler_type.to_lowercase() == "slurm" {
                // Check if this Slurm job is still running
                if let Ok(slurm) = SlurmInterface::new() {
                    let slurm_job_id = node.scheduler_id.to_string();
                    if let Ok(info) = slurm.get_status(&slurm_job_id)
                        && (info.status == HpcJobStatus::Running
                            || info.status == HpcJobStatus::Queued)
                    {
                        debug!(
                            "Found valid active Slurm allocation {} (status: {:?})",
                            slurm_job_id, info.status
                        );
                        return true;
                    }
                }
            }
        }
    }

    // Check for pending allocations
    let pending_nodes = send_with_retries(config, || {
        default_api::list_scheduled_compute_nodes(
            config,
            workflow_id,
            None,            // offset
            Some(1),         // limit - just need one
            None,            // sort_by
            None,            // reverse_sort
            None,            // scheduler_id
            None,            // scheduler_config_id
            Some("pending"), // status
        )
    });

    if let Ok(response) = pending_nodes
        && let Some(nodes) = response.items
    {
        for node in nodes {
            if node.scheduler_type.to_lowercase() == "slurm" {
                // Check if this Slurm job is still queued
                if let Ok(slurm) = SlurmInterface::new() {
                    let slurm_job_id = node.scheduler_id.to_string();
                    if let Ok(info) = slurm.get_status(&slurm_job_id)
                        && (info.status == HpcJobStatus::Running
                            || info.status == HpcJobStatus::Queued)
                    {
                        debug!(
                            "Found valid pending Slurm allocation {} (status: {:?})",
                            slurm_job_id, info.status
                        );
                        return true;
                    }
                }
            }
        }
    }

    // No valid Slurm allocations found
    debug!("No valid Slurm allocations found");
    false
}

// Note: fail_orphaned_running_jobs is now in orphan_detection module

/// Options for auto-scheduling behavior
struct AutoScheduleOptions {
    enabled: bool,
    threshold: u32,
    cooldown: Duration,
    stranded_timeout: Duration,
    output_dir: PathBuf,
}

/// Poll until workflow is complete, optionally printing status updates.
/// After the workflow is complete, continues to wait until all workers have exited
/// (no active compute nodes and no scheduled compute nodes). This is critical for
/// recovery scenarios to ensure workers complete their cleanup routines before
/// any recovery actions are taken.
fn poll_until_complete(
    config: &Configuration,
    workflow_id: i64,
    poll_interval: u64,
    show_job_counts: bool,
    auto_schedule: &AutoScheduleOptions,
) -> Result<HashMap<String, i64>, String> {
    use std::time::Instant;

    let mut workflow_complete = false;
    // Track when we last auto-scheduled (or started watching) for stranded job detection
    let mut last_auto_schedule: Instant = Instant::now();

    loop {
        // Check if workflow is complete
        if !workflow_complete {
            match send_with_retries(config, || {
                default_api::is_workflow_complete(config, workflow_id)
            }) {
                Ok(response) => {
                    if response.is_complete {
                        info!("Workflow complete workflow_id={}", workflow_id);
                        workflow_complete = true;
                        // Don't break yet - wait for workers to exit
                    }
                }
                Err(e) => {
                    return Err(format!("Error checking workflow status: {}", e));
                }
            }
        }

        // If workflow is complete, wait for all workers to exit before returning
        if workflow_complete {
            let workers_active = has_active_workers(config, workflow_id);
            if !workers_active {
                info!("Workers exited workflow_id={}", workflow_id);
                break;
            }
            debug!("Waiting for workers to exit...");
            std::thread::sleep(Duration::from_secs(poll_interval));
            continue;
        }

        // Print current status if requested
        if show_job_counts {
            match get_job_counts(config, workflow_id) {
                Ok(counts) => {
                    let completed = counts.get("Completed").unwrap_or(&0);
                    let running = counts.get("Running").unwrap_or(&0);
                    let ready = counts.get("Ready").unwrap_or(&0);
                    let failed = counts.get("Failed").unwrap_or(&0);
                    let blocked = counts.get("Blocked").unwrap_or(&0);
                    info!(
                        "Job counts workflow_id={} ready={} blocked={} running={} completed={} failed={}",
                        workflow_id, ready, blocked, running, completed, failed
                    );
                }
                Err(e) => {
                    error!("Error getting job counts: {}", e);
                }
            }
        }

        // Optimization: If there's at least one valid Slurm allocation (pending or running),
        // skip the expensive per-allocation orphan detection. This reduces N squeue calls
        // to just 1-2 calls when jobs are queued or running normally.
        if has_valid_slurm_allocation(config, workflow_id) {
            // Check if we should auto-schedule for retry jobs even though schedulers exist
            if auto_schedule.enabled {
                let cooldown_passed = last_auto_schedule.elapsed() >= auto_schedule.cooldown;

                if cooldown_passed {
                    match count_ready_retry_jobs(config, workflow_id) {
                        Ok((total_ready, retry_ready)) => {
                            // Check if we should schedule: either threshold met or stranded timeout
                            let threshold_met = retry_ready >= auto_schedule.threshold as i64;
                            let stranded = retry_ready > 0
                                && auto_schedule.stranded_timeout.as_secs() > 0
                                && last_auto_schedule.elapsed() >= auto_schedule.stranded_timeout;

                            if threshold_met {
                                info!(
                                    "Auto-schedule: {} retry jobs waiting (threshold: {}), scheduling more nodes...",
                                    retry_ready, auto_schedule.threshold
                                );
                                match regenerate_and_submit(workflow_id, &auto_schedule.output_dir)
                                {
                                    Ok(()) => {
                                        info!(
                                            "Auto-schedule: Successfully submitted new allocations"
                                        );
                                        last_auto_schedule = Instant::now();
                                    }
                                    Err(e) => {
                                        warn!("Auto-schedule failed: {}", e);
                                    }
                                }
                            } else if stranded {
                                info!(
                                    "Auto-schedule: {} retry jobs stranded for {}s (timeout: {}s), scheduling...",
                                    retry_ready,
                                    last_auto_schedule.elapsed().as_secs(),
                                    auto_schedule.stranded_timeout.as_secs()
                                );
                                match regenerate_and_submit(workflow_id, &auto_schedule.output_dir)
                                {
                                    Ok(()) => {
                                        info!(
                                            "Auto-schedule: Successfully submitted new allocations"
                                        );
                                        last_auto_schedule = Instant::now();
                                    }
                                    Err(e) => {
                                        warn!("Auto-schedule failed: {}", e);
                                    }
                                }
                            } else if retry_ready > 0 {
                                debug!(
                                    "Auto-schedule: {} retry jobs waiting, below threshold of {} (stranded after {}s)",
                                    retry_ready,
                                    auto_schedule.threshold,
                                    auto_schedule.stranded_timeout.as_secs()
                                );
                            }
                            // Log total ready for visibility
                            if total_ready > 0 && total_ready != retry_ready {
                                debug!(
                                    "Auto-schedule: {} total ready jobs ({} are retries)",
                                    total_ready, retry_ready
                                );
                            }
                        }
                        Err(e) => {
                            warn!("Failed to count retry jobs: {}", e);
                        }
                    }
                }
            }

            std::thread::sleep(Duration::from_secs(poll_interval));
            continue;
        }

        // No valid Slurm allocations found - check for orphaned jobs
        debug!("No valid Slurm allocations, checking for orphaned jobs...");

        // Use shared orphan detection to check for:
        // 1. Orphaned Slurm jobs (active allocations that are no longer running)
        // 2. Dead pending Slurm jobs (cancelled/failed before starting)
        // 3. Orphaned running jobs (stuck in "running" with no active compute nodes)
        match cleanup_orphaned_jobs(config, workflow_id, false) {
            Ok(result) => {
                if result.any_cleaned() {
                    info!(
                        "Orphan cleanup: {} Slurm jobs failed, {} pending cleaned, {} running jobs failed",
                        result.slurm_jobs_failed,
                        result.pending_allocations_cleaned,
                        result.running_jobs_failed
                    );
                }
            }
            Err(e) => {
                warn!("Error during orphan cleanup: {}", e);
            }
        }

        // Check if there are any pending or active scheduled compute nodes
        // If not, nothing can make progress unless we auto-schedule
        if !has_any_scheduled_compute_nodes(config, workflow_id) {
            // Check if there are ready jobs that need scheduling
            match count_ready_retry_jobs(config, workflow_id) {
                Ok((total_ready, retry_ready)) => {
                    if total_ready > 0 {
                        if auto_schedule.enabled {
                            info!(
                                "Auto-schedule: No schedulers available but {} ready jobs found ({} retries)",
                                total_ready, retry_ready
                            );
                            info!("Auto-schedule: Regenerating schedulers...");
                            match regenerate_and_submit(workflow_id, &auto_schedule.output_dir) {
                                Ok(()) => {
                                    info!("Auto-schedule: Successfully submitted new allocations");
                                    last_auto_schedule = Instant::now();
                                    // Continue polling - new schedulers should pick up work
                                    std::thread::sleep(Duration::from_secs(poll_interval));
                                    continue;
                                }
                                Err(e) => {
                                    warn!("Auto-schedule failed: {}", e);
                                    warn!(
                                        "Workflow cannot make progress without active allocations"
                                    );
                                    break;
                                }
                            }
                        } else {
                            warn!("No pending or active scheduled compute nodes found");
                            warn!(
                                "{} ready jobs with no schedulers. Use --auto-schedule to regenerate.",
                                total_ready
                            );
                            break;
                        }
                    } else {
                        // No ready jobs and no schedulers - workflow is stuck
                        warn!("No pending or active scheduled compute nodes found");
                        warn!("Workflow cannot make progress without active allocations");
                        break;
                    }
                }
                Err(e) => {
                    warn!("Failed to count ready jobs: {}", e);
                    warn!("No pending or active scheduled compute nodes found");
                    break;
                }
            }
        }

        std::thread::sleep(Duration::from_secs(poll_interval));
    }

    get_job_counts(config, workflow_id)
}

/// Run the watch command
pub fn run_watch(config: &Configuration, args: &WatchArgs) {
    let hostname = hostname::get()
        .expect("Failed to get hostname")
        .into_string()
        .expect("Hostname is not valid UTF-8");

    // Create output directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(&args.output_dir) {
        eprintln!(
            "Error creating output directory {}: {}",
            args.output_dir.display(),
            e
        );
        std::process::exit(1);
    }

    let log_file_path = get_watch_log_file(args.output_dir.clone(), &hostname, args.workflow_id);
    let log_file = match File::create(&log_file_path) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("Error creating log file {}: {}", log_file_path, e);
            std::process::exit(1);
        }
    };

    let multi_writer = MultiWriter {
        stdout: std::io::stdout(),
        file: log_file,
    };

    // Parse log level string to LevelFilter
    let log_level_filter = match args.log_level.to_lowercase().as_str() {
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => {
            eprintln!(
                "Invalid log level '{}', defaulting to 'info'",
                args.log_level
            );
            LevelFilter::Info
        }
    };

    let mut builder = Builder::from_default_env();
    builder
        .target(env_logger::Target::Pipe(Box::new(multi_writer)))
        .filter_level(log_level_filter)
        .try_init()
        .ok(); // Ignore error if logger is already initialized

    info!(
        "Watch started workflow_id={} hostname={} output_dir={} log_file={}",
        args.workflow_id,
        hostname,
        args.output_dir.display(),
        log_file_path
    );

    let mut retry_count = 0u32;

    // Early check: verify this workflow has scheduled compute nodes
    // The watch command is designed for Slurm/scheduler-based workflows.
    // For workflows run with `torc run` or `torc remote run`, use those commands directly.
    if !has_any_scheduled_compute_nodes(config, args.workflow_id) {
        error!(
            "No scheduled compute nodes found for workflow {}.",
            args.workflow_id
        );
        error!("");
        error!("The 'watch' command is designed for scheduler-based workflows (e.g., Slurm).");
        error!("For local execution, use: torc run <workflow_id>");
        error!("For remote execution, use: torc remote run <workflow_id>");
        std::process::exit(1);
    }

    info!(
        "Watching workflow {} (poll interval: {}s{}{})",
        args.workflow_id,
        args.poll_interval,
        if args.recover {
            format!(", recover enabled, max retries: {}", args.max_retries)
        } else {
            String::new()
        },
        if args.show_job_counts {
            ", job counts enabled"
        } else {
            ""
        }
    );

    if !args.show_job_counts {
        info!("  (use --show-job-counts to display per-status counts during polling)");
    }

    // Set up auto-schedule options
    let auto_schedule_opts = AutoScheduleOptions {
        enabled: args.auto_schedule,
        threshold: args.auto_schedule_threshold,
        cooldown: Duration::from_secs(args.auto_schedule_cooldown),
        stranded_timeout: Duration::from_secs(args.auto_schedule_stranded_timeout),
        output_dir: args.output_dir.clone(),
    };

    if args.auto_schedule {
        info!(
            "Auto-schedule enabled (threshold: {} retry jobs, cooldown: {}s, stranded timeout: {}s)",
            args.auto_schedule_threshold,
            args.auto_schedule_cooldown,
            args.auto_schedule_stranded_timeout
        );
    }

    loop {
        let counts = match poll_until_complete(
            config,
            args.workflow_id,
            args.poll_interval,
            args.show_job_counts,
            &auto_schedule_opts,
        ) {
            Ok(c) => c,
            Err(e) => {
                error!("Error: {}", e);
                std::process::exit(1);
            }
        };

        let completed = *counts.get("Completed").unwrap_or(&0);
        let failed = *counts.get("Failed").unwrap_or(&0);
        let canceled = *counts.get("Canceled").unwrap_or(&0);
        let terminated = *counts.get("Terminated").unwrap_or(&0);
        let pending_failed = *counts.get("PendingFailed").unwrap_or(&0);

        let needs_recovery = failed > 0 || canceled > 0 || terminated > 0;
        let has_pending_failed = pending_failed > 0;

        if !needs_recovery && !has_pending_failed {
            info!("\n✓ Workflow completed successfully ({} jobs)", completed);
            break;
        }

        warn!("\nWorkflow completed with failures:");
        warn!("  - Failed: {}", failed);
        warn!("  - Canceled: {}", canceled);
        warn!("  - Terminated: {}", terminated);
        if has_pending_failed {
            warn!(
                "  - Pending Failed: {} (awaiting AI classification)",
                pending_failed
            );
        }
        warn!("  - Completed: {}", completed);

        // Handle pending_failed jobs if --ai-recovery is enabled
        if has_pending_failed {
            if args.ai_recovery {
                info!(
                    "\n[EXPERIMENTAL] AI recovery: {} job(s) in pending_failed status",
                    pending_failed
                );
                info!("These jobs failed without a matching failure handler rule.");

                // Invoke the AI agent to classify pending_failed jobs
                match super::recover::invoke_ai_agent(
                    args.workflow_id,
                    &args.ai_agent,
                    &args.output_dir,
                ) {
                    Ok(()) => {
                        info!("AI agent completed classification, continuing...");
                        // Continue the loop to re-poll and check status
                        continue;
                    }
                    Err(e) => {
                        warn!("AI agent invocation failed: {}", e);
                        warn!("You can manually classify jobs using the torc MCP server:");
                        warn!("  1. list_pending_failed_jobs - View jobs with their stderr");
                        warn!("  2. classify_and_resolve_failures - Apply retry/fail decisions");
                        warn!(
                            "Or reset them manually: torc workflows reset-status {} --failed-only",
                            args.workflow_id
                        );
                        // Exit if only pending_failed jobs (no other failures to auto-recover)
                        if !needs_recovery {
                            std::process::exit(1);
                        }
                    }
                }
            } else {
                warn!(
                    "\n{} job(s) in pending_failed status (awaiting classification)",
                    pending_failed
                );
                warn!("Use --ai-recovery to enable AI-assisted classification via MCP tools.");
                warn!(
                    "Or reset them manually: torc workflows reset-status {} --failed-only",
                    args.workflow_id
                );
                // Exit if only pending_failed jobs (no other failures to auto-recover)
                if !needs_recovery {
                    std::process::exit(1);
                }
            }
        }

        // Check if we should attempt recovery
        if !args.recover {
            info!("\nRecovery disabled. To enable, use --recover flag.");
            info!("Or use the Torc MCP server with your AI assistant for manual recovery.");
            std::process::exit(1);
        }

        if retry_count >= args.max_retries {
            warn!(
                "\nMax retries ({}) exceeded. Manual intervention required.",
                args.max_retries
            );
            warn!("Use the Torc MCP server with your AI assistant to investigate.");
            std::process::exit(1);
        }

        retry_count += 1;
        info!(
            "\nAttempting automatic recovery (attempt {}/{})",
            retry_count, args.max_retries
        );

        // Step 1: Diagnose failures
        info!("\nDiagnosing failures...");
        let diagnosis = match diagnose_failures(args.workflow_id, &args.output_dir) {
            Ok(d) => d,
            Err(e) => {
                warn!("Warning: Could not diagnose failures: {}", e);
                warn!("Attempting retry without resource adjustments...");
                ResourceUtilizationReport {
                    workflow_id: args.workflow_id,
                    run_id: None,
                    total_results: 0,
                    over_utilization_count: 0,
                    violations: Vec::new(),
                    resource_violations_count: 0,
                    resource_violations: Vec::new(),
                    failed_jobs_count: 0,
                    failed_jobs: Vec::new(),
                }
            }
        };

        // Step 2: Apply heuristics to adjust resources
        info!("\nApplying recovery heuristics...");
        // If a recovery hook is provided, treat unknown failures as retryable
        // (the user is explicitly saying they'll handle them with their script)
        let retry_unknown = args.retry_unknown || args.recovery_hook.is_some();
        let recovery_result = match apply_recovery_heuristics(
            config,
            args.workflow_id,
            &diagnosis,
            args.memory_multiplier,
            args.runtime_multiplier,
            retry_unknown,
            &args.output_dir,
            false, // dry_run - always execute for watch
        ) {
            Ok(result) => {
                if result.oom_fixed > 0 || result.timeout_fixed > 0 {
                    info!(
                        "  Applied fixes: {} OOM, {} timeout",
                        result.oom_fixed, result.timeout_fixed
                    );
                }
                if result.other_failures > 0 {
                    if retry_unknown {
                        if args.recovery_hook.is_some() {
                            info!(
                                "  {} job(s) with unknown failure cause (will run recovery hook)",
                                result.other_failures
                            );
                        } else {
                            info!(
                                "  {} job(s) with unknown failure cause (will retry)",
                                result.other_failures
                            );
                        }
                    } else {
                        info!(
                            "  {} job(s) with unknown failure cause (skipped, use --retry-unknown to include)",
                            result.other_failures
                        );
                    }
                }
                result
            }
            Err(e) => {
                warn!("Warning: Error applying heuristics: {}", e);
                RecoveryResult {
                    oom_fixed: 0,
                    timeout_fixed: 0,
                    unknown_retried: 0,
                    other_failures: 0,
                    jobs_to_retry: Vec::new(),
                    adjustments: Vec::new(),
                    slurm_dry_run: None,
                }
            }
        };

        // Step 2.5: Run recovery hook if there are unknown failures
        if recovery_result.other_failures > 0
            && let Some(ref hook_cmd) = args.recovery_hook
        {
            info!(
                "\n{} job(s) with unknown failure cause - running recovery hook...",
                recovery_result.other_failures
            );
            if let Err(e) = run_recovery_hook(args.workflow_id, hook_cmd) {
                error!("Recovery hook failed: {}", e);
                std::process::exit(1);
            }
        }

        // Check if there are any jobs to retry
        if recovery_result.jobs_to_retry.is_empty() {
            warn!(
                "\nNo recoverable jobs found. {} job(s) failed with unknown causes.",
                recovery_result.other_failures
            );
            warn!("Use --retry-unknown to retry jobs with unknown failure causes.");
            warn!("Or use the Torc MCP server with your AI assistant to investigate.");
            std::process::exit(1);
        }

        // Step 3: Reset failed jobs
        info!(
            "\nResetting {} job(s) for retry...",
            recovery_result.jobs_to_retry.len()
        );
        match reset_failed_jobs(config, args.workflow_id, &recovery_result.jobs_to_retry) {
            Ok(count) => {
                info!("  Reset {} job(s)", count);
            }
            Err(e) => {
                error!("Error resetting jobs: {}", e);
                std::process::exit(1);
            }
        }

        // Step 4: Reinitialize workflow first (before creating new allocations)
        // Must happen before regenerate_and_submit because reset_workflow_status
        // rejects requests when there are pending scheduled compute nodes.
        info!("Reinitializing workflow...");
        if let Err(e) = reinitialize_workflow(args.workflow_id) {
            warn!("Error reinitializing workflow: {}", e);
            std::process::exit(1);
        }

        // Step 5: Regenerate Slurm schedulers (this also marks old actions as executed)
        info!("Regenerating Slurm schedulers...");
        if let Err(e) = regenerate_and_submit(args.workflow_id, &args.output_dir) {
            warn!("Error regenerating schedulers: {}", e);
            std::process::exit(1);
        }

        info!("\nRecovery initiated. Resuming monitoring...\n");
    }
}

// Tests for parse_memory_bytes, format_memory_bytes_short, format_duration_iso8601
// are in the recover module
