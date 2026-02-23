//! RO-Crate entity-related API endpoints

#![allow(clippy::too_many_arguments)]

use async_trait::async_trait;
use log::{debug, info};
use swagger::{ApiError, Has, XSpanIdString};

use crate::server::api_types::{
    CreateRoCrateEntityResponse, DeleteRoCrateEntitiesResponse, DeleteRoCrateEntityResponse,
    GetRoCrateEntityResponse, ListRoCrateEntitiesResponse, UpdateRoCrateEntityResponse,
};

use crate::models;

use super::{ApiContext, MAX_RECORD_TRANSFER_COUNT, database_error_with_msg};

/// Trait defining RO-Crate entity-related API operations
#[async_trait]
pub trait RoCrateApi<C> {
    /// Store one RO-Crate entity record.
    async fn create_ro_crate_entity(
        &self,
        body: models::RoCrateEntityModel,
        context: &C,
    ) -> Result<CreateRoCrateEntityResponse, ApiError>;

    /// Retrieve an RO-Crate entity record by ID.
    async fn get_ro_crate_entity(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetRoCrateEntityResponse, ApiError>;

    /// Retrieve all RO-Crate entities for one workflow.
    async fn list_ro_crate_entities(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        context: &C,
    ) -> Result<ListRoCrateEntitiesResponse, ApiError>;

    /// Update an RO-Crate entity record.
    async fn update_ro_crate_entity(
        &self,
        id: i64,
        body: models::RoCrateEntityModel,
        context: &C,
    ) -> Result<UpdateRoCrateEntityResponse, ApiError>;

    /// Delete an RO-Crate entity record.
    async fn delete_ro_crate_entity(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteRoCrateEntityResponse, ApiError>;

    /// Delete all RO-Crate entities for a workflow.
    async fn delete_ro_crate_entities(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteRoCrateEntitiesResponse, ApiError>;
}

/// Implementation of RO-Crate entity API for the server
#[derive(Clone)]
pub struct RoCrateApiImpl {
    pub context: ApiContext,
}

impl RoCrateApiImpl {
    pub fn new(context: ApiContext) -> Self {
        Self { context }
    }
}

#[async_trait]
impl<C> RoCrateApi<C> for RoCrateApiImpl
where
    C: Has<XSpanIdString> + Send + Sync,
{
    /// Store one RO-Crate entity record.
    async fn create_ro_crate_entity(
        &self,
        mut body: models::RoCrateEntityModel,
        context: &C,
    ) -> Result<CreateRoCrateEntityResponse, ApiError> {
        debug!(
            "create_ro_crate_entity({:?}) - X-Span-ID: {:?}",
            body,
            context.get().0.clone()
        );

        let result = match sqlx::query!(
            r#"
            INSERT INTO ro_crate_entity (workflow_id, file_id, entity_id, entity_type, metadata)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
            "#,
            body.workflow_id,
            body.file_id,
            body.entity_id,
            body.entity_type,
            body.metadata,
        )
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to create RO-Crate entity record",
                ));
            }
        };
        body.id = Some(result.id);
        info!(
            "Created RO-Crate entity with ID: {} for workflow_id={}",
            result.id, body.workflow_id
        );
        Ok(CreateRoCrateEntityResponse::SuccessfulResponse(body))
    }

    /// Retrieve an RO-Crate entity record by ID.
    async fn get_ro_crate_entity(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetRoCrateEntityResponse, ApiError> {
        debug!(
            "get_ro_crate_entity({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        let record = match sqlx::query!(
            r#"
            SELECT id, workflow_id, file_id, entity_id, entity_type, metadata
            FROM ro_crate_entity
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(record)) => record,
            Ok(None) => {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("RO-Crate entity not found with ID: {}", id)
                }));
                return Ok(GetRoCrateEntityResponse::NotFoundErrorResponse(
                    error_response,
                ));
            }
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to fetch RO-Crate entity",
                ));
            }
        };

        let model = models::RoCrateEntityModel {
            id: Some(record.id),
            workflow_id: record.workflow_id,
            file_id: record.file_id,
            entity_id: record.entity_id,
            entity_type: record.entity_type,
            metadata: record.metadata,
        };

        Ok(GetRoCrateEntityResponse::SuccessfulResponse(model))
    }

    /// Retrieve all RO-Crate entities for one workflow.
    async fn list_ro_crate_entities(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        context: &C,
    ) -> Result<ListRoCrateEntitiesResponse, ApiError> {
        debug!(
            "list_ro_crate_entities({}, {}, {}) - X-Span-ID: {:?}",
            workflow_id,
            offset,
            limit,
            context.get().0.clone()
        );

        let limit = std::cmp::min(limit, MAX_RECORD_TRANSFER_COUNT);

        let records = match sqlx::query!(
            r#"
            SELECT id, workflow_id, file_id, entity_id, entity_type, metadata
            FROM ro_crate_entity
            WHERE workflow_id = $1
            ORDER BY id
            LIMIT $2 OFFSET $3
            "#,
            workflow_id,
            limit,
            offset
        )
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(records) => records,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to list RO-Crate entities",
                ));
            }
        };

        let items: Vec<models::RoCrateEntityModel> = records
            .into_iter()
            .map(|record| models::RoCrateEntityModel {
                id: Some(record.id),
                workflow_id: record.workflow_id,
                file_id: record.file_id,
                entity_id: record.entity_id,
                entity_type: record.entity_type,
                metadata: record.metadata,
            })
            .collect();

        let count = items.len() as i64;

        // Get total count
        let total_count = match sqlx::query!(
            r#"SELECT COUNT(*) as total FROM ro_crate_entity WHERE workflow_id = $1"#,
            workflow_id
        )
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(row) => row.total,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to count RO-Crate entities",
                ));
            }
        };

        let has_more = offset + count < total_count;

        Ok(ListRoCrateEntitiesResponse::SuccessfulResponse(
            models::ListRoCrateEntitiesResponse {
                items: Some(items),
                offset,
                max_limit: MAX_RECORD_TRANSFER_COUNT,
                count,
                total_count,
                has_more,
            },
        ))
    }

    /// Update an RO-Crate entity record.
    async fn update_ro_crate_entity(
        &self,
        id: i64,
        mut body: models::RoCrateEntityModel,
        context: &C,
    ) -> Result<UpdateRoCrateEntityResponse, ApiError> {
        debug!(
            "update_ro_crate_entity({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        let result = match sqlx::query!(
            r#"
            UPDATE ro_crate_entity
            SET file_id = $1, entity_id = $2, entity_type = $3, metadata = $4
            WHERE id = $5
            "#,
            body.file_id,
            body.entity_id,
            body.entity_type,
            body.metadata,
            id,
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to update RO-Crate entity",
                ));
            }
        };

        if result.rows_affected() == 0 {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("RO-Crate entity not found with ID: {}", id)
            }));
            return Ok(UpdateRoCrateEntityResponse::NotFoundErrorResponse(
                error_response,
            ));
        }

        body.id = Some(id);
        info!("Updated RO-Crate entity with ID: {}", id);
        Ok(UpdateRoCrateEntityResponse::SuccessfulResponse(body))
    }

    /// Delete an RO-Crate entity record.
    async fn delete_ro_crate_entity(
        &self,
        id: i64,
        _body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteRoCrateEntityResponse, ApiError> {
        debug!(
            "delete_ro_crate_entity({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        let result = match sqlx::query!("DELETE FROM ro_crate_entity WHERE id = $1", id)
            .execute(self.context.pool.as_ref())
            .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to delete RO-Crate entity",
                ));
            }
        };

        if result.rows_affected() == 0 {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("RO-Crate entity not found with ID: {}", id)
            }));
            return Ok(DeleteRoCrateEntityResponse::NotFoundErrorResponse(
                error_response,
            ));
        }

        info!("Deleted RO-Crate entity with ID: {}", id);
        Ok(DeleteRoCrateEntityResponse::SuccessfulResponse(
            serde_json::json!({"message": "RO-Crate entity deleted successfully"}),
        ))
    }

    /// Delete all RO-Crate entities for a workflow.
    async fn delete_ro_crate_entities(
        &self,
        workflow_id: i64,
        _body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteRoCrateEntitiesResponse, ApiError> {
        debug!(
            "delete_ro_crate_entities(workflow_id={}) - X-Span-ID: {:?}",
            workflow_id,
            context.get().0.clone()
        );

        let result = match sqlx::query!(
            "DELETE FROM ro_crate_entity WHERE workflow_id = $1",
            workflow_id
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to delete RO-Crate entities",
                ));
            }
        };

        let deleted_count = result.rows_affected();
        info!(
            "Deleted {} RO-Crate entities for workflow_id={}",
            deleted_count, workflow_id
        );
        Ok(DeleteRoCrateEntitiesResponse::SuccessfulResponse(
            serde_json::json!({
                "message": format!("Deleted {} RO-Crate entities", deleted_count),
                "deleted_count": deleted_count
            }),
        ))
    }
}
