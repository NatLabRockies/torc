#![allow(dead_code)]

use axum::{
    Json,
    extract::{Path, Query},
};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::models::{ComputeNodeModel, DeleteCountResponse, ListComputeNodesResponse};

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ComputeNodesListQuery {
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
    pub hostname: Option<String>,
    #[param(nullable = true)]
    pub is_active: Option<bool>,
    #[param(nullable = true)]
    pub scheduled_compute_node_id: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct DeleteComputeNodesQuery {
    pub workflow_id: i64,
}

#[utoipa::path(
    post,
    path = "/compute_nodes",
    operation_id = "create_compute_node",
    request_body = ComputeNodeModel,
    responses((status = 200, description = "Successful response", body = ComputeNodeModel))
)]
pub async fn create_compute_node(Json(mut body): Json<ComputeNodeModel>) -> Json<ComputeNodeModel> {
    if body.id.is_none() {
        body.id = Some(0);
    }
    Json(body)
}

#[utoipa::path(
    delete,
    path = "/compute_nodes",
    operation_id = "delete_compute_nodes",
    params(DeleteComputeNodesQuery),
    responses((status = 200, description = "Successful response", body = DeleteCountResponse))
)]
pub async fn delete_compute_nodes(
    Query(_query): Query<DeleteComputeNodesQuery>,
) -> Json<DeleteCountResponse> {
    Json(DeleteCountResponse { count: 0 })
}

#[utoipa::path(
    get,
    path = "/compute_nodes",
    operation_id = "list_compute_nodes",
    params(ComputeNodesListQuery),
    responses((status = 200, description = "Successful response", body = ListComputeNodesResponse))
)]
pub async fn list_compute_nodes(
    Query(query): Query<ComputeNodesListQuery>,
) -> Json<ListComputeNodesResponse> {
    Json(ListComputeNodesResponse {
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
    path = "/compute_nodes/{id}",
    operation_id = "delete_compute_node",
    params(("id" = i64, Path, description = "ID of the compute node")),
    responses((status = 200, description = "Successful response", body = ComputeNodeModel))
)]
pub async fn delete_compute_node(Path(id): Path<i64>) -> Json<ComputeNodeModel> {
    Json(example_compute_node(Some(id)))
}

#[utoipa::path(
    get,
    path = "/compute_nodes/{id}",
    operation_id = "get_compute_node",
    params(("id" = i64, Path, description = "ID of the compute node record")),
    responses((status = 200, description = "Successful response", body = ComputeNodeModel))
)]
pub async fn get_compute_node(Path(id): Path<i64>) -> Json<ComputeNodeModel> {
    Json(example_compute_node(Some(id)))
}

#[utoipa::path(
    put,
    path = "/compute_nodes/{id}",
    operation_id = "update_compute_node",
    params(("id" = i64, Path, description = "ID of the compute node.")),
    request_body = ComputeNodeModel,
    responses((status = 200, description = "Successful response", body = ComputeNodeModel))
)]
pub async fn update_compute_node(
    Path(id): Path<i64>,
    Json(mut body): Json<ComputeNodeModel>,
) -> Json<ComputeNodeModel> {
    body.id = Some(id);
    Json(body)
}

pub fn example_compute_node(id: Option<i64>) -> ComputeNodeModel {
    ComputeNodeModel {
        id,
        workflow_id: 37,
        hostname: "hostname".to_string(),
        pid: 6,
        start_time: "start_time".to_string(),
        duration_seconds: Some(1.4658129805029452),
        is_active: Some(true),
        num_cpus: 5,
        memory_gb: 5.637376656633329,
        num_gpus: 2,
        num_nodes: 7,
        time_limit: Some("time_limit".to_string()),
        scheduler_config_id: Some(9),
        compute_node_type: "local".to_string(),
        scheduler: Some(serde_json::json!({})),
    }
}
