use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use crate::client::apis::configuration::Configuration;
use crate::client::apis::default_api;
use crate::client::commands::get_env_user_name;
use crate::client::commands::{
    output::{print_if_json, print_json, print_json_wrapped},
    pagination::{self, JobListParams},
    print_error, select_workflow_interactively,
    table_format::display_table_with_count,
};
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
    #[tabled(rename = "Command")]
    command: String,
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
        --cpus-per-job 4 --memory-per-job 8g --runtime-per-job P0DT2H

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
        ///   P0DT1M    = 1 minute
        ///   P0DT30M   = 30 minutes  
        ///   P0DT2H    = 2 hours
        ///   P1DT0H    = 1 day
        #[arg(long, default_value = "P0DT1M")]
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
        /// Maximum number of jobs to return
        #[arg(short, long, default_value = "10000")]
        limit: i64,
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

            match default_api::create_job(config, job) {
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
                .with_limit(*limit)
                .with_sort_by(sort_by.clone().unwrap_or_default())
                .with_reverse_sort(*reverse_sort)
                .with_include_relationships(*include_relationships);

            if let Some(job_status) = job_status {
                params = params.with_status(job_status);
            }

            if let Some(upstream_id) = upstream_job_id {
                params = params.with_upstream_job_id(*upstream_id);
            }

            match pagination::paginate_jobs(config, selected_workflow_id as i64, params) {
                Ok(jobs) => {
                    if format == "json" {
                        print_json_wrapped("jobs", &jobs, "jobs");
                    } else if jobs.is_empty() {
                        println!("No jobs found for workflow ID: {}", selected_workflow_id);
                    } else {
                        println!("Jobs for workflow ID {}:", selected_workflow_id);
                        let rows: Vec<JobTableRow> = jobs
                            .iter()
                            .map(|job| JobTableRow {
                                id: job.id.unwrap_or(-1),
                                name: job.name.clone(),
                                status: job.status.expect("Job status is missing").to_string(),
                                command: job.command.clone(),
                            })
                            .collect();
                        display_table_with_count(&rows, "jobs");
                    }
                }
                Err(e) => {
                    print_error("listing jobs", &e);
                    std::process::exit(1);
                }
            }
        }
        JobCommands::Get { id } => match default_api::get_job(config, *id) {
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
        } => {
            // First get the existing job
            match default_api::get_job(config, *id) {
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
                        match default_api::get_resource_requirements(config, rr_id) {
                            Ok(mut rr) => {
                                rr.runtime = new_runtime.clone();
                                match default_api::update_resource_requirements(config, rr_id, rr) {
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

                    match default_api::update_job(config, *id, job) {
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
                match default_api::get_job(config, *id) {
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
                match default_api::delete_job(config, *id, None) {
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
                print_json_wrapped("jobs", &deleted_jobs, "jobs");
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
            match default_api::list_jobs(
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
                    match default_api::delete_jobs(config, selected_workflow_id, None) {
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
                match default_api::get_job(config, *jid) {
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
                    match default_api::get_resource_requirements(config, rr_id) {
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

                if rows.is_empty() {
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
                match default_api::get_job(config, *jid) {
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
                    match default_api::get_failure_handler(config, fh_id) {
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

                if rows.is_empty() {
                    println!("No jobs with failure handlers found");
                } else {
                    display_table_with_count(&rows, "jobs with failure handlers");
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

    let created_resource_req = default_api::create_resource_requirements(config, resource_req)
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
    const BATCH_SIZE: usize = 1000;
    let mut total_created = 0;

    for batch in jobs.chunks(BATCH_SIZE) {
        let jobs_model = models::JobsModel::new(batch.to_vec());
        let response = default_api::create_jobs(config, jobs_model)
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
    let response = default_api::list_jobs(
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
    )
    .map_err(|e| format!("Failed to get job count: {:?}", e))?;

    Ok(response.total_count)
}

/// Get existing job names to avoid duplicates
pub fn get_existing_job_names(
    config: &Configuration,
    workflow_id: i64,
) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    let mut names = HashSet::new();
    let mut offset = 0;
    const PAGE_SIZE: i64 = 1000;

    loop {
        let response = default_api::list_jobs(
            config,
            workflow_id,
            None, // status
            None, // needs_file_id
            None, // upstream_job_id
            Some(offset),
            Some(PAGE_SIZE),
            None, // sort_by
            None, // reverse_sort
            None, // include_relationships
            None, // active_compute_node_id
        )
        .map_err(|e| format!("Failed to get existing job names: {:?}", e))?;

        if let Some(jobs) = response.items {
            for job in jobs {
                names.insert(job.name);
            }
        }

        if !response.has_more {
            break;
        }
        offset += PAGE_SIZE;
    }

    Ok(names)
}
