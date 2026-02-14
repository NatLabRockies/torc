//! Access group-related API endpoints for team-based access control

#![allow(clippy::too_many_arguments)]

use log::{debug, info};
use serde_json::json;
use sqlx::Row;
use swagger::{ApiError, Has, XSpanIdString};

use super::{ApiContext, MAX_RECORD_TRANSFER_COUNT, SqlQueryBuilder, database_error_with_msg};
use crate::models;
use crate::server::api_types::{
    AddUserToGroupResponse, AddWorkflowToGroupResponse, CheckWorkflowAccessResponse,
    CreateAccessGroupResponse, DeleteAccessGroupResponse, GetAccessGroupResponse,
    ListAccessGroupsApiResponse, ListGroupMembersResponse, ListUserGroupsApiResponse,
    ListWorkflowGroupsResponse, RemoveUserFromGroupResponse, RemoveWorkflowFromGroupResponse,
};

// ============================================================================
// API Implementation
// ============================================================================

/// Implementation of access groups API for the server
#[derive(Clone)]
pub struct AccessGroupsApiImpl {
    pub context: ApiContext,
}

impl AccessGroupsApiImpl {
    pub fn new(context: ApiContext) -> Self {
        Self { context }
    }

    /// Get all group IDs that a user belongs to
    pub async fn get_user_group_ids(&self, user_name: &str) -> Result<Vec<i64>, ApiError> {
        let records =
            match sqlx::query("SELECT group_id FROM user_group_membership WHERE user_name = $1")
                .bind(user_name)
                .fetch_all(self.context.pool.as_ref())
                .await
            {
                Ok(rows) => rows,
                Err(e) => {
                    return Err(database_error_with_msg(e, "Failed to get user groups"));
                }
            };

        Ok(records.into_iter().map(|row| row.get("group_id")).collect())
    }

    /// Check if user can access workflow (used internally by other APIs)
    pub async fn check_workflow_access_internal(
        &self,
        user_name: &str,
        workflow_id: i64,
    ) -> Result<bool, ApiError> {
        // Check 1: Is the user the owner of the workflow?
        let is_owner: bool = match sqlx::query(
            "SELECT EXISTS(SELECT 1 FROM workflow WHERE id = $1 AND user = $2) as is_owner",
        )
        .bind(workflow_id)
        .bind(user_name)
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(row) => row.get::<i32, _>("is_owner") == 1,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to check workflow access",
                ));
            }
        };

        if is_owner {
            return Ok(true);
        }

        // Check 2: Does the user belong to any group that has access to this workflow?
        let has_group_access: bool = match sqlx::query(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM workflow_access_group wag
                INNER JOIN user_group_membership ugm ON wag.group_id = ugm.group_id
                WHERE wag.workflow_id = $1 AND ugm.user_name = $2
            ) as has_access
            "#,
        )
        .bind(workflow_id)
        .bind(user_name)
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(row) => row.get::<i32, _>("has_access") == 1,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to check workflow access",
                ));
            }
        };

        Ok(has_group_access)
    }

    // ========================================================================
    // Group CRUD operations
    // ========================================================================

    pub async fn create_access_group<C>(
        &self,
        body: models::AccessGroupModel,
        context: &C,
    ) -> Result<CreateAccessGroupResponse, ApiError>
    where
        C: Has<XSpanIdString> + Send + Sync,
    {
        debug!(
            "create_access_group({:?}) - X-Span-ID: {:?}",
            body,
            context.get().0.clone()
        );

        let result = match sqlx::query(
            r#"
            INSERT INTO access_group (name, description)
            VALUES ($1, $2)
            RETURNING id, name, description, created_at
            "#,
        )
        .bind(&body.name)
        .bind(&body.description)
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(row) => row,
            Err(e) => {
                if e.to_string().contains("UNIQUE constraint failed") {
                    return Ok(CreateAccessGroupResponse::DefaultErrorResponse(
                        models::ErrorResponse::new(json!({
                            "error": "Conflict",
                            "message": format!("Group '{}' already exists", body.name)
                        })),
                    ));
                }
                return Err(database_error_with_msg(e, "Failed to create access group"));
            }
        };

        let group = models::AccessGroupModel {
            id: Some(result.get("id")),
            name: result.get("name"),
            description: result.get("description"),
            created_at: result.get("created_at"),
        };

        Ok(CreateAccessGroupResponse::SuccessfulResponse(group))
    }

    pub async fn get_access_group<C>(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetAccessGroupResponse, ApiError>
    where
        C: Has<XSpanIdString> + Send + Sync,
    {
        debug!(
            "get_access_group({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        let record = match sqlx::query(
            r#"
            SELECT id, name, description, created_at
            FROM access_group
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(row)) => row,
            Ok(None) => {
                return Ok(GetAccessGroupResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(json!({
                        "error": "NotFound",
                        "message": format!("Group not found with ID: {}", id)
                    })),
                ));
            }
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to fetch access group"));
            }
        };

        let group = models::AccessGroupModel {
            id: Some(record.get("id")),
            name: record.get("name"),
            description: record.get("description"),
            created_at: record.get("created_at"),
        };

        Ok(GetAccessGroupResponse::SuccessfulResponse(group))
    }

    pub async fn list_access_groups<C>(
        &self,
        offset: i64,
        limit: i64,
        context: &C,
    ) -> Result<ListAccessGroupsApiResponse, ApiError>
    where
        C: Has<XSpanIdString> + Send + Sync,
    {
        debug!(
            "list_access_groups({}, {}) - X-Span-ID: {:?}",
            offset,
            limit,
            context.get().0.clone()
        );

        let effective_limit = std::cmp::min(limit, MAX_RECORD_TRANSFER_COUNT);

        let query = SqlQueryBuilder::new(
            "SELECT id, name, description, created_at FROM access_group".to_string(),
        )
        .with_pagination_and_sorting(
            offset,
            effective_limit,
            Some("name".to_string()),
            None,
            "id",
        )
        .build();

        let records = match sqlx::query(&query)
            .fetch_all(self.context.pool.as_ref())
            .await
        {
            Ok(rows) => rows,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to list access groups"));
            }
        };

        let items: Vec<models::AccessGroupModel> = records
            .into_iter()
            .map(|row| models::AccessGroupModel {
                id: Some(row.get("id")),
                name: row.get("name"),
                description: row.get("description"),
                created_at: row.get("created_at"),
            })
            .collect();

        // Get total count
        let total_count: i64 = match sqlx::query("SELECT COUNT(*) as total FROM access_group")
            .fetch_one(self.context.pool.as_ref())
            .await
        {
            Ok(row) => row.get("total"),
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to count access groups"));
            }
        };

        let response =
            models::ListAccessGroupsResponse::new(items, offset, effective_limit, total_count);

        Ok(ListAccessGroupsApiResponse::SuccessfulResponse(response))
    }

    pub async fn delete_access_group<C>(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteAccessGroupResponse, ApiError>
    where
        C: Has<XSpanIdString> + Send + Sync,
    {
        debug!(
            "delete_access_group({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        // First get the group to return it after deletion
        let group = match self.get_access_group(id, context).await? {
            GetAccessGroupResponse::SuccessfulResponse(g) => g,
            GetAccessGroupResponse::NotFoundErrorResponse(e) => {
                return Ok(DeleteAccessGroupResponse::NotFoundErrorResponse(e));
            }
            GetAccessGroupResponse::DefaultErrorResponse(e) => {
                return Ok(DeleteAccessGroupResponse::DefaultErrorResponse(e));
            }
        };

        match sqlx::query("DELETE FROM access_group WHERE id = $1")
            .bind(id)
            .execute(self.context.pool.as_ref())
            .await
        {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    return Ok(DeleteAccessGroupResponse::NotFoundErrorResponse(
                        models::ErrorResponse::new(json!({
                            "error": "NotFound",
                            "message": format!("Group not found with ID: {}", id)
                        })),
                    ));
                }
                info!("Deleted access group with id: {}", id);
                Ok(DeleteAccessGroupResponse::SuccessfulResponse(group))
            }
            Err(e) => Err(database_error_with_msg(e, "Failed to delete access group")),
        }
    }

    // ========================================================================
    // User-Group membership operations
    // ========================================================================

    pub async fn add_user_to_group<C>(
        &self,
        group_id: i64,
        body: models::UserGroupMembershipModel,
        context: &C,
    ) -> Result<AddUserToGroupResponse, ApiError>
    where
        C: Has<XSpanIdString> + Send + Sync,
    {
        debug!(
            "add_user_to_group({}, {:?}) - X-Span-ID: {:?}",
            group_id,
            body,
            context.get().0.clone()
        );

        // Verify group exists
        match self.get_access_group(group_id, context).await? {
            GetAccessGroupResponse::SuccessfulResponse(_) => {}
            GetAccessGroupResponse::NotFoundErrorResponse(e) => {
                return Ok(AddUserToGroupResponse::NotFoundErrorResponse(e));
            }
            GetAccessGroupResponse::DefaultErrorResponse(e) => {
                return Ok(AddUserToGroupResponse::DefaultErrorResponse(e));
            }
        }

        let result = match sqlx::query(
            r#"
            INSERT INTO user_group_membership (user_name, group_id, role)
            VALUES ($1, $2, $3)
            RETURNING id, user_name, group_id, role, created_at
            "#,
        )
        .bind(&body.user_name)
        .bind(group_id)
        .bind(&body.role)
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(row) => row,
            Err(e) => {
                if e.to_string().contains("UNIQUE constraint failed") {
                    return Ok(AddUserToGroupResponse::DefaultErrorResponse(
                        models::ErrorResponse::new(json!({
                            "error": "Conflict",
                            "message": format!("User '{}' is already a member of group {}", body.user_name, group_id)
                        })),
                    ));
                }
                return Err(database_error_with_msg(e, "Failed to add user to group"));
            }
        };

        let membership = models::UserGroupMembershipModel {
            id: Some(result.get("id")),
            user_name: result.get("user_name"),
            group_id: result.get("group_id"),
            role: result.get("role"),
            created_at: result.get("created_at"),
        };

        Ok(AddUserToGroupResponse::SuccessfulResponse(membership))
    }

    pub async fn remove_user_from_group<C>(
        &self,
        group_id: i64,
        user_name: &str,
        context: &C,
    ) -> Result<RemoveUserFromGroupResponse, ApiError>
    where
        C: Has<XSpanIdString> + Send + Sync,
    {
        debug!(
            "remove_user_from_group({}, {}) - X-Span-ID: {:?}",
            group_id,
            user_name,
            context.get().0.clone()
        );

        // First get the membership to return it after deletion
        let membership = match sqlx::query(
            r#"
            SELECT id, user_name, group_id, role, created_at
            FROM user_group_membership
            WHERE user_name = $1 AND group_id = $2
            "#,
        )
        .bind(user_name)
        .bind(group_id)
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(row)) => models::UserGroupMembershipModel {
                id: Some(row.get("id")),
                user_name: row.get("user_name"),
                group_id: row.get("group_id"),
                role: row.get("role"),
                created_at: row.get("created_at"),
            },
            Ok(None) => {
                return Ok(RemoveUserFromGroupResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(json!({
                        "error": "NotFound",
                        "message": format!("Membership not found for user '{}' in group {}", user_name, group_id)
                    })),
                ));
            }
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to fetch group membership",
                ));
            }
        };

        match sqlx::query(
            "DELETE FROM user_group_membership WHERE user_name = $1 AND group_id = $2",
        )
        .bind(user_name)
        .bind(group_id)
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(_) => Ok(RemoveUserFromGroupResponse::SuccessfulResponse(membership)),
            Err(e) => Err(database_error_with_msg(
                e,
                "Failed to remove user from group",
            )),
        }
    }

    pub async fn list_group_members<C>(
        &self,
        group_id: i64,
        offset: i64,
        limit: i64,
        context: &C,
    ) -> Result<ListGroupMembersResponse, ApiError>
    where
        C: Has<XSpanIdString> + Send + Sync,
    {
        debug!(
            "list_group_members({}, {}, {}) - X-Span-ID: {:?}",
            group_id,
            offset,
            limit,
            context.get().0.clone()
        );

        // Verify group exists
        match self.get_access_group(group_id, context).await? {
            GetAccessGroupResponse::SuccessfulResponse(_) => {}
            GetAccessGroupResponse::NotFoundErrorResponse(e) => {
                return Ok(ListGroupMembersResponse::DefaultErrorResponse(e));
            }
            GetAccessGroupResponse::DefaultErrorResponse(e) => {
                return Ok(ListGroupMembersResponse::DefaultErrorResponse(e));
            }
        }

        let effective_limit = std::cmp::min(limit, MAX_RECORD_TRANSFER_COUNT);

        let query = r#"
            SELECT id, user_name, group_id, role, created_at
            FROM user_group_membership
            WHERE group_id = $1
            ORDER BY user_name
            LIMIT $2 OFFSET $3
        "#;

        let records = match sqlx::query(query)
            .bind(group_id)
            .bind(effective_limit)
            .bind(offset)
            .fetch_all(self.context.pool.as_ref())
            .await
        {
            Ok(rows) => rows,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to list group members"));
            }
        };

        let items: Vec<models::UserGroupMembershipModel> = records
            .into_iter()
            .map(|row| models::UserGroupMembershipModel {
                id: Some(row.get("id")),
                user_name: row.get("user_name"),
                group_id: row.get("group_id"),
                role: row.get("role"),
                created_at: row.get("created_at"),
            })
            .collect();

        // Get total count
        let total_count: i64 = match sqlx::query(
            "SELECT COUNT(*) as total FROM user_group_membership WHERE group_id = $1",
        )
        .bind(group_id)
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(row) => row.get("total"),
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to count group members"));
            }
        };

        let response = models::ListUserGroupMembershipsResponse::new(
            items,
            offset,
            effective_limit,
            total_count,
        );

        Ok(ListGroupMembersResponse::SuccessfulResponse(response))
    }

    pub async fn list_user_groups<C>(
        &self,
        user_name: &str,
        offset: i64,
        limit: i64,
        context: &C,
    ) -> Result<ListUserGroupsApiResponse, ApiError>
    where
        C: Has<XSpanIdString> + Send + Sync,
    {
        debug!(
            "list_user_groups({}, {}, {}) - X-Span-ID: {:?}",
            user_name,
            offset,
            limit,
            context.get().0.clone()
        );

        let effective_limit = std::cmp::min(limit, MAX_RECORD_TRANSFER_COUNT);

        let query = r#"
            SELECT g.id, g.name, g.description, g.created_at
            FROM access_group g
            INNER JOIN user_group_membership m ON g.id = m.group_id
            WHERE m.user_name = $1
            ORDER BY g.name
            LIMIT $2 OFFSET $3
        "#;

        let records = match sqlx::query(query)
            .bind(user_name)
            .bind(effective_limit)
            .bind(offset)
            .fetch_all(self.context.pool.as_ref())
            .await
        {
            Ok(rows) => rows,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to list user groups"));
            }
        };

        let items: Vec<models::AccessGroupModel> = records
            .into_iter()
            .map(|row| models::AccessGroupModel {
                id: Some(row.get("id")),
                name: row.get("name"),
                description: row.get("description"),
                created_at: row.get("created_at"),
            })
            .collect();

        // Get total count
        let total_count: i64 = match sqlx::query(
            r#"
            SELECT COUNT(*) as total
            FROM user_group_membership
            WHERE user_name = $1
        "#,
        )
        .bind(user_name)
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(row) => row.get("total"),
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to count user groups"));
            }
        };

        let response =
            models::ListAccessGroupsResponse::new(items, offset, effective_limit, total_count);

        Ok(ListUserGroupsApiResponse::SuccessfulResponse(response))
    }

    // ========================================================================
    // Workflow-Group association operations
    // ========================================================================

    pub async fn add_workflow_to_group<C>(
        &self,
        workflow_id: i64,
        group_id: i64,
        context: &C,
    ) -> Result<AddWorkflowToGroupResponse, ApiError>
    where
        C: Has<XSpanIdString> + Send + Sync,
    {
        debug!(
            "add_workflow_to_group({}, {}) - X-Span-ID: {:?}",
            workflow_id,
            group_id,
            context.get().0.clone()
        );

        // Verify group exists
        match self.get_access_group(group_id, context).await? {
            GetAccessGroupResponse::SuccessfulResponse(_) => {}
            GetAccessGroupResponse::NotFoundErrorResponse(e) => {
                return Ok(AddWorkflowToGroupResponse::NotFoundErrorResponse(e));
            }
            GetAccessGroupResponse::DefaultErrorResponse(e) => {
                return Ok(AddWorkflowToGroupResponse::DefaultErrorResponse(e));
            }
        }

        // Verify workflow exists
        let workflow_exists: bool =
            match sqlx::query("SELECT EXISTS(SELECT 1 FROM workflow WHERE id = $1) as exists_flag")
                .bind(workflow_id)
                .fetch_one(self.context.pool.as_ref())
                .await
            {
                Ok(row) => row.get::<i32, _>("exists_flag") == 1,
                Err(e) => {
                    return Err(database_error_with_msg(
                        e,
                        "Failed to check if workflow exists",
                    ));
                }
            };

        if !workflow_exists {
            return Ok(AddWorkflowToGroupResponse::NotFoundErrorResponse(
                models::ErrorResponse::new(json!({
                    "error": "NotFound",
                    "message": format!("Workflow not found with ID: {}", workflow_id)
                })),
            ));
        }

        let result = match sqlx::query(
            r#"
            INSERT INTO workflow_access_group (workflow_id, group_id)
            VALUES ($1, $2)
            RETURNING workflow_id, group_id, created_at
            "#,
        )
        .bind(workflow_id)
        .bind(group_id)
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(row) => row,
            Err(e) => {
                if e.to_string().contains("UNIQUE constraint failed")
                    || e.to_string().contains("PRIMARY KEY constraint failed")
                {
                    return Ok(AddWorkflowToGroupResponse::DefaultErrorResponse(
                        models::ErrorResponse::new(json!({
                            "error": "Conflict",
                            "message": format!("Workflow {} is already associated with group {}", workflow_id, group_id)
                        })),
                    ));
                }
                return Err(database_error_with_msg(
                    e,
                    "Failed to associate workflow with group",
                ));
            }
        };

        let association = models::WorkflowAccessGroupModel {
            workflow_id: result.get("workflow_id"),
            group_id: result.get("group_id"),
            created_at: result.get("created_at"),
        };

        Ok(AddWorkflowToGroupResponse::SuccessfulResponse(association))
    }

    pub async fn remove_workflow_from_group<C>(
        &self,
        workflow_id: i64,
        group_id: i64,
        context: &C,
    ) -> Result<RemoveWorkflowFromGroupResponse, ApiError>
    where
        C: Has<XSpanIdString> + Send + Sync,
    {
        debug!(
            "remove_workflow_from_group({}, {}) - X-Span-ID: {:?}",
            workflow_id,
            group_id,
            context.get().0.clone()
        );

        // First get the association to return it after deletion
        let association = match sqlx::query(
            r#"
            SELECT workflow_id, group_id, created_at
            FROM workflow_access_group
            WHERE workflow_id = $1 AND group_id = $2
            "#,
        )
        .bind(workflow_id)
        .bind(group_id)
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(row)) => models::WorkflowAccessGroupModel {
                workflow_id: row.get("workflow_id"),
                group_id: row.get("group_id"),
                created_at: row.get("created_at"),
            },
            Ok(None) => {
                return Ok(RemoveWorkflowFromGroupResponse::NotFoundErrorResponse(
                    models::ErrorResponse::new(json!({
                        "error": "NotFound",
                        "message": format!("Association not found for workflow {} in group {}", workflow_id, group_id)
                    })),
                ));
            }
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to fetch workflow-group association",
                ));
            }
        };

        match sqlx::query(
            "DELETE FROM workflow_access_group WHERE workflow_id = $1 AND group_id = $2",
        )
        .bind(workflow_id)
        .bind(group_id)
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(_) => Ok(RemoveWorkflowFromGroupResponse::SuccessfulResponse(
                association,
            )),
            Err(e) => Err(database_error_with_msg(
                e,
                "Failed to remove workflow from group",
            )),
        }
    }

    pub async fn list_workflow_groups<C>(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        context: &C,
    ) -> Result<ListWorkflowGroupsResponse, ApiError>
    where
        C: Has<XSpanIdString> + Send + Sync,
    {
        debug!(
            "list_workflow_groups({}) - X-Span-ID: {:?}",
            workflow_id,
            context.get().0.clone()
        );

        // Verify workflow exists
        let workflow_exists: bool =
            match sqlx::query("SELECT EXISTS(SELECT 1 FROM workflow WHERE id = $1) as exists_flag")
                .bind(workflow_id)
                .fetch_one(self.context.pool.as_ref())
                .await
            {
                Ok(row) => row.get::<i32, _>("exists_flag") == 1,
                Err(e) => {
                    return Err(database_error_with_msg(
                        e,
                        "Failed to check if workflow exists",
                    ));
                }
            };

        if !workflow_exists {
            return Ok(ListWorkflowGroupsResponse::DefaultErrorResponse(
                models::ErrorResponse::new(json!({
                    "error": "NotFound",
                    "message": format!("Workflow not found with ID: {}", workflow_id)
                })),
            ));
        }

        let effective_limit = std::cmp::min(limit, MAX_RECORD_TRANSFER_COUNT);

        let query = r#"
            SELECT g.id, g.name, g.description, g.created_at
            FROM access_group g
            INNER JOIN workflow_access_group w ON g.id = w.group_id
            WHERE w.workflow_id = $1
            ORDER BY g.name
            LIMIT $2 OFFSET $3
        "#;

        let records = match sqlx::query(query)
            .bind(workflow_id)
            .bind(effective_limit)
            .bind(offset)
            .fetch_all(self.context.pool.as_ref())
            .await
        {
            Ok(rows) => rows,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to list workflow groups"));
            }
        };
        let items: Vec<models::AccessGroupModel> = records
            .into_iter()
            .map(|row| models::AccessGroupModel {
                id: Some(row.get("id")),
                name: row.get("name"),
                description: row.get("description"),
                created_at: row.get("created_at"),
            })
            .collect();

        // Get total count
        let total_count: i64 = match sqlx::query(
            "SELECT COUNT(*) as total FROM workflow_access_group WHERE workflow_id = $1",
        )
        .bind(workflow_id)
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(row) => row.get("total"),
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to count workflow groups",
                ));
            }
        };

        let response =
            models::ListAccessGroupsResponse::new(items, offset, effective_limit, total_count);

        Ok(ListWorkflowGroupsResponse::SuccessfulResponse(response))
    }

    // ========================================================================
    // Authorization check
    // ========================================================================

    pub async fn check_workflow_access<C>(
        &self,
        workflow_id: i64,
        user_name: &str,
        context: &C,
    ) -> Result<CheckWorkflowAccessResponse, ApiError>
    where
        C: Has<XSpanIdString> + Send + Sync,
    {
        debug!(
            "check_workflow_access({}, {}) - X-Span-ID: {:?}",
            workflow_id,
            user_name,
            context.get().0.clone()
        );

        match self
            .check_workflow_access_internal(user_name, workflow_id)
            .await
        {
            Ok(has_access) => Ok(CheckWorkflowAccessResponse::SuccessfulResponse(
                models::AccessCheckResponse {
                    has_access,
                    user_name: user_name.to_string(),
                    workflow_id,
                    reason: None,
                },
            )),
            Err(e) => Err(e),
        }
    }
}
