use std::io::{BufRead, BufReader};

use chrono::{DateTime, Local, Utc};
use clap::Subcommand;
use serde::{Deserialize, Serialize};

use crate::client::apis;
use crate::client::apis::configuration::{
    CLIENT_USER_HEADER, Configuration, client_user_header_value,
};
use crate::client::commands::print_error;

#[derive(Subcommand)]
pub enum AdminCommands {
    /// Reload the htpasswd file from disk without restarting the server
    #[command(
        name = "reload-auth",
        after_long_help = "\
EXAMPLES:
    # Reload auth credentials after adding a user
    torc admin reload-auth

    # With JSON output
    torc -f json admin reload-auth
"
    )]
    ReloadAuth,

    /// Tail inbound API requests via Server-Sent Events
    #[command(
        name = "tail-api",
        after_long_help = "\
EXAMPLES:
    # Watch all API requests as they arrive
    torc admin tail-api

    # Include request and response bodies (subject to size limits)
    torc admin tail-api --include-bodies

    # Emit one JSON object per line, e.g. for piping to jq
    torc -f json admin tail-api
"
    )]
    TailApi {
        /// Stream captured request and response bodies along with metadata
        #[arg(long)]
        include_bodies: bool,
    },
}

pub fn handle_admin_commands(config: &Configuration, command: &AdminCommands, format: &str) {
    match command {
        AdminCommands::ReloadAuth => match apis::access_control_api::reload_auth(config) {
            Ok(response) => {
                if format == "json" {
                    println!("{}", serde_json::to_string_pretty(&response).unwrap());
                } else {
                    println!("{} ({} users)", response.message, response.user_count);
                }
            }
            Err(e) => {
                print_error("reloading auth", &e);
                std::process::exit(1);
            }
        },
        AdminCommands::TailApi { include_bodies } => {
            tail_api_events(config, *include_bodies, format);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CapturedBody {
    bytes: usize,
    truncated: bool,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiRequestEvent {
    timestamp_ms: i64,
    method: String,
    path: String,
    #[serde(default)]
    query: Option<String>,
    status: u16,
    latency_ms: u64,
    #[serde(default)]
    request_id: Option<String>,
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    request_body: Option<CapturedBody>,
    #[serde(default)]
    response_body: Option<CapturedBody>,
}

fn tail_api_events(config: &Configuration, include_bodies: bool, format: &str) {
    let mut url = format!("{}/admin/api-events/stream", config.base_path);
    if include_bodies {
        url.push_str("?include_bodies=true");
    }

    eprintln!("Connecting to {} ...", url);
    if include_bodies {
        eprintln!("Body capture enabled (truncated to server-configured limit).");
    }
    eprintln!("Press Ctrl+C to stop.\n");

    let mut builder = reqwest::blocking::Client::builder().timeout(None);
    if let Some(ref cookie) = config.cookie_header {
        let mut headers = reqwest::header::HeaderMap::new();
        match reqwest::header::HeaderValue::from_str(cookie) {
            Ok(v) => {
                headers.insert(reqwest::header::COOKIE, v);
                builder = builder.default_headers(headers);
            }
            Err(e) => {
                eprintln!("Invalid cookie header: {}", e);
                std::process::exit(1);
            }
        }
    }
    let client = match config.tls.configure_blocking_builder(builder).build() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to build HTTP client: {}", e);
            std::process::exit(1);
        }
    };

    let mut request = client.get(&url).header("Accept", "text/event-stream");
    if let Some(value) = client_user_header_value() {
        request = request.header(CLIENT_USER_HEADER, value);
    }
    if let Some((ref username, ref password)) = config.basic_auth {
        request = request.basic_auth(username.clone(), password.clone());
    } else if let Some(ref token) = config.bearer_access_token {
        request = request.bearer_auth(token.clone());
    } else if let Some(ref api_key) = config.api_key {
        let value = match api_key.prefix {
            Some(ref prefix) => format!("{} {}", prefix, api_key.key),
            None => api_key.key.clone(),
        };
        request = request.header("X-API-KEY", value);
    }

    let response = match request.send() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to connect to admin event stream: {}", e);
            std::process::exit(1);
        }
    };
    if !response.status().is_success() {
        eprintln!("Server returned error status: {}", response.status());
        std::process::exit(1);
    }

    let mut reader = BufReader::new(response);
    let mut event_type = String::new();
    let mut data = String::new();
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => {
                eprintln!("\nServer closed the stream.");
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("\nError reading stream: {}", e);
                break;
            }
        }

        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            if !data.is_empty() {
                handle_frame(&event_type, &data, format);
            }
            event_type.clear();
            data.clear();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("event: ") {
            event_type = value.to_string();
        } else if let Some(value) = trimmed.strip_prefix("data: ") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(value);
        }
    }
}

fn handle_frame(event_type: &str, data: &str, format: &str) {
    if event_type == "warning" {
        eprintln!("warning: {}", data);
        return;
    }
    match serde_json::from_str::<ApiRequestEvent>(data) {
        Ok(event) => print_event(&event, format),
        Err(e) => eprintln!("Failed to parse event: {} - data: {}", e, data),
    }
}

fn print_event(event: &ApiRequestEvent, format: &str) {
    if format == "json" {
        if let Ok(line) = serde_json::to_string(event) {
            println!("{}", line);
        }
        return;
    }

    let ts = format_ms(event.timestamp_ms);
    let user = event.user.as_deref().unwrap_or("-");
    let span = event.request_id.as_deref().unwrap_or("-");
    let path = match &event.query {
        Some(q) if !q.is_empty() => format!("{}?{}", event.path, q),
        _ => event.path.clone(),
    };
    println!(
        "[{ts}] {status:>3} {method:<6} {path}  ({latency}ms, user={user}, span={span})",
        status = event.status,
        method = event.method,
        latency = event.latency_ms,
    );
    if let Some(body) = &event.request_body {
        print_body("  request  ", body);
    }
    if let Some(body) = &event.response_body {
        print_body("  response ", body);
    }
}

fn print_body(label: &str, body: &CapturedBody) {
    let suffix = if body.truncated { " [truncated]" } else { "" };
    match &body.text {
        Some(text) => {
            let preview = text.replace('\n', " ");
            println!(
                "{label}{} bytes{suffix}: {}",
                body.bytes,
                preview.trim_end()
            );
        }
        None => println!("{label}{} bytes (binary){suffix}", body.bytes),
    }
}

fn format_ms(ts: i64) -> String {
    DateTime::from_timestamp_millis(ts)
        .map(|dt: DateTime<Utc>| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string()
        })
        .unwrap_or_else(|| format!("{}ms", ts))
}
