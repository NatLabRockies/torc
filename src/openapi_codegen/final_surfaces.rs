#![allow(dead_code)]

use axum::{
    Json,
    extract::{Path, Query},
};
use serde::Deserialize;
use serde_json::Value;
use utoipa::IntoParams;

use crate::api_models::{
    ClaimActionRequest, ClaimActionResponse, DeleteRoCrateEntitiesResponse, MessageResponse,
    ReloadAuthResponse, RemoteWorkerModel, RoCrateEntityModel, WorkflowActionModel,
};

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct PendingActionsQuery {
    #[param(nullable = true)]
    pub trigger_type: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct RoCrateEntitiesQuery {
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
    #[param(nullable = true)]
    pub sort_by: Option<String>,
    #[param(nullable = true)]
    pub reverse_sort: Option<bool>,
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

#[utoipa::path(
    post,
    path = "/workflows/{id}/remote_workers",
    operation_id = "create_remote_workers",
    params(("id" = i64, Path, description = "Workflow ID")),
    request_body = Vec<String>,
    responses((status = 200, description = "Successful response", body = [RemoteWorkerModel]))
)]
pub async fn create_remote_workers(
    Path(id): Path<i64>,
    Json(workers): Json<Vec<String>>,
) -> Json<Vec<RemoteWorkerModel>> {
    Json(
        workers
            .into_iter()
            .map(|worker| RemoteWorkerModel {
                worker,
                workflow_id: id,
            })
            .collect(),
    )
}

#[utoipa::path(
    get,
    path = "/workflows/{id}/remote_workers",
    operation_id = "list_remote_workers",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses((status = 200, description = "Successful response", body = [RemoteWorkerModel]))
)]
pub async fn list_remote_workers(Path(id): Path<i64>) -> Json<Vec<RemoteWorkerModel>> {
    Json(vec![RemoteWorkerModel {
        worker: "user@hostname:22".to_string(),
        workflow_id: id,
    }])
}

#[utoipa::path(
    delete,
    path = "/workflows/{id}/remote_workers/{worker}",
    operation_id = "delete_remote_worker",
    params(
        ("id" = i64, Path, description = "Workflow ID"),
        ("worker" = String, Path, description = "Worker address")
    ),
    responses((status = 200, description = "Successful response", body = RemoteWorkerModel))
)]
pub async fn delete_remote_worker(
    Path((id, worker)): Path<(i64, String)>,
) -> Json<RemoteWorkerModel> {
    Json(RemoteWorkerModel {
        worker,
        workflow_id: id,
    })
}

#[utoipa::path(
    post,
    path = "/ro_crate_entities",
    operation_id = "create_ro_crate_entity",
    request_body = RoCrateEntityModel,
    responses((status = 200, description = "Successful response", body = RoCrateEntityModel))
)]
pub async fn create_ro_crate_entity(
    Json(mut body): Json<RoCrateEntityModel>,
) -> Json<RoCrateEntityModel> {
    if body.id.is_none() {
        body.id = Some(0);
    }
    Json(body)
}

#[utoipa::path(
    get,
    path = "/ro_crate_entities/{id}",
    operation_id = "get_ro_crate_entity",
    params(("id" = i64, Path, description = "Entity ID")),
    responses((status = 200, description = "Successful response", body = RoCrateEntityModel))
)]
pub async fn get_ro_crate_entity(Path(id): Path<i64>) -> Json<RoCrateEntityModel> {
    Json(example_ro_crate_entity(Some(id), 1))
}

#[utoipa::path(
    put,
    path = "/ro_crate_entities/{id}",
    operation_id = "update_ro_crate_entity",
    params(("id" = i64, Path, description = "Entity ID")),
    request_body = RoCrateEntityModel,
    responses((status = 200, description = "Successful response", body = RoCrateEntityModel))
)]
pub async fn update_ro_crate_entity(
    Path(id): Path<i64>,
    Json(mut body): Json<RoCrateEntityModel>,
) -> Json<RoCrateEntityModel> {
    body.id = Some(id);
    Json(body)
}

#[utoipa::path(
    delete,
    path = "/ro_crate_entities/{id}",
    operation_id = "delete_ro_crate_entity",
    params(("id" = i64, Path, description = "Entity ID")),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = MessageResponse))
)]
pub async fn delete_ro_crate_entity(
    Path(_id): Path<i64>,
    Json(_body): Json<Option<Value>>,
) -> Json<MessageResponse> {
    Json(MessageResponse {
        message: "RO-Crate entity deleted".to_string(),
    })
}

#[utoipa::path(
    get,
    path = "/workflows/{id}/ro_crate_entities",
    operation_id = "list_ro_crate_entities",
    params(
        ("id" = i64, Path, description = "Workflow ID"),
        RoCrateEntitiesQuery
    ),
    responses((status = 200, description = "Successful response", body = crate::api_models::ListRoCrateEntitiesResponse))
)]
pub async fn list_ro_crate_entities(
    Path(id): Path<i64>,
    Query(query): Query<RoCrateEntitiesQuery>,
) -> Json<crate::api_models::ListRoCrateEntitiesResponse> {
    Json(crate::api_models::ListRoCrateEntitiesResponse {
        items: vec![example_ro_crate_entity(Some(1), id)],
        offset: query.offset.unwrap_or(0),
        max_limit: crate::MAX_RECORD_TRANSFER_COUNT,
        count: 1,
        total_count: 1,
        has_more: false,
    })
}

#[utoipa::path(
    delete,
    path = "/workflows/{id}/ro_crate_entities",
    operation_id = "delete_ro_crate_entities",
    params(("id" = i64, Path, description = "Workflow ID")),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = DeleteRoCrateEntitiesResponse))
)]
pub async fn delete_ro_crate_entities(
    Path(_id): Path<i64>,
    Json(_body): Json<Option<Value>>,
) -> Json<DeleteRoCrateEntitiesResponse> {
    Json(DeleteRoCrateEntitiesResponse {
        message: "RO-Crate entities deleted".to_string(),
        deleted_count: 1,
    })
}

#[utoipa::path(
    post,
    path = "/admin/reload-auth",
    operation_id = "reload_auth",
    responses((status = 200, description = "Successful response", body = ReloadAuthResponse))
)]
pub async fn reload_auth() -> Json<ReloadAuthResponse> {
    Json(ReloadAuthResponse {
        message: "Authentication data reloaded".to_string(),
        user_count: 1,
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

fn example_ro_crate_entity(id: Option<i64>, workflow_id: i64) -> RoCrateEntityModel {
    RoCrateEntityModel {
        id,
        workflow_id,
        file_id: Some(10),
        entity_id: "data/output.parquet".to_string(),
        entity_type: "File".to_string(),
        metadata: "{\"@id\":\"data/output.parquet\",\"@type\":\"File\"}".to_string(),
    }
}
