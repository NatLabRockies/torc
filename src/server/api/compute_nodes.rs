//! Compute node-related API endpoints

#![allow(clippy::too_many_arguments)]

use async_trait::async_trait;
use log::{debug, error, info};
use sqlx::Row;
use swagger::{ApiError, Has, XSpanIdString};

use crate::server::api_types::{
    CreateComputeNodeResponse, DeleteComputeNodeResponse, DeleteComputeNodesResponse,
    GetComputeNodeResponse, ListComputeNodesResponse, UpdateComputeNodeResponse,
};

use crate::models;

use super::{ApiContext, MAX_RECORD_TRANSFER_COUNT, SqlQueryBuilder, database_error};

/// Trait defining compute node-related API operations
#[async_trait]
pub trait ComputeNodesApi<C> {
    /// Store a compute node.
    async fn create_compute_node(
        &self,
        body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<CreateComputeNodeResponse, ApiError>;

    /// Delete all compute nodes for one workflow.
    async fn delete_compute_nodes(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteComputeNodesResponse, ApiError>;

    /// Retrieve a compute node by ID.
    async fn get_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetComputeNodeResponse, ApiError>;

    /// Retrieve all compute nodes for one workflow.
    async fn list_compute_nodes(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        hostname: Option<String>,
        is_active: Option<bool>,
        scheduled_compute_node_id: Option<i64>,
        context: &C,
    ) -> Result<ListComputeNodesResponse, ApiError>;

    /// Update a compute node.
    async fn update_compute_node(
        &self,
        id: i64,
        body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<UpdateComputeNodeResponse, ApiError>;

    /// Delete a compute node.
    async fn delete_compute_node(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteComputeNodeResponse, ApiError>;
}

/// Implementation of compute nodes API for the server
#[derive(Clone)]
pub struct ComputeNodesApiImpl {
    pub context: ApiContext,
}

impl ComputeNodesApiImpl {
    pub fn new(context: ApiContext) -> Self {
        Self { context }
    }
}

#[async_trait]
impl<C> ComputeNodesApi<C> for ComputeNodesApiImpl
where
    C: Has<XSpanIdString> + Send + Sync,
{
    /// Store a compute node.
    async fn create_compute_node(
        &self,
        mut body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<CreateComputeNodeResponse, ApiError> {
        debug!(
            "create_compute_node({:?}) - X-Span-ID: {:?}",
            body,
            context.get().0.clone()
        );

        // Serialize scheduler to JSON string if present
        let scheduler_json = body
            .scheduler
            .as_ref()
            .and_then(|s| serde_json::to_string(s).ok());

        match sqlx::query!(
            r#"INSERT INTO compute_node (
                workflow_id
                ,hostname
                ,pid
                ,start_time
                ,duration_seconds
                ,is_active
                ,num_cpus
                ,memory_gb
                ,num_gpus
                ,num_nodes
                ,time_limit
                ,scheduler_config_id
                ,compute_node_type
                ,scheduler
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING rowid
            "#,
            body.workflow_id,
            body.hostname,
            body.pid,
            body.start_time,
            body.duration_seconds,
            body.is_active,
            body.num_cpus,
            body.memory_gb,
            body.num_gpus,
            body.num_nodes,
            body.time_limit,
            body.scheduler_config_id,
            body.compute_node_type,
            scheduler_json,
        )
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(results) => {
                body.id = Some(results[0].id);
                Ok(CreateComputeNodeResponse::SuccessfulResponse(body))
            }
            Err(e) => {
                debug!("Database error inserting compute node: {}", e);
                Err(database_error(e))
            }
        }
    }

    /// Delete all compute nodes for one workflow.
    async fn delete_compute_nodes(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteComputeNodesResponse, ApiError> {
        debug!(
            "delete_compute_nodes({}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            body,
            context.get().0.clone()
        );

        let result = match sqlx::query!(
            "DELETE FROM compute_node WHERE workflow_id = $1",
            workflow_id
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("Database error: {}", e);
                return Err(database_error(e));
            }
        };

        let deleted_count = result.rows_affected() as i64;

        info!(
            "Deleted {} compute nodes for workflow {}",
            deleted_count, workflow_id
        );

        Ok(DeleteComputeNodesResponse::SuccessfulResponse(
            serde_json::json!({
                "count": deleted_count
            }),
        ))
    }

    /// Retrieve a compute node by ID.
    async fn get_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetComputeNodeResponse, ApiError> {
        debug!(
            "get_compute_node({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        let record = match sqlx::query(
            r#"
            SELECT id, workflow_id, hostname, pid, start_time, duration_seconds, is_active,
                   num_cpus, memory_gb, num_gpus, num_nodes, time_limit, scheduler_config_id, compute_node_type, scheduler
            FROM compute_node
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(record)) => record,
            Ok(None) => {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Compute node not found with ID: {}", id)
                }));
                return Ok(GetComputeNodeResponse::NotFoundErrorResponse(
                    error_response,
                ));
            }
            Err(e) => {
                error!("Database error: {}", e);
                return Err(database_error(e));
            }
        };

        let is_active_val: i64 = record.get("is_active");
        let duration_seconds: Option<f64> = record.get("duration_seconds");
        let time_limit: Option<String> = record.get("time_limit");
        let scheduler_str: Option<String> = record.get("scheduler");

        // Deserialize scheduler JSON string to Value
        let scheduler = scheduler_str.and_then(|s| serde_json::from_str(&s).ok());

        let compute_node_model = models::ComputeNodeModel {
            id: Some(record.get("id")),
            workflow_id: record.get("workflow_id"),
            hostname: record.get("hostname"),
            pid: record.get("pid"),
            start_time: record.get("start_time"),
            duration_seconds,
            is_active: if is_active_val == 1 {
                Some(true)
            } else {
                Some(false)
            },
            num_cpus: record.get("num_cpus"),
            memory_gb: record.get("memory_gb"),
            num_gpus: record.get("num_gpus"),
            num_nodes: record.get("num_nodes"),
            time_limit,
            scheduler_config_id: record.get("scheduler_config_id"),
            compute_node_type: record.get("compute_node_type"),
            scheduler,
        };

        Ok(GetComputeNodeResponse::SuccessfulResponse(
            compute_node_model,
        ))
    }

    /// Retrieve all compute nodes for one workflow.
    async fn list_compute_nodes(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        hostname: Option<String>,
        is_active: Option<bool>,
        scheduled_compute_node_id: Option<i64>,
        context: &C,
    ) -> Result<ListComputeNodesResponse, ApiError> {
        debug!(
            "list_compute_nodes({}, {}, {}, {:?}, {:?}, {:?}, {:?}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            hostname,
            is_active,
            scheduled_compute_node_id,
            context.get().0.clone()
        );

        // Build base query
        let base_query = "
            SELECT
                id
                ,workflow_id
                ,hostname
                ,pid
                ,start_time
                ,duration_seconds
                ,is_active
                ,num_cpus
                ,memory_gb
                ,num_gpus
                ,num_nodes
                ,time_limit
                ,scheduler_config_id
                ,compute_node_type
                ,scheduler
            FROM compute_node"
            .to_string();

        // Build WHERE clause conditions
        let mut where_conditions = vec!["workflow_id = ?".to_string()];
        if hostname.is_some() {
            where_conditions.push("hostname = ?".to_string());
        }
        if is_active.is_some() {
            where_conditions.push("is_active = ?".to_string());
        }
        if scheduled_compute_node_id.is_some() {
            // Filter by scheduler.scheduler_id in the JSON field
            where_conditions.push("json_extract(scheduler, '$.scheduler_id') = ?".to_string());
        }
        let where_clause = where_conditions.join(" AND ");

        let query = SqlQueryBuilder::new(base_query)
            .with_where(where_clause.clone())
            .with_pagination_and_sorting(offset, limit, sort_by, reverse_sort, "id")
            .build();

        debug!("Executing query: {}", query);

        // Execute the query
        let mut sqlx_query = sqlx::query(&query).bind(workflow_id);
        if let Some(ref h) = hostname {
            sqlx_query = sqlx_query.bind(h);
        }
        if let Some(active) = is_active {
            // Convert boolean to integer for SQLite
            sqlx_query = sqlx_query.bind(if active { 1i64 } else { 0i64 });
        }
        if let Some(scn_id) = scheduled_compute_node_id {
            sqlx_query = sqlx_query.bind(scn_id);
        }

        let records = match sqlx_query.fetch_all(self.context.pool.as_ref()).await {
            Ok(recs) => recs,
            Err(e) => {
                error!("Database error: {}", e);
                return Err(database_error(e));
            }
        };

        let mut items: Vec<models::ComputeNodeModel> = Vec::new();
        for record in records {
            let is_active_val: i64 = record.get("is_active");
            let duration_seconds: Option<f64> = record.get("duration_seconds");
            let time_limit: Option<String> = record.get("time_limit");
            let scheduler_str: Option<String> = record.get("scheduler");

            // Deserialize scheduler JSON string to Value
            let scheduler = scheduler_str.and_then(|s| serde_json::from_str(&s).ok());

            items.push(models::ComputeNodeModel {
                id: Some(record.get("id")),
                workflow_id: record.get("workflow_id"),
                hostname: record.get("hostname"),
                pid: record.get("pid"),
                start_time: record.get("start_time"),
                duration_seconds,
                is_active: if is_active_val == 1 {
                    Some(true)
                } else {
                    Some(false)
                },
                num_cpus: record.get("num_cpus"),
                memory_gb: record.get("memory_gb"),
                num_gpus: record.get("num_gpus"),
                num_nodes: record.get("num_nodes"),
                time_limit,
                scheduler_config_id: record.get("scheduler_config_id"),
                compute_node_type: record.get("compute_node_type"),
                scheduler,
            });
        }

        // For proper pagination, we should get the total count without LIMIT/OFFSET
        let count_query =
            SqlQueryBuilder::new("SELECT COUNT(*) as total FROM compute_node".to_string())
                .with_where(where_clause)
                .build();

        let mut count_sqlx_query = sqlx::query(&count_query).bind(workflow_id);
        if let Some(scn_id) = scheduled_compute_node_id {
            count_sqlx_query = count_sqlx_query.bind(scn_id);
        }

        let total_count = match count_sqlx_query.fetch_one(self.context.pool.as_ref()).await {
            Ok(row) => row.get::<i64, _>("total"),
            Err(e) => {
                error!("Database error getting count: {}", e);
                return Err(database_error(e));
            }
        };

        let current_count = items.len() as i64;
        let offset_val = offset;
        let has_more = offset_val + current_count < total_count;

        debug!(
            "list_compute_nodes({}, {}/{}) - X-Span-ID: {:?}",
            workflow_id,
            current_count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListComputeNodesResponse::SuccessfulResponse(
            models::ListComputeNodesResponse {
                items: Some(items),
                offset: offset_val,
                max_limit: MAX_RECORD_TRANSFER_COUNT,
                count: current_count,
                total_count,
                has_more,
            },
        ))
    }

    /// Update a compute node.
    async fn update_compute_node(
        &self,
        id: i64,
        body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<UpdateComputeNodeResponse, ApiError> {
        debug!(
            "update_compute_node({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First get the existing compute node to ensure it exists
        match self.get_compute_node(id, context).await? {
            GetComputeNodeResponse::SuccessfulResponse(compute_node) => compute_node,
            GetComputeNodeResponse::ForbiddenErrorResponse(err) => {
                return Ok(UpdateComputeNodeResponse::ForbiddenErrorResponse(err));
            }
            GetComputeNodeResponse::NotFoundErrorResponse(err) => {
                return Ok(UpdateComputeNodeResponse::NotFoundErrorResponse(err));
            }
            GetComputeNodeResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get compute node".to_string()));
            }
        };

        // Convert boolean to integer for database storage
        let is_active_val = body.is_active.map(|b| if b { 1i64 } else { 0i64 });

        // Serialize scheduler to JSON string if present
        let scheduler_json = body
            .scheduler
            .as_ref()
            .and_then(|s| serde_json::to_string(s).ok());

        let result = match sqlx::query(
            r#"
            UPDATE compute_node
            SET
                workflow_id = COALESCE($1, workflow_id)
                ,hostname = COALESCE($2, hostname)
                ,pid = COALESCE($3, pid)
                ,start_time = COALESCE($4, start_time)
                ,duration_seconds = COALESCE($5, duration_seconds)
                ,is_active = COALESCE($6, is_active)
                ,num_cpus = COALESCE($7, num_cpus)
                ,memory_gb = COALESCE($8, memory_gb)
                ,num_gpus = COALESCE($9, num_gpus)
                ,num_nodes = COALESCE($10, num_nodes)
                ,time_limit = COALESCE($11, time_limit)
                ,scheduler_config_id = COALESCE($12, scheduler_config_id)
                ,compute_node_type = COALESCE($13, compute_node_type)
                ,scheduler = COALESCE($14, scheduler)
            WHERE id = $15
            "#,
        )
        .bind(body.workflow_id)
        .bind(body.hostname)
        .bind(body.pid)
        .bind(body.start_time)
        .bind(body.duration_seconds)
        .bind(is_active_val)
        .bind(body.num_cpus)
        .bind(body.memory_gb)
        .bind(body.num_gpus)
        .bind(body.num_nodes)
        .bind(body.time_limit)
        .bind(body.scheduler_config_id)
        .bind(body.compute_node_type)
        .bind(scheduler_json)
        .bind(id)
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("Database error: {}", e);
                return Err(database_error(e));
            }
        };

        if result.rows_affected() == 0 {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Compute node not found with ID: {}", id)
            }));
            return Ok(UpdateComputeNodeResponse::NotFoundErrorResponse(
                error_response,
            ));
        }

        // Return the updated compute node by fetching it again
        let updated_compute_node = match self.get_compute_node(id, context).await? {
            GetComputeNodeResponse::SuccessfulResponse(compute_node) => compute_node,
            _ => return Err(ApiError("Failed to get updated compute node".to_string())),
        };

        debug!("Modified compute node with id: {}", id);
        Ok(UpdateComputeNodeResponse::SuccessfulResponse(
            updated_compute_node,
        ))
    }

    /// Delete a compute node.
    async fn delete_compute_node(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteComputeNodeResponse, ApiError> {
        debug!(
            "delete_compute_node({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First get the compute node to ensure it exists and extract the ComputeNodeModel
        let compute_node = match self.get_compute_node(id, context).await? {
            GetComputeNodeResponse::SuccessfulResponse(compute_node) => compute_node,
            GetComputeNodeResponse::ForbiddenErrorResponse(err) => {
                return Ok(DeleteComputeNodeResponse::ForbiddenErrorResponse(err));
            }
            GetComputeNodeResponse::NotFoundErrorResponse(err) => {
                return Ok(DeleteComputeNodeResponse::NotFoundErrorResponse(err));
            }
            GetComputeNodeResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get compute node".to_string()));
            }
        };

        match sqlx::query("DELETE FROM compute_node WHERE id = $1")
            .bind(id)
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
                    info!("Deleted compute node with id: {}", id);
                    Ok(DeleteComputeNodeResponse::SuccessfulResponse(compute_node))
                }
            }
            Err(e) => {
                error!("Database error: {}", e);
                Err(database_error(e))
            }
        }
    }
}
