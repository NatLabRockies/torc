use crate::client::apis::configuration::Configuration;
use crate::client::apis::default_api;
use crate::client::commands::output::print_if_json;
use crate::client::commands::table_format::display_table_with_count;
use crate::client::commands::{get_env_user_name, print_error, select_workflow_interactively};
use clap::Subcommand;
use tabled::Tabled;

#[derive(Subcommand)]
pub enum JobDependencyCommands {
    /// List job-to-job dependencies for a workflow
    JobJob {
        /// ID of the workflow (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Maximum number of dependencies to return (default: all)
        #[arg(short, long)]
        limit: Option<i64>,
        /// Offset for pagination (0-based)
        #[arg(long, default_value = "0")]
        offset: i64,
    },
    /// List job-file relationships for a workflow
    JobFile {
        /// ID of the workflow (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Maximum number of relationships to return (default: all)
        #[arg(short, long)]
        limit: Option<i64>,
        /// Offset for pagination (0-based)
        #[arg(long, default_value = "0")]
        offset: i64,
    },
    /// List job-user_data relationships for a workflow
    JobUserData {
        /// ID of the workflow (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Maximum number of relationships to return (default: all)
        #[arg(short, long)]
        limit: Option<i64>,
        /// Offset for pagination (0-based)
        #[arg(long, default_value = "0")]
        offset: i64,
    },
}

#[derive(Tabled)]
struct JobDependencyTableRow {
    #[tabled(rename = "Job ID")]
    job_id: i64,
    #[tabled(rename = "Job Name")]
    job_name: String,
    #[tabled(rename = "Blocked By Job ID")]
    depends_on_job_id: i64,
    #[tabled(rename = "Blocked By Job Name")]
    depends_on_job_name: String,
}

#[derive(Tabled)]
struct JobFileRelationshipTableRow {
    #[tabled(rename = "File ID")]
    file_id: i64,
    #[tabled(rename = "File Name")]
    file_name: String,
    #[tabled(rename = "Producer Job ID")]
    producer_job_id: String,
    #[tabled(rename = "Producer Job Name")]
    producer_job_name: String,
    #[tabled(rename = "Consumer Job ID")]
    consumer_job_id: String,
    #[tabled(rename = "Consumer Job Name")]
    consumer_job_name: String,
}

#[derive(Tabled)]
struct JobUserDataRelationshipTableRow {
    #[tabled(rename = "User Data ID")]
    user_data_id: i64,
    #[tabled(rename = "User Data Name")]
    user_data_name: String,
    #[tabled(rename = "Producer Job ID")]
    producer_job_id: String,
    #[tabled(rename = "Producer Job Name")]
    producer_job_name: String,
    #[tabled(rename = "Consumer Job ID")]
    consumer_job_id: String,
    #[tabled(rename = "Consumer Job Name")]
    consumer_job_name: String,
}

pub fn handle_job_dependency_commands(
    command: &JobDependencyCommands,
    config: &Configuration,
    format: &str,
) {
    match command {
        JobDependencyCommands::JobJob {
            workflow_id,
            limit,
            offset,
        } => {
            let selected_workflow_id = match workflow_id {
                Some(id) => *id,
                None => match select_workflow_interactively(config, &get_env_user_name()) {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error selecting workflow: {}", e);
                        std::process::exit(1);
                    }
                },
            };

            match default_api::list_job_dependencies(
                config,
                selected_workflow_id,
                Some(*offset),
                *limit,
            ) {
                Ok(response) => {
                    if print_if_json(format, &response, "job dependencies") {
                        // JSON was printed
                    } else {
                        let rows: Vec<JobDependencyTableRow> = response
                            .items
                            .unwrap_or_default()
                            .iter()
                            .map(|dep| JobDependencyTableRow {
                                job_id: dep.job_id,
                                job_name: dep.job_name.clone(),
                                depends_on_job_id: dep.depends_on_job_id,
                                depends_on_job_name: dep.depends_on_job_name.clone(),
                            })
                            .collect();

                        display_table_with_count(&rows, "job dependencies");
                    }
                }
                Err(e) => {
                    print_error("listing job dependencies", &e);
                    std::process::exit(1);
                }
            }
        }
        JobDependencyCommands::JobFile {
            workflow_id,
            limit,
            offset,
        } => {
            let selected_workflow_id = match workflow_id {
                Some(id) => *id,
                None => match select_workflow_interactively(config, &get_env_user_name()) {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error selecting workflow: {}", e);
                        std::process::exit(1);
                    }
                },
            };

            match default_api::list_job_file_relationships(
                config,
                selected_workflow_id,
                Some(*offset),
                *limit,
            ) {
                Ok(response) => {
                    if print_if_json(format, &response, "job-file relationships") {
                        // JSON was printed
                    } else {
                        let rows: Vec<JobFileRelationshipTableRow> = response
                            .items
                            .unwrap_or_default()
                            .iter()
                            .map(|rel| JobFileRelationshipTableRow {
                                file_id: rel.file_id,
                                file_name: rel.file_name.clone(),
                                producer_job_id: rel
                                    .producer_job_id
                                    .map(|id| id.to_string())
                                    .unwrap_or_else(|| "None".to_string()),
                                producer_job_name: rel
                                    .producer_job_name
                                    .clone()
                                    .unwrap_or_else(|| "N/A".to_string()),
                                consumer_job_id: rel
                                    .consumer_job_id
                                    .map(|id| id.to_string())
                                    .unwrap_or_else(|| "None".to_string()),
                                consumer_job_name: rel
                                    .consumer_job_name
                                    .clone()
                                    .unwrap_or_else(|| "N/A".to_string()),
                            })
                            .collect();

                        display_table_with_count(&rows, "job-file relationships");
                    }
                }
                Err(e) => {
                    print_error("listing job-file relationships", &e);
                    std::process::exit(1);
                }
            }
        }
        JobDependencyCommands::JobUserData {
            workflow_id,
            limit,
            offset,
        } => {
            let selected_workflow_id = match workflow_id {
                Some(id) => *id,
                None => match select_workflow_interactively(config, &get_env_user_name()) {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error selecting workflow: {}", e);
                        std::process::exit(1);
                    }
                },
            };

            match default_api::list_job_user_data_relationships(
                config,
                selected_workflow_id,
                Some(*offset),
                *limit,
            ) {
                Ok(response) => {
                    if print_if_json(format, &response, "job-user_data relationships") {
                        // JSON was printed
                    } else {
                        let rows: Vec<JobUserDataRelationshipTableRow> = response
                            .items
                            .unwrap_or_default()
                            .iter()
                            .map(|rel| JobUserDataRelationshipTableRow {
                                user_data_id: rel.user_data_id,
                                user_data_name: rel.user_data_name.clone(),
                                producer_job_id: rel
                                    .producer_job_id
                                    .map(|id| id.to_string())
                                    .unwrap_or_else(|| "None".to_string()),
                                producer_job_name: rel
                                    .producer_job_name
                                    .clone()
                                    .unwrap_or_else(|| "N/A".to_string()),
                                consumer_job_id: rel
                                    .consumer_job_id
                                    .map(|id| id.to_string())
                                    .unwrap_or_else(|| "None".to_string()),
                                consumer_job_name: rel
                                    .consumer_job_name
                                    .clone()
                                    .unwrap_or_else(|| "N/A".to_string()),
                            })
                            .collect();

                        display_table_with_count(&rows, "job-user_data relationships");
                    }
                }
                Err(e) => {
                    print_error("listing job-user_data relationships", &e);
                    std::process::exit(1);
                }
            }
        }
    }
}
