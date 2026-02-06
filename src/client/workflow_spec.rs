use crate::client::apis::{configuration::Configuration, default_api};
use crate::client::parameter_expansion::{
    ParameterValue, cartesian_product, parse_parameter_value, substitute_parameters, zip_parameters,
};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use crate::models;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Result of validating a workflow specification (dry-run)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the validation passed with no errors
    pub valid: bool,
    /// Validation errors that would prevent workflow creation
    pub errors: Vec<String>,
    /// Warnings that don't prevent creation but may indicate issues
    pub warnings: Vec<String>,
    /// Summary of what would be created
    pub summary: ValidationSummary,
}

/// Summary of workflow components that would be created
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationSummary {
    /// Name of the workflow
    pub workflow_name: String,
    /// Description of the workflow
    pub workflow_description: Option<String>,
    /// Number of jobs that would be created
    pub job_count: usize,
    /// Number of jobs before parameter expansion
    pub job_count_before_expansion: usize,
    /// Number of files that would be created
    pub file_count: usize,
    /// Number of files before parameter expansion
    pub file_count_before_expansion: usize,
    /// Number of user data records that would be created
    pub user_data_count: usize,
    /// Number of resource requirements that would be created
    pub resource_requirements_count: usize,
    /// Number of Slurm schedulers that would be created
    pub slurm_scheduler_count: usize,
    /// Number of workflow actions that would be created
    pub action_count: usize,
    /// Whether the workflow has on_workflow_start schedule_nodes action
    pub has_schedule_nodes_action: bool,
    /// List of job names that would be created
    pub job_names: Vec<String>,
    /// List of scheduler names
    pub scheduler_names: Vec<String>,
}

#[cfg(feature = "client")]
use kdl::{KdlDocument, KdlNode};

/// File specification for JSON serialization (without workflow_id, id, and st_mtime)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileSpec {
    /// Name of the file
    pub name: String,
    /// Path to the file
    pub path: String,
    /// Optional parameters for generating multiple files
    /// Supports range notation (e.g., "1:100" or "1:100:5") and lists (e.g., "[1,5,10]")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<HashMap<String, String>>,
    /// How to combine multiple parameters: "product" (default, Cartesian product) or "zip"
    /// With "zip", parameters are combined element-wise (all must have the same length)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_mode: Option<String>,
    /// Names of workflow-level parameters to use for this file
    /// If set, only these parameters from the workflow will be used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_parameters: Option<Vec<String>>,
}

impl FileSpec {
    /// Create a new FileSpec with only required fields
    #[allow(dead_code)]
    pub fn new(name: String, path: String) -> FileSpec {
        FileSpec {
            name,
            path,
            parameters: None,
            parameter_mode: None,
            use_parameters: None,
        }
    }

    /// Expand this FileSpec into multiple FileSpecs based on its parameters
    /// Returns a single-element vec if no parameters are present
    pub fn expand(&self) -> Result<Vec<FileSpec>, String> {
        // If no parameters, return a clone
        let Some(ref params) = self.parameters else {
            return Ok(vec![self.clone()]);
        };

        // Parse all parameter values
        let mut parsed_params: HashMap<String, Vec<ParameterValue>> = HashMap::new();
        for (name, value) in params {
            let values = parse_parameter_value(value)?;
            parsed_params.insert(name.clone(), values);
        }

        // Generate combinations based on parameter_mode
        let mode = self.parameter_mode.as_deref().unwrap_or("product");
        let combinations = match mode {
            "zip" => zip_parameters(&parsed_params)?,
            _ => cartesian_product(&parsed_params),
        };

        // Create a FileSpec for each combination
        let mut expanded = Vec::new();
        for combo in combinations {
            let mut new_spec = self.clone();
            new_spec.parameters = None; // Remove parameters from expanded specs
            new_spec.parameter_mode = None; // Remove parameter_mode from expanded specs

            // Substitute parameters in name and path
            new_spec.name = substitute_parameters(&self.name, &combo);
            new_spec.path = substitute_parameters(&self.path, &combo);

            expanded.push(new_spec);
        }

        Ok(expanded)
    }
}

/// User data specification for JSON serialization (without workflow_id and id)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserDataSpec {
    /// Whether the user data is ephemeral
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_ephemeral: Option<bool>,
    /// Name of the user data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The data content as JSON value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Workflow action specification for defining conditional actions
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowActionSpec {
    /// Trigger type: on_workflow_start, on_workflow_complete, on_jobs_ready, on_jobs_complete
    pub trigger_type: String,
    /// Action type: run_commands, schedule_nodes
    pub action_type: String,
    /// For on_jobs_ready/on_jobs_complete: exact job names to match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jobs: Option<Vec<String>>,
    /// For on_jobs_ready/on_jobs_complete: regex patterns to match job names
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_name_regexes: Option<Vec<String>>,
    /// For run_commands action: array of commands to execute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands: Option<Vec<String>>,
    /// For schedule_nodes action: scheduler name (will be translated to scheduler_id)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<String>,
    /// For schedule_nodes action: scheduler type (e.g., "slurm", "local")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler_type: Option<String>,
    /// For schedule_nodes action: number of node allocations to request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_allocations: Option<i64>,
    /// For schedule_nodes action: whether to start one worker per node
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_one_worker_per_node: Option<bool>,
    /// For schedule_nodes action: maximum parallel jobs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_parallel_jobs: Option<i32>,
    /// Whether the action persists and can be claimed by multiple workers (default: false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistent: Option<bool>,
}

/// Resource requirements specification for JSON serialization (without workflow_id and id)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceRequirementsSpec {
    /// Name of the resource requirements configuration
    pub name: String,
    /// Number of CPUs required
    pub num_cpus: i64,
    /// Number of GPUs required
    #[serde(default)]
    pub num_gpus: i64,
    /// Number of nodes required (defaults to 1)
    #[serde(default = "ResourceRequirementsSpec::default_num_nodes")]
    pub num_nodes: i64,
    /// Memory requirement
    pub memory: String,
    /// Runtime limit (defaults to 1 hour)
    #[serde(default = "ResourceRequirementsSpec::default_runtime")]
    pub runtime: String,
}

impl ResourceRequirementsSpec {
    fn default_num_nodes() -> i64 {
        1
    }

    fn default_runtime() -> String {
        "PT1H".to_string()
    }
}

/// A rule for handling specific exit codes in a failure handler
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FailureHandlerRuleSpec {
    /// Exit codes that trigger this rule. Can be omitted if match_all_exit_codes is true.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exit_codes: Vec<i32>,
    /// If true, this rule matches any non-zero exit code.
    /// Use this for simple retry-on-any-failure behavior.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub match_all_exit_codes: bool,
    /// Optional recovery script to run before retrying
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_script: Option<String>,
    /// Maximum number of retry attempts (defaults to 3)
    #[serde(default = "FailureHandlerRuleSpec::default_max_retries")]
    pub max_retries: i32,
}

impl FailureHandlerRuleSpec {
    fn default_max_retries() -> i32 {
        3
    }
}

/// Failure handler specification for JSON serialization (without workflow_id and id)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FailureHandlerSpec {
    /// Name of the failure handler
    pub name: String,
    /// Rules for handling different exit codes
    pub rules: Vec<FailureHandlerRuleSpec>,
}

/// Slurm scheduler specification for JSON serialization (without workflow_id and id)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlurmSchedulerSpec {
    /// Name of the scheduler
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Slurm account
    pub account: String,
    /// Generic resources (GRES)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gres: Option<String>,
    /// Memory specification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mem: Option<String>,
    /// Number of nodes (defaults to 1)
    #[serde(default = "SlurmSchedulerSpec::default_nodes")]
    pub nodes: i64,
    /// Number of tasks per node
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ntasks_per_node: Option<i64>,
    /// Partition name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
    /// Quality of service
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qos: Option<String>,
    /// Temporary storage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmp: Option<String>,
    /// Wall time limit (defaults to 1 hour)
    #[serde(default = "SlurmSchedulerSpec::default_walltime")]
    pub walltime: String,
    /// Extra parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<String>,
}

impl SlurmSchedulerSpec {
    fn default_nodes() -> i64 {
        1
    }

    fn default_walltime() -> String {
        "01:00:00".to_string()
    }
}

/// Parameters that are managed by torc and cannot be set in slurm_defaults
/// Note: "account" is allowed in slurm_defaults as a workflow-level default
pub const SLURM_EXCLUDED_PARAMS: &[&str] = &[
    "partition",
    "nodes",
    "walltime",
    "time",
    "mem",
    "gres",
    "name",
    "job-name",
];

/// Default Slurm parameters to apply to all schedulers in a workflow
///
/// These parameters are applied at runtime to both user-defined and auto-generated
/// Slurm schedulers. Any valid sbatch parameter can be specified except for those
/// managed by torc: partition, nodes, walltime/time, mem, gres, name/job-name.
///
/// The "account" parameter is allowed and can be used as a workflow-level default.
///
/// Parameters should use the sbatch long option name (without the leading --).
/// For example: "qos", "constraint", "mail-user", "mail-type", "reservation", etc.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SlurmDefaultsSpec(pub std::collections::HashMap<String, serde_json::Value>);

impl SlurmDefaultsSpec {
    /// Validate that no excluded parameters are present
    /// Returns an error listing all excluded parameters found
    pub fn validate(&self) -> Result<(), String> {
        let excluded_found: Vec<&str> = self
            .0
            .keys()
            .filter(|k| {
                let key_lower = k.to_lowercase();
                SLURM_EXCLUDED_PARAMS
                    .iter()
                    .any(|excluded| key_lower == *excluded)
            })
            .map(|k| k.as_str())
            .collect();

        if excluded_found.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "slurm_defaults contains excluded parameters managed by torc: {}. \
                 These cannot be set as defaults.",
                excluded_found.join(", ")
            ))
        }
    }

    /// Convert all values to strings for use in config map
    ///
    /// Only string, number, and boolean values are supported. Arrays, objects, and null
    /// values are skipped with a warning since they cannot be meaningfully converted
    /// to Slurm parameter values.
    pub fn to_string_map(&self) -> std::collections::HashMap<String, String> {
        self.0
            .iter()
            .filter_map(|(k, v)| {
                let value_str = match v {
                    serde_json::Value::String(s) => Some(s.clone()),
                    serde_json::Value::Number(n) => Some(n.to_string()),
                    serde_json::Value::Bool(b) => Some(b.to_string()),
                    serde_json::Value::Array(_)
                    | serde_json::Value::Object(_)
                    | serde_json::Value::Null => {
                        log::warn!(
                            "Skipping slurm_defaults key '{}': unsupported value type (arrays, objects, and null are not valid Slurm parameter values)",
                            k
                        );
                        None
                    }
                };
                value_str.map(|v| (k.clone(), v))
            })
            .collect()
    }
}

/// Specification for a job within a workflow
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobSpec {
    /// Name of the job
    pub name: String,
    /// Command to execute for this job
    pub command: String,
    /// Optional script for job invocation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invocation_script: Option<String>,
    /// Whether to cancel this job if a blocking job fails
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel_on_blocking_job_failure: Option<bool>,
    /// Whether this job supports termination
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_termination: Option<bool>,
    /// Name of the resource requirements configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_requirements: Option<String>,
    /// Name of the failure handler for this job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_handler: Option<String>,
    /// Names of jobs that must complete before this job can run (exact matches)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
    /// Regex patterns for jobs that must complete before this job can run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on_regexes: Option<Vec<String>>,
    /// Names of input files required by this job (exact matches)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_files: Option<Vec<String>>,
    /// Regex patterns for input files required by this job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_file_regexes: Option<Vec<String>>,
    /// Names of output files produced by this job (exact matches)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_files: Option<Vec<String>>,
    /// Regex patterns for output files produced by this job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_file_regexes: Option<Vec<String>>,
    /// Names of input user data required by this job (exact matches)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_user_data: Option<Vec<String>>,
    /// Regex patterns for input user data required by this job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_user_data_regexes: Option<Vec<String>>,
    /// Names of output data produced by this job (exact matches)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_user_data: Option<Vec<String>>,
    /// Regex patterns for output data produced by this job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_user_data_regexes: Option<Vec<String>>,
    /// Name of the scheduler to use for this job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<String>,
    /// Optional parameters for generating multiple jobs
    /// Supports range notation (e.g., "1:100" or "1:100:5") and lists (e.g., "[1,5,10]")
    /// Multiple parameters create a Cartesian product of jobs by default
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<HashMap<String, String>>,
    /// How to combine multiple parameters: "product" (default, Cartesian product) or "zip"
    /// With "zip", parameters are combined element-wise (all must have the same length)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_mode: Option<String>,
    /// Names of workflow-level parameters to use for this job
    /// If set, only these parameters from the workflow will be used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_parameters: Option<Vec<String>>,
}

impl JobSpec {
    /// Create a new JobSpec with only required fields
    #[allow(dead_code)]
    pub fn new(name: String, command: String) -> JobSpec {
        JobSpec {
            name,
            command,
            invocation_script: None,
            cancel_on_blocking_job_failure: Some(false),
            supports_termination: Some(false),
            resource_requirements: None,
            failure_handler: None,
            depends_on: None,
            depends_on_regexes: None,
            input_files: None,
            input_file_regexes: None,
            output_files: None,
            output_file_regexes: None,
            input_user_data: None,
            input_user_data_regexes: None,
            output_user_data: None,
            output_user_data_regexes: None,
            scheduler: None,
            parameters: None,
            parameter_mode: None,
            use_parameters: None,
        }
    }

    /// Expand this JobSpec into multiple JobSpecs based on its parameters
    /// Returns a single-element vec if no parameters are present
    pub fn expand(&self) -> Result<Vec<JobSpec>, String> {
        // If no parameters, return a clone
        let Some(ref params) = self.parameters else {
            return Ok(vec![self.clone()]);
        };

        // Parse all parameter values
        let mut parsed_params: HashMap<String, Vec<ParameterValue>> = HashMap::new();
        for (name, value) in params {
            let values = parse_parameter_value(value)?;
            parsed_params.insert(name.clone(), values);
        }

        // Generate combinations based on parameter_mode
        let mode = self.parameter_mode.as_deref().unwrap_or("product");
        let combinations = match mode {
            "zip" => zip_parameters(&parsed_params)?,
            _ => cartesian_product(&parsed_params),
        };

        // Create a JobSpec for each combination
        let mut expanded = Vec::new();
        for combo in combinations {
            let mut new_spec = self.clone();
            new_spec.parameters = None; // Remove parameters from expanded specs
            new_spec.parameter_mode = None; // Remove parameter_mode from expanded specs

            // Substitute parameters in all string fields
            new_spec.name = substitute_parameters(&self.name, &combo);
            new_spec.command = substitute_parameters(&self.command, &combo);

            if let Some(ref script) = self.invocation_script {
                new_spec.invocation_script = Some(substitute_parameters(script, &combo));
            }

            if let Some(ref rr_name) = self.resource_requirements {
                new_spec.resource_requirements = Some(substitute_parameters(rr_name, &combo));
            }

            if let Some(ref sched_name) = self.scheduler {
                new_spec.scheduler = Some(substitute_parameters(sched_name, &combo));
            }

            // Substitute parameters in name vectors
            if let Some(ref names) = self.depends_on {
                new_spec.depends_on = Some(
                    names
                        .iter()
                        .map(|n| substitute_parameters(n, &combo))
                        .collect(),
                );
            }

            if let Some(ref names) = self.input_files {
                new_spec.input_files = Some(
                    names
                        .iter()
                        .map(|n| substitute_parameters(n, &combo))
                        .collect(),
                );
            }

            if let Some(ref names) = self.output_files {
                new_spec.output_files = Some(
                    names
                        .iter()
                        .map(|n| substitute_parameters(n, &combo))
                        .collect(),
                );
            }

            if let Some(ref names) = self.input_user_data {
                new_spec.input_user_data = Some(
                    names
                        .iter()
                        .map(|n| substitute_parameters(n, &combo))
                        .collect(),
                );
            }

            if let Some(ref names) = self.output_user_data {
                new_spec.output_user_data = Some(
                    names
                        .iter()
                        .map(|n| substitute_parameters(n, &combo))
                        .collect(),
                );
            }

            // Substitute parameters in regex pattern vectors
            if let Some(ref regexes) = self.depends_on_regexes {
                new_spec.depends_on_regexes = Some(
                    regexes
                        .iter()
                        .map(|r| substitute_parameters(r, &combo))
                        .collect(),
                );
            }

            if let Some(ref regexes) = self.input_file_regexes {
                new_spec.input_file_regexes = Some(
                    regexes
                        .iter()
                        .map(|r| substitute_parameters(r, &combo))
                        .collect(),
                );
            }

            if let Some(ref regexes) = self.output_file_regexes {
                new_spec.output_file_regexes = Some(
                    regexes
                        .iter()
                        .map(|r| substitute_parameters(r, &combo))
                        .collect(),
                );
            }

            if let Some(ref regexes) = self.input_user_data_regexes {
                new_spec.input_user_data_regexes = Some(
                    regexes
                        .iter()
                        .map(|r| substitute_parameters(r, &combo))
                        .collect(),
                );
            }

            if let Some(ref regexes) = self.output_user_data_regexes {
                new_spec.output_user_data_regexes = Some(
                    regexes
                        .iter()
                        .map(|r| substitute_parameters(r, &combo))
                        .collect(),
                );
            }

            expanded.push(new_spec);
        }

        Ok(expanded)
    }
}

/// Specification for a complete workflow
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSpec {
    /// Name of the workflow
    pub name: String,
    /// User who owns this workflow (optional - will default to current user)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Description of the workflow (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Shared parameters that can be used by jobs and files
    /// Jobs/files can reference these by setting use_parameters to parameter names
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<HashMap<String, String>>,
    /// Inform all compute nodes to shut down this number of seconds before the expiration time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_node_expiration_buffer_seconds: Option<i64>,
    /// Inform all compute nodes to wait for new jobs for this time period before exiting
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_node_wait_for_new_jobs_seconds: Option<i64>,
    /// Inform all compute nodes to ignore workflow completions and hold onto allocations indefinitely
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_node_ignore_workflow_completion: Option<bool>,
    /// Inform all compute nodes to wait this number of minutes if the database becomes unresponsive
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_node_wait_for_healthy_database_minutes: Option<i64>,
    /// Method for sorting jobs when claiming them from the server
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jobs_sort_method: Option<models::ClaimJobsSortMethod>,
    /// Jobs that make up this workflow
    pub jobs: Vec<JobSpec>,
    /// Files associated with this workflow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileSpec>>,
    /// User data associated with this workflow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_data: Option<Vec<UserDataSpec>>,
    /// Resource requirements available for this workflow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_requirements: Option<Vec<ResourceRequirementsSpec>>,
    /// Failure handlers available for this workflow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_handlers: Option<Vec<FailureHandlerSpec>>,
    /// Slurm schedulers available for this workflow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slurm_schedulers: Option<Vec<SlurmSchedulerSpec>>,
    /// Default Slurm parameters to apply to all schedulers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slurm_defaults: Option<SlurmDefaultsSpec>,
    /// Resource monitoring configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_monitor: Option<crate::client::resource_monitor::ResourceMonitorConfig>,
    /// Actions to execute based on workflow/job state transitions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<WorkflowActionSpec>>,
    /// Use PendingFailed status for failed jobs (enables AI-assisted recovery)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_pending_failed: Option<bool>,
    /// Project name or identifier for grouping workflows
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Arbitrary metadata as JSON string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<String>,
}

impl WorkflowSpec {
    /// Create a new WorkflowSpec with required fields
    #[allow(dead_code)]
    pub fn new(
        name: String,
        user: String,
        description: Option<String>,
        jobs: Vec<JobSpec>,
    ) -> WorkflowSpec {
        WorkflowSpec {
            name,
            user: Some(user),
            description,
            parameters: None,
            compute_node_expiration_buffer_seconds: None,
            compute_node_wait_for_new_jobs_seconds: None,
            compute_node_ignore_workflow_completion: None,
            compute_node_wait_for_healthy_database_minutes: None,
            jobs_sort_method: None,
            jobs,
            files: None,
            user_data: None,
            resource_requirements: None,
            failure_handlers: None,
            slurm_schedulers: None,
            slurm_defaults: None,
            resource_monitor: None,
            actions: None,
            use_pending_failed: None,
            project: None,
            metadata: None,
        }
    }

    /// Deserialize a WorkflowSpec from a serde_json::Value
    /// This is the common conversion point for all file formats
    pub fn from_json_value(value: serde_json::Value) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(serde_json::from_value(value)?)
    }

    /// Expand all parameterized jobs and files in this workflow spec
    /// This modifies the spec in-place, replacing parameterized specs with their expanded versions
    ///
    /// Parameter resolution order:
    /// 1. If job/file has its own `parameters`, use those (local params override workflow params)
    /// 2. If job/file has `use_parameters`, select only those from workflow-level params
    pub fn expand_parameters(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let workflow_params = self.parameters.clone();

        // Expand all jobs
        let mut expanded_jobs = Vec::new();
        for job in &self.jobs {
            // Resolve parameters for this job
            let mut job_with_params = job.clone();
            job_with_params.parameters =
                Self::resolve_parameters(&job.parameters, &job.use_parameters, &workflow_params);
            // Clear use_parameters after resolution
            job_with_params.use_parameters = None;

            let expanded = job_with_params
                .expand()
                .map_err(|e| format!("Failed to expand job '{}': {}", job.name, e))?;
            expanded_jobs.extend(expanded);
        }
        self.jobs = expanded_jobs;

        // Expand all files
        if let Some(ref files) = self.files {
            let mut expanded_files = Vec::new();
            for file in files {
                // Resolve parameters for this file
                let mut file_with_params = file.clone();
                file_with_params.parameters = Self::resolve_parameters(
                    &file.parameters,
                    &file.use_parameters,
                    &workflow_params,
                );
                // Clear use_parameters after resolution
                file_with_params.use_parameters = None;

                let expanded = file_with_params
                    .expand()
                    .map_err(|e| format!("Failed to expand file '{}': {}", file.name, e))?;
                expanded_files.extend(expanded);
            }
            self.files = Some(expanded_files);
        }

        Ok(())
    }

    /// Resolve parameters for a job or file
    ///
    /// Returns the effective parameters based on:
    /// 1. If local_params is set, return it (local overrides workflow)
    /// 2. If use_params is set, filter workflow_params to only those names
    /// 3. If neither is set, return None (job/file is not parameterized)
    fn resolve_parameters(
        local_params: &Option<HashMap<String, String>>,
        use_params: &Option<Vec<String>>,
        workflow_params: &Option<HashMap<String, String>>,
    ) -> Option<HashMap<String, String>> {
        // If local parameters are defined, use them (they take precedence)
        if local_params.is_some() {
            return local_params.clone();
        }

        // If no use_parameters specified, don't inherit workflow parameters
        // Jobs must explicitly opt-in via use_parameters
        let Some(param_names) = use_params else {
            return None;
        };

        // If no workflow parameters, nothing to inherit
        let Some(wf_params) = workflow_params else {
            return None;
        };

        // Filter workflow parameters to only those specified in use_parameters
        let mut filtered = HashMap::new();
        for name in param_names {
            if let Some(value) = wf_params.get(name) {
                filtered.insert(name.clone(), value.clone());
            }
            // Silently ignore parameters that don't exist in workflow
            // (could add validation here if desired)
        }
        if filtered.is_empty() {
            None
        } else {
            Some(filtered)
        }
    }

    /// Validate workflow actions
    pub fn validate_actions(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref actions) = self.actions {
            for action in actions {
                // Validate schedule_nodes actions
                if action.action_type == "schedule_nodes" {
                    // Ensure scheduler_type is provided
                    let scheduler_type = action
                        .scheduler_type
                        .as_ref()
                        .ok_or("schedule_nodes action requires scheduler_type")?;

                    // Ensure scheduler is provided
                    let scheduler = action
                        .scheduler
                        .as_ref()
                        .ok_or("schedule_nodes action requires scheduler")?;

                    // If scheduler_type is slurm, verify that a slurm_scheduler with that name exists
                    if scheduler_type == "slurm" {
                        let slurm_schedulers = self
                            .slurm_schedulers
                            .as_ref()
                            .ok_or("schedule_nodes action with scheduler_type=slurm requires slurm_schedulers to be defined")?;

                        let scheduler_exists = slurm_schedulers
                            .iter()
                            .any(|s| s.name.as_ref() == Some(scheduler));

                        if !scheduler_exists {
                            return Err(format!(
                                "schedule_nodes action references slurm_scheduler '{}' which does not exist",
                                scheduler
                            )
                            .into());
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Validate that multi-node schedulers are properly utilized.
    ///
    /// This validation ensures that when a scheduler allocates multiple nodes (nodes > 1)
    /// and `start_one_worker_per_node` is NOT set, there are jobs that actually require
    /// that many nodes. This prevents scenarios where:
    ///
    /// 1. A scheduler allocates 2+ nodes from Slurm
    /// 2. Jobs only need 1 node each
    /// 3. A single-node scheduler claims all jobs first
    /// 4. The multi-node allocation is wasted or jobs fail unexpectedly
    ///
    /// If `start_one_worker_per_node` is true, each node runs its own worker and can
    /// independently claim single-node jobs, so no validation is needed.
    pub fn validate_scheduler_node_requirements(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Build lookup maps for resource requirements and schedulers
        let resource_req_map: HashMap<&str, &ResourceRequirementsSpec> = self
            .resource_requirements
            .as_ref()
            .map(|reqs| reqs.iter().map(|r| (r.name.as_str(), r)).collect())
            .unwrap_or_default();

        let scheduler_map: HashMap<&str, &SlurmSchedulerSpec> = self
            .slurm_schedulers
            .as_ref()
            .map(|schedulers| {
                schedulers
                    .iter()
                    .filter_map(|s| s.name.as_ref().map(|n| (n.as_str(), s)))
                    .collect()
            })
            .unwrap_or_default();

        // If no schedulers or no actions, skip validation
        if scheduler_map.is_empty() {
            return Ok(());
        }

        let actions = match &self.actions {
            Some(actions) => actions,
            None => return Ok(()),
        };

        let mut errors: Vec<String> = Vec::new();

        // Check each schedule_nodes action
        for action in actions {
            if action.action_type != "schedule_nodes" {
                continue;
            }

            // Get scheduler name from action
            let scheduler_name = match &action.scheduler {
                Some(name) => name,
                None => continue, // Validation of required fields is done elsewhere
            };

            // Only validate slurm schedulers
            let scheduler_type = action.scheduler_type.as_deref().unwrap_or("");
            if scheduler_type != "slurm" {
                continue;
            }

            // Get the scheduler spec
            let scheduler = match scheduler_map.get(scheduler_name.as_str()) {
                Some(s) => s,
                None => continue, // Missing scheduler is validated elsewhere
            };

            // If scheduler only allocates 1 node, no special validation needed
            if scheduler.nodes <= 1 {
                continue;
            }

            // If start_one_worker_per_node is true, each node gets its own worker
            // and can claim single-node jobs independently - no validation needed
            if action.start_one_worker_per_node == Some(true) {
                continue;
            }

            // Multi-node scheduler WITHOUT start_one_worker_per_node:
            // Find jobs that reference this scheduler and check their num_nodes
            let jobs_using_scheduler: Vec<&JobSpec> = self
                .jobs
                .iter()
                .filter(|job| job.scheduler.as_ref() == Some(scheduler_name))
                .collect();

            // If no jobs explicitly reference this scheduler, this might be intentional
            // (jobs could be dynamically assigned), so do not treat as an error.
            if jobs_using_scheduler.is_empty() {
                continue;
            }

            // Check if any job using this scheduler has matching num_nodes
            let has_matching_job = jobs_using_scheduler.iter().any(|job| {
                let job_num_nodes = job
                    .resource_requirements
                    .as_ref()
                    .and_then(|name| resource_req_map.get(name.as_str()))
                    .map(|req| req.num_nodes)
                    .unwrap_or(1);
                job_num_nodes == scheduler.nodes
            });

            if !has_matching_job {
                let job_names: Vec<&str> = jobs_using_scheduler
                    .iter()
                    .map(|j| j.name.as_str())
                    .collect();
                errors.push(format!(
                    "Scheduler '{}' allocates {} nodes but none of the jobs using it \
                     ({}) have num_nodes={} in their resource requirements. \
                     Either set num_nodes={} on job resource requirements, \
                     or set start_one_worker_per_node=true on the schedule_nodes action \
                     to run independent workers on each node.",
                    scheduler_name,
                    scheduler.nodes,
                    job_names.join(", "),
                    scheduler.nodes,
                    scheduler.nodes
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "Scheduler node validation failed:\n  - {}",
                errors.join("\n  - ")
            )
            .into())
        }
    }

    /// Check if the workflow spec has an on_workflow_start action with schedule_nodes
    /// Returns true if such an action exists, false otherwise
    pub fn has_schedule_nodes_action(&self) -> bool {
        if let Some(ref actions) = self.actions {
            actions.iter().any(|action| {
                action.trigger_type == "on_workflow_start" && action.action_type == "schedule_nodes"
            })
        } else {
            false
        }
    }

    /// Validate a workflow specification without creating anything (dry-run mode)
    ///
    /// This method performs all validation steps that would occur during `create_workflow_from_spec`
    /// but without actually creating the workflow. It returns a detailed validation result including:
    /// - Whether validation passed
    /// - Any errors that would prevent creation
    /// - Any warnings about potential issues
    /// - A summary of what would be created (job count, file count, etc.)
    ///
    /// # Arguments
    /// * `path` - Path to the workflow specification file
    ///
    /// # Returns
    /// A `ValidationResult` containing validation status and summary
    pub fn validate_spec<P: AsRef<Path>>(path: P) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Step 1: Try to parse the spec file
        let mut spec = match Self::from_spec_file(&path) {
            Ok(spec) => spec,
            Err(e) => {
                return ValidationResult {
                    valid: false,
                    errors: vec![format!("Failed to parse specification file: {}", e)],
                    warnings: vec![],
                    summary: ValidationSummary {
                        workflow_name: String::new(),
                        workflow_description: None,
                        job_count: 0,
                        job_count_before_expansion: 0,
                        file_count: 0,
                        file_count_before_expansion: 0,
                        user_data_count: 0,
                        resource_requirements_count: 0,
                        slurm_scheduler_count: 0,
                        action_count: 0,
                        has_schedule_nodes_action: false,
                        job_names: vec![],
                        scheduler_names: vec![],
                    },
                };
            }
        };

        // Capture counts before expansion
        let job_count_before_expansion = spec.jobs.len();
        let file_count_before_expansion = spec.files.as_ref().map(|f| f.len()).unwrap_or(0);

        // Step 2: Expand parameters
        if let Err(e) = spec.expand_parameters() {
            errors.push(format!("Parameter expansion failed: {}", e));
        }

        // Step 3: Validate actions (basic structure validation)
        if let Err(e) = spec.validate_actions() {
            errors.push(format!("Action validation failed: {}", e));
        }

        // Step 4: Validate scheduler node requirements
        // This is an error by default (same as create_workflow_from_spec with skip_checks=false)
        if let Err(e) = spec.validate_scheduler_node_requirements() {
            errors.push(format!("{}", e));
        }

        // Step 5: Validate variable substitution
        if let Err(e) = spec.substitute_variables() {
            errors.push(format!("Variable substitution failed: {}", e));
        }

        // Step 6: Check for duplicate names
        // Check duplicate job names
        let mut job_names_set = HashSet::new();
        for job in &spec.jobs {
            if !job_names_set.insert(job.name.clone()) {
                errors.push(format!("Duplicate job name: '{}'", job.name));
            }
        }

        // Check duplicate file names
        if let Some(ref files) = spec.files {
            let mut file_names_set = HashSet::new();
            for file in files {
                if !file_names_set.insert(file.name.clone()) {
                    errors.push(format!("Duplicate file name: '{}'", file.name));
                }
            }
        }

        // Check duplicate user_data names
        if let Some(ref user_data_list) = spec.user_data {
            let mut user_data_names_set = HashSet::new();
            for ud in user_data_list {
                if let Some(ref name) = ud.name
                    && !user_data_names_set.insert(name.clone())
                {
                    errors.push(format!("Duplicate user_data name: '{}'", name));
                }
            }
        }

        // Check duplicate resource_requirements names
        if let Some(ref resource_reqs) = spec.resource_requirements {
            let mut rr_names_set = HashSet::new();
            for rr in resource_reqs {
                if !rr_names_set.insert(rr.name.clone()) {
                    errors.push(format!(
                        "Duplicate resource_requirements name: '{}'",
                        rr.name
                    ));
                }
            }
        }

        // Check duplicate slurm_scheduler names
        if let Some(ref schedulers) = spec.slurm_schedulers {
            let mut scheduler_names_set = HashSet::new();
            for sched in schedulers {
                if let Some(ref name) = sched.name
                    && !scheduler_names_set.insert(name.clone())
                {
                    errors.push(format!("Duplicate slurm_scheduler name: '{}'", name));
                }
            }
        }

        // Step 7: Build lookup sets for reference validation
        let job_names: HashSet<String> = spec.jobs.iter().map(|j| j.name.clone()).collect();
        let file_names: HashSet<String> = spec
            .files
            .as_ref()
            .map(|files| files.iter().map(|f| f.name.clone()).collect())
            .unwrap_or_default();
        let user_data_names: HashSet<String> = spec
            .user_data
            .as_ref()
            .map(|uds| uds.iter().filter_map(|ud| ud.name.clone()).collect())
            .unwrap_or_default();
        let resource_req_names: HashSet<String> = spec
            .resource_requirements
            .as_ref()
            .map(|rrs| rrs.iter().map(|rr| rr.name.clone()).collect())
            .unwrap_or_default();
        let scheduler_names_set: HashSet<String> = spec
            .slurm_schedulers
            .as_ref()
            .map(|scheds| scheds.iter().filter_map(|s| s.name.clone()).collect())
            .unwrap_or_default();

        // Step 8: Validate job references and build dependency graph
        let mut dependencies: HashMap<String, Vec<String>> = HashMap::new();

        for job in &spec.jobs {
            let mut job_deps = Vec::new();

            // Validate depends_on references
            if let Some(ref deps) = job.depends_on {
                for dep_name in deps {
                    if !job_names.contains(dep_name) {
                        errors.push(format!(
                            "Job '{}' depends_on non-existent job '{}'",
                            job.name, dep_name
                        ));
                    } else {
                        job_deps.push(dep_name.clone());
                    }
                }
            }

            // Validate depends_on_regexes
            if let Some(ref regexes) = job.depends_on_regexes {
                for regex_str in regexes {
                    match Regex::new(regex_str) {
                        Ok(re) => {
                            let mut found_match = false;
                            for other_name in &job_names {
                                if re.is_match(other_name) && !job_deps.contains(other_name) {
                                    job_deps.push(other_name.clone());
                                    found_match = true;
                                }
                            }
                            if !found_match {
                                errors.push(format!(
                                    "Job '{}' depends_on_regexes '{}' did not match any jobs",
                                    job.name, regex_str
                                ));
                            }
                        }
                        Err(e) => {
                            errors.push(format!(
                                "Job '{}' has invalid depends_on_regexes '{}': {}",
                                job.name, regex_str, e
                            ));
                        }
                    }
                }
            }

            dependencies.insert(job.name.clone(), job_deps);

            // Validate resource_requirements reference
            if let Some(ref rr_name) = job.resource_requirements
                && !resource_req_names.contains(rr_name)
            {
                errors.push(format!(
                    "Job '{}' references non-existent resource_requirements '{}'",
                    job.name, rr_name
                ));
            }

            // Validate scheduler reference
            if let Some(ref sched_name) = job.scheduler
                && !scheduler_names_set.contains(sched_name)
            {
                errors.push(format!(
                    "Job '{}' references non-existent scheduler '{}'",
                    job.name, sched_name
                ));
            }

            // Validate input_files references
            if let Some(ref files) = job.input_files {
                for file_name in files {
                    if !file_names.contains(file_name) {
                        errors.push(format!(
                            "Job '{}' input_files references non-existent file '{}'",
                            job.name, file_name
                        ));
                    }
                }
            }

            // Validate input_file_regexes
            if let Some(ref regexes) = job.input_file_regexes {
                for regex_str in regexes {
                    if let Err(e) = Regex::new(regex_str) {
                        errors.push(format!(
                            "Job '{}' has invalid input_file_regexes '{}': {}",
                            job.name, regex_str, e
                        ));
                    }
                }
            }

            // Validate output_files references
            if let Some(ref files) = job.output_files {
                for file_name in files {
                    if !file_names.contains(file_name) {
                        errors.push(format!(
                            "Job '{}' output_files references non-existent file '{}'",
                            job.name, file_name
                        ));
                    }
                }
            }

            // Validate output_file_regexes
            if let Some(ref regexes) = job.output_file_regexes {
                for regex_str in regexes {
                    if let Err(e) = Regex::new(regex_str) {
                        errors.push(format!(
                            "Job '{}' has invalid output_file_regexes '{}': {}",
                            job.name, regex_str, e
                        ));
                    }
                }
            }

            // Validate input_user_data references
            if let Some(ref uds) = job.input_user_data {
                for ud_name in uds {
                    if !user_data_names.contains(ud_name) {
                        errors.push(format!(
                            "Job '{}' input_user_data references non-existent user_data '{}'",
                            job.name, ud_name
                        ));
                    }
                }
            }

            // Validate input_user_data_regexes
            if let Some(ref regexes) = job.input_user_data_regexes {
                for regex_str in regexes {
                    if let Err(e) = Regex::new(regex_str) {
                        errors.push(format!(
                            "Job '{}' has invalid input_user_data_regexes '{}': {}",
                            job.name, regex_str, e
                        ));
                    }
                }
            }

            // Validate output_user_data references
            if let Some(ref uds) = job.output_user_data {
                for ud_name in uds {
                    if !user_data_names.contains(ud_name) {
                        errors.push(format!(
                            "Job '{}' output_user_data references non-existent user_data '{}'",
                            job.name, ud_name
                        ));
                    }
                }
            }

            // Validate output_user_data_regexes
            if let Some(ref regexes) = job.output_user_data_regexes {
                for regex_str in regexes {
                    if let Err(e) = Regex::new(regex_str) {
                        errors.push(format!(
                            "Job '{}' has invalid output_user_data_regexes '{}': {}",
                            job.name, regex_str, e
                        ));
                    }
                }
            }
        }

        // Step 9: Check for circular dependencies using topological sort
        {
            let mut remaining: HashSet<String> = job_names.clone();
            let mut processed = HashSet::new();

            while !remaining.is_empty() {
                let mut current_level = Vec::new();

                for job_name in &remaining {
                    if let Some(deps) = dependencies.get(job_name)
                        && deps.iter().all(|d| processed.contains(d))
                    {
                        current_level.push(job_name.clone());
                    }
                }

                if current_level.is_empty() {
                    // Find jobs involved in cycle for better error message
                    let cycle_jobs: Vec<&String> = remaining.iter().collect();
                    errors.push(format!(
                        "Circular dependency detected involving jobs: {}",
                        cycle_jobs
                            .iter()
                            .map(|s| format!("'{}'", s))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                    break;
                }

                for job_name in current_level {
                    remaining.remove(&job_name);
                    processed.insert(job_name);
                }
            }
        }

        // Step 10: Validate action references
        if let Some(ref actions) = spec.actions {
            for (idx, action) in actions.iter().enumerate() {
                let action_desc = format!("Action #{} ({})", idx + 1, action.action_type);

                // Validate job references in actions
                if let Some(ref job_refs) = action.jobs {
                    for job_name in job_refs {
                        if !job_names.contains(job_name) {
                            errors.push(format!(
                                "{} references non-existent job '{}'",
                                action_desc, job_name
                            ));
                        }
                    }
                }

                // Validate job_name_regexes in actions
                if let Some(ref regexes) = action.job_name_regexes {
                    for regex_str in regexes {
                        if let Err(e) = Regex::new(regex_str) {
                            errors.push(format!(
                                "{} has invalid job_name_regexes '{}': {}",
                                action_desc, regex_str, e
                            ));
                        }
                    }
                }

                // Validate scheduler reference in schedule_nodes actions
                if action.action_type == "schedule_nodes"
                    && let Some(ref sched_name) = action.scheduler
                {
                    let sched_type = action.scheduler_type.as_deref().unwrap_or("");
                    if sched_type == "slurm" && !scheduler_names_set.contains(sched_name) {
                        errors.push(format!(
                            "{} references non-existent slurm scheduler '{}'",
                            action_desc, sched_name
                        ));
                    }
                }
            }
        }

        // Step 11: Warn about heterogeneous schedulers without jobs_sort_method
        // This helps users avoid suboptimal job-to-node matching
        if let Some(ref schedulers) = spec.slurm_schedulers
            && schedulers.len() > 1
            && spec.jobs_sort_method.is_none()
        {
            // Check if schedulers have different resource profiles
            let has_different_gres = schedulers
                .iter()
                .map(|s| &s.gres)
                .collect::<HashSet<_>>()
                .len()
                > 1;
            let has_different_mem = schedulers
                .iter()
                .map(|s| &s.mem)
                .collect::<HashSet<_>>()
                .len()
                > 1;
            let has_different_walltime = schedulers
                .iter()
                .map(|s| &s.walltime)
                .collect::<HashSet<_>>()
                .len()
                > 1;
            let has_different_partition = schedulers
                .iter()
                .map(|s| &s.partition)
                .collect::<HashSet<_>>()
                .len()
                > 1;

            let has_heterogeneous_schedulers = has_different_gres
                || has_different_mem
                || has_different_walltime
                || has_different_partition;

            // Check if any jobs don't have explicit scheduler assignments
            let jobs_without_scheduler = spec.jobs.iter().filter(|j| j.scheduler.is_none()).count();

            if has_heterogeneous_schedulers && jobs_without_scheduler > 0 {
                let mut differences = Vec::new();
                if has_different_gres {
                    differences.push("GPUs (gres)");
                }
                if has_different_mem {
                    differences.push("memory (mem)");
                }
                if has_different_walltime {
                    differences.push("walltime");
                }
                if has_different_partition {
                    differences.push("partition");
                }

                warnings.push(format!(
                        "Workflow has {} schedulers with different {} but {} job(s) have no explicit \
                        scheduler assignment and jobs_sort_method is not set. The default sort method \
                        'gpus_runtime_memory' will be used (jobs sorted by GPUs, then runtime, then \
                        memory). If this doesn't match your workload, consider setting jobs_sort_method \
                        explicitly to 'gpus_memory_runtime' (prioritize memory over runtime) or 'none' \
                        (no sorting).",
                        schedulers.len(),
                        differences.join(", "),
                        jobs_without_scheduler
                    ));
            }
        }

        // Collect scheduler names for summary
        let scheduler_names: Vec<String> = spec
            .slurm_schedulers
            .as_ref()
            .map(|schedulers| schedulers.iter().filter_map(|s| s.name.clone()).collect())
            .unwrap_or_default();

        // Build summary
        let summary = ValidationSummary {
            workflow_name: spec.name.clone(),
            workflow_description: spec.description.clone(),
            job_count: spec.jobs.len(),
            job_count_before_expansion,
            file_count: spec.files.as_ref().map(|f| f.len()).unwrap_or(0),
            file_count_before_expansion,
            user_data_count: spec.user_data.as_ref().map(|u| u.len()).unwrap_or(0),
            resource_requirements_count: spec
                .resource_requirements
                .as_ref()
                .map(|r| r.len())
                .unwrap_or(0),
            slurm_scheduler_count: spec.slurm_schedulers.as_ref().map(|s| s.len()).unwrap_or(0),
            action_count: spec.actions.as_ref().map(|a| a.len()).unwrap_or(0),
            has_schedule_nodes_action: spec.has_schedule_nodes_action(),
            job_names: spec.jobs.iter().map(|j| j.name.clone()).collect(),
            scheduler_names,
        };

        ValidationResult {
            valid: errors.is_empty(),
            errors,
            warnings,
            summary,
        }
    }

    /// Create a WorkflowModel on the server from a JSON file
    /// Create a workflow from a specification file (JSON, JSON5, or YAML) with all associated data
    ///
    /// This function will create the workflow and all associated models (files, user data, etc.)
    /// If any errors occur, the workflow will be deleted (which cascades to all other objects)
    ///
    /// # Arguments
    /// * `config` - Server configuration
    /// * `path` - Path to the workflow specification file
    /// * `user` - User that owns the workflow
    /// * `enable_resource_monitoring` - Whether to enable resource monitoring by default
    /// * `skip_checks` - Skip validation checks (use with caution)
    pub fn create_workflow_from_spec<P: AsRef<Path>>(
        config: &Configuration,
        path: P,
        user: &str,
        enable_resource_monitoring: bool,
        skip_checks: bool,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        // Step 1: Deserialize the WorkflowSpecification from spec file
        let mut spec = Self::from_spec_file(path)?;
        spec.user = Some(user.to_string());

        // Apply default resource monitoring if enabled and not already configured
        if enable_resource_monitoring && spec.resource_monitor.is_none() {
            spec.resource_monitor = Some(crate::client::resource_monitor::ResourceMonitorConfig {
                enabled: true,
                granularity: crate::client::resource_monitor::MonitorGranularity::Summary,
                sample_interval_seconds: 5,
                generate_plots: false,
            });
        }

        // Step 1.25: Expand parameterized jobs and files
        spec.expand_parameters()?;

        // Step 1.4: Validate workflow actions
        spec.validate_actions()?;

        // Step 1.45: Validate scheduler node requirements
        if !skip_checks {
            spec.validate_scheduler_node_requirements()?;
        }

        // Step 1.5: Perform variable substitution in commands
        spec.substitute_variables()?;

        // Step 2: Create WorkflowModel
        let workflow_id = Self::create_workflow(config, &spec)?;

        // If any step fails, delete the workflow (which cascades to all other objects)
        let rollback = |workflow_id: i64| {
            let _ = default_api::delete_workflow(config, workflow_id, None);
        };

        // Step 3: Create supporting models and build name-to-id mappings
        let file_name_to_id = match Self::create_files(config, workflow_id, &spec) {
            Ok(mapping) => mapping,
            Err(e) => {
                rollback(workflow_id);
                return Err(e);
            }
        };

        let user_data_name_to_id = match Self::create_user_data(config, workflow_id, &spec) {
            Ok(mapping) => mapping,
            Err(e) => {
                rollback(workflow_id);
                return Err(e);
            }
        };

        let resource_req_name_to_id =
            match Self::create_resource_requirements(config, workflow_id, &spec) {
                Ok(mapping) => mapping,
                Err(e) => {
                    rollback(workflow_id);
                    return Err(e);
                }
            };

        let slurm_scheduler_to_id = match Self::create_slurm_schedulers(config, workflow_id, &spec)
        {
            Ok(mapping) => mapping,
            Err(e) => {
                rollback(workflow_id);
                return Err(e);
            }
        };

        let failure_handler_name_to_id =
            match Self::create_failure_handlers(config, workflow_id, &spec) {
                Ok(mapping) => mapping,
                Err(e) => {
                    rollback(workflow_id);
                    return Err(e);
                }
            };

        // Step 4: Create JobModels (with dependencies set during creation)
        let (job_name_to_id, _created_jobs) = match Self::create_jobs(
            config,
            workflow_id,
            &spec,
            &file_name_to_id,
            &user_data_name_to_id,
            &resource_req_name_to_id,
            &slurm_scheduler_to_id,
            &failure_handler_name_to_id,
        ) {
            Ok((mapping, jobs)) => (mapping, jobs),
            Err(e) => {
                rollback(workflow_id);
                return Err(e);
            }
        };

        // Step 5: Create workflow actions
        match Self::create_actions(
            config,
            workflow_id,
            &spec,
            &slurm_scheduler_to_id,
            &job_name_to_id,
        ) {
            Ok(_) => {}
            Err(e) => {
                rollback(workflow_id);
                return Err(e);
            }
        }

        Ok(workflow_id)
    }

    /// Create the workflow on the server
    fn create_workflow(
        config: &Configuration,
        spec: &WorkflowSpec,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        let user = spec.user.clone().unwrap_or_else(|| "unknown".to_string());
        let mut workflow_model = models::WorkflowModel::new(spec.name.clone(), user);
        workflow_model.description = spec.description.clone();

        // Set compute node configuration fields if present
        if let Some(value) = spec.compute_node_expiration_buffer_seconds {
            workflow_model.compute_node_expiration_buffer_seconds = Some(value);
        }
        if let Some(value) = spec.compute_node_wait_for_new_jobs_seconds {
            workflow_model.compute_node_wait_for_new_jobs_seconds = Some(value);
        } else {
            // Default must be >= completion_check_interval_secs + job_completion_poll_interval
            // to avoid exiting before dependent jobs are unblocked. See ComputeNodeRules.
            workflow_model.compute_node_wait_for_new_jobs_seconds = Some(90);
        }
        if let Some(value) = spec.compute_node_ignore_workflow_completion {
            workflow_model.compute_node_ignore_workflow_completion = Some(value);
        }
        if let Some(value) = spec.compute_node_wait_for_healthy_database_minutes {
            workflow_model.compute_node_wait_for_healthy_database_minutes = Some(value);
        }
        if let Some(ref value) = spec.jobs_sort_method {
            workflow_model.jobs_sort_method = Some(*value);
        }

        // Serialize resource_monitor config if present
        if let Some(ref resource_monitor) = spec.resource_monitor {
            let config_json = serde_json::to_string(resource_monitor)
                .map_err(|e| format!("Failed to serialize resource monitor config: {}", e))?;
            workflow_model.resource_monitor_config = Some(config_json);
        }

        // Validate and serialize slurm_defaults if present
        if let Some(ref slurm_defaults) = spec.slurm_defaults {
            // Validate that no excluded parameters are present
            slurm_defaults.validate()?;
            let config_json = serde_json::to_string(slurm_defaults)
                .map_err(|e| format!("Failed to serialize slurm_defaults config: {}", e))?;
            workflow_model.slurm_defaults = Some(config_json);
        }

        // Set use_pending_failed if present
        if let Some(value) = spec.use_pending_failed {
            workflow_model.use_pending_failed = Some(value);
        }

        // Set project if present
        if let Some(ref value) = spec.project {
            workflow_model.project = Some(value.clone());
        }

        // Set metadata if present
        if let Some(ref value) = spec.metadata {
            workflow_model.metadata = Some(value.clone());
        }

        let created_workflow = default_api::create_workflow(config, workflow_model)
            .map_err(|e| format!("Failed to create workflow: {:?}", e))?;

        created_workflow
            .id
            .ok_or("Created workflow missing ID".into())
    }

    /// Create FileModels and build name-to-id mapping
    fn create_files(
        config: &Configuration,
        workflow_id: i64,
        spec: &WorkflowSpec,
    ) -> Result<HashMap<String, i64>, Box<dyn std::error::Error>> {
        let mut file_name_to_id = HashMap::new();

        if let Some(files) = &spec.files {
            for file_spec in files {
                // Check for duplicate names
                if file_name_to_id.contains_key(&file_spec.name) {
                    return Err(format!("Duplicate file name: {}", file_spec.name).into());
                }

                let file_model = models::FileModel {
                    id: None, // Server will assign ID
                    workflow_id,
                    name: file_spec.name.clone(),
                    path: file_spec.path.clone(),
                    st_mtime: None, // Not included in specification
                };

                let created_file = default_api::create_file(config, file_model)
                    .map_err(|e| format!("Failed to create file {}: {:?}", file_spec.name, e))?;

                let file_id = created_file.id.ok_or("Created file missing ID")?;
                file_name_to_id.insert(file_spec.name.clone(), file_id);
            }
        }

        Ok(file_name_to_id)
    }

    /// Create UserDataModels and build name-to-id mapping
    fn create_user_data(
        config: &Configuration,
        workflow_id: i64,
        spec: &WorkflowSpec,
    ) -> Result<HashMap<String, i64>, Box<dyn std::error::Error>> {
        let mut user_data_name_to_id = HashMap::new();

        if let Some(user_data_list) = &spec.user_data {
            for user_data_spec in user_data_list {
                if let Some(name) = &user_data_spec.name {
                    // Check for duplicate names
                    if user_data_name_to_id.contains_key(name) {
                        return Err(format!("Duplicate user data name: {}", name).into());
                    }

                    let user_data_model = models::UserDataModel {
                        id: None, // Server will assign ID
                        workflow_id,
                        is_ephemeral: user_data_spec.is_ephemeral,
                        name: name.clone(),
                        data: user_data_spec.data.clone(),
                    };

                    let created_user_data =
                        default_api::create_user_data(config, user_data_model, None, None)
                            .map_err(|e| format!("Failed to create user data {}: {:?}", name, e))?;

                    let user_data_id =
                        created_user_data.id.ok_or("Created user data missing ID")?;
                    user_data_name_to_id.insert(name.clone(), user_data_id);
                }
            }
        }

        Ok(user_data_name_to_id)
    }

    /// Create ResourceRequirementsModels and build name-to-id mapping
    fn create_resource_requirements(
        config: &Configuration,
        workflow_id: i64,
        spec: &WorkflowSpec,
    ) -> Result<HashMap<String, i64>, Box<dyn std::error::Error>> {
        let mut resource_req_name_to_id = HashMap::new();

        if let Some(resource_requirements) = &spec.resource_requirements {
            for resource_req_spec in resource_requirements {
                // Check for duplicate names
                if resource_req_name_to_id.contains_key(&resource_req_spec.name) {
                    return Err(format!(
                        "Duplicate resource requirements name: {}",
                        resource_req_spec.name
                    )
                    .into());
                }

                let resource_req_model = models::ResourceRequirementsModel {
                    id: None, // Server will assign ID
                    workflow_id,
                    name: resource_req_spec.name.clone(),
                    num_cpus: resource_req_spec.num_cpus,
                    num_gpus: resource_req_spec.num_gpus,
                    num_nodes: resource_req_spec.num_nodes,
                    memory: resource_req_spec.memory.clone(),
                    runtime: resource_req_spec.runtime.clone(),
                };

                let created_resource_req =
                    default_api::create_resource_requirements(config, resource_req_model).map_err(
                        |e| {
                            format!(
                                "Failed to create resource requirements {}: {:?}",
                                resource_req_spec.name, e
                            )
                        },
                    )?;

                let resource_req_id = created_resource_req
                    .id
                    .ok_or("Created resource requirements missing ID")?;
                resource_req_name_to_id.insert(resource_req_spec.name.clone(), resource_req_id);
            }
        }

        Ok(resource_req_name_to_id)
    }

    /// Create SlurmSchedulerModels and build name-to-id mapping
    fn create_slurm_schedulers(
        config: &Configuration,
        workflow_id: i64,
        spec: &WorkflowSpec,
    ) -> Result<HashMap<String, i64>, Box<dyn std::error::Error>> {
        let mut slurm_scheduler_to_id = HashMap::new();

        if let Some(slurm_schedulers) = &spec.slurm_schedulers {
            for scheduler_spec in slurm_schedulers {
                if let Some(name) = &scheduler_spec.name {
                    // Check for duplicate names
                    if slurm_scheduler_to_id.contains_key(name) {
                        return Err(format!("Duplicate slurm scheduler name: {}", name).into());
                    }

                    let scheduler_model = models::SlurmSchedulerModel {
                        id: None, // Server will assign ID
                        workflow_id,
                        name: scheduler_spec.name.clone(),
                        account: scheduler_spec.account.clone(),
                        gres: scheduler_spec.gres.clone(),
                        mem: scheduler_spec.mem.clone(),
                        nodes: scheduler_spec.nodes,
                        ntasks_per_node: scheduler_spec.ntasks_per_node,
                        partition: scheduler_spec.partition.clone(),
                        qos: scheduler_spec.qos.clone(),
                        tmp: scheduler_spec.tmp.clone(),
                        walltime: scheduler_spec.walltime.clone(),
                        extra: scheduler_spec.extra.clone(),
                    };

                    let created_scheduler =
                        default_api::create_slurm_scheduler(config, scheduler_model).map_err(
                            |e| format!("Failed to create slurm scheduler {}: {:?}", name, e),
                        )?;

                    let scheduler_id = created_scheduler
                        .id
                        .ok_or("Created slurm scheduler missing ID")?;
                    slurm_scheduler_to_id.insert(name.clone(), scheduler_id);
                }
            }
        }

        Ok(slurm_scheduler_to_id)
    }

    /// Create failure handlers and build name-to-id mapping
    fn create_failure_handlers(
        config: &Configuration,
        workflow_id: i64,
        spec: &WorkflowSpec,
    ) -> Result<HashMap<String, i64>, Box<dyn std::error::Error>> {
        let mut failure_handler_name_to_id = HashMap::new();

        if let Some(failure_handlers) = &spec.failure_handlers {
            for handler_spec in failure_handlers {
                // Check for duplicate names
                if failure_handler_name_to_id.contains_key(&handler_spec.name) {
                    return Err(
                        format!("Duplicate failure handler name: {}", handler_spec.name).into(),
                    );
                }

                // Serialize the rules to JSON
                let rules_json = serde_json::to_string(&handler_spec.rules)
                    .map_err(|e| format!("Failed to serialize failure handler rules: {}", e))?;

                let handler_model = models::FailureHandlerModel::new(
                    workflow_id,
                    handler_spec.name.clone(),
                    rules_json,
                );

                let created_handler = default_api::create_failure_handler(config, handler_model)
                    .map_err(|e| {
                        format!(
                            "Failed to create failure handler {}: {:?}",
                            handler_spec.name, e
                        )
                    })?;

                let handler_id = created_handler
                    .id
                    .ok_or("Created failure handler missing ID")?;
                failure_handler_name_to_id.insert(handler_spec.name.clone(), handler_id);
            }
        }

        Ok(failure_handler_name_to_id)
    }

    /// Create workflow actions
    fn create_actions(
        config: &Configuration,
        workflow_id: i64,
        spec: &WorkflowSpec,
        slurm_scheduler_to_id: &HashMap<String, i64>,
        job_name_to_id: &HashMap<String, i64>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(actions) = &spec.actions {
            for action_spec in actions {
                // Resolve job_names and job_name_regexes to job_ids
                let job_ids =
                    if action_spec.jobs.is_some() || action_spec.job_name_regexes.is_some() {
                        let mut matched_job_ids = Vec::new();

                        // Match exact job names
                        if let Some(ref patterns) = action_spec.jobs {
                            for pattern in patterns {
                                if let Some(job_id) = job_name_to_id.get(pattern) {
                                    matched_job_ids.push(*job_id);
                                } else {
                                    return Err(format!(
                                        "Action references job '{}' which does not exist",
                                        pattern
                                    )
                                    .into());
                                }
                            }
                        }

                        // Match job names using regexes
                        if let Some(ref regexes) = action_spec.job_name_regexes {
                            use regex::Regex;
                            for regex_str in regexes {
                                let re = Regex::new(regex_str)
                                    .map_err(|e| format!("Invalid regex '{}': {}", regex_str, e))?;

                                for (job_name, job_id) in job_name_to_id {
                                    if re.is_match(job_name) && !matched_job_ids.contains(job_id) {
                                        matched_job_ids.push(*job_id);
                                    }
                                }
                            }
                        }

                        if matched_job_ids.is_empty() {
                            return Err("Action did not match any jobs".into());
                        }

                        Some(matched_job_ids)
                    } else {
                        None
                    };

                // Build action_config JSON based on action_type
                let action_config = match action_spec.action_type.as_str() {
                    "run_commands" => {
                        let commands = action_spec
                            .commands
                            .as_ref()
                            .ok_or("run_commands action requires 'commands' field")?;
                        serde_json::json!({
                            "commands": commands
                        })
                    }
                    "schedule_nodes" => {
                        let scheduler_type = action_spec
                            .scheduler_type
                            .as_ref()
                            .ok_or("schedule_nodes action requires 'scheduler_type' field")?;
                        let scheduler = action_spec
                            .scheduler
                            .as_ref()
                            .ok_or("schedule_nodes action requires 'scheduler' field")?;

                        // Translate scheduler to scheduler_id
                        let scheduler_id = if scheduler_type == "slurm" {
                            slurm_scheduler_to_id
                                .get(scheduler)
                                .ok_or(format!("Slurm scheduler '{}' not found", scheduler))?
                        } else {
                            // For other scheduler types, we might need a different lookup
                            // For now, just use 0 as placeholder
                            &0
                        };

                        let mut config = serde_json::json!({
                            "scheduler_type": scheduler_type,
                            "scheduler_id": scheduler_id,
                            "num_allocations": action_spec.num_allocations.unwrap_or(1),
                            "start_one_worker_per_node": action_spec.start_one_worker_per_node.unwrap_or(false),
                        });
                        // Only include max_parallel_jobs if explicitly specified
                        if let Some(max_parallel_jobs) = action_spec.max_parallel_jobs {
                            config["max_parallel_jobs"] = serde_json::json!(max_parallel_jobs);
                        }
                        config
                    }
                    _ => {
                        return Err(
                            format!("Unknown action_type: {}", action_spec.action_type).into()
                        );
                    }
                };

                // Create the action via API
                let action_body = serde_json::json!({
                    "workflow_id": workflow_id,
                    "trigger_type": action_spec.trigger_type,
                    "action_type": action_spec.action_type,
                    "action_config": action_config,
                    "job_ids": job_ids,
                    "persistent": action_spec.persistent.unwrap_or(false),
                });

                default_api::create_workflow_action(config, workflow_id, action_body)
                    .map_err(|e| format!("Failed to create workflow action: {:?}", e))?;
            }
        }

        Ok(())
    }

    /// Helper function to resolve names and regex patterns to IDs
    /// Returns a vector of IDs matching either the exact names or the regex patterns
    fn resolve_names_and_regexes(
        exact_names: &Option<Vec<String>>,
        regex_patterns: &Option<Vec<String>>,
        name_to_id: &HashMap<String, i64>,
        resource_type: &str, // e.g., "Input file", "Job dependency"
        job_name: &str,      // The job that needs this resource
    ) -> Result<Vec<i64>, Box<dyn std::error::Error>> {
        let mut ids = Vec::new();

        // Add IDs for exact name matches
        if let Some(names) = exact_names {
            for name in names {
                match name_to_id.get(name) {
                    Some(&id) => ids.push(id),
                    None => {
                        return Err(format!(
                            "{} '{}' not found for job '{}'",
                            resource_type, name, job_name
                        )
                        .into());
                    }
                }
            }
        }

        // Add IDs for regex pattern matches
        if let Some(patterns) = regex_patterns {
            for pattern_str in patterns {
                let re = Regex::new(pattern_str).map_err(|e| {
                    format!(
                        "Invalid regex '{}' for {} in job '{}': {}",
                        pattern_str,
                        resource_type.to_lowercase(),
                        job_name,
                        e
                    )
                })?;

                let mut found_match = false;
                for (name, &id) in name_to_id {
                    if re.is_match(name) && !ids.contains(&id) {
                        ids.push(id);
                        found_match = true;
                    }
                }

                // Error if regex didn't match anything
                if !found_match {
                    return Err(format!(
                        "{} regex '{}' did not match any names for job '{}'",
                        resource_type, pattern_str, job_name
                    )
                    .into());
                }
            }
        }

        Ok(ids)
    }

    /// Topologically sort jobs into levels based on dependencies
    /// Returns a vector of levels, where each level contains jobs that can be created together
    fn topological_sort_jobs<'a>(
        jobs: &'a [JobSpec],
        dependencies: &HashMap<String, Vec<String>>,
    ) -> Result<Vec<Vec<&'a JobSpec>>, Box<dyn std::error::Error>> {
        let mut levels = Vec::new();
        let mut remaining: HashSet<String> = jobs.iter().map(|j| j.name.clone()).collect();
        let mut processed = HashSet::new();

        while !remaining.is_empty() {
            let mut current_level = Vec::new();

            // Find all jobs whose dependencies are satisfied
            for job in jobs {
                if remaining.contains(&job.name) {
                    let deps = dependencies.get(&job.name).unwrap();
                    if deps.iter().all(|d| processed.contains(d)) {
                        current_level.push(job);
                    }
                }
            }

            if current_level.is_empty() {
                return Err("Circular dependency detected in job graph".into());
            }

            // Mark these jobs as processed
            for job in &current_level {
                remaining.remove(&job.name);
                processed.insert(job.name.clone());
            }

            levels.push(current_level);
        }

        Ok(levels)
    }

    /// Create JobModels with proper ID mapping using bulk API in batches of 1000
    /// Jobs are created in dependency order with depends_on_job_ids set during initial creation
    #[allow(clippy::type_complexity, clippy::too_many_arguments)]
    fn create_jobs(
        config: &Configuration,
        workflow_id: i64,
        spec: &WorkflowSpec,
        file_name_to_id: &HashMap<String, i64>,
        user_data_name_to_id: &HashMap<String, i64>,
        resource_req_name_to_id: &HashMap<String, i64>,
        slurm_scheduler_to_id: &HashMap<String, i64>,
        failure_handler_name_to_id: &HashMap<String, i64>,
    ) -> Result<(HashMap<String, i64>, HashMap<String, models::JobModel>), Box<dyn std::error::Error>>
    {
        let mut job_name_to_id = HashMap::new();
        let mut created_jobs = HashMap::new();

        // Step 1: Build a set of all job names for validation
        let all_job_names: std::collections::HashSet<String> =
            spec.jobs.iter().map(|j| j.name.clone()).collect();

        // Step 2: Build dependency graph (job_name -> Vec<dependency_job_names>)
        let mut dependencies: HashMap<String, Vec<String>> = HashMap::new();

        for job_spec in &spec.jobs {
            let mut deps = Vec::new();

            // Add explicit dependencies
            if let Some(ref names) = job_spec.depends_on {
                for dep_name in names {
                    // Validate that the dependency exists
                    if !all_job_names.contains(dep_name) {
                        return Err(format!(
                            "Blocking job '{}' not found for job '{}'",
                            dep_name, job_spec.name
                        )
                        .into());
                    }
                    deps.push(dep_name.clone());
                }
            }

            // Resolve regex dependencies
            if let Some(ref regexes) = job_spec.depends_on_regexes {
                for regex_str in regexes {
                    let re = Regex::new(regex_str).map_err(|e| {
                        format!(
                            "Invalid regex '{}' in job '{}': {}",
                            regex_str, job_spec.name, e
                        )
                    })?;
                    let mut found_match = false;
                    for other_job in &spec.jobs {
                        if re.is_match(&other_job.name) && !deps.contains(&other_job.name) {
                            deps.push(other_job.name.clone());
                            found_match = true;
                        }
                    }
                    // Error if regex didn't match anything
                    if !found_match {
                        return Err(format!(
                            "Blocking job regex '{}' did not match any jobs for job '{}'",
                            regex_str, job_spec.name
                        )
                        .into());
                    }
                }
            }

            dependencies.insert(job_spec.name.clone(), deps);
        }

        // Step 3: Topologically sort jobs into levels
        let levels = Self::topological_sort_jobs(&spec.jobs, &dependencies)?;

        // Step 4: Create jobs level by level
        const BATCH_SIZE: usize = 1000;

        for level in levels {
            // Create job models for this level with depends_on_job_ids resolved
            let mut job_models = Vec::new();
            let mut job_spec_mapping = Vec::new();

            for job_spec in level {
                let mut job_model = models::JobModel::new(
                    workflow_id,
                    job_spec.name.clone(),
                    job_spec.command.clone(),
                );

                // Set optional fields
                job_model.invocation_script = job_spec.invocation_script.clone();
                // Only override cancel_on_blocking_job_failure if explicitly set in spec
                // (JobModel::new() defaults to Some(true))
                if job_spec.cancel_on_blocking_job_failure.is_some() {
                    job_model.cancel_on_blocking_job_failure =
                        job_spec.cancel_on_blocking_job_failure;
                }
                // Only override supports_termination if explicitly set in spec
                // (JobModel::new() defaults to Some(false))
                if job_spec.supports_termination.is_some() {
                    job_model.supports_termination = job_spec.supports_termination;
                }

                // Map file names and regexes to IDs
                let input_file_ids = Self::resolve_names_and_regexes(
                    &job_spec.input_files,
                    &job_spec.input_file_regexes,
                    file_name_to_id,
                    "Input file",
                    &job_spec.name,
                )?;
                if !input_file_ids.is_empty() {
                    job_model.input_file_ids = Some(input_file_ids);
                }

                let output_file_ids = Self::resolve_names_and_regexes(
                    &job_spec.output_files,
                    &job_spec.output_file_regexes,
                    file_name_to_id,
                    "Output file",
                    &job_spec.name,
                )?;
                if !output_file_ids.is_empty() {
                    job_model.output_file_ids = Some(output_file_ids);
                }

                // Map user data names and regexes to IDs
                let input_user_data_ids = Self::resolve_names_and_regexes(
                    &job_spec.input_user_data,
                    &job_spec.input_user_data_regexes,
                    user_data_name_to_id,
                    "Input user data",
                    &job_spec.name,
                )?;
                if !input_user_data_ids.is_empty() {
                    job_model.input_user_data_ids = Some(input_user_data_ids);
                }

                let output_user_data_ids = Self::resolve_names_and_regexes(
                    &job_spec.output_user_data,
                    &job_spec.output_user_data_regexes,
                    user_data_name_to_id,
                    "Output user data",
                    &job_spec.name,
                )?;
                if !output_user_data_ids.is_empty() {
                    job_model.output_user_data_ids = Some(output_user_data_ids);
                }

                // Map resource requirements name to ID
                if let Some(resource_req_name) = &job_spec.resource_requirements {
                    match resource_req_name_to_id.get(resource_req_name) {
                        Some(&resource_req_id) => {
                            job_model.resource_requirements_id = Some(resource_req_id)
                        }
                        None => {
                            return Err(format!(
                                "Resource requirements '{}' not found for job '{}'",
                                resource_req_name, job_spec.name
                            )
                            .into());
                        }
                    }
                }

                // Map scheduler name to ID
                if let Some(scheduler) = &job_spec.scheduler {
                    match slurm_scheduler_to_id.get(scheduler) {
                        Some(&scheduler_id) => job_model.scheduler_id = Some(scheduler_id),
                        None => {
                            return Err(format!(
                                "Scheduler '{}' not found for job '{}'",
                                scheduler, job_spec.name
                            )
                            .into());
                        }
                    }
                }

                // Map failure handler name to ID
                if let Some(failure_handler) = &job_spec.failure_handler {
                    match failure_handler_name_to_id.get(failure_handler) {
                        Some(&handler_id) => job_model.failure_handler_id = Some(handler_id),
                        None => {
                            return Err(format!(
                                "Failure handler '{}' not found for job '{}'",
                                failure_handler, job_spec.name
                            )
                            .into());
                        }
                    }
                }

                // NEW: Resolve depends_on_job_ids using accumulated job_name_to_id
                let dep_names = dependencies.get(&job_spec.name).unwrap();
                if !dep_names.is_empty() {
                    let mut depends_on_ids = Vec::new();
                    for dep_name in dep_names {
                        let dep_id = job_name_to_id.get(dep_name).ok_or_else(|| {
                            format!(
                                "Dependency '{}' not found for job '{}' (not yet created)",
                                dep_name, job_spec.name
                            )
                        })?;
                        depends_on_ids.push(*dep_id);
                    }
                    job_model.depends_on_job_ids = Some(depends_on_ids);
                }

                job_models.push(job_model);
                job_spec_mapping.push(job_spec);
            }

            // Create this level's jobs in batches of 1000
            for (batch_index, batch) in job_models.chunks(BATCH_SIZE).enumerate() {
                let jobs_model = models::JobsModel::new(batch.to_vec());

                let response = default_api::create_jobs(config, jobs_model).map_err(|e| {
                    format!(
                        "Failed to create batch {} of jobs: {:?}",
                        batch_index + 1,
                        e
                    )
                })?;

                let created_batch = response.jobs.ok_or("Create jobs response missing items")?;

                if created_batch.len() != batch.len() {
                    return Err(format!(
                        "Batch {} returned {} jobs but expected {}",
                        batch_index + 1,
                        created_batch.len(),
                        batch.len()
                    )
                    .into());
                }

                // Update mappings
                let batch_start = batch_index * BATCH_SIZE;
                for (i, created_job) in created_batch.iter().enumerate() {
                    let job_spec = job_spec_mapping[batch_start + i];
                    let job_id = created_job.id.ok_or("Created job missing ID")?;
                    job_name_to_id.insert(job_spec.name.clone(), job_id);
                    created_jobs.insert(job_spec.name.clone(), created_job.clone());
                }
            }
        }

        Ok((job_name_to_id, created_jobs))
    }

    /// Convert a byte offset to (line, column) for error reporting
    #[cfg(feature = "client")]
    fn offset_to_line_col(content: &str, offset: usize) -> (usize, usize) {
        let mut line = 1;
        let mut col = 1;
        for (i, ch) in content.char_indices() {
            if i >= offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    /// Convert a KDL parameters block to a JSON object
    #[cfg(feature = "client")]
    fn kdl_parameters_to_json(
        node: &KdlNode,
    ) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
        let Some(children) = node.children() else {
            return Ok(None);
        };

        let mut params = serde_json::Map::new();
        for child in children.nodes() {
            let param_name = child.name().value().to_string();
            let param_value = child
                .entries()
                .first()
                .and_then(|e| e.value().as_string())
                .ok_or_else(|| format!("Parameter '{}' must have a string value", param_name))?
                .to_string();
            params.insert(param_name, serde_json::Value::String(param_value));
        }

        if params.is_empty() {
            Ok(None)
        } else {
            Ok(Some(serde_json::Value::Object(params)))
        }
    }

    /// Convert a KDL job node to a JSON object
    #[cfg(feature = "client")]
    fn kdl_job_to_json(node: &KdlNode) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let name = node
            .entries()
            .first()
            .and_then(|e| e.value().as_string())
            .ok_or("job must have a name")?
            .to_string();

        let mut obj = serde_json::Map::new();
        obj.insert("name".to_string(), serde_json::Value::String(name));

        // Collect array fields
        let mut depends_on: Vec<serde_json::Value> = Vec::new();
        let mut depends_on_regexes: Vec<serde_json::Value> = Vec::new();
        let mut input_files: Vec<serde_json::Value> = Vec::new();
        let mut output_files: Vec<serde_json::Value> = Vec::new();
        let mut input_user_data: Vec<serde_json::Value> = Vec::new();
        let mut output_user_data: Vec<serde_json::Value> = Vec::new();

        if let Some(children) = node.children() {
            for child in children.nodes() {
                match child.name().value() {
                    "command" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "command".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    "invocation_script" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "invocation_script".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    "cancel_on_blocking_job_failure" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_bool()) {
                            obj.insert(
                                "cancel_on_blocking_job_failure".to_string(),
                                serde_json::Value::Bool(v),
                            );
                        }
                    }
                    "supports_termination" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_bool()) {
                            obj.insert(
                                "supports_termination".to_string(),
                                serde_json::Value::Bool(v),
                            );
                        }
                    }
                    "resource_requirements" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "resource_requirements".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    "failure_handler" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "failure_handler".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    "depends_on" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            depends_on.push(serde_json::Value::String(v.to_string()));
                        }
                    }
                    "depends_on_regexes" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            depends_on_regexes.push(serde_json::Value::String(v.to_string()));
                        }
                    }
                    "input_file" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            input_files.push(serde_json::Value::String(v.to_string()));
                        }
                    }
                    "output_file" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            output_files.push(serde_json::Value::String(v.to_string()));
                        }
                    }
                    "input_user_data" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            input_user_data.push(serde_json::Value::String(v.to_string()));
                        }
                    }
                    "output_user_data" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            output_user_data.push(serde_json::Value::String(v.to_string()));
                        }
                    }
                    "scheduler" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "scheduler".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    "parameters" => {
                        if let Some(params) = Self::kdl_parameters_to_json(child)? {
                            obj.insert("parameters".to_string(), params);
                        }
                    }
                    "parameter_mode" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "parameter_mode".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    "use_parameters" => {
                        let param_names: Vec<serde_json::Value> = child
                            .entries()
                            .iter()
                            .filter_map(|e| {
                                e.value()
                                    .as_string()
                                    .map(|s| serde_json::Value::String(s.to_string()))
                            })
                            .collect();
                        if !param_names.is_empty() {
                            obj.insert(
                                "use_parameters".to_string(),
                                serde_json::Value::Array(param_names),
                            );
                        }
                    }
                    _ => {}
                }
            }
        }

        // Add collected arrays if non-empty
        if !depends_on.is_empty() {
            obj.insert(
                "depends_on".to_string(),
                serde_json::Value::Array(depends_on),
            );
        }
        if !depends_on_regexes.is_empty() {
            obj.insert(
                "depends_on_regexes".to_string(),
                serde_json::Value::Array(depends_on_regexes),
            );
        }
        if !input_files.is_empty() {
            obj.insert(
                "input_files".to_string(),
                serde_json::Value::Array(input_files),
            );
        }
        if !output_files.is_empty() {
            obj.insert(
                "output_files".to_string(),
                serde_json::Value::Array(output_files),
            );
        }
        if !input_user_data.is_empty() {
            obj.insert(
                "input_user_data".to_string(),
                serde_json::Value::Array(input_user_data),
            );
        }
        if !output_user_data.is_empty() {
            obj.insert(
                "output_user_data".to_string(),
                serde_json::Value::Array(output_user_data),
            );
        }

        Ok(serde_json::Value::Object(obj))
    }

    /// Convert a KDL file node to a JSON object
    #[cfg(feature = "client")]
    fn kdl_file_to_json(node: &KdlNode) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let name = node
            .entries()
            .first()
            .and_then(|e| e.value().as_string())
            .ok_or("file must have a name")?
            .to_string();

        let mut obj = serde_json::Map::new();
        obj.insert("name".to_string(), serde_json::Value::String(name));

        // Path can be specified as a property (file "name" path="/path")
        if let Some(path) = node.get("path").and_then(|e| e.as_string()) {
            obj.insert(
                "path".to_string(),
                serde_json::Value::String(path.to_string()),
            );
        }

        // Check for child nodes
        if let Some(children) = node.children() {
            for child in children.nodes() {
                match child.name().value() {
                    "path" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "path".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    "parameters" => {
                        if let Some(params) = Self::kdl_parameters_to_json(child)? {
                            obj.insert("parameters".to_string(), params);
                        }
                    }
                    "parameter_mode" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "parameter_mode".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    "use_parameters" => {
                        let param_names: Vec<serde_json::Value> = child
                            .entries()
                            .iter()
                            .filter_map(|e| {
                                e.value()
                                    .as_string()
                                    .map(|s| serde_json::Value::String(s.to_string()))
                            })
                            .collect();
                        if !param_names.is_empty() {
                            obj.insert(
                                "use_parameters".to_string(),
                                serde_json::Value::Array(param_names),
                            );
                        }
                    }
                    _ => {}
                }
            }
        }

        // Validate required path field
        if !obj.contains_key("path") {
            return Err("file must have a path property".into());
        }

        Ok(serde_json::Value::Object(obj))
    }

    /// Convert a KDL user_data node to a JSON object
    #[cfg(feature = "client")]
    fn kdl_user_data_to_json(
        node: &KdlNode,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let mut obj = serde_json::Map::new();

        // Name is optional
        if let Some(name) = node.entries().first().and_then(|e| e.value().as_string()) {
            obj.insert(
                "name".to_string(),
                serde_json::Value::String(name.to_string()),
            );
        }

        let mut data_str: Option<&str> = None;

        if let Some(children) = node.children() {
            for child in children.nodes() {
                match child.name().value() {
                    "is_ephemeral" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_bool()) {
                            obj.insert("is_ephemeral".to_string(), serde_json::Value::Bool(v));
                        }
                    }
                    "data" => {
                        data_str = child.entries().first().and_then(|e| e.value().as_string());
                    }
                    _ => {}
                }
            }
        }

        // Parse data string as JSON
        let data_str = data_str.ok_or("user_data must have a data property")?;
        let data: serde_json::Value = serde_json::from_str(data_str)?;
        obj.insert("data".to_string(), data);

        Ok(serde_json::Value::Object(obj))
    }

    /// Convert a KDL resource_requirements node to a JSON object
    #[cfg(feature = "client")]
    fn kdl_resource_requirements_to_json(
        node: &KdlNode,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let name = node
            .entries()
            .first()
            .and_then(|e| e.value().as_string())
            .ok_or("resource_requirements must have a name")?
            .to_string();

        let mut obj = serde_json::Map::new();
        obj.insert("name".to_string(), serde_json::Value::String(name));

        if let Some(children) = node.children() {
            for child in children.nodes() {
                match child.name().value() {
                    "num_cpus" => {
                        if let Some(v) =
                            child.entries().first().and_then(|e| e.value().as_integer())
                        {
                            obj.insert(
                                "num_cpus".to_string(),
                                serde_json::Value::Number(serde_json::Number::from(v as i64)),
                            );
                        }
                    }
                    "num_gpus" => {
                        if let Some(v) =
                            child.entries().first().and_then(|e| e.value().as_integer())
                        {
                            obj.insert(
                                "num_gpus".to_string(),
                                serde_json::Value::Number(serde_json::Number::from(v as i64)),
                            );
                        }
                    }
                    "num_nodes" => {
                        if let Some(v) =
                            child.entries().first().and_then(|e| e.value().as_integer())
                        {
                            obj.insert(
                                "num_nodes".to_string(),
                                serde_json::Value::Number(serde_json::Number::from(v as i64)),
                            );
                        }
                    }
                    "memory" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "memory".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    "runtime" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "runtime".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(serde_json::Value::Object(obj))
    }

    /// Convert a KDL slurm_scheduler node to a JSON object
    #[cfg(feature = "client")]
    fn kdl_slurm_scheduler_to_json(
        node: &KdlNode,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let mut obj = serde_json::Map::new();

        // Name is optional
        if let Some(name) = node.entries().first().and_then(|e| e.value().as_string()) {
            obj.insert(
                "name".to_string(),
                serde_json::Value::String(name.to_string()),
            );
        }

        if let Some(children) = node.children() {
            for child in children.nodes() {
                match child.name().value() {
                    "account" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "account".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    "gres" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "gres".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    "mem" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert("mem".to_string(), serde_json::Value::String(v.to_string()));
                        }
                    }
                    "nodes" => {
                        if let Some(v) =
                            child.entries().first().and_then(|e| e.value().as_integer())
                        {
                            obj.insert(
                                "nodes".to_string(),
                                serde_json::Value::Number(serde_json::Number::from(v as i64)),
                            );
                        }
                    }
                    "ntasks_per_node" => {
                        if let Some(v) =
                            child.entries().first().and_then(|e| e.value().as_integer())
                        {
                            obj.insert(
                                "ntasks_per_node".to_string(),
                                serde_json::Value::Number(serde_json::Number::from(v as i64)),
                            );
                        }
                    }
                    "partition" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "partition".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    "qos" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert("qos".to_string(), serde_json::Value::String(v.to_string()));
                        }
                    }
                    "tmp" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert("tmp".to_string(), serde_json::Value::String(v.to_string()));
                        }
                    }
                    "walltime" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "walltime".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    "extra" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "extra".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(serde_json::Value::Object(obj))
    }

    /// Convert a KDL action node to a JSON object
    #[cfg(feature = "client")]
    fn kdl_action_to_json(node: &KdlNode) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let mut obj = serde_json::Map::new();

        // Collect array fields
        let mut job_names: Vec<serde_json::Value> = Vec::new();
        let mut job_name_regexes: Vec<serde_json::Value> = Vec::new();
        let mut commands: Vec<serde_json::Value> = Vec::new();

        if let Some(children) = node.children() {
            for child in children.nodes() {
                match child.name().value() {
                    "trigger_type" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "trigger_type".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    "action_type" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "action_type".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    "job" => {
                        // Collect individual job entries: job "prep_a" / job "prep_b"
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            job_names.push(serde_json::Value::String(v.to_string()));
                        }
                    }
                    "jobs" => {
                        // Parse jobs as multiple string arguments: jobs "job1" "job2" "job3"
                        for e in child.entries().iter() {
                            if let Some(s) = e.value().as_string() {
                                job_names.push(serde_json::Value::String(s.to_string()));
                            }
                        }
                    }
                    "job_name_regexes" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            job_name_regexes.push(serde_json::Value::String(v.to_string()));
                        }
                    }
                    "command" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            commands.push(serde_json::Value::String(v.to_string()));
                        }
                    }
                    "scheduler" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "scheduler".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    "scheduler_type" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "scheduler_type".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    "num_allocations" => {
                        if let Some(v) =
                            child.entries().first().and_then(|e| e.value().as_integer())
                        {
                            obj.insert(
                                "num_allocations".to_string(),
                                serde_json::Value::Number(serde_json::Number::from(v as i64)),
                            );
                        }
                    }
                    "start_one_worker_per_node" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_bool()) {
                            obj.insert(
                                "start_one_worker_per_node".to_string(),
                                serde_json::Value::Bool(v),
                            );
                        }
                    }
                    "max_parallel_jobs" => {
                        if let Some(v) =
                            child.entries().first().and_then(|e| e.value().as_integer())
                        {
                            obj.insert(
                                "max_parallel_jobs".to_string(),
                                serde_json::Value::Number(serde_json::Number::from(v as i64)),
                            );
                        }
                    }
                    "persistent" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_bool()) {
                            obj.insert("persistent".to_string(), serde_json::Value::Bool(v));
                        }
                    }
                    _ => {}
                }
            }
        }

        // Add collected arrays if non-empty
        if !job_names.is_empty() {
            obj.insert("jobs".to_string(), serde_json::Value::Array(job_names));
        }
        if !job_name_regexes.is_empty() {
            obj.insert(
                "job_name_regexes".to_string(),
                serde_json::Value::Array(job_name_regexes),
            );
        }
        if !commands.is_empty() {
            obj.insert("commands".to_string(), serde_json::Value::Array(commands));
        }

        Ok(serde_json::Value::Object(obj))
    }

    /// Convert a KDL resource_monitor node to a JSON object
    #[cfg(feature = "client")]
    fn kdl_resource_monitor_to_json(
        node: &KdlNode,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let mut obj = serde_json::Map::new();

        if let Some(children) = node.children() {
            for child in children.nodes() {
                match child.name().value() {
                    "enabled" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_bool()) {
                            obj.insert("enabled".to_string(), serde_json::Value::Bool(v));
                        }
                    }
                    "granularity" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "granularity".to_string(),
                                serde_json::Value::String(v.to_string()),
                            );
                        }
                    }
                    "sample_interval_seconds" => {
                        if let Some(v) =
                            child.entries().first().and_then(|e| e.value().as_integer())
                        {
                            obj.insert(
                                "sample_interval_seconds".to_string(),
                                serde_json::Value::Number(serde_json::Number::from(v as i64)),
                            );
                        }
                    }
                    "generate_plots" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_bool()) {
                            obj.insert("generate_plots".to_string(), serde_json::Value::Bool(v));
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(serde_json::Value::Object(obj))
    }

    /// Convert a KDL slurm_defaults node to a JSON object
    ///
    /// Parses slurm_defaults block containing arbitrary key-value pairs for Slurm parameters.
    /// Values can be strings, integers, or booleans.
    #[cfg(feature = "client")]
    fn kdl_slurm_defaults_to_json(
        node: &KdlNode,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let mut obj = serde_json::Map::new();

        if let Some(children) = node.children() {
            for child in children.nodes() {
                let key = child.name().value().to_string();
                if let Some(entry) = child.entries().first() {
                    let value = entry.value();
                    if let Some(s) = value.as_string() {
                        obj.insert(key, serde_json::Value::String(s.to_string()));
                    } else if let Some(i) = value.as_integer() {
                        obj.insert(
                            key,
                            serde_json::Value::Number(serde_json::Number::from(i as i64)),
                        );
                    } else if let Some(b) = value.as_bool() {
                        obj.insert(key, serde_json::Value::Bool(b));
                    }
                }
            }
        }

        Ok(serde_json::Value::Object(obj))
    }

    /// Convert a KDL failure_handler node to a JSON object
    #[cfg(feature = "client")]
    fn kdl_failure_handler_to_json(
        node: &KdlNode,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let name = node
            .entries()
            .first()
            .and_then(|e| e.value().as_string())
            .ok_or("failure_handler must have a name")?
            .to_string();

        let mut obj = serde_json::Map::new();
        obj.insert("name".to_string(), serde_json::Value::String(name));

        let mut rules: Vec<serde_json::Value> = Vec::new();

        if let Some(children) = node.children() {
            for child in children.nodes() {
                if child.name().value() == "rule" {
                    let mut rule_obj = serde_json::Map::new();

                    if let Some(rule_children) = child.children() {
                        for rule_child in rule_children.nodes() {
                            match rule_child.name().value() {
                                "exit_codes" => {
                                    let codes: Vec<serde_json::Value> = rule_child
                                        .entries()
                                        .iter()
                                        .filter_map(|e| {
                                            e.value().as_integer().map(|i| {
                                                serde_json::Value::Number((i as i64).into())
                                            })
                                        })
                                        .collect();
                                    if !codes.is_empty() {
                                        rule_obj.insert(
                                            "exit_codes".to_string(),
                                            serde_json::Value::Array(codes),
                                        );
                                    }
                                }
                                "match_all_exit_codes" => {
                                    if let Some(v) = rule_child
                                        .entries()
                                        .first()
                                        .and_then(|e| e.value().as_bool())
                                    {
                                        rule_obj.insert(
                                            "match_all_exit_codes".to_string(),
                                            serde_json::Value::Bool(v),
                                        );
                                    }
                                }
                                "recovery_script" => {
                                    if let Some(v) = rule_child
                                        .entries()
                                        .first()
                                        .and_then(|e| e.value().as_string())
                                    {
                                        rule_obj.insert(
                                            "recovery_script".to_string(),
                                            serde_json::Value::String(v.to_string()),
                                        );
                                    }
                                }
                                "max_retries" => {
                                    if let Some(v) = rule_child
                                        .entries()
                                        .first()
                                        .and_then(|e| e.value().as_integer())
                                    {
                                        rule_obj.insert(
                                            "max_retries".to_string(),
                                            serde_json::Value::Number((v as i64).into()),
                                        );
                                    }
                                }
                                _ => {}
                            }
                        }
                    }

                    rules.push(serde_json::Value::Object(rule_obj));
                }
            }
        }

        obj.insert("rules".to_string(), serde_json::Value::Array(rules));
        Ok(serde_json::Value::Object(obj))
    }

    /// Convert a KDL document string to a serde_json::Value
    /// This is the intermediate representation used by all file formats
    #[cfg(feature = "client")]
    fn kdl_to_json_value(content: &str) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let doc: KdlDocument = content.parse().map_err(|e: kdl::KdlError| {
            // Extract detailed diagnostic information from KDL parse errors
            let mut error_msg = String::from("Failed to parse KDL document:\n");
            for diag in e.diagnostics.iter() {
                let offset = diag.span.offset();
                let (line, col) = Self::offset_to_line_col(content, offset);

                if let Some(msg) = &diag.message {
                    error_msg.push_str(&format!("  Line {}, column {}: {}", line, col, msg));
                } else {
                    error_msg.push_str(&format!("  Line {}, column {}: syntax error", line, col));
                }
                if let Some(label) = &diag.label {
                    error_msg.push_str(&format!(" ({})", label));
                }
                error_msg.push('\n');
                if let Some(help) = &diag.help {
                    error_msg.push_str(&format!("    Help: {}\n", help));
                }
            }
            // Show the problematic line if we can
            if let Some(first_diag) = e.diagnostics.first() {
                let offset = first_diag.span.offset();
                let (line_num, col) = Self::offset_to_line_col(content, offset);
                if let Some(line_content) = content.lines().nth(line_num.saturating_sub(1)) {
                    error_msg.push_str(&format!("\n  {} | {}\n", line_num, line_content));
                    error_msg.push_str(&format!(
                        "  {} | {}^\n",
                        " ".repeat(line_num.to_string().len()),
                        " ".repeat(col.saturating_sub(1))
                    ));
                }
            }
            error_msg
        })?;

        let mut obj = serde_json::Map::new();
        let mut jobs: Vec<serde_json::Value> = Vec::new();
        let mut files: Vec<serde_json::Value> = Vec::new();
        let mut user_data: Vec<serde_json::Value> = Vec::new();
        let mut resource_requirements: Vec<serde_json::Value> = Vec::new();
        let mut failure_handlers: Vec<serde_json::Value> = Vec::new();
        let mut slurm_schedulers: Vec<serde_json::Value> = Vec::new();
        let mut actions: Vec<serde_json::Value> = Vec::new();

        for node in doc.nodes() {
            match node.name().value() {
                "name" => {
                    if let Some(v) = node.entries().first().and_then(|e| e.value().as_string()) {
                        obj.insert("name".to_string(), serde_json::Value::String(v.to_string()));
                    }
                }
                "user" => {
                    if let Some(v) = node.entries().first().and_then(|e| e.value().as_string()) {
                        obj.insert("user".to_string(), serde_json::Value::String(v.to_string()));
                    }
                }
                "description" => {
                    if let Some(v) = node.entries().first().and_then(|e| e.value().as_string()) {
                        obj.insert(
                            "description".to_string(),
                            serde_json::Value::String(v.to_string()),
                        );
                    }
                }
                "compute_node_expiration_buffer_seconds" => {
                    if let Some(v) = node.entries().first().and_then(|e| e.value().as_integer()) {
                        obj.insert(
                            "compute_node_expiration_buffer_seconds".to_string(),
                            serde_json::Value::Number(serde_json::Number::from(v as i64)),
                        );
                    }
                }
                "compute_node_wait_for_new_jobs_seconds" => {
                    if let Some(v) = node.entries().first().and_then(|e| e.value().as_integer()) {
                        obj.insert(
                            "compute_node_wait_for_new_jobs_seconds".to_string(),
                            serde_json::Value::Number(serde_json::Number::from(v as i64)),
                        );
                    }
                }
                "compute_node_ignore_workflow_completion" => {
                    if let Some(v) = node.entries().first().and_then(|e| e.value().as_bool()) {
                        obj.insert(
                            "compute_node_ignore_workflow_completion".to_string(),
                            serde_json::Value::Bool(v),
                        );
                    }
                }
                "compute_node_wait_for_healthy_database_minutes" => {
                    if let Some(v) = node.entries().first().and_then(|e| e.value().as_integer()) {
                        obj.insert(
                            "compute_node_wait_for_healthy_database_minutes".to_string(),
                            serde_json::Value::Number(serde_json::Number::from(v as i64)),
                        );
                    }
                }
                "jobs_sort_method" => {
                    if let Some(v) = node.entries().first().and_then(|e| e.value().as_string()) {
                        obj.insert(
                            "jobs_sort_method".to_string(),
                            serde_json::Value::String(v.to_string()),
                        );
                    }
                }
                "parameters" => {
                    if let Some(params) = Self::kdl_parameters_to_json(node)? {
                        obj.insert("parameters".to_string(), params);
                    }
                }
                "job" => {
                    jobs.push(Self::kdl_job_to_json(node)?);
                }
                "file" => {
                    files.push(Self::kdl_file_to_json(node)?);
                }
                "user_data" => {
                    user_data.push(Self::kdl_user_data_to_json(node)?);
                }
                "resource_requirements" => {
                    resource_requirements.push(Self::kdl_resource_requirements_to_json(node)?);
                }
                "failure_handler" => {
                    failure_handlers.push(Self::kdl_failure_handler_to_json(node)?);
                }
                "slurm_scheduler" => {
                    slurm_schedulers.push(Self::kdl_slurm_scheduler_to_json(node)?);
                }
                "action" => {
                    actions.push(Self::kdl_action_to_json(node)?);
                }
                "resource_monitor" => {
                    obj.insert(
                        "resource_monitor".to_string(),
                        Self::kdl_resource_monitor_to_json(node)?,
                    );
                }
                "slurm_defaults" => {
                    obj.insert(
                        "slurm_defaults".to_string(),
                        Self::kdl_slurm_defaults_to_json(node)?,
                    );
                }
                "use_pending_failed" => {
                    if let Some(v) = node.entries().first().and_then(|e| e.value().as_bool()) {
                        obj.insert("use_pending_failed".to_string(), serde_json::Value::Bool(v));
                    }
                }
                _ => {
                    // Ignore unknown nodes
                }
            }
        }

        // Add collected arrays - jobs is required (can be empty), others are optional
        obj.insert("jobs".to_string(), serde_json::Value::Array(jobs));
        if !files.is_empty() {
            obj.insert("files".to_string(), serde_json::Value::Array(files));
        }
        if !user_data.is_empty() {
            obj.insert("user_data".to_string(), serde_json::Value::Array(user_data));
        }
        if !resource_requirements.is_empty() {
            obj.insert(
                "resource_requirements".to_string(),
                serde_json::Value::Array(resource_requirements),
            );
        }
        if !failure_handlers.is_empty() {
            obj.insert(
                "failure_handlers".to_string(),
                serde_json::Value::Array(failure_handlers),
            );
        }
        if !slurm_schedulers.is_empty() {
            obj.insert(
                "slurm_schedulers".to_string(),
                serde_json::Value::Array(slurm_schedulers),
            );
        }
        if !actions.is_empty() {
            obj.insert("actions".to_string(), serde_json::Value::Array(actions));
        }

        Ok(serde_json::Value::Object(obj))
    }

    /// Serialize WorkflowSpec to KDL format
    #[cfg(feature = "client")]
    pub fn to_kdl_str(&self) -> String {
        let mut lines = Vec::new();

        // Helper to escape strings for KDL
        fn kdl_escape(s: &str) -> String {
            // Use raw strings for multi-line or strings with special chars
            if s.contains('\n') || s.contains('"') || s.contains('\\') {
                // Count the number of # needed for raw string
                let mut hashes = 0;
                loop {
                    let delimiter: String = std::iter::repeat_n('#', hashes).collect();
                    if !s.contains(&format!("\"{}", delimiter)) {
                        break;
                    }
                    hashes += 1;
                }
                let delimiter: String = std::iter::repeat_n('#', hashes).collect();
                // KDL raw string format: r#"..."# where # count can vary
                format!("r{}\"{}\"{}", delimiter, s, delimiter)
            } else {
                format!("\"{}\"", s)
            }
        }

        // Top-level fields
        lines.push(format!("name {}", kdl_escape(&self.name)));
        if let Some(ref user) = self.user {
            lines.push(format!("user {}", kdl_escape(user)));
        }
        if let Some(ref desc) = self.description {
            lines.push(format!("description {}", kdl_escape(desc)));
        }
        if let Some(val) = self.compute_node_expiration_buffer_seconds {
            lines.push(format!("compute_node_expiration_buffer_seconds {}", val));
        }
        if let Some(val) = self.compute_node_wait_for_new_jobs_seconds {
            lines.push(format!("compute_node_wait_for_new_jobs_seconds {}", val));
        }
        if let Some(val) = self.compute_node_ignore_workflow_completion {
            lines.push(format!(
                "compute_node_ignore_workflow_completion {}",
                if val { "#true" } else { "#false" }
            ));
        }
        if let Some(val) = self.compute_node_wait_for_healthy_database_minutes {
            lines.push(format!(
                "compute_node_wait_for_healthy_database_minutes {}",
                val
            ));
        }
        if let Some(ref method) = self.jobs_sort_method {
            let method_str = match method {
                models::ClaimJobsSortMethod::GpusRuntimeMemory => "gpus_runtime_memory",
                models::ClaimJobsSortMethod::GpusMemoryRuntime => "gpus_memory_runtime",
                models::ClaimJobsSortMethod::None => "none",
            };
            lines.push(format!("jobs_sort_method \"{}\"", method_str));
        }

        // Parameters
        if let Some(ref params) = self.parameters
            && !params.is_empty()
        {
            lines.push("parameters {".to_string());
            for (key, value) in params {
                lines.push(format!("    {} {}", key, kdl_escape(value)));
            }
            lines.push("}".to_string());
        }

        lines.push(String::new()); // Empty line for readability

        // Files
        if let Some(ref files) = self.files {
            for file in files {
                Self::file_spec_to_kdl(&mut lines, file, &kdl_escape);
            }
            if !files.is_empty() {
                lines.push(String::new());
            }
        }

        // User data
        if let Some(ref user_data) = self.user_data {
            for ud in user_data {
                Self::user_data_spec_to_kdl(&mut lines, ud, &kdl_escape);
            }
            if !user_data.is_empty() {
                lines.push(String::new());
            }
        }

        // Resource requirements
        if let Some(ref reqs) = self.resource_requirements {
            for req in reqs {
                Self::resource_requirements_spec_to_kdl(&mut lines, req, &kdl_escape);
            }
            if !reqs.is_empty() {
                lines.push(String::new());
            }
        }

        // Resource monitor
        if let Some(ref monitor) = self.resource_monitor {
            lines.push("resource_monitor {".to_string());
            lines.push(format!(
                "    enabled {}",
                if monitor.enabled { "#true" } else { "#false" }
            ));
            let granularity = match monitor.granularity {
                crate::client::resource_monitor::MonitorGranularity::Summary => "summary",
                crate::client::resource_monitor::MonitorGranularity::TimeSeries => "time_series",
            };
            lines.push(format!("    granularity \"{}\"", granularity));
            lines.push(format!(
                "    sample_interval_seconds {}",
                monitor.sample_interval_seconds
            ));
            lines.push(format!(
                "    generate_plots {}",
                if monitor.generate_plots {
                    "#true"
                } else {
                    "#false"
                }
            ));
            lines.push("}".to_string());
            lines.push(String::new());
        }

        // Jobs
        for job in &self.jobs {
            Self::job_spec_to_kdl(&mut lines, job, &kdl_escape);
        }
        if !self.jobs.is_empty() {
            lines.push(String::new());
        }

        // Slurm schedulers (placed after jobs since they may be auto-generated)
        if let Some(ref schedulers) = self.slurm_schedulers {
            for sched in schedulers {
                Self::slurm_scheduler_spec_to_kdl(&mut lines, sched, &kdl_escape);
            }
            if !schedulers.is_empty() {
                lines.push(String::new());
            }
        }

        // Actions (placed last since they may be auto-generated)
        if let Some(ref actions) = self.actions {
            for action in actions {
                Self::action_spec_to_kdl(&mut lines, action, &kdl_escape);
            }
        }

        lines.join("\n")
    }

    #[cfg(feature = "client")]
    fn file_spec_to_kdl(lines: &mut Vec<String>, file: &FileSpec, escape: &dyn Fn(&str) -> String) {
        let has_params = file
            .parameters
            .as_ref()
            .map(|p| !p.is_empty())
            .unwrap_or(false);
        let has_mode = file.parameter_mode.is_some();
        let has_use_params = file.use_parameters.is_some();

        if !has_params && !has_mode && !has_use_params {
            // Simple form: file "name" path="value"
            lines.push(format!(
                "file {} path={}",
                escape(&file.name),
                escape(&file.path)
            ));
        } else {
            lines.push(format!("file {} {{", escape(&file.name)));
            lines.push(format!("    path {}", escape(&file.path)));
            if let Some(ref params) = file.parameters
                && !params.is_empty()
            {
                lines.push("    parameters {".to_string());
                for (key, value) in params {
                    lines.push(format!("        {} {}", key, escape(value)));
                }
                lines.push("    }".to_string());
            }
            if let Some(ref mode) = file.parameter_mode {
                lines.push(format!("    parameter_mode {}", escape(mode)));
            }
            if let Some(ref use_params) = file.use_parameters {
                for param in use_params {
                    lines.push(format!("    use_parameter {}", escape(param)));
                }
            }
            lines.push("}".to_string());
        }
    }

    #[cfg(feature = "client")]
    fn user_data_spec_to_kdl(
        lines: &mut Vec<String>,
        ud: &UserDataSpec,
        escape: &dyn Fn(&str) -> String,
    ) {
        let name = ud.name.as_deref().unwrap_or("unnamed");
        lines.push(format!("user_data {} {{", escape(name)));
        if ud.is_ephemeral.unwrap_or(false) {
            lines.push("    is_ephemeral #true".to_string());
        }
        if let Some(ref data) = ud.data {
            // Serialize JSON value to string
            let data_str = serde_json::to_string(data).unwrap_or_default();
            lines.push(format!("    data {}", escape(&data_str)));
        }
        lines.push("}".to_string());
    }

    #[cfg(feature = "client")]
    fn resource_requirements_spec_to_kdl(
        lines: &mut Vec<String>,
        req: &ResourceRequirementsSpec,
        escape: &dyn Fn(&str) -> String,
    ) {
        lines.push(format!("resource_requirements {} {{", escape(&req.name)));
        lines.push(format!("    num_cpus {}", req.num_cpus));
        lines.push(format!("    num_gpus {}", req.num_gpus));
        lines.push(format!("    num_nodes {}", req.num_nodes));
        lines.push(format!("    memory {}", escape(&req.memory)));
        lines.push(format!("    runtime {}", escape(&req.runtime)));
        lines.push("}".to_string());
    }

    #[cfg(feature = "client")]
    fn slurm_scheduler_spec_to_kdl(
        lines: &mut Vec<String>,
        sched: &SlurmSchedulerSpec,
        escape: &dyn Fn(&str) -> String,
    ) {
        if let Some(ref name) = sched.name {
            lines.push(format!("slurm_scheduler {} {{", escape(name)));
        } else {
            lines.push("slurm_scheduler {".to_string());
        }
        lines.push(format!("    account {}", escape(&sched.account)));
        if let Some(ref gres) = sched.gres {
            lines.push(format!("    gres {}", escape(gres)));
        }
        if let Some(ref mem) = sched.mem {
            lines.push(format!("    mem {}", escape(mem)));
        }
        lines.push(format!("    nodes {}", sched.nodes));
        if let Some(ntasks) = sched.ntasks_per_node {
            lines.push(format!("    ntasks_per_node {}", ntasks));
        }
        if let Some(ref partition) = sched.partition {
            lines.push(format!("    partition {}", escape(partition)));
        }
        if let Some(ref qos) = sched.qos {
            lines.push(format!("    qos {}", escape(qos)));
        }
        if let Some(ref tmp) = sched.tmp {
            lines.push(format!("    tmp {}", escape(tmp)));
        }
        lines.push(format!("    walltime {}", escape(&sched.walltime)));
        if let Some(ref extra) = sched.extra {
            lines.push(format!("    extra {}", escape(extra)));
        }
        lines.push("}".to_string());
    }

    #[cfg(feature = "client")]
    fn action_spec_to_kdl(
        lines: &mut Vec<String>,
        action: &WorkflowActionSpec,
        escape: &dyn Fn(&str) -> String,
    ) {
        lines.push("action {".to_string());
        lines.push(format!("    trigger_type {}", escape(&action.trigger_type)));
        lines.push(format!("    action_type {}", escape(&action.action_type)));
        if let Some(ref jobs) = action.jobs {
            for job in jobs {
                lines.push(format!("    job {}", escape(job)));
            }
        }
        if let Some(ref regexes) = action.job_name_regexes {
            for regex in regexes {
                lines.push(format!("    job_name_regexes {}", escape(regex)));
            }
        }
        if let Some(ref commands) = action.commands {
            for cmd in commands {
                lines.push(format!("    command {}", escape(cmd)));
            }
        }
        if let Some(ref scheduler) = action.scheduler {
            lines.push(format!("    scheduler {}", escape(scheduler)));
        }
        if let Some(ref scheduler_type) = action.scheduler_type {
            lines.push(format!("    scheduler_type {}", escape(scheduler_type)));
        }
        if let Some(count) = action.num_allocations {
            lines.push(format!("    num_allocations {}", count));
        }
        if let Some(val) = action.start_one_worker_per_node {
            lines.push(format!(
                "    start_one_worker_per_node {}",
                if val { "#true" } else { "#false" }
            ));
        }
        if let Some(max) = action.max_parallel_jobs {
            lines.push(format!("    max_parallel_jobs {}", max));
        }
        if let Some(val) = action.persistent {
            lines.push(format!(
                "    persistent {}",
                if val { "#true" } else { "#false" }
            ));
        }
        lines.push("}".to_string());
    }

    #[cfg(feature = "client")]
    fn job_spec_to_kdl(lines: &mut Vec<String>, job: &JobSpec, escape: &dyn Fn(&str) -> String) {
        lines.push(format!("job {} {{", escape(&job.name)));
        lines.push(format!("    command {}", escape(&job.command)));
        if let Some(ref script) = job.invocation_script {
            lines.push(format!("    invocation_script {}", escape(script)));
        }
        if let Some(val) = job.cancel_on_blocking_job_failure {
            lines.push(format!(
                "    cancel_on_blocking_job_failure {}",
                if val { "#true" } else { "#false" }
            ));
        }
        if let Some(val) = job.supports_termination {
            lines.push(format!(
                "    supports_termination {}",
                if val { "#true" } else { "#false" }
            ));
        }
        if let Some(ref req) = job.resource_requirements {
            lines.push(format!("    resource_requirements {}", escape(req)));
        }
        if let Some(ref deps) = job.depends_on {
            for dep in deps {
                lines.push(format!("    depends_on {}", escape(dep)));
            }
        }
        if let Some(ref regexes) = job.depends_on_regexes {
            for regex in regexes {
                lines.push(format!("    depends_on_regexes {}", escape(regex)));
            }
        }
        if let Some(ref files) = job.input_files {
            for file in files {
                lines.push(format!("    input_file {}", escape(file)));
            }
        }
        if let Some(ref files) = job.output_files {
            for file in files {
                lines.push(format!("    output_file {}", escape(file)));
            }
        }
        if let Some(ref ud) = job.input_user_data {
            for name in ud {
                lines.push(format!("    input_user_data {}", escape(name)));
            }
        }
        if let Some(ref ud) = job.output_user_data {
            for name in ud {
                lines.push(format!("    output_user_data {}", escape(name)));
            }
        }
        if let Some(ref sched) = job.scheduler {
            lines.push(format!("    scheduler {}", escape(sched)));
        }
        if let Some(ref params) = job.parameters
            && !params.is_empty()
        {
            lines.push("    parameters {".to_string());
            for (key, value) in params {
                lines.push(format!("        {} {}", key, escape(value)));
            }
            lines.push("    }".to_string());
        }
        lines.push("}".to_string());
    }

    /// Deserialize a WorkflowSpec from a specification file (JSON, JSON5, YAML, or KDL)
    /// All formats are first converted to serde_json::Value, then to WorkflowSpec,
    /// ensuring consistent behavior across all file formats.
    pub fn from_spec_file<P: AsRef<Path>>(
        path: P,
    ) -> Result<WorkflowSpec, Box<dyn std::error::Error>> {
        let path_ref = path.as_ref();
        let file_content = fs::read_to_string(path_ref)?;

        // Determine file type based on extension
        let extension = path_ref
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        // Parse to JSON Value first, then convert to WorkflowSpec
        // This ensures consistent behavior across all formats
        let json_value: serde_json::Value = match extension.to_lowercase().as_str() {
            "json" => serde_json::from_str(&file_content)?,
            "json5" => json5::from_str(&file_content)?,
            "yaml" | "yml" => serde_yaml::from_str(&file_content)?,
            #[cfg(feature = "client")]
            "kdl" => Self::kdl_to_json_value(&file_content)?,
            _ => {
                // Try to parse as JSON first, then JSON5, then YAML, then KDL
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&file_content) {
                    value
                } else if let Ok(value) = json5::from_str::<serde_json::Value>(&file_content) {
                    value
                } else if let Ok(value) = serde_yaml::from_str::<serde_json::Value>(&file_content) {
                    value
                } else {
                    #[cfg(feature = "client")]
                    {
                        Self::kdl_to_json_value(&file_content)?
                    }
                    #[cfg(not(feature = "client"))]
                    {
                        return Err("Unable to parse workflow spec file".into());
                    }
                }
            }
        };

        Self::from_json_value(json_value)
    }

    /// Deserialize a WorkflowSpec from string content with a specified format
    /// Useful for testing or when content is already loaded
    /// All formats are first converted to serde_json::Value, then to WorkflowSpec,
    /// ensuring consistent behavior across all file formats.
    ///
    /// # Arguments
    /// * `content` - The workflow spec content as a string
    /// * `format` - The format type: "json", "json5", "yaml", "yml", or "kdl"
    pub fn from_spec_file_content(
        content: &str,
        format: &str,
    ) -> Result<WorkflowSpec, Box<dyn std::error::Error>> {
        // Parse to JSON Value first, then convert to WorkflowSpec
        let json_value: serde_json::Value = match format.to_lowercase().as_str() {
            "json" => serde_json::from_str(content)?,
            "json5" => json5::from_str(content)?,
            "yaml" | "yml" => serde_yaml::from_str(content)?,
            #[cfg(feature = "client")]
            "kdl" => Self::kdl_to_json_value(content)?,
            #[cfg(not(feature = "client"))]
            "kdl" => return Err("KDL format requires 'client' feature".into()),
            _ => return Err(format!("Unknown format: {}", format).into()),
        };

        Self::from_json_value(json_value)
    }

    /// Perform variable substitution on job commands and invocation scripts
    /// Supported variables:
    /// - ${files.input.NAME} - input file (automatically adds to input_files)
    /// - ${files.output.NAME} - output file (automatically adds to output_files)
    /// - ${user_data.input.NAME} - input user data (automatically adds to input_user_data)
    /// - ${user_data.output.NAME} - output user data (automatically adds to output_user_data)
    pub fn substitute_variables(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Build file name to path mapping
        let mut file_name_to_path = HashMap::new();
        if let Some(files) = &self.files {
            for file_spec in files {
                file_name_to_path.insert(file_spec.name.clone(), file_spec.path.clone());
            }
        }

        // Build user data name to data mapping
        let mut user_data_name_to_data = HashMap::new();
        if let Some(user_data_list) = &self.user_data {
            for user_data_spec in user_data_list {
                if let Some(name) = &user_data_spec.name
                    && let Some(data) = &user_data_spec.data
                {
                    user_data_name_to_data.insert(name.clone(), data.clone());
                }
            }
        }

        // Substitute variables in each job and extract dependencies
        for job in &mut self.jobs {
            let (new_command, input_files, output_files, input_user_data, output_user_data) =
                Self::substitute_and_extract(
                    &job.command,
                    &file_name_to_path,
                    &user_data_name_to_data,
                )?;
            job.command = new_command;

            // Set input/output file names from extracted dependencies
            if !input_files.is_empty() {
                job.input_files = Some(input_files);
            }
            if !output_files.is_empty() {
                job.output_files = Some(output_files);
            }
            if !input_user_data.is_empty() {
                job.input_user_data = Some(input_user_data);
            }
            if !output_user_data.is_empty() {
                job.output_user_data = Some(output_user_data);
            }

            // Process invocation script if present
            if let Some(script) = &job.invocation_script {
                let (
                    new_script,
                    script_input_files,
                    script_output_files,
                    script_input_user_data,
                    script_output_user_data,
                ) = Self::substitute_and_extract(
                    script,
                    &file_name_to_path,
                    &user_data_name_to_data,
                )?;
                job.invocation_script = Some(new_script);

                // Merge dependencies from invocation script
                if !script_input_files.is_empty() {
                    let mut combined = job.input_files.clone().unwrap_or_default();
                    combined.extend(script_input_files);
                    combined.sort();
                    combined.dedup();
                    job.input_files = Some(combined);
                }
                if !script_output_files.is_empty() {
                    let mut combined = job.output_files.clone().unwrap_or_default();
                    combined.extend(script_output_files);
                    combined.sort();
                    combined.dedup();
                    job.output_files = Some(combined);
                }
                if !script_input_user_data.is_empty() {
                    let mut combined = job.input_user_data.clone().unwrap_or_default();
                    combined.extend(script_input_user_data);
                    combined.sort();
                    combined.dedup();
                    job.input_user_data = Some(combined);
                }
                if !script_output_user_data.is_empty() {
                    let mut combined = job.output_user_data.clone().unwrap_or_default();
                    combined.extend(script_output_user_data);
                    combined.sort();
                    combined.dedup();
                    job.output_user_data = Some(combined);
                }
            }
        }

        Ok(())
    }

    /// Substitute variables and extract input/output dependencies
    /// Returns: (substituted_string, input_files, output_files, input_user_data, output_user_data)
    #[allow(clippy::type_complexity)]
    fn substitute_and_extract(
        input: &str,
        file_name_to_path: &HashMap<String, String>,
        user_data_name_to_data: &HashMap<String, serde_json::Value>,
    ) -> Result<
        (String, Vec<String>, Vec<String>, Vec<String>, Vec<String>),
        Box<dyn std::error::Error>,
    > {
        let mut result = input.to_string();
        let mut input_files = Vec::new();
        let mut output_files = Vec::new();
        let mut input_user_data = Vec::new();
        let mut output_user_data = Vec::new();

        // Extract and replace ${files.input.NAME}
        for (name, path) in file_name_to_path {
            let input_pattern = format!("${{files.input.{}}}", name);
            if result.contains(&input_pattern) {
                result = result.replace(&input_pattern, path);
                input_files.push(name.clone());
            }
        }

        // Extract and replace ${files.output.NAME}
        for (name, path) in file_name_to_path {
            let output_pattern = format!("${{files.output.{}}}", name);
            if result.contains(&output_pattern) {
                result = result.replace(&output_pattern, path);
                output_files.push(name.clone());
            }
        }

        // Extract and replace ${user_data.input.NAME}
        for (name, data) in user_data_name_to_data {
            let input_pattern = format!("${{user_data.input.{}}}", name);
            if result.contains(&input_pattern) {
                let data_str = serde_json::to_string(data)?;
                result = result.replace(&input_pattern, &data_str);
                input_user_data.push(name.clone());
            }
        }

        // Extract and replace ${user_data.output.NAME}
        for (name, data) in user_data_name_to_data {
            let output_pattern = format!("${{user_data.output.{}}}", name);
            if result.contains(&output_pattern) {
                let data_str = serde_json::to_string(data)?;
                result = result.replace(&output_pattern, &data_str);
                output_user_data.push(name.clone());
            }
        }

        Ok((
            result,
            input_files,
            output_files,
            input_user_data,
            output_user_data,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_kdl_job_parameterization() {
        let kdl_content = r#"
name "test_parameterized"
description "Test parameterized jobs in KDL format"

job "job_{i:03d}" {
    command "echo hello {i}"
    parameters {
        i "1:5"
    }
}
"#;

        let mut spec = WorkflowSpec::from_spec_file_content(kdl_content, "kdl")
            .expect("Failed to parse KDL workflow spec");

        // Before expansion, should have 1 job with parameters
        assert_eq!(spec.jobs.len(), 1);
        assert!(spec.jobs[0].parameters.is_some());

        // Expand parameters
        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // After expansion, should have 5 jobs
        assert_eq!(spec.jobs.len(), 5);
        assert_eq!(spec.jobs[0].name, "job_001");
        assert_eq!(spec.jobs[0].command, "echo hello 1");
        assert_eq!(spec.jobs[4].name, "job_005");
        assert_eq!(spec.jobs[4].command, "echo hello 5");

        // Parameters should be removed from expanded jobs
        for job in &spec.jobs {
            assert!(job.parameters.is_none());
        }
    }

    #[test]
    fn test_kdl_file_parameterization() {
        let kdl_content = r#"
name "test_parameterized_files"
description "Test parameterized files in KDL format"

file "output_{run_id}" {
    path "/data/output_{run_id}.txt"
    parameters {
        run_id "1:3"
    }
}

job "process" {
    command "echo test"
}
"#;

        let mut spec = WorkflowSpec::from_spec_file_content(kdl_content, "kdl")
            .expect("Failed to parse KDL workflow spec");

        // Before expansion, should have 1 file with parameters
        assert_eq!(spec.files.as_ref().unwrap().len(), 1);
        assert!(spec.files.as_ref().unwrap()[0].parameters.is_some());

        // Expand parameters
        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // After expansion, should have 3 files
        let files = spec.files.as_ref().unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].name, "output_1");
        assert_eq!(files[0].path, "/data/output_1.txt");
        assert_eq!(files[2].name, "output_3");
        assert_eq!(files[2].path, "/data/output_3.txt");

        // Parameters should be removed from expanded files
        for file in files {
            assert!(file.parameters.is_none());
        }
    }

    #[test]
    fn test_kdl_multi_dimensional_parameterization() {
        let kdl_content = r#"
name "test_multi_param"
description "Test multi-dimensional parameterization in KDL format"

job "train_lr{lr:.4f}_bs{batch_size}" {
    command "python train.py --lr={lr} --batch-size={batch_size}"
    parameters {
        lr "[0.001,0.01]"
        batch_size "[16,32]"
    }
}
"#;

        let mut spec = WorkflowSpec::from_spec_file_content(kdl_content, "kdl")
            .expect("Failed to parse KDL workflow spec");

        // Expand parameters
        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // Should have 2 * 2 = 4 jobs
        assert_eq!(spec.jobs.len(), 4);

        // Verify all expected combinations exist
        let names: Vec<&str> = spec.jobs.iter().map(|j| j.name.as_str()).collect();
        assert!(names.contains(&"train_lr0.0010_bs16"));
        assert!(names.contains(&"train_lr0.0010_bs32"));
        assert!(names.contains(&"train_lr0.0100_bs16"));
        assert!(names.contains(&"train_lr0.0100_bs32"));
    }

    #[test]
    fn test_kdl_example_file_hundred_jobs() {
        // Test parsing the actual KDL example file
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let path = PathBuf::from(manifest_dir).join("examples/kdl/hundred_jobs_parameterized.kdl");

        let mut spec =
            WorkflowSpec::from_spec_file(&path).expect("Failed to parse KDL example file");

        assert_eq!(spec.name, "hundred_jobs_parameterized");
        // 2 jobs before expansion: parameterized job template + postprocess
        assert_eq!(spec.jobs.len(), 2);
        assert!(spec.jobs[0].parameters.is_some());

        // Expand parameters
        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // Should have 101 jobs after expansion: 100 parameterized + 1 postprocess
        assert_eq!(spec.jobs.len(), 101);
        assert_eq!(spec.jobs[0].name, "job_001");
        assert_eq!(spec.jobs[99].name, "job_100");
        assert_eq!(spec.jobs[100].name, "postprocess");
    }

    #[test]
    fn test_kdl_example_file_hyperparameter_sweep() {
        // Test parsing the actual KDL hyperparameter sweep example
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let path = PathBuf::from(manifest_dir).join("examples/kdl/hyperparameter_sweep.kdl");

        let mut spec = WorkflowSpec::from_spec_file(&path)
            .expect("Failed to parse KDL hyperparameter sweep file");

        assert_eq!(spec.name, "hyperparameter_sweep");

        // Before expansion: 4 jobs (prepare_train, prepare_val, train template, aggregate template)
        assert_eq!(spec.jobs.len(), 4);

        // Before expansion: 4 files (train_data, val_data, model template, metrics template)
        assert_eq!(spec.files.as_ref().unwrap().len(), 4);

        // Expand parameters
        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // After expansion:
        // - 2 prepare jobs (unchanged)
        // - 18 training jobs (3 lr * 3 batch_size * 2 optimizer)
        // - 18 aggregate jobs (expanded from template)
        // Total: 2 + 18 + 18 = 38 jobs
        assert_eq!(spec.jobs.len(), 38);

        // Files after expansion:
        // - 2 data files (unchanged)
        // - 18 model files (parameterized)
        // - 18 metrics files (parameterized)
        // Total: 2 + 18 + 18 = 38 files
        assert_eq!(spec.files.as_ref().unwrap().len(), 38);
    }

    #[test]
    fn test_integer_range_expansion() {
        let mut job = JobSpec::new("job_{i}".to_string(), "echo {i}".to_string());

        let mut params = HashMap::new();
        params.insert("i".to_string(), "1:5".to_string());
        job.parameters = Some(params);

        let expanded = job.expand().expect("Failed to expand job");

        assert_eq!(expanded.len(), 5);
        assert_eq!(expanded[0].name, "job_1");
        assert_eq!(expanded[0].command, "echo 1");
        assert_eq!(expanded[4].name, "job_5");
        assert_eq!(expanded[4].command, "echo 5");
    }

    #[test]
    fn test_integer_range_with_step() {
        let mut job = JobSpec::new("job_{i}".to_string(), "echo {i}".to_string());

        let mut params = HashMap::new();
        params.insert("i".to_string(), "0:10:2".to_string());
        job.parameters = Some(params);

        let expanded = job.expand().expect("Failed to expand job");

        assert_eq!(expanded.len(), 6);
        assert_eq!(expanded[0].name, "job_0");
        assert_eq!(expanded[1].name, "job_2");
        assert_eq!(expanded[5].name, "job_10");
    }

    #[test]
    fn test_float_range_expansion() {
        let mut job = JobSpec::new("job_{lr}".to_string(), "train.py --lr={lr}".to_string());

        let mut params = HashMap::new();
        params.insert("lr".to_string(), "0.0:1.0:0.5".to_string());
        job.parameters = Some(params);

        let expanded = job.expand().expect("Failed to expand job");

        assert_eq!(expanded.len(), 3);
        assert_eq!(expanded[0].command, "train.py --lr=0");
        assert_eq!(expanded[1].command, "train.py --lr=0.5");
        assert_eq!(expanded[2].command, "train.py --lr=1");
    }

    #[test]
    fn test_list_expansion() {
        let mut job = JobSpec::new(
            "job_{dataset}".to_string(),
            "process.sh {dataset}".to_string(),
        );

        let mut params = HashMap::new();
        params.insert(
            "dataset".to_string(),
            "['train','test','validation']".to_string(),
        );
        job.parameters = Some(params);

        let expanded = job.expand().expect("Failed to expand job");

        assert_eq!(expanded.len(), 3);
        assert_eq!(expanded[0].name, "job_train");
        assert_eq!(expanded[0].command, "process.sh train");
        assert_eq!(expanded[2].name, "job_validation");
    }

    #[test]
    fn test_multi_dimensional_parameter_sweep() {
        let mut job = JobSpec::new(
            "job_lr{lr}_bs{batch_size}".to_string(),
            "train.py --lr={lr} --batch-size={batch_size}".to_string(),
        );

        let mut params = HashMap::new();
        params.insert("lr".to_string(), "[0.001,0.01,0.1]".to_string());
        params.insert("batch_size".to_string(), "[16,32,64]".to_string());
        job.parameters = Some(params);

        let expanded = job.expand().expect("Failed to expand job");

        // Should generate 3 * 3 = 9 combinations
        assert_eq!(expanded.len(), 9);

        // Check a few combinations
        let names: Vec<&str> = expanded.iter().map(|j| j.name.as_str()).collect();
        assert!(names.contains(&"job_lr0.001_bs16"));
        assert!(names.contains(&"job_lr0.1_bs64"));

        let commands: Vec<&str> = expanded.iter().map(|j| j.command.as_str()).collect();
        assert!(commands.contains(&"train.py --lr=0.001 --batch-size=16"));
        assert!(commands.contains(&"train.py --lr=0.1 --batch-size=64"));
    }

    #[test]
    fn test_format_specifier_zero_padding() {
        let mut job = JobSpec::new("job_{i:03d}".to_string(), "echo {i:03d}".to_string());

        let mut params = HashMap::new();
        params.insert("i".to_string(), "1:5".to_string());
        job.parameters = Some(params);

        let expanded = job.expand().expect("Failed to expand job");

        assert_eq!(expanded[0].name, "job_001");
        assert_eq!(expanded[0].command, "echo 001");
        assert_eq!(expanded[4].name, "job_005");
    }

    #[test]
    fn test_format_specifier_float_precision() {
        let mut job = JobSpec::new(
            "job_{lr:.2f}".to_string(),
            "train.py --lr={lr:.2f}".to_string(),
        );

        let mut params = HashMap::new();
        params.insert("lr".to_string(), "0.0:0.3:0.1".to_string());
        job.parameters = Some(params);

        let expanded = job.expand().expect("Failed to expand job");

        assert_eq!(expanded[0].name, "job_0.00");
        assert_eq!(expanded[1].name, "job_0.10");
        assert_eq!(expanded[2].name, "job_0.20");
    }

    #[test]
    fn test_file_parameterization() {
        let mut file = FileSpec::new(
            "output_{run_id}".to_string(),
            "/data/output_{run_id}.txt".to_string(),
        );

        let mut params = HashMap::new();
        params.insert("run_id".to_string(), "1:3".to_string());
        file.parameters = Some(params);

        let expanded = file.expand().expect("Failed to expand file");

        assert_eq!(expanded.len(), 3);
        assert_eq!(expanded[0].name, "output_1");
        assert_eq!(expanded[0].path, "/data/output_1.txt");
        assert_eq!(expanded[2].name, "output_3");
        assert_eq!(expanded[2].path, "/data/output_3.txt");
    }

    #[test]
    fn test_job_with_input_output_files() {
        let mut job = JobSpec::new(
            "process_{i}".to_string(),
            "process.sh input_{i}.txt output_{i}.txt".to_string(),
        );
        job.input_files = Some(vec!["input_{i}".to_string()]);
        job.output_files = Some(vec!["output_{i}".to_string()]);

        let mut params = HashMap::new();
        params.insert("i".to_string(), "1:3".to_string());
        job.parameters = Some(params);

        let expanded = job.expand().expect("Failed to expand job");

        assert_eq!(expanded.len(), 3);

        assert_eq!(expanded[0].name, "process_1");
        assert_eq!(expanded[0].input_files, Some(vec!["input_1".to_string()]));
        assert_eq!(expanded[0].output_files, Some(vec!["output_1".to_string()]));

        assert_eq!(expanded[2].name, "process_3");
        assert_eq!(expanded[2].input_files, Some(vec!["input_3".to_string()]));
        assert_eq!(expanded[2].output_files, Some(vec!["output_3".to_string()]));
    }

    #[test]
    fn test_job_with_depends_on_names() {
        let mut job = JobSpec::new(
            "dependent_{i}".to_string(),
            "echo dependent {i}".to_string(),
        );
        job.depends_on = Some(vec!["upstream_{i}".to_string()]);

        let mut params = HashMap::new();
        params.insert("i".to_string(), "1:3".to_string());
        job.parameters = Some(params);

        let expanded = job.expand().expect("Failed to expand job");

        assert_eq!(expanded.len(), 3);
        assert_eq!(expanded[0].name, "dependent_1");
        assert_eq!(expanded[0].depends_on, Some(vec!["upstream_1".to_string()]));
        assert_eq!(expanded[2].name, "dependent_3");
        assert_eq!(expanded[2].depends_on, Some(vec!["upstream_3".to_string()]));
    }

    #[test]
    fn test_no_parameters_returns_original() {
        let job = JobSpec::new("simple_job".to_string(), "echo hello".to_string());

        let expanded = job.expand().expect("Failed to expand job");

        assert_eq!(expanded.len(), 1);
        assert_eq!(expanded[0].name, "simple_job");
        assert_eq!(expanded[0].command, "echo hello");
    }

    #[test]
    fn test_invalid_range_format() {
        let mut job = JobSpec::new("job_{i}".to_string(), "echo {i}".to_string());

        let mut params = HashMap::new();
        params.insert("i".to_string(), "invalid:range:format:too:many".to_string());
        job.parameters = Some(params);

        let result = job.expand();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid range format"));
    }

    #[test]
    fn test_zero_step_error() {
        let mut job = JobSpec::new("job_{i}".to_string(), "echo {i}".to_string());

        let mut params = HashMap::new();
        params.insert("i".to_string(), "1:10:0".to_string());
        job.parameters = Some(params);

        let result = job.expand();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Step cannot be zero"));
    }

    #[test]
    fn test_workflow_spec_expand_parameters() {
        let mut spec = WorkflowSpec {
            name: "test_workflow".to_string(),
            description: Some("Test workflow with parameters".to_string()),
            user: Some("test_user".to_string()),
            compute_node_expiration_buffer_seconds: None,
            compute_node_wait_for_healthy_database_minutes: None,
            compute_node_ignore_workflow_completion: None,
            compute_node_wait_for_new_jobs_seconds: None,
            jobs_sort_method: None,
            parameters: None,
            jobs: vec![JobSpec {
                name: "job_{i}".to_string(),
                command: "echo {i}".to_string(),
                invocation_script: None,
                cancel_on_blocking_job_failure: Some(false),
                supports_termination: Some(false),
                resource_requirements: None,
                scheduler: None,
                depends_on: None,
                depends_on_regexes: None,
                input_files: None,
                input_file_regexes: None,
                output_files: None,
                output_file_regexes: None,
                input_user_data: None,
                input_user_data_regexes: None,
                output_user_data: None,
                output_user_data_regexes: None,
                parameters: Some({
                    let mut params = HashMap::new();
                    params.insert("i".to_string(), "1:3".to_string());
                    params
                }),
                parameter_mode: None,
                use_parameters: None,
                failure_handler: None,
            }],
            files: Some(vec![{
                let mut file =
                    FileSpec::new("file_{i}".to_string(), "/data/file_{i}.txt".to_string());
                file.parameters = Some({
                    let mut params = HashMap::new();
                    params.insert("i".to_string(), "1:3".to_string());
                    params
                });
                file
            }]),
            user_data: None,
            resource_requirements: None,
            slurm_schedulers: None,
            slurm_defaults: None,
            resource_monitor: None,
            actions: None,
            failure_handlers: None,
            use_pending_failed: None,
            project: None,
            metadata: None,
        };

        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // Jobs should be expanded
        assert_eq!(spec.jobs.len(), 3);
        assert_eq!(spec.jobs[0].name, "job_1");
        assert_eq!(spec.jobs[2].name, "job_3");

        // Files should be expanded
        assert_eq!(spec.files.as_ref().unwrap().len(), 3);
        assert_eq!(spec.files.as_ref().unwrap()[0].name, "file_1");
        assert_eq!(spec.files.as_ref().unwrap()[2].name, "file_3");
    }

    #[test]
    fn test_complex_multi_param_with_dependencies() {
        let mut job = JobSpec::new(
            "train_lr{lr}_bs{bs}_epoch{epoch}".to_string(),
            "train.py --lr={lr} --bs={bs} --epochs={epoch}".to_string(),
        );
        job.input_files = Some(vec!["data_{bs}".to_string()]);
        job.output_files = Some(vec!["model_lr{lr}_bs{bs}_epoch{epoch}.pt".to_string()]);

        let mut params = HashMap::new();
        params.insert("lr".to_string(), "[0.001,0.01]".to_string());
        params.insert("bs".to_string(), "[16,32]".to_string());
        params.insert("epoch".to_string(), "[10,20]".to_string());
        job.parameters = Some(params);

        let expanded = job.expand().expect("Failed to expand job");

        // Should generate 2 * 2 * 2 = 8 combinations
        assert_eq!(expanded.len(), 8);

        // Check one specific combination
        let job_001_16_10 = expanded
            .iter()
            .find(|j| j.name == "train_lr0.001_bs16_epoch10")
            .expect("Expected job not found");

        assert_eq!(
            job_001_16_10.command,
            "train.py --lr=0.001 --bs=16 --epochs=10"
        );
        assert_eq!(job_001_16_10.input_files, Some(vec!["data_16".to_string()]));
        assert_eq!(
            job_001_16_10.output_files,
            Some(vec!["model_lr0.001_bs16_epoch10.pt".to_string()])
        );
    }

    #[test]
    fn test_invocation_script_substitution() {
        let mut job = JobSpec::new("job_{i}".to_string(), "python train.py".to_string());
        job.invocation_script = Some("#!/bin/bash\nexport RUN_ID={i}\n".to_string());

        let mut params = HashMap::new();
        params.insert("i".to_string(), "1:2".to_string());
        job.parameters = Some(params);

        let expanded = job.expand().expect("Failed to expand job");

        assert_eq!(
            expanded[0].invocation_script,
            Some("#!/bin/bash\nexport RUN_ID=1\n".to_string())
        );
        assert_eq!(
            expanded[1].invocation_script,
            Some("#!/bin/bash\nexport RUN_ID=2\n".to_string())
        );
    }

    #[test]
    fn test_user_data_name_substitution() {
        let mut job = JobSpec::new("job_{stage}".to_string(), "process.sh {stage}".to_string());
        job.input_user_data = Some(vec!["config_{stage}".to_string()]);
        job.output_user_data = Some(vec!["results_{stage}".to_string()]);

        let mut params = HashMap::new();
        params.insert("stage".to_string(), "['train','test']".to_string());
        job.parameters = Some(params);

        let expanded = job.expand().expect("Failed to expand job");

        assert_eq!(expanded.len(), 2);
        assert_eq!(
            expanded[0].input_user_data,
            Some(vec!["config_train".to_string()])
        );
        assert_eq!(
            expanded[0].output_user_data,
            Some(vec!["results_train".to_string()])
        );
        assert_eq!(
            expanded[1].input_user_data,
            Some(vec!["config_test".to_string()])
        );
        assert_eq!(
            expanded[1].output_user_data,
            Some(vec!["results_test".to_string()])
        );
    }

    // ==================== Shared Parameters Tests ====================

    #[test]
    fn test_shared_parameters_yaml() {
        let yaml_content = r#"
name: shared_params_test
description: Test workflow-level shared parameters

parameters:
  i: "1:3"
  prefix: "['a','b']"

jobs:
  - name: job_{i}_{prefix}
    command: echo {i} {prefix}
    use_parameters:
      - i
      - prefix
"#;

        let mut spec = WorkflowSpec::from_spec_file_content(yaml_content, "yaml")
            .expect("Failed to parse YAML workflow spec");

        // Verify workflow-level parameters were parsed
        assert!(spec.parameters.is_some());
        let params = spec.parameters.as_ref().unwrap();
        assert_eq!(params.get("i").unwrap(), "1:3");
        assert_eq!(params.get("prefix").unwrap(), "['a','b']");

        // Verify job has use_parameters
        assert!(spec.jobs[0].use_parameters.is_some());
        assert_eq!(spec.jobs[0].use_parameters.as_ref().unwrap().len(), 2);

        // Expand parameters
        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // Should have 3 * 2 = 6 jobs
        assert_eq!(spec.jobs.len(), 6);

        // Check that all combinations exist
        let names: Vec<&str> = spec.jobs.iter().map(|j| j.name.as_str()).collect();
        assert!(names.contains(&"job_1_a"));
        assert!(names.contains(&"job_1_b"));
        assert!(names.contains(&"job_2_a"));
        assert!(names.contains(&"job_2_b"));
        assert!(names.contains(&"job_3_a"));
        assert!(names.contains(&"job_3_b"));
    }

    #[test]
    fn test_shared_parameters_kdl() {
        let kdl_content = r#"
name "shared_params_test"
description "Test workflow-level shared parameters in KDL"

parameters {
    i "1:3"
    prefix "['a','b']"
}

job "job_{i}_{prefix}" {
    command "echo {i} {prefix}"
    use_parameters "i" "prefix"
}
"#;

        let mut spec = WorkflowSpec::from_spec_file_content(kdl_content, "kdl")
            .expect("Failed to parse KDL workflow spec");

        // Verify workflow-level parameters were parsed
        assert!(spec.parameters.is_some());
        let params = spec.parameters.as_ref().unwrap();
        assert_eq!(params.get("i").unwrap(), "1:3");
        assert_eq!(params.get("prefix").unwrap(), "['a','b']");

        // Verify job has use_parameters
        assert!(spec.jobs[0].use_parameters.is_some());

        // Expand parameters
        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // Should have 3 * 2 = 6 jobs
        assert_eq!(spec.jobs.len(), 6);

        // Check that all combinations exist
        let names: Vec<&str> = spec.jobs.iter().map(|j| j.name.as_str()).collect();
        assert!(names.contains(&"job_1_a"));
        assert!(names.contains(&"job_3_b"));
    }

    #[test]
    fn test_shared_parameters_json5() {
        let json5_content = r#"
{
    name: "shared_params_test",
    description: "Test workflow-level shared parameters in JSON5",

    parameters: {
        i: "1:3",
        prefix: "['a','b']"
    },

    jobs: [
        {
            name: "job_{i}_{prefix}",
            command: "echo {i} {prefix}",
            use_parameters: ["i", "prefix"]
        }
    ]
}
"#;

        let mut spec = WorkflowSpec::from_spec_file_content(json5_content, "json5")
            .expect("Failed to parse JSON5 workflow spec");

        // Verify workflow-level parameters were parsed
        assert!(spec.parameters.is_some());

        // Expand parameters
        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // Should have 3 * 2 = 6 jobs
        assert_eq!(spec.jobs.len(), 6);
    }

    #[test]
    fn test_shared_parameters_selective_inheritance() {
        // Test that use_parameters only inherits specified parameters
        let yaml_content = r#"
name: selective_params_test
description: Test selective parameter inheritance

parameters:
  a: "1:2"
  b: "3:4"
  c: "5:6"

jobs:
  # This job should only use parameters a and b (4 jobs)
  - name: job_{a}_{b}
    command: echo {a} {b}
    use_parameters:
      - a
      - b
"#;

        let mut spec = WorkflowSpec::from_spec_file_content(yaml_content, "yaml")
            .expect("Failed to parse YAML workflow spec");

        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // Should have 2 * 2 = 4 jobs (not using parameter c)
        assert_eq!(spec.jobs.len(), 4);

        // Check that only a and b were used
        let names: Vec<&str> = spec.jobs.iter().map(|j| j.name.as_str()).collect();
        assert!(names.contains(&"job_1_3"));
        assert!(names.contains(&"job_1_4"));
        assert!(names.contains(&"job_2_3"));
        assert!(names.contains(&"job_2_4"));
    }

    #[test]
    fn test_shared_parameters_with_files() {
        let yaml_content = r#"
name: file_params_test
description: Test shared parameters with files

parameters:
  i: "1:2"

files:
  - name: file_{i}
    path: /data/file_{i}.txt
    use_parameters:
      - i

jobs:
  - name: job_{i}
    command: process /data/file_{i}.txt
    input_files:
      - file_{i}
    use_parameters:
      - i
"#;

        let mut spec = WorkflowSpec::from_spec_file_content(yaml_content, "yaml")
            .expect("Failed to parse YAML workflow spec");

        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // Should have 2 files
        assert_eq!(spec.files.as_ref().unwrap().len(), 2);
        let file_names: Vec<&str> = spec
            .files
            .as_ref()
            .unwrap()
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        assert!(file_names.contains(&"file_1"));
        assert!(file_names.contains(&"file_2"));

        // Should have 2 jobs
        assert_eq!(spec.jobs.len(), 2);
    }

    #[test]
    fn test_local_parameters_override_shared() {
        // Test that local parameters take precedence over shared parameters
        let yaml_content = r#"
name: override_params_test
description: Test local parameters override shared

parameters:
  i: "1:5"

jobs:
  # This job uses local parameters (overrides shared)
  - name: job_{i}
    command: echo {i}
    parameters:
      i: "10:12"
"#;

        let mut spec = WorkflowSpec::from_spec_file_content(yaml_content, "yaml")
            .expect("Failed to parse YAML workflow spec");

        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // Should have 3 jobs (from local 10:12), not 5 (from shared 1:5)
        assert_eq!(spec.jobs.len(), 3);

        // Check that local parameters were used
        let names: Vec<&str> = spec.jobs.iter().map(|j| j.name.as_str()).collect();
        assert!(names.contains(&"job_10"));
        assert!(names.contains(&"job_11"));
        assert!(names.contains(&"job_12"));
    }

    #[test]
    fn test_example_file_hyperparameter_sweep_shared_params_yaml() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("examples/yaml/hyperparameter_sweep_shared_params.yaml");

        let mut spec = WorkflowSpec::from_spec_file(&path)
            .expect("Failed to load hyperparameter_sweep_shared_params.yaml");

        // Verify workflow-level parameters were parsed
        assert!(spec.parameters.is_some());
        let params = spec.parameters.as_ref().unwrap();
        assert_eq!(params.len(), 3);
        assert!(params.contains_key("lr"));
        assert!(params.contains_key("batch_size"));
        assert!(params.contains_key("optimizer"));

        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // Should have same structure as non-shared version (hyperparameter_sweep.yaml):
        // - 2 prepare jobs (no parameters)
        // - 18 training jobs (3 lr * 3 batch_size * 2 optimizer)
        // - 18 aggregate jobs (expanded from template)
        // Total: 2 + 18 + 18 = 38 jobs
        assert_eq!(spec.jobs.len(), 38);

        // Files after expansion:
        // - 2 data files (no parameters)
        // - 18 model files (parameterized)
        // - 18 metrics files (parameterized)
        // Total: 2 + 18 + 18 = 38 files
        assert_eq!(spec.files.as_ref().unwrap().len(), 38);
    }

    #[test]
    fn test_example_file_hyperparameter_sweep_shared_params_kdl() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("examples/kdl/hyperparameter_sweep_shared_params.kdl");

        let mut spec = WorkflowSpec::from_spec_file(&path)
            .expect("Failed to load hyperparameter_sweep_shared_params.kdl");

        // Verify workflow-level parameters were parsed
        assert!(spec.parameters.is_some());

        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // Should have same structure as YAML version: 38 jobs, 38 files
        assert_eq!(spec.jobs.len(), 38);
        assert_eq!(spec.files.as_ref().unwrap().len(), 38);
    }

    #[test]
    fn test_example_file_hyperparameter_sweep_shared_params_json5() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("examples/json/hyperparameter_sweep_shared_params.json5");

        let mut spec = WorkflowSpec::from_spec_file(&path)
            .expect("Failed to load hyperparameter_sweep_shared_params.json5");

        // Verify workflow-level parameters were parsed
        assert!(spec.parameters.is_some());

        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // Should have same structure as YAML/KDL versions: 38 jobs, 38 files
        assert_eq!(spec.jobs.len(), 38);
        assert_eq!(spec.files.as_ref().unwrap().len(), 38);
    }

    // ==================== Zip Parameter Mode Tests ====================

    #[test]
    fn test_zip_parameter_mode_yaml() {
        let yaml_content = r#"
name: test_zip_parameters
description: Test zip parameter mode in YAML

jobs:
  - name: train_{dataset}_{model}
    command: python train.py --dataset={dataset} --model={model}
    parameters:
      dataset: "['cifar10', 'mnist', 'imagenet']"
      model: "['resnet', 'vgg', 'transformer']"
    parameter_mode: zip
"#;

        let mut spec = WorkflowSpec::from_spec_file_content(yaml_content, "yaml")
            .expect("Failed to parse YAML workflow spec");

        // Before expansion, should have 1 job
        assert_eq!(spec.jobs.len(), 1);
        assert_eq!(spec.jobs[0].parameter_mode, Some("zip".to_string()));

        // Expand parameters
        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // With zip mode: 3 zipped pairs, not 9 combinations
        assert_eq!(spec.jobs.len(), 3);
        assert_eq!(spec.jobs[0].name, "train_cifar10_resnet");
        assert_eq!(spec.jobs[1].name, "train_mnist_vgg");
        assert_eq!(spec.jobs[2].name, "train_imagenet_transformer");

        // Parameters and parameter_mode should be removed from expanded jobs
        for job in &spec.jobs {
            assert!(job.parameters.is_none());
            assert!(job.parameter_mode.is_none());
        }
    }

    #[test]
    fn test_zip_parameter_mode_json() {
        let json_content = r#"
{
    "name": "test_zip_parameters",
    "jobs": [
        {
            "name": "process_{input}_{output}",
            "command": "convert {input} {output}",
            "parameters": {
                "input": "['a.txt', 'b.txt']",
                "output": "['a.out', 'b.out']"
            },
            "parameter_mode": "zip"
        }
    ]
}
"#;

        let mut spec = WorkflowSpec::from_spec_file_content(json_content, "json")
            .expect("Failed to parse JSON workflow spec");

        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // With zip mode: 2 zipped pairs
        assert_eq!(spec.jobs.len(), 2);
        assert_eq!(spec.jobs[0].name, "process_a.txt_a.out");
        assert_eq!(spec.jobs[1].name, "process_b.txt_b.out");
    }

    #[test]
    fn test_zip_parameter_mode_kdl() {
        let kdl_content = r#"
name "test_zip_parameters"
description "Test zip parameter mode in KDL"

job "run_{stage}_{config}" {
    command "execute --stage={stage} --config={config}"
    parameters {
        stage "[1, 2, 3]"
        config "['a', 'b', 'c']"
    }
    parameter_mode "zip"
}
"#;

        let mut spec = WorkflowSpec::from_spec_file_content(kdl_content, "kdl")
            .expect("Failed to parse KDL workflow spec");

        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // With zip mode: 3 zipped pairs
        assert_eq!(spec.jobs.len(), 3);
        assert_eq!(spec.jobs[0].name, "run_1_a");
        assert_eq!(spec.jobs[1].name, "run_2_b");
        assert_eq!(spec.jobs[2].name, "run_3_c");
    }

    #[test]
    fn test_zip_parameter_mode_file_spec() {
        let yaml_content = r#"
name: test_zip_file_parameters
description: Test zip parameter mode for files

jobs:
  - name: dummy_job
    command: echo dummy

files:
  - name: data_{dataset}_{split}
    path: /data/{dataset}/{split}.csv
    parameters:
      dataset: "['train', 'test', 'val']"
      split: "['2023', '2024', '2025']"
    parameter_mode: zip
"#;

        let mut spec = WorkflowSpec::from_spec_file_content(yaml_content, "yaml")
            .expect("Failed to parse YAML workflow spec");

        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // With zip mode: 3 zipped pairs
        let files = spec.files.as_ref().unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].name, "data_train_2023");
        assert_eq!(files[0].path, "/data/train/2023.csv");
        assert_eq!(files[1].name, "data_test_2024");
        assert_eq!(files[2].name, "data_val_2025");
    }

    #[test]
    fn test_zip_parameter_mode_mismatched_lengths_error() {
        let yaml_content = r#"
name: test_zip_mismatched
jobs:
  - name: job_{a}_{b}
    command: echo {a} {b}
    parameters:
      a: "[1, 2, 3]"
      b: "['x', 'y']"
    parameter_mode: zip
"#;

        let mut spec = WorkflowSpec::from_spec_file_content(yaml_content, "yaml")
            .expect("Failed to parse YAML workflow spec");

        // Expansion should fail due to mismatched lengths
        let result = spec.expand_parameters();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("same number of values"));
    }

    #[test]
    fn test_product_parameter_mode_explicit() {
        // Test that explicit "product" mode works the same as default
        let yaml_content = r#"
name: test_product_explicit
jobs:
  - name: job_{a}_{b}
    command: echo {a} {b}
    parameters:
      a: "[1, 2]"
      b: "['x', 'y']"
    parameter_mode: product
"#;

        let mut spec = WorkflowSpec::from_spec_file_content(yaml_content, "yaml")
            .expect("Failed to parse YAML workflow spec");

        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // With product mode: 2 * 2 = 4 combinations
        assert_eq!(spec.jobs.len(), 4);
    }

    #[test]
    fn test_default_parameter_mode_is_product() {
        // Test that default mode (no parameter_mode specified) is Cartesian product
        let yaml_content = r#"
name: test_default_mode
jobs:
  - name: job_{a}_{b}
    command: echo {a} {b}
    parameters:
      a: "[1, 2]"
      b: "['x', 'y']"
"#;

        let mut spec = WorkflowSpec::from_spec_file_content(yaml_content, "yaml")
            .expect("Failed to parse YAML workflow spec");

        spec.expand_parameters()
            .expect("Failed to expand parameters");

        // Default should be product mode: 2 * 2 = 4 combinations
        assert_eq!(spec.jobs.len(), 4);
    }
}
