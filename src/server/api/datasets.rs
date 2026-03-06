//! Dataset-related API endpoints
//!
//! Issue: #184 - Datasets: First-Class Directory Outputs

#![allow(clippy::too_many_arguments)]

use async_trait::async_trait;
use log::{debug, info};
use sqlx::Row;
use swagger::{ApiError, Has, XSpanIdString};

use crate::models::{self, DatasetStatus, HashMode};
use crate::server::api_types::{
    CreateDatasetResponse, CreateJobDatasetInputResponse, CreateJobDatasetOutputResponse,
    DeleteDatasetResponse, DeleteDatasetsResponse, FinalizeDatasetResponse, GetDatasetResponse,
    ListDatasetsResponse, UpdateDatasetResponse,
};

use super::{ApiContext, MAX_RECORD_TRANSFER_COUNT, SqlQueryBuilder, database_error_with_msg};

/// Trait defining dataset-related API operations
#[async_trait]
pub trait DatasetsApi<C> {
    /// Create a dataset
    async fn create_dataset(
        &self,
        dataset: models::DatasetModel,
        context: &C,
    ) -> Result<CreateDatasetResponse, ApiError>;

    /// Get a dataset by ID
    async fn get_dataset(&self, id: i64, context: &C) -> Result<GetDatasetResponse, ApiError>;

    /// List datasets for a workflow
    async fn list_datasets(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        status: Option<String>,
        context: &C,
    ) -> Result<ListDatasetsResponse, ApiError>;

    /// Update a dataset
    async fn update_dataset(
        &self,
        id: i64,
        body: models::DatasetModel,
        context: &C,
    ) -> Result<UpdateDatasetResponse, ApiError>;

    /// Delete a dataset
    async fn delete_dataset(&self, id: i64, context: &C)
    -> Result<DeleteDatasetResponse, ApiError>;

    /// Delete all datasets for a workflow
    async fn delete_datasets(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteDatasetsResponse, ApiError>;

    /// Finalize a dataset (set computed hash, file count, etc.)
    async fn finalize_dataset(
        &self,
        id: i64,
        body: models::DatasetFinalizationRequest,
        context: &C,
    ) -> Result<FinalizeDatasetResponse, ApiError>;

    /// Create a job-dataset output relationship
    async fn create_job_dataset_output(
        &self,
        job_id: i64,
        dataset_id: i64,
        workflow_id: i64,
        context: &C,
    ) -> Result<CreateJobDatasetOutputResponse, ApiError>;

    /// Create a job-dataset input relationship
    async fn create_job_dataset_input(
        &self,
        job_id: i64,
        dataset_id: i64,
        workflow_id: i64,
        context: &C,
    ) -> Result<CreateJobDatasetInputResponse, ApiError>;
}

/// Implementation of datasets API for the server
#[derive(Clone)]
pub struct DatasetsApiImpl {
    pub context: ApiContext,
}

const DATASET_COLUMNS: &[&str] = &[
    "id",
    "workflow_id",
    "name",
    "path",
    "description",
    "hash_mode",
    "status",
    "claimed_by_node_id",
    "claimed_at",
    "file_count",
    "total_size_bytes",
    "manifest_hash",
    "finalized_at",
];

impl DatasetsApiImpl {
    pub fn new(context: ApiContext) -> Self {
        Self { context }
    }

    fn row_to_model(row: &sqlx::sqlite::SqliteRow) -> models::DatasetModel {
        let hash_mode_str: String = row.get("hash_mode");
        let status_str: String = row.get("status");

        models::DatasetModel {
            id: Some(row.get("id")),
            workflow_id: row.get("workflow_id"),
            name: row.get("name"),
            path: row.get("path"),
            description: row.get("description"),
            hash_mode: hash_mode_str.parse().unwrap_or(HashMode::Manifest),
            status: status_str.parse().unwrap_or(DatasetStatus::Pending),
            claimed_by_node_id: row.get("claimed_by_node_id"),
            claimed_at: row.get("claimed_at"),
            file_count: row.get("file_count"),
            total_size_bytes: row.get("total_size_bytes"),
            manifest_hash: row.get("manifest_hash"),
            finalized_at: row.get("finalized_at"),
        }
    }
}

#[async_trait]
impl<C> DatasetsApi<C> for DatasetsApiImpl
where
    C: Has<XSpanIdString> + Send + Sync,
{
    async fn create_dataset(
        &self,
        mut dataset: models::DatasetModel,
        context: &C,
    ) -> Result<CreateDatasetResponse, ApiError> {
        debug!(
            "create_dataset({:?}) - X-Span-ID: {:?}",
            dataset,
            context.get().0.clone()
        );

        let hash_mode_str = dataset.hash_mode.to_string();
        let status_str = dataset.status.to_string();

        let result = match sqlx::query!(
            r#"
            INSERT INTO dataset
            (
                workflow_id,
                name,
                path,
                description,
                hash_mode,
                status
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
            dataset.workflow_id,
            dataset.name,
            dataset.path,
            dataset.description,
            hash_mode_str,
            status_str,
        )
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to create dataset"));
            }
        };

        dataset.id = Some(result.id);

        // Update workflow's has_datasets flag
        if let Err(e) = sqlx::query!(
            "UPDATE workflow SET has_datasets = 1 WHERE id = $1",
            dataset.workflow_id
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            debug!(
                "Failed to update has_datasets flag for workflow {}: {}",
                dataset.workflow_id, e
            );
        }

        info!(
            "Created dataset '{}' (id: {}) for workflow {}",
            dataset.name, result.id, dataset.workflow_id
        );

        Ok(CreateDatasetResponse::SuccessfulResponse(dataset))
    }

    async fn get_dataset(&self, id: i64, context: &C) -> Result<GetDatasetResponse, ApiError> {
        debug!(
            "get_dataset({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        let record = match sqlx::query(
            "SELECT id, workflow_id, name, path, description, hash_mode, status, \
             claimed_by_node_id, claimed_at, file_count, total_size_bytes, \
             manifest_hash, finalized_at FROM dataset WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(rec)) => rec,
            Ok(None) => {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Dataset not found with ID: {}", id)
                }));
                return Ok(GetDatasetResponse::NotFoundErrorResponse(error_response));
            }
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to fetch dataset"));
            }
        };

        let dataset = Self::row_to_model(&record);
        Ok(GetDatasetResponse::SuccessfulResponse(dataset))
    }

    async fn list_datasets(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        status: Option<String>,
        context: &C,
    ) -> Result<ListDatasetsResponse, ApiError> {
        debug!(
            "list_datasets({}, {}, {}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            offset,
            limit,
            status,
            context.get().0.clone()
        );

        let base_query = "SELECT id, workflow_id, name, path, description, hash_mode, status, \
             claimed_by_node_id, claimed_at, file_count, total_size_bytes, \
             manifest_hash, finalized_at FROM dataset"
            .to_string();

        let mut where_conditions = vec!["workflow_id = ?".to_string()];
        if status.is_some() {
            where_conditions.push("status = ?".to_string());
        }

        let where_clause = where_conditions.join(" AND ");

        let query = SqlQueryBuilder::new(base_query)
            .with_where(where_clause.clone())
            .with_pagination_and_sorting(offset, limit, None, None, "id", DATASET_COLUMNS)
            .build();

        let mut sqlx_query = sqlx::query(&query);
        sqlx_query = sqlx_query.bind(workflow_id);
        if let Some(ref status_filter) = status {
            sqlx_query = sqlx_query.bind(status_filter);
        }

        let records = match sqlx_query.fetch_all(self.context.pool.as_ref()).await {
            Ok(recs) => recs,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to list datasets"));
            }
        };

        let items: Vec<models::DatasetModel> = records.iter().map(Self::row_to_model).collect();

        // Get total count
        let count_base = "SELECT COUNT(*) as total FROM dataset".to_string();
        let count_query = SqlQueryBuilder::new(count_base)
            .with_where(where_clause)
            .build();

        let mut count_sqlx_query = sqlx::query(&count_query);
        count_sqlx_query = count_sqlx_query.bind(workflow_id);
        if let Some(ref status_filter) = status {
            count_sqlx_query = count_sqlx_query.bind(status_filter);
        }

        let total_count = match count_sqlx_query.fetch_one(self.context.pool.as_ref()).await {
            Ok(row) => row.get::<i64, _>("total"),
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to count datasets"));
            }
        };

        let current_count = items.len() as i64;
        let has_more = offset + current_count < total_count;

        Ok(ListDatasetsResponse::SuccessfulResponse(
            models::ListDatasetsResponse {
                items: Some(items),
                offset,
                max_limit: MAX_RECORD_TRANSFER_COUNT,
                count: current_count,
                total_count,
                has_more,
            },
        ))
    }

    async fn update_dataset(
        &self,
        id: i64,
        body: models::DatasetModel,
        context: &C,
    ) -> Result<UpdateDatasetResponse, ApiError> {
        debug!(
            "update_dataset({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First check if dataset exists
        match self.get_dataset(id, context).await? {
            GetDatasetResponse::SuccessfulResponse(_) => {}
            GetDatasetResponse::ForbiddenErrorResponse(err) => {
                return Ok(UpdateDatasetResponse::ForbiddenErrorResponse(err));
            }
            GetDatasetResponse::NotFoundErrorResponse(err) => {
                return Ok(UpdateDatasetResponse::NotFoundErrorResponse(err));
            }
            GetDatasetResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get dataset".to_string()));
            }
        };

        let hash_mode_str = body.hash_mode.to_string();
        let status_str = body.status.to_string();

        let result = match sqlx::query!(
            r#"
            UPDATE dataset
            SET
                name = COALESCE($1, name),
                path = COALESCE($2, path),
                description = $3,
                hash_mode = COALESCE($4, hash_mode),
                status = COALESCE($5, status),
                claimed_by_node_id = $6,
                claimed_at = $7,
                file_count = $8,
                total_size_bytes = $9,
                manifest_hash = $10,
                finalized_at = $11
            WHERE id = $12
            "#,
            body.name,
            body.path,
            body.description,
            hash_mode_str,
            status_str,
            body.claimed_by_node_id,
            body.claimed_at,
            body.file_count,
            body.total_size_bytes,
            body.manifest_hash,
            body.finalized_at,
            id,
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to update dataset"));
            }
        };

        if result.rows_affected() == 0 {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Dataset not found with ID: {}", id)
            }));
            return Ok(UpdateDatasetResponse::NotFoundErrorResponse(error_response));
        }

        // Return updated dataset
        match self.get_dataset(id, context).await? {
            GetDatasetResponse::SuccessfulResponse(dataset) => {
                Ok(UpdateDatasetResponse::SuccessfulResponse(dataset))
            }
            _ => Err(ApiError("Failed to get updated dataset".to_string())),
        }
    }

    async fn delete_dataset(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteDatasetResponse, ApiError> {
        debug!(
            "delete_dataset({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        // Get dataset first
        let dataset = match self.get_dataset(id, context).await? {
            GetDatasetResponse::SuccessfulResponse(d) => d,
            GetDatasetResponse::ForbiddenErrorResponse(err) => {
                return Ok(DeleteDatasetResponse::ForbiddenErrorResponse(err));
            }
            GetDatasetResponse::NotFoundErrorResponse(err) => {
                return Ok(DeleteDatasetResponse::NotFoundErrorResponse(err));
            }
            GetDatasetResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get dataset".to_string()));
            }
        };

        match sqlx::query!("DELETE FROM dataset WHERE id = $1", id)
            .execute(self.context.pool.as_ref())
            .await
        {
            Ok(res) => {
                if res.rows_affected() == 0 {
                    return Err(database_error_with_msg(
                        "No rows affected",
                        "Failed to delete dataset",
                    ));
                }
                info!(
                    "Deleted dataset {} (name: {}) from workflow {}",
                    id, dataset.name, dataset.workflow_id
                );
                Ok(DeleteDatasetResponse::SuccessfulResponse(dataset))
            }
            Err(e) => Err(database_error_with_msg(e, "Failed to delete dataset")),
        }
    }

    async fn delete_datasets(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteDatasetsResponse, ApiError> {
        debug!(
            "delete_datasets({}) - X-Span-ID: {:?}",
            workflow_id,
            context.get().0.clone()
        );

        let result = match sqlx::query!("DELETE FROM dataset WHERE workflow_id = $1", workflow_id)
            .execute(self.context.pool.as_ref())
            .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to delete datasets"));
            }
        };

        let deleted_count = result.rows_affected();
        info!(
            "Deleted {} datasets for workflow {}",
            deleted_count, workflow_id
        );

        // Update has_datasets flag if all datasets deleted
        if deleted_count > 0
            && sqlx::query!(
                "UPDATE workflow SET has_datasets = 0 WHERE id = $1 AND NOT EXISTS (SELECT 1 FROM dataset WHERE workflow_id = $1)",
                workflow_id
            )
            .execute(self.context.pool.as_ref())
            .await
            .is_err()
        {
            debug!(
                "Failed to update has_datasets flag for workflow {}",
                workflow_id
            );
        }

        Ok(DeleteDatasetsResponse::SuccessfulResponse(
            serde_json::json!({
                "deleted_count": deleted_count
            }),
        ))
    }

    async fn finalize_dataset(
        &self,
        id: i64,
        body: models::DatasetFinalizationRequest,
        context: &C,
    ) -> Result<FinalizeDatasetResponse, ApiError> {
        debug!(
            "finalize_dataset({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        let now = chrono::Utc::now().timestamp() as f64;
        let status_str = DatasetStatus::Finalized.to_string();

        let result = match sqlx::query!(
            r#"
            UPDATE dataset
            SET
                status = $1,
                file_count = $2,
                total_size_bytes = $3,
                manifest_hash = $4,
                finalized_at = $5,
                claimed_by_node_id = NULL,
                claimed_at = NULL
            WHERE id = $6
            "#,
            status_str,
            body.file_count,
            body.total_size_bytes,
            body.manifest_hash,
            now,
            id,
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to finalize dataset"));
            }
        };

        if result.rows_affected() == 0 {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Dataset not found with ID: {}", id)
            }));
            return Ok(FinalizeDatasetResponse::NotFoundErrorResponse(
                error_response,
            ));
        }

        info!(
            "Finalized dataset {} with {} files, {} bytes",
            id, body.file_count, body.total_size_bytes
        );

        // Return finalized dataset
        match self.get_dataset(id, context).await? {
            GetDatasetResponse::SuccessfulResponse(dataset) => {
                Ok(FinalizeDatasetResponse::SuccessfulResponse(dataset))
            }
            _ => Err(ApiError("Failed to get finalized dataset".to_string())),
        }
    }

    async fn create_job_dataset_output(
        &self,
        job_id: i64,
        dataset_id: i64,
        workflow_id: i64,
        context: &C,
    ) -> Result<CreateJobDatasetOutputResponse, ApiError> {
        debug!(
            "create_job_dataset_output({}, {}, {}) - X-Span-ID: {:?}",
            job_id,
            dataset_id,
            workflow_id,
            context.get().0.clone()
        );

        match sqlx::query!(
            r#"
            INSERT INTO job_dataset_output (job_id, dataset_id, workflow_id)
            VALUES ($1, $2, $3)
            "#,
            job_id,
            dataset_id,
            workflow_id,
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(_) => {
                info!(
                    "Created job-dataset output: job {} -> dataset {}",
                    job_id, dataset_id
                );
                Ok(CreateJobDatasetOutputResponse::SuccessfulResponse(
                    models::JobDatasetOutputModel {
                        job_id,
                        dataset_id,
                        workflow_id,
                    },
                ))
            }
            Err(e) => Err(database_error_with_msg(
                e,
                "Failed to create job-dataset output relationship",
            )),
        }
    }

    async fn create_job_dataset_input(
        &self,
        job_id: i64,
        dataset_id: i64,
        workflow_id: i64,
        context: &C,
    ) -> Result<CreateJobDatasetInputResponse, ApiError> {
        debug!(
            "create_job_dataset_input({}, {}, {}) - X-Span-ID: {:?}",
            job_id,
            dataset_id,
            workflow_id,
            context.get().0.clone()
        );

        match sqlx::query!(
            r#"
            INSERT INTO job_dataset_input (job_id, dataset_id, workflow_id)
            VALUES ($1, $2, $3)
            "#,
            job_id,
            dataset_id,
            workflow_id,
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(_) => {
                info!(
                    "Created job-dataset input: job {} <- dataset {}",
                    job_id, dataset_id
                );
                Ok(CreateJobDatasetInputResponse::SuccessfulResponse(
                    models::JobDatasetInputModel {
                        job_id,
                        dataset_id,
                        workflow_id,
                    },
                ))
            }
            Err(e) => Err(database_error_with_msg(
                e,
                "Failed to create job-dataset input relationship",
            )),
        }
    }
}
