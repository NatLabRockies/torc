//! Remote worker commands for distributed execution via SSH.

use clap::Subcommand;
use log::{debug, info, warn};
use std::fs;
use std::path::{Path, PathBuf};

use crate::client::apis::configuration::Configuration;
use crate::client::apis::default_api;
use crate::client::commands::{get_env_user_name, select_workflow_interactively};
use crate::client::remote::{
    RemoteOperationResult, RemoteWorkerState, WorkerEntry, check_all_connectivity,
    check_ssh_connectivity, parallel_execute, parse_worker_content, parse_worker_file,
    scp_download, ssh_execute, ssh_execute_capture, verify_all_versions,
};
use crate::client::workflow_manager::WorkflowManager;
use crate::config::TorcConfig;

/// Remote worker execution commands.
#[derive(Subcommand)]
#[command(after_long_help = "\
EXAMPLES:
    # Add remote workers
    torc remote add-workers 123 user@host1 user@host2

    # List workers
    torc remote list-workers 123

    # Run workers via SSH
    torc remote run 123

    # Check worker status
    torc remote status 123

    # Stop all workers
    torc remote stop 123
")]
pub enum RemoteCommands {
    /// Add one or more remote workers to a workflow
    ///
    /// Workers are stored in the database and used by subsequent commands.
    /// Format: [user@]hostname[:port]
    #[command(
        name = "add-workers",
        after_long_help = "\
EXAMPLES:
    torc remote add-workers 123 user@host1 user@host2
    torc remote add-workers 123 host1 host2 host3
"
    )]
    AddWorkers {
        /// Workflow ID
        #[arg()]
        workflow_id: i64,

        /// Worker addresses (format: [user@]hostname[:port])
        #[arg(required = true, num_args = 1..)]
        workers: Vec<String>,

        /// Skip SSH connectivity check (for testing only)
        #[arg(long, hide = true)]
        skip_ssh_check: bool,
    },

    /// Add remote workers to a workflow from a file
    ///
    /// Each line in the file should be a worker address.
    /// Lines starting with # are comments.
    #[command(name = "add-workers-from-file")]
    AddWorkersFromFile {
        /// Path to worker file listing remote machines
        #[arg()]
        worker_file: PathBuf,

        /// Workflow ID (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,

        /// Skip SSH connectivity check (for testing only)
        #[arg(long, hide = true)]
        skip_ssh_check: bool,
    },

    /// Remove a remote worker from a workflow
    #[command(name = "remove-worker")]
    RemoveWorker {
        /// Worker address to remove
        #[arg()]
        worker: String,

        /// Workflow ID (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
    },

    /// List remote workers stored in the database for a workflow
    #[command(name = "list-workers")]
    ListWorkers {
        /// Workflow ID (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
    },

    /// Run workers on remote machines via SSH
    ///
    /// SSH into each stored worker and start a torc worker process.
    /// Workers run detached (via nohup) and survive SSH disconnection.
    /// Use add-workers first, or provide --workers to add and run in one step.
    Run {
        /// Workflow ID to run (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,

        /// Path to worker file (optional - adds workers before running)
        #[arg(short, long)]
        workers: Option<PathBuf>,

        /// Output directory on remote machines (relative to home)
        #[arg(short, long, default_value = "torc_output")]
        output_dir: String,

        /// Maximum parallel SSH connections
        #[arg(long, default_value = "10")]
        max_parallel_ssh: usize,

        /// Poll interval in seconds for workers
        #[arg(short, long, default_value = "5.0")]
        poll_interval: f64,

        /// Maximum number of parallel jobs per worker
        #[arg(long)]
        max_parallel_jobs: Option<i64>,

        /// Number of CPUs per worker (auto-detect if not specified)
        #[arg(long)]
        num_cpus: Option<i64>,

        /// Memory in GB per worker (auto-detect if not specified)
        #[arg(long)]
        memory_gb: Option<f64>,

        /// Number of GPUs per worker (auto-detect if not specified)
        #[arg(long)]
        num_gpus: Option<i64>,

        /// Skip version check (not recommended)
        #[arg(long, default_value = "false")]
        skip_version_check: bool,
    },

    /// Check status of remote workers
    Status {
        /// Workflow ID (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,

        /// Remote output directory (must match what was used in run)
        #[arg(long, default_value = "torc_output")]
        output_dir: String,

        /// Maximum parallel SSH connections
        #[arg(long, default_value = "10")]
        max_parallel_ssh: usize,
    },

    /// Stop workers on remote machines
    Stop {
        /// Workflow ID (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,

        /// Remote output directory (must match what was used in run)
        #[arg(long, default_value = "torc_output")]
        output_dir: String,

        /// Maximum parallel SSH connections
        #[arg(long, default_value = "10")]
        max_parallel_ssh: usize,

        /// Force kill (SIGKILL instead of SIGTERM)
        #[arg(long, default_value = "false")]
        force: bool,
    },

    /// Collect logs from remote workers
    #[command(name = "collect-logs")]
    CollectLogs {
        /// Workflow ID (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,

        /// Local directory to save collected logs
        #[arg(short, long, default_value = "remote_logs")]
        local_output_dir: PathBuf,

        /// Remote output directory (must match what was used in run)
        #[arg(long, default_value = "torc_output")]
        remote_output_dir: String,

        /// Maximum parallel SSH connections
        #[arg(long, default_value = "10")]
        max_parallel_ssh: usize,

        /// Delete remote logs after successful collection
        #[arg(long, default_value = "false")]
        delete: bool,
    },

    /// Delete logs from remote workers
    ///
    /// Removes the output directory from all remote workers.
    /// Use collect-logs --delete to safely collect before deleting.
    #[command(name = "delete-logs")]
    DeleteLogs {
        /// Workflow ID (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,

        /// Remote output directory to delete (must match what was used in run)
        #[arg(long, default_value = "torc_output")]
        remote_output_dir: String,

        /// Maximum parallel SSH connections
        #[arg(long, default_value = "10")]
        max_parallel_ssh: usize,
    },
}

/// Resolve optional workflow_id by prompting user if not provided.
fn resolve_workflow_id(config: &Configuration, workflow_id: Option<i64>) -> i64 {
    workflow_id.unwrap_or_else(|| {
        let user_name = get_env_user_name();
        select_workflow_interactively(config, &user_name).unwrap_or_else(|e| {
            eprintln!("Error selecting workflow: {}", e);
            std::process::exit(1);
        })
    })
}

/// Handle remote commands.
pub fn handle_remote_commands(config: &Configuration, command: &RemoteCommands) {
    match command {
        RemoteCommands::AddWorkers {
            workflow_id,
            workers,
            skip_ssh_check,
        } => {
            // AddWorkers requires workflow_id (not optional)
            handle_add_workers(config, *workflow_id, workers, *skip_ssh_check);
        }
        RemoteCommands::AddWorkersFromFile {
            workflow_id,
            worker_file,
            skip_ssh_check,
        } => {
            let wf_id = resolve_workflow_id(config, *workflow_id);
            handle_add_workers_from_file(config, wf_id, worker_file, *skip_ssh_check);
        }
        RemoteCommands::RemoveWorker {
            workflow_id,
            worker,
        } => {
            let wf_id = resolve_workflow_id(config, *workflow_id);
            handle_remove_worker(config, worker, wf_id);
        }
        RemoteCommands::ListWorkers { workflow_id } => {
            let wf_id = resolve_workflow_id(config, *workflow_id);
            handle_list_workers(config, wf_id);
        }
        RemoteCommands::Run {
            workflow_id,
            workers,
            output_dir,
            max_parallel_ssh,
            poll_interval,
            max_parallel_jobs,
            num_cpus,
            memory_gb,
            num_gpus,
            skip_version_check,
        } => {
            let wf_id = resolve_workflow_id(config, *workflow_id);
            handle_run(
                config,
                wf_id,
                workers.as_ref(),
                output_dir,
                *max_parallel_ssh,
                *poll_interval,
                *max_parallel_jobs,
                *num_cpus,
                *memory_gb,
                *num_gpus,
                *skip_version_check,
            );
        }
        RemoteCommands::Status {
            workflow_id,
            output_dir,
            max_parallel_ssh,
        } => {
            let wf_id = resolve_workflow_id(config, *workflow_id);
            handle_status(config, wf_id, output_dir, *max_parallel_ssh);
        }
        RemoteCommands::Stop {
            workflow_id,
            output_dir,
            max_parallel_ssh,
            force,
        } => {
            let wf_id = resolve_workflow_id(config, *workflow_id);
            handle_stop(config, wf_id, output_dir, *max_parallel_ssh, *force);
        }
        RemoteCommands::CollectLogs {
            workflow_id,
            local_output_dir,
            remote_output_dir,
            max_parallel_ssh,
            delete,
        } => {
            let wf_id = resolve_workflow_id(config, *workflow_id);
            handle_collect_logs(
                config,
                wf_id,
                local_output_dir,
                remote_output_dir,
                *max_parallel_ssh,
                *delete,
            );
        }
        RemoteCommands::DeleteLogs {
            workflow_id,
            remote_output_dir,
            max_parallel_ssh,
        } => {
            let wf_id = resolve_workflow_id(config, *workflow_id);
            handle_delete_logs(config, wf_id, remote_output_dir, *max_parallel_ssh);
        }
    }
}

/// Initialize workflow if all jobs are uninitialized.
///
/// This must be done on the manager before starting remote workers to avoid
/// race conditions where multiple workers try to initialize simultaneously.
fn initialize_workflow_if_needed(config: &Configuration, workflow_id: i64) {
    // Get workflow info
    let workflow = match default_api::get_workflow(config, workflow_id) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Error getting workflow {}: {}", workflow_id, e);
            std::process::exit(1);
        }
    };

    // Check if workflow needs initialization
    match default_api::is_workflow_uninitialized(config, workflow_id) {
        Ok(response) => {
            if let Some(is_uninitialized) =
                response.get("is_uninitialized").and_then(|v| v.as_bool())
                && is_uninitialized
            {
                info!(
                    "Workflow {} has all jobs uninitialized. Initializing on manager...",
                    workflow_id
                );
                let torc_config = TorcConfig::load().unwrap_or_default();
                let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow);
                match workflow_manager.initialize(false) {
                    Ok(()) => {
                        info!("Successfully initialized workflow {}", workflow_id);
                    }
                    Err(e) => {
                        eprintln!("Error initializing workflow: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Error checking if workflow is uninitialized: {}", e);
            std::process::exit(1);
        }
    }
}

/// Start workers on remote machines.
#[allow(clippy::too_many_arguments)]
fn handle_run(
    config: &Configuration,
    workflow_id: i64,
    workers_file: Option<&PathBuf>,
    output_dir: &str,
    max_parallel_ssh: usize,
    poll_interval: f64,
    max_parallel_jobs: Option<i64>,
    num_cpus: Option<i64>,
    memory_gb: Option<f64>,
    num_gpus: Option<i64>,
    skip_version_check: bool,
) {
    // If a workers file is provided, validate SSH connectivity before adding to database
    if let Some(worker_file) = workers_file {
        let workers = match parse_worker_file(worker_file) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Error parsing worker file: {}", e);
                std::process::exit(1);
            }
        };

        if workers.is_empty() {
            eprintln!("No workers found in {}", worker_file.display());
            std::process::exit(1);
        }

        // Check SSH connectivity for each worker before adding to database
        let source = worker_file.display().to_string();
        let valid_workers = match validate_workers_ssh(&workers, max_parallel_ssh, Some(&source)) {
            Ok(workers) => workers,
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        };

        // Add only valid workers to the database
        println!("Adding {} worker(s) to database...", valid_workers.len());

        match default_api::create_remote_workers(config, workflow_id, valid_workers) {
            Ok(created) => {
                info!(
                    "Added {} workers from {}",
                    created.len(),
                    worker_file.display()
                );
            }
            Err(e) => {
                eprintln!("Error adding workers from file: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Fetch all workers from the database
    let workers = fetch_workers_from_db(config, workflow_id);
    if workers.is_empty() {
        eprintln!(
            "No workers configured for workflow {}. Use 'torc remote add-workers' or '--workers' flag.",
            workflow_id
        );
        std::process::exit(1);
    }

    println!(
        "Found {} worker(s) for workflow {}",
        workers.len(),
        workflow_id
    );

    // Check SSH connectivity
    if let Err(e) = check_all_connectivity(&workers, max_parallel_ssh) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    // Version check
    if !skip_version_check {
        let local_version = env!("CARGO_PKG_VERSION");
        if let Err(e) = verify_all_versions(&workers, local_version, max_parallel_ssh) {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    } else {
        warn!("Skipping version check as requested");
    }

    // Initialize workflow on manager if needed (before starting workers to avoid race condition)
    initialize_workflow_if_needed(config, workflow_id);

    // Start workers
    println!("Starting workers...");

    let api_url = config.base_path.clone();
    let output_dir_owned = output_dir.to_string();

    let results: Vec<RemoteOperationResult> = parallel_execute(
        &workers,
        move |worker| {
            start_remote_worker(
                worker,
                &api_url,
                workflow_id,
                &output_dir_owned,
                poll_interval,
                max_parallel_jobs,
                num_cpus,
                memory_gb,
                num_gpus,
            )
        },
        max_parallel_ssh,
    );

    // Report results
    let mut success_count = 0;
    for result in &results {
        let status = if result.success { "OK" } else { "FAILED" };
        println!(
            "  [{}] {}: {}",
            status,
            result.worker.display_name(),
            result.message
        );
        if result.success {
            success_count += 1;
        }
    }

    println!("\nStarted {}/{} workers", success_count, workers.len());

    if success_count < workers.len() {
        std::process::exit(1);
    }
}

/// Start a single worker on a remote machine.
#[allow(clippy::too_many_arguments)]
fn start_remote_worker(
    worker: &WorkerEntry,
    api_url: &str,
    workflow_id: i64,
    output_dir: &str,
    poll_interval: f64,
    max_parallel_jobs: Option<i64>,
    num_cpus: Option<i64>,
    memory_gb: Option<f64>,
    num_gpus: Option<i64>,
) -> RemoteOperationResult {
    // Create output directory on remote
    let mkdir_cmd = format!("mkdir -p {}", output_dir);
    if let Err(e) = ssh_execute_capture(worker, &mkdir_cmd) {
        return RemoteOperationResult::failure(
            worker.clone(),
            format!("Failed to create output directory: {}", e),
        );
    }

    // Build the torc run command
    // --url is a global option that must come before the subcommand
    let mut torc_cmd = format!(
        "torc --url {} run {} --output-dir {} --poll-interval {}",
        api_url, workflow_id, output_dir, poll_interval
    );

    if let Some(cpus) = num_cpus {
        torc_cmd.push_str(&format!(" --num-cpus {}", cpus));
    }
    if let Some(mem) = memory_gb {
        torc_cmd.push_str(&format!(" --memory-gb {}", mem));
    }
    if let Some(gpus) = num_gpus {
        torc_cmd.push_str(&format!(" --num-gpus {}", gpus));
    }
    if let Some(max) = max_parallel_jobs {
        torc_cmd.push_str(&format!(" --max-parallel-jobs {}", max));
    }

    // PID file and log file paths
    let pid_file = format!("{}/torc_worker_{}.pid", output_dir, workflow_id);
    let log_file = format!("{}/torc_worker_{}.log", output_dir, workflow_id);

    // Start with nohup, redirect output, save PID
    // Use bash -c to ensure proper shell handling
    let start_cmd = format!(
        "bash -c 'nohup {} > {} 2>&1 & echo $! > {}; disown'",
        torc_cmd, log_file, pid_file
    );

    debug!(
        "Starting worker on {}: {}",
        worker.display_name(),
        start_cmd
    );

    if let Err(e) = ssh_execute(worker, &start_cmd, Some(60)) {
        return RemoteOperationResult::failure(
            worker.clone(),
            format!("Failed to start worker: {}", e),
        );
    }

    // Give it a moment to start
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Read PID file
    let pid_cmd = format!("cat {}", pid_file);
    let pid_output = match ssh_execute_capture(worker, &pid_cmd) {
        Ok(output) => output,
        Err(e) => {
            return RemoteOperationResult::failure(
                worker.clone(),
                format!("Failed to read PID file: {}", e),
            );
        }
    };

    let pid: u32 = match pid_output.trim().parse() {
        Ok(p) => p,
        Err(_) => {
            return RemoteOperationResult::failure(
                worker.clone(),
                format!("Invalid PID in file: '{}'", pid_output.trim()),
            );
        }
    };

    // Verify process is running with retries
    // We check both kill -0 and the log file for evidence of startup
    const MAX_RETRIES: u32 = 5;
    const RETRY_DELAY_MS: u64 = 1000;

    for attempt in 0..MAX_RETRIES {
        // First try kill -0
        let check_cmd = format!(
            "kill -0 {} 2>/dev/null && echo running || echo stopped",
            pid
        );
        let check_output = ssh_execute_capture(worker, &check_cmd).unwrap_or_default();

        if check_output.trim() == "running" {
            return RemoteOperationResult::success(
                worker.clone(),
                format!("Started (PID {})", pid),
            );
        }

        // Also check if log file shows successful startup (job_runner logs this on start)
        let log_check_cmd = format!(
            "grep -q 'Starting torc job runner' {} 2>/dev/null && echo started || echo waiting",
            log_file
        );
        let log_check_output = ssh_execute_capture(worker, &log_check_cmd).unwrap_or_default();

        if log_check_output.trim() == "started" {
            // Log shows startup, verify process is still running with pgrep
            let pgrep_cmd = format!(
                "pgrep -f 'torc.*run.*{}' >/dev/null 2>&1 && echo running || echo stopped",
                workflow_id
            );
            let pgrep_output = ssh_execute_capture(worker, &pgrep_cmd).unwrap_or_default();

            if pgrep_output.trim() == "running" {
                return RemoteOperationResult::success(
                    worker.clone(),
                    format!("Started (PID {})", pid),
                );
            }
        }

        // If not the last attempt, wait and retry
        if attempt < MAX_RETRIES - 1 {
            std::thread::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS));
        }
    }

    // All retries exhausted - process appears to have died
    let tail_cmd = format!(
        "tail -5 {} 2>/dev/null || echo 'No log available'",
        log_file
    );
    let log_output = ssh_execute_capture(worker, &tail_cmd).unwrap_or_default();
    RemoteOperationResult::failure(
        worker.clone(),
        format!("Process died immediately. Last log:\n{}", log_output.trim()),
    )
}

/// Check status of workers on remote machines.
fn handle_status(
    config: &Configuration,
    workflow_id: i64,
    output_dir: &str,
    max_parallel_ssh: usize,
) {
    let workers = fetch_workers_from_db(config, workflow_id);
    if workers.is_empty() {
        eprintln!(
            "No workers configured for workflow {}. Use 'torc remote add-workers' first.",
            workflow_id
        );
        std::process::exit(1);
    }

    let output_dir_owned = output_dir.to_string();

    let statuses: Vec<(WorkerEntry, RemoteWorkerState)> = parallel_execute(
        &workers,
        move |worker| {
            let state = check_remote_worker_status(worker, workflow_id, &output_dir_owned);
            (worker.clone(), state)
        },
        max_parallel_ssh,
    );

    // Print table
    println!("{:<30} {:<20}", "Host", "Status");
    println!("{}", "-".repeat(50));

    let mut running = 0;
    for (worker, status) in &statuses {
        println!("{:<30} {:<20}", worker.display_name(), status);
        if matches!(status, RemoteWorkerState::Running { .. }) {
            running += 1;
        }
    }

    println!("\n{}/{} workers running", running, workers.len());
}

/// Check status of a single remote worker.
fn check_remote_worker_status(
    worker: &WorkerEntry,
    workflow_id: i64,
    output_dir: &str,
) -> RemoteWorkerState {
    let pid_file = format!("{}/torc_worker_{}.pid", output_dir, workflow_id);

    // Read PID file
    let pid_cmd = format!("cat {} 2>/dev/null", pid_file);
    let pid_output = match ssh_execute_capture(worker, &pid_cmd) {
        Ok(output) => output,
        Err(_) => {
            // No PID file, but check if process is running anyway via pgrep
            return check_worker_via_pgrep(worker, workflow_id);
        }
    };

    let pid: u32 = match pid_output.trim().parse() {
        Ok(p) => p,
        Err(_) => {
            // Invalid PID file, fall back to pgrep
            return check_worker_via_pgrep(worker, workflow_id);
        }
    };

    // Check if process is running via kill -0
    let check_cmd = format!(
        "kill -0 {} 2>/dev/null && echo running || echo stopped",
        pid
    );
    match ssh_execute_capture(worker, &check_cmd) {
        Ok(output) => {
            if output.trim() == "running" {
                RemoteWorkerState::Running { pid }
            } else {
                // PID file exists with valid PID but process has exited - worker completed.
                // We intentionally do NOT fall back to pgrep here because:
                // 1. The PID file is authoritative - if the tracked process exited, the worker is done
                // 2. pgrep can match unrelated processes, especially when workflow IDs are substrings
                //    of other workflow IDs (e.g., workflow 123 matching a process for workflow 1234)
                RemoteWorkerState::NotRunning
            }
        }
        Err(_) => check_worker_via_pgrep(worker, workflow_id),
    }
}

/// Check if a torc worker is running via pgrep (fallback when PID check fails).
fn check_worker_via_pgrep(worker: &WorkerEntry, workflow_id: i64) -> RemoteWorkerState {
    let pgrep_cmd = format!(
        "pgrep -f 'torc.*run.*{}' 2>/dev/null | head -1",
        workflow_id
    );
    match ssh_execute_capture(worker, &pgrep_cmd) {
        Ok(output) => {
            let trimmed = output.trim();
            if let Ok(pid) = trimmed.parse::<u32>() {
                RemoteWorkerState::Running { pid }
            } else {
                RemoteWorkerState::NotRunning
            }
        }
        Err(_) => RemoteWorkerState::NotRunning,
    }
}

/// Stop workers on remote machines.
fn handle_stop(
    config: &Configuration,
    workflow_id: i64,
    output_dir: &str,
    max_parallel_ssh: usize,
    force: bool,
) {
    let workers = fetch_workers_from_db(config, workflow_id);
    if workers.is_empty() {
        eprintln!(
            "No workers configured for workflow {}. Use 'torc remote add-workers' first.",
            workflow_id
        );
        std::process::exit(1);
    }

    let output_dir_owned = output_dir.to_string();
    let signal = if force { "KILL" } else { "TERM" };

    println!(
        "Stopping workers (signal: {})...",
        if force { "SIGKILL" } else { "SIGTERM" }
    );

    let results: Vec<RemoteOperationResult> = parallel_execute(
        &workers,
        move |worker| stop_remote_worker(worker, workflow_id, &output_dir_owned, signal),
        max_parallel_ssh,
    );

    // Report results
    for result in &results {
        let status = if result.success { "OK" } else { "FAILED" };
        println!(
            "  [{}] {}: {}",
            status,
            result.worker.display_name(),
            result.message
        );
    }
}

/// Stop a single remote worker.
fn stop_remote_worker(
    worker: &WorkerEntry,
    workflow_id: i64,
    output_dir: &str,
    signal: &str,
) -> RemoteOperationResult {
    let pid_file = format!("{}/torc_worker_{}.pid", output_dir, workflow_id);

    // Read PID
    let pid_cmd = format!("cat {} 2>/dev/null", pid_file);
    let pid_output = match ssh_execute_capture(worker, &pid_cmd) {
        Ok(output) => output,
        Err(_) => {
            return RemoteOperationResult::success(worker.clone(), "No worker found (no PID file)");
        }
    };

    let pid: u32 = match pid_output.trim().parse() {
        Ok(p) => p,
        Err(_) => {
            return RemoteOperationResult::failure(worker.clone(), "Invalid PID file");
        }
    };

    // Send signal
    let kill_cmd = format!(
        "kill -{} {} 2>/dev/null && echo killed || echo not_found",
        signal, pid
    );
    match ssh_execute_capture(worker, &kill_cmd) {
        Ok(output) => {
            if output.trim() == "killed" {
                RemoteOperationResult::success(
                    worker.clone(),
                    format!("Sent SIG{} to PID {}", signal, pid),
                )
            } else {
                RemoteOperationResult::success(worker.clone(), "Process not running")
            }
        }
        Err(e) => RemoteOperationResult::failure(worker.clone(), format!("Failed: {}", e)),
    }
}

/// Collect logs from remote workers.
fn handle_collect_logs(
    config: &Configuration,
    workflow_id: i64,
    local_output_dir: &Path,
    remote_output_dir: &str,
    max_parallel_ssh: usize,
    delete_after: bool,
) {
    let workers = fetch_workers_from_db(config, workflow_id);
    if workers.is_empty() {
        eprintln!(
            "No workers configured for workflow {}. Use 'torc remote add-workers' first.",
            workflow_id
        );
        std::process::exit(1);
    }

    // Create local output directory
    if let Err(e) = fs::create_dir_all(local_output_dir) {
        eprintln!("Error creating output directory: {}", e);
        std::process::exit(1);
    }

    let action = if delete_after {
        "Collecting and deleting"
    } else {
        "Collecting"
    };
    println!(
        "{} logs from {} worker(s) to {}...",
        action,
        workers.len(),
        local_output_dir.display()
    );

    let local_dir = local_output_dir.to_path_buf();
    let remote_dir = remote_output_dir.to_string();

    let results: Vec<RemoteOperationResult> = parallel_execute(
        &workers,
        move |worker| {
            collect_worker_logs(worker, workflow_id, &local_dir, &remote_dir, delete_after)
        },
        max_parallel_ssh,
    );

    // Report results
    let mut success_count = 0;
    for result in &results {
        let status = if result.success { "OK" } else { "FAILED" };
        println!(
            "  [{}] {}: {}",
            status,
            result.worker.display_name(),
            result.message
        );
        if result.success {
            success_count += 1;
        }
    }

    let verb = if delete_after {
        "Collected and deleted"
    } else {
        "Collected"
    };
    println!(
        "\n{} logs from {}/{} workers",
        verb,
        success_count,
        workers.len()
    );
}

/// Collect logs from a single remote worker.
fn collect_worker_logs(
    worker: &WorkerEntry,
    workflow_id: i64,
    local_output_dir: &Path,
    remote_output_dir: &str,
    delete_after: bool,
) -> RemoteOperationResult {
    // Create tarball on remote
    let tarball_name = format!(
        "torc_logs_{}_{}.tar.gz",
        workflow_id,
        worker.host.replace('.', "_")
    );
    let remote_tarball = format!("/tmp/{}", tarball_name);

    // Check if remote directory exists
    let check_cmd = format!(
        "test -d {} && echo exists || echo missing",
        remote_output_dir
    );
    match ssh_execute_capture(worker, &check_cmd) {
        Ok(output) => {
            if output.trim() == "missing" {
                return RemoteOperationResult::success(worker.clone(), "No output directory found");
            }
        }
        Err(e) => {
            return RemoteOperationResult::failure(
                worker.clone(),
                format!("Failed to check directory: {}", e),
            );
        }
    }

    // Create tarball
    let tar_cmd = format!(
        "tar -czf {} -C {} . 2>/dev/null",
        remote_tarball, remote_output_dir
    );
    if let Err(e) = ssh_execute(worker, &tar_cmd, Some(300)) {
        return RemoteOperationResult::failure(
            worker.clone(),
            format!("Failed to create archive: {}", e),
        );
    }

    // SCP the tarball back
    let local_file = local_output_dir.join(&tarball_name);
    let local_file_str = local_file.to_string_lossy().to_string();

    match scp_download(worker, &remote_tarball, &local_file_str, Some(600)) {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return RemoteOperationResult::failure(
                    worker.clone(),
                    format!("SCP failed: {}", stderr.trim()),
                );
            }
        }
        Err(e) => {
            return RemoteOperationResult::failure(worker.clone(), format!("SCP error: {}", e));
        }
    }

    // Clean up remote tarball
    let rm_cmd = format!("rm -f {}", remote_tarball);
    let _ = ssh_execute(worker, &rm_cmd, None);

    // Delete remote output directory if requested
    if delete_after {
        let delete_cmd = format!("rm -rf {}", remote_output_dir);
        if let Err(e) = ssh_execute(worker, &delete_cmd, Some(60)) {
            return RemoteOperationResult::failure(
                worker.clone(),
                format!("Collected but failed to delete: {}", e),
            );
        }
        RemoteOperationResult::success(
            worker.clone(),
            format!("Saved to {} and deleted remote", local_file.display()),
        )
    } else {
        RemoteOperationResult::success(worker.clone(), format!("Saved to {}", local_file.display()))
    }
}

/// Delete logs from remote workers.
fn handle_delete_logs(
    config: &Configuration,
    workflow_id: i64,
    remote_output_dir: &str,
    max_parallel_ssh: usize,
) {
    let workers = fetch_workers_from_db(config, workflow_id);
    if workers.is_empty() {
        eprintln!(
            "No workers configured for workflow {}. Use 'torc remote add-workers' first.",
            workflow_id
        );
        std::process::exit(1);
    }

    println!("Deleting logs from {} worker(s)...", workers.len());

    let remote_dir = remote_output_dir.to_string();

    let results: Vec<RemoteOperationResult> = parallel_execute(
        &workers,
        move |worker| delete_worker_logs(worker, &remote_dir),
        max_parallel_ssh,
    );

    // Report results
    let mut success_count = 0;
    for result in &results {
        let status = if result.success { "OK" } else { "FAILED" };
        println!(
            "  [{}] {}: {}",
            status,
            result.worker.display_name(),
            result.message
        );
        if result.success {
            success_count += 1;
        }
    }

    println!(
        "\nDeleted logs from {}/{} workers",
        success_count,
        workers.len()
    );
}

/// Delete logs from a single remote worker.
fn delete_worker_logs(worker: &WorkerEntry, remote_output_dir: &str) -> RemoteOperationResult {
    // Check if remote directory exists
    let check_cmd = format!(
        "test -d {} && echo exists || echo missing",
        remote_output_dir
    );
    match ssh_execute_capture(worker, &check_cmd) {
        Ok(output) => {
            if output.trim() == "missing" {
                return RemoteOperationResult::success(
                    worker.clone(),
                    "No output directory found (already clean)",
                );
            }
        }
        Err(e) => {
            return RemoteOperationResult::failure(
                worker.clone(),
                format!("Failed to check directory: {}", e),
            );
        }
    }

    // Delete the directory
    let delete_cmd = format!("rm -rf {}", remote_output_dir);
    match ssh_execute(worker, &delete_cmd, Some(60)) {
        Ok(_) => {
            RemoteOperationResult::success(worker.clone(), format!("Deleted {}", remote_output_dir))
        }
        Err(e) => {
            RemoteOperationResult::failure(worker.clone(), format!("Failed to delete: {}", e))
        }
    }
}

/// List remote workers stored in the database for a workflow.
fn handle_list_workers(config: &Configuration, workflow_id: i64) {
    match default_api::list_remote_workers(config, workflow_id) {
        Ok(workers) => {
            if workers.is_empty() {
                println!("No remote workers stored for workflow {}", workflow_id);
            } else {
                println!(
                    "Remote workers for workflow {} ({} total):",
                    workflow_id,
                    workers.len()
                );
                for worker in &workers {
                    println!("  {}", worker.worker);
                }
            }
        }
        Err(e) => {
            eprintln!("Error listing remote workers: {}", e);
            std::process::exit(1);
        }
    }
}

/// Default max parallel SSH connections for add-workers commands.
const DEFAULT_MAX_PARALLEL_SSH: usize = 10;

/// Validate workers by checking SSH connectivity and return only valid workers.
///
/// Returns the list of valid worker addresses (as strings) that passed SSH checks.
/// Prints error messages for workers that failed connectivity checks.
/// Returns an error if no workers pass the check.
///
/// # Arguments
/// * `workers` - The workers to validate
/// * `max_parallel_ssh` - Maximum number of parallel SSH connections
/// * `source` - Optional source description (e.g., file path) for log messages
fn validate_workers_ssh(
    workers: &[WorkerEntry],
    max_parallel_ssh: usize,
    source: Option<&str>,
) -> Result<Vec<String>, String> {
    if let Some(src) = source {
        println!(
            "Checking SSH connectivity for {} worker(s) from {}...",
            workers.len(),
            src
        );
    } else {
        println!(
            "Checking SSH connectivity for {} worker(s)...",
            workers.len()
        );
    }

    let results: Vec<Result<(), String>> =
        parallel_execute(workers, check_ssh_connectivity, max_parallel_ssh);

    let mut valid_workers: Vec<String> = Vec::new();
    let mut failed_workers: Vec<(String, String)> = Vec::new();

    for (worker, result) in workers.iter().zip(results) {
        match result {
            Ok(()) => {
                valid_workers.push(worker.original.clone());
            }
            Err(e) => {
                failed_workers.push((worker.display_name().to_string(), e));
            }
        }
    }

    // Report failed workers
    if !failed_workers.is_empty() {
        eprintln!(
            "SSH connectivity check failed for {} worker(s):",
            failed_workers.len()
        );
        for (host, error) in &failed_workers {
            eprintln!("  {}: {}", host, error);
        }
    }

    if valid_workers.is_empty() {
        return Err("No workers passed SSH connectivity check".to_string());
    }

    println!(
        "{}/{} workers passed SSH check",
        valid_workers.len(),
        workers.len()
    );

    Ok(valid_workers)
}

/// Add remote workers to the database.
fn handle_add_workers(
    config: &Configuration,
    workflow_id: i64,
    workers: &[String],
    skip_ssh_check: bool,
) {
    if workers.is_empty() {
        eprintln!("No workers specified");
        std::process::exit(1);
    }

    // Parse worker strings into WorkerEntry for SSH checking
    let worker_content = workers.join("\n");
    let parsed_workers = match parse_worker_content(&worker_content, "command line") {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Error parsing worker addresses: {}", e);
            std::process::exit(1);
        }
    };

    let valid_workers = if skip_ssh_check {
        // Skip SSH check - add all workers directly
        parsed_workers.iter().map(|w| w.original.clone()).collect()
    } else {
        // Check SSH connectivity for each worker before adding to database
        match validate_workers_ssh(&parsed_workers, DEFAULT_MAX_PARALLEL_SSH, None) {
            Ok(workers) => workers,
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    };

    match default_api::create_remote_workers(config, workflow_id, valid_workers) {
        Ok(created) => {
            if created.is_empty() {
                println!("All workers already exist for workflow {}", workflow_id);
            } else {
                println!(
                    "Added {} worker(s) to workflow {}",
                    created.len(),
                    workflow_id
                );
                for worker in &created {
                    println!("  {}", worker.worker);
                }
            }
        }
        Err(e) => {
            eprintln!("Error adding workers: {}", e);
            std::process::exit(1);
        }
    }
}

/// Add remote workers from a file to the database.
fn handle_add_workers_from_file(
    config: &Configuration,
    workflow_id: i64,
    worker_file: &Path,
    skip_ssh_check: bool,
) {
    // Parse worker file
    let workers = match parse_worker_file(worker_file) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Error parsing worker file: {}", e);
            std::process::exit(1);
        }
    };

    if workers.is_empty() {
        eprintln!("No workers found in {}", worker_file.display());
        std::process::exit(1);
    }

    let valid_workers = if skip_ssh_check {
        // Skip SSH check - add all workers directly
        workers.iter().map(|w| w.original.clone()).collect()
    } else {
        // Check SSH connectivity for each worker before adding to database
        let source = worker_file.display().to_string();
        match validate_workers_ssh(&workers, DEFAULT_MAX_PARALLEL_SSH, Some(&source)) {
            Ok(workers) => workers,
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    };

    match default_api::create_remote_workers(config, workflow_id, valid_workers) {
        Ok(created) => {
            println!(
                "Added {} worker(s) from {} to workflow {}",
                created.len(),
                worker_file.display(),
                workflow_id
            );
            for worker in &created {
                println!("  {}", worker.worker);
            }
        }
        Err(e) => {
            eprintln!("Error adding workers: {}", e);
            std::process::exit(1);
        }
    }
}

/// Remove a remote worker from the database.
fn handle_remove_worker(config: &Configuration, worker: &str, workflow_id: i64) {
    match default_api::delete_remote_worker(config, workflow_id, worker) {
        Ok(_) => {
            println!("Removed worker {} from workflow {}", worker, workflow_id);
        }
        Err(e) => {
            eprintln!("Error removing worker: {}", e);
            std::process::exit(1);
        }
    }
}

/// Fetch workers from the database and convert to WorkerEntry.
fn fetch_workers_from_db(config: &Configuration, workflow_id: i64) -> Vec<WorkerEntry> {
    match default_api::list_remote_workers(config, workflow_id) {
        Ok(workers) => {
            workers
                .iter()
                .filter_map(|w| {
                    // Parse the worker string into a WorkerEntry
                    match parse_single_worker(&w.worker) {
                        Ok(entry) => Some(entry),
                        Err(e) => {
                            warn!("Failed to parse worker entry '{}': {}", w.worker, e);
                            None
                        }
                    }
                })
                .collect()
        }
        Err(e) => {
            eprintln!("Error fetching workers from database: {}", e);
            std::process::exit(1);
        }
    }
}

/// Parse a single worker string into a WorkerEntry.
fn parse_single_worker(line: &str) -> Result<WorkerEntry, String> {
    let line = line.trim();
    if line.is_empty() {
        return Err("Empty worker string".to_string());
    }

    let original = line.to_string();

    // Format: [user@]hostname[:port]
    // First, split off the user if present
    let (user, host_port) = if let Some(at_pos) = line.find('@') {
        let user = &line[..at_pos];
        let rest = &line[at_pos + 1..];

        if user.is_empty() {
            return Err("Empty username before '@'".to_string());
        }

        (Some(user.to_string()), rest)
    } else {
        (None, line)
    };

    // Now split off the port if present
    // Handle IPv6 addresses: [::1]:22 or [2001:db8::1]:22
    let (host, port) = if host_port.starts_with('[') {
        // IPv6 address in brackets
        if let Some(bracket_end) = host_port.find(']') {
            let ipv6 = &host_port[1..bracket_end];
            let rest = &host_port[bracket_end + 1..];
            if rest.is_empty() {
                (ipv6.to_string(), None)
            } else if let Some(port_str) = rest.strip_prefix(':') {
                let port: u16 = port_str
                    .parse()
                    .map_err(|_| format!("Invalid port '{}'", port_str))?;
                (ipv6.to_string(), Some(port))
            } else {
                return Err("Invalid format after IPv6 address".to_string());
            }
        } else {
            return Err("Unclosed bracket in IPv6 address".to_string());
        }
    } else {
        // Regular hostname or IPv4
        // Split on the last colon to handle port
        if let Some(colon_pos) = host_port.rfind(':') {
            let host = &host_port[..colon_pos];
            let port_str = &host_port[colon_pos + 1..];

            // Make sure port looks like a number (to avoid treating IPv6 as host:port)
            if port_str.chars().all(|c| c.is_ascii_digit()) && !port_str.is_empty() {
                let port: u16 = port_str
                    .parse()
                    .map_err(|_| format!("Invalid port '{}'", port_str))?;
                (host.to_string(), Some(port))
            } else {
                // Not a port, treat the whole thing as the host
                (host_port.to_string(), None)
            }
        } else {
            (host_port.to_string(), None)
        }
    };

    if host.is_empty() {
        return Err("Empty hostname".to_string());
    }

    Ok(WorkerEntry {
        original,
        user,
        host,
        port,
    })
}
