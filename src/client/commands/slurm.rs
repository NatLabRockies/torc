use chrono::Utc;
use clap::{Subcommand, ValueEnum};
use log::{debug, error, info, warn};

const SLURM_HELP_TEMPLATE: &str = "\
{before-help}{about-with-newline}
{usage-heading} {usage}

{all-args}

\x1b[1;32mScheduler Configuration:\x1b[0m
  \x1b[1;36mcreate\x1b[0m           Add a Slurm scheduler to the database
  \x1b[1;36mupdate\x1b[0m           Modify a Slurm scheduler in the database
  \x1b[1;36mlist\x1b[0m             List Slurm schedulers for a workflow
  \x1b[1;36mget\x1b[0m              Get a specific Slurm scheduler by ID
  \x1b[1;36mdelete\x1b[0m           Delete a Slurm scheduler by ID

\x1b[1;32mScheduler Generation:\x1b[0m
  \x1b[1;36mgenerate\x1b[0m         Generate Slurm schedulers for a workflow spec
  \x1b[1;36mregenerate\x1b[0m       Regenerate schedulers for pending jobs (recovery)

\x1b[1;32mExecution:\x1b[0m
  \x1b[1;36mschedule-nodes\x1b[0m   Submit Slurm allocations for a scheduler

\x1b[1;32mDiagnostics:\x1b[0m
  \x1b[1;36mparse-logs\x1b[0m       Parse Slurm logs for error messages
  \x1b[1;36msacct\x1b[0m            Show Slurm accounting info for allocations
{after-help}";
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::output::{print_if_json, print_json, print_wrapped_if_json};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::client::apis::configuration::Configuration;
use crate::client::apis::default_api;
use crate::client::commands::get_env_user_name;
use crate::client::commands::hpc::create_registry_with_config_public;
use crate::client::commands::pagination::{
    ComputeNodeListParams, JobListParams, ResourceRequirementsListParams, ResultListParams,
    ScheduledComputeNodeListParams, SlurmSchedulersListParams, paginate_compute_nodes,
    paginate_jobs, paginate_resource_requirements, paginate_results,
    paginate_scheduled_compute_nodes, paginate_slurm_schedulers,
};
use crate::client::commands::{
    print_error, select_workflow_interactively, table_format::display_table_with_count,
};
use crate::client::hpc::HpcProfile;
use crate::client::hpc::hpc_interface::HpcInterface;
use crate::client::utils;
use crate::client::workflow_graph::WorkflowGraph;
use crate::client::workflow_manager::WorkflowManager;
use crate::client::workflow_spec::{ResourceRequirementsSpec, SlurmDefaultsSpec, WorkflowSpec};
use crate::config::TorcConfig;
use crate::models;
use tabled::Tabled;

/// Strategy for grouping jobs into Slurm schedulers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum GroupByStrategy {
    /// Group by resource requirements name (default)
    ///
    /// Each unique resource_requirements creates a separate scheduler.
    /// This preserves the user's intent and provides fine-grained control.
    #[default]
    #[value(name = "resource-requirements")]
    ResourceRequirements,

    /// Group by partition
    ///
    /// Jobs whose resource requirements map to the same partition are grouped together.
    /// This reduces the number of schedulers and can improve resource utilization.
    #[value(name = "partition")]
    Partition,
}

impl std::fmt::Display for GroupByStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GroupByStrategy::ResourceRequirements => write!(f, "resource-requirements"),
            GroupByStrategy::Partition => write!(f, "partition"),
        }
    }
}

/// Strategy for determining Slurm job walltime
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Serialize, Deserialize)]
pub enum WalltimeStrategy {
    /// Use the maximum job runtime multiplied by a safety factor (default)
    ///
    /// Calculates walltime based on the longest job's runtime requirement,
    /// multiplied by --walltime-multiplier. This typically results in shorter
    /// walltime requests, which can improve queue priority on HPC systems.
    #[default]
    #[value(name = "max-job-runtime")]
    MaxJobRuntime,

    /// Use the partition's maximum allowed walltime
    ///
    /// Sets walltime to the maximum time allowed by the target partition.
    /// This is more conservative but may negatively impact queue scheduling.
    #[value(name = "max-partition-time")]
    MaxPartitionTime,
}

impl std::fmt::Display for WalltimeStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WalltimeStrategy::MaxJobRuntime => write!(f, "max-job-runtime"),
            WalltimeStrategy::MaxPartitionTime => write!(f, "max-partition-time"),
        }
    }
}

#[derive(Tabled)]
struct SlurmSchedulerTableRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Account")]
    account: String,
    #[tabled(rename = "Nodes")]
    nodes: i64,
    #[tabled(rename = "Walltime")]
    walltime: String,
    #[tabled(rename = "Partition")]
    partition: String,
    #[tabled(rename = "QOS")]
    qos: String,
}

/// Select a Slurm scheduler interactively from available schedulers for a workflow
fn select_slurm_scheduler_interactively(
    config: &Configuration,
    workflow_id: i64,
) -> Result<i64, Box<dyn std::error::Error>> {
    match paginate_slurm_schedulers(
        config,
        workflow_id,
        SlurmSchedulersListParams::new().with_limit(50),
    ) {
        Ok(schedulers) => {
            if schedulers.is_empty() {
                eprintln!("No Slurm schedulers found for workflow: {}", workflow_id);
                std::process::exit(1);
            }

            if schedulers.len() == 1 {
                let scheduler_id = schedulers[0].id.unwrap_or(-1);
                return Ok(scheduler_id);
            }

            eprintln!("Available Slurm schedulers:");
            eprintln!(
                "{:<5} {:<20} {:<15} {:<8} {:<12}",
                "ID", "Name", "Account", "Nodes", "Walltime"
            );
            eprintln!("{}", "-".repeat(70));
            for scheduler in schedulers.iter() {
                eprintln!(
                    "{:<5} {:<20} {:<15} {:<8} {:<12}",
                    scheduler.id.unwrap_or(-1),
                    scheduler.name.as_deref().unwrap_or(""),
                    &scheduler.account,
                    scheduler.nodes,
                    &scheduler.walltime
                );
            }

            eprintln!("\nEnter scheduler ID: ");
            use std::io::{self, Write};
            io::stdout().flush().unwrap();
            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(_) => match input.trim().parse::<i64>() {
                    Ok(id) => {
                        if schedulers.iter().any(|s| s.id == Some(id)) {
                            Ok(id)
                        } else {
                            eprintln!("Invalid scheduler ID: {}", id);
                            std::process::exit(1);
                        }
                    }
                    Err(_) => {
                        eprintln!("Invalid input. Please enter a numeric scheduler ID.");
                        std::process::exit(1);
                    }
                },
                Err(e) => {
                    eprintln!("Failed to read input: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            print_error("listing Slurm schedulers", &e);
            std::process::exit(1);
        }
    }
}

#[derive(Subcommand)]
#[command(
    help_template = SLURM_HELP_TEMPLATE,
    subcommand_help_heading = None,
    after_long_help = "\
EXAMPLES:
    # List Slurm schedulers for a workflow
    torc slurm list 123

    # Generate schedulers for a workflow spec
    torc slurm generate --account myproject workflow.yaml

    # Schedule compute nodes
    torc slurm schedule-nodes 123 --scheduler-name gpu --num-nodes 4

    # Get Slurm accounting info
    torc slurm sacct 123
")]
pub enum SlurmCommands {
    /// Add a Slurm config to the database
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # Create a basic Slurm scheduler
    torc slurm create 123 --name cpu_jobs --account myproject --walltime 04:00:00

    # Create with GPU requirements
    torc slurm create 123 --name gpu_jobs --account myproject \\
        --partition gpu --gres gpu:1 --mem 32G

    # Create with specific partition and QOS
    torc slurm create 123 --name large_jobs --account myproject \\
        --partition bigmem --qos high --nodes 2
"
    )]
    Create {
        /// Workflow ID
        #[arg()]
        workflow_id: Option<i64>,
        /// Name of config
        #[arg(short, long, required = true)]
        name: String,
        /// HPC account
        #[arg(short, long, required = true)]
        account: String,
        /// Request nodes that have at least this number of GPUs. Ex: 'gpu:2'
        #[arg(short, long)]
        gres: Option<String>,
        /// Request nodes that have at least this amount of memory. Ex: '180G'
        #[arg(short, long)]
        mem: Option<String>,
        /// Number of nodes to use for each job
        #[arg(short = 'N', long, default_value = "1")]
        nodes: i64,
        /// HPC partition. Default is determined by the scheduler
        #[arg(short, long)]
        partition: Option<String>,
        /// Controls priority of the jobs
        #[arg(short, long, default_value = "normal")]
        qos: String,
        /// Request nodes that have at least this amount of storage scratch space
        #[arg(short, long)]
        tmp: Option<String>,
        /// Slurm job walltime
        #[arg(short = 'W', long, default_value = "04:00:00")]
        walltime: String,
        /// Add extra Slurm parameters, for example --extra='--reservation=my-reservation'
        #[arg(short, long)]
        extra: Option<String>,
    },
    /// Modify a Slurm config in the database
    #[command(hide = true)]
    Update {
        #[arg()]
        scheduler_id: i64,
        /// Name of config
        #[arg(short = 'N', long)]
        name: Option<String>,
        /// HPC account
        #[arg(short, long)]
        account: Option<String>,
        /// Request nodes that have at least this number of GPUs. Ex: 'gpu:2'
        #[arg(short, long)]
        gres: Option<String>,
        /// Request nodes that have at least this amount of memory. Ex: '180G'
        #[arg(short, long)]
        mem: Option<String>,
        /// Number of nodes to use for each job
        #[arg(short, long)]
        nodes: Option<i64>,
        /// HPC partition
        #[arg(short, long)]
        partition: Option<String>,
        /// Controls priority of the jobs
        #[arg(short, long)]
        qos: Option<String>,
        /// Request nodes that have at least this amount of storage scratch space
        #[arg(short, long)]
        tmp: Option<String>,
        /// Slurm job walltime
        #[arg(long)]
        walltime: Option<String>,
        /// Add extra Slurm parameters
        #[arg(short, long)]
        extra: Option<String>,
    },
    /// Show the current Slurm configs in the database
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    torc slurm list 123
    torc -f json slurm list 123
"
    )]
    List {
        /// Workflow ID
        #[arg()]
        workflow_id: Option<i64>,
        /// Maximum number of configs to return
        #[arg(short, long, default_value = "10000")]
        limit: i64,
        /// Offset for pagination (0-based)
        #[arg(long, default_value = "0")]
        offset: i64,
    },
    /// Get a specific Slurm config by ID
    #[command(hide = true)]
    Get {
        /// ID of the Slurm config to get
        #[arg()]
        id: i64,
    },
    /// Delete a Slurm config by ID
    #[command(hide = true)]
    Delete {
        /// ID of the Slurm config to delete
        #[arg()]
        id: i64,
    },
    /// Schedule compute nodes using Slurm
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # Schedule 4 compute nodes
    torc slurm schedule-nodes 123 --num-hpc-jobs 4

    # Use specific scheduler
    torc slurm schedule-nodes 123 --scheduler-config-id 456 --num-hpc-jobs 2

    # Keep submission scripts for debugging
    torc slurm schedule-nodes 123 --keep-submission-scripts --num-hpc-jobs 4
"
    )]
    ScheduleNodes {
        /// Workflow ID
        #[arg()]
        workflow_id: Option<i64>,
        /// Job prefix for the Slurm job names
        #[arg(short, long, default_value = "worker")]
        job_prefix: String,
        /// Keep submission scripts after job submission
        #[arg(long, default_value = "false")]
        keep_submission_scripts: bool,
        /// Maximum number of parallel jobs
        #[arg(short, long)]
        max_parallel_jobs: Option<i32>,
        /// Number of HPC jobs to submit
        #[arg(short, long, default_value = "1")]
        num_hpc_jobs: i32,
        /// Output directory for job output files
        #[arg(short, long, default_value = "output")]
        output: String,
        /// Poll interval in seconds
        #[arg(short, long, default_value = "60")]
        poll_interval: i32,
        /// Scheduler config ID
        #[arg(long)]
        scheduler_config_id: Option<i64>,
        /// Start one worker per node
        #[arg(long, default_value = "false")]
        start_one_worker_per_node: bool,
    },
    /// Parse Slurm log files for known error messages
    #[command(hide = true)]
    ParseLogs {
        /// Workflow ID
        #[arg()]
        workflow_id: Option<i64>,
        /// Output directory containing Slurm log files
        #[arg(short, long, default_value = "output")]
        output_dir: PathBuf,
        /// Only show errors (skip warnings)
        #[arg(long, default_value = "false")]
        errors_only: bool,
    },
    /// Call sacct for scheduled compute nodes and display summary
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    torc slurm sacct 123
    torc slurm sacct 123 --save-json --output-dir ./reports
"
    )]
    Sacct {
        /// Workflow ID
        #[arg()]
        workflow_id: Option<i64>,
        /// Output directory for sacct JSON files (only used with --save-json)
        #[arg(short, long, default_value = "output")]
        output_dir: PathBuf,
        /// Save full JSON output to files in addition to displaying summary
        #[arg(long, default_value = "false")]
        save_json: bool,
    },
    /// Generate Slurm schedulers for a workflow based on job resource requirements
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # Preview generated schedulers
    torc slurm generate --account myproject workflow.yaml

    # Save to new file
    torc slurm generate --account myproject -o workflow_with_slurm.yaml workflow.yaml

    # Use specific HPC profile
    torc slurm generate --account myproject --profile kestrel workflow.yaml

    # Group by partition instead of resource requirements
    torc slurm generate --account myproject --group-by partition workflow.yaml
"
    )]
    Generate {
        /// Path to workflow specification file (YAML, JSON, JSON5, or KDL)
        #[arg()]
        workflow_file: PathBuf,

        /// Slurm account to use (can also be specified in workflow's slurm_defaults)
        #[arg(short, long)]
        account: Option<String>,

        /// HPC profile to use (if not specified, tries to detect current system)
        #[arg(long)]
        profile: Option<String>,

        /// Output file path (if not specified, prints to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Bundle all nodes into a single Slurm allocation per scheduler
        ///
        /// By default, creates one Slurm allocation per node (N×1 mode), which allows
        /// jobs to start as nodes become available and provides better fault tolerance.
        ///
        /// With this flag, creates one large allocation with all nodes (1×N mode),
        /// which requires all nodes to be available simultaneously but uses a single sbatch.
        #[arg(long)]
        single_allocation: bool,

        /// Strategy for grouping jobs into schedulers
        ///
        /// - resource-requirements: Each unique resource_requirements creates a separate
        ///   scheduler. This preserves user intent and provides fine-grained control.
        /// - partition: Jobs whose resource requirements map to the same partition are
        ///   grouped together, reducing the number of schedulers.
        #[arg(long, value_enum, default_value_t = GroupByStrategy::ResourceRequirements)]
        group_by: GroupByStrategy,

        /// Strategy for determining Slurm job walltime
        ///
        /// - max-job-runtime: Use the maximum job runtime multiplied by --walltime-multiplier.
        ///   This typically results in shorter walltime requests, improving queue priority.
        /// - max-partition-time: Use the partition's maximum allowed walltime. More conservative
        ///   but may negatively impact queue scheduling.
        #[arg(long, value_enum, default_value_t = WalltimeStrategy::MaxJobRuntime)]
        walltime_strategy: WalltimeStrategy,

        /// Multiplier for job runtime when using --walltime-strategy=max-job-runtime
        ///
        /// The maximum job runtime is multiplied by this value to provide a safety margin.
        /// For example, 1.5 means requesting 50% more time than the longest job estimate.
        #[arg(long, default_value = "1.5")]
        walltime_multiplier: f64,

        /// Don't add workflow actions for scheduling nodes
        #[arg(long)]
        no_actions: bool,

        /// Overwrite existing schedulers in the workflow
        #[arg(long)]
        overwrite: bool,

        /// Show what would be generated without writing to output
        #[arg(long)]
        dry_run: bool,
    },
    /// Regenerate Slurm schedulers for an existing workflow based on pending jobs
    ///
    /// Analyzes jobs that are uninitialized, ready, or blocked and generates new
    /// Slurm schedulers to run them. Uses existing scheduler configurations as
    /// defaults for account, partition, and other settings.
    ///
    /// This is useful for recovery after job failures: update job resources,
    /// reset failed jobs, then regenerate schedulers to submit new allocations.
    #[command(hide = true)]
    Regenerate {
        /// Workflow ID
        #[arg()]
        workflow_id: i64,

        /// Slurm account to use (defaults to account from existing schedulers)
        #[arg(short, long)]
        account: Option<String>,

        /// HPC profile to use (if not specified, tries to detect current system)
        #[arg(long)]
        profile: Option<String>,

        /// Bundle all nodes into a single Slurm allocation per scheduler
        #[arg(long)]
        single_allocation: bool,

        /// Strategy for grouping jobs into schedulers
        #[arg(long, value_enum, default_value_t = GroupByStrategy::ResourceRequirements)]
        group_by: GroupByStrategy,

        /// Strategy for determining Slurm job walltime
        ///
        /// - max-job-runtime: Use the maximum job runtime multiplied by --walltime-multiplier.
        ///   This typically results in shorter walltime requests, improving queue priority.
        /// - max-partition-time: Use the partition's maximum allowed walltime. More conservative
        ///   but may negatively impact queue scheduling.
        #[arg(long, value_enum, default_value_t = WalltimeStrategy::MaxJobRuntime)]
        walltime_strategy: WalltimeStrategy,

        /// Multiplier for job runtime when using --walltime-strategy=max-job-runtime
        ///
        /// The maximum job runtime is multiplied by this value to provide a safety margin.
        /// For example, 1.5 means requesting 50% more time than the longest job estimate.
        #[arg(long, default_value = "1.5")]
        walltime_multiplier: f64,

        /// Submit the generated allocations immediately
        #[arg(long)]
        submit: bool,

        /// Output directory for job output files (used when submitting)
        #[arg(short, long, default_value = "output")]
        output_dir: PathBuf,

        /// Poll interval in seconds (used when submitting)
        #[arg(short, long, default_value = "60")]
        poll_interval: i32,

        /// Show what would be created without making changes
        #[arg(long)]
        dry_run: bool,

        /// Include specific job IDs in planning regardless of their status
        /// (useful for recovery dry-run to include failed jobs)
        #[arg(long, value_delimiter = ',')]
        include_job_ids: Option<Vec<i64>>,
    },
}

/// Convert seconds to Slurm walltime format (HH:MM:SS or D-HH:MM:SS)
pub fn secs_to_walltime(secs: u64) -> String {
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let s = secs % 60;

    if hours >= 24 {
        let days = hours / 24;
        let h = hours % 24;
        format!("{}-{:02}:{:02}:{:02}", days, h, mins, s)
    } else {
        format!("{:02}:{:02}:{:02}", hours, mins, s)
    }
}

/// Generate Slurm schedulers for a workflow spec based on resource requirements
///
/// This creates one scheduler per unique resource requirement (not per job).
/// All jobs with the same resource requirements share a scheduler.
/// Actions are generated based on whether any job using that scheduler has dependencies.
///
/// # Arguments
/// * `spec` - Workflow specification to modify
/// * `profile` - HPC profile with partition information
/// * `account` - Slurm account to use
/// * `single_allocation` - If true, create 1 allocation with N nodes (1×N mode).
///   If false (default), create N allocations with 1 node each (N×1 mode).
/// * `group_by` - Strategy for grouping jobs into schedulers
/// * `walltime_strategy` - Strategy for determining walltime
/// * `walltime_multiplier` - Multiplier for job runtime when using max-job-runtime strategy
/// * `add_actions` - Whether to add workflow actions for scheduling
/// * `overwrite` - If true, overwrite existing schedulers/actions. If false, error when they exist.
#[allow(clippy::too_many_arguments)]
pub fn generate_schedulers_for_workflow(
    spec: &mut WorkflowSpec,
    profile: &HpcProfile,
    account: &str,
    single_allocation: bool,
    group_by: GroupByStrategy,
    walltime_strategy: WalltimeStrategy,
    walltime_multiplier: f64,
    add_actions: bool,
    overwrite: bool,
) -> Result<GenerateResult, String> {
    // Check if workflow already has schedulers or actions
    let has_schedulers =
        spec.slurm_schedulers.is_some() && !spec.slurm_schedulers.as_ref().unwrap().is_empty();
    let has_actions = spec.actions.is_some() && !spec.actions.as_ref().unwrap().is_empty();

    if has_schedulers || has_actions {
        if !overwrite {
            // Error out - user must explicitly use --overwrite to replace existing schedulers
            let mut msg = String::from("Workflow spec already has ");
            if has_schedulers && has_actions {
                msg.push_str("slurm_schedulers and actions");
            } else if has_schedulers {
                msg.push_str("slurm_schedulers");
            } else {
                msg.push_str("actions");
            }
            msg.push_str(" defined.\n\nOptions:\n");
            msg.push_str("  1. Use --overwrite to generate new schedulers (replaces existing)\n");
            msg.push_str("  2. Use 'torc submit' to use the existing schedulers as-is\n");
            msg.push_str("  3. Remove schedulers/actions from the spec and run submit-slurm again");
            return Err(msg);
        }
        // overwrite=true: Clear existing and regenerate
        spec.slurm_schedulers = None;
        spec.actions = None;
    }

    use crate::client::scheduler_plan::{apply_plan_to_spec, generate_scheduler_plan};

    // Save original jobs and files before expansion so we can restore them later
    let original_jobs = spec.jobs.clone();
    let original_files = spec.files.clone();

    // Expand parameters before building the graph to properly detect file-based dependencies
    spec.expand_parameters()
        .map_err(|e| format!("Failed to expand parameters: {}", e))?;

    // Build a map of resource requirements by name
    let rr_vec = spec.resource_requirements.as_deref().unwrap_or(&[]);
    let rr_map: HashMap<&str, &ResourceRequirementsSpec> =
        rr_vec.iter().map(|rr| (rr.name.as_str(), rr)).collect();

    if rr_map.is_empty() {
        return Err(
            "Workflow has no resource_requirements defined. Cannot generate schedulers."
                .to_string(),
        );
    }

    // Build workflow graph for dependency analysis and job grouping
    let graph = WorkflowGraph::from_spec(spec)
        .map_err(|e| format!("Failed to build workflow graph: {}", e))?;

    // Check for jobs without resource requirements and collect warnings
    let mut warnings: Vec<String> = Vec::new();
    for job in &spec.jobs {
        if job.resource_requirements.is_none() {
            warnings.push(format!(
                "Job '{}' has no resource_requirements, skipping scheduler generation",
                job.name
            ));
        }
    }

    // Generate the scheduler plan using shared logic
    let plan = generate_scheduler_plan(
        &graph,
        &rr_map,
        profile,
        account,
        single_allocation,
        group_by,
        walltime_strategy,
        walltime_multiplier,
        add_actions,
        None,  // No suffix for regular generation (uses "_scheduler")
        false, // Not a recovery scenario
    );

    // Combine warnings
    warnings.extend(plan.warnings.clone());

    if plan.schedulers.is_empty() {
        let mut msg = String::from("No schedulers could be generated.\n");
        if warnings.is_empty() {
            msg.push_str("No jobs with resource_requirements found.");
        } else {
            msg.push_str("\nReasons:\n");
            let max_warnings = 5;
            for warning in warnings.iter().take(max_warnings) {
                msg.push_str(&format!("  - {}\n", warning));
            }
            if warnings.len() > max_warnings {
                msg.push_str(&format!(
                    "  ... and {} more issues\n",
                    warnings.len() - max_warnings
                ));
            }
        }
        return Err(msg);
    }

    // Apply the plan to the spec
    apply_plan_to_spec(&plan, spec);

    // Restore original jobs and files to preserve parameterized format in output,
    // but keep scheduler assignments from the expanded jobs
    let mut original_jobs = original_jobs;
    for orig_job in &mut original_jobs {
        // Try direct lookup first (for non-parameterized jobs)
        if let Some(scheduler) = plan.job_assignments.get(&orig_job.name) {
            orig_job.scheduler = Some(scheduler.clone());
        } else if orig_job.use_parameters.is_some() || orig_job.parameters.is_some() {
            // For parameterized jobs, find any expanded job that matches and use its scheduler
            // All expansions of the same job will have the same resource_requirements,
            // so they'll all get the same scheduler
            let pattern_prefix = orig_job.name.split('{').next().unwrap_or(&orig_job.name);
            for (expanded_name, scheduler) in &plan.job_assignments {
                if expanded_name.starts_with(pattern_prefix) {
                    orig_job.scheduler = Some(scheduler.clone());
                    break;
                }
            }
        }
    }
    spec.jobs = original_jobs;
    spec.files = original_files;

    Ok(GenerateResult {
        scheduler_count: plan.schedulers.len(),
        action_count: plan.actions.len(),
        warnings,
    })
}

/// Result of generating schedulers
pub struct GenerateResult {
    pub scheduler_count: usize,
    pub action_count: usize,
    pub warnings: Vec<String>,
}

/// Parse memory string like "100g", "512m", "1024" (MB) into MB
pub fn parse_memory_mb(s: &str) -> Result<u64, String> {
    let s = s.trim().to_lowercase();
    if s.is_empty() {
        return Err("Empty memory string".to_string());
    }

    // Check for suffix
    if let Some(num_str) = s.strip_suffix('g') {
        let num: f64 = num_str
            .parse()
            .map_err(|_| format!("Invalid number: {}", num_str))?;
        Ok((num * 1024.0) as u64)
    } else if let Some(num_str) = s.strip_suffix('m') {
        let num: u64 = num_str
            .parse()
            .map_err(|_| format!("Invalid number: {}", num_str))?;
        Ok(num)
    } else if let Some(num_str) = s.strip_suffix('k') {
        let num: f64 = num_str
            .parse()
            .map_err(|_| format!("Invalid number: {}", num_str))?;
        Ok((num / 1024.0) as u64)
    } else {
        // Assume MB
        s.parse()
            .map_err(|_| format!("Invalid memory value: {}", s))
    }
}

/// Parse walltime string like "4:00:00", "2-00:00:00" into seconds
pub fn parse_walltime_secs(s: &str) -> Result<u64, String> {
    let s = s.trim();

    // Check for day format: D-HH:MM:SS
    if let Some((days_str, rest)) = s.split_once('-') {
        let days: u64 = days_str
            .parse()
            .map_err(|_| format!("Invalid days: {}", days_str))?;
        let hms_secs = parse_hms(rest)?;
        return Ok(days * 24 * 3600 + hms_secs);
    }

    // Check for hours format: H, HH:MM, or HH:MM:SS
    parse_hms(s)
}

fn parse_hms(s: &str) -> Result<u64, String> {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        1 => {
            // Just hours
            let hours: u64 = parts[0]
                .parse()
                .map_err(|_| format!("Invalid hours: {}", parts[0]))?;
            Ok(hours * 3600)
        }
        2 => {
            // HH:MM
            let hours: u64 = parts[0]
                .parse()
                .map_err(|_| format!("Invalid hours: {}", parts[0]))?;
            let mins: u64 = parts[1]
                .parse()
                .map_err(|_| format!("Invalid minutes: {}", parts[1]))?;
            Ok(hours * 3600 + mins * 60)
        }
        3 => {
            // HH:MM:SS
            let hours: u64 = parts[0]
                .parse()
                .map_err(|_| format!("Invalid hours: {}", parts[0]))?;
            let mins: u64 = parts[1]
                .parse()
                .map_err(|_| format!("Invalid minutes: {}", parts[1]))?;
            let secs: u64 = parts[2]
                .parse()
                .map_err(|_| format!("Invalid seconds: {}", parts[2]))?;
            Ok(hours * 3600 + mins * 60 + secs)
        }
        _ => Err(format!("Invalid time format: {}", s)),
    }
}

pub fn handle_slurm_commands(config: &Configuration, command: &SlurmCommands, format: &str) {
    match command {
        SlurmCommands::Create {
            workflow_id,
            name,
            account,
            gres,
            mem,
            nodes,
            partition,
            qos,
            tmp,
            walltime,
            extra,
        } => {
            let user_name = get_env_user_name();
            let wf_id = workflow_id.unwrap_or_else(|| {
                select_workflow_interactively(config, &user_name).unwrap_or_else(|e| {
                    eprintln!("Error selecting workflow: {}", e);
                    std::process::exit(1);
                })
            });

            let scheduler = models::SlurmSchedulerModel {
                id: None,
                workflow_id: wf_id,
                name: Some(name.clone()),
                account: account.clone(),
                gres: gres.clone(),
                mem: mem.clone(),
                nodes: *nodes,
                ntasks_per_node: None,
                partition: partition.clone(),
                qos: Some(qos.clone()),
                tmp: tmp.clone(),
                walltime: walltime.clone(),
                extra: extra.clone(),
            };

            match default_api::create_slurm_scheduler(config, scheduler) {
                Ok(created) => {
                    if print_if_json(format, &created, "Slurm scheduler") {
                        // JSON was printed
                    } else {
                        eprintln!(
                            "Added Slurm configuration '{}' (ID: {}) to workflow {}",
                            name,
                            created.id.unwrap_or(-1),
                            wf_id
                        );
                    }
                }
                Err(e) => {
                    print_error("creating Slurm scheduler", &e);
                    std::process::exit(1);
                }
            }
        }
        SlurmCommands::Update {
            scheduler_id,
            name,
            account,
            gres,
            mem,
            nodes,
            partition,
            qos,
            tmp,
            walltime,
            extra,
        } => {
            let mut scheduler = match default_api::get_slurm_scheduler(config, *scheduler_id) {
                Ok(s) => s,
                Err(e) => {
                    print_error("getting Slurm scheduler", &e);
                    std::process::exit(1);
                }
            };

            // Update fields if provided
            let mut changed = false;
            if let Some(n) = name {
                scheduler.name = Some(n.clone());
                changed = true;
            }
            if let Some(a) = account {
                scheduler.account = a.clone();
                changed = true;
            }
            if let Some(g) = gres {
                scheduler.gres = Some(g.clone());
                changed = true;
            }
            if let Some(m) = mem {
                scheduler.mem = Some(m.clone());
                changed = true;
            }
            if let Some(n) = nodes {
                scheduler.nodes = *n;
                changed = true;
            }
            if let Some(p) = partition {
                scheduler.partition = Some(p.clone());
                changed = true;
            }
            if let Some(q) = qos {
                scheduler.qos = Some(q.clone());
                changed = true;
            }
            if let Some(t) = tmp {
                scheduler.tmp = Some(t.clone());
                changed = true;
            }
            if let Some(w) = walltime {
                scheduler.walltime = w.clone();
                changed = true;
            }
            if let Some(e) = extra {
                scheduler.extra = Some(e.clone());
                changed = true;
            }

            if !changed {
                warn!("No changes requested");
                return;
            }

            match default_api::update_slurm_scheduler(config, *scheduler_id, scheduler) {
                Ok(updated) => {
                    if print_if_json(format, &updated, "Slurm scheduler") {
                        // JSON was printed
                    } else {
                        eprintln!("Updated Slurm configuration {}", scheduler_id);
                    }
                }
                Err(e) => {
                    print_error("updating Slurm scheduler", &e);
                    std::process::exit(1);
                }
            }
        }
        SlurmCommands::List {
            workflow_id,
            limit,
            offset,
        } => {
            let user_name = get_env_user_name();
            let wf_id = workflow_id.unwrap_or_else(|| {
                select_workflow_interactively(config, &user_name).unwrap_or_else(|e| {
                    eprintln!("Error selecting workflow: {}", e);
                    std::process::exit(1);
                })
            });

            match paginate_slurm_schedulers(
                config,
                wf_id,
                SlurmSchedulersListParams::new()
                    .with_offset(*offset)
                    .with_limit(*limit),
            ) {
                Ok(schedulers) => {
                    if print_wrapped_if_json(
                        format,
                        "slurm_schedulers",
                        &schedulers,
                        "Slurm schedulers",
                    ) {
                        // JSON was printed
                    } else {
                        let rows: Vec<SlurmSchedulerTableRow> = schedulers
                            .iter()
                            .map(|s| SlurmSchedulerTableRow {
                                id: s.id.unwrap_or(-1),
                                name: s.name.clone().unwrap_or_default(),
                                account: s.account.clone(),
                                nodes: s.nodes,
                                walltime: s.walltime.clone(),
                                partition: s.partition.clone().unwrap_or_default(),
                                qos: s.qos.clone().unwrap_or_default(),
                            })
                            .collect();

                        println!("Slurm configurations for workflow {}", wf_id);
                        display_table_with_count(&rows, "configs");
                    }
                }
                Err(e) => {
                    print_error("listing Slurm schedulers", &e);
                    std::process::exit(1);
                }
            }
        }
        SlurmCommands::Get { id } => match default_api::get_slurm_scheduler(config, *id) {
            Ok(scheduler) => {
                if print_if_json(format, &scheduler, "Slurm scheduler") {
                    // JSON was printed
                } else {
                    eprintln!("Slurm Config ID {}:", id);
                    eprintln!("  Name: {}", scheduler.name.unwrap_or_default());
                    eprintln!("  Workflow ID: {}", scheduler.workflow_id);
                    eprintln!("  Account: {}", scheduler.account);
                    eprintln!("  Nodes: {}", scheduler.nodes);
                    eprintln!("  Walltime: {}", scheduler.walltime);
                    eprintln!("  Partition: {}", scheduler.partition.unwrap_or_default());
                    eprintln!("  QOS: {}", scheduler.qos.unwrap_or_default());
                    eprintln!(
                        "  GRES: {}",
                        scheduler.gres.unwrap_or_else(|| "None".to_string())
                    );
                    eprintln!(
                        "  Memory: {}",
                        scheduler.mem.unwrap_or_else(|| "None".to_string())
                    );
                    eprintln!(
                        "  Tmp: {}",
                        scheduler.tmp.unwrap_or_else(|| "None".to_string())
                    );
                    eprintln!(
                        "  Extra: {}",
                        scheduler.extra.unwrap_or_else(|| "None".to_string())
                    );
                }
            }
            Err(e) => {
                print_error("getting Slurm scheduler", &e);
                std::process::exit(1);
            }
        },
        SlurmCommands::Delete { id } => {
            match default_api::delete_slurm_scheduler(config, *id, None) {
                Ok(deleted_scheduler) => {
                    if print_if_json(format, &deleted_scheduler, "Slurm scheduler") {
                        // JSON was printed
                    } else {
                        eprintln!("Successfully deleted Slurm config ID {}", id);
                        eprintln!("  Name: {}", deleted_scheduler.name.unwrap_or_default());
                        eprintln!("  Workflow ID: {}", deleted_scheduler.workflow_id);
                    }
                }
                Err(e) => {
                    print_error("deleting Slurm scheduler", &e);
                    std::process::exit(1);
                }
            }
        }
        SlurmCommands::ScheduleNodes {
            workflow_id,
            job_prefix,
            keep_submission_scripts,
            max_parallel_jobs,
            num_hpc_jobs,
            output,
            poll_interval,
            scheduler_config_id,
            start_one_worker_per_node,
        } => {
            let user_name = get_env_user_name();
            let wf_id = workflow_id.unwrap_or_else(|| {
                select_workflow_interactively(config, &user_name).unwrap_or_else(|e| {
                    eprintln!("Error selecting workflow: {}", e);
                    std::process::exit(1);
                })
            });

            // Get the workflow object
            let workflow = match default_api::get_workflow(config, wf_id) {
                Ok(w) => w,
                Err(e) => {
                    print_error("getting workflow", &e);
                    std::process::exit(1);
                }
            };

            // Check if all jobs are uninitialized and initialize the workflow if needed
            match default_api::is_workflow_uninitialized(config, wf_id) {
                Ok(response) => {
                    if let Some(is_uninitialized) =
                        response.get("is_uninitialized").and_then(|v| v.as_bool())
                    {
                        if is_uninitialized {
                            info!(
                                "Workflow {} has all jobs uninitialized. Initializing workflow...",
                                wf_id
                            );
                            let torc_config = TorcConfig::load().unwrap_or_default();
                            let workflow_manager =
                                WorkflowManager::new(config.clone(), torc_config, workflow.clone());
                            match workflow_manager.initialize(false) {
                                Ok(()) => {
                                    info!("Successfully initialized workflow {}", wf_id);
                                }
                                Err(e) => {
                                    error!("Error initializing workflow: {}", e);
                                    eprintln!("Error initializing workflow: {}", e);
                                    std::process::exit(1);
                                }
                            }
                        } else {
                            info!("Workflow {} already has initialized jobs", wf_id);
                        }
                    }
                }
                Err(e) => {
                    error!("Error checking if workflow is uninitialized: {}", e);
                    eprintln!("Error checking if workflow is uninitialized: {}", e);
                    std::process::exit(1);
                }
            }

            let sched_config_id = scheduler_config_id.unwrap_or_else(|| {
                select_slurm_scheduler_interactively(config, wf_id).unwrap_or_else(|e| {
                    eprintln!("Error selecting scheduler: {}", e);
                    std::process::exit(1);
                })
            });

            match schedule_slurm_nodes(
                config,
                wf_id,
                sched_config_id,
                *num_hpc_jobs,
                job_prefix,
                output,
                *poll_interval,
                *max_parallel_jobs,
                *start_one_worker_per_node,
                *keep_submission_scripts,
            ) {
                Ok(()) => {
                    eprintln!("Successfully running {} Slurm job(s)", num_hpc_jobs);
                }
                Err(e) => {
                    eprintln!("Error scheduling Slurm nodes: {}", e);
                    std::process::exit(1);
                }
            }
        }
        SlurmCommands::ParseLogs {
            workflow_id,
            output_dir,
            errors_only,
        } => {
            let user_name = get_env_user_name();
            let wf_id = workflow_id.unwrap_or_else(|| {
                select_workflow_interactively(config, &user_name).unwrap_or_else(|e| {
                    eprintln!("Error selecting workflow: {}", e);
                    std::process::exit(1);
                })
            });
            parse_slurm_logs(config, wf_id, output_dir, *errors_only, format);
        }
        SlurmCommands::Sacct {
            workflow_id,
            output_dir,
            save_json,
        } => {
            let user_name = get_env_user_name();
            let wf_id = workflow_id.unwrap_or_else(|| {
                select_workflow_interactively(config, &user_name).unwrap_or_else(|e| {
                    eprintln!("Error selecting workflow: {}", e);
                    std::process::exit(1);
                })
            });
            run_sacct_for_workflow(config, wf_id, output_dir, *save_json, format);
        }
        SlurmCommands::Generate {
            workflow_file,
            account,
            profile: profile_name,
            output,
            single_allocation,
            group_by,
            walltime_strategy,
            walltime_multiplier,
            no_actions,
            overwrite,
            dry_run,
        } => {
            // Validate walltime_multiplier
            if *walltime_multiplier <= 0.0 {
                eprintln!("Error: --walltime-multiplier must be greater than 0");
                std::process::exit(1);
            }
            handle_generate(
                workflow_file,
                account.as_deref(),
                profile_name.as_deref(),
                output.as_ref(),
                *single_allocation,
                *group_by,
                *walltime_strategy,
                *walltime_multiplier,
                *no_actions,
                *overwrite,
                *dry_run,
                format,
            );
        }
        SlurmCommands::Regenerate {
            workflow_id,
            account,
            profile: profile_name,
            single_allocation,
            group_by,
            walltime_strategy,
            walltime_multiplier,
            submit,
            output_dir,
            poll_interval,
            dry_run,
            include_job_ids,
        } => {
            // Validate walltime_multiplier
            if *walltime_multiplier <= 0.0 {
                eprintln!("Error: --walltime-multiplier must be greater than 0");
                std::process::exit(1);
            }
            handle_regenerate(
                config,
                *workflow_id,
                account.as_deref(),
                profile_name.as_deref(),
                *single_allocation,
                *group_by,
                *walltime_strategy,
                *walltime_multiplier,
                *submit,
                output_dir,
                *poll_interval,
                *dry_run,
                include_job_ids.as_deref(),
                format,
            );
        }
    }
}

/// Schedule Slurm compute nodes for a workflow
///
/// # Arguments
/// * `config` - API configuration
/// * `workflow_id` - Workflow ID
/// * `scheduler_config_id` - Slurm scheduler configuration ID
/// * `num_hpc_jobs` - Number of HPC jobs to submit
/// * `job_prefix` - Prefix for job names
/// * `output` - Output directory for job output files
/// * `poll_interval` - Poll interval in seconds
/// * `max_parallel_jobs` - Maximum number of parallel jobs
/// * `start_one_worker_per_node` - Start one worker per node
/// * `keep_submission_scripts` - Keep submission scripts after job submission
///
/// # Returns
/// Default wait time (in minutes) for the database to recover from network errors
const WAIT_FOR_HEALTHY_DATABASE_MINUTES: u64 = 20;

/// Result indicating success or failure
#[allow(clippy::too_many_arguments)]
pub fn schedule_slurm_nodes(
    config: &Configuration,
    workflow_id: i64,
    scheduler_config_id: i64,
    num_hpc_jobs: i32,
    job_prefix: &str,
    output: &str,
    poll_interval: i32,
    max_parallel_jobs: Option<i32>,
    start_one_worker_per_node: bool,
    keep_submission_scripts: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let scheduler = match utils::send_with_retries(
        config,
        || default_api::get_slurm_scheduler(config, scheduler_config_id),
        WAIT_FOR_HEALTHY_DATABASE_MINUTES,
    ) {
        Ok(s) => s,
        Err(e) => {
            return Err(format!("Failed to get Slurm scheduler: {}", e).into());
        }
    };

    // Fetch workflow to get slurm_defaults
    let workflow = match utils::send_with_retries(
        config,
        || default_api::get_workflow(config, workflow_id),
        WAIT_FOR_HEALTHY_DATABASE_MINUTES,
    ) {
        Ok(w) => w,
        Err(e) => {
            return Err(format!("Failed to get workflow: {}", e).into());
        }
    };

    let slurm_interface = match crate::client::hpc::slurm_interface::SlurmInterface::new() {
        Ok(interface) => interface,
        Err(e) => {
            return Err(format!("Failed to create Slurm interface: {}", e).into());
        }
    };

    let mut config_map = HashMap::new();

    // Apply workflow-level slurm_defaults first (scheduler-specific values will override)
    if let Some(ref defaults_json) = workflow.slurm_defaults {
        match serde_json::from_str::<SlurmDefaultsSpec>(defaults_json) {
            Ok(defaults) => {
                // Validate that no excluded parameters are present
                if let Err(e) = defaults.validate() {
                    return Err(e.into());
                }
                debug!("Applying slurm_defaults from workflow");
                // Apply all default parameters to config_map
                for (key, value) in defaults.to_string_map() {
                    config_map.insert(key, value);
                }
            }
            Err(e) => {
                warn!("Failed to parse slurm_defaults: {}", e);
            }
        }
    }

    // Apply scheduler-specific values (these override defaults)
    config_map.insert("account".to_string(), scheduler.account.clone());
    config_map.insert("walltime".to_string(), scheduler.walltime.clone());
    config_map.insert("nodes".to_string(), scheduler.nodes.to_string());

    if let Some(partition) = &scheduler.partition {
        config_map.insert("partition".to_string(), partition.clone());
    }
    if let Some(qos) = &scheduler.qos {
        config_map.insert("qos".to_string(), qos.clone());
    }
    if let Some(gres) = &scheduler.gres {
        config_map.insert("gres".to_string(), gres.clone());
    }
    if let Some(mem) = &scheduler.mem {
        config_map.insert("mem".to_string(), mem.clone());
    }
    if let Some(tmp) = &scheduler.tmp {
        config_map.insert("tmp".to_string(), tmp.clone());
    }
    if let Some(extra) = &scheduler.extra {
        config_map.insert("extra".to_string(), extra.clone());
    }

    std::fs::create_dir_all(output)?;

    for job_num in 1..num_hpc_jobs + 1 {
        let job_name = format!("{}_{}", job_prefix, job_num);
        let script_path = format!("{}/{}.sh", output, job_name);

        if let Err(e) = slurm_interface.create_submission_script(
            &job_name,
            &config.base_path,
            workflow_id,
            output,
            poll_interval,
            max_parallel_jobs,
            Path::new(&script_path),
            &config_map,
            start_one_worker_per_node,
        ) {
            error!("Error creating submission script: {}", e);
            return Err(e.into());
        }

        match slurm_interface.submit(Path::new(&script_path)) {
            Ok((return_code, slurm_job_id, stderr)) => {
                if return_code != 0 {
                    error!("Error submitting job: {}", stderr);
                    return Err(format!("Job submission failed: {}", stderr).into());
                }
                let slurm_job_id_int: i64 = slurm_job_id
                    .parse()
                    .unwrap_or_else(|_| panic!("Failed to parse Slurm job ID {}", slurm_job_id));

                // Create the scheduled compute node record only after we have a valid Slurm job ID
                let scheduled_compute_node = models::ScheduledComputeNodesModel::new(
                    workflow_id,
                    slurm_job_id_int,
                    scheduler_config_id,
                    "slurm".to_string(),
                    "pending".to_string(),
                );
                let created_scn = match utils::send_with_retries(
                    config,
                    || {
                        default_api::create_scheduled_compute_node(
                            config,
                            scheduled_compute_node.clone(),
                        )
                    },
                    WAIT_FOR_HEALTHY_DATABASE_MINUTES,
                ) {
                    Ok(scn) => scn,
                    Err(e) => {
                        error!("Failed to create scheduled compute node: {}", e);
                        return Err(
                            format!("Failed to create scheduled compute node: {}", e).into()
                        );
                    }
                };
                let scn_id = created_scn
                    .id
                    .expect("Created scheduled compute node should have an ID");
                info!(
                    "Submitted Slurm job name={} with ID={} (scheduled_compute_node_id={})",
                    job_name, slurm_job_id_int, scn_id
                );
            }
            Err(e) => {
                error!("Error submitting job: {}", e);
                return Err(e.into());
            }
        }

        if !keep_submission_scripts && let Err(e) = std::fs::remove_file(&script_path) {
            error!("Failed to remove submission script: {}", e);
        }
    }

    Ok(())
}

/// Create a ComputeNodesResources instance by reading information from the Slurm environment
///
/// # Arguments
/// * `interface` - SlurmInterface instance to query for system resources
/// * `scheduler_config_id` - The scheduler configuration ID to use
/// * `is_subtask` - If true, use CPUs per task instead of CPUs per node
///
/// # Returns
/// A ComputeNodesResources instance populated with Slurm environment data
pub fn create_node_resources(
    interface: &crate::client::hpc::slurm_interface::SlurmInterface,
    scheduler_config_id: Option<i64>,
    is_subtask: bool,
) -> models::ComputeNodesResources {
    let num_cpus_in_node = interface.get_num_cpus() as i64;
    let memory_gb_in_node = interface.get_memory_gb();
    let num_cpus = if is_subtask {
        interface.get_num_cpus_per_task() as i64
    } else {
        num_cpus_in_node
    };
    let memory_gb = if is_subtask {
        let num_workers = num_cpus_in_node / num_cpus;
        memory_gb_in_node / num_workers as f64
    } else {
        memory_gb_in_node
    };

    let num_gpus = interface.get_num_gpus() as i64;
    let num_nodes = interface.get_num_nodes() as i64;
    let mut resources =
        models::ComputeNodesResources::new(num_cpus, memory_gb, num_gpus, num_nodes);
    resources.scheduler_config_id = scheduler_config_id;
    resources
}

/// Create a ComputeNodeModel instance.
///
/// # Arguments
/// * `resources` - ComputeNodesResources
///
/// # Returns
/// A ComputeNodeModel instance populated with Slurm environment data
pub fn create_compute_node(
    config: &Configuration,
    workflow_id: i64,
    resources: &models::ComputeNodesResources,
    hostname: &str,
    scheduler: serde_json::Value,
) -> models::ComputeNodeModel {
    let pid = std::process::id() as i64;
    let compute_node = models::ComputeNodeModel::new(
        workflow_id,
        hostname.to_string(),
        pid,
        Utc::now().to_rfc3339(),
        resources.num_cpus,
        resources.memory_gb,
        resources.num_gpus,
        resources.num_nodes,
        "slurm".to_string(),
        Some(scheduler),
    );

    match utils::send_with_retries(
        config,
        || default_api::create_compute_node(config, compute_node.clone()),
        WAIT_FOR_HEALTHY_DATABASE_MINUTES,
    ) {
        Ok(node) => node,
        Err(e) => {
            error!("Error creating compute node: {}", e);
            std::process::exit(1);
        }
    }
}

/// Known Slurm error patterns and their descriptions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlurmErrorPattern {
    pub pattern: String,
    pub description: String,
    pub severity: String, // "error", "warning", "info"
}

/// Information about a Torc job affected by a Slurm error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectedJob {
    pub job_id: i64,
    pub job_name: String,
}

/// A detected error in a Slurm log file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlurmLogError {
    pub file: String,
    pub slurm_job_id: String,
    pub line_number: usize,
    pub line: String,
    pub pattern_description: String,
    pub severity: String,
    pub node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affected_jobs: Option<Vec<AffectedJob>>,
}

/// Get known Slurm error patterns to search for
fn get_slurm_error_patterns() -> Vec<SlurmErrorPattern> {
    vec![
        // Memory-related errors
        SlurmErrorPattern {
            pattern: r"(?i)out of memory".to_string(),
            description: "Out of memory error".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)oom-kill".to_string(),
            description: "OOM killer terminated process".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)cannot allocate memory".to_string(),
            description: "Memory allocation failure".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)memory cgroup out of memory".to_string(),
            description: "Cgroup memory limit exceeded".to_string(),
            severity: "error".to_string(),
        },
        // Slurm-specific errors
        SlurmErrorPattern {
            pattern: r"(?i)slurmstepd: error:".to_string(),
            description: "Slurm step daemon error".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)srun: error:".to_string(),
            description: "Slurm srun error".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)DUE TO TIME LIMIT".to_string(),
            description: "Job terminated due to time limit".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)CANCELLED".to_string(),
            description: "Job was cancelled".to_string(),
            severity: "warning".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)DUE TO PREEMPTION".to_string(),
            description: "Job terminated due to preemption".to_string(),
            severity: "warning".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)NODE_FAIL".to_string(),
            description: "Node failure".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)FAILED".to_string(),
            description: "Job failed".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)Exceeded job memory limit".to_string(),
            description: "Exceeded job memory limit".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)task/cgroup: .*: Killed".to_string(),
            description: "Task killed by cgroup".to_string(),
            severity: "error".to_string(),
        },
        // File system errors
        SlurmErrorPattern {
            pattern: r"(?i)No space left on device".to_string(),
            description: "Disk full".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)Disk quota exceeded".to_string(),
            description: "Disk quota exceeded".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)Read-only file system".to_string(),
            description: "Read-only file system".to_string(),
            severity: "error".to_string(),
        },
        // Network errors
        SlurmErrorPattern {
            pattern: r"(?i)Connection refused".to_string(),
            description: "Connection refused".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)Connection timed out".to_string(),
            description: "Connection timed out".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)Network is unreachable".to_string(),
            description: "Network unreachable".to_string(),
            severity: "error".to_string(),
        },
        // GPU errors
        SlurmErrorPattern {
            pattern: r"(?i)CUDA out of memory".to_string(),
            description: "CUDA out of memory".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)CUDA error".to_string(),
            description: "CUDA error".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)GPU memory.*exceeded".to_string(),
            description: "GPU memory exceeded".to_string(),
            severity: "error".to_string(),
        },
        // Signal-related
        SlurmErrorPattern {
            pattern: r"(?i)Segmentation fault".to_string(),
            description: "Segmentation fault".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)SIGSEGV".to_string(),
            description: "SIGSEGV signal".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)Bus error".to_string(),
            description: "Bus error".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)SIGBUS".to_string(),
            description: "SIGBUS signal".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)killed by signal".to_string(),
            description: "Process killed by signal".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"(?i)core dumped".to_string(),
            description: "Core dump generated".to_string(),
            severity: "error".to_string(),
        },
        // Python errors
        SlurmErrorPattern {
            pattern: r"Traceback \(most recent call last\)".to_string(),
            description: "Python traceback".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"ModuleNotFoundError".to_string(),
            description: "Python module not found".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"ImportError".to_string(),
            description: "Python import error".to_string(),
            severity: "error".to_string(),
        },
        // Memory allocation errors (C++/Python)
        SlurmErrorPattern {
            pattern: r"std::bad_alloc".to_string(),
            description: "C++ memory allocation failure".to_string(),
            severity: "error".to_string(),
        },
        SlurmErrorPattern {
            pattern: r"MemoryError".to_string(),
            description: "Python memory error".to_string(),
            severity: "error".to_string(),
        },
        // Permission errors
        SlurmErrorPattern {
            pattern: r"(?i)Permission denied".to_string(),
            description: "Permission denied".to_string(),
            severity: "error".to_string(),
        },
        // Slurm job info
        SlurmErrorPattern {
            pattern: r"slurmstepd: error: .*Exceeded.*step.*limit".to_string(),
            description: "Exceeded step resource limit".to_string(),
            severity: "error".to_string(),
        },
    ]
}

/// Extract node name from log line if present
fn extract_node_from_line(line: &str) -> Option<String> {
    // Common patterns for node names in Slurm logs
    // Pattern: "node123" or "x1234c0s1b0n0" or "r123i1n2"
    let node_patterns = [
        r"\b([a-z]+\d+[a-z]*\d*)\b",                       // Simple: node123
        r"\b([xrc]\d+[a-z]\d+[a-z]\d+[a-z]\d+[a-z]\d+)\b", // HPC: x1234c0s1b0n0
    ];

    for pattern in node_patterns.iter() {
        if let Ok(re) = Regex::new(pattern)
            && let Some(caps) = re.captures(line)
            && let Some(node) = caps.get(1)
        {
            return Some(node.as_str().to_string());
        }
    }
    None
}

/// Extract workflow ID and Slurm job ID from filename
/// Returns (workflow_id, slurm_job_id) if successful
fn extract_slurm_job_id_from_filename(filename: &str) -> Option<(i64, String)> {
    // Pattern: slurm_output_wf1234_sl12345.o or slurm_output_wf1234_sl12345.e
    let re = Regex::new(r"slurm_output_wf(\d+)_sl(\d+)\.[oe]$").ok()?;
    re.captures(filename).and_then(|caps| {
        let wf_id = caps.get(1)?.as_str().parse::<i64>().ok()?;
        let slurm_id = caps.get(2)?.as_str().to_string();
        Some((wf_id, slurm_id))
    })
}

/// Build a map of Slurm job ID -> affected Torc jobs
/// This queries the API to correlate:
/// 1. ScheduledComputeNode.scheduler_id (Slurm job ID) -> ScheduledComputeNode.id
/// 2. ComputeNode.scheduler.scheduler_id -> ScheduledComputeNode.id -> Slurm job ID
/// 3. Result.compute_node_id -> job_id
/// 4. Job.id -> job_name
fn build_slurm_to_jobs_map(
    config: &Configuration,
    workflow_id: i64,
) -> HashMap<String, Vec<AffectedJob>> {
    let mut slurm_to_jobs: HashMap<String, Vec<AffectedJob>> = HashMap::new();

    // Step 1: Get all scheduled compute nodes (they have scheduler_id = Slurm job ID)
    let scheduled_nodes = match paginate_scheduled_compute_nodes(
        config,
        workflow_id,
        ScheduledComputeNodeListParams::new(),
    ) {
        Ok(nodes) => nodes,
        Err(e) => {
            warn!(
                "Could not fetch scheduled compute nodes for job correlation: {}",
                e
            );
            return slurm_to_jobs;
        }
    };

    // Build scn_id -> slurm_job_id map
    let scn_to_slurm: HashMap<i64, String> = scheduled_nodes
        .iter()
        .filter(|scn| scn.scheduler_type == "slurm")
        .filter_map(|scn| scn.id.map(|id| (id, scn.scheduler_id.to_string())))
        .collect();

    if scn_to_slurm.is_empty() {
        return slurm_to_jobs;
    }

    // Step 2: Get all compute nodes and build slurm_job_id -> compute_node_ids map
    let compute_nodes =
        match paginate_compute_nodes(config, workflow_id, ComputeNodeListParams::new()) {
            Ok(nodes) => nodes,
            Err(e) => {
                warn!("Could not fetch compute nodes for job correlation: {}", e);
                return slurm_to_jobs;
            }
        };

    // Build slurm_job_id -> Vec<compute_node_id> map using SCN relationship
    let mut slurm_to_compute_nodes: HashMap<String, Vec<i64>> = HashMap::new();
    for node in &compute_nodes {
        if node.compute_node_type != "slurm" {
            continue;
        }
        if let Some(scheduler) = &node.scheduler {
            // Get the SCN ID from the scheduler JSON
            if let Some(scn_id) = scheduler.get("scheduler_id").and_then(|v| v.as_i64()) {
                // Look up the Slurm job ID from our SCN map
                if let Some(slurm_job_id) = scn_to_slurm.get(&scn_id)
                    && let Some(node_id) = node.id
                {
                    slurm_to_compute_nodes
                        .entry(slurm_job_id.clone())
                        .or_default()
                        .push(node_id);
                }
            }
        }
    }

    if slurm_to_compute_nodes.is_empty() {
        return slurm_to_jobs;
    }

    // Step 3: Get all results and build compute_node_id -> Vec<job_id> map
    let results = match paginate_results(
        config,
        workflow_id,
        ResultListParams::new().with_all_runs(true),
    ) {
        Ok(results) => results,
        Err(e) => {
            warn!("Could not fetch results for job correlation: {}", e);
            return slurm_to_jobs;
        }
    };

    let mut compute_node_to_jobs: HashMap<i64, Vec<i64>> = HashMap::new();
    for result in &results {
        compute_node_to_jobs
            .entry(result.compute_node_id)
            .or_default()
            .push(result.job_id);
    }

    // Step 4: Get all jobs and build job_id -> job_name map
    let jobs = match paginate_jobs(config, workflow_id, JobListParams::new()) {
        Ok(jobs) => jobs,
        Err(e) => {
            warn!("Could not fetch jobs for job correlation: {}", e);
            return slurm_to_jobs;
        }
    };

    let job_id_to_name: HashMap<i64, String> = jobs
        .iter()
        .filter_map(|j| j.id.map(|id| (id, j.name.clone())))
        .collect();

    // Step 5: Build the final slurm_job_id -> Vec<AffectedJob> map
    for (slurm_id, compute_node_ids) in &slurm_to_compute_nodes {
        let mut affected_jobs: Vec<AffectedJob> = Vec::new();
        let mut seen_job_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();

        for compute_node_id in compute_node_ids {
            if let Some(job_ids) = compute_node_to_jobs.get(compute_node_id) {
                for job_id in job_ids {
                    if seen_job_ids.insert(*job_id) {
                        let job_name = job_id_to_name
                            .get(job_id)
                            .cloned()
                            .unwrap_or_else(|| format!("job_{}", job_id));
                        affected_jobs.push(AffectedJob {
                            job_id: *job_id,
                            job_name,
                        });
                    }
                }
            }
        }

        if !affected_jobs.is_empty() {
            // Sort by job_id for consistent output
            affected_jobs.sort_by_key(|j| j.job_id);
            slurm_to_jobs.insert(slurm_id.clone(), affected_jobs);
        }
    }

    slurm_to_jobs
}

/// Parse Slurm log files for known error messages
pub fn parse_slurm_logs(
    config: &Configuration,
    workflow_id: i64,
    output_dir: &PathBuf,
    errors_only: bool,
    format: &str,
) {
    if !output_dir.exists() {
        eprintln!(
            "Error: Output directory does not exist: {}",
            output_dir.display()
        );
        std::process::exit(1);
    }

    // Get scheduled compute nodes for this workflow to find valid Slurm job IDs
    let all_scheduled_nodes = match paginate_scheduled_compute_nodes(
        config,
        workflow_id,
        ScheduledComputeNodeListParams::new(),
    ) {
        Ok(nodes) => nodes,
        Err(e) => {
            print_error("listing scheduled compute nodes", &e);
            std::process::exit(1);
        }
    };

    // Filter for Slurm scheduler type only
    let scheduled_nodes: Vec<_> = all_scheduled_nodes
        .into_iter()
        .filter(|n| n.scheduler_type.to_lowercase() == "slurm")
        .collect();

    // Build set of valid Slurm job IDs for this workflow
    let valid_slurm_job_ids: std::collections::HashSet<String> = scheduled_nodes
        .iter()
        .map(|n| n.scheduler_id.to_string())
        .collect();

    if valid_slurm_job_ids.is_empty() {
        if format == "json" {
            print_json(
                &serde_json::json!({
                    "workflow_id": workflow_id,
                    "output_dir": output_dir.display().to_string(),
                    "message": "No Slurm scheduled compute nodes found for this workflow",
                    "total_issues": 0,
                    "errors": 0,
                    "warnings": 0,
                    "issues": []
                }),
                "Slurm parse logs",
            );
        } else {
            println!(
                "No Slurm scheduled compute nodes found for workflow {}",
                workflow_id
            );
        }
        return;
    }

    info!(
        "Found {} Slurm job(s) for workflow {}: {:?}",
        valid_slurm_job_ids.len(),
        workflow_id,
        valid_slurm_job_ids
    );

    // Build job correlation map
    let slurm_to_jobs = build_slurm_to_jobs_map(config, workflow_id);

    let patterns = get_slurm_error_patterns();
    let compiled_patterns: Vec<(Regex, &SlurmErrorPattern)> = patterns
        .iter()
        .filter_map(|p| Regex::new(&p.pattern).ok().map(|re| (re, p)))
        .collect();

    let mut all_errors: Vec<SlurmLogError> = Vec::new();
    let mut scanned_files = 0;

    // Find all slurm log files (*.o and *.e files matching slurm_output_*.*)
    let entries = match fs::read_dir(output_dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Error reading directory: {}", e);
            std::process::exit(1);
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => continue,
        };

        // Check if this is a slurm output file
        if !filename.starts_with("slurm_output_") {
            continue;
        }

        let (file_wf_id, slurm_job_id) = match extract_slurm_job_id_from_filename(filename) {
            Some(ids) => ids,
            None => continue,
        };

        // Only process log files for this workflow
        if file_wf_id != workflow_id {
            debug!(
                "Skipping log file {} - workflow ID {} does not match {}",
                filename, file_wf_id, workflow_id
            );
            continue;
        }

        // Only process log files for Slurm jobs belonging to this workflow
        if !valid_slurm_job_ids.contains(&slurm_job_id) {
            debug!(
                "Skipping log file {} - Slurm job {} not in workflow {}",
                filename, slurm_job_id, workflow_id
            );
            continue;
        }

        debug!("Scanning log file: {}", path.display());
        scanned_files += 1;

        let file = match fs::File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                warn!("Could not open file {}: {}", path.display(), e);
                continue;
            }
        };

        let reader = BufReader::new(file);
        for (line_num, line_result) in reader.lines().enumerate() {
            let line = match line_result {
                Ok(l) => l,
                Err(_) => continue,
            };

            for (regex, pattern) in &compiled_patterns {
                if errors_only && pattern.severity != "error" {
                    continue;
                }

                if regex.is_match(&line) {
                    let node = extract_node_from_line(&line);
                    let affected_jobs = slurm_to_jobs.get(&slurm_job_id).cloned();
                    all_errors.push(SlurmLogError {
                        file: path.display().to_string(),
                        slurm_job_id: slurm_job_id.clone(),
                        line_number: line_num + 1,
                        line: line.clone(),
                        pattern_description: pattern.description.clone(),
                        severity: pattern.severity.clone(),
                        node,
                        affected_jobs,
                    });
                    break; // Only match one pattern per line
                }
            }
        }
    }

    info!(
        "Scanned {} log file(s) for workflow {}",
        scanned_files, workflow_id
    );

    // Output results
    if format == "json" {
        let output = serde_json::json!({
            "workflow_id": workflow_id,
            "output_dir": output_dir.display().to_string(),
            "slurm_jobs_count": valid_slurm_job_ids.len(),
            "files_scanned": scanned_files,
            "total_issues": all_errors.len(),
            "errors": all_errors.iter().filter(|e| e.severity == "error").count(),
            "warnings": all_errors.iter().filter(|e| e.severity == "warning").count(),
            "issues": all_errors,
        });
        print_json(&output, "Slurm parse logs");
    } else if all_errors.is_empty() {
        println!(
            "No issues found in Slurm log files for workflow {} (scanned {} file(s) in {})",
            workflow_id,
            scanned_files,
            output_dir.display()
        );
    } else {
        println!("Found {} issue(s) in Slurm log files:\n", all_errors.len());

        // Group by Slurm job ID
        let mut errors_by_job: HashMap<String, Vec<&SlurmLogError>> = HashMap::new();
        for err in &all_errors {
            errors_by_job
                .entry(err.slurm_job_id.clone())
                .or_default()
                .push(err);
        }

        for (job_id, errors) in errors_by_job.iter() {
            let job_label = if job_id.is_empty() {
                "Unknown Job".to_string()
            } else {
                format!("Slurm Job {}", job_id)
            };

            // Get affected Torc jobs for this Slurm job (from first error, they should all be the same)
            let affected_jobs_info = errors
                .first()
                .and_then(|e| e.affected_jobs.as_ref())
                .map(|jobs| {
                    let job_list: Vec<String> = jobs
                        .iter()
                        .map(|j| format!("{} (ID: {})", j.job_name, j.job_id))
                        .collect();
                    format!("\n  Affected Torc jobs: {}", job_list.join(", "))
                })
                .unwrap_or_default();

            println!("=== {} ==={}", job_label, affected_jobs_info);
            for err in errors {
                let severity_marker = match err.severity.as_str() {
                    "error" => "[ERROR]",
                    "warning" => "[WARN]",
                    _ => "[INFO]",
                };
                let node_info = err
                    .node
                    .as_ref()
                    .map(|n| format!(" (node: {})", n))
                    .unwrap_or_default();

                println!(
                    "  {} {}{}: {}",
                    severity_marker, err.pattern_description, node_info, err.line
                );
                println!("    Location: {}:{}", err.file, err.line_number);
            }
            println!();
        }

        // Summary
        let error_count = all_errors.iter().filter(|e| e.severity == "error").count();
        let warning_count = all_errors
            .iter()
            .filter(|e| e.severity == "warning")
            .count();
        println!(
            "Summary: {} error(s), {} warning(s)",
            error_count, warning_count
        );
    }
}

/// Table row for sacct summary output
#[derive(Tabled, Serialize, Deserialize, Clone)]
pub struct SacctSummaryRow {
    #[tabled(rename = "Slurm Job")]
    pub slurm_job_id: String,
    #[tabled(rename = "Job Step")]
    pub job_step: String,
    #[tabled(rename = "State")]
    pub state: String,
    #[tabled(rename = "Exit Code")]
    pub exit_code: String,
    #[tabled(rename = "Elapsed")]
    pub elapsed: String,
    #[tabled(rename = "Max RSS")]
    pub max_rss: String,
    #[tabled(rename = "CPU Time")]
    pub cpu_time: String,
    #[tabled(rename = "Nodes")]
    pub nodes: String,
}

/// Extract state string from various sacct JSON formats
/// Handles: state.current (array or string), state (string), job_state (array or string)
fn extract_state_from_job(job: &serde_json::Value) -> String {
    // Try state.current first (newer API format)
    if let Some(state_obj) = job.get("state") {
        if let Some(current) = state_obj.get("current") {
            // current might be an array of strings or a single string
            if let Some(arr) = current.as_array() {
                // Join multiple states (e.g., ["CANCELLED", "TIMEOUT"])
                let states: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
                if !states.is_empty() {
                    return states.join(", ");
                }
            } else if let Some(s) = current.as_str() {
                return s.to_string();
            }
        }
        // state might be a simple string
        if let Some(s) = state_obj.as_str() {
            return s.to_string();
        }
    }

    // Try job_state (alternative field name in some versions)
    if let Some(job_state) = job.get("job_state") {
        if let Some(arr) = job_state.as_array() {
            let states: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
            if !states.is_empty() {
                return states.join(", ");
            }
        } else if let Some(s) = job_state.as_str() {
            return s.to_string();
        }
    }

    "-".to_string()
}

/// Parse sacct JSON output and extract summary rows
fn parse_sacct_json_to_rows(
    sacct_json: &serde_json::Value,
    slurm_job_id: &str,
) -> Vec<SacctSummaryRow> {
    let mut rows = Vec::new();

    if let Some(jobs) = sacct_json.get("jobs").and_then(|j| j.as_array()) {
        for job in jobs {
            // Get job step name
            let job_step = job
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();

            // Get state - handles various sacct JSON formats
            let state = extract_state_from_job(job);

            // Get exit code - handle different sacct JSON formats
            let exit_code = job
                .get("exit_code")
                .and_then(|e| {
                    // Could be {"status": "SUCCESS", "return_code": 0} or just a number
                    if let Some(obj) = e.as_object() {
                        obj.get("return_code")
                            .and_then(|r| r.as_i64())
                            .map(|r| r.to_string())
                    } else {
                        e.as_i64().map(|r| r.to_string())
                    }
                })
                .unwrap_or("-".to_string());

            // Get elapsed time - could be in different formats
            let elapsed = job
                .get("time")
                .and_then(|t| t.get("elapsed"))
                .and_then(|e| e.as_i64())
                .map(format_duration_seconds)
                .or_else(|| {
                    job.get("elapsed")
                        .and_then(|e| e.as_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or("-".to_string());

            // Get max RSS - handle different formats
            let max_rss = job
                .get("tres")
                .and_then(|t| t.get("requested"))
                .and_then(|r| r.get("max"))
                .and_then(|m| m.as_array())
                .and_then(|arr| {
                    arr.iter()
                        .find(|item| item.get("type").and_then(|t| t.as_str()) == Some("mem"))
                })
                .and_then(|mem| mem.get("count").and_then(|c| c.as_i64()))
                .map(|bytes| format_bytes(bytes as u64))
                .or_else(|| {
                    job.get("steps")
                        .and_then(|s| s.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|step| step.get("tres").and_then(|t| t.get("requested")))
                        .and_then(|r| r.get("max"))
                        .and_then(|m| m.as_array())
                        .and_then(|arr| {
                            arr.iter().find(|item| {
                                item.get("type").and_then(|t| t.as_str()) == Some("mem")
                            })
                        })
                        .and_then(|mem| mem.get("count").and_then(|c| c.as_i64()))
                        .map(|bytes| format_bytes(bytes as u64))
                })
                .unwrap_or("-".to_string());

            // Get CPU time
            let cpu_time = job
                .get("time")
                .and_then(|t| t.get("total"))
                .and_then(|t| t.get("seconds"))
                .and_then(|s| s.as_i64())
                .map(format_duration_seconds)
                .unwrap_or("-".to_string());

            // Get nodes
            let nodes = job
                .get("nodes")
                .and_then(|n| n.as_str())
                .unwrap_or("-")
                .to_string();

            rows.push(SacctSummaryRow {
                slurm_job_id: slurm_job_id.to_string(),
                job_step,
                state,
                exit_code,
                elapsed,
                max_rss,
                cpu_time,
                nodes,
            });
        }
    }

    rows
}

/// Format duration in seconds to human-readable format
fn format_duration_seconds(secs: i64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        format!("{}h {}m", hours, mins)
    }
}

/// Format bytes to human-readable format
fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1}GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Run sacct for all scheduled compute nodes of type slurm and display summary
pub fn run_sacct_for_workflow(
    config: &Configuration,
    workflow_id: i64,
    output_dir: &PathBuf,
    save_json: bool,
    format: &str,
) {
    // Get scheduled compute nodes for the workflow
    let all_nodes = match paginate_scheduled_compute_nodes(
        config,
        workflow_id,
        ScheduledComputeNodeListParams::new(),
    ) {
        Ok(nodes) => nodes,
        Err(e) => {
            print_error("listing scheduled compute nodes", &e);
            std::process::exit(1);
        }
    };

    // Filter for Slurm scheduler type only
    let nodes: Vec<_> = all_nodes
        .into_iter()
        .filter(|n| n.scheduler_type.to_lowercase() == "slurm")
        .collect();

    if nodes.is_empty() {
        if format == "json" {
            print_json(
                &serde_json::json!({
                    "workflow_id": workflow_id,
                    "message": "No Slurm scheduled compute nodes found",
                    "summary": []
                }),
                "Slurm sacct",
            );
        } else {
            println!(
                "No Slurm scheduled compute nodes found for workflow {}",
                workflow_id
            );
        }
        return;
    }

    // Create output directory if saving JSON
    if save_json && let Err(e) = fs::create_dir_all(output_dir) {
        eprintln!("Error creating output directory: {}", e);
        std::process::exit(1);
    }

    let mut all_summary_rows: Vec<SacctSummaryRow> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for node in &nodes {
        let slurm_job_id = node.scheduler_id.to_string();

        info!("Running sacct for Slurm job ID: {}", slurm_job_id);

        // Run sacct with JSON output
        let sacct_result = Command::new("sacct")
            .args(["-j", &slurm_job_id, "--json"])
            .output();

        match sacct_result {
            Ok(output) => {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);

                    // Parse the JSON output
                    match serde_json::from_str::<serde_json::Value>(&stdout) {
                        Ok(sacct_json) => {
                            // Extract summary rows
                            let rows = parse_sacct_json_to_rows(&sacct_json, &slurm_job_id);
                            all_summary_rows.extend(rows);

                            // Optionally save full JSON to file
                            if save_json {
                                let output_file =
                                    output_dir.join(format!("sacct_{}.json", slurm_job_id));
                                if let Err(e) = fs::write(&output_file, stdout.as_bytes()) {
                                    error!(
                                        "Failed to write sacct output for job {}: {}",
                                        slurm_job_id, e
                                    );
                                    errors.push(format!(
                                        "Job {}: Failed to write output: {}",
                                        slurm_job_id, e
                                    ));
                                } else {
                                    info!("Saved sacct output to {}", output_file.display());
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to parse sacct JSON for job {}: {}", slurm_job_id, e);
                            errors
                                .push(format!("Job {}: Invalid JSON output: {}", slurm_job_id, e));
                        }
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    error!("sacct command failed for job {}: {}", slurm_job_id, stderr);
                    errors.push(format!("Job {}: sacct failed: {}", slurm_job_id, stderr));
                }
            }
            Err(e) => {
                error!("Failed to run sacct for job {}: {}", slurm_job_id, e);
                errors.push(format!(
                    "Job {}: Failed to execute sacct: {}",
                    slurm_job_id, e
                ));
            }
        }
    }

    // Output results
    if format == "json" {
        let output = serde_json::json!({
            "workflow_id": workflow_id,
            "total_slurm_jobs": nodes.len(),
            "summary": all_summary_rows,
            "errors": errors,
        });
        print_json(&output, "Slurm sacct");
    } else if all_summary_rows.is_empty() && errors.is_empty() {
        println!(
            "No sacct data available for workflow {} (checked {} Slurm job(s))",
            workflow_id,
            nodes.len()
        );
    } else {
        println!("Slurm Accounting Summary for Workflow {}\n", workflow_id);

        if !all_summary_rows.is_empty() {
            display_table_with_count(&all_summary_rows, "job steps");
        }

        if !errors.is_empty() {
            println!("\nErrors:");
            for err in &errors {
                println!("  {}", err);
            }
        }

        if save_json {
            println!("\nFull JSON saved to: {}", output_dir.display());
        }
    }
}

/// Handle the generate command - generates Slurm schedulers for a workflow
#[allow(clippy::too_many_arguments)]
fn handle_generate(
    workflow_file: &PathBuf,
    account: Option<&str>,
    profile_name: Option<&str>,
    output: Option<&PathBuf>,
    single_allocation: bool,
    group_by: GroupByStrategy,
    walltime_strategy: WalltimeStrategy,
    walltime_multiplier: f64,
    no_actions: bool,
    force: bool,
    dry_run: bool,
    format: &str,
) {
    // Load HPC config and registry
    let torc_config = TorcConfig::load().unwrap_or_default();
    let registry = create_registry_with_config_public(&torc_config.client.hpc);

    // Get the HPC profile
    let profile = if let Some(n) = profile_name {
        registry.get(n)
    } else {
        registry.detect()
    };

    let profile = match profile {
        Some(p) => p,
        None => {
            if let Some(name) = profile_name {
                eprintln!("Unknown HPC profile: {}", name);
            } else {
                eprintln!("No HPC profile specified and no system detected.");
                eprintln!("Use --profile <name> or run on an HPC system.");
            }
            std::process::exit(1);
        }
    };

    // Parse the workflow spec (supports YAML, JSON, JSON5, and KDL)
    let mut spec: WorkflowSpec = match WorkflowSpec::from_spec_file(workflow_file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to parse workflow file: {}", e);
            std::process::exit(1);
        }
    };

    // Resolve account: CLI option takes precedence, then slurm_defaults
    let resolved_account = if let Some(acct) = account {
        acct.to_string()
    } else if let Some(ref defaults) = spec.slurm_defaults {
        defaults
            .0
            .get("account")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| {
                eprintln!(
                    "Error: No account specified. Use --account or set 'account' in slurm_defaults."
                );
                std::process::exit(1);
            })
    } else {
        eprintln!("Error: No account specified. Use --account or set 'account' in slurm_defaults.");
        std::process::exit(1);
    };

    // Generate schedulers
    let result = match generate_schedulers_for_workflow(
        &mut spec,
        profile,
        &resolved_account,
        single_allocation,
        group_by,
        walltime_strategy,
        walltime_multiplier,
        !no_actions,
        force,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    // In dry run mode, show what would be generated without writing to output
    if dry_run {
        #[derive(Serialize)]
        struct GenerateDryRunResult<'a> {
            dry_run: bool,
            scheduler_count: usize,
            action_count: usize,
            profile_name: &'a str,
            profile_display_name: &'a str,
            slurm_schedulers: &'a Option<Vec<crate::client::workflow_spec::SlurmSchedulerSpec>>,
            actions: &'a Option<Vec<crate::client::workflow_spec::WorkflowActionSpec>>,
            warnings: &'a [String],
        }

        let dry_run_result = GenerateDryRunResult {
            dry_run: true,
            scheduler_count: result.scheduler_count,
            action_count: result.action_count,
            profile_name: &profile.name,
            profile_display_name: &profile.display_name,
            slurm_schedulers: &spec.slurm_schedulers,
            actions: &spec.actions,
            warnings: &result.warnings,
        };

        if format == "json" {
            print_json(&dry_run_result, "dry run result");
        } else {
            println!("[DRY RUN] Would generate the following Slurm schedulers:");
            println!();
            if let Some(schedulers) = &spec.slurm_schedulers {
                for sched in schedulers {
                    println!(
                        "  Scheduler: {} (account: {}, partition: {}, walltime: {}, nodes: {})",
                        sched.name.as_deref().unwrap_or("unnamed"),
                        sched.account,
                        sched.partition.as_deref().unwrap_or("default"),
                        sched.walltime,
                        sched.nodes
                    );
                }
            }
            if !no_actions {
                println!();
                println!(
                    "[DRY RUN] Would add {} workflow action(s)",
                    result.action_count
                );
            }
            println!();
            println!("Profile: {} ({})", profile.display_name, profile.name);

            if !result.warnings.is_empty() {
                println!();
                println!("Warnings:");
                for warning in &result.warnings {
                    println!("  - {}", warning);
                }
            }
        }
        return;
    }

    // Determine output format: use output file extension if provided, otherwise match input format
    let format_ext = if let Some(out_path) = output {
        out_path.extension().and_then(|e| e.to_str())
    } else {
        workflow_file.extension().and_then(|e| e.to_str())
    };

    let output_content = match format_ext {
        Some("json") => serde_json::to_string_pretty(&spec).unwrap(),
        Some("json5") => serde_json::to_string_pretty(&spec).unwrap(), // Output as JSON
        Some("kdl") => spec.to_kdl_str(),
        Some("yaml") | Some("yml") => pretty_print_yaml(&spec),
        _ => serde_json::to_string_pretty(&spec).unwrap(), // Default to JSON
    };

    if let Some(output_path) = output {
        match std::fs::write(output_path, &output_content) {
            Ok(_) => {
                if format != "json" {
                    println!("Generated workflow written to: {}", output_path.display());
                    println!();
                    println!("Summary:");
                    println!("  Schedulers generated: {}", result.scheduler_count);
                    println!("  Actions added: {}", result.action_count);
                    println!(
                        "  Profile used: {} ({})",
                        profile.display_name, profile.name
                    );

                    if !result.warnings.is_empty() {
                        println!();
                        println!("Warnings:");
                        for warning in &result.warnings {
                            println!("  - {}", warning);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to write output file: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        // Print to stdout
        if format == "json" {
            print_json(&spec, "workflow spec");
        } else {
            println!("{}", output_content);

            // Print summary to stderr so it doesn't mix with the workflow output
            // Use // for KDL-compatible comments
            eprintln!();
            eprintln!("// Summary:");
            eprintln!("//   Schedulers generated: {}", result.scheduler_count);
            eprintln!("//   Actions added: {}", result.action_count);
            eprintln!(
                "//   Profile used: {} ({})",
                profile.display_name, profile.name
            );

            if !result.warnings.is_empty() {
                eprintln!("//");
                eprintln!("// Warnings:");
                for warning in &result.warnings {
                    eprintln!("//   - {}", warning);
                }
            }
        }
    }
}

/// Pretty-print a WorkflowSpec as YAML with blank lines between top-level sections
fn pretty_print_yaml(spec: &WorkflowSpec) -> String {
    let yaml = serde_yaml::to_string(spec).unwrap();
    let mut result = String::new();
    let mut prev_was_section_start = false;

    for line in yaml.lines() {
        // Check if this is a top-level key (must contain colon and not be indented/list/marker/comment)
        let trimmed = line.trim_start();
        let is_top_level = if trimmed.is_empty() {
            false
        } else if line.starts_with(' ') || line.starts_with('-') {
            // Indented content or list items are not top-level keys
            false
        } else if trimmed.starts_with("---")
            || trimmed.starts_with("...")
            || trimmed.starts_with('#')
        {
            // YAML document markers and comments are not top-level sections
            false
        } else {
            // A top-level key must contain a colon (either "key:" or "key: value")
            trimmed.contains(':')
        };

        // Add blank line before top-level sections (except the first one)
        if is_top_level && !result.is_empty() && !prev_was_section_start {
            result.push('\n');
        }

        result.push_str(line);
        result.push('\n');

        prev_was_section_start = is_top_level;
    }

    result
}

/// Result of regenerating schedulers for an existing workflow
#[derive(Debug, Serialize, Deserialize)]
pub struct RegenerateResult {
    pub workflow_id: i64,
    pub pending_jobs: usize,
    pub schedulers_created: Vec<SchedulerInfo>,
    pub total_allocations: i64,
    /// Number of allocations actually submitted immediately
    pub allocations_submitted: i64,
    /// Number of allocations deferred (will be submitted via on_jobs_ready action)
    pub allocations_deferred: i64,
    pub warnings: Vec<String>,
    pub submitted: bool,
}

/// Information about a planned scheduler (for dry run output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedSchedulerInfo {
    pub name: String,
    pub account: String,
    pub partition: Option<String>,
    pub walltime: String,
    pub mem: Option<String>,
    pub nodes: i64,
    pub num_allocations: i64,
    pub job_count: usize,
    pub job_names: Vec<String>,
    pub has_dependencies: bool,
}

/// Dry run result for regenerate command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegenerateDryRunResult {
    pub dry_run: bool,
    pub workflow_id: i64,
    pub pending_jobs: usize,
    pub profile_name: String,
    pub profile_display_name: String,
    pub planned_schedulers: Vec<PlannedSchedulerInfo>,
    pub total_allocations: i64,
    pub would_submit: bool,
    pub warnings: Vec<String>,
}

/// Information about a created scheduler
#[derive(Debug, Serialize, Deserialize)]
pub struct SchedulerInfo {
    pub id: i64,
    pub name: String,
    pub account: String,
    pub partition: Option<String>,
    pub walltime: String,
    pub nodes: i64,
    pub num_allocations: i64,
    pub job_count: usize,
    /// Whether the jobs using this scheduler have dependencies on other pending jobs.
    /// If true, allocations should not be submitted immediately - they will be
    /// submitted when the on_jobs_ready action fires.
    pub has_dependencies: bool,
}

/// Handle the regenerate command - regenerates Slurm schedulers for pending jobs
#[allow(clippy::too_many_arguments)]
fn handle_regenerate(
    config: &Configuration,
    workflow_id: i64,
    account: Option<&str>,
    profile_name: Option<&str>,
    single_allocation: bool,
    group_by: GroupByStrategy,
    walltime_strategy: WalltimeStrategy,
    walltime_multiplier: f64,
    submit: bool,
    output_dir: &PathBuf,
    poll_interval: i32,
    dry_run: bool,
    include_job_ids: Option<&[i64]>,
    format: &str,
) {
    // Load HPC config and registry
    let torc_config = TorcConfig::load().unwrap_or_default();
    let registry = create_registry_with_config_public(&torc_config.client.hpc);

    // Get the HPC profile
    let profile = if let Some(n) = profile_name {
        registry.get(n)
    } else {
        registry.detect()
    };

    let profile = match profile {
        Some(p) => p,
        None => {
            if let Some(name) = profile_name {
                eprintln!("Unknown HPC profile: {}", name);
            } else {
                eprintln!("No HPC profile specified and no system detected.");
                eprintln!("Use --profile <name> or run on an HPC system.");
            }
            std::process::exit(1);
        }
    };

    // Fetch pending jobs (uninitialized, ready, blocked)
    let pending_statuses = [
        models::JobStatus::Uninitialized,
        models::JobStatus::Ready,
        models::JobStatus::Blocked,
    ];
    let mut pending_jobs: Vec<models::JobModel> = Vec::new();

    for status in &pending_statuses {
        match paginate_jobs(
            config,
            workflow_id,
            JobListParams::new()
                .with_status(*status)
                .with_include_relationships(true),
        ) {
            Ok(jobs) => {
                pending_jobs.extend(jobs);
            }
            Err(e) => {
                print_error(&format!("listing {:?} jobs", status), &e);
                std::process::exit(1);
            }
        }
    }

    // Include additional job IDs (e.g., failed jobs for recovery dry-run)
    if let Some(job_ids) = include_job_ids {
        let existing_ids: std::collections::HashSet<i64> =
            pending_jobs.iter().filter_map(|j| j.id).collect();

        for &job_id in job_ids {
            if !existing_ids.contains(&job_id) {
                match default_api::get_job(config, job_id) {
                    Ok(job) => {
                        pending_jobs.push(job);
                    }
                    Err(e) => {
                        debug!("Could not fetch job {}: {:?}", job_id, e);
                    }
                }
            }
        }
    }

    if pending_jobs.is_empty() {
        if format == "json" {
            if dry_run {
                print_json(
                    &RegenerateDryRunResult {
                        dry_run: true,
                        workflow_id,
                        pending_jobs: 0,
                        profile_name: profile.name.clone(),
                        profile_display_name: profile.display_name.clone(),
                        planned_schedulers: Vec::new(),
                        total_allocations: 0,
                        would_submit: submit,
                        warnings: vec!["No pending jobs found".to_string()],
                    },
                    "dry run result",
                );
            } else {
                print_json(
                    &RegenerateResult {
                        workflow_id,
                        pending_jobs: 0,
                        schedulers_created: Vec::new(),
                        total_allocations: 0,
                        allocations_submitted: 0,
                        allocations_deferred: 0,
                        warnings: vec!["No pending jobs found".to_string()],
                        submitted: false,
                    },
                    "regenerate result",
                );
            }
        } else {
            println!(
                "No pending jobs (uninitialized, ready, or blocked) found in workflow {}",
                workflow_id
            );
        }
        return;
    }

    let mut warnings: Vec<String> = Vec::new();

    // Mark existing schedule_nodes actions as executed to prevent duplicate allocations
    // This is critical for recovery scenarios where original actions would otherwise fire again
    match utils::send_with_retries(
        config,
        || default_api::get_workflow_actions(config, workflow_id),
        WAIT_FOR_HEALTHY_DATABASE_MINUTES,
    ) {
        Ok(actions) => {
            for action in actions {
                // Only mark non-recovery, unexecuted schedule_nodes actions
                if action.action_type == "schedule_nodes"
                    && !action.is_recovery
                    && !action.executed
                    && let Some(action_id) = action.id
                {
                    match utils::send_with_retries(
                        config,
                        || {
                            default_api::claim_action(
                                config,
                                workflow_id,
                                action_id,
                                serde_json::json!({}),
                            )
                        },
                        WAIT_FOR_HEALTHY_DATABASE_MINUTES,
                    ) {
                        Ok(_) => {
                            info!(
                                "Marked action {} ({} -> schedule_nodes) as executed for recovery",
                                action_id, action.trigger_type
                            );
                        }
                        Err(e) => {
                            // 409 Conflict means already claimed, which is fine
                            if !format!("{:?}", e).contains("409") {
                                warnings.push(format!(
                                    "Failed to mark action {} as executed: {:?}",
                                    action_id, e
                                ));
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            // Non-fatal: we can still proceed with regeneration
            warnings.push(format!("Failed to fetch workflow actions: {:?}", e));
        }
    }

    // Fetch all resource requirements for the workflow
    let resource_requirements = match paginate_resource_requirements(
        config,
        workflow_id,
        ResourceRequirementsListParams::new(),
    ) {
        Ok(rrs) => rrs,
        Err(e) => {
            print_error("listing resource requirements", &e);
            std::process::exit(1);
        }
    };

    // Get existing schedulers to use as defaults
    let existing_schedulers =
        match paginate_slurm_schedulers(config, workflow_id, SlurmSchedulersListParams::new()) {
            Ok(schedulers) => schedulers,
            Err(e) => {
                print_error("listing existing schedulers", &e);
                std::process::exit(1);
            }
        };

    // Determine account to use
    let account_to_use = account
        .map(|s| s.to_string())
        .or_else(|| existing_schedulers.first().map(|s| s.account.clone()))
        .unwrap_or_else(|| {
            eprintln!("No account specified and no existing schedulers found.");
            eprintln!("Use --account <account> to specify a Slurm account.");
            std::process::exit(1);
        });

    use crate::client::scheduler_plan::generate_scheduler_plan;

    // Build WorkflowGraph from pending jobs for proper dependency-aware grouping
    // This aligns with create-slurm's behavior of separating jobs by (rr, has_dependencies)
    let graph = match WorkflowGraph::from_jobs(&pending_jobs, &resource_requirements) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Failed to build workflow graph: {}", e);
            std::process::exit(1);
        }
    };

    // Warn about jobs without resource requirements
    for job in &pending_jobs {
        if job.resource_requirements_id.is_none() {
            warnings.push(format!(
                "Job '{}' (ID: {}) has no resource requirements, skipping",
                job.name,
                job.id.unwrap_or(-1)
            ));
        }
    }

    // Build a map of RR name -> RR model for lookups
    let rr_name_to_model: HashMap<&str, &models::ResourceRequirementsModel> = resource_requirements
        .iter()
        .map(|rr| (rr.name.as_str(), rr))
        .collect();

    // Generate scheduler plan using shared logic
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let plan = generate_scheduler_plan(
        &graph,
        &rr_name_to_model,
        profile,
        &account_to_use,
        single_allocation,
        group_by,
        walltime_strategy,
        walltime_multiplier,
        true, // add_actions (we'll create them as recovery actions)
        Some(&format!("regen_{}", timestamp)),
        true, // is_recovery
    );

    // Combine warnings from planning
    warnings.extend(plan.warnings.clone());

    if plan.schedulers.is_empty() {
        if format == "json" {
            if dry_run {
                print_json(
                    &RegenerateDryRunResult {
                        dry_run: true,
                        workflow_id,
                        pending_jobs: pending_jobs.len(),
                        profile_name: profile.name.clone(),
                        profile_display_name: profile.display_name.clone(),
                        planned_schedulers: Vec::new(),
                        total_allocations: 0,
                        would_submit: submit,
                        warnings: warnings.clone(),
                    },
                    "dry run result",
                );
            } else {
                print_json(
                    &RegenerateResult {
                        workflow_id,
                        pending_jobs: pending_jobs.len(),
                        schedulers_created: Vec::new(),
                        total_allocations: 0,
                        allocations_submitted: 0,
                        allocations_deferred: 0,
                        warnings,
                        submitted: false,
                    },
                    "regenerate result",
                );
            }
        } else {
            println!("No pending jobs with resource requirements found");
            for warning in &warnings {
                println!("  Warning: {}", warning);
            }
        }
        return;
    }

    // In dry run mode, show what would be created without making changes
    if dry_run {
        // Build planned scheduler info without IDs (since nothing is created)
        let planned_schedulers: Vec<PlannedSchedulerInfo> = plan
            .schedulers
            .iter()
            .map(|p| PlannedSchedulerInfo {
                name: p.name.clone(),
                account: p.account.clone(),
                partition: p.partition.clone(),
                walltime: p.walltime.clone(),
                mem: p.mem.clone(),
                nodes: p.nodes,
                num_allocations: p.num_allocations,
                job_count: p.job_count,
                job_names: p.job_names.clone(),
                has_dependencies: p.has_dependencies,
            })
            .collect();

        let total_allocations: i64 = plan.schedulers.iter().map(|p| p.num_allocations).sum();

        let dry_run_result = RegenerateDryRunResult {
            dry_run: true,
            workflow_id,
            pending_jobs: pending_jobs.len(),
            profile_name: profile.name.clone(),
            profile_display_name: profile.display_name.clone(),
            planned_schedulers,
            total_allocations,
            would_submit: submit,
            warnings: warnings.clone(),
        };

        if format == "json" {
            print_json(&dry_run_result, "dry run result");
        } else {
            println!("[DRY RUN] Would create the following Slurm schedulers:");
            println!();
            for sched in &dry_run_result.planned_schedulers {
                let deps = if sched.has_dependencies {
                    " (deferred - has dependencies)"
                } else {
                    ""
                };
                println!(
                    "  {} - {} job(s), {} allocation(s){}",
                    sched.name, sched.job_count, sched.num_allocations, deps
                );
                println!(
                    "    Account: {}, Partition: {}, Walltime: {}, Nodes: {}",
                    sched.account,
                    sched.partition.as_deref().unwrap_or("default"),
                    sched.walltime,
                    sched.nodes
                );
                if let Some(mem) = &sched.mem {
                    println!("    Memory: {}", mem);
                }
            }
            println!();
            println!("Total allocations: {}", dry_run_result.total_allocations);
            if submit {
                println!("[DRY RUN] Would submit allocations immediately");
            }
            println!("Profile: {} ({})", profile.display_name, profile.name);

            if !warnings.is_empty() {
                println!();
                println!("Warnings:");
                for warning in &warnings {
                    println!("  - {}", warning);
                }
            }
        }
        return;
    }

    // Apply plan to database: create schedulers and track IDs
    let mut schedulers_created: Vec<SchedulerInfo> = Vec::new();
    let mut total_allocations: i64 = 0;
    // Maps scheduler name -> (scheduler_id, start_one_worker_per_node)
    let mut scheduler_name_to_info: HashMap<String, (i64, bool)> = HashMap::new();

    for planned in &plan.schedulers {
        // Create the scheduler in the database
        let scheduler = models::SlurmSchedulerModel {
            id: None,
            workflow_id,
            name: Some(planned.name.clone()),
            account: planned.account.clone(),
            partition: planned.partition.clone(),
            mem: planned.mem.clone(),
            walltime: planned.walltime.clone(),
            nodes: planned.nodes,
            gres: planned.gres.clone(),
            ntasks_per_node: None,
            qos: planned.qos.clone(),
            tmp: None,
            extra: None,
        };

        let created_scheduler = match utils::send_with_retries(
            config,
            || default_api::create_slurm_scheduler(config, scheduler.clone()),
            WAIT_FOR_HEALTHY_DATABASE_MINUTES,
        ) {
            Ok(s) => s,
            Err(e) => {
                print_error("creating scheduler", &e);
                std::process::exit(1);
            }
        };

        let scheduler_id = created_scheduler.id.unwrap_or(-1);
        let start_one_worker_per_node = planned.nodes > 1;

        scheduler_name_to_info.insert(
            planned.name.clone(),
            (scheduler_id, start_one_worker_per_node),
        );

        schedulers_created.push(SchedulerInfo {
            id: scheduler_id,
            name: planned.name.clone(),
            account: planned.account.clone(),
            partition: created_scheduler.partition.clone(),
            walltime: created_scheduler.walltime.clone(),
            nodes: planned.nodes,
            num_allocations: planned.num_allocations,
            job_count: planned.job_count,
            has_dependencies: planned.has_dependencies,
        });

        total_allocations += planned.num_allocations;

        // Update jobs in this group to reference this scheduler
        for job_name in &planned.job_names {
            if let Some(job) = pending_jobs.iter().find(|j| &j.name == job_name)
                && let Some(job_id) = job.id
            {
                let mut updated_job = job.clone();
                updated_job.scheduler_id = Some(scheduler_id);
                // Clear status so server ignores it during comparison
                updated_job.status = None;
                if let Err(e) = utils::send_with_retries(
                    config,
                    || default_api::update_job(config, job_id, updated_job.clone()),
                    WAIT_FOR_HEALTHY_DATABASE_MINUTES,
                ) {
                    warnings.push(format!(
                        "Failed to update job {} with scheduler: {}",
                        job_id, e
                    ));
                }
            }
        }
    }

    // Create recovery actions for deferred groups (from planned actions with is_recovery=true)
    for action in &plan.actions {
        if !action.is_recovery {
            continue; // Skip non-recovery actions
        }

        let (scheduler_id, start_one_worker_per_node) =
            match scheduler_name_to_info.get(&action.scheduler_name) {
                Some(info) => *info,
                None => continue,
            };

        // Get job IDs for this action's jobs
        // Prefer exact job_names over job_name_patterns (regexes) when available
        let job_ids: Vec<i64> = if let Some(ref names) = action.job_names {
            // Use exact name matching
            pending_jobs
                .iter()
                .filter(|j| names.contains(&j.name))
                .filter_map(|j| j.id)
                .collect()
        } else if let Some(ref patterns) = action.job_name_patterns {
            // Fall back to regex patterns
            pending_jobs
                .iter()
                .filter(|j| {
                    patterns.iter().any(|p| {
                        regex::Regex::new(p)
                            .map(|re| re.is_match(&j.name))
                            .unwrap_or(false)
                    })
                })
                .filter_map(|j| j.id)
                .collect()
        } else {
            Vec::new()
        };

        if job_ids.is_empty() {
            continue;
        }

        let action_config = serde_json::json!({
            "scheduler_type": "slurm",
            "scheduler_id": scheduler_id,
            "num_allocations": action.num_allocations,
            "start_one_worker_per_node": start_one_worker_per_node,
        });

        let action_body = serde_json::json!({
            "workflow_id": workflow_id,
            "trigger_type": "on_jobs_ready",
            "action_type": "schedule_nodes",
            "action_config": action_config,
            "job_ids": job_ids,
            "persistent": false,
            "is_recovery": true,
        });

        match utils::send_with_retries(
            config,
            || default_api::create_workflow_action(config, workflow_id, action_body.clone()),
            WAIT_FOR_HEALTHY_DATABASE_MINUTES,
        ) {
            Ok(created_action) => {
                info!(
                    "Created recovery action {} for {} deferred jobs using scheduler {}",
                    created_action.id.unwrap_or(-1),
                    job_ids.len(),
                    scheduler_id
                );
            }
            Err(e) => {
                warnings.push(format!(
                    "Failed to create recovery action for scheduler {}: {:?}",
                    scheduler_id, e
                ));
            }
        }
    }

    // Submit allocations if requested
    // Only submit allocations for schedulers without dependencies.
    // Schedulers for jobs with dependencies will be submitted when the
    // on_jobs_ready action fires (after their dependencies complete).
    let mut submitted = false;
    let mut allocations_submitted: i64 = 0;
    let mut allocations_deferred: i64 = 0;

    if submit && !schedulers_created.is_empty() {
        // Create output directory
        if let Err(e) = std::fs::create_dir_all(output_dir) {
            eprintln!("Error creating output directory: {}", e);
            std::process::exit(1);
        }

        for scheduler_info in &schedulers_created {
            // Skip schedulers for jobs with dependencies - they will be submitted
            // when their on_jobs_ready action fires
            if scheduler_info.has_dependencies {
                println!(
                    "  Deferring scheduler '{}' ({} allocation(s)) - will submit via on_jobs_ready action",
                    scheduler_info.name, scheduler_info.num_allocations
                );
                allocations_deferred += scheduler_info.num_allocations;
                continue;
            }

            let start_one_worker_per_node = scheduler_info.nodes > 1;

            match schedule_slurm_nodes(
                config,
                workflow_id,
                scheduler_info.id,
                scheduler_info.num_allocations as i32,
                "worker",
                output_dir.to_str().unwrap_or("output"),
                poll_interval,
                None, // max_parallel_jobs
                start_one_worker_per_node,
                false, // keep_submission_scripts
            ) {
                Ok(()) => {
                    println!(
                        "  Submitted {} allocation(s) for scheduler '{}'",
                        scheduler_info.num_allocations, scheduler_info.name
                    );
                    allocations_submitted += scheduler_info.num_allocations;
                }
                Err(e) => {
                    eprintln!(
                        "Error submitting allocations for scheduler '{}': {}",
                        scheduler_info.name, e
                    );
                    std::process::exit(1);
                }
            }
        }
        submitted = true;
    }

    // Output results
    let result = RegenerateResult {
        workflow_id,
        pending_jobs: pending_jobs.len(),
        schedulers_created,
        total_allocations,
        allocations_submitted,
        allocations_deferred,
        warnings,
        submitted,
    };

    if format == "json" {
        print_json(&result, "regenerate result");
    } else {
        println!("Regenerated Slurm schedulers for workflow {}", workflow_id);
        println!();
        println!("Summary:");
        println!("  Pending jobs: {}", result.pending_jobs);
        println!("  Schedulers created: {}", result.schedulers_created.len());
        if result.submitted {
            println!(
                "  Allocations submitted: {} (deferred: {})",
                result.allocations_submitted, result.allocations_deferred
            );
        } else {
            println!("  Total allocations: {}", result.total_allocations);
        }
        println!(
            "  Profile used: {} ({})",
            profile.display_name, profile.name
        );

        if !result.schedulers_created.is_empty() {
            println!();
            println!("Schedulers:");
            for sched in &result.schedulers_created {
                let deferred_marker = if sched.has_dependencies {
                    " [deferred]"
                } else {
                    ""
                };
                println!(
                    "  - {} (ID: {}): {} job(s), {} allocation(s) × {} node(s){}",
                    sched.name,
                    sched.id,
                    sched.job_count,
                    sched.num_allocations,
                    sched.nodes,
                    deferred_marker
                );
            }
        }

        if !result.warnings.is_empty() {
            println!();
            println!("Warnings:");
            for warning in &result.warnings {
                println!("  - {}", warning);
            }
        }

        if result.submitted && result.allocations_submitted > 0 {
            println!();
            if result.allocations_deferred > 0 {
                println!(
                    "Submitted {} allocation(s). {} deferred allocation(s) will be submitted when dependencies complete.",
                    result.allocations_submitted, result.allocations_deferred
                );
            } else {
                println!("Allocations submitted successfully.");
            }
        } else if !result.schedulers_created.is_empty() {
            println!();
            println!("To submit the allocations, run:");
            println!("  torc slurm regenerate {} --submit", workflow_id);
        }
    }
}
