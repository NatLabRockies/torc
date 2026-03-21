// This binary is only supported on Unix systems (Slurm is Linux-only)
#[cfg(not(unix))]
fn main() {
    eprintln!("torc-slurm-job-runner is only supported on Unix systems (Linux/macOS).");
    eprintln!("Slurm is not available on Windows.");
    std::process::exit(1);
}

#[cfg(unix)]
mod unix_main {
    use chrono::Local;
    use clap::{Parser, builder::styling};
    use env_logger::Builder;
    use log::{LevelFilter, debug, error, info, warn};
    use signal_hook::consts::SIGTERM;
    use signal_hook::iterator::Signals;
    use std::fs::File;
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;
    use std::thread;
    use torc::client::apis::configuration::{Configuration, TlsConfig};
    use torc::client::apis::default_api;
    use torc::client::commands::slurm::{create_compute_node, create_node_resources};
    use torc::client::config::TorcConfig;
    use torc::client::hpc::hpc_interface::HpcInterface;
    use torc::client::hpc::slurm_interface::SlurmInterface;
    use torc::client::job_runner::{JobRunner, PerNodeTracker};
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
    #[command(version)]
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
        #[arg(short, long)]
        poll_interval: Option<i64>,

        /// Set to true if this is a subtask and multiple workers are running on one Slurm allocation
        #[arg(long, default_value = "false")]
        is_subtask: bool,

        /// Wait this number of minutes if the database is offline
        #[arg(long, default_value = "20")]
        wait_for_healthy_database_minutes: u64,

        /// Path to a PEM-encoded CA certificate to trust for TLS connections
        #[arg(long, env = "TORC_TLS_CA_CERT")]
        tls_ca_cert: Option<String>,

        /// Skip TLS certificate verification (for testing only)
        #[arg(long, env = "TORC_TLS_INSECURE")]
        tls_insecure: bool,

        /// Password for authentication (can also use TORC_PASSWORD env var)
        #[arg(long, env = "TORC_PASSWORD", hide_env_values = true)]
        password: Option<String>,

        /// Log level: error, warn, info, debug, trace
        #[arg(long)]
        log_level: Option<String>,

        /// Maximum startup delay in seconds for thundering herd mitigation.
        /// Each runner sleeps a deterministic jitter in [0, N) seconds before
        /// contacting the server, spreading load when many nodes start at once.
        #[arg(long, default_value = "0")]
        startup_delay_seconds: u64,
    }

    fn workflow_has_multi_node_jobs(
        config: &Configuration,
        workflow_id: i64,
        wait_for_healthy_database_minutes: u64,
    ) -> bool {
        let mut offset = 0i64;
        loop {
            let response = match utils::send_with_retries(
                config,
                || {
                    default_api::list_resource_requirements(
                        config,
                        workflow_id,
                        None,
                        Some(offset),
                        Some(100),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    )
                },
                wait_for_healthy_database_minutes,
            ) {
                Ok(response) => response,
                Err(e) => {
                    warn!(
                        "Could not inspect workflow resource requirements for multi-node jobs: {}. \
                         Disabling per-node placement to avoid over-allocation.",
                        e
                    );
                    return true;
                }
            };

            let items = response.items.unwrap_or_default();

            if items.iter().any(|rr| rr.num_nodes > 1) {
                return true;
            }

            if !response.has_more || items.is_empty() {
                return false;
            }
            offset += items.len() as i64;
        }
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

        // Resolve log level: CLI arg > config file > default ("info")
        let file_config = TorcConfig::load().unwrap_or_default();
        let log_level_str = args
            .log_level
            .clone()
            .unwrap_or_else(|| file_config.client.log_level.clone());

        let level_filter = match log_level_str.to_lowercase().as_str() {
            "error" => LevelFilter::Error,
            "warn" => LevelFilter::Warn,
            "info" => LevelFilter::Info,
            "debug" => LevelFilter::Debug,
            "trace" => LevelFilter::Trace,
            _ => {
                eprintln!(
                    "Warning: unknown log level '{}', defaulting to 'info'",
                    log_level_str
                );
                LevelFilter::Info
            }
        };

        // Initialize logger now that we have the log file
        let mut builder = Builder::from_default_env();
        builder
            .target(env_logger::Target::Pipe(Box::new(log_file)))
            .filter_level(level_filter)
            .init();

        let hostname = hostname::get()
            .expect("Failed to get hostname")
            .into_string()
            .expect("Hostname is not valid UTF-8");

        info!("Starting Slurm job runner (log_level={})", log_level_str);
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

        // Set up configuration with TLS
        let tls = TlsConfig {
            ca_cert_path: args.tls_ca_cert.as_ref().map(std::path::PathBuf::from),
            insecure: args.tls_insecure,
        };
        let mut config = Configuration::with_tls(tls);
        config.base_path = args.url.clone();

        // Set up authentication if password is provided
        if let Some(ref password) = args.password {
            let username = torc::get_username();
            config.basic_auth = Some((username, Some(password.clone())));
        }

        // Set cookie header for authentication (e.g., from browser-based MFA)
        if let Err(e) = config.apply_cookie_header_from_env() {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }

        // Stagger startup to avoid thundering herd when many compute nodes start
        // simultaneously. The delay window is set by the caller (sbatch script)
        // based on the number of concurrent allocations.
        if args.startup_delay_seconds > 0 {
            let jitter = {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                hostname.hash(&mut hasher);
                job_id.hash(&mut hasher);
                node_id.hash(&mut hasher);
                task_pid.hash(&mut hasher);
                hasher.finish() % args.startup_delay_seconds
            };
            info!(
                "Startup jitter: sleeping {} seconds (window={})",
                jitter, args.startup_delay_seconds
            );
            thread::sleep(std::time::Duration::from_secs(jitter));
        }

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

        if workflow.compute_node_expiration_buffer_seconds.is_some() {
            warn!(
                "compute_node_expiration_buffer_seconds is deprecated and will be ignored. \
                 Slurm now manages job termination signals via srun --time. \
                 Configure Slurm's KillWait parameter instead."
            );
        }

        let job_end_time = match slurm_interface.get_job_end_time() {
            Ok(end_time) => end_time,
            Err(e) => {
                error!("Error getting job end time: {}", e);
                std::process::exit(1);
            }
        };
        info!("Slurm job end time: {}", job_end_time);

        // All compute nodes get the scheduled compute node
        let scheduled_compute_node =
            get_scheduled_compute_node(&config, args.workflow_id, &slurm_interface);

        let scheduler_id = scheduled_compute_node.as_ref().map(|node| node.id);
        let scheduler_config_id = scheduled_compute_node
            .as_ref()
            .map(|node| node.scheduler_config_id);

        let per_node_resources =
            create_node_resources(&slurm_interface, scheduler_config_id, args.is_subtask);

        // Multiply per-node values by num_nodes to get total allocation capacity.
        // The job runner uses total capacity for its resource pool tracking
        // (decrement on job start, increment on completion). When claiming jobs
        // from the server, it converts back to per-node for correct comparison.
        let num_nodes = per_node_resources.num_nodes;
        let mut resources = torc::models::ComputeNodesResources::new(
            per_node_resources.num_cpus * num_nodes,
            per_node_resources.memory_gb * num_nodes as f64,
            per_node_resources.num_gpus * num_nodes,
            num_nodes,
        );
        resources.scheduler_config_id = per_node_resources.scheduler_config_id;
        resources.time_limit = per_node_resources.time_limit;

        // Initialize per-node resource tracker for multi-node allocations.
        // This tracks which node each job lands on so resources_per_node()
        // reports accurate per-node availability instead of dividing
        // remaining total by num_nodes (which gives incorrect values when
        // jobs are unevenly distributed across nodes).
        let has_multi_node_jobs = num_nodes > 1
            && workflow_has_multi_node_jobs(
                &config,
                args.workflow_id,
                args.wait_for_healthy_database_minutes,
            );

        if slurm_interface.is_head_node()
            && let Some(ref node) = scheduled_compute_node
        {
            set_scheduled_compute_node_status(&config, node, "active");
        }

        let node_tracker = if num_nodes > 1 && !has_multi_node_jobs {
            match slurm_interface.list_active_nodes(&job_id) {
                Ok(node_names) => {
                    info!(
                        "Multi-node allocation: {} nodes {:?}, initializing per-node resource tracker",
                        num_nodes, node_names
                    );
                    Some(PerNodeTracker::new(
                        node_names,
                        per_node_resources.num_cpus,
                        per_node_resources.memory_gb,
                        per_node_resources.num_gpus,
                    ))
                }
                Err(e) => {
                    warn!(
                        "Could not enumerate nodes for multi-node allocation: {}. \
                         Falling back to aggregate resource tracking.",
                        e
                    );
                    None
                }
            }
        } else if has_multi_node_jobs {
            info!(
                "Workflow has multi-node jobs; using aggregate resource tracking. \
                 Multi-node jobs reserve whole nodes exclusively."
            );
            None
        } else {
            None
        };

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

        let unique_label = format!(
            "wf{}_sl{}_n{}_p{}",
            args.workflow_id, job_id, node_id, task_pid
        );

        let mut job_runner = JobRunner::new(
            config.clone(),
            workflow,
            run_id,
            compute_node.id.expect("Compute node ID is required"),
            args.output_dir.clone(),
            args.poll_interval
                .unwrap_or(file_config.client.slurm.poll_interval as i64) as f64,
            args.max_parallel_jobs.map(|x| x as i64),
            None, // time_limit
            Some(job_end_time),
            resources,
            scheduler_config_id,
            None, // log_prefix
            None, // cpu_affinity_cpus_per_job
            args.is_subtask,
            unique_label,
            node_tracker,
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
