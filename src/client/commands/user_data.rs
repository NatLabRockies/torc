use clap::Subcommand;
use serde_json;

use crate::client::apis;
use crate::client::apis::configuration::Configuration;
use crate::client::commands::output::{print_if_json, print_json, print_json_wrapped};
use crate::client::commands::{get_env_user_name, pagination};
use crate::client::commands::{
    print_error, select_workflow_interactively, table_format::display_table_with_count,
};
use crate::models;
use tabled::Tabled;

#[derive(Tabled)]
struct UserDataTableRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Ephemeral")]
    ephemeral: String,
    #[tabled(rename = "Data Preview")]
    data_preview: String,
}

#[derive(Subcommand)]
#[command(after_long_help = "\
EXAMPLES:
    # List user data for a workflow
    torc user-data list 123

    # Get JSON output
    torc -f json user-data list 123

    # Find missing user data
    torc user-data list-missing 123
")]
pub enum UserDataCommands {
    /// Create a new user data record
    #[command(after_long_help = "\
EXAMPLES:
    torc user-data create 123 --name config --data '{\"key\": \"value\"}'
    torc user-data create 123 --name temp_data --ephemeral
")]
    Create {
        /// Workflow ID
        #[arg()]
        workflow_id: Option<i64>,
        /// Name of the data object
        #[arg(short, long)]
        name: String,
        /// JSON data content
        #[arg(short, long)]
        data: Option<String>,
        /// Whether the data is ephemeral (cleared between runs)
        #[arg(long)]
        ephemeral: bool,
        /// Consumer job ID (optional)
        #[arg(long)]
        consumer_job_id: Option<i64>,
        /// Producer job ID (optional)
        #[arg(long)]
        producer_job_id: Option<i64>,
    },
    /// List user data records
    List {
        /// Workflow ID (if not provided, will be selected interactively)
        workflow_id: Option<i64>,
        /// Maximum number of records to return
        #[arg(short, long, default_value = "50")]
        limit: i64,
        /// Number of records to skip
        #[arg(short, long, default_value = "0")]
        offset: i64,
        /// Field to sort by
        #[arg(long)]
        sort_by: Option<String>,
        /// Reverse sort order
        #[arg(long)]
        reverse_sort: bool,
        /// Filter by name
        #[arg(long)]
        name: Option<String>,
        /// Filter by ephemeral status
        #[arg(long)]
        is_ephemeral: Option<bool>,
        /// Filter by consumer job ID
        #[arg(long)]
        consumer_job_id: Option<i64>,
        /// Filter by producer job ID
        #[arg(long)]
        producer_job_id: Option<i64>,
    },
    /// Get a specific user data record
    Get {
        /// User data record ID
        id: i64,
    },
    /// Update a user data record
    Update {
        /// User data record ID
        id: i64,
        /// New name for the data object
        #[arg(short, long)]
        name: Option<String>,
        /// New JSON data content
        #[arg(short, long)]
        data: Option<String>,
        /// Update ephemeral status
        #[arg(long)]
        ephemeral: Option<bool>,
    },
    /// Delete a user data record
    Delete {
        /// User data record ID
        id: i64,
    },
    /// Delete all user data records for a workflow
    DeleteAll {
        /// Workflow ID
        workflow_id: i64,
    },
    /// List missing user data for a workflow
    ListMissing {
        /// Workflow ID
        workflow_id: i64,
    },
}

pub fn handle_user_data_commands(config: &Configuration, command: &UserDataCommands, format: &str) {
    match command {
        UserDataCommands::Create {
            workflow_id,
            name,
            data,
            ephemeral,
            consumer_job_id,
            producer_job_id,
        } => {
            let user_name = crate::client::commands::get_env_user_name();
            let wf_id = workflow_id.unwrap_or_else(|| {
                select_workflow_interactively(config, &user_name).unwrap_or_else(|e| {
                    eprintln!("Error selecting workflow: {}", e);
                    std::process::exit(1);
                })
            });

            let mut user_data = models::UserDataModel::new(wf_id, name.clone());

            if let Some(data_str) = data {
                match serde_json::from_str::<serde_json::Value>(data_str) {
                    Ok(json_data) => {
                        user_data.data = Some(json_data);
                    }
                    Err(e) => {
                        eprintln!("Error parsing JSON data: {}", e);
                        std::process::exit(1);
                    }
                }
            }

            user_data.is_ephemeral = Some(*ephemeral);

            match apis::user_data_api::create_user_data(
                config,
                user_data,
                *consumer_job_id,
                *producer_job_id,
            ) {
                Ok(created_user_data) => {
                    if print_if_json(format, &created_user_data, "user data") {
                        // JSON was printed
                    } else {
                        println!("Successfully created user data:");
                        println!("  ID: {}", created_user_data.id.unwrap_or(-1));
                        println!("  Workflow ID: {}", created_user_data.workflow_id);
                        println!("  Name: {:?}", created_user_data.name);
                        println!("  Is Ephemeral: {:?}", created_user_data.is_ephemeral);
                        if let Some(data) = &created_user_data.data {
                            println!(
                                "  Data: {}",
                                serde_json::to_string_pretty(data).unwrap_or_default()
                            );
                        }
                    }
                }
                Err(e) => {
                    print_error("creating user data", &e);
                    std::process::exit(1);
                }
            }
        }
        UserDataCommands::List {
            workflow_id,
            limit,
            offset,
            sort_by,
            reverse_sort,
            name,
            is_ephemeral,
            consumer_job_id,
            producer_job_id,
        } => {
            let user_name = get_env_user_name();
            let selected_workflow_id = match workflow_id {
                Some(id) => *id,
                None => select_workflow_interactively(config, &user_name).unwrap(),
            };

            let mut params = pagination::UserDataListParams::new()
                .with_offset(*offset)
                .with_limit(*limit)
                .with_reverse_sort(*reverse_sort);

            if let Some(consumer_id) = consumer_job_id {
                params = params.with_consumer_job_id(*consumer_id);
            }
            if let Some(producer_id) = producer_job_id {
                params = params.with_producer_job_id(*producer_id);
            }
            if let Some(sort_field) = sort_by {
                params = params.with_sort_by(sort_field.clone());
            }
            if let Some(name_filter) = name {
                params = params.with_name(name_filter.clone());
            }
            if let Some(ephemeral_filter) = is_ephemeral {
                params = params.with_is_ephemeral(*ephemeral_filter);
            }

            match pagination::paginate_user_data(config, selected_workflow_id, params) {
                Ok(user_data_list) => {
                    if format == "json" {
                        print_json_wrapped("user_data", &user_data_list, "user_data");
                    } else if user_data_list.is_empty() {
                        println!(
                            "No user data found for workflow ID: {}",
                            selected_workflow_id
                        );
                    } else {
                        println!("User data for workflow ID {}:", selected_workflow_id);
                        let rows: Vec<UserDataTableRow> = user_data_list
                            .iter()
                            .map(|user_data| UserDataTableRow {
                                id: user_data.id.unwrap_or(-1),
                                name: user_data.name.clone(),
                                ephemeral: user_data
                                    .is_ephemeral
                                    .map_or("N/A".to_string(), |e| e.to_string()),
                                data_preview: user_data
                                    .data
                                    .as_ref()
                                    .and_then(|d| serde_json::to_string(d).ok())
                                    .unwrap_or_else(|| "N/A".to_string()),
                            })
                            .collect();
                        display_table_with_count(&rows, "user data records");
                    }
                }
                Err(e) => {
                    print_error("listing user data", &e);
                    std::process::exit(1);
                }
            }
        }
        UserDataCommands::Get { id } => match apis::user_data_api::get_user_data(config, *id) {
            Ok(user_data) => {
                if print_if_json(format, &user_data, "user data") {
                    // JSON was printed
                } else {
                    println!("User data ID {}:", id);
                    println!("  Workflow ID: {}", user_data.workflow_id);
                    println!("  Name: {}", user_data.name);
                    println!("  Is Ephemeral: {:?}", user_data.is_ephemeral);
                    if let Some(data) = &user_data.data {
                        println!(
                            "  Data: {}",
                            serde_json::to_string_pretty(data).unwrap_or_default()
                        );
                    } else {
                        println!("  Data: None");
                    }
                }
            }
            Err(e) => {
                print_error("getting user data", &e);
                std::process::exit(1);
            }
        },
        UserDataCommands::Update {
            id,
            name,
            data,
            ephemeral,
        } => {
            // First get the existing user data
            match apis::user_data_api::get_user_data(config, *id) {
                Ok(mut user_data) => {
                    // Update fields that were provided
                    if let Some(new_name) = name {
                        user_data.name = new_name.clone();
                    }

                    if let Some(data_str) = data {
                        match serde_json::from_str::<serde_json::Value>(data_str) {
                            Ok(json_data) => {
                                user_data.data = Some(json_data);
                            }
                            Err(e) => {
                                eprintln!("Error parsing JSON data: {}", e);
                                std::process::exit(1);
                            }
                        }
                    }

                    if let Some(new_ephemeral) = ephemeral {
                        user_data.is_ephemeral = Some(*new_ephemeral);
                    }

                    match apis::user_data_api::update_user_data(config, *id, user_data) {
                        Ok(updated_user_data) => {
                            if print_if_json(format, &updated_user_data, "user data") {
                                // JSON was printed
                            } else {
                                println!("Successfully updated user data:");
                                println!("  ID: {}", updated_user_data.id.unwrap_or(-1));
                                println!("  Workflow ID: {}", updated_user_data.workflow_id);
                                println!("  Name: {:?}", updated_user_data.name);
                                println!("  Is Ephemeral: {:?}", updated_user_data.is_ephemeral);
                                if let Some(data) = &updated_user_data.data {
                                    println!(
                                        "  Data: {}",
                                        serde_json::to_string_pretty(data).unwrap_or_default()
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            print_error("updating user data", &e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    print_error("getting user data for update", &e);
                    std::process::exit(1);
                }
            }
        }
        UserDataCommands::Delete { id } => {
            match apis::user_data_api::delete_user_data(config, *id) {
                Ok(removed_user_data) => {
                    if print_if_json(format, &removed_user_data, "user data") {
                        // JSON was printed
                    } else {
                        println!("Successfully removed user data:");
                        println!("  ID: {}", removed_user_data.id.unwrap_or(-1));
                        println!("  Workflow ID: {}", removed_user_data.workflow_id);
                        println!("  Name: {:?}", removed_user_data.name);
                    }
                }
                Err(e) => {
                    print_error("removing user data", &e);
                    std::process::exit(1);
                }
            }
        }
        UserDataCommands::DeleteAll { workflow_id } => {
            match apis::user_data_api::delete_all_user_data(config, *workflow_id) {
                Ok(response) => {
                    if format == "json" {
                        print_json(&response, "user data delete-all response");
                    } else {
                        println!(
                            "Successfully deleted all user data for workflow ID: {}",
                            workflow_id
                        );
                    }
                }
                Err(e) => {
                    print_error("deleting user data", &e);
                    std::process::exit(1);
                }
            }
        }
        UserDataCommands::ListMissing { workflow_id } => {
            match apis::workflows_api::list_missing_user_data(config, *workflow_id) {
                Ok(missing_data) => {
                    if print_if_json(format, &missing_data, "missing user data") {
                        // JSON was printed
                    } else if missing_data.user_data.is_empty() {
                        println!("No missing user data for workflow ID: {}", workflow_id);
                    } else {
                        println!("Missing user data for workflow ID {}:", workflow_id);
                        println!("{:<30}", "Missing User Data ID");
                        println!("{}", "-".repeat(30));
                        for user_data_id in &missing_data.user_data {
                            println!("{:<30}", user_data_id);
                        }
                    }
                }
                Err(e) => {
                    print_error("listing missing user data", &e);
                    std::process::exit(1);
                }
            }
        }
    }
}
