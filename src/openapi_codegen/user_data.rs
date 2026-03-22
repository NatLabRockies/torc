#![allow(dead_code)]

use axum::{
    Json,
    extract::{Path, Query},
};
use serde::Deserialize;
use serde_json::{Value, json};
use utoipa::IntoParams;

use crate::api_models::{ListUserDataResponse, UserDataModel};

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct CreateUserDataQuery {
    #[param(nullable = true)]
    pub consumer_job_id: Option<i64>,
    #[param(nullable = true)]
    pub producer_job_id: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct UserDataListQuery {
    pub workflow_id: i64,
    #[param(nullable = true)]
    pub consumer_job_id: Option<i64>,
    #[param(nullable = true)]
    pub producer_job_id: Option<i64>,
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
    pub is_ephemeral: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct DeleteAllUserDataQuery {
    pub workflow_id: i64,
}

#[utoipa::path(
    post,
    path = "/user_data",
    operation_id = "create_user_data",
    params(CreateUserDataQuery),
    request_body = UserDataModel,
    responses((status = 200, description = "Successful response", body = UserDataModel))
)]
pub async fn create_user_data(
    Query(_query): Query<CreateUserDataQuery>,
    Json(mut body): Json<UserDataModel>,
) -> Json<UserDataModel> {
    if body.id.is_none() {
        body.id = Some(0);
    }
    Json(body)
}

#[utoipa::path(
    delete,
    path = "/user_data",
    operation_id = "delete_all_user_data",
    params(DeleteAllUserDataQuery),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = Value))
)]
pub async fn delete_all_user_data(
    Query(query): Query<DeleteAllUserDataQuery>,
    Json(_body): Json<Option<Value>>,
) -> Json<Value> {
    Json(json!({
        "message": format!("Deleted 0 user data records for workflow {}", query.workflow_id),
        "deleted_count": 0
    }))
}

#[utoipa::path(
    get,
    path = "/user_data",
    operation_id = "list_user_data",
    params(UserDataListQuery),
    responses((status = 200, description = "Successful response", body = ListUserDataResponse))
)]
pub async fn list_user_data(Query(query): Query<UserDataListQuery>) -> Json<ListUserDataResponse> {
    Json(ListUserDataResponse {
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
    path = "/user_data/{id}",
    operation_id = "delete_user_data",
    params(("id" = i64, Path, description = "User data record ID")),
    request_body = Option<Value>,
    responses((status = 200, description = "Successful response", body = UserDataModel))
)]
pub async fn delete_user_data(
    Path(id): Path<i64>,
    Json(_body): Json<Option<Value>>,
) -> Json<UserDataModel> {
    Json(example_user_data(Some(id)))
}

#[utoipa::path(
    get,
    path = "/user_data/{id}",
    operation_id = "get_user_data",
    params(("id" = i64, Path, description = "User data record ID")),
    responses((status = 200, description = "Successful response", body = UserDataModel))
)]
pub async fn get_user_data(Path(id): Path<i64>) -> Json<UserDataModel> {
    Json(example_user_data(Some(id)))
}

#[utoipa::path(
    put,
    path = "/user_data/{id}",
    operation_id = "update_user_data",
    params(("id" = i64, Path, description = "User data record ID")),
    request_body = UserDataModel,
    responses((status = 200, description = "Successful response", body = UserDataModel))
)]
pub async fn update_user_data(
    Path(id): Path<i64>,
    Json(mut body): Json<UserDataModel>,
) -> Json<UserDataModel> {
    body.id = Some(id);
    Json(body)
}

pub fn example_user_data(id: Option<i64>) -> UserDataModel {
    UserDataModel {
        id,
        workflow_id: 6,
        is_ephemeral: Some(false),
        name: "name".to_string(),
        data: Some(json!({})),
    }
}
