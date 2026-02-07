use std::fs;
use std::io::{self, Read, Write};

use clap::Subcommand;

const WORKFLOWS_HELP_TEMPLATE: &str = "\
{before-help}{about-with-newline}
{usage-heading} {usage}

{all-args}

\x1b[1;32mWorkflow Creation:\x1b[0m
  \x1b[1;36mcreate\x1b[0m           Create a workflow from a specification file
  \x1b[1;36mcreate-slurm\x1b[0m     Create with auto-generated Slurm schedulers
  \x1b[1;36mnew\x1b[0m              Create a new empty workflow

\x1b[1;32mWorkflow Lifecycle:\x1b[0m
  \x1b[1;36msubmit\x1b[0m           Submit a workflow to scheduler
  \x1b[1;36mrun\x1b[0m              Run a workflow locally
  \x1b[1;36minitialize\x1b[0m       Initialize workflow dependencies
  \x1b[1;36mreinitialize\x1b[0m     Reinitialize jobs with changed inputs
  \x1b[1;36mcancel\x1b[0m           Cancel a workflow and Slurm jobs

\x1b[1;32mWorkflow State:\x1b[0m
  \x1b[1;36mstatus\x1b[0m           Get workflow status
  \x1b[1;36mreset-status\x1b[0m     Reset workflow and job statuses
  \x1b[1;36mis-complete\x1b[0m      Check if workflow is complete
  \x1b[1;36msync-status\x1b[0m      Sync job statuses with Slurm

\x1b[1;32mListing & Query:\x1b[0m
  \x1b[1;36mlist\x1b[0m             List workflows
  \x1b[1;36mget\x1b[0m              Get a specific workflow
  \x1b[1;36mexecution-plan\x1b[0m   Show execution plan
  \x1b[1;36mlist-actions\x1b[0m     List workflow actions

\x1b[1;32mWorkflow Maintenance:\x1b[0m
  \x1b[1;36mupdate\x1b[0m           Update workflow properties
  \x1b[1;36mdelete\x1b[0m           Delete one or more workflows
  \x1b[1;36marchive\x1b[0m          Archive or unarchive workflows

\x1b[1;32mImport & Export:\x1b[0m
  \x1b[1;36mexport\x1b[0m           Export a workflow to JSON
  \x1b[1;36mimport\x1b[0m           Import a workflow from JSON
{after-help}";

use crate::client::apis::configuration::Configuration;
use crate::client::apis::default_api;
use crate::client::commands::hpc::create_registry_with_config_public;
use crate::client::commands::pagination::{
    EventListParams, FileListParams, JobListParams, ResourceRequirementsListParams,
    ResultListParams, ScheduledComputeNodeListParams, SlurmSchedulersListParams,
    UserDataListParams, WorkflowListParams, paginate_events, paginate_files, paginate_jobs,
    paginate_resource_requirements, paginate_results, paginate_scheduled_compute_nodes,
    paginate_slurm_schedulers, paginate_user_data, paginate_workflows,
};
use crate::client::commands::slurm::{
    GroupByStrategy, WalltimeStrategy, generate_schedulers_for_workflow,
};
use crate::client::commands::workflow_export::{
    EXPORT_VERSION, ExportImportStats, IdMappings, WorkflowExport,
};
use crate::client::commands::{
    get_env_user_name, output::print_json_wrapped, print_error, select_workflow_interactively,
    table_format::display_table_with_count,
};
use crate::client::hpc::hpc_interface::HpcInterface;
use crate::client::workflow_manager::WorkflowManager;
use crate::client::workflow_spec::WorkflowSpec;
use crate::config::TorcConfig;
use crate::memory_utils::memory_string_to_bytes;
use crate::models;
use crate::models::JobStatus;
use crate::time_utils::duration_string_to_seconds;
use serde_json;
use tabled::Tabled;

#[derive(Tabled)]
struct WorkflowTableRowNoUser {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Description")]
    description: String,
    #[tabled(rename = "Project")]
    project: String,
    #[tabled(rename = "Metadata")]
    metadata: String,
    #[tabled(rename = "Timestamp")]
    timestamp: String,
}

#[derive(Tabled)]
struct WorkflowTableRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "User")]
    user: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Description")]
    description: String,
    #[tabled(rename = "Project")]
    project: String,
    #[tabled(rename = "Metadata")]
    metadata: String,
    #[tabled(rename = "Timestamp")]
    timestamp: String,
}

#[derive(Tabled)]
struct WorkflowActionTableRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Trigger")]
    trigger_type: String,
    #[tabled(rename = "Action")]
    action_type: String,
    #[tabled(rename = "Progress")]
    progress: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Executed At")]
    executed_at: String,
    #[tabled(rename = "Job IDs")]
    job_ids: String,
}

#[derive(Subcommand)]
#[command(
    help_template = WORKFLOWS_HELP_TEMPLATE,
    subcommand_help_heading = None,
    after_long_help = "\
EXAMPLES:
    # Create a workflow from a YAML spec file
    torc workflows create workflow.yaml

    # Create from JSON5 with comments
    torc workflows create config.json5

    # Get JSON output for automation
    torc -f json workflows create workflow.yaml

    # Validate without creating (dry-run)
    torc workflows create --dry-run workflow.yaml
")]
pub enum WorkflowCommands {
    /// Create a workflow from a specification file (supports JSON, JSON5, YAML, and KDL formats)
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # Create workflow from YAML
    torc workflows create my_workflow.yaml

    # Validate spec before creating
    torc workflows create --dry-run my_workflow.yaml

    # Get JSON output with workflow ID
    torc -f json workflows create my_workflow.yaml
"
    )]
    Create {
        /// Path to specification file containing WorkflowSpec
        ///
        /// Supported formats:
        /// - JSON (.json): Standard JSON format
        /// - JSON5 (.json5): JSON with comments and trailing commas
        /// - YAML (.yaml, .yml): Human-readable YAML format
        /// - KDL (.kdl): KDL document format
        ///
        /// Format is auto-detected from file extension, with fallback parsing attempted
        #[arg()]
        file: String,
        /// Disable resource monitoring (default: enabled with summary granularity and 5s sample rate)
        #[arg(long, default_value = "false")]
        no_resource_monitoring: bool,
        /// Skip validation checks (e.g., scheduler node requirements). Use with caution.
        #[arg(long, default_value = "false")]
        skip_checks: bool,
        /// Validate the workflow specification without creating it (dry-run mode)
        /// Returns a summary of what would be created including job count after parameter expansion
        #[arg(long)]
        dry_run: bool,
    },
    /// Create a workflow with auto-generated Slurm schedulers
    ///
    /// Automatically generates Slurm schedulers based on job resource requirements
    /// and HPC profile. For Slurm workflows without pre-configured schedulers.
    #[command(
        hide = true,
        name = "create-slurm",
        after_long_help = "\
EXAMPLES:
    # Create with auto-generated Slurm schedulers
    torc workflows create-slurm --account myproject workflow.yaml

    # Specify HPC profile explicitly
    torc workflows create-slurm --account myproject --hpc-profile kestrel workflow.yaml

    # Use single allocation mode (1xN instead of Nx1)
    torc workflows create-slurm --account myproject --single-allocation workflow.yaml
"
    )]
    CreateSlurm {
        /// Path to specification file containing WorkflowSpec
        #[arg()]
        file: String,
        /// Slurm account to use for allocations (can also be specified in workflow's slurm_defaults)
        #[arg(short, long)]
        account: Option<String>,
        /// HPC profile to use (auto-detected if not specified)
        #[arg(long)]
        hpc_profile: Option<String>,
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
        /// Disable resource monitoring (default: enabled with summary granularity and 5s sample rate)
        #[arg(long, default_value = "false")]
        no_resource_monitoring: bool,
        /// Skip validation checks (e.g., scheduler node requirements). Use with caution.
        #[arg(long, default_value = "false")]
        skip_checks: bool,
        /// Validate the workflow specification without creating it (dry-run mode)
        #[arg(long)]
        dry_run: bool,
    },
    /// Create a new empty workflow
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # Create an empty workflow
    torc workflows new --name my_workflow

    # Create with description
    torc workflows new --name my_workflow --description 'Data processing pipeline'
"
    )]
    New {
        /// Name of the workflow
        #[arg(short, long)]
        name: String,
        /// Description of the workflow
        #[arg(short, long)]
        description: Option<String>,
    },
    /// List workflows
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # List all workflows for current user
    torc workflows list

    # Get JSON output for scripting
    torc -f json workflows list

    # Paginate results
    torc workflows list --limit 50 --offset 100

    # Sort by creation time (newest first)
    torc workflows list --sort-by timestamp --reverse-sort

    # Show archived workflows
    torc workflows list --archived-only
    torc workflows list --include-archived

    # Show workflows from all users
    torc workflows list --all-users
"
    )]
    List {
        /// Maximum number of workflows to return
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
        /// Show only archived workflows
        #[arg(long, default_value = "false")]
        archived_only: bool,
        /// Include both archived and non-archived workflows
        #[arg(long, default_value = "false")]
        include_archived: bool,
        /// Show workflows from all users (filtered by access when authentication is enabled)
        #[arg(long, default_value = "false")]
        all_users: bool,
    },
    /// Get a specific workflow by ID
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # Get workflow details
    torc workflows get 123

    # Get as JSON for automation
    torc -f json workflows get 123
"
    )]
    Get {
        /// ID of the workflow to get (optional - will prompt if not provided)
        #[arg()]
        id: Option<i64>,
    },
    /// Update an existing workflow
    #[command(
        hide = true,
        after_long_help = r#"EXAMPLES:
    # Update workflow name
    torc workflows update 123 --name 'New Name'

    # Update description
    torc workflows update 123 --description 'Updated description'

    # Transfer ownership
    torc workflows update 123 --owner-user newuser

    # Update project
    torc workflows update 123 --project my-project

    # Update metadata (pass JSON as string; use single quotes in shell)
    torc workflows update 123 --metadata '{"key":"value","stage":"production"}'
"#
    )]
    Update {
        /// ID of the workflow to update (optional - will prompt if not provided)
        #[arg()]
        id: Option<i64>,
        /// Name of the workflow
        #[arg(short, long)]
        name: Option<String>,
        /// Description of the workflow
        #[arg(short, long)]
        description: Option<String>,
        /// User that owns the workflow
        #[arg(long)]
        owner_user: Option<String>,
        /// Project name or identifier
        #[arg(long)]
        project: Option<String>,
        /// Metadata as JSON string
        #[arg(long)]
        metadata: Option<String>,
    },
    /// Cancel a workflow and all associated Slurm jobs. All state will be preserved and the
    /// workflow can be resumed after it is reinitialized.
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # Cancel a workflow and its Slurm jobs
    torc workflows cancel 123

    # Get JSON status of cancellation
    torc -f json workflows cancel 123
"
    )]
    Cancel {
        /// ID of the workflow to cancel (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
    },
    /// Delete one or more workflows
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # Delete a single workflow (with confirmation)
    torc workflows delete 123

    # Delete multiple workflows
    torc workflows delete 123 456 789

    # Delete without confirmation prompt
    torc workflows delete 123 --no-prompts
"
    )]
    Delete {
        /// IDs of workflows to remove (optional - will prompt if not provided)
        #[arg()]
        ids: Vec<i64>,
        /// Skip confirmation prompt
        #[arg(long)]
        no_prompts: bool,
    },
    /// Archive or unarchive one or more workflows
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # Archive workflows
    torc workflows archive true 123 456

    # Unarchive workflows
    torc workflows archive false 123
"
    )]
    Archive {
        /// Set to true to archive, false to unarchive
        #[arg()]
        is_archived: String,
        /// IDs of workflows to archive/unarchive (if empty, will prompt for selection)
        #[arg()]
        workflow_ids: Vec<i64>,
    },
    /// Submit a workflow: initialize if needed and schedule nodes for on_workflow_start actions
    /// This command requires the workflow to have an on_workflow_start action with schedule_nodes
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # Submit workflow to scheduler
    torc workflows submit 123

    # Submit even with missing user data
    torc workflows submit 123 --force
"
    )]
    Submit {
        /// ID of the workflow to submit (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// If false, fail the operation if missing data is present (defaults to false)
        #[arg(long, default_value = "false")]
        force: bool,
    },
    /// Run a workflow locally on the current node
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # Run workflow locally
    torc workflows run 123

    # Run with custom settings
    torc workflows run 123 --poll-interval 10 --max-parallel-jobs 4

    # Specify output directory
    torc workflows run 123 --output-dir /path/to/output
"
    )]
    Run {
        /// ID of the workflow to run (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Poll interval in seconds for checking job completion
        #[arg(short, long, default_value = "5.0")]
        poll_interval: f64,
        /// Maximum number of parallel jobs to run (defaults to available CPUs)
        #[arg(long)]
        max_parallel_jobs: Option<i64>,
        /// Output directory for job logs and results
        #[arg(long, default_value = "output")]
        output_dir: std::path::PathBuf,
    },
    /// Initialize a workflow, including all job statuses.
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # Initialize workflow (set up dependencies)
    torc workflows initialize 123

    # Dry-run to check for missing files
    torc workflows initialize 123 --dry-run

    # Force initialization with missing data
    torc workflows initialize 123 --force
"
    )]
    Initialize {
        /// ID of the workflow to start (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// If false, fail the operation if missing data is present (defaults to false)
        #[arg(long, default_value = "false")]
        force: bool,
        /// Skip confirmation prompt
        #[arg(long)]
        no_prompts: bool,
        /// Perform a dry run without making changes
        #[arg(long)]
        dry_run: bool,
    },
    /// Reinitialize a workflow. This will reinitialize all jobs with a status of
    /// canceled, submitting, pending, or terminated. Jobs with a status of
    /// done will also be reinitialized if an input_file or user_data record has
    /// changed.
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # Reinitialize workflow after input changes
    torc workflows reinitialize 123

    # Dry-run to preview changes
    torc workflows reinitialize 123 --dry-run
"
    )]
    Reinitialize {
        /// ID of the workflow to reinitialize (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// If false, fail the operation if missing data is present (defaults to false)
        #[arg(long, default_value = "false")]
        force: bool,
        /// Perform a dry run without making changes
        #[arg(long)]
        dry_run: bool,
    },
    /// Get workflow status
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # Get workflow status
    torc workflows status 123

    # Get JSON status for scripting
    torc -f json workflows status 123
"
    )]
    Status {
        /// ID of the workflow to get status for (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
    },
    /// Reset workflow and job status
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # Reset all job statuses
    torc workflows reset-status 123

    # Reset only failed jobs
    torc workflows reset-status 123 --failed-only

    # Reset and reinitialize
    torc workflows reset-status 123 --reinitialize

    # Force reset (ignore running jobs check)
    torc workflows reset-status 123 --force --no-prompts
"
    )]
    ResetStatus {
        /// ID of the workflow to reset status for (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Only reset failed jobs
        #[arg(long, default_value = "false")]
        failed_only: bool,
        /// Reinitialize the workflow after resetting status
        #[arg(short, long, default_value = "false")]
        reinitialize: bool,
        /// Force reset even if there are active jobs (ignores running/pending jobs check)
        #[arg(long, default_value = "false")]
        force: bool,
        /// Skip confirmation prompt
        #[arg(long)]
        no_prompts: bool,
    },
    /// Correct resource requirements based on actual job usage (proactive optimization)
    ///
    /// Analyzes completed jobs and adjusts resource requirements to better match actual usage.
    /// Unlike `torc recover`, this command does NOT reset or rerun jobs - it only updates
    /// resource requirements for future runs.
    #[command(
        name = "correct-resources",
        after_long_help = "\
EXAMPLES:
    # Preview corrections (dry-run)
    torc workflows correct-resources 123 --dry-run

    # Apply corrections to all over-utilized jobs
    torc workflows correct-resources 123

    # Apply corrections only to specific jobs
    torc workflows correct-resources 123 --job-ids 45,67,89

    # Use custom multipliers
    torc workflows correct-resources 123 --memory-multiplier 1.5 --runtime-multiplier 1.4

    # Output as JSON for programmatic use
    torc -f json workflows correct-resources 123 --dry-run
"
    )]
    CorrectResources {
        /// ID of the workflow to analyze (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
        /// Memory multiplier for jobs that exceeded memory (default: 1.2)
        #[arg(long, default_value = "1.2")]
        memory_multiplier: f64,
        /// Runtime multiplier for jobs that exceeded runtime (default: 1.2)
        #[arg(long, default_value = "1.2")]
        runtime_multiplier: f64,
        /// Only correct resource requirements for specific jobs (comma-separated IDs)
        #[arg(long, value_delimiter = ',')]
        job_ids: Option<Vec<i64>>,
        /// Show what would be changed without applying (default: false)
        #[arg(long)]
        dry_run: bool,
    },
    /// Show the execution plan for a workflow specification or existing workflow
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # Show execution plan from spec file
    torc workflows execution-plan workflow.yaml

    # Show execution plan for existing workflow
    torc workflows execution-plan 123

    # Get JSON output
    torc -f json workflows execution-plan workflow.yaml
"
    )]
    ExecutionPlan {
        /// Path to specification file OR workflow ID
        #[arg()]
        spec_or_id: String,
    },
    /// List workflow actions and their statuses (useful for debugging action triggers)
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # List actions for a workflow
    torc workflows list-actions 123

    # Get JSON output
    torc -f json workflows list-actions 123
"
    )]
    ListActions {
        /// ID of the workflow to show actions for (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,
    },
    /// Check if a workflow is complete
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # Check if workflow is complete
    torc workflows is-complete 123

    # Use in shell script
    if torc -f json workflows is-complete 123 | jq -e '.is_complete'; then
        echo 'Workflow finished!'
    fi
"
    )]
    IsComplete {
        /// ID of the workflow to check (optional - will prompt if not provided)
        #[arg()]
        id: Option<i64>,
    },

    /// Export a workflow to a portable JSON file
    ///
    /// Creates a self-contained export that can be imported into the same or
    /// different torc-server instance. All entity IDs are preserved in the export
    /// and remapped during import.
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # Export workflow to stdout
    torc workflows export 123

    # Export to a file
    torc workflows export 123 -o workflow.json

    # Include job results in export
    torc workflows export 123 --include-results -o backup.json

    # Include events (history) in export
    torc workflows export 123 --include-events -o full-backup.json

    # Export with all optional data
    torc workflows export 123 --include-results --include-events -o complete.json
"
    )]
    Export {
        /// ID of the workflow to export (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<String>,

        /// Include job results in export
        #[arg(long)]
        include_results: bool,

        /// Include events (workflow history) in export
        #[arg(long)]
        include_events: bool,
    },

    /// Import a workflow from an exported JSON file
    ///
    /// Imports a workflow that was previously exported. All entity IDs are
    /// remapped to new IDs assigned by the server. By default, all job statuses
    /// are reset to uninitialized for a fresh start.
    #[command(
        hide = true,
        after_long_help = "\
EXAMPLES:
    # Import a workflow (resets job statuses by default)
    torc workflows import workflow.json

    # Import from stdin
    cat workflow.json | torc workflows import -

    # Import with a different name
    torc workflows import workflow.json --name 'my-copy'

    # Skip importing results even if present in file
    torc workflows import workflow.json --skip-results
"
    )]
    Import {
        /// Path to the exported workflow JSON file (use '-' for stdin)
        #[arg()]
        file: String,

        /// Override the workflow name
        #[arg(long)]
        name: Option<String>,

        /// Skip importing results even if present in export
        #[arg(long)]
        skip_results: bool,

        /// Skip importing events even if present in export
        #[arg(long)]
        skip_events: bool,
    },

    /// Synchronize job statuses with Slurm (detect and fail orphaned jobs)
    ///
    /// This command detects jobs that are stuck in "running" status because their
    /// Slurm allocation terminated unexpectedly (e.g., due to timeout, node failure,
    /// or admin intervention). It marks these orphaned jobs as failed so the workflow
    /// can be recovered or restarted.
    ///
    /// Use this when:
    /// - `torc recover` reports "there are active Slurm allocations" but squeue shows none
    /// - Jobs appear stuck in "running" status after a Slurm allocation ended
    /// - You want to clean up workflow state before running `torc recover`
    #[command(
        hide = true,
        name = "sync-status",
        after_long_help = "\
EXAMPLES:
    # Preview what would be cleaned up
    torc workflows sync-status 123 --dry-run

    # Clean up orphaned jobs
    torc workflows sync-status 123

    # Get JSON output for scripting
    torc -f json workflows sync-status 123
"
    )]
    SyncStatus {
        /// ID of the workflow to sync (optional - will prompt if not provided)
        #[arg()]
        workflow_id: Option<i64>,

        /// Preview changes without applying them
        #[arg(long)]
        dry_run: bool,
    },
}

/// Parse JSON string fields into objects for better JSON output formatting
///
/// Converts JSON string fields (resource_monitor_config, slurm_defaults, metadata)
/// from string representations into actual JSON objects in the output.
/// This improves readability in JSON output while keeping them as strings in the database.
fn parse_json_fields(mut json: serde_json::Value) -> serde_json::Value {
    // Parse resource_monitor_config if present
    if let Some(config_str) = json["resource_monitor_config"].as_str()
        && let Ok(config_obj) = serde_json::from_str::<serde_json::Value>(config_str)
    {
        json["resource_monitor_config"] = config_obj;
    }

    // Parse slurm_defaults if present
    if let Some(defaults_str) = json["slurm_defaults"].as_str()
        && let Ok(defaults_obj) = serde_json::from_str::<serde_json::Value>(defaults_str)
    {
        json["slurm_defaults"] = defaults_obj;
    }

    // Parse metadata if present
    if let Some(metadata_str) = json["metadata"].as_str()
        && let Ok(metadata_obj) = serde_json::from_str::<serde_json::Value>(metadata_str)
    {
        json["metadata"] = metadata_obj;
    }

    json
}

fn show_execution_plan_from_spec(file_path: &str, format: &str) {
    // Parse the workflow spec
    let mut spec = match WorkflowSpec::from_spec_file(file_path) {
        Ok(spec) => spec,
        Err(e) => {
            eprintln!("Error parsing workflow specification: {}", e);
            std::process::exit(1);
        }
    };

    // Expand parameters
    if let Err(e) = spec.expand_parameters() {
        eprintln!("Error expanding parameters: {}", e);
        std::process::exit(1);
    }

    // Validate actions
    if let Err(e) = spec.validate_actions() {
        eprintln!("Error validating actions: {}", e);
        std::process::exit(1);
    }

    // Perform variable substitution to extract file/data dependencies
    if let Err(e) = spec.substitute_variables() {
        eprintln!("Error substituting variables: {}", e);
        std::process::exit(1);
    }

    // Build execution plan
    match crate::client::execution_plan::ExecutionPlan::from_spec(&spec) {
        Ok(plan) => {
            if format == "json" {
                // For JSON output, use the new DAG-based event structure
                let events_json: Vec<serde_json::Value> = plan.events.values().map(|event| {
                    serde_json::json!({
                        "id": event.id,
                        "trigger": event.trigger,
                        "trigger_description": event.trigger_description,
                        "scheduler_allocations": event.scheduler_allocations.iter().map(|alloc| {
                            serde_json::json!({
                                "scheduler": alloc.scheduler,
                                "scheduler_type": alloc.scheduler_type,
                                "num_allocations": alloc.num_allocations,
                                "job_names": alloc.jobs,
                            })
                        }).collect::<Vec<_>>(),
                        "jobs_becoming_ready": event.jobs_becoming_ready,
                        "depends_on_events": event.depends_on_events,
                        "unlocks_events": event.unlocks_events,
                    })
                }).collect();

                let output = serde_json::json!({
                    "status": "success",
                    "source": "spec_file",
                    "workflow_name": spec.name,
                    "total_events": plan.events.len(),
                    "total_jobs": spec.jobs.len(),
                    "root_events": plan.root_events,
                    "leaf_events": plan.leaf_events,
                    "events": events_json,
                });

                match serde_json::to_string_pretty(&output) {
                    Ok(json) => println!("{}", json),
                    Err(e) => {
                        eprintln!("Error serializing execution plan: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                // Display in human-readable format
                println!("\nWorkflow: {}", spec.name);
                if let Some(ref desc) = spec.description {
                    println!("Description: {}", desc);
                }
                println!("Total Jobs: {}", spec.jobs.len());
                plan.display();
            }
        }
        Err(e) => {
            eprintln!("Error building execution plan: {}", e);
            std::process::exit(1);
        }
    }
}

fn show_execution_plan_from_database(config: &Configuration, workflow_id: i64, format: &str) {
    // Fetch workflow from database
    let workflow = match default_api::get_workflow(config, workflow_id) {
        Ok(wf) => wf,
        Err(e) => {
            eprintln!("Error fetching workflow {}: {}", workflow_id, e);
            std::process::exit(1);
        }
    };

    // Fetch all jobs for this workflow
    let jobs = match paginate_jobs(
        config,
        workflow_id,
        JobListParams::new().with_include_relationships(true),
    ) {
        Ok(jobs) => jobs,
        Err(e) => {
            eprintln!("Error fetching jobs for workflow {}: {}", workflow_id, e);
            std::process::exit(1);
        }
    };

    // Fetch workflow actions
    let actions = match default_api::get_workflow_actions(config, workflow_id) {
        Ok(actions) => actions,
        Err(e) => {
            eprintln!("Error fetching actions for workflow {}: {}", workflow_id, e);
            std::process::exit(1);
        }
    };

    // Fetch slurm schedulers for this workflow
    let slurm_schedulers =
        match paginate_slurm_schedulers(config, workflow_id, SlurmSchedulersListParams::new()) {
            Ok(schedulers) => schedulers,
            Err(e) => {
                eprintln!(
                    "Warning: Could not fetch slurm schedulers for workflow {}: {}",
                    workflow_id, e
                );
                vec![]
            }
        };

    // Fetch resource requirements for this workflow
    let resource_requirements = match paginate_resource_requirements(
        config,
        workflow_id,
        ResourceRequirementsListParams::new(),
    ) {
        Ok(rrs) => rrs,
        Err(e) => {
            eprintln!(
                "Warning: Could not fetch resource requirements for workflow {}: {}",
                workflow_id, e
            );
            vec![]
        }
    };

    // Build execution plan from database models
    match crate::client::execution_plan::ExecutionPlan::from_database_models(
        &workflow,
        &jobs,
        &actions,
        &slurm_schedulers,
        &resource_requirements,
    ) {
        Ok(plan) => {
            if format == "json" {
                // For JSON output, use the new DAG-based event structure
                let events_json: Vec<serde_json::Value> = plan.events.values().map(|event| {
                    serde_json::json!({
                        "id": event.id,
                        "trigger": event.trigger,
                        "trigger_description": event.trigger_description,
                        "scheduler_allocations": event.scheduler_allocations.iter().map(|alloc| {
                            serde_json::json!({
                                "scheduler": alloc.scheduler,
                                "scheduler_type": alloc.scheduler_type,
                                "num_allocations": alloc.num_allocations,
                                "job_names": alloc.jobs,
                            })
                        }).collect::<Vec<_>>(),
                        "jobs_becoming_ready": event.jobs_becoming_ready,
                        "depends_on_events": event.depends_on_events,
                        "unlocks_events": event.unlocks_events,
                    })
                }).collect();

                let output = serde_json::json!({
                    "status": "success",
                    "source": "database",
                    "workflow_id": workflow_id,
                    "workflow_name": workflow.name,
                    "total_events": plan.events.len(),
                    "total_jobs": jobs.len(),
                    "root_events": plan.root_events,
                    "leaf_events": plan.leaf_events,
                    "events": events_json,
                });

                match serde_json::to_string_pretty(&output) {
                    Ok(json) => println!("{}", json),
                    Err(e) => {
                        eprintln!("Error serializing execution plan: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                println!("\nWorkflow ID: {}", workflow_id);
                println!("Workflow: {}", workflow.name);
                if let Some(desc) = &workflow.description {
                    println!("Description: {}", desc);
                }
                println!("Total Jobs: {}", jobs.len());
                plan.display();
            }
        }
        Err(e) => {
            eprintln!("Error building execution plan from database: {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_execution_plan(config: &Configuration, spec_or_id: &str, format: &str) {
    // Try to parse as workflow ID first, otherwise treat as file path
    if let Ok(workflow_id) = spec_or_id.parse::<i64>() {
        // Show execution plan for existing workflow from database
        show_execution_plan_from_database(config, workflow_id, format);
    } else {
        // Show execution plan for workflow from spec file
        show_execution_plan_from_spec(spec_or_id, format);
    }
}

fn handle_list_actions(
    config: &Configuration,
    workflow_id: &Option<i64>,
    user: &str,
    format: &str,
) {
    let selected_workflow_id = match workflow_id {
        Some(id) => *id,
        None => select_workflow_interactively(config, user).unwrap(),
    };

    match default_api::get_workflow_actions(config, selected_workflow_id) {
        Ok(actions) => {
            if format == "json" {
                let output = serde_json::json!({
                    "actions": actions
                });
                match serde_json::to_string_pretty(&output) {
                    Ok(json) => println!("{}", json),
                    Err(e) => {
                        eprintln!("Error serializing actions to JSON: {}", e);
                        std::process::exit(1);
                    }
                }
            } else if actions.is_empty() {
                println!(
                    "No workflow actions found for workflow {}",
                    selected_workflow_id
                );
            } else {
                println!("Workflow Actions for workflow {}:", selected_workflow_id);
                println!();

                let rows: Vec<WorkflowActionTableRow> = actions
                    .iter()
                    .map(|action| {
                        // Determine status based on trigger_count, required_triggers, and executed
                        let status = if action.executed {
                            "Executed".to_string()
                        } else if action.trigger_count >= action.required_triggers {
                            "Pending (ready to claim)".to_string()
                        } else {
                            "Waiting".to_string()
                        };

                        // Format progress as "trigger_count/required_triggers"
                        let progress =
                            format!("{}/{}", action.trigger_count, action.required_triggers);

                        // Format job_ids for display
                        let job_ids = match &action.job_ids {
                            Some(ids) if !ids.is_empty() => {
                                if ids.len() <= 5 {
                                    ids.iter()
                                        .map(|id| id.to_string())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                } else {
                                    format!(
                                        "{}, ... (+{} more)",
                                        ids.iter()
                                            .take(3)
                                            .map(|id| id.to_string())
                                            .collect::<Vec<_>>()
                                            .join(", "),
                                        ids.len() - 3
                                    )
                                }
                            }
                            _ => "(all jobs)".to_string(),
                        };

                        WorkflowActionTableRow {
                            id: action.id.unwrap_or(-1),
                            trigger_type: action.trigger_type.clone(),
                            action_type: action.action_type.clone(),
                            progress,
                            status,
                            executed_at: action.executed_at.as_deref().unwrap_or("-").to_string(),
                            job_ids,
                        }
                    })
                    .collect();

                display_table_with_count(&rows, "actions");

                // Print a helpful legend
                println!();
                println!("Status legend:");
                println!(
                    "  Waiting  - trigger_count < required_triggers (action not yet triggered)"
                );
                println!(
                    "  Pending  - trigger_count >= required_triggers (ready to be claimed and executed)"
                );
                println!("  Executed - action has been claimed and executed");
            }
        }
        Err(e) => {
            print_error("getting workflow actions", &e);
            std::process::exit(1);
        }
    }
}

/// Context for looking up jobs and their resource requirements
///
/// This struct centralizes job and resource requirement lookups to reduce
/// repeated nested if-let chains throughout violation detection.
struct ResourceLookupContext<'a> {
    jobs: &'a [models::JobModel],
    resource_requirements: &'a [models::ResourceRequirementsModel],
}

impl<'a> ResourceLookupContext<'a> {
    fn new(
        jobs: &'a [models::JobModel],
        resource_requirements: &'a [models::ResourceRequirementsModel],
    ) -> Self {
        Self {
            jobs,
            resource_requirements,
        }
    }

    fn find_job(&self, job_id: i64) -> Option<&models::JobModel> {
        self.jobs.iter().find(|j| j.id == Some(job_id))
    }

    fn find_resource_requirements(&self, rr_id: i64) -> Option<&models::ResourceRequirementsModel> {
        self.resource_requirements
            .iter()
            .find(|r| r.id == Some(rr_id))
    }
}

/// Detect memory violation based on actual peak memory usage
fn detect_memory_violation(
    ctx: &ResourceLookupContext,
    result: &models::ResultModel,
    job: &models::JobModel,
) -> bool {
    if let Some(peak_mem) = result.peak_memory_bytes
        && let Some(rr_id) = job.resource_requirements_id
        && let Some(rr) = ctx.find_resource_requirements(rr_id)
        && let Ok(specified_memory_bytes) = memory_string_to_bytes(&rr.memory)
    {
        peak_mem > specified_memory_bytes
    } else {
        false
    }
}

/// Detect CPU violation based on actual peak CPU percentage
fn detect_cpu_violation(
    ctx: &ResourceLookupContext,
    result: &models::ResultModel,
    job: &models::JobModel,
) -> bool {
    if let Some(peak_cpu) = result.peak_cpu_percent
        && let Some(rr_id) = job.resource_requirements_id
        && let Some(rr) = ctx.find_resource_requirements(rr_id)
    {
        let configured_cpus = rr.num_cpus as f64;
        let specified_cpu_percent = configured_cpus * 100.0;
        peak_cpu > specified_cpu_percent
    } else {
        false
    }
}

/// Detect runtime violation based on actual execution time
fn detect_runtime_violation(
    ctx: &ResourceLookupContext,
    result: &models::ResultModel,
    job: &models::JobModel,
) -> bool {
    if let Some(rr_id) = job.resource_requirements_id
        && let Some(rr) = ctx.find_resource_requirements(rr_id)
        && let Ok(specified_runtime_seconds) = duration_string_to_seconds(&rr.runtime)
    {
        let exec_time_seconds = result.exec_time_minutes * 60.0;
        let specified_runtime_seconds = specified_runtime_seconds as f64;
        exec_time_seconds > specified_runtime_seconds
    } else {
        false
    }
}

/// Detect timeout based on return code
fn detect_timeout(result: &models::ResultModel) -> bool {
    result.return_code == 152
}

fn handle_correct_resources(
    config: &Configuration,
    workflow_id: &Option<i64>,
    memory_multiplier: f64,
    runtime_multiplier: f64,
    job_ids: &Option<Vec<i64>>,
    dry_run: bool,
    format: &str,
) {
    use crate::client::report_models::ResourceUtilizationReport;
    use crate::client::resource_correction::apply_resource_corrections;

    let user_name = get_env_user_name();

    let selected_workflow_id = match workflow_id {
        Some(id) => *id,
        None => select_workflow_interactively(config, &user_name).unwrap(),
    };

    if format != "json" {
        if dry_run {
            eprintln!(
                "Analyzing and correcting resource requirements for workflow {} (dry-run mode)",
                selected_workflow_id
            );
        } else {
            eprintln!(
                "Analyzing and correcting resource requirements for workflow {}",
                selected_workflow_id
            );
        }
    }

    // Step 1: Fetch completed and failed results for diagnosis (uses latest run)
    let params = ResultListParams::new().with_status(models::JobStatus::Completed);
    let completed_results = match paginate_results(config, selected_workflow_id, params) {
        Ok(results) => results,
        Err(e) => {
            if format == "json" {
                let error_response = serde_json::json!({
                    "status": "error",
                    "message": format!("Failed to fetch completed results: {}", e),
                    "workflow_id": selected_workflow_id
                });
                println!("{}", serde_json::to_string_pretty(&error_response).unwrap());
            } else {
                eprintln!("Error: Failed to fetch completed results: {}", e);
            }
            std::process::exit(1);
        }
    };

    // Fetch failed results to analyze resource issues (uses latest run)
    let failed_params = ResultListParams::new().with_status(models::JobStatus::Failed);
    let failed_results =
        paginate_results(config, selected_workflow_id, failed_params).unwrap_or_default();

    let mut all_results = completed_results;
    all_results.extend(failed_results);

    if all_results.is_empty() {
        if format == "json" {
            let response = serde_json::json!({
                "status": "success",
                "workflow_id": selected_workflow_id,
                "resource_requirements_updated": 0,
                "jobs_analyzed": 0,
                "message": "No completed jobs found"
            });
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        } else {
            println!(
                "No completed jobs found for workflow {}",
                selected_workflow_id
            );
        }
        return;
    }

    // Step 2: Get jobs and resource requirements to build failed_jobs list
    let jobs = match paginate_jobs(config, selected_workflow_id, JobListParams::new()) {
        Ok(j) => j,
        Err(e) => {
            if format == "json" {
                let error_response = serde_json::json!({
                    "status": "error",
                    "message": format!("Failed to fetch jobs: {}", e),
                    "workflow_id": selected_workflow_id
                });
                println!("{}", serde_json::to_string_pretty(&error_response).unwrap());
            } else {
                eprintln!("Error: Failed to fetch jobs: {}", e);
            }
            std::process::exit(1);
        }
    };

    // Fetch resource requirements to check CPU allocations
    let resource_requirements = paginate_resource_requirements(
        config,
        selected_workflow_id,
        ResourceRequirementsListParams::new(),
    )
    .unwrap_or_default();

    // Build resource_violations list with violation detection
    let ctx = ResourceLookupContext::new(&jobs, &resource_requirements);
    let mut resource_violations = Vec::new();

    for result in &all_results {
        if let Some(job) = ctx.find_job(result.job_id) {
            let likely_oom = detect_memory_violation(&ctx, result, job);
            let likely_timeout = detect_timeout(result);
            let likely_cpu_violation = detect_cpu_violation(&ctx, result, job);
            let likely_runtime_violation = detect_runtime_violation(&ctx, result, job);

            if likely_oom || likely_timeout || likely_cpu_violation || likely_runtime_violation {
                let peak_memory_bytes = result.peak_memory_bytes;
                let (configured_cpus, configured_memory, configured_runtime) =
                    if let Some(rr_id) = job.resource_requirements_id {
                        if let Some(rr) = ctx.find_resource_requirements(rr_id) {
                            (rr.num_cpus, rr.memory.clone(), rr.runtime.clone())
                        } else {
                            (0, String::new(), String::new())
                        }
                    } else {
                        (0, String::new(), String::new())
                    };

                let violation_info = crate::client::report_models::ResourceViolationInfo {
                    job_id: result.job_id,
                    job_name: job.name.clone(),
                    return_code: result.return_code,
                    exec_time_minutes: result.exec_time_minutes,
                    configured_memory,
                    configured_runtime,
                    configured_cpus,
                    peak_memory_bytes,
                    peak_memory_formatted: None,
                    likely_oom,
                    oom_reason: if likely_oom {
                        Some("sigkill_137".to_string())
                    } else {
                        None
                    },
                    memory_over_utilization: None,
                    likely_timeout,
                    timeout_reason: if likely_timeout {
                        Some("sigxcpu_152".to_string())
                    } else {
                        None
                    },
                    runtime_utilization: None,
                    likely_cpu_violation,
                    peak_cpu_percent: result.peak_cpu_percent,
                };
                resource_violations.push(violation_info);
            }
        }
    }

    if resource_violations.is_empty() {
        if format == "json" {
            let response = serde_json::json!({
                "status": "success",
                "workflow_id": selected_workflow_id,
                "resource_requirements_updated": 0,
                "jobs_analyzed": all_results.len(),
                "message": "No jobs with over-utilization detected"
            });
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        } else {
            println!("No jobs with over-utilization (OOM/timeout/CPU) detected");
        }
        return;
    }

    // Create diagnosis report
    let diagnosis = ResourceUtilizationReport {
        workflow_id: selected_workflow_id,
        run_id: None,
        total_results: all_results.len(),
        over_utilization_count: resource_violations.len(),
        violations: Vec::new(),
        resource_violations_count: resource_violations.len(),
        resource_violations: resource_violations.clone(),
        failed_jobs_count: 0,
        failed_jobs: vec![],
    };

    // Step 3: Apply resource corrections
    let include_job_list = job_ids.as_deref().unwrap_or(&[]);

    match apply_resource_corrections(
        config,
        selected_workflow_id,
        &diagnosis,
        memory_multiplier,
        runtime_multiplier,
        include_job_list,
        dry_run,
    ) {
        Ok(result) => {
            if format == "json" {
                let response = serde_json::json!({
                    "status": "success",
                    "workflow_id": selected_workflow_id,
                    "dry_run": dry_run,
                    "memory_multiplier": memory_multiplier,
                    "runtime_multiplier": runtime_multiplier,
                    "resource_requirements_updated": result.resource_requirements_updated,
                    "jobs_analyzed": result.jobs_analyzed,
                    "memory_corrections": result.memory_corrections,
                    "runtime_corrections": result.runtime_corrections,
                    "adjustments": result.adjustments
                });
                println!("{}", serde_json::to_string_pretty(&response).unwrap());
            } else {
                // Print summary
                println!();
                println!("Resource Correction Summary:");
                println!("  Workflow: {}", selected_workflow_id);
                println!("  Jobs analyzed: {}", result.jobs_analyzed);
                println!(
                    "  Resource requirements updated: {}",
                    result.resource_requirements_updated
                );
                println!("  Memory corrections: {}", result.memory_corrections);
                println!("  Runtime corrections: {}", result.runtime_corrections);

                // Print details if any corrections were made
                if !result.adjustments.is_empty() {
                    println!();
                    println!("Adjustment Details:");
                    for adj in &result.adjustments {
                        println!(
                            "  RR {}: {} job(s)",
                            adj.resource_requirements_id,
                            adj.job_ids.len()
                        );
                        if let (Some(old_mem), Some(new_mem)) =
                            (&adj.original_memory, &adj.new_memory)
                        {
                            println!("    Memory: {} -> {}", old_mem, new_mem);
                        }
                        if let (Some(old_rt), Some(new_rt)) =
                            (&adj.original_runtime, &adj.new_runtime)
                        {
                            println!("    Runtime: {} -> {}", old_rt, new_rt);
                        }
                    }
                }

                if dry_run {
                    println!();
                    println!("(dry-run mode - changes not applied)");
                } else {
                    println!();
                    println!("Resource requirements updated successfully");
                }
            }
        }
        Err(e) => {
            if format == "json" {
                let error_response = serde_json::json!({
                    "status": "error",
                    "message": format!("Failed to apply resource corrections: {}", e),
                    "workflow_id": selected_workflow_id
                });
                println!("{}", serde_json::to_string_pretty(&error_response).unwrap());
            } else {
                eprintln!("Error: Failed to apply resource corrections: {}", e);
            }
            std::process::exit(1);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_cancel(config: &Configuration, workflow_id: &Option<i64>, format: &str) {
    let user_name = get_env_user_name();

    let selected_workflow_id = match workflow_id {
        Some(id) => *id,
        None => select_workflow_interactively(config, &user_name).unwrap(),
    };

    match default_api::cancel_workflow(config, selected_workflow_id, None) {
        Ok(_) => {
            if format != "json" {
                eprintln!("Successfully canceled workflow {}", selected_workflow_id);
            }
        }
        Err(e) => {
            if format == "json" {
                let error_response = serde_json::json!({
                    "status": "error",
                    "message": format!("Failed to cancel workflow: {}", e),
                    "workflow_id": selected_workflow_id
                });
                println!("{}", serde_json::to_string_pretty(&error_response).unwrap());
            } else {
                print_error("canceling workflow", &e);
            }
            std::process::exit(1);
        }
    }

    // Get all scheduled compute nodes for this workflow
    let nodes = match paginate_scheduled_compute_nodes(
        config,
        selected_workflow_id,
        ScheduledComputeNodeListParams::new(),
    ) {
        Ok(nodes) => nodes,
        Err(e) => {
            if format == "json" {
                let error_response = serde_json::json!({
                    "status": "error",
                    "message": format!("Failed to list scheduled compute nodes: {}", e),
                    "workflow_id": selected_workflow_id
                });
                println!("{}", serde_json::to_string_pretty(&error_response).unwrap());
            } else {
                print_error("listing scheduled compute nodes", &e);
            }
            std::process::exit(1);
        }
    };

    let mut canceled_jobs = Vec::new();
    let mut errors = Vec::new();

    for node in nodes {
        if node.scheduler_type == "slurm" {
            match crate::client::hpc::slurm_interface::SlurmInterface::new() {
                Ok(slurm_interface) => {
                    let job_id_str = node.scheduler_id.to_string();
                    match slurm_interface.cancel_job(&job_id_str) {
                        Ok(_) => {
                            canceled_jobs.push(node.scheduler_id);
                            if format != "json" {
                                println!("  Canceled Slurm job: {}", node.scheduler_id);
                            }
                            // Update the ScheduledComputeNode status to "complete"
                            if let Some(node_id) = node.id {
                                let updated_node = models::ScheduledComputeNodesModel::new(
                                    node.workflow_id,
                                    node.scheduler_id,
                                    node.scheduler_config_id,
                                    node.scheduler_type.clone(),
                                    "complete".to_string(),
                                );
                                if let Err(e) = default_api::update_scheduled_compute_node(
                                    config,
                                    node_id,
                                    updated_node,
                                ) {
                                    let error_msg =
                                        format!("Failed to update node {} status: {}", node_id, e);
                                    errors.push(error_msg.clone());
                                    if format != "json" {
                                        eprintln!("  {}", error_msg);
                                    }
                                } else if format != "json" {
                                    println!(
                                        "  Updated node {} status to complete",
                                        node.scheduler_id
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            let error_msg =
                                format!("Failed to cancel Slurm job {}: {}", node.scheduler_id, e);
                            errors.push(error_msg.clone());
                            if format != "json" {
                                eprintln!("  {}", error_msg);
                            }
                        }
                    }
                }
                Err(e) => {
                    let error_msg = format!(
                        "Failed to create SlurmInterface for job {}: {}",
                        node.scheduler_id, e
                    );
                    errors.push(error_msg.clone());
                    if format != "json" {
                        eprintln!("  {}", error_msg);
                    }
                }
            }
        }
    }

    if format == "json" {
        let response = serde_json::json!({
            "status": if errors.is_empty() { "success" } else { "partial_success" },
            "workflow_id": selected_workflow_id,
            "canceled_slurm_jobs": canceled_jobs,
            "errors": if errors.is_empty() { None } else { Some(errors) }
        });
        println!("{}", serde_json::to_string_pretty(&response).unwrap());
    } else if !canceled_jobs.is_empty() {
        println!("Canceled {} Slurm job(s)", canceled_jobs.len());
    }
}

fn handle_reset_status(
    config: &Configuration,
    workflow_id: &Option<i64>,
    failed_only: bool,
    reinitialize: bool,
    force: bool,
    no_prompts: bool,
    format: &str,
) {
    let user_name = get_env_user_name();

    let selected_workflow_id = match workflow_id {
        Some(id) => *id,
        None => select_workflow_interactively(config, &user_name).unwrap(),
    };

    // Show confirmation prompt unless --no-prompt or format is json
    if !no_prompts && format != "json" {
        eprintln!(
            "\nWarning: You are about to reset the status for workflow {}.",
            selected_workflow_id
        );
        if failed_only {
            eprintln!("This will reset the status of all failed jobs.");
        } else {
            eprintln!(
                "This will reset the status of all jobs as well as results of completed jobs."
            );
        }
        if reinitialize {
            eprintln!("The workflow will be reinitialized after reset.");
        }
        if force {
            eprintln!("Force mode is enabled (will ignore running/pending jobs check).");
        }
        print!("\nDo you want to continue? (y/N): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                let response = input.trim().to_lowercase();
                if response != "y" && response != "yes" {
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

    // Track the results of each operation for JSON output
    let mut workflow_reset_success = false;
    let mut job_reset_success = false;
    let mut reinitialize_success = false;
    let mut errors = Vec::<String>::new();

    // Pass force as query parameter
    let force_param = if force { Some(true) } else { None };

    // Reset workflow status
    match default_api::reset_workflow_status(config, selected_workflow_id, force_param, None) {
        Ok(_) => {
            workflow_reset_success = true;
            if format != "json" {
                eprintln!(
                    "Successfully reset workflow status for workflow {}",
                    selected_workflow_id
                );
            }
        }
        Err(e) => {
            errors.push(format!("resetting workflow status: {}", e));
            if format != "json" {
                print_error("resetting workflow status", &e);
            }
        }
    }

    // Reset job status
    match default_api::reset_job_status(config, selected_workflow_id, Some(failed_only), None) {
        Ok(_) => {
            job_reset_success = true;
            if format != "json" {
                if failed_only {
                    eprintln!(
                        "Successfully reset failed job status for workflow {}",
                        selected_workflow_id
                    );
                } else {
                    eprintln!(
                        "Successfully reset all job status for workflow {}",
                        selected_workflow_id
                    );
                }
            }
        }
        Err(e) => {
            errors.push(format!("resetting job status: {}", e));
            if format != "json" {
                print_error("resetting job status", &e);
            }
        }
    }

    // If reinitialize is true, reinitialize the workflow
    if reinitialize {
        match default_api::get_workflow(config, selected_workflow_id) {
            Ok(workflow) => {
                let torc_config = TorcConfig::load().unwrap_or_default();
                let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow);
                match workflow_manager.reinitialize(false, false) {
                    Ok(()) => {
                        reinitialize_success = true;
                        if format != "json" {
                            eprintln!(
                                "Successfully reinitialized workflow {}",
                                selected_workflow_id
                            );
                        }
                    }
                    Err(e) => {
                        errors.push(format!("reinitializing workflow: {}", e));
                        if format != "json" {
                            eprintln!(
                                "Error reinitializing workflow {}: {}",
                                selected_workflow_id, e
                            );
                        }
                    }
                }
            }
            Err(e) => {
                errors.push(format!("getting workflow for reinitialize: {}", e));
                if format != "json" {
                    print_error("getting workflow for reinitialize", &e);
                }
            }
        }
    }

    // Output combined JSON or exit with error if any operation failed
    if format == "json" {
        let overall_success =
            workflow_reset_success && job_reset_success && (!reinitialize || reinitialize_success);

        let mut messages = Vec::new();
        if workflow_reset_success {
            messages.push(format!(
                "Successfully reset workflow status for workflow {}",
                selected_workflow_id
            ));
        }
        if job_reset_success {
            if failed_only {
                messages.push(format!(
                    "Successfully reset failed job status for workflow {}",
                    selected_workflow_id
                ));
            } else {
                messages.push(format!(
                    "Successfully reset all job status for workflow {}",
                    selected_workflow_id
                ));
            }
        }
        if reinitialize && reinitialize_success {
            messages.push(format!(
                "Successfully reinitialized workflow {}",
                selected_workflow_id
            ));
        }

        let response = if overall_success {
            serde_json::json!({
                "status": "success",
                "workflow_id": selected_workflow_id,
                "operations": {
                    "workflow_reset": workflow_reset_success,
                    "job_reset": job_reset_success,
                    "reinitialize": if reinitialize { Some(reinitialize_success) } else { None }
                },
                "failed_only": failed_only,
                "messages": messages
            })
        } else {
            serde_json::json!({
                "status": "error",
                "workflow_id": selected_workflow_id,
                "operations": {
                    "workflow_reset": workflow_reset_success,
                    "job_reset": job_reset_success,
                    "reinitialize": if reinitialize { Some(reinitialize_success) } else { None }
                },
                "failed_only": failed_only,
                "messages": messages,
                "errors": errors
            })
        };

        println!("{}", serde_json::to_string_pretty(&response).unwrap());
    }

    // Exit with error if any operation failed
    if !errors.is_empty() {
        std::process::exit(1);
    }
}

fn handle_status(config: &Configuration, workflow_id: &Option<i64>, user: &str, format: &str) {
    let selected_workflow_id = match workflow_id {
        Some(id) => *id,
        None => select_workflow_interactively(config, user).unwrap(),
    };

    match default_api::get_workflow_status(config, selected_workflow_id) {
        Ok(status) => {
            if format == "json" {
                match serde_json::to_string_pretty(&status) {
                    Ok(json) => println!("{}", json),
                    Err(e) => {
                        eprintln!("Error serializing workflow status to JSON: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                println!("Workflow Status for ID {}:", selected_workflow_id);
                if let Some(id) = status.id {
                    println!("  Status ID: {}", id);
                }
                println!("  Run ID: {}", status.run_id);
                println!("  Is Canceled: {}", status.is_canceled);
                if let Some(is_archived) = status.is_archived {
                    println!("  Is Archived: {}", is_archived);
                }
            }
        }
        Err(e) => {
            print_error("getting workflow status", &e);
            std::process::exit(1);
        }
    }
}

fn handle_reinitialize(
    config: &Configuration,
    workflow_id: &Option<i64>,
    force: bool,
    dry_run: bool,
    format: &str,
) {
    let user_name = get_env_user_name();

    let selected_workflow_id = match workflow_id {
        Some(id) => *id,
        None => select_workflow_interactively(config, &user_name).unwrap(),
    };
    // First get the workflow
    match default_api::get_workflow(config, selected_workflow_id) {
        Ok(workflow) => {
            let torc_config = TorcConfig::load().unwrap_or_default();
            let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow);

            // Handle dry-run mode
            if dry_run {
                match workflow_manager.check_initialization() {
                    Ok(check_result) => {
                        if format == "json" {
                            let response = serde_json::json!({
                                "workflow_id": selected_workflow_id,
                                "safe": check_result.safe,
                                "missing_input_files": check_result.missing_input_files,
                                "missing_input_file_count": check_result.missing_input_files.len(),
                                "existing_output_files": check_result.existing_output_files,
                                "existing_output_file_count": check_result.existing_output_files.len(),
                            });
                            println!("{}", serde_json::to_string_pretty(&response).unwrap());
                        } else {
                            println!(
                                "Re-initialization check for workflow {}:",
                                selected_workflow_id
                            );
                            if !check_result.missing_input_files.is_empty() {
                                eprintln!(
                                    "\n❌ Missing {} required input file(s):",
                                    check_result.missing_input_files.len()
                                );
                                for file in &check_result.missing_input_files {
                                    eprintln!("  - {}", file);
                                }
                            }
                            if !check_result.existing_output_files.is_empty() {
                                eprintln!(
                                    "\n⚠️  Found {} existing output file(s):",
                                    check_result.existing_output_files.len()
                                );
                                for file in &check_result.existing_output_files {
                                    eprintln!("  - {}", file);
                                }
                            }
                            if check_result.safe {
                                println!("\n✅ Safe to reinitialize (no missing input files)");
                            } else {
                                eprintln!("\n❌ Cannot reinitialize: missing required input files");
                            }
                        }

                        // Exit with appropriate code
                        if !check_result.safe {
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        if format == "json" {
                            let error_response = serde_json::json!({
                                "status": "error",
                                "message": format!("Failed to check re-initialization: {}", e),
                                "workflow_id": selected_workflow_id
                            });
                            println!("{}", serde_json::to_string_pretty(&error_response).unwrap());
                        } else {
                            eprintln!(
                                "Error checking re-initialization for workflow {}: {}",
                                selected_workflow_id, e
                            );
                        }
                        std::process::exit(1);
                    }
                }
            } else {
                // Normal reinitialization (not dry-run)
                match workflow_manager.reinitialize(force, dry_run) {
                    Ok(()) => {
                        if format == "json" {
                            let success_response = serde_json::json!({
                                "status": "success",
                                "message": format!("Successfully reinitialized workflow {}", selected_workflow_id),
                                "workflow_id": selected_workflow_id
                            });
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&success_response).unwrap()
                            );
                        } else {
                            eprintln!("Successfully reinitialized workflow:");
                            println!("  Workflow ID: {}", selected_workflow_id);
                        }
                    }
                    Err(e) => {
                        if format == "json" {
                            let error_response = serde_json::json!({
                                "status": "error",
                                "message": format!("Failed to reinitialize workflow: {}", e),
                                "workflow_id": selected_workflow_id
                            });
                            println!("{}", serde_json::to_string_pretty(&error_response).unwrap());
                        } else {
                            eprintln!(
                                "Error reinitializing workflow {}: {}",
                                selected_workflow_id, e
                            );
                        }
                        std::process::exit(1);
                    }
                }
            }
        }
        Err(e) => {
            print_error("getting workflow for reinitialize", &e);
            std::process::exit(1);
        }
    }
}

fn handle_initialize(
    config: &Configuration,
    workflow_id: &Option<i64>,
    force: bool,
    no_prompts: bool,
    dry_run: bool,
    format: &str,
) {
    let user_name = get_env_user_name();

    let selected_workflow_id = match workflow_id {
        Some(id) => *id,
        None => select_workflow_interactively(config, &user_name).unwrap(),
    };

    // First get the workflow
    match default_api::get_workflow(config, selected_workflow_id) {
        Ok(workflow) => {
            let torc_config = TorcConfig::load().unwrap_or_default();
            let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow);

            // Handle dry-run mode
            if dry_run {
                match workflow_manager.check_initialization() {
                    Ok(check_result) => {
                        if format == "json" {
                            let response = serde_json::json!({
                                "workflow_id": selected_workflow_id,
                                "safe": check_result.safe,
                                "missing_input_files": check_result.missing_input_files,
                                "missing_input_file_count": check_result.missing_input_files.len(),
                                "existing_output_files": check_result.existing_output_files,
                                "existing_output_file_count": check_result.existing_output_files.len(),
                            });
                            println!("{}", serde_json::to_string_pretty(&response).unwrap());
                        } else {
                            println!(
                                "Initialization check for workflow {}:",
                                selected_workflow_id
                            );
                            if !check_result.missing_input_files.is_empty() {
                                eprintln!(
                                    "\n❌ Missing {} required input file(s):",
                                    check_result.missing_input_files.len()
                                );
                                for file in &check_result.missing_input_files {
                                    eprintln!("  - {}", file);
                                }
                            }
                            if !check_result.existing_output_files.is_empty() {
                                eprintln!(
                                    "\n⚠️  Found {} existing output file(s):",
                                    check_result.existing_output_files.len()
                                );
                                for file in &check_result.existing_output_files {
                                    eprintln!("  - {}", file);
                                }
                            }
                            if check_result.safe {
                                println!("\n✅ Safe to initialize (no missing input files)");
                            } else {
                                eprintln!("\n❌ Cannot initialize: missing required input files");
                            }
                        }

                        // Exit with appropriate code
                        if !check_result.safe {
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        if format == "json" {
                            let error_response = serde_json::json!({
                                "status": "error",
                                "message": format!("Failed to check initialization: {}", e),
                                "workflow_id": selected_workflow_id
                            });
                            println!("{}", serde_json::to_string_pretty(&error_response).unwrap());
                        } else {
                            eprintln!(
                                "Error checking initialization for workflow {}: {}",
                                selected_workflow_id, e
                            );
                        }
                        std::process::exit(1);
                    }
                }
            } else {
                // Normal initialization (not dry-run)
                match default_api::is_workflow_uninitialized(config, selected_workflow_id) {
                    Ok(is_initialized) => {
                        if is_initialized.as_bool().unwrap_or(false)
                            && !no_prompts
                            && format != "json"
                        {
                            println!("\nWarning: This workflow has already been initialized.");
                            println!("Some jobs already have initialized status.");
                            print!("\nDo you want to continue? (y/N): ");
                            io::stdout().flush().unwrap();

                            let mut input = String::new();
                            match io::stdin().read_line(&mut input) {
                                Ok(_) => {
                                    let response = input.trim().to_lowercase();
                                    if response != "y" && response != "yes" {
                                        println!("Initialization cancelled.");
                                        std::process::exit(0);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to read input: {}", e);
                                    std::process::exit(1);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        print_error("checking workflow initialization status", &e);
                        std::process::exit(1);
                    }
                }
                match workflow_manager.initialize(force) {
                    Ok(()) => {
                        if format == "json" {
                            let success_response = serde_json::json!({
                                "status": "success",
                                "message": format!("Successfully started workflow {}", selected_workflow_id),
                                "workflow_id": selected_workflow_id
                            });
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&success_response).unwrap()
                            );
                        } else {
                            println!("Successfully started workflow:");
                            println!("  Workflow ID: {}", selected_workflow_id);
                        }
                    }
                    Err(e) => {
                        if format == "json" {
                            let error_response = serde_json::json!({
                                "status": "error",
                                "message": format!("Failed to start workflow: {}", e),
                                "workflow_id": selected_workflow_id
                            });
                            println!("{}", serde_json::to_string_pretty(&error_response).unwrap());
                        } else {
                            eprintln!("Error starting workflow {}: {}", selected_workflow_id, e);
                        }
                        std::process::exit(1);
                    }
                }
            }
        }
        Err(e) => {
            print_error("getting workflow for start", &e);
            std::process::exit(1);
        }
    }
}

fn handle_run(
    config: &Configuration,
    workflow_id: &Option<i64>,
    poll_interval: f64,
    max_parallel_jobs: Option<i64>,
    output_dir: &std::path::Path,
) {
    let user_name = get_env_user_name();

    let selected_workflow_id = match workflow_id {
        Some(id) => *id,
        None => select_workflow_interactively(config, &user_name).unwrap(),
    };

    // Build args for run_jobs_cmd with sensible defaults
    // Pass through authentication from config
    let password = config.basic_auth.as_ref().and_then(|(_, p)| p.clone());
    let args = crate::run_jobs_cmd::Args {
        workflow_id: Some(selected_workflow_id),
        url: config.base_path.clone(),
        output_dir: output_dir.to_path_buf(),
        poll_interval,
        max_parallel_jobs,
        time_limit: None,
        end_time: None,
        num_cpus: None,
        memory_gb: None,
        num_gpus: None,
        num_nodes: None,
        scheduler_config_id: None,
        log_prefix: None,
        cpu_affinity_cpus_per_job: None,
        log_level: "info".to_string(),
        password,
    };

    crate::run_jobs_cmd::run(&args);
}

fn handle_submit(config: &Configuration, workflow_id: &Option<i64>, force: bool, format: &str) {
    let user_name = get_env_user_name();

    let selected_workflow_id = match workflow_id {
        Some(id) => *id,
        None => select_workflow_interactively(config, &user_name).unwrap(),
    };

    // Check if workflow has schedule_nodes actions
    match default_api::get_workflow_actions(config, selected_workflow_id) {
        Ok(actions) => {
            let has_schedule_nodes = actions.iter().any(|action| {
                action.trigger_type == "on_workflow_start" && action.action_type == "schedule_nodes"
            });

            if !has_schedule_nodes {
                if format == "json" {
                    let error_response = serde_json::json!({
                        "status": "error",
                        "message": "Cannot submit workflow: no on_workflow_start action with schedule_nodes found",
                        "workflow_id": selected_workflow_id
                    });
                    println!("{}", serde_json::to_string_pretty(&error_response).unwrap());
                } else {
                    eprintln!("Error: Cannot submit workflow {}", selected_workflow_id);
                    eprintln!();
                    eprintln!(
                        "The workflow does not define an on_workflow_start action with schedule_nodes."
                    );
                    eprintln!("To submit to a scheduler, add a workflow action like:");
                    eprintln!();
                    eprintln!("  actions:");
                    eprintln!("    - trigger_type: on_workflow_start");
                    eprintln!("      action_type: schedule_nodes");
                    eprintln!("      scheduler_type: slurm");
                    eprintln!("      scheduler: \"my-cluster\"");
                    eprintln!();
                    eprintln!("Or run locally instead:");
                    eprintln!("  torc workflows run {}", selected_workflow_id);
                }
                std::process::exit(1);
            }
        }
        Err(e) => {
            print_error("getting workflow actions", &e);
            std::process::exit(1);
        }
    }

    // Get the workflow and submit it
    match default_api::get_workflow(config, selected_workflow_id) {
        Ok(workflow) => {
            let torc_config = TorcConfig::load().unwrap_or_default();
            let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow);
            match workflow_manager.start(force) {
                Ok(()) => {
                    if format == "json" {
                        let success_response = serde_json::json!({
                            "status": "success",
                            "message": format!("Successfully submitted workflow {}", selected_workflow_id),
                            "workflow_id": selected_workflow_id
                        });
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&success_response).unwrap()
                        );
                    } else {
                        println!("Successfully submitted workflow:");
                        println!("  Workflow ID: {}", selected_workflow_id);
                    }
                }
                Err(e) => {
                    if format == "json" {
                        let error_response = serde_json::json!({
                            "status": "error",
                            "message": format!("Failed to submit workflow: {}", e),
                            "workflow_id": selected_workflow_id
                        });
                        println!("{}", serde_json::to_string_pretty(&error_response).unwrap());
                    } else {
                        eprintln!("Error submitting workflow {}: {}", selected_workflow_id, e);
                    }
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            print_error("getting workflow for submit", &e);
            std::process::exit(1);
        }
    }
}

fn handle_archive(config: &Configuration, is_archived: &str, workflow_ids: &[i64], format: &str) {
    // Parse is_archived string to bool
    let is_archived_bool = match is_archived.to_lowercase().as_str() {
        "true" | "1" | "yes" => true,
        "false" | "0" | "no" => false,
        _ => {
            eprintln!("Error: is_archived must be 'true' or 'false'");
            std::process::exit(1);
        }
    };

    let user_name = get_env_user_name();

    // If no workflow IDs provided, prompt for interactive selection
    let ids_to_update = if workflow_ids.is_empty() {
        vec![select_workflow_interactively(config, &user_name).unwrap()]
    } else {
        workflow_ids.to_vec()
    };

    let mut updated_workflows = Vec::new();
    let mut errors = Vec::new();
    let action = if is_archived_bool {
        "archive"
    } else {
        "unarchive"
    };
    let action_past = if is_archived_bool {
        "archived"
    } else {
        "unarchived"
    };

    for workflow_id in ids_to_update {
        // First, get the current workflow status
        match default_api::get_workflow_status(config, workflow_id) {
            Ok(mut status) => {
                // Set is_archived to the specified value
                status.is_archived = Some(is_archived_bool);

                // Update the workflow status
                match default_api::update_workflow_status(config, workflow_id, status) {
                    Ok(_) => {
                        updated_workflows.push(workflow_id);
                        if format != "json" {
                            println!("Successfully {} workflow {}", action_past, workflow_id);
                        }
                    }
                    Err(e) => {
                        let error_msg =
                            format!("Failed to {} workflow {}: {}", action, workflow_id, e);
                        errors.push(error_msg.clone());
                        if format != "json" {
                            eprintln!("Error: {}", error_msg);
                        }
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("Failed to get status for workflow {}: {}", workflow_id, e);
                errors.push(error_msg.clone());
                if format != "json" {
                    eprintln!("Error: {}", error_msg);
                }
            }
        }
    }

    // Output JSON response if requested
    if format == "json" {
        let response = if errors.is_empty() {
            serde_json::json!({
                "status": "success",
                "updated_workflows": updated_workflows,
                "is_archived": is_archived_bool,
            })
        } else {
            serde_json::json!({
                "status": if updated_workflows.is_empty() { "error" } else { "partial_success" },
                "updated_workflows": updated_workflows,
                "is_archived": is_archived_bool,
                "errors": errors,
            })
        };
        println!("{}", serde_json::to_string_pretty(&response).unwrap());
    }

    // Exit with error if any workflow failed to update
    if !errors.is_empty() {
        std::process::exit(1);
    }
}

fn handle_delete(config: &Configuration, ids: &[i64], no_prompts: bool, format: &str) {
    let user_name = get_env_user_name();

    // Get list of workflow IDs to delete
    let workflow_ids = if ids.is_empty() {
        // No IDs provided - select one interactively
        vec![select_workflow_interactively(config, &user_name).unwrap()]
    } else {
        ids.to_vec()
    };

    let mut deleted_workflows = Vec::new();
    let mut failed_deletions = Vec::new();

    for selected_id in workflow_ids {
        // Fetch workflow details to show what will be deleted
        let workflow = match default_api::get_workflow(config, selected_id) {
            Ok(wf) => wf,
            Err(e) => {
                failed_deletions.push((selected_id, format!("Failed to get workflow: {}", e)));
                continue;
            }
        };

        // Check if user owns the workflow
        if workflow.user != user_name {
            let error_msg = format!(
                "Cannot delete workflow owned by user '{}' (you are '{}').",
                workflow.user, user_name
            );
            failed_deletions.push((selected_id, error_msg));
            continue;
        }

        // Count jobs in this workflow
        let job_count = match default_api::list_jobs(
            config,
            selected_id,
            None,    // status
            None,    // needs_file_id
            None,    // upstream_job_id
            None,    // offset
            Some(1), // limit (we just need the total count)
            None,    // sort_by
            None,    // reverse_sort
            None,    // include_relationships
            None,    // active_compute_node_id
        ) {
            Ok(response) => response.total_count,
            Err(e) => {
                failed_deletions.push((selected_id, format!("Failed to count jobs: {}", e)));
                continue;
            }
        };

        // If not skipping prompts, show what will be deleted and ask for confirmation
        if !no_prompts && format != "json" {
            println!("\nWarning: You are about to delete the following workflow:");
            println!("  ID: {}", workflow.id.unwrap_or(-1));
            println!("  Name: {}", workflow.name);
            println!("  User: {}", workflow.user);
            if let Some(desc) = &workflow.description {
                println!("  Description: {}", desc);
            }
            println!("\nThis will also delete:");
            println!("  - {} job(s)", job_count);
            println!("  - All associated files, user data, and results");
            println!("  - All job dependencies and relationships");
            println!("\nThis action cannot be undone.");
            print!("\nAre you sure you want to delete this workflow? (y/N): ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(_) => {
                    let response = input.trim().to_lowercase();
                    if response != "y" && response != "yes" {
                        println!("Deletion cancelled for workflow {}.", selected_id);
                        continue;
                    }
                }
                Err(e) => {
                    failed_deletions.push((selected_id, format!("Failed to read input: {}", e)));
                    continue;
                }
            }
        }

        // Proceed with deletion
        match default_api::delete_workflow(config, selected_id, None) {
            Ok(removed_workflow) => {
                deleted_workflows.push(removed_workflow);
            }
            Err(e) => {
                failed_deletions.push((selected_id, format!("Failed to delete: {}", e)));
            }
        }
    }

    // Output results
    if format == "json" {
        // For JSON output, return array of deleted workflows
        let json_array: Vec<_> = deleted_workflows
            .iter()
            .map(|wf| {
                let json = serde_json::to_value(wf).unwrap();
                parse_json_fields(json)
            })
            .collect();

        match serde_json::to_string_pretty(&json_array) {
            Ok(json_str) => println!("{}", json_str),
            Err(e) => {
                eprintln!("Error serializing workflows to JSON: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        // For table output, show summary
        if !deleted_workflows.is_empty() {
            println!(
                "\nSuccessfully deleted {} workflow(s):",
                deleted_workflows.len()
            );
            for wf in &deleted_workflows {
                println!(
                    "  - ID: {}, Name: {}, User: {}",
                    wf.id.unwrap_or(-1),
                    wf.name,
                    wf.user
                );
            }
        }

        if !failed_deletions.is_empty() {
            eprintln!("\nFailed to delete {} workflow(s):", failed_deletions.len());
            for (id, error) in &failed_deletions {
                eprintln!("  - ID {}: {}", id, error);
            }
        }
    }

    // Exit with error if any deletions failed
    if !failed_deletions.is_empty() && deleted_workflows.is_empty() {
        std::process::exit(1);
    }
}

struct WorkflowUpdateFields {
    name: Option<String>,
    description: Option<String>,
    owner_user: Option<String>,
    project: Option<String>,
    metadata: Option<String>,
}

fn handle_update(
    config: &Configuration,
    id: &Option<i64>,
    updates: &WorkflowUpdateFields,
    format: &str,
) {
    let user_name = get_env_user_name();

    let selected_id = match id {
        Some(workflow_id) => *workflow_id,
        None => select_workflow_interactively(config, &user_name).unwrap(),
    };
    // First get the existing workflow
    match default_api::get_workflow(config, selected_id) {
        Ok(mut workflow) => {
            // Update fields that were provided
            if let Some(new_name) = &updates.name {
                workflow.name = new_name.clone();
            }
            if updates.description.is_some() {
                workflow.description = updates.description.clone();
            }
            if let Some(new_user) = &updates.owner_user {
                workflow.user = new_user.clone();
            }
            if updates.project.is_some() {
                workflow.project = updates.project.clone();
            }
            if updates.metadata.is_some() {
                workflow.metadata = updates.metadata.clone();
            }

            match default_api::update_workflow(config, selected_id, workflow) {
                Ok(updated_workflow) => {
                    if format == "json" {
                        // Convert workflow to JSON value, parsing JSON string fields to objects
                        let json = serde_json::to_value(&updated_workflow).unwrap();
                        let json = parse_json_fields(json);

                        match serde_json::to_string_pretty(&json) {
                            Ok(json_str) => println!("{}", json_str),
                            Err(e) => {
                                eprintln!("Error serializing workflow to JSON: {}", e);
                                std::process::exit(1);
                            }
                        }
                    } else {
                        println!("Successfully updated workflow:");
                        println!("  ID: {}", updated_workflow.id.unwrap_or(-1));
                        println!("  Name: {}", updated_workflow.name);
                        println!("  User: {}", updated_workflow.user);
                        if let Some(desc) = &updated_workflow.description {
                            println!("  Description: {}", desc);
                        }
                    }
                }
                Err(e) => {
                    print_error("updating workflow", &e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            print_error("getting workflow for update", &e);
            std::process::exit(1);
        }
    }
}

fn handle_get(config: &Configuration, id: &Option<i64>, user: &str, format: &str) {
    let selected_id = match id {
        Some(workflow_id) => *workflow_id,
        None => select_workflow_interactively(config, user).unwrap(),
    };

    match default_api::get_workflow(config, selected_id) {
        Ok(workflow) => {
            if format == "json" {
                // Convert workflow to JSON value, parsing JSON string fields to objects
                let json = serde_json::to_value(&workflow).unwrap();
                let json = parse_json_fields(json);

                match serde_json::to_string_pretty(&json) {
                    Ok(json_str) => println!("{}", json_str),
                    Err(e) => {
                        eprintln!("Error serializing workflow to JSON: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                println!("Workflow ID {}:", selected_id);
                println!("  Name: {}", workflow.name);
                println!("  User: {}", workflow.user);
                if let Some(desc) = &workflow.description {
                    println!("  Description: {}", desc);
                }
                if let Some(timestamp) = &workflow.timestamp {
                    println!("  Timestamp: {}", timestamp);
                }
                if let Some(defaults_str) = &workflow.slurm_defaults
                    && let Ok(defaults) = serde_json::from_str::<serde_json::Value>(defaults_str)
                    && let Some(obj) = defaults.as_object()
                {
                    println!("  Slurm Defaults:");
                    for (key, value) in obj {
                        let value_str = match value {
                            serde_json::Value::String(s) => s.clone(),
                            _ => value.to_string(),
                        };
                        println!("    {}: {}", key, value_str);
                    }
                }
                if let Some(config_str) = &workflow.resource_monitor_config
                    && let Ok(config) = serde_json::from_str::<serde_json::Value>(config_str)
                    && let Some(obj) = config.as_object()
                {
                    println!("  Resource Monitor:");
                    for (key, value) in obj {
                        let value_str = match value {
                            serde_json::Value::String(s) => s.clone(),
                            serde_json::Value::Bool(b) => b.to_string(),
                            serde_json::Value::Number(n) => n.to_string(),
                            _ => value.to_string(),
                        };
                        println!("    {}: {}", key, value_str);
                    }
                }
            }
        }
        Err(e) => {
            print_error("getting workflow", &e);
            std::process::exit(1);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_list(
    config: &Configuration,
    user: &str,
    limit: i64,
    offset: i64,
    sort_by: &Option<String>,
    reverse_sort: bool,
    archived_only: bool,
    include_archived: bool,
    all_users: bool,
    format: &str,
) {
    // Use pagination utility to get all workflows
    let mut params = WorkflowListParams::new()
        .with_offset(offset)
        .with_limit(limit)
        .with_reverse_sort(reverse_sort);

    // When --all-users is not set, filter by current user (default behavior)
    if !all_users {
        params = params.with_user(user.to_string());
    }

    // Handle archive filtering:
    // - include_archived: show both archived and non-archived (is_archived = None)
    // - archived_only: show only archived (is_archived = Some(true))
    // - default: show only non-archived (is_archived = Some(false))
    if !include_archived {
        params = params.with_is_archived(archived_only);
    }

    if let Some(sort_field) = sort_by {
        params = params.with_sort_by(sort_field.clone());
    }

    match paginate_workflows(config, params) {
        Ok(workflows) => {
            if format == "json" {
                // Convert workflows to JSON values, parsing JSON string fields to objects
                let workflows_json: Vec<serde_json::Value> = workflows
                    .iter()
                    .map(|workflow| {
                        let json = serde_json::to_value(workflow).unwrap();
                        parse_json_fields(json)
                    })
                    .collect();

                print_json_wrapped("workflows", &workflows_json, "workflows");
            } else if workflows.is_empty() {
                if all_users {
                    println!("No workflows found.");
                } else {
                    println!("No workflows found for user: {}", user);
                }
            } else if all_users {
                println!("All workflows:");
                let rows: Vec<WorkflowTableRow> = workflows
                    .iter()
                    .map(|workflow| WorkflowTableRow {
                        id: workflow.id.unwrap_or(-1),
                        user: workflow.user.clone(),
                        name: workflow.name.clone(),
                        description: workflow.description.as_deref().unwrap_or("").to_string(),
                        project: workflow.project.as_deref().unwrap_or("").to_string(),
                        metadata: workflow.metadata.as_deref().unwrap_or("").to_string(),
                        timestamp: workflow.timestamp.as_deref().unwrap_or("").to_string(),
                    })
                    .collect();
                display_table_with_count(&rows, "workflows");
            } else {
                println!("Workflows for user {}:", user);
                let rows: Vec<WorkflowTableRowNoUser> = workflows
                    .iter()
                    .map(|workflow| WorkflowTableRowNoUser {
                        id: workflow.id.unwrap_or(-1),
                        name: workflow.name.clone(),
                        description: workflow.description.as_deref().unwrap_or("").to_string(),
                        project: workflow.project.as_deref().unwrap_or("").to_string(),
                        metadata: workflow.metadata.as_deref().unwrap_or("").to_string(),
                        timestamp: workflow.timestamp.as_deref().unwrap_or("").to_string(),
                    })
                    .collect();
                display_table_with_count(&rows, "workflows");
            }
        }
        Err(e) => {
            print_error("listing workflows", &e);
            std::process::exit(1);
        }
    }
}

fn handle_new(
    config: &Configuration,
    name: &str,
    description: &Option<String>,
    user: &str,
    format: &str,
) {
    let mut workflow = models::WorkflowModel::new(name.to_string(), user.to_string());
    workflow.description = description.clone();

    match default_api::create_workflow(config, workflow) {
        Ok(created_workflow) => {
            if format == "json" {
                // Convert workflow to JSON value, parsing JSON string fields to objects
                let json = serde_json::to_value(&created_workflow).unwrap();
                let json = parse_json_fields(json);

                match serde_json::to_string_pretty(&json) {
                    Ok(json_str) => println!("{}", json_str),
                    Err(e) => {
                        eprintln!("Error serializing workflow to JSON: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                println!("Successfully created workflow:");
                println!("  ID: {}", created_workflow.id.unwrap_or(-1));
                println!("  Name: {}", created_workflow.name);
                println!("  User: {}", created_workflow.user);
                if let Some(desc) = created_workflow.description {
                    println!("  Description: {}", desc);
                }
            }
        }
        Err(e) => {
            print_error("creating workflow", &e);
            std::process::exit(1);
        }
    }
}

fn handle_create(
    config: &Configuration,
    file: &str,
    user: &str,
    no_resource_monitoring: bool,
    skip_checks: bool,
    dry_run: bool,
    format: &str,
) {
    // Handle dry-run mode
    if dry_run {
        let result = WorkflowSpec::validate_spec(file);

        if format == "json" {
            match serde_json::to_string_pretty(&result) {
                Ok(json) => println!("{}", json),
                Err(e) => {
                    eprintln!("Error serializing validation result: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            // Human-readable output
            println!("Workflow Validation Results");
            println!("===========================");
            println!();

            let summary = &result.summary;
            println!("Workflow: {}", summary.workflow_name);
            if let Some(ref desc) = summary.workflow_description {
                println!("Description: {}", desc);
            }
            println!();

            // Show what would be created
            println!("Components to be created:");
            if summary.job_count != summary.job_count_before_expansion {
                println!(
                    "  Jobs: {} (expanded from {} parameterized job specs)",
                    summary.job_count, summary.job_count_before_expansion
                );
            } else {
                println!("  Jobs: {}", summary.job_count);
            }
            if summary.file_count != summary.file_count_before_expansion {
                println!(
                    "  Files: {} (expanded from {} parameterized file specs)",
                    summary.file_count, summary.file_count_before_expansion
                );
            } else {
                println!("  Files: {}", summary.file_count);
            }
            println!("  User data records: {}", summary.user_data_count);
            println!(
                "  Resource requirements: {}",
                summary.resource_requirements_count
            );
            println!("  Slurm schedulers: {}", summary.slurm_scheduler_count);
            println!("  Workflow actions: {}", summary.action_count);
            println!();

            if summary.has_schedule_nodes_action {
                println!(
                    "Submission: Ready for scheduler submission (has on_workflow_start schedule_nodes action)"
                );
            } else {
                println!(
                    "Submission: Local execution only (no on_workflow_start schedule_nodes action)"
                );
            }
            println!();

            // Show errors
            if !result.errors.is_empty() {
                eprintln!("Errors ({}):", result.errors.len());
                for error in &result.errors {
                    eprintln!("  - {}", error);
                }
                eprintln!();
            }

            // Show warnings
            if !result.warnings.is_empty() {
                eprintln!("Warnings ({}):", result.warnings.len());
                for warning in &result.warnings {
                    eprintln!("  - {}", warning);
                }
                eprintln!();
            }

            // Final verdict
            if result.valid {
                if result.warnings.is_empty() {
                    println!("Validation: PASSED");
                } else {
                    println!(
                        "Validation: PASSED (with {} warning(s))",
                        result.warnings.len()
                    );
                }
            } else {
                eprintln!("Validation: FAILED");
            }
        }

        // Exit with appropriate code
        if !result.valid {
            std::process::exit(1);
        }
        return;
    }

    // Normal create mode
    match WorkflowSpec::create_workflow_from_spec(
        config,
        file,
        user,
        !no_resource_monitoring,
        skip_checks,
    ) {
        Ok(workflow_id) => {
            if format == "json" {
                let json_output = serde_json::json!({
                    "workflow_id": workflow_id,
                    "status": "success",
                    "message": format!("Workflow created successfully with ID: {}", workflow_id)
                });
                match serde_json::to_string_pretty(&json_output) {
                    Ok(json) => println!("{}", json),
                    Err(e) => {
                        eprintln!("Error serializing JSON: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                println!("Created workflow {}", workflow_id);
            }
        }
        Err(e) => {
            eprintln!("Error creating workflow from spec: {}", e);
            std::process::exit(1);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_create_slurm(
    config: &Configuration,
    file: &str,
    account: Option<&str>,
    hpc_profile: Option<&str>,
    single_allocation: bool,
    group_by: GroupByStrategy,
    no_resource_monitoring: bool,
    skip_checks: bool,
    dry_run: bool,
    format: &str,
) {
    let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    // Handle dry-run mode first
    if dry_run {
        let result = WorkflowSpec::validate_spec(file);
        if format == "json" {
            match serde_json::to_string_pretty(&result) {
                Ok(json) => println!("{}", json),
                Err(e) => {
                    eprintln!("Error serializing validation result: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            println!("Workflow Validation Results (with Slurm scheduler generation)");
            println!("==============================================================");
            println!();
            println!("Note: Dry-run validates the spec before scheduler generation.");
            println!("Use 'torc slurm generate' to preview generated schedulers.");
            println!();

            let summary = &result.summary;
            println!("Workflow: {}", summary.workflow_name);
            println!("Jobs: {}", summary.job_count);
            println!(
                "Resource requirements: {}",
                summary.resource_requirements_count
            );
            println!();

            if !result.errors.is_empty() {
                eprintln!("Errors ({}):", result.errors.len());
                for error in &result.errors {
                    eprintln!("  - {}", error);
                }
            }

            if result.valid {
                println!("Validation: PASSED");
            } else {
                eprintln!("Validation: FAILED");
            }
        }

        if !result.valid {
            std::process::exit(1);
        }
        return;
    }

    // Load HPC config and registry
    let torc_config = TorcConfig::load().unwrap_or_default();
    let registry = create_registry_with_config_public(&torc_config.client.hpc);

    // Get the HPC profile
    let profile = if let Some(name) = hpc_profile {
        registry.get(name)
    } else {
        registry.detect()
    };

    let profile = match profile {
        Some(p) => p,
        None => {
            if let Some(name) = hpc_profile {
                eprintln!("Unknown HPC profile: {}", name);
            } else {
                eprintln!("No HPC profile specified and no system detected.");
                eprintln!("Use --hpc-profile <name> to specify a profile.");
            }
            std::process::exit(1);
        }
    };

    // Parse the workflow spec
    let mut spec = match WorkflowSpec::from_spec_file(file) {
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
    // Don't allow force=true - if schedulers already exist, user should use the _no_slurm variant
    match generate_schedulers_for_workflow(
        &mut spec,
        profile,
        &resolved_account,
        single_allocation,
        group_by,
        WalltimeStrategy::MaxJobRuntime,
        1.5, // Default walltime multiplier
        true,
        false,
    ) {
        Ok(result) => {
            if format != "json" {
                eprintln!(
                    "Auto-generated {} scheduler(s) and {} action(s) using {} profile",
                    result.scheduler_count, result.action_count, profile.name
                );
                for warning in &result.warnings {
                    eprintln!("  Warning: {}", warning);
                }
            }
        }
        Err(e) => {
            eprintln!("Error generating schedulers: {}", e);
            std::process::exit(1);
        }
    }

    // Write modified spec to temp file
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("torc_workflow_{}.yaml", std::process::id()));
    match std::fs::write(&temp_file, serde_yaml::to_string(&spec).unwrap()) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Failed to write temporary workflow file: {}", e);
            std::process::exit(1);
        }
    }

    // Create workflow from modified spec
    match WorkflowSpec::create_workflow_from_spec(
        config,
        &temp_file,
        &user,
        !no_resource_monitoring,
        skip_checks,
    ) {
        Ok(workflow_id) => {
            if format == "json" {
                let json_output = serde_json::json!({
                    "workflow_id": workflow_id,
                    "status": "success",
                    "message": format!("Workflow created successfully with ID: {}", workflow_id)
                });
                match serde_json::to_string_pretty(&json_output) {
                    Ok(json) => println!("{}", json),
                    Err(e) => {
                        eprintln!("Error serializing JSON: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                println!("Created workflow {}", workflow_id);
            }
        }
        Err(e) => {
            eprintln!("Error creating workflow from spec: {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_is_complete(config: &Configuration, id: Option<i64>, format: &str) {
    // Get or select workflow ID
    let user = get_env_user_name();
    let id = match id {
        Some(id) => id,
        None => match select_workflow_interactively(config, &user) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("Error selecting workflow: {}", e);
                std::process::exit(1);
            }
        },
    };

    match default_api::is_workflow_complete(config, id) {
        Ok(response) => {
            if format == "json" {
                match serde_json::to_string_pretty(&response) {
                    Ok(json) => println!("{}", json),
                    Err(e) => {
                        eprintln!("Error serializing response to JSON: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                println!("Workflow {} completion status:", id);
                println!("  Is Complete: {}", response.is_complete);
                println!("  Is Canceled: {}", response.is_canceled);
            }
        }
        Err(e) => {
            print_error("checking workflow completion", &e);
            std::process::exit(1);
        }
    }
}

fn handle_sync_status(
    config: &Configuration,
    workflow_id: Option<i64>,
    dry_run: bool,
    current_user: &str,
    format: &str,
) {
    // Get workflow ID (prompt if not provided)
    let workflow_id = match workflow_id {
        Some(id) => id,
        None => match select_workflow_interactively(config, current_user) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("Error selecting workflow: {}", e);
                std::process::exit(1);
            }
        },
    };

    // Status messages go to stderr so they don't pollute JSON output
    if dry_run {
        eprintln!(
            "[DRY RUN] Checking for orphaned jobs in workflow {}...",
            workflow_id
        );
    } else {
        eprintln!("Synchronizing job statuses for workflow {}...", workflow_id);
    }

    match super::orphan_detection::cleanup_orphaned_jobs(config, workflow_id, dry_run) {
        Ok(result) => {
            if format == "json" {
                match serde_json::to_string_pretty(&result) {
                    Ok(json) => println!("{}", json),
                    Err(e) => {
                        eprintln!("Error serializing result to JSON: {}", e);
                        std::process::exit(1);
                    }
                }
            } else if result.any_cleaned() {
                let action = if dry_run {
                    "Would clean up"
                } else {
                    "Cleaned up"
                };
                println!("\n{} orphaned jobs:", action);
                if result.slurm_jobs_failed > 0 {
                    println!(
                        "  - {} job(s) from terminated Slurm allocations",
                        result.slurm_jobs_failed
                    );
                }
                if result.pending_allocations_cleaned > 0 {
                    println!(
                        "  - {} pending allocation(s) that no longer exist in Slurm",
                        result.pending_allocations_cleaned
                    );
                }
                if result.running_jobs_failed > 0 {
                    println!(
                        "  - {} job(s) stuck in running with no active compute nodes",
                        result.running_jobs_failed
                    );
                }

                if !result.failed_job_details.is_empty() {
                    println!("\nAffected jobs:");
                    for detail in &result.failed_job_details {
                        // Use a simplified reason when Slurm job ID is available to avoid
                        // redundant output like "Slurm job 12345 no longer running (Slurm job 12345)"
                        let (reason, slurm_info) = if let Some(id) = detail.slurm_job_id.as_ref() {
                            ("Allocation terminated", format!(" (Slurm job {})", id))
                        } else {
                            (detail.reason.as_str(), String::new())
                        };
                        println!(
                            "  - Job {} ({}): {}{}",
                            detail.job_id, detail.job_name, reason, slurm_info
                        );
                    }
                }

                if !dry_run {
                    println!(
                        "\nTotal: {} job(s) marked as failed",
                        result.total_jobs_failed()
                    );
                    println!(
                        "\nYou can now run `torc recover {}` to retry failed jobs.",
                        workflow_id
                    );
                }
            } else {
                println!("No orphaned jobs found. Workflow state is in sync with Slurm.");
            }
        }
        Err(e) => {
            eprintln!("Error synchronizing job statuses: {}", e);
            std::process::exit(1);
        }
    }
}

pub fn handle_workflow_commands(config: &Configuration, command: &WorkflowCommands, format: &str) {
    // Get the current user from environment
    let current_user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string());

    match command {
        WorkflowCommands::Create {
            file,
            no_resource_monitoring,
            skip_checks,
            dry_run,
        } => {
            handle_create(
                config,
                file,
                &current_user,
                *no_resource_monitoring,
                *skip_checks,
                *dry_run,
                format,
            );
        }
        WorkflowCommands::CreateSlurm {
            file,
            account,
            hpc_profile,
            single_allocation,
            group_by,
            no_resource_monitoring,
            skip_checks,
            dry_run,
        } => {
            handle_create_slurm(
                config,
                file,
                account.as_deref(),
                hpc_profile.as_deref(),
                *single_allocation,
                *group_by,
                *no_resource_monitoring,
                *skip_checks,
                *dry_run,
                format,
            );
        }
        WorkflowCommands::New { name, description } => {
            handle_new(config, name, description, &current_user, format);
        }
        WorkflowCommands::List {
            limit,
            offset,
            sort_by,
            reverse_sort,
            archived_only,
            include_archived,
            all_users,
        } => {
            handle_list(
                config,
                &current_user,
                *limit,
                *offset,
                sort_by,
                *reverse_sort,
                *archived_only,
                *include_archived,
                *all_users,
                format,
            );
        }
        WorkflowCommands::Get { id } => {
            handle_get(config, id, &current_user, format);
        }
        WorkflowCommands::Update {
            id,
            name,
            description,
            owner_user,
            project,
            metadata,
        } => {
            let updates = WorkflowUpdateFields {
                name: name.clone(),
                description: description.clone(),
                owner_user: owner_user.clone(),
                project: project.clone(),
                metadata: metadata.clone(),
            };
            handle_update(config, id, &updates, format);
        }
        WorkflowCommands::Delete { ids, no_prompts } => {
            handle_delete(config, ids, *no_prompts, format);
        }
        WorkflowCommands::Archive {
            is_archived,
            workflow_ids,
        } => {
            handle_archive(config, is_archived, workflow_ids, format);
        }
        WorkflowCommands::Submit { workflow_id, force } => {
            handle_submit(config, workflow_id, *force, format);
        }
        WorkflowCommands::Run {
            workflow_id,
            poll_interval,
            max_parallel_jobs,
            output_dir,
        } => {
            handle_run(
                config,
                workflow_id,
                *poll_interval,
                *max_parallel_jobs,
                output_dir,
            );
        }
        WorkflowCommands::Initialize {
            workflow_id,
            force,
            no_prompts,
            dry_run,
        } => {
            handle_initialize(config, workflow_id, *force, *no_prompts, *dry_run, format);
        }
        WorkflowCommands::Reinitialize {
            workflow_id,
            force,
            dry_run,
        } => {
            handle_reinitialize(config, workflow_id, *force, *dry_run, format);
        }
        WorkflowCommands::Status { workflow_id } => {
            handle_status(config, workflow_id, &current_user, format);
        }
        WorkflowCommands::ResetStatus {
            workflow_id,
            failed_only,
            reinitialize,
            force,
            no_prompts,
        } => {
            handle_reset_status(
                config,
                workflow_id,
                *failed_only,
                *reinitialize,
                *force,
                *no_prompts,
                format,
            );
        }
        WorkflowCommands::CorrectResources {
            workflow_id,
            memory_multiplier,
            runtime_multiplier,
            job_ids,
            dry_run,
        } => {
            handle_correct_resources(
                config,
                workflow_id,
                *memory_multiplier,
                *runtime_multiplier,
                job_ids,
                *dry_run,
                format,
            );
        }
        WorkflowCommands::Cancel { workflow_id } => {
            handle_cancel(config, workflow_id, format);
        }
        WorkflowCommands::ExecutionPlan { spec_or_id } => {
            handle_execution_plan(config, spec_or_id, format);
        }
        WorkflowCommands::ListActions { workflow_id } => {
            handle_list_actions(config, workflow_id, &current_user, format);
        }
        WorkflowCommands::IsComplete { id } => {
            handle_is_complete(config, *id, format);
        }
        WorkflowCommands::Export {
            workflow_id,
            output,
            include_results,
            include_events,
        } => {
            handle_export(
                config,
                workflow_id,
                output.as_deref(),
                *include_results,
                *include_events,
                &current_user,
                format,
            );
        }
        WorkflowCommands::Import {
            file,
            name,
            skip_results,
            skip_events,
        } => {
            handle_import(
                config,
                file,
                name.as_deref(),
                *skip_results,
                *skip_events,
                &current_user,
                format,
            );
        }
        WorkflowCommands::SyncStatus {
            workflow_id,
            dry_run,
        } => {
            handle_sync_status(config, *workflow_id, *dry_run, &current_user, format);
        }
    }
}

// ============================================================================
// Export/Import Implementation
// ============================================================================

fn handle_export(
    config: &Configuration,
    workflow_id: &Option<i64>,
    output: Option<&str>,
    include_results: bool,
    include_events: bool,
    current_user: &str,
    format: &str,
) {
    // Get workflow ID (prompt if not provided)
    let workflow_id = match workflow_id {
        Some(id) => *id,
        None => match select_workflow_interactively(config, current_user) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("Error selecting workflow: {}", e);
                std::process::exit(1);
            }
        },
    };

    // Get workflow
    let workflow = match default_api::get_workflow(config, workflow_id) {
        Ok(w) => w,
        Err(e) => {
            print_error("getting workflow", &e);
            std::process::exit(1);
        }
    };

    let workflow_name = workflow.name.clone();

    // Build export document
    let mut export = WorkflowExport::new(workflow);

    // Get all files
    let file_params = FileListParams {
        workflow_id,
        ..Default::default()
    };
    export.files = match paginate_files(config, workflow_id, file_params) {
        Ok(files) => files,
        Err(e) => {
            print_error("listing files", &e);
            std::process::exit(1);
        }
    };

    // Get all user_data
    let user_data_params = UserDataListParams {
        workflow_id,
        ..Default::default()
    };
    export.user_data = match paginate_user_data(config, workflow_id, user_data_params) {
        Ok(ud) => ud,
        Err(e) => {
            print_error("listing user_data", &e);
            std::process::exit(1);
        }
    };

    // Get all resource requirements
    let rr_params = ResourceRequirementsListParams {
        workflow_id,
        ..Default::default()
    };
    export.resource_requirements =
        match paginate_resource_requirements(config, workflow_id, rr_params) {
            Ok(rr) => rr,
            Err(e) => {
                print_error("listing resource requirements", &e);
                std::process::exit(1);
            }
        };

    // Get all slurm schedulers
    let slurm_params = SlurmSchedulersListParams {
        workflow_id,
        ..Default::default()
    };
    export.slurm_schedulers = match paginate_slurm_schedulers(config, workflow_id, slurm_params) {
        Ok(s) => s,
        Err(e) => {
            print_error("listing slurm schedulers", &e);
            std::process::exit(1);
        }
    };

    // Get all local schedulers
    export.local_schedulers = match default_api::list_local_schedulers(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
    ) {
        Ok(response) => response.items.unwrap_or_default(),
        Err(e) => {
            print_error("listing local schedulers", &e);
            std::process::exit(1);
        }
    };

    // Get all failure handlers
    export.failure_handlers =
        match default_api::list_failure_handlers(config, workflow_id, None, None) {
            Ok(response) => response.items.unwrap_or_default(),
            Err(e) => {
                print_error("listing failure handlers", &e);
                std::process::exit(1);
            }
        };

    // Get all jobs (with relationships)
    let job_params = JobListParams {
        workflow_id,
        include_relationships: Some(true),
        ..Default::default()
    };
    export.jobs = match paginate_jobs(config, workflow_id, job_params) {
        Ok(jobs) => jobs,
        Err(e) => {
            print_error("listing jobs", &e);
            std::process::exit(1);
        }
    };

    // Get workflow actions
    export.workflow_actions = match default_api::get_workflow_actions(config, workflow_id) {
        Ok(actions) => actions,
        Err(e) => {
            print_error("getting workflow actions", &e);
            std::process::exit(1);
        }
    };

    // Optionally get results
    if include_results {
        let result_params = ResultListParams {
            workflow_id,
            all_runs: Some(true), // Include results from all runs
            ..Default::default()
        };
        export.results = match paginate_results(config, workflow_id, result_params) {
            Ok(results) => Some(results),
            Err(e) => {
                print_error("listing results", &e);
                std::process::exit(1);
            }
        };
    }

    // Optionally get events
    if include_events {
        let event_params = EventListParams {
            workflow_id,
            ..Default::default()
        };
        export.events = match paginate_events(config, workflow_id, event_params) {
            Ok(events) => Some(events),
            Err(e) => {
                print_error("listing events", &e);
                std::process::exit(1);
            }
        };
    }

    // Serialize to JSON
    let json = match serde_json::to_string_pretty(&export) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("Error serializing export: {}", e);
            std::process::exit(1);
        }
    };

    // Calculate stats
    let stats = ExportImportStats::from_export(&export);

    // Write output
    match output {
        Some(path) => {
            if let Err(e) = fs::write(path, &json) {
                eprintln!("Error writing to file: {}", e);
                std::process::exit(1);
            }
            if format == "json" {
                println!(
                    "{}",
                    serde_json::json!({
                        "success": true,
                        "workflow_id": workflow_id,
                        "workflow_name": workflow_name,
                        "output_file": path,
                        "jobs": stats.jobs,
                        "files": stats.files,
                        "user_data": stats.user_data,
                        "results": stats.results,
                        "events": stats.events,
                    })
                );
            } else {
                eprintln!(
                    "Exported workflow '{}' ({} jobs, {} files) to {}",
                    workflow_name, stats.jobs, stats.files, path
                );
            }
        }
        None => {
            // Write to stdout
            println!("{}", json);
        }
    }
}

fn handle_import(
    config: &Configuration,
    file: &str,
    name_override: Option<&str>,
    skip_results: bool,
    _skip_events: bool, // Events import not yet implemented
    current_user: &str,
    format: &str,
) {
    // Read input
    let json = if file == "-" {
        let mut buffer = String::new();
        if let Err(e) = io::stdin().read_to_string(&mut buffer) {
            eprintln!("Error reading from stdin: {}", e);
            std::process::exit(1);
        }
        buffer
    } else {
        match fs::read_to_string(file) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Error reading file '{}': {}", file, e);
                std::process::exit(1);
            }
        }
    };

    // Parse export document
    let export: WorkflowExport = match serde_json::from_str(&json) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error parsing export file: {}", e);
            std::process::exit(1);
        }
    };

    // Check version compatibility
    if export.export_version != EXPORT_VERSION {
        eprintln!(
            "Warning: Export version {} differs from current version {}",
            export.export_version, EXPORT_VERSION
        );
    }

    let mut mappings = IdMappings::new();

    // Create workflow with optional name override
    let mut new_workflow = export.workflow.clone();
    new_workflow.id = None; // Clear ID for creation
    if let Some(name) = name_override {
        new_workflow.name = name.to_string();
    }
    new_workflow.user = current_user.to_string(); // Set current user as owner

    let created_workflow = match default_api::create_workflow(config, new_workflow) {
        Ok(w) => w,
        Err(e) => {
            print_error("creating workflow", &e);
            std::process::exit(1);
        }
    };
    let new_workflow_id = created_workflow.id.unwrap();
    let workflow_name = created_workflow.name.clone();

    // Create files and build mapping
    for file_model in &export.files {
        let mut new_file = file_model.clone();
        let old_id = new_file.id.unwrap();
        new_file.id = None;
        new_file.workflow_id = new_workflow_id;

        match default_api::create_file(config, new_file) {
            Ok(created) => {
                mappings.files.insert(old_id, created.id.unwrap());
            }
            Err(e) => {
                print_error("creating file", &e);
                // Clean up: delete the workflow we just created
                let _ = default_api::delete_workflow(config, new_workflow_id, None);
                std::process::exit(1);
            }
        }
    }

    // Create user_data and build mapping
    // Note: consumer_job_id and producer_job_id relationships are established
    // via job's input_user_data_ids and output_user_data_ids, not here
    for ud_model in &export.user_data {
        let mut new_ud = ud_model.clone();
        let old_id = new_ud.id.unwrap();
        new_ud.id = None;
        new_ud.workflow_id = new_workflow_id;

        match default_api::create_user_data(config, new_ud, None, None) {
            Ok(created) => {
                mappings.user_data.insert(old_id, created.id.unwrap());
            }
            Err(e) => {
                print_error("creating user_data", &e);
                let _ = default_api::delete_workflow(config, new_workflow_id, None);
                std::process::exit(1);
            }
        }
    }

    // Create resource requirements and build mapping
    // First, get the 'default' resource requirement that was auto-created with the workflow
    // so we can map old 'default' IDs to it
    let default_rr = default_api::list_resource_requirements(
        config,
        new_workflow_id,
        None,            // job_id
        None,            // offset
        None,            // limit
        None,            // sort_by
        None,            // reverse_sort
        Some("default"), // name
        None,            // memory
        None,            // num_cpus
        None,            // num_gpus
        None,            // num_nodes
        None,            // runtime
    )
    .ok()
    .and_then(|response| response.items)
    .and_then(|items| items.into_iter().next());

    for rr_model in &export.resource_requirements {
        let old_id = rr_model.id.unwrap();

        // Skip 'default' resource requirements since they're auto-created,
        // but map the old ID to the new workflow's default ID
        if rr_model.name == "default" {
            if let Some(ref default) = default_rr {
                mappings
                    .resource_requirements
                    .insert(old_id, default.id.unwrap());
            } else {
                eprintln!(
                    "Warning: Default resource requirement for workflow {} could not be found; \
                     exported 'default' resource requirement with old ID {} will not be mapped.",
                    new_workflow_id, old_id
                );
            }
            continue;
        }

        let mut new_rr = rr_model.clone();
        new_rr.id = None;
        new_rr.workflow_id = new_workflow_id;

        match default_api::create_resource_requirements(config, new_rr) {
            Ok(created) => {
                mappings
                    .resource_requirements
                    .insert(old_id, created.id.unwrap());
            }
            Err(e) => {
                print_error("creating resource requirements", &e);
                let _ = default_api::delete_workflow(config, new_workflow_id, None);
                std::process::exit(1);
            }
        }
    }

    // Create slurm schedulers and build mapping
    for scheduler in &export.slurm_schedulers {
        let mut new_scheduler = scheduler.clone();
        let old_id = new_scheduler.id.unwrap();
        new_scheduler.id = None;
        new_scheduler.workflow_id = new_workflow_id;

        match default_api::create_slurm_scheduler(config, new_scheduler) {
            Ok(created) => {
                mappings
                    .slurm_schedulers
                    .insert(old_id, created.id.unwrap());
            }
            Err(e) => {
                print_error("creating slurm scheduler", &e);
                let _ = default_api::delete_workflow(config, new_workflow_id, None);
                std::process::exit(1);
            }
        }
    }

    // Create local schedulers and build mapping
    for scheduler in &export.local_schedulers {
        let mut new_scheduler = scheduler.clone();
        let old_id = new_scheduler.id.unwrap();
        new_scheduler.id = None;
        new_scheduler.workflow_id = new_workflow_id;

        match default_api::create_local_scheduler(config, new_scheduler) {
            Ok(created) => {
                mappings
                    .local_schedulers
                    .insert(old_id, created.id.unwrap());
            }
            Err(e) => {
                print_error("creating local scheduler", &e);
                let _ = default_api::delete_workflow(config, new_workflow_id, None);
                std::process::exit(1);
            }
        }
    }

    // Create failure handlers and build mapping
    for handler in &export.failure_handlers {
        let mut new_handler = handler.clone();
        let old_id = new_handler.id.unwrap();
        new_handler.id = None;
        new_handler.workflow_id = new_workflow_id;

        match default_api::create_failure_handler(config, new_handler) {
            Ok(created) => {
                mappings
                    .failure_handlers
                    .insert(old_id, created.id.unwrap());
            }
            Err(e) => {
                print_error("creating failure handler", &e);
                let _ = default_api::delete_workflow(config, new_workflow_id, None);
                std::process::exit(1);
            }
        }
    }

    // Create jobs with remapped IDs (but without depends_on_job_ids yet)
    for job_model in &export.jobs {
        let mut new_job = job_model.clone();
        let old_id = new_job.id.unwrap();
        new_job.id = None;
        new_job.workflow_id = new_workflow_id;

        // Remap file IDs
        if let Some(ref ids) = new_job.input_file_ids {
            new_job.input_file_ids = Some(mappings.remap_file_ids(ids));
        }
        if let Some(ref ids) = new_job.output_file_ids {
            new_job.output_file_ids = Some(mappings.remap_file_ids(ids));
        }

        // Remap user_data IDs
        if let Some(ref ids) = new_job.input_user_data_ids {
            new_job.input_user_data_ids = Some(mappings.remap_user_data_ids(ids));
        }
        if let Some(ref ids) = new_job.output_user_data_ids {
            new_job.output_user_data_ids = Some(mappings.remap_user_data_ids(ids));
        }

        // Remap resource_requirements_id
        if let Some(rr_id) = new_job.resource_requirements_id {
            new_job.resource_requirements_id = mappings.remap_resource_requirements_id(rr_id);
        }

        // Remap scheduler_id
        if let Some(sched_id) = new_job.scheduler_id {
            new_job.scheduler_id = mappings.remap_scheduler_id(sched_id);
        }

        // Remap failure_handler_id
        if let Some(fh_id) = new_job.failure_handler_id {
            new_job.failure_handler_id = mappings.remap_failure_handler_id(fh_id);
        }

        // Clear depends_on_job_ids - we'll set these after all jobs are created
        new_job.depends_on_job_ids = None;

        // Always reset status to uninitialized - status is computed by server
        // based on dependencies and cannot be preserved through update_job API
        new_job.status = Some(JobStatus::Uninitialized);

        match default_api::create_job(config, new_job) {
            Ok(created) => {
                mappings.jobs.insert(old_id, created.id.unwrap());
            }
            Err(e) => {
                print_error("creating job", &e);
                let _ = default_api::delete_workflow(config, new_workflow_id, None);
                std::process::exit(1);
            }
        }
    }

    // Now update jobs with their depends_on_job_ids
    // This must be done while jobs are in Uninitialized status (server constraint)
    for job_model in &export.jobs {
        if let Some(ref depends_on) = job_model.depends_on_job_ids
            && !depends_on.is_empty()
        {
            let old_job_id = job_model.id.unwrap();
            let new_job_id = mappings.jobs.get(&old_job_id).unwrap();
            let new_depends_on = mappings.remap_job_ids(depends_on);

            // Create update request preserving the job's name/command
            // Keep status as Uninitialized so depends_on can be modified
            let mut update_job = models::JobModel::new(
                new_workflow_id,
                job_model.name.clone(),
                job_model.command.clone(),
            );
            update_job.depends_on_job_ids = Some(new_depends_on);
            // Keep Uninitialized so we can modify depends_on_job_ids
            update_job.status = Some(JobStatus::Uninitialized);

            if let Err(e) = default_api::update_job(config, *new_job_id, update_job) {
                print_error("updating job dependencies", &e);
                let _ = default_api::delete_workflow(config, new_workflow_id, None);
                std::process::exit(1);
            }
        }
    }

    // Create workflow actions with remapped scheduler IDs
    for action in &export.workflow_actions {
        let mut new_action = action.clone();
        new_action.id = None;
        new_action.workflow_id = new_workflow_id;

        // Remap scheduler_id in action_config if present
        // The action_config may contain a scheduler_id field for schedule_nodes actions
        if let Some(obj) = new_action.action_config.as_object_mut()
            && let Some(serde_json::Value::Number(n)) = obj.get("scheduler_id")
            && let Some(old_id) = n.as_i64()
            && let Some(new_id) = mappings.remap_scheduler_id(old_id)
        {
            obj.insert(
                "scheduler_id".to_string(),
                serde_json::Value::Number(new_id.into()),
            );
        }

        // Remap job_ids in the action if present
        if let Some(ref job_ids) = new_action.job_ids {
            new_action.job_ids = Some(mappings.remap_job_ids(job_ids));
        }

        // Serialize to JSON Value for the API
        let action_json = serde_json::to_value(&new_action).unwrap_or_default();

        if let Err(e) = default_api::create_workflow_action(config, new_workflow_id, action_json) {
            print_error("creating workflow action", &e);
            let _ = default_api::delete_workflow(config, new_workflow_id, None);
            std::process::exit(1);
        }
    }

    // Import results if present and not skipped
    if !skip_results && let Some(ref results) = export.results {
        for result in results {
            let mut new_result = result.clone();
            new_result.id = None;
            new_result.workflow_id = new_workflow_id;
            // Remap job_id - job_id is required (i64, not Option<i64>)
            if let Some(new_job_id) = mappings.remap_job_id(new_result.job_id) {
                new_result.job_id = new_job_id;
            } else {
                // Skip this result if we can't remap the job_id
                continue;
            }

            if let Err(e) = default_api::create_result(config, new_result) {
                print_error("creating result", &e);
                // Continue anyway - results are optional
            }
        }
    }

    // Note: Events are typically not imported as they represent historical data
    // that would be recreated by new operations on the imported workflow.
    // The skip_events flag exists for future use or special cases.

    // Calculate stats
    let stats = ExportImportStats::from_export(&export);

    if format == "json" {
        println!(
            "{}",
            serde_json::json!({
                "success": true,
                "workflow_id": new_workflow_id,
                "workflow_name": workflow_name,
                "jobs": stats.jobs,
                "files": stats.files,
                "user_data": stats.user_data,
            })
        );
    } else {
        eprintln!(
            "Imported workflow '{}' as ID {} ({} jobs, {} files, status reset)",
            workflow_name, new_workflow_id, stats.jobs, stats.files
        );
    }
}
