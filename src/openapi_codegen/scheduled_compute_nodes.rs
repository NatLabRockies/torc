#![allow(dead_code)]

use axum::{
    Json,
    extract::{Path, Query},
};
use serde::Deserialize;
use serde_json::Value;
use utoipa::IntoParams;

use crate::models::{
    DeleteCountResponse, ListScheduledComputeNodesResponse, ScheduledComputeNodesModel,
};

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ScheduledComputeNodesListQuery {
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
    pub scheduler_id: Option<String>,
    #[param(nullable = true)]
    pub scheduler_config_id: Option<String>,
    #[param(nullable = true)]
    pub status: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct DeleteScheduledComputeNodesQuery {
    pub workflow_id: i64,
}

#[utoipa::path(
    post,
    path = "/scheduled_compute_nodes",
    operation_id = "create_scheduled_compute_node",
    request_body = ScheduledComputeNodesModel,
    responses((status = 200, description = "Successful response", body = ScheduledComputeNodesModel))
)]
pub async fn create_scheduled_compute_node(
    Json(mut body): Json<ScheduledComputeNodesModel>,
) -> Json<ScheduledComputeNodesModel> {
    if body.id.is_none() {
        body.id = Some(0);
    }
    Json(body)
}

#[utoipa::path(
    delete,
    path = "/scheduled_compute_nodes",
    operation_id = "delete_scheduled_compute_nodes",
    params(DeleteScheduledComputeNodesQuery),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = DeleteCountResponse))
)]
pub async fn delete_scheduled_compute_nodes(
    Query(_query): Query<DeleteScheduledComputeNodesQuery>,
    Json(_body): Json<Option<Value>>,
) -> Json<DeleteCountResponse> {
    Json(DeleteCountResponse { count: 0 })
}

#[utoipa::path(
    get,
    path = "/scheduled_compute_nodes",
    operation_id = "list_scheduled_compute_nodes",
    params(ScheduledComputeNodesListQuery),
    responses((status = 200, description = "Successful response", body = ListScheduledComputeNodesResponse))
)]
pub async fn list_scheduled_compute_nodes(
    Query(query): Query<ScheduledComputeNodesListQuery>,
) -> Json<ListScheduledComputeNodesResponse> {
    Json(ListScheduledComputeNodesResponse {
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
    path = "/scheduled_compute_nodes/{id}",
    operation_id = "delete_scheduled_compute_node",
    params(("id" = i64, Path, description = "Scheduled compute node ID")),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = ScheduledComputeNodesModel))
)]
pub async fn delete_scheduled_compute_node(
    Path(id): Path<i64>,
    Json(_body): Json<Option<Value>>,
) -> Json<ScheduledComputeNodesModel> {
    Json(example_scheduled_compute_node(Some(id)))
}

#[utoipa::path(
    get,
    path = "/scheduled_compute_nodes/{id}",
    operation_id = "get_scheduled_compute_node",
    params(("id" = i64, Path, description = "ID of the scheduled_compute_nodes record")),
    responses((status = 200, description = "HTTP 200 OK.", body = ScheduledComputeNodesModel))
)]
pub async fn get_scheduled_compute_node(Path(id): Path<i64>) -> Json<ScheduledComputeNodesModel> {
    Json(example_scheduled_compute_node(Some(id)))
}

#[utoipa::path(
    put,
    path = "/scheduled_compute_nodes/{id}",
    operation_id = "update_scheduled_compute_node",
    params(("id" = i64, Path, description = "Scheduled compute node ID")),
    request_body = ScheduledComputeNodesModel,
    responses((status = 200, description = "scheduled compute node updated in the table.", body = ScheduledComputeNodesModel))
)]
pub async fn update_scheduled_compute_node(
    Path(id): Path<i64>,
    Json(mut body): Json<ScheduledComputeNodesModel>,
) -> Json<ScheduledComputeNodesModel> {
    body.id = Some(id);
    Json(body)
}

pub fn example_scheduled_compute_node(id: Option<i64>) -> ScheduledComputeNodesModel {
    ScheduledComputeNodesModel {
        id,
        workflow_id: 6,
        scheduler_id: 1,
        scheduler_config_id: 5,
        scheduler_type: "local".to_string(),
        status: "status".to_string(),
    }
}
