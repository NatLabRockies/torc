use crate::client::apis;
use crate::client::apis::configuration::Configuration;
use crate::client::commands::get_env_user_name;
use crate::client::commands::output::{print_if_json, print_json, print_wrapped_if_json};
use crate::client::commands::{
    print_error, select_workflow_interactively, table_format::display_table_with_count,
};
use crate::models;
use tabled::Tabled;

#[derive(Tabled)]
struct ScheduledComputeNodeTableRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Scheduler ID")]
    scheduler_id: i64,
    #[tabled(rename = "Config ID")]
    scheduler_config_id: i64,
    #[tabled(rename = "Type")]
    scheduler_type: String,
    #[tabled(rename = "Status")]
    status: String,
}

impl From<&models::ScheduledComputeNodesModel> for ScheduledComputeNodeTableRow {
    fn from(node: &models::ScheduledComputeNodesModel) -> Self {
        ScheduledComputeNodeTableRow {
            id: node.id.unwrap_or(-1),
            scheduler_id: node.scheduler_id,
            scheduler_config_id: node.scheduler_config_id,
            scheduler_type: node.scheduler_type.clone(),
            status: node.status.clone(),
        }
    }
}

#[derive(clap::Subcommand)]
pub enum ScheduledComputeNodeCommands {
    /// Get a scheduled compute node by ID
    Get {
        /// ID of the scheduled compute node
        #[arg()]
        id: i64,
    },
    /// List scheduled compute nodes for a workflow
    List {
        /// List scheduled compute nodes for this workflow (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Maximum number of scheduled compute nodes to return (default: all)
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
        /// Filter by scheduler ID
        #[arg(long)]
        scheduler_id: Option<String>,
        /// Filter by scheduler config ID
        #[arg(long)]
        scheduler_config_id: Option<String>,
        /// Filter by status
        #[arg(long)]
        status: Option<String>,
    },
    /// List jobs that ran under a scheduled compute node
    ListJobs {
        /// ID of the scheduled compute node
        #[arg()]
        id: i64,
    },
}

pub fn handle_scheduled_compute_node_commands(
    config: &Configuration,
    command: &ScheduledComputeNodeCommands,
    format: &str,
) {
    match command {
        ScheduledComputeNodeCommands::Get { id } => {
            match apis::scheduled_compute_nodes_api::get_scheduled_compute_node(config, *id) {
                Ok(node) => {
                    if print_if_json(format, &node, "scheduled compute node") {
                        // JSON was printed
                    } else {
                        println!("Scheduled Compute Node ID {}:", id);
                        println!("  Workflow ID: {}", node.workflow_id);
                        println!("  Scheduler ID: {}", node.scheduler_id);
                        println!("  Scheduler Config ID: {}", node.scheduler_config_id);
                        println!("  Scheduler Type: {}", node.scheduler_type);
                        println!("  Status: {}", node.status);
                    }
                }
                Err(e) => {
                    print_error("getting scheduled compute node", &e);
                    std::process::exit(1);
                }
            }
        }
        ScheduledComputeNodeCommands::List {
            workflow_id,
            limit,
            offset,
            sort_by,
            reverse_sort,
            scheduler_id,
            scheduler_config_id,
            status,
        } => {
            let user_name = get_env_user_name();
            let selected_workflow_id = match workflow_id {
                Some(id) => *id,
                None => match select_workflow_interactively(config, &user_name) {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Error selecting workflow: {}", e);
                        std::process::exit(1);
                    }
                },
            };

            match apis::scheduled_compute_nodes_api::list_scheduled_compute_nodes(
                config,
                selected_workflow_id,
                Some(*offset),
                *limit,
                sort_by.as_deref(),
                Some(*reverse_sort),
                scheduler_id.as_deref(),
                scheduler_config_id.as_deref(),
                status.as_deref(),
            ) {
                Ok(response) => {
                    let nodes = response.items;

                    if print_wrapped_if_json(
                        format,
                        "scheduled_compute_nodes",
                        &nodes,
                        "scheduled compute nodes",
                    ) {
                        // JSON was printed
                    } else if nodes.is_empty() {
                        println!(
                            "No scheduled compute nodes found for workflow {}",
                            selected_workflow_id
                        );
                    } else {
                        let rows: Vec<ScheduledComputeNodeTableRow> =
                            nodes.iter().map(|n| n.into()).collect();
                        display_table_with_count(&rows, "scheduled compute nodes");
                        if response.total_count as usize > nodes.len() {
                            println!(
                                "\nShowing {} of {} total scheduled compute nodes",
                                nodes.len(),
                                response.total_count
                            );
                        }
                    }
                }
                Err(e) => {
                    print_error("listing scheduled compute nodes", &e);
                    std::process::exit(1);
                }
            }
        }
        ScheduledComputeNodeCommands::ListJobs { id } => {
            // Step 1: Get the scheduled compute node to find the workflow_id
            let scheduled_node =
                match apis::scheduled_compute_nodes_api::get_scheduled_compute_node(config, *id) {
                    Ok(node) => node,
                    Err(e) => {
                        print_error("getting scheduled compute node", &e);
                        std::process::exit(1);
                    }
                };

            let workflow_id = scheduled_node.workflow_id;

            // Step 2: Get all compute nodes created by this scheduled compute node
            let compute_nodes = match apis::compute_nodes_api::list_compute_nodes(
                config,
                workflow_id,
                None,      // offset
                None,      // limit
                None,      // sort_by
                None,      // reverse_sort
                None,      // hostname
                None,      // is_active
                Some(*id), // scheduled_compute_node_id
            ) {
                Ok(response) => response.items,
                Err(e) => {
                    print_error("listing compute nodes", &e);
                    std::process::exit(1);
                }
            };

            if compute_nodes.is_empty() {
                if format == "json" {
                    println!("{{\"job_ids\": []}}");
                } else {
                    println!("No compute nodes found for scheduled compute node {}", id);
                }
                return;
            }

            // Step 3: For each compute node, get results and collect job IDs
            let mut job_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();

            for compute_node in &compute_nodes {
                if let Some(compute_node_id) = compute_node.id {
                    match apis::results_api::list_results(
                        config,
                        workflow_id,
                        None,                  // job_id
                        None,                  // run_id
                        None,                  // return_code
                        None,                  // status
                        Some(compute_node_id), // compute_node_id filter
                        None,                  // offset
                        None,                  // limit
                        None,                  // sort_by
                        None,                  // reverse_sort
                        Some(true),            // all_runs - include all historical results
                    ) {
                        Ok(response) => {
                            for result in response.items {
                                job_ids.insert(result.job_id);
                            }
                        }
                        Err(e) => {
                            eprintln!(
                                "Warning: Could not fetch results for compute node {}: {}",
                                compute_node_id, e
                            );
                        }
                    }
                }
            }

            let mut job_ids_vec: Vec<i64> = job_ids.into_iter().collect();
            job_ids_vec.sort();

            if format == "json" {
                let json_output = serde_json::json!({
                    "scheduled_compute_node_id": id,
                    "workflow_id": workflow_id,
                    "compute_node_count": compute_nodes.len(),
                    "job_ids": job_ids_vec,
                });
                print_json(&json_output, "scheduled compute node jobs");
            } else {
                println!(
                    "Jobs that ran under scheduled compute node {} (workflow {}):",
                    id, workflow_id
                );
                println!("  Compute nodes: {}", compute_nodes.len());
                println!("  Total jobs: {}", job_ids_vec.len());
                if !job_ids_vec.is_empty() {
                    println!("  Job IDs: {:?}", job_ids_vec);
                }
            }
        }
    }
}
