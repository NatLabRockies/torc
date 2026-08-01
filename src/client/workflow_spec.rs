use crate::client::apis::{self, configuration::Configuration};
use crate::client::parameter_expansion::{
    ParameterValue, cartesian_product, load_parameter_table, parse_parameter_value,
    substitute_parameters, zip_parameters,
};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use crate::models;
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize};

/// Deserialize parameter maps while accepting both the legacy string syntax and
/// native YAML/JSON sequences. Sequences are normalized to the existing list
/// syntax so the parameter expansion code has one representation to process.
fn deserialize_parameter_map<'de, D>(
    deserializer: D,
) -> Result<Option<HashMap<String, String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let values = Option::<HashMap<String, serde_json::Value>>::deserialize(deserializer)?;
    values
        .map(|values| {
            values
                .into_iter()
                .map(|(name, value)| {
                    let value = match value {
                        serde_json::Value::String(value) => value,
                        value => serde_json::to_string(&value).map_err(serde::de::Error::custom)?,
                    };
                    Ok((name, value))
                })
                .collect()
        })
        .transpose()
}

static SRUN_MPI_MODE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9+_.-]+$").expect("hardcoded regex must compile"));

/// Build the set of parameter combinations for a parameterized spec.
///
/// The combinations come from exactly one source:
/// - `parameters_file`: a CSV/JSON table where each row is one combination.
/// - `parameters`: inline parameter values combined via `parameter_mode`
///   ("product" by default, or "zip").
///
/// Returns `Ok(None)` when the spec is not parameterized (neither source set),
/// signalling the caller to emit a single un-expanded clone.
///
/// The two sources are mutually exclusive: setting `parameters_file` alongside
/// inline `parameters`/`parameter_mode`/`use_parameters` is an error. This guard
/// also catches callers that invoke `expand()` directly, without going through
/// the workflow-level [`validate_parameter_source`] check.
fn build_parameter_combinations(
    parameters: &Option<HashMap<String, String>>,
    parameter_mode: &Option<String>,
    use_parameters: &Option<Vec<String>>,
    parameters_file: &Option<String>,
) -> Result<Option<Vec<HashMap<String, ParameterValue>>>, String> {
    if let Some(path) = parameters_file {
        if parameters.is_some() || parameter_mode.is_some() || use_parameters.is_some() {
            return Err(
                "`parameters_file` cannot be combined with `parameters`, `parameter_mode`, or \
                 `use_parameters`"
                    .to_string(),
            );
        }
        return Ok(Some(load_parameter_table(path)?));
    }

    let Some(params) = parameters else {
        return Ok(None);
    };

    let mut parsed_params: HashMap<String, Vec<ParameterValue>> = HashMap::new();
    for (name, value) in params {
        parsed_params.insert(name.clone(), parse_parameter_value(value)?);
    }

    let combinations = match parameter_mode.as_deref().unwrap_or("product") {
        "zip" => zip_parameters(&parsed_params)?,
        _ => cartesian_product(&parsed_params),
    };
    Ok(Some(combinations))
}

/// Validate that a parameterized spec uses only one parameter source.
///
/// A CSV/JSON parameter table -- whether a local `parameters_file` or the
/// workflow-level table opted into via `use_parameters_file: true` -- defines
/// explicit combinations, so it cannot be combined with the inline
/// `parameters`/`parameter_mode` mechanism or with `use_parameters` inheritance.
fn validate_parameter_source(
    label: &str,
    parameters: &Option<HashMap<String, String>>,
    parameter_mode: &Option<String>,
    use_parameters: &Option<Vec<String>>,
    parameters_file: &Option<String>,
    use_parameters_file: Option<bool>,
    workflow_parameters_file: &Option<String>,
) -> Result<(), String> {
    let opts_into_workflow_table = use_parameters_file == Some(true);
    let table_source = parameters_file.is_some() || opts_into_workflow_table;
    let inline_source =
        parameters.is_some() || use_parameters.is_some() || parameter_mode.is_some();

    if table_source && inline_source {
        return Err(format!(
            "{}: a CSV/JSON parameter table (`parameters_file`/`use_parameters_file`) cannot be \
             combined with `parameters`, `parameter_mode`, or `use_parameters`",
            label
        ));
    }
    if parameters_file.is_some() && opts_into_workflow_table {
        return Err(format!(
            "{}: set either a local `parameters_file` or `use_parameters_file: true`, not both",
            label
        ));
    }
    if opts_into_workflow_table && workflow_parameters_file.is_none() {
        return Err(format!(
            "{}: `use_parameters_file: true` requires a workflow-level `parameters_file`",
            label
        ));
    }
    Ok(())
}

/// Matches the four workflow-variable forms understood by [`substitute_and_extract`]:
///   `${files.input.NAME}`, `${files.output.NAME}`,
///   `${user_data.input.NAME}`, `${user_data.output.NAME}`.
/// Group 1 = namespace (`files`|`user_data`), group 2 = direction
/// (`input`|`output`), group 3 = name (any character except `}`).
static WORKFLOW_VARIABLE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$\{(files|user_data)\.(input|output)\.([^}]+)\}")
        .expect("hardcoded regex must compile")
});

pub(crate) fn validate_srun_mpi_value(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("srun_mpi must not be empty when provided. \
             Set a non-empty Slurm MPI mode such as 'pmix' or omit the field."
            .to_string());
    }
    if trimmed != value || !SRUN_MPI_MODE_REGEX.is_match(trimmed) {
        return Err(
            "srun_mpi must be a single safe token matching [A-Za-z0-9+_.-]+ \
             (for example 'none' or 'pmix')."
                .to_string(),
        );
    }
    Ok(())
}

fn validate_env_var_name(name: &str) -> Result<(), String> {
    if models::is_valid_env_var_name(name) {
        Ok(())
    } else {
        Err(format!(
            "invalid environment variable name '{}'; expected [A-Za-z_][A-Za-z0-9_]*",
            name
        ))
    }
}
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
    /// Whether the workflow has a schedule_nodes action that `torc submit` can fire
    /// (on_workflow_start, on_jobs_ready, or on_jobs_complete)
    pub has_schedule_nodes_action: bool,
    /// List of job names that would be created
    pub job_names: Vec<String>,
    /// List of scheduler names
    pub scheduler_names: Vec<String>,
}

#[cfg(feature = "client")]
use kdl::{KdlDocument, KdlNode};

/// File specification for JSON serialization (without workflow_id and id)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileSpec {
    /// Name of the file
    pub name: String,
    /// Path to the file
    pub path: String,
    /// Optional stable RO-Crate identifier for this file (e.g. a DOI, PURL, or URN).
    /// When provided, this string is used as the `@id` of the file's RO-Crate entity
    /// instead of the file path. The path is still recorded as `sameAs` so the
    /// local location is preserved. Identifiers must be unique within the workflow
    /// after parameter expansion. Parameter tokens (`{name}` / `{name:fmt}`) are
    /// substituted into the identifier just like `name` and `path`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    /// File modification time as Unix timestamp (seconds since epoch).
    /// If not specified, torc automatically checks if the file exists on disk
    /// during workflow creation and uses its actual modification time.
    /// This distinguishes input files (exist before workflow) from output files
    /// (created by jobs). Used by RO-Crate for automatic entity generation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub st_mtime: Option<f64>,
    /// Optional parameters for generating multiple files
    /// Supports range notation (e.g., "1:100" or "1:100:5") and lists (e.g., "[1,5,10]")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, deserialize_with = "deserialize_parameter_map")]
    pub parameters: Option<HashMap<String, String>>,
    /// How to combine multiple parameters: "product" (default, Cartesian product) or "zip"
    /// With "zip", parameters are combined element-wise (all must have the same length)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_mode: Option<String>,
    /// Names of workflow-level parameters to use for this file
    /// If set, only these parameters from the workflow will be used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_parameters: Option<Vec<String>>,
    /// Path to a CSV or JSON file supplying parameter combinations as a table.
    /// Each CSV row / JSON array object becomes one generated file. Mutually
    /// exclusive with `parameters`, `parameter_mode`, and `use_parameters`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters_file: Option<String>,
    /// Expand this file over the workflow-level `parameters_file` table when set to
    /// true. Mutually exclusive with the per-file parameter sources above.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_parameters_file: Option<bool>,
}

impl FileSpec {
    /// Create a new FileSpec with only required fields
    #[allow(dead_code)]
    pub fn new(name: String, path: String) -> FileSpec {
        FileSpec {
            name,
            path,
            identifier: None,
            st_mtime: None,
            parameters: None,
            parameter_mode: None,
            use_parameters: None,
            parameters_file: None,
            use_parameters_file: None,
        }
    }

    /// Expand this FileSpec into multiple FileSpecs based on its parameters
    /// Returns a single-element vec if no parameters are present
    pub fn expand(&self) -> Result<Vec<FileSpec>, String> {
        let combinations = match build_parameter_combinations(
            &self.parameters,
            &self.parameter_mode,
            &self.use_parameters,
            &self.parameters_file,
        )? {
            Some(combos) => combos,
            None => return Ok(vec![self.clone()]),
        };

        // Create a FileSpec for each combination
        let mut expanded = Vec::new();
        for combo in combinations {
            let mut new_spec = self.clone();
            new_spec.parameters = None; // Remove parameters from expanded specs
            new_spec.parameter_mode = None; // Remove parameter_mode from expanded specs
            new_spec.parameters_file = None; // Remove parameters_file from expanded specs

            // Substitute parameters in name, path, and (when set) identifier.
            new_spec.name = substitute_parameters(&self.name, &combo);
            new_spec.path = substitute_parameters(&self.path, &combo);
            new_spec.identifier = self
                .identifier
                .as_deref()
                .map(|template| substitute_parameters(template, &combo));

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
    /// Optional parameters for generating multiple user_data records
    /// Supports range notation (e.g., "1:100" or "1:100:5") and lists (e.g., "[1,5,10]").
    /// Tokens of the form `{param_name}` or `{param_name:format}` are substituted into
    /// `name` and into any string value found inside `data` (recursively).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, deserialize_with = "deserialize_parameter_map")]
    pub parameters: Option<HashMap<String, String>>,
    /// How to combine multiple parameters: "product" (default, Cartesian product) or "zip"
    /// With "zip", parameters are combined element-wise (all must have the same length)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_mode: Option<String>,
    /// Names of workflow-level parameters to use for this user_data
    /// If set, only these parameters from the workflow will be used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_parameters: Option<Vec<String>>,
    /// Path to a CSV or JSON file supplying parameter combinations as a table.
    /// Each CSV row / JSON array object becomes one generated user_data record.
    /// Mutually exclusive with `parameters`, `parameter_mode`, and `use_parameters`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters_file: Option<String>,
    /// Expand this user_data over the workflow-level `parameters_file` table when set
    /// to true. Mutually exclusive with the per-record parameter sources above.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_parameters_file: Option<bool>,
}

impl UserDataSpec {
    /// Expand this UserDataSpec into multiple UserDataSpecs based on its parameters.
    /// Returns a single-element vec if no parameters are present.
    ///
    /// Parameter tokens (`{name}` / `{name:fmt}`) are substituted into `name` and into
    /// every string value found anywhere inside `data` (recursively walking objects and
    /// arrays). Non-string JSON values (numbers, bools, null) are not modified, even
    /// though they could in principle be rewritten -- substitution is string-only,
    /// matching how FileSpec handles `name` and `path`.
    pub fn expand(&self) -> Result<Vec<UserDataSpec>, String> {
        let combinations = match build_parameter_combinations(
            &self.parameters,
            &self.parameter_mode,
            &self.use_parameters,
            &self.parameters_file,
        )? {
            Some(combos) => combos,
            None => return Ok(vec![self.clone()]),
        };

        // Create a UserDataSpec for each combination
        let mut expanded = Vec::new();
        for combo in combinations {
            let mut new_spec = self.clone();
            new_spec.parameters = None; // Remove parameters from expanded specs
            new_spec.parameter_mode = None; // Remove parameter_mode from expanded specs
            new_spec.parameters_file = None; // Remove parameters_file from expanded specs

            // Substitute parameters in name (if any)
            if let Some(ref n) = self.name {
                new_spec.name = Some(substitute_parameters(n, &combo));
            }

            // Substitute parameters recursively inside data, if present
            if let Some(ref data) = self.data {
                let mut substituted = data.clone();
                substitute_parameters_in_json(&mut substituted, &combo);
                new_spec.data = Some(substituted);
            }

            expanded.push(new_spec);
        }

        Ok(expanded)
    }
}

/// Recursively walk a `serde_json::Value` and substitute parameter tokens
/// (`{name}` / `{name:fmt}`) in every string node. Object keys are not rewritten
/// (they are identifiers); only string values inside objects/arrays change.
fn substitute_parameters_in_json(
    value: &mut serde_json::Value,
    params: &HashMap<String, ParameterValue>,
) {
    match value {
        serde_json::Value::String(s) => {
            *s = substitute_parameters(s, params);
        }
        serde_json::Value::Array(items) => {
            for item in items {
                substitute_parameters_in_json(item, params);
            }
        }
        serde_json::Value::Object(map) => {
            for v in map.values_mut() {
                substitute_parameters_in_json(v, params);
            }
        }
        _ => {}
    }
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
    /// Run this scheduler's allocations strictly one at a time.
    ///
    /// Every allocation submitted for this scheduler shares one Slurm job name and
    /// carries `--dependency=singleton`, so Slurm chains them instead of running them
    /// concurrently. Submit N allocations up front and each starts as its predecessor
    /// finishes -- useful when a workflow's sequential work outlives a single walltime.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serialize_allocations: Option<bool>,
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
    /// Environment variables to export for this job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
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
    #[serde(default, deserialize_with = "deserialize_parameter_map")]
    pub parameters: Option<HashMap<String, String>>,
    /// How to combine multiple parameters: "product" (default, Cartesian product) or "zip"
    /// With "zip", parameters are combined element-wise (all must have the same length)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_mode: Option<String>,
    /// Names of workflow-level parameters to use for this job
    /// If set, only these parameters from the workflow will be used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_parameters: Option<Vec<String>>,
    /// Path to a CSV or JSON file supplying parameter combinations as a table.
    /// Each CSV row / JSON array object becomes one generated job, with its
    /// columns/keys available for template substitution. Mutually exclusive with
    /// `parameters`, `parameter_mode`, and `use_parameters`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters_file: Option<String>,
    /// Expand this job over the workflow-level `parameters_file` table when set to
    /// true. Mutually exclusive with the per-job parameter sources above.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_parameters_file: Option<bool>,
    /// Per-job override for stdout/stderr capture configuration.
    /// If set, overrides the workflow-level `execution_config.stdio` for this job.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdio: Option<StdioConfig>,
    /// Scheduling priority; higher values are submitted to workers first. Minimum 0, default 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i64>,
}

impl JobSpec {
    /// Create a new JobSpec with only required fields
    #[allow(dead_code)]
    pub fn new(name: String, command: String) -> JobSpec {
        JobSpec {
            name,
            command,
            invocation_script: None,
            env: None,
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
            parameters_file: None,
            use_parameters_file: None,
            stdio: None,
            priority: None,
        }
    }

    /// Expand this JobSpec into multiple JobSpecs based on its parameters
    /// Returns a single-element vec if no parameters are present
    pub fn expand(&self) -> Result<Vec<JobSpec>, String> {
        let combinations = match build_parameter_combinations(
            &self.parameters,
            &self.parameter_mode,
            &self.use_parameters,
            &self.parameters_file,
        )? {
            Some(combos) => combos,
            None => return Ok(vec![self.clone()]),
        };

        // Create a JobSpec for each combination
        let mut expanded = Vec::new();
        for combo in combinations {
            let mut new_spec = self.clone();
            new_spec.parameters = None; // Remove parameters from expanded specs
            new_spec.parameter_mode = None; // Remove parameter_mode from expanded specs
            new_spec.parameters_file = None; // Remove parameters_file from expanded specs

            // Substitute parameters in all string fields
            new_spec.name = substitute_parameters(&self.name, &combo);
            new_spec.command = substitute_parameters(&self.command, &combo);

            if let Some(ref script) = self.invocation_script {
                new_spec.invocation_script = Some(substitute_parameters(script, &combo));
            }

            if let Some(ref env) = self.env {
                new_spec.env = Some(
                    env.iter()
                        .map(|(key, value)| (key.clone(), substitute_parameters(value, &combo)))
                        .collect(),
                );
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

// Stdio + Execution config types now live in `crate::models` so the OpenAPI
// surface can expose them as typed nested objects. The aliases below preserve
// the existing import paths for the rest of the crate.
pub use crate::models::{ExecutionConfig, ExecutionMode, StdioConfig, StdioMode};

/// Apply workflow-level `variables` substitution to every string in the spec value.
///
/// Runs before `serde_json::from_value` so that all string fields -- including ones
/// that are not currently parameter-substituted -- benefit. The `variables` map is
/// preserved in the output Value for round-trip serialization.
///
/// Skip rules: keys of `parameters` maps and entries of `use_parameters` arrays are
/// identifiers, not user-facing strings, so they are not substituted.
fn apply_workflow_variables(
    mut value: serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let serde_json::Value::Object(ref map) = value else {
        return Ok(value);
    };
    let Some(vars_value) = map.get("variables") else {
        return Ok(value);
    };
    let serde_json::Value::Object(vars_map) = vars_value else {
        return Err("workflow `variables` must be an object of string key/value pairs".into());
    };
    if vars_map.is_empty() {
        return Ok(value);
    }

    let mut variables: HashMap<String, ParameterValue> = HashMap::with_capacity(vars_map.len());
    for (name, value) in vars_map {
        if !is_identifier(name) {
            return Err(format!(
                "workflow variable name '{}' must be a valid identifier \
                 ([A-Za-z_][A-Za-z0-9_]*). Rename the variable.",
                name
            )
            .into());
        }
        let serde_json::Value::String(s) = value else {
            return Err(format!(
                "workflow variable '{}' must be a string (got {})",
                name,
                json_value_kind(value)
            )
            .into());
        };
        variables.insert(name.clone(), ParameterValue::String(s.clone()));
    }

    let mut parameter_names: HashSet<String> = HashSet::new();
    collect_parameter_names(&value, &variables, &mut parameter_names);
    let mut collisions: Vec<&String> = variables
        .keys()
        .filter(|name| parameter_names.contains(*name))
        .collect();
    if !collisions.is_empty() {
        collisions.sort();
        return Err(format!(
            "workflow `variables` collide with parameter names: {}. \
             Rename the variable(s) or the parameter(s) so each name appears in only one map.",
            collisions
                .iter()
                .map(|n| n.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
        .into());
    }

    // Variable values must be plain literal strings: no `{...}` template
    // references at all (shell-style `${...}` is allowed, since it is reserved
    // for shell expansion and the `${files.input.X}` family). Allowing template
    // references inside variable values would either (a) make resolution
    // order-dependent when one variable references another (HashMap iteration
    // is randomized and cycles would not be detected), or (b) leak unresolved
    // parameter tokens into wherever the variable is used. Composition belongs
    // at the use site -- e.g. `command: "{base}/{sub}"`, not
    // `combo: "{base}/{sub}"`.
    for (name, vars_value) in vars_map {
        let serde_json::Value::String(s) = vars_value else {
            continue; // shape already validated above; non-strings already errored
        };
        check_variable_value_tokens(name, s, &variables)?;
    }

    let valid_token_names: HashSet<String> = variables
        .keys()
        .cloned()
        .chain(parameter_names.iter().cloned())
        .collect();

    substitute_variables_in_value(&mut value, &variables, &valid_token_names, false)?;

    Ok(value)
}

/// Validate the tokens inside a workflow variable's value.
///
/// Variable values must be plain literal strings: any `{name}` template
/// reference is rejected. Shell-style `${...}` is allowed (it is reserved for
/// shell expansion and for the `${files.input.X}` / `${user_data.input.X}`
/// substitution that runs later in the workflow lifecycle).
///
/// The "references another variable" branch produces a more pointed error;
/// every other template reference (parameter names, undefined names) is
/// rejected with the same uniform "must be a literal" message.
fn check_variable_value_tokens(
    var_name: &str,
    s: &str,
    variables: &HashMap<String, ParameterValue>,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }
        if i > 0 && bytes[i - 1] == b'$' {
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut j = start;
        while j < bytes.len() && bytes[j] != b'}' && bytes[j] != b'{' {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b'}' {
            i += 1;
            continue;
        }
        let inner = &s[start..j];
        let token_name = inner.split(':').next().unwrap_or("");
        if !is_identifier(token_name) {
            i = j + 1;
            continue;
        }
        return if variables.contains_key(token_name) {
            Err(format!(
                "variable '{}' value '{}' references another variable '{{{}}}'. \
                 Variable values may not reference other variables; resolution \
                 order would be undefined. Inline the constant or compose at the \
                 use site.",
                var_name, s, inner
            )
            .into())
        } else {
            Err(format!(
                "variable '{}' value '{}' contains template reference '{{{}}}'. \
                 Variable values must be plain literal strings (shell-style \
                 `${{...}}` is allowed). Compose at the use site instead.",
                var_name, s, inner
            )
            .into())
        };
    }
    Ok(())
}

fn json_value_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Collect every parameter name declared anywhere in the spec value.
/// Looks at top-level `parameters`, and at `parameters` inside any object found
/// in the `jobs`, `files`, or `user_data` arrays.
fn collect_parameter_names(
    value: &serde_json::Value,
    variables: &HashMap<String, ParameterValue>,
    out: &mut HashSet<String>,
) {
    let serde_json::Value::Object(map) = value else {
        return;
    };
    if let Some(serde_json::Value::Object(params)) = map.get("parameters") {
        for k in params.keys() {
            out.insert(k.clone());
        }
    }

    // Cache column lookups by resolved path so a `parameters_file` referenced by
    // multiple jobs/files/user_data (or shared at the workflow level) is parsed
    // at most once during this validation pass.
    let mut table_cache: HashMap<String, Option<Vec<String>>> = HashMap::new();

    // Column names from a workflow-level `parameters_file` are shared with any
    // job/file/user_data that opts in via `use_parameters_file: true`, so we add
    // them to the global valid-token set for the pre-substitution check.
    let workflow_table_cols: Option<Vec<String>> = map
        .get("parameters_file")
        .and_then(|v| table_columns_for_path(v, variables, &mut table_cache));
    if let Some(cols) = workflow_table_cols.as_ref() {
        for k in cols {
            out.insert(k.clone());
        }
    }

    for field in ["jobs", "files", "user_data"] {
        let Some(serde_json::Value::Array(items)) = map.get(field) else {
            continue;
        };
        for item in items {
            let serde_json::Value::Object(item_map) = item else {
                continue;
            };
            if let Some(serde_json::Value::Object(params)) = item_map.get("parameters") {
                for k in params.keys() {
                    out.insert(k.clone());
                }
            }
            if let Some(cols) = item_map
                .get("parameters_file")
                .and_then(|v| table_columns_for_path(v, variables, &mut table_cache))
            {
                for k in cols {
                    out.insert(k);
                }
            }
            if matches!(
                item_map.get("use_parameters_file"),
                Some(serde_json::Value::Bool(true))
            ) && let Some(cols) = workflow_table_cols.as_ref()
            {
                for k in cols {
                    out.insert(k.clone());
                }
            }
        }
    }
}

/// Resolve a `parameters_file` JSON value to its column names for the
/// pre-substitution validation pass.
///
/// The path may still contain workflow-variable tokens (e.g.
/// `"{data_dir}/sweep.csv"`), so any `variables` are substituted before the file
/// is read. Column collection is strictly best-effort: a table that cannot be
/// read here (missing file, unresolved non-variable token, parse error) yields
/// `None` rather than an error.
///
/// Returning `None` means the table's columns are *not* added to the valid-token
/// set, so if the spec references a `{column}` token from an unreadable table the
/// undefined-token check may surface that first (reporting the token) rather than
/// the table-read failure. When the table *is* readable, an authoritative
/// diagnostic for any remaining problem is still left to `expand_parameters`,
/// which reads the fully-substituted spec.
///
/// `cache` memoizes results by resolved path so a table shared across multiple
/// specs is parsed at most once per validation pass.
fn table_columns_for_path(
    value: &serde_json::Value,
    variables: &HashMap<String, ParameterValue>,
    cache: &mut HashMap<String, Option<Vec<String>>>,
) -> Option<Vec<String>> {
    let serde_json::Value::String(path) = value else {
        return None;
    };
    let resolved = substitute_workflow_variables_in_string(path, variables);
    cache
        .entry(resolved.clone())
        .or_insert_with(|| load_parameter_table_columns(&resolved).ok())
        .clone()
}

/// Read a `parameters_file` and return the union of its column names across all
/// rows. Used during the pre-substitution validation pass so that `{col}` tokens
/// driven by a CSV/JSON table are recognized as valid parameter references.
///
/// The union (rather than just the first row) matters for JSON/JSONL tables,
/// whose objects are not required to share a uniform key set.
fn load_parameter_table_columns(path: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let rows =
        load_parameter_table(path).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    let mut cols: HashSet<String> = HashSet::new();
    for row in &rows {
        cols.extend(row.keys().cloned());
    }
    let mut cols: Vec<String> = cols.into_iter().collect();
    cols.sort();
    Ok(cols)
}

/// Recursively walk a JSON value, substituting `{var}` and `{var:fmt}` in every
/// string node. Tokens whose name matches a parameter (rather than a variable)
/// are left intact for later parameter expansion. Tokens whose name matches
/// neither are reported as undefined-variable errors.
///
/// `inside_variables` is true when the caller has descended into the top-level
/// `variables` map; in that scope the values must not be touched (they are the
/// substitution source itself).
fn substitute_variables_in_value(
    value: &mut serde_json::Value,
    variables: &HashMap<String, ParameterValue>,
    valid_token_names: &HashSet<String>,
    inside_variables: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    match value {
        serde_json::Value::String(s) => {
            if !inside_variables {
                check_undefined_tokens(s, valid_token_names)?;
                *s = substitute_workflow_variables_in_string(s, variables);
            }
            Ok(())
        }
        serde_json::Value::Array(items) => {
            for item in items {
                substitute_variables_in_value(
                    item,
                    variables,
                    valid_token_names,
                    inside_variables,
                )?;
            }
            Ok(())
        }
        serde_json::Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if inside_variables {
                    // Don't substitute inside the variables map -- those strings define
                    // the substitution source, not consumers of it.
                    continue;
                }
                if key == "variables" {
                    substitute_variables_in_value(child, variables, valid_token_names, true)?;
                    continue;
                }
                if key == "parameters" {
                    // Substitute only in the values of the parameters map; keys are
                    // identifiers and must remain untouched.
                    if let serde_json::Value::Object(params) = child {
                        for v in params.values_mut() {
                            substitute_variables_in_value(v, variables, valid_token_names, false)?;
                        }
                    }
                    continue;
                }
                if key == "use_parameters" {
                    // Identifiers, not user-facing strings.
                    continue;
                }
                substitute_variables_in_value(child, variables, valid_token_names, false)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Scan a string for `{name}` and `{name:fmt}` tokens; error if any token's name
/// does not appear in `valid_token_names`. Tokens with a non-identifier name
/// (e.g. format-only `{:>5}`, JSON-like `{"x": 1}`) are ignored on the assumption
/// they are unrelated to template substitution.
///
/// `${...}` blocks are skipped: that syntax is reserved for shell-style variable
/// expansion and the repo's existing `${files.input.X}` / `${user_data.input.X}`
/// substitution. They are never treated as workflow variable references.
fn check_undefined_tokens(
    s: &str,
    valid_token_names: &HashSet<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }
        // Skip `${...}` -- that's shell-style variable expansion, not a workflow
        // variable reference.
        if i > 0 && bytes[i - 1] == b'$' {
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut j = start;
        while j < bytes.len() && bytes[j] != b'}' && bytes[j] != b'{' {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b'}' {
            i += 1;
            continue;
        }
        let inner = &s[start..j];
        let name = inner.split(':').next().unwrap_or("");
        if is_identifier(name) && !valid_token_names.contains(name) {
            return Err(format!(
                "undefined template name '{{{}}}' in '{}': not declared in `variables` \
                 or any `parameters` map. Add it to `variables` or fix the typo.",
                inner, s
            )
            .into());
        }
        i = j + 1;
    }
    Ok(())
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Substitute workflow variables into a string in a single pass.
///
/// Replaces `{name}` and `{name:fmt}` with the value of `variables[name]` when
/// `name` matches a key. Unmatched tokens (parameter names that get expanded
/// later, or tokens whose name is non-identifier text) are left intact.
///
/// Critically, `${...}` blocks are skipped entirely so that shell-style
/// expansions like `${HOME}` or `${TORC_JOB_ID}` are preserved verbatim --
/// even when a variable happens to share a name with a shell variable. This
/// is what `substitute_parameters` (used by parameter expansion) does *not*
/// guarantee, since it relies on naive `string.replace`.
fn substitute_workflow_variables_in_string(
    s: &str,
    variables: &HashMap<String, ParameterValue>,
) -> String {
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut last_copied = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }
        if i > 0 && bytes[i - 1] == b'$' {
            // Shell-style ${...}; do not substitute.
            i += 1;
            continue;
        }
        let start = i + 1;
        let Some(rel_end) = s[start..].find('}') else {
            i += 1;
            continue;
        };
        let inner_end = start + rel_end;
        let inner = &s[start..inner_end];
        let (name, fmt) = match inner.split_once(':') {
            Some((n, f)) => (n, Some(f)),
            None => (inner, None),
        };
        let Some(value) = variables.get(name) else {
            // Not a workflow variable -- leave intact (it might be a parameter
            // name that gets expanded later, or just literal text).
            i = inner_end + 1;
            continue;
        };
        result.push_str(&s[last_copied..i]);
        result.push_str(&value.format(fmt));
        i = inner_end + 1;
        last_copied = i;
    }
    result.push_str(&s[last_copied..]);
    result
}

/// Dynamic job spawning (orchestrator continuation) configuration.
///
/// The spec and the persisted `WorkflowModel.dynamic_jobs` share one type
/// — they're identical by design.
pub use crate::models::DynamicJobsConfig as DynamicJobsSpec;

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
    #[serde(default, deserialize_with = "deserialize_parameter_map")]
    pub parameters: Option<HashMap<String, String>>,
    /// Shared CSV/JSON parameter table for the whole workflow. Jobs/files/user_data
    /// opt in by setting `use_parameters_file: true`, which expands them over every
    /// row of this table. Mutually exclusive with the workflow-level `parameters`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters_file: Option<String>,
    /// Workflow-level constants substituted into every string field of the spec.
    /// Unlike `parameters`, variables do not trigger Cartesian expansion -- each
    /// `{name}` reference is replaced once with the variable's value before the
    /// spec is processed. Variable names must not collide with any parameter name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<HashMap<String, String>>,
    /// Environment variables exported for every job in the workflow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    /// Inform all compute nodes to shut down this number of seconds before the expiration time
    /// Deprecated.
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
    /// Dynamic job spawning (orchestrator continuation) configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_jobs: Option<DynamicJobsSpec>,
    /// Use PendingFailed status for failed jobs (enables AI-assisted recovery)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_pending_failed: Option<bool>,
    /// When true, automatically create RO-Crate entities for workflow files.
    /// Input files get entities during initialization; output files get entities on job completion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_ro_crate: Option<bool>,
    /// Project name or identifier for grouping workflows
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Arbitrary metadata as a JSON object
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    /// Unified execution configuration controlling how jobs are run.
    /// Controls execution mode (direct, slurm, or auto) and related settings like
    /// resource limits, termination signals, and timeouts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_config: Option<ExecutionConfig>,
    /// Names of access groups granted shared access to this workflow.
    /// Names are resolved to group IDs at workflow-creation time; an unknown
    /// name fails the whole create with a clear error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_groups: Option<Vec<String>>,
}

/// A workflow-spec source resolved from a CLI argument that may be `-` (stdin).
///
/// When the argument is `-`, the stdin contents are staged in a temp file whose
/// handle is held here; the file is removed when this value is dropped, so it
/// must outlive any use of [`ResolvedSpecSource::path`].
#[cfg(feature = "client")]
pub struct ResolvedSpecSource {
    _temp: Option<tempfile::NamedTempFile>,
    path: PathBuf,
}

#[cfg(feature = "client")]
impl ResolvedSpecSource {
    /// Path to the spec file (the original argument, or the staged stdin temp file).
    pub fn path(&self) -> &Path {
        &self.path
    }
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
            parameters_file: None,
            variables: None,
            env: None,
            compute_node_expiration_buffer_seconds: None,
            compute_node_wait_for_new_jobs_seconds: None,
            compute_node_ignore_workflow_completion: None,
            compute_node_wait_for_healthy_database_minutes: None,
            jobs,
            files: None,
            user_data: None,
            resource_requirements: None,
            failure_handlers: None,
            slurm_schedulers: None,
            slurm_defaults: None,
            resource_monitor: None,
            actions: None,
            dynamic_jobs: None,
            use_pending_failed: None,
            enable_ro_crate: None,
            project: None,
            metadata: None,
            execution_config: None,
            access_groups: None,
        }
    }

    /// Deserialize a WorkflowSpec from a serde_json::Value
    /// This is the common conversion point for all file formats
    pub fn from_json_value(value: serde_json::Value) -> Result<Self, Box<dyn std::error::Error>> {
        // Check for removed fields and provide helpful migration guidance
        // before serde's deny_unknown_fields produces a generic error.
        if let serde_json::Value::Object(ref map) = value
            && map.contains_key("slurm_config")
        {
            return Err(
                "The 'slurm_config' field has been removed from the workflow spec. \
                 Use 'execution_config' instead.\n\
                 See docs: docs/src/core/reference/workflow-spec.md \
                 and docs/src/core/concepts/execution-modes.md"
                    .into(),
            );
        }
        let value = apply_workflow_variables(value)?;
        Ok(serde_json::from_value(value)?)
    }

    /// Expand all parameterized jobs, files, and user_data in this workflow spec
    /// This modifies the spec in-place, replacing parameterized specs with their expanded versions
    ///
    /// Parameter resolution order:
    /// 1. If job/file/user_data has its own `parameters`, use those (local params override
    ///    workflow params)
    /// 2. If job/file/user_data has `use_parameters`, select only those from workflow-level params
    pub fn expand_parameters(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.parameters.is_some() && self.parameters_file.is_some() {
            return Err(
                "Workflow-level `parameters` and `parameters_file` are mutually \
                        exclusive; use one shared parameter source per workflow"
                    .into(),
            );
        }
        let workflow_params = self.parameters.clone();
        let workflow_parameters_file = self.parameters_file.clone();
        let workflow_env_params: Option<HashMap<String, ParameterValue>> =
            workflow_params.as_ref().map(|params| {
                params
                    .iter()
                    .map(|(key, value)| {
                        let parameter_value = parse_parameter_value(value)
                            .ok()
                            .and_then(|values| {
                                (values.len() == 1)
                                    .then(|| values.into_iter().next())
                                    .flatten()
                            })
                            .unwrap_or_else(|| ParameterValue::String(value.clone()));
                        (key.clone(), parameter_value)
                    })
                    .collect()
            });
        if let (Some(env), Some(params)) = (&mut self.env, workflow_env_params.as_ref()) {
            for value in env.values_mut() {
                *value = substitute_parameters(value, params);
            }
        }

        // Expand all jobs
        let mut expanded_jobs = Vec::new();
        for job in &self.jobs {
            validate_parameter_source(
                &format!("Job '{}'", job.name),
                &job.parameters,
                &job.parameter_mode,
                &job.use_parameters,
                &job.parameters_file,
                job.use_parameters_file,
                &workflow_parameters_file,
            )?;
            // Resolve parameters for this job
            let mut job_with_params = job.clone();
            job_with_params.parameters =
                Self::resolve_parameters(&job.parameters, &job.use_parameters, &workflow_params);
            job_with_params.parameters_file = Self::resolve_parameters_file(
                &job.parameters_file,
                job.use_parameters_file,
                &workflow_parameters_file,
            );
            // Clear the inheritance opt-ins after resolution
            job_with_params.use_parameters = None;
            job_with_params.use_parameters_file = None;

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
                validate_parameter_source(
                    &format!("File '{}'", file.name),
                    &file.parameters,
                    &file.parameter_mode,
                    &file.use_parameters,
                    &file.parameters_file,
                    file.use_parameters_file,
                    &workflow_parameters_file,
                )?;
                // Resolve parameters for this file
                let mut file_with_params = file.clone();
                file_with_params.parameters = Self::resolve_parameters(
                    &file.parameters,
                    &file.use_parameters,
                    &workflow_params,
                );
                file_with_params.parameters_file = Self::resolve_parameters_file(
                    &file.parameters_file,
                    file.use_parameters_file,
                    &workflow_parameters_file,
                );
                // Clear the inheritance opt-ins after resolution
                file_with_params.use_parameters = None;
                file_with_params.use_parameters_file = None;

                let expanded = file_with_params
                    .expand()
                    .map_err(|e| format!("Failed to expand file '{}': {}", file.name, e))?;
                expanded_files.extend(expanded);
            }
            self.files = Some(expanded_files);
        }

        // Expand all user_data
        if let Some(ref user_data) = self.user_data {
            let mut expanded_user_data = Vec::new();
            for ud in user_data {
                validate_parameter_source(
                    &format!("User data '{}'", ud.name.as_deref().unwrap_or("<unnamed>")),
                    &ud.parameters,
                    &ud.parameter_mode,
                    &ud.use_parameters,
                    &ud.parameters_file,
                    ud.use_parameters_file,
                    &workflow_parameters_file,
                )?;
                // Resolve parameters for this user_data record
                let mut ud_with_params = ud.clone();
                ud_with_params.parameters =
                    Self::resolve_parameters(&ud.parameters, &ud.use_parameters, &workflow_params);
                ud_with_params.parameters_file = Self::resolve_parameters_file(
                    &ud.parameters_file,
                    ud.use_parameters_file,
                    &workflow_parameters_file,
                );
                // Clear the inheritance opt-ins after resolution
                ud_with_params.use_parameters = None;
                ud_with_params.use_parameters_file = None;

                let label = ud.name.as_deref().unwrap_or("<unnamed>");
                let expanded = ud_with_params
                    .expand()
                    .map_err(|e| format!("Failed to expand user_data '{}': {}", label, e))?;
                expanded_user_data.extend(expanded);
            }
            self.user_data = Some(expanded_user_data);
        }

        Ok(())
    }

    /// Resolve the effective `parameters_file` for a job, file, or user_data.
    ///
    /// A local `parameters_file` takes precedence; otherwise `use_parameters_file: true`
    /// inherits the workflow-level table. Returns `None` when neither applies.
    /// Validation (mutual exclusion, missing workflow table) is handled separately
    /// by [`validate_parameter_source`].
    fn resolve_parameters_file(
        local_file: &Option<String>,
        use_parameters_file: Option<bool>,
        workflow_file: &Option<String>,
    ) -> Option<String> {
        if local_file.is_some() {
            return local_file.clone();
        }
        if use_parameters_file == Some(true) {
            return workflow_file.clone();
        }
        None
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

    fn validate_env_maps(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(env) = &self.env {
            for key in env.keys() {
                validate_env_var_name(key)?;
            }
        }

        for job in &self.jobs {
            if let Some(env) = &job.env {
                for key in env.keys() {
                    validate_env_var_name(key)
                        .map_err(|err| format!("Job '{}': {}", job.name, err))?;
                }
            }
        }

        Ok(())
    }

    /// Verify that `dynamic_jobs.max_iterations`, if set, is a positive
    /// integer. `0` would silently disable spawning for the entire workflow
    /// — the first `spawn_jobs` call would be rejected with a confusing
    /// "cap reached" 422; a negative value would always fail. Either is a
    /// spec-author footgun, so we reject up front with a clear message.
    fn validate_dynamic_jobs(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(spec) = &self.dynamic_jobs
            && let Some(n) = spec.max_iterations
            && n < 1
        {
            return Err(format!(
                "dynamic_jobs.max_iterations must be >= 1 (got {}); omit the field to use the server default",
                n
            )
            .into());
        }
        Ok(())
    }

    /// Verify that every job and every file in `self.jobs` / `self.files` has a unique
    /// name. Run this *after* [`Self::expand_parameters`] so it catches the most common
    /// authoring mistake: declaring a parameterized job or file with no `{…}` placeholder
    /// in its name (e.g. `use_parameters: [lr]` on `name: aggregate_results`), which
    /// silently expands into N records that share a name and then trample each other in
    /// the name→id maps the rest of the creation pipeline relies on. The remedy is
    /// usually to use `depends_on_regexes` / `input_file_regexes` for fan-in instead of
    /// parameterizing the consumer; see `examples/yaml/fan_in_with_regexes.yaml`.
    fn validate_unique_names_after_expansion(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut seen_jobs: HashSet<&str> = HashSet::with_capacity(self.jobs.len());
        for job in &self.jobs {
            if !seen_jobs.insert(job.name.as_str()) {
                return Err(format!(
                    "Duplicate job name '{}' after parameter expansion. A parameterized \
                     job's name template must include a placeholder for every parameter \
                     in `parameters` / `use_parameters`, otherwise expansion produces \
                     multiple jobs with identical names. For fan-in patterns (a single \
                     consumer of many parameterized producers), use `depends_on_regexes` \
                     and `input_file_regexes` instead of parameterizing the consumer.",
                    job.name
                )
                .into());
            }
        }

        if let Some(files) = &self.files {
            let mut seen_files: HashSet<&str> = HashSet::with_capacity(files.len());
            let mut seen_identifiers: HashMap<&str, &str> = HashMap::with_capacity(files.len());
            for file in files {
                if !seen_files.insert(file.name.as_str()) {
                    return Err(format!(
                        "Duplicate file name '{}' after parameter expansion. A \
                         parameterized file's name template must include a placeholder \
                         for every parameter in `parameters` / `use_parameters`.",
                        file.name
                    )
                    .into());
                }
                if let Some(identifier) = file.identifier.as_deref()
                    && let Some(prior_name) =
                        seen_identifiers.insert(identifier, file.name.as_str())
                {
                    return Err(format!(
                        "Duplicate file identifier '{}' after parameter expansion \
                         (used by files '{}' and '{}'). Identifiers must be unique \
                         within a workflow; for parameterized files, the identifier \
                         template must include a placeholder for every parameter in \
                         `parameters` / `use_parameters`.",
                        identifier, prior_name, file.name
                    )
                    .into());
                }
            }
        }

        if let Some(user_data) = &self.user_data {
            let mut seen_user_data: HashSet<&str> = HashSet::with_capacity(user_data.len());
            for ud in user_data {
                // user_data names are optional; only check duplicates when a name is set.
                let Some(name) = ud.name.as_deref() else {
                    continue;
                };
                if !seen_user_data.insert(name) {
                    return Err(format!(
                        "Duplicate user_data name '{}' after parameter expansion. A \
                         parameterized user_data's name template must include a placeholder \
                         for every parameter in `parameters` / `use_parameters`.",
                        name
                    )
                    .into());
                }
            }
        }

        Ok(())
    }

    /// Validate user-supplied RO-Crate identifiers on files.
    ///
    /// Five checks, all run together because they share the file/job traversal:
    ///
    /// 1. `identifier` only has effect when `enable_ro_crate: true`. Setting it
    ///    on a workflow that opted out of RO-Crate would silently create a single
    ///    partial entity row with no other provenance — confusing rather than
    ///    helpful, so reject.
    /// 2. `identifier` only applies to **input** files. Outputs are produced by
    ///    jobs and follow a separate entity-creation path that always rewrites
    ///    `entity_id` to the file path (`build_file_entity_with_provenance`).
    ///    Reject any file referenced as a job output (including files used as
    ///    both input and output — the output completion clobbers the identifier).
    /// 3. Identifiers must not match reserved values or prefixes used by Torc's
    ///    own provenance and synthetic export entities: `#torc-`, `#software-`,
    ///    `#job-` (CreateActions), `ro-crate-metadata.json`, and `./` (synthetic
    ///    root). All of these share the `(workflow_id, entity_id)` uniqueness
    ///    index or would produce duplicate `@id` entries in the exported graph.
    /// 4. No file's `identifier` may collide with another file's path. The
    ///    `(workflow_id, entity_id)` unique index means a user identifier equal
    ///    to another file's path would silently lose one of the two file
    ///    entities at init time. Reject up-front.
    /// 5. Run AFTER [`Self::substitute_variables`] so that `input_files` /
    ///    `output_files` populated from `${files.input.NAME}` /
    ///    `${files.output.NAME}` tokens are visible. Otherwise an output-only
    ///    file declared via a token escapes check (2). The check also accounts
    ///    for `input_file_regexes` / `output_file_regexes` so regex-matched
    ///    files aren't misclassified.
    fn validate_file_identifiers(&self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(files) = &self.files else {
            return Ok(());
        };
        if !files.iter().any(|f| f.identifier.is_some()) {
            return Ok(());
        }

        if self.enable_ro_crate != Some(true) {
            // Pick the first offender so the error names a concrete file.
            let offender = files
                .iter()
                .find(|f| f.identifier.is_some())
                .expect("at least one file has identifier");
            return Err(format!(
                "File '{}' sets `identifier` but the workflow does not have \
                 `enable_ro_crate: true`. Stable RO-Crate identifiers only have \
                 effect when automatic RO-Crate provenance is enabled; set \
                 `enable_ro_crate: true` at the workflow level or remove the \
                 identifier.",
                offender.name
            )
            .into());
        }

        // Classify each file as input/output by how jobs reference it. Both
        // exact-name lists (`input_files` / `output_files`) and regex lists
        // (`input_file_regexes` / `output_file_regexes`) contribute, because
        // `resolve_names_and_regexes` later merges them at creation time.
        let mut input_names: HashSet<&str> = HashSet::new();
        let mut output_names: HashSet<&str> = HashSet::new();
        let mut input_regexes: Vec<Regex> = Vec::new();
        let mut output_regexes: Vec<Regex> = Vec::new();
        for job in &self.jobs {
            if let Some(inputs) = &job.input_files {
                input_names.extend(inputs.iter().map(String::as_str));
            }
            if let Some(outputs) = &job.output_files {
                output_names.extend(outputs.iter().map(String::as_str));
            }
            if let Some(patterns) = &job.input_file_regexes {
                for p in patterns {
                    // Skip malformed regexes silently here; `validate_spec` /
                    // `create_jobs` surface those errors via their own checks.
                    if let Ok(re) = Regex::new(p) {
                        input_regexes.push(re);
                    }
                }
            }
            if let Some(patterns) = &job.output_file_regexes {
                for p in patterns {
                    if let Ok(re) = Regex::new(p) {
                        output_regexes.push(re);
                    }
                }
            }
        }
        let is_referenced_as = |name: &str, names: &HashSet<&str>, regexes: &[Regex]| {
            names.contains(name) || regexes.iter().any(|re| re.is_match(name))
        };

        // Build path → file-name map for the path-collision check.
        let mut path_to_name: HashMap<&str, &str> = HashMap::with_capacity(files.len());
        for file in files {
            path_to_name.insert(file.path.as_str(), file.name.as_str());
        }

        for file in files {
            let Some(identifier) = file.identifier.as_deref() else {
                continue;
            };
            let name = file.name.as_str();

            // Check 0: identifier must be a non-empty, non-whitespace string.
            // An empty or blank identifier would round-trip as `entity_id = ""`
            // and `@id = ""` in the exported graph, which is meaningless and
            // bypasses every other check below.
            if identifier.trim().is_empty() {
                return Err(format!(
                    "File '{}' has an empty or whitespace-only `identifier`. \
                     Identifiers must be non-blank strings; remove the field \
                     or set it to a stable @id value (DOI, PURL, URN, …).",
                    file.name
                )
                .into());
            }

            // Check 3: reserved IDs. The validator and the exporter share one
            // list (see `ro_crate_utils::RESERVED_ENTITY_ID_PREFIXES` /
            // `RESERVED_ENTITY_IDS`) so a new synthetic prefix added to the
            // exporter doesn't drift past this check.
            if crate::client::ro_crate_utils::is_reserved_entity_id(identifier) {
                return Err(format!(
                    "File '{}' uses identifier '{}', which matches a reserved \
                     value or prefix (`#torc-`, `#software-`, `#job-`, \
                     `ro-crate-metadata.json`, or `./`). Those are used by \
                     Torc's own provenance entities or the synthetic export \
                     root and would collide at export time.",
                    file.name, identifier
                )
                .into());
            }

            // Check 4: identifier must not equal another file's path.
            if let Some(&other_name) = path_to_name.get(identifier)
                && other_name != name
            {
                return Err(format!(
                    "File '{}' uses identifier '{}', which is also the path of \
                     file '{}'. Both would map to the same `entity_id`; \
                     workflow-wide RO-Crate entity IDs must be unique. Pick a \
                     distinct identifier.",
                    file.name, identifier, other_name
                )
                .into());
            }

            // Checks 2 + 5: dual-use rejection.
            let is_output = is_referenced_as(name, &output_names, &output_regexes);
            if is_output {
                return Err(format!(
                    "File '{}' sets `identifier` but is referenced as an output \
                     of some job. RO-Crate identifiers are only honored for \
                     input files; the output completion path always resets \
                     `entity_id` to the file path and would silently overwrite \
                     the identifier. Remove the identifier, or restructure so \
                     this file is only an input.",
                    file.name
                )
                .into());
            }

            let is_input =
                file.st_mtime.is_some() || is_referenced_as(name, &input_names, &input_regexes);
            if !is_input {
                // Not referenced anywhere — there's no input File entity for the
                // identifier to attach to. Reject so the user catches typos
                // rather than silently exporting an orphan.
                return Err(format!(
                    "File '{}' sets `identifier` but is not referenced by any \
                     job's `input_files` / `input_file_regexes` and has no \
                     pre-existing `st_mtime`. Identifiers attach to input \
                     entities, so this would create a dangling RO-Crate row. \
                     Either reference it as a job input or remove the identifier.",
                    file.name
                )
                .into());
            }
        }

        Ok(())
    }

    /// Run [`Self::validate_file_identifiers`] against a substituted copy of
    /// `self` so the caller's spec stays in its unsubstituted form.
    ///
    /// Identifier classification needs `input_files` / `output_files` populated
    /// from `${files.*.NAME}` tokens (which only happens after
    /// `substitute_variables`), but the *return* value of some validators is
    /// the unsubstituted spec — callers will substitute again later. Cloning
    /// here keeps that contract.
    ///
    /// Call sites where the spec has already been substituted in place should
    /// call [`Self::validate_file_identifiers`] directly instead.
    fn validate_file_identifiers_on_clone(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut clone = self.clone();
        clone.substitute_variables()?;
        clone.validate_file_identifiers()
    }

    /// Recognized workflow action trigger types.
    ///
    /// Kept in sync with the server's `check_and_trigger_actions` dispatch
    /// (`src/server/api/workflow_actions.rs`). An unknown trigger type silently never
    /// fires, so we reject it at creation rather than producing a confusing
    /// "0 allocations" failure at submit time.
    const VALID_TRIGGER_TYPES: &'static [&'static str] = &[
        "on_workflow_start",
        "on_workflow_complete",
        "on_worker_start",
        "on_worker_complete",
        "on_jobs_ready",
        "on_jobs_complete",
    ];

    /// Validate workflow actions
    pub fn validate_actions(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref actions) = self.actions {
            for action in actions {
                // Reject unknown trigger types (e.g. a typo'd `on_ready_jobs`), which would
                // otherwise be stored silently and never fire.
                if !Self::VALID_TRIGGER_TYPES.contains(&action.trigger_type.as_str()) {
                    return Err(format!(
                        "action has unknown trigger_type '{}'; valid trigger types are: {}",
                        action.trigger_type,
                        Self::VALID_TRIGGER_TYPES.join(", ")
                    )
                    .into());
                }

                // Job-gated triggers must name at least one job (exact names or regexes);
                // without any, the action can never become due.
                if matches!(
                    action.trigger_type.as_str(),
                    "on_jobs_ready" | "on_jobs_complete"
                ) {
                    let has_jobs = action.jobs.as_ref().is_some_and(|j| !j.is_empty());
                    let has_regexes = action
                        .job_name_regexes
                        .as_ref()
                        .is_some_and(|r| !r.is_empty());
                    if !has_jobs && !has_regexes {
                        return Err(format!(
                            "action with trigger_type '{}' must specify at least one job via \
                             'jobs' or 'job_name_regexes'",
                            action.trigger_type
                        )
                        .into());
                    }
                }

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
    /// This validation ensures that when a scheduler allocates multiple nodes (nodes > 1),
    /// jobs using it have consistent node requirements. Both patterns are valid:
    ///
    /// 1. **Single-node jobs in a multi-node allocation** — a single worker tracks
    ///    per-node resources and places each job step on a specific node via
    ///    `srun --nodelist=<node> --exact` (job `num_nodes=1` or unset).
    /// 2. **True multi-node jobs** — jobs span the full allocation (job `num_nodes` matches
    ///    scheduler `nodes`).
    ///
    /// The validation rejects the case where jobs request a different multi-node count
    /// than the scheduler provides (e.g., scheduler allocates 4 nodes but jobs request 2).
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

            // Find jobs that reference this scheduler
            let jobs_using_scheduler: Vec<&JobSpec> = self
                .jobs
                .iter()
                .filter(|job| job.scheduler.as_ref() == Some(scheduler_name))
                .collect();

            // If no jobs explicitly reference this scheduler, skip
            if jobs_using_scheduler.is_empty() {
                continue;
            }

            // Check for mismatched multi-node requirements: reject jobs that request
            // a different multi-node count than the scheduler provides.
            // Single-node jobs (num_nodes=1 or unset) are always valid in any allocation.
            let mismatched_jobs: Vec<&str> = jobs_using_scheduler
                .iter()
                .filter(|job| {
                    let job_num_nodes = job
                        .resource_requirements
                        .as_ref()
                        .and_then(|name| resource_req_map.get(name.as_str()))
                        .map(|req| req.num_nodes)
                        .unwrap_or(1);
                    // Mismatch: job wants >1 node but not the same count as scheduler
                    job_num_nodes > 1 && job_num_nodes != scheduler.nodes
                })
                .map(|j| j.name.as_str())
                .collect();

            if !mismatched_jobs.is_empty() {
                errors.push(format!(
                    "Scheduler '{}' allocates {} nodes but jobs ({}) request a different \
                     multi-node count in their resource requirements. Set num_nodes={} \
                     on job resource requirements to match the scheduler, or use \
                     num_nodes=1 for single-node jobs.",
                    scheduler_name,
                    scheduler.nodes,
                    mismatched_jobs.join(", "),
                    scheduler.nodes,
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

    /// Validate that job resource requirements (runtime, memory, GPUs) are compatible
    /// with the slurm schedulers in the workflow. Returns a list of warning messages.
    ///
    /// For jobs with an explicit scheduler, each resource dimension is checked against
    /// that scheduler. For jobs without a scheduler, at least one scheduler must be
    /// suitable across all dimensions (since any scheduler can pick up unassigned jobs).
    ///
    /// Scheduler fields that are not set (e.g., `mem: None`, `gres: None`) are skipped
    /// for that dimension.
    pub fn validate_scheduler_resources(&self) -> Vec<String> {
        let resource_req_map: HashMap<&str, &ResourceRequirementsSpec> = self
            .resource_requirements
            .as_ref()
            .map(|reqs| reqs.iter().map(|r| (r.name.as_str(), r)).collect())
            .unwrap_or_default();

        let schedulers: Vec<&SlurmSchedulerSpec> = self
            .slurm_schedulers
            .as_ref()
            .map(|s| s.iter().collect())
            .unwrap_or_default();

        if resource_req_map.is_empty() || schedulers.is_empty() {
            return Vec::new();
        }

        // Pre-parse scheduler resources into a structured form
        struct ParsedScheduler<'a> {
            name: &'a str,
            sched: &'a SlurmSchedulerSpec,
            walltime_secs: Option<u64>,
            memory_bytes: Option<i64>,
            gpu_count: Option<u32>,
        }

        let mut warnings: Vec<String> = Vec::new();

        let mut parsed_schedulers: Vec<ParsedScheduler> = Vec::new();
        for sched in &schedulers {
            let Some(name) = sched.name.as_deref() else {
                continue;
            };
            let walltime_secs =
                match crate::client::commands::slurm::parse_walltime_secs(&sched.walltime) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        warnings.push(format!(
                            "Scheduler '{}': invalid walltime '{}': {}",
                            name, sched.walltime, e,
                        ));
                        None
                    }
                };
            let memory_bytes = match sched.mem.as_ref() {
                Some(m) => match crate::memory_utils::memory_string_to_bytes(m) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        warnings.push(format!(
                            "Scheduler '{}': invalid memory '{}': {}",
                            name, m, e,
                        ));
                        None
                    }
                },
                None => None,
            };
            let gpu_count = if sched.gres.as_ref().is_some_and(|g| !g.trim().is_empty()) {
                let (parsed, _) = crate::client::hpc::slurm::parse_gres(&sched.gres);
                // Non-empty but unparseable gres → treat as 0 GPUs so validation
                // isn't silently bypassed
                Some(parsed.unwrap_or(0))
            } else {
                None
            };
            parsed_schedulers.push(ParsedScheduler {
                name,
                sched,
                walltime_secs,
                memory_bytes,
                gpu_count,
            });
        }

        if parsed_schedulers.is_empty() {
            return warnings;
        }

        for job in &self.jobs {
            let rr_name = match &job.resource_requirements {
                Some(name) => name,
                None => continue,
            };
            let rr = match resource_req_map.get(rr_name.as_str()) {
                Some(r) => r,
                None => continue,
            };

            // Parse job resource values
            let job_runtime_secs = match crate::time_utils::duration_string_to_seconds(&rr.runtime)
            {
                Ok(secs) => Some(secs as u64),
                Err(e) => {
                    warnings.push(format!(
                        "Job '{}': invalid runtime '{}': {}",
                        job.name, rr.runtime, e,
                    ));
                    None
                }
            };
            let job_memory_bytes = match crate::memory_utils::memory_string_to_bytes(&rr.memory) {
                Ok(bytes) => Some(bytes),
                Err(e) => {
                    warnings.push(format!(
                        "Job '{}': invalid memory '{}': {}",
                        job.name, rr.memory, e,
                    ));
                    None
                }
            };
            let job_gpus = rr.num_gpus;

            if job_gpus < 0 {
                warnings.push(format!(
                    "Job '{}': invalid negative num_gpus {}",
                    job.name, job_gpus,
                ));
                continue;
            }

            if let Some(ref scheduler_name) = job.scheduler {
                // Job has an explicit scheduler — check each dimension against it
                let Some(ps) = parsed_schedulers
                    .iter()
                    .find(|ps| ps.name == scheduler_name.as_str())
                else {
                    continue; // Missing scheduler validated elsewhere
                };

                if let (Some(rt), Some(wt)) = (job_runtime_secs, ps.walltime_secs)
                    && rt > wt
                {
                    warnings.push(format!(
                        "Job '{}': runtime '{}' ({} s) exceeds scheduler '{}' \
                         walltime '{}' ({} s)",
                        job.name, rr.runtime, rt, scheduler_name, ps.sched.walltime, wt,
                    ));
                }
                if let (Some(jm), Some(sm)) = (job_memory_bytes, ps.memory_bytes)
                    && jm > sm
                {
                    warnings.push(format!(
                        "Job '{}': memory '{}' exceeds scheduler '{}' mem '{}'",
                        job.name,
                        rr.memory,
                        scheduler_name,
                        ps.sched.mem.as_deref().unwrap_or("?"),
                    ));
                }
                if let Some(sg) = ps.gpu_count
                    && job_gpus > sg as i64
                {
                    warnings.push(format!(
                        "Job '{}': num_gpus {} exceeds scheduler '{}' gres '{}'",
                        job.name,
                        job_gpus,
                        scheduler_name,
                        ps.sched.gres.as_deref().unwrap_or("?"),
                    ));
                }
            } else {
                // Job has no explicit scheduler — at least one must be suitable
                // across ALL dimensions simultaneously
                let suitable = parsed_schedulers.iter().any(|ps| {
                    let runtime_ok = match (job_runtime_secs, ps.walltime_secs) {
                        (Some(rt), Some(wt)) => rt <= wt,
                        _ => true, // Can't check, assume ok
                    };
                    let memory_ok = match (job_memory_bytes, ps.memory_bytes) {
                        (Some(jm), Some(sm)) => jm <= sm,
                        _ => true,
                    };
                    let gpu_ok = match ps.gpu_count {
                        Some(sg) => job_gpus <= sg as i64,
                        None => true,
                    };
                    runtime_ok && memory_ok && gpu_ok
                });

                if !suitable {
                    let mut reasons: Vec<String> = Vec::new();
                    for ps in &parsed_schedulers {
                        let mut mismatches: Vec<String> = Vec::new();
                        if let (Some(rt), Some(wt)) = (job_runtime_secs, ps.walltime_secs)
                            && rt > wt
                        {
                            mismatches.push(format!("runtime {} s > walltime {} s", rt, wt));
                        }
                        if let (Some(jm), Some(sm)) = (job_memory_bytes, ps.memory_bytes)
                            && jm > sm
                        {
                            mismatches.push(format!(
                                "memory '{}' > mem '{}'",
                                rr.memory,
                                ps.sched.mem.as_deref().unwrap_or("?"),
                            ));
                        }
                        if let Some(sg) = ps.gpu_count
                            && job_gpus > sg as i64
                        {
                            mismatches.push(format!(
                                "num_gpus {} > gres '{}'",
                                job_gpus,
                                ps.sched.gres.as_deref().unwrap_or("?"),
                            ));
                        }
                        if !mismatches.is_empty() {
                            reasons.push(format!("'{}': {}", ps.name, mismatches.join(", ")));
                        }
                    }
                    warnings.push(format!(
                        "Job '{}' has no explicit scheduler and no scheduler can \
                         accommodate its resource requirements. Mismatches: [{}]",
                        job.name,
                        reasons.join("; "),
                    ));
                }
            }
        }

        warnings
    }

    /// Validate a spec file for creation by non-interactive callers (MCP server, TUI).
    ///
    /// Runs parameter expansion, duplicate-name/identifier checks, env-map checks,
    /// file-identifier checks (which internally substitute variables on a clone),
    /// scheduler node requirement checks, and scheduler resource checks. Returns
    /// the parsed spec **unsubstituted** on success so callers can pass it to
    /// [`create_from_validated_spec`] without re-reading the file.
    ///
    /// **Note:** Action validation is still performed later by
    /// [`create_from_validated_spec`], so a spec that passes here can still fail
    /// during creation if it has invalid actions. Variable substitution errors,
    /// in contrast, surface here because the file-identifier check substitutes on
    /// a clone — the returned spec stays in its unsubstituted form.
    pub fn validate_for_creation<P: AsRef<Path>>(
        path: P,
    ) -> Result<WorkflowSpec, Box<dyn std::error::Error>> {
        let mut spec = Self::from_spec_file(path)?;
        spec.expand_parameters()?;
        spec.validate_unique_names_after_expansion()?;
        spec.validate_env_maps()?;
        spec.validate_dynamic_jobs()?;
        // The returned spec must stay unsubstituted -- `create_from_validated_spec`
        // substitutes again -- so this helper does the substitute-then-validate
        // dance on a clone.
        spec.validate_file_identifiers_on_clone()?;

        spec.validate_scheduler_node_requirements()?;

        let resource_warnings = spec.validate_scheduler_resources();
        if !resource_warnings.is_empty() {
            return Err(format!(
                "Resource validation failed:\n  - {}",
                resource_warnings.join("\n  - ")
            )
            .into());
        }
        Ok(spec)
    }

    /// Pre-validate a spec file for interactive CLI callers.
    /// Node requirement failures are hard errors. Resource mismatches prompt the user.
    /// Exits the process if validation fails or the user declines.
    /// Note: CLI-only. Calls `process::exit` on failure — use `validate_for_creation`
    /// for non-interactive/library contexts.
    pub fn prevalidate_or_exit<P: AsRef<Path>>(path: P) {
        let mut spec = match Self::from_spec_file(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error reading spec: {}", e);
                std::process::exit(1);
            }
        };
        if let Err(e) = spec.expand_parameters() {
            eprintln!("Error expanding parameters: {}", e);
            std::process::exit(1);
        }
        if let Err(e) = spec.validate_unique_names_after_expansion() {
            eprintln!("Validation error: {}", e);
            std::process::exit(1);
        }
        if let Err(e) = spec.validate_env_maps() {
            eprintln!("Validation error: {}", e);
            std::process::exit(1);
        }
        if let Err(e) = spec.validate_dynamic_jobs() {
            eprintln!("Validation error: {}", e);
            std::process::exit(1);
        }
        // Identifier classification needs input/output_files populated from
        // ${files.*.NAME} tokens; this helper substitutes on a clone so this
        // remains pre-validation only.
        if let Err(e) = spec.validate_file_identifiers_on_clone() {
            eprintln!("Validation error: {}", e);
            std::process::exit(1);
        }

        // Node requirements are hard errors (no prompt)
        if let Err(e) = spec.validate_scheduler_node_requirements() {
            eprintln!("Validation error: {}", e);
            std::process::exit(1);
        }

        // Resource checks are interactive warnings
        let warnings = spec.validate_scheduler_resources();
        if !warnings.is_empty() && !Self::prompt_scheduler_warnings(&warnings) {
            std::process::exit(1);
        }
    }

    /// Display scheduler resource warnings and prompt the user for confirmation.
    /// Returns true if the user confirms (or if there are no warnings).
    /// In non-interactive contexts (stdin is not a TTY), prints a message and returns false.
    fn prompt_scheduler_warnings(warnings: &[String]) -> bool {
        use std::io::{IsTerminal, Write};

        if warnings.is_empty() {
            return true;
        }

        eprintln!("Resource validation warnings:");
        for w in warnings {
            eprintln!("  - {}", w);
        }
        eprintln!();

        if std::io::stdin().is_terminal() {
            eprint!("Proceed anyway? [y/N] ");
            if std::io::stderr().flush().is_err() {
                return false;
            }
            let mut input = String::new();
            if std::io::stdin().read_line(&mut input).is_ok() {
                return input.trim().eq_ignore_ascii_case("y");
            }
            false
        } else {
            eprintln!("Use --skip-checks to bypass resource validation.");
            false
        }
    }

    /// Check if the workflow spec has a `schedule_nodes` action that `torc submit` can act on.
    ///
    /// `submit` fires every pending Slurm `schedule_nodes` action regardless of trigger type
    /// (on_workflow_start to bootstrap, on_jobs_ready/on_jobs_complete for job-gated scheduling and
    /// re-runs), so any of those qualifies a spec as submittable. Returns false otherwise.
    pub fn has_schedule_nodes_action(&self) -> bool {
        if let Some(ref actions) = self.actions {
            actions.iter().any(|action| {
                action.action_type == "schedule_nodes"
                    && matches!(
                        action.trigger_type.as_str(),
                        "on_workflow_start" | "on_jobs_ready" | "on_jobs_complete"
                    )
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
        let warnings = Vec::new();

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

        if let Err(e) = spec.validate_env_maps() {
            errors.push(format!("Environment validation failed: {}", e));
        }

        if let Err(e) = spec.validate_dynamic_jobs() {
            errors.push(format!("Validation error: {}", e));
        }

        // Check duplicates of names and identifiers after expansion. This was
        // missing from the dry-run path -- without it, a spec with two files
        // sharing an expanded identifier would pass `validate` and only fail
        // later in `create_files`.
        if let Err(e) = spec.validate_unique_names_after_expansion() {
            errors.push(format!("Validation error: {}", e));
        }

        // Step 4: Validate scheduler node requirements
        if let Err(e) = spec.validate_scheduler_node_requirements() {
            errors.push(format!("{}", e));
        }

        // Step 4.5: Validate scheduler resources (runtime, memory, GPUs)
        let resource_warnings = spec.validate_scheduler_resources();
        errors.extend(resource_warnings);

        // Step 5: Validate variable substitution
        if let Err(e) = spec.substitute_variables() {
            errors.push(format!("Variable substitution failed: {}", e));
        }

        // Identifier validation MUST run after substitute_variables: jobs that
        // declare files via `${files.output.NAME}` only have `output_files`
        // populated post-substitution, and we'd otherwise miss output-only
        // files that should reject `identifier`.
        if let Err(e) = spec.validate_file_identifiers() {
            errors.push(format!("File identifier validation failed: {}", e));
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
    /// **Note:** This function does not run scheduler resource validation
    /// (node requirements, memory/runtime limits). The CLI performs those checks
    /// interactively before calling this. Non-interactive callers (MCP, TUI)
    /// should use [`validate_for_creation`] followed by [`create_from_validated_spec`].
    ///
    /// # Arguments
    /// * `config` - Server configuration
    /// * `path` - Path to the workflow specification file
    /// * `user` - User that owns the workflow
    /// * `enable_resource_monitoring` - Whether to enable resource monitoring by default
    pub fn create_workflow_from_spec<P: AsRef<Path>>(
        config: &Configuration,
        path: P,
        user: &str,
        enable_resource_monitoring: bool,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        let mut spec = Self::from_spec_file(path)?;
        Self::prepare_spec_for_creation(&mut spec, user, enable_resource_monitoring)?;
        Self::create_from_prepared_spec(config, spec)
    }

    /// Create a workflow from a pre-parsed and validated spec.
    /// Use this after `validate_for_creation` to avoid re-reading the file.
    pub fn create_from_validated_spec(
        config: &Configuration,
        mut spec: WorkflowSpec,
        user: &str,
        enable_resource_monitoring: bool,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        // validate_for_creation already expanded parameters, but we still need
        // the remaining preparation steps (user, monitoring, actions, variables).
        spec.user = Some(user.to_string());
        if enable_resource_monitoring && spec.resource_monitor.is_none() {
            spec.resource_monitor = Some(crate::client::resource_monitor::ResourceMonitorConfig {
                enabled: true,
                granularity: crate::client::resource_monitor::MonitorGranularity::Summary,
                sample_interval_seconds: 10,
                generate_plots: false,
                jobs: Some(crate::client::resource_monitor::JobMonitorConfig {
                    enabled: true,
                    granularity: crate::client::resource_monitor::MonitorGranularity::Summary,
                }),
                compute_node: None,
                ..crate::client::resource_monitor::ResourceMonitorConfig::default()
            });
        }
        spec.validate_env_maps()?;
        spec.validate_actions()?;
        spec.substitute_variables()?;
        // Identifier validation runs AFTER substitution so jobs that declare
        // files via ${files.*.NAME} have their input_files/output_files filled
        // in -- otherwise the input/output classification is wrong.
        spec.validate_file_identifiers()?;
        Self::create_from_prepared_spec(config, spec)
    }

    /// Prepare a spec for creation: set user, expand parameters, validate, substitute.
    fn prepare_spec_for_creation(
        spec: &mut WorkflowSpec,
        user: &str,
        enable_resource_monitoring: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        spec.user = Some(user.to_string());
        if enable_resource_monitoring && spec.resource_monitor.is_none() {
            spec.resource_monitor = Some(crate::client::resource_monitor::ResourceMonitorConfig {
                enabled: true,
                granularity: crate::client::resource_monitor::MonitorGranularity::Summary,
                sample_interval_seconds: 10,
                generate_plots: false,
                jobs: Some(crate::client::resource_monitor::JobMonitorConfig {
                    enabled: true,
                    granularity: crate::client::resource_monitor::MonitorGranularity::Summary,
                }),
                compute_node: None,
                ..crate::client::resource_monitor::ResourceMonitorConfig::default()
            });
        }
        spec.expand_parameters()?;
        spec.validate_unique_names_after_expansion()?;
        spec.validate_env_maps()?;
        spec.validate_actions()?;
        spec.substitute_variables()?;
        // After substitution: input_files / output_files populated from
        // ${files.*.NAME} tokens are visible to the identifier classification.
        spec.validate_file_identifiers()?;
        Ok(())
    }

    /// Create a workflow from a spec that has already been prepared (user set,
    /// parameters expanded, actions validated, variables substituted).
    fn create_from_prepared_spec(
        config: &Configuration,
        mut spec: WorkflowSpec,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        // Step 1.6: Collect per-job stdio overrides into execution_config
        {
            let overrides: HashMap<String, StdioConfig> = spec
                .jobs
                .iter()
                .filter_map(|job| {
                    job.stdio
                        .as_ref()
                        .map(|stdio| (job.name.clone(), stdio.clone()))
                })
                .collect();
            if !overrides.is_empty() {
                let ec = spec
                    .execution_config
                    .get_or_insert_with(ExecutionConfig::default);
                ec.job_stdio_overrides = Some(overrides);
            }
        }
        // Step 2: Create WorkflowModel
        let workflow_id = Self::create_workflow(config, &spec)?;

        // If any step fails, delete the workflow (which cascades to all other objects)
        let rollback = |workflow_id: i64| {
            let _ = apis::workflows_api::delete_workflow(config, workflow_id);
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
        workflow_model.env = spec.env.clone().filter(|env| !env.is_empty());

        // Set compute node configuration fields if present
        if spec.compute_node_expiration_buffer_seconds.is_some() {
            log::warn!(
                "compute_node_expiration_buffer_seconds is deprecated and will be ignored. \
                 Slurm manages job termination signals via srun --time."
            );
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
        // Serialize resource_monitor config if present
        if let Some(ref resource_monitor) = spec.resource_monitor {
            workflow_model.resource_monitor_config = Some(resource_monitor.clone());
        }

        // Validate and serialize slurm_defaults if present
        if let Some(ref slurm_defaults) = spec.slurm_defaults {
            // Validate that no excluded parameters are present
            slurm_defaults.validate()?;
            workflow_model.slurm_defaults = Some(slurm_defaults.0.clone());
        }

        // dynamic_jobs is the same struct on both sides — copy through.
        workflow_model.dynamic_jobs = spec.dynamic_jobs.clone();

        // Set use_pending_failed if present
        if let Some(value) = spec.use_pending_failed {
            workflow_model.use_pending_failed = Some(value);
        }

        // Store execution_config if any non-default settings are configured
        if let Some(ref execution_config) = spec.execution_config
            && *execution_config != ExecutionConfig::default()
        {
            workflow_model.execution_config = Some(execution_config.clone());
        }

        // Validate that execution_config fields match the effective mode.
        // For mode=auto, infer from slurm_schedulers presence.
        if let Some(ref ec) = spec.execution_config {
            let will_use_slurm = match ec.mode {
                ExecutionMode::Slurm => true,
                ExecutionMode::Auto => spec
                    .slurm_schedulers
                    .as_ref()
                    .is_some_and(|s| !s.is_empty()),
                ExecutionMode::Direct => false,
            };
            let will_use_direct = match ec.mode {
                ExecutionMode::Direct => true,
                ExecutionMode::Auto => !will_use_slurm,
                ExecutionMode::Slurm => false,
            };

            let mut errors = Vec::new();
            let has_worker_per_node_schedule_action =
                spec.actions.as_ref().is_some_and(|actions| {
                    actions.iter().any(|action| {
                        action.action_type == "schedule_nodes"
                            && action.start_one_worker_per_node == Some(true)
                    })
                });

            if let Some(value) = ec.srun_mpi.as_deref() {
                if let Err(err) = validate_srun_mpi_value(value) {
                    errors.push(err);
                }
                if !has_worker_per_node_schedule_action {
                    errors.push(
                        "srun_mpi requires schedule_nodes.start_one_worker_per_node = true. \
                        It only applies to the outer srun that launches one job runner per node."
                            .to_string(),
                    );
                }
            }

            if will_use_slurm {
                if ec.limit_resources == Some(false) {
                    errors.push(
                        "limit_resources: false is only supported in direct mode. \
                        Slurm mode requires resource limits for correct srun behavior."
                            .to_string(),
                    );
                }
                if ec.termination_signal.is_some() {
                    errors.push(
                        "termination_signal is only supported in direct mode. \
                        In slurm mode, use srun_termination_signal instead."
                            .to_string(),
                    );
                }
                if ec.sigterm_lead_seconds.is_some() {
                    errors.push(
                        "sigterm_lead_seconds is only supported in direct mode. \
                        In slurm mode, termination timing is controlled by \
                        srun_termination_signal."
                            .to_string(),
                    );
                }
                if ec.oom_exit_code.is_some() {
                    errors.push(
                        "oom_exit_code is only supported in direct mode. \
                        In slurm mode, Slurm manages OOM detection."
                            .to_string(),
                    );
                }
            }

            if will_use_direct {
                if ec.srun_termination_signal.is_some() {
                    errors.push(
                        "srun_termination_signal is only supported in slurm mode. \
                        In direct mode, use termination_signal instead."
                            .to_string(),
                    );
                }
                if ec.enable_cpu_bind == Some(true) {
                    errors.push(
                        "enable_cpu_bind is only supported in slurm mode. \
                        It has no effect in direct mode."
                            .to_string(),
                    );
                }
            }

            if !errors.is_empty() {
                return Err(errors.join(" ").into());
            }
        }

        if spec.actions.as_ref().is_some_and(|actions| {
            actions.iter().any(|action| {
                action.action_type == "schedule_nodes"
                    && action.start_one_worker_per_node == Some(true)
            })
        }) {
            let mode = spec
                .execution_config
                .as_ref()
                .map(|config| &config.mode)
                .unwrap_or(&ExecutionMode::Direct);
            if *mode != ExecutionMode::Direct {
                return Err(
                    "start_one_worker_per_node requires execution_config.mode to be 'direct'"
                        .into(),
                );
            }
        }

        // Set enable_ro_crate if present
        if let Some(value) = spec.enable_ro_crate {
            workflow_model.enable_ro_crate = Some(value);
        }

        // Set project if present
        if let Some(ref value) = spec.project {
            workflow_model.project = Some(value.clone());
        }

        // Set metadata if present
        if let Some(ref value) = spec.metadata {
            workflow_model.metadata = Some(value.clone());
        }

        // Pass through declared access groups; the server resolves names to
        // group IDs in the same transaction as the workflow create.
        if let Some(ref groups) = spec.access_groups {
            workflow_model.access_groups = Some(groups.clone());
        }

        // Record where the workflow was submitted from so jobs can resolve
        // relative paths via TORC_WORKFLOW_SUBMISSION_DIR even when run on a
        // compute node with a different CWD.
        workflow_model.submission_directory = crate::client::utils::capture_submission_directory();

        let created_workflow = apis::workflows_api::create_workflow(config, workflow_model)
            .map_err(|e| format!("Failed to create workflow: {:?}", e))?;

        created_workflow
            .id
            .ok_or("Created workflow missing ID".into())
    }

    /// Create FileModels and build name-to-id mapping.
    ///
    /// Files are created via the bulk `create_files` endpoint in batches of
    /// `MAX_RECORD_TRANSFER_COUNT`, so a workflow with thousands of files
    /// reaches the server in a handful of requests instead of one per file.
    ///
    /// Only files that are referenced as an input by at least one job get a
    /// `std::fs::metadata` call to populate `st_mtime`. The server uses
    /// `st_mtime IS NOT NULL` as the marker that distinguishes inputs from
    /// outputs (see `ro_crate.rs::WHERE st_mtime IS NOT NULL`), and stat'ing
    /// the 10k+ outputs of a large parameter sweep is both wasted work and
    /// risks misclassifying them as inputs if they happen to exist on disk.
    fn create_files(
        config: &Configuration,
        workflow_id: i64,
        spec: &WorkflowSpec,
    ) -> Result<HashMap<String, i64>, Box<dyn std::error::Error>> {
        let mut file_name_to_id = HashMap::new();

        let Some(files) = &spec.files else {
            return Ok(file_name_to_id);
        };

        // Collect the set of file names referenced as inputs by any job. `substitute_variables`
        // has already populated `job.input_files` from `${files.input.NAME}` tokens before
        // we reach this point, so this captures both explicitly declared inputs and the
        // implicit inputs extracted from job commands.
        let input_file_names: HashSet<&str> = spec
            .jobs
            .iter()
            .flat_map(|job| job.input_files.iter().flatten().map(String::as_str))
            .collect();

        // Also collect compiled `input_file_regexes` so files whose names are
        // matched only by a regex (rather than an exact `input_files` entry)
        // still get classified as inputs -- both for the st_mtime stat and for
        // the identifier pre-create below. Malformed regexes are surfaced by
        // `resolve_names_and_regexes` later; tolerate them here so we don't
        // double-report.
        let input_file_regexes: Vec<Regex> = spec
            .jobs
            .iter()
            .flat_map(|job| job.input_file_regexes.iter().flatten())
            .filter_map(|p| Regex::new(p).ok())
            .collect();
        let is_input_name = |name: &str| {
            input_file_names.contains(name) || input_file_regexes.iter().any(|re| re.is_match(name))
        };

        // Build the full list of FileModels up front. file_name_to_id holds sentinel
        // zeros for every requested name; each batch's response replaces them with the
        // server-assigned ids. Looking the id up by the *returned* model's `name` field
        // (rather than positional iteration) means we do not depend on the server
        // preserving request order — SQLite's `RETURNING` order is officially undefined.
        let mut file_models = Vec::with_capacity(files.len());
        for file_spec in files {
            if !file_name_to_id.contains_key(&file_spec.name) {
                // Sentinel until the server assigns the real ID below.
                file_name_to_id.insert(file_spec.name.clone(), 0);
            } else {
                return Err(format!("Duplicate file name: {}", file_spec.name).into());
            }

            // Use the spec-provided value when given; otherwise stat the path only for
            // files used as inputs (exact name OR regex match). Output-only files
            // stay `None`, which is the marker the server uses to identify outputs.
            let st_mtime = match file_spec.st_mtime {
                Some(t) => Some(t),
                None if is_input_name(file_spec.name.as_str()) => {
                    std::fs::metadata(&file_spec.path)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs_f64())
                }
                None => None,
            };

            file_models.push(models::FileModel {
                id: None,
                workflow_id,
                name: file_spec.name.clone(),
                path: file_spec.path.clone(),
                st_mtime,
            });
        }

        let batch_size = crate::MAX_RECORD_TRANSFER_COUNT as usize;
        for batch in file_models.chunks(batch_size) {
            let body = models::FilesModel::new(batch.to_vec());
            let response = apis::files_api::create_files(config, body)
                .map_err(|e| format!("Failed to bulk-create files: {:?}", e))?;
            let created = response
                .files
                .ok_or("create_files response missing files array")?;
            if created.len() != batch.len() {
                return Err(format!(
                    "create_files returned {} files, expected {}",
                    created.len(),
                    batch.len()
                )
                .into());
            }
            for created_file in created {
                let file_id = created_file.id.ok_or("Created file missing ID")?;
                // The server echoes back the same name we sent. Look up the entry by
                // that name — order-independent.
                match file_name_to_id.get_mut(&created_file.name) {
                    Some(slot) => *slot = file_id,
                    None => {
                        return Err(format!(
                            "create_files returned unknown file name '{}'",
                            created_file.name
                        )
                        .into());
                    }
                }
            }
        }

        // Final guard: every name we inserted as a sentinel `0` above must have been
        // replaced by a positive server-assigned id. Surface a hard error rather than
        // letting a stale `0` substitute as a job's input_file_id downstream.
        if let Some((name, _)) = file_name_to_id.iter().find(|&(_, &id)| id == 0) {
            return Err(format!(
                "create_files did not return an id for file '{}'; the server response \
                 omitted this name (possible API contract violation)",
                name
            )
            .into());
        }

        // Pre-create RO-Crate entities for input files that carry a user-supplied
        // identifier. Persisting the identifier into the `entity_id` column here
        // is what makes it survive the server's init-time upsert -- see
        // `create_input_file_entity_with_identifier` for the round-trip details.
        //
        // Gate on "declared as a job input OR has st_mtime" rather than just
        // `st_mtime.is_some()`. An input file that doesn't yet exist on disk has
        // st_mtime=None here; if we skipped it, the identifier would be silently
        // dropped because `initialize_files` later refreshes st_mtime but the
        // FileSpec (and its identifier) is no longer in scope at that point. The
        // server-side filter on st_mtime still applies for *whether* init touches
        // the entity at all -- but the pre-created row carries the identifier
        // regardless of disk state.
        for (file_spec, file_model) in files.iter().zip(file_models.iter()) {
            let Some(identifier) = file_spec.identifier.as_deref() else {
                continue;
            };
            let is_input = file_model.st_mtime.is_some() || is_input_name(file_spec.name.as_str());
            if !is_input {
                continue;
            }
            let file_id = *file_name_to_id
                .get(&file_spec.name)
                .ok_or_else(|| format!("missing file id for '{}'", file_spec.name))?;
            let file_with_id = models::FileModel {
                id: Some(file_id),
                ..file_model.clone()
            };
            crate::client::ro_crate_utils::create_input_file_entity_with_identifier(
                config,
                workflow_id,
                &file_with_id,
                identifier,
            )?;
        }

        Ok(file_name_to_id)
    }

    /// Create UserDataModels and build name-to-id mapping.
    ///
    /// Like [`create_files`], records are pushed in bulk batches keyed off
    /// `MAX_RECORD_TRANSFER_COUNT` so large user_data lists don't translate to
    /// one HTTP round trip per entry.
    fn create_user_data(
        config: &Configuration,
        workflow_id: i64,
        spec: &WorkflowSpec,
    ) -> Result<HashMap<String, i64>, Box<dyn std::error::Error>> {
        let mut user_data_name_to_id = HashMap::new();

        let Some(user_data_list) = &spec.user_data else {
            return Ok(user_data_name_to_id);
        };

        // `user_data_name_to_id` holds sentinel zeros for every requested name; each
        // batch's response replaces them with the server-assigned ids. Looking the id
        // up by the *returned* entry's `name` field (rather than positional iteration)
        // means we do not depend on the server preserving request order — SQLite's
        // `RETURNING` order is officially undefined.
        let mut user_data_models = Vec::new();
        for user_data_spec in user_data_list {
            // Spec entries without a name are not addressable by jobs and are skipped here,
            // matching the legacy per-record path.
            let Some(name) = &user_data_spec.name else {
                continue;
            };
            if user_data_name_to_id.contains_key(name) {
                return Err(format!("Duplicate user data name: {}", name).into());
            }
            user_data_name_to_id.insert(name.clone(), 0);
            user_data_models.push(models::UserDataModel {
                id: None,
                workflow_id,
                is_ephemeral: user_data_spec.is_ephemeral,
                name: name.clone(),
                data: user_data_spec.data.clone(),
            });
        }

        let batch_size = crate::MAX_RECORD_TRANSFER_COUNT as usize;
        for batch in user_data_models.chunks(batch_size) {
            let body = models::UserDataListModel::new(batch.to_vec());
            let response = apis::user_data_api::create_user_data_list(config, body)
                .map_err(|e| format!("Failed to bulk-create user_data: {:?}", e))?;
            let created = response
                .user_data
                .ok_or("create_user_data_list response missing user_data array")?;
            if created.len() != batch.len() {
                return Err(format!(
                    "create_user_data_list returned {} records, expected {}",
                    created.len(),
                    batch.len()
                )
                .into());
            }
            for created_entry in created {
                let user_data_id = created_entry.id.ok_or("Created user data missing ID")?;
                // The server echoes back the same name we sent. Look up by that name
                // — order-independent.
                match user_data_name_to_id.get_mut(&created_entry.name) {
                    Some(slot) => *slot = user_data_id,
                    None => {
                        return Err(format!(
                            "create_user_data_list returned unknown user_data name '{}'",
                            created_entry.name
                        )
                        .into());
                    }
                }
            }
        }

        // Final guard: every name we inserted as a sentinel `0` above must have been
        // replaced by a positive server-assigned id. Surface a hard error rather than
        // letting a stale `0` substitute as a job's input_user_data_id downstream.
        if let Some((name, _)) = user_data_name_to_id.iter().find(|&(_, &id)| id == 0) {
            return Err(format!(
                "create_user_data_list did not return an id for user_data '{}'; the \
                 server response omitted this name (possible API contract violation)",
                name
            )
            .into());
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
                    apis::resource_requirements_api::create_resource_requirements(
                        config,
                        resource_req_model,
                    )
                    .map_err(|e| {
                        format!(
                            "Failed to create resource requirements {}: {:?}",
                            resource_req_spec.name, e
                        )
                    })?;

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
                        serialize_allocations: scheduler_spec.serialize_allocations,
                    };

                    let created_scheduler =
                        apis::slurm_schedulers_api::create_slurm_scheduler(config, scheduler_model)
                            .map_err(|e| {
                                format!("Failed to create slurm scheduler {}: {:?}", name, e)
                            })?;

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

                let created_handler =
                    apis::failure_handlers_api::create_failure_handler(config, handler_model)
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
                let action_body = models::WorkflowActionModel {
                    id: None,
                    workflow_id,
                    trigger_type: action_spec.trigger_type.clone(),
                    action_type: action_spec.action_type.clone(),
                    action_config,
                    job_ids,
                    trigger_count: 0,
                    required_triggers: 1,
                    executed: false,
                    executed_at: None,
                    executed_by: None,
                    persistent: action_spec.persistent.unwrap_or(false),
                    is_recovery: false,
                };

                apis::workflow_actions_api::create_workflow_action(
                    config,
                    workflow_id,
                    action_body,
                )
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

    /// Create JobModels with proper ID mapping using bulk API in batches
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
        let batch_size = crate::MAX_RECORD_TRANSFER_COUNT as usize;

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
                job_model.env = job_spec.env.clone().filter(|env| !env.is_empty());
                // Only override cancel_on_blocking_job_failure if explicitly set in spec
                // (JobModel::new() defaults to Some(true))
                if job_spec.cancel_on_blocking_job_failure.is_some() {
                    job_model.cancel_on_blocking_job_failure =
                        job_spec.cancel_on_blocking_job_failure;
                }
                // supports_termination is deprecated — Slurm manages termination
                // signals via srun --time and KillWait. Accept the field silently
                // to avoid breaking existing specs.

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

                if let Some(p) = job_spec.priority {
                    if p < 0 {
                        return Err(format!(
                            "priority must be >= 0, got {} for job '{}'",
                            p, job_spec.name
                        )
                        .into());
                    }
                    job_model.priority = Some(p);
                }

                job_models.push(job_model);
                job_spec_mapping.push(job_spec);
            }

            // Create this level's jobs in batches
            for (batch_index, batch) in job_models.chunks(batch_size).enumerate() {
                let jobs_model = models::JobsModel::new(batch.to_vec());

                let response = apis::jobs_api::create_jobs(config, jobs_model).map_err(|e| {
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
                let batch_start = batch_index * batch_size;
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

    /// Convert a KDL string map block to a JSON object
    #[cfg(feature = "client")]
    fn kdl_string_map_to_json(
        node: &KdlNode,
        label: &str,
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
                .ok_or_else(|| format!("{} '{}' must have a string value", label, param_name))?
                .to_string();
            params.insert(param_name, serde_json::Value::String(param_value));
        }

        if params.is_empty() {
            Ok(None)
        } else {
            Ok(Some(serde_json::Value::Object(params)))
        }
    }

    /// Convert a KDL parameters block to a JSON object
    #[cfg(feature = "client")]
    fn kdl_parameters_to_json(
        node: &KdlNode,
    ) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
        Self::kdl_string_map_to_json(node, "Parameter")
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
                    "env" => {
                        if let Some(env) = Self::kdl_string_map_to_json(child, "Environment key")? {
                            obj.insert("env".to_string(), env);
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
                    "stdio" => {
                        let stdio_obj = Self::kdl_stdio_config_to_json(child)?;
                        obj.insert("stdio".to_string(), stdio_obj);
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

        // identifier can also be specified as a property on the same line.
        if let Some(identifier) = node.get("identifier").and_then(|e| e.as_string()) {
            obj.insert(
                "identifier".to_string(),
                serde_json::Value::String(identifier.to_string()),
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
                    "identifier" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_string())
                        {
                            obj.insert(
                                "identifier".to_string(),
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
                    "serialize_allocations" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_bool()) {
                            obj.insert(
                                "serialize_allocations".to_string(),
                                serde_json::Value::Bool(v),
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
                    "flush_interval_seconds" => {
                        if let Some(v) =
                            child.entries().first().and_then(|e| e.value().as_integer())
                        {
                            obj.insert(
                                "flush_interval_seconds".to_string(),
                                serde_json::Value::Number(serde_json::Number::from(v as i64)),
                            );
                        }
                    }
                    "generate_plots" => {
                        if let Some(v) = child.entries().first().and_then(|e| e.value().as_bool()) {
                            obj.insert("generate_plots".to_string(), serde_json::Value::Bool(v));
                        }
                    }
                    "jobs" | "compute_node" => {
                        let mut nested_obj = serde_json::Map::new();
                        if let Some(nested_children) = child.children() {
                            for nested_child in nested_children.nodes() {
                                let key = nested_child.name().value();
                                if let Some(entry) = nested_child.entries().first() {
                                    let value = entry.value();
                                    match key {
                                        "enabled" | "cpu" | "memory" => {
                                            if let Some(v) = value.as_bool() {
                                                nested_obj.insert(
                                                    key.to_string(),
                                                    serde_json::Value::Bool(v),
                                                );
                                            }
                                        }
                                        "granularity" => {
                                            if let Some(v) = value.as_string() {
                                                nested_obj.insert(
                                                    key.to_string(),
                                                    serde_json::Value::String(v.to_string()),
                                                );
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        obj.insert(
                            child.name().value().to_string(),
                            serde_json::Value::Object(nested_obj),
                        );
                    }
                    _ => {}
                }
            }
        }

        Ok(serde_json::Value::Object(obj))
    }

    /// Convert a KDL execution_config node to a JSON object
    ///
    /// Parses execution_config block with mode and various settings for job execution.
    #[cfg(feature = "client")]
    fn kdl_execution_config_to_json(
        node: &KdlNode,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let mut obj = serde_json::Map::new();

        if let Some(children) = node.children() {
            for child in children.nodes() {
                let key = child.name().value();
                // Handle child blocks (no entry value, only children)
                if key == "stdio" {
                    let stdio_obj = Self::kdl_stdio_config_to_json(child)?;
                    obj.insert("stdio".to_string(), stdio_obj);
                    continue;
                }
                if let Some(entry) = child.entries().first() {
                    let value = entry.value();
                    match key {
                        "mode" => {
                            if let Some(s) = value.as_string() {
                                obj.insert(
                                    "mode".to_string(),
                                    serde_json::Value::String(s.to_string()),
                                );
                            }
                        }
                        "limit_resources" | "enable_cpu_bind" => {
                            if let Some(b) = value.as_bool() {
                                obj.insert(key.to_string(), serde_json::Value::Bool(b));
                            }
                        }
                        "termination_signal" | "srun_termination_signal" | "srun_mpi" => {
                            if let Some(s) = value.as_string() {
                                obj.insert(
                                    key.to_string(),
                                    serde_json::Value::String(s.to_string()),
                                );
                            }
                        }
                        "sigterm_lead_seconds" | "sigkill_headroom_seconds" => {
                            if let Some(i) = value.as_integer() {
                                obj.insert(
                                    key.to_string(),
                                    serde_json::Value::Number(serde_json::Number::from(i as i64)),
                                );
                            }
                        }
                        "timeout_exit_code" | "oom_exit_code" => {
                            if let Some(i) = value.as_integer() {
                                obj.insert(
                                    key.to_string(),
                                    serde_json::Value::Number(serde_json::Number::from(i as i64)),
                                );
                            }
                        }
                        _ => {
                            log::warn!("Unknown execution_config field '{}' will be ignored", key);
                        }
                    }
                }
            }
        }

        Ok(serde_json::Value::Object(obj))
    }

    /// Convert a KDL stdio config node to a JSON object.
    ///
    /// Handles blocks like:
    /// ```kdl
    /// stdio {
    ///     mode "combined"
    ///     delete_on_success #true
    /// }
    /// ```
    #[cfg(feature = "client")]
    fn kdl_stdio_config_to_json(
        node: &KdlNode,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let mut obj = serde_json::Map::new();

        if let Some(children) = node.children() {
            for child in children.nodes() {
                let key = child.name().value();
                if let Some(entry) = child.entries().first() {
                    match key {
                        "mode" => {
                            if let Some(s) = entry.value().as_string() {
                                obj.insert(
                                    "mode".to_string(),
                                    serde_json::Value::String(s.to_string()),
                                );
                            }
                        }
                        "delete_on_success" => {
                            if let Some(b) = entry.value().as_bool() {
                                obj.insert(
                                    "delete_on_success".to_string(),
                                    serde_json::Value::Bool(b),
                                );
                            }
                        }
                        _ => {
                            log::warn!("Unknown stdio field '{}' will be ignored", key);
                        }
                    }
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
                "parameters" => {
                    if let Some(params) = Self::kdl_parameters_to_json(node)? {
                        obj.insert("parameters".to_string(), params);
                    }
                }
                "variables" => {
                    if let Some(vars) = Self::kdl_string_map_to_json(node, "Variable")? {
                        obj.insert("variables".to_string(), vars);
                    }
                }
                "env" => {
                    if let Some(env) = Self::kdl_string_map_to_json(node, "Environment key")? {
                        obj.insert("env".to_string(), env);
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
                "execution_config" => {
                    obj.insert(
                        "execution_config".to_string(),
                        Self::kdl_execution_config_to_json(node)?,
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
        // Variables (workflow-level constants)
        if let Some(ref vars) = self.variables
            && !vars.is_empty()
        {
            lines.push("variables {".to_string());
            let mut entries: Vec<_> = vars.iter().collect();
            entries.sort_by_key(|(left, _)| *left);
            for (key, value) in entries {
                lines.push(format!("    {} {}", key, kdl_escape(value)));
            }
            lines.push("}".to_string());
        }
        if let Some(ref env) = self.env
            && !env.is_empty()
        {
            lines.push("env {".to_string());
            let mut entries: Vec<_> = env.iter().collect();
            entries.sort_by_key(|(left, _)| *left);
            for (key, value) in entries {
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
                "    sample_interval_seconds {}",
                monitor.sample_interval_seconds
            ));
            lines.push(format!(
                "    flush_interval_seconds {}",
                monitor.flush_interval_seconds
            ));
            lines.push(format!(
                "    generate_plots {}",
                if monitor.generate_plots {
                    "#true"
                } else {
                    "#false"
                }
            ));
            if monitor.jobs.is_some() || monitor.enabled {
                let jobs = monitor.jobs_config();
                lines.push("    jobs {".to_string());
                lines.push(format!(
                    "        enabled {}",
                    if jobs.enabled { "#true" } else { "#false" }
                ));
                let granularity = match jobs.granularity {
                    crate::client::resource_monitor::MonitorGranularity::Summary => "summary",
                    crate::client::resource_monitor::MonitorGranularity::TimeSeries => {
                        "time_series"
                    }
                };
                lines.push(format!("        granularity \"{}\"", granularity));
                lines.push("    }".to_string());
            }
            if let Some(ref compute_node) = monitor.compute_node {
                lines.push("    compute_node {".to_string());
                lines.push(format!(
                    "        enabled {}",
                    if compute_node.enabled {
                        "#true"
                    } else {
                        "#false"
                    }
                ));
                let granularity = match compute_node.granularity {
                    crate::client::resource_monitor::MonitorGranularity::Summary => "summary",
                    crate::client::resource_monitor::MonitorGranularity::TimeSeries => {
                        "time_series"
                    }
                };
                lines.push(format!("        granularity \"{}\"", granularity));
                lines.push(format!(
                    "        cpu {}",
                    if compute_node.cpu { "#true" } else { "#false" }
                ));
                lines.push(format!(
                    "        memory {}",
                    if compute_node.memory {
                        "#true"
                    } else {
                        "#false"
                    }
                ));
                lines.push("    }".to_string());
            }
            lines.push("}".to_string());
            lines.push(String::new());
        }

        // Execution config
        if let Some(ref exec_config) = self.execution_config {
            lines.push("execution_config {".to_string());
            match exec_config.mode {
                ExecutionMode::Direct => lines.push("    mode \"direct\"".to_string()),
                ExecutionMode::Slurm => lines.push("    mode \"slurm\"".to_string()),
                ExecutionMode::Auto => lines.push("    mode \"auto\"".to_string()),
            }
            if let Some(limit) = exec_config.limit_resources {
                lines.push(format!(
                    "    limit_resources {}",
                    if limit { "#true" } else { "#false" }
                ));
            }
            if let Some(ref signal) = exec_config.termination_signal {
                lines.push(format!("    termination_signal {}", kdl_escape(signal)));
            }
            if let Some(secs) = exec_config.sigterm_lead_seconds {
                lines.push(format!("    sigterm_lead_seconds {}", secs));
            }
            if let Some(secs) = exec_config.sigkill_headroom_seconds {
                lines.push(format!("    sigkill_headroom_seconds {}", secs));
            }
            if let Some(code) = exec_config.timeout_exit_code {
                lines.push(format!("    timeout_exit_code {}", code));
            }
            if let Some(code) = exec_config.oom_exit_code {
                lines.push(format!("    oom_exit_code {}", code));
            }
            if let Some(ref signal) = exec_config.srun_termination_signal {
                lines.push(format!(
                    "    srun_termination_signal {}",
                    kdl_escape(signal)
                ));
            }
            if let Some(ref mpi) = exec_config.srun_mpi {
                lines.push(format!("    srun_mpi {}", kdl_escape(mpi)));
            }
            if let Some(bind) = exec_config.enable_cpu_bind {
                lines.push(format!(
                    "    enable_cpu_bind {}",
                    if bind { "#true" } else { "#false" }
                ));
            }
            if let Some(ref stdio) = exec_config.stdio {
                Self::stdio_config_to_kdl(&mut lines, stdio, "    ");
            }
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

    /// Serialize a `StdioConfig` to KDL lines with a given indent prefix.
    #[cfg(feature = "client")]
    fn stdio_config_to_kdl(lines: &mut Vec<String>, stdio: &StdioConfig, indent: &str) {
        lines.push(format!("{}stdio {{", indent));
        let mode_str = match stdio.mode {
            StdioMode::Separate => "separate",
            StdioMode::Combined => "combined",
            StdioMode::NoStdout => "no_stdout",
            StdioMode::NoStderr => "no_stderr",
            StdioMode::None => "none",
        };
        lines.push(format!("{}    mode \"{}\"", indent, mode_str));
        if let Some(delete) = stdio.delete_on_success {
            lines.push(format!(
                "{}    delete_on_success {}",
                indent,
                if delete { "#true" } else { "#false" }
            ));
        }
        lines.push(format!("{}}}", indent));
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

        let has_identifier = file.identifier.is_some();

        if !has_params && !has_mode && !has_use_params && !has_identifier {
            // Simple form: file "name" path="value"
            lines.push(format!(
                "file {} path={}",
                escape(&file.name),
                escape(&file.path)
            ));
        } else {
            lines.push(format!("file {} {{", escape(&file.name)));
            lines.push(format!("    path {}", escape(&file.path)));
            if let Some(ref identifier) = file.identifier {
                lines.push(format!("    identifier {}", escape(identifier)));
            }
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
        if let Some(serialize) = sched.serialize_allocations {
            lines.push(format!(
                "    serialize_allocations {}",
                if serialize { "#true" } else { "#false" }
            ));
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
        if let Some(ref env) = job.env
            && !env.is_empty()
        {
            lines.push("    env {".to_string());
            let mut entries: Vec<_> = env.iter().collect();
            entries.sort_by_key(|(left, _)| *left);
            for (key, value) in entries {
                lines.push(format!("        {} {}", key, escape(value)));
            }
            lines.push("    }".to_string());
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
        if let Some(ref stdio) = job.stdio {
            Self::stdio_config_to_kdl(lines, stdio, "    ");
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

    /// Detect the format of workflow-spec content by attempting each parser in
    /// the same order as `from_spec_file`'s extension-less fallback. Returns a
    /// canonical file extension ("json", "json5", "yaml", or "kdl"), or `None`
    /// when the content cannot be parsed by any supported format.
    pub fn detect_spec_format(content: &str) -> Option<&'static str> {
        // A workflow spec is always a mapping/object. Requiring an object (rather
        // than just "parses successfully") avoids false positives -- notably YAML,
        // which happily parses arbitrary text such as KDL as a bare scalar string.
        let parses_as_object = |v: Option<serde_json::Value>| v.is_some_and(|v| v.is_object());
        if parses_as_object(serde_json::from_str(content).ok()) {
            Some("json")
        } else if parses_as_object(json5::from_str(content).ok()) {
            Some("json5")
        } else if parses_as_object(serde_yaml::from_str(content).ok()) {
            Some("yaml")
        } else {
            #[cfg(feature = "client")]
            {
                if Self::kdl_to_json_value(content).is_ok() {
                    return Some("kdl");
                }
            }
            None
        }
    }

    /// Resolve a workflow-spec CLI argument that may be `-` (stdin).
    ///
    /// For a normal path, the argument is used as-is. For `-`, the spec is read
    /// once from stdin, its format is detected, and it is staged in a temp file
    /// with a matching extension so the existing path-based loaders -- which may
    /// read the file more than once (prevalidate, then create) -- work unchanged.
    ///
    /// The returned [`ResolvedSpecSource`] owns the temp file; keep it alive for
    /// as long as its `path()` is used.
    #[cfg(feature = "client")]
    pub fn resolve_spec_source(
        arg: &str,
    ) -> Result<ResolvedSpecSource, Box<dyn std::error::Error>> {
        use std::io::{IsTerminal, Read, Write};

        if arg != "-" {
            return Ok(ResolvedSpecSource {
                _temp: None,
                path: PathBuf::from(arg),
            });
        }

        if std::io::stdin().is_terminal() {
            return Err("workflow spec '-' requires piped stdin, but stdin is a terminal".into());
        }

        let mut content = String::new();
        std::io::stdin().read_to_string(&mut content)?;
        if content.trim().is_empty() {
            return Err("workflow spec read from stdin is empty".into());
        }

        let ext = Self::detect_spec_format(&content).ok_or(
            "unable to detect the format of the workflow spec read from stdin \
             (expected JSON, JSON5, YAML, or KDL)",
        )?;

        let mut tmp = tempfile::Builder::new()
            .prefix("torc-stdin-spec-")
            .suffix(&format!(".{}", ext))
            .tempfile()?;
        tmp.write_all(content.as_bytes())?;
        tmp.flush()?;
        let path = tmp.path().to_path_buf();

        Ok(ResolvedSpecSource {
            _temp: Some(tmp),
            path,
        })
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

    /// Substitute variables and extract input/output dependencies.
    ///
    /// Scans the command exactly once with a single regex pass that matches all four
    /// workflow-variable forms. Previously this iterated every declared file/user_data
    /// entry and did a `format!`+`String::contains` per entry, which is `O(jobs * files)`
    /// across the workflow and dominates creation time once either side reaches the
    /// thousands. The regex pass is `O(command_length + matches_in_command)`.
    ///
    /// Behavior preserved from the legacy implementation:
    /// - Unknown names are left in the command verbatim (the original silently skipped
    ///   them via `contains` returning `false`).
    /// - Returned name vectors are deduplicated; a token that appears N times in the
    ///   command contributes a single entry. The vectors now follow command order
    ///   rather than `HashMap` iteration order, which is deterministic but did not
    ///   exist before.
    ///
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
        let mut input_files: Vec<String> = Vec::new();
        let mut output_files: Vec<String> = Vec::new();
        let mut input_user_data: Vec<String> = Vec::new();
        let mut output_user_data: Vec<String> = Vec::new();
        // Hoist the first JSON serialization error out of the closure; `replace_all`
        // can't propagate `Result`, so we stash it and surface it below.
        let mut serialization_error: Option<Box<dyn std::error::Error>> = None;

        // Per-command vectors stay small (handful of files/user_data refs), so a linear
        // `iter().any` dedup beats spinning up a HashSet.
        fn push_unique(vec: &mut Vec<String>, name: &str) {
            if !vec.iter().any(|n| n == name) {
                vec.push(name.to_string());
            }
        }

        let result = WORKFLOW_VARIABLE_REGEX.replace_all(input, |caps: &regex::Captures<'_>| {
            let full_match = caps.get(0).expect("match always present").as_str();
            let namespace = caps.get(1).expect("group 1 always captured").as_str();
            let direction = caps.get(2).expect("group 2 always captured").as_str();
            let name = caps.get(3).expect("group 3 always captured").as_str();

            match namespace {
                "files" => match file_name_to_path.get(name) {
                    Some(path) => {
                        let bucket = if direction == "input" {
                            &mut input_files
                        } else {
                            &mut output_files
                        };
                        push_unique(bucket, name);
                        path.clone()
                    }
                    None => full_match.to_string(),
                },
                "user_data" => match user_data_name_to_data.get(name) {
                    Some(data) => match serde_json::to_string(data) {
                        Ok(serialized) => {
                            let bucket = if direction == "input" {
                                &mut input_user_data
                            } else {
                                &mut output_user_data
                            };
                            push_unique(bucket, name);
                            serialized
                        }
                        Err(e) => {
                            if serialization_error.is_none() {
                                serialization_error = Some(Box::new(e));
                            }
                            // Leave the token in place so the failure points at the source.
                            full_match.to_string()
                        }
                    },
                    None => full_match.to_string(),
                },
                _ => full_match.to_string(), // regex restricts to the two namespaces above
            }
        });

        if let Some(err) = serialization_error {
            return Err(err);
        }

        Ok((
            result.into_owned(),
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
    use crate::client::resource_monitor::MonitorGranularity;
    use std::path::PathBuf;

    #[test]
    fn test_detect_spec_format() {
        // Strict JSON is detected as JSON (and takes precedence over the JSON5/YAML
        // supersets, since the checks run in that order).
        assert_eq!(
            WorkflowSpec::detect_spec_format(r#"{"name": "wf", "jobs": []}"#),
            Some("json")
        );
        // JSON5-only syntax (comments, trailing commas, unquoted keys) is not valid JSON.
        assert_eq!(
            WorkflowSpec::detect_spec_format("{ name: 'wf', /* c */ jobs: [], }"),
            Some("json5")
        );
        // Block-style YAML is neither JSON nor JSON5.
        assert_eq!(
            WorkflowSpec::detect_spec_format("name: wf\njobs:\n  - name: a\n    command: echo"),
            Some("yaml")
        );
        // KDL is only attempted under the client feature.
        #[cfg(feature = "client")]
        assert_eq!(
            WorkflowSpec::detect_spec_format("name \"wf\"\njobs {\n  job name=\"a\"\n}"),
            Some("kdl")
        );
    }

    #[test]
    fn test_legacy_resource_monitor_yaml_controls_jobs() {
        let yaml_content = r#"
name: legacy_resource_monitor_yaml
jobs:
  - name: job1
    command: echo hello
resource_monitor:
  enabled: true
  granularity: time_series
  sample_interval_seconds: 2
"#;

        let spec = WorkflowSpec::from_spec_file_content(yaml_content, "yaml")
            .expect("Failed to parse YAML workflow spec");
        let monitor = spec.resource_monitor.expect("missing resource monitor");
        let jobs = monitor.jobs_config();

        assert!(jobs.enabled);
        assert_eq!(jobs.granularity, MonitorGranularity::TimeSeries);
        assert_eq!(monitor.sample_interval_seconds, 2);
        assert!(monitor.compute_node_config().is_none());
    }

    #[test]
    fn test_legacy_resource_monitor_json5_controls_jobs() {
        let json5_content = r#"
{
  name: "legacy_resource_monitor_json5",
  jobs: [
    { name: "job1", command: "echo hello" },
  ],
  resource_monitor: {
    enabled: true,
    granularity: "time_series",
    sample_interval_seconds: 2,
  },
}
"#;

        let spec = WorkflowSpec::from_spec_file_content(json5_content, "json5")
            .expect("Failed to parse JSON5 workflow spec");
        let monitor = spec.resource_monitor.expect("missing resource monitor");
        let jobs = monitor.jobs_config();

        assert!(jobs.enabled);
        assert_eq!(jobs.granularity, MonitorGranularity::TimeSeries);
        assert_eq!(monitor.sample_interval_seconds, 2);
        assert!(monitor.compute_node_config().is_none());
    }

    #[test]
    fn test_legacy_resource_monitor_kdl_controls_jobs() {
        let kdl_content = r#"
name "legacy_resource_monitor_kdl"

resource_monitor {
    enabled #true
    granularity "time_series"
    sample_interval_seconds 2
    flush_interval_seconds 17
}

job "job1" {
    command "echo hello"
}
"#;

        let spec = WorkflowSpec::from_spec_file_content(kdl_content, "kdl")
            .expect("Failed to parse KDL workflow spec");
        let monitor = spec.resource_monitor.expect("missing resource monitor");
        let jobs = monitor.jobs_config();

        assert!(jobs.enabled);
        assert_eq!(jobs.granularity, MonitorGranularity::TimeSeries);
        assert_eq!(monitor.sample_interval_seconds, 2);
        assert_eq!(monitor.flush_interval_seconds, 17);
        assert!(monitor.compute_node_config().is_none());
    }

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
    fn test_kdl_file_identifier_round_trip() {
        // KDL parser/serializer must round-trip the new `identifier` field so
        // it doesn't regress independently of YAML/JSON support. Cover both the
        // child-node form (block) and the property form (inline).
        let kdl_block = r#"
name "with_identifier"

file "ref" {
    path "/data/ref.csv"
    identifier "urn:dataset:ref"
}

job "consume" {
    command "echo"
    input_files "ref"
}
"#;
        let spec_from_block = WorkflowSpec::from_spec_file_content(kdl_block, "kdl")
            .expect("Failed to parse KDL spec with child-node identifier");
        let file = &spec_from_block.files.as_ref().unwrap()[0];
        assert_eq!(file.identifier.as_deref(), Some("urn:dataset:ref"));

        // Property form on the same line as `file "name"`.
        let kdl_property = r#"
name "with_identifier_property"

file "ref" path="/data/ref.csv" identifier="urn:dataset:ref"

job "consume" {
    command "echo"
    input_files "ref"
}
"#;
        let spec_from_prop = WorkflowSpec::from_spec_file_content(kdl_property, "kdl")
            .expect("Failed to parse KDL spec with property-form identifier");
        let file_prop = &spec_from_prop.files.as_ref().unwrap()[0];
        assert_eq!(file_prop.identifier.as_deref(), Some("urn:dataset:ref"));

        // to_kdl_str → parse → check that identifier survives the round-trip.
        let mut spec_emit =
            WorkflowSpec::new("emit".to_string(), "tester".to_string(), None, vec![]);
        let mut f = FileSpec::new("ref".to_string(), "/data/ref.csv".to_string());
        f.identifier = Some("urn:dataset:ref".to_string());
        spec_emit.files = Some(vec![f]);

        let emitted = spec_emit.to_kdl_str();
        assert!(
            emitted.contains("identifier"),
            "emitted KDL missing identifier: {}",
            emitted
        );
        let reparsed = WorkflowSpec::from_spec_file_content(&emitted, "kdl")
            .expect("Failed to re-parse emitted KDL");
        assert_eq!(
            reparsed.files.as_ref().unwrap()[0].identifier.as_deref(),
            Some("urn:dataset:ref")
        );
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
        // - 18 aggregate jobs (parameterized name includes every parameter
        //   so post-expansion names are unique)
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
    fn test_file_identifier_parameter_substitution() {
        // Parameterized identifiers must expand the same way `name` and `path` do.
        // Otherwise an unparameterized identifier template silently collapses every
        // expanded file onto the same `@id` and the duplicate-identifier validation
        // below would always fire -- which is misleading when the real bug is a
        // missing placeholder.
        let mut file = FileSpec::new("input_{i}".to_string(), "/data/input_{i}.csv".to_string());
        file.identifier = Some("urn:dataset:{i}".to_string());

        let mut params = HashMap::new();
        params.insert("i".to_string(), "1:3".to_string());
        file.parameters = Some(params);

        let expanded = file.expand().expect("Failed to expand file");

        assert_eq!(expanded.len(), 3);
        assert_eq!(expanded[0].identifier.as_deref(), Some("urn:dataset:1"));
        assert_eq!(expanded[2].identifier.as_deref(), Some("urn:dataset:3"));
    }

    #[test]
    fn test_validate_unique_names_rejects_duplicate_file_identifiers() {
        // Two distinct files trying to claim the same RO-Crate `@id` must be
        // rejected up-front; otherwise the pre-create step in `create_files`
        // would fail mid-creation with a server-side uniqueness error and roll
        // back the entire workflow, which is much harder to debug than a clear
        // spec-load error.
        let mut a = FileSpec::new("a".to_string(), "/data/a.csv".to_string());
        a.identifier = Some("urn:dataset:shared".to_string());
        let mut b = FileSpec::new("b".to_string(), "/data/b.csv".to_string());
        b.identifier = Some("urn:dataset:shared".to_string());

        let mut spec = WorkflowSpec::new("wf".to_string(), "tester".to_string(), None, vec![]);
        spec.files = Some(vec![a, b]);

        let err = spec
            .validate_unique_names_after_expansion()
            .expect_err("expected duplicate-identifier rejection");
        let msg = err.to_string();
        assert!(
            msg.contains("Duplicate file identifier 'urn:dataset:shared'"),
            "unexpected error message: {}",
            msg
        );
        assert!(msg.contains("'a'") && msg.contains("'b'"), "{}", msg);
    }

    #[test]
    fn test_validate_file_identifiers_requires_enable_ro_crate() {
        // Setting `identifier` without enabling RO-Crate would silently create one
        // dangling entity row with no other provenance -- friendlier to reject than
        // to half-honor.
        let mut f = FileSpec::new("a".to_string(), "/data/a.csv".to_string());
        f.identifier = Some("urn:dataset:abc".to_string());
        let mut job = JobSpec::new("consume".to_string(), "process".to_string());
        job.input_files = Some(vec!["a".to_string()]);
        let mut spec = WorkflowSpec::new("wf".to_string(), "tester".to_string(), None, vec![job]);
        spec.files = Some(vec![f]);
        // enable_ro_crate is None by default

        let err = spec
            .validate_file_identifiers()
            .expect_err("expected enable_ro_crate requirement");
        let msg = err.to_string();
        assert!(msg.contains("enable_ro_crate"), "{}", msg);
        assert!(msg.contains("'a'"), "{}", msg);

        // Enabling it makes the same spec validate cleanly.
        spec.enable_ro_crate = Some(true);
        spec.validate_file_identifiers()
            .expect("identifier+enable_ro_crate should be allowed");
    }

    #[test]
    fn test_validate_file_identifiers_rejects_output_only_file() {
        // A file referenced only as a job output cannot carry an identifier
        // because the output-file entity-creation path doesn't consult it.
        // Silently dropping the value would be confusing, so reject.
        let mut producer = JobSpec::new("producer".to_string(), "echo".to_string());
        producer.output_files = Some(vec!["out".to_string()]);

        let mut out = FileSpec::new("out".to_string(), "/data/out.csv".to_string());
        out.identifier = Some("urn:dataset:out".to_string());

        let mut spec =
            WorkflowSpec::new("wf".to_string(), "tester".to_string(), None, vec![producer]);
        spec.enable_ro_crate = Some(true);
        spec.files = Some(vec![out]);

        let err = spec
            .validate_file_identifiers()
            .expect_err("expected output-only rejection");
        let msg = err.to_string();
        assert!(msg.contains("output"), "{}", msg);
        assert!(msg.contains("'out'"), "{}", msg);
    }

    #[test]
    fn test_validate_file_identifiers_rejects_dual_use_file() {
        // A file referenced as both input AND output also has to be rejected.
        // When the producing job completes, `create_ro_crate_entity_for_output_file`
        // calls `build_file_entity_with_provenance` which always sets
        // `entity_id = file.path`, clobbering the user's identifier in both
        // the entity_id column and metadata. Allowing this would silently
        // strip the identifier after the first job completes.
        let mut producer = JobSpec::new("producer".to_string(), "make".to_string());
        producer.output_files = Some(vec!["shared".to_string()]);
        let mut consumer = JobSpec::new("consumer".to_string(), "transform".to_string());
        consumer.input_files = Some(vec!["shared".to_string()]);

        let mut shared = FileSpec::new("shared".to_string(), "/data/shared.csv".to_string());
        shared.identifier = Some("urn:dataset:shared".to_string());

        let mut spec = WorkflowSpec::new(
            "wf".to_string(),
            "tester".to_string(),
            None,
            vec![producer, consumer],
        );
        spec.enable_ro_crate = Some(true);
        spec.files = Some(vec![shared]);

        let err = spec
            .validate_file_identifiers()
            .expect_err("dual-use identifier should be rejected");
        let msg = err.to_string();
        assert!(msg.contains("'shared'"), "{}", msg);
        assert!(msg.contains("output"), "{}", msg);
    }

    #[test]
    fn test_validate_file_identifiers_accepts_input_via_regex() {
        // A job that declares its inputs via `input_file_regexes` (rather than
        // an exact `input_files` list) still classifies matching files as
        // inputs, so identifiers on those files must validate cleanly.
        let mut consumer = JobSpec::new("consumer".to_string(), "process".to_string());
        consumer.input_file_regexes = Some(vec!["^input_.*$".to_string()]);

        let mut f = FileSpec::new("input_a".to_string(), "/data/a.csv".to_string());
        f.identifier = Some("urn:dataset:a".to_string());

        let mut spec =
            WorkflowSpec::new("wf".to_string(), "tester".to_string(), None, vec![consumer]);
        spec.enable_ro_crate = Some(true);
        spec.files = Some(vec![f]);

        spec.validate_file_identifiers()
            .expect("regex-matched input should be accepted");
    }

    #[test]
    fn test_validate_file_identifiers_rejects_identifier_equal_to_other_path() {
        // Identifier of file A equal to path of file B would collide in the
        // (workflow_id, entity_id) unique index and silently drop one entity.
        let mut a = FileSpec::new("a".to_string(), "/data/a.csv".to_string());
        a.identifier = Some("/data/b.csv".to_string());
        let b = FileSpec::new("b".to_string(), "/data/b.csv".to_string());

        // Reference both as inputs so the dual-use / orphan checks don't fire first.
        let mut job = JobSpec::new("consume".to_string(), "process".to_string());
        job.input_files = Some(vec!["a".to_string(), "b".to_string()]);

        let mut spec = WorkflowSpec::new("wf".to_string(), "tester".to_string(), None, vec![job]);
        spec.enable_ro_crate = Some(true);
        spec.files = Some(vec![a, b]);

        let err = spec
            .validate_file_identifiers()
            .expect_err("identifier-equals-other-path should be rejected");
        let msg = err.to_string();
        assert!(msg.contains("'a'") && msg.contains("'b'"), "{}", msg);
        assert!(msg.contains("/data/b.csv"), "{}", msg);
    }

    #[test]
    fn test_validate_file_identifiers_rejects_orphan_identifier() {
        // A file with an identifier that's not referenced by any job and has
        // no `st_mtime` would create a dangling entity at export time. Reject
        // so typos in input_files surface as a clear error.
        let mut f = FileSpec::new("orphan".to_string(), "/data/orphan.csv".to_string());
        f.identifier = Some("urn:dataset:orphan".to_string());

        let mut spec = WorkflowSpec::new("wf".to_string(), "tester".to_string(), None, vec![]);
        spec.enable_ro_crate = Some(true);
        spec.files = Some(vec![f]);

        let err = spec
            .validate_file_identifiers()
            .expect_err("orphan identifier should be rejected");
        let msg = err.to_string();
        assert!(msg.contains("'orphan'"), "{}", msg);
        assert!(msg.contains("input"), "{}", msg);
    }

    #[test]
    fn test_validate_file_identifiers_rejects_blank_identifier() {
        // Empty/whitespace identifiers would round-trip as `entity_id = ""`
        // and `@id = ""` in the exported graph -- nonsense values that
        // bypass every other identifier check. Reject early.
        for blank in ["", "   ", "\t\n"] {
            let mut f = FileSpec::new("a".to_string(), "/data/a.csv".to_string());
            f.identifier = Some(blank.to_string());
            let mut spec = WorkflowSpec::new("wf".to_string(), "tester".to_string(), None, vec![]);
            spec.enable_ro_crate = Some(true);
            spec.files = Some(vec![f]);

            let err = spec
                .validate_file_identifiers()
                .expect_err(&format!("expected rejection of blank id {:?}", blank));
            let msg = err.to_string();
            assert!(msg.contains("empty"), "for {:?}: {}", blank, msg);
            assert!(msg.contains("'a'"), "for {:?}: {}", blank, msg);
        }
    }

    #[test]
    fn test_validate_file_identifiers_rejects_reserved_prefixes() {
        // Identifiers starting with reserved prefixes would collide with Torc's own
        // provenance entities, which share the (workflow_id, entity_id) uniqueness
        // index, or with the synthetic root entities the exporter always emits.
        // Reject at spec load to surface a clear error.
        for reserved in [
            "#torc-workflow",
            "#torc-run-id-1",
            "#software-torc-run-id-1",
            "#job-42-attempt-1",
            "ro-crate-metadata.json",
            "./",
        ] {
            let mut f = FileSpec::new("a".to_string(), "/data/a.csv".to_string());
            f.identifier = Some(reserved.to_string());
            let mut spec = WorkflowSpec::new("wf".to_string(), "tester".to_string(), None, vec![]);
            spec.enable_ro_crate = Some(true);
            spec.files = Some(vec![f]);

            let err = spec
                .validate_file_identifiers()
                .expect_err(&format!("expected rejection of reserved id '{}'", reserved));
            let msg = err.to_string();
            assert!(msg.contains("reserved"), "for '{}': {}", reserved, msg);
            assert!(msg.contains(reserved), "for '{}': {}", reserved, msg);
        }
    }

    #[test]
    fn test_user_data_parameterization() {
        // A parameterized user_data with a string substitution inside its data payload.
        let mut ud = UserDataSpec {
            is_ephemeral: Some(false),
            name: Some("config_{experiment}".to_string()),
            data: Some(serde_json::json!({
                "experiment": "{experiment}",
                "learning_rate": 0.001,
                "tags": ["base", "{experiment}"],
                "nested": { "label": "exp-{experiment}" },
            })),
            parameters: None,
            parameter_mode: None,
            use_parameters: None,
            parameters_file: None,
            use_parameters_file: None,
        };
        let mut params = HashMap::new();
        params.insert(
            "experiment".to_string(),
            "['baseline','ablation','full']".to_string(),
        );
        ud.parameters = Some(params);

        let expanded = ud.expand().expect("Failed to expand user_data");
        assert_eq!(expanded.len(), 3);

        // Find the baseline copy and verify substitution in name + every string slot of data.
        let baseline = expanded
            .iter()
            .find(|u| u.name.as_deref() == Some("config_baseline"))
            .expect("baseline copy must exist");

        // parameters/parameter_mode are cleared after expansion.
        assert!(baseline.parameters.is_none());
        assert!(baseline.parameter_mode.is_none());

        let data = baseline.data.as_ref().expect("data preserved");
        assert_eq!(data["experiment"], serde_json::json!("baseline"));
        // Non-string values are passed through unchanged.
        assert_eq!(data["learning_rate"], serde_json::json!(0.001));
        // Array string elements are substituted; non-template strings pass through.
        assert_eq!(data["tags"], serde_json::json!(["base", "baseline"]));
        // Nested object string values are substituted too.
        assert_eq!(data["nested"]["label"], serde_json::json!("exp-baseline"));
    }

    #[test]
    fn test_user_data_parameterization_from_yaml() {
        // End-to-end YAML parse → expand_parameters round-trip: mirrors the shape of
        // examples/yaml/parameterized_user_data.yaml.
        let yaml = r#"
name: ud_yaml_test
jobs:
  - name: train_{experiment}
    command: "python train.py --config-name config_{experiment}"
    input_user_data:
      - config_{experiment}
    parameters:
      experiment: "['baseline','ablation','full']"
user_data:
  - name: config_{experiment}
    data:
      experiment: "{experiment}"
      learning_rate: 0.001
      output_dir: /results/{experiment}
    parameters:
      experiment: "['baseline','ablation','full']"
"#;
        let mut spec =
            WorkflowSpec::from_spec_file_content(yaml, "yaml").expect("YAML parse must succeed");
        spec.expand_parameters().expect("expand must succeed");

        // 3 expanded jobs and 3 expanded user_data records, names lining up.
        assert_eq!(spec.jobs.len(), 3);
        let job_names: Vec<&str> = spec.jobs.iter().map(|j| j.name.as_str()).collect();
        assert!(job_names.contains(&"train_baseline"));
        assert!(job_names.contains(&"train_ablation"));
        assert!(job_names.contains(&"train_full"));

        let ud = spec.user_data.as_ref().expect("user_data preserved");
        assert_eq!(ud.len(), 3);
        let baseline = ud
            .iter()
            .find(|u| u.name.as_deref() == Some("config_baseline"))
            .expect("baseline user_data exists");
        let data = baseline.data.as_ref().unwrap();
        assert_eq!(data["experiment"], serde_json::json!("baseline"));
        assert_eq!(data["output_dir"], serde_json::json!("/results/baseline"));
        // Number values pass through unchanged.
        assert_eq!(data["learning_rate"], serde_json::json!(0.001));
    }

    #[test]
    fn test_user_data_no_parameters_returns_clone() {
        // Without `parameters`, expand() yields the input unchanged.
        let ud = UserDataSpec {
            is_ephemeral: Some(true),
            name: Some("config".to_string()),
            data: Some(serde_json::json!({"key": "value"})),
            parameters: None,
            parameter_mode: None,
            use_parameters: None,
            parameters_file: None,
            use_parameters_file: None,
        };
        let expanded = ud.expand().expect("expand should succeed");
        assert_eq!(expanded.len(), 1);
        assert_eq!(expanded[0], ud);
    }

    #[test]
    fn test_workflow_spec_expand_parameters_user_data() {
        // End-to-end check: workflow-level params reach user_data via use_parameters,
        // and the user_data section is expanded alongside jobs/files.
        let mut spec = WorkflowSpec::new(
            "ud_param_test".to_string(),
            "tester".to_string(),
            None,
            vec![],
        );
        spec.parameters = Some(HashMap::from([("i".to_string(), "1:2".to_string())]));
        spec.user_data = Some(vec![UserDataSpec {
            is_ephemeral: Some(false),
            name: Some("config_{i}".to_string()),
            data: Some(serde_json::json!({"index": "{i}"})),
            parameters: None,
            parameter_mode: None,
            use_parameters: Some(vec!["i".to_string()]),
            parameters_file: None,
            use_parameters_file: None,
        }]);

        spec.expand_parameters()
            .expect("expand_parameters must succeed");

        let expanded = spec.user_data.as_ref().expect("user_data preserved");
        assert_eq!(expanded.len(), 2);
        assert_eq!(expanded[0].name.as_deref(), Some("config_1"));
        assert_eq!(expanded[1].name.as_deref(), Some("config_2"));
        assert_eq!(
            expanded[0].data.as_ref().unwrap()["index"],
            serde_json::json!("1")
        );
        assert_eq!(
            expanded[1].data.as_ref().unwrap()["index"],
            serde_json::json!("2")
        );
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
            parameters: None,
            parameters_file: None,
            variables: None,
            env: None,
            jobs: vec![JobSpec {
                name: "job_{i}".to_string(),
                command: "echo {i}".to_string(),
                invocation_script: None,
                env: None,
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
                parameters_file: None,
                use_parameters_file: None,
                failure_handler: None,
                stdio: None,
                priority: None,
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
            dynamic_jobs: None,
            use_pending_failed: None,
            enable_ro_crate: None,
            project: None,
            metadata: None,
            execution_config: None,
            access_groups: None,
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
    fn test_workflow_env_stays_on_workflow_during_parameter_expansion() {
        let mut job = JobSpec::new("job".to_string(), "echo hi".to_string());
        job.env = Some(HashMap::from([
            ("JOB_ONLY".to_string(), "job".to_string()),
            ("SHARED".to_string(), "job".to_string()),
        ]));

        let mut spec = WorkflowSpec::new("wf".to_string(), "user".to_string(), None, vec![job]);
        spec.env = Some(HashMap::from([
            ("WF_ONLY".to_string(), "workflow".to_string()),
            ("SHARED".to_string(), "workflow".to_string()),
        ]));

        spec.expand_parameters().expect("expand parameters");

        assert_eq!(
            spec.env,
            Some(HashMap::from([
                ("WF_ONLY".to_string(), "workflow".to_string()),
                ("SHARED".to_string(), "workflow".to_string()),
            ]))
        );
        assert_eq!(spec.jobs[0].command, "echo hi".to_string());
        assert_eq!(
            spec.jobs[0].env,
            Some(HashMap::from([
                ("JOB_ONLY".to_string(), "job".to_string()),
                ("SHARED".to_string(), "job".to_string()),
            ]))
        );
    }

    #[test]
    fn test_workflow_env_parameters_are_substituted() {
        let job = JobSpec::new("job".to_string(), "echo hi".to_string());
        let mut spec = WorkflowSpec::new("wf".to_string(), "user".to_string(), None, vec![job]);
        spec.parameters = Some(HashMap::from([("target".to_string(), "gpu".to_string())]));
        spec.env = Some(HashMap::from([(
            "QUEUE".to_string(),
            "queue_{target}".to_string(),
        )]));

        spec.expand_parameters().expect("expand parameters");

        assert_eq!(
            spec.env
                .as_ref()
                .and_then(|env| env.get("QUEUE"))
                .map(String::as_str),
            Some("queue_gpu")
        );
    }

    #[test]
    fn test_validate_env_maps_rejects_invalid_names() {
        let mut job = JobSpec::new("job".to_string(), "echo hi".to_string());
        job.env = Some(HashMap::from([(
            "BAD-NAME".to_string(),
            "value".to_string(),
        )]));
        let spec = WorkflowSpec::new("wf".to_string(), "user".to_string(), None, vec![job]);

        let err = spec
            .validate_env_maps()
            .expect_err("expected validation error");
        assert!(err.to_string().contains("BAD-NAME"));
    }

    #[test]
    fn test_validate_dynamic_jobs_rejects_non_positive_max_iterations() {
        let job = JobSpec::new("job".to_string(), "echo hi".to_string());
        let mut spec = WorkflowSpec::new("wf".to_string(), "user".to_string(), None, vec![job]);

        spec.dynamic_jobs = Some(DynamicJobsSpec {
            max_iterations: Some(0),
        });
        let err = spec
            .validate_dynamic_jobs()
            .expect_err("expected validation error for max_iterations=0");
        assert!(
            err.to_string().contains("max_iterations must be >= 1"),
            "unexpected error message: {}",
            err
        );

        spec.dynamic_jobs = Some(DynamicJobsSpec {
            max_iterations: Some(-1),
        });
        let err = spec
            .validate_dynamic_jobs()
            .expect_err("expected validation error for max_iterations=-1");
        assert!(err.to_string().contains("max_iterations must be >= 1"));

        // Positive values and an absent field are both fine.
        spec.dynamic_jobs = Some(DynamicJobsSpec {
            max_iterations: Some(1),
        });
        spec.validate_dynamic_jobs().expect("max_iterations=1 ok");
        spec.dynamic_jobs = None;
        spec.validate_dynamic_jobs().expect("no dynamic_jobs ok");
    }

    #[test]
    fn test_kdl_env_round_trip() {
        let kdl_content = r#"
name "env_workflow"
env {
    PIXI_CACHE_FOLDER "/tmp/cache"
}

job "train" {
    command "python train.py"
    env {
        JOB_FLAG "true"
    }
}
"#;

        let spec =
            WorkflowSpec::from_spec_file_content(kdl_content, "kdl").expect("parse KDL spec");
        assert_eq!(
            spec.env
                .as_ref()
                .and_then(|env| env.get("PIXI_CACHE_FOLDER")),
            Some(&"/tmp/cache".to_string())
        );
        assert_eq!(
            spec.jobs[0]
                .env
                .as_ref()
                .and_then(|env| env.get("JOB_FLAG")),
            Some(&"true".to_string())
        );

        let round_tripped =
            WorkflowSpec::from_spec_file_content(&spec.to_kdl_str(), "kdl").expect("round trip");
        assert_eq!(round_tripped.env, spec.env);
        assert_eq!(round_tripped.jobs[0].env, spec.jobs[0].env);
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

        // After expansion:
        // - 2 prepare jobs (no parameters)
        // - 18 training jobs (3 lr * 3 batch_size * 2 optimizer)
        // - 18 aggregate jobs (parameterized name includes every parameter
        //   so post-expansion names are unique)
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

        // Same structure as the YAML version: 38 jobs (2 prep + 18 train +
        // 18 parameterized aggregate), 38 files (2 data + 18 model + 18 metrics).
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

        // Same structure as the YAML/KDL versions: 38 jobs, 38 files.
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
    fn test_native_yaml_parameter_sequences_support_product_and_zip() {
        let product_yaml = r#"
name: native_lists
jobs:
  - name: run_{x}_{y}
    command: echo
    parameters:
      x:
        - a
        - b
      y:
        - 1
        - 2
"#;
        let mut product = WorkflowSpec::from_spec_file_content(product_yaml, "yaml").unwrap();
        product.expand_parameters().unwrap();
        assert_eq!(product.jobs.len(), 4);

        let zip_yaml = r#"
name: native_lists_zip
jobs:
  - name: run_{x}_{y}
    command: echo
    parameter_mode: zip
    parameters:
      x:
        - a
        - b
      y:
        - 1
        - 2
"#;
        let mut zip = WorkflowSpec::from_spec_file_content(zip_yaml, "yaml").unwrap();
        zip.expand_parameters().unwrap();
        assert_eq!(zip.jobs.len(), 2);
        assert_eq!(zip.jobs[0].name, "run_a_1");
        assert_eq!(zip.jobs[1].name, "run_b_2");
    }

    #[test]
    fn test_native_yaml_parameter_sequences_for_all_spec_scopes() {
        let yaml = r#"
name: native_lists_all_scopes
parameters:
  shared:
    - one
    - two
jobs:
  - name: job_{shared}
    command: echo
    use_parameters:
      - shared
  - name: local_{local}
    command: echo
    parameters:
      local:
        - a
        - b
files:
  - name: file_{file}
    path: /tmp/file_{file}
    parameters:
      file:
        - x
        - y
user_data:
  - name: data_{data}
    data:
      value: "{data}"
    parameters:
      data:
        - first
        - second
"#;
        let mut spec = WorkflowSpec::from_spec_file_content(yaml, "yaml").unwrap();
        spec.expand_parameters().unwrap();

        assert_eq!(spec.jobs.len(), 4);
        assert_eq!(spec.files.as_ref().unwrap().len(), 2);
        assert_eq!(spec.user_data.as_ref().unwrap().len(), 2);
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

    // ========== ExecutionConfig Tests ==========

    #[test]
    fn test_execution_config_defaults() {
        let config = ExecutionConfig::default();
        assert_eq!(config.mode, ExecutionMode::Direct);
        assert!(config.limit_resources.is_none());
        assert!(config.termination_signal.is_none());
        assert!(config.sigterm_lead_seconds.is_none());
        assert!(config.sigkill_headroom_seconds.is_none());
        assert!(config.timeout_exit_code.is_none());
        assert!(config.oom_exit_code.is_none());
    }

    #[test]
    fn test_execution_config_default_getters() {
        let config = ExecutionConfig::default();
        assert!(config.limit_resources());
        assert_eq!(config.termination_signal(), "SIGTERM");
        assert_eq!(config.sigterm_lead_seconds(), 30);
        assert_eq!(config.sigkill_headroom_seconds(), 60);
        assert_eq!(config.timeout_exit_code(), 152);
        assert_eq!(config.oom_exit_code(), 137);
    }

    #[test]
    fn test_execution_config_custom_values() {
        let config = ExecutionConfig {
            mode: ExecutionMode::Direct,
            limit_resources: Some(false),
            termination_signal: Some("SIGUSR1".to_string()),
            sigterm_lead_seconds: Some(60),
            sigkill_headroom_seconds: Some(120),
            timeout_exit_code: Some(200),
            oom_exit_code: Some(201),
            srun_termination_signal: None,
            srun_mpi: None,
            enable_cpu_bind: None,
            staggered_start: None,
            stdio: None,
            job_stdio_overrides: None,
        };
        assert!(!config.limit_resources());
        assert_eq!(config.termination_signal(), "SIGUSR1");
        assert_eq!(config.sigterm_lead_seconds(), 60);
        assert_eq!(config.sigkill_headroom_seconds(), 120);
        assert_eq!(config.timeout_exit_code(), 200);
        assert_eq!(config.oom_exit_code(), 201);
    }

    #[test]
    fn test_execution_mode_serialization() {
        let config = ExecutionConfig {
            mode: ExecutionMode::Direct,
            ..Default::default()
        };
        let json = serde_json::to_string(&config).expect("Failed to serialize");
        assert!(json.contains("\"mode\":\"direct\""));

        let config = ExecutionConfig {
            mode: ExecutionMode::Slurm,
            ..Default::default()
        };
        let json = serde_json::to_string(&config).expect("Failed to serialize");
        assert!(json.contains("\"mode\":\"slurm\""));

        let config = ExecutionConfig {
            mode: ExecutionMode::Auto,
            ..Default::default()
        };
        let json = serde_json::to_string(&config).expect("Failed to serialize");
        assert!(json.contains("\"mode\":\"auto\""));
    }

    #[test]
    fn test_execution_mode_deserialization() {
        let json = r#"{"mode":"direct"}"#;
        let config: ExecutionConfig = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(config.mode, ExecutionMode::Direct);

        let json = r#"{"mode":"slurm"}"#;
        let config: ExecutionConfig = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(config.mode, ExecutionMode::Slurm);

        let json = r#"{"mode":"auto"}"#;
        let config: ExecutionConfig = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(config.mode, ExecutionMode::Auto);
    }

    #[test]
    fn test_execution_config_use_srun_by_mode() {
        // Direct mode: use_srun = false
        let config = ExecutionConfig {
            mode: ExecutionMode::Direct,
            ..Default::default()
        };
        assert!(!config.use_srun());

        // Slurm mode: use_srun = true
        let config = ExecutionConfig {
            mode: ExecutionMode::Slurm,
            ..Default::default()
        };
        assert!(config.use_srun());
    }

    #[test]
    fn test_execution_config_yaml_parsing() {
        let yaml = r#"
name: test_workflow
jobs:
  - name: test_job
    command: echo hello
execution_config:
  mode: direct
  limit_resources: false
  termination_signal: SIGUSR2
  sigterm_lead_seconds: 45
  sigkill_headroom_seconds: 90
  timeout_exit_code: 200
  oom_exit_code: 201
"#;
        let spec: WorkflowSpec = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        let config = spec
            .execution_config
            .expect("execution_config should be present");
        assert_eq!(config.mode, ExecutionMode::Direct);
        assert_eq!(config.limit_resources, Some(false));
        assert_eq!(config.termination_signal, Some("SIGUSR2".to_string()));
        assert_eq!(config.sigterm_lead_seconds, Some(45));
        assert_eq!(config.sigkill_headroom_seconds, Some(90));
        assert_eq!(config.timeout_exit_code, Some(200));
        assert_eq!(config.oom_exit_code, Some(201));
    }

    #[test]
    fn test_execution_config_with_slurm_settings() {
        let yaml = r#"
name: test_workflow
jobs:
  - name: test_job
    command: echo hello
execution_config:
  mode: slurm
  srun_termination_signal: "TERM@120"
  srun_mpi: "pmix"
  enable_cpu_bind: true
"#;
        let spec: WorkflowSpec = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        let config = spec
            .execution_config
            .expect("execution_config should be present");
        assert_eq!(config.mode, ExecutionMode::Slurm);
        assert_eq!(config.srun_termination_signal, Some("TERM@120".to_string()));
        assert_eq!(config.srun_mpi, Some("pmix".to_string()));
        assert_eq!(config.enable_cpu_bind, Some(true));
    }

    // --- validate_scheduler_resources tests ---

    #[test]
    fn test_validate_scheduler_resources_runtime_within_walltime() {
        let yaml = r#"
name: test_workflow
jobs:
  - name: test_job
    command: echo hello
    resource_requirements: small
    scheduler: my_scheduler
resource_requirements:
  - name: small
    num_cpus: 1
    memory: "1g"
    runtime: "PT30M"
slurm_schedulers:
  - name: my_scheduler
    account: test
    walltime: "01:00:00"
"#;
        let spec: WorkflowSpec = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert!(spec.validate_scheduler_resources().is_empty());
    }

    #[test]
    fn test_validate_scheduler_resources_runtime_equals_walltime() {
        let yaml = r#"
name: test_workflow
jobs:
  - name: test_job
    command: echo hello
    resource_requirements: small
    scheduler: my_scheduler
resource_requirements:
  - name: small
    num_cpus: 1
    memory: "1g"
    runtime: "PT1H"
slurm_schedulers:
  - name: my_scheduler
    account: test
    walltime: "01:00:00"
"#;
        let spec: WorkflowSpec = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert!(spec.validate_scheduler_resources().is_empty());
    }

    #[test]
    fn test_validate_scheduler_resources_runtime_exceeds_walltime() {
        let yaml = r#"
name: test_workflow
jobs:
  - name: test_job
    command: echo hello
    resource_requirements: big
    scheduler: my_scheduler
resource_requirements:
  - name: big
    num_cpus: 1
    memory: "1g"
    runtime: "PT2H"
slurm_schedulers:
  - name: my_scheduler
    account: test
    walltime: "01:00:00"
"#;
        let spec: WorkflowSpec = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        let warnings = spec.validate_scheduler_resources();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("test_job"));
        assert!(warnings[0].contains("runtime"));
    }

    #[test]
    fn test_validate_scheduler_resources_memory_exceeds_scheduler_mem() {
        let yaml = r#"
name: test_workflow
jobs:
  - name: test_job
    command: echo hello
    resource_requirements: big
    scheduler: my_scheduler
resource_requirements:
  - name: big
    num_cpus: 1
    memory: "16g"
    runtime: "PT30M"
slurm_schedulers:
  - name: my_scheduler
    account: test
    walltime: "01:00:00"
    mem: "8g"
"#;
        let spec: WorkflowSpec = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        let warnings = spec.validate_scheduler_resources();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("test_job"));
        assert!(warnings[0].contains("memory"));
    }

    #[test]
    fn test_validate_scheduler_resources_gpus_exceed_scheduler_gres() {
        let yaml = r#"
name: test_workflow
jobs:
  - name: test_job
    command: echo hello
    resource_requirements: gpu_heavy
    scheduler: my_scheduler
resource_requirements:
  - name: gpu_heavy
    num_cpus: 1
    num_gpus: 4
    memory: "1g"
    runtime: "PT30M"
slurm_schedulers:
  - name: my_scheduler
    account: test
    walltime: "01:00:00"
    gres: "gpu:2"
"#;
        let spec: WorkflowSpec = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        let warnings = spec.validate_scheduler_resources();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("test_job"));
        assert!(warnings[0].contains("num_gpus"));
    }

    #[test]
    fn test_validate_scheduler_resources_skips_when_scheduler_mem_not_set() {
        let yaml = r#"
name: test_workflow
jobs:
  - name: test_job
    command: echo hello
    resource_requirements: big
    scheduler: my_scheduler
resource_requirements:
  - name: big
    num_cpus: 1
    memory: "999g"
    runtime: "PT30M"
slurm_schedulers:
  - name: my_scheduler
    account: test
    walltime: "01:00:00"
"#;
        let spec: WorkflowSpec = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        // No mem set on scheduler, so memory check is skipped
        assert!(spec.validate_scheduler_resources().is_empty());
    }

    #[test]
    fn test_validate_scheduler_resources_skips_when_scheduler_gres_not_set() {
        let yaml = r#"
name: test_workflow
jobs:
  - name: test_job
    command: echo hello
    resource_requirements: gpu_heavy
    scheduler: my_scheduler
resource_requirements:
  - name: gpu_heavy
    num_cpus: 1
    num_gpus: 100
    memory: "1g"
    runtime: "PT30M"
slurm_schedulers:
  - name: my_scheduler
    account: test
    walltime: "01:00:00"
"#;
        let spec: WorkflowSpec = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        // No gres set on scheduler, so GPU check is skipped
        assert!(spec.validate_scheduler_resources().is_empty());
    }

    #[test]
    fn test_validate_scheduler_resources_multiple_warnings() {
        let yaml = r#"
name: test_workflow
jobs:
  - name: test_job
    command: echo hello
    resource_requirements: big
    scheduler: my_scheduler
resource_requirements:
  - name: big
    num_cpus: 1
    num_gpus: 4
    memory: "16g"
    runtime: "PT2H"
slurm_schedulers:
  - name: my_scheduler
    account: test
    walltime: "01:00:00"
    mem: "8g"
    gres: "gpu:2"
"#;
        let spec: WorkflowSpec = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        let warnings = spec.validate_scheduler_resources();
        assert_eq!(warnings.len(), 3); // runtime + memory + GPUs
    }

    #[test]
    fn test_validate_scheduler_resources_skips_jobs_without_resource_requirements() {
        let yaml = r#"
name: test_workflow
jobs:
  - name: test_job
    command: echo hello
slurm_schedulers:
  - name: my_scheduler
    account: test
    walltime: "00:30:00"
"#;
        let spec: WorkflowSpec = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert!(spec.validate_scheduler_resources().is_empty());
    }

    #[test]
    fn test_validate_scheduler_resources_unassigned_job_passes_if_any_scheduler_fits() {
        // Job has no explicit scheduler, one scheduler can handle it
        let yaml = r#"
name: test_workflow
jobs:
  - name: test_job
    command: echo hello
    resource_requirements: big
resource_requirements:
  - name: big
    num_cpus: 1
    memory: "4g"
    runtime: "PT2H"
slurm_schedulers:
  - name: short_scheduler
    account: test
    walltime: "01:00:00"
    mem: "8g"
  - name: long_scheduler
    account: test
    walltime: "04:00:00"
    mem: "8g"
"#;
        let spec: WorkflowSpec = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert!(spec.validate_scheduler_resources().is_empty());
    }

    #[test]
    fn test_validate_scheduler_resources_unassigned_job_fails_if_no_scheduler_fits_all_dims() {
        // Scheduler A has enough walltime but not enough memory
        // Scheduler B has enough memory but not enough walltime
        // No single scheduler satisfies all dimensions
        let yaml = r#"
name: test_workflow
jobs:
  - name: test_job
    command: echo hello
    resource_requirements: tricky
resource_requirements:
  - name: tricky
    num_cpus: 1
    memory: "16g"
    runtime: "PT2H"
slurm_schedulers:
  - name: long_but_small
    account: test
    walltime: "04:00:00"
    mem: "8g"
  - name: short_but_big
    account: test
    walltime: "01:00:00"
    mem: "32g"
"#;
        let spec: WorkflowSpec = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        let warnings = spec.validate_scheduler_resources();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("test_job"));
        assert!(warnings[0].contains("no explicit scheduler"));
    }

    #[test]
    fn test_validate_scheduler_resources_no_schedulers_returns_empty() {
        let yaml = r#"
name: test_workflow
jobs:
  - name: test_job
    command: echo hello
    resource_requirements: big
resource_requirements:
  - name: big
    num_cpus: 1
    memory: "1g"
    runtime: "PT2H"
"#;
        let spec: WorkflowSpec = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert!(spec.validate_scheduler_resources().is_empty());
    }

    #[test]
    fn test_workflow_variables_substitute_into_strings() {
        let yaml = r#"
name: vars_demo
variables:
  base_path: /scratch/proj
  image: pytorch:2.4
jobs:
  - name: train
    command: "{base_path}/run.sh --img {image}"
files:
  - name: out
    path: "{base_path}/out.txt"
"#;
        let spec = WorkflowSpec::from_spec_file_content(yaml, "yaml")
            .expect("variables substitution should succeed");
        assert_eq!(
            spec.jobs[0].command,
            "/scratch/proj/run.sh --img pytorch:2.4"
        );
        assert_eq!(
            spec.files.as_ref().unwrap()[0].path,
            "/scratch/proj/out.txt"
        );
        // The variables map itself must be preserved for round-trip serialization.
        let vars = spec.variables.expect("variables map preserved");
        assert_eq!(
            vars.get("base_path").map(String::as_str),
            Some("/scratch/proj")
        );
    }

    #[test]
    fn test_workflow_variables_combined_with_parameters() {
        let yaml = r#"
name: vars_and_params
variables:
  base_path: /scratch/proj
jobs:
  - name: "job_{i:03d}"
    command: "{base_path}/run.sh --idx {i}"
    parameters:
      i: "1:3"
"#;
        let mut spec = WorkflowSpec::from_spec_file_content(yaml, "yaml")
            .expect("variables + parameters should parse");
        spec.expand_parameters().expect("expansion should succeed");
        assert_eq!(spec.jobs.len(), 3);
        assert_eq!(spec.jobs[0].name, "job_001");
        assert_eq!(spec.jobs[0].command, "/scratch/proj/run.sh --idx 1");
        assert_eq!(spec.jobs[2].name, "job_003");
        assert_eq!(spec.jobs[2].command, "/scratch/proj/run.sh --idx 3");
    }

    #[test]
    fn test_workflow_variables_with_local_parameters_file() {
        // Regression: a spec combining workflow `variables` with a job-level
        // `parameters_file` must not reject the table's column tokens ({lr}, {tag})
        // as "undefined" during the pre-substitution validation pass.
        let dir = tempfile::tempdir().unwrap();
        let csv_path = dir.path().join("sweep.csv");
        std::fs::write(&csv_path, "lr,tag\n0.1,fast\n0.01,slow\n").unwrap();
        let yaml = format!(
            r#"
name: vars_and_table
variables:
  base: /scratch/proj
jobs:
  - name: "job_{{tag}}"
    command: "{{base}}/train.sh --lr {{lr}} --tag {{tag}}"
    parameters_file: "{path}"
"#,
            path = csv_path.display()
        );
        let mut spec = WorkflowSpec::from_spec_file_content(&yaml, "yaml")
            .expect("variables + local parameters_file should parse without undefined-token error");
        // The variable is substituted at parse time; table columns survive for expansion.
        assert_eq!(
            spec.jobs[0].command,
            "/scratch/proj/train.sh --lr {lr} --tag {tag}"
        );
        spec.expand_parameters().expect("expansion should succeed");
        assert_eq!(spec.jobs.len(), 2);
        assert_eq!(spec.jobs[0].name, "job_fast");
        assert_eq!(
            spec.jobs[0].command,
            "/scratch/proj/train.sh --lr 0.1 --tag fast"
        );
        assert_eq!(spec.jobs[1].name, "job_slow");
        assert_eq!(
            spec.jobs[1].command,
            "/scratch/proj/train.sh --lr 0.01 --tag slow"
        );
    }

    #[test]
    fn test_workflow_variables_with_shared_parameters_file() {
        // Regression: a workflow-level `parameters_file` opted into via
        // `use_parameters_file: true` must contribute its column names to the
        // valid-token set so `{region}` is not flagged as undefined.
        let dir = tempfile::tempdir().unwrap();
        let csv_path = dir.path().join("regions.csv");
        std::fs::write(&csv_path, "region\nwest\neast\n").unwrap();
        let yaml = format!(
            r#"
name: vars_and_shared_table
variables:
  base: /scratch/proj
parameters_file: "{path}"
jobs:
  - name: "job_{{region}}"
    command: "{{base}}/run.sh --region {{region}}"
    use_parameters_file: true
"#,
            path = csv_path.display()
        );
        let mut spec = WorkflowSpec::from_spec_file_content(&yaml, "yaml").expect(
            "variables + shared parameters_file should parse without undefined-token error",
        );
        spec.expand_parameters().expect("expansion should succeed");
        assert_eq!(spec.jobs.len(), 2);
        assert_eq!(spec.jobs[0].name, "job_west");
        assert_eq!(spec.jobs[0].command, "/scratch/proj/run.sh --region west");
        assert_eq!(spec.jobs[1].name, "job_east");
        assert_eq!(spec.jobs[1].command, "/scratch/proj/run.sh --region east");
    }

    #[test]
    fn test_workflow_variables_with_variable_in_parameters_file_path() {
        // Regression: the `parameters_file` path itself may reference a workflow
        // variable. The pre-substitution column scan must resolve variables in the
        // path before reading, rather than trying to open the literal
        // "{data_dir}/sweep.csv" and erroring.
        let dir = tempfile::tempdir().unwrap();
        let csv_path = dir.path().join("sweep.csv");
        std::fs::write(&csv_path, "lr\n0.1\n0.01\n").unwrap();
        let yaml = format!(
            r#"
name: var_in_path
variables:
  data_dir: "{parent}"
jobs:
  - name: "job_{{lr}}"
    command: "train.sh --lr {{lr}}"
    parameters_file: "{{data_dir}}/sweep.csv"
"#,
            parent = dir.path().display()
        );
        let mut spec = WorkflowSpec::from_spec_file_content(&yaml, "yaml")
            .expect("a variable in the parameters_file path must not cause a spurious read error");
        spec.expand_parameters().expect("expansion should succeed");
        assert_eq!(spec.jobs.len(), 2);
        assert_eq!(spec.jobs[0].name, "job_0.1");
        assert_eq!(spec.jobs[0].command, "train.sh --lr 0.1");
    }

    #[test]
    fn test_workflow_variables_with_heterogeneous_json_table() {
        // Regression: JSON parameter tables are not required to share a uniform
        // key set. A column that appears only in a later row ({extra}) must still
        // be recognized during the pre-substitution validation pass, so the union
        // of all rows' keys is collected (not just the first row's).
        let dir = tempfile::tempdir().unwrap();
        let json_path = dir.path().join("rows.json");
        std::fs::write(&json_path, r#"[{"tag": "a"}, {"tag": "b", "extra": "x"}]"#).unwrap();
        let yaml = format!(
            r#"
name: heterogeneous_table
variables:
  base: /scratch/proj
jobs:
  - name: "job_{{tag}}"
    command: "{{base}}/run.sh --tag {{tag}} --extra {{extra}}"
    parameters_file: "{path}"
"#,
            path = json_path.display()
        );
        // Without unioning every row's keys, `{extra}` would be flagged as an
        // undefined template name here.
        let spec = WorkflowSpec::from_spec_file_content(&yaml, "yaml")
            .expect("columns present only in later JSON rows must be recognized as valid tokens");
        assert_eq!(
            spec.jobs[0].command,
            "/scratch/proj/run.sh --tag {tag} --extra {extra}"
        );
    }

    #[test]
    fn test_workflow_variables_collide_with_parameter_name() {
        let yaml = r#"
name: collision
variables:
  i: not_an_index
jobs:
  - name: "job_{i}"
    command: "echo {i}"
    parameters:
      i: "1:3"
"#;
        let err = WorkflowSpec::from_spec_file_content(yaml, "yaml")
            .expect_err("collision must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("collide with parameter names"),
            "expected collision error, got: {msg}"
        );
        assert!(msg.contains('i'), "expected colliding name 'i', got: {msg}");
    }

    #[test]
    fn test_workflow_variables_undefined_token_rejected() {
        let yaml = r#"
name: typo
variables:
  base_path: /scratch/proj
jobs:
  - name: train
    command: "{baes_path}/run.sh"
"#;
        let err = WorkflowSpec::from_spec_file_content(yaml, "yaml")
            .expect_err("undefined token must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("undefined template name"),
            "expected undefined-token error, got: {msg}"
        );
        assert!(
            msg.contains("baes_path"),
            "error should name the typo: {msg}"
        );
    }

    #[test]
    fn test_workflow_variables_substitute_into_parameter_value() {
        let yaml = r#"
name: var_in_param_value
variables:
  n_max: "5"
jobs:
  - name: "job_{i}"
    command: "echo {i}"
    parameters:
      i: "1:{n_max}"
"#;
        let mut spec = WorkflowSpec::from_spec_file_content(yaml, "yaml")
            .expect("variable inside parameter value should be substituted");
        spec.expand_parameters().expect("expansion should succeed");
        assert_eq!(spec.jobs.len(), 5);
    }

    #[test]
    fn test_workflow_variables_substitute_into_env_and_scheduler() {
        let yaml = r#"
name: env_and_scheduler
variables:
  proj: my_project
env:
  PROJECT: "{proj}"
jobs:
  - name: t
    command: echo
    scheduler: "{proj}_sched"
slurm_schedulers:
  - name: "{proj}_sched"
    account: "{proj}"
    partition: short
    walltime: "PT1H"
    nodes: 1
"#;
        let spec = WorkflowSpec::from_spec_file_content(yaml, "yaml")
            .expect("variables in env and scheduler should substitute");
        assert_eq!(
            spec.env
                .as_ref()
                .and_then(|e| e.get("PROJECT"))
                .map(String::as_str),
            Some("my_project")
        );
        assert_eq!(spec.jobs[0].scheduler.as_deref(), Some("my_project_sched"));
        let sched = &spec.slurm_schedulers.as_ref().unwrap()[0];
        assert_eq!(sched.name.as_deref(), Some("my_project_sched"));
        assert_eq!(sched.account, "my_project");
    }

    #[test]
    fn test_workflow_variables_does_not_reject_shell_style_expansion() {
        // `${...}` is shell-style variable expansion (and is also used by the
        // existing `${files.input.X}` / `${TORC_*}` substitution). The
        // workflow-level variables system must leave those alone, even when
        // `variables` is set.
        let yaml = r#"
name: shell_vars_ok
variables:
  base_path: /scratch/proj
jobs:
  - name: t
    command: "echo ${TORC_JOB_ID} ${HOME} {base_path}/run.sh"
    env:
      OUT: "${TORC_JOB_ID}.log"
"#;
        let spec = WorkflowSpec::from_spec_file_content(yaml, "yaml")
            .expect("shell-style ${...} must not trigger undefined-token errors");
        assert_eq!(
            spec.jobs[0].command,
            "echo ${TORC_JOB_ID} ${HOME} /scratch/proj/run.sh"
        );
        assert_eq!(
            spec.jobs[0]
                .env
                .as_ref()
                .and_then(|e| e.get("OUT"))
                .map(String::as_str),
            Some("${TORC_JOB_ID}.log")
        );
    }

    #[test]
    fn test_workflow_variables_undefined_token_inside_variable_value() {
        // A typo in a variable's value should be rejected at load time; otherwise
        // it would silently propagate to wherever the variable is used.
        let yaml = r#"
name: typo_in_var_value
variables:
  base_path: /scratch/proj
  bad: "{baes_path}/sub"
jobs:
  - name: t
    command: "echo {bad}"
"#;
        let err = WorkflowSpec::from_spec_file_content(yaml, "yaml")
            .expect_err("typo inside a variable value must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("variable 'bad'"),
            "error should name the offending variable, got: {msg}"
        );
        assert!(
            msg.contains("baes_path"),
            "error should name the typo, got: {msg}"
        );
    }

    #[test]
    fn test_workflow_variables_substitution_preserves_shell_expansion_with_colliding_name() {
        // Even when a workflow variable's name matches a shell variable used
        // in the spec via `${...}` syntax, the substitution must leave the
        // shell expansion alone. Naive `string.replace` would corrupt
        // `${HOME}` into `$<value>`; the workflow-variables substituter is
        // `${...}`-aware to avoid that.
        let yaml = r#"
name: shell_collision
variables:
  HOME: /should/not/leak
  base: /scratch/proj
jobs:
  - name: t
    command: "echo ${HOME} {base}/run.sh"
    env:
      OUT_DIR: "${HOME}-{base}"
"#;
        let spec = WorkflowSpec::from_spec_file_content(yaml, "yaml")
            .expect("variable named HOME must not corrupt ${HOME}");
        assert_eq!(
            spec.jobs[0].command, "echo ${HOME} /scratch/proj/run.sh",
            "${{HOME}} must be preserved verbatim even though `HOME` is a workflow variable"
        );
        assert_eq!(
            spec.jobs[0]
                .env
                .as_ref()
                .and_then(|e| e.get("OUT_DIR"))
                .map(String::as_str),
            Some("${HOME}-/scratch/proj"),
            "${{HOME}} in env must also be preserved while {{base}} is substituted"
        );
    }

    #[test]
    fn test_workflow_variables_value_with_parameter_reference_rejected() {
        // Variable values must be plain literal strings; even a
        // valid-looking `{i}` referencing a parameter is rejected so the rule
        // stays uniform. Composition belongs at the use site.
        let yaml = r#"
name: param_ref_in_var_value
variables:
  output_pattern: "results-{i}.json"
jobs:
  - name: "job_{i}"
    command: "echo {output_pattern}"
    parameters:
      i: "1:3"
"#;
        let err = WorkflowSpec::from_spec_file_content(yaml, "yaml")
            .expect_err("parameter reference inside a variable value must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("must be plain literal strings"),
            "expected literal-only error, got: {msg}"
        );
        assert!(
            msg.contains("variable 'output_pattern'"),
            "error should name the offending variable, got: {msg}"
        );
    }

    #[test]
    fn test_workflow_variables_value_referencing_another_variable_rejected() {
        // Variable values must not reference other variables: HashMap iteration
        // order would otherwise determine whether the inner reference resolves
        // or leaks through as a literal token.
        let yaml = r#"
name: nested_vars_rejected
variables:
  base: /scratch
  inputs: "{base}/inputs"
jobs:
  - name: t
    command: "ls {inputs}"
"#;
        let err = WorkflowSpec::from_spec_file_content(yaml, "yaml")
            .expect_err("variable referencing another variable must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("references another variable"),
            "expected explicit cross-variable error, got: {msg}"
        );
        assert!(
            msg.contains("variable 'inputs'") && msg.contains("{base}"),
            "error should name both the variable and its bad reference, got: {msg}"
        );
    }

    #[test]
    fn test_workflow_variables_invalid_name_rejected() {
        // Variable names must be valid identifiers so they participate in typo
        // detection and serialize cleanly to KDL.
        let yaml = r#"
name: bad_var_name
variables:
  "foo.bar": x
jobs:
  - name: t
    command: echo
"#;
        let err = WorkflowSpec::from_spec_file_content(yaml, "yaml")
            .expect_err("non-identifier variable name must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("must be a valid identifier"),
            "expected identifier-validation error, got: {msg}"
        );
        assert!(
            msg.contains("foo.bar"),
            "error should name the offending name, got: {msg}"
        );
    }

    #[test]
    fn test_workflow_variables_round_trip_json() {
        let yaml = r#"
name: roundtrip
variables:
  base: /a/b
jobs:
  - name: t
    command: "{base}/run"
"#;
        let spec = WorkflowSpec::from_spec_file_content(yaml, "yaml").unwrap();
        let json = serde_json::to_string(&spec).unwrap();
        // Reparsing the serialized form must succeed (substitutions are baked in,
        // so the variables map is harmless on a second pass).
        let spec2 = WorkflowSpec::from_spec_file_content(&json, "json").unwrap();
        assert_eq!(spec.jobs[0].command, spec2.jobs[0].command);
        assert_eq!(spec.variables, spec2.variables);
    }

    fn assert_variables_demo_substituted(spec: &WorkflowSpec) {
        assert_eq!(spec.name, "variables_demo");
        let prepare = spec
            .jobs
            .iter()
            .find(|j| j.name == "prepare_inputs")
            .expect("prepare_inputs job present");
        assert_eq!(
            prepare.command,
            "python prepare.py --in /scratch/proj42/raw --out /scratch/proj42/clean"
        );
        let train = spec
            .jobs
            .iter()
            .find(|j| j.name.starts_with("train_"))
            .expect("train job template present");
        assert!(
            train.command.contains("--img pytorch:2.4"),
            "image_tag should be substituted, got: {}",
            train.command
        );
        assert!(
            train.command.contains("--in /scratch/proj42/clean"),
            "data_root should be substituted, got: {}",
            train.command
        );
        let aggregate = spec
            .jobs
            .iter()
            .find(|j| j.name == "aggregate")
            .expect("aggregate job present");
        assert_eq!(
            aggregate.command,
            "python aggregate.py --in /shared/proj42/results --tag proj42"
        );
        let env = spec.env.as_ref().expect("env block present");
        assert_eq!(env.get("PROJECT").map(String::as_str), Some("proj42"));
        assert_eq!(env.get("IMAGE").map(String::as_str), Some("pytorch:2.4"));
        let schedulers = spec
            .slurm_schedulers
            .as_ref()
            .expect("slurm_schedulers present");
        for sched in schedulers {
            assert_eq!(
                sched.account, "my_hpc_account",
                "scheduler account should be substituted, got: {}",
                sched.account
            );
        }
    }

    #[test]
    fn test_variables_demo_yaml_example() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/yaml/variables_demo.yaml");
        let mut spec =
            WorkflowSpec::from_spec_file(&path).expect("YAML variables_demo should parse");
        spec.expand_parameters()
            .expect("YAML variables_demo should expand");
        // 3 jobs declared; train_{i:02d} expands i=1..=4, plus prepare and aggregate -> 6 total.
        assert_eq!(spec.jobs.len(), 6);
        assert_variables_demo_substituted(&spec);
    }

    #[test]
    fn test_variables_demo_json_example() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/json/variables_demo.json");
        let mut spec =
            WorkflowSpec::from_spec_file(&path).expect("JSON variables_demo should parse");
        spec.expand_parameters()
            .expect("JSON variables_demo should expand");
        assert_eq!(spec.jobs.len(), 6);
        assert_variables_demo_substituted(&spec);
    }

    #[test]
    fn test_variables_demo_json5_example() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/json/variables_demo.json5");
        let mut spec =
            WorkflowSpec::from_spec_file(&path).expect("JSON5 variables_demo should parse");
        spec.expand_parameters()
            .expect("JSON5 variables_demo should expand");
        assert_eq!(spec.jobs.len(), 6);
        assert_variables_demo_substituted(&spec);
    }

    #[test]
    fn test_variables_demo_kdl_example() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/kdl/variables_demo.kdl");
        let mut spec =
            WorkflowSpec::from_spec_file(&path).expect("KDL variables_demo should parse");
        spec.expand_parameters()
            .expect("KDL variables_demo should expand");
        assert_eq!(spec.jobs.len(), 6);
        assert_variables_demo_substituted(&spec);
    }

    /// Build a minimal spec carrying a single action, for `validate_actions` tests.
    fn spec_with_action(action: WorkflowActionSpec) -> WorkflowSpec {
        let mut spec = WorkflowSpec::new(
            "wf".to_string(),
            "user".to_string(),
            None,
            vec![JobSpec::new("job1".to_string(), "exit 0".to_string())],
        );
        spec.actions = Some(vec![action]);
        spec
    }

    /// Build a `run_commands` action with the given trigger and optional gating jobs.
    fn run_commands_action(trigger_type: &str, jobs: Option<Vec<String>>) -> WorkflowActionSpec {
        WorkflowActionSpec {
            trigger_type: trigger_type.to_string(),
            action_type: "run_commands".to_string(),
            jobs,
            job_name_regexes: None,
            commands: Some(vec!["echo hi".to_string()]),
            scheduler: None,
            scheduler_type: None,
            num_allocations: None,
            start_one_worker_per_node: None,
            max_parallel_jobs: None,
            persistent: None,
        }
    }

    #[test]
    fn test_validate_actions_rejects_unknown_trigger_type() {
        // `on_ready_jobs` is a transposed typo of `on_jobs_ready`.
        let spec = spec_with_action(run_commands_action("on_ready_jobs", None));
        let err = spec
            .validate_actions()
            .expect_err("unknown trigger should fail");
        let msg = err.to_string();
        assert!(msg.contains("unknown trigger_type"), "got: {msg}");
        assert!(msg.contains("on_ready_jobs"), "got: {msg}");
    }

    #[test]
    fn test_validate_actions_accepts_known_trigger_types() {
        for trigger in WorkflowSpec::VALID_TRIGGER_TYPES {
            // Job-gated triggers need a job; supply one unconditionally.
            let spec =
                spec_with_action(run_commands_action(trigger, Some(vec!["job1".to_string()])));
            spec.validate_actions()
                .unwrap_or_else(|e| panic!("trigger '{trigger}' should be valid: {e}"));
        }
    }

    #[test]
    fn test_validate_actions_rejects_job_gated_trigger_without_jobs() {
        for trigger in ["on_jobs_ready", "on_jobs_complete"] {
            let spec = spec_with_action(run_commands_action(trigger, None));
            let err = spec
                .validate_actions()
                .expect_err("job-gated trigger without jobs should fail");
            let msg = err.to_string();
            assert!(msg.contains("must specify at least one job"), "got: {msg}");
        }
    }

    #[test]
    fn test_validate_actions_accepts_job_gated_trigger_with_regexes() {
        let mut action = run_commands_action("on_jobs_ready", None);
        action.job_name_regexes = Some(vec!["^job.*$".to_string()]);
        let spec = spec_with_action(action);
        spec.validate_actions()
            .expect("regex-gated on_jobs_ready should be valid");
    }

    #[test]
    fn test_validate_actions_rejects_job_gated_trigger_with_empty_jobs() {
        // An explicitly-empty `jobs` list is as useless as omitting it.
        let spec = spec_with_action(run_commands_action("on_jobs_complete", Some(vec![])));
        let err = spec
            .validate_actions()
            .expect_err("empty jobs list should fail");
        assert!(err.to_string().contains("must specify at least one job"));
    }
}
