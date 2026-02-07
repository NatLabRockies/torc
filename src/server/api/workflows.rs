//! Workflow-related API endpoints

#![allow(clippy::too_many_arguments)]

use async_trait::async_trait;
use chrono::Utc;
use log::{debug, error, info};
use sqlx::Row;
use swagger::{ApiError, Has, XSpanIdString};

use crate::server::api_types::{
    CancelWorkflowResponse, CreateWorkflowResponse, DeleteWorkflowResponse, GetWorkflowResponse,
    GetWorkflowStatusResponse, IsWorkflowCompleteResponse, IsWorkflowUninitializedResponse,
    ListJobDependenciesResponse, ListJobFileRelationshipsResponse,
    ListJobUserDataRelationshipsResponse, ListWorkflowsResponse, ResetWorkflowStatusResponse,
    UpdateWorkflowResponse, UpdateWorkflowStatusResponse,
};

use crate::models;

use super::{ApiContext, MAX_RECORD_TRANSFER_COUNT, SqlQueryBuilder, database_error};

/// Trait defining workflow-related API operations
#[async_trait]
pub trait WorkflowsApi<C> {
    /// Check if the workflow exists.
    async fn does_workflow_exist(&self, id: i64, context: &C) -> Result<bool, ApiError>;

    /// Store a workflow.
    async fn create_workflow(
        &self,
        mut body: models::WorkflowModel,
        context: &C,
    ) -> Result<CreateWorkflowResponse, ApiError>;

    /// Cancel a workflow. Workers will detect the status change and cancel jobs.
    async fn cancel_workflow(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<CancelWorkflowResponse, ApiError>;

    /// Retrieve a workflow.
    async fn get_workflow(&self, id: i64, context: &C) -> Result<GetWorkflowResponse, ApiError>;

    /// Return the workflow status.
    async fn get_workflow_status(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetWorkflowStatusResponse, ApiError>;

    /// Return true if all jobs in the workflow are complete.
    async fn is_workflow_complete(
        &self,
        id: i64,
        context: &C,
    ) -> Result<IsWorkflowCompleteResponse, ApiError>;

    /// Return true if all jobs in the workflow are uninitialized or disabled.
    async fn is_workflow_uninitialized(
        &self,
        id: i64,
        context: &C,
    ) -> Result<IsWorkflowUninitializedResponse, ApiError>;

    /// Retrieve all workflows.
    async fn list_workflows(
        &self,
        offset: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        limit: i64,
        name: Option<String>,
        user: Option<String>,
        description: Option<String>,
        is_archived: Option<bool>,
        context: &C,
    ) -> Result<ListWorkflowsResponse, ApiError>;

    /// Retrieve job blocking relationships for a workflow.
    async fn list_job_dependencies(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListJobDependenciesResponse, ApiError>;

    /// Retrieve job-file relationships for a workflow.
    async fn list_job_file_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListJobFileRelationshipsResponse, ApiError>;

    /// Retrieve job-user_data relationships for a workflow.
    async fn list_job_user_data_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListJobUserDataRelationshipsResponse, ApiError>;

    /// Update a workflow.
    async fn update_workflow(
        &self,
        id: i64,
        body: models::WorkflowModel,
        context: &C,
    ) -> Result<UpdateWorkflowResponse, ApiError>;

    /// Update the workflow status.
    async fn update_workflow_status(
        &self,
        id: i64,
        body: models::WorkflowStatusModel,
        context: &C,
    ) -> Result<UpdateWorkflowStatusResponse, ApiError>;

    /// Delete a workflow.
    async fn delete_workflow(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteWorkflowResponse, ApiError>;

    /// Reset workflow status.
    async fn reset_workflow_status(
        &self,
        id: i64,
        force: Option<bool>,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<ResetWorkflowStatusResponse, ApiError>;
}

/// Implementation of workflows API for the server
#[derive(Clone)]
pub struct WorkflowsApiImpl {
    pub context: ApiContext,
}

impl WorkflowsApiImpl {
    pub fn new(context: ApiContext) -> Self {
        Self { context }
    }

    /// List workflows with optional access control filtering.
    ///
    /// When `accessible_ids` is `Some(ids)`, only workflows with IDs in the list are returned.
    /// When `accessible_ids` is `None`, no ID-based filtering is applied.
    pub async fn list_workflows_filtered<C>(
        &self,
        offset: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        limit: i64,
        name: Option<String>,
        user: Option<String>,
        description: Option<String>,
        is_archived: Option<bool>,
        accessible_ids: Option<Vec<i64>>,
        context: &C,
    ) -> Result<ListWorkflowsResponse, ApiError>
    where
        C: Has<XSpanIdString> + Send + Sync,
    {
        debug!(
            "list_workflows_filtered({}, {:?}, {:?}, {}, {:?}, {:?}, {:?}, {:?}, accessible_ids={:?}) - X-Span-ID: {:?}",
            offset,
            sort_by,
            reverse_sort,
            limit,
            name,
            user,
            description,
            is_archived,
            accessible_ids.as_ref().map(|ids| ids.len()),
            context.get().0.clone()
        );

        // Build base query - join with workflow_status if is_archived filter is needed
        let base_query = if is_archived.is_some() {
            "
            SELECT
                w.id
                ,w.name
                ,w.user
                ,w.description
                ,w.timestamp
                ,w.compute_node_expiration_buffer_seconds
                ,w.compute_node_wait_for_new_jobs_seconds
                ,w.compute_node_ignore_workflow_completion
                ,w.compute_node_wait_for_healthy_database_minutes
                ,w.compute_node_min_time_for_new_jobs_seconds
                ,w.jobs_sort_method
                ,w.resource_monitor_config
                ,w.slurm_defaults
                ,w.use_pending_failed
                ,w.project
                ,w.metadata
                ,w.status_id
            FROM workflow w
            INNER JOIN workflow_status ws ON w.status_id = ws.id
            "
            .to_string()
        } else {
            "
            SELECT
                id
                ,name
                ,user
                ,description
                ,timestamp
                ,compute_node_expiration_buffer_seconds
                ,compute_node_wait_for_new_jobs_seconds
                ,compute_node_ignore_workflow_completion
                ,compute_node_wait_for_healthy_database_minutes
                ,compute_node_min_time_for_new_jobs_seconds
                ,jobs_sort_method
                ,resource_monitor_config
                ,slurm_defaults
                ,use_pending_failed
                ,project
                ,metadata
                ,status_id
            FROM workflow
            "
            .to_string()
        };

        // Build WHERE clause conditions
        let mut where_conditions = Vec::new();

        // Use table prefix when joining with workflow_status
        let table_prefix = if is_archived.is_some() { "w." } else { "" };

        if name.is_some() {
            where_conditions.push(format!("{}name = ?", table_prefix));
        }

        if user.is_some() {
            where_conditions.push(format!("{}user = ?", table_prefix));
        }

        if description.is_some() {
            where_conditions.push(format!("{}description LIKE ?", table_prefix));
        }

        if let Some(archived) = is_archived {
            if archived {
                where_conditions.push("ws.is_archived = 1".to_string());
            } else {
                where_conditions.push("(ws.is_archived IS NULL OR ws.is_archived = 0)".to_string());
            }
        }

        // Access control filtering: restrict to accessible workflow IDs
        if let Some(ref ids) = accessible_ids {
            if ids.is_empty() {
                // No accessible workflows - return empty result
                return Ok(ListWorkflowsResponse::SuccessfulResponse(
                    models::ListWorkflowsResponse {
                        items: Some(Vec::new()),
                        offset,
                        max_limit: MAX_RECORD_TRANSFER_COUNT,
                        count: 0,
                        total_count: 0,
                        has_more: false,
                    },
                ));
            }
            let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
            where_conditions.push(format!(
                "{}id IN ({})",
                table_prefix,
                placeholders.join(", ")
            ));
        }

        let where_clause = if where_conditions.is_empty() {
            String::new()
        } else {
            where_conditions.join(" AND ")
        };

        // Build the complete query with pagination and sorting
        // Use table prefix for default sort column when joining
        let default_sort_column = if is_archived.is_some() { "w.id" } else { "id" };

        // Add table prefix to sort_by column if joining and column is ambiguous
        let prefixed_sort_by = if is_archived.is_some() {
            sort_by.as_ref().map(|col| {
                // If the column is "id" (ambiguous), prefix it with "w."
                // For other workflow columns, also add prefix to be consistent
                if col == "id" || !col.contains('.') {
                    format!("w.{}", col)
                } else {
                    col.clone()
                }
            })
        } else {
            sort_by.clone()
        };

        let query = if where_clause.is_empty() {
            SqlQueryBuilder::new(base_query)
                .with_pagination_and_sorting(
                    offset,
                    limit,
                    prefixed_sort_by,
                    reverse_sort,
                    default_sort_column,
                )
                .build()
        } else {
            SqlQueryBuilder::new(base_query)
                .with_where(where_clause.clone())
                .with_pagination_and_sorting(
                    offset,
                    limit,
                    prefixed_sort_by,
                    reverse_sort,
                    default_sort_column,
                )
                .build()
        };

        debug!("Executing query: {}", query);

        // Execute the query
        let mut sqlx_query = sqlx::query(&query);

        // Bind optional parameters in order
        if let Some(workflow_name) = &name {
            sqlx_query = sqlx_query.bind(workflow_name);
        }
        if let Some(workflow_user) = &user {
            sqlx_query = sqlx_query.bind(workflow_user);
        }
        if let Some(workflow_description) = &description {
            sqlx_query = sqlx_query.bind(format!("%{}%", workflow_description));
        }
        // Bind accessible IDs
        if let Some(ref ids) = accessible_ids {
            for id in ids {
                sqlx_query = sqlx_query.bind(id);
            }
        }

        let records = match sqlx_query.fetch_all(self.context.pool.as_ref()).await {
            Ok(recs) => recs,
            Err(e) => {
                error!("Database error: {}", e);
                return Err(database_error(e));
            }
        };

        let mut items: Vec<models::WorkflowModel> = Vec::new();
        for record in records {
            let jobs_sort_method_str: String = record.get("jobs_sort_method");
            let sort_method = jobs_sort_method_str
                .parse::<models::ClaimJobsSortMethod>()
                .ok();
            items.push(models::WorkflowModel {
                id: Some(record.get("id")),
                name: record.get("name"),
                user: record.get("user"),
                description: record.get("description"),
                timestamp: Some(record.get("timestamp")),
                compute_node_expiration_buffer_seconds: Some(
                    record.get("compute_node_expiration_buffer_seconds"),
                ),
                compute_node_wait_for_new_jobs_seconds: Some(
                    record.get("compute_node_wait_for_new_jobs_seconds"),
                ),
                compute_node_ignore_workflow_completion: Some(
                    record.get::<i64, _>("compute_node_ignore_workflow_completion") != 0,
                ),
                compute_node_wait_for_healthy_database_minutes: Some(
                    record.get("compute_node_wait_for_healthy_database_minutes"),
                ),
                compute_node_min_time_for_new_jobs_seconds: Some(
                    record.get("compute_node_min_time_for_new_jobs_seconds"),
                ),
                jobs_sort_method: sort_method,
                resource_monitor_config: record.get("resource_monitor_config"),
                slurm_defaults: record.get("slurm_defaults"),
                use_pending_failed: record
                    .try_get::<Option<i64>, _>("use_pending_failed")
                    .ok()
                    .flatten()
                    .map(|v| v != 0),
                project: record.get("project"),
                metadata: record.get("metadata"),
                status_id: Some(record.get("status_id")),
            });
        }

        // For proper pagination, we should get the total count without LIMIT/OFFSET
        let count_base_query = if is_archived.is_some() {
            "SELECT COUNT(*) as total FROM workflow w INNER JOIN workflow_status ws ON w.status_id = ws.id"
        } else {
            "SELECT COUNT(*) as total FROM workflow"
        };
        let count_query = if where_clause.is_empty() {
            count_base_query.to_string()
        } else {
            format!("{} WHERE {}", count_base_query, where_clause)
        };

        let mut count_sqlx_query = sqlx::query(&count_query);
        if let Some(workflow_name) = &name {
            count_sqlx_query = count_sqlx_query.bind(workflow_name);
        }
        if let Some(workflow_user) = &user {
            count_sqlx_query = count_sqlx_query.bind(workflow_user);
        }
        if let Some(workflow_description) = &description {
            count_sqlx_query = count_sqlx_query.bind(format!("%{}%", workflow_description));
        }
        // Bind accessible IDs for count query
        if let Some(ref ids) = accessible_ids {
            for id in ids {
                count_sqlx_query = count_sqlx_query.bind(id);
            }
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
            "list_workflows_filtered({}/{}) - X-Span-ID: {:?}",
            current_count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListWorkflowsResponse::SuccessfulResponse(
            models::ListWorkflowsResponse {
                items: Some(items),
                offset: offset_val,
                max_limit: MAX_RECORD_TRANSFER_COUNT,
                count: current_count,
                total_count,
                has_more,
            },
        ))
    }
}

#[async_trait]
impl<C> WorkflowsApi<C> for WorkflowsApiImpl
where
    C: Has<XSpanIdString> + Send + Sync,
{
    /// Check if the workflow exists.
    async fn does_workflow_exist(&self, id: i64, _context: &C) -> Result<bool, ApiError> {
        let workflow_exists = sqlx::query("SELECT id FROM workflow WHERE id = $1")
            .bind(id)
            .fetch_optional(self.context.pool.as_ref())
            .await
            .map_err(database_error)?;

        Ok(workflow_exists.is_some())
    }

    /// Store a workflow.
    ///
    /// This operation wraps workflow and workflow_status creation in a transaction
    /// to ensure atomicity. If either insert fails, both will be rolled back.
    async fn create_workflow(
        &self,
        mut body: models::WorkflowModel,
        context: &C,
    ) -> Result<CreateWorkflowResponse, ApiError> {
        info!(
            "create_workflow({:?}) - X-Span-ID: {:?}",
            body,
            context.get().0.clone()
        );

        // Begin a transaction to ensure workflow and workflow_status are created atomically
        let mut tx = match self.context.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                return Err(database_error(e));
            }
        };

        // First, create the workflow_status record
        let status_result = match sqlx::query!(
            r#"
            INSERT INTO workflow_status
            (run_id, is_archived, is_canceled, has_detected_need_to_run_completion_script)
            VALUES (0, 0, 0, 0)
            RETURNING rowid
            "#
        )
        .fetch_all(&mut *tx)
        .await
        {
            Ok(status_result) => status_result,
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(database_error(e));
            }
        };

        body.timestamp = Some(Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string());
        let jobs_sort_method_str = body
            .jobs_sort_method
            .map(|m| m.to_string())
            .unwrap_or_else(|| "gpus_runtime_memory".to_string());

        let compute_node_expiration_buffer_seconds =
            body.compute_node_expiration_buffer_seconds.unwrap_or(180);
        // Default must be >= completion_check_interval_secs + job_completion_poll_interval
        // to avoid workers exiting before dependent jobs are unblocked.
        let compute_node_wait_for_new_jobs_seconds =
            body.compute_node_wait_for_new_jobs_seconds.unwrap_or(90);
        let compute_node_ignore_workflow_completion = body
            .compute_node_ignore_workflow_completion
            .unwrap_or(false) as i64;
        let compute_node_wait_for_healthy_database_minutes = body
            .compute_node_wait_for_healthy_database_minutes
            .unwrap_or(20);
        let compute_node_min_time_for_new_jobs_seconds = body
            .compute_node_min_time_for_new_jobs_seconds
            .unwrap_or(300);

        // Then, create the workflow record
        let use_pending_failed_int = body.use_pending_failed.map(|v| if v { 1 } else { 0 });

        let workflow_result = match sqlx::query!(
            r#"
            INSERT INTO workflow
            (
                name,
                description,
                user,
                timestamp,
                compute_node_expiration_buffer_seconds,
                compute_node_wait_for_new_jobs_seconds,
                compute_node_ignore_workflow_completion,
                compute_node_wait_for_healthy_database_minutes,
                compute_node_min_time_for_new_jobs_seconds,
                jobs_sort_method,
                resource_monitor_config,
                slurm_defaults,
                use_pending_failed,
                project,
                metadata,
                status_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            RETURNING rowid
            "#,
            body.name,
            body.description,
            body.user,
            body.timestamp,
            compute_node_expiration_buffer_seconds,
            compute_node_wait_for_new_jobs_seconds,
            compute_node_ignore_workflow_completion,
            compute_node_wait_for_healthy_database_minutes,
            compute_node_min_time_for_new_jobs_seconds,
            jobs_sort_method_str,
            body.resource_monitor_config,
            body.slurm_defaults,
            use_pending_failed_int,
            body.project,
            body.metadata,
            status_result[0].id,
        )
        .fetch_all(&mut *tx)
        .await
        {
            Ok(workflow_result) => workflow_result,
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(database_error(e));
            }
        };

        // Commit the transaction
        if let Err(e) = tx.commit().await {
            return Err(database_error(e));
        }

        debug!("Workflow inserted with id: {:?}", workflow_result[0].id);
        body.id = Some(workflow_result[0].id);
        body.status_id = Some(status_result[0].id);
        let response = CreateWorkflowResponse::SuccessfulResponse(body);
        Ok(response)
    }

    /// Cancel a workflow. Workers will detect the status change and cancel jobs.
    ///
    /// This operation wraps workflow status update and job cancellation in a transaction
    /// to ensure atomicity. If either update fails, both will be rolled back.
    async fn cancel_workflow(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<CancelWorkflowResponse, ApiError> {
        info!(
            "cancel_workflow({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First get the current workflow status to preserve other fields
        let current_status = match self.get_workflow_status(id, context).await? {
            GetWorkflowStatusResponse::SuccessfulResponse(status) => status,
            _ => {
                error!(
                    "Failed to get current workflow status for workflow_id={}",
                    id
                );
                return Err(ApiError(
                    "Failed to get current workflow status".to_string(),
                ));
            }
        };

        // Begin a transaction to ensure workflow cancellation and job cancellation are atomic
        let mut tx = match self.context.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                return Err(database_error(e));
            }
        };

        // Convert boolean values to integers for SQLite storage
        let is_canceled_int = 1; // Setting to canceled
        let is_archived_int = if current_status.is_archived.unwrap_or(false) {
            1
        } else {
            0
        };
        let has_detected_need_to_run_completion_script_int = if current_status
            .has_detected_need_to_run_completion_script
            .unwrap_or(false)
        {
            1
        } else {
            0
        };

        // Update the workflow status to mark it as canceled
        let result = match sqlx::query!(
            r#"
            UPDATE workflow_status
            SET run_id = ?,
                has_detected_need_to_run_completion_script = ?,
                is_canceled = ?,
                is_archived = ?
            WHERE id = ?
            "#,
            current_status.run_id,
            has_detected_need_to_run_completion_script_int,
            is_canceled_int,
            is_archived_int,
            id
        )
        .execute(&mut *tx)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(database_error(e));
            }
        };

        if result.rows_affected() == 0 {
            let _ = tx.rollback().await;
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow status not found with ID: {}", id)
            }));
            return Ok(CancelWorkflowResponse::DefaultErrorResponse(error_response));
        }

        info!("Successfully canceled workflow with ID: {}", id);

        // Create an event to record the workflow cancellation
        let timestamp = Utc::now().timestamp_millis();
        let event_data = serde_json::json!({
            "category": "workflow_canceled",
            "message": format!("Workflow {} (run_id={}) was canceled", id, current_status.run_id),
            "workflow_id": id,
            "run_id": current_status.run_id,
        });
        let event_data_str = event_data.to_string();

        match sqlx::query(
            r#"
            INSERT INTO event (workflow_id, timestamp, data)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(id)
        .bind(timestamp)
        .bind(&event_data_str)
        .execute(&mut *tx)
        .await
        {
            Ok(_) => {
                debug!("Created workflow_canceled event for workflow {}", id);
            }
            Err(e) => {
                let _ = tx.rollback().await;
                error!("Failed to create workflow_canceled event: {}", e);
                return Err(database_error(e));
            }
        }

        // Cancel all running and pending jobs in the workflow
        let submitted_status = models::JobStatus::Running.to_int();
        let submitted_pending_status = models::JobStatus::Pending.to_int();
        let canceled_status = models::JobStatus::Canceled.to_int();

        match sqlx::query!(
            r#"
            UPDATE job
            SET status = $1
            WHERE workflow_id = $2 AND (status = $3 OR status = $4)
            "#,
            canceled_status,
            id,
            submitted_status,
            submitted_pending_status
        )
        .execute(&mut *tx)
        .await
        {
            Ok(result) => {
                let canceled_jobs_count = result.rows_affected();
                if canceled_jobs_count > 0 {
                    info!(
                        "Canceled {} running/pending jobs for workflow {}",
                        canceled_jobs_count, id
                    );
                } else {
                    info!("No running/pending jobs to cancel for workflow {}", id);
                }
            }
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(database_error(e));
            }
        }

        // Commit the transaction
        if let Err(e) = tx.commit().await {
            return Err(database_error(e));
        }

        let response_json = serde_json::json!({
            "id": current_status.id,
            "is_canceled": true,
            "is_archived": current_status.is_archived.unwrap_or(false),
            "run_id": current_status.run_id,
            "has_detected_need_to_run_completion_script": current_status.has_detected_need_to_run_completion_script.unwrap_or(false),
        });
        Ok(CancelWorkflowResponse::SuccessfulResponse(response_json))
    }

    /// Retrieve a workflow.
    async fn get_workflow(&self, id: i64, context: &C) -> Result<GetWorkflowResponse, ApiError> {
        debug!(
            "get_workflow({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );
        match sqlx::query!(
            r#"
                SELECT *
                FROM workflow
                WHERE id = $1
            "#,
            id
        )
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(row)) => Ok(GetWorkflowResponse::SuccessfulResponse(
                models::WorkflowModel {
                    id: Some(row.id),
                    name: row.name,
                    user: row.user,
                    description: row.description,
                    timestamp: Some(row.timestamp),
                    compute_node_expiration_buffer_seconds: Some(
                        row.compute_node_expiration_buffer_seconds,
                    ),
                    compute_node_wait_for_new_jobs_seconds: Some(
                        row.compute_node_wait_for_new_jobs_seconds,
                    ),
                    compute_node_ignore_workflow_completion: Some(
                        row.compute_node_ignore_workflow_completion != 0,
                    ),
                    compute_node_wait_for_healthy_database_minutes: Some(
                        row.compute_node_wait_for_healthy_database_minutes,
                    ),
                    compute_node_min_time_for_new_jobs_seconds: Some(
                        row.compute_node_min_time_for_new_jobs_seconds,
                    ),
                    jobs_sort_method: row
                        .jobs_sort_method
                        .parse::<models::ClaimJobsSortMethod>()
                        .ok(),
                    resource_monitor_config: row.resource_monitor_config,
                    slurm_defaults: row.slurm_defaults,
                    use_pending_failed: row.use_pending_failed.map(|v| v != 0),
                    project: row.project,
                    metadata: row.metadata,
                    status_id: Some(row.status_id),
                },
            )),
            Ok(None) => {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Workflow not found with ID: {}", id)
                }));
                Ok(GetWorkflowResponse::NotFoundErrorResponse(error_response))
            }
            Err(e) => Err(database_error(e)),
        }
    }

    /// Return the workflow status.
    ///
    /// Retrieves the workflow status from the workflow_status table for the specified workflow ID.
    /// Converts SQLite INTEGER boolean fields (0/1) to proper Rust boolean values.
    ///
    /// # Parameters
    /// - `id`: The workflow ID to retrieve status for
    /// - `context`: Request context containing span ID for tracing
    ///
    /// # Returns
    /// - `Ok(GetWorkflowStatusResponse::SuccessfulResponse(WorkflowStatusModel))` on success
    /// - `Err(ApiError)` if workflow not found or database error occurs
    ///
    /// # Database Schema
    /// Queries the workflow_status table with columns:
    /// - id (INTEGER PRIMARY KEY) - Workflow identifier
    /// - run_id (INTEGER) - Current run iteration
    /// - has_detected_need_to_run_completion_script (INTEGER) - Boolean flag (0/1)
    /// - is_canceled (INTEGER) - Boolean cancellation flag (0/1)
    /// - is_archived (INTEGER) - Boolean archival flag (0/1)
    async fn get_workflow_status(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetWorkflowStatusResponse, ApiError> {
        debug!(
            "get_workflow_status({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        // Query the workflow_status table for the specified workflow ID
        let row = match sqlx::query!(
            "SELECT id, run_id, has_detected_need_to_run_completion_script, is_canceled, is_archived FROM workflow_status WHERE id = ?",
            id
        )
        .fetch_optional(&*self.context.pool)
        .await
        {
            Ok(Some(row)) => row,
            Ok(None) => {
                let error_response = models::ErrorResponse::new(
                    serde_json::json!({
                        "message": format!("Workflow status not found with ID: {}", id)
                    })
                );
                return Ok(GetWorkflowStatusResponse::NotFoundErrorResponse(error_response));
            }
            Err(e) => {
                return Err(database_error(e));
            }
        };

        // Convert database row to WorkflowStatusModel
        // SQLite INTEGER fields (0/1) are converted to proper boolean values
        let workflow_status = models::WorkflowStatusModel {
            id: Some(row.id),
            is_canceled: row.is_canceled != 0, // Convert INTEGER to bool
            is_archived: Some(row.is_archived != 0), // Convert INTEGER to bool
            run_id: row.run_id,
            has_detected_need_to_run_completion_script: Some(
                row.has_detected_need_to_run_completion_script != 0,
            ), // Convert INTEGER to bool
        };

        Ok(GetWorkflowStatusResponse::SuccessfulResponse(
            workflow_status,
        ))
    }

    /// Return true if all jobs in the workflow are complete.
    async fn is_workflow_complete(
        &self,
        id: i64,
        context: &C,
    ) -> Result<IsWorkflowCompleteResponse, ApiError> {
        debug!(
            "is_workflow_complete({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        // Get workflow status to check if canceled
        let workflow_status = match self.get_workflow_status(id, context).await? {
            GetWorkflowStatusResponse::SuccessfulResponse(status) => status,
            _ => {
                error!("Failed to get workflow status for workflow_id={}", id);
                return Err(ApiError("Failed to get workflow status".to_string()));
            }
        };

        let is_canceled = workflow_status.is_canceled;
        let needs_to_run_completion_script = workflow_status
            .has_detected_need_to_run_completion_script
            .unwrap_or(false);

        if is_canceled {
            debug!("Workflow {} is canceled, returning complete=true", id);
            return Ok(IsWorkflowCompleteResponse::SuccessfulResponse(
                models::IsCompleteResponse {
                    is_complete: true,
                    is_canceled,
                    needs_to_run_completion_script,
                },
            ));
        }

        // Check if any jobs exist that are NOT in complete states
        let completed_status = models::JobStatus::Completed.to_int();
        let failed_status = models::JobStatus::Failed.to_int();
        let canceled_status = models::JobStatus::Canceled.to_int();
        let terminated_status = models::JobStatus::Terminated.to_int();
        let disabled_status = models::JobStatus::Disabled.to_int();

        let has_incomplete_jobs = match sqlx::query(
            r#"
            SELECT 1 as found
            FROM job
            WHERE workflow_id = $1
            AND status NOT IN ($2, $3, $4, $5, $6)
            LIMIT 1
            "#,
        )
        .bind(id)
        .bind(completed_status)
        .bind(failed_status)
        .bind(canceled_status)
        .bind(terminated_status)
        .bind(disabled_status)
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result.is_some(),
            Err(e) => {
                error!("Database error: {}", e);
                return Err(database_error(e));
            }
        };

        let is_complete = !has_incomplete_jobs;

        debug!(
            "Workflow {} completion status: is_complete={}, is_canceled={}, has_incomplete_jobs={}",
            id, is_complete, is_canceled, has_incomplete_jobs
        );

        Ok(IsWorkflowCompleteResponse::SuccessfulResponse(
            models::IsCompleteResponse {
                is_complete,
                is_canceled,
                needs_to_run_completion_script,
            },
        ))
    }

    /// Return true if all jobs in the workflow are uninitialized or disabled.
    async fn is_workflow_uninitialized(
        &self,
        id: i64,
        context: &C,
    ) -> Result<IsWorkflowUninitializedResponse, ApiError> {
        debug!(
            "is_workflow_uninitialized({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        // Check if any jobs exist that are NOT uninitialized or disabled
        let uninitialized_status = models::JobStatus::Uninitialized.to_int();
        let disabled_status = models::JobStatus::Disabled.to_int();

        let has_non_uninitialized_jobs = match sqlx::query(
            r#"
            SELECT 1 as found
            FROM job
            WHERE workflow_id = $1
            AND status NOT IN ($2, $3)
            LIMIT 1
            "#,
        )
        .bind(id)
        .bind(uninitialized_status)
        .bind(disabled_status)
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result.is_some(),
            Err(e) => {
                error!("Database error: {}", e);
                return Err(database_error(e));
            }
        };

        let is_uninitialized = !has_non_uninitialized_jobs;

        debug!(
            "Workflow {} uninitialized status: is_uninitialized={}, has_non_uninitialized_jobs={}",
            id, is_uninitialized, has_non_uninitialized_jobs
        );

        Ok(IsWorkflowUninitializedResponse::SuccessfulResponse(
            serde_json::json!({
                "is_uninitialized": is_uninitialized
            }),
        ))
    }

    /// Retrieve all workflows.
    async fn list_workflows(
        &self,
        offset: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        limit: i64,
        name: Option<String>,
        user: Option<String>,
        description: Option<String>,
        is_archived: Option<bool>,
        context: &C,
    ) -> Result<ListWorkflowsResponse, ApiError> {
        self.list_workflows_filtered(
            offset,
            sort_by,
            reverse_sort,
            limit,
            name,
            user,
            description,
            is_archived,
            None, // no access control filtering
            context,
        )
        .await
    }

    /// Update a workflow.
    async fn update_workflow(
        &self,
        id: i64,
        body: models::WorkflowModel,
        context: &C,
    ) -> Result<UpdateWorkflowResponse, ApiError> {
        debug!(
            "update_workflow({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First check if the workflow exists
        match self.get_workflow(id, context).await? {
            GetWorkflowResponse::SuccessfulResponse(_) => {}
            GetWorkflowResponse::ForbiddenErrorResponse(err) => {
                return Ok(UpdateWorkflowResponse::ForbiddenErrorResponse(err));
            }
            GetWorkflowResponse::NotFoundErrorResponse(err) => {
                return Ok(UpdateWorkflowResponse::NotFoundErrorResponse(err));
            }
            GetWorkflowResponse::DefaultErrorResponse(_) => {
                error!("Failed to get workflow {} before update", id);
                return Err(ApiError("Failed to get workflow".to_string()));
            }
        };

        // Convert enum to string for database storage
        let jobs_sort_method_str = body.jobs_sort_method.map(|m| m.to_string());

        // Convert boolean to integer for SQLite if provided
        let compute_node_ignore_workflow_completion_int = body
            .compute_node_ignore_workflow_completion
            .map(|val| if val { 1 } else { 0 });
        let use_pending_failed_int = body.use_pending_failed.map(|val| if val { 1 } else { 0 });

        // Update the workflow record using COALESCE to only update non-null fields
        let result = match sqlx::query!(
            r#"
            UPDATE workflow
            SET
                name = COALESCE($1, name),
                description = COALESCE($2, description),
                user = COALESCE($3, user),
                compute_node_expiration_buffer_seconds = COALESCE($4, compute_node_expiration_buffer_seconds),
                compute_node_wait_for_new_jobs_seconds = COALESCE($5, compute_node_wait_for_new_jobs_seconds),
                compute_node_ignore_workflow_completion = COALESCE($6, compute_node_ignore_workflow_completion),
                compute_node_wait_for_healthy_database_minutes = COALESCE($7, compute_node_wait_for_healthy_database_minutes),
                jobs_sort_method = COALESCE($8, jobs_sort_method),
                use_pending_failed = COALESCE($9, use_pending_failed),
                project = COALESCE($10, project),
                metadata = COALESCE($11, metadata)
            WHERE id = $12
            "#,
            body.name,
            body.description,
            body.user,
            body.compute_node_expiration_buffer_seconds,
            body.compute_node_wait_for_new_jobs_seconds,
            compute_node_ignore_workflow_completion_int,
            body.compute_node_wait_for_healthy_database_minutes,
            jobs_sort_method_str,
            use_pending_failed_int,
            body.project,
            body.metadata,
            id
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error(e));
            }
        };

        if result.rows_affected() == 0 {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", id)
            }));
            return Ok(UpdateWorkflowResponse::NotFoundErrorResponse(
                error_response,
            ));
        }

        // Return the updated workflow by fetching it again
        let updated_workflow = match self.get_workflow(id, context).await? {
            GetWorkflowResponse::SuccessfulResponse(workflow) => workflow,
            _ => {
                error!(
                    "Failed to get updated workflow after update for workflow_id={}",
                    id
                );
                return Err(ApiError("Failed to get updated workflow".to_string()));
            }
        };

        debug!("Modified workflow with id: {}", id);
        Ok(UpdateWorkflowResponse::SuccessfulResponse(updated_workflow))
    }

    /// Update the workflow status.
    async fn update_workflow_status(
        &self,
        id: i64,
        body: models::WorkflowStatusModel,
        context: &C,
    ) -> Result<UpdateWorkflowStatusResponse, ApiError> {
        debug!(
            "update_workflow_status({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First check if the workflow status exists
        match self.get_workflow_status(id, context).await? {
            GetWorkflowStatusResponse::SuccessfulResponse(_) => {}
            GetWorkflowStatusResponse::ForbiddenErrorResponse(err) => {
                return Ok(UpdateWorkflowStatusResponse::ForbiddenErrorResponse(err));
            }
            GetWorkflowStatusResponse::NotFoundErrorResponse(err) => {
                return Ok(UpdateWorkflowStatusResponse::NotFoundErrorResponse(err));
            }
            GetWorkflowStatusResponse::DefaultErrorResponse(_) => {
                error!(
                    "Failed to get workflow status before update for workflow_id={}",
                    id
                );
                return Err(ApiError("Failed to get workflow status".to_string()));
            }
        };

        // Convert boolean values to integers for SQLite storage
        let is_canceled_int = if body.is_canceled { 1 } else { 0 };
        let is_archived_int = if body.is_archived.unwrap_or(false) {
            1
        } else {
            0
        };
        let has_detected_need_to_run_completion_script_int = if body
            .has_detected_need_to_run_completion_script
            .unwrap_or(false)
        {
            1
        } else {
            0
        };

        debug!("Sending db workflow status update for ID: {}", id);
        // Update the workflow status
        let result = match sqlx::query!(
            r#"
            UPDATE workflow_status 
            SET run_id = ?, 
                has_detected_need_to_run_completion_script = ?, 
                is_canceled = ?, 
                is_archived = ?
            WHERE id = ?
            "#,
            body.run_id,
            has_detected_need_to_run_completion_script_int,
            is_canceled_int,
            is_archived_int,
            id
        )
        .execute(&*self.context.pool)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to update workflow status for ID: {}: {:?}", id, e);
                return Err(database_error(e));
            }
        };

        if result.rows_affected() == 0 {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow status not found with ID: {}", id)
            }));
            return Ok(UpdateWorkflowStatusResponse::NotFoundErrorResponse(
                error_response,
            ));
        }

        debug!("Updated workflow status for ID: {}", id);

        // Return the updated workflow status
        let updated_status = models::WorkflowStatusModel {
            id: Some(id),
            is_canceled: body.is_canceled,
            is_archived: body.is_archived,
            run_id: body.run_id,
            has_detected_need_to_run_completion_script: body
                .has_detected_need_to_run_completion_script,
        };

        debug!(
            "Returning updated workflow status for ID: {}: {:?}",
            id, updated_status
        );
        Ok(UpdateWorkflowStatusResponse::SuccessfulResponse(
            updated_status,
        ))
    }

    /// Delete a workflow.
    async fn delete_workflow(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteWorkflowResponse, ApiError> {
        debug!(
            "delete_workflow({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First get the workflow to ensure it exists and extract the WorkflowModel
        let workflow = match self.get_workflow(id, context).await? {
            GetWorkflowResponse::SuccessfulResponse(workflow) => workflow,
            GetWorkflowResponse::ForbiddenErrorResponse(err) => {
                return Ok(DeleteWorkflowResponse::ForbiddenErrorResponse(err));
            }
            GetWorkflowResponse::NotFoundErrorResponse(err) => {
                return Ok(DeleteWorkflowResponse::NotFoundErrorResponse(err));
            }
            GetWorkflowResponse::DefaultErrorResponse(_) => {
                error!("Failed to get workflow {} before deletion", id);
                return Err(ApiError("Failed to get workflow".to_string()));
            }
        };

        match sqlx::query!(r#"DELETE FROM workflow WHERE id = $1"#, id)
            .execute(self.context.pool.as_ref())
            .await
        {
            Ok(res) => {
                if res.rows_affected() > 1 {
                    error!(
                        "Unexpected number of rows affected when deleting workflow {}: {}",
                        id,
                        res.rows_affected()
                    );
                    Err(ApiError(format!(
                        "Database error: Unexpected number of rows affected: {}",
                        res.rows_affected()
                    )))
                } else {
                    info!(
                        "Successfully deleted workflow {} (name: {:?})",
                        id, workflow.name
                    );
                    Ok(DeleteWorkflowResponse::SuccessfulResponse(workflow))
                }
            }
            Err(e) => {
                error!("Database error when deleting workflow {}: {}", id, e);
                Err(ApiError(format!("Database error: {}", e)))
            }
        }
    }

    /// Reset workflow status.
    async fn reset_workflow_status(
        &self,
        id: i64,
        force: Option<bool>,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<ResetWorkflowStatusResponse, ApiError> {
        debug!(
            "reset_workflow_status({}, force={:?}, {:?}) - X-Span-ID: {:?}",
            id,
            force,
            body,
            context.get().0.clone()
        );

        // Use force flag from query parameter (defaults to false)
        let force = force.unwrap_or(false);

        let workflow_status = match self.get_workflow_status(id, context).await? {
            GetWorkflowStatusResponse::SuccessfulResponse(status) => status,
            GetWorkflowStatusResponse::ForbiddenErrorResponse(err) => {
                // Return the forbidden error as a default error since this endpoint
                // doesn't have a ForbiddenErrorResponse variant in the OpenAPI spec
                return Ok(ResetWorkflowStatusResponse::DefaultErrorResponse(err));
            }
            GetWorkflowStatusResponse::NotFoundErrorResponse(err) => {
                return Ok(ResetWorkflowStatusResponse::NotFoundErrorResponse(err));
            }
            GetWorkflowStatusResponse::DefaultErrorResponse(_) => {
                error!(
                    "Failed to get workflow status before reset for workflow_id={}",
                    id
                );
                return Err(ApiError("Failed to get workflow status".to_string()));
            }
        };

        // Check if workflow is archived
        if workflow_status.is_archived.unwrap_or(false) {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": "Cannot reset archived workflow status. Unarchive the workflow first."
            }));
            return Ok(
                ResetWorkflowStatusResponse::UnprocessableContentErrorResponse(error_response),
            );
        }

        // Check if any jobs are in running or SubmittedPending status
        let submitted_status = models::JobStatus::Running.to_int();
        let submitted_pending_status = models::JobStatus::Pending.to_int();

        let has_active_jobs = match sqlx::query_scalar::<_, i64>(
            "SELECT id FROM job WHERE workflow_id = ? AND (status = ? OR status = ?) LIMIT 1",
        )
        .bind(id)
        .bind(submitted_status)
        .bind(submitted_pending_status)
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result.is_some(),
            Err(e) => {
                return Err(database_error(e));
            }
        };

        if has_active_jobs {
            if force {
                info!(
                    "Force flag set: ignoring active jobs check for workflow {} reset",
                    id
                );
            } else {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": "Cannot reset workflow status: jobs are currently running or pending submission"
                }));
                return Ok(
                    ResetWorkflowStatusResponse::UnprocessableContentErrorResponse(error_response),
                );
            }
        }

        // Check if any scheduled compute nodes are in pending or active status
        let has_active_scheduled_nodes = match sqlx::query_scalar::<_, i64>(
            "SELECT id FROM scheduled_compute_node WHERE workflow_id = ? AND (status = 'pending' OR status = 'active') LIMIT 1",
        )
        .bind(id)
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result.is_some(),
            Err(e) => {
                return Err(database_error(e));
            }
        };

        if has_active_scheduled_nodes {
            if force {
                info!(
                    "Force flag set: ignoring active scheduled compute nodes check for workflow {} reset",
                    id
                );
            } else {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": "Cannot reset workflow status: scheduled compute nodes are currently pending or active"
                }));
                return Ok(
                    ResetWorkflowStatusResponse::UnprocessableContentErrorResponse(error_response),
                );
            }
        }

        // Begin a transaction to ensure workflow status reset is atomic
        let mut tx = match self.context.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                return Err(database_error(e));
            }
        };

        // Reset workflow status to default values:
        // is_canceled = false (0), is_archived = false (0), run_id = 0,
        // has_detected_need_to_run_completion_script = false (0)
        match sqlx::query!(
            r#"
            UPDATE workflow_status
            SET has_detected_need_to_run_completion_script = 0,
                is_canceled = 0,
                is_archived = 0
            WHERE id = ?
            "#,
            id
        )
        .execute(&mut *tx)
        .await
        {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    let _ = tx.rollback().await;
                    error!("No workflow status updated for workflow_id={}", id);
                    return Err(ApiError(format!(
                        "No workflow status updated for ID: {}",
                        id
                    )));
                }
                debug!("Reset workflow status for ID: {}", id);
            }
            Err(e) => {
                let _ = tx.rollback().await;
                debug!("Failed to reset workflow status for ID: {}: {:?}", id, e);
                return Err(database_error(e));
            }
        }

        // Commit the transaction
        if let Err(e) = tx.commit().await {
            return Err(database_error(e));
        }

        // Return success response with the reset values
        info!("Successfully reset workflow status for ID: {}", id);
        Ok(ResetWorkflowStatusResponse::SuccessfulResponse(
            serde_json::json!({
                "id": id,
                "run_id": workflow_status.run_id,
                "is_canceled": false,
                "is_archived": false,
                "has_detected_need_to_run_completion_script": false
            }),
        ))
    }

    /// Retrieve job blocking relationships for a workflow.
    async fn list_job_dependencies(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListJobDependenciesResponse, ApiError> {
        debug!(
            "list_job_dependencies({}, {:?}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            offset,
            limit,
            context.get().0.clone()
        );

        let offset_val = offset.unwrap_or(0);
        let limit_val = limit.unwrap_or(10000).min(10000);

        // Query job_depends_on table with JOIN to get job names
        let dependencies = match sqlx::query_as!(
            models::JobDependencyModel,
            r#"
            SELECT
                jb.job_id as job_id,
                j1.name as job_name,
                jb.depends_on_job_id as depends_on_job_id,
                j2.name as depends_on_job_name,
                jb.workflow_id as workflow_id
            FROM job_depends_on jb
            INNER JOIN job j1 ON jb.job_id = j1.id
            INNER JOIN job j2 ON jb.depends_on_job_id = j2.id
            WHERE jb.workflow_id = ?
            LIMIT ? OFFSET ?
            "#,
            workflow_id,
            limit_val,
            offset_val
        )
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(deps) => deps,
            Err(e) => {
                return Err(database_error(e));
            }
        };

        // Get total count
        let total_count = match sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM job_depends_on WHERE workflow_id = ?",
        )
        .bind(workflow_id)
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(count) => count,
            Err(e) => {
                return Err(database_error(e));
            }
        };

        let current_count = dependencies.len() as i64;
        let has_more = offset_val + current_count < total_count;

        debug!(
            "list_job_dependencies({}) - returning {}/{} dependencies - X-Span-ID: {:?}",
            workflow_id,
            current_count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListJobDependenciesResponse::SuccessfulResponse(
            models::ListJobDependenciesResponse {
                items: Some(dependencies),
                offset: offset_val,
                max_limit: 10000,
                count: current_count,
                total_count,
                has_more,
            },
        ))
    }

    /// Retrieve job-file relationships for a workflow.
    async fn list_job_file_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListJobFileRelationshipsResponse, ApiError> {
        debug!(
            "list_job_file_relationships({}, {:?}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            offset,
            limit,
            context.get().0.clone()
        );

        let offset_val = offset.unwrap_or(0);
        let limit_val = limit.unwrap_or(10000).min(10000);

        // Query job_input_file and job_output_file tables with JOINs
        // UNION the input and output relationships
        let relationships = match sqlx::query_as!(
            models::JobFileRelationshipModel,
            r#"
            SELECT
                f.id as file_id,
                f.name as file_name,
                f.path as file_path,
                jof.job_id as "producer_job_id?",
                producer.name as "producer_job_name?",
                jif.job_id as "consumer_job_id?",
                consumer.name as "consumer_job_name?",
                f.workflow_id as workflow_id
            FROM file f
            LEFT JOIN job_output_file jof ON f.id = jof.file_id
            LEFT JOIN job producer ON jof.job_id = producer.id
            LEFT JOIN job_input_file jif ON f.id = jif.file_id
            LEFT JOIN job consumer ON jif.job_id = consumer.id
            WHERE f.workflow_id = ?
                AND (jof.job_id IS NOT NULL OR jif.job_id IS NOT NULL)
            LIMIT ? OFFSET ?
            "#,
            workflow_id,
            limit_val,
            offset_val
        )
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(rels) => rels,
            Err(e) => {
                return Err(database_error(e));
            }
        };

        // Get total count
        let total_count = match sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM (
                SELECT f.id, jof.job_id as producer, jif.job_id as consumer
                FROM file f
                LEFT JOIN job_output_file jof ON f.id = jof.file_id
                LEFT JOIN job_input_file jif ON f.id = jif.file_id
                WHERE f.workflow_id = ?
                    AND (jof.job_id IS NOT NULL OR jif.job_id IS NOT NULL)
            )
            "#,
        )
        .bind(workflow_id)
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(count) => count,
            Err(e) => {
                return Err(database_error(e));
            }
        };

        let current_count = relationships.len() as i64;
        let has_more = offset_val + current_count < total_count;

        debug!(
            "list_job_file_relationships({}) - returning {}/{} relationships - X-Span-ID: {:?}",
            workflow_id,
            current_count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListJobFileRelationshipsResponse::SuccessfulResponse(
            models::ListJobFileRelationshipsResponse {
                items: Some(relationships),
                offset: offset_val,
                max_limit: 10000,
                count: current_count,
                total_count,
                has_more,
            },
        ))
    }

    /// Retrieve job-user_data relationships for a workflow.
    async fn list_job_user_data_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListJobUserDataRelationshipsResponse, ApiError> {
        debug!(
            "list_job_user_data_relationships({}, {:?}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            offset,
            limit,
            context.get().0.clone()
        );

        let offset_val = offset.unwrap_or(0);
        let limit_val = limit.unwrap_or(10000).min(10000);

        // Query job_input_user_data and job_output_user_data tables with JOINs
        let relationships = match sqlx::query_as!(
            models::JobUserDataRelationshipModel,
            r#"
            SELECT
                ud.id as user_data_id,
                ud.name as user_data_name,
                joud.job_id as "producer_job_id?",
                producer.name as "producer_job_name?",
                jiud.job_id as "consumer_job_id?",
                consumer.name as "consumer_job_name?",
                ud.workflow_id as workflow_id
            FROM user_data ud
            LEFT JOIN job_output_user_data joud ON ud.id = joud.user_data_id
            LEFT JOIN job producer ON joud.job_id = producer.id
            LEFT JOIN job_input_user_data jiud ON ud.id = jiud.user_data_id
            LEFT JOIN job consumer ON jiud.job_id = consumer.id
            WHERE ud.workflow_id = ?
                AND (joud.job_id IS NOT NULL OR jiud.job_id IS NOT NULL)
            LIMIT ? OFFSET ?
            "#,
            workflow_id,
            limit_val,
            offset_val
        )
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(rels) => rels,
            Err(e) => {
                return Err(database_error(e));
            }
        };

        // Get total count
        let total_count = match sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM (
                SELECT ud.id, joud.job_id as producer, jiud.job_id as consumer
                FROM user_data ud
                LEFT JOIN job_output_user_data joud ON ud.id = joud.user_data_id
                LEFT JOIN job_input_user_data jiud ON ud.id = jiud.user_data_id
                WHERE ud.workflow_id = ?
                    AND (joud.job_id IS NOT NULL OR jiud.job_id IS NOT NULL)
            )
            "#,
        )
        .bind(workflow_id)
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(count) => count,
            Err(e) => {
                return Err(database_error(e));
            }
        };

        let current_count = relationships.len() as i64;
        let has_more = offset_val + current_count < total_count;

        debug!(
            "list_job_user_data_relationships({}) - returning {}/{} relationships - X-Span-ID: {:?}",
            workflow_id,
            current_count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListJobUserDataRelationshipsResponse::SuccessfulResponse(
            models::ListJobUserDataRelationshipsResponse {
                items: Some(relationships),
                offset: offset_val,
                max_limit: 10000,
                count: current_count,
                total_count,
                has_more,
            },
        ))
    }
}
