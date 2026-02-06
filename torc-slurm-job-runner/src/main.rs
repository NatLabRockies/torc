// This binary is only supported on Unix systems (Slurm is Linux-only)
#[cfg(not(unix))]
fn main() {
    eprintln!("torc-slurm-job-runner is only supported on Unix systems (Linux/macOS).");
    eprintln!("Slurm is not available on Windows.");
    std::process::exit(1);
}

#[cfg(unix)]
mod unix_main {
    use chrono::Duration;
    use chrono::Local;
    use clap::{Parser, builder::styling};
    use env_logger::Builder;
    use log::{LevelFilter, debug, error, info};
    use signal_hook::consts::SIGTERM;
    use signal_hook::iterator::Signals;
    use std::fs::File;
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;
    use std::thread;
    use torc::client::apis::configuration::Configuration;
    use torc::client::apis::default_api;
    use torc::client::commands::slurm::{create_compute_node, create_node_resources};
    use torc::client::hpc::hpc_interface::HpcInterface;
    use torc::client::hpc::slurm_interface::SlurmInterface;
    use torc::client::job_runner::JobRunner;
    use torc::client::log_paths::{
        get_slurm_dmesg_log_file, get_slurm_env_log_file, get_slurm_job_runner_log_file,
    };
    use torc::client::utils;

    const STYLES: styling::Styles = styling::Styles::styled()
        .header(styling::AnsiColor::Green.on_default().bold())
        .usage(styling::AnsiColor::Green.on_default().bold())
        .literal(styling::AnsiColor::Cyan.on_default().bold())
        .placeholder(styling::AnsiColor::Cyan.on_default());

    #[derive(Parser, Debug)]
    #[command(name = "torc-slurm-job-runner")]
    #[command(about = "Slurm job runner for Torc workflows", long_about = None)]
    #[command(styles = STYLES)]
    struct Args {
        /// Server URL
        #[arg()]
        url: String,

        /// Workflow ID
        #[arg()]
        workflow_id: i64,

        /// Output directory for compute nodes
        #[arg()]
        output_dir: PathBuf,

        /// Maximum number of parallel jobs to run concurrently.
        /// When NOT set: Uses resource-based job allocation (considers CPU, memory, GPU requirements).
        /// When set: Uses simple queue-based allocation with this parallel limit (ignores resource requirements).
        #[arg(long)]
        max_parallel_jobs: Option<i32>,

        /// Poll interval for job completions (seconds)
        #[arg(short, long, default_value = "60")]
        poll_interval: i64,

        /// Set to true if this is a subtask and multiple workers are running on one Slurm allocation
        #[arg(long, default_value = "false")]
        is_subtask: bool,

        /// Wait this number of minutes if the database is offline
        #[arg(long, default_value = "20")]
        wait_for_healthy_database_minutes: u64,
    }

    pub fn main() {
        let args = Args::parse();

        // Record start time for dmesg filtering (with 60-minute buffer)
        let dmesg_cutoff = Local::now() - chrono::Duration::minutes(60);

        // Create Slurm interface to get environment info
        let slurm_interface = match SlurmInterface::new() {
            Ok(interface) => interface,
            Err(e) => {
                eprintln!("Error creating Slurm interface: {}", e);
                std::process::exit(1);
            }
        };

        let job_id = slurm_interface.get_current_job_id();
        let node_id = slurm_interface.get_node_id();
        let task_pid = slurm_interface.get_task_pid();

        // Now we can configure the logger with the specific log file path
        let log_file_path = get_slurm_job_runner_log_file(
            args.output_dir.clone(),
            args.workflow_id,
            &job_id,
            &node_id,
            task_pid,
        );

        let log_file = match File::create(&log_file_path) {
            Ok(file) => file,
            Err(e) => {
                eprintln!("Error creating log file {}: {}", log_file_path, e);
                std::process::exit(1);
            }
        };

        // Initialize logger now that we have the log file
        let mut builder = Builder::from_default_env();
        builder
            .target(env_logger::Target::Pipe(Box::new(log_file)))
            .filter_level(LevelFilter::Info)
            .init();

        let hostname = hostname::get()
            .expect("Failed to get hostname")
            .into_string()
            .expect("Hostname is not valid UTF-8");

        info!("Starting Slurm job runner");
        info!("Job ID: {}", job_id);
        info!("Node ID: {}", node_id);
        info!("Task PID: {}", task_pid);
        info!("Hostname: {}", hostname);
        info!("Output directory: {}", args.output_dir.display());
        info!("Log file: {}", log_file_path);

        // Capture SLURM environment variables for debugging
        let slurm_env_path = get_slurm_env_log_file(
            args.output_dir.clone(),
            args.workflow_id,
            &job_id,
            &node_id,
            task_pid,
        );
        utils::capture_env_vars(std::path::Path::new(&slurm_env_path), "SLURM");

        // Set up configuration
        let mut config = Configuration::new();
        config.base_path = args.url;

        // First, ping the server to ensure we can connect
        match utils::send_with_retries(
            &config,
            || default_api::ping(&config),
            args.wait_for_healthy_database_minutes,
        ) {
            Ok(_) => {
                info!("Successfully connected to server");
            }
            Err(e) => {
                error!("Error pinging server: {}", e);
                std::process::exit(1);
            }
        }

        let workflow = match utils::send_with_retries(
            &config,
            || default_api::get_workflow(&config, args.workflow_id),
            args.wait_for_healthy_database_minutes,
        ) {
            Ok(wf) => wf,
            Err(e) => {
                error!("Error getting workflow: {}", e);
                std::process::exit(1);
            }
        };

        let expiration_buffer_seconds = workflow
            .compute_node_expiration_buffer_seconds
            .unwrap_or(180)
            .max(120);
        info!("Expiration buffer seconds: {}", expiration_buffer_seconds);

        let job_end_time = match slurm_interface.get_job_end_time() {
            Ok(end_time) => end_time,
            Err(e) => {
                error!("Error getting job end time: {}", e);
                std::process::exit(1);
            }
        };

        let effective_end_time = job_end_time - Duration::seconds(expiration_buffer_seconds);
        info!("Effective end time (with buffer): {}", effective_end_time);

        // All compute nodes get the scheduled compute node
        let scheduled_compute_node =
            get_scheduled_compute_node(&config, args.workflow_id, &slurm_interface);

        if slurm_interface.is_head_node()
            && let Some(ref node) = scheduled_compute_node
        {
            set_scheduled_compute_node_status(&config, node, "active");
        }

        let scheduler_id = scheduled_compute_node.as_ref().map(|node| node.id);
        let scheduler_config_id = scheduled_compute_node
            .as_ref()
            .map(|node| node.scheduler_config_id);

        let resources =
            create_node_resources(&slurm_interface, scheduler_config_id, args.is_subtask);
        let job_id_int: i64 = job_id.parse().unwrap_or(0);
        let scheduler = serde_json::json!({
            "scheduler_id": scheduler_id,
            "type": "slurm",
            "slurm_job_id": job_id_int,
        });
        let compute_node =
            create_compute_node(&config, args.workflow_id, &resources, &hostname, scheduler);
        let run_id = match utils::send_with_retries(
            &config,
            || default_api::get_workflow_status(&config, args.workflow_id),
            args.wait_for_healthy_database_minutes,
        ) {
            Ok(status) => status.run_id,
            Err(e) => {
                error!("Error getting workflow status: {}", e);
                std::process::exit(1);
            }
        };

        let unique_label = format!("{}_{}_{}", job_id, node_id, task_pid);

        let mut job_runner = JobRunner::new(
            config.clone(),
            workflow,
            run_id,
            compute_node.id.expect("Compute node ID is required"),
            args.output_dir.clone(),
            args.poll_interval as f64,
            args.max_parallel_jobs.map(|x| x as i64),
            None, // time_limit
            Some(effective_end_time),
            resources,
            scheduler_config_id,
            None, // log_prefix
            None, // cpu_affinity_cpus_per_job
            args.is_subtask,
            unique_label,
        );

        // Register SIGTERM signal handler
        // When Slurm is about to reach walltime, it sends SIGTERM to this process.
        // The handler sets a flag that the job runner checks in its main loop.
        let termination_flag = job_runner.get_termination_flag();
        let mut signals = match Signals::new([SIGTERM]) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to register SIGTERM handler: {}", e);
                std::process::exit(1);
            }
        };

        // Spawn a thread to handle signals
        thread::spawn(move || {
            for sig in signals.forever() {
                if sig == SIGTERM {
                    info!("Received SIGTERM signal from Slurm. Initiating graceful shutdown.");
                    termination_flag.store(true, Ordering::SeqCst);
                    // Exit the signal handler thread after setting the flag
                    break;
                }
            }
        });

        let job_runner_result = job_runner.run_worker();

        match &job_runner_result {
            Ok(result) => {
                info!(
                    "JobRunner completed successfully (had_failures={}, had_terminations={})",
                    result.had_failures, result.had_terminations
                );

                // Only capture dmesg output if there were failures or terminations
                if result.had_failures || result.had_terminations {
                    info!("Capturing dmesg output due to job failures or terminations");
                    let dmesg_path = get_slurm_dmesg_log_file(
                        args.output_dir.clone(),
                        args.workflow_id,
                        &job_id,
                        &node_id,
                        task_pid,
                    );
                    utils::capture_dmesg(std::path::Path::new(&dmesg_path), Some(dmesg_cutoff));
                }
            }
            Err(e) => {
                error!("JobRunner::run_worker failed: {}", e);
                // Capture dmesg on error as well
                let dmesg_path = get_slurm_dmesg_log_file(
                    args.output_dir.clone(),
                    args.workflow_id,
                    &job_id,
                    &node_id,
                    task_pid,
                );
                utils::capture_dmesg(std::path::Path::new(&dmesg_path), Some(dmesg_cutoff));
                std::process::exit(1);
            }
        }

        if slurm_interface.is_head_node()
            && let Some(ref node) = scheduled_compute_node
        {
            set_scheduled_compute_node_status(&config, node, "complete");
        }
    }

    /// Get the scheduled compute node for a Slurm job ID.
    /// Returns the node model if successfully found, None otherwise.
    fn get_scheduled_compute_node(
        config: &Configuration,
        workflow_id: i64,
        slurm_interface: &SlurmInterface,
    ) -> Option<torc::models::ScheduledComputeNodesModel> {
        let job_id = slurm_interface.get_current_job_id();
        debug!(
            "Getting scheduled compute node for Slurm job ID: {}",
            job_id
        );

        let scheduled_nodes = match default_api::list_scheduled_compute_nodes(
            config,
            workflow_id,
            None,          // offset
            None,          // limit
            None,          // sort_by
            None,          // reverse_sort
            Some(&job_id), // scheduler_id
            None,          // scheduler_config_id
            None,          // status
        ) {
            Ok(response) => response,
            Err(e) => {
                error!("Error listing scheduled compute nodes: {}", e);
                return None;
            }
        };

        let items = scheduled_nodes.items.unwrap_or_default();
        if items.len() != 1 {
            error!(
                "Expected exactly 1 scheduled compute node for Slurm job ID {}, found {}",
                job_id,
                items.len()
            );
            return None;
        }

        Some(items[0].clone())
    }

    /// Set the status of a scheduled compute node.
    fn set_scheduled_compute_node_status(
        config: &Configuration,
        node: &torc::models::ScheduledComputeNodesModel,
        status: &str,
    ) {
        let mut updated_node = node.clone();
        let node_id = updated_node
            .id
            .expect("Scheduled compute node must have an ID");

        updated_node.status = status.to_string();

        match default_api::update_scheduled_compute_node(config, node_id, updated_node) {
            Ok(result) => {
                info!(
                    "Successfully updated scheduled compute node {} to status: {}",
                    node_id, result.status
                );
            }
            Err(e) => {
                error!(
                    "Error updating scheduled compute node {} to status '{}': {}",
                    node_id, status, e
                );
            }
        }
    }
}

#[cfg(unix)]
fn main() {
    unix_main::main();
}
