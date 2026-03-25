use crate::client::apis;
use crate::client::apis::configuration::Configuration;
use crate::client::commands::get_env_user_name;
use crate::client::commands::output::{print_if_json, print_wrapped_if_json};
use crate::client::commands::{
    pagination, print_error, select_workflow_interactively, table_format::display_table_with_count,
};
use crate::models;
use tabled::Tabled;

/// Format memory bytes into a human-readable string
fn format_memory(bytes: Option<i64>) -> String {
    match bytes {
        Some(b) if b < 0 => "-".to_string(),
        Some(b) => {
            let mb = b as f64 / (1024.0 * 1024.0);
            if mb < 1024.0 {
                format!("{:.1}MB", mb)
            } else {
                format!("{:.2}GB", mb / 1024.0)
            }
        }
        None => "-".to_string(),
    }
}

/// Format CPU percentage
fn format_cpu(percent: Option<f64>) -> String {
    match percent {
        Some(p) if p < 0.0 => "-".to_string(),
        Some(p) => format!("{:.1}%", p),
        None => "-".to_string(),
    }
}

/// Helper function to create a map of job IDs to job names for a workflow
fn get_job_name_map(
    config: &Configuration,
    workflow_id: i64,
) -> std::collections::HashMap<i64, String> {
    let mut job_names = std::collections::HashMap::new();

    match pagination::paginate_jobs(config, workflow_id, pagination::JobListParams::new()) {
        Ok(jobs) => {
            for job in jobs {
                if let Some(id) = job.id {
                    job_names.insert(id, job.name);
                }
            }
        }
        Err(_) => {
            // If we can't fetch jobs, just continue without names
        }
    }

    job_names
}

#[derive(Tabled)]
struct ResultTableRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Job ID")]
    job_id: i64,
    #[tabled(rename = "Job Name")]
    job_name: String,
    #[tabled(rename = "WF ID")]
    workflow_id: i64,
    #[tabled(rename = "Run ID")]
    run_id: i64,
    #[tabled(rename = "Attempt")]
    attempt_id: i64,
    #[tabled(rename = "Return Code")]
    return_code: i64,
    #[tabled(rename = "Exec Time")]
    exec_time: String,
    #[tabled(rename = "Peak Mem")]
    peak_memory: String,
    #[tabled(rename = "Avg CPU %")]
    avg_cpu: String,
    #[tabled(rename = "Completion Time")]
    completion_time: String,
    #[tabled(rename = "Status")]
    status: String,
}

#[derive(clap::Subcommand)]
pub enum ResultCommands {
    /// List results
    List {
        /// List results for this workflow (optional - will prompt if not provided). By default,
        /// only lists results for the latest run of the workflow.
        #[arg()]
        workflow_id: Option<i64>,
        /// List results for this job
        #[arg(short, long)]
        job_id: Option<i64>,
        /// List results for this run_id
        #[arg(short, long)]
        run_id: Option<i64>,
        /// Filter by return code
        #[arg(long)]
        return_code: Option<i64>,
        /// Show only failed jobs (non-zero return code)
        #[arg(long)]
        failed: bool,
        /// Filter by job status (uninitialized, blocked, canceled, terminated, done, ready, scheduled, running, pending, disabled)
        #[arg(short, long)]
        status: Option<String>,
        /// Maximum number of results to return (default: all)
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
        /// Show all historical results (default: false, only shows current results)
        #[arg(long)]
        all_runs: bool,
        /// Filter by compute node ID
        #[arg(long)]
        compute_node: Option<i64>,
    },
    /// Get a specific result by ID
    Get {
        /// ID of the result to get
        #[arg()]
        id: i64,
    },
    /// Delete a result
    Delete {
        /// ID of the result to remove
        #[arg()]
        id: i64,
    },
}

pub fn handle_result_commands(config: &Configuration, command: &ResultCommands, format: &str) {
    match command {
        ResultCommands::List {
            workflow_id,
            job_id,
            run_id,
            return_code,
            failed,
            status,
            limit,
            offset,
            sort_by,
            reverse_sort,
            all_runs,
            compute_node,
        } => {
            let user_name = get_env_user_name();
            let selected_workflow_id = match workflow_id {
                Some(id) => *id,
                None => select_workflow_interactively(config, &user_name).unwrap(),
            };

            // Use pagination utility to get all results
            let mut params = pagination::ResultListParams::new().with_offset(*offset);

            if let Some(limit_val) = limit {
                params = params.with_limit(*limit_val);
            }

            if let Some(job_id_val) = job_id {
                params = params.with_job_id(*job_id_val);
            }

            if let Some(run_id_val) = run_id {
                params = params.with_run_id(*run_id_val);
            }

            if let Some(return_code_val) = return_code {
                params = params.with_return_code(*return_code_val);
            }

            if let Some(status_str) = status {
                // Parse string to JobStatus
                let status_val = match status_str.as_str() {
                    "uninitialized" => models::JobStatus::Uninitialized,
                    "blocked" => models::JobStatus::Blocked,
                    "ready" => models::JobStatus::Ready,
                    "pending" => models::JobStatus::Pending,
                    "running" => models::JobStatus::Running,
                    "completed" => models::JobStatus::Completed,
                    "failed" => models::JobStatus::Failed,
                    "canceled" => models::JobStatus::Canceled,
                    "terminated" => models::JobStatus::Terminated,
                    "disabled" => models::JobStatus::Disabled,
                    _ => {
                        eprintln!(
                            "Invalid status: {}. Valid values are: uninitialized, blocked, ready, pending, running, completed, failed, canceled, terminated, disabled",
                            status_str
                        );
                        std::process::exit(1);
                    }
                };
                params = params.with_status(status_val);
            }

            if let Some(sort_by_str) = sort_by {
                params = params.with_sort_by(sort_by_str.clone());
            }

            params = params.with_reverse_sort(*reverse_sort);
            params = params.with_all_runs(*all_runs);

            if let Some(compute_node_id) = compute_node {
                params = params.with_compute_node_id(*compute_node_id);
            }

            match pagination::paginate_results(config, selected_workflow_id as i64, params) {
                Ok(mut results) => {
                    // Apply client-side filtering for failed jobs
                    if *failed {
                        results.retain(|r| r.return_code != 0);
                    }

                    if print_wrapped_if_json(format, "results", &results, "results") {
                        // JSON was printed
                    } else if results.is_empty() {
                        if let Some(jid) = job_id {
                            println!(
                                "No results found for workflow ID {} and job ID: {}",
                                selected_workflow_id, jid
                            );
                        } else {
                            println!("No results found for workflow ID: {}", selected_workflow_id);
                        }
                    } else {
                        if let Some(jid) = job_id {
                            println!(
                                "Results for workflow ID {} and job ID {}:",
                                selected_workflow_id, jid
                            );
                        } else {
                            println!("Results for workflow ID {}:", selected_workflow_id);
                        }

                        // Fetch job names for the workflow
                        let job_names = get_job_name_map(config, selected_workflow_id);

                        let rows: Vec<ResultTableRow> = results
                            .iter()
                            .map(|result| ResultTableRow {
                                id: result.id.unwrap_or(-1),
                                job_id: result.job_id,
                                job_name: job_names
                                    .get(&result.job_id)
                                    .cloned()
                                    .unwrap_or_else(|| "-".to_string()),
                                workflow_id: result.workflow_id,
                                run_id: result.run_id,
                                attempt_id: result.attempt_id.unwrap_or(1),
                                return_code: result.return_code,
                                exec_time: format!("{:.2}", result.exec_time_minutes),
                                peak_memory: format_memory(result.peak_memory_bytes),
                                avg_cpu: format_cpu(result.avg_cpu_percent),
                                completion_time: result.completion_time.clone(),
                                status: format!("{:?}", result.status),
                            })
                            .collect();
                        display_table_with_count(&rows, "results");
                    }
                }
                Err(e) => {
                    print_error("listing results", &e);
                    std::process::exit(1);
                }
            }
        }
        ResultCommands::Get { id } => match apis::results_api::get_result(config, *id) {
            Ok(result) => {
                if print_if_json(format, &result, "result") {
                    // JSON was printed
                } else {
                    println!("Result ID {}:", id);
                    println!("  Job ID: {}", result.job_id);
                    println!("  Workflow ID: {}", result.workflow_id);
                    println!("  Run ID: {}", result.run_id);
                    println!("  Attempt ID: {}", result.attempt_id.unwrap_or(1));
                    println!("  Return Code: {}", result.return_code);
                    println!(
                        "  Execution Time (minutes): {:.2}",
                        result.exec_time_minutes
                    );
                    println!("  Completion Time: {}", result.completion_time);
                    println!("  Status: {:?}", result.status);

                    // Display resource metrics if available
                    if result.peak_memory_bytes.is_some()
                        || result.avg_memory_bytes.is_some()
                        || result.avg_cpu_percent.is_some()
                    {
                        println!("\n  Resource Metrics:");
                        if let Some(peak_mem) = result.peak_memory_bytes {
                            println!("    Peak Memory: {}", format_memory(Some(peak_mem)));
                        }
                        if let Some(avg_mem) = result.avg_memory_bytes {
                            println!("    Avg Memory:  {}", format_memory(Some(avg_mem)));
                        }
                        if let Some(avg_cpu) = result.avg_cpu_percent {
                            println!("    Avg CPU:     {}", format_cpu(Some(avg_cpu)));
                        }
                    }
                }
            }
            Err(e) => {
                print_error("getting result", &e);
                std::process::exit(1);
            }
        },
        ResultCommands::Delete { id } => {
            match apis::results_api::delete_result(config, *id) {
                Ok(removed_result) => {
                    if print_if_json(format, &removed_result, "result") {
                        // JSON was printed
                    } else {
                        println!("Successfully removed result:");
                        println!("  ID: {}", removed_result.id.unwrap_or(-1));
                        println!("  Job ID: {}", removed_result.job_id);
                        println!("  Workflow ID: {}", removed_result.workflow_id);
                    }
                }
                Err(e) => {
                    print_error("removing result", &e);
                    std::process::exit(1);
                }
            }
        }
    }
}
