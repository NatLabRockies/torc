use crate::client::apis;
use crate::client::apis::configuration::{Configuration, TlsConfig};
use crate::client::commands::get_env_user_name;
use crate::client::commands::select_workflow_interactively;
use crate::client::job_runner::{JobRunner, WorkerResult};
use crate::client::log_paths::get_job_runner_log_file;
use crate::client::utils::detect_nvidia_gpus;
use crate::client::workflow_manager::WorkflowManager;
use crate::config::TorcConfig;
use crate::models;
use crate::time_utils::duration_string_to_seconds;
use chrono::{DateTime, Utc};
use clap::Parser;
use env_logger::Builder;
use log::{LevelFilter, error, info};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use sysinfo::{CpuRefreshKind, RefreshKind, System, SystemExt};

pub enum LogStream {
    Stdout,
    Stderr,
}

enum ConsoleWriter {
    Stdout(std::io::Stdout),
    Stderr(std::io::Stderr),
}

impl Write for ConsoleWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            ConsoleWriter::Stdout(stdout) => {
                stdout.write_all(buf)?;
            }
            ConsoleWriter::Stderr(stderr) => {
                stderr.write_all(buf)?;
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            ConsoleWriter::Stdout(stdout) => stdout.flush(),
            ConsoleWriter::Stderr(stderr) => stderr.flush(),
        }
    }
}

/// A writer that writes to a console stream and a file.
struct MultiWriter {
    console: ConsoleWriter,
    file: File,
}

impl Write for MultiWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.console.write_all(buf)?;
        self.file.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.console.flush()?;
        self.file.flush()
    }
}

#[derive(Parser, Debug)]
#[command(about = "Run jobs locally on the current node", long_about = None)]
pub struct Args {
    /// Workflow ID
    #[arg()]
    pub workflow_id: Option<i64>,
    /// URL of torc server
    #[arg(short, long, default_value = "http://localhost:8080/torc-service/v1")]
    pub url: String,
    /// Output directory for jobs
    #[arg(short, long, default_value = "torc_output")]
    pub output_dir: PathBuf,
    /// Job completion poll interval in seconds
    #[arg(short, long, default_value = "5.0")]
    pub poll_interval: f64,
    /// Maximum number of parallel jobs to run concurrently.
    /// When NOT set: Uses resource-based job allocation (considers CPU, memory, GPU requirements).
    /// When set: Uses simple queue-based allocation with this parallel limit (ignores resource requirements).
    #[arg(long)]
    pub max_parallel_jobs: Option<i64>,
    /// Time limit for jobs
    #[arg(long)]
    pub time_limit: Option<String>,
    /// End time for job execution
    #[arg(long)]
    pub end_time: Option<String>,
    /// Number of CPUs
    #[arg(long)]
    pub num_cpus: Option<i64>,
    /// Memory in GB
    #[arg(long)]
    pub memory_gb: Option<f64>,
    /// Number of GPUs
    #[arg(long)]
    pub num_gpus: Option<i64>,
    /// Number of nodes
    #[arg(long)]
    pub num_nodes: Option<i64>,
    /// Scheduler config ID
    #[arg(long)]
    pub scheduler_config_id: Option<i64>,
    /// Log prefix
    #[arg(long)]
    pub log_prefix: Option<String>,
    /// CPU affinity CPUs per job
    #[arg(long)]
    pub cpu_affinity_cpus_per_job: Option<i64>,
    /// Log level (error, warn, info, debug, trace)
    #[arg(long, default_value = "info")]
    pub log_level: String,
    /// Password for authentication (can also use TORC_PASSWORD env var)
    #[arg(long, env = "TORC_PASSWORD", hide_env_values = true)]
    pub password: Option<String>,
    /// Path to a PEM-encoded CA certificate to trust for TLS connections
    #[arg(long, env = "TORC_TLS_CA_CERT")]
    pub tls_ca_cert: Option<String>,
    /// Skip TLS certificate verification (for testing only)
    #[arg(long, env = "TORC_TLS_INSECURE")]
    pub tls_insecure: bool,
    /// Cookie header value for authentication (e.g., from browser-based MFA)
    #[arg(long, env = "TORC_COOKIE_HEADER", hide_env_values = true)]
    pub cookie_header: Option<String>,
}

fn resolve_end_time(
    end_time: Option<&str>,
    time_limit: Option<&str>,
) -> Result<Option<DateTime<Utc>>, String> {
    if let Some(end_time_str) = end_time {
        return end_time_str
            .parse::<DateTime<Utc>>()
            .map(Some)
            .map_err(|e| format!("Error parsing end_time: {}", e));
    }

    if let Some(time_limit_str) = time_limit {
        let seconds = duration_string_to_seconds(time_limit_str)
            .map_err(|e| format!("Error parsing time_limit '{}': {}", time_limit_str, e))?;
        return Ok(Some(Utc::now() + chrono::Duration::seconds(seconds)));
    }

    Ok(None)
}

pub fn run(args: &Args) {
    let _ = run_with_log_stream(args, LogStream::Stdout);
}

pub fn run_with_log_stream(args: &Args, log_stream: LogStream) -> WorkerResult {
    let hostname = hostname::get()
        .expect("Failed to get hostname")
        .into_string()
        .expect("Hostname is not valid UTF-8");
    let tls = TlsConfig {
        ca_cert_path: args.tls_ca_cert.as_ref().map(std::path::PathBuf::from),
        insecure: args.tls_insecure,
    };
    let mut config = Configuration::with_tls(tls);
    config.base_path = args.url.clone();

    // Set up authentication if password is provided
    if let Some(ref password) = args.password {
        let username = get_env_user_name();
        config.basic_auth = Some((username, Some(password.clone())));
    }

    // Set cookie header for authentication (e.g., from browser-based MFA)
    if let Some(ref cookie_header) = args.cookie_header {
        config.cookie_header = Some(cookie_header.clone());
        if let Err(e) = config.apply_cookie_header() {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
    let user = get_env_user_name();
    let workflow_id = args.workflow_id.unwrap_or_else(|| {
        select_workflow_interactively(&config, &user).unwrap_or_else(|e| {
            eprintln!("Error selecting workflow: {}", e);
            std::process::exit(1);
        })
    });
    let workflow = match apis::workflows_api::get_workflow(&config, workflow_id) {
        Ok(workflow) => workflow,
        Err(e) => {
            eprintln!("Error getting workflow: {}", e);
            std::process::exit(1);
        }
    };

    // Check if all jobs are uninitialized and initialize the workflow if needed
    match apis::workflows_api::is_workflow_uninitialized(&config, workflow_id) {
        Ok(response) => {
            if let Some(is_uninitialized) =
                response.get("is_uninitialized").and_then(|v| v.as_bool())
                && is_uninitialized
            {
                eprintln!(
                    "Workflow {} has all jobs uninitialized. Initializing workflow...",
                    workflow_id
                );
                let torc_config = TorcConfig::load().unwrap_or_default();
                let workflow_manager =
                    WorkflowManager::new(config.clone(), torc_config, workflow.clone());
                match workflow_manager.initialize(false) {
                    Ok(()) => {
                        eprintln!("Successfully initialized workflow {}", workflow_id);
                    }
                    Err(e) => {
                        eprintln!("Error initializing workflow: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            // If workflow is already initialized, proceed to run it
            // (no action needed, just continue)
        }
        Err(e) => {
            eprintln!("Error checking if workflow is uninitialized: {}", e);
            std::process::exit(1);
        }
    }

    let run_id = match apis::workflows_api::get_workflow_status(&config, workflow_id) {
        Ok(status) => status.run_id,
        Err(e) => {
            eprintln!("Error getting workflow status: {}", e);
            std::process::exit(1);
        }
    };

    // Create output directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(&args.output_dir) {
        eprintln!(
            "Error creating output directory {}: {}",
            args.output_dir.display(),
            e
        );
        std::process::exit(1);
    }

    let log_file_path =
        get_job_runner_log_file(args.output_dir.clone(), &hostname, workflow_id, run_id);
    let log_file = match File::create(&log_file_path) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("Error creating log file {}: {}", log_file_path, e);
            std::process::exit(1);
        }
    };

    let console = match log_stream {
        LogStream::Stdout => ConsoleWriter::Stdout(std::io::stdout()),
        LogStream::Stderr => ConsoleWriter::Stderr(std::io::stderr()),
    };
    let multi_writer = MultiWriter {
        console,
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

    info!("Starting job runner");
    info!("Hostname: {}", hostname);
    info!("Output directory: {}", args.output_dir.display());
    info!("Log file: {}", log_file_path);

    let parsed_end_time =
        match resolve_end_time(args.end_time.as_deref(), args.time_limit.as_deref()) {
            Ok(end_time) => end_time,
            Err(e) => {
                error!("{}", e);
                std::process::exit(1);
            }
        };

    // Use new_with_specifics to only refresh CPU and memory, avoiding user enumeration
    // which can crash on HPC systems with large LDAP user databases
    let refresh_kind = RefreshKind::new()
        .with_cpu(CpuRefreshKind::everything())
        .with_memory();
    let mut system = System::new_with_specifics(refresh_kind);
    system.refresh_cpu();
    system.refresh_memory();
    let system_cpus = system.cpus().len() as i64;
    let system_memory_gb = (system.total_memory() as f64) / (1024.0 * 1024.0 * 1024.0);
    let system_gpus = detect_nvidia_gpus();

    let mut resources = models::ComputeNodesResources::new(
        args.num_cpus.unwrap_or(system_cpus),
        args.memory_gb.unwrap_or(system_memory_gb),
        args.num_gpus.unwrap_or(system_gpus),
        args.num_nodes.unwrap_or(1),
    );
    resources.time_limit.clone_from(&args.time_limit);
    let pid = 1; // TODO
    let unique_label = format!("wf{}_h{}_r{}", workflow_id, hostname, run_id);

    let mut compute_node_model = models::ComputeNodeModel::new(
        workflow_id,
        hostname.clone(),
        pid,
        Utc::now().to_rfc3339(),
        resources.num_cpus,
        resources.memory_gb,
        resources.num_gpus,
        resources.num_nodes,
        "local".to_string(),
        None,
    );
    compute_node_model.is_active = Some(true);

    let compute_node =
        match apis::compute_nodes_api::create_compute_node(&config, compute_node_model) {
            Ok(node) => node,
            Err(e) => {
                error!("Error creating compute node: {}", e);
                std::process::exit(1);
            }
        };

    let mut job_runner = JobRunner::new(
        config.clone(),
        workflow,
        run_id,
        compute_node.id.expect("Compute node ID should be set"),
        args.output_dir.clone(),
        args.poll_interval,
        args.max_parallel_jobs,
        args.time_limit.clone(),
        parsed_end_time,
        resources,
        args.scheduler_config_id,
        args.log_prefix.clone(),
        args.cpu_affinity_cpus_per_job,
        false,
        unique_label,
        None, // No per-node tracking for local runner
    );

    match job_runner.run_worker() {
        Ok(result) => {
            info!(
                "Job runner completed successfully (had_failures={}, had_terminations={})",
                result.had_failures, result.had_terminations
            );
            result
        }
        Err(e) => {
            error!("Job runner failed: {}", e);
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_end_time;
    use chrono::{Duration, Utc};

    #[test]
    fn test_resolve_end_time_prefers_explicit_end_time() {
        let explicit = "2026-03-15T12:00:00Z";
        let resolved = resolve_end_time(Some(explicit), Some("PT1H")).unwrap();

        assert_eq!(resolved.unwrap().to_rfc3339(), "2026-03-15T12:00:00+00:00");
    }

    #[test]
    fn test_resolve_end_time_from_time_limit() {
        let before = Utc::now();
        let resolved = resolve_end_time(None, Some("PT1M")).unwrap().unwrap();
        let after = Utc::now();

        assert!(resolved >= before + Duration::seconds(60));
        assert!(resolved <= after + Duration::seconds(60));
    }

    #[test]
    fn test_resolve_end_time_rejects_invalid_time_limit() {
        let err = resolve_end_time(None, Some("not-a-duration")).unwrap_err();
        assert!(err.contains("Error parsing time_limit"));
    }
}
