#![allow(dead_code)]

use axum::{
    Json,
    extract::{Path, Query},
};
use serde::Deserialize;
use serde_json::Value;
use utoipa::IntoParams;

use crate::api_models::{DeleteCountResponse, ListLocalSchedulersResponse, LocalSchedulerModel};

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct LocalSchedulersListQuery {
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
    pub memory: Option<String>,
    #[param(nullable = true)]
    pub num_cpus: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct DeleteLocalSchedulersQuery {
    pub workflow_id: i64,
}

#[utoipa::path(
    post,
    path = "/local_schedulers",
    operation_id = "create_local_scheduler",
    request_body = LocalSchedulerModel,
    responses((status = 200, description = "Successful response", body = LocalSchedulerModel))
)]
pub async fn create_local_scheduler(
    Json(mut body): Json<LocalSchedulerModel>,
) -> Json<LocalSchedulerModel> {
    if body.id.is_none() {
        body.id = Some(0);
    }
    Json(body)
}

#[utoipa::path(
    delete,
    path = "/local_schedulers",
    operation_id = "delete_local_schedulers",
    params(DeleteLocalSchedulersQuery),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = DeleteCountResponse))
)]
pub async fn delete_local_schedulers(
    Query(_query): Query<DeleteLocalSchedulersQuery>,
    Json(_body): Json<Option<Value>>,
) -> Json<DeleteCountResponse> {
    Json(DeleteCountResponse { count: 0 })
}

#[utoipa::path(
    get,
    path = "/local_schedulers",
    operation_id = "list_local_schedulers",
    params(LocalSchedulersListQuery),
    responses((status = 200, description = "HTTP 200 OK.", body = ListLocalSchedulersResponse))
)]
pub async fn list_local_schedulers(
    Query(query): Query<LocalSchedulersListQuery>,
) -> Json<ListLocalSchedulersResponse> {
    Json(ListLocalSchedulersResponse {
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
    path = "/local_schedulers/{id}",
    operation_id = "delete_local_scheduler",
    params(("id" = i64, Path, description = "ID of the local compute node configuration record.")),
    request_body = Option<Value>,
    responses((status = 200, description = "local compute node configuration stored in the table.", body = LocalSchedulerModel))
)]
pub async fn delete_local_scheduler(
    Path(id): Path<i64>,
    Json(_body): Json<Option<Value>>,
) -> Json<LocalSchedulerModel> {
    Json(example_local_scheduler(Some(id)))
}

#[utoipa::path(
    get,
    path = "/local_schedulers/{id}",
    operation_id = "get_local_scheduler",
    params(("id" = i64, Path, description = "Scheduler ID")),
    responses((status = 200, description = "Successful response", body = LocalSchedulerModel))
)]
pub async fn get_local_scheduler(Path(id): Path<i64>) -> Json<LocalSchedulerModel> {
    Json(example_local_scheduler(Some(id)))
}

#[utoipa::path(
    put,
    path = "/local_schedulers/{id}",
    operation_id = "update_local_scheduler",
    params(("id" = i64, Path, description = "Scheduler ID")),
    request_body = LocalSchedulerModel,
    responses((status = 200, description = "Successful response", body = LocalSchedulerModel))
)]
pub async fn update_local_scheduler(
    Path(id): Path<i64>,
    Json(mut body): Json<LocalSchedulerModel>,
) -> Json<LocalSchedulerModel> {
    body.id = Some(id);
    Json(body)
}

pub fn example_local_scheduler(id: Option<i64>) -> LocalSchedulerModel {
    LocalSchedulerModel {
        id,
        workflow_id: 6,
        name: Some("default".to_string()),
        memory: Some("memory".to_string()),
        num_cpus: Some(1),
    }
}
