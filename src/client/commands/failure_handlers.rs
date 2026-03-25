use crate::client::apis;
use crate::client::apis::configuration::Configuration;
use crate::client::commands::get_env_user_name;
use crate::client::commands::output::{print_if_json, print_wrapped_if_json};
use crate::client::commands::{
    print_error, select_workflow_interactively, table_format::display_table_with_count,
};
use tabled::Tabled;

#[derive(Tabled)]
struct FailureHandlerTableRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Rules")]
    rules: String,
}

#[derive(clap::Subcommand)]
pub enum FailureHandlerCommands {
    /// List failure handlers for a workflow
    List {
        /// List failure handlers for this workflow (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Maximum number of failure handlers to return (default: all)
        #[arg(short, long)]
        limit: Option<i64>,
        /// Offset for pagination (0-based)
        #[arg(long, default_value = "0")]
        offset: i64,
    },
    /// Get a specific failure handler by ID
    Get {
        /// ID of the failure handler to get
        #[arg()]
        id: i64,
    },
}

pub fn handle_failure_handler_commands(
    config: &Configuration,
    command: &FailureHandlerCommands,
    format: &str,
) {
    match command {
        FailureHandlerCommands::List {
            workflow_id,
            limit,
            offset,
        } => {
            let user_name = get_env_user_name();
            let selected_workflow_id = match workflow_id {
                Some(id) => *id,
                None => select_workflow_interactively(config, &user_name).unwrap(),
            };

            match apis::failure_handlers_api::list_failure_handlers(
                config,
                selected_workflow_id,
                Some(*offset),
                *limit,
            ) {
                Ok(response) => {
                    let handlers = response.items;
                    if print_wrapped_if_json(
                        format,
                        "failure_handlers",
                        &handlers,
                        "failure handlers",
                    ) {
                        // JSON was printed
                    } else if handlers.is_empty() {
                        println!(
                            "No failure handlers found for workflow ID: {}",
                            selected_workflow_id
                        );
                    } else {
                        println!("Failure handlers for workflow ID {}:", selected_workflow_id);
                        let rows: Vec<FailureHandlerTableRow> = handlers
                            .iter()
                            .map(|handler| {
                                // Parse and format the rules for display
                                let rules_display = format_rules_for_display(&handler.rules);
                                FailureHandlerTableRow {
                                    id: handler.id.unwrap_or(-1),
                                    name: handler.name.clone(),
                                    rules: rules_display,
                                }
                            })
                            .collect();
                        display_table_with_count(&rows, "failure handlers");
                    }
                }
                Err(e) => {
                    print_error("listing failure handlers", &e);
                    std::process::exit(1);
                }
            }
        }
        FailureHandlerCommands::Get { id } => {
            match apis::failure_handlers_api::get_failure_handler(config, *id) {
                Ok(handler) => {
                    if print_if_json(format, &handler, "failure handler") {
                        // JSON was printed
                    } else {
                        println!("Failure handler ID {}:", id);
                        println!("  Workflow ID: {}", handler.workflow_id);
                        println!("  Name: {}", handler.name);
                        println!("  Rules:");
                        print_rules_detailed(&handler.rules);
                    }
                }
                Err(e) => {
                    print_error("getting failure handler", &e);
                    std::process::exit(1);
                }
            }
        }
    }
}

/// Format rules for table display (compact summary)
fn format_rules_for_display(rules_json: &str) -> String {
    // Try to parse and format as a summary
    if let Ok(rules) = serde_json::from_str::<Vec<serde_json::Value>>(rules_json) {
        let summaries: Vec<String> = rules
            .iter()
            .filter_map(|rule| {
                let exit_codes = rule.get("exit_codes")?;
                let max_retries = rule
                    .get("max_retries")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(3);
                let has_script = rule.get("recovery_script").is_some();
                let script_indicator = if has_script { "+script" } else { "" };
                Some(format!(
                    "{}: {} retries{}",
                    exit_codes, max_retries, script_indicator
                ))
            })
            .collect();
        if summaries.is_empty() {
            rules_json.to_string()
        } else {
            summaries.join("; ")
        }
    } else {
        rules_json.to_string()
    }
}

/// Print rules in detailed format
fn print_rules_detailed(rules_json: &str) {
    if let Ok(rules) = serde_json::from_str::<Vec<serde_json::Value>>(rules_json) {
        for (i, rule) in rules.iter().enumerate() {
            println!("    Rule {}:", i + 1);
            if let Some(exit_codes) = rule.get("exit_codes") {
                println!("      Exit codes: {}", exit_codes);
            }
            if let Some(max_retries) = rule.get("max_retries") {
                println!("      Max retries: {}", max_retries);
            }
            if let Some(script) = rule.get("recovery_script") {
                println!("      Recovery script: {}", script);
            }
        }
    } else {
        println!("    (raw JSON): {}", rules_json);
    }
}
