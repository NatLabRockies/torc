//! Resource requirements-related API endpoints

#![allow(clippy::too_many_arguments)]

use async_trait::async_trait;
use log::{debug, error, info};
use sqlx::Row;
use swagger::{ApiError, Has, XSpanIdString};

use crate::server::api_types::{
    CreateResourceRequirementsResponse, DeleteAllResourceRequirementsResponse,
    DeleteResourceRequirementsResponse, GetResourceRequirementsResponse,
    ListResourceRequirementsResponse, UpdateResourceRequirementsResponse,
};

use crate::memory_utils::memory_string_to_bytes;
use crate::models;
use crate::time_utils::duration_string_to_seconds;

use super::{ApiContext, MAX_RECORD_TRANSFER_COUNT, SqlQueryBuilder, database_error_with_msg};

/// Trait defining resource requirements-related API operations
#[async_trait]
pub trait ResourceRequirementsApi<C> {
    /// Store one resource requirements record.
    async fn create_resource_requirements(
        &self,
        mut body: models::ResourceRequirementsModel,
        context: &C,
    ) -> Result<CreateResourceRequirementsResponse, ApiError>;

    /// Delete all resource requirements for one workflow.
    async fn delete_all_resource_requirements(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteAllResourceRequirementsResponse, ApiError>;

    /// Retrieve a resource requirements record by ID.
    async fn get_resource_requirements(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetResourceRequirementsResponse, ApiError>;

    /// Retrieve all resource requirements records for one workflow.
    async fn list_resource_requirements(
        &self,
        workflow_id: i64,
        job_id: Option<i64>,
        name: Option<String>,
        memory: Option<String>,
        num_cpus: Option<i64>,
        num_gpus: Option<i64>,
        num_nodes: Option<i64>,
        runtime: Option<i64>,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListResourceRequirementsResponse, ApiError>;

    /// Update a resource requirements record.
    async fn update_resource_requirements(
        &self,
        id: i64,
        body: models::ResourceRequirementsModel,
        context: &C,
    ) -> Result<UpdateResourceRequirementsResponse, ApiError>;

    /// Delete a resource requirements record.
    async fn delete_resource_requirements(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteResourceRequirementsResponse, ApiError>;
}

/// Implementation of resource requirements API for the server
#[derive(Clone)]
pub struct ResourceRequirementsApiImpl {
    pub context: ApiContext,
}

const RESOURCE_REQUIREMENTS_COLUMNS: &[&str] = &[
    "id",
    "workflow_id",
    "name",
    "num_cpus",
    "num_gpus",
    "num_nodes",
    "memory",
    "runtime",
];

impl ResourceRequirementsApiImpl {
    pub fn new(context: ApiContext) -> Self {
        Self { context }
    }
}

#[async_trait]
impl<C> ResourceRequirementsApi<C> for ResourceRequirementsApiImpl
where
    C: Has<XSpanIdString> + Send + Sync,
{
    /// Store one resource requirements record.
    async fn create_resource_requirements(
        &self,
        mut body: models::ResourceRequirementsModel,
        context: &C,
    ) -> Result<CreateResourceRequirementsResponse, ApiError> {
        debug!(
            "create_resource_requirements({:?}) - X-Span-ID: {:?}",
            body,
            context.get().0.clone()
        );

        let memory_bytes = match memory_string_to_bytes(&body.memory) {
            Ok(bytes) => bytes,
            Err(e) => {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Invalid memory format '{}': {}", body.memory, e),
                    "field": "memory",
                    "value": body.memory
                }));
                return Ok(
                    CreateResourceRequirementsResponse::UnprocessableContentErrorResponse(
                        error_response,
                    ),
                );
            }
        };

        let runtime_seconds = match duration_string_to_seconds(&body.runtime) {
            Ok(seconds) => seconds,
            Err(e) => {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Invalid runtime format '{}': {}", body.runtime, e),
                    "field": "runtime",
                    "value": body.runtime
                }));
                return Ok(
                    CreateResourceRequirementsResponse::UnprocessableContentErrorResponse(
                        error_response,
                    ),
                );
            }
        };

        let result = match sqlx::query!(
            r#"
            INSERT INTO resource_requirements
            (
                workflow_id
                ,name
                ,num_cpus
                ,num_gpus
                ,num_nodes
                ,memory
                ,runtime
                ,memory_bytes
                ,runtime_s
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING rowid
        "#,
            body.workflow_id,
            body.name,
            body.num_cpus,
            body.num_gpus,
            body.num_nodes,
            body.memory,
            body.runtime,
            memory_bytes,
            runtime_seconds,
        )
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to create resource requirements record",
                ));
            }
        };
        body.id = Some(result.id);
        Ok(CreateResourceRequirementsResponse::SuccessfulResponse(body))
    }

    /// Delete all resource requirements for one workflow.
    async fn delete_all_resource_requirements(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteAllResourceRequirementsResponse, ApiError> {
        debug!(
            "delete_all_resource_requirements({}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            body,
            context.get().0.clone()
        );

        let result = match sqlx::query!(
            "DELETE FROM resource_requirements WHERE workflow_id = $1",
            workflow_id
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to delete all resource requirements",
                ));
            }
        };

        let deleted_count = result.rows_affected() as i64;

        info!(
            "Deleted {} resource requirements for workflow {}",
            deleted_count, workflow_id
        );

        Ok(DeleteAllResourceRequirementsResponse::SuccessfulResponse(
            serde_json::json!({
                "count": deleted_count
            }),
        ))
    }

    /// Retrieve a resource requirements record by ID.
    async fn get_resource_requirements(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetResourceRequirementsResponse, ApiError> {
        debug!(
            "get_resource_requirements({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        let record = match sqlx::query!(
            r#"
                SELECT id, workflow_id, name, num_cpus, num_gpus, num_nodes, memory, runtime
                FROM resource_requirements
                WHERE id = $1
            "#,
            id
        )
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(rec)) => rec,
            Ok(None) => {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Resource requirements not found with ID: {}", id)
                }));
                return Ok(GetResourceRequirementsResponse::NotFoundErrorResponse(
                    error_response,
                ));
            }
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to fetch resource requirements",
                ));
            }
        };

        let resource_requirements = models::ResourceRequirementsModel {
            id: Some(record.id),
            workflow_id: record.workflow_id,
            name: record.name,
            num_cpus: record.num_cpus,
            num_gpus: record.num_gpus,
            num_nodes: record.num_nodes,
            memory: record.memory,
            runtime: record.runtime,
        };

        Ok(GetResourceRequirementsResponse::SuccessfulResponse(
            resource_requirements,
        ))
    }

    /// Retrieve all resource requirements records for one workflow.
    async fn list_resource_requirements(
        &self,
        workflow_id: i64,
        job_id: Option<i64>,
        name: Option<String>,
        memory: Option<String>,
        num_cpus: Option<i64>,
        num_gpus: Option<i64>,
        num_nodes: Option<i64>,
        runtime: Option<i64>,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListResourceRequirementsResponse, ApiError> {
        debug!(
            "list_resource_requirements({}, {:?}, {:?}, {:?}, {:?}, {:?}, {:?}, {:?}, {}, {}, {:?}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            job_id,
            name,
            memory,
            num_cpus,
            num_gpus,
            num_nodes,
            runtime,
            offset,
            limit,
            sort_by,
            reverse_sort,
            context.get().0.clone()
        );

        // Build base query
        let base_query = "SELECT id, workflow_id, name, num_cpus, num_gpus, num_nodes, memory, runtime FROM resource_requirements".to_string();

        // Build WHERE clause conditions
        let mut where_conditions = vec!["workflow_id = ?".to_string()];

        if job_id.is_some() {
            where_conditions
                .push("id IN (SELECT resource_requirements_id FROM jobs WHERE id = ?)".to_string());
        }

        if name.is_some() {
            where_conditions.push("name = ?".to_string());
        }

        if memory.is_some() {
            where_conditions.push("memory = ?".to_string());
        }

        if num_cpus.is_some() {
            where_conditions.push("num_cpus = ?".to_string());
        }

        if num_gpus.is_some() {
            where_conditions.push("num_gpus = ?".to_string());
        }

        if num_nodes.is_some() {
            where_conditions.push("num_nodes = ?".to_string());
        }

        if runtime.is_some() {
            where_conditions.push("runtime_s = ?".to_string());
        }

        let where_clause = where_conditions.join(" AND ");

        // Validate sort_by against whitelist
        let validated_sort_by = if let Some(ref col) = sort_by {
            if RESOURCE_REQUIREMENTS_COLUMNS.contains(&col.as_str()) {
                Some(col.clone())
            } else {
                debug!("Invalid sort column requested: {}", col);
                None // Fall back to default
            }
        } else {
            None
        };

        // Build the complete query with pagination and sorting
        let query = SqlQueryBuilder::new(base_query)
            .with_where(where_clause.clone())
            .with_pagination_and_sorting(offset, limit, validated_sort_by, reverse_sort, "id")
            .build();

        debug!("Executing query: {}", query);

        // Execute the query
        let mut sqlx_query = sqlx::query(&query);

        // Bind workflow_id first
        sqlx_query = sqlx_query.bind(workflow_id);

        // Bind optional parameters in order they appear in the WHERE clause
        if let Some(job_id_val) = job_id {
            sqlx_query = sqlx_query.bind(job_id_val);
        }

        if let Some(name_filter) = &name {
            sqlx_query = sqlx_query.bind(name_filter);
        }

        if let Some(memory_filter) = &memory {
            sqlx_query = sqlx_query.bind(memory_filter);
        }

        if let Some(num_cpus_filter) = num_cpus {
            sqlx_query = sqlx_query.bind(num_cpus_filter);
        }

        if let Some(num_gpus_filter) = num_gpus {
            sqlx_query = sqlx_query.bind(num_gpus_filter);
        }

        if let Some(num_nodes_filter) = num_nodes {
            sqlx_query = sqlx_query.bind(num_nodes_filter);
        }

        if let Some(runtime_filter) = runtime {
            sqlx_query = sqlx_query.bind(runtime_filter);
        }

        let records = match sqlx_query.fetch_all(self.context.pool.as_ref()).await {
            Ok(recs) => recs,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to list resource requirements",
                ));
            }
        };

        let mut items: Vec<models::ResourceRequirementsModel> = Vec::new();
        for record in records {
            items.push(models::ResourceRequirementsModel {
                id: Some(record.get("id")),
                workflow_id: record.get("workflow_id"),
                name: record.get("name"),
                num_cpus: record.get("num_cpus"),
                num_gpus: record.get("num_gpus"),
                num_nodes: record.get("num_nodes"),
                memory: record.get("memory"),
                runtime: record.get("runtime"),
            });
        }

        // For proper pagination, we should get the total count without LIMIT/OFFSET
        let count_query =
            SqlQueryBuilder::new("SELECT COUNT(*) as total FROM resource_requirements".to_string())
                .with_where(where_clause)
                .build();

        let mut count_sqlx_query = sqlx::query(&count_query);
        count_sqlx_query = count_sqlx_query.bind(workflow_id);

        if let Some(job_id_val) = job_id {
            count_sqlx_query = count_sqlx_query.bind(job_id_val);
        }

        if let Some(name_filter) = &name {
            count_sqlx_query = count_sqlx_query.bind(name_filter);
        }

        if let Some(memory_filter) = &memory {
            count_sqlx_query = count_sqlx_query.bind(memory_filter);
        }

        if let Some(num_cpus_filter) = num_cpus {
            count_sqlx_query = count_sqlx_query.bind(num_cpus_filter);
        }

        if let Some(num_gpus_filter) = num_gpus {
            count_sqlx_query = count_sqlx_query.bind(num_gpus_filter);
        }

        if let Some(num_nodes_filter) = num_nodes {
            count_sqlx_query = count_sqlx_query.bind(num_nodes_filter);
        }

        if let Some(runtime_filter) = runtime {
            count_sqlx_query = count_sqlx_query.bind(runtime_filter);
        }

        let total_count = match count_sqlx_query.fetch_one(self.context.pool.as_ref()).await {
            Ok(row) => row.get::<i64, _>("total"),
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to list resource requirements",
                ));
            }
        };

        let current_count = items.len() as i64;
        let offset_val = offset;
        let has_more = offset_val + current_count < total_count;

        debug!(
            "list_resource_requirements({}, {}/{}) - X-Span-ID: {:?}",
            workflow_id,
            current_count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListResourceRequirementsResponse::SuccessfulResponse(
            models::ListResourceRequirementsResponse {
                items: Some(items),
                offset: offset_val,
                max_limit: MAX_RECORD_TRANSFER_COUNT,
                count: current_count,
                total_count,
                has_more,
            },
        ))
    }

    /// Update a resource requirements record.
    async fn update_resource_requirements(
        &self,
        id: i64,
        body: models::ResourceRequirementsModel,
        context: &C,
    ) -> Result<UpdateResourceRequirementsResponse, ApiError> {
        debug!(
            "update_resource_requirements({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        let memory_bytes = memory_string_to_bytes(&body.memory)
            .map_err(|e| ApiError(format!("Invalid memory format '{}': {}", body.memory, e)))?;

        let runtime_seconds = duration_string_to_seconds(&body.runtime)
            .map_err(|e| ApiError(format!("Invalid runtime format '{}': {}", body.runtime, e)))?;

        // First check if the record exists
        match self.get_resource_requirements(id, context).await? {
            GetResourceRequirementsResponse::SuccessfulResponse(_) => {}
            GetResourceRequirementsResponse::ForbiddenErrorResponse(err) => {
                return Ok(UpdateResourceRequirementsResponse::ForbiddenErrorResponse(
                    err,
                ));
            }
            GetResourceRequirementsResponse::NotFoundErrorResponse(err) => {
                return Ok(UpdateResourceRequirementsResponse::NotFoundErrorResponse(
                    err,
                ));
            }
            GetResourceRequirementsResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get resource requirements".to_string()));
            }
        };

        // Update the record
        match sqlx::query!(
            r#"
            UPDATE resource_requirements
            SET workflow_id = $1,
                name = $2,
                num_cpus = $3,
                num_gpus = $4,
                num_nodes = $5,
                memory = $6,
                runtime = $7,
                memory_bytes = $8,
                runtime_s = $9
            WHERE id = $10
            "#,
            body.workflow_id,
            body.name,
            body.num_cpus,
            body.num_gpus,
            body.num_nodes,
            body.memory,
            body.runtime,
            memory_bytes,
            runtime_seconds,
            id,
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(_) => {
                info!(
                    "Updated resource requirements with id: {} (name: {:?})",
                    id, body.name
                );
                let mut updated_body = body;
                updated_body.id = Some(id);
                Ok(UpdateResourceRequirementsResponse::SuccessfulResponse(
                    updated_body,
                ))
            }
            Err(e) => Err(database_error_with_msg(
                e,
                "Failed to update resource requirements",
            )),
        }
    }

    /// Delete a resource requirements record.
    async fn delete_resource_requirements(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteResourceRequirementsResponse, ApiError> {
        debug!(
            "delete_resource_requirements({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First get the resource requirements to ensure it exists and extract the ResourceRequirementsModel
        let resource_requirements = match self.get_resource_requirements(id, context).await? {
            GetResourceRequirementsResponse::SuccessfulResponse(resource_requirements) => {
                resource_requirements
            }
            GetResourceRequirementsResponse::ForbiddenErrorResponse(err) => {
                return Ok(DeleteResourceRequirementsResponse::ForbiddenErrorResponse(
                    err,
                ));
            }
            GetResourceRequirementsResponse::NotFoundErrorResponse(err) => {
                return Ok(DeleteResourceRequirementsResponse::NotFoundErrorResponse(
                    err,
                ));
            }
            GetResourceRequirementsResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get resource requirements".to_string()));
            }
        };

        match sqlx::query!(r#"DELETE FROM resource_requirements WHERE id = $1"#, id)
            .execute(self.context.pool.as_ref())
            .await
        {
            Ok(res) => {
                if res.rows_affected() > 1 {
                    Err(ApiError(format!(
                        "Database error: Unexpected number of rows affected: {}",
                        res.rows_affected()
                    )))
                } else if res.rows_affected() == 0 {
                    Err(ApiError("Database error: No rows affected".to_string()))
                } else {
                    info!("Deleted resource requirements with id: {}", id);
                    Ok(DeleteResourceRequirementsResponse::SuccessfulResponse(
                        resource_requirements,
                    ))
                }
            }
            Err(e) => {
                error!("Database error: {}", e);
                return Err(database_error_with_msg(
                    e,
                    "Failed to delete resource requirements",
                ));
            }
        }
    }
}

// Tests for memory_string_to_bytes are in src/memory_utils.rs
