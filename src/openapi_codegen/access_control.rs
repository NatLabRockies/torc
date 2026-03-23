#![allow(dead_code)]

use axum::{
    Json,
    extract::{Path, Query},
};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::models::{
    AccessCheckResponse, AccessGroupModel, ListAccessGroupsResponse,
    ListUserGroupMembershipsResponse, ReloadAuthResponse, UserGroupMembershipModel,
    WorkflowAccessGroupModel,
};

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct AccessPaginationQuery {
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
}

#[utoipa::path(
    post,
    path = "/access_groups",
    operation_id = "create_access_group",
    request_body = AccessGroupModel,
    responses((status = 200, description = "Successful response", body = AccessGroupModel))
)]
pub async fn create_access_group(Json(mut body): Json<AccessGroupModel>) -> Json<AccessGroupModel> {
    if body.id.is_none() {
        body.id = Some(0);
    }
    if body.created_at.is_none() {
        body.created_at = Some("2026-03-21T00:00:00Z".to_string());
    }
    Json(body)
}

#[utoipa::path(
    get,
    path = "/access_groups",
    operation_id = "list_access_groups",
    params(AccessPaginationQuery),
    responses((status = 200, description = "Successful response", body = ListAccessGroupsResponse))
)]
pub async fn list_access_groups(
    Query(query): Query<AccessPaginationQuery>,
) -> Json<ListAccessGroupsResponse> {
    Json(ListAccessGroupsResponse {
        items: vec![example_access_group(Some(1))],
        offset: query.offset.unwrap_or(0),
        limit: query.limit.unwrap_or(100),
        total_count: 1,
        has_more: false,
    })
}

#[utoipa::path(
    get,
    path = "/access_groups/{id}",
    operation_id = "get_access_group",
    params(("id" = i64, Path, description = "ID of the access group")),
    responses((status = 200, description = "Successful response", body = AccessGroupModel))
)]
pub async fn get_access_group(Path(id): Path<i64>) -> Json<AccessGroupModel> {
    Json(example_access_group(Some(id)))
}

#[utoipa::path(
    delete,
    path = "/access_groups/{id}",
    operation_id = "delete_access_group",
    params(("id" = i64, Path, description = "ID of the access group")),
    responses((status = 200, description = "Successful response", body = AccessGroupModel))
)]
pub async fn delete_access_group(Path(id): Path<i64>) -> Json<AccessGroupModel> {
    Json(example_access_group(Some(id)))
}

#[utoipa::path(
    post,
    path = "/access_groups/{id}/members",
    operation_id = "add_user_to_group",
    params(("id" = i64, Path, description = "ID of the access group")),
    request_body = UserGroupMembershipModel,
    responses((status = 200, description = "Successful response", body = UserGroupMembershipModel))
)]
pub async fn add_user_to_group(
    Path(id): Path<i64>,
    Json(mut body): Json<UserGroupMembershipModel>,
) -> Json<UserGroupMembershipModel> {
    body.group_id = id;
    if body.id.is_none() {
        body.id = Some(0);
    }
    Json(body)
}

#[utoipa::path(
    get,
    path = "/access_groups/{id}/members",
    operation_id = "list_group_members",
    params(
        ("id" = i64, Path, description = "ID of the access group"),
        AccessPaginationQuery
    ),
    responses((status = 200, description = "Successful response", body = ListUserGroupMembershipsResponse))
)]
pub async fn list_group_members(
    Path(id): Path<i64>,
    Query(query): Query<AccessPaginationQuery>,
) -> Json<ListUserGroupMembershipsResponse> {
    Json(ListUserGroupMembershipsResponse {
        items: vec![example_membership(id)],
        offset: query.offset.unwrap_or(0),
        limit: query.limit.unwrap_or(100),
        total_count: 1,
        has_more: false,
    })
}

#[utoipa::path(
    delete,
    path = "/access_groups/{id}/members/{user_name}",
    operation_id = "remove_user_from_group",
    params(
        ("id" = i64, Path, description = "ID of the access group"),
        ("user_name" = String, Path, description = "Username to remove")
    ),
    responses((status = 200, description = "Successful response", body = UserGroupMembershipModel))
)]
pub async fn remove_user_from_group(
    Path((id, user_name)): Path<(i64, String)>,
) -> Json<UserGroupMembershipModel> {
    Json(UserGroupMembershipModel {
        id: Some(1),
        user_name,
        group_id: id,
        role: "member".to_string(),
        created_at: Some("2026-03-21T00:00:00Z".to_string()),
    })
}

#[utoipa::path(
    get,
    path = "/users/{user_name}/groups",
    operation_id = "list_user_groups",
    params(
        ("user_name" = String, Path, description = "Username"),
        AccessPaginationQuery
    ),
    responses((status = 200, description = "Successful response", body = ListAccessGroupsResponse))
)]
pub async fn list_user_groups(
    Path(_user_name): Path<String>,
    Query(query): Query<AccessPaginationQuery>,
) -> Json<ListAccessGroupsResponse> {
    Json(ListAccessGroupsResponse {
        items: vec![example_access_group(Some(1))],
        offset: query.offset.unwrap_or(0),
        limit: query.limit.unwrap_or(100),
        total_count: 1,
        has_more: false,
    })
}

#[utoipa::path(
    post,
    path = "/workflows/{id}/access_groups/{group_id}",
    operation_id = "add_workflow_to_group",
    params(
        ("id" = i64, Path, description = "ID of the workflow"),
        ("group_id" = i64, Path, description = "ID of the access group")
    ),
    responses((status = 200, description = "Successful response", body = WorkflowAccessGroupModel))
)]
pub async fn add_workflow_to_group(
    Path((id, group_id)): Path<(i64, i64)>,
) -> Json<WorkflowAccessGroupModel> {
    Json(WorkflowAccessGroupModel {
        workflow_id: id,
        group_id,
        created_at: Some("2026-03-21T00:00:00Z".to_string()),
    })
}

#[utoipa::path(
    get,
    path = "/workflows/{id}/access_groups",
    operation_id = "list_workflow_groups",
    params(
        ("id" = i64, Path, description = "ID of the workflow"),
        AccessPaginationQuery
    ),
    responses((status = 200, description = "Successful response", body = ListAccessGroupsResponse))
)]
pub async fn list_workflow_groups(
    Path(_id): Path<i64>,
    Query(query): Query<AccessPaginationQuery>,
) -> Json<ListAccessGroupsResponse> {
    Json(ListAccessGroupsResponse {
        items: vec![example_access_group(Some(1))],
        offset: query.offset.unwrap_or(0),
        limit: query.limit.unwrap_or(100),
        total_count: 1,
        has_more: false,
    })
}

#[utoipa::path(
    delete,
    path = "/workflows/{id}/access_groups/{group_id}",
    operation_id = "remove_workflow_from_group",
    params(
        ("id" = i64, Path, description = "ID of the workflow"),
        ("group_id" = i64, Path, description = "ID of the access group")
    ),
    responses((status = 200, description = "Successful response", body = WorkflowAccessGroupModel))
)]
pub async fn remove_workflow_from_group(
    Path((id, group_id)): Path<(i64, i64)>,
) -> Json<WorkflowAccessGroupModel> {
    Json(WorkflowAccessGroupModel {
        workflow_id: id,
        group_id,
        created_at: Some("2026-03-21T00:00:00Z".to_string()),
    })
}

#[utoipa::path(
    get,
    path = "/access_check/{workflow_id}/{user_name}",
    operation_id = "check_workflow_access",
    params(
        ("workflow_id" = i64, Path, description = "ID of the workflow"),
        ("user_name" = String, Path, description = "Username to check")
    ),
    responses((status = 200, description = "Successful response", body = AccessCheckResponse))
)]
pub async fn check_workflow_access(
    Path((workflow_id, user_name)): Path<(i64, String)>,
) -> Json<AccessCheckResponse> {
    Json(AccessCheckResponse {
        has_access: true,
        user_name,
        workflow_id,
        reason: None,
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

fn example_access_group(id: Option<i64>) -> AccessGroupModel {
    AccessGroupModel {
        id,
        name: "research".to_string(),
        description: Some("Shared access group".to_string()),
        created_at: Some("2026-03-21T00:00:00Z".to_string()),
    }
}

fn example_membership(group_id: i64) -> UserGroupMembershipModel {
    UserGroupMembershipModel {
        id: Some(1),
        user_name: "alice".to_string(),
        group_id,
        role: "member".to_string(),
        created_at: Some("2026-03-21T00:00:00Z".to_string()),
    }
}
