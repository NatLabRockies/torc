//! Failure handlers-related API endpoints

#![allow(clippy::too_many_arguments)]

use async_trait::async_trait;
use log::{debug, info};
use swagger::{ApiError, Has, XSpanIdString};

use crate::server::api_types::{
    CreateFailureHandlerResponse, DeleteFailureHandlerResponse, GetFailureHandlerResponse,
    ListFailureHandlersResponse,
};

use crate::models;

use super::{ApiContext, MAX_RECORD_TRANSFER_COUNT, database_error_with_msg};

/// Trait defining failure handler-related API operations
#[async_trait]
pub trait FailureHandlersApi<C> {
    /// Store one failure handler record.
    async fn create_failure_handler(
        &self,
        body: models::FailureHandlerModel,
        context: &C,
    ) -> Result<CreateFailureHandlerResponse, ApiError>;

    /// Retrieve a failure handler record by ID.
    async fn get_failure_handler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetFailureHandlerResponse, ApiError>;

    /// Retrieve all failure handlers for one workflow.
    async fn list_failure_handlers(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        context: &C,
    ) -> Result<ListFailureHandlersResponse, ApiError>;

    /// Delete a failure handler record.
    async fn delete_failure_handler(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteFailureHandlerResponse, ApiError>;
}

/// Implementation of failure handlers API for the server
#[derive(Clone)]
pub struct FailureHandlersApiImpl {
    pub context: ApiContext,
}

impl FailureHandlersApiImpl {
    pub fn new(context: ApiContext) -> Self {
        Self { context }
    }
}

#[async_trait]
impl<C> FailureHandlersApi<C> for FailureHandlersApiImpl
where
    C: Has<XSpanIdString> + Send + Sync,
{
    /// Store one failure handler record.
    async fn create_failure_handler(
        &self,
        mut body: models::FailureHandlerModel,
        context: &C,
    ) -> Result<CreateFailureHandlerResponse, ApiError> {
        debug!(
            "create_failure_handler({:?}) - X-Span-ID: {:?}",
            body,
            context.get().0.clone()
        );

        let result = match sqlx::query!(
            r#"
            INSERT INTO failure_handler (workflow_id, name, rules)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
            body.workflow_id,
            body.name,
            body.rules,
        )
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to create failure handler record",
                ));
            }
        };
        body.id = Some(result.id);
        info!(
            "Created failure handler with ID: {} for workflow {}",
            result.id, body.workflow_id
        );
        Ok(CreateFailureHandlerResponse::SuccessfulResponse(body))
    }

    /// Retrieve a failure handler record by ID.
    async fn get_failure_handler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetFailureHandlerResponse, ApiError> {
        debug!(
            "get_failure_handler({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        let record = match sqlx::query!(
            r#"
            SELECT id, workflow_id, name, rules
            FROM failure_handler
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
                    "message": format!("Failure handler not found with ID: {}", id)
                }));
                return Ok(GetFailureHandlerResponse::NotFoundErrorResponse(
                    error_response,
                ));
            }
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to fetch failure handler",
                ));
            }
        };

        let failure_handler_model = models::FailureHandlerModel {
            id: Some(record.id),
            workflow_id: record.workflow_id,
            name: record.name,
            rules: record.rules,
        };

        Ok(GetFailureHandlerResponse::SuccessfulResponse(
            failure_handler_model,
        ))
    }

    /// Retrieve all failure handlers for one workflow.
    async fn list_failure_handlers(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        context: &C,
    ) -> Result<ListFailureHandlersResponse, ApiError> {
        debug!(
            "list_failure_handlers({}, {}, {}) - X-Span-ID: {:?}",
            workflow_id,
            offset,
            limit,
            context.get().0.clone()
        );

        let limit = std::cmp::min(limit, MAX_RECORD_TRANSFER_COUNT);

        let records = match sqlx::query!(
            r#"
            SELECT id, workflow_id, name, rules
            FROM failure_handler
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
                    "Failed to list failure handlers",
                ));
            }
        };

        let items: Vec<models::FailureHandlerModel> = records
            .into_iter()
            .map(|record| models::FailureHandlerModel {
                id: Some(record.id),
                workflow_id: record.workflow_id,
                name: record.name,
                rules: record.rules,
            })
            .collect();

        let count = items.len() as i64;

        // Get total count
        let total_count = match sqlx::query!(
            r#"SELECT COUNT(*) as total FROM failure_handler WHERE workflow_id = $1"#,
            workflow_id
        )
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(row) => row.total,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to count failure handlers",
                ));
            }
        };

        let has_more = offset + count < total_count;

        Ok(ListFailureHandlersResponse::SuccessfulResponse(
            models::ListFailureHandlersResponse {
                items: Some(items),
                offset,
                max_limit: MAX_RECORD_TRANSFER_COUNT,
                count,
                total_count,
                has_more,
            },
        ))
    }

    /// Delete a failure handler record.
    async fn delete_failure_handler(
        &self,
        id: i64,
        _body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteFailureHandlerResponse, ApiError> {
        debug!(
            "delete_failure_handler({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        let result = match sqlx::query!("DELETE FROM failure_handler WHERE id = $1", id)
            .execute(self.context.pool.as_ref())
            .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to delete failure handler",
                ));
            }
        };

        if result.rows_affected() == 0 {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Failure handler not found with ID: {}", id)
            }));
            return Ok(DeleteFailureHandlerResponse::NotFoundErrorResponse(
                error_response,
            ));
        }

        info!("Deleted failure handler with ID: {}", id);
        Ok(DeleteFailureHandlerResponse::SuccessfulResponse(
            serde_json::json!({"message": "Failure handler deleted successfully"}),
        ))
    }
}
