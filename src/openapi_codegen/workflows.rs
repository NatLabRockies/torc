#![allow(dead_code)]

use axum::{
    Json,
    extract::{Path, Query},
};
use serde::Deserialize;
use serde_json::Value;
use utoipa::IntoParams;

use crate::api_models::{
    ClaimJobsBasedOnResources, ClaimNextJobsResponse, ComputeNodesResources,
    GetReadyJobRequirementsResponse, IsCompleteResponse, IsUninitializedResponse,
    JobDependencyModel, JobFileRelationshipModel, JobStatus, JobUserDataRelationshipModel,
    ListJobDependenciesResponse, ListJobFileRelationshipsResponse, ListJobIdsResponse,
    ListJobUserDataRelationshipsResponse, ListMissingUserDataResponse,
    ListRequiredExistingFilesResponse, ListWorkflowsResponse, ProcessChangedJobInputsResponse,
    ResetJobStatusResponse, WorkflowModel, WorkflowStatusModel,
};

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct WorkflowsListQuery {
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
    #[param(nullable = true)]
    pub sort_by: Option<String>,
    #[param(nullable = true)]
    pub reverse_sort: Option<bool>,
    #[param(nullable = true)]
    pub name: Option<String>,
    #[param(nullable = true)]
    pub user: Option<String>,
    #[param(nullable = true)]
    pub description: Option<String>,
    #[param(nullable = true)]
    pub is_archived: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct InitializeJobsQuery {
    #[param(nullable = true)]
    pub only_uninitialized: Option<bool>,
    #[param(nullable = true)]
    pub clear_ephemeral_user_data: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ResetWorkflowStatusQuery {
    #[param(nullable = true)]
    pub force: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ResetJobStatusQuery {
    #[param(nullable = true)]
    pub failed_only: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ClaimJobsBasedOnResourcesQuery {
    #[param(nullable = true)]
    pub strict_scheduler_match: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ClaimNextJobsQuery {
    #[param(nullable = true)]
    pub limit: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct WorkflowRelationshipsQuery {
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ProcessChangedJobInputsQuery {
    #[param(nullable = true)]
    pub dry_run: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ReadyJobRequirementsQuery {
    #[param(nullable = true)]
    pub scheduler_config_id: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/workflows",
    operation_id = "list_workflows",
    params(WorkflowsListQuery),
    responses((status = 200, description = "Successful response", body = ListWorkflowsResponse))
)]
pub async fn list_workflows(
    Query(query): Query<WorkflowsListQuery>,
) -> Json<ListWorkflowsResponse> {
    Json(ListWorkflowsResponse {
        items: vec![],
        offset: query.offset.unwrap_or(0),
        max_limit: crate::MAX_RECORD_TRANSFER_COUNT,
        count: 0,
        total_count: 0,
        has_more: false,
    })
}

#[utoipa::path(
    post,
    path = "/workflows",
    operation_id = "create_workflow",
    request_body = WorkflowModel,
    responses((status = 200, description = "Successful response", body = WorkflowModel))
)]
pub async fn create_workflow(Json(mut body): Json<WorkflowModel>) -> Json<WorkflowModel> {
    if body.id.is_none() {
        body.id = Some(0);
    }
    Json(body)
}

#[utoipa::path(
    delete,
    path = "/workflows/{id}",
    operation_id = "delete_workflow",
    params(("id" = i64, Path, description = "Workflow ID.")),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = WorkflowModel))
)]
pub async fn delete_workflow(
    Path(id): Path<i64>,
    Json(_body): Json<Option<Value>>,
) -> Json<WorkflowModel> {
    Json(example_workflow(Some(id)))
}

#[utoipa::path(
    get,
    path = "/workflows/{id}",
    operation_id = "get_workflow",
    params(("id" = i64, Path, description = "ID of the workflows record")),
    responses((status = 200, description = "Successful response", body = WorkflowModel))
)]
pub async fn get_workflow(Path(id): Path<i64>) -> Json<WorkflowModel> {
    Json(example_workflow(Some(id)))
}

#[utoipa::path(
    put,
    path = "/workflows/{id}",
    operation_id = "update_workflow",
    params(("id" = i64, Path, description = "Workflow ID")),
    request_body = WorkflowModel,
    responses((status = 200, description = "Successful response", body = WorkflowModel))
)]
pub async fn update_workflow(
    Path(id): Path<i64>,
    Json(mut body): Json<WorkflowModel>,
) -> Json<WorkflowModel> {
    body.id = Some(id);
    Json(body)
}

#[utoipa::path(
    put,
    path = "/workflows/{id}/cancel",
    operation_id = "cancel_workflow",
    params(("id" = i64, Path, description = "Workflow ID")),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = Value))
)]
pub async fn cancel_workflow(Path(id): Path<i64>, Json(_body): Json<Option<Value>>) -> Json<Value> {
    Json(serde_json::json!({
        "workflow_id": id,
        "status": "canceled"
    }))
}

#[utoipa::path(
    post,
    path = "/workflows/{id}/initialize_jobs",
    operation_id = "initialize_jobs",
    params(("id" = i64, Path, description = "Workflow ID"), InitializeJobsQuery),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = Value))
)]
pub async fn initialize_jobs(
    Path(id): Path<i64>,
    Query(query): Query<InitializeJobsQuery>,
    Json(_body): Json<Option<Value>>,
) -> Json<Value> {
    Json(serde_json::json!({
        "workflow_id": id,
        "only_uninitialized": query.only_uninitialized.unwrap_or(false),
        "clear_ephemeral_user_data": query.clear_ephemeral_user_data.unwrap_or(true)
    }))
}

#[utoipa::path(
    get,
    path = "/workflows/{id}/is_complete",
    operation_id = "is_workflow_complete",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses((status = 200, description = "Successful response", body = IsCompleteResponse))
)]
pub async fn is_workflow_complete(Path(_id): Path<i64>) -> Json<IsCompleteResponse> {
    Json(IsCompleteResponse {
        is_canceled: true,
        is_complete: true,
        needs_to_run_completion_script: true,
    })
}

#[utoipa::path(
    get,
    path = "/workflows/{id}/is_uninitialized",
    operation_id = "is_workflow_uninitialized",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses((status = 200, description = "Successful response", body = IsUninitializedResponse))
)]
pub async fn is_workflow_uninitialized(Path(_id): Path<i64>) -> Json<IsUninitializedResponse> {
    Json(IsUninitializedResponse {
        is_uninitialized: true,
    })
}

#[utoipa::path(
    post,
    path = "/workflows/{id}/reset_status",
    operation_id = "reset_workflow_status",
    params(("id" = i64, Path, description = "Workflow ID"), ResetWorkflowStatusQuery),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = Value))
)]
pub async fn reset_workflow_status(
    Path(id): Path<i64>,
    Query(query): Query<ResetWorkflowStatusQuery>,
    Json(_body): Json<Option<Value>>,
) -> Json<Value> {
    Json(serde_json::json!({
        "workflow_id": id,
        "force": query.force.unwrap_or(false),
        "status": "reset"
    }))
}

#[utoipa::path(
    post,
    path = "/workflows/{id}/reset_job_status",
    operation_id = "reset_job_status",
    params(("id" = i64, Path, description = "Workflow ID"), ResetJobStatusQuery),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = ResetJobStatusResponse))
)]
pub async fn reset_job_status(
    Path(id): Path<i64>,
    Query(query): Query<ResetJobStatusQuery>,
    Json(_body): Json<Option<Value>>,
) -> Json<ResetJobStatusResponse> {
    Json(ResetJobStatusResponse {
        workflow_id: id,
        updated_count: 0,
        status: "uninitialized".to_string(),
        reset_type: Some(if query.failed_only.unwrap_or(false) {
            "failed_only".to_string()
        } else {
            "all".to_string()
        }),
    })
}

#[utoipa::path(
    get,
    path = "/workflows/{id}/status",
    operation_id = "get_workflow_status",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses((status = 200, description = "Successful response", body = WorkflowStatusModel))
)]
pub async fn get_workflow_status(Path(id): Path<i64>) -> Json<WorkflowStatusModel> {
    Json(example_workflow_status(Some(id)))
}

#[utoipa::path(
    put,
    path = "/workflows/{id}/status",
    operation_id = "update_workflow_status",
    params(("id" = i64, Path, description = "Workflow ID")),
    request_body = WorkflowStatusModel,
    responses((status = 200, description = "Successful response", body = WorkflowStatusModel))
)]
pub async fn update_workflow_status(
    Path(id): Path<i64>,
    Json(mut body): Json<WorkflowStatusModel>,
) -> Json<WorkflowStatusModel> {
    body.id = Some(id);
    Json(body)
}

#[utoipa::path(
    post,
    path = "/workflows/{id}/claim_jobs_based_on_resources/{limit}",
    operation_id = "claim_jobs_based_on_resources",
    params(
        ("id" = i64, Path, description = "Workflow ID"),
        ClaimJobsBasedOnResourcesQuery,
        ("limit" = i64, Path, description = "Maximum number of jobs to claim")
    ),
    request_body = ComputeNodesResources,
    responses((
        status = 200,
        description = "Successful response",
        body = ClaimJobsBasedOnResources
    ))
)]
pub async fn claim_jobs_based_on_resources(
    Path((id, limit)): Path<(i64, i64)>,
    Query(query): Query<ClaimJobsBasedOnResourcesQuery>,
    Json(body): Json<ComputeNodesResources>,
) -> Json<ClaimJobsBasedOnResources> {
    let mut claimed_job = super::jobs::example_job(Some(0));
    claimed_job.workflow_id = id;
    claimed_job.status = Some(JobStatus::Pending);
    claimed_job.scheduler_id = body.scheduler_config_id;

    let jobs = if limit > 0 && body.num_cpus > 0 {
        Some(vec![claimed_job])
    } else {
        Some(vec![])
    };

    let reason =
        if query.strict_scheduler_match.unwrap_or(false) && body.scheduler_config_id.is_none() {
            Some("strict_scheduler_match_requires_scheduler_config_id".to_string())
        } else if limit <= 0 {
            Some("limit_must_be_positive".to_string())
        } else if body.num_cpus <= 0 {
            Some("worker_has_no_available_cpus".to_string())
        } else {
            Some("claimed_jobs".to_string())
        };

    Json(ClaimJobsBasedOnResources { jobs, reason })
}

#[utoipa::path(
    post,
    path = "/workflows/{id}/claim_next_jobs",
    operation_id = "claim_next_jobs",
    params(("id" = i64, Path, description = "Workflow ID"), ClaimNextJobsQuery),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = ClaimNextJobsResponse))
)]
pub async fn claim_next_jobs(
    Path(id): Path<i64>,
    Query(query): Query<ClaimNextJobsQuery>,
    Json(_body): Json<Option<Value>>,
) -> Json<ClaimNextJobsResponse> {
    let limit = query.limit.unwrap_or(1);
    let jobs = if limit > 0 {
        let mut items = Vec::new();
        for job_id in 0..limit.min(2) {
            let mut job = super::jobs::example_job(Some(job_id));
            job.workflow_id = id;
            job.status = Some(JobStatus::Pending);
            items.push(job);
        }
        Some(items)
    } else {
        Some(vec![])
    };

    Json(ClaimNextJobsResponse { jobs })
}

#[utoipa::path(
    get,
    path = "/workflows/{id}/job_dependencies",
    operation_id = "list_job_dependencies",
    params(("id" = i64, Path, description = "Workflow ID"), WorkflowRelationshipsQuery),
    responses((status = 200, description = "Successful response", body = ListJobDependenciesResponse))
)]
pub async fn list_job_dependencies(
    Path(id): Path<i64>,
    Query(query): Query<WorkflowRelationshipsQuery>,
) -> Json<ListJobDependenciesResponse> {
    Json(ListJobDependenciesResponse {
        items: vec![JobDependencyModel {
            job_id: 123,
            job_name: "process_data".to_string(),
            depends_on_job_id: 456,
            depends_on_job_name: "download_data".to_string(),
            workflow_id: id,
        }],
        offset: query.offset.unwrap_or(0),
        max_limit: crate::MAX_RECORD_TRANSFER_COUNT,
        count: 1,
        total_count: 1,
        has_more: false,
    })
}

#[utoipa::path(
    get,
    path = "/workflows/{id}/job_file_relationships",
    operation_id = "list_job_file_relationships",
    params(("id" = i64, Path, description = "Workflow ID"), WorkflowRelationshipsQuery),
    responses((status = 200, description = "Successful response", body = ListJobFileRelationshipsResponse))
)]
pub async fn list_job_file_relationships(
    Path(id): Path<i64>,
    Query(query): Query<WorkflowRelationshipsQuery>,
) -> Json<ListJobFileRelationshipsResponse> {
    Json(ListJobFileRelationshipsResponse {
        items: vec![JobFileRelationshipModel {
            file_id: 42,
            file_name: "data.csv".to_string(),
            file_path: "/path/to/data.csv".to_string(),
            producer_job_id: Some(123),
            producer_job_name: Some("generate_data".to_string()),
            consumer_job_id: Some(456),
            consumer_job_name: Some("process_data".to_string()),
            workflow_id: id,
        }],
        offset: query.offset.unwrap_or(0),
        max_limit: crate::MAX_RECORD_TRANSFER_COUNT,
        count: 1,
        total_count: 1,
        has_more: false,
    })
}

#[utoipa::path(
    get,
    path = "/workflows/{id}/job_user_data_relationships",
    operation_id = "list_job_user_data_relationships",
    params(("id" = i64, Path, description = "Workflow ID"), WorkflowRelationshipsQuery),
    responses((status = 200, description = "Successful response", body = ListJobUserDataRelationshipsResponse))
)]
pub async fn list_job_user_data_relationships(
    Path(id): Path<i64>,
    Query(query): Query<WorkflowRelationshipsQuery>,
) -> Json<ListJobUserDataRelationshipsResponse> {
    Json(ListJobUserDataRelationshipsResponse {
        items: vec![JobUserDataRelationshipModel {
            user_data_id: 42,
            user_data_name: "config".to_string(),
            producer_job_id: Some(123),
            producer_job_name: Some("generate_config".to_string()),
            consumer_job_id: Some(456),
            consumer_job_name: Some("use_config".to_string()),
            workflow_id: id,
        }],
        offset: query.offset.unwrap_or(0),
        max_limit: crate::MAX_RECORD_TRANSFER_COUNT,
        count: 1,
        total_count: 1,
        has_more: false,
    })
}

#[utoipa::path(
    get,
    path = "/workflows/{id}/job_ids",
    operation_id = "list_job_ids",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses((status = 200, description = "Successful response", body = ListJobIdsResponse))
)]
pub async fn list_job_ids(Path(id): Path<i64>) -> Json<ListJobIdsResponse> {
    Json(ListJobIdsResponse {
        job_ids: vec![id * 10 + 1, id * 10 + 2],
        count: 2,
    })
}

#[utoipa::path(
    get,
    path = "/workflows/{id}/missing_user_data",
    operation_id = "list_missing_user_data",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses((status = 200, description = "Successful response", body = ListMissingUserDataResponse))
)]
pub async fn list_missing_user_data(Path(_id): Path<i64>) -> Json<ListMissingUserDataResponse> {
    Json(ListMissingUserDataResponse {
        user_data: vec![1, 2],
    })
}

#[utoipa::path(
    post,
    path = "/workflows/{id}/process_changed_job_inputs",
    operation_id = "process_changed_job_inputs",
    params(("id" = i64, Path, description = "Workflow ID"), ProcessChangedJobInputsQuery),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = ProcessChangedJobInputsResponse))
)]
pub async fn process_changed_job_inputs(
    Path(id): Path<i64>,
    Query(query): Query<ProcessChangedJobInputsQuery>,
    Json(_body): Json<Option<Value>>,
) -> Json<ProcessChangedJobInputsResponse> {
    Json(ProcessChangedJobInputsResponse {
        reinitialized_jobs: if query.dry_run.unwrap_or(false) {
            vec![format!("workflow_{id}_would_reinitialize_job")]
        } else {
            vec![format!("workflow_{id}_reinitialized_job")]
        },
    })
}

#[utoipa::path(
    get,
    path = "/workflows/{id}/ready_job_requirements",
    operation_id = "get_ready_job_requirements",
    params(("id" = i64, Path, description = "Workflow ID"), ReadyJobRequirementsQuery),
    responses((status = 200, description = "Successful response", body = GetReadyJobRequirementsResponse))
)]
pub async fn get_ready_job_requirements(
    Path(_id): Path<i64>,
    Query(query): Query<ReadyJobRequirementsQuery>,
) -> Json<GetReadyJobRequirementsResponse> {
    Json(GetReadyJobRequirementsResponse {
        num_jobs: 0,
        num_cpus: 6,
        num_gpus: 1,
        memory_gb: 5.962133916683182,
        max_num_nodes: 5,
        max_runtime: query
            .scheduler_config_id
            .map(|id| format!("scheduler_{id}_runtime"))
            .unwrap_or_else(|| "max_runtime".to_string()),
    })
}

#[utoipa::path(
    get,
    path = "/workflows/{id}/required_existing_files",
    operation_id = "list_required_existing_files",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses((status = 200, description = "Successful response", body = ListRequiredExistingFilesResponse))
)]
pub async fn list_required_existing_files(
    Path(id): Path<i64>,
) -> Json<ListRequiredExistingFilesResponse> {
    Json(ListRequiredExistingFilesResponse {
        files: vec![id * 1000 + 1],
    })
}

pub fn example_workflow(id: Option<i64>) -> WorkflowModel {
    WorkflowModel {
        id,
        name: "name".to_string(),
        user: "user".to_string(),
        description: Some("description".to_string()),
        timestamp: Some("timestamp".to_string()),
        compute_node_expiration_buffer_seconds: None,
        compute_node_wait_for_new_jobs_seconds: Some(1),
        compute_node_ignore_workflow_completion: Some(false),
        compute_node_wait_for_healthy_database_minutes: Some(5),
        compute_node_min_time_for_new_jobs_seconds: Some(300),
        resource_monitor_config: None,
        slurm_defaults: None,
        use_pending_failed: Some(false),
        enable_ro_crate: Some(false),
        project: None,
        metadata: None,
        status_id: Some(1),
        slurm_config: Some("null".to_string()),
        execution_config: Some(
            "{\"mode\":\"auto\",\"limit_resources\":true,\"srun_termination_signal\":\"TERM@120\"}"
                .to_string(),
        ),
    }
}

pub fn example_workflow_status(id: Option<i64>) -> WorkflowStatusModel {
    WorkflowStatusModel {
        id,
        is_canceled: true,
        is_archived: Some(false),
        run_id: 6,
        has_detected_need_to_run_completion_script: Some(false),
    }
}
