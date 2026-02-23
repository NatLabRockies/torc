pub mod access_groups;
pub mod admin;
pub mod compute_nodes;
pub mod config;
pub mod events;
pub mod failure_handlers;
pub mod files;
pub mod hpc;
pub mod job_dependencies;
pub mod jobs;
pub mod logs;
pub mod orphan_detection;
pub mod output;
pub mod pagination;
pub mod recover;
pub mod remote;
pub mod reports;
pub mod resource_requirements;
pub mod results;
pub mod ro_crate;
pub mod scheduled_compute_nodes;
pub mod slurm;
pub mod table_format;
pub mod user_data;
pub mod watch;
pub mod workflow_export;
pub mod workflows;

use std::env;
use std::io::{self, Write};

use crate::client::apis::configuration::Configuration;
use crate::client::apis::default_api;

/// Helper function to prompt user to select a workflow when workflow_id is not provided
pub fn select_workflow_interactively(
    config: &Configuration,
    user: &str,
) -> Result<i64, Box<dyn std::error::Error>> {
    match default_api::list_workflows(
        config,
        None,     // offset
        None,     // sort_by
        None,     // reverse_sort
        Some(50), // limit
        None,     // name filter
        Some(user),
        None,        // description filter
        Some(false), // is_archived - exclude archived workflows
    ) {
        Ok(response) => {
            let workflows = response.items.unwrap_or_default();
            if workflows.is_empty() {
                eprintln!("No workflows found for user: {}", user);
                std::process::exit(1);
            }

            if workflows.len() == 1 {
                let workflow_id = workflows[0].id.unwrap_or(-1);
                return Ok(workflow_id);
            }

            println!("Available workflows:");
            println!(
                "{:<5} {:<30} {:<30} {:<20}",
                "ID", "Name", "Description", "Created"
            );
            println!("{}", "-".repeat(105));
            for workflow in workflows.iter() {
                let desc = workflow.description.as_deref().unwrap_or("");
                let timestamp = workflow.timestamp.as_deref().unwrap_or("");
                println!(
                    "{:<5} {:<30} {:<30} {:<20}",
                    workflow.id.unwrap_or(-1),
                    truncate_string(&workflow.name, 30),
                    truncate_string(desc, 30),
                    truncate_string(timestamp, 20)
                );
            }

            println!("\nEnter workflow ID: ");
            io::stdout().flush().unwrap();
            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(_) => match input.trim().parse::<i64>() {
                    Ok(id) => {
                        if workflows.iter().any(|w| w.id == Some(id)) {
                            Ok(id)
                        } else {
                            eprintln!("Invalid workflow ID: {}", id);
                            std::process::exit(1);
                        }
                    }
                    Err(_) => {
                        eprintln!("Invalid input. Please enter a numeric workflow ID.");
                        std::process::exit(1);
                    }
                },
                Err(e) => {
                    eprintln!("Error reading input: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            print_error("listing workflows", &e);
            std::process::exit(1);
        }
    }
}

/// Helper function to get user name from parameter or environment variables
pub fn get_user_name(user: &Option<String>) -> String {
    if user.is_some() {
        return user.as_deref().unwrap().to_string();
    }
    get_env_user_name()
}

pub fn get_env_user_name() -> String {
    env::var("USER")
        .or_else(|_| env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Truncate string to specified length
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Print API errors in a user-friendly way
pub fn print_error<T>(operation: &str, error: &crate::client::apis::Error<T>) {
    match error {
        crate::client::apis::Error::Reqwest(e) => {
            eprintln!("Network error while {}: {}", operation, e);
            if e.is_timeout() {
                eprintln!("Hint: Check if the Torc service is running and accessible");
            }
        }
        crate::client::apis::Error::Serde(e) => {
            eprintln!("Data parsing error while {}: {}", operation, e);
        }
        crate::client::apis::Error::Io(e) => {
            eprintln!("IO error while {}: {}", operation, e);
        }
        crate::client::apis::Error::ResponseError(resp) => {
            eprintln!(
                "API error while {} (status: {}): {}",
                operation, resp.status, resp.content
            );
            if resp.status == 404 {
                eprintln!("Hint: Check if the resource exists or the API endpoint is correct");
            } else if resp.status == 401 || resp.status == 403 {
                eprintln!("Hint: Check your authentication credentials");
            } else if resp.status.is_server_error() {
                eprintln!("Hint: This appears to be a server-side issue");
            }
        }
    }
}
