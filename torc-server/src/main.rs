#![allow(missing_docs)]

use anyhow::Result;
use clap::{Args, Parser, builder::styling};
use dotenvy::dotenv;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use std::env;
use std::path::PathBuf;
use std::str::FromStr;
use torc::config::TorcConfig;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tracing_timing::{Builder, Histogram};

mod logging;
mod server;
mod service;

/// Server configuration options shared between `run` and `service install`
#[derive(Args, Clone, Default)]
struct ServerConfig {
    /// Log level (error, warn, info, debug, trace)
    #[arg(short, long, default_value = "info", env = "RUST_LOG")]
    log_level: String,

    /// Whether to use HTTPS or not
    #[arg(long)]
    https: bool,

    /// Hostname or IP address to bind the server to
    #[arg(short = 'H', long, default_value = "0.0.0.0")]
    host: String,

    /// Defines the port to listen on
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// Defines the number of threads to use
    #[arg(short, long, default_value_t = 1)]
    threads: u32,

    /// Path to the SQLite database file. If not specified, uses DATABASE_URL environment variable
    #[arg(short, long)]
    database: Option<String>,

    /// Path to htpasswd file for basic authentication (username:bcrypt_hash format, one per line)
    #[arg(long, env = "TORC_AUTH_FILE")]
    auth_file: Option<String>,

    /// Require authentication for all requests (if false, auth is optional for backward compatibility)
    #[arg(long, default_value_t = false)]
    require_auth: bool,

    /// TTL in seconds for credential cache (0 to disable). Caching avoids repeated bcrypt
    /// verification (~250ms each at cost 12) for the same credentials within the TTL window.
    #[arg(long, default_value_t = 60, env = "TORC_CREDENTIAL_CACHE_TTL_SECS")]
    credential_cache_ttl_secs: u64,

    /// Enforce access control based on workflow ownership and group membership.
    /// When enabled, users can only access workflows they own or have group access to.
    #[arg(long, default_value_t = false)]
    enforce_access_control: bool,

    /// Directory for log files (enables file logging with size-based rotation)
    #[arg(long, env = "TORC_LOG_DIR")]
    log_dir: Option<PathBuf>,

    /// Use JSON format for log files (useful for log aggregation systems)
    #[arg(long, default_value_t = false)]
    json_logs: bool,

    /// Run as daemon (Unix/Linux only)
    #[arg(long, default_value_t = false)]
    daemon: bool,

    /// PID file location (Unix only, used when running as daemon)
    #[arg(long, default_value = "/var/run/torc-server.pid")]
    pid_file: PathBuf,

    /// Interval in seconds for background task that processes job completions and unblocks dependent jobs.
    /// Defaults to 60s for `run`, 5s for `service install`
    #[arg(short, long, env = "TORC_COMPLETION_CHECK_INTERVAL_SECS")]
    completion_check_interval_secs: Option<f64>,

    /// Users to add to the admin group (can be specified multiple times).
    /// These users can create and manage access groups.
    #[arg(long = "admin-user", env = "TORC_ADMIN_USERS")]
    admin_users: Vec<String>,
}

const STYLES: styling::Styles = styling::Styles::styled()
    .header(styling::AnsiColor::Green.on_default().bold())
    .usage(styling::AnsiColor::Green.on_default().bold())
    .literal(styling::AnsiColor::Cyan.on_default().bold())
    .placeholder(styling::AnsiColor::Cyan.on_default());

#[derive(Parser)]
#[command(name = "torc-server")]
#[command(about = "Torc workflow orchestration server")]
#[command(styles = STYLES)]
#[command(
    after_help = "Use 'torc-server run --help' to see server configuration options.\n\
Use 'torc-server service --help' to see service management options."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Run the server (default if no subcommand specified)
    Run {
        #[command(flatten)]
        config: ServerConfig,
    },
    /// Manage system service (install, uninstall, start, stop, status)
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
}

#[derive(clap::Subcommand)]
#[allow(clippy::large_enum_variant)]
enum ServiceAction {
    /// Install the server as a system service
    Install {
        /// Install as user service (no root required)
        #[arg(long)]
        user: bool,

        #[command(flatten)]
        config: ServerConfig,
    },
    /// Uninstall the system service
    Uninstall {
        /// Uninstall user service
        #[arg(long)]
        user: bool,
    },
    /// Start the system service
    Start {
        /// Start user service
        #[arg(long)]
        user: bool,
    },
    /// Stop the system service
    Stop {
        /// Stop user service
        #[arg(long)]
        user: bool,
    },
    /// Check the status of the system service
    Status {
        /// Check user service status
        #[arg(long)]
        user: bool,
    },
}

/// Daemonize the process (Unix only)
#[cfg(unix)]
fn daemonize_process(pid_file: &std::path::Path) -> Result<()> {
    use daemonize::Daemonize;

    let daemonize = Daemonize::new()
        .pid_file(pid_file)
        .working_directory(env::current_dir()?)
        .umask(0o027);

    daemonize
        .start()
        .map_err(|e| anyhow::anyhow!("Failed to daemonize: {}", e))?;

    Ok(())
}

#[cfg(not(unix))]
fn daemonize_process(_pid_file: &std::path::Path) -> Result<()> {
    anyhow::bail!("Daemon mode is only supported on Unix/Linux systems");
}

/// Default completion check interval for `run` command (30 seconds)
const DEFAULT_RUN_INTERVAL_SECS: f64 = 30.0;

/// Create custom server, wire it to the autogenerated router,
/// and pass it to the web server.
fn main() -> Result<()> {
    dotenv().ok();

    let cli = Cli::parse();

    // Handle commands - default to Run with default config if no subcommand
    match cli.command {
        Some(Commands::Service { action }) => handle_service_action(action),
        Some(Commands::Run { config }) => run_server(config),
        None => {
            // Default: run server with default config
            // We need to re-parse as "run" to get ServerConfig defaults from clap
            let cli = Cli::parse_from(["torc-server", "run"]);
            if let Some(Commands::Run { config }) = cli.command {
                run_server(config)
            } else {
                unreachable!()
            }
        }
    }
}

fn handle_service_action(action: ServiceAction) -> Result<()> {
    let (command, user_level, config) = match action {
        ServiceAction::Install { user, config } => {
            let svc_config = service::ServiceConfig {
                log_dir: config.log_dir,
                database: config.database,
                host: config.host,
                port: config.port,
                threads: config.threads,
                auth_file: config.auth_file,
                require_auth: config.require_auth,
                log_level: config.log_level,
                json_logs: config.json_logs,
                completion_check_interval_secs: config.completion_check_interval_secs,
            };
            (service::ServiceCommand::Install, user, Some(svc_config))
        }
        ServiceAction::Uninstall { user } => (service::ServiceCommand::Uninstall, user, None),
        ServiceAction::Start { user } => (service::ServiceCommand::Start, user, None),
        ServiceAction::Stop { user } => (service::ServiceCommand::Stop, user, None),
        ServiceAction::Status { user } => (service::ServiceCommand::Status, user, None),
    };

    service::execute_service_command(command, config.as_ref(), user_level)
}

fn run_server(cli_config: ServerConfig) -> Result<()> {
    // Load configuration from files and merge with CLI arguments
    // CLI arguments take precedence over file config
    let file_config = TorcConfig::load().unwrap_or_default();
    let server_file_config = &file_config.server;

    // Merge CLI config with file config (CLI takes precedence for non-default values)
    let config = ServerConfig {
        log_level: if cli_config.log_level != "info" {
            cli_config.log_level
        } else {
            server_file_config.log_level.clone()
        },
        https: cli_config.https || server_file_config.https,
        host: if cli_config.host != "0.0.0.0" {
            cli_config.host
        } else {
            server_file_config.host.clone()
        },
        port: if cli_config.port != 8080 {
            cli_config.port
        } else {
            server_file_config.port
        },
        threads: if cli_config.threads != 1 {
            cli_config.threads
        } else {
            server_file_config.threads
        },
        database: cli_config
            .database
            .or_else(|| server_file_config.database.clone()),
        auth_file: cli_config
            .auth_file
            .or_else(|| server_file_config.auth_file.clone()),
        require_auth: cli_config.require_auth || server_file_config.require_auth,
        credential_cache_ttl_secs: if cli_config.credential_cache_ttl_secs != 60 {
            cli_config.credential_cache_ttl_secs
        } else {
            server_file_config.credential_cache_ttl_secs
        },
        enforce_access_control: cli_config.enforce_access_control
            || server_file_config.enforce_access_control,
        log_dir: cli_config
            .log_dir
            .or_else(|| server_file_config.logging.log_dir.clone()),
        json_logs: cli_config.json_logs || server_file_config.logging.json_logs,
        daemon: cli_config.daemon,
        pid_file: cli_config.pid_file,
        completion_check_interval_secs: cli_config
            .completion_check_interval_secs
            .or(Some(server_file_config.completion_check_interval_secs)),
        admin_users: cli_config.admin_users,
    };

    // Handle daemonization BEFORE initializing logging
    // This is important because daemonization forks the process
    if config.daemon {
        daemonize_process(&config.pid_file)?;
    }

    // Check if timing instrumentation should be enabled
    let timing_enabled = env::var("TORC_TIMING_ENABLED")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false);

    // Initialize logging with file rotation support
    if timing_enabled {
        // When timing is enabled, we need to set up tracing manually to include the timing layer
        // Set up tracing with timing layer
        let timing_layer = Builder::default()
            .no_span_recursion()
            .layer(|| Histogram::new_with_max(60_000_000_000, 2).unwrap());

        let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| config.log_level.clone().into());

        if let Some(ref log_dir) = config.log_dir {
            // File logging with timing (size-based rotation: 10 MiB, 5 files)
            let file_writer = logging::create_rotating_writer(log_dir)?;
            let (non_blocking, _guard) = tracing_appender::non_blocking(file_writer);
            std::mem::forget(_guard); // Keep guard alive

            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_target(true)
                        .with_level(true)
                        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE),
                )
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_writer(non_blocking)
                        .with_ansi(false)
                        .with_target(true)
                        .with_level(true)
                        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE),
                )
                .with(env_filter)
                .with(timing_layer)
                .init();
        } else {
            // Console only with timing
            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_target(true)
                        .with_level(true)
                        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE),
                )
                .with(env_filter)
                .with(timing_layer)
                .init();
        }

        info!("Timing instrumentation enabled - timing data is being collected");
        if config.log_dir.is_some() {
            info!(
                "File logging configured with size-based rotation (10 MiB per file, 5 files max)"
            );
        }
        info!(
            "Use external tools like tokio-console or OpenTelemetry exporters to view timing data"
        );
    } else {
        // Use the new logging module for standard (non-timing) logging
        logging::init_logging(
            config.log_dir.as_deref(),
            &config.log_level,
            config.json_logs,
        )?;
    }

    // Use database path from command line if provided, otherwise fall back to DATABASE_URL env var
    let database_url = if let Some(db_path) = &config.database {
        format!("sqlite:{}", db_path)
    } else {
        env::var("DATABASE_URL").expect("DATABASE_URL must be set or --database must be provided")
    };

    // Build Tokio runtime with user-specified thread count
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(config.threads as usize)
        .enable_all()
        .build()?;

    runtime.block_on(async {
        // Configure SQLite connection with WAL journal mode for better concurrency
        // and foreign key constraints enabled
        let connect_options = SqliteConnectOptions::from_str(&database_url)?
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .create_if_missing(true)
            .busy_timeout(std::time::Duration::from_secs(45));

        let pool = SqlitePoolOptions::new()
            .connect_with(connect_options)
            .await?;

        let version = env!("CARGO_PKG_VERSION");
        let git_hash = env!("GIT_HASH");
        let git_dirty = env!("GIT_DIRTY");
        info!(
            "Starting torc-server version={} ({}{})",
            version, git_hash, git_dirty
        );
        info!("Connected to database: {}", database_url);
        info!("Database configured with WAL journal mode and foreign key constraints");

        // Run embedded migrations
        info!("Running database migrations...");
        sqlx::migrate!("../migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");
        info!("Database migrations completed successfully");

        // Load htpasswd file if provided
        let htpasswd = if let Some(auth_file_path) = &config.auth_file {
            info!("Loading htpasswd file from: {}", auth_file_path);
            match torc::server::htpasswd::HtpasswdFile::load(auth_file_path) {
                Ok(htpasswd) => {
                    info!("Loaded {} users from htpasswd file", htpasswd.user_count());
                    Some(htpasswd)
                }
                Err(e) => {
                    eprintln!("Error loading htpasswd file: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            if config.require_auth {
                eprintln!("Error: --require-auth specified but no --auth-file provided");
                std::process::exit(1);
            }
            info!("No htpasswd file configured, authentication disabled");
            None
        };

        if config.require_auth {
            info!("Authentication is REQUIRED for all requests");
        } else if htpasswd.is_some() {
            info!("Authentication is OPTIONAL (backward compatible mode)");
        }

        let addr = format!("{}:{}", config.host, config.port);
        info!(
            "Tokio runtime configured with {} worker threads",
            config.threads
        );
        info!("Listening on {}", addr);

        // Default to 60s for `run` command
        let completion_check_interval_secs = config
            .completion_check_interval_secs
            .unwrap_or(DEFAULT_RUN_INTERVAL_SECS);

        if config.enforce_access_control {
            info!("Access control is ENABLED - users can only access their own workflows and workflows shared via access groups");
        }

        // Get admin users from config and CLI (merge, with CLI taking precedence)
        let mut admin_users = server_file_config.admin_users.clone();
        for user in &config.admin_users {
            if !admin_users.contains(user) {
                admin_users.push(user.clone());
            }
        }
        if !admin_users.is_empty() {
            info!("Admin users configured: {:?}", admin_users);
        }

        server::create(
            &addr,
            config.https,
            pool,
            htpasswd,
            config.require_auth,
            config.credential_cache_ttl_secs,
            config.enforce_access_control,
            completion_check_interval_secs,
            admin_users,
        )
        .await;
        Ok(())
    })
}
