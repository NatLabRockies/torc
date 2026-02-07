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
    http::{Request, StatusCode, header},
    response::{
        Html, IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use clap::Parser;
use rust_embed::Embed;
use serde::{Deserialize, Serialize};
use std::path::Path as FsPath;
use std::process::Command as StdCommand;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;
use torc::config::TorcConfig;
use torc::network_utils::find_available_port;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Embedded static assets for the dashboard
#[derive(Embed)]
#[folder = "static/"]
struct Assets;

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
    /// Managed server process (if started by torc-dash)
    managed_server: Mutex<ManagedServer>,
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

    info!(
        "Starting torc-dash v{} ({}{}) on {}:{} torc_bin={} server_bin={}",
        env!("CARGO_PKG_VERSION"),
        env!("GIT_HASH"),
        env!("GIT_DIRTY"),
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

    let state = Arc::new(AppState {
        api_url: final_api_url,
        client: reqwest::Client::new(),
        torc_bin,
        torc_server_bin: torc_server_bin.clone(),
        managed_server: Mutex::new(managed_server),
    });

    // Build router
    let app = Router::new()
        // Static files and dashboard
        .route("/", get(index_handler))
        .route("/static/*path", get(static_handler))
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
        // Version endpoint
        .route("/api/version", get(version_handler))
        // User endpoint
        .route("/api/user", get(user_handler))
        // API proxy - catch all /torc-service/* requests
        .route(
            "/torc-service/*path",
            get(proxy_handler)
                .post(proxy_handler)
                .put(proxy_handler)
                .patch(proxy_handler)
                .delete(proxy_handler),
        )
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state);

    // Try to bind to the requested port, incrementing if in use
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

/// Extract workflow ID from CLI output like "Created workflow 123" or "ID: 123"
fn extract_workflow_id(stdout: &str) -> Option<String> {
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
    "output".to_string()
}

#[derive(Serialize)]
struct CliResponse {
    success: bool,
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
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
            &["workflows", "create", &req.spec],
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
            &["workflows", "create", &temp_path],
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
        // Spec is a file path
        let mut args = vec!["workflows", "create-slurm", "--account", &req.account];
        if let Some(ref profile) = req.profile {
            args.push("--hpc-profile");
            args.push(profile);
        }
        args.push(&req.spec);
        run_torc_command(&state.torc_bin, &args, &state.api_url).await
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

        let mut args = vec!["workflows", "create-slurm", "--account", &req.account];
        if let Some(ref profile) = req.profile {
            args.push("--hpc-profile");
            args.push(profile);
        }
        args.push(&temp_path);

        let result = run_torc_command(&state.torc_bin, &args, &state.api_url).await;

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
            &["-f", "json", "workflows", "create", &req.spec, "--dry-run"],
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
            &["-f", "json", "workflows", "create", &temp_path, "--dry-run"],
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
    let mut args = vec!["workflows", "initialize", &req.workflow_id];
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
            "initialize",
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
    let mut args = vec!["workflows", "reinitialize", &req.workflow_id];
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
            .args(["workflows", "run", &workflow_id])
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
                let url = format!("{}/jobs?workflow_id={}&limit=10000", api_url_status, workflow_id_status);
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
        &workflow_id_str,
        "--output-dir",
        &req.output_dir,
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
    server_version: Option<String>,
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

    let (server_version, version_mismatch, mismatch_severity) = match result {
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
                (Some(server_ver.clone()), mismatch_msg, severity_str)
            }
            None => (None, None, None),
        },
        None => (None, None, None),
    };

    // Extract just the semver from server version (strip git hash suffix for display)
    let server_version_display = server_version
        .as_ref()
        .map(|v| v.split(" (").next().unwrap_or(v).to_string());

    Json(VersionResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        server_version: server_version_display,
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
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string());

    Json(UserResponse { user })
}

/// Execute a torc CLI command
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
