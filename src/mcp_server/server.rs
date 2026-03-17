//! MCP server implementation for Torc.

use rmcp::{
    Error as McpError, RoleServer, ServerHandler,
    model::{
        CallToolResult, Implementation, PaginatedRequestParam, ProtocolVersion, ReadResourceResult,
        ServerCapabilities, ServerInfo,
    },
    schemars, tool,
};
use serde::Deserialize;
use std::path::PathBuf;

use crate::client::apis::configuration::{Configuration, TlsConfig};

use super::tools;

/// MCP server that exposes Torc workflow operations as tools.
#[derive(Debug, Clone)]
pub struct TorcMcpServer {
    config: Configuration,
    output_dir: PathBuf,
    docs_dir: Option<PathBuf>,
    examples_dir: Option<PathBuf>,
}

impl TorcMcpServer {
    /// Create a new TorcMcpServer with the given API URL and output directory.
    pub fn new(api_url: String, output_dir: PathBuf) -> Self {
        Self::new_with_tls(api_url, output_dir, TlsConfig::default())
    }

    /// Create a new TorcMcpServer with TLS configuration.
    pub fn new_with_tls(api_url: String, output_dir: PathBuf, tls: TlsConfig) -> Self {
        let mut config = Configuration::with_tls(tls);
        config.base_path = api_url;

        Self {
            config,
            output_dir,
            docs_dir: None,
            examples_dir: None,
        }
    }

    /// Create a new TorcMcpServer with authentication.
    pub fn with_auth(
        api_url: String,
        output_dir: PathBuf,
        username: Option<String>,
        password: Option<String>,
    ) -> Self {
        Self::with_auth_and_tls(
            api_url,
            output_dir,
            username,
            password,
            TlsConfig::default(),
        )
    }

    /// Create a new TorcMcpServer with authentication and TLS configuration.
    pub fn with_auth_and_tls(
        api_url: String,
        output_dir: PathBuf,
        username: Option<String>,
        password: Option<String>,
        tls: TlsConfig,
    ) -> Self {
        let mut config = Configuration::with_tls(tls);
        config.base_path = api_url;

        if let (Some(user), Some(pass)) = (username, password) {
            config.basic_auth = Some((user, Some(pass)));
        }

        Self {
            config,
            output_dir,
            docs_dir: None,
            examples_dir: None,
        }
    }

    /// Set the documentation directory.
    pub fn with_docs_dir(mut self, docs_dir: Option<PathBuf>) -> Self {
        self.docs_dir = docs_dir;
        self
    }

    /// Set the examples directory.
    pub fn with_examples_dir(mut self, examples_dir: Option<PathBuf>) -> Self {
        self.examples_dir = examples_dir;
        self
    }
}

// Tool parameter types

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WorkflowIdParam {
    #[schemars(description = "The workflow ID")]
    pub workflow_id: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct JobIdParam {
    #[schemars(description = "The job ID")]
    pub job_id: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetJobLogsParams {
    #[schemars(description = "The workflow ID")]
    pub workflow_id: i64,
    #[schemars(description = "The job ID")]
    pub job_id: i64,
    #[schemars(description = "The run ID (1 for first run, increments on restart)")]
    pub run_id: i64,
    #[schemars(
        description = "The attempt ID (1 for first attempt, increments on retry). Defaults to 1."
    )]
    #[serde(default = "default_attempt_id")]
    pub attempt_id: i64,
    #[schemars(description = "Log type: 'stdout' or 'stderr'")]
    pub log_type: String,
    #[schemars(
        description = "Number of lines to return from the end (optional, returns all if not specified)"
    )]
    pub tail_lines: Option<usize>,
}

fn default_attempt_id() -> i64 {
    1
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListJobsByStatusParams {
    #[schemars(description = "The workflow ID")]
    pub workflow_id: i64,
    #[schemars(
        description = "Job status to filter by: 'uninitialized', 'blocked', 'ready', 'pending', 'running', 'completed', 'failed', 'canceled', 'terminated', 'disabled'"
    )]
    pub status: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateJobResourcesParams {
    #[schemars(description = "The job ID")]
    pub job_id: i64,
    #[schemars(description = "Number of CPUs (optional)")]
    pub num_cpus: Option<i64>,
    #[schemars(description = "Memory requirement, e.g., '4g', '512m' (optional)")]
    pub memory: Option<String>,
    #[schemars(
        description = "Runtime in ISO8601 duration format, e.g., 'PT30M', 'PT2H' (optional)"
    )]
    pub runtime: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateWorkflowParams {
    #[schemars(
        description = "Workflow specification as a JSON object (not a string). For Slurm workflows, must include a 'resource_requirements' section and each job must reference one."
    )]
    pub spec_json: serde_json::Value,
    #[schemars(description = "User that owns the workflow (optional, defaults to current user)")]
    pub user: Option<String>,
    #[schemars(
        description = "Action to perform: 'create_workflow' to create in the database, 'save_spec_file' to save to filesystem only, 'validate' to validate without creating"
    )]
    pub action: String,
    #[schemars(description = "Workflow type: 'local' for local execution, 'slurm' for Slurm HPC")]
    pub workflow_type: String,
    #[schemars(description = "Slurm account (required for slurm workflow_type)")]
    pub account: Option<String>,
    #[schemars(
        description = "HPC profile to use (optional, auto-detected if not specified). Required for slurm if auto-detection fails."
    )]
    pub hpc_profile: Option<String>,
    #[schemars(
        description = "Output file path for save_spec_file action (required for save_spec_file, use .json extension)"
    )]
    pub output_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CheckResourceUtilizationParams {
    #[schemars(description = "The workflow ID")]
    pub workflow_id: i64,
    #[schemars(
        description = "Include failed jobs in the analysis (recommended for recovery diagnostics)"
    )]
    pub include_failed: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetExecutionPlanParams {
    #[schemars(
        description = "Either a workflow ID (integer) to get plan for existing workflow, or a JSON workflow specification string to preview execution plan before creating"
    )]
    pub spec_or_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AnalyzeWorkflowLogsParams {
    #[schemars(description = "Workflow ID to analyze logs for")]
    pub workflow_id: i64,
    #[schemars(
        description = "Output directory where logs are stored (the same directory passed to `torc run`). Defaults to 'torc_output'."
    )]
    pub output_dir: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetWorkflowSummaryParams {
    #[schemars(description = "The workflow ID")]
    pub workflow_id: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListResultsParams {
    #[schemars(description = "The workflow ID")]
    pub workflow_id: i64,
    #[schemars(description = "Filter by job ID")]
    pub job_id: Option<i64>,
    #[schemars(description = "Filter by run ID")]
    pub run_id: Option<i64>,
    #[schemars(description = "Filter by return code (e.g., 0 for success, 1 for failure)")]
    pub return_code: Option<i64>,
    #[schemars(description = "Show only failed jobs (non-zero return code)")]
    pub failed_only: Option<bool>,
    #[schemars(
        description = "Filter by job status: completed, failed, terminated, canceled, etc."
    )]
    pub status: Option<String>,
    #[schemars(description = "Maximum number of results to return (default: 100)")]
    pub limit: Option<i64>,
    #[schemars(
        description = "Field to sort by: exec_time_minutes, peak_memory_bytes, peak_cpu_percent, return_code"
    )]
    pub sort_by: Option<String>,
    #[schemars(description = "Reverse the sort order (descending instead of ascending)")]
    pub reverse_sort: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSlurmSacctParams {
    #[schemars(description = "The workflow ID")]
    pub workflow_id: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RecoverWorkflowParams {
    #[schemars(description = "The workflow ID to recover")]
    pub workflow_id: i64,
    #[schemars(
        description = "If true, shows what would be done without making any changes. \
        ALWAYS use dry_run=true first to preview recovery actions, then confirm with user before running with dry_run=false."
    )]
    pub dry_run: bool,
    #[schemars(
        description = "Memory multiplier for OOM failures (default: 1.5 = 50% increase). \
        Jobs that failed due to OOM will have their memory increased by this factor."
    )]
    pub memory_multiplier: Option<f64>,
    #[schemars(
        description = "Runtime multiplier for timeout failures (default: 1.4 = 40% increase). \
        Jobs that timed out will have their runtime increased by this factor."
    )]
    pub runtime_multiplier: Option<f64>,
    #[schemars(
        description = "If true, also retry jobs with unknown failure causes (not OOM or timeout). \
        Default is false - only retry jobs with diagnosable resource issues."
    )]
    pub retry_unknown: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListPendingFailedJobsParams {
    #[schemars(description = "The workflow ID")]
    pub workflow_id: i64,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct FailureClassificationParam {
    #[schemars(description = "The job ID to classify")]
    pub job_id: i64,
    #[schemars(description = "The classification action: 'retry' or 'fail'")]
    pub action: String,
    #[schemars(description = "Optional new memory requirement (e.g., '8g')")]
    pub memory: Option<String>,
    #[schemars(description = "Optional new runtime (ISO8601 duration, e.g., 'PT2H')")]
    pub runtime: Option<String>,
    #[schemars(description = "Reason for the classification (for logging)")]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ClassifyAndResolveFailuresParams {
    #[schemars(description = "The workflow ID")]
    pub workflow_id: i64,
    #[schemars(description = "List of classifications for pending_failed jobs")]
    pub classifications: Vec<FailureClassificationParam>,
    #[schemars(
        description = "If true, shows what would be done without making any changes. \
        ALWAYS use dry_run=true first to preview classifications, then confirm with user before running with dry_run=false."
    )]
    pub dry_run: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AnalyzeResourceUsageParams {
    #[schemars(description = "The workflow ID")]
    pub workflow_id: i64,
    #[schemars(
        description = "If true, only include jobs with return_code=0 (successful). \
        If false (default), include all jobs with results."
    )]
    pub completed_only: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetExampleParams {
    #[schemars(
        description = "Name of the example to retrieve (e.g., 'diamond_workflow', 'hyperparameter_sweep')"
    )]
    pub name: String,
    #[schemars(
        description = "Preferred format: 'yaml' (default), 'json', or 'kdl'. Falls back to available format."
    )]
    pub format: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetDocsParams {
    #[schemars(description = "Documentation topic to retrieve. Topics: \
        'workflow-spec' (spec reference), 'dependencies' (job dependency types), \
        'parameterization' (parameter sweeps), 'slurm' (HPC/Slurm setup), \
        'job-states' (status lifecycle), 'actions' (workflow actions), \
        'failure-handlers' (error recovery rules), 'recovery' (automated recovery), \
        'ai-recovery' (AI-assisted failure classification), \
        'resource-monitoring' (CPU/memory monitoring), 'cli' (CLI reference), \
        'quick-start' (getting started), 'architecture' (system design), \
        'checkpointing' (job checkpointing), 'hpc-profiles' (HPC profile config), \
        'workflow-formats' (YAML/JSON/KDL formats), \
        'allocation-strategies' (single-large vs many-small Slurm allocations), \
        'tutorials' (list of available tutorials)")]
    pub topic: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PlanAllocationsParams {
    #[schemars(
        description = "Workflow specification as a JSON object (not a string). Must include 'resource_requirements' section with CPU, memory, and runtime for each job type."
    )]
    pub spec_json: serde_json::Value,
    #[schemars(description = "Slurm account to use for allocation estimates")]
    pub account: String,
    #[schemars(description = "Partition to target (optional, auto-selected if not specified)")]
    pub partition: Option<String>,
    #[schemars(
        description = "HPC profile to use (optional, auto-detected if not specified). Use when auto-detection fails."
    )]
    pub hpc_profile: Option<String>,
    #[schemars(
        description = "Skip sbatch --test-only probes (faster, uses heuristics only). Default: false"
    )]
    #[serde(default)]
    pub skip_test_only: bool,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ResourceGroupParam {
    #[schemars(description = "Memory requirement, e.g., '10g', '512m'")]
    pub memory: String,
    #[schemars(description = "Number of CPUs")]
    pub num_cpus: i64,
    #[schemars(description = "Runtime in ISO8601 duration format, e.g., 'PT2H', 'PT30M'")]
    pub runtime: String,
    #[schemars(description = "Number of GPUs (defaults to the job's current RR value, or 0)")]
    pub num_gpus: Option<i64>,
    #[schemars(description = "Number of nodes (defaults to the job's current RR value, or 1)")]
    pub num_nodes: Option<i64>,
    #[schemars(description = "Name for this resource group (auto-generated if not provided)")]
    pub name: Option<String>,
    #[schemars(description = "Job IDs to assign to this resource group")]
    pub job_ids: Vec<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RegroupJobResourcesParams {
    #[schemars(description = "The workflow ID")]
    pub workflow_id: i64,
    #[schemars(description = "List of new resource groups with job assignments. \
        Each group defines resource requirements and which jobs belong to it.")]
    pub groups: Vec<ResourceGroupParam>,
    #[schemars(
        description = "If true, shows what would be done without making any changes. \
        ALWAYS use dry_run=true first to preview the regrouping, then confirm with user before running with dry_run=false."
    )]
    pub dry_run: bool,
}

// Tool implementations using #[tool(tool_box)]
// Tools are ordered by workflow lifecycle: create → plan → inspect → monitor → analyze → fix

#[tool(tool_box)]
impl TorcMcpServer {
    /// Create a workflow from a specification.
    #[tool(description = r#"Create a workflow specification file or workflow.

IMPORTANT - DEFAULT TO SAVING FILES, NOT CREATING WORKFLOWS:
- AI-generated specs are TEMPLATES with placeholder commands - users must customize them
- ALWAYS use action="save_spec_file" unless user explicitly says "run", "submit", or "execute"
- "create a workflow" or "create a workflow file" -> save_spec_file (user wants a file to edit)
- "run this workflow" or "submit to slurm" -> create_workflow (user wants immediate execution)
- Ask user for output filename if not specified (suggest: workflow_name.json in current directory)

CRITICAL: When user mentions FILES or DATA FLOW -> use "files" section with input_files/output_files on jobs.

ACTIONS:
- "validate" - Check spec for errors without saving/creating (use first to catch issues)
- "save_spec_file" - DEFAULT: Save spec to a .json file for user to review/edit before running
- "create_workflow" - ONLY when user explicitly wants to run/submit immediately

BEFORE CREATING THE SPEC - ask the user:
- "Will you run this on a Slurm HPC cluster or locally?" (if not already clear from context)
- If Slurm: ask for the Slurm account/allocation name, then use workflow_type="slurm"
- If local: use workflow_type="local"
This determines the workflow_type AND the CLI commands you suggest afterward.

AFTER SAVING A SPEC FILE - tell users:
1. Edit the spec to replace placeholder commands with actual scripts/commands
2. Ensure input files exist at the specified paths
3. How to run depends on the workflow_type:
   - LOCAL workflows: "torc run <file>"
   - SLURM workflows (spec saved with workflow_type="slurm"):
     - "torc submit <file>" (uses schedulers already generated in the spec)
     - Or create and submit separately: "torc workflows create-slurm --account <acct> <file>" then "torc workflows submit <id>"
IMPORTANT: Do NOT fabricate CLI commands or options. Only use the exact commands shown above.

WORKFLOW_TYPE: "local" or "slurm" (slurm requires account)

SPEC STRUCTURE:
{
  "name": "workflow_name",
  "files": [
    {"name": "input_data", "path": "input.txt", "st_mtime": 1234567890.0},
    {"name": "output_data", "path": "output.txt", "st_mtime": null}
  ],
  "jobs": [
    {"name": "job1", "command": "cmd", "input_files": ["input_data"], "output_files": ["output_data"], "resource_requirements": "small"},
    {"name": "job2_{i}", "command": "work {i}", "input_files": ["output_data"], "resource_requirements": "large", "parameters": {"i": "0:9"}}
  ],
  "resource_requirements": [
    {"name": "small", "num_cpus": 1, "memory": "4g", "runtime": "PT1H", "num_gpus": 0, "num_nodes": 1}
  ]
}

FILES (use when user mentions input/output files, data flow):
- "files": define with name, path, st_mtime (null for outputs, timestamp for inputs)
- "input_files": exact file names job reads (creates automatic dependency on producer job)
- "output_files": exact file names job writes
- "input_file_regexes": regex patterns for FAN-IN (collecting many files into one job)
- Files with parameters: {"name": "out_{i}", "path": "out_{i}.txt", "st_mtime": null, "parameters": {"i": "0:9"}}

PARAMETERIZATION (use for N similar jobs):
- parameters: {"i": "0:9"} generates i=0,1,2,...,9
- Use {i} in name/command: "job_{i}", "python work.py {i}"
- Formats: {i:03d} for zero-padding

FAN-IN PATTERN (aggregating multiple files):
When a NON-parameterized job needs to consume files from parameterized jobs, use input_file_regexes:
- WRONG: {"name": "aggregate", "input_files": ["work_out_{i}"]} -- {i} won't expand!
- RIGHT: {"name": "aggregate", "input_file_regexes": ["^work_out_\\d+$"]} -- matches all work_out_0, work_out_1, etc.

EXAMPLE - Fan-out/Fan-in with files (3 groups, 10 workers each, aggregation):
{
  "files": [
    {"name": "input_{g}", "path": "input_{g}.txt", "st_mtime": 1234567890.0, "parameters": {"g": "0:2"}},
    {"name": "work_{g}_{i}", "path": "work_{g}_{i}.txt", "st_mtime": null, "parameters": {"g": "0:2", "i": "0:9"}},
    {"name": "agg_{g}", "path": "agg_{g}.txt", "st_mtime": null, "parameters": {"g": "0:2"}},
    {"name": "final", "path": "final.txt", "st_mtime": null}
  ],
  "jobs": [
    {"name": "init_{g}", "command": "prep {g}", "output_files": ["input_{g}"], "resource_requirements": "small", "parameters": {"g": "0:2"}},
    {"name": "work_{g}_{i}", "command": "work {g} {i}", "input_files": ["input_{g}"], "output_files": ["work_{g}_{i}"], "resource_requirements": "large", "parameters": {"g": "0:2", "i": "0:9"}},
    {"name": "aggregate_{g}", "command": "agg {g}", "input_file_regexes": ["^work_{g}_\\d+$"], "output_files": ["agg_{g}"], "resource_requirements": "small", "parameters": {"g": "0:2"}},
    {"name": "final", "command": "finalize", "input_file_regexes": ["^agg_\\d+$"], "output_files": ["final"], "resource_requirements": "small"}
  ]
}"#)]
    async fn create_workflow(
        &self,
        #[tool(aggr)] params: CreateWorkflowParams,
    ) -> Result<CallToolResult, McpError> {
        let config = self.config.clone();
        let spec_json = serde_json::to_string(&params.spec_json)
            .map_err(|e| McpError::invalid_params(format!("Invalid spec JSON: {}", e), None))?;
        let user = params
            .user
            .unwrap_or_else(|| std::env::var("USER").unwrap_or_else(|_| "unknown".to_string()));
        let action = params.action;
        let workflow_type = params.workflow_type;
        let account = params.account;
        let hpc_profile = params.hpc_profile;
        let output_path = params.output_path;
        tokio::task::spawn_blocking(move || {
            tools::create_workflow(
                &config,
                &spec_json,
                &user,
                &action,
                &workflow_type,
                account.as_deref(),
                hpc_profile.as_deref(),
                output_path.as_deref(),
            )
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// Get the execution plan for a workflow, showing what will happen when it runs.
    #[tool(
        description = r#"Get the execution plan for a workflow, showing the DAG of events and job execution order.

This tool shows:
- What jobs run at each stage
- Dependencies between jobs
- Scheduler allocations that will be triggered
- Which jobs become ready after each event

INPUT OPTIONS:
1. Workflow ID (integer as string): Get plan for an existing workflow in the database
   Example: "123"

2. Workflow spec JSON: Preview execution plan before creating the workflow
   Example: {"name": "my_workflow", "jobs": [...]}

OUTPUT:
Returns a DAG (directed acyclic graph) of execution events showing:
- root_events: Entry points (typically "Workflow Start")
- leaf_events: Exit points (final jobs)
- events: Map of event_id -> event details
  - trigger: What triggers this event (WorkflowStart or JobsComplete)
  - jobs_becoming_ready: Jobs that can run when this event fires
  - scheduler_allocations: Slurm allocations triggered
  - depends_on_events: Events that must complete first
  - unlocks_events: Events that depend on this one

USE CASES:
- Preview workflow execution before creating it
- Understand job dependencies and parallelism
- Debug why jobs aren't starting
- Verify scheduler allocation timing"#
    )]
    async fn get_execution_plan(
        &self,
        #[tool(aggr)] params: GetExecutionPlanParams,
    ) -> Result<CallToolResult, McpError> {
        let config = self.config.clone();
        let spec_or_id = params.spec_or_id;
        tokio::task::spawn_blocking(move || tools::get_execution_plan(&config, &spec_or_id))
            .await
            .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// Get detailed information about a specific job.
    #[tool(
        description = "Get detailed job information including command, status, resource requirements, and latest result"
    )]
    async fn get_job_details(
        &self,
        #[tool(aggr)] params: JobIdParam,
    ) -> Result<CallToolResult, McpError> {
        let config = self.config.clone();
        let job_id = params.job_id;
        tokio::task::spawn_blocking(move || tools::get_job_details(&config, job_id))
            .await
            .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// List jobs filtered by status.
    #[tool(
        description = "List jobs in a workflow filtered by status (uninitialized, blocked, ready, pending, running, completed, failed, canceled, terminated, disabled)"
    )]
    async fn list_jobs_by_status(
        &self,
        #[tool(aggr)] params: ListJobsByStatusParams,
    ) -> Result<CallToolResult, McpError> {
        let config = self.config.clone();
        let workflow_id = params.workflow_id;
        let status = params.status;
        tokio::task::spawn_blocking(move || {
            tools::list_jobs_by_status(&config, workflow_id, &status)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// Get the status of a workflow including job counts by status.
    #[tool(
        description = "Get workflow status summary with job counts by status (completed, failed, running, etc.)"
    )]
    async fn get_workflow_status(
        &self,
        #[tool(aggr)] params: WorkflowIdParam,
    ) -> Result<CallToolResult, McpError> {
        let config = self.config.clone();
        let workflow_id = params.workflow_id;
        tokio::task::spawn_blocking(move || tools::get_workflow_status(&config, workflow_id))
            .await
            .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// Get a completion summary for a workflow.
    #[tool(
        description = "Get workflow completion summary including total execution time, walltime, \
        and job counts by status. Use this to get a quick overview of workflow results. \
        Only works for completed workflows."
    )]
    async fn get_workflow_summary(
        &self,
        #[tool(aggr)] params: GetWorkflowSummaryParams,
    ) -> Result<CallToolResult, McpError> {
        let workflow_id = params.workflow_id;
        tokio::task::spawn_blocking(move || tools::get_workflow_summary(workflow_id))
            .await
            .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// List job results with filtering options.
    #[tool(
        description = "List job execution results with optional filtering. Returns return codes, \
        execution time, peak memory, and peak CPU for each job. \
        Use filters to find specific results: failed_only=true for failures, \
        sort_by='exec_time_minutes' with reverse_sort=true for slowest jobs, \
        sort_by='peak_memory_bytes' for memory-hungry jobs."
    )]
    async fn list_results(
        &self,
        #[tool(aggr)] params: ListResultsParams,
    ) -> Result<CallToolResult, McpError> {
        let workflow_id = params.workflow_id;
        let job_id = params.job_id;
        let run_id = params.run_id;
        let return_code = params.return_code;
        let failed_only = params.failed_only.unwrap_or(false);
        let status = params.status;
        let limit = params.limit.unwrap_or(100);
        let sort_by = params.sort_by;
        let reverse_sort = params.reverse_sort.unwrap_or(false);
        tokio::task::spawn_blocking(move || {
            tools::list_results(
                workflow_id,
                job_id,
                run_id,
                return_code,
                failed_only,
                status,
                limit,
                sort_by,
                reverse_sort,
            )
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// Analyze workflow logs for errors.
    #[tool(
        description = "Scan all log files for a workflow and detect common error patterns. \
        Detects: OOM (out of memory), timeout/walltime exceeded, segmentation faults, \
        permission denied, file not found, disk full, connection errors, Python exceptions, \
        Rust panics, and Slurm errors. Returns a summary with error counts by type and sample error lines. \
        Use this to quickly diagnose why jobs failed without reading each log file individually."
    )]
    async fn analyze_workflow_logs(
        &self,
        #[tool(aggr)] params: AnalyzeWorkflowLogsParams,
    ) -> Result<CallToolResult, McpError> {
        let output_dir = self.output_dir.clone();
        let output_path = params
            .output_dir
            .map(std::path::PathBuf::from)
            .unwrap_or(output_dir);
        let workflow_id = params.workflow_id;
        tokio::task::spawn_blocking(move || tools::analyze_workflow_logs(&output_path, workflow_id))
            .await
            .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// Read job stdout or stderr logs.
    #[tool(
        description = "Read job execution logs (stdout or stderr). Optionally return only the last N lines."
    )]
    async fn get_job_logs(
        &self,
        #[tool(aggr)] params: GetJobLogsParams,
    ) -> Result<CallToolResult, McpError> {
        let output_dir = self.output_dir.clone();
        let workflow_id = params.workflow_id;
        let job_id = params.job_id;
        let run_id = params.run_id;
        let attempt_id = params.attempt_id;
        let log_type = params.log_type;
        let tail_lines = params.tail_lines;
        tokio::task::spawn_blocking(move || {
            tools::get_job_logs(
                &output_dir,
                workflow_id,
                job_id,
                run_id,
                attempt_id,
                &log_type,
                tail_lines,
            )
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// List all failed jobs in a workflow.
    #[tool(
        description = "List all jobs with 'failed' status in a workflow, including their error information"
    )]
    async fn list_failed_jobs(
        &self,
        #[tool(aggr)] params: WorkflowIdParam,
    ) -> Result<CallToolResult, McpError> {
        let config = self.config.clone();
        let workflow_id = params.workflow_id;
        tokio::task::spawn_blocking(move || tools::list_failed_jobs(&config, workflow_id))
            .await
            .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// Check resource utilization for a workflow.
    #[tool(
        description = "Check resource utilization and identify jobs that exceeded their limits (memory, CPU, runtime). \
        Use include_failed=true to analyze failed jobs for recovery diagnostics. \
        To update resources for jobs that exceeded limits, use the update_job_resources tool (not a CLI command)."
    )]
    async fn check_resource_utilization(
        &self,
        #[tool(aggr)] params: CheckResourceUtilizationParams,
    ) -> Result<CallToolResult, McpError> {
        let workflow_id = params.workflow_id;
        let include_failed = params.include_failed.unwrap_or(true);
        tokio::task::spawn_blocking(move || {
            tools::check_resource_utilization(workflow_id, include_failed)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// Get Slurm accounting data for a workflow.
    #[tool(
        description = "Get Slurm sacct accounting data for all scheduled compute nodes in a workflow. \
        Shows job state, exit codes, elapsed time, max RSS (memory), CPU time, and nodes used. \
        Includes a summary of total walltime consumed across all Slurm allocations. \
        Useful for understanding HPC resource usage and diagnosing Slurm-level failures."
    )]
    async fn get_slurm_sacct(
        &self,
        #[tool(aggr)] params: GetSlurmSacctParams,
    ) -> Result<CallToolResult, McpError> {
        let workflow_id = params.workflow_id;
        tokio::task::spawn_blocking(move || tools::get_slurm_sacct(workflow_id))
            .await
            .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// Update resource requirements for a job.
    #[tool(
        description = "Update a job's resource requirements (CPU, memory, runtime). \
        Use this for jobs that failed or will fail due to resource constraints. \
        IMPORTANT: Update ALL jobs with over-utilization from check_resource_utilization, not just failed ones. \
        After updating resources, use the recover_workflow tool or tell user the command: \
        - torc recover <workflow_id>  (RECOMMENDED: automated Slurm recovery) \
        - Or for manual recovery: torc workflows reset-status + reinitialize + submit. \
        DO NOT suggest 'torc workflows restart' - that command does not exist."
    )]
    async fn update_job_resources(
        &self,
        #[tool(aggr)] params: UpdateJobResourcesParams,
    ) -> Result<CallToolResult, McpError> {
        let config = self.config.clone();
        let job_id = params.job_id;
        let num_cpus = params.num_cpus;
        let memory = params.memory;
        let runtime = params.runtime;
        tokio::task::spawn_blocking(move || {
            tools::update_job_resources(&config, job_id, num_cpus, memory, runtime)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// Recover a Slurm workflow from failures.
    #[tool(
        description = "Automatically recover a Slurm workflow from failures (OOM, timeout). \
        This tool diagnoses failures, adjusts resource requirements, resets failed jobs, \
        and resubmits Slurm allocations. \
        \n\nIMPORTANT WORKFLOW: \
        \n1. ALWAYS call with dry_run=true FIRST to preview what will be changed \
        \n2. Show the user the preview results in a clear format \
        \n3. Ask user: 'Would you like me to proceed with these recovery actions?' \
        \n4. Only if user confirms, call again with dry_run=false to execute \
        \n\nThe tool will: \
        \n- Diagnose OOM failures and increase memory (default: 1.5x) \
        \n- Diagnose timeout failures and increase runtime (default: 1.4x) \
        \n- Reset failed jobs and reinitialize the workflow \
        \n- Regenerate Slurm schedulers and submit new allocations"
    )]
    async fn recover_workflow(
        &self,
        #[tool(aggr)] params: RecoverWorkflowParams,
    ) -> Result<CallToolResult, McpError> {
        let output_dir = self.output_dir.clone();
        let workflow_id = params.workflow_id;
        let dry_run = params.dry_run;
        let memory_multiplier = params.memory_multiplier.unwrap_or(1.5);
        let runtime_multiplier = params.runtime_multiplier.unwrap_or(1.4);
        let retry_unknown = params.retry_unknown.unwrap_or(false);
        tokio::task::spawn_blocking(move || {
            tools::recover_workflow(
                workflow_id,
                &output_dir,
                dry_run,
                memory_multiplier,
                runtime_multiplier,
                retry_unknown,
            )
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// List jobs awaiting AI-assisted classification.
    #[tool(
        description = r#"List jobs with pending_failed status that are awaiting classification.
These are jobs that failed without a matching failure handler. The AI agent should:
1. Analyze the stderr output for each job
2. Classify the failure as transient (retry) or permanent (fail)
3. Use classify_and_resolve_failures to act on the classification

Transient errors (should retry):
- Connection refused, network timeout, DNS resolution failures
- NCCL timeout, GPU communication errors
- EIO, disk I/O errors, temporary storage issues
- Slurm node failures, preemption

Permanent errors (should fail):
- Syntax errors, import errors, missing modules
- Invalid arguments, assertion failures
- Out of bounds, null pointer dereference
- Permission denied (code bug, not transient)"#
    )]
    async fn list_pending_failed_jobs(
        &self,
        #[tool(aggr)] params: ListPendingFailedJobsParams,
    ) -> Result<CallToolResult, McpError> {
        let config = self.config.clone();
        let output_dir = self.output_dir.clone();
        let workflow_id = params.workflow_id;
        tokio::task::spawn_blocking(move || {
            tools::list_pending_failed_jobs(&config, workflow_id, &output_dir)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// Classify and resolve pending_failed jobs.
    #[tool(
        description = r#"Classify pending_failed jobs and either retry them or mark them as failed.

IMPORTANT WORKFLOW:
1. First call list_pending_failed_jobs to see jobs awaiting classification
2. Analyze stderr for each job to determine if failure is transient or permanent
3. Call this tool with dry_run=true to preview classifications
4. Show user the preview and ask for confirmation
5. If approved, call again with dry_run=false to apply

For each job, specify:
- action: 'retry' (transient error) or 'fail' (permanent error)
- memory: optional new memory requirement for retry (e.g., '8g')
- runtime: optional new runtime for retry (e.g., 'PT2H')
- reason: explanation for the classification (for audit trail)"#
    )]
    async fn classify_and_resolve_failures(
        &self,
        #[tool(aggr)] params: ClassifyAndResolveFailuresParams,
    ) -> Result<CallToolResult, McpError> {
        let config = self.config.clone();
        let workflow_id = params.workflow_id;
        let dry_run = params.dry_run;
        // Convert from param type to tools type
        let classifications: Vec<tools::FailureClassification> = params
            .classifications
            .into_iter()
            .map(|c| tools::FailureClassification {
                job_id: c.job_id,
                action: c.action,
                memory: c.memory,
                runtime: c.runtime,
                reason: c.reason,
            })
            .collect();
        tokio::task::spawn_blocking(move || {
            tools::classify_and_resolve_failures(&config, workflow_id, classifications, dry_run)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// Analyze resource usage for a workflow.
    #[tool(
        description = r#"Analyze actual resource usage for all jobs in a workflow, grouped by resource requirement.

Returns per-job peak memory, CPU%, and execution time alongside configured limits.
Use this to identify natural resource clusters for regrouping with regroup_job_resources.

OUTPUT includes for each resource group:
- Current config (memory, CPUs, runtime, GPUs, nodes)
- Summary stats: min/max/mean/median for peak_memory_bytes, peak_cpu_percent, exec_time_minutes
- Per-job detail with actual measurements

WORKFLOW:
1. Call analyze_resource_usage to see actual usage patterns
2. Identify natural clusters (e.g., jobs using 2GB vs 20GB in same RR)
3. Use regroup_job_resources with dry_run=true to preview new groupings
4. If approved, apply with dry_run=false"#
    )]
    async fn analyze_resource_usage(
        &self,
        #[tool(aggr)] params: AnalyzeResourceUsageParams,
    ) -> Result<CallToolResult, McpError> {
        let config = self.config.clone();
        let workflow_id = params.workflow_id;
        let completed_only = params.completed_only.unwrap_or(false);
        tokio::task::spawn_blocking(move || {
            tools::analyze_resource_usage(&config, workflow_id, completed_only)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// Regroup jobs into new resource requirement groups.
    #[tool(
        description = r#"Create new resource requirement groups and reassign jobs to them.

Use this after analyze_resource_usage reveals that jobs within a single RR have
very different actual resource needs. This tool creates new RR records and
reassigns specified jobs, enabling more efficient resource allocation.

IMPORTANT WORKFLOW:
1. ALWAYS call with dry_run=true FIRST to preview the regrouping
2. Show the user the before/after for each job
3. Ask user: 'Would you like me to proceed with this regrouping?'
4. Only if confirmed, call again with dry_run=false to apply

NOTES:
- Jobs NOT listed in any group keep their current RR (partial regrouping is OK)
- Each job can only appear in one group
- num_gpus/num_nodes default to the job's current RR values if not specified
- New RR records are created; existing RRs are not modified or deleted"#
    )]
    async fn regroup_job_resources(
        &self,
        #[tool(aggr)] params: RegroupJobResourcesParams,
    ) -> Result<CallToolResult, McpError> {
        let config = self.config.clone();
        let workflow_id = params.workflow_id;
        let dry_run = params.dry_run;
        let groups: Vec<tools::ResourceGroup> = params
            .groups
            .into_iter()
            .map(|g| tools::ResourceGroup {
                memory: g.memory,
                num_cpus: g.num_cpus,
                runtime: g.runtime,
                num_gpus: g.num_gpus,
                num_nodes: g.num_nodes,
                name: g.name,
                job_ids: g.job_ids,
            })
            .collect();
        tokio::task::spawn_blocking(move || {
            tools::regroup_job_resources(&config, workflow_id, groups, dry_run)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// List available example workflow specifications.
    #[tool(
        description = "List available example workflow specifications with descriptions. \
        Use this to discover example workflows that can be retrieved with get_example. \
        Examples cover common patterns: diamond (fan-out/fan-in), parameterized jobs, \
        hyperparameter sweeps, Slurm pipelines, workflow actions, failure handlers, and more. \
        For the graceful job termination / checkpointing pattern (catching SIGTERM, saving \
        checkpoints, resuming from where you left off), use get_docs with topic='checkpointing'."
    )]
    async fn list_examples(&self) -> Result<CallToolResult, McpError> {
        let examples_dir = self.examples_dir.clone();
        tokio::task::spawn_blocking(move || tools::list_examples(examples_dir.as_deref()))
            .await
            .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// Get an example workflow specification.
    #[tool(
        description = "Retrieve a complete example workflow specification by name. \
        Use list_examples first to see available examples. \
        Returns the full spec content that can be adapted for new workflows. \
        Prefer YAML format for parameterized workflows (KDL doesn't support parameters)."
    )]
    async fn get_example(
        &self,
        #[tool(aggr)] params: GetExampleParams,
    ) -> Result<CallToolResult, McpError> {
        let examples_dir = self.examples_dir.clone();
        let name = params.name;
        let format = params.format.unwrap_or_else(|| "yaml".to_string());
        tokio::task::spawn_blocking(move || {
            tools::get_example(examples_dir.as_deref(), &name, &format)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// Get Torc documentation on a topic.
    #[tool(description = r#"Retrieve Torc documentation on a specific topic.

Use this to understand Torc concepts before creating or debugging workflows.

KEY TOPICS:
- "workflow-spec" - Complete workflow specification reference (all fields, types, defaults)
- "dependencies" - How job dependencies work (explicit, file-based, user_data)
- "parameterization" - Parameter sweeps, ranges, format specifiers, Cartesian products
- "slurm" - Slurm HPC integration, schedulers, accounts, partitions
- "job-states" - Job status lifecycle (uninitialized → ready → running → completed/failed)
- "actions" - Workflow actions (on_workflow_start, on_jobs_ready, schedule_nodes)
- "failure-handlers" - Automatic retry rules for specific exit codes
- "recovery" - Automated workflow recovery (OOM, timeout diagnosis)
- "ai-recovery" - AI-assisted failure classification (pending_failed status)
- "resource-monitoring" - CPU/memory monitoring, time-series collection
- "cli" - CLI command reference
- "quick-start" - Getting started guide
- "architecture" - System architecture overview
- "checkpointing" - Graceful job termination on HPC: catching SIGTERM, saving checkpoints, resuming from where you left off (srun_termination_signal, shutdown-flag pattern)
- "hpc-profiles" - HPC profile configuration for different clusters
- "workflow-formats" - YAML, JSON, JSON5, KDL format comparison
- "allocation-strategies" - Single-large vs many-small Slurm allocation tradeoffs, fair-share scheduling, sbatch --test-only probes
- "tutorials" - List of available tutorials

WHEN TO USE:
- Before creating a workflow: check "workflow-spec" and "parameterization"
- Before Slurm submission: check "slurm" and "hpc-profiles"
- To understand dependencies: check "dependencies"
- To set up error handling: check "failure-handlers" and "recovery"
- When user asks about workflow patterns or long-running jobs: check "checkpointing"
- When planning Slurm allocation strategy: check "allocation-strategies""#)]
    async fn get_docs(
        &self,
        #[tool(aggr)] params: GetDocsParams,
    ) -> Result<CallToolResult, McpError> {
        let docs_dir = self.docs_dir.clone();
        let topic = params.topic;
        tokio::task::spawn_blocking(move || tools::get_docs(docs_dir.as_deref(), &topic))
            .await
            .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }

    /// Analyze a workflow and recommend Slurm allocation strategy.
    #[tool(
        description = r#"Analyze a workflow specification and current cluster state to recommend
whether to use a single large Slurm allocation or many small allocations.

This tool runs sbatch --test-only probes to get estimated start times from the Slurm
scheduler, then compares completion times for different strategies.

WHAT IT RETURNS:
- Workflow analysis: job count, dependency depth, max parallelism, resource groups
- Cluster state: node availability, queue depth per partition
- sbatch --test-only estimates: predicted start/completion for single-large vs many-small
- Recommendation: which strategy minimizes total completion time (makespan)

KEY CONCEPTS FOR INTERPRETING RESULTS:
- "single-large" (1 x N nodes): One allocation requesting all needed nodes. Slurm
  prioritizes larger jobs via backfill scheduling. All work completes in one walltime window.
- "many-small" (N x 1 node): N separate single-node allocations. First jobs start faster,
  but fair-share degradation means later jobs wait longer.
- The "many-small" wait estimate is OPTIMISTIC (first job only). The tool applies a
  fair-share penalty factor but actual degradation depends on account balance.
- For deep DAGs, check max_parallelism vs ideal_nodes - you may not need as many nodes
  as the flat calculation suggests.

WHEN TO RECOMMEND SINGLE-LARGE:
- Cluster is busy (few idle nodes) - Slurm reserves slots for large jobs
- User cares about total completion time, not time-to-first-result
- sbatch --test-only shows large allocation completes sooner

WHEN TO RECOMMEND MANY-SMALL:
- User needs partial results quickly
- Large allocation wait is extremely long (many hours more than small)
- Ideal nodes exceeds partition's max_nodes_per_user limit

For detailed background on allocation strategies, use get_docs with
topic='allocation-strategies'.

IMPORTANT: Always present both the raw estimates AND the recommendation to the user
so they can make an informed decision."#
    )]
    async fn plan_allocations(
        &self,
        #[tool(aggr)] params: PlanAllocationsParams,
    ) -> Result<CallToolResult, McpError> {
        let spec_json = serde_json::to_string(&params.spec_json)
            .map_err(|e| McpError::invalid_params(format!("Invalid spec JSON: {}", e), None))?;
        let account = params.account;
        let partition = params.partition;
        let hpc_profile = params.hpc_profile;
        let skip_test_only = params.skip_test_only;
        tokio::task::spawn_blocking(move || {
            tools::plan_allocations(
                &spec_json,
                &account,
                partition.as_deref(),
                hpc_profile.as_deref(),
                skip_test_only,
            )
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?
    }
}

#[tool(tool_box)]
impl ServerHandler for TorcMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "Torc MCP Server - Manage computational workflows.\n\n\
                 WORKFLOW CREATION - SAVE FILES BY DEFAULT:\n\
                 - When user asks to 'create a workflow', save a spec FILE (action=save_spec_file)\n\
                 - AI-generated specs have placeholder commands - users must customize before running\n\
                 - Only use action=create_workflow when user explicitly says 'run' or 'submit'\n\
                 - IMPORTANT: Ask the user whether they will run on Slurm or locally before creating \
                 the workflow. This determines the workflow_type and the CLI commands to suggest.\n\n\
                 DOCUMENTATION & EXAMPLES:\n\
                 - Use get_docs to retrieve documentation on any topic before creating workflows\n\
                 - Use list_examples + get_example to find and adapt example workflow specs\n\
                 - For the checkpointing/graceful termination pattern, use get_docs with topic='checkpointing'\n\
                 - Resources are also available at torc://docs/{topic} and torc://examples/{name}\n\n\
                 FILE-BASED DEPENDENCIES:\n\
                 1. Add a 'files' section defining each file with name, path, st_mtime\n\
                 2. Add 'input_files' to jobs that read files (exact names)\n\
                 3. Add 'output_files' to jobs that write files (exact names)\n\
                 4. For FAN-IN (aggregating multiple files into one job), use 'input_file_regexes' with a regex pattern\n\
                    Example: input_file_regexes: [\"^work_out_\\\\d+$\"] matches work_out_0, work_out_1, etc.\n\n\
                 Tools: get_execution_plan (preview execution), get_workflow_status (check progress), \
                 list_failed_jobs, get_job_logs, analyze_workflow_logs (scan all logs for errors), \
                 check_resource_utilization, update_job_resources, \
                 analyze_resource_usage (per-job resource data for cluster analysis), \
                 regroup_job_resources (reassign jobs to new resource groups), \
                 get_docs (documentation), list_examples + get_example (example workflows)."
                    .to_string(),
            ),
        }
    }

    fn list_resources(
        &self,
        _request: PaginatedRequestParam,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<rmcp::model::ListResourcesResult, McpError>> + Send + '_
    {
        let docs_dir = self.docs_dir.clone();
        let examples_dir = self.examples_dir.clone();
        async move {
            let resources = tokio::task::spawn_blocking(move || {
                tools::list_mcp_resources(docs_dir.as_deref(), examples_dir.as_deref())
            })
            .await
            .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))?;

            Ok(rmcp::model::ListResourcesResult {
                resources,
                next_cursor: None,
            })
        }
    }

    fn read_resource(
        &self,
        request: rmcp::model::ReadResourceRequestParam,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ReadResourceResult, McpError>> + Send + '_ {
        let docs_dir = self.docs_dir.clone();
        let examples_dir = self.examples_dir.clone();
        let uri = request.uri;
        async move {
            let contents = tokio::task::spawn_blocking(move || {
                tools::read_mcp_resource(docs_dir.as_deref(), examples_dir.as_deref(), &uri)
            })
            .await
            .map_err(|e| McpError::internal_error(format!("Task join error: {}", e), None))??;

            Ok(ReadResourceResult {
                contents: vec![contents],
            })
        }
    }
}
