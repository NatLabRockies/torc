use std::io::{BufRead, BufReader, IsTerminal};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use clap::{CommandFactory, Parser};

use torc::cli::{Cli, Commands};
use torc::client::apis;
use torc::client::apis::configuration::{Configuration, TlsConfig};
use torc::client::commands::access_groups::handle_access_group_commands;
use torc::client::commands::admin::handle_admin_commands;
use torc::client::commands::compute_nodes::handle_compute_node_commands;
use torc::client::commands::config::handle_config_commands;
use torc::client::commands::events::handle_event_commands;
use torc::client::commands::failure_handlers::handle_failure_handler_commands;
use torc::client::commands::files::handle_file_commands;
use torc::client::commands::hpc::handle_hpc_commands;
use torc::client::commands::job_dependencies::handle_job_dependency_commands;
use torc::client::commands::jobs::handle_job_commands;
use torc::client::commands::logs::handle_log_commands;
use torc::client::commands::recover::{
    RecoverArgs, RecoveryReport, diagnose_failures, recover_workflow,
};
use torc::client::commands::remote::handle_remote_commands;
use torc::client::commands::resource_requirements::handle_resource_requirements_commands;
use torc::client::commands::results::handle_result_commands;
use torc::client::commands::ro_crate::handle_ro_crate_commands;
use torc::client::commands::scheduled_compute_nodes::handle_scheduled_compute_node_commands;
use torc::client::commands::slurm::handle_slurm_commands;
use torc::client::commands::tasks::handle_tasks_commands;
use torc::client::commands::user_data::handle_user_data_commands;
use torc::client::commands::watch::{WatchArgs, run_watch};
use torc::client::commands::workflows::{handle_cancel, handle_workflow_commands};
use torc::client::config::TorcConfig;
use torc::client::version_check;
use torc::client::workflow_manager::WorkflowManager;
use torc::client::workflow_spec::WorkflowSpec;

// Import the binary command modules from the library
use torc::exec_cmd;
use torc::plot_resources_cmd;
use torc::run_jobs_cmd;
use torc::tui_runner;

/// Helper to print a workflow message in the appropriate format (JSON or plain text).
fn print_workflow_message(format: &str, workflow_id: i64, message: &str) {
    if format == "json" {
        println!(
            "{}",
            serde_json::json!({"workflow_id": workflow_id, "message": message})
        );
    } else {
        println!("{}", message);
    }
}

fn command_used_delimiter(command_name: &str) -> bool {
    let mut seen_command = false;
    for arg in std::env::args_os().skip(1) {
        if seen_command {
            if arg == "--" {
                return true;
            }
            continue;
        }
        if arg == command_name {
            seen_command = true;
        }
    }
    false
}

/// Handle to an ephemeral torc-server subprocess started by `--standalone`.
///
/// Normal-exit cleanup is handled by `Drop`, which kills and reaps the child.
/// `std::process::exit()` bypasses destructors, so we also hand the child a
/// piped stdin that we never close and pass `--shutdown-on-stdin-eof` to the
/// server. When the parent process terminates by any means (normal return,
/// `process::exit`, SIGKILL, crash), the kernel closes the pipe write end, the
/// server reads EOF, and it shuts itself down gracefully. The pipe is the
/// safety net; the explicit kill() is the fast path.
struct StandaloneServer {
    child: std::process::Child,
    api_url: String,
    db_path: std::path::PathBuf,
    // Write end of the child's stdin pipe. Held open to keep the child alive;
    // dropping it (explicitly in `Drop::drop` or implicitly on parent exit)
    // triggers the server's graceful shutdown via its `--shutdown-on-stdin-eof`
    // path. Stored as Option so `Drop::drop` can move it out and close it
    // before waiting.
    stdin: Option<std::process::ChildStdin>,
    /// True when launched with `--in-memory`. Drives final-snapshot behavior
    /// and unlocks the SIGUSR1 path.
    in_memory: bool,
    /// Receiver for `TORC_SNAPSHOT_DONE=<path>` lines from the child's stdout.
    /// Bounded (capacity 1) so periodic-snapshot notifications can't pile up.
    /// `None` outside in-memory mode.
    snapshot_done_rx: Option<mpsc::Receiver<()>>,
    /// Sender that, when dropped, signals the periodic-snapshot thread to
    /// stop. `None` if no periodic thread was spawned.
    periodic_stop_tx: Option<mpsc::Sender<()>>,
    /// Join handle for the periodic-snapshot thread, taken in `Drop`.
    periodic_handle: Option<thread::JoinHandle<()>>,
}

impl StandaloneServer {
    /// Send `SIGUSR1` to the child process to trigger a snapshot.
    /// No-op on non-Unix; relies on the child being addressable by its PID.
    #[cfg(unix)]
    fn send_sigusr1(&self) -> std::io::Result<()> {
        let pid = self.child.id() as libc::pid_t;
        let rc = unsafe { libc::kill(pid, libc::SIGUSR1) };
        if rc == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    #[cfg(not(unix))]
    fn send_sigusr1(&self) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "SIGUSR1 snapshots are only supported on Unix",
        ))
    }
}

impl Drop for StandaloneServer {
    fn drop(&mut self) {
        // Stop the periodic-snapshot thread first so we don't race a tick
        // against the final snapshot.
        drop(self.periodic_stop_tx.take());
        if let Some(h) = self.periodic_handle.take() {
            let _ = h.join();
        }

        // For --in-memory mode, request a final snapshot and wait for the
        // server to confirm it landed on disk before we shut things down.
        // This is what makes the user-facing contract "when the command
        // returns, the workflow is queryable from --db" hold.
        if self.in_memory {
            // Drain any pending notifications from periodic snapshots before
            // requesting the final one — otherwise `recv_timeout` below could
            // return immediately on a stale notification and proceed to
            // shutdown before the *final* snapshot has actually completed.
            if let Some(rx) = &self.snapshot_done_rx {
                while rx.try_recv().is_ok() {}
            }
            match self.send_sigusr1() {
                Ok(()) => {
                    if let Some(rx) = &self.snapshot_done_rx {
                        // Cap the wait so a stuck server can't hang shutdown
                        // indefinitely. Five seconds is enough for any
                        // reasonable in-memory database.
                        match rx.recv_timeout(Duration::from_secs(5)) {
                            Ok(()) => {}
                            Err(mpsc::RecvTimeoutError::Timeout) => eprintln!(
                                "warning: timed out waiting for final snapshot to {}",
                                self.db_path.display()
                            ),
                            Err(mpsc::RecvTimeoutError::Disconnected) => eprintln!(
                                "warning: server exited before confirming final snapshot to {}",
                                self.db_path.display()
                            ),
                        }
                    }
                }
                Err(e) => eprintln!("warning: failed to request final snapshot: {}", e),
            }
        }

        // Close our end of the child's stdin pipe. The server is running with
        // `--shutdown-on-stdin-eof`, so this alone should cause it to drain
        // connections and exit on its own — a hard kill risks interrupting an
        // in-flight SQLite write.
        drop(self.stdin.take());

        // Poll for graceful exit for up to 2 seconds. In practice the server
        // exits in a few tens of milliseconds after EOF when no connections are
        // active, which is the common case for a one-shot `torc -s ...`.
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        break;
                    }
                    thread::sleep(Duration::from_millis(25));
                }
                Err(_) => break,
            }
        }

        // Fell through the grace window — force-kill as a last resort.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Send `SIGUSR1` every `interval` until the stop channel is closed.
/// Used by `--snapshot-interval-seconds` to drive periodic snapshots from the
/// parent without touching the server-side scheduler.
#[cfg(unix)]
fn periodic_snapshot_loop(pid: u32, interval: Duration, stop: mpsc::Receiver<()>) {
    loop {
        match stop.recv_timeout(interval) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let rc = unsafe { libc::kill(pid as libc::pid_t, libc::SIGUSR1) };
                if rc != 0 {
                    eprintln!(
                        "warning: periodic snapshot SIGUSR1 failed: {}",
                        std::io::Error::last_os_error()
                    );
                    return;
                }
            }
        }
    }
}

#[cfg(not(unix))]
fn periodic_snapshot_loop(_pid: u32, _interval: Duration, _stop: mpsc::Receiver<()>) {}

/// Options for the standalone server spawn.
struct StandaloneOptions {
    server_bin: String,
    /// On-disk path. In normal mode this is the live database; in `--in-memory`
    /// mode this is the snapshot destination.
    db: Option<std::path::PathBuf>,
    in_memory: bool,
    /// When `Some(secs)`, parent-side timer that sends SIGUSR1 every `secs`.
    /// Only honored when `in_memory` is true.
    snapshot_interval_seconds: Option<u64>,
}

/// Spawn a torc-server subprocess bound to 127.0.0.1 on an auto-assigned port,
/// backed by the given SQLite database (default: `./torc_output/torc.db`).
/// Waits up to 15 seconds for the server to print its `TORC_SERVER_PORT=<port>` line.
fn start_standalone_server(opts: StandaloneOptions) -> Result<StandaloneServer, String> {
    if opts.in_memory && !cfg!(unix) {
        return Err("--in-memory is only supported on Unix systems".to_string());
    }

    let db_path = opts
        .db
        .unwrap_or_else(|| std::path::PathBuf::from("torc_output/torc.db"));
    if let Some(parent) = db_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "could not create database parent directory {}: {}",
                parent.display(),
                e
            )
        })?;
    }

    // In --in-memory mode the server's `--database` argument becomes literal
    // `:memory:` and the on-disk path is wired through as the snapshot
    // destination via env. Default to `KEEP=1` so users see one canonical
    // file at the path they expect, not rotated `.1`/`.2` siblings.
    let database_arg = if opts.in_memory {
        ":memory:".to_string()
    } else {
        db_path.display().to_string()
    };

    let mut command = std::process::Command::new(&opts.server_bin);
    command
        .args([
            "run",
            "--host",
            "127.0.0.1",
            "--port",
            "0",
            "--database",
            &database_arg,
            "--shutdown-on-stdin-eof",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    if opts.in_memory {
        command.env("TORC_SERVER_SNAPSHOT_PATH", &db_path);
        // Only set KEEP=1 if the user hasn't already overridden it, so power
        // users can opt into rotation by exporting the env var themselves.
        if std::env::var_os("TORC_SERVER_SNAPSHOT_KEEP").is_none() {
            command.env("TORC_SERVER_SNAPSHOT_KEEP", "1");
        }
    }

    let mut child = command.spawn().map_err(|e| {
        format!(
            "failed to spawn '{}': {}. Set --torc-server-bin or TORC_SERVER_BIN.",
            opts.server_bin, e
        )
    })?;

    let child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| "failed to capture torc-server stdin".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture torc-server stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture torc-server stderr".to_string())?;

    // Forward torc-server's stderr to our own stderr line-by-line. We don't inherit
    // the fd (that would let the child keep our stderr pipe open after we exit via
    // process::exit, which skips Drop) and we don't let the pipe buffer fill.
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            eprintln!("[torc-server] {}", line);
        }
    });

    // Read stdout in a background thread. Until the port is reported, lines are
    // forwarded over `tx` so we can enforce a timeout on the startup handshake.
    // After the receiver is dropped (port found), the thread keeps draining stdout
    // for the lifetime of the child so the pipe buffer can't fill and block it.
    //
    // For --in-memory mode we additionally route `TORC_SNAPSHOT_DONE=<path>`
    // lines onto `snapshot_done_tx` so the parent can synchronize on snapshot
    // completion (final snapshot during Drop, or for tests).
    let (tx, rx) = mpsc::channel::<Result<String, std::io::Error>>();
    // Bounded `sync_channel(1)` plus `try_send` gives drop-on-full semantics:
    // if the parent hasn't yet consumed the previous snapshot notification (it
    // only does so during Drop), additional notifications from periodic
    // snapshots are silently dropped rather than accumulating unboundedly.
    // The parent drains the channel before requesting the final snapshot, so
    // it never waits on a stale notification.
    let (snapshot_done_tx, snapshot_done_rx) = if opts.in_memory {
        let (s, r) = mpsc::sync_channel::<()>(1);
        (Some(s), Some(r))
    } else {
        (None, None)
    };
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut tx = Some(tx);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if line.trim().starts_with("TORC_SNAPSHOT_DONE=") {
                        if let Some(s) = &snapshot_done_tx {
                            let _ = s.try_send(());
                        }
                        // Don't forward to stderr — internal sync line.
                        continue;
                    }
                    if let Some(sender) = tx.as_ref() {
                        if sender.send(Ok(line.clone())).is_ok() {
                            continue;
                        }
                        tx = None;
                    }
                    eprintln!("[torc-server] {}", line);
                }
                Err(e) => {
                    if let Some(sender) = tx.take() {
                        let _ = sender.send(Err(e));
                    }
                    break;
                }
            }
        }
    });

    let kill_and_reap = |child: &mut std::process::Child| {
        let _ = child.kill();
        let _ = child.wait();
    };

    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            kill_and_reap(&mut child);
            return Err("timeout waiting for torc-server to report its port".to_string());
        }
        match rx.recv_timeout(remaining) {
            Ok(Ok(line)) => {
                if let Some(rest) = line.trim().strip_prefix("TORC_SERVER_PORT=")
                    && let Ok(port) = rest.parse::<u16>()
                {
                    // Match the bind address (127.0.0.1) rather than `localhost`. On
                    // systems where `localhost` resolves to `::1` first but the server
                    // is only bound to v4, connections would fail.
                    let api_url = format!("http://127.0.0.1:{}/torc-service/v1", port);
                    let pid = child.id();

                    // Spawn the periodic-snapshot thread if requested. Park it
                    // on a stop channel with timeout so Drop can stop it
                    // before the final synchronous snapshot.
                    let (periodic_stop_tx, periodic_handle) = if opts.in_memory
                        && let Some(secs) = opts.snapshot_interval_seconds
                    {
                        let (stop_tx, stop_rx) = mpsc::channel::<()>();
                        let interval = Duration::from_secs(secs);
                        let h =
                            thread::spawn(move || periodic_snapshot_loop(pid, interval, stop_rx));
                        (Some(stop_tx), Some(h))
                    } else {
                        (None, None)
                    };

                    return Ok(StandaloneServer {
                        child,
                        api_url,
                        db_path,
                        stdin: Some(child_stdin),
                        in_memory: opts.in_memory,
                        snapshot_done_rx,
                        periodic_stop_tx,
                        periodic_handle,
                    });
                }
                // Other lines are passed through to stderr so users see startup logs.
                eprintln!("[torc-server] {}", line);
            }
            Ok(Err(e)) => {
                kill_and_reap(&mut child);
                return Err(format!("error reading torc-server stdout: {}", e));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                kill_and_reap(&mut child);
                return Err("timeout waiting for torc-server to report its port".to_string());
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = child.wait();
                return Err("torc-server exited before reporting a port".to_string());
            }
        }
    }
}

/// Helper function to determine if a string is a file path or workflow ID
fn is_spec_file(arg: &str) -> bool {
    arg.ends_with(".yaml")
        || arg.ends_with(".yml")
        || arg.ends_with(".json")
        || arg.ends_with(".json5")
        || std::path::Path::new(arg).is_file()
}

fn main() {
    let cli = Cli::parse();

    // Load configuration from files (system, user, local) and environment variables
    // CLI arguments take precedence over file/env config
    let file_config = TorcConfig::load().unwrap_or_default();

    // Resolve log level with priority: CLI arg > file config > default
    let log_level = cli
        .log_level
        .clone()
        .unwrap_or_else(|| file_config.client.log_level.clone());

    // Initialize logger with CLI argument or RUST_LOG env var
    // Skip initialization for commands that set up their own logging (e.g., Run, Watch, Tui)
    // or output to stdout (e.g., Completions)
    let skip_logger_init = matches!(
        cli.command,
        Commands::Run { .. }
            | Commands::Exec { .. }
            | Commands::Watch { .. }
            | Commands::Tui(..)
            | Commands::Completions { .. }
    );

    if !skip_logger_init {
        env_logger::Builder::new().parse_filters(&log_level).init();
    }

    // Resolve format with priority: CLI arg (non-default) > file config > CLI default
    // Note: clap sets default to "table", so we check if user explicitly provided it
    let format = if cli.format != "table" {
        // User explicitly provided a format
        cli.format.clone()
    } else {
        // Use file config if available, otherwise CLI default
        file_config.client.format.clone()
    };

    // Validate format option for API commands
    if !matches!(format.as_str(), "table" | "json") {
        eprintln!("Error: format must be either 'table' or 'json'");
        std::process::exit(1);
    }

    // Resolve URL with priority: CLI arg > file config > default.
    // `--standalone` may override this once the ephemeral server is running.
    let mut url = cli
        .url
        .clone()
        .unwrap_or_else(|| file_config.client.api_url.clone());

    // Resolve TLS settings with priority: CLI arg > config file > defaults
    let tls_ca_cert = cli
        .tls_ca_cert
        .clone()
        .or_else(|| file_config.client.tls.ca_cert.clone());
    let tls_insecure = cli.tls_insecure || file_config.client.tls.insecure;
    let tls = TlsConfig {
        ca_cert_path: tls_ca_cert.as_ref().map(std::path::PathBuf::from),
        insecure: tls_insecure,
    };

    // Create configuration for API commands with TLS settings
    let mut config = Configuration::with_tls(tls);
    config.base_path = url.clone();

    // Handle authentication: use USER env var as username, password from CLI/env or prompt
    let password = if cli.prompt_password {
        // Prompt for password securely
        match rpassword::prompt_password("Password: ") {
            Ok(pwd) => Some(pwd),
            Err(e) => {
                eprintln!("Error reading password: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        cli.password.clone()
    };

    if let Some(password) = password {
        let username = torc::get_username();
        config.basic_auth = Some((username, Some(password)));
    }

    // Set cookie header for authentication (e.g., from browser-based MFA)
    if let Some(ref cookie_header) = cli.cookie_header {
        config.cookie_header = Some(cookie_header.clone());
        if let Err(e) = config.apply_cookie_header() {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
        // Also expose via env var so TUI and other subprocesses can pick it up
        // SAFETY: This runs during single-threaded CLI initialization before any threads are spawned.
        unsafe { std::env::set_var("TORC_COOKIE_HEADER", cookie_header) };
    }

    // Check server version for commands that communicate with the server
    // Skip for local-only commands or if --skip-version-check is set
    let requires_server = !matches!(
        cli.command,
        Commands::Completions { .. }
            | Commands::PlotResources(..)
            | Commands::Tui(..)
            | Commands::Config { .. }
            | Commands::Hpc { .. }
            | Commands::Exec { dry_run: true, .. }
    );

    // --in-memory snapshots the empty DB over the on-disk path on exit, which
    // would destroy prior data for any command that doesn't create workflow
    // state in the same invocation (e.g. `results list`, `workflows list`).
    // Restrict it to commands that produce the data they're snapshotting.
    if cli.in_memory && !matches!(cli.command, Commands::Exec { .. } | Commands::Run { .. }) {
        eprintln!(
            "Error: --in-memory is only supported with `exec` and `run`. \
             Other commands would snapshot an empty database over your existing data."
        );
        std::process::exit(1);
    }

    // Spawn an ephemeral torc-server when --standalone is set. The guard's Drop
    // terminates the subprocess on normal exit of this function.
    let _standalone_server = if cli.standalone {
        if !requires_server {
            eprintln!("--standalone has no effect for this command; ignoring.");
            None
        } else {
            match start_standalone_server(StandaloneOptions {
                server_bin: cli.torc_server_bin.clone(),
                db: cli.db.clone(),
                in_memory: cli.in_memory,
                snapshot_interval_seconds: cli.snapshot_interval_seconds,
            }) {
                Ok(server) => {
                    url = server.api_url.clone();
                    config.base_path = url.clone();
                    // Do NOT mutate the process-global environment here. By the time we
                    // get here, start_standalone_server has already spawned background
                    // threads, so std::env::set_var would be unsound. Every subprocess
                    // torc spawns (job_runner, async_cli_command, torc-dash, etc.) already
                    // passes TORC_API_URL explicitly at spawn time via .env(), so ambient
                    // env is unnecessary for propagation.
                    eprintln!(
                        "Started standalone torc-server on {} (db: {})",
                        server.api_url,
                        server.db_path.display()
                    );
                    Some(server)
                }
                Err(e) => {
                    eprintln!("Error starting standalone torc-server: {}", e);
                    std::process::exit(1);
                }
            }
        }
    } else {
        None
    };

    if requires_server && !cli.skip_version_check {
        let result = version_check::check_version(&config);
        if result.server_version.is_some() {
            let severity = version_check::print_version_warning(&result);
            if severity.is_blocking() {
                eprintln!("Use --skip-version-check to bypass this check (not recommended)");
                std::process::exit(1);
            }
        }
        // If server is unreachable, we'll let the actual command fail with a better error
    }

    match &cli.command {
        Commands::Create {
            file,
            no_resource_monitoring,
            skip_checks,
            dry_run,
        } => {
            let user = torc::get_username();
            torc::client::commands::workflows::handle_create(
                &config,
                file,
                &user,
                *no_resource_monitoring,
                *skip_checks,
                *dry_run,
                &format,
            );
        }
        Commands::Run {
            workflow_spec_or_id,
            max_parallel_jobs,
            num_cpus,
            memory_gb,
            num_gpus,
            poll_interval,
            output_dir,
            time_limit,
            end_time,
            skip_checks,
        } => {
            let workflow_id = if is_spec_file(workflow_spec_or_id) {
                if !*skip_checks {
                    WorkflowSpec::prevalidate_or_exit(workflow_spec_or_id);
                }

                // Create workflow from spec file
                let user = torc::get_username();
                match WorkflowSpec::create_workflow_from_spec(
                    &config,
                    workflow_spec_or_id,
                    &user,
                    true,
                ) {
                    Ok(id) => {
                        print_workflow_message(&format, id, &format!("Created workflow {}", id));
                        id
                    }
                    Err(e) => {
                        eprintln!("Error creating workflow from spec: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                // Parse as workflow ID
                match workflow_spec_or_id.parse::<i64>() {
                    Ok(id) => id,
                    Err(_) => {
                        eprintln!(
                            "Error: '{}' is neither a valid workflow spec file nor a workflow ID",
                            workflow_spec_or_id
                        );
                        std::process::exit(1);
                    }
                }
            };

            // Build args for run_jobs_cmd with config file fallbacks
            let run_config = &file_config.client.run;
            // Pass through authentication from config
            let password = config.basic_auth.as_ref().and_then(|(_, p)| p.clone());
            let args = run_jobs_cmd::Args {
                workflow_id: Some(workflow_id),
                url: url.clone(),
                output_dir: output_dir
                    .clone()
                    .unwrap_or_else(|| run_config.output_dir.clone()),
                poll_interval: poll_interval.unwrap_or(run_config.poll_interval),
                max_parallel_jobs: max_parallel_jobs.or(run_config.max_parallel_jobs),
                time_limit: time_limit.clone(),
                end_time: end_time.clone(),
                num_cpus: num_cpus.or(run_config.num_cpus),
                memory_gb: memory_gb.or(run_config.memory_gb),
                num_gpus: num_gpus.or(run_config.num_gpus),
                num_nodes: None,
                scheduler_config_id: None,
                log_prefix: None,
                cpu_affinity_cpus_per_job: None,
                log_level: log_level.clone(),
                password,
                tls_ca_cert: tls_ca_cert.clone(),
                tls_insecure,
                cookie_header: config.cookie_header.clone(),
            };

            run_jobs_cmd::run(&args);
        }
        Commands::Exec {
            name,
            description,
            command,
            commands_file,
            param,
            link,
            max_parallel_jobs,
            output_dir,
            dry_run,
            monitor,
            monitor_compute_node,
            generate_plots,
            sample_interval_seconds,
            stdio,
            trailing,
        } => {
            let run_config = &file_config.client.run;
            let user = torc::get_username();
            let password = config.basic_auth.as_ref().and_then(|(_, p)| p.clone());
            let exec_args = exec_cmd::ExecArgs {
                name: name.clone(),
                description: description.clone(),
                commands: command.clone(),
                commands_file: commands_file.clone(),
                params: param.clone(),
                link: link.clone(),
                max_parallel_jobs: max_parallel_jobs.or(run_config.max_parallel_jobs),
                output_dir: output_dir
                    .clone()
                    .unwrap_or_else(|| run_config.output_dir.clone()),
                dry_run: *dry_run,
                monitor: monitor.clone(),
                monitor_compute_node: monitor_compute_node.clone(),
                generate_plots: *generate_plots,
                sample_interval_seconds: *sample_interval_seconds,
                stdio: stdio.clone(),
                trailing: trailing.clone(),
                shell_command_delimited: command_used_delimiter("exec"),
                format: format.clone(),
                log_level: log_level.clone(),
                url: url.clone(),
                password,
                tls_ca_cert: tls_ca_cert.clone(),
                tls_insecure,
                cookie_header: config.cookie_header.clone(),
            };
            exec_cmd::run(exec_args, &config, &user);
        }
        Commands::Submit {
            workflow_spec_or_id,
            ignore_missing_data,
            skip_checks,
            max_parallel_jobs,
            output_dir,
            poll_interval,
        } => {
            let workflow_id = if is_spec_file(workflow_spec_or_id) {
                // Load and validate spec file
                let spec = match WorkflowSpec::from_spec_file(workflow_spec_or_id) {
                    Ok(spec) => spec,
                    Err(e) => {
                        eprintln!("Error loading workflow spec: {}", e);
                        std::process::exit(1);
                    }
                };

                // Check if spec has schedule_nodes action
                if !spec.has_schedule_nodes_action() {
                    eprintln!("Error: Cannot submit workflow");
                    eprintln!();
                    eprintln!(
                        "The spec does not define an on_workflow_start action with schedule_nodes."
                    );
                    eprintln!("To submit to Slurm, either:");
                    eprintln!();
                    eprintln!("  1. Use 'torc slurm generate' to auto-generate schedulers:");
                    eprintln!(
                        "     torc slurm generate --account <account> -o {} {}",
                        workflow_spec_or_id, workflow_spec_or_id
                    );
                    eprintln!("     torc submit {}", workflow_spec_or_id);
                    eprintln!();
                    eprintln!("  2. Add a workflow action manually:");
                    eprintln!("     actions:");
                    eprintln!("       - trigger_type: on_workflow_start");
                    eprintln!("         action_type: schedule_nodes");
                    eprintln!("         scheduler_type: slurm");
                    eprintln!("         scheduler: \"my-scheduler\"");
                    eprintln!();
                    eprintln!("Or run locally instead:");
                    eprintln!("  torc run {}", workflow_spec_or_id);
                    std::process::exit(1);
                }

                if !*skip_checks {
                    WorkflowSpec::prevalidate_or_exit(workflow_spec_or_id);
                }

                // Create workflow from spec
                let user = torc::get_username();

                match WorkflowSpec::create_workflow_from_spec(
                    &config,
                    workflow_spec_or_id,
                    &user,
                    true,
                ) {
                    Ok(id) => {
                        print_workflow_message(&format, id, &format!("Created workflow {}", id));
                        id
                    }
                    Err(e) => {
                        eprintln!("Error creating workflow from spec: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                // Parse as workflow ID
                match workflow_spec_or_id.parse::<i64>() {
                    Ok(id) => id,
                    Err(_) => {
                        eprintln!(
                            "Error: '{}' is neither a valid workflow spec file nor a workflow ID",
                            workflow_spec_or_id
                        );
                        std::process::exit(1);
                    }
                }
            };

            // Check if workflow has schedule_nodes actions (for existing workflows)
            if !is_spec_file(workflow_spec_or_id) {
                match apis::workflow_actions_api::get_workflow_actions(&config, workflow_id) {
                    Ok(actions) => {
                        let has_schedule_nodes = actions.iter().any(|action| {
                            action.trigger_type == "on_workflow_start"
                                && action.action_type == "schedule_nodes"
                        });

                        if !has_schedule_nodes {
                            eprintln!("Error: Cannot submit workflow {}", workflow_id);
                            eprintln!();
                            eprintln!(
                                "The workflow does not define an on_workflow_start action with schedule_nodes."
                            );
                            eprintln!(
                                "To submit to a scheduler, the workflow must have an action configured."
                            );
                            eprintln!();
                            eprintln!("Or run locally instead:");
                            eprintln!("  torc run {}", workflow_id);
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error getting workflow actions: {}", e);
                        std::process::exit(1);
                    }
                }
            }

            // Submit the workflow
            match apis::workflows_api::get_workflow(&config, workflow_id) {
                Ok(workflow) => {
                    let torc_config = TorcConfig::load().unwrap_or_default();
                    let workflow_manager =
                        WorkflowManager::new(config.clone(), torc_config, workflow);
                    match workflow_manager.start(
                        *ignore_missing_data,
                        *max_parallel_jobs,
                        output_dir,
                        *poll_interval,
                    ) {
                        Ok(()) => {
                            print_workflow_message(
                                &format,
                                workflow_id,
                                &format!("Successfully submitted workflow {}", workflow_id),
                            );
                        }
                        Err(e) => {
                            eprintln!("Error submitting workflow {}: {}", workflow_id, e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error getting workflow {}: {}", workflow_id, e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Watch {
            workflow_id,
            poll_interval,
            recover,
            max_retries,
            memory_multiplier,
            runtime_multiplier,
            retry_unknown,
            recovery_hook,
            output_dir,
            show_job_counts,
            auto_schedule,
            auto_schedule_threshold,
            auto_schedule_cooldown,
            auto_schedule_stranded_timeout,
            ai_recovery,
            ai_agent,
            partition,
            walltime,
        } => {
            let args = WatchArgs {
                workflow_id: *workflow_id,
                poll_interval: *poll_interval,
                recover: *recover,
                max_retries: *max_retries,
                memory_multiplier: *memory_multiplier,
                runtime_multiplier: *runtime_multiplier,
                retry_unknown: *retry_unknown,
                recovery_hook: recovery_hook.clone(),
                output_dir: output_dir.clone(),
                show_job_counts: *show_job_counts,
                log_level: log_level.clone(),
                auto_schedule: *auto_schedule,
                auto_schedule_threshold: *auto_schedule_threshold,
                auto_schedule_cooldown: *auto_schedule_cooldown,
                auto_schedule_stranded_timeout: *auto_schedule_stranded_timeout,
                ai_recovery: *ai_recovery,
                ai_agent: ai_agent.clone(),
                partition: partition.clone(),
                walltime: walltime.clone(),
            };
            run_watch(&config, &args);
        }
        Commands::Recover {
            workflow_id,
            output_dir,
            memory_multiplier,
            runtime_multiplier,
            retry_unknown,
            recovery_hook,
            dry_run,
            no_prompts,
            ai_recovery,
            ai_agent,
        } => {
            let interactive = !no_prompts && std::io::stdin().is_terminal();
            let args = RecoverArgs {
                workflow_id: *workflow_id,
                output_dir: output_dir.clone(),
                memory_multiplier: *memory_multiplier,
                runtime_multiplier: *runtime_multiplier,
                retry_unknown: *retry_unknown,
                recovery_hook: recovery_hook.clone(),
                dry_run: *dry_run,
                interactive,
                ai_recovery: *ai_recovery,
                ai_agent: ai_agent.clone(),
            };

            // For JSON output, get diagnosis data to include in the report
            let diagnosis = if format == "json" {
                diagnose_failures(&config, *workflow_id).ok()
            } else {
                None
            };

            match recover_workflow(&config, &args) {
                Ok(result) => {
                    if format == "json" {
                        // Output structured JSON report
                        let report = RecoveryReport {
                            workflow_id: *workflow_id,
                            dry_run: *dry_run,
                            memory_multiplier: *memory_multiplier,
                            runtime_multiplier: *runtime_multiplier,
                            result,
                            diagnosis,
                        };
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&report).unwrap_or_else(|e| {
                                format!("{{\"error\": \"Failed to serialize: {}\"}}", e)
                            })
                        );
                    } else if *dry_run {
                        println!("[DRY RUN] Summary for workflow {}", workflow_id);
                        if result.oom_fixed > 0 {
                            println!(
                                "  - {} job(s) would have memory increased",
                                result.oom_fixed
                            );
                        }
                        if result.timeout_fixed > 0 {
                            println!(
                                "  - {} job(s) would have runtime increased",
                                result.timeout_fixed
                            );
                        }
                        if result.unknown_retried > 0 {
                            println!(
                                "  - {} job(s) with unknown failures would be reset",
                                result.unknown_retried
                            );
                        }
                        if result.jobs_to_retry.is_empty() {
                            println!("No recoverable jobs found.");
                        } else {
                            println!(
                                "Would reset {} job(s) and regenerate Slurm schedulers.",
                                result.jobs_to_retry.len()
                            );
                        }
                        println!("\nRun without --dry-run to apply these changes.");
                    } else {
                        println!("Recovery complete for workflow {}", workflow_id);
                        if result.oom_fixed > 0 {
                            println!("  - {} job(s) had memory increased", result.oom_fixed);
                        }
                        if result.timeout_fixed > 0 {
                            println!("  - {} job(s) had runtime increased", result.timeout_fixed);
                        }
                        if result.unknown_retried > 0 {
                            println!(
                                "  - {} job(s) with unknown failures reset",
                                result.unknown_retried
                            );
                        }
                        if result.jobs_to_retry.is_empty() {
                            println!("No recoverable jobs found.");
                        } else {
                            println!(
                                "Reset {} job(s). Slurm schedulers regenerated and submitted.",
                                result.jobs_to_retry.len()
                            );
                        }
                    }
                }
                Err(e) => {
                    if format == "json" {
                        println!(
                            "{}",
                            serde_json::json!({
                                "error": e,
                                "workflow_id": workflow_id,
                                "dry_run": dry_run,
                            })
                        );
                        std::process::exit(1);
                    } else {
                        eprintln!("Recovery failed: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Commands::Cancel { workflow_id } => {
            handle_cancel(&config, workflow_id, &format);
        }
        Commands::Status { workflow_id } => {
            torc::client::commands::reports::generate_summary(&config, *workflow_id, &format);
        }
        Commands::Delete {
            workflow_ids,
            force,
        } => {
            torc::client::commands::workflows::handle_delete(
                &config,
                workflow_ids,
                *force,
                &format,
            );
        }
        Commands::Workflows { command } => {
            handle_workflow_commands(&config, command, &format);
        }
        Commands::ComputeNodes { command } => {
            handle_compute_node_commands(&config, command, &format);
        }
        Commands::Files { command } => {
            handle_file_commands(&config, command, &format);
        }
        Commands::Jobs { command } => {
            handle_job_commands(&config, command, &format);
        }
        Commands::JobDependencies { command } => {
            handle_job_dependency_commands(command, &config, &format);
        }
        Commands::ResourceRequirements { command } => {
            handle_resource_requirements_commands(&config, command, &format);
        }
        Commands::FailureHandlers { command } => {
            handle_failure_handler_commands(&config, command, &format);
        }
        Commands::RoCrate { command } => {
            handle_ro_crate_commands(&config, command, &format);
        }
        Commands::Events { command } => {
            handle_event_commands(&config, command, &format);
        }
        Commands::Results { command } => {
            handle_result_commands(&config, command, &format);
        }
        Commands::Tasks { command } => {
            handle_tasks_commands(&config, command, &format);
        }
        Commands::UserData { command } => {
            handle_user_data_commands(&config, command, &format);
        }
        Commands::Slurm { command } => {
            handle_slurm_commands(&config, command, &format);
        }
        Commands::Remote { command } => {
            handle_remote_commands(&config, command);
        }
        Commands::ScheduledComputeNodes { command } => {
            handle_scheduled_compute_node_commands(&config, command, &format);
        }
        Commands::Hpc { command } => {
            handle_hpc_commands(command, &format);
        }
        Commands::Logs { command } => {
            handle_log_commands(&config, command);
        }
        Commands::AccessGroups { command } => {
            handle_access_group_commands(&config, command, &format);
        }
        Commands::Admin { command } => {
            handle_admin_commands(&config, command, &format);
        }
        Commands::Config { command } => {
            handle_config_commands(command);
        }
        Commands::Tui(args) => {
            let basic_auth = config.basic_auth.clone();
            if let Err(e) = tui_runner::run(args, basic_auth) {
                eprintln!("Error running TUI: {}", e);
                std::process::exit(1);
            }
        }
        Commands::PlotResources(args) => {
            if let Err(e) = plot_resources_cmd::run(args) {
                eprintln!("Error generating plots: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Ping => match apis::system_api::ping(&config) {
            Ok(_) => {
                if cli.format == "json" {
                    println!(r#"{{"status": "Server is running"}}"#);
                } else {
                    println!("Server is running");
                }
            }
            Err(e) => {
                if cli.format == "json" {
                    println!(
                        r#"{{"status": "error", "message": "{}"}}"#,
                        e.to_string().replace('"', "\\\"")
                    );
                } else {
                    eprintln!("Failed to connect to server: {}", e);
                }
                std::process::exit(1);
            }
        },
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(*shell, &mut cmd, "torc", &mut std::io::stdout());
        }
    }
}
