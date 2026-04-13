use chrono::DateTime;
use clap::Subcommand;

use crate::client::apis;
use crate::client::apis::configuration::Configuration;
use crate::client::commands::get_env_user_name;
use crate::client::commands::{
    output::{print_if_json, print_json_wrapped},
    pagination::{self, FileListParams},
    print_error, select_workflow_interactively,
    table_format::display_table_with_count,
};
use crate::models;
use tabled::Tabled;

/// Format Unix timestamp to human-readable string
fn format_mtime(st_mtime: Option<f64>) -> String {
    match st_mtime {
        Some(timestamp) => {
            let dt = DateTime::from_timestamp(timestamp as i64, 0).unwrap_or_default();
            dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
        }
        None => "N/A".to_string(),
    }
}

#[derive(Tabled)]
struct FileTableRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Path")]
    path: String,
    #[tabled(rename = "Modified Time")]
    st_mtime: String,
}

#[derive(Subcommand)]
#[command(after_long_help = "\
EXAMPLES:
    # List files for a workflow
    torc files list 123

    # Get JSON output
    torc -f json files list 123

    # List required input files
    torc files list-required-existing 123
")]
pub enum FileCommands {
    /// Create a new file
    #[command(after_long_help = "\
EXAMPLES:
    # Create a file record
    torc files create 123 --name input_data --path /data/input.csv
")]
    Create {
        /// Create the file in this workflow.
        #[arg()]
        workflow_id: Option<i64>,
        /// Name of the job
        #[arg(short, long, required = true)]
        name: String,
        /// Path of the file
        #[arg(short, long, required = true)]
        path: String,
    },
    /// List files
    #[command(after_long_help = "\
EXAMPLES:
    # List all files for a workflow
    torc files list 123

    # Get JSON output
    torc -f json files list 123

    # Filter by producing job
    torc files list 123 --produced-by-job-id 456
")]
    List {
        /// List files for this workflow (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Filter by job ID that produced the files
        #[arg(long)]
        produced_by_job_id: Option<i64>,
        /// Maximum number of files to return (default: all)
        #[arg(short, long)]
        limit: Option<i64>,
        /// Offset for pagination (0-based)
        #[arg(long, default_value = "0")]
        offset: i64,
        /// Field to sort by
        #[arg(long)]
        sort_by: Option<String>,
        /// Reverse sort order
        #[arg(long)]
        reverse_sort: bool,
    },
    /// Get a specific file by ID
    #[command(after_long_help = "\
EXAMPLES:
    torc files get 456
    torc -f json files get 456
")]
    Get {
        /// ID of the file to get
        #[arg()]
        id: i64,
    },
    /// Update an existing file
    #[command(after_long_help = "\
EXAMPLES:
    torc files update 456 --name new_name
    torc files update 456 --path /new/path.csv
")]
    Update {
        /// ID of the file to update
        #[arg()]
        id: i64,
        /// Name of the file
        #[arg(short, long)]
        name: Option<String>,
        /// Path of the file
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Delete a file
    #[command(after_long_help = "\
EXAMPLES:
    torc files delete 456
")]
    Delete {
        /// ID of the file to remove
        #[arg()]
        id: i64,
    },
    /// List required existing files for a workflow
    #[command(after_long_help = "\
EXAMPLES:
    # Find files that must exist before workflow can run
    torc files list-required-existing 123
")]
    ListRequiredExisting {
        /// List required existing files for this workflow (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
    },
}

pub fn handle_file_commands(config: &Configuration, command: &FileCommands, format: &str) {
    match command {
        FileCommands::Create {
            workflow_id,
            name,
            path,
        } => {
            let user_name = get_env_user_name();
            let wf_id = workflow_id.unwrap_or_else(|| {
                select_workflow_interactively(config, &user_name).unwrap_or_else(|e| {
                    eprintln!("Error selecting workflow: {}", e);
                    std::process::exit(1);
                })
            });

            let file = models::FileModel::new(wf_id, name.clone(), path.clone());

            match apis::files_api::create_file(config, file) {
                Ok(created_file) => {
                    if print_if_json(format, &created_file, "file") {
                        // JSON was printed
                    } else {
                        println!("Successfully created file:");
                        println!("  ID: {}", created_file.id.unwrap_or(-1));
                        println!("  Name: {}", created_file.name);
                        println!("  Path: {}", created_file.path);
                        println!("  Workflow ID: {}", created_file.workflow_id);
                    }
                }
                Err(e) => {
                    print_error("creating file", &e);
                    std::process::exit(1);
                }
            }
        }
        FileCommands::List {
            workflow_id,
            produced_by_job_id,
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

            let mut params = FileListParams::new()
                .with_offset(*offset)
                .with_sort_by(sort_by.clone().unwrap_or_default())
                .with_reverse_sort(*reverse_sort);

            if let Some(limit_val) = limit {
                params = params.with_limit(*limit_val);
            }

            if let Some(job_id) = produced_by_job_id {
                params = params.with_produced_by_job_id(*job_id);
            }

            match pagination::paginate_files(config, selected_workflow_id as i64, params) {
                Ok(files) => {
                    if format == "json" {
                        print_json_wrapped("files", &files, "files");
                    } else if files.is_empty() {
                        println!("No files found for workflow ID: {}", selected_workflow_id);
                    } else {
                        println!("Files for workflow ID {}:", selected_workflow_id);
                        let rows: Vec<FileTableRow> = files
                            .iter()
                            .map(|file| FileTableRow {
                                id: file.id.unwrap_or(-1),
                                name: file.name.clone(),
                                path: file.path.clone(),
                                st_mtime: format_mtime(file.st_mtime),
                            })
                            .collect();
                        display_table_with_count(&rows, "files");
                    }
                }
                Err(e) => {
                    print_error("listing files", &e);
                    std::process::exit(1);
                }
            }
        }
        FileCommands::Get { id } => match apis::files_api::get_file(config, *id) {
            Ok(file) => {
                if print_if_json(format, &file, "file") {
                    // JSON was printed
                } else {
                    println!("File ID {}:", id);
                    println!("  Name: {}", file.name);
                    println!("  Path: {}", file.path);
                    println!("  Workflow ID: {}", file.workflow_id);
                }
            }
            Err(e) => {
                print_error("getting file", &e);
                std::process::exit(1);
            }
        },
        FileCommands::Update { id, name, path } => {
            // First get the existing file
            match apis::files_api::get_file(config, *id) {
                Ok(mut file) => {
                    // Update fields that were provided
                    if let Some(new_name) = name {
                        file.name = new_name.clone();
                    }
                    if let Some(new_path) = path {
                        file.path = new_path.clone();
                    }

                    match apis::files_api::update_file(config, *id, file) {
                        Ok(updated_file) => {
                            if print_if_json(format, &updated_file, "file") {
                                // JSON was printed
                            } else {
                                println!("Successfully updated file:");
                                println!("  ID: {}", updated_file.id.unwrap_or(-1));
                                println!("  Name: {}", updated_file.name);
                                println!("  Path: {}", updated_file.path);
                                println!("  Workflow ID: {}", updated_file.workflow_id);
                            }
                        }
                        Err(e) => {
                            print_error("updating file", &e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    print_error("getting file for update", &e);
                    std::process::exit(1);
                }
            }
        }
        FileCommands::Delete { id } => match apis::files_api::delete_file(config, *id) {
            Ok(removed_file) => {
                if print_if_json(format, &removed_file, "file") {
                    // JSON was printed
                } else {
                    println!("Successfully removed file:");
                    println!("  ID: {}", removed_file.id.unwrap_or(-1));
                    println!("  Name: {}", removed_file.name);
                    println!("  Path: {}", removed_file.path);
                }
            }
            Err(e) => {
                print_error("removing file", &e);
                std::process::exit(1);
            }
        },
        FileCommands::ListRequiredExisting { workflow_id } => {
            let user_name = get_env_user_name();
            let selected_workflow_id = match workflow_id {
                Some(id) => *id,
                None => select_workflow_interactively(config, &user_name).unwrap(),
            };

            match apis::workflows_api::list_required_existing_files(config, selected_workflow_id) {
                Ok(response) => {
                    if print_if_json(format, &response, "required existing files") {
                        // JSON was printed
                    } else if response.files.is_empty() {
                        println!(
                            "No missing required files found for workflow ID: {}",
                            selected_workflow_id
                        );
                    } else {
                        println!(
                            "Missing required files for workflow ID {}:",
                            selected_workflow_id
                        );
                        println!("These files are needed by jobs but do not exist:");
                        println!("{}", "-".repeat(50));
                        for file_id in response.files.iter() {
                            println!("File ID: {}", file_id);
                        }
                        println!("\nTotal missing files: {}", response.files.len());
                        println!("\nNote: This includes:");
                        println!(
                            "- Files needed by jobs but not produced by any job (user-provided)"
                        );
                        println!(
                            "- Files that should have been produced by completed jobs but are missing"
                        );
                    }
                }
                Err(e) => {
                    print_error("listing required existing files", &e);
                    std::process::exit(1);
                }
            }
        }
    }
}
