use crate::client::apis;
use crate::client::apis::configuration::Configuration;
use crate::client::commands::get_env_user_name;
use crate::client::commands::output::{print_if_json, print_wrapped_if_json};
use crate::client::commands::pagination::{ComputeNodeListParams, paginate_compute_nodes};
use crate::client::commands::{
    print_error, select_workflow_interactively, table_format::display_table_with_count,
};
use crate::models;
use tabled::Tabled;

#[derive(Tabled)]
struct ComputeNodeTableRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Hostname")]
    hostname: String,
    #[tabled(rename = "PID")]
    pid: i64,
    #[tabled(rename = "CPUs")]
    num_cpus: i64,
    #[tabled(rename = "Memory (GB)")]
    memory_gb: String,
    #[tabled(rename = "GPUs")]
    num_gpus: i64,
    #[tabled(rename = "Active")]
    is_active: String,
    #[tabled(rename = "Start Time")]
    start_time: String,
    #[tabled(rename = "Duration")]
    duration: String,
    #[tabled(rename = "CPU peak/avg")]
    system_cpu: String,
    #[tabled(rename = "Mem peak/avg")]
    system_memory: String,
}

impl From<&models::ComputeNodeModel> for ComputeNodeTableRow {
    fn from(node: &models::ComputeNodeModel) -> Self {
        let duration = match node.duration_seconds {
            Some(d) => format!("{:.1}s", d),
            None => "-".to_string(),
        };

        let is_active = match node.is_active {
            Some(true) => "Yes".to_string(),
            Some(false) => "No".to_string(),
            None => "-".to_string(),
        };

        ComputeNodeTableRow {
            id: node.id.unwrap_or(-1),
            hostname: node.hostname.clone(),
            pid: node.pid,
            num_cpus: node.num_cpus,
            memory_gb: format!("{:.2}", node.memory_gb),
            num_gpus: node.num_gpus,
            is_active,
            start_time: node.start_time.clone(),
            duration,
            system_cpu: format_system_cpu(node),
            system_memory: format_system_memory(node),
        }
    }
}

fn format_system_cpu(node: &models::ComputeNodeModel) -> String {
    match (node.peak_cpu_percent, node.avg_cpu_percent) {
        (Some(peak), Some(avg)) => format!("{:.1}%/{:.1}%", peak, avg),
        _ => "-".to_string(),
    }
}

fn format_system_memory(node: &models::ComputeNodeModel) -> String {
    match (node.peak_memory_bytes, node.avg_memory_bytes) {
        (Some(peak), Some(avg)) => format!("{} / {}", format_bytes(peak), format_bytes(avg)),
        _ => "-".to_string(),
    }
}

fn format_bytes(bytes: i64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let bytes_f = bytes as f64;
    if bytes_f >= GB {
        format!("{:.1} GB", bytes_f / GB)
    } else if bytes_f >= MB {
        format!("{:.1} MB", bytes_f / MB)
    } else if bytes_f >= KB {
        format!("{:.1} KB", bytes_f / KB)
    } else {
        format!("{} B", bytes)
    }
}

#[derive(clap::Subcommand)]
pub enum ComputeNodeCommands {
    /// Get a specific compute node by ID
    Get {
        /// ID of the compute node
        #[arg()]
        id: i64,
    },
    /// List compute nodes for a workflow
    List {
        /// List compute nodes for this workflow (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Maximum number of compute nodes to return (default: all)
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
        /// Filter by scheduled compute node ID
        #[arg(long)]
        scheduled_compute_node: Option<i64>,
    },
}

pub fn handle_compute_node_commands(
    config: &Configuration,
    command: &ComputeNodeCommands,
    format: &str,
) {
    match command {
        ComputeNodeCommands::Get { id } => {
            match apis::compute_nodes_api::get_compute_node(config, *id) {
                Ok(node) => {
                    if print_if_json(format, &node, "compute node") {
                        // JSON was printed
                    } else {
                        println!("Compute Node Details:");
                        println!("  ID: {}", node.id.unwrap_or(-1));
                        println!("  Workflow ID: {}", node.workflow_id);
                        println!("  Hostname: {}", node.hostname);
                        println!("  PID: {}", node.pid);
                        println!("  CPUs: {}", node.num_cpus);
                        println!("  Memory: {:.2} GB", node.memory_gb);
                        println!("  GPUs: {}", node.num_gpus);
                        println!(
                            "  Active: {}",
                            match node.is_active {
                                Some(true) => "Yes",
                                Some(false) => "No",
                                None => "Unknown",
                            }
                        );
                        println!("  Start Time: {}", node.start_time);
                        if let Some(duration) = node.duration_seconds {
                            println!("  Duration: {:.2} seconds", duration);
                        }
                        if node.sample_count.is_some() {
                            println!("  CPU peak/avg: {}", format_system_cpu(&node));
                            println!("  Memory peak/avg: {}", format_system_memory(&node));
                            if let Some(samples) = node.sample_count {
                                println!("  Resource samples: {}", samples);
                            }
                        }
                    }
                }
                Err(e) => {
                    print_error("getting compute node", &e);
                    std::process::exit(1);
                }
            }
        }
        ComputeNodeCommands::List {
            workflow_id,
            limit,
            offset,
            sort_by,
            reverse_sort,
            scheduled_compute_node,
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

            let mut params = ComputeNodeListParams::new().with_offset(*offset);

            if let Some(limit_val) = limit {
                params = params.with_limit(*limit_val);
            }

            if let Some(sort) = sort_by {
                params = params.with_sort_by(sort.clone());
            }
            if *reverse_sort {
                params = params.with_reverse_sort(true);
            }
            if let Some(scn_id) = scheduled_compute_node {
                params = params.with_scheduled_compute_node_id(*scn_id);
            }

            match paginate_compute_nodes(config, selected_workflow_id, params) {
                Ok(nodes) => {
                    if print_wrapped_if_json(format, "compute_nodes", &nodes, "compute_nodes") {
                        // JSON was printed
                    } else if nodes.is_empty() {
                        println!(
                            "No compute nodes found for workflow {}",
                            selected_workflow_id
                        );
                    } else {
                        let rows: Vec<ComputeNodeTableRow> =
                            nodes.iter().map(|n| n.into()).collect();
                        display_table_with_count(&rows, "compute nodes");
                    }
                }
                Err(e) => {
                    print_error("listing compute nodes", &e);
                    std::process::exit(1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_compute_node() -> models::ComputeNodeModel {
        models::ComputeNodeModel::new(
            7,
            "node-a".to_string(),
            1234,
            "2026-04-19T00:00:00Z".to_string(),
            12,
            32.0,
            0,
            1,
            "local".to_string(),
            None,
        )
    }

    #[test]
    fn compute_node_list_row_shows_resource_summary_when_present() {
        let mut node = make_compute_node();
        node.sample_count = Some(7);
        node.peak_cpu_percent = Some(41.1843);
        node.avg_cpu_percent = Some(37.2608);
        node.peak_memory_bytes = Some(2 * 1024 * 1024 * 1024);
        node.avg_memory_bytes = Some(1024 * 1024 * 1024);

        let row = ComputeNodeTableRow::from(&node);

        assert_eq!(row.system_cpu, "41.2%/37.3%");
        assert_eq!(row.system_memory, "2.0 GB / 1.0 GB");
    }

    #[test]
    fn compute_node_list_row_hides_resource_summary_when_absent() {
        let node = make_compute_node();

        let row = ComputeNodeTableRow::from(&node);

        assert_eq!(row.system_cpu, "-");
        assert_eq!(row.system_memory, "-");
    }

    #[test]
    fn compute_node_json_uses_plain_resource_summary_fields_when_present() {
        let mut node = make_compute_node();
        node.sample_count = Some(7);
        node.peak_cpu_percent = Some(41.1843);
        node.avg_cpu_percent = Some(37.2608);
        node.peak_memory_bytes = Some(2 * 1024 * 1024 * 1024);
        node.avg_memory_bytes = Some(1024 * 1024 * 1024);

        let output = serde_json::json!({ "compute_nodes": [node] });

        let json = output["compute_nodes"][0].as_object().unwrap();
        assert_eq!(json["sample_count"], 7);
        assert_eq!(json["peak_cpu_percent"], 41.1843);
        assert_eq!(json["avg_cpu_percent"], 37.2608);
        assert_eq!(json["peak_memory_bytes"], 2 * 1024 * 1024 * 1024i64);
        assert_eq!(json["avg_memory_bytes"], 1024 * 1024 * 1024i64);
        assert!(!json.contains_key("system_monitor_sample_count"));
        assert!(!json.contains_key("system_monitor_peak_cpu_percent"));
        assert!(!json.contains_key("system_monitor_avg_cpu_percent"));
        assert!(!json.contains_key("system_monitor_peak_memory_bytes"));
        assert!(!json.contains_key("system_monitor_avg_memory_bytes"));
    }

    #[test]
    fn compute_node_json_omits_resource_summary_fields_when_absent() {
        let output = serde_json::json!({ "compute_nodes": [make_compute_node()] });

        let json = output["compute_nodes"][0].as_object().unwrap();
        assert!(!json.contains_key("sample_count"));
        assert!(!json.contains_key("peak_cpu_percent"));
        assert!(!json.contains_key("avg_cpu_percent"));
        assert!(!json.contains_key("peak_memory_bytes"));
        assert!(!json.contains_key("avg_memory_bytes"));
    }
}
