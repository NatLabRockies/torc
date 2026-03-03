use crate::client::apis::configuration::Configuration;
use crate::client::apis::default_api;
use crate::client::commands::get_env_user_name;
use crate::client::commands::output::{print_if_json, print_wrapped_if_json};
use crate::client::commands::{
    pagination, print_error, select_workflow_interactively, table_format::display_table_with_count,
};
use crate::models;
use tabled::Tabled;

#[derive(Tabled)]
struct ResourceRequirementsTableRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "CPUs")]
    cpus: i64,
    #[tabled(rename = "GPUs")]
    gpus: i64,
    #[tabled(rename = "Nodes")]
    nodes: i64,
    #[tabled(rename = "Memory")]
    memory: String,
    #[tabled(rename = "Runtime")]
    runtime: String,
}

#[derive(clap::Subcommand)]
pub enum ResourceRequirementsCommands {
    /// Create new resource requirements
    Create {
        /// Create resource requirements in this workflow.
        #[arg()]
        workflow_id: Option<i64>,
        /// Name of the resource requirements
        #[arg(short, long, required = true)]
        name: String,
        /// Number of CPUs required
        #[arg(long, default_value = "1")]
        num_cpus: i64,
        /// Number of GPUs required
        #[arg(long, default_value = "0")]
        num_gpus: i64,
        /// Number of nodes required
        #[arg(long, default_value = "1")]
        num_nodes: i64,
        /// Amount of memory required (e.g., "20g")
        #[arg(short, long, default_value = "1m")]
        memory: String,
        /// Maximum runtime in ISO 8601 duration format (e.g., "PT1H", "PT30M")
        #[arg(short, long, default_value = "PT1M")]
        runtime: String,
    },
    /// List resource requirements
    List {
        /// List resource requirements for this workflow (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Maximum number of resource requirements to return (default: all)
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
    /// Get a specific resource requirement by ID
    Get {
        /// ID of the resource requirement to get
        #[arg()]
        id: i64,
    },
    /// Update existing resource requirements
    Update {
        /// ID of the resource requirement to update
        #[arg()]
        id: i64,
        /// Name of the resource requirements
        #[arg(short, long)]
        name: Option<String>,
        /// Number of CPUs required
        #[arg(long)]
        num_cpus: Option<i64>,
        /// Number of GPUs required
        #[arg(long)]
        num_gpus: Option<i64>,
        /// Number of nodes required
        #[arg(long)]
        num_nodes: Option<i64>,
        /// Amount of memory required (e.g., "20g")
        #[arg(long)]
        memory: Option<String>,
        /// Maximum runtime in ISO 8601 duration format (e.g., "PT1H", "PT30M")
        #[arg(long)]
        runtime: Option<String>,
    },
    /// Delete resource requirements
    Delete {
        /// ID of the resource requirement to remove
        #[arg()]
        id: i64,
    },
}

pub fn handle_resource_requirements_commands(
    config: &Configuration,
    command: &ResourceRequirementsCommands,
    format: &str,
) {
    match command {
        ResourceRequirementsCommands::Create {
            workflow_id,
            name,
            num_cpus,
            num_gpus,
            num_nodes,
            memory,
            runtime,
        } => {
            let user_name = crate::client::commands::get_env_user_name();
            let wf_id = workflow_id.unwrap_or_else(|| {
                select_workflow_interactively(config, &user_name).unwrap_or_else(|e| {
                    eprintln!("Error selecting workflow: {}", e);
                    std::process::exit(1);
                })
            });

            let mut req = models::ResourceRequirementsModel::new(wf_id, name.to_string());
            req.num_cpus = *num_cpus;
            req.num_gpus = *num_gpus;
            req.num_nodes = *num_nodes;
            req.memory = memory.clone();
            req.runtime = runtime.clone();

            match default_api::create_resource_requirements(config, req) {
                Ok(created_req) => {
                    if print_if_json(format, &created_req, "resource requirements") {
                        // JSON was printed
                    } else {
                        println!("Successfully created resource requirements:");
                        println!("  ID: {}", created_req.id.unwrap_or(-1));
                        println!("  Workflow ID: {}", created_req.workflow_id);
                        println!("  Name: {}", created_req.name);
                        println!("  Number of CPUs: {}", created_req.num_cpus);
                        println!("  Number of GPUs: {}", created_req.num_gpus);
                        println!("  Number of nodes: {}", created_req.num_nodes);
                        println!("  Memory: {}", created_req.memory);
                        println!("  Runtime: {}", created_req.runtime);
                    }
                }
                Err(e) => {
                    print_error("creating resource requirements", &e);
                    std::process::exit(1);
                }
            }
        }
        ResourceRequirementsCommands::List {
            workflow_id,
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

            // Use pagination utility to get all resource requirements
            let mut params = pagination::ResourceRequirementsListParams::new()
                .with_offset(*offset)
                .with_sort_by(sort_by.clone().unwrap_or_default())
                .with_reverse_sort(*reverse_sort);

            if let Some(limit_val) = limit {
                params = params.with_limit(*limit_val);
            }

            match pagination::paginate_resource_requirements(
                config,
                selected_workflow_id as i64,
                params,
            ) {
                Ok(requirements) => {
                    if print_wrapped_if_json(
                        format,
                        "resource_requirements",
                        &requirements,
                        "resource requirements",
                    ) {
                        // JSON was printed
                    } else if requirements.is_empty() {
                        println!(
                            "No resource requirements found for workflow ID: {}",
                            selected_workflow_id
                        );
                    } else {
                        println!(
                            "Resource requirements for workflow ID {}:",
                            selected_workflow_id
                        );
                        let rows: Vec<ResourceRequirementsTableRow> = requirements
                            .iter()
                            .map(|req| ResourceRequirementsTableRow {
                                id: req.id.unwrap_or(-1),
                                name: req.name.clone(),
                                cpus: req.num_cpus,
                                gpus: req.num_gpus,
                                nodes: req.num_nodes,
                                memory: req.memory.clone(),
                                runtime: req.runtime.clone(),
                            })
                            .collect();
                        display_table_with_count(&rows, "resource requirements");
                    }
                }
                Err(e) => {
                    print_error("listing resource requirements", &e);
                    std::process::exit(1);
                }
            }
        }
        ResourceRequirementsCommands::Get { id } => {
            match default_api::get_resource_requirements(config, *id) {
                Ok(req) => {
                    if print_if_json(format, &req, "resource requirements") {
                        // JSON was printed
                    } else {
                        println!("Resource requirements ID {}:", id);
                        println!("  Workflow ID: {}", req.workflow_id);
                        println!("  Name: {}", req.name);
                        println!("  Number of CPUs: {}", req.num_cpus);
                        println!("  Number of GPUs: {}", req.num_gpus);
                        println!("  Number of nodes: {}", req.num_nodes);
                        println!("  Memory: {}", req.memory);
                        println!("  Runtime: {}", req.runtime);
                    }
                }
                Err(e) => {
                    print_error("getting resource requirements", &e);
                    std::process::exit(1);
                }
            }
        }
        ResourceRequirementsCommands::Update {
            id,
            name,
            num_cpus,
            num_gpus,
            num_nodes,
            memory,
            runtime,
        } => {
            // First get the existing resource requirements
            match default_api::get_resource_requirements(config, *id) {
                Ok(mut req) => {
                    // Update fields that were provided
                    if name.is_some() {
                        req.name = name.clone().unwrap();
                    }
                    if num_cpus.is_some() {
                        req.num_cpus = num_cpus.unwrap();
                    }
                    if num_gpus.is_some() {
                        req.num_gpus = num_gpus.unwrap();
                    }
                    if num_nodes.is_some() {
                        req.num_nodes = num_nodes.unwrap();
                    }
                    if memory.is_some() {
                        req.memory = memory.clone().unwrap();
                    }
                    if runtime.is_some() {
                        req.runtime = runtime.clone().unwrap();
                    }

                    match default_api::update_resource_requirements(config, *id, req) {
                        Ok(updated_req) => {
                            if print_if_json(format, &updated_req, "resource requirements") {
                                // JSON was printed
                            } else {
                                println!("Successfully updated resource requirements:");
                                println!("  ID: {}", updated_req.id.unwrap_or(-1));
                                println!("  Workflow ID: {}", updated_req.workflow_id);
                                println!("  Name: {}", updated_req.name);
                                println!("  Number of CPUs: {}", updated_req.num_cpus);
                                println!("  Number of GPUs: {}", updated_req.num_gpus);
                                println!("  Number of nodes: {}", updated_req.num_nodes);
                                println!("  Memory: {}", updated_req.memory);
                                println!("  Runtime: {}", updated_req.runtime);
                            }
                        }
                        Err(e) => {
                            print_error("updating resource requirements", &e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    print_error("getting resource requirements for update", &e);
                    std::process::exit(1);
                }
            }
        }
        ResourceRequirementsCommands::Delete { id } => {
            match default_api::delete_resource_requirements(config, *id, None) {
                Ok(removed_req) => {
                    if print_if_json(format, &removed_req, "resource requirements") {
                        // JSON was printed
                    } else {
                        println!("Successfully removed resource requirements:");
                        println!("  ID: {}", removed_req.id.unwrap_or(-1));
                        println!("  Workflow ID: {}", removed_req.workflow_id);
                        println!("  Name: {}", removed_req.name);
                    }
                }
                Err(e) => {
                    print_error("removing resource requirements", &e);
                    std::process::exit(1);
                }
            }
        }
    }
}
