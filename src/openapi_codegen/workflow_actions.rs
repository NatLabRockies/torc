#![allow(dead_code)]

use axum::{
    Json,
    extract::{Path, Query},
};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::models::{ClaimActionRequest, ClaimActionResponse, WorkflowActionModel};

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct PendingActionsQuery {
    #[param(nullable = true)]
    pub trigger_type: Option<Vec<String>>,
}

#[utoipa::path(
    post,
    path = "/workflows/{id}/actions",
    operation_id = "create_workflow_action",
    params(("id" = i64, Path, description = "Workflow ID")),
    request_body = WorkflowActionModel,
    responses((status = 200, description = "Successful response", body = WorkflowActionModel))
)]
pub async fn create_workflow_action(
    Path(id): Path<i64>,
    Json(mut body): Json<WorkflowActionModel>,
) -> Json<WorkflowActionModel> {
    body.workflow_id = id;
    if body.id.is_none() {
        body.id = Some(0);
    }
    Json(body)
}

#[utoipa::path(
    get,
    path = "/workflows/{id}/actions",
    operation_id = "get_workflow_actions",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses((status = 200, description = "Successful response", body = [WorkflowActionModel]))
)]
pub async fn get_workflow_actions(Path(id): Path<i64>) -> Json<Vec<WorkflowActionModel>> {
    Json(vec![example_workflow_action(Some(1), id)])
}

#[utoipa::path(
    get,
    path = "/workflows/{id}/actions/pending",
    operation_id = "get_pending_actions",
    params(
        ("id" = i64, Path, description = "Workflow ID"),
        PendingActionsQuery
    ),
    responses((status = 200, description = "Successful response", body = [WorkflowActionModel]))
)]
pub async fn get_pending_actions(
    Path(id): Path<i64>,
    Query(_query): Query<PendingActionsQuery>,
) -> Json<Vec<WorkflowActionModel>> {
    Json(vec![example_workflow_action(Some(2), id)])
}

#[utoipa::path(
    post,
    path = "/workflows/{id}/actions/{action_id}/claim",
    operation_id = "claim_action",
    params(
        ("id" = i64, Path, description = "Workflow ID"),
        ("action_id" = i64, Path, description = "Action ID")
    ),
    request_body = ClaimActionRequest,
    responses((status = 200, description = "Successful response", body = ClaimActionResponse))
)]
pub async fn claim_action(
    Path((_id, action_id)): Path<(i64, i64)>,
    Json(_body): Json<ClaimActionRequest>,
) -> Json<ClaimActionResponse> {
    Json(ClaimActionResponse {
        action_id,
        success: true,
    })
}

fn example_workflow_action(id: Option<i64>, workflow_id: i64) -> WorkflowActionModel {
    WorkflowActionModel {
        id,
        workflow_id,
        trigger_type: "on_workflow_start".to_string(),
        action_type: "run_commands".to_string(),
        action_config: serde_json::json!({"commands": ["echo ready"]}),
        job_ids: Some(vec![1, 2, 3]),
        trigger_count: 0,
        required_triggers: 1,
        executed: false,
        executed_at: None,
        executed_by: None,
        persistent: false,
        is_recovery: false,
    }
}
