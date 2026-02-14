use std::time::Instant;

use crate::client::apis::configuration::Configuration;
use crate::client::apis::default_api;
use crate::client::commands::get_env_user_name;
use crate::client::commands::output::{print_if_json, print_json, print_json_wrapped};
use crate::client::commands::pagination::{EventListParams, paginate_events};
use crate::client::commands::{
    print_error, select_workflow_interactively, table_format::display_table_with_count,
};
use crate::client::sse_client::{SseConnection, SseEvent};
use crate::models;
use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use serde_json;
use tabled::Tabled;

/// Format a timestamp (milliseconds since epoch) as a human-readable local time string
fn format_timestamp_ms(timestamp_ms: i64) -> String {
    DateTime::from_timestamp_millis(timestamp_ms)
        .map(|dt: DateTime<Utc>| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string()
        })
        .unwrap_or_else(|| format!("{}ms", timestamp_ms))
}

/// Event model for JSON output with human-readable timestamp
#[derive(Serialize, Deserialize)]
struct EventJsonOutput {
    id: Option<i64>,
    workflow_id: i64,
    timestamp: i64,
    timestamp_formatted: String,
    data: serde_json::Value,
}

impl From<&models::EventModel> for EventJsonOutput {
    fn from(event: &models::EventModel) -> Self {
        EventJsonOutput {
            id: event.id,
            workflow_id: event.workflow_id,
            timestamp: event.timestamp,
            timestamp_formatted: format_timestamp_ms(event.timestamp),
            data: event.data.clone(),
        }
    }
}

#[derive(Tabled)]
struct EventTableRow {
    #[tabled(rename = "Timestamp")]
    timestamp: String,
    #[tabled(rename = "Level")]
    level: String,
    #[tabled(rename = "Event Type")]
    event_type: String,
    #[tabled(rename = "Data")]
    data: String,
}

#[derive(clap::Subcommand)]
#[command(after_long_help = "\
EXAMPLES:
    # List events for a workflow
    torc events list 123

    # Monitor events in real-time
    torc events monitor 123 --type job_started

    # Get JSON output
    torc -f json events list 123
")]
pub enum EventCommands {
    /// Create a new event
    #[command(after_long_help = "\
EXAMPLES:
    torc events create 123 --data '{\"type\": \"custom\", \"message\": \"hello\"}'
")]
    Create {
        /// Create the event in this workflow.
        #[arg()]
        workflow_id: Option<i64>,
        /// JSON data for the event
        #[arg(short, long)]
        data: String,
    },
    /// List events for a workflow
    #[command(after_long_help = "\
EXAMPLES:
    torc events list 123
    torc events list 123 --type user_action
    torc -f json events list 123
")]
    List {
        /// List events for this workflow (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Filter events by type or category
        #[arg(short = 't', long = "type", alias = "category")]
        event_type: Option<String>,
        /// Maximum number of events to return (default: all)
        #[arg(short, long)]
        limit: Option<i64>,
        /// Offset for pagination (0-based)
        #[arg(short, long, default_value = "0")]
        offset: i64,
        /// Field to sort by
        #[arg(short, long)]
        sort_by: Option<String>,
        /// Reverse sort order
        #[arg(short, long, default_value = "false")]
        reverse_sort: bool,
    },
    /// Monitor events for a workflow in real-time
    #[command(after_long_help = "\
EXAMPLES:
    torc events monitor 123
    torc events monitor 123 --level warning --filename events.log
")]
    Monitor {
        /// Monitor events for this workflow (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Duration to monitor in minutes (default: infinite)
        #[arg(short, long)]
        duration: Option<i64>,
        /// Filter events by level (default: info). Values: debug, info, warning, error
        #[arg(long, default_value = "info")]
        level: Option<models::EventSeverity>,
        /// Log events to this file
        #[arg(long)]
        filename: Option<String>,
    },
    /// Get the latest event for a workflow
    GetLatestEvent {
        /// Get the latest event for this workflow (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
    },
    /// Delete an event
    Delete {
        /// ID of the event to remove
        id: i64,
    },
}

pub fn handle_event_commands(config: &Configuration, command: &EventCommands, format: &str) {
    match command {
        EventCommands::Create { workflow_id, data } => {
            let user_name = crate::client::commands::get_env_user_name();
            let wf_id = workflow_id.unwrap_or_else(|| {
                select_workflow_interactively(config, &user_name).unwrap_or_else(|e| {
                    eprintln!("Error selecting workflow: {}", e);
                    std::process::exit(1);
                })
            });

            let json_data: serde_json::Value = match serde_json::from_str(data) {
                Ok(json) => json,
                Err(e) => {
                    eprintln!("Error parsing JSON data: {}", e);
                    std::process::exit(1);
                }
            };

            let event = models::EventModel::new(wf_id, json_data);

            match default_api::create_event(config, event) {
                Ok(created_event) => {
                    let json_event = EventJsonOutput::from(&created_event);
                    if print_if_json(format, &json_event, "event") {
                        // JSON was printed
                    } else {
                        println!("Successfully created event:");
                        println!("  ID: {}", created_event.id.unwrap_or(-1));
                        println!("  Workflow ID: {}", created_event.workflow_id);
                        println!(
                            "  Timestamp: {}",
                            format_timestamp_ms(created_event.timestamp)
                        );
                        println!(
                            "  Data: {}",
                            serde_json::to_string_pretty(&created_event.data)
                                .unwrap_or_else(|_| "Unable to display data".to_string())
                        );
                    }
                }
                Err(e) => {
                    print_error("creating event", &e);
                    std::process::exit(1);
                }
            }
        }
        EventCommands::List {
            workflow_id,
            event_type,
            limit,
            offset,
            sort_by,
            reverse_sort,
        } => {
            let user_name = get_env_user_name();
            let selected_workflow_id = match workflow_id {
                Some(id) => *id,
                None => select_workflow_interactively(config, &user_name).unwrap(),
            };

            let mut params = EventListParams::new().with_offset(*offset);

            if let Some(limit_val) = limit {
                params = params.with_limit(*limit_val);
            }

            if let Some(event_type_str) = event_type {
                params = params.with_category(event_type_str.clone());
            }

            if let Some(sort_by_str) = sort_by {
                params = params.with_sort_by(sort_by_str.clone());
            }

            params = params.with_reverse_sort(*reverse_sort);

            match paginate_events(config, selected_workflow_id as i64, params) {
                Ok(events) => {
                    if format == "json" {
                        let json_events: Vec<EventJsonOutput> =
                            events.iter().map(EventJsonOutput::from).collect();
                        print_json_wrapped("events", &json_events, "events");
                    } else if events.is_empty() {
                        println!("No events found for workflow {}", selected_workflow_id);
                    } else {
                        println!("Events for workflow {}:", selected_workflow_id);
                        let rows: Vec<EventTableRow> = events
                            .iter()
                            .map(|event| {
                                let etype = event
                                    .data
                                    .get("category")
                                    .or_else(|| event.data.get("event_type"))
                                    .or_else(|| event.data.get("type"))
                                    .or_else(|| event.data.get("action"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("-")
                                    .to_string();
                                let level = event
                                    .data
                                    .get("severity")
                                    .or_else(|| event.data.get("level"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("info")
                                    .to_string();

                                EventTableRow {
                                    timestamp: format_timestamp_ms(event.timestamp),
                                    level,
                                    event_type: etype,
                                    data: serde_json::to_string(&event.data)
                                        .unwrap_or_else(|_| "Unable to display".to_string()),
                                }
                            })
                            .collect();
                        display_table_with_count(&rows, "events");
                    }
                }
                Err(e) => {
                    print_error("listing events", &e);
                    std::process::exit(1);
                }
            }
        }
        EventCommands::Monitor {
            workflow_id,
            duration,
            level,
            filename,
        } => {
            let user_name = get_env_user_name();
            let selected_workflow_id = match workflow_id {
                Some(id) => *id,
                None => select_workflow_interactively(config, &user_name).unwrap(),
            };

            handle_monitor_events(
                config,
                selected_workflow_id,
                *duration,
                *level,
                filename.clone(),
                format,
            );
        }
        EventCommands::GetLatestEvent { workflow_id } => {
            let user_name = get_env_user_name();
            let selected_workflow_id = match workflow_id {
                Some(id) => *id,
                None => select_workflow_interactively(config, &user_name).unwrap(),
            };

            match default_api::list_events(
                config,
                selected_workflow_id as i64,
                None,              // offset
                Some(1),           // limit to 1 event
                Some("timestamp"), // sort by timestamp
                Some(true),        // reverse sort (newest first)
                None,              // category
                None,              // after_timestamp
            ) {
                Ok(response) => {
                    if let Some(events) = response.items {
                        if let Some(latest_event) = events.first() {
                            let json_event = EventJsonOutput::from(latest_event);
                            if print_if_json(format, &json_event, "event") {
                                // JSON was printed
                            } else {
                                println!("Latest event for workflow {}:", selected_workflow_id);
                                println!("  ID: {}", latest_event.id.unwrap_or(-1));
                                println!(
                                    "  Timestamp: {}",
                                    format_timestamp_ms(latest_event.timestamp)
                                );
                                println!(
                                    "  Data: {}",
                                    serde_json::to_string_pretty(&latest_event.data)
                                        .unwrap_or_else(|_| "Unable to display data".to_string())
                                );
                            }
                        } else {
                            println!("No events found for workflow {}", selected_workflow_id);
                        }
                    } else {
                        println!("No events found for workflow {}", selected_workflow_id);
                    }
                }
                Err(e) => {
                    print_error("getting latest event", &e);
                    std::process::exit(1);
                }
            }
        }
        EventCommands::Delete { id } => match default_api::delete_event(config, *id, None) {
            Ok(removed_event) => {
                let json_event = EventJsonOutput::from(&removed_event);
                if print_if_json(format, &json_event, "event") {
                    // JSON was printed
                } else {
                    println!("Successfully removed event:");
                    println!("  ID: {}", removed_event.id.unwrap_or(-1));
                    println!("  Workflow ID: {}", removed_event.workflow_id);
                }
            }
            Err(e) => {
                print_error("removing event", &e);
                std::process::exit(1);
            }
        },
    }
}

/// Event output format for SSE events (for JSON output)
#[derive(Serialize, Deserialize)]
struct SseEventJsonOutput {
    workflow_id: i64,
    timestamp: i64,
    timestamp_formatted: String,
    event_type: String,
    severity: models::EventSeverity,
    data: serde_json::Value,
}

impl From<&SseEvent> for SseEventJsonOutput {
    fn from(event: &SseEvent) -> Self {
        SseEventJsonOutput {
            workflow_id: event.workflow_id,
            timestamp: event.timestamp,
            timestamp_formatted: format_timestamp_ms(event.timestamp),
            event_type: event.event_type.clone(),
            severity: event.severity,
            data: event.data.clone(),
        }
    }
}

fn handle_monitor_events(
    config: &Configuration,
    workflow_id: i64,
    duration: Option<i64>,
    level: Option<models::EventSeverity>,
    filename: Option<String>,
    format: &str,
) {
    let start_time = Instant::now();
    let duration_seconds = duration.map(|d| d * 60); // Convert minutes to seconds

    eprintln!(
        "Monitoring events for workflow {} via SSE (real-time streaming{})",
        workflow_id,
        match duration {
            Some(d) => format!(", duration: {} minutes", d),
            None => String::from(", duration: infinite"),
        }
    );

    if let Some(lvl) = level {
        eprintln!("Filtering by level: {}", lvl);
    }

    if let Some(ref fname) = filename {
        eprintln!("Logging events to file: {}", fname);
    }

    eprintln!("Press Ctrl+C to stop monitoring\n");

    // Open log file if specified
    let mut log_file = if let Some(ref fname) = filename {
        use std::fs::OpenOptions;
        match OpenOptions::new().create(true).append(true).open(fname) {
            Ok(file) => Some(file),
            Err(e) => {
                eprintln!("Failed to open log file '{}': {}", fname, e);
                std::process::exit(1);
            }
        }
    } else {
        None
    };

    // Connect to SSE endpoint
    let mut connection = match SseConnection::connect(config, workflow_id, level) {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("Failed to connect to SSE endpoint: {}", e);
            eprintln!(
                "Make sure the server supports SSE at /workflows/{}/events/stream",
                workflow_id
            );
            std::process::exit(1);
        }
    };

    eprintln!("Connected to SSE stream. Waiting for events...\n");

    loop {
        // Check if we've exceeded the duration
        if let Some(max_seconds) = duration_seconds
            && start_time.elapsed().as_secs() >= max_seconds as u64
        {
            println!("\nMonitoring duration completed.");
            break;
        }

        // Read next event from SSE stream
        match connection.next_event() {
            Ok(Some(event)) => {
                // Note: Level filtering is handled by the server via query param,
                // but we could also filter client-side if needed.

                // Output the event
                if format == "json" {
                    let json_event = SseEventJsonOutput::from(&event);
                    let json_str = serde_json::to_string(&json_event).unwrap_or_default();

                    print_json(&json_event, "event");

                    // Also write to log file if enabled
                    if let Some(ref mut file) = log_file {
                        use std::io::Write;
                        if let Err(e) = writeln!(file, "{}", json_str) {
                            eprintln!("Error writing to log file: {}", e);
                        }
                    }
                } else {
                    let output_str = format!(
                        "[{}] [{}] {}: {}",
                        format_timestamp_ms(event.timestamp),
                        event.severity,
                        event.event_type,
                        serde_json::to_string(&event.data)
                            .unwrap_or_else(|_| "Unable to display".to_string())
                    );

                    println!("{}", output_str);

                    // Also write to log file if enabled
                    if let Some(ref mut file) = log_file {
                        use std::io::Write;
                        if let Err(e) = writeln!(file, "{}", output_str) {
                            eprintln!("Error writing to log file: {}", e);
                        }
                    }
                }
            }
            Ok(None) => {
                // Connection closed
                eprintln!("\nSSE connection closed by server.");
                break;
            }
            Err(e) => {
                eprintln!("\nError reading SSE stream: {}", e);
                break;
            }
        }
    }
}
