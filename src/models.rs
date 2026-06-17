//! Hand-owned API models for the code-first OpenAPI migration.
//!
//! These models are introduced one resource group at a time and provide a stable place for
//! schema derives and conversions away from generated Rust types.

#![allow(clippy::new_without_default, clippy::too_many_arguments)]

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Returns true when `name` is a valid POSIX-style environment variable name:
/// starts with a letter or underscore, followed by letters, digits, or underscores.
pub fn is_valid_env_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first == '_' || first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

const fn default_trigger_count() -> i64 {
    0
}

const fn default_required_triggers() -> i64 {
    1
}

const fn default_false() -> bool {
    false
}

const fn default_num_cpus() -> i64 {
    1
}

const fn default_num_gpus() -> i64 {
    0
}

const fn default_num_nodes() -> i64 {
    1
}

fn default_memory() -> String {
    "1m".to_string()
}

fn default_runtime() -> String {
    "PT1M".to_string()
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "lowercase")]
pub enum EventSeverity {
    Debug,
    #[default]
    Info,
    Warning,
    Error,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComputeNodeSchedule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_parallel_jobs: Option<i64>,
    pub num_jobs: i64,
    pub scheduler_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_one_worker_per_node: Option<bool>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: Value,
    #[serde(rename = "errorNum", skip_serializing_if = "Option::is_none")]
    pub error_num: Option<i64>,
    #[serde(rename = "errorMessage", skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i64>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PingResponse {
    pub status: String,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VersionResponse {
    pub version: String,
    pub api_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_hash: Option<String>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComputeNodeModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub workflow_id: i64,
    pub hostname: String,
    pub pid: i64,
    pub start_time: String,
    /// Allocation end time (RFC3339), reported by the runner at registration.
    /// Used to compute remaining walltime for active nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
    pub num_cpus: i64,
    pub memory_gb: f64,
    pub num_gpus: i64,
    pub num_nodes: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler_config_id: Option<i64>,
    pub compute_node_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_cpu_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_cpu_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_memory_bytes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_memory_bytes: Option<i64>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListComputeNodesResponse {
    pub items: Vec<ComputeNodeModel>,
    pub offset: i64,
    pub max_limit: i64,
    pub count: i64,
    pub total_count: i64,
    pub has_more: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteCountResponse {
    pub count: i64,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub workflow_id: i64,
    pub timestamp: i64,
    pub data: Value,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListEventsResponse {
    pub items: Vec<EventModel>,
    pub offset: i64,
    pub max_limit: i64,
    pub count: i64,
    pub total_count: i64,
    pub has_more: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub workflow_id: i64,
    pub name: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub st_mtime: Option<f64>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListFilesResponse {
    pub items: Vec<FileModel>,
    pub offset: i64,
    pub max_limit: i64,
    pub count: i64,
    pub total_count: i64,
    pub has_more: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserDataModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub workflow_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_ephemeral: Option<bool>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListUserDataResponse {
    pub items: Vec<UserDataModel>,
    pub offset: i64,
    pub max_limit: i64,
    pub count: i64,
    pub total_count: i64,
    pub has_more: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub workflow_id: i64,
    pub name: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invocation_script: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<JobStatus>,
    /// Timestamp when the current attempt began running. Set by start_job and
    /// cleared by complete_job and the reset/retry paths. NULL when the job is
    /// not running (use `status` as the source of truth for "is running").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    /// Compute node executing the current attempt. Set by start_job and cleared
    /// by complete_job and the reset/retry paths. For completed attempts, the
    /// compute node is recorded on the result record.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_node_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule_compute_nodes: Option<ComputeNodeSchedule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel_on_blocking_job_failure: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_termination: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on_job_ids: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_file_ids: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_file_ids: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_user_data_ids: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_user_data_ids: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_requirements_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_handler_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempt_id: Option<i64>,
    /// Scheduling priority; higher values are submitted first. Minimum 0, default 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "openapi-codegen", schema(minimum = 0, default = 0))]
    pub priority: Option<i64>,
    /// Provenance marker: NULL for jobs declared at workflow creation,
    /// `"retry"` for jobs resurrected by failure-handler retries,
    /// `"spawn"` for jobs added at runtime by `spawn_jobs`. `torc watch
    /// --auto-schedule` uses this to detect jobs that need unplanned Slurm
    /// allocations (deferred `schedule_nodes` actions only account for the
    /// originally-declared workload).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListJobsResponse {
    pub items: Vec<JobModel>,
    pub offset: i64,
    pub max_limit: i64,
    pub count: i64,
    pub total_count: i64,
    pub has_more: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Uninitialized,
    Blocked,
    Ready,
    Pending,
    Running,
    Completed,
    Failed,
    Canceled,
    Terminated,
    Disabled,
    PendingFailed,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub job_id: i64,
    pub workflow_id: i64,
    pub run_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempt_id: Option<i64>,
    pub compute_node_id: i64,
    pub return_code: i64,
    pub exec_time_minutes: f64,
    pub completion_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_memory_bytes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_memory_bytes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_cpu_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_cpu_percent: Option<f64>,
    pub status: JobStatus,
    /// Name of the job this result belongs to. Populated by the server on read
    /// paths (list/get) as a convenience so clients need not re-fetch jobs; it
    /// is ignored on create/update input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_name: Option<String>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListResultsResponse {
    pub items: Vec<ResultModel>,
    pub offset: i64,
    pub max_limit: i64,
    pub count: i64,
    pub total_count: i64,
    pub has_more: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobCompletionEntry {
    pub job_id: i64,
    pub status: JobStatus,
    pub run_id: i64,
    pub result: ResultModel,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchCompleteJobsRequest {
    pub completions: Vec<JobCompletionEntry>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobCompletionError {
    pub job_id: i64,
    pub message: String,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchCompleteJobsResponse {
    pub completed: Vec<i64>,
    pub errors: Vec<JobCompletionError>,
}

/// One job to add atomically as part of `spawn_jobs`.
///
/// `depends_on` entries may reference jobs that already exist in the workflow
/// or sibling jobs created in the same request (resolved by name within the
/// transaction). Every spawned job is created `blocked` — the server auto-
/// injects a dependency edge to the calling job in addition to any explicit
/// `depends_on`, so spawned jobs are promoted by the normal background
/// unblock path once the caller (and any explicit deps) become terminal.
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnJobModel {
    pub name: String,
    pub command: String,
    /// Name of an existing resource_requirements record in the workflow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_requirements: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel_on_blocking_job_failure: Option<bool>,
    /// Job names this job depends on (existing jobs or siblings in this batch).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
}

/// Add a batch of new jobs to an initialized workflow, all blocked on the
/// calling job. The calling job is **not** completed by this call — the
/// orchestrator script exits normally and the runner completes it, at which
/// point the unblock cascade promotes the spawned jobs.
///
/// The per-lineage spawn-iteration counter is advanced and an opaque state
/// payload is persisted, all in the same transaction as the inserts.
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnJobsRequest {
    /// Orchestrator lineage identifier. Defaults to the calling job's name.
    /// The per-lineage spawn counter and state records are keyed on this.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lineage: Option<String>,
    /// Jobs to add. May be empty (record final state without spawning).
    pub jobs: Vec<SpawnJobModel>,
    /// Opaque JSON state attached to this generation (or as the converged
    /// final state when `jobs` is empty).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Value>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpawnJobsResponse {
    /// IDs of the spawned jobs. On a fresh call this is the IDs of the newly
    /// inserted jobs; on an idempotent replay (same names already exist) it
    /// is the IDs of those pre-existing jobs in the order they appear in the
    /// request. Empty only when the request's `jobs` array is empty (e.g. a
    /// final-state convergence call).
    pub spawned_job_ids: Vec<i64>,
    /// This lineage's spawn-iteration counter after the call.
    pub iteration: i64,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduledComputeNodesModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub workflow_id: i64,
    pub scheduler_id: i64,
    pub scheduler_config_id: i64,
    pub scheduler_type: String,
    pub status: String,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListScheduledComputeNodesResponse {
    pub items: Vec<ScheduledComputeNodesModel>,
    pub offset: i64,
    pub max_limit: i64,
    pub count: i64,
    pub total_count: i64,
    pub has_more: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalSchedulerModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub workflow_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_cpus: Option<i64>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListLocalSchedulersResponse {
    pub items: Vec<LocalSchedulerModel>,
    pub offset: i64,
    pub max_limit: i64,
    pub count: i64,
    pub total_count: i64,
    pub has_more: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlurmSchedulerModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub workflow_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub account: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gres: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mem: Option<String>,
    pub nodes: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ntasks_per_node: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qos: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmp: Option<String>,
    pub walltime: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<String>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListSlurmSchedulersResponse {
    pub items: Vec<SlurmSchedulerModel>,
    pub offset: i64,
    pub max_limit: i64,
    pub count: i64,
    pub total_count: i64,
    pub has_more: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub name: String,
    pub user: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_node_expiration_buffer_seconds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_node_wait_for_new_jobs_seconds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_node_ignore_workflow_completion: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_node_wait_for_healthy_database_minutes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_node_min_time_for_new_jobs_seconds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_monitor_config: Option<ResourceMonitorConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slurm_defaults: Option<HashMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_pending_failed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_ro_crate: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_config: Option<ExecutionConfig>,
    /// Current run number; incremented on each restart/recovery.
    /// Read-only on the API: incremented as a side effect of
    /// `POST /workflows/{id}/reset_status`. Values supplied to
    /// create/update workflow endpoints are ignored.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "openapi-codegen", schema(read_only))]
    pub run_id: Option<i64>,
    /// True when a user (or scheduler) has canceled the workflow.
    /// Read-only on the API: set via `POST /workflows/{id}/cancel`,
    /// cleared via `POST /workflows/{id}/reset_status`. Values supplied to
    /// create/update workflow endpoints are ignored.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "openapi-codegen", schema(read_only))]
    pub is_canceled: Option<bool>,
    /// True when the workflow has been archived.
    /// Read-only on the API: set via `POST /workflows/{id}/archive`,
    /// cleared via `POST /workflows/{id}/reset_status`. Values supplied to
    /// create/update workflow endpoints are ignored.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "openapi-codegen", schema(read_only))]
    pub is_archived: Option<bool>,
    /// Dynamic job spawning configuration. Mirrors the workflow-spec
    /// `dynamic_jobs` section identically. Runtime-immutable after
    /// workflow creation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_jobs: Option<DynamicJobsConfig>,
    /// Access group names granted shared access to this workflow.
    /// Projection of the `workflow_access_group` join table (which remains the
    /// source of truth). On create, names are resolved to group IDs and join
    /// rows are inserted in the same transaction as the workflow row; an
    /// unknown name fails the whole create. On read, populated from the join
    /// table. Use `add_workflow_to_group` / `remove_workflow_from_group` for
    /// post-creation changes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_groups: Option<Vec<String>>,
    /// Absolute directory the workflow was originally submitted from
    /// (captured at `torc create` / `torc run` / `torc submit` time). Exposed
    /// to jobs as `TORC_WORKFLOW_SUBMISSION_DIR` so user code with relative
    /// paths can resolve against the original CWD even when run on a compute
    /// node. Set once at workflow creation and not overwritten by later
    /// `schedule-nodes`/`watch` invocations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submission_directory: Option<String>,
}

/// Dynamic job spawning configuration. Used both as the user-authored
/// `WorkflowSpec.dynamic_jobs` and as the persisted `WorkflowModel.dynamic_jobs`
/// (stored as JSON in the `workflow.dynamic_jobs` column). Runtime-immutable.
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicJobsConfig {
    /// Cap on `spawn_jobs` calls per orchestrator lineage. `None` applies
    /// the server default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_iterations: Option<i64>,
}

/// How to capture stdout and stderr for job processes.
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StdioMode {
    /// Separate stdout and stderr files (.o and .e) — the default.
    #[default]
    Separate,
    /// Combine stdout and stderr into a single file (.log) per job.
    Combined,
    /// Don't capture stdout (send to /dev/null); capture stderr only.
    NoStdout,
    /// Don't capture stderr (send to /dev/null); capture stdout only.
    NoStderr,
    /// Don't capture either stdout or stderr.
    None,
}

/// Configuration for job stdout/stderr capture.
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct StdioConfig {
    /// How to capture stdout/stderr. Default: separate files.
    #[serde(default)]
    pub mode: StdioMode,
    /// Delete stdout/stderr files if the job completes successfully.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete_on_success: Option<bool>,
}

/// Execution mode for job processes.
///
/// Controls how torc manages job processes during execution, particularly
/// around termination and resource enforcement.
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    /// Direct shell execution - torc manages termination via SIGTERM/SIGKILL.
    /// This is the default mode. Works everywhere: local machines, cloud VMs,
    /// containers, and inside Slurm allocations.
    #[default]
    Direct,
    /// Slurm srun execution - Slurm manages resource limits and termination.
    /// Jobs are wrapped with `srun` inside Slurm allocations.
    Slurm,
    /// Auto-detect based on environment - uses `slurm` if SLURM_JOB_ID is set,
    /// otherwise `direct`.
    Auto,
}

/// Unified execution configuration that controls how jobs are run.
///
/// Used both as the user-authored `WorkflowSpec.execution_config` and as the
/// persisted `WorkflowModel.execution_config` (stored as JSON in the
/// `workflow.execution_config` column).
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ExecutionConfig {
    /// Execution mode: direct (default), slurm, or auto.
    #[serde(default)]
    pub mode: ExecutionMode,

    /// Seconds before end_time to send SIGKILL (direct mode) or set srun --time (slurm mode).
    /// Default: 60.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sigkill_headroom_seconds: Option<i64>,

    /// Exit code to use when a job times out.
    /// Default: 152 (matches Slurm's TIMEOUT exit code).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_exit_code: Option<i32>,

    /// Enable staggered startup for job runners to mitigate thundering herd.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staggered_start: Option<bool>,

    /// When true (default), monitor memory/CPU usage and kill jobs that exceed
    /// their resource requirements (OOM enforcement). Only applies in direct mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_resources: Option<bool>,

    /// Signal to send before SIGKILL for graceful termination (direct mode only).
    /// Default: "SIGTERM".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub termination_signal: Option<String>,

    /// Seconds before SIGKILL to send the termination signal (direct mode only).
    /// Default: 30.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sigterm_lead_seconds: Option<i64>,

    /// Exit code to use when a job is OOM-killed (direct mode only).
    /// Default: 137 (128 + SIGKILL = 128 + 9).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oom_exit_code: Option<i32>,

    /// Signal specification for srun steps, passed as `srun --signal=<value>` (slurm mode only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub srun_termination_signal: Option<String>,

    /// MPI launcher mode for the outer `srun` used to launch one job runner per allocated node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub srun_mpi: Option<String>,

    /// When true, allow Slurm to bind tasks to specific CPU cores (slurm mode only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_cpu_bind: Option<bool>,

    /// Workflow-level default for stdout/stderr capture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdio: Option<StdioConfig>,

    /// Per-job stdio overrides keyed by job name.
    /// Populated during workflow creation from per-job `stdio` fields in the spec.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_stdio_overrides: Option<HashMap<String, StdioConfig>>,
}

impl ExecutionConfig {
    /// Default value for sigterm_lead_seconds (30 seconds)
    pub const DEFAULT_SIGTERM_LEAD_SECONDS: i64 = 30;
    /// Default value for sigkill_headroom_seconds (60 seconds)
    pub const DEFAULT_SIGKILL_HEADROOM_SECONDS: i64 = 60;
    /// Default exit code for timeout (matches Slurm's TIMEOUT)
    pub const DEFAULT_TIMEOUT_EXIT_CODE: i32 = 152;
    /// Default exit code for OOM kill (128 + SIGKILL)
    pub const DEFAULT_OOM_EXIT_CODE: i32 = 137;

    /// Resolve the effective execution mode based on the configured mode and environment.
    pub fn effective_mode(&self) -> ExecutionMode {
        match self.mode {
            ExecutionMode::Direct => ExecutionMode::Direct,
            ExecutionMode::Slurm => ExecutionMode::Slurm,
            ExecutionMode::Auto => {
                if std::env::var("SLURM_JOB_ID").is_ok() {
                    ExecutionMode::Slurm
                } else {
                    ExecutionMode::Direct
                }
            }
        }
    }

    /// Whether to use srun wrapping (true for effective Slurm mode).
    pub fn use_srun(&self) -> bool {
        matches!(self.effective_mode(), ExecutionMode::Slurm)
    }

    /// Whether to enforce resource limits.
    pub fn limit_resources(&self) -> bool {
        self.limit_resources.unwrap_or(true)
    }

    /// Whether to enable CPU binding in Slurm mode.
    pub fn enable_cpu_bind(&self) -> bool {
        self.enable_cpu_bind.unwrap_or(false)
    }

    /// Get the termination signal name for direct mode.
    pub fn termination_signal(&self) -> &str {
        self.termination_signal.as_deref().unwrap_or("SIGTERM")
    }

    /// Get the sigterm lead time in seconds.
    pub fn sigterm_lead_seconds(&self) -> i64 {
        self.sigterm_lead_seconds
            .unwrap_or(Self::DEFAULT_SIGTERM_LEAD_SECONDS)
    }

    /// Get the sigkill headroom time in seconds.
    pub fn sigkill_headroom_seconds(&self) -> i64 {
        self.sigkill_headroom_seconds
            .unwrap_or(Self::DEFAULT_SIGKILL_HEADROOM_SECONDS)
    }

    /// Get the timeout exit code.
    pub fn timeout_exit_code(&self) -> i32 {
        self.timeout_exit_code
            .unwrap_or(Self::DEFAULT_TIMEOUT_EXIT_CODE)
    }

    /// Get the OOM exit code.
    pub fn oom_exit_code(&self) -> i32 {
        self.oom_exit_code.unwrap_or(Self::DEFAULT_OOM_EXIT_CODE)
    }

    /// Whether staggered startup is enabled for Slurm job runners.
    pub fn staggered_start(&self) -> bool {
        self.staggered_start.unwrap_or(true)
    }

    /// Resolve the effective `StdioConfig` for a job, checking per-job overrides first.
    pub fn stdio_for_job(&self, job_name: &str) -> StdioConfig {
        if let Some(ref overrides) = self.job_stdio_overrides
            && let Some(cfg) = overrides.get(job_name)
        {
            return cfg.clone();
        }
        self.stdio.clone().unwrap_or_default()
    }

    /// Whether to delete stdio files on successful completion for a job.
    pub fn delete_stdio_on_success(&self, job_name: &str) -> bool {
        self.stdio_for_job(job_name)
            .delete_on_success
            .unwrap_or(false)
    }

    /// Build from a WorkflowModel's execution_config field.
    pub fn from_workflow_model(workflow: &WorkflowModel) -> ExecutionConfig {
        workflow.execution_config.clone().unwrap_or_default()
    }
}

/// Granularity for resource monitoring sampling.
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MonitorGranularity {
    #[default]
    Summary,
    TimeSeries,
}

/// Configuration for per-job resource monitoring.
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobMonitorConfig {
    pub enabled: bool,
    pub granularity: MonitorGranularity,
}

impl Default for JobMonitorConfig {
    fn default() -> Self {
        JobMonitorConfig {
            enabled: false,
            granularity: MonitorGranularity::Summary,
        }
    }
}

/// Configuration for compute-node resource monitoring.
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ComputeNodeMonitorConfig {
    pub enabled: bool,
    pub granularity: MonitorGranularity,
    pub cpu: bool,
    pub memory: bool,
}

impl Default for ComputeNodeMonitorConfig {
    fn default() -> Self {
        ComputeNodeMonitorConfig {
            enabled: false,
            granularity: MonitorGranularity::Summary,
            cpu: true,
            memory: true,
        }
    }
}

/// Configuration for resource monitoring.
///
/// Used both as the user-authored `WorkflowSpec.resource_monitor` and as the
/// persisted `WorkflowModel.resource_monitor_config` (stored as JSON in the
/// `workflow.resource_monitor_config` column).
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ResourceMonitorConfig {
    /// Deprecated compatibility field. Use `jobs.enabled` for new workflow specs.
    pub enabled: bool,
    /// Deprecated compatibility field. Use `jobs.granularity` for new workflow specs.
    pub granularity: MonitorGranularity,
    pub sample_interval_seconds: i32,
    /// How often buffered time-series samples are flushed to SQLite, in seconds.
    pub flush_interval_seconds: i32,
    pub generate_plots: bool,
    pub jobs: Option<JobMonitorConfig>,
    pub compute_node: Option<ComputeNodeMonitorConfig>,
}

impl Default for ResourceMonitorConfig {
    fn default() -> Self {
        ResourceMonitorConfig {
            enabled: false,
            granularity: MonitorGranularity::Summary,
            sample_interval_seconds: 10,
            flush_interval_seconds: 300,
            generate_plots: false,
            jobs: None,
            compute_node: None,
        }
    }
}

impl ResourceMonitorConfig {
    pub fn jobs_config(&self) -> JobMonitorConfig {
        self.jobs.clone().unwrap_or(JobMonitorConfig {
            enabled: self.enabled,
            granularity: self.granularity.clone(),
        })
    }

    pub fn compute_node_config(&self) -> Option<ComputeNodeMonitorConfig> {
        self.compute_node.clone().filter(|config| config.enabled)
    }

    pub fn is_enabled(&self) -> bool {
        self.jobs_config().enabled || self.compute_node_config().is_some()
    }

    /// Returns true if any enabled scope uses time-series granularity.
    pub fn has_timeseries_db(&self) -> bool {
        let jobs_ts = {
            let jobs = self.jobs_config();
            jobs.enabled && matches!(jobs.granularity, MonitorGranularity::TimeSeries)
        };
        let node_ts = self
            .compute_node_config()
            .is_some_and(|c| matches!(c.granularity, MonitorGranularity::TimeSeries));
        jobs_ts || node_ts
    }
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListWorkflowsResponse {
    pub items: Vec<WorkflowModel>,
    pub offset: i64,
    pub max_limit: i64,
    pub count: i64,
    pub total_count: i64,
    pub has_more: bool,
}

/// Request body for `POST /workflows/{id}/archive`. Setting `is_archived`
/// to true marks the workflow as archived; false unarchives it.
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveWorkflowRequest {
    pub is_archived: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComputeNodesResources {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub num_cpus: i64,
    pub memory_gb: f64,
    pub num_gpus: i64,
    pub num_nodes: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler_config_id: Option<i64>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaimJobsBasedOnResources {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jobs: Option<Vec<JobModel>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaimNextJobsResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jobs: Option<Vec<JobModel>>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobDependencyModel {
    pub job_id: i64,
    pub job_name: String,
    pub depends_on_job_id: i64,
    pub depends_on_job_name: String,
    pub workflow_id: i64,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListJobDependenciesResponse {
    pub items: Vec<JobDependencyModel>,
    pub offset: i64,
    pub max_limit: i64,
    pub count: i64,
    pub total_count: i64,
    pub has_more: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobFileRelationshipModel {
    pub file_id: i64,
    pub file_name: String,
    pub file_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub producer_job_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub producer_job_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumer_job_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumer_job_name: Option<String>,
    pub workflow_id: i64,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListJobFileRelationshipsResponse {
    pub items: Vec<JobFileRelationshipModel>,
    pub offset: i64,
    pub max_limit: i64,
    pub count: i64,
    pub total_count: i64,
    pub has_more: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobUserDataRelationshipModel {
    pub user_data_id: i64,
    pub user_data_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub producer_job_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub producer_job_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumer_job_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumer_job_name: Option<String>,
    pub workflow_id: i64,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListJobUserDataRelationshipsResponse {
    pub items: Vec<JobUserDataRelationshipModel>,
    pub offset: i64,
    pub max_limit: i64,
    pub count: i64,
    pub total_count: i64,
    pub has_more: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListJobIdsResponse {
    pub job_ids: Vec<i64>,
    pub count: i64,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListMissingUserDataResponse {
    pub user_data: Vec<i64>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessChangedJobInputsResponse {
    pub reinitialized_jobs: Vec<String>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetReadyJobRequirementsResponse {
    pub num_jobs: i64,
    pub num_cpus: i64,
    pub num_gpus: i64,
    pub memory_gb: f64,
    pub max_num_nodes: i64,
    pub max_runtime: String,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListRequiredExistingFilesResponse {
    pub files: Vec<i64>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccessGroupModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserGroupMembershipModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub user_name: String,
    pub group_id: i64,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowAccessGroupModel {
    pub workflow_id: i64,
    pub group_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListAccessGroupsResponse {
    pub items: Vec<AccessGroupModel>,
    pub offset: i64,
    pub limit: i64,
    pub total_count: i64,
    pub has_more: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListUserGroupMembershipsResponse {
    pub items: Vec<UserGroupMembershipModel>,
    pub offset: i64,
    pub limit: i64,
    pub total_count: i64,
    pub has_more: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccessCheckResponse {
    pub has_access: bool,
    pub user_name: String,
    pub workflow_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobsModel {
    pub jobs: Vec<JobModel>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateJobsResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jobs: Option<Vec<JobModel>>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilesModel {
    pub files: Vec<FileModel>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateFilesResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileModel>>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserDataListModel {
    pub user_data: Vec<UserDataModel>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateUserDataListResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_data: Option<Vec<UserDataModel>>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceRequirementsModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub workflow_id: i64,
    pub name: String,
    #[serde(default = "default_num_cpus")]
    #[cfg_attr(feature = "openapi-codegen", schema(required = false, default = 1))]
    pub num_cpus: i64,
    #[serde(default = "default_num_gpus")]
    #[cfg_attr(feature = "openapi-codegen", schema(required = false, default = 0))]
    pub num_gpus: i64,
    #[serde(default = "default_num_nodes")]
    #[cfg_attr(feature = "openapi-codegen", schema(required = false, default = 1))]
    pub num_nodes: i64,
    #[serde(default = "default_memory")]
    #[cfg_attr(feature = "openapi-codegen", schema(required = false, default = "1m"))]
    pub memory: String,
    #[serde(default = "default_runtime")]
    #[cfg_attr(
        feature = "openapi-codegen",
        schema(required = false, default = "PT1M")
    )]
    pub runtime: String,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListResourceRequirementsResponse {
    pub items: Vec<ResourceRequirementsModel>,
    pub offset: i64,
    pub max_limit: i64,
    pub count: i64,
    pub total_count: i64,
    pub has_more: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureHandlerModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub workflow_id: i64,
    pub name: String,
    pub rules: String,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListFailureHandlersResponse {
    pub items: Vec<FailureHandlerModel>,
    pub offset: i64,
    pub max_limit: i64,
    pub count: i64,
    pub total_count: i64,
    pub has_more: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlurmStatsModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub workflow_id: i64,
    pub job_id: i64,
    pub run_id: i64,
    pub attempt_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slurm_job_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_rss_bytes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_vm_size_bytes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_disk_read_bytes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_disk_write_bytes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ave_cpu_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_list: Option<String>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListSlurmStatsResponse {
    pub items: Vec<SlurmStatsModel>,
    pub offset: i64,
    pub max_limit: i64,
    pub count: i64,
    pub total_count: i64,
    pub has_more: bool,
}

pub struct JobStatusMap;

impl JobStatusMap {
    pub fn enum_to_int_map() -> &'static HashMap<JobStatus, i32> {
        static MAP: OnceLock<HashMap<JobStatus, i32>> = OnceLock::new();
        MAP.get_or_init(|| {
            let mut map = HashMap::new();
            map.insert(JobStatus::Uninitialized, 0);
            map.insert(JobStatus::Blocked, 1);
            map.insert(JobStatus::Ready, 2);
            map.insert(JobStatus::Pending, 3);
            map.insert(JobStatus::Running, 4);
            map.insert(JobStatus::Completed, 5);
            map.insert(JobStatus::Failed, 6);
            map.insert(JobStatus::Canceled, 7);
            map.insert(JobStatus::Terminated, 8);
            map.insert(JobStatus::Disabled, 9);
            map.insert(JobStatus::PendingFailed, 10);
            map
        })
    }

    pub fn int_to_enum_map() -> &'static HashMap<i32, JobStatus> {
        static MAP: OnceLock<HashMap<i32, JobStatus>> = OnceLock::new();
        MAP.get_or_init(|| {
            let mut map = HashMap::new();
            map.insert(0, JobStatus::Uninitialized);
            map.insert(1, JobStatus::Blocked);
            map.insert(2, JobStatus::Ready);
            map.insert(3, JobStatus::Pending);
            map.insert(4, JobStatus::Running);
            map.insert(5, JobStatus::Completed);
            map.insert(6, JobStatus::Failed);
            map.insert(7, JobStatus::Canceled);
            map.insert(8, JobStatus::Terminated);
            map.insert(9, JobStatus::Disabled);
            map.insert(10, JobStatus::PendingFailed);
            map
        })
    }

    pub fn to_int(status: &JobStatus) -> i32 {
        *Self::enum_to_int_map().get(status).unwrap_or(&-1)
    }

    pub fn from_int(value: i32) -> Option<JobStatus> {
        Self::int_to_enum_map().get(&value).copied()
    }

    pub fn from_i64(value: i64) -> Option<JobStatus> {
        Self::from_int(value as i32)
    }
}

impl std::fmt::Display for EventSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventSeverity::Debug => write!(f, "debug"),
            EventSeverity::Info => write!(f, "info"),
            EventSeverity::Warning => write!(f, "warning"),
            EventSeverity::Error => write!(f, "error"),
        }
    }
}

impl std::str::FromStr for EventSeverity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "debug" => Ok(EventSeverity::Debug),
            "info" => Ok(EventSeverity::Info),
            "warning" => Ok(EventSeverity::Warning),
            "error" => Ok(EventSeverity::Error),
            _ => Err(format!("Invalid severity level: {}", s)),
        }
    }
}

impl CreateJobsResponse {
    pub fn new() -> CreateJobsResponse {
        CreateJobsResponse { jobs: None }
    }
}

impl ComputeNodeModel {
    pub fn new(
        workflow_id: i64,
        hostname: String,
        pid: i64,
        start_time: String,
        num_cpus: i64,
        memory_gb: f64,
        num_gpus: i64,
        num_nodes: i64,
        compute_node_type: String,
        scheduler: Option<serde_json::Value>,
    ) -> ComputeNodeModel {
        ComputeNodeModel {
            id: None,
            workflow_id,
            hostname,
            pid,
            start_time,
            end_time: None,
            duration_seconds: None,
            is_active: None,
            num_cpus,
            memory_gb,
            num_gpus,
            num_nodes,
            time_limit: None,
            scheduler_config_id: None,
            compute_node_type,
            scheduler,
            sample_count: None,
            peak_cpu_percent: None,
            avg_cpu_percent: None,
            peak_memory_bytes: None,
            avg_memory_bytes: None,
        }
    }
}

impl ComputeNodeSchedule {
    pub fn new(num_jobs: i64, scheduler_id: i64) -> ComputeNodeSchedule {
        ComputeNodeSchedule {
            max_parallel_jobs: None,
            num_jobs,
            scheduler_id,
            start_one_worker_per_node: Some(false),
        }
    }
}

impl ComputeNodesResources {
    pub fn new(
        num_cpus: i64,
        memory_gb: f64,
        num_gpus: i64,
        num_nodes: i64,
    ) -> ComputeNodesResources {
        ComputeNodesResources {
            id: None,
            num_cpus,
            memory_gb,
            num_gpus,
            num_nodes,
            time_limit: None,
            scheduler_config_id: None,
        }
    }
}

impl ErrorResponse {
    pub fn new(error: serde_json::Value) -> ErrorResponse {
        ErrorResponse {
            error,
            error_num: None,
            error_message: None,
            code: None,
        }
    }
}

impl EventModel {
    pub fn new(workflow_id: i64, data: serde_json::Value) -> EventModel {
        EventModel {
            id: None,
            workflow_id,
            timestamp: Utc::now().timestamp_millis(),
            data,
        }
    }

    pub fn timestamp_as_string(&self) -> String {
        use chrono::{DateTime, Utc};
        DateTime::from_timestamp_millis(self.timestamp)
            .map(|dt: DateTime<Utc>| dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
            .unwrap_or_else(|| format!("{}ms", self.timestamp))
    }
}

impl FileModel {
    pub fn new(workflow_id: i64, name: String, path: String) -> FileModel {
        FileModel {
            id: None,
            workflow_id,
            name,
            path,
            st_mtime: None,
        }
    }
}

impl FailureHandlerModel {
    pub fn new(workflow_id: i64, name: String, rules: String) -> FailureHandlerModel {
        FailureHandlerModel {
            id: None,
            workflow_id,
            name,
            rules,
        }
    }
}

impl ListFailureHandlersResponse {
    pub fn new(
        offset: i64,
        max_limit: i64,
        count: i64,
        total_count: i64,
        has_more: bool,
    ) -> ListFailureHandlersResponse {
        ListFailureHandlersResponse {
            items: vec![],
            offset,
            max_limit,
            count,
            total_count,
            has_more,
        }
    }
}

impl RoCrateEntityModel {
    pub fn new(
        workflow_id: i64,
        entity_id: String,
        entity_type: String,
        metadata: HashMap<String, Value>,
    ) -> RoCrateEntityModel {
        RoCrateEntityModel {
            id: None,
            workflow_id,
            file_id: None,
            entity_id,
            entity_type,
            metadata,
        }
    }
}

impl ListRoCrateEntitiesResponse {
    pub fn new(
        offset: i64,
        max_limit: i64,
        count: i64,
        total_count: i64,
        has_more: bool,
    ) -> ListRoCrateEntitiesResponse {
        ListRoCrateEntitiesResponse {
            items: vec![],
            offset,
            max_limit,
            count,
            total_count,
            has_more,
        }
    }
}

impl GetReadyJobRequirementsResponse {
    pub fn new(
        num_jobs: i64,
        num_cpus: i64,
        num_gpus: i64,
        memory_gb: f64,
        max_num_nodes: i64,
        max_runtime: String,
    ) -> GetReadyJobRequirementsResponse {
        GetReadyJobRequirementsResponse {
            num_jobs,
            num_cpus,
            num_gpus,
            memory_gb,
            max_num_nodes,
            max_runtime,
        }
    }
}

impl IsCompleteResponse {
    pub fn new(is_canceled: bool, is_complete: bool) -> IsCompleteResponse {
        IsCompleteResponse {
            is_canceled,
            is_complete,
        }
    }
}

impl JobModel {
    pub fn new(workflow_id: i64, name: String, command: String) -> JobModel {
        JobModel {
            id: None,
            workflow_id,
            name,
            command,
            invocation_script: None,
            env: None,
            status: Some(JobStatus::Uninitialized),
            start_time: None,
            compute_node_id: None,
            schedule_compute_nodes: None,
            cancel_on_blocking_job_failure: Some(true),
            supports_termination: Some(false),
            depends_on_job_ids: None,
            input_file_ids: None,
            output_file_ids: None,
            input_user_data_ids: None,
            output_user_data_ids: None,
            resource_requirements_id: None,
            scheduler_id: None,
            failure_handler_id: None,
            attempt_id: Some(1),
            priority: None,
            origin: None,
        }
    }
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            JobStatus::Uninitialized => write!(f, "uninitialized"),
            JobStatus::Blocked => write!(f, "blocked"),
            JobStatus::Ready => write!(f, "ready"),
            JobStatus::Pending => write!(f, "pending"),
            JobStatus::Running => write!(f, "running"),
            JobStatus::Completed => write!(f, "completed"),
            JobStatus::Failed => write!(f, "failed"),
            JobStatus::Canceled => write!(f, "canceled"),
            JobStatus::Terminated => write!(f, "terminated"),
            JobStatus::Disabled => write!(f, "disabled"),
            JobStatus::PendingFailed => write!(f, "pending_failed"),
        }
    }
}

impl std::str::FromStr for JobStatus {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "uninitialized" => Ok(JobStatus::Uninitialized),
            "blocked" => Ok(JobStatus::Blocked),
            "ready" => Ok(JobStatus::Ready),
            "pending" => Ok(JobStatus::Pending),
            "running" => Ok(JobStatus::Running),
            "completed" => Ok(JobStatus::Completed),
            "failed" => Ok(JobStatus::Failed),
            "canceled" => Ok(JobStatus::Canceled),
            "terminated" => Ok(JobStatus::Terminated),
            "disabled" => Ok(JobStatus::Disabled),
            "pending_failed" => Ok(JobStatus::PendingFailed),
            _ => Err(format!("Value not valid: {}", s)),
        }
    }
}

impl JobStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobStatus::Completed
                | JobStatus::Failed
                | JobStatus::Canceled
                | JobStatus::Terminated
                | JobStatus::PendingFailed
        )
    }

    pub fn is_complete(&self) -> bool {
        matches!(
            self,
            JobStatus::Completed | JobStatus::Failed | JobStatus::Canceled | JobStatus::Terminated
        )
    }

    pub fn to_int(&self) -> i32 {
        match *self {
            JobStatus::Uninitialized => 0,
            JobStatus::Blocked => 1,
            JobStatus::Ready => 2,
            JobStatus::Pending => 3,
            JobStatus::Running => 4,
            JobStatus::Completed => 5,
            JobStatus::Failed => 6,
            JobStatus::Canceled => 7,
            JobStatus::Terminated => 8,
            JobStatus::Disabled => 9,
            JobStatus::PendingFailed => 10,
        }
    }

    pub fn from_int(value: i32) -> std::result::Result<Self, String> {
        match value {
            0 => Ok(JobStatus::Uninitialized),
            1 => Ok(JobStatus::Blocked),
            2 => Ok(JobStatus::Ready),
            3 => Ok(JobStatus::Pending),
            4 => Ok(JobStatus::Running),
            5 => Ok(JobStatus::Completed),
            6 => Ok(JobStatus::Failed),
            7 => Ok(JobStatus::Canceled),
            8 => Ok(JobStatus::Terminated),
            9 => Ok(JobStatus::Disabled),
            10 => Ok(JobStatus::PendingFailed),
            _ => Err(format!("Invalid JobStatus integer value: {}", value)),
        }
    }

    pub fn from_i64(value: i64) -> std::result::Result<Self, String> {
        Self::from_int(value as i32)
    }
}

impl JobsModel {
    pub fn new(jobs: Vec<JobModel>) -> JobsModel {
        JobsModel { jobs }
    }
}

impl FilesModel {
    pub fn new(files: Vec<FileModel>) -> FilesModel {
        FilesModel { files }
    }
}

impl CreateFilesResponse {
    pub fn new() -> CreateFilesResponse {
        CreateFilesResponse { files: None }
    }
}

impl UserDataListModel {
    pub fn new(user_data: Vec<UserDataModel>) -> UserDataListModel {
        UserDataListModel { user_data }
    }
}

impl CreateUserDataListResponse {
    pub fn new() -> CreateUserDataListResponse {
        CreateUserDataListResponse { user_data: None }
    }
}

macro_rules! empty_list_response_new {
    ($ty:ident) => {
        impl $ty {
            pub fn new(
                offset: i64,
                max_limit: i64,
                count: i64,
                total_count: i64,
                has_more: bool,
            ) -> $ty {
                $ty {
                    items: vec![],
                    offset,
                    max_limit,
                    count,
                    total_count,
                    has_more,
                }
            }
        }
    };
}

empty_list_response_new!(ListComputeNodesResponse);
empty_list_response_new!(ListEventsResponse);
empty_list_response_new!(ListFilesResponse);
empty_list_response_new!(ListJobsResponse);
empty_list_response_new!(ListLocalSchedulersResponse);
empty_list_response_new!(ListResourceRequirementsResponse);
empty_list_response_new!(ListResultsResponse);
empty_list_response_new!(ListScheduledComputeNodesResponse);
empty_list_response_new!(ListSlurmSchedulersResponse);
empty_list_response_new!(ListUserDataResponse);
empty_list_response_new!(ListWorkflowsResponse);
empty_list_response_new!(ListJobDependenciesResponse);
empty_list_response_new!(ListJobFileRelationshipsResponse);
empty_list_response_new!(ListJobUserDataRelationshipsResponse);
empty_list_response_new!(ListSlurmStatsResponse);

impl ListMissingUserDataResponse {
    pub fn new() -> ListMissingUserDataResponse {
        ListMissingUserDataResponse {
            user_data: Vec::new(),
        }
    }
}

impl ListRequiredExistingFilesResponse {
    pub fn new() -> ListRequiredExistingFilesResponse {
        ListRequiredExistingFilesResponse { files: Vec::new() }
    }
}

impl LocalSchedulerModel {
    pub fn new(workflow_id: i64) -> LocalSchedulerModel {
        LocalSchedulerModel {
            id: None,
            workflow_id,
            name: Some("default".to_string()),
            memory: None,
            num_cpus: None,
        }
    }
}

impl ClaimJobsBasedOnResources {
    pub fn new() -> ClaimJobsBasedOnResources {
        ClaimJobsBasedOnResources {
            jobs: None,
            reason: None,
        }
    }
}

impl ClaimNextJobsResponse {
    pub fn new() -> ClaimNextJobsResponse {
        ClaimNextJobsResponse { jobs: None }
    }
}

impl ProcessChangedJobInputsResponse {
    pub fn new() -> ProcessChangedJobInputsResponse {
        ProcessChangedJobInputsResponse {
            reinitialized_jobs: vec![],
        }
    }
}

impl ResourceRequirementsModel {
    pub fn new(workflow_id: i64, name: String) -> ResourceRequirementsModel {
        ResourceRequirementsModel {
            id: None,
            workflow_id,
            name,
            num_cpus: default_num_cpus(),
            num_gpus: default_num_gpus(),
            num_nodes: default_num_nodes(),
            memory: default_memory(),
            runtime: default_runtime(),
        }
    }
}

impl ResultModel {
    pub fn new(
        job_id: i64,
        workflow_id: i64,
        run_id: i64,
        attempt_id: i64,
        compute_node_id: i64,
        return_code: i64,
        exec_time_minutes: f64,
        completion_time: String,
        status: JobStatus,
    ) -> ResultModel {
        ResultModel {
            id: None,
            job_id,
            workflow_id,
            run_id,
            attempt_id: Some(attempt_id),
            compute_node_id,
            return_code,
            exec_time_minutes,
            completion_time,
            peak_memory_bytes: None,
            avg_memory_bytes: None,
            peak_cpu_percent: None,
            avg_cpu_percent: None,
            status,
            job_name: None,
        }
    }
}

impl ScheduledComputeNodesModel {
    pub fn new(
        workflow_id: i64,
        scheduler_id: i64,
        scheduler_config_id: i64,
        scheduler_type: String,
        status: String,
    ) -> ScheduledComputeNodesModel {
        ScheduledComputeNodesModel {
            id: None,
            workflow_id,
            scheduler_id,
            scheduler_config_id,
            scheduler_type,
            status,
        }
    }
}

impl SlurmSchedulerModel {
    pub fn new(
        workflow_id: i64,
        account: String,
        nodes: i64,
        walltime: String,
    ) -> SlurmSchedulerModel {
        SlurmSchedulerModel {
            id: None,
            workflow_id,
            name: None,
            account,
            gres: None,
            mem: None,
            nodes,
            ntasks_per_node: None,
            partition: None,
            qos: Some("normal".to_string()),
            tmp: None,
            walltime,
            extra: None,
        }
    }
}

impl UserDataModel {
    pub fn new(workflow_id: i64, name: String) -> UserDataModel {
        UserDataModel {
            id: None,
            workflow_id,
            is_ephemeral: Some(false),
            name,
            data: None,
        }
    }
}

impl WorkflowModel {
    pub fn new(name: String, user: String) -> WorkflowModel {
        WorkflowModel {
            id: None,
            name,
            user,
            description: None,
            env: None,
            timestamp: None,
            compute_node_expiration_buffer_seconds: None,
            compute_node_wait_for_new_jobs_seconds: Some(0),
            compute_node_ignore_workflow_completion: Some(false),
            compute_node_wait_for_healthy_database_minutes: Some(20),
            compute_node_min_time_for_new_jobs_seconds: Some(300),
            resource_monitor_config: None,
            slurm_defaults: None,
            use_pending_failed: Some(false),
            enable_ro_crate: None,
            project: None,
            metadata: None,
            execution_config: None,
            run_id: None,
            is_canceled: None,
            is_archived: None,
            dynamic_jobs: None,
            access_groups: None,
            submission_directory: None,
        }
    }
}

impl JobDependencyModel {
    pub fn new(
        job_id: i64,
        job_name: String,
        depends_on_job_id: i64,
        depends_on_job_name: String,
        workflow_id: i64,
    ) -> JobDependencyModel {
        JobDependencyModel {
            job_id,
            job_name,
            depends_on_job_id,
            depends_on_job_name,
            workflow_id,
        }
    }
}

impl JobFileRelationshipModel {
    pub fn new(
        file_id: i64,
        file_name: String,
        file_path: String,
        workflow_id: i64,
    ) -> JobFileRelationshipModel {
        JobFileRelationshipModel {
            file_id,
            file_name,
            file_path,
            producer_job_id: None,
            producer_job_name: None,
            consumer_job_id: None,
            consumer_job_name: None,
            workflow_id,
        }
    }
}

impl JobUserDataRelationshipModel {
    pub fn new(
        user_data_id: i64,
        user_data_name: String,
        workflow_id: i64,
    ) -> JobUserDataRelationshipModel {
        JobUserDataRelationshipModel {
            user_data_id,
            user_data_name,
            producer_job_id: None,
            producer_job_name: None,
            consumer_job_id: None,
            consumer_job_name: None,
            workflow_id,
        }
    }
}

impl WorkflowActionModel {
    pub fn new(
        workflow_id: i64,
        trigger_type: String,
        action_type: String,
        action_config: serde_json::Value,
    ) -> WorkflowActionModel {
        WorkflowActionModel {
            id: None,
            workflow_id,
            trigger_type,
            action_type,
            action_config,
            job_ids: None,
            trigger_count: 0,
            required_triggers: 1,
            executed: false,
            executed_at: None,
            executed_by: None,
            persistent: false,
            is_recovery: false,
        }
    }
}

impl RemoteWorkerModel {
    pub fn new(worker: String, workflow_id: i64) -> RemoteWorkerModel {
        RemoteWorkerModel {
            worker,
            workflow_id,
        }
    }
}

impl ResetJobStatusResponse {
    pub fn new(workflow_id: i64, updated_count: i64, status: String) -> ResetJobStatusResponse {
        ResetJobStatusResponse {
            workflow_id,
            updated_count,
            status,
            reset_type: None,
        }
    }

    pub fn with_reset_type(mut self, reset_type: String) -> Self {
        self.reset_type = Some(reset_type);
        self
    }
}

impl DeleteCountResponse {
    pub fn get(&self, key: &str) -> Option<Value> {
        match key {
            "count" => Some(Value::from(self.count)),
            _ => None,
        }
    }
}

impl VersionResponse {
    pub fn is_object(&self) -> bool {
        true
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        match key {
            "version" => Some(Value::from(self.version.clone())),
            "api_version" => Some(Value::from(self.api_version.clone())),
            "git_hash" => self.git_hash.clone().map(Value::from),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        Some(self.version.as_str())
    }
}

impl ClaimActionResponse {
    pub fn get(&self, key: &str) -> Option<Value> {
        match key {
            "claimed" => Some(Value::from(self.success)),
            "success" => Some(Value::from(self.success)),
            "action_id" => Some(Value::from(self.action_id)),
            _ => None,
        }
    }
}

impl ReloadAuthResponse {
    pub fn get(&self, key: &str) -> Option<Value> {
        match key {
            "message" => Some(Value::from(self.message.clone())),
            "user_count" => Some(Value::from(self.user_count)),
            _ => None,
        }
    }
}

impl IsUninitializedResponse {
    pub fn get(&self, key: &str) -> Option<Value> {
        match key {
            "is_uninitialized" => Some(Value::from(self.is_uninitialized)),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        Some(self.is_uninitialized)
    }
}

impl ListJobIdsResponse {
    pub fn new(job_ids: Vec<i64>) -> ListJobIdsResponse {
        let count = job_ids.len() as i64;
        ListJobIdsResponse { job_ids, count }
    }
}

impl AccessGroupModel {
    pub fn new(name: String) -> AccessGroupModel {
        AccessGroupModel {
            id: None,
            name,
            description: None,
            created_at: None,
        }
    }
}

impl UserGroupMembershipModel {
    pub fn new(user_name: String, group_id: i64) -> UserGroupMembershipModel {
        UserGroupMembershipModel {
            id: None,
            user_name,
            group_id,
            role: "member".to_string(),
            created_at: None,
        }
    }
}

impl WorkflowAccessGroupModel {
    pub fn new(workflow_id: i64, group_id: i64) -> WorkflowAccessGroupModel {
        WorkflowAccessGroupModel {
            workflow_id,
            group_id,
            created_at: None,
        }
    }
}

impl ListAccessGroupsResponse {
    pub fn new(items: Vec<AccessGroupModel>, offset: i64, limit: i64, total_count: i64) -> Self {
        let has_more = offset + (items.len() as i64) < total_count;
        ListAccessGroupsResponse {
            items,
            offset,
            limit,
            total_count,
            has_more,
        }
    }
}

impl ListUserGroupMembershipsResponse {
    pub fn new(
        items: Vec<UserGroupMembershipModel>,
        offset: i64,
        limit: i64,
        total_count: i64,
    ) -> Self {
        let has_more = offset + (items.len() as i64) < total_count;
        ListUserGroupMembershipsResponse {
            items,
            offset,
            limit,
            total_count,
            has_more,
        }
    }
}

impl SlurmStatsModel {
    pub fn new(workflow_id: i64, job_id: i64, run_id: i64, attempt_id: i64) -> SlurmStatsModel {
        SlurmStatsModel {
            id: None,
            workflow_id,
            job_id,
            run_id,
            attempt_id,
            slurm_job_id: None,
            max_rss_bytes: None,
            max_vm_size_bytes: None,
            max_disk_read_bytes: None,
            max_disk_write_bytes: None,
            ave_cpu_seconds: None,
            node_list: None,
        }
    }
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowActionModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "openapi-codegen", schema(required = false))]
    pub id: Option<i64>,
    pub workflow_id: i64,
    pub trigger_type: String,
    pub action_type: String,
    pub action_config: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_ids: Option<Vec<i64>>,
    #[serde(default = "default_trigger_count")]
    #[cfg_attr(feature = "openapi-codegen", schema(required = false, default = 0))]
    pub trigger_count: i64,
    #[serde(default = "default_required_triggers")]
    #[cfg_attr(feature = "openapi-codegen", schema(required = false, default = 1))]
    pub required_triggers: i64,
    #[serde(default = "default_false")]
    #[cfg_attr(feature = "openapi-codegen", schema(required = false, default = false))]
    pub executed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_by: Option<i64>,
    #[serde(default = "default_false")]
    #[cfg_attr(feature = "openapi-codegen", schema(required = false, default = false))]
    pub persistent: bool,
    #[serde(default = "default_false")]
    #[cfg_attr(feature = "openapi-codegen", schema(required = false, default = false))]
    pub is_recovery: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaimActionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_node_id: Option<i64>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaimActionResponse {
    pub action_id: i64,
    #[serde(default, alias = "claimed")]
    #[cfg_attr(feature = "openapi-codegen", schema(required = false, default = false))]
    pub success: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteWorkerModel {
    pub worker: String,
    pub workflow_id: i64,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoCrateEntityModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub workflow_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<i64>,
    pub entity_id: String,
    pub entity_type: String,
    pub metadata: HashMap<String, Value>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListRoCrateEntitiesResponse {
    pub items: Vec<RoCrateEntityModel>,
    pub offset: i64,
    pub max_limit: i64,
    pub count: i64,
    pub total_count: i64,
    pub has_more: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageResponse {
    pub message: String,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteRoCrateEntitiesResponse {
    pub message: String,
    pub deleted_count: i64,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReloadAuthResponse {
    pub message: String,
    pub user_count: i64,
}

/// Request body for the admin raw-SQL endpoint (`POST /admin/sql`).
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdminSqlRequest {
    /// The single SQL statement to execute.
    pub sql: String,
    /// Opt into the write path. When false (default) the statement runs on a
    /// read-only connection, so any write fails at the SQLite layer.
    #[serde(default)]
    pub write: bool,
    /// Permit an unqualified UPDATE/DELETE (no WHERE clause). Ignored on the
    /// read-only path.
    #[serde(default)]
    pub allow_full_table: bool,
    /// Write path only: run inside a transaction, report rows affected, then
    /// roll back instead of committing (preview).
    #[serde(default)]
    pub dry_run: bool,
    /// Maximum number of SELECT result rows to return. Defaults to and is capped
    /// at 10,000 (the standard list cap); values above the cap are clamped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

/// Response body for the admin raw-SQL endpoint (`POST /admin/sql`).
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdminSqlResponse {
    /// Column names for SELECT results (empty for write statements).
    pub columns: Vec<String>,
    /// Result rows; each row is a list of JSON-encoded cell values aligned with `columns`.
    pub rows: Vec<Vec<serde_json::Value>>,
    /// Number of rows affected by a write statement, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows_affected: Option<i64>,
    /// True when a write was committed to the database.
    pub committed: bool,
}

/// One row of the admin raw-SQL audit log (`admin_audit_log`), returned by
/// `GET /admin/audit-log` (`torc admin list-audit-log`).
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdminAuditLogEntry {
    /// Auto-increment row id.
    pub id: i64,
    /// User that executed the statement.
    pub user_name: String,
    /// Execution time in milliseconds since the Unix epoch.
    pub timestamp: i64,
    /// The SQL statement text.
    pub sql_text: String,
    /// True for write-path statements (all audited rows are writes).
    pub is_write: bool,
    /// True when the full-table guard was overridden for this statement.
    pub allow_full_table: bool,
    /// Rows affected by the statement, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows_affected: Option<i64>,
    /// True when the write was committed to the database.
    pub committed: bool,
    /// True when the statement executed without error.
    pub success: bool,
    /// Error message captured for a failed statement, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Paginated response for `GET /admin/audit-log` (entries newest first).
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListAdminAuditLogResponse {
    /// Audit-log entries, newest first.
    pub items: Vec<AdminAuditLogEntry>,
    /// Offset applied to this page.
    pub offset: i64,
    /// Maximum page size enforced by the server.
    pub max_limit: i64,
    /// Number of entries returned in this page.
    pub count: i64,
    /// Total number of audit-log entries.
    pub total_count: i64,
    /// True when more entries exist beyond this page.
    pub has_more: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IsCompleteResponse {
    pub is_canceled: bool,
    pub is_complete: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IsUninitializedResponse {
    pub is_uninitialized: bool,
}

/// Counts of jobs grouped by status for a single workflow.
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobStatusCounts {
    pub uninitialized: i64,
    pub blocked: i64,
    pub ready: i64,
    pub pending: i64,
    pub running: i64,
    pub completed: i64,
    pub failed: i64,
    pub canceled: i64,
    pub terminated: i64,
    pub disabled: i64,
    pub pending_failed: i64,
}

/// Aggregated status summary for a workflow, computed server-side.
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowStatusResponse {
    pub workflow_id: i64,
    pub workflow_name: String,
    pub workflow_user: String,
    pub total_jobs: i64,
    pub jobs_by_status: JobStatusCounts,
    pub total_exec_time_minutes: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub walltime_seconds: Option<f64>,
    pub active_compute_nodes: i64,
    pub pending_scheduled_nodes: i64,
    pub active_scheduled_nodes: i64,
    pub is_complete: bool,
    pub is_canceled: bool,
    /// Ready jobs whose required runtime exceeds the remaining walltime of every
    /// active allocation, so they cannot start until a fresh allocation appears.
    /// 0 when there are no walltime-bounded allocations. See `torc workflows diagnose`.
    pub runtime_blocked_ready_jobs: i64,
    /// Longest required runtime (seconds) among ready jobs. Only populated when
    /// some ready jobs are runtime-blocked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longest_ready_runtime_seconds: Option<i64>,
    /// Greatest remaining walltime (seconds) across active walltime-bounded
    /// allocations. None when no active allocation reports an end time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_allocation_remaining_seconds: Option<i64>,
}

/// One Slurm-job-to-Torc-job correlation row: the Slurm job that ran a given
/// Torc job, derived from scheduled_compute_node -> compute_node -> result.
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlurmJobCorrelationModel {
    pub slurm_job_id: String,
    pub job_id: i64,
    pub job_name: String,
}

/// A page of Slurm-job-to-Torc-job correlations for a workflow, computed
/// server-side.
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlurmJobCorrelationsResponse {
    pub items: Vec<SlurmJobCorrelationModel>,
    pub offset: i64,
    pub max_limit: i64,
    pub count: i64,
    pub total_count: i64,
    pub has_more: bool,
}

/// A currently-running job together with the compute node it occupies and,
/// when the node was provisioned by a scheduler, that scheduler's job ID.
/// `scheduler_type` is generic (e.g. "slurm", "local"); `scheduler_job_id`
/// is populated only for scheduler-managed nodes (the Slurm job ID today).
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunningJobModel {
    pub job_id: i64,
    pub job_name: String,
    pub compute_node_name: String,
    pub scheduler_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler_job_id: Option<String>,
    /// RFC3339 time the job started running, for computing elapsed time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
}

/// A page of currently-running jobs for a workflow, computed server-side.
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunningJobsResponse {
    pub items: Vec<RunningJobModel>,
    pub offset: i64,
    pub max_limit: i64,
    pub count: i64,
    pub total_count: i64,
    pub has_more: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResetJobStatusResponse {
    pub workflow_id: i64,
    pub updated_count: i64,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_type: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{
        ClaimJobsBasedOnResources, ClaimNextJobsResponse, ComputeNodeModel, ComputeNodesResources,
        CreateJobsResponse, EventModel, FileModel, GetReadyJobRequirementsResponse, JobModel,
        JobStatus, ListComputeNodesResponse, ListFilesResponse, ResourceRequirementsModel,
        ResultModel, UserDataModel, WorkflowModel,
    };
    use serde_json::json;

    #[test]
    fn workflow_support_models_serialize_expected_shapes() {
        let workflow = WorkflowModel {
            id: Some(9),
            name: "wf".to_string(),
            user: "alice".to_string(),
            description: Some("desc".to_string()),
            env: None,
            timestamp: Some("2026-03-20T12:00:00Z".to_string()),
            compute_node_expiration_buffer_seconds: Some(30),
            compute_node_wait_for_new_jobs_seconds: Some(0),
            compute_node_ignore_workflow_completion: Some(false),
            compute_node_wait_for_healthy_database_minutes: Some(20),
            compute_node_min_time_for_new_jobs_seconds: Some(300),
            resource_monitor_config: None,
            slurm_defaults: None,
            use_pending_failed: Some(false),
            enable_ro_crate: Some(true),
            project: Some("proj".to_string()),
            metadata: Some(
                [("k".to_string(), json!("v"))]
                    .into_iter()
                    .collect::<std::collections::HashMap<_, _>>(),
            ),
            execution_config: None,
            run_id: Some(1),
            is_canceled: Some(false),
            is_archived: Some(false),
            dynamic_jobs: None,
            access_groups: None,
            submission_directory: Some("/home/alice/runs/wf".to_string()),
        };
        let serialized = serde_json::to_value(&workflow).unwrap();
        assert_eq!(serialized["name"], "wf");
        assert_eq!(serialized["user"], "alice");
    }

    #[test]
    fn job_status_serializes_as_expected() {
        assert_eq!(
            serde_json::to_string(&JobStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::from_str::<JobStatus>("\"completed\"").unwrap(),
            JobStatus::Completed
        );
    }

    #[test]
    fn representative_models_round_trip_through_json() {
        let compute_node = ComputeNodeModel {
            id: Some(42),
            workflow_id: 7,
            hostname: "node-a".into(),
            pid: 1234,
            start_time: "2026-03-20T12:00:00Z".into(),
            end_time: Some("2026-03-20T13:00:00Z".into()),
            duration_seconds: Some(10.5),
            is_active: Some(true),
            num_cpus: 8,
            memory_gb: 64.0,
            num_gpus: 1,
            num_nodes: 2,
            time_limit: Some("PT1H".into()),
            scheduler_config_id: Some(3),
            compute_node_type: "local".into(),
            scheduler: Some(json!({"kind": "local"})),
            sample_count: Some(2),
            peak_cpu_percent: Some(50.0),
            avg_cpu_percent: Some(30.0),
            peak_memory_bytes: Some(4096),
            avg_memory_bytes: Some(2048),
        };
        let job = JobModel {
            id: Some(5),
            workflow_id: 7,
            name: "job".into(),
            command: "echo hi".into(),
            invocation_script: None,
            env: None,
            status: Some(JobStatus::Ready),
            start_time: None,
            compute_node_id: None,
            schedule_compute_nodes: None,
            cancel_on_blocking_job_failure: Some(true),
            supports_termination: Some(false),
            depends_on_job_ids: Some(vec![1, 2]),
            input_file_ids: None,
            output_file_ids: None,
            input_user_data_ids: None,
            output_user_data_ids: None,
            resource_requirements_id: Some(4),
            scheduler_id: Some(2),
            failure_handler_id: None,
            attempt_id: Some(1),
            priority: Some(0),
            origin: None,
        };
        let result = ResultModel {
            id: Some(1),
            job_id: 5,
            workflow_id: 7,
            run_id: 3,
            attempt_id: Some(1),
            compute_node_id: 9,
            return_code: 0,
            exec_time_minutes: 0.5,
            completion_time: "2026-03-20T12:05:00Z".into(),
            peak_memory_bytes: Some(123),
            avg_memory_bytes: Some(120),
            peak_cpu_percent: Some(90.0),
            avg_cpu_percent: Some(60.0),
            status: JobStatus::Completed,
            job_name: None,
        };
        let file = FileModel {
            id: Some(1),
            workflow_id: 7,
            name: "f".into(),
            path: "/tmp/f".into(),
            st_mtime: Some(1.0),
        };
        let user_data = UserDataModel {
            id: Some(1),
            workflow_id: 7,
            is_ephemeral: Some(false),
            name: "ud".into(),
            data: Some(json!({"x":1})),
        };
        let event = EventModel {
            id: Some(1),
            workflow_id: 7,
            timestamp: 10,
            data: json!({"msg":"ok"}),
        };
        let rr = ResourceRequirementsModel {
            id: Some(4),
            workflow_id: 7,
            name: "small".into(),
            num_cpus: 1,
            num_gpus: 0,
            num_nodes: 1,
            memory: "1m".into(),
            runtime: "P0DT1M".into(),
        };
        let _ =
            serde_json::from_value::<ComputeNodeModel>(serde_json::to_value(compute_node).unwrap())
                .unwrap();
        let _ = serde_json::from_value::<JobModel>(serde_json::to_value(job).unwrap()).unwrap();
        let _ =
            serde_json::from_value::<ResultModel>(serde_json::to_value(result).unwrap()).unwrap();
        let _ = serde_json::from_value::<FileModel>(serde_json::to_value(file).unwrap()).unwrap();
        let _ = serde_json::from_value::<UserDataModel>(serde_json::to_value(user_data).unwrap())
            .unwrap();
        let _ = serde_json::from_value::<EventModel>(serde_json::to_value(event).unwrap()).unwrap();
        let _ =
            serde_json::from_value::<ResourceRequirementsModel>(serde_json::to_value(rr).unwrap())
                .unwrap();
    }

    #[test]
    fn resource_requirements_defaults_apply_when_fields_are_missing() {
        let rr = serde_json::from_value::<ResourceRequirementsModel>(json!({
            "workflow_id": 7,
            "name": "defaulted"
        }))
        .unwrap();

        assert_eq!(rr.num_cpus, 1);
        assert_eq!(rr.num_gpus, 0);
        assert_eq!(rr.num_nodes, 1);
        assert_eq!(rr.memory, "1m");
        assert_eq!(rr.runtime, "PT1M");
    }

    #[test]
    fn response_shapes_serialize_expected_fields() {
        let jobs = CreateJobsResponse { jobs: Some(vec![]) };
        let resources = ComputeNodesResources {
            id: None,
            num_cpus: 8,
            memory_gb: 16.0,
            num_gpus: 0,
            num_nodes: 1,
            time_limit: None,
            scheduler_config_id: None,
        };
        let claim = ClaimJobsBasedOnResources {
            jobs: Some(vec![]),
            reason: None,
        };
        let next = ClaimNextJobsResponse { jobs: Some(vec![]) };
        let list = ListComputeNodesResponse {
            items: vec![],
            offset: 0,
            max_limit: 100,
            count: 0,
            total_count: 0,
            has_more: false,
        };
        let files = ListFilesResponse {
            items: vec![],
            offset: 0,
            max_limit: 100,
            count: 0,
            total_count: 0,
            has_more: false,
        };
        let ready = GetReadyJobRequirementsResponse {
            num_jobs: 1,
            num_cpus: 2,
            num_gpus: 0,
            memory_gb: 4.0,
            max_num_nodes: 1,
            max_runtime: "PT10M".into(),
        };
        assert!(serde_json::to_value(jobs).unwrap().get("jobs").is_some());
        assert!(
            serde_json::to_value(resources)
                .unwrap()
                .get("num_cpus")
                .is_some()
        );
        assert!(serde_json::to_value(claim).unwrap().get("jobs").is_some());
        assert!(serde_json::to_value(next).unwrap().get("jobs").is_some());
        assert!(serde_json::to_value(list).unwrap().get("items").is_some());
        assert!(serde_json::to_value(files).unwrap().get("items").is_some());
        assert!(
            serde_json::to_value(ready)
                .unwrap()
                .get("num_jobs")
                .is_some()
        );
    }
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TaskStatus::Queued => "queued",
            TaskStatus::Running => "running",
            TaskStatus::Succeeded => "succeeded",
            TaskStatus::Failed => "failed",
        };
        write!(f, "{s}")
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "queued" => Ok(TaskStatus::Queued),
            "running" => Ok(TaskStatus::Running),
            "succeeded" => Ok(TaskStatus::Succeeded),
            "failed" => Ok(TaskStatus::Failed),
            other => Err(format!("Unknown task status: {other}")),
        }
    }
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TaskModel {
    #[serde(rename = "id")]
    pub id: i64,

    #[serde(rename = "workflow_id")]
    pub workflow_id: i64,

    #[serde(rename = "operation")]
    pub operation: String,

    #[serde(rename = "status")]
    pub status: TaskStatus,

    #[serde(rename = "created_at_ms")]
    pub created_at_ms: i64,

    #[serde(rename = "started_at_ms")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at_ms: Option<i64>,

    #[serde(rename = "finished_at_ms")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at_ms: Option<i64>,

    #[serde(rename = "error")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl TaskModel {
    pub fn new(
        id: i64,
        workflow_id: i64,
        operation: String,
        status: TaskStatus,
        created_at_ms: i64,
    ) -> TaskModel {
        TaskModel {
            id,
            workflow_id,
            operation,
            status,
            created_at_ms,
            started_at_ms: None,
            finished_at_ms: None,
            error: None,
        }
    }
}

/// Wrapper for `GET /workflows/{id}/active_task` so the response always has a JSON body,
/// even when the workflow currently has no active async task. The `task` field is the
/// active task for this workflow, or null if none is in-flight.
#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ActiveTaskResponse {
    // The `///` doc comment is intentionally on the struct, not here: utoipa would otherwise
    // emit the description as a sibling of `$ref` inside `oneOf`, which is invalid OpenAPI 3.1.
    #[serde(rename = "task")]
    pub task: Option<TaskModel>,
}
