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
    pub resource_monitor_config: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slurm_defaults: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_pending_failed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_ro_crate: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slurm_config: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_config: Option<String>,
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
        metadata: String,
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
    pub fn new(
        is_canceled: bool,
        is_complete: bool,
        needs_to_run_completion_script: bool,
    ) -> IsCompleteResponse {
        IsCompleteResponse {
            is_canceled,
            is_complete,
            needs_to_run_completion_script,
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
            status_id: None,
            slurm_config: None,
            execution_config: None,
        }
    }
}

impl WorkflowStatusModel {
    pub fn new(is_canceled: bool, run_id: i64) -> WorkflowStatusModel {
        WorkflowStatusModel {
            id: None,
            is_canceled,
            is_archived: Some(false),
            run_id,
            has_detected_need_to_run_completion_script: Some(false),
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
    pub metadata: String,
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

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowStatusModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub is_canceled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_archived: Option<bool>,
    pub run_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_detected_need_to_run_completion_script: Option<bool>,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IsCompleteResponse {
    pub is_canceled: bool,
    pub is_complete: bool,
    pub needs_to_run_completion_script: bool,
}

#[cfg_attr(feature = "openapi-codegen", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IsUninitializedResponse {
    pub is_uninitialized: bool,
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
        ResultModel, UserDataModel, WorkflowModel, WorkflowStatusModel,
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
            metadata: Some(json!({"k": "v"}).to_string()),
            status_id: Some(1),
            slurm_config: None,
            execution_config: None,
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
        let wf_status = WorkflowStatusModel {
            id: Some(1),
            is_canceled: false,
            is_archived: Some(false),
            run_id: 1,
            has_detected_need_to_run_completion_script: Some(false),
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
        let _ =
            serde_json::from_value::<WorkflowStatusModel>(serde_json::to_value(wf_status).unwrap())
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
