use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use crate::client::apis;
use crate::client::apis::configuration::Configuration;
use crate::client::commands::get_env_user_name;
use crate::client::commands::{
    output::{print_if_json, print_json, print_json_wrapped},
    pagination::{self, JobListParams},
    print_error, select_workflow_interactively,
    table_format::{
        display_csv, display_csv_excluding, display_table_excluding, display_table_with_count,
    },
};
use crate::client::utils::format_local_timestamp;
use crate::client::workflow_manager::WorkflowManager;
use crate::config::TorcConfig;
use crate::models;
use tabled::Tabled;

#[derive(Tabled)]
struct JobTableRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Priority")]
    priority: i64,
    #[tabled(rename = "Compute Node")]
    compute_node: String,
    #[tabled(rename = "Elapsed")]
    elapsed: String,
    #[tabled(rename = "Command")]
    command: String,
}

/// Compute elapsed time from an RFC3339 start_time to now, formatted compactly.
/// Returns an empty string if start_time is missing or unparseable.
fn format_elapsed(start_time: Option<&str>) -> String {
    let Some(s) = start_time else {
        return String::new();
    };
    let Ok(start) = chrono::DateTime::parse_from_rfc3339(s) else {
        return String::new();
    };
    let elapsed = chrono::Utc::now().signed_duration_since(start.with_timezone(&chrono::Utc));
    let total_secs = elapsed.num_seconds().max(0);
    let days = total_secs / 86_400;
    let hours = (total_secs % 86_400) / 3_600;
    let mins = (total_secs % 3_600) / 60;
    let secs = total_secs % 60;
    if days > 0 {
        format!("{}d {:02}h", days, hours)
    } else if hours > 0 {
        format!("{}h {:02}m", hours, mins)
    } else if mins > 0 {
        format!("{}m {:02}s", mins, secs)
    } else {
        format!("{}s", secs)
    }
}

#[derive(Tabled)]
struct JobResourceRequirementsTableRow {
    #[tabled(rename = "Job ID")]
    job_id: i64,
    #[tabled(rename = "Job Name")]
    job_name: String,
    #[tabled(rename = "RR ID")]
    rr_id: i64,
    #[tabled(rename = "RR Name")]
    rr_name: String,
    #[tabled(rename = "CPUs")]
    num_cpus: i64,
    #[tabled(rename = "GPUs")]
    num_gpus: i64,
    #[tabled(rename = "Nodes")]
    num_nodes: i64,
    #[tabled(rename = "Memory")]
    memory: String,
    #[tabled(rename = "Runtime")]
    runtime: String,
}

#[derive(Tabled)]
struct JobFailureHandlerTableRow {
    #[tabled(rename = "Job ID")]
    job_id: i64,
    #[tabled(rename = "Job Name")]
    job_name: String,
    #[tabled(rename = "FH ID")]
    fh_id: i64,
    #[tabled(rename = "FH Name")]
    fh_name: String,
    #[tabled(rename = "Rules Summary")]
    rules_summary: String,
}

#[derive(Tabled)]
struct RunningJobTableRow {
    #[tabled(rename = "Job ID")]
    job_id: i64,
    #[tabled(rename = "Job Name")]
    job_name: String,
    #[tabled(rename = "Compute Node")]
    compute_node: String,
    #[tabled(rename = "Elapsed")]
    elapsed: String,
    #[tabled(rename = "Scheduler")]
    scheduler_type: String,
    #[tabled(rename = "Scheduler Job ID")]
    scheduler_job_id: String,
}

#[derive(clap::Subcommand)]
#[command(after_long_help = "\
EXAMPLES:
    # List jobs for a workflow
    torc jobs list 123

    # Filter by status
    torc jobs list 123 --status failed

    # Get JSON output for scripting
    torc -f json jobs list 123

    # Get job details
    torc jobs get 456
")]
pub enum JobCommands {
    /// Create a new job
    #[command(after_long_help = "\
EXAMPLES:
    # Create a simple job
    torc jobs create 123 --name my_job --command 'python script.py'

    # Create job with dependencies
    torc jobs create 123 --name process --command 'python process.py' \\
        --blocking-job-ids 1 2 3

    # Create job with file I/O
    torc jobs create 123 --name analyze --command 'python analyze.py' \\
        --input-file-ids 10 --output-file-ids 20
")]
    Create {
        /// Create the job in this workflow.
        #[arg()]
        workflow_id: Option<i64>,
        /// Name of the job
        #[arg(short, long, required = true)]
        name: String,
        /// Command to execute
        #[arg(short, long, required = true)]
        command: String,
        /// Resource requirements ID for this job
        #[arg(short, long)]
        resource_requirements_id: Option<i64>,
        /// Job IDs that block this job
        #[arg(short, long, num_args = 1..)]
        blocking_job_ids: Vec<i64>,
        /// Input files needed by this job.
        #[arg(short, long, num_args = 1..)]
        input_file_ids: Vec<i64>,
        /// Output files produced by this job.
        #[arg(short, long, num_args = 1..)]
        output_file_ids: Vec<i64>,
    },
    /// Create multiple jobs from a text file containing one command per line
    ///
    /// This command reads a text file where each line contains a job command.
    /// Lines starting with '#' are treated as comments and ignored.
    /// Empty lines are also ignored.
    ///
    /// Jobs will be named sequentially as job1, job2, job3, etc., starting
    /// from the current job count + 1 to avoid naming conflicts.
    ///
    /// All jobs created will share the same resource requirements, which
    /// are automatically created and assigned.
    #[command(
        name = "create-from-file",
        after_long_help = "\
EXAMPLES:
    # Create jobs from a file with default resources
    torc jobs create-from-file 123 batch_jobs.txt

    # Specify resources per job
    torc jobs create-from-file 123 batch_jobs.txt \\
        --cpus-per-job 4 --memory-per-job 8g --runtime-per-job PT2H

    # Example file format (batch_jobs.txt):
    # # Data processing jobs
    # python process.py --batch 1
    # python process.py --batch 2
    # python process.py --batch 3
"
    )]
    CreateFromFile {
        /// Workflow ID to create jobs for
        #[arg()]
        workflow_id: i64,
        /// Path to text file containing job commands (one per line)
        ///
        /// File format:
        /// - One command per line
        /// - Lines starting with # are comments (ignored)
        /// - Empty lines are ignored
        ///
        /// Example file content:
        ///   # Data processing jobs
        ///   python process.py --batch 1
        ///   python process.py --batch 2
        ///   python process.py --batch 3
        #[arg()]
        file: String,
        /// Number of CPUs per job
        #[arg(long, default_value = "1")]
        cpus_per_job: i64,
        /// Memory per job (e.g., "1m", "2g", "16g")
        #[arg(long, default_value = "1m")]
        memory_per_job: String,
        /// Runtime per job (ISO 8601 duration format)
        ///
        /// Examples:
        ///   PT1M      = 1 minute
        ///   PT30M     = 30 minutes
        ///   PT2H      = 2 hours
        ///   P1D       = 1 day
        #[arg(long, default_value = "PT1M")]
        runtime_per_job: String,
    },
    /// List jobs
    #[command(after_long_help = "\
EXAMPLES:
    # List all jobs for a workflow
    torc jobs list 123

    # Filter by status
    torc jobs list 123 --status ready
    torc jobs list 123 --status failed
    torc jobs list 123 --status running

    # Get JSON output for scripting
    torc -f json jobs list 123

    # Include dependency information
    torc jobs list 123 --include-relationships

    # Paginate results
    torc jobs list 123 --limit 100 --offset 0

    # Hide the command column
    torc jobs list 123 -x command

    # Hide multiple columns
    torc jobs list 123 -x command -x name
")]
    List {
        /// List jobs for this workflow (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// User to filter by (defaults to USER environment variable)
        #[arg(short, long)]
        status: Option<String>,
        /// Filter by upstream job ID (jobs that depend on this job)
        #[arg(long)]
        upstream_job_id: Option<i64>,
        /// Maximum number of jobs to return (default: all)
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
        /// Include job relationships (depends_on_job_ids, input/output file/user_data IDs) - slower but more complete
        #[arg(long)]
        include_relationships: bool,
        /// Exclude columns from table/csv output (case-insensitive, can be repeated)
        #[arg(short = 'x', long = "exclude")]
        exclude_columns: Vec<String>,
    },
    /// Get a specific job by ID
    #[command(after_long_help = "\
EXAMPLES:
    # Get job details
    torc jobs get 456

    # Get as JSON
    torc -f json jobs get 456
")]
    Get {
        /// ID of the job to get
        #[arg()]
        id: i64,
    },
    /// Update an existing job
    #[command(after_long_help = "\
EXAMPLES:
    # Update job name
    torc jobs update 456 --name 'new_name'

    # Update job command
    torc jobs update 456 --command 'python new_script.py'

    # Update job runtime (requires existing resource requirements)
    torc jobs update 456 --runtime PT2H

    # Change resource requirements
    torc jobs update 456 --resource-requirements-id 10

    # Set scheduling priority (higher = submitted first)
    torc jobs update 456 --priority 10
")]
    Update {
        /// ID of the job to update
        #[arg()]
        id: i64,
        /// Name of the job
        #[arg(short, long)]
        name: Option<String>,
        /// Command to execute
        #[arg(short, long)]
        command: Option<String>,
        /// Runtime for the job (ISO 8601 duration format, e.g., PT30M, PT2H)
        ///
        /// This updates the runtime on the job's associated resource requirements.
        /// The job must already have a resource_requirements_id assigned.
        #[arg(long)]
        runtime: Option<String>,
        /// Resource requirements ID to assign to this job
        #[arg(long)]
        resource_requirements_id: Option<i64>,
        /// Scheduling priority (0 or higher; higher = submitted first)
        #[arg(long)]
        priority: Option<i64>,
    },
    /// Delete one or more jobs
    #[command(after_long_help = "\
EXAMPLES:
    # Delete a single job
    torc jobs delete 456

    # Delete multiple jobs
    torc jobs delete 456 457 458
")]
    Delete {
        /// IDs of the jobs to remove
        #[arg(num_args = 1..)]
        ids: Vec<i64>,
    },
    /// Delete all jobs for a workflow
    #[command(
        name = "delete-all",
        after_long_help = "\
EXAMPLES:
    # Delete all jobs from a workflow
    torc jobs delete-all 123
"
    )]
    DeleteAll {
        /// Workflow ID to delete all jobs from (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
    },
    /// List jobs with their resource requirements
    #[command(
        name = "list-resource-requirements",
        after_long_help = "\
EXAMPLES:
    # List all jobs with their resource requirements
    torc jobs list-resource-requirements 123

    # Get JSON output
    torc -f json jobs list-resource-requirements 123

    # Filter by specific job
    torc jobs list-resource-requirements 123 --job-id 456
"
    )]
    ListResourceRequirements {
        /// Workflow ID to list jobs from (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Filter by specific job ID
        #[arg(short, long)]
        job_id: Option<i64>,
    },
    /// List jobs with their failure handlers
    #[command(
        name = "list-failure-handlers",
        after_long_help = "\
EXAMPLES:
    # List all jobs with their failure handlers
    torc jobs list-failure-handlers 123

    # Get JSON output
    torc -f json jobs list-failure-handlers 123

    # Filter by specific job
    torc jobs list-failure-handlers 123 --job-id 456
"
    )]
    ListFailureHandlers {
        /// Workflow ID to list jobs from (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Filter by specific job ID
        #[arg(short, long)]
        job_id: Option<i64>,
    },
    /// List currently-running jobs with their compute node and scheduler info
    #[command(
        name = "running",
        after_long_help = "\
EXAMPLES:
    # List running jobs with compute node names (and Slurm job IDs, if any)
    torc jobs running 123

    # JSON output
    torc -f json jobs running 123
"
    )]
    Running {
        /// Workflow ID to list running jobs for (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
    },
    /// Reset specific jobs to uninitialized status for selective rerun
    ///
    /// Resets only the given job IDs to uninitialized so they can be rerun. All jobs
    /// must belong to the same workflow. Downstream dependents are not reset by this
    /// command; they are reset transitively by 'torc workflows reinit' (the command
    /// lists them for you).
    ///
    /// After resetting, run 'torc workflows reinit <workflow_id>' to rebuild
    /// dependency state (this bumps run_id exactly once), then 'torc run' or
    /// 'torc submit' to execute. Use --reinit to perform the reinit step automatically
    /// in the same invocation.
    #[command(
        name = "reset-status",
        after_long_help = "\
EXAMPLES:
    # Reset two specific jobs for rerun (preview first)
    torc jobs reset-status 101 102 --dry-run

    # Reset and reinitialize in one step, then run
    torc jobs reset-status 101 102 --reinit
    torc run 42

    # Reset and rerun (manual two-step flow)
    torc jobs reset-status 101 102
    torc workflows reinit 42
    torc run 42

    # Reset without prompts (for scripts/CI)
    torc jobs reset-status 101 102 --no-prompts

    # Override quiescence check (workflow still running)
    torc jobs reset-status 101 --force

    # JSON output for scripting
    torc -f json jobs reset-status 101 102
"
    )]
    ResetStatus {
        /// Job IDs to reset (must all belong to the same workflow)
        #[arg(required = true, num_args = 1..)]
        job_ids: Vec<i64>,
        /// Skip precondition checks (workflow complete, no active workers)
        #[arg(long)]
        force: bool,
        /// Preview which jobs would be reset without applying changes
        #[arg(long)]
        dry_run: bool,
        /// Skip confirmation prompt
        #[arg(long)]
        no_prompts: bool,
        /// Reinitialize the workflow after resetting (runs 'torc workflows reinit' for you)
        #[arg(long, alias = "reinitialize")]
        reinit: bool,
    },
}

pub fn handle_job_commands(config: &Configuration, command: &JobCommands, format: &str) {
    match command {
        JobCommands::Create {
            name,
            command,
            workflow_id,
            resource_requirements_id,
            blocking_job_ids,
            input_file_ids,
            output_file_ids,
        } => {
            let user_name = crate::client::commands::get_env_user_name();
            let wf_id = workflow_id.unwrap_or_else(|| {
                select_workflow_interactively(config, &user_name).unwrap_or_else(|e| {
                    eprintln!("Error selecting workflow: {}", e);
                    std::process::exit(1);
                })
            });

            let mut job = models::JobModel::new(wf_id, name.clone(), command.clone());
            if let Some(rr_id) = resource_requirements_id {
                job.resource_requirements_id = Some(*rr_id);
            }
            if !blocking_job_ids.is_empty() {
                job.depends_on_job_ids = Some(blocking_job_ids.clone());
            }
            if !input_file_ids.is_empty() {
                job.input_file_ids = Some(input_file_ids.clone());
            }
            if !output_file_ids.is_empty() {
                job.output_file_ids = Some(output_file_ids.clone());
            }

            match apis::jobs_api::create_job(config, job) {
                Ok(created_job) => {
                    if print_if_json(format, &created_job, "job") {
                        // JSON was printed
                    } else {
                        println!("Successfully created job:");
                        println!("  ID: {}", created_job.id.unwrap_or(-1));
                        println!("  Name: {}", created_job.name);
                        println!("  Command: {}", created_job.command);
                        println!("  Workflow ID: {}", created_job.workflow_id);
                        println!(
                            "  Blocking job IDs: {}",
                            created_job
                                .depends_on_job_ids
                                .as_ref()
                                .map(|ids| format!("{:?}", ids))
                                .unwrap_or_else(|| "None".to_string())
                        );
                        println!(
                            "  Input file IDs: {}",
                            created_job
                                .input_file_ids
                                .as_ref()
                                .map(|ids| format!("{:?}", ids))
                                .unwrap_or_else(|| "None".to_string())
                        );
                        println!(
                            "  Output file IDs: {}",
                            created_job
                                .output_file_ids
                                .as_ref()
                                .map(|ids| format!("{:?}", ids))
                                .unwrap_or_else(|| "None".to_string())
                        );
                    }
                }
                Err(e) => {
                    print_error("creating job", &e);
                    std::process::exit(1);
                }
            }
        }
        JobCommands::List {
            workflow_id,
            status,
            upstream_job_id,
            limit,
            offset,
            sort_by,
            reverse_sort,
            include_relationships,
            exclude_columns,
        } => {
            let user_name = get_env_user_name();
            let selected_workflow_id = match workflow_id {
                Some(id) => *id,
                None => select_workflow_interactively(config, &user_name).unwrap(),
            };

            // Convert string status to JobStatus enum if provided
            let job_status = match status {
                Some(status_str) => match status_str.to_lowercase().as_str() {
                    "uninitialized" => Some(models::JobStatus::Uninitialized),
                    "blocked" => Some(models::JobStatus::Blocked),
                    "ready" => Some(models::JobStatus::Ready),
                    "pending" => Some(models::JobStatus::Pending),
                    "running" => Some(models::JobStatus::Running),
                    "completed" => Some(models::JobStatus::Completed),
                    "failed" => Some(models::JobStatus::Failed),
                    "canceled" => Some(models::JobStatus::Canceled),
                    "terminated" => Some(models::JobStatus::Terminated),
                    "disabled" => Some(models::JobStatus::Disabled),
                    _ => {
                        eprintln!(
                            "Invalid status: {}. Valid values are: uninitialized, blocked, ready, pending, running, completed, failed, canceled, terminated, disabled",
                            status_str
                        );
                        std::process::exit(1);
                    }
                },
                None => None,
            };

            let mut params = JobListParams::new()
                .with_offset(*offset)
                .with_sort_by(sort_by.clone().unwrap_or_default())
                .with_reverse_sort(*reverse_sort)
                .with_include_relationships(*include_relationships);

            if let Some(limit_val) = limit {
                params = params.with_limit(*limit_val);
            }

            if let Some(job_status) = job_status {
                params = params.with_status(job_status);
            }

            if let Some(upstream_id) = upstream_job_id {
                params = params.with_upstream_job_id(*upstream_id);
            }

            match pagination::paginate_jobs(config, selected_workflow_id, params) {
                Ok(jobs) => {
                    if format == "json" {
                        print_json_wrapped(&jobs, "jobs");
                    } else if jobs.is_empty() && format != "csv" {
                        println!("No jobs found for workflow ID: {}", selected_workflow_id);
                    } else {
                        let rows: Vec<JobTableRow> = jobs
                            .iter()
                            .map(|job| {
                                let status = job.status.expect("Job status is missing");
                                let elapsed = if matches!(status, models::JobStatus::Running) {
                                    format_elapsed(job.start_time.as_deref())
                                } else {
                                    String::new()
                                };
                                JobTableRow {
                                    id: job.id.unwrap_or(-1),
                                    name: job.name.clone(),
                                    status: status.to_string(),
                                    priority: job.priority.unwrap_or(0),
                                    compute_node: job
                                        .compute_node_id
                                        .map(|n| n.to_string())
                                        .unwrap_or_default(),
                                    elapsed,
                                    command: job.command.clone(),
                                }
                            })
                            .collect();
                        if format == "csv" {
                            if exclude_columns.is_empty() {
                                display_csv(&rows);
                            } else {
                                display_csv_excluding(&rows, exclude_columns);
                            }
                        } else {
                            println!("Jobs for workflow ID {}:", selected_workflow_id);
                            if exclude_columns.is_empty() {
                                display_table_with_count(&rows, "jobs");
                            } else {
                                display_table_excluding(&rows, exclude_columns, "jobs");
                            }
                        }
                    }
                }
                Err(e) => {
                    print_error("listing jobs", &e);
                    std::process::exit(1);
                }
            }
        }
        JobCommands::Get { id } => match apis::jobs_api::get_job(config, *id) {
            Ok(job) => {
                if print_if_json(format, &job, "job") {
                    // JSON was printed
                } else {
                    let status = job.status.expect("Job status is missing").to_string();
                    println!("Job ID {}:", id);
                    println!("  Name: {}", job.name);
                    println!("  Command: {}", job.command);
                    println!("  Workflow ID: {}", job.workflow_id);
                    println!("  Status: {}", status);
                    println!("  Priority: {}", job.priority.unwrap_or(0));
                    println!(
                        "  Compute Node: {}",
                        job.compute_node_id
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "None".to_string())
                    );
                    println!(
                        "  Start Time: {}",
                        job.start_time
                            .as_deref()
                            .map(format_local_timestamp)
                            .unwrap_or_else(|| "None".to_string())
                    );
                    println!(
                        "  Blocking job IDs: {}",
                        job.depends_on_job_ids
                            .as_ref()
                            .map(|ids| format!("{:?}", ids))
                            .unwrap_or_else(|| "None".to_string())
                    );
                    println!(
                        "  Input file IDs: {}",
                        job.input_file_ids
                            .as_ref()
                            .map(|ids| format!("{:?}", ids))
                            .unwrap_or_else(|| "None".to_string())
                    );
                    println!(
                        "  Output file IDs: {}",
                        job.output_file_ids
                            .as_ref()
                            .map(|ids| format!("{:?}", ids))
                            .unwrap_or_else(|| "None".to_string())
                    );
                }
            }
            Err(e) => {
                print_error("getting job", &e);
                std::process::exit(1);
            }
        },
        JobCommands::Update {
            id,
            name,
            command,
            runtime,
            resource_requirements_id,
            priority,
        } => {
            // First get the existing job
            match apis::jobs_api::get_job(config, *id) {
                Ok(mut job) => {
                    // Update fields that were provided
                    if let Some(new_name) = name {
                        job.name = new_name.clone();
                    }
                    if let Some(new_command) = command {
                        job.command = new_command.clone();
                    }
                    if let Some(new_rr_id) = resource_requirements_id {
                        job.resource_requirements_id = Some(*new_rr_id);
                    }
                    if let Some(p) = priority {
                        job.priority = Some(*p);
                    }

                    // Handle runtime update (requires updating resource requirements)
                    if let Some(new_runtime) = runtime {
                        let rr_id = job.resource_requirements_id.unwrap_or_else(|| {
                            eprintln!(
                                "Error: Cannot update runtime - job {} has no resource requirements assigned.",
                                id
                            );
                            eprintln!(
                                "Hint: First assign resource requirements with --resource-requirements-id"
                            );
                            std::process::exit(1);
                        });

                        // Get and update the resource requirements
                        match apis::resource_requirements_api::get_resource_requirements(
                            config, rr_id,
                        ) {
                            Ok(mut rr) => {
                                rr.runtime = new_runtime.clone();
                                match apis::resource_requirements_api::update_resource_requirements(
                                    config, rr_id, rr,
                                ) {
                                    Ok(_) => {
                                        if format != "json" {
                                            println!(
                                                "Updated runtime to {} on resource requirements ID {}",
                                                new_runtime, rr_id
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        print_error("updating resource requirements", &e);
                                        std::process::exit(1);
                                    }
                                }
                            }
                            Err(e) => {
                                print_error("getting resource requirements", &e);
                                std::process::exit(1);
                            }
                        }
                    }

                    match apis::jobs_api::update_job(config, *id, job) {
                        Ok(updated_job) => {
                            if print_if_json(format, &updated_job, "job") {
                                // JSON was printed
                            } else {
                                println!("Successfully updated job:");
                                println!("  ID: {}", updated_job.id.unwrap_or(-1));
                                println!("  Name: {}", updated_job.name);
                                println!("  Command: {}", updated_job.command);
                                println!("  Workflow ID: {}", updated_job.workflow_id);
                                println!(
                                    "  Resource Requirements ID: {}",
                                    updated_job
                                        .resource_requirements_id
                                        .map(|id| id.to_string())
                                        .unwrap_or_else(|| "None".to_string())
                                );
                                println!(
                                    "  Blocking job IDs: {}",
                                    updated_job
                                        .depends_on_job_ids
                                        .as_ref()
                                        .map(|ids| format!("{:?}", ids))
                                        .unwrap_or_else(|| "None".to_string())
                                );
                                println!(
                                    "  Input file IDs: {}",
                                    updated_job
                                        .input_file_ids
                                        .as_ref()
                                        .map(|ids| format!("{:?}", ids))
                                        .unwrap_or_else(|| "None".to_string())
                                );
                                println!(
                                    "  Output file IDs: {}",
                                    updated_job
                                        .output_file_ids
                                        .as_ref()
                                        .map(|ids| format!("{:?}", ids))
                                        .unwrap_or_else(|| "None".to_string())
                                );
                                println!(
                                    "  Status: {}",
                                    updated_job
                                        .status
                                        .as_ref()
                                        .map(|s| s.to_string())
                                        .unwrap_or_else(|| "None".to_string())
                                );
                            }
                        }
                        Err(e) => {
                            print_error("updating job", &e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    print_error("getting job for update", &e);
                    std::process::exit(1);
                }
            }
        }
        JobCommands::Delete { ids } => {
            if ids.is_empty() {
                eprintln!("Error: At least one job ID must be provided");
                std::process::exit(1);
            }

            // First, validate that all job IDs exist
            let mut missing_ids = Vec::new();
            for id in ids {
                match apis::jobs_api::get_job(config, *id) {
                    Ok(_) => {
                        // Job exists, continue
                    }
                    Err(_) => {
                        missing_ids.push(*id);
                    }
                }
            }

            // If any jobs don't exist, exit without deleting anything
            if !missing_ids.is_empty() {
                if format == "json" {
                    let error_result = serde_json::json!({
                        "error": "One or more job IDs do not exist",
                        "missing_ids": missing_ids
                    });
                    print_json(&error_result, "error");
                } else {
                    eprintln!("Error: The following job ID(s) do not exist:");
                    for id in &missing_ids {
                        eprintln!("  {}", id);
                    }
                    eprintln!("No jobs were deleted.");
                }
                std::process::exit(1);
            }

            // All jobs exist, proceed with deletion
            let mut deleted_jobs = Vec::new();
            for id in ids {
                match apis::jobs_api::delete_job(config, *id) {
                    Ok(removed_job) => {
                        deleted_jobs.push(removed_job);
                    }
                    Err(e) => {
                        // This should not happen since we validated existence above
                        eprintln!("Unexpected error deleting job {}: {:?}", id, e);
                        std::process::exit(1);
                    }
                }
            }

            if format == "json" {
                print_json_wrapped(&deleted_jobs, "jobs");
            } else {
                println!("Successfully removed {} job(s):", deleted_jobs.len());
                for job in &deleted_jobs {
                    println!(
                        "  ID: {} - Name: {} - Command: {}",
                        job.id.unwrap_or(-1),
                        job.name,
                        job.command
                    );
                }
            }
        }
        JobCommands::DeleteAll { workflow_id } => {
            let user_name = get_env_user_name();
            let selected_workflow_id = match workflow_id {
                Some(id) => *id,
                None => select_workflow_interactively(config, &user_name).unwrap_or_else(|e| {
                    eprintln!("Error selecting workflow: {}", e);
                    std::process::exit(1);
                }),
            };

            // Get count of jobs to delete
            match apis::jobs_api::list_jobs(
                config,
                selected_workflow_id,
                None,    // status
                None,    // needs_file_id
                None,    // upstream_job_id
                Some(0), // offset
                Some(1), // limit
                None,    // sort_by
                None,    // reverse_sort
                None,    // include_relationships
                None,    // active_compute_node_id
                None,    // origin_is_set
                None,    // name
                None,    // command
            ) {
                Ok(response) => {
                    let job_count = response.total_count;

                    if job_count == 0 {
                        if format == "json" {
                            println!("{{\"deleted\": 0, \"message\": \"No jobs to delete\"}}");
                        } else {
                            println!("No jobs found for workflow ID: {}", selected_workflow_id);
                        }
                        return;
                    }

                    // Confirm deletion
                    if format != "json" {
                        println!(
                            "About to delete {} job(s) from workflow ID: {}",
                            job_count, selected_workflow_id
                        );
                        print!("Are you sure? (y/N): ");
                        io::stdout().flush().unwrap();

                        let mut input = String::new();
                        io::stdin().read_line(&mut input).unwrap();

                        if !input.trim().eq_ignore_ascii_case("y") {
                            println!("Deletion cancelled");
                            return;
                        }
                    }

                    // Delete all jobs
                    match apis::jobs_api::delete_jobs(config, selected_workflow_id) {
                        Ok(result) => {
                            if print_if_json(format, &result, "result") {
                                // JSON was printed
                            } else if let Some(count) = result.get("count") {
                                println!(
                                    "Successfully deleted {} job(s) from workflow ID: {}",
                                    count, selected_workflow_id
                                );
                            } else {
                                println!(
                                    "Successfully deleted jobs from workflow ID: {}",
                                    selected_workflow_id
                                );
                            }
                        }
                        Err(e) => {
                            print_error("deleting all jobs", &e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    print_error("getting job count", &e);
                    std::process::exit(1);
                }
            }
        }
        JobCommands::CreateFromFile {
            workflow_id,
            file,
            cpus_per_job,
            memory_per_job,
            runtime_per_job,
        } => {
            match create_jobs_from_file(
                config,
                *workflow_id,
                file,
                *cpus_per_job,
                memory_per_job,
                runtime_per_job,
                format,
            ) {
                Ok(job_count) => {
                    if format == "json" {
                        let json_output = serde_json::json!({
                            "status": "success",
                            "message": format!("Successfully created {} jobs from file", job_count),
                            "workflow_id": workflow_id,
                            "jobs_created": job_count
                        });
                        print_json(&json_output, "response");
                    } else {
                        println!("Successfully created {} jobs from file:", job_count);
                        println!("  File: {}", file);
                        println!("  Workflow ID: {}", workflow_id);
                        println!("  CPUs per job: {}", cpus_per_job);
                        println!("  Memory per job: {}", memory_per_job);
                        println!("  Runtime per job: {}", runtime_per_job);
                    }
                }
                Err(e) => {
                    eprintln!("Error creating jobs from file '{}': {}", file, e);
                    std::process::exit(1);
                }
            }
        }
        JobCommands::ListResourceRequirements {
            workflow_id,
            job_id,
        } => {
            // Get jobs - either a single job or all jobs for a workflow
            let jobs: Vec<models::JobModel> = if let Some(jid) = job_id {
                // Get single job
                match apis::jobs_api::get_job(config, *jid) {
                    Ok(job) => vec![job],
                    Err(e) => {
                        print_error("getting job", &e);
                        std::process::exit(1);
                    }
                }
            } else {
                // Get all jobs for workflow
                let user_name = get_env_user_name();
                let selected_workflow_id = match workflow_id {
                    Some(id) => *id,
                    None => select_workflow_interactively(config, &user_name).unwrap_or_else(|e| {
                        eprintln!("Error selecting workflow: {}", e);
                        std::process::exit(1);
                    }),
                };

                match pagination::paginate_jobs(config, selected_workflow_id, JobListParams::new())
                {
                    Ok(jobs) => jobs,
                    Err(e) => {
                        print_error("listing jobs", &e);
                        std::process::exit(1);
                    }
                }
            };

            if jobs.is_empty() {
                if format == "json" {
                    println!("[]");
                } else {
                    println!("No jobs found");
                }
                return;
            }

            // Build HashMap of unique resource_requirements_id -> ResourceRequirementsModel
            let mut rr_map: HashMap<i64, models::ResourceRequirementsModel> = HashMap::new();
            for job in &jobs {
                if let Some(rr_id) = job.resource_requirements_id
                    && let std::collections::hash_map::Entry::Vacant(e) = rr_map.entry(rr_id)
                {
                    match apis::resource_requirements_api::get_resource_requirements(config, rr_id)
                    {
                        Ok(rr) => {
                            e.insert(rr);
                        }
                        Err(e) => {
                            print_error(&format!("getting resource requirements {}", rr_id), &e);
                            std::process::exit(1);
                        }
                    }
                }
            }

            if format == "json" {
                // Build JSON output - only include jobs with resource requirements
                let output: Vec<serde_json::Value> = jobs
                    .iter()
                    .filter_map(|job| {
                        job.resource_requirements_id.and_then(|rr_id| {
                            rr_map.get(&rr_id).map(|rr| {
                                serde_json::json!({
                                    "job_id": job.id,
                                    "job_name": &job.name,
                                    "rr_id": rr_id,
                                    "rr_name": &rr.name,
                                    "workflow_id": rr.workflow_id,
                                    "num_cpus": rr.num_cpus,
                                    "num_gpus": rr.num_gpus,
                                    "num_nodes": rr.num_nodes,
                                    "memory": &rr.memory,
                                    "runtime": &rr.runtime,
                                })
                            })
                        })
                    })
                    .collect();

                print_json(&output, "resource requirements");
            } else {
                // Build table rows
                let rows: Vec<JobResourceRequirementsTableRow> = jobs
                    .iter()
                    .filter_map(|job| {
                        job.resource_requirements_id.and_then(|rr_id| {
                            rr_map
                                .get(&rr_id)
                                .map(|rr| JobResourceRequirementsTableRow {
                                    job_id: job.id.unwrap_or(-1),
                                    job_name: job.name.clone(),
                                    rr_id,
                                    rr_name: rr.name.clone(),
                                    num_cpus: rr.num_cpus,
                                    num_gpus: rr.num_gpus,
                                    num_nodes: rr.num_nodes,
                                    memory: rr.memory.clone(),
                                    runtime: rr.runtime.clone(),
                                })
                        })
                    })
                    .collect();

                if format == "csv" {
                    display_csv(&rows);
                } else if rows.is_empty() {
                    println!("No jobs with resource requirements found");
                } else {
                    display_table_with_count(&rows, "jobs with resource requirements");
                }
            }
        }
        JobCommands::ListFailureHandlers {
            workflow_id,
            job_id,
        } => {
            // Get jobs - either a single job or all jobs for a workflow
            let jobs: Vec<models::JobModel> = if let Some(jid) = job_id {
                // Get single job
                match apis::jobs_api::get_job(config, *jid) {
                    Ok(job) => vec![job],
                    Err(e) => {
                        print_error("getting job", &e);
                        std::process::exit(1);
                    }
                }
            } else {
                // Get all jobs for workflow
                let user_name = get_env_user_name();
                let selected_workflow_id = match workflow_id {
                    Some(id) => *id,
                    None => select_workflow_interactively(config, &user_name).unwrap_or_else(|e| {
                        eprintln!("Error selecting workflow: {}", e);
                        std::process::exit(1);
                    }),
                };

                match pagination::paginate_jobs(config, selected_workflow_id, JobListParams::new())
                {
                    Ok(jobs) => jobs,
                    Err(e) => {
                        print_error("listing jobs", &e);
                        std::process::exit(1);
                    }
                }
            };

            if jobs.is_empty() {
                if format == "json" {
                    println!("[]");
                } else {
                    println!("No jobs found");
                }
                return;
            }

            // Build HashMap of unique failure_handler_id -> FailureHandlerModel
            let mut fh_map: HashMap<i64, models::FailureHandlerModel> = HashMap::new();
            for job in &jobs {
                if let Some(fh_id) = job.failure_handler_id
                    && let std::collections::hash_map::Entry::Vacant(e) = fh_map.entry(fh_id)
                {
                    match apis::failure_handlers_api::get_failure_handler(config, fh_id) {
                        Ok(fh) => {
                            e.insert(fh);
                        }
                        Err(e) => {
                            print_error(&format!("getting failure handler {}", fh_id), &e);
                            std::process::exit(1);
                        }
                    }
                }
            }

            if format == "json" {
                // Build JSON output - only include jobs with failure handlers
                let output: Vec<serde_json::Value> = jobs
                    .iter()
                    .filter_map(|job| {
                        job.failure_handler_id.and_then(|fh_id| {
                            fh_map.get(&fh_id).map(|fh| {
                                serde_json::json!({
                                    "job_id": job.id,
                                    "job_name": &job.name,
                                    "failure_handler_id": fh_id,
                                    "failure_handler_name": &fh.name,
                                    "rules": &fh.rules,
                                })
                            })
                        })
                    })
                    .collect();

                print_json(&output, "failure handlers");
            } else {
                // Build table rows
                let rows: Vec<JobFailureHandlerTableRow> = jobs
                    .iter()
                    .filter_map(|job| {
                        job.failure_handler_id.and_then(|fh_id| {
                            fh_map.get(&fh_id).map(|fh| JobFailureHandlerTableRow {
                                job_id: job.id.unwrap_or(-1),
                                job_name: job.name.clone(),
                                fh_id,
                                fh_name: fh.name.clone(),
                                rules_summary: format_rules_summary(&fh.rules),
                            })
                        })
                    })
                    .collect();

                if format == "csv" {
                    display_csv(&rows);
                } else if rows.is_empty() {
                    println!("No jobs with failure handlers found");
                } else {
                    display_table_with_count(&rows, "jobs with failure handlers");
                }
            }
        }
        JobCommands::ResetStatus {
            job_ids,
            force,
            dry_run,
            no_prompts,
            reinit,
        } => {
            handle_reset_job_status(
                config,
                job_ids,
                *force,
                *dry_run,
                *no_prompts,
                *reinit,
                format,
            );
        }
        JobCommands::Running { workflow_id } => {
            let user_name = get_env_user_name();
            let selected_workflow_id = match workflow_id {
                Some(id) => *id,
                None => select_workflow_interactively(config, &user_name).unwrap_or_else(|e| {
                    eprintln!("Error selecting workflow: {}", e);
                    std::process::exit(1);
                }),
            };

            // Page through the server-side endpoint, which joins running jobs to
            // their compute node and (when scheduler-managed) the scheduler job ID.
            let mut running: Vec<models::RunningJobModel> = Vec::new();
            let mut offset = 0;
            loop {
                let response = match apis::workflows_api::get_running_jobs(
                    config,
                    selected_workflow_id,
                    Some(offset),
                    None,
                ) {
                    Ok(response) => response,
                    Err(e) => {
                        print_error("listing running jobs", &e);
                        std::process::exit(1);
                    }
                };
                running.extend(response.items);
                if !response.has_more {
                    break;
                }
                offset += response.count;
            }

            if format == "json" {
                // Wrap in {"items": [...]} for consistency with other list commands.
                print_json_wrapped(&running, "running jobs");
            } else {
                let rows: Vec<RunningJobTableRow> = running
                    .iter()
                    .map(|j| RunningJobTableRow {
                        job_id: j.job_id,
                        job_name: j.job_name.clone(),
                        compute_node: j.compute_node_name.clone(),
                        elapsed: format_elapsed(j.start_time.as_deref()),
                        scheduler_type: j.scheduler_type.clone(),
                        // Match the compute-nodes listing: render absent IDs as "-".
                        scheduler_job_id: j
                            .scheduler_job_id
                            .clone()
                            .unwrap_or_else(|| "-".to_string()),
                    })
                    .collect();

                if format == "csv" {
                    display_csv(&rows);
                } else if rows.is_empty() {
                    println!("No running jobs found");
                } else {
                    display_table_with_count(&rows, "running jobs");
                }
            }
        }
    }
}

/// Format failure handler rules for table display (compact summary)
fn format_rules_summary(rules_json: &str) -> String {
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

/// Create jobs from a text file containing one command per line
pub fn create_jobs_from_file(
    config: &Configuration,
    workflow_id: i64,
    file_path: &str,
    cpus_per_job: i64,
    memory_per_job: &str,
    runtime_per_job: &str,
    _format: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    // Read the file
    let file_path = Path::new(file_path);
    if !file_path.exists() {
        return Err(format!("File does not exist: {}", file_path.display()).into());
    }

    let file_content = fs::read_to_string(file_path)?;
    let commands: Vec<&str> = file_content
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();

    if commands.is_empty() {
        return Err("No valid commands found in file".into());
    }

    // Get current job count to determine starting index
    let current_job_count = get_current_job_count(config, workflow_id)?;

    // Create resource requirements for the jobs
    let resource_req_name = format!("batch_jobs_req_{}", chrono::Utc::now().timestamp());
    let mut resource_req =
        models::ResourceRequirementsModel::new(workflow_id, resource_req_name.clone());
    resource_req.num_cpus = cpus_per_job;
    resource_req.memory = memory_per_job.to_string();
    resource_req.runtime = runtime_per_job.to_string();

    let created_resource_req =
        apis::resource_requirements_api::create_resource_requirements(config, resource_req)
            .map_err(|e| format!("Failed to create resource requirements: {:?}", e))?;

    // Create jobs
    let mut jobs = Vec::new();
    let mut used_names = get_existing_job_names(config, workflow_id)?;

    for (i, command) in commands.iter().enumerate() {
        let mut job_name = format!("job{}", current_job_count + i as i64 + 1);

        // Ensure unique job names
        let mut counter = 1;
        while used_names.contains(&job_name) {
            job_name = format!("job{}_{}", current_job_count + i as i64 + 1, counter);
            counter += 1;
        }
        used_names.insert(job_name.clone());

        let mut job = models::JobModel::new(workflow_id, job_name, command.to_string());
        job.resource_requirements_id = created_resource_req.id;
        jobs.push(job);
    }

    // Create jobs in batches using bulk API
    let batch_size = crate::MAX_RECORD_TRANSFER_COUNT as usize;
    let mut total_created = 0;

    for batch in jobs.chunks(batch_size) {
        let jobs_model = models::JobsModel::new(batch.to_vec());
        let response = apis::jobs_api::create_jobs(config, jobs_model)
            .map_err(|e| format!("Failed to create batch of jobs: {:?}", e))?;

        total_created += response.jobs.as_ref().map(|jobs| jobs.len()).unwrap_or(0);
    }

    Ok(total_created)
}

/// Get the current job count for a workflow
pub fn get_current_job_count(
    config: &Configuration,
    workflow_id: i64,
) -> Result<i64, Box<dyn std::error::Error>> {
    let response = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,    // status
        None,    // needs_file_id
        None,    // upstream_job_id
        Some(0), // offset
        Some(1), // limit (we only need the count)
        None,    // sort_by
        None,    // reverse_sort
        None,    // include_relationships
        None,    // active_compute_node_id
        None,    // origin_is_set
        None,    // name
        None,    // command
    )
    .map_err(|e| format!("Failed to get job count: {:?}", e))?;

    Ok(response.total_count)
}

// ---------------------------------------------------------------------------
// reset-status helpers
// ---------------------------------------------------------------------------

/// A single row in the jobs-to-reset preview table.
#[derive(Tabled)]
struct ResetJobRow {
    #[tabled(rename = "Job ID")]
    id: i64,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Current Status")]
    status: String,
}

/// Handler for `torc jobs reset-status`.
///
/// Resets ONLY the explicitly requested job IDs to `Uninitialized` so they can be
/// rerun.  Downstream dependents are not touched here: the server's recursive
/// `uninitialize_blocked_jobs` CTE resets every transitive dependent of an
/// uninitialized job when the user runs `torc workflows reinit` — this command
/// merely computes and displays that closure.  Does NOT bump the workflow
/// `run_id` by default — the caller should follow up with `torc workflows reinit <id>`
/// (which resets workflow status and bumps `run_id` exactly once).  Pass
/// `reinit = true` (or `--reinit` on the CLI) to perform that step automatically.
///
/// # Known limitation
/// `manage_status_change` does not clear `start_time` / `compute_node_id`; those
/// fields are overwritten when the job next starts.  Clearing them via `update_job`
/// is restricted and the cosmetic difference is acceptable.
/// The server always assigns job IDs, so a `None` here indicates a server bug;
/// fail hard rather than propagate a sentinel value into status changes.
fn require_job_id(job: &models::JobModel) -> i64 {
    job.id.unwrap_or_else(|| {
        eprintln!(
            "Error: server returned job '{}' (workflow_id={}) without an ID",
            job.name, job.workflow_id
        );
        std::process::exit(1);
    })
}

#[allow(clippy::too_many_arguments)]
fn handle_reset_job_status(
    config: &Configuration,
    job_ids: &[i64],
    force: bool,
    dry_run: bool,
    no_prompts: bool,
    reinit: bool,
    format: &str,
) {
    // -----------------------------------------------------------------------
    // 1. Fetch and validate: all IDs must exist and belong to the same workflow
    // -----------------------------------------------------------------------
    let mut fetched: Vec<models::JobModel> = Vec::with_capacity(job_ids.len());
    for &id in job_ids {
        match apis::jobs_api::get_job(config, id) {
            Ok(job) => fetched.push(job),
            Err(e) => {
                eprintln!("Error: job {} not found: {}", id, e);
                std::process::exit(1);
            }
        }
    }

    // De-duplicate by ID while preserving first-occurrence order
    let mut seen_ids: HashSet<i64> = HashSet::new();
    let mut unique_jobs: Vec<models::JobModel> = Vec::new();
    for job in fetched {
        let id = require_job_id(&job);
        if seen_ids.insert(id) {
            unique_jobs.push(job);
        }
    }

    // Determine workflow from first job; reject if any job belongs elsewhere
    let workflow_id = unique_jobs[0].workflow_id;
    let mut mismatched: Vec<(i64, i64)> = Vec::new();
    for job in &unique_jobs {
        if job.workflow_id != workflow_id {
            mismatched.push((require_job_id(job), job.workflow_id));
        }
    }
    if !mismatched.is_empty() {
        eprintln!(
            "Error: all job IDs must belong to the same workflow (inferred workflow_id={} from \
             first job), but the following jobs belong to different workflows:",
            workflow_id
        );
        for (id, wf) in &mismatched {
            eprintln!("  job {} belongs to workflow {}", id, wf);
        }
        eprintln!("Nothing was reset.");
        std::process::exit(1);
    }

    // -----------------------------------------------------------------------
    // 2. Preconditions
    // -----------------------------------------------------------------------
    if !force {
        if let Err(msg) =
            crate::client::commands::recover::check_workflow_quiesced(config, workflow_id)
        {
            eprintln!("Error: {}", msg);
            eprintln!("Use --force to skip this check.");
            std::process::exit(1);
        }

        // Reject jobs currently in Running or Pending status
        let active_targets: Vec<i64> = unique_jobs
            .iter()
            .filter(|j| {
                matches!(
                    j.status,
                    Some(models::JobStatus::Running) | Some(models::JobStatus::Pending)
                )
            })
            .map(require_job_id)
            .collect();
        if !active_targets.is_empty() {
            eprintln!(
                "Error: the following jobs are currently Running or Pending and cannot be reset \
                 without --force:"
            );
            for id in &active_targets {
                eprintln!("  job {}", id);
            }
            std::process::exit(1);
        }
    }

    // -----------------------------------------------------------------------
    // 3. Compute the downstream closure — READ-ONLY, for display only
    // -----------------------------------------------------------------------
    // For each requested job, find its transitive downstream dependents via BFS.
    // No status-change calls are issued for these jobs: the server's recursive
    // uninitialize_blocked_jobs CTE resets every transitive dependent of an
    // uninitialized job — regardless of its status — when the user runs
    // 'torc workflows reinit' (a rerun job produces new outputs, so consumers
    // must rerun too).  The closure computed here mirrors that CTE (no status
    // filter) so the user can see exactly what reinit will reset.
    let requested_ids: HashSet<i64> = unique_jobs.iter().map(require_job_id).collect();

    let mut downstream_map: HashMap<i64, models::JobModel> = HashMap::new();
    let mut visited: HashSet<i64> = requested_ids.clone();
    let mut bfs_queue: VecDeque<i64> = requested_ids.iter().copied().collect();

    while let Some(upstream_id) = bfs_queue.pop_front() {
        // Fetch direct dependents of this upstream job
        let params = JobListParams::new().with_upstream_job_id(upstream_id);
        let dependents = match pagination::paginate_jobs(config, workflow_id, params) {
            Ok(jobs) => jobs,
            Err(e) => {
                eprintln!(
                    "Error: failed to list dependents of job {}: {}",
                    upstream_id, e
                );
                std::process::exit(1);
            }
        };

        for dep in dependents {
            let dep_id = require_job_id(&dep);
            if visited.insert(dep_id) {
                downstream_map.insert(dep_id, dep);
                bfs_queue.push_back(dep_id);
            }
        }
    }

    let mut downstream_jobs: Vec<&models::JobModel> = downstream_map.values().collect();
    downstream_jobs.sort_by_key(|j| require_job_id(j));

    // -----------------------------------------------------------------------
    // 4. Warn for already-uninitialized requested jobs (no-ops)
    // -----------------------------------------------------------------------
    for job in &unique_jobs {
        if job.status == Some(models::JobStatus::Uninitialized) {
            eprintln!(
                "Warning: job {} ({}) is already uninitialized — will be a no-op.",
                require_job_id(job),
                job.name
            );
        }
    }

    // -----------------------------------------------------------------------
    // 5. Display: requested jobs table + informational downstream section
    // -----------------------------------------------------------------------
    let to_row = |job: &models::JobModel| ResetJobRow {
        id: require_job_id(job),
        name: job.name.clone(),
        status: job
            .status
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
    };
    let to_json = |job: &models::JobModel| {
        serde_json::json!({
            "job_id": require_job_id(job),
            "name": job.name,
            "status": job.status.map(|s| s.to_string()).unwrap_or_default(),
        })
    };

    if format != "json" {
        let rows: Vec<ResetJobRow> = unique_jobs.iter().map(to_row).collect();
        display_table_with_count(&rows, "jobs to reset");

        if !downstream_jobs.is_empty() {
            if reinit {
                println!(
                    "\nThe following downstream jobs will be reset now by the reinit step \
                     (a rerun job produces new outputs, so its consumers must rerun too):",
                );
            } else {
                println!(
                    "\nThe following downstream jobs will be reset when you run \
                     'torc workflows reinit {}' (a rerun job produces new outputs, so its \
                     consumers must rerun too):",
                    workflow_id
                );
            }
            let ds_rows: Vec<ResetJobRow> = downstream_jobs.iter().map(|j| to_row(j)).collect();
            display_table_with_count(&ds_rows, "downstream jobs (reset at reinit time)");
        }
    }

    // -----------------------------------------------------------------------
    // 6. Dry-run: stop here
    // -----------------------------------------------------------------------
    if dry_run {
        if format == "json" {
            let response = serde_json::json!({
                "dry_run": true,
                "reinit_requested": reinit,
                "workflow_id": workflow_id,
                "jobs": unique_jobs.iter().map(to_json).collect::<Vec<_>>(),
                "downstream_jobs": downstream_jobs
                    .iter()
                    .map(|j| to_json(j))
                    .collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        } else {
            println!("Dry run: no changes were made.");
            if reinit {
                println!("Dry run: the workflow would also be reinitialized.");
            }
        }
        return;
    }

    // -----------------------------------------------------------------------
    // 7. Confirmation prompt
    // -----------------------------------------------------------------------
    if !no_prompts && format != "json" {
        eprintln!(
            "\nAbout to reset {} job(s) in workflow {} to uninitialized.",
            unique_jobs.len(),
            workflow_id
        );
        if !downstream_jobs.is_empty() {
            if reinit {
                eprintln!(
                    "The workflow will be reinitialized now and {} downstream job(s) reset as \
                     part of it.",
                    downstream_jobs.len()
                );
            } else {
                eprintln!(
                    "{} downstream job(s) will be reset later by 'torc workflows reinit {}'.",
                    downstream_jobs.len(),
                    workflow_id
                );
            }
        }
        eprintln!("This is an idempotent operation and can be re-run if it partially fails.");
        print!("Continue? (y/N): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                if input.trim().to_lowercase() != "y" && input.trim().to_lowercase() != "yes" {
                    eprintln!("Reset cancelled.");
                    std::process::exit(0);
                }
            }
            Err(e) => {
                eprintln!("Failed to read input: {}", e);
                std::process::exit(1);
            }
        }
    }

    // -----------------------------------------------------------------------
    // 8. Apply: fetch run_id, then reset ONLY the requested jobs
    // -----------------------------------------------------------------------
    let run_id = match apis::workflows_api::get_workflow(config, workflow_id) {
        Ok(wf) => wf.run_id.unwrap_or(1),
        Err(e) => {
            eprintln!("Error: failed to fetch workflow {}: {}", workflow_id, e);
            std::process::exit(1);
        }
    };

    // The server's one-level reinitialize_downstream_jobs may incidentally reset
    // some direct Completed/Failed dependents on complete→uninitialized; that is
    // harmless — the recursive reset at reinit time covers the rest.
    let mut succeeded: Vec<i64> = Vec::new();
    let mut failures: Vec<(i64, String)> = Vec::new();

    for job in &unique_jobs {
        let id = require_job_id(job);
        match apis::jobs_api::manage_status_change(
            config,
            id,
            models::JobStatus::Uninitialized,
            run_id,
        ) {
            Ok(_) => succeeded.push(id),
            Err(e) => failures.push((id, e.to_string())),
        }
    }

    // -----------------------------------------------------------------------
    // 8b. Apply reinit (only when requested and all resets succeeded)
    // -----------------------------------------------------------------------
    let mut reinit_applied = false;
    let mut reinit_error: Option<String> = None;
    if reinit && failures.is_empty() {
        match apis::workflows_api::get_workflow(config, workflow_id) {
            Ok(workflow) => {
                let torc_config = TorcConfig::load().unwrap_or_default();
                let manager = WorkflowManager::new(config.clone(), torc_config, workflow);
                match manager.reinitialize(false, false) {
                    Ok(()) => reinit_applied = true,
                    Err(e) => reinit_error = Some(e.to_string()),
                }
            }
            Err(e) => reinit_error = Some(format!("failed to fetch workflow: {}", e)),
        }
    }

    // -----------------------------------------------------------------------
    // 9. Output
    // -----------------------------------------------------------------------
    let next_steps = if reinit_applied {
        format!(
            "Run 'torc run {}' or 'torc submit {}' to execute.",
            workflow_id, workflow_id
        )
    } else {
        format!(
            "Run 'torc workflows reinit {}' (then 'torc run'/'torc submit') to rerun these \
             jobs.",
            workflow_id
        )
    };

    if format == "json" {
        let failure_list: Vec<serde_json::Value> = failures
            .iter()
            .map(|(id, msg)| serde_json::json!({"job_id": id, "error": msg}))
            .collect();
        let all_succeeded = failures.is_empty() && (!reinit || reinit_applied);
        let status_str = if all_succeeded {
            "success"
        } else {
            "partial_failure"
        };
        let response = serde_json::json!({
            "status": status_str,
            "workflow_id": workflow_id,
            "reset_count": succeeded.len(),
            "requested_job_ids": unique_jobs.iter().map(require_job_id).collect::<Vec<_>>(),
            "reset_job_ids": succeeded,
            // Informational: these are reset by the server at reinit time,
            // not by this command.
            "downstream_jobs": downstream_jobs
                .iter()
                .map(|j| to_json(j))
                .collect::<Vec<_>>(),
            "failures": failure_list,
            "next_steps": next_steps,
            "reinit": {
                "requested": reinit,
                "applied": reinit_applied,
                "error": reinit_error,
            },
        });
        println!("{}", serde_json::to_string_pretty(&response).unwrap());
        if !all_succeeded {
            std::process::exit(1);
        }
    } else {
        if !failures.is_empty() {
            eprintln!("Error: {} job(s) could not be reset:", failures.len());
            for (id, msg) in &failures {
                eprintln!("  job {}: {}", id, msg);
            }
            eprintln!(
                "Successfully reset {} job(s). This command is idempotent — re-run it to retry \
                 the failed resets.",
                succeeded.len()
            );
            if reinit {
                eprintln!(
                    "Reinit was skipped because of the above failures. Re-run this command to \
                     retry both the resets and reinit."
                );
            }
            std::process::exit(1);
        }
        println!(
            "Successfully reset {} job(s) to uninitialized.",
            succeeded.len()
        );
        if let Some(ref err) = reinit_error {
            eprintln!("Error reinitializing workflow {}: {}", workflow_id, err);
            eprintln!(
                "The job resets succeeded. Run 'torc workflows reinit {}' manually to complete \
                 the reinit step.",
                workflow_id
            );
            std::process::exit(1);
        }
        if reinit_applied {
            println!("Reinitialized workflow {}.", workflow_id);
        }
        println!("{}", next_steps);
    }
}

/// Get existing job names to avoid duplicates
pub fn get_existing_job_names(
    config: &Configuration,
    workflow_id: i64,
) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    let mut names = HashSet::new();
    let mut offset = 0;
    let page_size = crate::MAX_RECORD_TRANSFER_COUNT;

    loop {
        let response = apis::jobs_api::list_jobs(
            config,
            workflow_id,
            None, // status
            None, // needs_file_id
            None, // upstream_job_id
            Some(offset),
            Some(page_size),
            None, // sort_by
            None, // reverse_sort
            None, // include_relationships
            None, // active_compute_node_id
            None, // origin_is_set
            None, // name
            None, // command
        )
        .map_err(|e| format!("Failed to get existing job names: {:?}", e))?;

        for job in response.items {
            names.insert(job.name);
        }

        if !response.has_more {
            break;
        }
        offset += page_size;
    }

    Ok(names)
}
