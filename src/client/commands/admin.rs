use std::io::{BufRead, BufReader};

use chrono::{DateTime, Local, Utc};
use clap::Subcommand;
use serde::{Deserialize, Serialize};

use crate::client::apis;
use crate::client::apis::configuration::{
    CLIENT_USER_HEADER, Configuration, client_user_header_value,
};
use crate::client::commands::print_error;
use crate::client::commands::table_format::{
    display_csv, display_dynamic_csv, display_dynamic_table, display_table,
};
use crate::client::utils::format_local_timestamp_epoch;
use crate::models;
use tabled::Tabled;

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

    /// Show recent API request rate, throughput, and status mix
    #[command(
        name = "api-stats",
        after_long_help = "\
EXAMPLES:
    # Last hour, in 1-minute buckets
    torc admin api-stats

    # Last 5 minutes, in 30-second buckets
    torc admin api-stats --window 300 --interval 30

    # Raw JSON, e.g. for scripts or jq
    torc -f json admin api-stats
"
    )]
    ApiStats {
        /// Total span to report on, in seconds. Defaults to 3600 (1 hour).
        #[arg(long, default_value_t = 3600)]
        window: u64,
        /// Aggregation bucket width, in seconds. Defaults to 60.
        #[arg(long, default_value_t = 60)]
        interval: u64,
    },

    /// List recent admin raw-SQL audit-log entries (admin only)
    #[command(
        name = "list-audit-log",
        after_long_help = "\
Shows the durable audit trail of admin raw-SQL writes (`torc admin sql --write`),
newest first. A committing write is recorded atomically with the change, and a
write that reaches the database and fails is also recorded. Read-only queries,
dry-run previews, and statements rejected before execution (e.g. by validation)
are not audited.

EXAMPLES:
    # Most recent entries
    torc admin list-audit-log

    # Page through older entries
    torc admin list-audit-log --limit 20 --offset 20

    # JSON output (includes pagination metadata), e.g. for jq
    torc -f json admin list-audit-log
"
    )]
    ListAuditLog {
        /// Maximum number of entries to return (capped at 100,000)
        #[arg(long)]
        limit: Option<i64>,
        /// Offset for pagination (0-based)
        #[arg(long, default_value_t = 0)]
        offset: i64,
    },

    /// Run a raw SQL statement against the server database (admin only)
    #[command(
        name = "sql",
        after_long_help = "\
A controlled, audit-logged escape hatch for inspecting and surgically repairing
database state when a bug leaves a workflow stuck. Reads run on a read-only
connection; writes require --write and run inside a transaction.

WARNING: raw writes bypass application invariants (the job status state machine,
the background unblocking contract). A direct UPDATE/DELETE can leave the
database in a state torc never produces. Prefer a dedicated torc command when one
exists, and you may need to run 'torc workflows reconcile' afterward.

EXAMPLES:
    # Inspect state (read-only)
    torc admin sql \"SELECT id, status FROM job LIMIT 5\"

    # CSV / JSON output for scripting
    torc -f csv admin sql \"SELECT id, status FROM job\"
    torc -f json admin sql \"SELECT * FROM admin_audit_log\"

    # Apply a fix (previews affected rows, then prompts before committing)
    torc admin sql --write \"UPDATE result SET return_code=0 WHERE id=42\"

    # Skip the confirmation prompt
    torc admin sql --write --yes \"UPDATE result SET return_code=0 WHERE id=42\"

    # A full-table write must be explicitly allowed
    torc admin sql --write --allow-full-table \"DELETE FROM slurm_stats\"
"
    )]
    Sql {
        /// The SQL statement to execute (a single statement)
        statement: String,
        /// Execute a write statement. Without this flag the statement runs
        /// read-only and any write is rejected by SQLite.
        #[arg(long)]
        write: bool,
        /// Allow an UPDATE/DELETE with no WHERE clause (full-table write)
        #[arg(long)]
        allow_full_table: bool,
        /// Skip the confirmation prompt for writes
        #[arg(long, short = 'y')]
        yes: bool,
        /// Maximum number of result rows to return (read-only; capped at 100,000)
        #[arg(long)]
        limit: Option<i64>,
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
        AdminCommands::ApiStats { window, interval } => {
            show_api_stats(config, *window, *interval, format);
        }
        AdminCommands::ListAuditLog { limit, offset } => {
            list_audit_log(config, *limit, *offset, format);
        }
        AdminCommands::Sql {
            statement,
            write,
            allow_full_table,
            yes,
            limit,
        } => {
            handle_admin_sql(
                config,
                statement,
                *write,
                *allow_full_table,
                *yes,
                *limit,
                format,
            );
        }
    }
}

fn handle_admin_sql(
    config: &Configuration,
    statement: &str,
    write: bool,
    allow_full_table: bool,
    yes: bool,
    limit: Option<i64>,
    format: &str,
) {
    if write {
        // Preview: run the statement in a transaction that is rolled back, so we
        // can report how many rows would change before committing anything.
        let preview = models::AdminSqlRequest {
            sql: statement.to_string(),
            write: true,
            allow_full_table,
            dry_run: true,
            limit: None,
        };
        let preview = match apis::access_control_api::admin_sql(config, preview) {
            Ok(resp) => resp,
            Err(e) => {
                print_error("previewing SQL statement", &e);
                std::process::exit(1);
            }
        };

        if !yes {
            let affected = preview.rows_affected.unwrap_or(0);
            eprintln!("Statement: {statement}");
            eprintln!("This write will affect {affected} row(s).");
            eprint!("Proceed? [y/N] ");
            use std::io::Write;
            let _ = std::io::stderr().flush();
            let mut input = String::new();
            if std::io::stdin().read_line(&mut input).is_err() {
                eprintln!("Aborted.");
                std::process::exit(1);
            }
            let answer = input.trim().to_lowercase();
            if answer != "y" && answer != "yes" {
                eprintln!("Aborted.");
                return;
            }
        }

        let commit = models::AdminSqlRequest {
            sql: statement.to_string(),
            write: true,
            allow_full_table,
            dry_run: false,
            limit: None,
        };
        match apis::access_control_api::admin_sql(config, commit) {
            Ok(resp) => {
                if format == "json" {
                    println!("{}", serde_json::to_string_pretty(&resp).unwrap());
                } else {
                    println!(
                        "Committed. {} row(s) affected.",
                        resp.rows_affected.unwrap_or(0)
                    );
                }
            }
            Err(e) => {
                print_error("executing SQL statement", &e);
                std::process::exit(1);
            }
        }
    } else {
        let request = models::AdminSqlRequest {
            sql: statement.to_string(),
            write: false,
            allow_full_table: false,
            dry_run: false,
            limit,
        };
        match apis::access_control_api::admin_sql(config, request) {
            Ok(resp) => render_sql_result(&resp, format),
            Err(e) => {
                print_error("executing SQL statement", &e);
                std::process::exit(1);
            }
        }
    }
}

fn render_sql_result(resp: &models::AdminSqlResponse, format: &str) {
    if format == "json" {
        println!("{}", serde_json::to_string_pretty(resp).unwrap_or_default());
        return;
    }

    // Each item is an object keyed by column name; project it back into the
    // server-provided column order for tabular display.
    let rows: Vec<Vec<String>> = resp
        .items
        .iter()
        .map(|item| {
            resp.columns
                .iter()
                .map(|col| item.get(col).map(value_to_cell).unwrap_or_default())
                .collect()
        })
        .collect();

    if format == "csv" {
        display_dynamic_csv(&resp.columns, &rows);
        return;
    }

    if resp.columns.is_empty() {
        println!("(no rows)");
        return;
    }
    display_dynamic_table(&resp.columns, &rows);
    println!("\nTotal: {} row(s)", resp.items.len());
}

#[derive(Tabled)]
struct AuditLogTableRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Time")]
    time: String,
    #[tabled(rename = "User")]
    user: String,
    #[tabled(rename = "Committed")]
    committed: String,
    #[tabled(rename = "Success")]
    success: String,
    #[tabled(rename = "Full Table")]
    full_table: String,
    #[tabled(rename = "Rows")]
    rows_affected: String,
    #[tabled(rename = "SQL")]
    sql: String,
    #[tabled(rename = "Error")]
    error: String,
}

fn yes_no(value: bool) -> String {
    if value { "yes" } else { "no" }.to_string()
}

fn list_audit_log(config: &Configuration, limit: Option<i64>, offset: i64, format: &str) {
    let response = match apis::access_control_api::list_admin_audit_log(config, Some(offset), limit)
    {
        Ok(resp) => resp,
        Err(e) => {
            print_error("listing admin audit log", &e);
            std::process::exit(1);
        }
    };

    // JSON output carries the full response, including pagination metadata.
    if format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&response).unwrap_or_default()
        );
        return;
    }

    // Timestamps render the same way as `torc results list` (local time,
    // HUMAN_TIMESTAMP_FORMAT); the audit log stores epoch milliseconds.
    let rows: Vec<AuditLogTableRow> = response
        .items
        .iter()
        .map(|entry| AuditLogTableRow {
            id: entry.id,
            time: format_local_timestamp_epoch(entry.timestamp as f64 / 1000.0),
            user: entry.user_name.clone(),
            committed: yes_no(entry.committed),
            success: yes_no(entry.success),
            full_table: yes_no(entry.allow_full_table),
            rows_affected: entry
                .rows_affected
                .map(|n| n.to_string())
                .unwrap_or_else(|| "-".to_string()),
            sql: entry.sql_text.clone(),
            error: entry.error.clone().unwrap_or_default(),
        })
        .collect();

    if format == "csv" {
        display_csv(&rows);
        return;
    }

    if rows.is_empty() {
        println!("(no audit-log entries)");
        return;
    }
    display_table(&rows);
    println!(
        "\nShowing {} of {} entr{} (offset {})",
        response.count,
        response.total_count,
        if response.total_count == 1 {
            "y"
        } else {
            "ies"
        },
        response.offset
    );
    if response.has_more {
        println!(
            "More available: --offset {} to continue.",
            response.offset + response.count
        );
    }
}

/// Render one JSON cell value as a plain table/CSV cell: strings unquoted, null
/// as empty, scalars stringified, and arrays/objects as compact JSON.
fn value_to_cell(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        other => other.to_string(),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiStatsBucket {
    start_ms: i64,
    request_count: u64,
    bytes_in: u64,
    bytes_out: u64,
    status_2xx: u64,
    status_4xx: u64,
    status_5xx: u64,
    #[serde(default)]
    status_other: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiStatsSnapshot {
    now_ms: i64,
    interval_seconds: u64,
    window_seconds: u64,
    buckets: Vec<ApiStatsBucket>,
}

fn show_api_stats(config: &Configuration, window: u64, interval: u64, format: &str) {
    let url = format!(
        "{}/admin/api-stats?window_seconds={window}&interval_seconds={interval}",
        config.base_path
    );

    let request = config.client.get(&url).header("Accept", "application/json");
    let request = config.apply_auth(request);

    let snapshot: ApiStatsSnapshot = match request.send() {
        Ok(resp) if resp.status().is_success() => match resp.json() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to parse api-stats response: {}", e);
                std::process::exit(1);
            }
        },
        Ok(resp) => {
            eprintln!("Server returned error status: {}", resp.status());
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to fetch api-stats: {}", e);
            std::process::exit(1);
        }
    };

    if format == "json" {
        match serde_json::to_string_pretty(&snapshot) {
            Ok(s) => println!("{}", s),
            Err(e) => eprintln!("Failed to serialize: {}", e),
        }
        return;
    }

    print_stats_table(&snapshot);
}

fn print_stats_table(snap: &ApiStatsSnapshot) {
    let interval = snap.interval_seconds.max(1);
    let mut total_requests = 0u64;
    let mut total_bytes_in = 0u64;
    let mut total_bytes_out = 0u64;

    println!(
        "Window: last {}s in {}s buckets (newest first)\n",
        snap.window_seconds, snap.interval_seconds
    );
    println!(
        "{:<22} {:>8} {:>8} {:>10} {:>10} {:>6} {:>6} {:>6}",
        "bucket start", "req", "req/s", "in", "out", "2xx", "4xx", "5xx"
    );
    println!("{}", "-".repeat(82));

    for b in &snap.buckets {
        let req_per_s = b.request_count as f64 / interval as f64;
        total_requests += b.request_count;
        total_bytes_in += b.bytes_in;
        total_bytes_out += b.bytes_out;
        println!(
            "{:<22} {:>8} {:>8.2} {:>10} {:>10} {:>6} {:>6} {:>6}",
            format_ms(b.start_ms),
            b.request_count,
            req_per_s,
            humanize_bytes(b.bytes_in),
            humanize_bytes(b.bytes_out),
            b.status_2xx,
            b.status_4xx,
            b.status_5xx,
        );
    }

    println!("{}", "-".repeat(82));
    println!(
        "Total: {} requests, {} in, {} out over {}s",
        total_requests,
        humanize_bytes(total_bytes_in),
        humanize_bytes(total_bytes_out),
        snap.window_seconds
    );
}

fn humanize_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", value, UNITS[unit])
    }
}
