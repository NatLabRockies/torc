//! Remote workers API endpoints

use async_trait::async_trait;
use log::{debug, info};
use sqlx::Row;
use swagger::{ApiError, Has, XSpanIdString};

use crate::models;
use crate::server::api_types::{
    CreateRemoteWorkersResponse, DeleteRemoteWorkerResponse, ListRemoteWorkersResponse,
};

use super::{ApiContext, database_error_with_msg};

/// Trait defining remote worker API operations
#[async_trait]
pub trait RemoteWorkersApi<C> {
    /// Store remote workers for a workflow.
    async fn create_remote_workers(
        &self,
        workflow_id: i64,
        workers: Vec<String>,
        context: &C,
    ) -> Result<CreateRemoteWorkersResponse, ApiError>;

    /// List remote workers for a workflow.
    async fn list_remote_workers(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<ListRemoteWorkersResponse, ApiError>;

    /// Delete a remote worker from a workflow.
    async fn delete_remote_worker(
        &self,
        workflow_id: i64,
        worker: String,
        context: &C,
    ) -> Result<DeleteRemoteWorkerResponse, ApiError>;
}

/// Implementation of remote workers API for the server
#[derive(Clone)]
pub struct RemoteWorkersApiImpl {
    pub context: ApiContext,
}

impl RemoteWorkersApiImpl {
    pub fn new(context: ApiContext) -> Self {
        Self { context }
    }
}

#[async_trait]
impl<C> RemoteWorkersApi<C> for RemoteWorkersApiImpl
where
    C: Has<XSpanIdString> + Send + Sync,
{
    /// Store remote workers for a workflow.
    async fn create_remote_workers(
        &self,
        workflow_id: i64,
        workers: Vec<String>,
        context: &C,
    ) -> Result<CreateRemoteWorkersResponse, ApiError> {
        debug!(
            "create_remote_workers({}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            workers,
            context.get().0.clone()
        );

        // Verify workflow exists
        match sqlx::query!("SELECT id FROM workflow WHERE id = $1", workflow_id)
            .fetch_optional(self.context.pool.as_ref())
            .await
        {
            Ok(Some(_)) => {}
            Ok(None) => {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Workflow not found with ID: {}", workflow_id)
                }));
                return Ok(CreateRemoteWorkersResponse::NotFoundErrorResponse(
                    error_response,
                ));
            }
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to check workflow existence",
                ));
            }
        };

        // Insert workers (ignore duplicates using INSERT OR IGNORE)
        let mut created_workers = Vec::new();
        for worker in &workers {
            match sqlx::query!(
                r#"
                INSERT OR IGNORE INTO remote_worker (worker, workflow_id)
                VALUES ($1, $2)
                "#,
                worker,
                workflow_id,
            )
            .execute(self.context.pool.as_ref())
            .await
            {
                Ok(_) => {
                    created_workers
                        .push(models::RemoteWorkerModel::new(worker.clone(), workflow_id));
                }
                Err(e) => {
                    return Err(database_error_with_msg(e, "Failed to create remote worker"));
                }
            }
        }

        debug!(
            "create_remote_workers({}) created {} workers - X-Span-ID: {:?}",
            workflow_id,
            created_workers.len(),
            context.get().0.clone()
        );

        Ok(CreateRemoteWorkersResponse::SuccessfulResponse(
            created_workers,
        ))
    }

    /// List remote workers for a workflow.
    async fn list_remote_workers(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<ListRemoteWorkersResponse, ApiError> {
        debug!(
            "list_remote_workers({}) - X-Span-ID: {:?}",
            workflow_id,
            context.get().0.clone()
        );

        // Verify workflow exists
        match sqlx::query!("SELECT id FROM workflow WHERE id = $1", workflow_id)
            .fetch_optional(self.context.pool.as_ref())
            .await
        {
            Ok(Some(_)) => {}
            Ok(None) => {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Workflow not found with ID: {}", workflow_id)
                }));
                return Ok(ListRemoteWorkersResponse::NotFoundErrorResponse(
                    error_response,
                ));
            }
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to check workflow existence",
                ));
            }
        };

        // Fetch all workers for the workflow
        let records = match sqlx::query(
            "SELECT worker, workflow_id FROM remote_worker WHERE workflow_id = ? ORDER BY worker",
        )
        .bind(workflow_id)
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(records) => records,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to fetch remote workers"));
            }
        };

        let workers: Vec<models::RemoteWorkerModel> = records
            .iter()
            .map(|row| models::RemoteWorkerModel {
                worker: row.get("worker"),
                workflow_id: row.get("workflow_id"),
            })
            .collect();

        debug!(
            "list_remote_workers({}) found {} workers - X-Span-ID: {:?}",
            workflow_id,
            workers.len(),
            context.get().0.clone()
        );

        Ok(ListRemoteWorkersResponse::SuccessfulResponse(workers))
    }

    /// Delete a remote worker from a workflow.
    async fn delete_remote_worker(
        &self,
        workflow_id: i64,
        worker: String,
        context: &C,
    ) -> Result<DeleteRemoteWorkerResponse, ApiError> {
        debug!(
            "delete_remote_worker({}, {}) - X-Span-ID: {:?}",
            workflow_id,
            worker,
            context.get().0.clone()
        );

        // Verify workflow exists
        match sqlx::query!("SELECT id FROM workflow WHERE id = $1", workflow_id)
            .fetch_optional(self.context.pool.as_ref())
            .await
        {
            Ok(Some(_)) => {}
            Ok(None) => {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Workflow not found with ID: {}", workflow_id)
                }));
                return Ok(DeleteRemoteWorkerResponse::NotFoundErrorResponse(
                    error_response,
                ));
            }
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to check workflow existence",
                ));
            }
        };

        // Delete the worker
        match sqlx::query!(
            "DELETE FROM remote_worker WHERE workflow_id = $1 AND worker = $2",
            workflow_id,
            worker,
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    let error_response = models::ErrorResponse::new(serde_json::json!({
                        "message": format!("Worker '{}' not found for workflow {}", worker, workflow_id)
                    }));
                    return Ok(DeleteRemoteWorkerResponse::NotFoundErrorResponse(
                        error_response,
                    ));
                }
                info!(
                    "Deleted remote worker {} from workflow {}",
                    worker, workflow_id
                );
                Ok(DeleteRemoteWorkerResponse::SuccessfulResponse(
                    models::RemoteWorkerModel::new(worker, workflow_id),
                ))
            }
            Err(e) => Err(database_error_with_msg(e, "Failed to delete remote worker")),
        }
    }
}
