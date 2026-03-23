#![allow(dead_code)]

use axum::{
    Json,
    extract::{Path, Query},
};
use serde::Deserialize;
use serde_json::Value;
use utoipa::IntoParams;

use crate::models::{DeleteRoCrateEntitiesResponse, MessageResponse, RoCrateEntityModel};

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
    responses((status = 200, description = "Successful response", body = MessageResponse))
)]
pub async fn delete_ro_crate_entity(Path(_id): Path<i64>) -> Json<MessageResponse> {
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
    responses((status = 200, description = "Successful response", body = crate::models::ListRoCrateEntitiesResponse))
)]
pub async fn list_ro_crate_entities(
    Path(id): Path<i64>,
    Query(query): Query<RoCrateEntitiesQuery>,
) -> Json<crate::models::ListRoCrateEntitiesResponse> {
    Json(crate::models::ListRoCrateEntitiesResponse {
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
