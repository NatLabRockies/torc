#![allow(dead_code)]

use axum::{
    Json,
    extract::{Path, Query},
};
use serde::Deserialize;
use serde_json::Value;
use utoipa::IntoParams;

use crate::api_models::{
    CreateJobsResponse, DeleteCountResponse, FailureHandlerModel, JobsModel,
    ListFailureHandlersResponse, ListResourceRequirementsResponse, ListSlurmStatsResponse,
    ResourceRequirementsModel, SlurmStatsModel,
};

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ResourceRequirementsListQuery {
    pub workflow_id: i64,
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
    #[param(nullable = true)]
    pub sort_by: Option<String>,
    #[param(nullable = true)]
    pub reverse_sort: Option<bool>,
    #[param(nullable = true)]
    pub job_id: Option<i64>,
    #[param(nullable = true)]
    pub name: Option<String>,
    #[param(nullable = true)]
    pub memory: Option<String>,
    #[param(nullable = true)]
    pub num_cpus: Option<i64>,
    #[param(nullable = true)]
    pub num_gpus: Option<i64>,
    #[param(nullable = true)]
    pub num_nodes: Option<i64>,
    #[param(nullable = true)]
    pub runtime: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct DeleteResourceRequirementsQuery {
    pub workflow_id: i64,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct FailureHandlersListQuery {
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct SlurmStatsListQuery {
    pub workflow_id: i64,
    #[param(nullable = true)]
    pub job_id: Option<i64>,
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
}

#[utoipa::path(
    post,
    path = "/bulk_jobs",
    operation_id = "create_jobs",
    request_body = JobsModel,
    responses((status = 200, description = "Successful response", body = CreateJobsResponse))
)]
pub async fn create_jobs(Json(body): Json<JobsModel>) -> Json<CreateJobsResponse> {
    Json(CreateJobsResponse {
        jobs: Some(body.jobs),
    })
}

#[utoipa::path(
    post,
    path = "/resource_requirements",
    operation_id = "create_resource_requirements",
    request_body = ResourceRequirementsModel,
    responses((status = 200, description = "Successful response", body = ResourceRequirementsModel))
)]
pub async fn create_resource_requirements(
    Json(mut body): Json<ResourceRequirementsModel>,
) -> Json<ResourceRequirementsModel> {
    if body.id.is_none() {
        body.id = Some(0);
    }
    Json(body)
}

#[utoipa::path(
    delete,
    path = "/resource_requirements",
    operation_id = "delete_resource_requirements",
    params(DeleteResourceRequirementsQuery),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = DeleteCountResponse))
)]
pub async fn delete_resource_requirements(
    Query(_query): Query<DeleteResourceRequirementsQuery>,
    Json(_body): Json<Option<Value>>,
) -> Json<DeleteCountResponse> {
    Json(DeleteCountResponse { count: 0 })
}

#[utoipa::path(
    get,
    path = "/resource_requirements",
    operation_id = "list_resource_requirements",
    params(ResourceRequirementsListQuery),
    responses((status = 200, description = "Successful response", body = ListResourceRequirementsResponse))
)]
pub async fn list_resource_requirements(
    Query(query): Query<ResourceRequirementsListQuery>,
) -> Json<ListResourceRequirementsResponse> {
    Json(ListResourceRequirementsResponse {
        items: vec![example_resource_requirements(Some(1), query.workflow_id)],
        offset: query.offset.unwrap_or(0),
        max_limit: crate::MAX_RECORD_TRANSFER_COUNT,
        count: 1,
        total_count: 1,
        has_more: false,
    })
}

#[utoipa::path(
    delete,
    path = "/resource_requirements/{id}",
    operation_id = "delete_resource_requirement",
    params(("id" = i64, Path, description = "Resource requirements ID")),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = ResourceRequirementsModel))
)]
pub async fn delete_resource_requirement(
    Path(id): Path<i64>,
    Json(_body): Json<Option<Value>>,
) -> Json<ResourceRequirementsModel> {
    Json(example_resource_requirements(Some(id), 1))
}

#[utoipa::path(
    get,
    path = "/resource_requirements/{id}",
    operation_id = "get_resource_requirements",
    params(("id" = i64, Path, description = "Resource requirements ID")),
    responses((status = 200, description = "Successful response", body = ResourceRequirementsModel))
)]
pub async fn get_resource_requirements(Path(id): Path<i64>) -> Json<ResourceRequirementsModel> {
    Json(example_resource_requirements(Some(id), 1))
}

#[utoipa::path(
    put,
    path = "/resource_requirements/{id}",
    operation_id = "update_resource_requirements",
    params(("id" = i64, Path, description = "Resource requirements ID")),
    request_body = ResourceRequirementsModel,
    responses((status = 200, description = "Successful response", body = ResourceRequirementsModel))
)]
pub async fn update_resource_requirements(
    Path(id): Path<i64>,
    Json(mut body): Json<ResourceRequirementsModel>,
) -> Json<ResourceRequirementsModel> {
    body.id = Some(id);
    Json(body)
}

#[utoipa::path(
    post,
    path = "/failure_handlers",
    operation_id = "create_failure_handler",
    request_body = FailureHandlerModel,
    responses((status = 200, description = "Successful response", body = FailureHandlerModel))
)]
pub async fn create_failure_handler(
    Json(mut body): Json<FailureHandlerModel>,
) -> Json<FailureHandlerModel> {
    if body.id.is_none() {
        body.id = Some(0);
    }
    Json(body)
}

#[utoipa::path(
    get,
    path = "/failure_handlers/{id}",
    operation_id = "get_failure_handler",
    params(("id" = i64, Path, description = "Failure handler ID")),
    responses((status = 200, description = "Successful response", body = FailureHandlerModel))
)]
pub async fn get_failure_handler(Path(id): Path<i64>) -> Json<FailureHandlerModel> {
    Json(example_failure_handler(Some(id), 42))
}

#[utoipa::path(
    delete,
    path = "/failure_handlers/{id}",
    operation_id = "delete_failure_handler",
    params(("id" = i64, Path, description = "Failure handler ID")),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = FailureHandlerModel))
)]
pub async fn delete_failure_handler(
    Path(id): Path<i64>,
    Json(_body): Json<Option<Value>>,
) -> Json<FailureHandlerModel> {
    Json(example_failure_handler(Some(id), 42))
}

#[utoipa::path(
    get,
    path = "/workflows/{id}/failure_handlers",
    operation_id = "list_failure_handlers",
    params(
        ("id" = i64, Path, description = "Workflow ID"),
        FailureHandlersListQuery
    ),
    responses((status = 200, description = "Successful response", body = ListFailureHandlersResponse))
)]
pub async fn list_failure_handlers(
    Path(id): Path<i64>,
    Query(query): Query<FailureHandlersListQuery>,
) -> Json<ListFailureHandlersResponse> {
    Json(ListFailureHandlersResponse {
        items: vec![example_failure_handler(Some(1), id)],
        offset: query.offset.unwrap_or(0),
        max_limit: crate::MAX_RECORD_TRANSFER_COUNT,
        count: 1,
        total_count: 1,
        has_more: false,
    })
}

#[utoipa::path(
    post,
    path = "/slurm_stats",
    operation_id = "create_slurm_stats",
    request_body = SlurmStatsModel,
    responses((status = 200, description = "Successful response", body = SlurmStatsModel))
)]
pub async fn create_slurm_stats(Json(mut body): Json<SlurmStatsModel>) -> Json<SlurmStatsModel> {
    if body.id.is_none() {
        body.id = Some(0);
    }
    Json(body)
}

#[utoipa::path(
    get,
    path = "/slurm_stats",
    operation_id = "list_slurm_stats",
    params(SlurmStatsListQuery),
    responses((status = 200, description = "Successful response", body = ListSlurmStatsResponse))
)]
pub async fn list_slurm_stats(
    Query(query): Query<SlurmStatsListQuery>,
) -> Json<ListSlurmStatsResponse> {
    Json(ListSlurmStatsResponse {
        items: vec![example_slurm_stats(Some(1), query.workflow_id)],
        offset: query.offset.unwrap_or(0),
        max_limit: crate::MAX_RECORD_TRANSFER_COUNT,
        count: 1,
        total_count: 1,
        has_more: false,
    })
}

fn example_resource_requirements(id: Option<i64>, workflow_id: i64) -> ResourceRequirementsModel {
    ResourceRequirementsModel {
        id,
        workflow_id,
        name: "default".to_string(),
        num_cpus: 4,
        num_gpus: 1,
        num_nodes: 1,
        memory: "16g".to_string(),
        runtime: "PT2H".to_string(),
    }
}

fn example_failure_handler(id: Option<i64>, workflow_id: i64) -> FailureHandlerModel {
    FailureHandlerModel {
        id,
        workflow_id,
        name: "simple_retry".to_string(),
        rules: "[{\"match_all_exit_codes\":true,\"max_retries\":3}]".to_string(),
    }
}

fn example_slurm_stats(id: Option<i64>, workflow_id: i64) -> SlurmStatsModel {
    SlurmStatsModel {
        id,
        workflow_id,
        job_id: 10,
        run_id: 2,
        attempt_id: 1,
        slurm_job_id: Some("12345".to_string()),
        max_rss_bytes: Some(1_048_576),
        max_vm_size_bytes: Some(2_097_152),
        max_disk_read_bytes: Some(4_096),
        max_disk_write_bytes: Some(8_192),
        ave_cpu_seconds: Some(120.5),
        node_list: Some("node001".to_string()),
    }
}
