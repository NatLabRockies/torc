#![allow(dead_code)]

use axum::{
    Json,
    extract::{Path, Query},
};
use serde::Deserialize;
use serde_json::Value;
use utoipa::IntoParams;

use crate::api_models::{DeleteCountResponse, JobModel, JobStatus, ListJobsResponse, ResultModel};

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct JobsListQuery {
    pub workflow_id: i64,
    #[param(nullable = true)]
    pub status: Option<JobStatus>,
    #[param(nullable = true)]
    pub needs_file_id: Option<i64>,
    #[param(nullable = true)]
    pub upstream_job_id: Option<i64>,
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
    #[param(nullable = true)]
    pub sort_by: Option<String>,
    #[param(nullable = true)]
    pub reverse_sort: Option<bool>,
    #[param(nullable = true)]
    pub include_relationships: Option<bool>,
    #[param(nullable = true)]
    pub active_compute_node_id: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct DeleteJobsQuery {
    pub workflow_id: i64,
}

#[utoipa::path(
    post,
    path = "/jobs",
    operation_id = "create_job",
    request_body = JobModel,
    responses((status = 200, description = "Successful response", body = JobModel))
)]
pub async fn create_job(Json(mut body): Json<JobModel>) -> Json<JobModel> {
    if body.id.is_none() {
        body.id = Some(0);
    }
    Json(body)
}

#[utoipa::path(
    delete,
    path = "/jobs",
    operation_id = "delete_jobs",
    params(DeleteJobsQuery),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = DeleteCountResponse))
)]
pub async fn delete_jobs(
    Query(_query): Query<DeleteJobsQuery>,
    Json(_body): Json<Option<Value>>,
) -> Json<DeleteCountResponse> {
    Json(DeleteCountResponse { count: 0 })
}

#[utoipa::path(
    get,
    path = "/jobs",
    operation_id = "list_jobs",
    params(JobsListQuery),
    responses((status = 200, description = "Successful response", body = ListJobsResponse))
)]
pub async fn list_jobs(Query(query): Query<JobsListQuery>) -> Json<ListJobsResponse> {
    Json(ListJobsResponse {
        items: vec![],
        offset: query.offset.unwrap_or(0),
        max_limit: crate::MAX_RECORD_TRANSFER_COUNT,
        count: 0,
        total_count: 0,
        has_more: false,
    })
}

#[utoipa::path(
    delete,
    path = "/jobs/{id}",
    operation_id = "delete_job",
    params(("id" = i64, Path, description = "Job ID")),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = JobModel))
)]
pub async fn delete_job(Path(id): Path<i64>, Json(_body): Json<Option<Value>>) -> Json<JobModel> {
    Json(example_job(Some(id)))
}

#[utoipa::path(
    get,
    path = "/jobs/{id}",
    operation_id = "get_job",
    params(("id" = i64, Path, description = "ID of the job record")),
    responses((status = 200, description = "Successful response", body = JobModel))
)]
pub async fn get_job(Path(id): Path<i64>) -> Json<JobModel> {
    Json(example_job(Some(id)))
}

#[utoipa::path(
    put,
    path = "/jobs/{id}",
    operation_id = "update_job",
    params(("id" = i64, Path, description = "ID of the job.")),
    request_body = JobModel,
    responses((status = 200, description = "Successful response", body = JobModel))
)]
pub async fn update_job(Path(id): Path<i64>, Json(mut body): Json<JobModel>) -> Json<JobModel> {
    body.id = Some(id);
    Json(body)
}

#[utoipa::path(
    post,
    path = "/jobs/{id}/complete_job/{status}/{run_id}",
    operation_id = "complete_job",
    params(
        ("id" = i64, Path, description = "Job ID"),
        ("status" = JobStatus, Path, description = "New job status."),
        ("run_id" = i64, Path, description = "Current job run ID")
    ),
    request_body = ResultModel,
    responses((status = 200, description = "Successful response", body = JobModel))
)]
pub async fn complete_job(
    Path((id, status, run_id)): Path<(i64, JobStatus, i64)>,
    Json(body): Json<ResultModel>,
) -> Json<JobModel> {
    let mut job = example_job(Some(id));
    job.status = Some(status);
    job.attempt_id = body.attempt_id.or(Some(run_id));
    Json(job)
}

#[utoipa::path(
    put,
    path = "/jobs/{id}/manage_status_change/{status}/{run_id}",
    operation_id = "manage_status_change",
    params(
        ("id" = i64, Path, description = "Job ID"),
        ("status" = JobStatus, Path, description = "New job status"),
        ("run_id" = i64, Path, description = "Current job run ID")
    ),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = JobModel))
)]
pub async fn manage_status_change(
    Path((id, status, run_id)): Path<(i64, JobStatus, i64)>,
    Json(_body): Json<Option<Value>>,
) -> Json<JobModel> {
    let mut job = example_job(Some(id));
    job.status = Some(status);
    job.attempt_id = Some(run_id);
    Json(job)
}

#[utoipa::path(
    put,
    path = "/jobs/{id}/start_job/{run_id}/{compute_node_id}",
    operation_id = "start_job",
    params(
        ("id" = i64, Path, description = "Job ID"),
        ("run_id" = i64, Path, description = "Current job run ID"),
        ("compute_node_id" = i64, Path, description = "Compute node ID that started the job")
    ),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = JobModel))
)]
pub async fn start_job(
    Path((id, run_id, _compute_node_id)): Path<(i64, i64, i64)>,
    Json(_body): Json<Option<Value>>,
) -> Json<JobModel> {
    let mut job = example_job(Some(id));
    job.status = Some(JobStatus::Running);
    job.attempt_id = Some(run_id);
    Json(job)
}

#[utoipa::path(
    post,
    path = "/jobs/{id}/retry/{run_id}",
    operation_id = "retry_job",
    params(
        ("id" = i64, Path, description = "Job ID"),
        ("run_id" = i64, Path, description = "Current workflow run ID")
    ),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = JobModel))
)]
pub async fn retry_job(
    Path((id, run_id)): Path<(i64, i64)>,
    Json(_body): Json<Option<Value>>,
) -> Json<JobModel> {
    let mut job = example_job(Some(id));
    job.status = Some(JobStatus::Ready);
    job.attempt_id = Some(run_id + 1);
    Json(job)
}

pub fn example_job(id: Option<i64>) -> JobModel {
    JobModel {
        id,
        workflow_id: 6,
        name: "name".to_string(),
        command: "command".to_string(),
        invocation_script: Some("invocation_script".to_string()),
        status: Some(JobStatus::Ready),
        schedule_compute_nodes: None,
        cancel_on_blocking_job_failure: Some(true),
        supports_termination: Some(false),
        depends_on_job_ids: Some(vec![5, 5]),
        input_file_ids: Some(vec![2, 2]),
        output_file_ids: Some(vec![7, 7]),
        input_user_data_ids: Some(vec![9, 9]),
        output_user_data_ids: Some(vec![3, 3]),
        resource_requirements_id: Some(2),
        scheduler_id: Some(4),
        failure_handler_id: Some(8),
        attempt_id: Some(1),
    }
}
