#![allow(dead_code)]

use axum::{
    Json,
    extract::{Path, Query},
};
use serde::Deserialize;
use serde_json::Value;
use utoipa::IntoParams;

use crate::models::{DeleteCountResponse, JobStatus, ListResultsResponse, ResultModel};

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ResultsListQuery {
    pub workflow_id: i64,
    #[param(nullable = true)]
    pub job_id: Option<i64>,
    #[param(nullable = true)]
    pub run_id: Option<i64>,
    #[param(nullable = true)]
    pub return_code: Option<i64>,
    #[param(nullable = true)]
    pub status: Option<JobStatus>,
    #[param(nullable = true)]
    pub compute_node_id: Option<i64>,
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
    #[param(nullable = true)]
    pub sort_by: Option<String>,
    #[param(nullable = true)]
    pub reverse_sort: Option<bool>,
    #[param(nullable = true)]
    pub all_runs: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct DeleteResultsQuery {
    pub workflow_id: i64,
}

#[utoipa::path(
    post,
    path = "/results",
    operation_id = "create_result",
    request_body = ResultModel,
    responses((status = 200, description = "Successful response", body = ResultModel))
)]
pub async fn create_result(Json(mut body): Json<ResultModel>) -> Json<ResultModel> {
    if body.id.is_none() {
        body.id = Some(0);
    }
    Json(body)
}

#[utoipa::path(
    delete,
    path = "/results",
    operation_id = "delete_results",
    params(DeleteResultsQuery),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = DeleteCountResponse))
)]
pub async fn delete_results(
    Query(_query): Query<DeleteResultsQuery>,
    Json(_body): Json<Option<Value>>,
) -> Json<DeleteCountResponse> {
    Json(DeleteCountResponse { count: 0 })
}

#[utoipa::path(
    get,
    path = "/results",
    operation_id = "list_results",
    params(ResultsListQuery),
    responses((status = 200, description = "Successful response", body = ListResultsResponse))
)]
pub async fn list_results(Query(query): Query<ResultsListQuery>) -> Json<ListResultsResponse> {
    Json(ListResultsResponse {
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
    path = "/results/{id}",
    operation_id = "delete_result",
    params(("id" = i64, Path, description = "Results ID")),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = ResultModel))
)]
pub async fn delete_result(
    Path(id): Path<i64>,
    Json(_body): Json<Option<Value>>,
) -> Json<ResultModel> {
    Json(example_result(Some(id)))
}

#[utoipa::path(
    get,
    path = "/results/{id}",
    operation_id = "get_result",
    params(("id" = i64, Path, description = "Results ID")),
    responses((status = 200, description = "Successful response", body = ResultModel))
)]
pub async fn get_result(Path(id): Path<i64>) -> Json<ResultModel> {
    Json(example_result(Some(id)))
}

#[utoipa::path(
    put,
    path = "/results/{id}",
    operation_id = "update_result",
    params(("id" = i64, Path, description = "Result ID")),
    request_body = ResultModel,
    responses((status = 200, description = "Successful response", body = ResultModel))
)]
pub async fn update_result(
    Path(id): Path<i64>,
    Json(mut body): Json<ResultModel>,
) -> Json<ResultModel> {
    body.id = Some(id);
    Json(body)
}

pub fn example_result(id: Option<i64>) -> ResultModel {
    ResultModel {
        id,
        job_id: 6,
        workflow_id: 1,
        run_id: 5,
        attempt_id: Some(1),
        compute_node_id: 1,
        return_code: 5,
        exec_time_minutes: 2.3021358869347655,
        completion_time: "completion_time".to_string(),
        peak_memory_bytes: Some(524_288_000),
        avg_memory_bytes: Some(419_430_400),
        peak_cpu_percent: Some(150.5),
        avg_cpu_percent: Some(85.2),
        status: JobStatus::Completed,
    }
}
