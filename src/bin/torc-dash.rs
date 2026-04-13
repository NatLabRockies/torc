//! Torc Dashboard - Web UI with CLI integration
//!
//! This binary provides a web dashboard that:
//! - Serves embedded static files (HTML/CSS/JS)
//! - Proxies API requests to a remote torc-server
//! - Executes torc CLI commands locally (for workflow creation, running, submitting)

use anyhow::Result;
use axum::{
    Json, Router,
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, Request, StatusCode, header},
    response::{
        Html, IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use clap::Parser;
use rmcp::{ServiceExt, model::CallToolRequestParams, transport::child_process::TokioChildProcess};
use rust_embed::Embed;
use serde::{Deserialize, Serialize};
use std::path::Path as FsPath;
use std::process::Command as StdCommand;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock};
use torc::config::TorcConfig;
use torc::network_utils::find_available_port;
use tracing::{debug, error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Embedded static assets for the dashboard
#[derive(Embed)]
#[folder = "torc-dash/static/"]
struct Assets;

/// LLM provider type for AI chat
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum LlmProvider {
    /// Anthropic Claude API (direct or via Foundry)
    #[default]
    Anthropic,
    /// OpenAI API (GPT models)
    OpenAI,
    /// Ollama (local, OpenAI-compatible)
    Ollama,
    /// GitHub Models (Azure inference, OpenAI-compatible)
    GitHub,
}

/// Managed server process state
#[derive(Default)]
struct ManagedServer {
    /// Process ID if server is running
    pid: Option<u32>,
    /// Port the server is running on
    port: Option<u16>,
    /// Recent output lines from the server
    output_lines: Vec<String>,
}

/// MCP client connection to torc-mcp-server subprocess
struct McpClient {
    /// The peer handle for sending requests to the MCP server
    peer: rmcp::service::Peer<rmcp::service::RoleClient>,
    /// Cached tool definitions from the MCP server
    tools: Vec<rmcp::model::Tool>,
}

/// Application state shared across handlers
struct AppState {
    /// URL of the torc-server API
    api_url: String,
    /// HTTP client for proxying requests
    client: reqwest::Client,
    /// Path to the torc CLI binary (defaults to "torc" in PATH)
    torc_bin: String,
    /// Path to the torc-server binary
    torc_server_bin: String,
    /// Path to the torc-mcp-server binary
    torc_mcp_server_bin: String,
    /// Managed server process (if started by torc-dash)
    managed_server: Mutex<ManagedServer>,
    /// LLM provider type (Anthropic, Ollama, GitHub)
    llm_provider: RwLock<LlmProvider>,
    /// API key for AI chat (Anthropic key, GitHub token, or "ollama" for local)
    llm_api_key: RwLock<Option<String>>,
    /// Base URL for LLM API
    llm_base_url: RwLock<String>,
    /// Auth header name (e.g., "x-api-key", "Authorization")
    llm_auth_header: RwLock<String>,
    /// Model to use for AI chat (can be changed at runtime)
    llm_model: RwLock<String>,
    /// MCP client connection (lazily initialized)
    mcp_client: Mutex<Option<McpClient>>,
}

/// CLI arguments
#[derive(Parser)]
#[command(name = "torc-dash")]
#[command(about = "Torc workflow dashboard with CLI integration")]
struct Cli {
    /// Port to listen on
    #[arg(short, long, default_value_t = 8090)]
    port: u16,

    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// URL of the torc-server API
    #[arg(
        short,
        long,
        default_value = "http://localhost:8080/torc-service/v1",
        env = "TORC_API_URL"
    )]
    api_url: String,

    /// Path to torc CLI binary
    #[arg(long, default_value = "torc", env = "TORC_BIN")]
    torc_bin: String,

    /// Path to torc-server binary
    #[arg(long, default_value = "torc-server", env = "TORC_SERVER_BIN")]
    torc_server_bin: String,

    /// Run in standalone mode: automatically start torc-server alongside the dashboard
    #[arg(long)]
    standalone: bool,

    /// Port for torc-server when running in standalone mode (0 = auto-detect free port)
    #[arg(long, default_value_t = 0)]
    server_port: u16,

    /// Host for torc-server to bind to in standalone mode (default: 0.0.0.0 for external access)
    #[arg(long, default_value = "0.0.0.0")]
    server_host: String,

    /// Database path for torc-server when running in standalone mode
    #[arg(long, env = "DATABASE_URL")]
    database: Option<String>,

    /// Completion check interval (seconds) for torc-server in standalone mode
    #[arg(long, default_value_t = 5)]
    completion_check_interval_secs: u32,

    /// Listen on a UNIX domain socket instead of TCP (more secure on shared hosts).
    /// The socket file is created with owner-only permissions (0600).
    #[cfg(unix)]
    #[arg(long, value_name = "PATH")]
    socket: Option<std::path::PathBuf>,

    /// Path to a PEM-encoded CA certificate to trust for TLS connections
    #[arg(long, env = "TORC_TLS_CA_CERT")]
    tls_ca_cert: Option<String>,

    /// Skip TLS certificate verification (for testing only)
    #[arg(long, env = "TORC_TLS_INSECURE")]
    tls_insecure: bool,

    /// Anthropic API key for AI chat feature (direct API access)
    #[arg(long, env = "ANTHROPIC_API_KEY")]
    anthropic_api_key: Option<String>,

    /// Anthropic API key for AI chat via Microsoft Azure AI Foundry
    #[arg(long, env = "ANTHROPIC_FOUNDRY_API_KEY")]
    anthropic_foundry_api_key: Option<String>,

    /// Azure AI Foundry resource name (e.g. "my-resource").
    /// Constructs the base URL: https://{resource}.services.ai.azure.com/anthropic/v1
    #[arg(long, env = "ANTHROPIC_FOUNDRY_RESOURCE")]
    anthropic_foundry_resource: Option<String>,

    /// Override the Claude API base URL (e.g. "https://my-proxy.example.com/v1").
    /// The /messages path is appended automatically. Overrides any auto-detected URL.
    #[arg(long, env = "ANTHROPIC_BASE_URL")]
    anthropic_base_url: Option<String>,

    /// Override the auth header name (default: "x-api-key")
    #[arg(long, env = "ANTHROPIC_AUTH_HEADER")]
    anthropic_auth_header: Option<String>,

    /// Model to use for AI chat (default: claude-sonnet-4-20250514)
    #[arg(
        long,
        default_value = "claude-sonnet-4-20250514",
        env = "ANTHROPIC_MODEL"
    )]
    anthropic_model: String,

    /// Ollama base URL for local AI chat (OpenAI-compatible API)
    #[arg(
        long,
        default_value = "http://localhost:11434/v1",
        env = "OLLAMA_BASE_URL"
    )]
    ollama_base_url: String,

    /// Ollama model name (e.g. "llama3.2", "qwen3.5:35b-a3b")
    #[arg(long, default_value = "llama3.2", env = "OLLAMA_MODEL")]
    ollama_model: String,

    /// GitHub token for GitHub Models (requires "models:read" scope)
    #[arg(long, env = "GITHUB_TOKEN")]
    github_token: Option<String>,

    /// GitHub Models base URL (Azure ML inference endpoint)
    #[arg(
        long,
        default_value = "https://models.inference.ai.azure.com",
        env = "GITHUB_MODELS_BASE_URL"
    )]
    github_models_base_url: String,

    /// GitHub Models model name (e.g. "gpt-4o", "Meta-Llama-3.1-70B-Instruct")
    #[arg(long, default_value = "gpt-4o", env = "GITHUB_MODELS_MODEL")]
    github_models_model: String,

    /// OpenAI API key
    #[arg(long, env = "OPENAI_API_KEY")]
    openai_api_key: Option<String>,

    /// OpenAI API base URL (for API-compatible services)
    #[arg(
        long,
        default_value = "https://api.openai.com/v1",
        env = "OPENAI_BASE_URL"
    )]
    openai_base_url: String,

    /// OpenAI model name (e.g. "gpt-4o", "gpt-4o-mini", "o1")
    #[arg(long, default_value = "gpt-4o", env = "OPENAI_MODEL")]
    openai_model: String,

    /// LLM provider to use: "anthropic", "openai", "ollama", or "github"
    #[arg(long, default_value = "anthropic", env = "LLM_PROVIDER")]
    llm_provider: String,

    /// Path to torc-mcp-server binary
    #[arg(long, default_value = "torc-mcp-server", env = "TORC_MCP_SERVER_BIN")]
    torc_mcp_server_bin: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("torc_dash=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();

    // Load configuration from files and merge with CLI arguments
    let file_config = TorcConfig::load().unwrap_or_default();
    let dash_config = &file_config.dash;

    // Merge CLI config with file config (CLI takes precedence for non-default values)
    let host = if cli.host != "127.0.0.1" {
        cli.host.clone()
    } else {
        dash_config.host.clone()
    };
    let port = if cli.port != 8090 {
        cli.port
    } else {
        dash_config.port
    };
    let api_url = if cli.api_url != "http://localhost:8080/torc-service/v1" {
        cli.api_url.clone()
    } else {
        dash_config.api_url.clone()
    };
    let torc_bin = if cli.torc_bin != "torc" {
        cli.torc_bin.clone()
    } else {
        dash_config.torc_bin.clone()
    };
    let torc_server_bin = if cli.torc_server_bin != "torc-server" {
        cli.torc_server_bin.clone()
    } else {
        dash_config.torc_server_bin.clone()
    };
    let standalone = cli.standalone || dash_config.standalone;
    let server_port = if cli.server_port != 0 {
        cli.server_port
    } else {
        dash_config.server_port
    };
    let server_host = if cli.server_host != "0.0.0.0" {
        cli.server_host.clone()
    } else {
        dash_config.server_host.clone()
    };
    let database = cli
        .database
        .clone()
        .or_else(|| dash_config.database.clone());
    let completion_check_interval_secs = if cli.completion_check_interval_secs != 5 {
        cli.completion_check_interval_secs
    } else {
        dash_config.completion_check_interval_secs
    };

    #[cfg(unix)]
    let socket_path = cli
        .socket
        .clone()
        .or_else(|| dash_config.socket.as_ref().map(std::path::PathBuf::from));

    #[cfg(unix)]
    if let Some(ref socket_path) = socket_path {
        info!(
            "Starting torc-dash v{} ({}) on unix:{} torc_bin={} server_bin={}",
            env!("CARGO_PKG_VERSION"),
            env!("GIT_HASH"),
            socket_path.display(),
            torc_bin,
            torc_server_bin
        );
    } else {
        info!(
            "Starting torc-dash v{} ({}) on {}:{} torc_bin={} server_bin={}",
            env!("CARGO_PKG_VERSION"),
            env!("GIT_HASH"),
            host,
            port,
            torc_bin,
            torc_server_bin
        );
    }
    #[cfg(not(unix))]
    info!(
        "Starting torc-dash v{} ({}) on {}:{} torc_bin={} server_bin={}",
        env!("CARGO_PKG_VERSION"),
        env!("GIT_HASH"),
        host,
        port,
        torc_bin,
        torc_server_bin
    );

    // Track the actual server port (may differ from server_port if using port 0)
    let mut actual_server_port = server_port;

    // In standalone mode, start the server first to get the actual port
    let managed_server = if standalone {
        // Warn if --api-url is specified with --standalone (it will be ignored)
        if api_url != "http://localhost:8080/torc-service/v1" {
            warn!(
                "--api-url is ignored in standalone mode. Use --server-host and --server-port to configure the managed server."
            );
        }

        info!(
            "Standalone mode: starting torc-server on {}:{} (port 0 = auto-detect)",
            server_host, server_port
        );

        let mut args = vec![
            "run".to_string(),
            "--host".to_string(),
            server_host.clone(),
            "--port".to_string(),
            server_port.to_string(),
            "--completion-check-interval-secs".to_string(),
            completion_check_interval_secs.to_string(),
        ];

        if let Some(ref db) = database {
            args.push("--database".to_string());
            args.push(db.clone());
        }

        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        match Command::new(&torc_server_bin)
            .args(&args_refs)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(mut child) => {
                let pid = child.id();
                info!("Started torc-server with PID {:?}", pid);
                let mut port_reported = false;

                // Read stdout to find the actual port
                if let Some(stdout) = child.stdout.take() {
                    let mut reader = BufReader::new(stdout);
                    let mut line = String::new();

                    // Read lines until we find TORC_SERVER_PORT or timeout
                    let timeout = tokio::time::Duration::from_secs(10);
                    let start = std::time::Instant::now();

                    loop {
                        if start.elapsed() > timeout {
                            error!("Timeout waiting for server to report port");
                            break;
                        }

                        match tokio::time::timeout(
                            tokio::time::Duration::from_millis(100),
                            reader.read_line(&mut line),
                        )
                        .await
                        {
                            Ok(Ok(0)) => break, // EOF
                            Ok(Ok(_)) => {
                                // Check for the port line
                                if let Some(port_str) = line.strip_prefix("TORC_SERVER_PORT=")
                                    && let Ok(port) = port_str.trim().parse::<u16>()
                                {
                                    actual_server_port = port;
                                    port_reported = true;
                                    info!("Server reported actual port: {}", actual_server_port);
                                    break;
                                }
                                line.clear();
                            }
                            Ok(Err(e)) => {
                                error!("Error reading server output: {}", e);
                                break;
                            }
                            Err(_) => {
                                // Timeout on this read, continue
                                continue;
                            }
                        }
                    }
                }

                if server_port == 0 && !port_reported {
                    let mut stderr_output = String::new();
                    if let Some(mut stderr) = child.stderr.take() {
                        let _ = stderr.read_to_string(&mut stderr_output).await;
                    }

                    if let Ok(Some(status)) = child.try_wait() {
                        let stderr_output = stderr_output.trim();
                        let details = if stderr_output.is_empty() {
                            format!("torc-server exited with status {}", status)
                        } else {
                            format!(
                                "torc-server exited with status {}: {}",
                                status, stderr_output
                            )
                        };
                        return Err(anyhow::anyhow!(
                            "Managed server failed to start before reporting a port: {}",
                            details
                        ));
                    }

                    return Err(anyhow::anyhow!(
                        "Managed server did not report an assigned port within 10 seconds"
                    ));
                }

                ManagedServer {
                    pid,
                    port: Some(actual_server_port),
                    output_lines: vec![format!(
                        "Server started with PID {} on port {}",
                        pid.unwrap_or(0),
                        actual_server_port
                    )],
                }
            }
            Err(e) => {
                error!("Failed to start torc-server: {}", e);
                error!("Make sure torc-server is in your PATH or specify --torc-server-bin");
                return Err(anyhow::anyhow!("Failed to start torc-server: {}", e));
            }
        }
    } else {
        ManagedServer::default()
    };

    // Determine API URL
    // - Standalone mode: dashboard connects to the managed server
    // - Non-standalone: use the user-provided --api-url
    let final_api_url = if standalone {
        // In standalone mode, determine the connect host based on server_host:
        // - If binding to all interfaces (0.0.0.0 or ::), connect via localhost
        // - If binding to a specific IP, connect via that IP
        let connect_host = if server_host == "0.0.0.0" || server_host == "::" {
            "localhost".to_string()
        } else {
            server_host.clone()
        };
        format!(
            "http://{}:{}/torc-service/v1",
            connect_host, actual_server_port
        )
    } else {
        api_url.clone()
    };
    info!("API URL: {}", final_api_url);

    // Build HTTP client with TLS settings
    let tls = torc::client::apis::configuration::TlsConfig {
        ca_cert_path: cli.tls_ca_cert.as_ref().map(std::path::PathBuf::from),
        insecure: cli.tls_insecure,
    };
    let http_client = tls
        .build_async_client()
        .expect("Failed to build HTTP client with TLS config");

    // Resolve LLM provider configuration based on --llm-provider or available credentials
    let (llm_provider, llm_api_key, llm_base_url, llm_auth_header, llm_model) =
        match cli.llm_provider.to_lowercase().as_str() {
            "ollama" => {
                info!("AI Chat: using Ollama ({})", cli.ollama_base_url);
                (
                    LlmProvider::Ollama,
                    Some("ollama".to_string()), // Ollama doesn't need auth
                    cli.ollama_base_url.clone(),
                    "Authorization".to_string(),
                    cli.ollama_model.clone(),
                )
            }
            "github" => {
                if let Some(ref token) = cli.github_token {
                    info!("AI Chat: using GitHub Models");
                    (
                        LlmProvider::GitHub,
                        Some(token.clone()),
                        cli.github_models_base_url.clone(),
                        "Authorization".to_string(),
                        cli.github_models_model.clone(),
                    )
                } else {
                    info!("AI Chat: GitHub Models selected but no GITHUB_TOKEN configured");
                    (
                        LlmProvider::GitHub,
                        None,
                        cli.github_models_base_url.clone(),
                        "Authorization".to_string(),
                        cli.github_models_model.clone(),
                    )
                }
            }
            "openai" => {
                if let Some(ref api_key) = cli.openai_api_key {
                    info!("AI Chat: using OpenAI API");
                    (
                        LlmProvider::OpenAI,
                        Some(api_key.clone()),
                        cli.openai_base_url.clone(),
                        "Authorization".to_string(),
                        cli.openai_model.clone(),
                    )
                } else {
                    info!("AI Chat: OpenAI selected but no OPENAI_API_KEY configured");
                    (
                        LlmProvider::OpenAI,
                        None,
                        cli.openai_base_url.clone(),
                        "Authorization".to_string(),
                        cli.openai_model.clone(),
                    )
                }
            }
            _ => {
                // Default to Anthropic: Foundry takes precedence over direct API
                if let (Some(foundry_key), Some(foundry_resource)) = (
                    cli.anthropic_foundry_api_key.as_ref(),
                    cli.anthropic_foundry_resource.as_ref(),
                ) {
                    info!(
                        "AI Chat: using Azure AI Foundry (resource={})",
                        foundry_resource
                    );
                    let mut base_url = format!(
                        "https://{}.services.ai.azure.com/anthropic/v1",
                        foundry_resource
                    );
                    if let Some(ref url_override) = cli.anthropic_base_url {
                        base_url = url_override.clone();
                    }
                    let auth_header = cli
                        .anthropic_auth_header
                        .clone()
                        .unwrap_or_else(|| "x-api-key".to_string());
                    (
                        LlmProvider::Anthropic,
                        Some(foundry_key.clone()),
                        base_url,
                        auth_header,
                        cli.anthropic_model.clone(),
                    )
                } else if let Some(ref api_key) = cli.anthropic_api_key {
                    info!("AI Chat: using direct Anthropic API");
                    let base_url = cli
                        .anthropic_base_url
                        .clone()
                        .unwrap_or_else(|| "https://api.anthropic.com/v1".to_string());
                    let auth_header = cli
                        .anthropic_auth_header
                        .clone()
                        .unwrap_or_else(|| "x-api-key".to_string());
                    (
                        LlmProvider::Anthropic,
                        Some(api_key.clone()),
                        base_url,
                        auth_header,
                        cli.anthropic_model.clone(),
                    )
                } else {
                    info!("AI Chat: disabled (no API key configured)");
                    (
                        LlmProvider::Anthropic,
                        None,
                        "https://api.anthropic.com/v1".to_string(),
                        "x-api-key".to_string(),
                        cli.anthropic_model.clone(),
                    )
                }
            }
        };

    let state = Arc::new(AppState {
        api_url: final_api_url,
        client: http_client,
        torc_bin,
        torc_server_bin: torc_server_bin.clone(),
        torc_mcp_server_bin: cli.torc_mcp_server_bin.clone(),
        managed_server: Mutex::new(managed_server),
        llm_provider: RwLock::new(llm_provider),
        llm_api_key: RwLock::new(llm_api_key),
        llm_base_url: RwLock::new(llm_base_url),
        llm_auth_header: RwLock::new(llm_auth_header),
        llm_model: RwLock::new(llm_model),
        mcp_client: Mutex::new(None),
    });

    // Build router
    let app = Router::new()
        // Static files and dashboard
        .route("/", get(index_handler))
        .route("/static/{*path}", get(static_handler))
        // CLI command endpoints
        .route("/api/cli/create", post(cli_create_handler))
        .route("/api/cli/create-slurm", post(cli_create_slurm_handler))
        .route("/api/cli/validate", post(cli_validate_handler))
        .route("/api/cli/run", post(cli_run_handler))
        .route("/api/cli/submit", post(cli_submit_handler))
        .route("/api/cli/initialize", post(cli_initialize_handler))
        .route(
            "/api/cli/check-initialize",
            post(cli_check_initialize_handler),
        )
        .route("/api/cli/delete", post(cli_delete_handler))
        .route("/api/cli/cancel", post(cli_cancel_handler))
        .route("/api/cli/reinitialize", post(cli_reinitialize_handler))
        .route("/api/cli/reset-status", post(cli_reset_status_handler))
        .route("/api/cli/execution-plan", post(cli_execution_plan_handler))
        .route("/api/cli/run-stream", get(cli_run_stream_handler))
        .route("/api/cli/recover", post(cli_recover_handler))
        .route("/api/cli/sync-status", post(cli_sync_status_handler))
        .route("/api/cli/export", post(cli_export_handler))
        .route("/api/cli/import", post(cli_import_handler))
        .route("/api/cli/read-file", post(cli_read_file_handler))
        .route("/api/cli/plot-resources", post(cli_plot_resources_handler))
        .route(
            "/api/cli/list-resource-dbs",
            post(cli_list_resource_dbs_handler),
        )
        // Slurm debugging endpoints
        .route(
            "/api/cli/slurm-parse-logs",
            post(cli_slurm_parse_logs_handler),
        )
        .route("/api/cli/slurm-sacct", post(cli_slurm_sacct_handler))
        // HPC profile detection (used for Slurm checkbox in create modal)
        .route("/api/cli/hpc-profiles", get(cli_hpc_profiles_handler))
        // Server management endpoints
        .route("/api/server/start", post(server_start_handler))
        .route("/api/server/stop", post(server_stop_handler))
        .route("/api/server/status", get(server_status_handler))
        // AI Chat endpoints
        .route("/api/chat", post(chat_handler))
        .route("/api/chat/status", get(chat_status_handler))
        .route("/api/chat/configure", post(configure_chat_handler))
        // Version endpoint
        .route("/api/version", get(version_handler))
        // User endpoint
        .route("/api/user", get(user_handler))
        // API proxy - catch all /torc-service/* requests
        .route(
            "/torc-service/{*path}",
            get(proxy_handler)
                .post(proxy_handler)
                .put(proxy_handler)
                .patch(proxy_handler)
                .delete(proxy_handler),
        )
        .with_state(state);

    // Bind to UNIX domain socket or TCP, depending on configuration
    #[cfg(unix)]
    if let Some(ref socket_path) = socket_path {
        // Remove stale socket file from a previous run
        if socket_path.exists() {
            std::fs::remove_file(socket_path)?;
        }

        let uds = tokio::net::UnixListener::bind(socket_path)?;

        // Restrict to owner-only (0600)
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600))?;

        info!("Dashboard available at unix:{}", socket_path.display());
        info!(
            "To connect via SSH tunnel:\n  ssh -L 8090:{} user@this-host\n  Then open http://localhost:8090",
            socket_path.display()
        );

        axum::serve(uds, app).await?;
        return Ok(());
    }

    // TCP path (default)
    let (std_listener, actual_port) = find_available_port(&host, port)?;
    info!("Dashboard available at http://{}:{}", host, actual_port);
    if actual_port != port {
        info!(
            "Note: Requested port {} was in use, using port {} instead",
            port, actual_port
        );
    }

    // Convert std listener to tokio listener for axum
    std_listener.set_nonblocking(true)?;
    let listener = tokio::net::TcpListener::from_std(std_listener)?;

    axum::serve(listener, app).await?;

    Ok(())
}

// ============== Static File Handlers ==============

async fn index_handler() -> impl IntoResponse {
    match Assets::get("index.html") {
        Some(content) => Html(content.data.into_owned()).into_response(),
        None => (StatusCode::NOT_FOUND, "index.html not found").into_response(),
    }
}

async fn static_handler(Path(path): Path<String>) -> impl IntoResponse {
    let path = path.trim_start_matches('/');

    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data.into_owned(),
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, format!("File not found: {}", path)).into_response(),
    }
}

// ============== API Proxy Handler ==============

async fn proxy_handler(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
) -> impl IntoResponse {
    let path = req.uri().path();
    let query = req
        .uri()
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();
    let method = req.method().clone();

    // Build the target URL - strip /torc-service/v1 prefix since api_url already contains it
    let target_path = path.strip_prefix("/torc-service/v1").unwrap_or(path);
    let target_url = format!("{}{}{}", state.api_url, target_path, query);

    // Build the proxied request
    let mut proxy_req = state.client.request(method, &target_url);

    // Copy headers
    for (name, value) in req.headers() {
        if name != header::HOST {
            proxy_req = proxy_req.header(name, value);
        }
    }

    // Get body
    let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to read request body: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                format!("Failed to read body: {}", e),
            )
                .into_response();
        }
    };

    if !body_bytes.is_empty() {
        proxy_req = proxy_req.body(body_bytes);
    }

    // Execute request
    match proxy_req.send().await {
        Ok(resp) => {
            let status = StatusCode::from_u16(resp.status().as_u16())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let headers = resp.headers().clone();

            let mut response = Response::builder().status(status);

            for (name, value) in headers.iter() {
                response = response.header(name, value);
            }

            // Stream the body instead of buffering it
            let stream = resp.bytes_stream();
            response
                .body(Body::from_stream(stream))
                .unwrap()
                .into_response()
        }
        Err(e) => {
            error!("Proxy request failed: {}", e);
            (StatusCode::BAD_GATEWAY, format!("Proxy error: {}", e)).into_response()
        }
    }
}

// ============== CLI Command Handlers ==============

/// Extract workflow ID from CLI output.
/// Tries JSON first ({"workflow_id": 123}), then falls back to text patterns
/// like "Created workflow 123" or "ID: 123".
fn extract_workflow_id(stdout: &str) -> Option<String> {
    // Try JSON parsing first
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(stdout)
        && let Some(id) = json.get("workflow_id")
    {
        return Some(id.to_string().trim_matches('"').to_string());
    }

    // Fall back to text pattern matching
    for line in stdout.lines() {
        if line.contains("Created workflow") {
            // Extract the number after "Created workflow"
            if let Some(id) = line.split_whitespace().last() {
                return Some(id.trim().to_string());
            }
        } else if let Some(pos) = line.find("ID:") {
            // Extract the ID after "ID:"
            let after = &line[pos + "ID:".len()..];
            if let Some(id) = after.split_whitespace().next() {
                return Some(id.to_string());
            }
        }
    }
    None
}

/// Validate and sanitize file extension to prevent path traversal attacks.
/// Returns the sanitized extension or None if invalid.
fn validate_file_extension(ext: &str) -> Option<&'static str> {
    // Allowlist of valid extensions - prevents path traversal via malicious extensions
    match ext {
        ".json" | "json" => Some(".json"),
        ".yaml" | "yaml" => Some(".yaml"),
        ".yml" | "yml" => Some(".yml"),
        ".kdl" | "kdl" => Some(".kdl"),
        ".json5" | "json5" => Some(".json5"),
        _ => None,
    }
}

#[derive(Deserialize)]
struct CreateRequest {
    /// Path to workflow spec file OR inline spec content
    spec: String,
    /// If true, spec is file path; if false, spec is inline content
    #[serde(default)]
    is_file: bool,
    /// File extension for inline content (e.g., ".yaml", ".kdl")
    /// Used to create temp file with correct extension for format detection
    #[serde(default)]
    file_extension: Option<String>,
}

#[derive(Deserialize)]
struct CreateSlurmRequest {
    /// Path to workflow spec file OR inline spec content
    spec: String,
    /// If true, spec is file path; if false, spec is inline content
    #[serde(default)]
    is_file: bool,
    /// File extension for inline content (e.g., ".yaml", ".kdl")
    #[serde(default)]
    file_extension: Option<String>,
    /// Slurm account name (required)
    account: String,
    /// HPC profile name (optional - auto-detected if not provided)
    #[serde(default)]
    profile: Option<String>,
}

#[derive(Deserialize)]
struct WorkflowIdRequest {
    workflow_id: String,
}

#[derive(Deserialize)]
struct InitializeRequest {
    workflow_id: String,
    #[serde(default)]
    force: bool,
}

#[derive(Deserialize)]
struct RecoverRequest {
    workflow_id: String,
    #[serde(default)]
    dry_run: bool,
    #[serde(default = "default_memory_multiplier")]
    memory_multiplier: f64,
    #[serde(default = "default_runtime_multiplier")]
    runtime_multiplier: f64,
    #[serde(default)]
    retry_unknown: bool,
    #[serde(default = "default_output_dir")]
    output_dir: String,
}

fn default_memory_multiplier() -> f64 {
    1.5
}

fn default_runtime_multiplier() -> f64 {
    1.5
}

fn default_output_dir() -> String {
    "torc_output".to_string()
}

#[derive(Deserialize)]
struct SyncStatusRequest {
    workflow_id: String,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Serialize)]
struct CliResponse {
    success: bool,
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
}

#[derive(Deserialize)]
struct ExportRequest {
    workflow_id: String,
    /// Output file path on the server (default: workflow_<id>.json in current dir)
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    include_results: bool,
    #[serde(default)]
    include_events: bool,
}

#[derive(Deserialize)]
struct ImportRequest {
    /// Server-side file path to import from (mutually exclusive with content)
    #[serde(default)]
    file_path: Option<String>,
    /// Inline JSON content uploaded from browser (mutually exclusive with file_path)
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    skip_results: bool,
    #[serde(default)]
    skip_events: bool,
}

#[derive(Deserialize)]
struct ReadFileRequest {
    path: String,
}

#[derive(Serialize)]
struct ReadFileResponse {
    success: bool,
    content: Option<String>,
    error: Option<String>,
    is_json: bool,
    exists: bool,
}

async fn cli_create_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateRequest>,
) -> impl IntoResponse {
    let spec_content = req.spec.clone();

    // Validate file extension to prevent path traversal attacks
    let raw_extension = req.file_extension.as_deref().unwrap_or(".json");
    let file_extension = match validate_file_extension(raw_extension) {
        Some(ext) => ext,
        None => {
            return Json(CliResponse {
                success: false,
                stdout: String::new(),
                stderr: format!(
                    "Invalid file extension '{}'. Allowed: .json, .yaml, .yml, .kdl, .json5",
                    raw_extension
                ),
                exit_code: None,
            });
        }
    };

    let result = if req.is_file {
        // Spec is a file path
        run_torc_command(
            &state.torc_bin,
            &["-f", "json", "create", &req.spec],
            &state.api_url,
        )
        .await
    } else {
        // Spec is inline content - write to current directory with random name
        let unique_id = uuid::Uuid::new_v4();
        let temp_path = format!("torc_spec_{}{}", unique_id, file_extension);
        if let Err(e) = tokio::fs::write(&temp_path, &spec_content).await {
            return Json(CliResponse {
                success: false,
                stdout: String::new(),
                stderr: format!("Failed to write spec file: {}", e),
                exit_code: None,
            });
        }

        let result = run_torc_command(
            &state.torc_bin,
            &["-f", "json", "create", &temp_path],
            &state.api_url,
        )
        .await;

        // Handle file after creation attempt
        if result.success {
            if let Some(workflow_id) = extract_workflow_id(&result.stdout) {
                // Parse spec to get workflow name for final filename
                let workflow_name = serde_json::from_str::<serde_json::Value>(&spec_content)
                    .ok()
                    .and_then(|spec| spec.get("name").and_then(|v| v.as_str()).map(String::from))
                    .unwrap_or_else(|| "workflow".to_string());

                // Sanitize the workflow name for use as a filename
                let sanitized_name: String = workflow_name
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == '-' || c == '_' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();

                let final_path = format!("{}_{}{}", sanitized_name, workflow_id, file_extension);
                match tokio::fs::rename(&temp_path, &final_path).await {
                    Ok(_) => {
                        info!("Saved workflow spec to: {}", final_path);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to rename workflow spec from {} to {}: {}. Keeping original file.",
                            temp_path, final_path, e
                        );
                    }
                }
            } else {
                // Couldn't extract workflow ID from output, but creation succeeded.
                // Preserve the temp file with a fallback name to avoid data loss.
                let fallback_path = format!("workflow_{}{}", uuid::Uuid::new_v4(), file_extension);
                if let Err(e) = tokio::fs::rename(&temp_path, &fallback_path).await {
                    warn!(
                        "Failed to preserve workflow spec as {}: {}. File remains at {}.",
                        fallback_path, e, temp_path
                    );
                } else {
                    info!(
                        "Saved workflow spec to: {} (ID extraction failed but workflow was created)",
                        fallback_path
                    );
                }
            }
        } else {
            // Creation failed, delete temp file to avoid accumulating failed specs
            if let Err(e) = tokio::fs::remove_file(&temp_path).await {
                warn!("Failed to clean up temp file {}: {}", temp_path, e);
            }
        }

        result
    };

    Json(result)
}

/// Create a workflow with auto-generated Slurm schedulers
async fn cli_create_slurm_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateSlurmRequest>,
) -> impl IntoResponse {
    let spec_content = req.spec.clone();

    // Validate file extension to prevent path traversal attacks
    let raw_extension = req.file_extension.as_deref().unwrap_or(".json");
    let file_extension = match validate_file_extension(raw_extension) {
        Some(ext) => ext,
        None => {
            return Json(CliResponse {
                success: false,
                stdout: String::new(),
                stderr: format!(
                    "Invalid file extension '{}'. Allowed: .json, .yaml, .yml, .kdl, .json5",
                    raw_extension
                ),
                exit_code: None,
            });
        }
    };

    let result = if req.is_file {
        // Spec is a file path - two-step process: generate schedulers then create
        // Step 1: Generate Slurm schedulers
        let slurm_spec_path = format!(
            "{}_slurm.yaml",
            req.spec
                .trim_end_matches(".yaml")
                .trim_end_matches(".yml")
                .trim_end_matches(".json")
                .trim_end_matches(".json5")
                .trim_end_matches(".kdl")
        );
        let mut gen_args = vec!["slurm", "generate", "--account", &req.account];
        if let Some(ref profile) = req.profile {
            gen_args.push("--profile");
            gen_args.push(profile);
        }
        gen_args.push("-o");
        gen_args.push(&slurm_spec_path);
        gen_args.push(&req.spec);

        let gen_result = run_torc_command(&state.torc_bin, &gen_args, &state.api_url).await;
        if !gen_result.success {
            return Json(gen_result);
        }

        // Step 2: Create the workflow from generated spec
        let create_args = vec!["-f", "json", "create", &slurm_spec_path];
        run_torc_command(&state.torc_bin, &create_args, &state.api_url).await
    } else {
        // Spec is inline content - write to current directory with random name
        let unique_id = uuid::Uuid::new_v4();
        let temp_path = format!("torc_spec_{}{}", unique_id, file_extension);
        if let Err(e) = tokio::fs::write(&temp_path, &spec_content).await {
            return Json(CliResponse {
                success: false,
                stdout: String::new(),
                stderr: format!("Failed to write spec file: {}", e),
                exit_code: None,
            });
        }

        // Step 1: Generate Slurm schedulers
        let slurm_spec_path = format!("{}_slurm", temp_path);
        let mut gen_args = vec!["slurm", "generate", "--account", &req.account];
        if let Some(ref profile) = req.profile {
            gen_args.push("--profile");
            gen_args.push(profile);
        }
        gen_args.push("-o");
        gen_args.push(&slurm_spec_path);
        gen_args.push(&temp_path);

        let gen_result = run_torc_command(&state.torc_bin, &gen_args, &state.api_url).await;
        if !gen_result.success {
            // Clean up temp file
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Json(gen_result);
        }

        // Step 2: Create the workflow from generated spec
        let create_args = vec!["-f", "json", "create", &slurm_spec_path];
        let result = run_torc_command(&state.torc_bin, &create_args, &state.api_url).await;

        // Handle file after creation attempt
        if result.success {
            if let Some(workflow_id) = extract_workflow_id(&result.stdout) {
                // Parse spec to get workflow name for final filename
                let workflow_name = serde_json::from_str::<serde_json::Value>(&spec_content)
                    .ok()
                    .and_then(|spec| spec.get("name").and_then(|v| v.as_str()).map(String::from))
                    .unwrap_or_else(|| "workflow".to_string());

                // Sanitize the workflow name for use as a filename
                let sanitized_name: String = workflow_name
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == '-' || c == '_' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();

                let final_path = format!("{}_{}{}", sanitized_name, workflow_id, file_extension);
                match tokio::fs::rename(&temp_path, &final_path).await {
                    Ok(_) => {
                        info!("Saved workflow spec to: {}", final_path);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to rename workflow spec from {} to {}: {}. Keeping original file.",
                            temp_path, final_path, e
                        );
                    }
                }
            } else {
                // Couldn't extract workflow ID from output, but creation succeeded.
                // Preserve the temp file with a fallback name to avoid data loss.
                let fallback_path = format!("workflow_{}{}", uuid::Uuid::new_v4(), file_extension);
                if let Err(e) = tokio::fs::rename(&temp_path, &fallback_path).await {
                    warn!(
                        "Failed to preserve workflow spec as {}: {}. File remains at {}.",
                        fallback_path, e, temp_path
                    );
                } else {
                    info!(
                        "Saved workflow spec to: {} (ID extraction failed but workflow was created)",
                        fallback_path
                    );
                }
            }
        } else {
            // Creation failed, delete temp file to avoid accumulating failed specs
            if let Err(e) = tokio::fs::remove_file(&temp_path).await {
                warn!("Failed to clean up temp file {}: {}", temp_path, e);
            }
        }

        result
    };

    Json(result)
}

/// Validate a workflow specification using --dry-run to get structured validation info
async fn cli_validate_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateRequest>,
) -> impl IntoResponse {
    let result = if req.is_file {
        // Spec is a file path
        run_torc_command(
            &state.torc_bin,
            &["-f", "json", "create", &req.spec, "--dry-run"],
            &state.api_url,
        )
        .await
    } else {
        // Spec is inline content - write to temp file with correct extension
        // Use UUID to avoid race conditions with concurrent requests
        let extension = req.file_extension.as_deref().unwrap_or(".json");
        let unique_id = uuid::Uuid::new_v4();
        let temp_path = format!("/tmp/torc_spec_{}{}", unique_id, extension);
        if let Err(e) = tokio::fs::write(&temp_path, &req.spec).await {
            return Json(CliResponse {
                success: false,
                stdout: String::new(),
                stderr: format!("Failed to write temp file: {}", e),
                exit_code: None,
            });
        }
        let result = run_torc_command(
            &state.torc_bin,
            &["-f", "json", "create", &temp_path, "--dry-run"],
            &state.api_url,
        )
        .await;
        let _ = tokio::fs::remove_file(&temp_path).await;
        result
    };

    Json(result)
}

async fn cli_run_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WorkflowIdRequest>,
) -> impl IntoResponse {
    let result = run_torc_command(
        &state.torc_bin,
        &["workflows", "run", &req.workflow_id],
        &state.api_url,
    )
    .await;
    Json(result)
}

async fn cli_submit_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WorkflowIdRequest>,
) -> impl IntoResponse {
    let result = run_torc_command(
        &state.torc_bin,
        &["workflows", "submit", &req.workflow_id],
        &state.api_url,
    )
    .await;
    Json(result)
}

async fn cli_initialize_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<InitializeRequest>,
) -> impl IntoResponse {
    let mut args = vec!["workflows", "init", &req.workflow_id];
    if req.force {
        args.push("--force");
    }
    let result = run_torc_command(&state.torc_bin, &args, &state.api_url).await;
    Json(result)
}

/// Check initialization status using --dry-run to see if there are existing output files
async fn cli_check_initialize_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WorkflowIdRequest>,
) -> impl IntoResponse {
    // Run with -f json and --dry-run to get structured output about existing files
    let result = run_torc_command(
        &state.torc_bin,
        &[
            "-f",
            "json",
            "workflows",
            "init",
            &req.workflow_id,
            "--dry-run",
        ],
        &state.api_url,
    )
    .await;
    Json(result)
}

async fn cli_delete_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WorkflowIdRequest>,
) -> impl IntoResponse {
    let result = run_torc_command(
        &state.torc_bin,
        &["workflows", "delete", "--no-prompts", &req.workflow_id],
        &state.api_url,
    )
    .await;
    Json(result)
}

/// Cancel a workflow and its Slurm jobs
async fn cli_cancel_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WorkflowIdRequest>,
) -> impl IntoResponse {
    let result = run_torc_command(
        &state.torc_bin,
        &["workflows", "cancel", &req.workflow_id],
        &state.api_url,
    )
    .await;
    Json(result)
}

/// Reinitialize a workflow using the CLI reinitialize command
async fn cli_reinitialize_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<InitializeRequest>,
) -> impl IntoResponse {
    let mut args = vec!["workflows", "reinit", &req.workflow_id];
    if req.force {
        args.push("--force");
    }
    let result = run_torc_command(&state.torc_bin, &args, &state.api_url).await;
    Json(result)
}

/// Reset workflow status using the CLI reset-status command
async fn cli_reset_status_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WorkflowIdRequest>,
) -> impl IntoResponse {
    let result = run_torc_command(
        &state.torc_bin,
        &[
            "workflows",
            "reset-status",
            "--no-prompts",
            &req.workflow_id,
        ],
        &state.api_url,
    )
    .await;
    Json(result)
}

#[derive(Serialize)]
struct ExecutionPlanResponse {
    success: bool,
    /// Parsed execution plan data
    data: Option<serde_json::Value>,
    error: Option<String>,
}

/// Get the execution plan for a workflow
async fn cli_execution_plan_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WorkflowIdRequest>,
) -> impl IntoResponse {
    let args = vec![
        "-f",
        "json",
        "workflows",
        "execution-plan",
        &req.workflow_id,
    ];

    info!("Running: {} {}", state.torc_bin, args.join(" "));

    let output = Command::new(&state.torc_bin)
        .args(&args)
        .env("TORC_API_URL", &state.api_url)
        .output()
        .await;

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if !output.status.success() {
                return Json(ExecutionPlanResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Command failed: {}", stderr)),
                });
            }

            // Parse the JSON output
            match serde_json::from_str::<serde_json::Value>(&stdout) {
                Ok(data) => Json(ExecutionPlanResponse {
                    success: true,
                    data: Some(data),
                    error: None,
                }),
                Err(e) => Json(ExecutionPlanResponse {
                    success: false,
                    data: None,
                    error: Some(format!(
                        "Failed to parse JSON output: {}. Output: {}",
                        e, stdout
                    )),
                }),
            }
        }
        Err(e) => Json(ExecutionPlanResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to execute command: {}", e)),
        }),
    }
}

/// Streaming workflow run handler using Server-Sent Events
async fn cli_run_stream_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let workflow_id = match params.get("workflow_id") {
        Some(id) => id.clone(),
        None => {
            return Err((StatusCode::BAD_REQUEST, "Missing workflow_id parameter"));
        }
    };

    info!("Starting streaming run for workflow: {}", workflow_id);

    let torc_bin = state.torc_bin.clone();
    let api_url = state.api_url.clone();
    let http_client = state.client.clone();

    // Create the stream
    let stream = async_stream::stream! {
        // Send start event
        yield Ok::<_, std::convert::Infallible>(Event::default()
            .event("start")
            .data(format!("Running workflow {}", workflow_id)));

        // Spawn the command with piped stdout/stderr
        let mut child = match Command::new(&torc_bin)
            .args(["run", &workflow_id])
            .env("TORC_API_URL", &api_url)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                yield Ok(Event::default()
                    .event("error")
                    .data(format!("Failed to start command: {}", e)));
                yield Ok(Event::default()
                    .event("end")
                    .data("error"));
                return;
            }
        };

        // Read stdout and stderr
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        // Create channels for stdout/stderr lines
        let (tx, mut rx) = tokio::sync::mpsc::channel::<(String, String)>(100);

        // Spawn task to read stdout
        if let Some(stdout) = stdout {
            let tx_stdout = tx.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx_stdout.send(("stdout".to_string(), line)).await;
                }
            });
        }

        // Spawn task to read stderr
        if let Some(stderr) = stderr {
            let tx_stderr = tx.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx_stderr.send(("stderr".to_string(), line)).await;
                }
            });
        }

        // Drop the original sender so the channel closes when tasks finish
        drop(tx);

        // Spawn task to periodically poll job status
        let api_url_status = api_url.clone();
        let workflow_id_status = workflow_id.clone();
        let http_client_status = http_client.clone();
        let (status_tx, mut status_rx) = tokio::sync::mpsc::channel::<String>(10);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
            loop {
                interval.tick().await;

                // Fetch jobs from API and count statuses
                let url = format!("{}/jobs?workflow_id={}&limit={}", api_url_status, workflow_id_status, torc::MAX_RECORD_TRANSFER_COUNT);
                match http_client_status.get(&url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                            // Count jobs by status
                            let items = json.get("items").and_then(|v| v.as_array());
                            if let Some(jobs) = items {
                                let mut uninitialized = 0i64;
                                let mut blocked = 0i64;
                                let mut ready = 0i64;
                                let mut pending = 0i64;
                                let mut running = 0i64;
                                let mut completed = 0i64;
                                let mut failed = 0i64;
                                let mut canceled = 0i64;
                                let mut terminated = 0i64;
                                let mut disabled = 0i64;

                                for job in jobs {
                                    let status = job.get("status").and_then(|v| v.as_i64()).unwrap_or(-1);
                                    match status {
                                        0 => uninitialized += 1,
                                        1 => blocked += 1,
                                        2 => ready += 1,
                                        3 => pending += 1,
                                        4 => running += 1,
                                        5 => completed += 1,
                                        6 => failed += 1,
                                        7 => canceled += 1,
                                        8 => terminated += 1,
                                        9 => disabled += 1,
                                        _ => {} // Unknown status, ignore
                                    }
                                }

                                let total = jobs.len() as i64;

                                // Build status message with only non-zero counts
                                let mut parts = Vec::new();
                                if completed > 0 { parts.push(format!("{} completed", completed)); }
                                if running > 0 { parts.push(format!("{} running", running)); }
                                if ready > 0 { parts.push(format!("{} ready", ready)); }
                                if pending > 0 { parts.push(format!("{} pending", pending)); }
                                if blocked > 0 { parts.push(format!("{} blocked", blocked)); }
                                if failed > 0 { parts.push(format!("{} failed", failed)); }
                                if uninitialized > 0 { parts.push(format!("{} uninitialized", uninitialized)); }
                                if canceled > 0 { parts.push(format!("{} canceled", canceled)); }
                                if terminated > 0 { parts.push(format!("{} terminated", terminated)); }
                                if disabled > 0 { parts.push(format!("{} disabled", disabled)); }

                                let status_msg = format!(
                                    "Jobs: {} (total: {})",
                                    if parts.is_empty() { "none".to_string() } else { parts.join(", ") },
                                    total
                                );

                                if status_tx.send(status_msg).await.is_err() {
                                    break; // Receiver dropped, exit
                                }

                                // If no jobs are in an active/waiting state, exit polling
                                // Active states: running, pending, ready, blocked, uninitialized
                                if running == 0 && ready == 0 && pending == 0 && blocked == 0 && uninitialized == 0 {
                                    break;
                                }
                            }
                        }
                    }
                    _ => {} // Ignore errors, continue polling
                }
            }
        });

        // Main loop: receive from both channels
        loop {
            tokio::select! {
                // Process output lines
                msg = rx.recv() => {
                    match msg {
                        Some((event_type, line)) => {
                            yield Ok(Event::default()
                                .event(&event_type)
                                .data(line));
                        }
                        None => {
                            // Channel closed, output streams are done
                            break;
                        }
                    }
                }
                // Process status updates
                status = status_rx.recv() => {
                    if let Some(status_msg) = status {
                        yield Ok(Event::default()
                            .event("status")
                            .data(status_msg));
                    }
                }
            }
        }

        // Wait for the process to exit
        match child.wait().await {
            Ok(status) => {
                let exit_code = status.code().unwrap_or(-1);
                yield Ok(Event::default()
                    .event("end")
                    .data(if status.success() { "success" } else { "failed" }));
                yield Ok(Event::default()
                    .event("exit_code")
                    .data(exit_code.to_string()));
            }
            Err(e) => {
                yield Ok(Event::default()
                    .event("error")
                    .data(format!("Failed to wait for command: {}", e)));
                yield Ok(Event::default()
                    .event("end")
                    .data("error"));
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// Workflow recovery handler
/// Runs `torc -f json recover` for the specified workflow
/// Supports dry-run mode to preview changes before applying
async fn cli_recover_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RecoverRequest>,
) -> impl IntoResponse {
    info!(
        "Running recover for workflow: {} (dry_run={})",
        req.workflow_id, req.dry_run
    );

    let torc_bin = state.torc_bin.clone();
    let api_url = state.api_url.clone();

    // Build command arguments
    let mut args = vec![
        "-f".to_string(),
        "json".to_string(),
        "recover".to_string(),
        req.workflow_id.clone(),
        "-o".to_string(),
        req.output_dir.clone(),
        "--memory-multiplier".to_string(),
        req.memory_multiplier.to_string(),
        "--runtime-multiplier".to_string(),
        req.runtime_multiplier.to_string(),
    ];

    if req.dry_run {
        args.push("--dry-run".to_string());
    }

    if req.retry_unknown {
        args.push("--retry-unknown".to_string());
    }

    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    // Run the command and capture output
    let output = Command::new(&torc_bin)
        .args(&args_refs)
        .env("TORC_API_URL", &api_url)
        .output()
        .await;

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code();

            // Try to parse stdout as JSON (since we requested -f json)
            // If parsing fails, return raw output
            if output.status.success() {
                // Return the JSON directly if it parses
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                    Json(serde_json::json!({
                        "success": true,
                        "data": json,
                        "stderr": stderr,
                        "exit_code": exit_code
                    }))
                } else {
                    // JSON parsing failed, return raw output
                    Json(serde_json::json!({
                        "success": true,
                        "stdout": stdout,
                        "stderr": stderr,
                        "exit_code": exit_code
                    }))
                }
            } else {
                // Command failed - try to extract error from JSON output
                let error_msg = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout)
                {
                    json.get("error")
                        .and_then(|e| e.as_str())
                        .unwrap_or(&stderr)
                        .to_string()
                } else {
                    stderr.clone()
                };

                Json(serde_json::json!({
                    "success": false,
                    "error": error_msg,
                    "stdout": stdout,
                    "stderr": stderr,
                    "exit_code": exit_code
                }))
            }
        }
        Err(e) => Json(serde_json::json!({
            "success": false,
            "error": format!("Failed to execute recover command: {}", e)
        })),
    }
}

/// Sync job statuses with Slurm — detect and fail orphaned running jobs
/// Runs `torc -f json workflows sync-status <workflow_id>` with optional --dry-run
async fn cli_sync_status_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SyncStatusRequest>,
) -> impl IntoResponse {
    info!(
        "Running sync-status for workflow: {} (dry_run={})",
        req.workflow_id, req.dry_run
    );

    let mut args = vec![
        "-f".to_string(),
        "json".to_string(),
        "workflows".to_string(),
        "sync-status".to_string(),
        req.workflow_id.clone(),
    ];

    if req.dry_run {
        args.push("--dry-run".to_string());
    }

    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    let output = Command::new(&state.torc_bin)
        .args(&args_refs)
        .env("TORC_API_URL", &state.api_url)
        .output()
        .await;

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code();

            if output.status.success() {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                    Json(serde_json::json!({
                        "success": true,
                        "data": json,
                        "stderr": stderr,
                        "exit_code": exit_code
                    }))
                } else {
                    Json(serde_json::json!({
                        "success": true,
                        "stdout": stdout,
                        "stderr": stderr,
                        "exit_code": exit_code
                    }))
                }
            } else {
                let error_msg = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout)
                {
                    json.get("error")
                        .and_then(|e| e.as_str())
                        .unwrap_or(&stderr)
                        .to_string()
                } else {
                    stderr.clone()
                };

                Json(serde_json::json!({
                    "success": false,
                    "error": error_msg,
                    "stdout": stdout,
                    "stderr": stderr,
                    "exit_code": exit_code
                }))
            }
        }
        Err(e) => Json(serde_json::json!({
            "success": false,
            "error": format!("Failed to execute sync-status command: {}", e)
        })),
    }
}

/// Read file contents from filesystem
async fn cli_read_file_handler(Json(req): Json<ReadFileRequest>) -> impl IntoResponse {
    let path = FsPath::new(&req.path);
    let exists = path.exists();

    if !exists {
        return Json(ReadFileResponse {
            success: true,
            content: None,
            error: None,
            is_json: false,
            exists: false,
        });
    }

    // Check if it's a file (not a directory)
    if !path.is_file() {
        return Json(ReadFileResponse {
            success: false,
            content: None,
            error: Some("Path is not a file".to_string()),
            is_json: false,
            exists: true,
        });
    }

    // Read the file contents
    match tokio::fs::read_to_string(&req.path).await {
        Ok(content) => {
            // Check if it's JSON by file extension and try to parse
            let is_json = req.path.to_lowercase().ends_with(".json")
                || req.path.to_lowercase().ends_with(".json5");

            // If it looks like JSON, try to pretty-print it
            let content = if is_json {
                match serde_json::from_str::<serde_json::Value>(&content) {
                    Ok(value) => serde_json::to_string_pretty(&value).unwrap_or(content),
                    Err(_) => content, // Return as-is if parsing fails
                }
            } else {
                content
            };

            Json(ReadFileResponse {
                success: true,
                content: Some(content),
                error: None,
                is_json,
                exists: true,
            })
        }
        Err(e) => Json(ReadFileResponse {
            success: false,
            content: None,
            error: Some(format!("Failed to read file: {}", e)),
            is_json: false,
            exists: true,
        }),
    }
}

/// Export a workflow to a JSON file on the server
async fn cli_export_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExportRequest>,
) -> impl IntoResponse {
    // Default output path: workflow_<id>.json in current directory
    let default_output = format!("workflow_{}.json", req.workflow_id);
    let output_raw = req.output.as_deref().unwrap_or(&default_output);

    // Resolve to absolute path so the user knows exactly where the file goes
    let output_path = FsPath::new(output_raw);
    let output_abs = if output_path.is_relative() {
        std::env::current_dir()
            .map(|cwd| cwd.join(output_path).to_string_lossy().to_string())
            .unwrap_or_else(|_| output_raw.to_string())
    } else {
        output_raw.to_string()
    };

    let mut args = vec![
        "-f",
        "json",
        "workflows",
        "export",
        &req.workflow_id,
        "-o",
        &output_abs,
    ];
    if req.include_results {
        args.push("--include-results");
    }
    if req.include_events {
        args.push("--include-events");
    }
    let result = run_torc_command(&state.torc_bin, &args, &state.api_url).await;
    Json(result)
}

/// Import a workflow from a server-side file path or uploaded content
async fn cli_import_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ImportRequest>,
) -> impl IntoResponse {
    // Determine the file path to import from
    let temp_path_owned;
    let import_path = if let Some(ref path) = req.file_path {
        // Use server-side file path directly
        path.as_str()
    } else if let Some(ref content) = req.content {
        // Write uploaded content to a temp file
        let unique_id = uuid::Uuid::new_v4();
        temp_path_owned = format!("/tmp/torc_import_{}.json", unique_id);
        if let Err(e) = tokio::fs::write(&temp_path_owned, content).await {
            return Json(CliResponse {
                success: false,
                stdout: String::new(),
                stderr: format!("Failed to write temp file: {}", e),
                exit_code: None,
            });
        }
        temp_path_owned.as_str()
    } else {
        return Json(CliResponse {
            success: false,
            stdout: String::new(),
            stderr: "Either file_path or content must be provided".to_string(),
            exit_code: None,
        });
    };

    let name_str;
    let mut args = vec!["-f", "json", "workflows", "import", import_path];
    if let Some(ref name) = req.name {
        name_str = name.clone();
        args.push("--name");
        args.push(&name_str);
    }
    if req.skip_results {
        args.push("--skip-results");
    }
    if req.skip_events {
        args.push("--skip-events");
    }

    let result = run_torc_command(&state.torc_bin, &args, &state.api_url).await;

    // Clean up temp file if we created one (not for server-side paths)
    if req.file_path.is_none() && req.content.is_some() {
        let _ = tokio::fs::remove_file(import_path).await;
    }

    Json(result)
}

#[derive(Deserialize)]
struct PlotResourcesRequest {
    /// Path to resource database file(s)
    db_paths: Vec<String>,
    /// Output directory for generated plots (optional, defaults to temp)
    #[serde(default)]
    output_dir: Option<String>,
    /// Prefix for output filenames
    #[serde(default = "default_prefix")]
    prefix: String,
}

fn default_prefix() -> String {
    "resource_plot".to_string()
}

#[derive(Serialize)]
struct PlotResourcesResponse {
    success: bool,
    /// List of generated plot JSON files
    plots: Vec<PlotData>,
    error: Option<String>,
}

#[derive(Serialize)]
struct PlotData {
    name: String,
    /// The actual Plotly JSON data
    data: serde_json::Value,
}

#[derive(Deserialize)]
struct ListResourceDbsRequest {
    /// Base directory to search for resource databases
    base_dir: String,
}

#[derive(Serialize)]
struct ListResourceDbsResponse {
    success: bool,
    databases: Vec<ResourceDbInfo>,
    error: Option<String>,
}

#[derive(Serialize)]
struct ResourceDbInfo {
    path: String,
    name: String,
    size_bytes: u64,
    modified: String,
}

/// Generate resource plots from database files
async fn cli_plot_resources_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PlotResourcesRequest>,
) -> impl IntoResponse {
    if req.db_paths.is_empty() {
        return Json(PlotResourcesResponse {
            success: false,
            plots: vec![],
            error: Some("No database paths provided".to_string()),
        });
    }

    // Create a temp directory for output
    let temp_dir = std::env::temp_dir().join(format!("torc_plots_{}", std::process::id()));
    let has_custom_output_dir = req.output_dir.is_some();
    let output_dir = req
        .output_dir
        .map(std::path::PathBuf::from)
        .unwrap_or(temp_dir.clone());

    if let Err(e) = tokio::fs::create_dir_all(&output_dir).await {
        return Json(PlotResourcesResponse {
            success: false,
            plots: vec![],
            error: Some(format!("Failed to create output directory: {}", e)),
        });
    }

    // Build command arguments
    let mut args: Vec<&str> = vec!["plot-resources"];

    // Add all database paths
    let db_paths: Vec<&str> = req.db_paths.iter().map(|s| s.as_str()).collect();
    args.extend(db_paths.iter());

    let output_dir_str = output_dir.to_string_lossy().to_string();
    args.push("--output-dir");
    args.push(&output_dir_str);

    args.push("--prefix");
    args.push(&req.prefix);

    args.push("--format");
    args.push("json");

    info!("Running: {} {}", state.torc_bin, args.join(" "));

    let output = Command::new(&state.torc_bin).args(&args).output().await;

    match output {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Json(PlotResourcesResponse {
                    success: false,
                    plots: vec![],
                    error: Some(format!("Command failed: {}", stderr)),
                });
            }

            // Read all generated JSON files
            let mut plots = Vec::new();

            match tokio::fs::read_dir(&output_dir).await {
                Ok(mut entries) => {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        let path = entry.path();
                        if let Some(name) = path.file_name().and_then(|n| n.to_str())
                            && name.starts_with(&req.prefix)
                            && name.ends_with(".json")
                        {
                            match tokio::fs::read_to_string(&path).await {
                                Ok(content) => {
                                    match serde_json::from_str::<serde_json::Value>(&content) {
                                        Ok(data) => {
                                            plots.push(PlotData {
                                                name: name.to_string(),
                                                data,
                                            });
                                        }
                                        Err(e) => {
                                            warn!("Failed to parse plot JSON {}: {}", name, e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to read plot file {}: {}", name, e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    return Json(PlotResourcesResponse {
                        success: false,
                        plots: vec![],
                        error: Some(format!("Failed to read output directory: {}", e)),
                    });
                }
            }

            // Clean up temp directory if we created it
            if !has_custom_output_dir {
                let _ = tokio::fs::remove_dir_all(&temp_dir).await;
            }

            // Sort plots by name for consistent ordering
            plots.sort_by(|a, b| a.name.cmp(&b.name));

            Json(PlotResourcesResponse {
                success: true,
                plots,
                error: None,
            })
        }
        Err(e) => Json(PlotResourcesResponse {
            success: false,
            plots: vec![],
            error: Some(format!("Failed to execute command: {}", e)),
        }),
    }
}

/// List resource database files in a directory
async fn cli_list_resource_dbs_handler(
    Json(req): Json<ListResourceDbsRequest>,
) -> impl IntoResponse {
    let base_path = FsPath::new(&req.base_dir);

    if !base_path.exists() {
        return Json(ListResourceDbsResponse {
            success: true,
            databases: vec![],
            error: None,
        });
    }

    if !base_path.is_dir() {
        return Json(ListResourceDbsResponse {
            success: false,
            databases: vec![],
            error: Some("Path is not a directory".to_string()),
        });
    }

    let mut databases = Vec::new();

    match tokio::fs::read_dir(base_path).await {
        Ok(mut entries) => {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if let Some(ext) = path.extension()
                    && ext == "db"
                    && let Ok(metadata) = tokio::fs::metadata(&path).await
                {
                    let modified = metadata
                        .modified()
                        .ok()
                        .and_then(|t| {
                            t.duration_since(std::time::UNIX_EPOCH).ok().map(|d| {
                                chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                                    .unwrap_or_default()
                            })
                        })
                        .unwrap_or_default();

                    databases.push(ResourceDbInfo {
                        path: path.to_string_lossy().to_string(),
                        name: path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        size_bytes: metadata.len(),
                        modified,
                    });
                }
            }
        }
        Err(e) => {
            return Json(ListResourceDbsResponse {
                success: false,
                databases: vec![],
                error: Some(format!("Failed to read directory: {}", e)),
            });
        }
    }

    // Sort by modification time (newest first)
    databases.sort_by(|a, b| b.modified.cmp(&a.modified));

    Json(ListResourceDbsResponse {
        success: true,
        databases,
        error: None,
    })
}

// ============== Slurm Debugging Handlers ==============

#[derive(Deserialize)]
struct SlurmParseLogsRequest {
    /// Workflow ID
    workflow_id: i64,
    /// Output directory containing Slurm log files
    #[serde(default = "default_output_dir")]
    output_dir: String,
    /// Only show errors (skip warnings)
    #[serde(default)]
    errors_only: bool,
}

#[derive(Serialize)]
struct SlurmParseLogsResponse {
    success: bool,
    /// Parsed JSON output from the CLI command
    data: Option<serde_json::Value>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct SlurmSacctRequest {
    /// Workflow ID
    workflow_id: i64,
    /// Output directory for sacct JSON files
    #[serde(default = "default_output_dir")]
    output_dir: String,
}

#[derive(Serialize)]
struct SlurmSacctResponse {
    success: bool,
    /// Parsed JSON output from the CLI command
    data: Option<serde_json::Value>,
    error: Option<String>,
}

/// Parse Slurm log files for known error messages
async fn cli_slurm_parse_logs_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SlurmParseLogsRequest>,
) -> impl IntoResponse {
    let workflow_id_str = req.workflow_id.to_string();

    let mut args = vec![
        "-f",
        "json",
        "slurm",
        "parse-logs",
        &req.output_dir,
        "--workflow-id",
        &workflow_id_str,
    ];

    if req.errors_only {
        args.push("--errors-only");
    }

    info!("Running: {} {}", state.torc_bin, args.join(" "));

    let output = Command::new(&state.torc_bin)
        .args(&args)
        .env("TORC_API_URL", &state.api_url)
        .output()
        .await;

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if !output.status.success() {
                return Json(SlurmParseLogsResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Command failed: {}", stderr)),
                });
            }

            // Parse the JSON output
            match serde_json::from_str::<serde_json::Value>(&stdout) {
                Ok(data) => Json(SlurmParseLogsResponse {
                    success: true,
                    data: Some(data),
                    error: None,
                }),
                Err(e) => Json(SlurmParseLogsResponse {
                    success: false,
                    data: None,
                    error: Some(format!(
                        "Failed to parse JSON output: {}. Output: {}",
                        e, stdout
                    )),
                }),
            }
        }
        Err(e) => Json(SlurmParseLogsResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to execute command: {}", e)),
        }),
    }
}

/// Run sacct for scheduled compute nodes and save JSON output
async fn cli_slurm_sacct_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SlurmSacctRequest>,
) -> impl IntoResponse {
    let workflow_id_str = req.workflow_id.to_string();

    let args = vec![
        "-f",
        "json",
        "slurm",
        "sacct",
        &workflow_id_str,
        "--output-dir",
        &req.output_dir,
    ];

    info!("Running: {} {}", state.torc_bin, args.join(" "));

    let output = Command::new(&state.torc_bin)
        .args(&args)
        .env("TORC_API_URL", &state.api_url)
        .output()
        .await;

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if !output.status.success() {
                return Json(SlurmSacctResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Command failed: {}", stderr)),
                });
            }

            // Parse the JSON output
            match serde_json::from_str::<serde_json::Value>(&stdout) {
                Ok(data) => Json(SlurmSacctResponse {
                    success: true,
                    data: Some(data),
                    error: None,
                }),
                Err(e) => Json(SlurmSacctResponse {
                    success: false,
                    data: None,
                    error: Some(format!(
                        "Failed to parse JSON output: {}. Output: {}",
                        e, stdout
                    )),
                }),
            }
        }
        Err(e) => Json(SlurmSacctResponse {
            success: false,
            data: None,
            error: Some(format!("Failed to execute command: {}", e)),
        }),
    }
}

// ============== HPC Profile Detection Handlers ==============

#[derive(Serialize)]
struct HpcProfileInfo {
    name: String,
    display_name: String,
    description: String,
    is_detected: bool,
}

#[derive(Serialize)]
struct HpcProfilesResponse {
    success: bool,
    profiles: Vec<HpcProfileInfo>,
    detected_profile: Option<String>,
    error: Option<String>,
}

/// List available HPC profiles and detect current profile
async fn cli_hpc_profiles_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Run: torc hpc list -f json
    let args = vec!["-f", "json", "hpc", "list"];

    info!("Running: {} {}", state.torc_bin, args.join(" "));

    let output = Command::new(&state.torc_bin)
        .args(&args)
        .env("TORC_API_URL", &state.api_url)
        .output()
        .await;

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if !output.status.success() {
                return Json(HpcProfilesResponse {
                    success: false,
                    profiles: vec![],
                    detected_profile: None,
                    error: Some(format!("Command failed: {}", stderr)),
                });
            }

            // Parse the JSON output - it's an array of profiles directly
            match serde_json::from_str::<Vec<serde_json::Value>>(&stdout) {
                Ok(items) => {
                    let mut profiles = Vec::new();
                    let mut detected_profile = None;

                    for item in items {
                        let name = item
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let display_name = item
                            .get("display_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&name)
                            .to_string();
                        let description = item
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let is_detected = item
                            .get("detected")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);

                        if is_detected {
                            detected_profile = Some(name.clone());
                        }

                        profiles.push(HpcProfileInfo {
                            name,
                            display_name,
                            description,
                            is_detected,
                        });
                    }

                    Json(HpcProfilesResponse {
                        success: true,
                        profiles,
                        detected_profile,
                        error: None,
                    })
                }
                Err(e) => Json(HpcProfilesResponse {
                    success: false,
                    profiles: vec![],
                    detected_profile: None,
                    error: Some(format!(
                        "Failed to parse JSON output: {}. Output: {}",
                        e, stdout
                    )),
                }),
            }
        }
        Err(e) => Json(HpcProfilesResponse {
            success: false,
            profiles: vec![],
            detected_profile: None,
            error: Some(format!("Failed to execute command: {}", e)),
        }),
    }
}

// ============== Server Management Handlers ==============

#[derive(Deserialize)]
struct ServerStartRequest {
    /// Port for the server to listen on
    #[serde(default = "default_server_port")]
    port: u16,
    /// Database path (optional)
    #[serde(default)]
    database: Option<String>,
    /// Completion check interval in seconds
    #[serde(default = "default_completion_interval")]
    completion_check_interval_secs: u32,
    /// Log level
    #[serde(default = "default_log_level")]
    log_level: String,
}

fn default_server_port() -> u16 {
    8080
}

fn default_completion_interval() -> u32 {
    5
}

fn default_log_level() -> String {
    "info".to_string()
}

#[derive(Serialize)]
struct ServerStartResponse {
    success: bool,
    message: String,
    pid: Option<u32>,
    port: Option<u16>,
}

#[derive(Serialize)]
struct ServerStopResponse {
    success: bool,
    message: String,
}

#[derive(Serialize)]
struct ServerStatusResponse {
    running: bool,
    managed: bool,
    pid: Option<u32>,
    port: Option<u16>,
    output_lines: Vec<String>,
}

/// Start the torc-server process
async fn server_start_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ServerStartRequest>,
) -> impl IntoResponse {
    let mut managed = state.managed_server.lock().await;

    // Check if we're already managing a server
    if managed.pid.is_some() {
        return Json(ServerStartResponse {
            success: false,
            message: "Server is already running".to_string(),
            pid: managed.pid,
            port: managed.port,
        });
    }

    // Build command arguments
    let mut args = vec![
        "run".to_string(),
        "--port".to_string(),
        req.port.to_string(),
        "--log-level".to_string(),
        req.log_level.clone(),
        "--completion-check-interval-secs".to_string(),
        req.completion_check_interval_secs.to_string(),
    ];

    if let Some(ref db) = req.database
        && !db.is_empty()
    {
        args.push("--database".to_string());
        args.push(db.clone());
    }

    info!(
        "Starting torc-server: {} {}",
        state.torc_server_bin,
        args.join(" ")
    );

    // Start the server process
    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    match Command::new(&state.torc_server_bin)
        .args(&args_refs)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(mut child) => {
            let pid = child.id();
            let mut actual_port = req.port;

            // Read stdout to find the actual port (especially important when port 0 is used)
            if let Some(stdout) = child.stdout.take() {
                let mut reader = BufReader::new(stdout);
                let mut line = String::new();

                // Read lines until we find TORC_SERVER_PORT or timeout
                let timeout = tokio::time::Duration::from_secs(10);
                let start = std::time::Instant::now();

                loop {
                    if start.elapsed() > timeout {
                        warn!("Timeout waiting for server to report port, using requested port");
                        break;
                    }

                    match tokio::time::timeout(
                        tokio::time::Duration::from_millis(100),
                        reader.read_line(&mut line),
                    )
                    .await
                    {
                        Ok(Ok(0)) => break, // EOF
                        Ok(Ok(_)) => {
                            // Check for the port line
                            if let Some(port_str) = line.strip_prefix("TORC_SERVER_PORT=")
                                && let Ok(port) = port_str.trim().parse::<u16>()
                            {
                                actual_port = port;
                                info!("Server reported actual port: {}", actual_port);
                                break;
                            }
                            line.clear();
                        }
                        Ok(Err(e)) => {
                            warn!("Error reading server output: {}", e);
                            break;
                        }
                        Err(_) => {
                            // Timeout on this read, continue
                            continue;
                        }
                    }
                }
            }

            managed.pid = pid;
            managed.port = Some(actual_port);
            managed.output_lines.clear();
            managed.output_lines.push(format!(
                "Server started with PID {} on port {}",
                pid.unwrap_or(0),
                actual_port
            ));

            Json(ServerStartResponse {
                success: true,
                message: format!("Server started on port {}", actual_port),
                pid,
                port: Some(actual_port),
            })
        }
        Err(e) => Json(ServerStartResponse {
            success: false,
            message: format!("Failed to start server: {}", e),
            pid: None,
            port: None,
        }),
    }
}

/// Stop the managed torc-server process
async fn server_stop_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut managed = state.managed_server.lock().await;

    if let Some(pid) = managed.pid {
        // Try to kill the process
        #[cfg(unix)]
        {
            let result = StdCommand::new("kill").arg(pid.to_string()).status();

            match result {
                Ok(status) if status.success() => {
                    managed.pid = None;
                    managed.port = None;
                    managed.output_lines.push("Server stopped".to_string());
                    Json(ServerStopResponse {
                        success: true,
                        message: "Server stopped".to_string(),
                    })
                }
                Ok(_) => {
                    // Try SIGKILL
                    let _ = StdCommand::new("kill")
                        .args(["-9", &pid.to_string()])
                        .status();
                    managed.pid = None;
                    managed.port = None;
                    Json(ServerStopResponse {
                        success: true,
                        message: "Server force stopped".to_string(),
                    })
                }
                Err(e) => Json(ServerStopResponse {
                    success: false,
                    message: format!("Failed to stop server: {}", e),
                }),
            }
        }

        #[cfg(not(unix))]
        {
            // On Windows, try taskkill
            let result = StdCommand::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .status();

            match result {
                Ok(status) if status.success() => {
                    managed.pid = None;
                    managed.port = None;
                    Json(ServerStopResponse {
                        success: true,
                        message: "Server stopped".to_string(),
                    })
                }
                _ => Json(ServerStopResponse {
                    success: false,
                    message: "Failed to stop server".to_string(),
                }),
            }
        }
    } else {
        Json(ServerStopResponse {
            success: false,
            message: "No managed server is running".to_string(),
        })
    }
}

/// Get the status of the managed server
async fn server_status_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let managed = state.managed_server.lock().await;

    // Check if the process is still running
    let mut running = false;
    if let Some(pid) = managed.pid {
        #[cfg(unix)]
        {
            // Check if process exists by sending signal 0
            if let Ok(status) = StdCommand::new("kill")
                .args(["-0", &pid.to_string()])
                .status()
            {
                running = status.success();
            }
        }

        #[cfg(not(unix))]
        {
            // On Windows, check with tasklist
            if let Ok(output) = StdCommand::new("tasklist")
                .args(["/FI", &format!("PID eq {}", pid)])
                .output()
            {
                let output_str = String::from_utf8_lossy(&output.stdout);
                running = output_str.contains(&pid.to_string());
            }
        }
    }

    Json(ServerStatusResponse {
        running,
        managed: managed.pid.is_some(),
        pid: if running { managed.pid } else { None },
        port: if running { managed.port } else { None },
        output_lines: managed.output_lines.clone(),
    })
}

// ============== Version Handler ==============

#[derive(Serialize)]
struct VersionResponse {
    version: String,
    api_version: String,
    server_version: Option<String>,
    server_api_version: Option<String>,
    version_mismatch: Option<String>,
    mismatch_severity: Option<String>,
}

/// Return the torc-dash version and check server version compatibility
async fn version_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    use torc::client::version_check;

    // Create a configuration to fetch server version
    let api_url = state.api_url.clone();

    // Run the blocking version check in a spawn_blocking task to avoid runtime panics
    let result = tokio::task::spawn_blocking(move || {
        let config = torc::client::apis::configuration::Configuration {
            base_path: api_url,
            ..Default::default()
        };
        version_check::check_version(&config)
    })
    .await
    .ok();

    let (server_version, server_api_version, version_mismatch, mismatch_severity) = match result {
        Some(result) => match &result.server_version {
            Some(server_ver) => {
                let severity_str = match result.severity {
                    version_check::VersionMismatchSeverity::None => None,
                    version_check::VersionMismatchSeverity::Patch => Some("patch".to_string()),
                    version_check::VersionMismatchSeverity::Minor => Some("minor".to_string()),
                    version_check::VersionMismatchSeverity::Major => Some("major".to_string()),
                };
                let mismatch_msg = if result.severity.has_warning() {
                    Some(result.message.clone())
                } else {
                    None
                };
                (
                    Some(server_ver.clone()),
                    result.server_api_version.clone(),
                    mismatch_msg,
                    severity_str,
                )
            }
            None => (None, None, None, None),
        },
        None => (None, None, None, None),
    };

    // Extract just the semver from server version (strip git hash suffix for display)
    let server_version_display = server_version
        .as_ref()
        .map(|v| v.split(" (").next().unwrap_or(v).to_string());

    Json(VersionResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        api_version: version_check::CLIENT_API_VERSION.to_string(),
        server_version: server_version_display,
        server_api_version,
        version_mismatch,
        mismatch_severity,
    })
}

// ============== User Handler ==============

#[derive(Serialize)]
struct UserResponse {
    user: String,
}

/// Return the current user from the environment
async fn user_handler() -> impl IntoResponse {
    let user = torc::get_username();

    Json(UserResponse { user })
}

/// Execute a torc CLI command
// ============== AI Chat Handlers ==============

#[derive(Deserialize)]
struct ChatRequest {
    messages: Vec<ChatMessage>,
    #[serde(default)]
    workflow_id: Option<i64>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct ChatMessage {
    role: String,
    content: ChatContent,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(untagged)]
enum ChatContent {
    Text(String),
    Blocks(Vec<serde_json::Value>),
}

#[derive(Serialize)]
struct ChatStatusResponse {
    available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

async fn chat_status_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let provider = *state.llm_provider.read().await;
    let has_key = state.llm_api_key.read().await.is_some();

    // Ollama doesn't require an API key (local), but others do
    let available = match provider {
        LlmProvider::Ollama => true, // Always available (assumes local Ollama is running)
        _ => has_key,
    };

    let reason = if !available {
        let msg = match provider {
            LlmProvider::Anthropic => {
                "No API key configured. Set ANTHROPIC_API_KEY or use Azure AI Foundry."
            }
            LlmProvider::OpenAI => "No API key configured. Set OPENAI_API_KEY.",
            LlmProvider::GitHub => "No GitHub token configured. Set GITHUB_TOKEN.",
            LlmProvider::Ollama => "Ollama should be available locally.",
        };
        Some(msg.to_string())
    } else {
        None
    };

    Json(ChatStatusResponse { available, reason })
}

#[derive(Deserialize)]
struct ConfigureChatRequest {
    /// API key (or "ollama" for local Ollama, or GitHub token for GitHub Models)
    api_key: String,
    /// Provider type: "anthropic", "foundry", "custom", "ollama", or "github"
    #[serde(default = "default_provider")]
    provider: String,
    /// Azure AI Foundry resource name (required when provider = "foundry")
    #[serde(default)]
    foundry_resource: Option<String>,
    /// Custom base URL (required when provider = "custom", optional for others)
    #[serde(default)]
    base_url: Option<String>,
    /// Custom auth header name (optional, defaults to provider-appropriate value)
    #[serde(default)]
    auth_header: Option<String>,
    /// Model name (optional, defaults to provider-appropriate model)
    #[serde(default)]
    model: Option<String>,
}

fn default_provider() -> String {
    "anthropic".to_string()
}

/// Configure API key and provider at runtime (stored in memory only for this session).
async fn configure_chat_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ConfigureChatRequest>,
) -> impl IntoResponse {
    validate_same_origin(&headers)
        .map_err(|status| (status, "Cross-origin requests are not allowed"))?;

    let key = req.api_key.trim().to_string();

    let (provider, base_url, auth_header, model) = match req.provider.as_str() {
        "ollama" => {
            // Ollama doesn't require a real API key
            let url = req
                .base_url
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("http://localhost:11434/v1")
                .to_string();
            let model = req
                .model
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("llama3.2")
                .to_string();
            info!(
                "AI Chat: configured via dashboard UI as Ollama (url={})",
                url
            );
            (LlmProvider::Ollama, url, "Authorization".to_string(), model)
        }
        "github" => {
            if key.is_empty() {
                return Err((StatusCode::BAD_REQUEST, "GitHub token is required"));
            }
            let url = req
                .base_url
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("https://models.inference.ai.azure.com")
                .to_string();
            let model = req
                .model
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("gpt-4o")
                .to_string();
            info!("AI Chat: configured via dashboard UI as GitHub Models");
            (LlmProvider::GitHub, url, "Authorization".to_string(), model)
        }
        "openai" => {
            if key.is_empty() {
                return Err((StatusCode::BAD_REQUEST, "OpenAI API key is required"));
            }
            let url = req
                .base_url
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("https://api.openai.com/v1")
                .to_string();
            let model = req
                .model
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("gpt-4o")
                .to_string();
            info!("AI Chat: configured via dashboard UI as OpenAI");
            (LlmProvider::OpenAI, url, "Authorization".to_string(), model)
        }
        "foundry" => {
            if key.is_empty() {
                return Err((StatusCode::BAD_REQUEST, "API key must not be empty"));
            }
            let resource = req
                .foundry_resource
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or((StatusCode::BAD_REQUEST, "Foundry resource name is required"))?;
            let model = req
                .model
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("claude-sonnet-4-20250514")
                .to_string();
            info!(
                "AI Chat: configured via dashboard UI as Azure AI Foundry (resource={})",
                resource
            );
            (
                LlmProvider::Anthropic,
                format!("https://{}.services.ai.azure.com/anthropic/v1", resource),
                "x-api-key".to_string(),
                model,
            )
        }
        "custom" => {
            if key.is_empty() {
                return Err((StatusCode::BAD_REQUEST, "API key must not be empty"));
            }
            let url = req
                .base_url
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or((StatusCode::BAD_REQUEST, "Base URL is required"))?
                .to_string();
            let header = req
                .auth_header
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("x-api-key")
                .to_string();
            let model = req
                .model
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("claude-sonnet-4-20250514")
                .to_string();
            info!(
                "AI Chat: configured via dashboard UI as custom endpoint (url={})",
                url
            );
            (LlmProvider::Anthropic, url, header, model)
        }
        _ => {
            // "anthropic" (direct)
            if key.is_empty() {
                return Err((StatusCode::BAD_REQUEST, "API key must not be empty"));
            }
            let model = req
                .model
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("claude-sonnet-4-20250514")
                .to_string();
            info!("AI Chat: configured via dashboard UI as direct Anthropic API");
            (
                LlmProvider::Anthropic,
                "https://api.anthropic.com/v1".to_string(),
                "x-api-key".to_string(),
                model,
            )
        }
    };

    // For Ollama, we use a dummy key
    let final_key = if provider == LlmProvider::Ollama {
        Some("ollama".to_string())
    } else {
        Some(key)
    };

    *state.llm_provider.write().await = provider;
    *state.llm_api_key.write().await = final_key;
    *state.llm_base_url.write().await = base_url;
    *state.llm_auth_header.write().await = auth_header;
    *state.llm_model.write().await = model;

    Ok(Json(ChatStatusResponse {
        available: true,
        reason: None,
    }))
}

/// Ensure the MCP client is connected, spawning the subprocess if needed.
/// Returns a clone of the peer handle and cached tools.
async fn ensure_mcp_client(
    state: &AppState,
) -> Result<(
    rmcp::service::Peer<rmcp::service::RoleClient>,
    Vec<rmcp::model::Tool>,
)> {
    let mut guard = state.mcp_client.lock().await;

    if let Some(ref client) = *guard {
        return Ok((client.peer.clone(), client.tools.clone()));
    }

    info!(
        "Spawning torc-mcp-server: {} --api-url {}",
        state.torc_mcp_server_bin, state.api_url
    );

    let mut command = tokio::process::Command::new(&state.torc_mcp_server_bin);
    command
        .arg("--api-url")
        .arg(&state.api_url)
        .stderr(std::process::Stdio::inherit());
    let child_process = TokioChildProcess::new(command)
        .map_err(|e| anyhow::anyhow!("Failed to spawn torc-mcp-server: {}", e))?;

    // Connect as MCP client
    let running_service: rmcp::service::RunningService<rmcp::service::RoleClient, _> = ()
        .serve(child_process)
        .await
        .map_err(|e| anyhow::anyhow!("MCP handshake failed: {}", e))?;

    let peer = running_service.peer().clone();

    // Discover all tools
    let tools = peer
        .list_all_tools()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to list MCP tools: {}", e))?;

    // Estimate token usage for tools (helps debug GitHub Models limits)
    let openai_tools: Vec<serde_json::Value> = tools
        .iter()
        .map(|tool| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": tool.name.as_ref(),
                    "description": tool.description.as_ref(),
                    "parameters": tool.schema_as_json_value(),
                }
            })
        })
        .collect();
    let tools_json = serde_json::json!(openai_tools);
    let tools_tokens = estimate_tokens(&tools_json);

    info!(
        "MCP client connected, discovered {} tools (~{} tokens, {} chars)",
        tools.len(),
        tools_tokens,
        serde_json::to_string(&tools_json)
            .map(|s| s.len())
            .unwrap_or(0)
    );

    if tools_tokens > 6000 {
        warn!("Tools exceed 6000 tokens - GitHub Models will not be able to use them");
    }

    *guard = Some(McpClient {
        peer: peer.clone(),
        tools: tools.clone(),
    });

    Ok((peer, tools))
}

/// Convert MCP tools to Claude API tool format (Anthropic).
fn mcp_tools_to_claude_tools(tools: &[rmcp::model::Tool]) -> Vec<serde_json::Value> {
    tools
        .iter()
        .map(|tool| {
            serde_json::json!({
                "name": tool.name.as_ref(),
                "description": tool.description.as_ref(),
                "input_schema": tool.schema_as_json_value(),
            })
        })
        .collect()
}

/// Convert MCP tools to OpenAI function calling format.
fn mcp_tools_to_openai_tools(tools: &[rmcp::model::Tool]) -> Vec<serde_json::Value> {
    tools
        .iter()
        .map(|tool| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": tool.name.as_ref(),
                    "description": tool.description.as_ref(),
                    "parameters": tool.schema_as_json_value(),
                }
            })
        })
        .collect()
}

/// Essential tools for providers with limited context (e.g., GitHub Models with 8k limit).
/// These are the most useful tools for basic workflow management.
const ESSENTIAL_TOOLS: &[&str] = &[
    "get_workflow_status",
    "get_workflow_summary",
    "list_jobs_by_status",
    "list_failed_jobs",
    "get_job_logs",
    "get_job_details",
    "recover_workflow",
    "get_docs",
    "list_examples",
    "get_example",
    "create_workflow",
];

/// Filter tools to only essential ones for limited-context providers.
fn filter_essential_tools(tools: &[rmcp::model::Tool]) -> Vec<&rmcp::model::Tool> {
    tools
        .iter()
        .filter(|tool| ESSENTIAL_TOOLS.contains(&tool.name.as_ref()))
        .collect()
}

/// Convert MCP tools to OpenAI format, filtered to essential tools only.
fn mcp_tools_to_openai_tools_filtered(tools: &[rmcp::model::Tool]) -> Vec<serde_json::Value> {
    filter_essential_tools(tools)
        .iter()
        .map(|tool| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": tool.name.as_ref(),
                    "description": tool.description.as_ref(),
                    "parameters": tool.schema_as_json_value(),
                }
            })
        })
        .collect()
}

/// Convert chat messages to OpenAI format.
fn messages_to_openai_format(
    messages: &[serde_json::Value],
    system_prompt: &str,
) -> Vec<serde_json::Value> {
    let mut openai_messages = vec![serde_json::json!({
        "role": "system",
        "content": system_prompt,
    })];

    for msg in messages {
        let role = msg["role"].as_str().unwrap_or("user");

        // Handle tool results (from previous round)
        if let Some(content_array) = msg["content"].as_array() {
            // Check if this is an array of tool_result blocks
            if content_array
                .first()
                .map(|c| c["type"].as_str() == Some("tool_result"))
                .unwrap_or(false)
            {
                // Convert each tool_result to an OpenAI tool message
                for result in content_array {
                    if let Some(tool_use_id) = result["tool_use_id"].as_str() {
                        let content = result["content"].as_str().unwrap_or("");
                        openai_messages.push(serde_json::json!({
                            "role": "tool",
                            "tool_call_id": tool_use_id,
                            "content": content,
                        }));
                    }
                }
                continue;
            }

            // Check if this is an assistant message with tool_use blocks
            if role == "assistant" {
                let mut text_content = String::new();
                let mut tool_calls = Vec::new();

                for block in content_array {
                    match block["type"].as_str() {
                        Some("text") => {
                            if let Some(text) = block["text"].as_str() {
                                text_content.push_str(text);
                            }
                        }
                        Some("tool_use") => {
                            tool_calls.push(serde_json::json!({
                                "id": block["id"],
                                "type": "function",
                                "function": {
                                    "name": block["name"],
                                    "arguments": serde_json::to_string(&block["input"]).unwrap_or_default(),
                                }
                            }));
                        }
                        _ => {}
                    }
                }

                let mut assistant_msg = serde_json::json!({
                    "role": "assistant",
                });
                if !text_content.is_empty() {
                    assistant_msg["content"] = serde_json::json!(text_content);
                }
                if !tool_calls.is_empty() {
                    assistant_msg["tool_calls"] = serde_json::json!(tool_calls);
                }
                openai_messages.push(assistant_msg);
                continue;
            }
        }

        // Simple text message
        let content = match &msg["content"] {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(arr) => {
                // Extract text from content blocks
                arr.iter()
                    .filter_map(|block| block["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            _ => String::new(),
        };

        openai_messages.push(serde_json::json!({
            "role": role,
            "content": content,
        }));
    }

    openai_messages
}

/// Estimate token count for a JSON value.
/// Uses conservative estimate of ~3 chars per token (JSON has overhead from quotes, braces, etc.)
fn estimate_tokens(value: &serde_json::Value) -> usize {
    let json_str = serde_json::to_string(value).unwrap_or_default();
    // Conservative estimate: ~3 characters per token for JSON content
    // This accounts for JSON overhead (quotes, braces, colons) and mixed content
    json_str.len().div_ceil(3)
}

/// Truncate messages to fit within a token limit, preserving system prompt and recent messages.
/// Returns truncated messages and whether truncation occurred.
fn truncate_messages_to_token_limit(
    messages: Vec<serde_json::Value>,
    tools: &serde_json::Value,
    max_tokens: usize,
) -> (Vec<serde_json::Value>, bool) {
    // Reserve tokens for tools, model overhead, and response
    let tools_tokens = estimate_tokens(tools);
    let overhead = 1000; // Buffer for model name, max_tokens field, request structure, etc.
    let available_tokens = max_tokens.saturating_sub(tools_tokens + overhead);

    info!(
        "Token budget: max={}, tools={}, overhead={}, available for messages={}",
        max_tokens, tools_tokens, overhead, available_tokens
    );

    // Always keep the system message (first message)
    if messages.is_empty() {
        return (messages, false);
    }

    let system_msg = &messages[0];
    let system_tokens = estimate_tokens(system_msg);

    info!(
        "System message tokens: {}, remaining after system: {}",
        system_tokens,
        available_tokens.saturating_sub(system_tokens)
    );

    // If system message alone exceeds limit, truncate it
    if system_tokens >= available_tokens {
        warn!(
            "System message ({} tokens) exceeds available budget ({} tokens), truncating",
            system_tokens, available_tokens
        );
        return (vec![system_msg.clone()], true);
    }

    let mut remaining_tokens = available_tokens - system_tokens;
    let mut kept_messages = vec![system_msg.clone()];
    let mut truncated = false;

    // Process messages from most recent to oldest (skip system message at index 0)
    let other_messages: Vec<_> = messages[1..].iter().collect();
    let mut selected_indices = Vec::new();

    for (i, msg) in other_messages.iter().enumerate().rev() {
        let msg_tokens = estimate_tokens(msg);
        if msg_tokens <= remaining_tokens {
            remaining_tokens -= msg_tokens;
            selected_indices.push(i);
        } else {
            truncated = true;
        }
    }

    // Reverse to maintain chronological order
    selected_indices.reverse();
    for i in selected_indices {
        kept_messages.push(other_messages[i].clone());
    }

    info!(
        "Kept {} of {} messages (truncated={})",
        kept_messages.len(),
        messages.len(),
        truncated
    );

    (kept_messages, truncated)
}

/// The system prompt for the AI chat assistant.
fn chat_system_prompt(workflow_id: Option<i64>) -> String {
    let mut prompt = String::from(
        "You are an AI assistant for the Torc workflow orchestration system. \
         You help users manage, monitor, debug, and recover computational workflows. \
         You have access to tools that let you interact with the Torc server.\n\n\
         When a user asks about workflows or jobs, use the available tools to get real data \
         rather than speculating. Be concise and helpful.\n\n\
         When showing job or workflow data, format it clearly. \
         If a tool returns an error, explain what went wrong and suggest alternatives.",
    );

    if let Some(wf_id) = workflow_id {
        prompt.push_str(&format!(
            "\n\nThe user is currently viewing workflow {}. \
             Use this workflow_id by default when calling tools, unless they specify a different one.",
            wf_id
        ));
    }

    prompt
}

/// Extract text from MCP CallToolResult content.
fn extract_tool_result_text(result: &rmcp::model::CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn validate_same_origin(headers: &HeaderMap) -> Result<(), StatusCode> {
    let origin = match headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        Some(origin) => origin,
        None => return Ok(()),
    };

    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::FORBIDDEN)?;

    let expected_http = format!("http://{}", host);
    let expected_https = format!("https://{}", host);

    if origin == expected_http || origin == expected_https {
        Ok(())
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

/// Chat endpoint: streams SSE events as the AI processes the conversation.
/// Supports both Anthropic API and OpenAI-compatible APIs (Ollama, GitHub Models).
async fn chat_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> impl IntoResponse {
    validate_same_origin(&headers)
        .map_err(|status| (status, "Cross-origin requests are not allowed"))?;

    let provider = *state.llm_provider.read().await;
    let api_key = match state.llm_api_key.read().await.clone() {
        Some(key) => key,
        None if provider == LlmProvider::Ollama => "ollama".to_string(),
        None => {
            return Err((StatusCode::SERVICE_UNAVAILABLE, "No API key configured"));
        }
    };

    let model = state.llm_model.read().await.clone();
    let base_url = state.llm_base_url.read().await.clone();
    let auth_header_name = state.llm_auth_header.read().await.clone();
    let workflow_id = req.workflow_id;
    let initial_messages = req.messages;

    // Get MCP client and tools
    let (peer, tools) = match ensure_mcp_client(&state).await {
        Ok(result) => result,
        Err(e) => {
            // Reset the client so next request retries
            *state.mcp_client.lock().await = None;
            error!("MCP client error: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to connect to MCP server",
            ));
        }
    };

    let system_prompt = chat_system_prompt(workflow_id);
    let http_client = state.client.clone();

    // Choose API format based on provider
    let is_openai_compatible = matches!(
        provider,
        LlmProvider::OpenAI | LlmProvider::Ollama | LlmProvider::GitHub
    );

    let stream = async_stream::stream! {
        // Build the messages for the API (Anthropic format initially)
        let mut messages: Vec<serde_json::Value> = initial_messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                })
            })
            .collect();

        let max_tool_rounds = 20;
        let mut round = 0;

        loop {
            if round >= max_tool_rounds {
                yield Ok::<_, std::convert::Infallible>(Event::default()
                    .event("error")
                    .data("Maximum tool call rounds reached"));
                break;
            }
            round += 1;

            // Build API request based on provider
            let (api_url, api_body, auth_header_value) = if is_openai_compatible {
                // OpenAI-compatible API (Ollama, GitHub Models)
                let url = format!("{}/chat/completions", base_url);
                let openai_tools = mcp_tools_to_openai_tools(&tools);
                let openai_messages = messages_to_openai_format(&messages, &system_prompt);

                // GitHub Models has a strict 8000 token input limit for gpt-4o
                // Use filtered essential tools only for GitHub to fit within limit
                let (final_messages, final_tools, was_truncated) = if provider == LlmProvider::GitHub {
                    // Use only essential tools for GitHub Models
                    let filtered_tools = mcp_tools_to_openai_tools_filtered(&tools);
                    let tools_json = serde_json::json!(&filtered_tools);
                    let tools_tokens = estimate_tokens(&tools_json);

                    info!(
                        "GitHub Models: using {} essential tools (~{} tokens)",
                        filtered_tools.len(),
                        tools_tokens
                    );

                    let (msgs, truncated) = truncate_messages_to_token_limit(openai_messages, &tools_json, 6000);
                    (msgs, filtered_tools, truncated)
                } else {
                    (openai_messages, openai_tools.clone(), false)
                };

                if was_truncated {
                    info!("Truncated conversation history to fit GitHub Models token limit");
                }

                let body = serde_json::json!({
                    "model": model,
                    "max_tokens": 8192,
                    "messages": final_messages,
                    "tools": final_tools,
                });

                // Both GitHub Models and Ollama use Bearer token format
                let auth_value = format!("Bearer {}", api_key);

                (url, body, auth_value)
            } else {
                // Anthropic API
                let url = format!("{}/messages", base_url);
                let claude_tools = mcp_tools_to_claude_tools(&tools);

                let body = serde_json::json!({
                    "model": model,
                    "max_tokens": 8192,
                    "system": system_prompt,
                    "messages": messages,
                    "tools": claude_tools,
                });

                (url, body, api_key.clone())
            };

            // Make API request
            let mut request_builder = http_client
                .post(&api_url)
                .header(&auth_header_name, &auth_header_value)
                .header("content-type", "application/json");

            // Add Anthropic-specific header
            if !is_openai_compatible {
                request_builder = request_builder.header("anthropic-version", "2023-06-01");
            }

            let response = match request_builder.json(&api_body).send().await {
                Ok(resp) => resp,
                Err(e) => {
                    yield Ok(Event::default()
                        .event("error")
                        .data(format!("API request failed: {}", e)));
                    break;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                let provider_name = match provider {
                    LlmProvider::Anthropic => "Claude",
                    LlmProvider::OpenAI => "OpenAI",
                    LlmProvider::Ollama => "Ollama",
                    LlmProvider::GitHub => "GitHub Models",
                };
                yield Ok(Event::default()
                    .event("error")
                    .data(format!("{} API error ({}): {}", provider_name, status, body)));
                break;
            }

            let resp_json: serde_json::Value = match response.json().await {
                Ok(json) => json,
                Err(e) => {
                    yield Ok(Event::default()
                        .event("error")
                        .data(format!("Failed to parse API response: {}", e)));
                    break;
                }
            };

            // Parse response based on API format
            let (text_content, tool_calls, should_continue) = if is_openai_compatible {
                // OpenAI format response
                let choice = &resp_json["choices"][0];
                let message = &choice["message"];
                let finish_reason = choice["finish_reason"].as_str().unwrap_or("stop");

                let text = message["content"].as_str().unwrap_or("").to_string();
                let tool_calls_arr = message["tool_calls"].as_array().cloned().unwrap_or_default();

                debug!(
                    "OpenAI response: finish_reason={}, tool_calls={}, content_len={}",
                    finish_reason,
                    tool_calls_arr.len(),
                    text.len()
                );

                // Convert OpenAI tool calls to our internal format
                let tools: Vec<serde_json::Value> = tool_calls_arr
                    .iter()
                    .map(|tc| {
                        let func = &tc["function"];
                        let args_str = func["arguments"].as_str().unwrap_or("{}");
                        let input: serde_json::Value = serde_json::from_str(args_str)
                            .unwrap_or(serde_json::json!({}));
                        debug!("Tool call: {} with args: {}", func["name"], args_str);
                        serde_json::json!({
                            "id": tc["id"],
                            "name": func["name"],
                            "input": input,
                        })
                    })
                    .collect();

                // Continue if there are tool calls, regardless of finish_reason
                // (some models/providers use different finish_reason values)
                let has_tool_calls = !tools.is_empty();
                (text, tools, has_tool_calls)
            } else {
                // Anthropic format response
                let stop_reason = resp_json["stop_reason"].as_str().unwrap_or("end_turn");
                let content_blocks = resp_json["content"].as_array().cloned().unwrap_or_default();

                let mut text_parts = Vec::new();
                let mut tool_uses = Vec::new();

                for block in &content_blocks {
                    match block["type"].as_str() {
                        Some("text") => {
                            if let Some(text) = block["text"].as_str() {
                                text_parts.push(text.to_string());
                            }
                        }
                        Some("tool_use") => {
                            tool_uses.push(serde_json::json!({
                                "id": block["id"],
                                "name": block["name"],
                                "input": block["input"],
                            }));
                        }
                        _ => {}
                    }
                }

                (text_parts.join(""), tool_uses, stop_reason == "tool_use")
            };

            // Send text content to frontend
            if !text_content.is_empty() {
                let json_text = serde_json::to_string(&text_content)
                    .unwrap_or_else(|_| format!("\"{}\"", text_content));
                yield Ok(Event::default()
                    .event("text")
                    .data(json_text));
            }

            // Send tool calls to frontend
            for tool_call in &tool_calls {
                yield Ok(Event::default()
                    .event("tool_use")
                    .data(serde_json::json!({
                        "id": tool_call["id"],
                        "name": tool_call["name"],
                        "input": tool_call["input"],
                    }).to_string()));
            }

            if !should_continue || tool_calls.is_empty() {
                // Done - no more tool calls
                yield Ok(Event::default()
                    .event("done")
                    .data(""));
                break;
            }

            // Execute tool calls via MCP and build tool results
            let mut tool_results = Vec::new();
            let mut anthropic_content_blocks: Vec<serde_json::Value> = Vec::new();

            // Add text block if present (for Anthropic format)
            if !text_content.is_empty() {
                anthropic_content_blocks.push(serde_json::json!({
                    "type": "text",
                    "text": text_content,
                }));
            }

            for tool_call in &tool_calls {
                let tool_name = tool_call["name"].as_str().unwrap_or("");
                let tool_id = tool_call["id"].as_str().unwrap_or("");
                let tool_input = tool_call["input"].as_object();

                // Add tool_use block for Anthropic format
                anthropic_content_blocks.push(serde_json::json!({
                    "type": "tool_use",
                    "id": tool_id,
                    "name": tool_name,
                    "input": tool_call["input"],
                }));

                let arguments = tool_input.cloned();

                let request = arguments.map_or_else(
                    || CallToolRequestParams::new(tool_name.to_string()),
                    |arguments| {
                        CallToolRequestParams::new(tool_name.to_string()).with_arguments(arguments)
                    },
                );

                match peer.call_tool(request).await {
                    Ok(result) => {
                        let result_text = extract_tool_result_text(&result);
                        let is_error = result.is_error.unwrap_or(false);

                        // Notify frontend about tool result
                        yield Ok(Event::default()
                            .event("tool_result")
                            .data(serde_json::json!({
                                "id": tool_id,
                                "name": tool_name,
                                "result": result_text,
                                "is_error": is_error,
                            }).to_string()));

                        tool_results.push(serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": tool_id,
                            "content": result_text,
                            "is_error": is_error,
                        }));
                    }
                    Err(e) => {
                        let error_msg = format!("Tool call failed: {}", e);

                        yield Ok(Event::default()
                            .event("tool_result")
                            .data(serde_json::json!({
                                "id": tool_id,
                                "name": tool_name,
                                "result": error_msg,
                                "is_error": true,
                            }).to_string()));

                        tool_results.push(serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": tool_id,
                            "content": error_msg,
                            "is_error": true,
                        }));
                    }
                }
            }

            // Append assistant message and tool results to conversation
            // We store in Anthropic format internally, convert to OpenAI when needed
            messages.push(serde_json::json!({
                "role": "assistant",
                "content": anthropic_content_blocks,
            }));
            messages.push(serde_json::json!({
                "role": "user",
                "content": tool_results,
            }));
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn run_torc_command(torc_bin: &str, args: &[&str], api_url: &str) -> CliResponse {
    info!("Running: {} {}", torc_bin, args.join(" "));

    let output = Command::new(torc_bin)
        .args(args)
        .env("TORC_API_URL", api_url)
        .output()
        .await;

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let success = output.status.success();

            if !success {
                warn!("Command failed: {} {}", torc_bin, args.join(" "));
                warn!("stderr: {}", stderr);
            }

            CliResponse {
                success,
                stdout,
                stderr,
                exit_code: output.status.code(),
            }
        }
        Err(e) => {
            error!("Failed to execute command: {}", e);
            CliResponse {
                success: false,
                stdout: String::new(),
                stderr: format!("Failed to execute command: {}", e),
                exit_code: None,
            }
        }
    }
}
