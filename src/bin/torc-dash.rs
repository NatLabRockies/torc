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
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use clap::parser::ValueSource;
use clap::{CommandFactory, FromArgMatches, Parser};
use rmcp::{ServiceExt, model::CallToolRequestParams, transport::child_process::TokioChildProcess};
use rust_embed::Embed;
use serde::{Deserialize, Serialize};
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
#[path = "torc-dash/ui/mod.rs"]
mod ui;

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
    /// Keeps the session alive: dropping the running service cancels it and closes the
    /// transport, so every later tool call would fail with "Transport closed".
    _service: rmcp::service::RunningService<rmcp::service::RoleClient, ()>,
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
    /// HTTP basic auth for torc-server: (username, password) from TORC_USERNAME/USER and
    /// TORC_PASSWORD, like the torc CLI. The torc CLI subprocesses inherit the same variables.
    server_auth: Option<(String, String)>,
    /// Hostnames requests may name in `Host` (lowercase, no port); None allows any.
    allowed_hosts: Option<Vec<String>>,
    /// Secret that API and proxy requests must carry (header or `token` query parameter). None
    /// on a UNIX socket, whose file permissions already restrict access, or with --no-token.
    token: Option<String>,
    /// PIDs of the live `torc run` processes started from the dashboard, by workflow id, so they
    /// can be stopped after the browser stops following them. A workflow can have several.
    local_runs: Mutex<std::collections::HashMap<String, Vec<u32>>>,
}

/// Header carrying the dashboard token. Not `Authorization`, which the proxy passes through to
/// torc-server as the user's own login.
const TOKEN_HEADER: &str = "x-torc-dash-token";

/// Whether `path` reaches torc-dash's API or the torc-server proxy, rather than the static UI.
fn is_api_path(path: &str) -> bool {
    path.starts_with("/api/") || path.starts_with("/torc-service/") || path.starts_with("/ui/")
}

/// Splits a `token` parameter out of a query string: (token, remaining query).
fn take_token_param(query: &str) -> (Option<String>, String) {
    let mut token = None;
    let rest: Vec<&str> = query
        .split('&')
        .filter(
            |kv| match url::form_urlencoded::parse(kv.as_bytes()).next() {
                Some((k, v)) if k == "token" => {
                    token = Some(v.into_owned());
                    false
                }
                _ => !kv.is_empty(),
            },
        )
        .collect();
    (token, rest.join("&"))
}

/// Compares secrets without exiting at the first differing byte.
fn same_secret(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// Hostnames allowed in the `Host` header: loopback names, the bind host when it is a specific
/// name, and `extra`. None (no check) if `extra` contains "*".
fn allowed_host_list(bind_host: &str, extra: &[String]) -> Option<Vec<String>> {
    let mut hosts: Vec<String> = ["localhost", "127.0.0.1", "[::1]"]
        .map(String::from)
        .to_vec();
    let bind = bind_host.trim().to_lowercase();
    if !matches!(bind.as_str(), "" | "0.0.0.0" | "::" | "[::]") {
        hosts.push(if bind.contains(':') && !bind.starts_with('[') {
            format!("[{bind}]")
        } else {
            bind
        });
    }
    for h in extra.iter().map(|h| h.trim().to_lowercase()) {
        if h == "*" {
            return None;
        }
        if !h.is_empty() {
            hosts.push(h);
        }
    }
    Some(hosts)
}

/// Whether to serve a request, given its `Host` and `Sec-Fetch-Site` headers.
///
/// - `Host` must be an allowed name. A DNS-rebinding page reaches the dashboard under its own
///   hostname, so this keeps it out even though the browser considers it same-origin.
/// - API and proxy routes refuse requests the browser marks as coming from another site
///   (`Sec-Fetch-Site` other than same-origin or none). That covers requests a hostile page can
///   make without a CORS preflight, such as a GET of `/api/cli/run-stream`. UI routes are exempt:
///   opening the dashboard is a navigation, which Torc Desktop makes from its own `tauri://` page.
///   Clients that don't send the header (curl, Torc Desktop's readiness probe) are unaffected.
fn check_request(
    host: Option<&str>,
    sec_fetch_site: Option<&str>,
    path: &str,
    allowed: Option<&[String]>,
) -> Result<(), String> {
    if let (Some(allowed), Some(host)) = (allowed, host) {
        let host = host.trim().to_lowercase();
        // Drop the port: "[::1]:8090" -> "[::1]", "localhost:8090" -> "localhost".
        let name = match host.rfind(':') {
            Some(i) if !host[i..].contains(']') => &host[..i],
            _ => host.as_str(),
        };
        if !allowed.iter().any(|a| a == name) {
            return Err(format!(
                "Host '{name}' is not allowed. To reach the dashboard by this name, start \
                 torc-dash with --allowed-hosts {name}"
            ));
        }
    }
    if is_api_path(path) && !matches!(sec_fetch_site, None | Some("same-origin") | Some("none")) {
        return Err("Cross-site requests are not allowed".to_string());
    }
    Ok(())
}

/// Middleware applying [`check_request`] to every request, and requiring the dashboard token on
/// API and proxy routes. Anyone who can connect to a TCP port (on a shared login node, every
/// user) would otherwise act with this user's torc-server login and files.
async fn guard_requests(
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
    next: axum::middleware::Next,
) -> Response {
    // Scoped so no borrow of the request (whose body isn't Sync) lives across the await.
    let verdict = {
        let header = |name: &str| req.headers().get(name).and_then(|v| v.to_str().ok());
        check_request(
            header("host"),
            header("sec-fetch-site"),
            req.uri().path(),
            state.allowed_hosts.as_deref(),
        )
    };
    if let Err(message) = verdict {
        warn!("Refused {} {}: {}", req.method(), req.uri().path(), message);
        return (StatusCode::FORBIDDEN, message).into_response();
    }
    if let Some(expected) = &state.token
        && is_api_path(req.uri().path())
    {
        // EventSource cannot set headers, so streams pass the token as a query parameter.
        // Strip it so it is never forwarded to torc-server.
        let (from_query, rest) = take_token_param(req.uri().query().unwrap_or(""));
        let given = req
            .headers()
            .get(TOKEN_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(String::from)
            .or(from_query)
            .or_else(|| ui::token_from_cookie(req.headers(), expected));
        if !given.is_some_and(|t| same_secret(&t, expected)) {
            warn!(
                "Refused {} {}: missing or wrong token",
                req.method(),
                req.uri().path()
            );
            let message = "Missing or wrong dashboard token. Open the dashboard with the URL \
                           torc-dash printed at startup (it ends in ?token=...).";
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "message": message })),
            )
                .into_response();
        }
        if req.uri().query().is_some() {
            let path_and_query = if rest.is_empty() {
                req.uri().path().to_string()
            } else {
                format!("{}?{}", req.uri().path(), rest)
            };
            if let Ok(uri) = path_and_query.parse() {
                *req.uri_mut() = uri;
            }
        }
        req.headers_mut().remove(TOKEN_HEADER);
    }
    next.run(req).await
}

/// Adds the torc-server login to a request bound for torc-server (never for other hosts, such
/// as the AI chat provider, which share the same HTTP client).
fn with_server_auth(
    req: reqwest::RequestBuilder,
    auth: &Option<(String, String)>,
) -> reqwest::RequestBuilder {
    match auth {
        Some((user, password)) => req.basic_auth(user, Some(password)),
        None => req,
    }
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

    /// Extra hostnames the dashboard may be reached by, comma-separated (e.g. a reverse proxy's
    /// name). Requests for other hosts are refused, which blocks DNS-rebinding attacks.
    /// localhost, 127.0.0.1, ::1, and a specific --host are always allowed; "*" allows any.
    #[arg(long, env = "TORC_DASH_ALLOWED_HOSTS", value_delimiter = ',')]
    allowed_hosts: Vec<String>,

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

    /// Host for torc-server to bind to in standalone mode (default: 0.0.0.0 for external access,
    /// or 127.0.0.1 with --socket)
    #[arg(long, default_value = "0.0.0.0")]
    server_host: String,

    /// Database path for torc-server when running in standalone mode
    #[arg(long, env = "DATABASE_URL")]
    database: Option<String>,

    /// Deprecated and ignored: torc-server processes job completions as they arrive.
    #[arg(long, hide = true)]
    completion_check_interval_secs: Option<u32>,

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

    /// Override the Claude API base URL, e.g. a LiteLLM gateway. Accepts the Claude Code form
    /// ("https://gateway.example.com") or one ending in /v1; /v1/messages is used either way.
    #[arg(long, env = "ANTHROPIC_BASE_URL")]
    anthropic_base_url: Option<String>,

    /// Bearer token for the Claude API, sent as "Authorization: Bearer <token>" (as Claude Code
    /// does). Used when ANTHROPIC_API_KEY is not set; typical for LLM gateways such as LiteLLM.
    #[arg(long, env = "ANTHROPIC_AUTH_TOKEN", hide_env_values = true)]
    anthropic_auth_token: Option<String>,

    /// Override the auth header name (default: "x-api-key")
    #[arg(long, env = "ANTHROPIC_AUTH_HEADER")]
    anthropic_auth_header: Option<String>,

    /// Model to use for AI chat (default: claude-opus-5-5)
    #[arg(long, default_value = "claude-opus-5-5", env = "ANTHROPIC_MODEL")]
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

    /// Serve the API without a token. On TCP, torc-dash otherwise requires a token (from
    /// TORC_DASH_TOKEN, or generated and printed in the startup URL), because anyone who can
    /// connect could act as you. Use only behind a proxy that authenticates users itself.
    #[arg(long)]
    no_token: bool,
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

    let matches = Cli::command().get_matches();
    let mut cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    // Load configuration from files and merge with CLI arguments
    let file_config = TorcConfig::load().unwrap_or_default();
    let dash_config = &file_config.dash;

    // Merge CLI config with file config. A value from the command line or its environment
    // variable wins, even when it equals the clap default.
    let explicit = |id: &str| {
        matches
            .value_source(id)
            .is_some_and(|source| source != ValueSource::DefaultValue)
    };
    let host = if explicit("host") {
        cli.host.clone()
    } else {
        dash_config.host.clone()
    };
    let port = if explicit("port") {
        cli.port
    } else {
        dash_config.port
    };
    let api_url = if explicit("api_url") {
        cli.api_url.clone()
    } else {
        dash_config.api_url.clone()
    };
    // Some commands run in the workflow's submission directory, so a relative path like
    // ./target/release/torc must not be resolved from there.
    let torc_bin = absolute_if_relative_path(if explicit("torc_bin") {
        cli.torc_bin.clone()
    } else {
        dash_config.torc_bin.clone()
    });
    let torc_server_bin = if explicit("torc_server_bin") {
        cli.torc_server_bin.clone()
    } else {
        dash_config.torc_server_bin.clone()
    };
    let standalone = cli.standalone || dash_config.standalone;
    let server_port = if explicit("server_port") {
        cli.server_port
    } else {
        dash_config.server_port
    };
    #[cfg(unix)]
    let socket_path = cli
        .socket
        .clone()
        .or_else(|| dash_config.socket.as_ref().map(std::path::PathBuf::from));
    #[cfg(unix)]
    let private = socket_path.is_some();
    #[cfg(not(unix))]
    let private = false;
    let server_host = if explicit("server_host") {
        cli.server_host.clone()
    } else if private {
        // A dashboard on a private socket must not put its managed server on the network.
        "127.0.0.1".to_string()
    } else {
        dash_config.server_host.clone()
    };
    let database = cli
        .database
        .clone()
        .or_else(|| dash_config.database.clone());
    if cli.completion_check_interval_secs.is_some()
        || dash_config.completion_check_interval_secs.is_some()
    {
        warn!(
            "completion_check_interval_secs is deprecated and ignored: torc-server now \
             processes job completions as they arrive. Remove it from your torc config file \
             (see 'torc config paths') or the command line"
        );
    }

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
                        let mut details = if stderr_output.is_empty() {
                            format!("torc-server exited with status {}", status)
                        } else {
                            format!(
                                "torc-server exited with status {}: {}",
                                status, stderr_output
                            )
                        };
                        if database.is_none() {
                            details.push_str(
                                "\nHint: no database was given; pass --database <path> to \
                                 torc-dash (e.g. --database $HOME/torc.db)",
                            );
                        }
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

    apply_chat_config(&mut cli, &dash_config.chat, &explicit);

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
                        base_url = anthropic_api_base(url_override);
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
                } else if let Some(api_key) = cli
                    .anthropic_api_key
                    .as_ref()
                    .or(cli.anthropic_auth_token.as_ref())
                {
                    // An API key goes in x-api-key; an auth token is a bearer token (Claude Code's
                    // ANTHROPIC_AUTH_TOKEN), which gateways like LiteLLM expect.
                    let default_header = if cli.anthropic_api_key.is_some() {
                        "x-api-key"
                    } else {
                        "Authorization"
                    };
                    let base_url = cli
                        .anthropic_base_url
                        .as_deref()
                        .map(anthropic_api_base)
                        .unwrap_or_else(|| "https://api.anthropic.com/v1".to_string());
                    info!("AI Chat: using Claude API at {}", base_url);
                    let auth_header = cli
                        .anthropic_auth_header
                        .clone()
                        .unwrap_or_else(|| default_header.to_string());
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

    let server_auth = std::env::var("TORC_PASSWORD")
        .ok()
        .filter(|p| !p.is_empty())
        .map(|p| (torc::get_username(), p));
    if let Some((user, _)) = &server_auth {
        info!("Authenticating to torc-server as {}", user);
    }
    let allowed_hosts = allowed_host_list(
        &host,
        if explicit("allowed_hosts") {
            &cli.allowed_hosts
        } else {
            &dash_config.allowed_hosts
        },
    );
    if allowed_hosts.is_none() {
        warn!("Host check disabled (--allowed-hosts '*'); the dashboard is open to DNS rebinding");
    }

    // Read from the environment only: a command-line value would be visible to other users in ps.
    let token_from_env = std::env::var("TORC_DASH_TOKEN")
        .ok()
        .filter(|t| !t.trim().is_empty());
    let token = if private {
        None
    } else if cli.no_token {
        warn!("Token check disabled (--no-token); anyone who can connect can use the dashboard");
        None
    } else {
        Some(
            token_from_env
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string()),
        )
    };
    // Shown in the startup URL only when generated; a token from the environment stays secret.
    let url_token_suffix = match (&token, &token_from_env) {
        (Some(t), None) => format!("/?token={t}"),
        _ => String::new(),
    };

    let state = Arc::new(AppState {
        token,
        server_auth,
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
        allowed_hosts,
        local_runs: Mutex::new(std::collections::HashMap::new()),
    });

    // Build router
    let app = Router::new()
        // Static files and dashboard
        .route("/", get(ui::index_handler))
        .route("/static/{*path}", get(static_handler))
        .merge(ui::routes())
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
        .route("/api/cli/reset-jobs", post(cli_reset_jobs_handler))
        .route("/api/cli/execution-plan", post(cli_execution_plan_handler))
        .route("/api/cli/run-stream", get(cli_run_stream_handler))
        .route("/api/cli/recover", post(cli_recover_handler))
        .route("/api/cli/sync-status", post(cli_sync_status_handler))
        .route("/api/cli/op", post(cli_op_handler))
        .route("/api/cli/stop-run", post(cli_stop_run_handler))
        .route("/api/download/export", get(download_export_handler))
        .route("/api/download/logs", get(download_logs_handler))
        .route("/api/cli/import", post(cli_import_handler))
        .route("/api/cli/read-file", post(cli_read_file_handler))
        .route("/api/cli/write-file", post(cli_write_file_handler))
        .route("/api/cli/tail-file", post(cli_tail_file_handler))
        .route(
            "/api/cli/resource-samples",
            post(cli_resource_samples_handler),
        )
        .route("/api/workflow-schema", get(workflow_schema_handler))
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
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            guard_requests,
        ))
        .with_state(state.clone());

    // Stop the managed torc-server when we are told to exit, so it does not outlive us.
    let shutdown = async move {
        shutdown_signal().await;
        if let Some(pid) = state.managed_server.lock().await.pid.take() {
            info!("Stopping managed torc-server (PID {})", pid);
            stop_process(pid);
        }
    };

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

        tokio::select! {
            r = axum::serve(uds, app).into_future() => r?,
            () = shutdown => {}
        }
        return Ok(());
    }

    // TCP path (default)
    let (std_listener, actual_port) = find_available_port(&host, port)?;
    info!(
        "Dashboard available at http://{}:{}{}",
        host, actual_port, url_token_suffix
    );
    if actual_port != port {
        info!(
            "Note: Requested port {} was in use, using port {} instead",
            port, actual_port
        );
    }

    // Convert std listener to tokio listener for axum
    std_listener.set_nonblocking(true)?;
    let listener = tokio::net::TcpListener::from_std(std_listener)?;

    tokio::select! {
        r = axum::serve(listener, app).into_future() => r?,
        () = shutdown => {}
    }

    Ok(())
}

/// Resolves on Ctrl-C, or on SIGTERM/SIGHUP on Unix.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate()).expect("install SIGTERM handler");
        let mut hup = signal(SignalKind::hangup()).expect("install SIGHUP handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
            _ = hup.recv() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}

fn stop_process(pid: u32) {
    #[cfg(unix)]
    let _ = StdCommand::new("kill").arg(pid.to_string()).status();
    #[cfg(not(unix))]
    let _ = StdCommand::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status();
}

/// `bin` made absolute against the current directory if it is a relative path (`./torc`,
/// `bin/torc`). A bare name like `torc` is left alone so it is still found on PATH.
fn absolute_if_relative_path(bin: String) -> String {
    let path = std::path::Path::new(&bin);
    if path.is_relative()
        && path.components().count() > 1
        && let Ok(abs) = std::path::absolute(path)
    {
        return abs.to_string_lossy().into_owned();
    }
    bin
}

/// Stops the process with this PID when dropped, unless cleared first.
struct KillOnDrop(Option<u32>);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if let Some(pid) = self.0 {
            stop_process(pid);
        }
    }
}

// ============== Static File Handlers ==============

/// The dashboard UI, built from torc-dash/web into torc-dash/static/app/.
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
    // A login the browser sent itself takes precedence.
    let browser_auth = req.headers().contains_key(header::AUTHORIZATION);
    if !browser_auth {
        proxy_req = with_server_auth(proxy_req, &state.server_auth);
    }

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
            if status == StatusCode::UNAUTHORIZED && !browser_auth {
                // Say why, instead of the server's bare "Unauthorized".
                let message = unauthorized_message(&state, &target_url, resp.url().as_str());
                warn!("{}", message);
                return (status, Json(serde_json::json!({ "message": message }))).into_response();
            }
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

/// Explains a 401 from torc-server so the user can tell a missing login from a rejected one.
fn unauthorized_message(state: &AppState, requested: &str, final_url: &str) -> String {
    let Some((user, _)) = &state.server_auth else {
        return "torc-server requires a login, and torc-dash has none. Set TORC_PASSWORD \
                (and TORC_USERNAME if your server user name is not your login name) where \
                torc-dash runs, or enter the password in Torc Desktop."
            .to_string();
    };
    let redirected = if final_url != requested {
        format!(
            " The request was redirected to {final_url}; the login is not sent across a \
             redirect to another host or scheme, so use that URL as the server URL."
        )
    } else {
        String::new()
    };
    format!(
        "torc-server rejected the login for user '{user}'. Check the password, or set \
         TORC_USERNAME if your server user name differs.{redirected}"
    )
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

#[derive(Serialize, Deserialize)]
struct CliResponse {
    success: bool,
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
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
        let spec = expand_tilde(&req.spec).to_string_lossy().into_owned();
        run_torc_command(
            &state.torc_bin,
            &["-f", "json", "create", &spec],
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
        let spec = expand_tilde(&req.spec).to_string_lossy().into_owned();
        let slurm_spec_path = format!(
            "{}_slurm.yaml",
            spec.trim_end_matches(".yaml")
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
        gen_args.push(&spec);

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
        let spec = expand_tilde(&req.spec).to_string_lossy().into_owned();
        run_torc_command(
            &state.torc_bin,
            &["-f", "json", "create", &spec, "--dry-run"],
            &state.api_url,
        )
        .await
    } else {
        let extension = req.file_extension.as_deref().unwrap_or(".json");
        dry_run_spec(&state.torc_bin, &state.api_url, &req.spec, extension).await
    };

    Json(result)
}

async fn cli_run_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WorkflowIdRequest>,
) -> impl IntoResponse {
    let result =
        run_torc_command(&state.torc_bin, &["run", &req.workflow_id], &state.api_url).await;
    Json(result)
}

async fn cli_submit_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WorkflowIdRequest>,
) -> impl IntoResponse {
    let result = run_torc_command(
        &state.torc_bin,
        &["submit", "--no-prompts", &req.workflow_id],
        &state.api_url,
    )
    .await;
    Json(result)
}

async fn cli_initialize_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<InitializeRequest>,
) -> impl IntoResponse {
    // --no-prompts: the CLI otherwise asks on stdin before re-initializing, reads nothing from
    // the subprocess, and cancels while exiting 0.
    let mut args = vec!["workflows", "init", "--no-prompts", &req.workflow_id];
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
        &["delete", "--no-prompts", &req.workflow_id],
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
        &["cancel", "--no-prompts", &req.workflow_id],
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

#[derive(Deserialize)]
struct ResetJobsRequest {
    job_ids: Vec<i64>,
}

/// Reset selected jobs (and, via reinit, their dependents) so they run again.
async fn cli_reset_jobs_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ResetJobsRequest>,
) -> impl IntoResponse {
    let ids: Vec<String> = req.job_ids.iter().map(i64::to_string).collect();
    info!("Resetting jobs job_ids={}", ids.join(","));
    let mut args = vec!["jobs", "reset-status", "--no-prompts", "--reinit"];
    args.extend(ids.iter().map(String::as_str));
    Json(run_torc_command(&state.torc_bin, &args, &state.api_url).await)
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

/// Streams `torc run <id>`, or with `mode=watch` / `mode=watch-recover`, `torc watch <id>`,
/// as Server-Sent Events.
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

    let mode = params.get("mode").map_or("run", String::as_str);
    let (args, verb): (Vec<&str>, &str) = match mode {
        "run" => (vec!["run", &workflow_id], "Running"),
        "watch" => (vec!["watch", &workflow_id, "--show-job-counts"], "Watching"),
        "watch-recover" => (
            vec!["watch", &workflow_id, "--show-job-counts", "--recover"],
            "Watching (with recovery)",
        ),
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                "mode must be run, watch, or watch-recover",
            ));
        }
    };
    let args: Vec<String> = args.into_iter().map(String::from).collect();
    let start_msg = format!("{verb} workflow {workflow_id}");
    // A watch only observes, so stop it when the browser stops listening. A run keeps going.
    let kill_on_drop = mode != "run";
    info!("Starting streaming {} workflow_id={}", mode, workflow_id);

    let torc_bin = state.torc_bin.clone();
    let api_url = state.api_url.clone();
    let http_client = state.client.clone();
    let server_auth = state.server_auth.clone();

    // Create the stream
    let stream = async_stream::stream! {
        // Send start event
        yield Ok::<_, std::convert::Infallible>(Event::default()
            .event("start")
            .data(start_msg));

        // Spawn the command with piped stdout/stderr
        let mut child = match Command::new(&torc_bin)
            .args(&args)
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

        // Wait for the process in its own task, so a run outlives this stream (the browser may
        // leave) and stays stoppable through `local_runs` until it exits.
        let pid = child.id();
        if let (false, Some(pid)) = (kill_on_drop, pid) {
            state
                .local_runs
                .lock()
                .await
                .entry(workflow_id.clone())
                .or_default()
                .push(pid);
        }
        let (exit_tx, exit_rx) = tokio::sync::oneshot::channel();
        {
            let state = state.clone();
            let workflow_id = workflow_id.clone();
            tokio::spawn(async move {
                let status = child.wait().await;
                let mut runs = state.local_runs.lock().await;
                if let (Some(pid), Some(pids)) = (pid, runs.get_mut(&workflow_id)) {
                    pids.retain(|&p| p != pid);
                    if pids.is_empty() {
                        runs.remove(&workflow_id);
                    }
                }
                drop(runs);
                let _ = exit_tx.send(status);
            });
        }
        // A watch only observes, so stop it if the browser stops listening.
        let mut watch_guard = KillOnDrop(if kill_on_drop { pid } else { None });

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
                // The browser left; stop even if the fetches below keep failing.
                if status_tx.is_closed() {
                    break;
                }

                // Fetch jobs from API and count statuses
                let url = format!("{}/jobs?workflow_id={}&limit={}", api_url_status, workflow_id_status, torc::MAX_RECORD_TRANSFER_COUNT);
                match with_server_auth(http_client_status.get(&url), &server_auth).send().await {
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
                                    // The API sends status names ("ready"); older servers sent integers.
                                    let status = job.get("status");
                                    match status.and_then(|v| v.as_str()).or_else(|| {
                                        const NAMES: [&str; 10] = ["uninitialized", "blocked", "ready", "pending", "running", "completed", "failed", "canceled", "terminated", "disabled"];
                                        status.and_then(|v| v.as_u64()).and_then(|i| NAMES.get(i as usize).copied())
                                    }) {
                                        Some("uninitialized") => uninitialized += 1,
                                        Some("blocked") => blocked += 1,
                                        Some("ready") => ready += 1,
                                        Some("pending") => pending += 1,
                                        Some("running") => running += 1,
                                        Some("completed") => completed += 1,
                                        Some("failed") => failed += 1,
                                        Some("canceled") => canceled += 1,
                                        Some("terminated") => terminated += 1,
                                        Some("disabled") => disabled += 1,
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
        let status = exit_rx
            .await
            .unwrap_or_else(|_| Err(std::io::Error::other("process waiter stopped")));
        watch_guard.0 = None;
        match status {
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

/// Run `torc -f json <args>` and wrap the result in the envelope the dashboard expects:
/// `{success, data, stdout, stderr, exit_code}`, plus `error` on failure. `data` is the parsed
/// stdout, or null when it is not JSON. `cwd` sets the working directory (e.g., the workflow's
/// submission directory, so relative paths resolve as they would for the user).
async fn run_torc_json(state: &AppState, args: &[String], cwd: Option<&str>) -> serde_json::Value {
    let mut full = vec!["-f".to_string(), "json".to_string()];
    full.extend_from_slice(args);
    info!("Running: {} {}", state.torc_bin, full.join(" "));
    let mut cmd = Command::new(&state.torc_bin);
    cmd.args(&full).env("TORC_API_URL", &state.api_url);
    if let Some(dir) = cwd {
        cmd.current_dir(expand_tilde(dir));
    }
    let output = match cmd.output().await {
        Ok(output) => output,
        Err(e) => {
            return serde_json::json!({
                "success": false,
                "error": format!("Failed to execute torc: {}", e),
            });
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let data = serde_json::from_str::<serde_json::Value>(stdout.trim()).ok();
    let mut body = serde_json::json!({
        "success": output.status.success(),
        "data": data,
        "stdout": stdout,
        "stderr": stderr,
        "exit_code": output.status.code(),
    });
    if !output.status.success() {
        warn!("Command failed: {} {}", state.torc_bin, full.join(" "));
        let error = data
            .as_ref()
            .and_then(|d| d.get("error"))
            .and_then(|e| e.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| {
                if stderr.trim().is_empty() {
                    stdout.trim().to_string()
                } else {
                    stderr.trim().to_string()
                }
            });
        body["error"] = serde_json::Value::String(error);
    }
    body
}

/// Workflow recovery handler: `torc -f json recover`, with --dry-run to preview changes.
async fn cli_recover_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RecoverRequest>,
) -> impl IntoResponse {
    info!(
        "Running recover workflow_id={} dry_run={}",
        req.workflow_id, req.dry_run
    );
    let mut args = vec![
        "recover".to_string(),
        req.workflow_id.clone(),
        "-o".to_string(),
        expand_tilde(&req.output_dir).to_string_lossy().into_owned(),
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
    Json(run_torc_json(&state, &args, None).await)
}

/// Sync job statuses with Slurm — detect and fail orphaned running jobs
/// Runs `torc -f json workflows sync-status <workflow_id>` with optional --dry-run
async fn cli_sync_status_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SyncStatusRequest>,
) -> impl IntoResponse {
    info!(
        "Running sync-status workflow_id={} dry_run={}",
        req.workflow_id, req.dry_run
    );
    let mut args = vec![
        "workflows".to_string(),
        "sync-status".to_string(),
        req.workflow_id.clone(),
    ];
    if req.dry_run {
        args.push("--dry-run".to_string());
    }
    Json(run_torc_json(&state, &args, None).await)
}

/// A dashboard operation that maps to one `torc` command. `params` holds the op's options;
/// each op reads only the keys it knows, so the browser cannot pass arbitrary CLI arguments.
#[derive(Deserialize)]
struct OpRequest {
    op: String,
    #[serde(default)]
    workflow_id: Option<i64>,
    #[serde(flatten)]
    params: serde_json::Map<String, serde_json::Value>,
}

/// The `torc` arguments for an [`OpRequest`].
struct OpCommand {
    args: Vec<String>,
    /// Working directory for the command, if not torc-dash's own.
    cwd: Option<String>,
    /// Spec written for the command to read; deleted afterwards.
    temp_file: Option<String>,
}

/// Run one whitelisted `torc` command and return its JSON envelope (see `run_torc_json`).
async fn cli_op_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<OpRequest>,
) -> impl IntoResponse {
    match build_op(&req) {
        Ok(cmd) => {
            let result = run_torc_json(&state, &cmd.args, cmd.cwd.as_deref()).await;
            if let Some(path) = cmd.temp_file {
                let _ = tokio::fs::remove_file(path).await;
            }
            Json(result)
        }
        Err(error) => Json(serde_json::json!({ "success": false, "error": error })),
    }
}

fn build_op(req: &OpRequest) -> Result<OpCommand, String> {
    let p = &req.params;
    let flag = |k: &str| p.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
    let num = |k: &str| p.get(k).and_then(|v| v.as_f64());
    let int = |k: &str| p.get(k).and_then(|v| v.as_i64());
    let text = |k: &str| {
        p.get(k)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let id = || {
        req.workflow_id
            .map(|i| i.to_string())
            .ok_or_else(|| "workflow_id is required".to_string())
    };
    let s = |v: &str| v.to_string();
    let mut cmd = OpCommand {
        args: Vec::new(),
        cwd: None,
        temp_file: None,
    };

    cmd.args = match req.op.as_str() {
        "diagnose" => vec![s("workflows"), s("diagnose"), id()?],
        "list-actions" => vec![s("workflows"), s("list-actions"), id()?],
        "slurm-usage" => vec![s("slurm"), s("usage"), id()?],
        "check-resources" => {
            let mut a = vec![s("workflows"), s("check-resources"), id()?];
            if flag("all") {
                a.push(s("--all"));
            }
            if flag("include_failed") {
                a.push(s("--include-failed"));
            }
            a
        }
        "correct-resources" => {
            let mut a = vec![s("workflows"), s("correct-resources"), id()?];
            for (key, opt) in [
                ("memory_multiplier", "--memory-multiplier"),
                ("cpu_multiplier", "--cpu-multiplier"),
                ("runtime_multiplier", "--runtime-multiplier"),
            ] {
                if let Some(m) = num(key) {
                    a.extend([s(opt), m.to_string()]);
                }
            }
            if flag("dry_run") {
                a.push(s("--dry-run"));
            }
            if flag("no_downsize") {
                a.push(s("--no-downsize"));
            }
            a
        }
        "archive" => {
            let ids: Vec<String> = p
                .get("workflow_ids")
                .and_then(|v| v.as_array())
                .map(|ids| {
                    ids.iter()
                        .filter_map(|i| i.as_i64())
                        .map(|i| i.to_string())
                        .collect()
                })
                .unwrap_or_default();
            if ids.is_empty() {
                return Err("workflow_ids is required".to_string());
            }
            let mut a = vec![s("workflows"), s("archive"), flag("archived").to_string()];
            a.extend(ids);
            a
        }
        "update-workflow" => {
            let mut a = vec![s("workflows"), s("update"), id()?];
            if let Some(name) = text("name") {
                a.extend([s("--name"), name]);
            }
            // Description and project may be cleared, so take them verbatim.
            for (key, opt) in [("description", "--description"), ("project", "--project")] {
                if let Some(v) = p.get(key).and_then(|v| v.as_str()) {
                    a.extend([s(opt), v.to_string()]);
                }
            }
            a
        }
        "schedule-nodes" => {
            let scheduler = int("scheduler_config_id")
                .ok_or_else(|| "scheduler_config_id is required".to_string())?;
            let mut a = vec![
                s("slurm"),
                s("schedule-nodes"),
                id()?,
                s("--scheduler-config-id"),
                scheduler.to_string(),
                s("--num-hpc-jobs"),
                int("num_allocations").unwrap_or(1).max(1).to_string(),
                s("--no-prompts"),
            ];
            if let Some(dir) = text("output_dir") {
                a.extend([
                    s("--output"),
                    expand_tilde(&dir).to_string_lossy().into_owned(),
                ]);
            }
            if let Some(m) = int("max_parallel_jobs") {
                a.extend([s("--max-parallel-jobs"), m.to_string()]);
            }
            if flag("suppress_actions") {
                a.push(s("--suppress-actions"));
            }
            if flag("start_one_worker_per_node") {
                a.push(s("--start-one-worker-per-node"));
            }
            // sbatch inherits the working directory; use the submission directory so jobs run
            // where the workflow was created.
            cmd.cwd = text("cwd");
            a
        }
        "plan-allocations" => {
            // The spec is a file on this host (`spec_path`) or inline content (`spec`).
            let path = if let Some(path) = text("spec_path") {
                expand_tilde(&path).to_string_lossy().into_owned()
            } else {
                let spec = p
                    .get("spec")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "spec or spec_path is required".to_string())?;
                let raw_ext = text("file_extension").unwrap_or_else(|| s(".yaml"));
                let ext = validate_file_extension(&raw_ext)
                    .ok_or_else(|| format!("Invalid file extension '{}'", raw_ext))?;
                // Same location `create` uses, so relative paths in the spec resolve alike.
                let path = format!("torc_spec_{}{}", uuid::Uuid::new_v4(), ext);
                std::fs::write(&path, spec).map_err(|e| format!("Failed to write spec: {}", e))?;
                cmd.temp_file = Some(path.clone());
                path
            };
            let mut a = vec![s("slurm"), s("plan-allocations"), path];
            if let Some(account) = text("account") {
                a.extend([s("--account"), account]);
            }
            if let Some(profile) = text("profile") {
                a.extend([s("--profile"), profile]);
            }
            // Without live sinfo/squeue, e.g. when planning for another cluster's profile.
            if flag("offline") {
                a.push(s("--offline"));
            }
            a
        }
        "list-access-groups" => vec![s("access-groups"), s("list"), s("--limit"), s("1000")],
        "list-workflow-groups" => vec![s("access-groups"), s("list-workflow-groups"), id()?],
        "add-workflow-group" | "remove-workflow-group" => {
            let group = int("group_id").ok_or_else(|| "group_id is required".to_string())?;
            let verb = if req.op == "add-workflow-group" {
                "add-workflow"
            } else {
                "remove-workflow"
            };
            vec![s("access-groups"), s(verb), id()?, group.to_string()]
        }
        other => return Err(format!("Unknown operation '{}'", other)),
    };
    Ok(cmd)
}

#[derive(Deserialize)]
struct DownloadQuery {
    workflow_id: i64,
    #[serde(default)]
    include_results: bool,
    #[serde(default)]
    include_events: bool,
    /// Log directory for the bundle (absolute, or relative to torc-dash's working directory).
    #[serde(default)]
    output_dir: Option<String>,
    /// Suggested file name for the export.
    #[serde(default)]
    filename: Option<String>,
}

/// `Content-Disposition` for a download, keeping only safe file-name characters.
fn attachment(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("attachment; filename=\"{}\"", safe)
}

fn download_error(msg: String) -> axum::response::Response {
    (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
}

/// Download `torc workflows export` as a JSON file (to the browser, not the torc-dash host).
async fn download_export_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<DownloadQuery>,
) -> axum::response::Response {
    let id = q.workflow_id.to_string();
    let mut args = vec!["workflows", "export", &id];
    if q.include_results {
        args.push("--include-results");
    }
    if q.include_events {
        args.push("--include-events");
    }
    let result = run_torc_command(&state.torc_bin, &args, &state.api_url).await;
    if !result.success {
        return download_error(result.stderr);
    }
    let name = q
        .filename
        .unwrap_or_else(|| format!("workflow_{}.json", q.workflow_id));
    (
        [
            (header::CONTENT_TYPE, "application/json".to_string()),
            (header::CONTENT_DISPOSITION, attachment(&name)),
        ],
        result.stdout,
    )
        .into_response()
}

/// Download `torc logs bundle` (a .tar.gz of the workflow's logs) without leaving a copy behind.
async fn download_logs_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<DownloadQuery>,
) -> axum::response::Response {
    let dir = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(e) => return download_error(format!("Failed to create temp dir: {}", e)),
    };
    let id = q.workflow_id.to_string();
    let output_dir = expand_tilde(q.output_dir.as_deref().unwrap_or("torc_output"))
        .to_string_lossy()
        .into_owned();
    let bundle_dir = dir.path().to_string_lossy().into_owned();
    let args = [
        "logs",
        "bundle",
        &id,
        "--output-dir",
        &output_dir,
        "--bundle-dir",
        &bundle_dir,
    ];
    let result = run_torc_command(&state.torc_bin, &args, &state.api_url).await;
    let name = format!("wf{}.tar.gz", q.workflow_id);
    let file = match tokio::fs::File::open(dir.path().join(&name)).await {
        Ok(file) if result.success => file,
        Ok(_) => return download_error(result.stderr),
        Err(e) => return download_error(format!("{}\n{}", result.stderr, e)),
    };
    // Stream the file; the stream owns the temp dir, which is removed when it finishes.
    let stream = async_stream::stream! {
        let _dir = dir;
        let mut file = file;
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            match file.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => yield Ok::<_, std::io::Error>(axum::body::Bytes::copy_from_slice(&buf[..n])),
                Err(e) => {
                    yield Err(e);
                    break;
                }
            }
        }
    };
    (
        [
            (header::CONTENT_TYPE, "application/gzip".to_string()),
            (header::CONTENT_DISPOSITION, attachment(&name)),
        ],
        axum::body::Body::from_stream(stream),
    )
        .into_response()
}

#[derive(Deserialize)]
struct StopRunRequest {
    workflow_id: String,
}

/// Stop a `torc run` started from the dashboard. SIGTERM makes the job runner terminate its
/// jobs and exit, as Slurm does at walltime. Windows has no such signal: `taskkill /F` would kill
/// the runner before it records its jobs as terminated, leaving them marked running, so Stop is
/// refused there (and hidden in the UI via `can_stop_runs`).
async fn cli_stop_run_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StopRunRequest>,
) -> impl IntoResponse {
    if !cfg!(unix) {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(serde_json::json!({
                "success": false,
                "error": "Stopping a local run is not supported on Windows",
            })),
        );
    }
    let pids = state
        .local_runs
        .lock()
        .await
        .get(&req.workflow_id)
        .cloned()
        .unwrap_or_default();
    for &pid in &pids {
        info!(
            "Stopping local run workflow_id={} pid={}",
            req.workflow_id, pid
        );
        stop_process(pid);
    }
    (
        StatusCode::OK,
        Json(serde_json::json!({ "success": true, "stopped": !pids.is_empty() })),
    )
}

/// Read file contents from filesystem
async fn cli_read_file_handler(Json(req): Json<ReadFileRequest>) -> impl IntoResponse {
    let path = expand_tilde(&req.path);
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
    match tokio::fs::read_to_string(&path).await {
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

#[derive(Deserialize)]
struct ResourceSamplesRequest {
    db_path: String,
    /// Return only system samples with `timestamp > since` (unix seconds).
    #[serde(default)]
    since: i64,
    /// `(job_id, since)` pairs. Each series has its own cursor, so a read that sees the tables
    /// at different checkpoints can't make one series skip samples another already returned.
    #[serde(default)]
    jobs: Vec<(i64, i64)>,
    /// Return only samples within this many seconds of the newest stored one (0 = all).
    #[serde(default)]
    window: i64,
}

/// Column-oriented samples, ready to hand to Plotly.
#[derive(Serialize, Default)]
struct SampleSeries {
    timestamp: Vec<i64>,
    cpu_percent: Vec<f64>,
    memory_bytes: Vec<i64>,
    /// Node capacity, for utilization percentages (system samples only).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    total_memory_bytes: Vec<i64>,
}

#[derive(Serialize, Default)]
struct ResourceSamplesResponse {
    success: bool,
    system: SampleSeries,
    /// Samples for the requested `job_ids`, keyed by job ID.
    jobs: std::collections::HashMap<i64, SampleSeries>,
    /// All jobs this node has monitored: (job_id, job_name).
    job_names: Vec<(i64, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Samples appended to a live resource metrics database since `since`.
async fn cli_resource_samples_handler(
    Json(req): Json<ResourceSamplesRequest>,
) -> Json<ResourceSamplesResponse> {
    let result = tokio::task::spawn_blocking(move || read_resource_samples(&req))
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r.map_err(|e| e.to_string()));
    Json(result.unwrap_or_else(|e| ResourceSamplesResponse {
        error: Some(e),
        ..Default::default()
    }))
}

fn read_resource_samples(
    req: &ResourceSamplesRequest,
) -> rusqlite::Result<ResourceSamplesResponse> {
    use rusqlite::{Connection, OpenFlags};
    // The writer usually runs on another host. WAL shared memory does not work across hosts
    // on a network filesystem, so ignore the WAL (`immutable=1`) and read only checkpointed
    // pages, which the writer produces after each flush when `live_monitoring` is on.
    // ponytail: a read racing a checkpoint can fail or miss rows; the next poll retries.
    const URI_RESERVED: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
        .add(b'%')
        .add(b'?')
        .add(b'#')
        .add(b' ');
    let uri = format!(
        "file:{}?immutable=1",
        percent_encoding::utf8_percent_encode(
            &expand_tilde(&req.db_path).to_string_lossy(),
            URI_RESERVED
        )
    );
    let conn = Connection::open_with_flags(
        &uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;
    let mut out = ResourceSamplesResponse {
        success: true,
        ..Default::default()
    };
    // Tables appear at the first checkpoint; until then the database looks empty.
    let has_table = |name: &str| -> rusqlite::Result<bool> {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [name],
            |r| r.get(0),
        )
    };
    let read_series = |sql: &str, params: &[&dyn rusqlite::ToSql]| {
        let mut series = SampleSeries::default();
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query(params)?;
        while let Some(row) = rows.next()? {
            series.timestamp.push(row.get(0)?);
            series.cpu_percent.push(row.get(1)?);
            series.memory_bytes.push(row.get(2)?);
            if row.as_ref().column_count() > 3 {
                series.total_memory_bytes.push(row.get(3)?);
            }
        }
        Ok::<_, rusqlite::Error>(series)
    };
    let has_system = has_table("system_resource_samples")?;
    let has_jobs = has_table("job_resource_samples")?;
    // Anchor the window on the newest stored sample, not the clock, so finished runs stay
    // viewable.
    let floor = if req.window > 0 {
        let mut newest: Option<i64> = None;
        if has_system {
            newest = conn.query_row(
                "SELECT MAX(timestamp) FROM system_resource_samples",
                [],
                |r| r.get(0),
            )?;
        }
        if has_jobs {
            for (job_id, _) in &req.jobs {
                let t: Option<i64> = conn.query_row(
                    "SELECT MAX(timestamp) FROM job_resource_samples WHERE job_id = ?1",
                    [job_id],
                    |r| r.get(0),
                )?;
                newest = newest.max(t);
            }
        }
        newest.map_or(i64::MIN, |t| t - req.window)
    } else {
        i64::MIN
    };
    if has_system {
        out.system = read_series(
            "SELECT timestamp, cpu_percent, memory_bytes, total_memory_bytes \
             FROM system_resource_samples \
             WHERE timestamp > ?1 ORDER BY timestamp",
            &[&req.since.max(floor)],
        )?;
    }
    if has_jobs {
        for (job_id, since) in &req.jobs {
            let series = read_series(
                "SELECT timestamp, cpu_percent, memory_bytes FROM job_resource_samples \
                 WHERE job_id = ?1 AND timestamp > ?2 ORDER BY timestamp",
                &[job_id, &(*since).max(floor)],
            )?;
            out.jobs.insert(*job_id, series);
        }
    }
    if has_table("job_metadata")? {
        let mut stmt = conn.prepare("SELECT job_id, job_name FROM job_metadata ORDER BY job_id")?;
        out.job_names = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
    }
    Ok(out)
}

#[derive(Deserialize)]
struct TailFileRequest {
    path: String,
    /// Byte offset returned by the previous call. Omit to start near the end of the file.
    offset: Option<u64>,
}

#[derive(Serialize, Debug, PartialEq)]
struct TailFileResponse {
    exists: bool,
    content: String,
    /// Pass back as `offset` on the next call.
    offset: u64,
    error: Option<String>,
}

/// How much existing output a new tail shows, and what a pane jumps to when it falls behind.
const TAIL_INITIAL_BYTES: u64 = 64 * 1024;
/// A backlog bigger than this (a fast writer, a long pause) is skipped rather than streamed.
const TAIL_MAX_BYTES: u64 = 512 * 1024;

/// Follow a growing file (a running job's stdout/stderr) by polling with the returned offset.
async fn cli_tail_file_handler(Json(req): Json<TailFileRequest>) -> Json<TailFileResponse> {
    Json(
        tail_file(&req.path, req.offset)
            .await
            .unwrap_or_else(|e| TailFileResponse {
                exists: true,
                content: String::new(),
                offset: req.offset.unwrap_or(0),
                error: Some(format!("Failed to read file: {e}")),
            }),
    )
}

/// Open for reading without blocking on a FIFO (O_NONBLOCK); regular-file reads are unaffected.
async fn open_nonblocking(path: &std::path::Path) -> std::io::Result<tokio::fs::File> {
    let mut opts = tokio::fs::OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    opts.custom_flags(libc::O_NONBLOCK);
    opts.open(path).await
}

/// Length of `buf` without a multi-byte character cut off at the end.
fn complete_utf8_len(buf: &[u8]) -> usize {
    let from = buf.len().saturating_sub(4);
    match buf[from..].iter().rposition(|b| b & 0xC0 != 0x80) {
        Some(i) if matches!(std::str::from_utf8(&buf[from + i..]), Err(e) if e.error_len().is_none()) => {
            from + i
        }
        _ => buf.len(),
    }
}

/// Expand a leading `~` or `~/` to the home directory; the browser sends paths verbatim and
/// neither the filesystem nor the torc CLI expands `~`.
fn expand_tilde(path: &str) -> std::path::PathBuf {
    let rest = path
        .strip_prefix('~')
        .filter(|r| r.is_empty() || r.starts_with(std::path::is_separator));
    match (rest, dirs::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest.trim_start_matches(std::path::is_separator)),
        _ => std::path::PathBuf::from(path),
    }
}

async fn tail_file(path: &str, offset: Option<u64>) -> std::io::Result<TailFileResponse> {
    use tokio::io::AsyncSeekExt;
    let path = expand_tilde(path);
    let mut file = match open_nonblocking(&path).await {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // A missing directory means a wrong output dir (or a job on another host), not a
            // job that hasn't started: say so instead of waiting forever.
            if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty())
                && !dir.is_dir()
            {
                return Err(std::io::Error::other(format!(
                    "directory {} not found; check the output directory",
                    dir.display()
                )));
            }
            // Not created yet (job not started) or deleted. Keep the offset so a brief
            // disappearance doesn't replay the file; a recreated shorter file restarts at 0.
            return Ok(TailFileResponse {
                exists: false,
                content: String::new(),
                offset: offset.unwrap_or(0),
                error: None,
            });
        }
        r => r?,
    };
    // Checked on the opened handle, so the path can't be swapped in between. Devices never end.
    let meta = file.metadata().await?;
    if !meta.is_file() {
        return Err(std::io::Error::other("not a regular file"));
    }
    let len = meta.len();
    let (start, skipped) = match offset {
        Some(o) if o > len => (0, 0), // Shorter than before: truncated or recreated.
        Some(o) if len - o > TAIL_MAX_BYTES => {
            (len - TAIL_INITIAL_BYTES, len - TAIL_INITIAL_BYTES - o)
        }
        Some(o) => (o, 0),
        None => (len.saturating_sub(TAIL_INITIAL_BYTES), 0),
    };
    file.seek(std::io::SeekFrom::Start(start)).await?;
    let mut buf = Vec::new();
    file.take(TAIL_MAX_BYTES).read_to_end(&mut buf).await?;
    // Jumping into the middle of the file can land inside a character; drop its tail bytes.
    let lead = if start > 0 && (offset.is_none() || skipped > 0) {
        buf.iter().take(3).take_while(|b| *b & 0xC0 == 0x80).count()
    } else {
        0
    };
    // Hold back a character cut off at the end; the next call returns it whole.
    let end = complete_utf8_len(&buf).max(lead);
    let mut content = String::from_utf8_lossy(&buf[lead..end]).into_owned();
    if skipped > 0 {
        content.insert_str(0, &format!("\n[… skipped {skipped} bytes to catch up …]\n"));
    }
    Ok(TailFileResponse {
        exists: true,
        content,
        offset: start + end as u64,
        error: None,
    })
}

#[derive(Deserialize)]
struct WriteFileRequest {
    path: String,
    content: String,
    #[serde(default)]
    overwrite: bool,
}

#[derive(Serialize)]
struct WriteFileResponse {
    success: bool,
    /// Absolute path written (relative paths resolve against torc-dash's working directory).
    path: Option<String>,
    error: Option<String>,
    /// The file exists and `overwrite` was false; nothing was written.
    exists: bool,
}

/// Save text (a workflow spec from the editor) to a file on the machine running torc-dash.
async fn cli_write_file_handler(Json(req): Json<WriteFileRequest>) -> impl IntoResponse {
    let fail = |error: String, exists: bool| WriteFileResponse {
        success: false,
        path: None,
        error: Some(error),
        exists,
    };
    let path = match std::path::absolute(expand_tilde(req.path.trim())) {
        Ok(p) if !req.path.trim().is_empty() => p,
        _ => return Json(fail("Enter a file path".into(), false)),
    };
    if path.is_dir() {
        return Json(fail("Path is a directory".into(), true));
    }
    // create_new checks and creates in one step, so a file appearing meanwhile is not clobbered.
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true);
    if req.overwrite {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    let mut file = match options.open(&path).await {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Json(fail("File already exists".into(), true));
        }
        Err(e) => return Json(fail(format!("Failed to write file: {e}"), false)),
    };
    let written = async {
        use tokio::io::AsyncWriteExt;
        file.write_all(req.content.as_bytes()).await?;
        file.flush().await
    };
    match written.await {
        Ok(()) => Json(WriteFileResponse {
            success: true,
            path: Some(path.display().to_string()),
            error: None,
            exists: false,
        }),
        Err(e) => Json(fail(format!("Failed to write file: {e}"), false)),
    }
}

/// JSON Schema for workflow spec files; drives completion, hover, and lint in the spec editor.
async fn workflow_schema_handler() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        torc::client::workflow_spec::workflow_spec_schema(),
    )
}

/// Import a workflow from a server-side file path or uploaded content
async fn cli_import_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ImportRequest>,
) -> impl IntoResponse {
    // Determine the file path to import from
    let temp_path_owned;
    let expanded_path;
    let import_path = if let Some(ref path) = req.file_path {
        // Use server-side file path directly
        expanded_path = expand_tilde(path).to_string_lossy().into_owned();
        expanded_path.as_str()
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
    #[serde(skip_serializing_if = "Option::is_none")]
    workflow_id: Option<i64>,
}

fn workflow_id_from_resource_db_name(name: &str) -> Option<i64> {
    let start = name.find("_wf")? + 3;
    let rest = &name[start..];
    let digit_count = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    if digit_count == 0 {
        return None;
    }
    if rest.chars().nth(digit_count) != Some('_') {
        return None;
    }
    rest[..digit_count].parse().ok()
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

    // Each request gets its own temp dir, so concurrent requests can't read or delete each
    // other's plots.
    let mut temp_dir = None;
    let output_dir = match req.output_dir.as_deref() {
        Some(dir) => expand_tilde(dir),
        None => match tempfile::Builder::new().prefix("torc_plots_").tempdir() {
            Ok(dir) => temp_dir.insert(dir).path().to_path_buf(),
            Err(e) => {
                return Json(PlotResourcesResponse {
                    success: false,
                    plots: vec![],
                    error: Some(format!("Failed to create output directory: {}", e)),
                });
            }
        },
    };
    let response = plot_resources(&state.torc_bin, &req, &output_dir).await;
    // Dropping a TempDir deletes it synchronously; keep that off the async worker threads.
    if let Some(dir) = temp_dir {
        tokio::task::spawn_blocking(move || dir.close());
    }
    Json(response)
}

async fn plot_resources(
    torc_bin: &str,
    req: &PlotResourcesRequest,
    output_dir: &std::path::Path,
) -> PlotResourcesResponse {
    if let Err(e) = tokio::fs::create_dir_all(output_dir).await {
        return PlotResourcesResponse {
            success: false,
            plots: vec![],
            error: Some(format!("Failed to create output directory: {}", e)),
        };
    }

    // Build command arguments
    let mut args: Vec<&str> = vec!["plot-resources"];

    // Add all database paths
    let db_paths: Vec<String> = req
        .db_paths
        .iter()
        .map(|p| expand_tilde(p).to_string_lossy().into_owned())
        .collect();
    args.extend(db_paths.iter().map(String::as_str));

    let output_dir_str = output_dir.to_string_lossy().to_string();
    args.push("--output-dir");
    args.push(&output_dir_str);

    args.push("--prefix");
    args.push(&req.prefix);

    args.push("--format");
    args.push("json");

    info!("Running: {} {}", torc_bin, args.join(" "));

    let output = Command::new(torc_bin).args(&args).output().await;

    match output {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return PlotResourcesResponse {
                    success: false,
                    plots: vec![],
                    error: Some(format!("Command failed: {}", stderr)),
                };
            }

            // Read all generated JSON files
            let mut plots = Vec::new();

            match tokio::fs::read_dir(output_dir).await {
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
                    return PlotResourcesResponse {
                        success: false,
                        plots: vec![],
                        error: Some(format!("Failed to read output directory: {}", e)),
                    };
                }
            }

            // Sort plots by name for consistent ordering
            plots.sort_by(|a, b| a.name.cmp(&b.name));

            PlotResourcesResponse {
                success: true,
                plots,
                error: None,
            }
        }
        Err(e) => PlotResourcesResponse {
            success: false,
            plots: vec![],
            error: Some(format!("Failed to execute command: {}", e)),
        },
    }
}

/// List resource database files in a directory
async fn cli_list_resource_dbs_handler(
    Json(req): Json<ListResourceDbsRequest>,
) -> impl IntoResponse {
    let base_path = &expand_tilde(&req.base_dir);

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

                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();

                    databases.push(ResourceDbInfo {
                        workflow_id: workflow_id_from_resource_db_name(&name),
                        path: path.to_string_lossy().to_string(),
                        name,
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
    let output_dir = expand_tilde(&req.output_dir).to_string_lossy().into_owned();

    let mut args = vec![
        "-f",
        "json",
        "slurm",
        "parse-logs",
        &output_dir,
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
    let output_dir = expand_tilde(&req.output_dir).to_string_lossy().into_owned();

    let args = vec![
        "-f",
        "json",
        "slurm",
        "sacct",
        &workflow_id_str,
        "--output-dir",
        &output_dir,
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
    // Run: torc -f json hpc list
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
    /// Log level
    #[serde(default = "default_log_level")]
    log_level: String,
}

fn default_server_port() -> u16 {
    8080
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
    /// torc-server URL this dashboard talks to, without any embedded credentials.
    api_url: String,
    /// Whether Stop can end a local run cleanly here (see `cli_stop_run_handler`).
    can_stop_runs: bool,
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
        api_url: redact_url(&state.api_url),
        can_stop_runs: cfg!(unix),
    })
}

/// `url` with any user:password removed, for display.
fn redact_url(url: &str) -> String {
    match reqwest::Url::parse(url) {
        Ok(mut u) => {
            let _ = u.set_password(None);
            let _ = u.set_username("");
            u.to_string()
        }
        Err(_) => url.to_string(),
    }
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
    /// The spec editor's buffer, when chatting from the editor. Enables the spec tools.
    #[serde(default)]
    spec: Option<String>,
}

/// Tools for the spec editor's buffer, handled in torc-dash rather than by the MCP server:
/// (name, description, JSON Schema of the input).
fn spec_tools() -> [(&'static str, &'static str, serde_json::Value); 3] {
    [
        (
            "edit_spec",
            "Edit the workflow spec in the user's editor: replace old_str, which must occur exactly \
             once in the spec, with new_str. Include enough surrounding text to make old_str \
             unique. Keep the user's formatting and comments. The edit appears in the editor \
             immediately.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "old_str": {"type": "string", "description": "Exact text to replace"},
                    "new_str": {"type": "string", "description": "Replacement text"}
                },
                "required": ["old_str", "new_str"]
            }),
        ),
        (
            "replace_spec",
            "Replace the entire workflow spec in the user's editor. Use it to write a new spec or \
             rewrite most of one; use edit_spec for changes.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {"type": "string", "description": "The complete spec (YAML)"}
                },
                "required": ["content"]
            }),
        ),
        (
            "validate_spec",
            "Validate the spec in the user's editor as `torc create --dry-run` would. Returns \
             errors, warnings, and a summary (job counts after parameter expansion, etc.).",
            serde_json::json!({"type": "object", "properties": {}}),
        ),
    ]
}

/// Applies an edit_spec or replace_spec call to `buf`. Returns a message for the model and the
/// edit to send to the editor, or an error for the model to correct.
fn apply_spec_edit(
    buf: &mut String,
    tool: &str,
    input: &serde_json::Value,
) -> Result<(String, serde_json::Value), String> {
    match tool {
        "edit_spec" => {
            let old = input["old_str"].as_str().unwrap_or_default();
            let new = input["new_str"].as_str().unwrap_or_default();
            if old.is_empty() {
                return Err(
                    "old_str is empty. Use replace_spec to write the whole spec.".to_string(),
                );
            }
            match buf.matches(old).count() {
                0 => Err(
                    "old_str was not found. Copy it exactly from the current spec, \
                          including indentation."
                        .to_string(),
                ),
                1 => {
                    *buf = buf.replacen(old, new, 1);
                    Ok((
                        "Edited the spec.".to_string(),
                        serde_json::json!({"old_str": old, "new_str": new}),
                    ))
                }
                n => Err(format!(
                    "old_str occurs {n} times. Include more surrounding text so it is unique."
                )),
            }
        }
        "replace_spec" => {
            let content = input["content"]
                .as_str()
                .ok_or("content is required")?
                .to_string();
            *buf = content.clone();
            Ok((
                "Replaced the spec.".to_string(),
                serde_json::json!({ "content": content }),
            ))
        }
        _ => Err(format!("Unknown spec tool {tool}")),
    }
}

/// `torc -f json create --dry-run` on spec text, via a temp file with the given extension.
async fn dry_run_spec(torc_bin: &str, api_url: &str, spec: &str, extension: &str) -> CliResponse {
    let temp_path =
        std::env::temp_dir().join(format!("torc_spec_{}{}", uuid::Uuid::new_v4(), extension));
    if let Err(e) = tokio::fs::write(&temp_path, spec).await {
        return CliResponse {
            success: false,
            stdout: String::new(),
            stderr: format!("Failed to write temp file: {}", e),
            exit_code: None,
        };
    }
    let path = temp_path.to_string_lossy();
    let result = run_torc_command(
        torc_bin,
        &["-f", "json", "create", &path, "--dry-run"],
        api_url,
    )
    .await;
    let _ = tokio::fs::remove_file(&temp_path).await;
    result
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
                "No API key configured. Set ANTHROPIC_API_KEY or ANTHROPIC_AUTH_TOKEN, or use Azure AI Foundry."
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

/// Fill AI Chat settings from `[dash.chat]` (and the API key from the user config file), mapping
/// each config provider onto the existing CLI fields. A flag or its environment variable wins.
fn apply_chat_config(
    cli: &mut Cli,
    chat: &torc::config::DashChatConfig,
    explicit: &dyn Fn(&str) -> bool,
) {
    let (api_key, warnings) = torc::config::user_chat_api_key(&torc::config::ConfigPaths::new());
    for w in warnings {
        warn!("{w}");
    }
    let provider = match &chat.provider {
        Some(p) if !explicit("llm_provider") => p.to_lowercase(),
        _ => cli.llm_provider.to_lowercase(),
    };
    let fill = |slot: &mut Option<String>, id: &str, value: &Option<String>| {
        if !explicit(id) && value.is_some() {
            *slot = value.clone();
        }
    };
    let fill_str = |slot: &mut String, id: &str, value: &Option<String>| {
        if let Some(v) = value.as_ref().filter(|_| !explicit(id)) {
            *slot = v.clone();
        }
    };
    match provider.as_str() {
        "openai" => {
            fill(&mut cli.openai_api_key, "openai_api_key", &api_key);
            fill_str(&mut cli.openai_base_url, "openai_base_url", &chat.base_url);
            fill_str(&mut cli.openai_model, "openai_model", &chat.model);
        }
        "ollama" => {
            fill_str(&mut cli.ollama_base_url, "ollama_base_url", &chat.base_url);
            fill_str(&mut cli.ollama_model, "ollama_model", &chat.model);
        }
        "github" => {
            fill(&mut cli.github_token, "github_token", &api_key);
            fill_str(
                &mut cli.github_models_base_url,
                "github_models_base_url",
                &chat.base_url,
            );
            fill_str(
                &mut cli.github_models_model,
                "github_models_model",
                &chat.model,
            );
        }
        "foundry" => {
            fill(
                &mut cli.anthropic_foundry_api_key,
                "anthropic_foundry_api_key",
                &api_key,
            );
            fill(
                &mut cli.anthropic_foundry_resource,
                "anthropic_foundry_resource",
                &chat.foundry_resource,
            );
            fill(
                &mut cli.anthropic_auth_header,
                "anthropic_auth_header",
                &chat.auth_header,
            );
            fill_str(&mut cli.anthropic_model, "anthropic_model", &chat.model);
        }
        // A custom gateway is the Anthropic path with a base URL.
        "anthropic" | "custom" => {
            fill(&mut cli.anthropic_api_key, "anthropic_api_key", &api_key);
            fill(
                &mut cli.anthropic_base_url,
                "anthropic_base_url",
                &chat.base_url,
            );
            fill(
                &mut cli.anthropic_auth_header,
                "anthropic_auth_header",
                &chat.auth_header,
            );
            fill_str(&mut cli.anthropic_model, "anthropic_model", &chat.model);
        }
        other => {
            warn!("Ignoring unknown dash.chat.provider {other:?}");
            return;
        }
    }
    if provider == "foundry" && cli.anthropic_foundry_resource.is_none() {
        warn!("dash.chat.provider is \"foundry\" but no foundry_resource is set");
    }
    if provider == "custom" && cli.anthropic_base_url.is_none() {
        warn!("dash.chat.provider is \"custom\" but no base_url is set");
    }
    if !explicit("llm_provider") {
        cli.llm_provider = match provider.as_str() {
            "foundry" | "custom" => "anthropic".to_string(),
            p => p.to_string(),
        };
    }
}

/// Claude API base URL ending in /v1, from either the Claude Code form of ANTHROPIC_BASE_URL
/// (gateway root; the client appends /v1/messages) or a URL that already ends in /v1.
fn anthropic_api_base(url: &str) -> String {
    let url = url.trim().trim_end_matches('/');
    if url.ends_with("/v1") {
        url.to_string()
    } else {
        format!("{url}/v1")
    }
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
                .unwrap_or("claude-opus-5-5")
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
                .map(anthropic_api_base)
                .ok_or((StatusCode::BAD_REQUEST, "Base URL is required"))?;
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
                .unwrap_or("claude-opus-5-5")
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
                .unwrap_or("claude-opus-5-5")
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
        // Its tools run the torc CLI; use the same one as the dashboard.
        .env("TORC_BIN", &state.torc_bin)
        .stderr(std::process::Stdio::inherit());
    let child_process = TokioChildProcess::new(command)
        .map_err(|e| anyhow::anyhow!("Failed to spawn torc-mcp-server: {}", e))?;

    // Connect as MCP client
    let running_service: rmcp::service::RunningService<rmcp::service::RoleClient, ()> = ()
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
        _service: running_service,
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
fn chat_system_prompt(workflow_id: Option<i64>, spec: Option<&str>) -> String {
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

    if let Some(spec) = spec {
        prompt.push_str(&format!(
            "\n\nThe user is editing a Torc workflow spec (YAML) in the dashboard's spec editor, \
             and you share that buffer. Its current contents:\n\n```yaml\n{spec}\n```\n\n\
             To change it, call edit_spec for targeted changes (copy old_str exactly) or \
             replace_spec to write a new spec; the user sees each edit as you make it. After \
             editing, call validate_spec and fix any errors it reports. Explain briefly what you \
             changed. Do not create workflows or write files: the user creates the workflow from \
             the editor. Use get_docs and get_example for spec syntax and patterns. If it is \
             unclear whether the workflow runs locally or on Slurm, ask."
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
    let mut spec_buf = req.spec;

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

    let system_prompt = chat_system_prompt(workflow_id, spec_buf.as_deref());
    let http_client = state.client.clone();
    // From the editor, the model edits the buffer instead of saving spec files.
    let tools: Vec<rmcp::model::Tool> = if spec_buf.is_some() {
        tools
            .into_iter()
            .filter(|t| t.name != "create_workflow")
            .collect()
    } else {
        tools
    };
    let local_tools: Vec<_> = if spec_buf.is_some() {
        spec_tools().to_vec()
    } else {
        Vec::new()
    };
    let openai_local_tools = local_tools.iter().map(|(name, description, schema)| {
        serde_json::json!({
            "type": "function",
            "function": {"name": name, "description": description, "parameters": schema}
        })
    });
    let openai_local_tools: Vec<serde_json::Value> = openai_local_tools.collect();
    let claude_local_tools: Vec<serde_json::Value> = local_tools
        .iter()
        .map(|(name, description, schema)| {
            serde_json::json!({"name": name, "description": description, "input_schema": schema})
        })
        .collect();
    let (torc_bin, torc_api_url) = (state.torc_bin.clone(), state.api_url.clone());

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
                let mut openai_tools = mcp_tools_to_openai_tools(&tools);
                openai_tools.extend(openai_local_tools.iter().cloned());
                let openai_messages = messages_to_openai_format(&messages, &system_prompt);

                // GitHub Models has a strict 8000 token input limit for gpt-4o
                // Use filtered essential tools only for GitHub to fit within limit
                let (final_messages, final_tools, was_truncated) = if provider == LlmProvider::GitHub {
                    // Use only essential tools for GitHub Models
                    let mut filtered_tools = mcp_tools_to_openai_tools_filtered(&tools);
                    filtered_tools.extend(openai_local_tools.iter().cloned());
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
                let mut claude_tools = mcp_tools_to_claude_tools(&tools);
                claude_tools.extend(claude_local_tools.iter().cloned());

                let body = serde_json::json!({
                    "model": model,
                    "max_tokens": 8192,
                    "system": system_prompt,
                    "messages": messages,
                    "tools": claude_tools,
                });

                // A bare token in an Authorization header is a bearer token.
                let auth_value = if auth_header_name.eq_ignore_ascii_case("authorization")
                    && !api_key.starts_with("Bearer ")
                {
                    format!("Bearer {api_key}")
                } else {
                    api_key.clone()
                };
                (url, body, auth_value)
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

                // Spec editor tools run here, against this request's copy of the buffer.
                if let Some(buf) = spec_buf.as_mut()
                    && local_tools.iter().any(|(name, _, _)| *name == tool_name)
                {
                    let (result_text, is_error) = if tool_name == "validate_spec" {
                        let r = dry_run_spec(&torc_bin, &torc_api_url, buf, ".yaml").await;
                        let text = if r.stdout.trim().is_empty() { r.stderr } else { r.stdout };
                        (text, false)
                    } else {
                        match apply_spec_edit(buf, tool_name, &tool_call["input"]) {
                            Ok((message, edit)) => {
                                yield Ok(Event::default().event("spec_edit").data(edit.to_string()));
                                (message, false)
                            }
                            Err(message) => (message, true),
                        }
                    };
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
                    continue;
                }

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

#[cfg(test)]
mod tests {
    use super::{
        ResourceSamplesRequest, allowed_host_list, anthropic_api_base, apply_spec_edit,
        check_request, expand_tilde, read_resource_samples, same_secret, tail_file,
        take_token_param, workflow_id_from_resource_db_name,
    };

    #[test]
    fn relative_bin_paths_become_absolute_but_bare_names_stay() {
        assert_eq!(super::absolute_if_relative_path("torc".into()), "torc");
        let rel = super::absolute_if_relative_path("./target/release/torc".into());
        assert!(std::path::Path::new(&rel).is_absolute(), "{rel}");
        assert!(rel.ends_with("torc"), "{rel}");
        let abs = std::env::temp_dir()
            .join("torc")
            .to_string_lossy()
            .into_owned();
        assert_eq!(super::absolute_if_relative_path(abs.clone()), abs);
    }

    fn op_args(body: serde_json::Value) -> Result<Vec<String>, String> {
        let req: super::OpRequest = serde_json::from_value(body).unwrap();
        super::build_op(&req).map(|cmd| cmd.args)
    }

    #[test]
    fn ops_map_only_known_params_to_cli_args() {
        assert_eq!(
            op_args(serde_json::json!({
                "op": "archive", "workflow_ids": [3, 4], "archived": true, "extra": "--evil"
            }))
            .unwrap(),
            ["workflows", "archive", "true", "3", "4"]
        );
        assert_eq!(
            op_args(serde_json::json!({
                "op": "correct-resources", "workflow_id": 7, "memory_multiplier": 1.5,
                "dry_run": true
            }))
            .unwrap(),
            [
                "workflows",
                "correct-resources",
                "7",
                "--memory-multiplier",
                "1.5",
                "--dry-run"
            ]
        );
        assert_eq!(
            op_args(serde_json::json!({
                "op": "schedule-nodes", "workflow_id": 7, "scheduler_config_id": 2,
                "num_allocations": 0
            }))
            .unwrap(),
            [
                "slurm",
                "schedule-nodes",
                "7",
                "--scheduler-config-id",
                "2",
                "--num-hpc-jobs",
                "1",
                "--no-prompts"
            ]
        );
        assert!(op_args(serde_json::json!({ "op": "diagnose" })).is_err());
        assert!(op_args(serde_json::json!({ "op": "admin", "workflow_id": 1 })).is_err());
    }

    #[test]
    fn resource_samples_see_only_checkpointed_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("resource metrics?.db");
        let writer = rusqlite::Connection::open(&path).unwrap();
        writer.pragma_update(None, "journal_mode", "WAL").unwrap();
        writer
            .execute_batch(
                "CREATE TABLE system_resource_samples (id INTEGER PRIMARY KEY, timestamp INTEGER, \
                   cpu_percent REAL, memory_bytes INTEGER, total_memory_bytes INTEGER);
                 CREATE TABLE job_resource_samples (job_id INTEGER, timestamp INTEGER, \
                   cpu_percent REAL, memory_bytes INTEGER, num_processes INTEGER);
                 CREATE TABLE job_metadata (job_id INTEGER PRIMARY KEY, job_name TEXT);
                 INSERT INTO system_resource_samples VALUES (NULL, 10, 1.0, 100, 1000), \
                   (NULL, 20, 2.0, 200, 1000);
                 INSERT INTO job_resource_samples VALUES (7, 10, 50.0, 5, 1), (7, 20, 60.0, 6, 1), \
                   (8, 20, 70.0, 7, 1);
                 INSERT INTO job_metadata VALUES (7, 'a'), (8, 'b');",
            )
            .unwrap();
        let req = ResourceSamplesRequest {
            db_path: path.to_str().unwrap().to_string(),
            since: 10,
            jobs: vec![(7, 10)],
            window: 0,
        };

        let before = read_resource_samples(&req).unwrap();
        assert!(before.system.timestamp.is_empty() && before.job_names.is_empty());

        writer
            .query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |_| Ok(()))
            .unwrap();
        let after = read_resource_samples(&req).unwrap();
        assert_eq!(after.system.timestamp, vec![20]);
        assert_eq!(after.system.total_memory_bytes, vec![1000]);
        assert!(after.jobs[&7].total_memory_bytes.is_empty());
        assert_eq!(after.jobs.len(), 1);
        assert_eq!(after.jobs[&7].cpu_percent, vec![60.0]);
        assert_eq!(after.job_names, vec![(7, "a".into()), (8, "b".into())]);

        // Each series resumes from its own cursor.
        let req = ResourceSamplesRequest {
            jobs: vec![(7, 0)],
            ..req
        };
        let r = read_resource_samples(&req).unwrap();
        assert_eq!(r.system.timestamp, vec![20]);
        assert_eq!(r.jobs[&7].timestamp, vec![10, 20]);

        // A window keeps only samples near the newest stored one (job 8 at 20 anchors it).
        let req = ResourceSamplesRequest {
            since: 0,
            jobs: vec![(7, 0), (8, 0)],
            window: 5,
            ..req
        };
        let r = read_resource_samples(&req).unwrap();
        assert_eq!(r.system.timestamp, vec![20]);
        assert_eq!(r.jobs[&7].timestamp, vec![20]);
        assert_eq!(r.jobs[&8].timestamp, vec![20]);
    }

    #[tokio::test]
    async fn tail_file_follows_appends() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("job.o");
        let p = path.to_str().unwrap();

        let missing = tail_file(p, None).await.unwrap();
        assert!(!missing.exists);

        std::fs::write(&path, "one\n").unwrap();
        let r = tail_file(p, Some(missing.offset)).await.unwrap();
        assert_eq!((r.content.as_str(), r.offset), ("one\n", 4));

        // Append "two é" with the é's second byte not yet written.
        std::fs::write(&path, b"one\ntwo \xc3").unwrap();
        let r = tail_file(p, Some(r.offset)).await.unwrap();
        assert_eq!((r.content.as_str(), r.offset), ("two ", 8));
        std::fs::write(&path, "one\ntwo \u{e9}\n").unwrap();
        let r = tail_file(p, Some(r.offset)).await.unwrap();
        assert_eq!(r.content, "\u{e9}\n");

        // Truncated (recreated) file restarts from 0.
        std::fs::write(&path, "x").unwrap();
        let r = tail_file(p, Some(r.offset)).await.unwrap();
        assert_eq!((r.content.as_str(), r.offset), ("x", 1));

        // A fresh tail shows only the last TAIL_INITIAL_BYTES.
        std::fs::write(&path, "a".repeat(100_000)).unwrap();
        let r = tail_file(p, None).await.unwrap();
        assert_eq!(r.content.len() as u64, super::TAIL_INITIAL_BYTES);

        // Directories (and FIFOs/devices) are refused instead of opened.
        assert!(tail_file(dir.path().to_str().unwrap(), None).await.is_err());

        // A deleted file keeps the offset, so a brief disappearance doesn't replay it.
        std::fs::remove_file(&path).unwrap();
        let r = tail_file(p, Some(100)).await.unwrap();
        assert_eq!((r.exists, r.offset), (false, 100));

        // A missing directory is an error, not an endless "waiting for file".
        let wrong = dir.path().join("nope/job.o");
        assert!(tail_file(wrong.to_str().unwrap(), None).await.is_err());

        // A big backlog is skipped down to the last TAIL_INITIAL_BYTES.
        std::fs::write(&path, "b".repeat(2_000_000)).unwrap();
        let r = tail_file(p, Some(0)).await.unwrap();
        assert_eq!(r.offset, 2_000_000);
        assert!(r.content.starts_with("\n[… skipped "));

        // An invalid byte earlier in the chunk doesn't defeat holding back a split character.
        std::fs::write(&path, b"x\xffy\xc3").unwrap();
        let r = tail_file(p, Some(0)).await.unwrap();
        assert_eq!((r.content.as_str(), r.offset), ("x\u{fffd}y", 3));

        // A fresh tail whose window starts inside "é" drops the stray continuation byte, and
        // still holds back a split character at the end.
        let mut bytes = "\u{e9}".as_bytes().to_vec();
        bytes.extend(
            "a".repeat(super::TAIL_INITIAL_BYTES as usize - 2)
                .as_bytes(),
        );
        bytes.push(0xc3);
        std::fs::write(&path, &bytes).unwrap();
        let r = tail_file(p, None).await.unwrap();
        assert_eq!(
            r.content,
            "a".repeat(super::TAIL_INITIAL_BYTES as usize - 2)
        );
        assert_eq!(r.offset, bytes.len() as u64 - 1);
    }

    #[test]
    fn token_param_is_split_from_query() {
        let split = |q| take_token_param(q);
        assert_eq!(
            split("workflow_id=3&token=abc&mode=watch"),
            (
                Some("abc".to_string()),
                "workflow_id=3&mode=watch".to_string()
            )
        );
        assert_eq!(split("token=abc"), (Some("abc".to_string()), String::new()));
        assert_eq!(
            split("token=a%2Bb"),
            (Some("a+b".to_string()), String::new())
        );
        assert_eq!(split("a=1"), (None, "a=1".to_string()));
        assert_eq!(split(""), (None, String::new()));
        // Only an exact `token` key counts.
        assert_eq!(split("mytoken=x"), (None, "mytoken=x".to_string()));

        assert!(same_secret("abc", "abc"));
        assert!(!same_secret("abc", "abd"));
        assert!(!same_secret("abc", "ab"));
    }

    #[test]
    fn request_guard_checks_host_and_fetch_site() {
        let allowed = allowed_host_list("0.0.0.0", &["dash.example.org".to_string()]);
        let allowed = allowed.as_deref();
        let ok = |host, site, path| check_request(Some(host), site, path, allowed).is_ok();

        // Loopback names and configured names, with or without a port.
        assert!(ok("localhost:8090", None, "/api/cli/run"));
        assert!(ok("127.0.0.1:23456", Some("same-origin"), "/api/chat"));
        assert!(ok("[::1]:8090", None, "/"));
        assert!(ok("Dash.Example.org", None, "/"));
        // DNS rebinding: an attacker's hostname pointing at loopback.
        assert!(!ok(
            "evil.example.com:8090",
            Some("same-origin"),
            "/api/cli/write-file"
        ));
        // Cross-site requests to API routes, e.g. <img src=".../run-stream?mode=run">.
        assert!(!ok(
            "localhost:8090",
            Some("cross-site"),
            "/api/cli/run-stream"
        ));
        assert!(!ok(
            "localhost:8090",
            Some("same-site"),
            "/torc-service/v1/workflows"
        ));
        // ...but navigating to the UI from elsewhere (Torc Desktop's tauri:// page) is fine.
        assert!(ok("127.0.0.1:23456", Some("cross-site"), "/"));
        assert!(ok("localhost", Some("none"), "/api/version"));
        // No Host header (HTTP/1.0 probes) and "*" skip the host check.
        assert!(check_request(None, None, "/", allowed).is_ok());
        let any = allowed_host_list("127.0.0.1", &["*".to_string()]);
        assert!(check_request(Some("anything:1"), None, "/api/x", any.as_deref()).is_ok());
        // A specific bind host is allowed automatically.
        let bound = allowed_host_list("login1.hpc.org", &[]);
        assert!(check_request(Some("login1.hpc.org:8090"), None, "/", bound.as_deref()).is_ok());
    }
    use serde_json::json;

    #[test]
    fn spec_edits_apply_to_the_buffer() {
        let mut buf = "name: a\njobs:\n  - name: x\n    command: echo\n".to_string();
        let (_, edit) = apply_spec_edit(
            &mut buf,
            "edit_spec",
            &json!({"old_str": "command: echo", "new_str": "command: echo hi"}),
        )
        .unwrap();
        assert_eq!(buf, "name: a\njobs:\n  - name: x\n    command: echo hi\n");
        assert_eq!(edit["new_str"], "command: echo hi");

        // Missing, ambiguous, and empty old_str are errors and leave the buffer alone.
        let before = buf.clone();
        let edit = |b: &mut String, old: &str| {
            apply_spec_edit(b, "edit_spec", &json!({"old_str": old, "new_str": "z"}))
        };
        assert!(edit(&mut buf, "nope").unwrap_err().contains("not found"));
        assert!(edit(&mut buf, "name: ").unwrap_err().contains("2 times"));
        assert!(edit(&mut buf, "").is_err());
        assert_eq!(buf, before);

        apply_spec_edit(&mut buf, "replace_spec", &json!({"content": "name: b\n"})).unwrap();
        assert_eq!(buf, "name: b\n");
    }

    #[test]
    fn anthropic_base_url_accepts_claude_code_form() {
        // Claude Code style (gateway root) and explicit /v1 both resolve to .../v1.
        assert_eq!(
            anthropic_api_base("https://gw.example.org"),
            "https://gw.example.org/v1"
        );
        assert_eq!(
            anthropic_api_base("https://gw.example.org/"),
            "https://gw.example.org/v1"
        );
        assert_eq!(
            anthropic_api_base("https://gw.example.org/v1/"),
            "https://gw.example.org/v1"
        );
        assert_eq!(
            anthropic_api_base(" https://x.services.ai.azure.com/anthropic/v1"),
            "https://x.services.ai.azure.com/anthropic/v1"
        );
    }

    #[test]
    fn expand_tilde_only_expands_a_leading_home_component() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_tilde("~"), home);
        assert_eq!(expand_tilde("~/runs/out"), home.join("runs/out"));
        assert_eq!(expand_tilde("~bob/x"), std::path::PathBuf::from("~bob/x"));
        assert_eq!(
            expand_tilde("/abs/~/x"),
            std::path::PathBuf::from("/abs/~/x")
        );
        // Backslash is a filename character on Unix, a separator on Windows.
        let backslash = if cfg!(windows) {
            home.join("x")
        } else {
            std::path::PathBuf::from("~\\x")
        };
        assert_eq!(expand_tilde("~\\x"), backslash);
    }

    #[test]
    fn parses_workflow_id_from_resource_database_name() {
        assert_eq!(
            workflow_id_from_resource_db_name("resource_metrics_wf42_j7.db"),
            Some(42)
        );
        assert_eq!(
            workflow_id_from_resource_db_name("resource_metrics_wf42_compute_node.db"),
            Some(42)
        );
    }

    #[test]
    fn rejects_names_without_delimited_workflow_id() {
        assert_eq!(
            workflow_id_from_resource_db_name("resource_metrics.db"),
            None
        );
        assert_eq!(
            workflow_id_from_resource_db_name("resource_metrics_wf.db"),
            None
        );
        assert_eq!(
            workflow_id_from_resource_db_name("resource_metrics_wf42.db"),
            None
        );
        assert_eq!(
            workflow_id_from_resource_db_name("resource_metrics_wfabc_.db"),
            None
        );
    }
}
