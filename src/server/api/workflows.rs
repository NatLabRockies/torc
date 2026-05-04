//! Workflow-related API endpoints

#![allow(clippy::too_many_arguments)]

use crate::server::transport_types::context_types::{ApiError, Has, XSpanIdString};
use async_trait::async_trait;
use chrono::Utc;
use log::{debug, error, info};
use sqlx::Row;

use crate::server::api_responses::{
    ArchiveWorkflowResponse, CancelWorkflowResponse, CreateWorkflowResponse,
    DeleteWorkflowResponse, GetWorkflowResponse, IsWorkflowCompleteResponse,
    IsWorkflowUninitializedResponse, ListJobDependenciesResponse, ListJobFileRelationshipsResponse,
    ListJobUserDataRelationshipsResponse, ListWorkflowsResponse, ResetWorkflowStatusResponse,
    UpdateWorkflowResponse,
};

use crate::models;

use super::{
    ApiContext, MAX_RECORD_TRANSFER_COUNT, SqlQueryBuilder, database_error_with_msg,
    deserialize_env_map, escape_like_pattern, normalize_env_map, serialize_env_map,
    validate_env_map,
};

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
        context: &C,
    ) -> Result<CancelWorkflowResponse, ApiError>;

    /// Archive or unarchive a workflow.
    async fn archive_workflow(
        &self,
        id: i64,
        body: models::ArchiveWorkflowRequest,
        context: &C,
    ) -> Result<ArchiveWorkflowResponse, ApiError>;

    /// Retrieve a workflow.
    async fn get_workflow(&self, id: i64, context: &C) -> Result<GetWorkflowResponse, ApiError>;

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
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListJobDependenciesResponse, ApiError>;

    /// Retrieve job-file relationships for a workflow.
    async fn list_job_file_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListJobFileRelationshipsResponse, ApiError>;

    /// Retrieve job-user_data relationships for a workflow.
    async fn list_job_user_data_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListJobUserDataRelationshipsResponse, ApiError>;

    /// Update a workflow.
    async fn update_workflow(
        &self,
        id: i64,
        body: models::WorkflowModel,
        context: &C,
    ) -> Result<UpdateWorkflowResponse, ApiError>;

    /// Delete a workflow.
    async fn delete_workflow(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteWorkflowResponse, ApiError>;

    /// Reset workflow status.
    async fn reset_workflow_status(
        &self,
        id: i64,
        force: Option<bool>,
        context: &C,
    ) -> Result<ResetWorkflowStatusResponse, ApiError>;
}

/// Implementation of workflows API for the server
#[derive(Clone)]
pub struct WorkflowsApiImpl {
    pub context: ApiContext,
}

const WORKFLOW_COLUMNS: &[&str] = &[
    "id",
    "name",
    "user",
    "description",
    "env",
    "timestamp",
    "compute_node_expiration_buffer_seconds",
    "compute_node_wait_for_new_jobs_seconds",
    "compute_node_ignore_workflow_completion",
    "compute_node_wait_for_healthy_database_minutes",
    "compute_node_min_time_for_new_jobs_seconds",
    "resource_monitor_config",
    "slurm_defaults",
    "use_pending_failed",
    "enable_ro_crate",
    "project",
    "metadata",
    "run_id",
    "is_archived",
    "is_canceled",
];

const JOB_DEPENDENCY_COLUMNS: &[&str] = &[
    "job_id",
    "job_name",
    "depends_on_job_id",
    "depends_on_job_name",
    "workflow_id",
];

const JOB_FILE_RELATIONSHIP_COLUMNS: &[&str] = &[
    "file_id",
    "file_name",
    "file_path",
    "producer_job_id",
    "producer_job_name",
    "consumer_job_id",
    "consumer_job_name",
    "workflow_id",
];

const JOB_USER_DATA_RELATIONSHIP_COLUMNS: &[&str] = &[
    "user_data_id",
    "user_data_name",
    "producer_job_id",
    "producer_job_name",
    "consumer_job_id",
    "consumer_job_name",
    "workflow_id",
];

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

        // Validate sort_by against whitelist
        let validated_sort_by = if let Some(ref col) = sort_by {
            if WORKFLOW_COLUMNS.contains(&col.as_str()) {
                Some(col.clone())
            } else {
                debug!("Invalid sort column requested: {}", col);
                None // Fall back to default
            }
        } else {
            None
        };

        let base_query = "
            SELECT
                id
                ,name
                ,user
                ,description
                ,env
                ,timestamp
                ,compute_node_expiration_buffer_seconds
                ,compute_node_wait_for_new_jobs_seconds
                ,compute_node_ignore_workflow_completion
                ,compute_node_wait_for_healthy_database_minutes
                ,compute_node_min_time_for_new_jobs_seconds
                ,resource_monitor_config
                ,slurm_defaults
                ,use_pending_failed
                ,enable_ro_crate
                ,project
                ,metadata
                ,slurm_config
                ,execution_config
                ,run_id
                ,is_archived
                ,is_canceled
            FROM workflow
            "
        .to_string();

        // Build WHERE clause conditions
        let mut where_conditions = Vec::new();

        if name.is_some() {
            where_conditions.push("name = ?".to_string());
        }

        if user.is_some() {
            where_conditions.push("user = ?".to_string());
        }

        if description.is_some() {
            where_conditions.push("description LIKE ? ESCAPE '\\'".to_string());
        }

        if let Some(archived) = is_archived {
            if archived {
                where_conditions.push("is_archived = 1".to_string());
            } else {
                where_conditions.push("is_archived = 0".to_string());
            }
        }

        // Access control filtering: restrict to accessible workflow IDs
        if let Some(ref ids) = accessible_ids {
            if ids.is_empty() {
                // No accessible workflows - return empty result
                return Ok(ListWorkflowsResponse::SuccessfulResponse(
                    models::ListWorkflowsResponse {
                        items: Vec::new(),
                        offset,
                        max_limit: MAX_RECORD_TRANSFER_COUNT,
                        count: 0,
                        total_count: 0,
                        has_more: false,
                    },
                ));
            }
            let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
            where_conditions.push(format!("id IN ({})", placeholders.join(", ")));
        }

        let where_clause = if where_conditions.is_empty() {
            String::new()
        } else {
            where_conditions.join(" AND ")
        };

        let query = if where_clause.is_empty() {
            SqlQueryBuilder::new(base_query)
                .with_pagination_and_sorting(
                    offset,
                    limit,
                    validated_sort_by,
                    reverse_sort,
                    "id",
                    WORKFLOW_COLUMNS,
                )
                .build()
        } else {
            SqlQueryBuilder::new(base_query)
                .with_where(where_clause.clone())
                .with_pagination_and_sorting(
                    offset,
                    limit,
                    validated_sort_by,
                    reverse_sort,
                    "id",
                    WORKFLOW_COLUMNS,
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
            sqlx_query =
                sqlx_query.bind(format!("%{}%", escape_like_pattern(workflow_description)));
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
                return Err(database_error_with_msg(e, "Failed to list workflows"));
            }
        };

        let mut items: Vec<models::WorkflowModel> = Vec::new();
        for record in records {
            items.push(models::WorkflowModel {
                id: Some(record.get("id")),
                name: record.get("name"),
                user: record.get("user"),
                description: record.get("description"),
                env: deserialize_env_map(record.get("env"), "workflow env")?,
                timestamp: Some(record.get("timestamp")),
                compute_node_expiration_buffer_seconds: record
                    .try_get::<Option<i64>, _>("compute_node_expiration_buffer_seconds")
                    .unwrap_or(None),
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
                resource_monitor_config: record.get("resource_monitor_config"),
                slurm_defaults: record.get("slurm_defaults"),
                use_pending_failed: record
                    .try_get::<Option<i64>, _>("use_pending_failed")
                    .ok()
                    .flatten()
                    .map(|v| v != 0),
                enable_ro_crate: record
                    .try_get::<Option<i64>, _>("enable_ro_crate")
                    .ok()
                    .flatten()
                    .map(|v| v != 0),
                project: record.get("project"),
                metadata: record.get("metadata"),
                slurm_config: record
                    .try_get::<Option<String>, _>("slurm_config")
                    .ok()
                    .flatten(),
                execution_config: record
                    .try_get::<Option<String>, _>("execution_config")
                    .ok()
                    .flatten(),
                run_id: Some(record.get("run_id")),
                is_canceled: Some(record.get::<i64, _>("is_canceled") != 0),
                is_archived: Some(record.get::<i64, _>("is_archived") != 0),
            });
        }

        // For proper pagination, we should get the total count without LIMIT/OFFSET
        let count_base_query = "SELECT COUNT(*) as total FROM workflow";
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
            count_sqlx_query =
                count_sqlx_query.bind(format!("%{}%", escape_like_pattern(workflow_description)));
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
                return Err(database_error_with_msg(e, "Failed to list workflows"));
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
                items,
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
            .map_err(|e| database_error_with_msg(e, "Failed to check if workflow exists"))?;

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
        info!("create_workflow - X-Span-ID: {:?}", context.get().0.clone());

        body.timestamp = Some(Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string());
        if let Err(err) = validate_env_map(body.env.as_ref(), "workflow env") {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": err.0
            }));
            return Ok(CreateWorkflowResponse::UnprocessableContentErrorResponse(
                error_response,
            ));
        }
        body.env = normalize_env_map(body.env.take());
        let workflow_env = serialize_env_map(body.env.clone(), "workflow env")?;
        let compute_node_expiration_buffer_seconds = body.compute_node_expiration_buffer_seconds;
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

        let use_pending_failed_int = body.use_pending_failed.map(|v| if v { 1 } else { 0 });
        let enable_ro_crate_int = body.enable_ro_crate.map(|v| if v { 1 } else { 0 });

        let workflow_result = sqlx::query!(
            r#"
            INSERT INTO workflow
            (
                name,
                description,
                user,
                env,
                timestamp,
                compute_node_expiration_buffer_seconds,
                compute_node_wait_for_new_jobs_seconds,
                compute_node_ignore_workflow_completion,
                compute_node_wait_for_healthy_database_minutes,
                compute_node_min_time_for_new_jobs_seconds,
                resource_monitor_config,
                slurm_defaults,
                use_pending_failed,
                enable_ro_crate,
                project,
                metadata,
                slurm_config,
                execution_config,
                run_id,
                is_archived,
                is_canceled
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, 0, 0, 0)
            RETURNING rowid
            "#,
            body.name,
            body.description,
            body.user,
            workflow_env,
            body.timestamp,
            compute_node_expiration_buffer_seconds,
            compute_node_wait_for_new_jobs_seconds,
            compute_node_ignore_workflow_completion,
            compute_node_wait_for_healthy_database_minutes,
            compute_node_min_time_for_new_jobs_seconds,
            body.resource_monitor_config,
            body.slurm_defaults,
            use_pending_failed_int,
            enable_ro_crate_int,
            body.project,
            body.metadata,
            body.slurm_config,
            body.execution_config,
        )
        .fetch_one(self.context.pool.as_ref())
        .await
        .map_err(|e| database_error_with_msg(e, "Failed to create workflow record"))?;

        let workflow_id = workflow_result.id;
        debug!("Workflow inserted with id: {:?}", workflow_id);
        body.id = Some(workflow_id);
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
        context: &C,
    ) -> Result<CancelWorkflowResponse, ApiError> {
        info!(
            "cancel_workflow({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        // First get the current workflow to preserve other fields and capture run_id
        // for the cancellation event.
        let current_workflow = match self.get_workflow(id, context).await? {
            GetWorkflowResponse::SuccessfulResponse(wf) => wf,
            GetWorkflowResponse::ForbiddenErrorResponse(err) => {
                return Ok(CancelWorkflowResponse::DefaultErrorResponse(err));
            }
            GetWorkflowResponse::NotFoundErrorResponse(err) => {
                return Ok(CancelWorkflowResponse::DefaultErrorResponse(err));
            }
            GetWorkflowResponse::DefaultErrorResponse(_) => {
                error!("Failed to get current workflow for workflow_id={}", id);
                return Err(ApiError("Failed to get current workflow".to_string()));
            }
        };
        let current_run_id = current_workflow.run_id.unwrap_or(0);

        // Begin a transaction to ensure workflow cancellation and job cancellation are atomic
        let mut tx = match self.context.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to begin transaction"));
            }
        };

        let result = match sqlx::query!(
            r#"
            UPDATE workflow
            SET is_canceled = 1
            WHERE id = ?
            "#,
            id
        )
        .execute(&mut *tx)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(database_error_with_msg(
                    e,
                    "Failed to update workflow status",
                ));
            }
        };

        if result.rows_affected() == 0 {
            let _ = tx.rollback().await;
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", id)
            }));
            return Ok(CancelWorkflowResponse::DefaultErrorResponse(error_response));
        }

        info!("Successfully canceled workflow with ID: {}", id);

        // Create an event to record the workflow cancellation
        let timestamp = Utc::now().timestamp_millis();
        let event_data = serde_json::json!({
            "category": "workflow_canceled",
            "message": format!("Workflow {} (run_id={}) was canceled", id, current_run_id),
            "workflow_id": id,
            "run_id": current_run_id,
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
                return Err(database_error_with_msg(
                    e,
                    "Failed to create workflow cancellation event",
                ));
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
                return Err(database_error_with_msg(
                    e,
                    "Failed to cancel associated jobs",
                ));
            }
        }

        // Commit the transaction
        if let Err(e) = tx.commit().await {
            return Err(database_error_with_msg(e, "Failed to commit transaction"));
        }

        // Re-fetch so the response reflects post-cancel state of the workflow row.
        let updated = match self.get_workflow(id, context).await? {
            GetWorkflowResponse::SuccessfulResponse(wf) => wf,
            _ => {
                error!("Failed to get workflow {} after cancel", id);
                return Err(ApiError("Failed to get workflow after cancel".to_string()));
            }
        };
        let response_json = serde_json::to_value(updated)
            .map_err(|e| ApiError(format!("Failed to serialize workflow: {e}")))?;
        Ok(CancelWorkflowResponse::SuccessfulResponse(response_json))
    }

    /// Archive or unarchive a workflow.
    async fn archive_workflow(
        &self,
        id: i64,
        body: models::ArchiveWorkflowRequest,
        context: &C,
    ) -> Result<ArchiveWorkflowResponse, ApiError> {
        debug!(
            "archive_workflow({}, is_archived={}) - X-Span-ID: {:?}",
            id,
            body.is_archived,
            context.get().0.clone()
        );

        let is_archived_int = if body.is_archived { 1 } else { 0 };
        let result = sqlx::query!(
            "UPDATE workflow SET is_archived = ? WHERE id = ?",
            is_archived_int,
            id
        )
        .execute(self.context.pool.as_ref())
        .await
        .map_err(|e| database_error_with_msg(e, "Failed to update workflow archive flag"))?;

        if result.rows_affected() == 0 {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", id)
            }));
            return Ok(ArchiveWorkflowResponse::NotFoundErrorResponse(
                error_response,
            ));
        }

        let updated = match self.get_workflow(id, context).await? {
            GetWorkflowResponse::SuccessfulResponse(wf) => wf,
            _ => {
                error!("Failed to get workflow {} after archive update", id);
                return Err(ApiError(
                    "Failed to get workflow after archive update".to_string(),
                ));
            }
        };
        Ok(ArchiveWorkflowResponse::SuccessfulResponse(updated))
    }

    /// Retrieve a workflow.
    async fn get_workflow(&self, id: i64, context: &C) -> Result<GetWorkflowResponse, ApiError> {
        debug!(
            "get_workflow({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );
        match sqlx::query(
            r#"
                SELECT
                    id,
                    name,
                    user,
                    description,
                    env,
                    timestamp,
                    compute_node_expiration_buffer_seconds,
                    compute_node_wait_for_new_jobs_seconds,
                    compute_node_ignore_workflow_completion,
                    compute_node_wait_for_healthy_database_minutes,
                    compute_node_min_time_for_new_jobs_seconds,
                    resource_monitor_config,
                    slurm_defaults,
                    use_pending_failed,
                    enable_ro_crate,
                    project,
                    metadata,
                    slurm_config,
                    execution_config,
                    run_id,
                    is_canceled,
                    is_archived
                FROM workflow
                WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(row)) => Ok(GetWorkflowResponse::SuccessfulResponse(
                models::WorkflowModel {
                    id: Some(row.get("id")),
                    name: row.get("name"),
                    user: row.get("user"),
                    description: row.get("description"),
                    env: deserialize_env_map(row.get("env"), "workflow env")?,
                    timestamp: Some(row.get("timestamp")),
                    compute_node_expiration_buffer_seconds: row
                        .try_get::<Option<i64>, _>("compute_node_expiration_buffer_seconds")
                        .unwrap_or(None),
                    compute_node_wait_for_new_jobs_seconds: Some(
                        row.get("compute_node_wait_for_new_jobs_seconds"),
                    ),
                    compute_node_ignore_workflow_completion: Some(
                        row.get::<i64, _>("compute_node_ignore_workflow_completion") != 0,
                    ),
                    compute_node_wait_for_healthy_database_minutes: Some(
                        row.get("compute_node_wait_for_healthy_database_minutes"),
                    ),
                    compute_node_min_time_for_new_jobs_seconds: Some(
                        row.get("compute_node_min_time_for_new_jobs_seconds"),
                    ),
                    resource_monitor_config: row.get("resource_monitor_config"),
                    slurm_defaults: row.get("slurm_defaults"),
                    use_pending_failed: row
                        .try_get::<Option<i64>, _>("use_pending_failed")
                        .ok()
                        .flatten()
                        .map(|v| v != 0),
                    enable_ro_crate: row
                        .try_get::<Option<i64>, _>("enable_ro_crate")
                        .ok()
                        .flatten()
                        .map(|v| v != 0),
                    project: row.get("project"),
                    metadata: row.get("metadata"),
                    slurm_config: row
                        .try_get::<Option<String>, _>("slurm_config")
                        .ok()
                        .flatten(),
                    execution_config: row
                        .try_get::<Option<String>, _>("execution_config")
                        .ok()
                        .flatten(),
                    run_id: Some(row.get("run_id")),
                    is_canceled: Some(row.get::<i64, _>("is_canceled") != 0),
                    is_archived: Some(row.get::<i64, _>("is_archived") != 0),
                },
            )),
            Ok(None) => {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Workflow not found with ID: {}", id)
                }));
                Ok(GetWorkflowResponse::NotFoundErrorResponse(error_response))
            }
            Err(e) => Err(database_error_with_msg(e, "Failed to get workflow")),
        }
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

        // Read the cancellation flag directly from the workflow row.
        let is_canceled: bool =
            match sqlx::query_scalar::<_, i64>("SELECT is_canceled FROM workflow WHERE id = ?")
                .bind(id)
                .fetch_optional(self.context.pool.as_ref())
                .await
            {
                Ok(Some(v)) => v != 0,
                Ok(None) => {
                    let error_response = models::ErrorResponse::new(serde_json::json!({
                        "message": format!("Workflow not found with ID: {}", id)
                    }));
                    return Ok(IsWorkflowCompleteResponse::NotFoundErrorResponse(
                        error_response,
                    ));
                }
                Err(e) => {
                    return Err(database_error_with_msg(e, "Failed to get workflow"));
                }
            };

        if is_canceled {
            debug!("Workflow {} is canceled, returning complete=true", id);
            return Ok(IsWorkflowCompleteResponse::SuccessfulResponse(
                models::IsCompleteResponse {
                    is_complete: true,
                    is_canceled,
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
                return Err(database_error_with_msg(
                    e,
                    "Failed to check workflow completion",
                ));
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
                return Err(database_error_with_msg(
                    e,
                    "Failed to check if workflow is uninitialized",
                ));
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
            "update_workflow({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        // First check if the workflow exists
        let current_workflow = match self.get_workflow(id, context).await? {
            GetWorkflowResponse::SuccessfulResponse(workflow) => workflow,
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

        // Convert boolean to integer for SQLite if provided.
        // run_id, is_canceled, is_archived are read-only on this endpoint —
        // mutate them via /workflows/{id}/cancel, /archive, and
        // /reset_status (which also bumps run_id) so their dedicated side
        // effects fire.
        let compute_node_ignore_workflow_completion_int = body
            .compute_node_ignore_workflow_completion
            .map(|val| if val { 1 } else { 0 });
        let use_pending_failed_int = body.use_pending_failed.map(|val| if val { 1 } else { 0 });
        let enable_ro_crate_int = body.enable_ro_crate.map(|val| if val { 1 } else { 0 });
        let body_env = normalize_env_map(body.env.clone());
        if body.env.is_some() && body_env != current_workflow.env {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": "Cannot modify env - this field is immutable after workflow creation"
            }));
            return Ok(UpdateWorkflowResponse::UnprocessableContentErrorResponse(
                error_response,
            ));
        }

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
                use_pending_failed = COALESCE($8, use_pending_failed),
                enable_ro_crate = COALESCE($9, enable_ro_crate),
                project = COALESCE($10, project),
                metadata = COALESCE($11, metadata),
                slurm_config = COALESCE($12, slurm_config),
                execution_config = COALESCE($13, execution_config)
            WHERE id = $14
            "#,
            body.name,
            body.description,
            body.user,
            body.compute_node_expiration_buffer_seconds,
            body.compute_node_wait_for_new_jobs_seconds,
            compute_node_ignore_workflow_completion_int,
            body.compute_node_wait_for_healthy_database_minutes,
            use_pending_failed_int,
            enable_ro_crate_int,
            body.project,
            body.metadata,
            body.slurm_config,
            body.execution_config,
            id
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to update workflow"));
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

    /// Delete a workflow.
    async fn delete_workflow(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteWorkflowResponse, ApiError> {
        debug!(
            "delete_workflow({}) - X-Span-ID: {:?}",
            id,
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

        // Explicitly delete from child tables instead of relying on CASCADE.
        // With PRAGMA foreign_keys = ON (our default), SQLite checks FK constraints
        // on every deleted row by scanning all referencing tables. For a 100K-job
        // workflow this causes ~46s deletes. Since we delete children first, we can
        // safely disable FK checks for the delete transaction. Benchmarked: 46s → <1s.
        //
        // PRAGMA foreign_keys is a no-op inside a transaction, so we must set it
        // on the connection before BEGIN and restore it after COMMIT.
        let pool = self.context.pool.as_ref();

        let mut conn = pool
            .acquire()
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to acquire connection"))?;

        // Disable FK checks — safe because we explicitly delete all child rows
        // before parents. Must be set outside a transaction.
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *conn)
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to disable foreign keys"))?;

        // Wrap all deletes in a transaction for atomicity. If anything fails,
        // the transaction rolls back and we re-enable FK checks in the cleanup below.
        let result = async {
            sqlx::query("BEGIN IMMEDIATE")
                .execute(&mut *conn)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to begin transaction"))?;

            // Tier 1: Leaf junction tables (no other tables reference these)
            sqlx::query!("DELETE FROM workflow_result WHERE workflow_id = $1", id)
                .execute(&mut *conn)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to delete workflow_result"))?;
            sqlx::query!("DELETE FROM job_depends_on WHERE workflow_id = $1", id)
                .execute(&mut *conn)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to delete job_depends_on"))?;
            sqlx::query!("DELETE FROM job_input_file WHERE workflow_id = $1", id)
                .execute(&mut *conn)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to delete job_input_file"))?;
            sqlx::query!("DELETE FROM job_output_file WHERE workflow_id = $1", id)
                .execute(&mut *conn)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to delete job_output_file"))?;
            // These junction tables lack workflow_id, so delete via job subquery.
            sqlx::query!(
                "DELETE FROM job_input_user_data WHERE job_id IN \
                 (SELECT id FROM job WHERE workflow_id = $1)",
                id
            )
            .execute(&mut *conn)
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to delete job_input_user_data"))?;
            sqlx::query!(
                "DELETE FROM job_output_user_data WHERE job_id IN \
                 (SELECT id FROM job WHERE workflow_id = $1)",
                id
            )
            .execute(&mut *conn)
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to delete job_output_user_data"))?;
            sqlx::query!(
                "DELETE FROM job_internal WHERE job_id IN \
                 (SELECT id FROM job WHERE workflow_id = $1)",
                id
            )
            .execute(&mut *conn)
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to delete job_internal"))?;
            sqlx::query!("DELETE FROM event WHERE workflow_id = $1", id)
                .execute(&mut *conn)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to delete event"))?;
            sqlx::query!(
                "DELETE FROM workflow_access_group WHERE workflow_id = $1",
                id
            )
            .execute(&mut *conn)
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to delete workflow_access_group"))?;
            sqlx::query!("DELETE FROM remote_worker WHERE workflow_id = $1", id)
                .execute(&mut *conn)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to delete remote_worker"))?;

            // Tier 2: Tables referenced by tier 1 (now safe to delete)
            sqlx::query!("DELETE FROM result WHERE workflow_id = $1", id)
                .execute(&mut *conn)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to delete result"))?;
            sqlx::query!("DELETE FROM workflow_action WHERE workflow_id = $1", id)
                .execute(&mut *conn)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to delete workflow_action"))?;

            // Tier 3: Core entity tables
            sqlx::query!("DELETE FROM job WHERE workflow_id = $1", id)
                .execute(&mut *conn)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to delete job"))?;
            sqlx::query!("DELETE FROM file WHERE workflow_id = $1", id)
                .execute(&mut *conn)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to delete file"))?;
            sqlx::query!("DELETE FROM user_data WHERE workflow_id = $1", id)
                .execute(&mut *conn)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to delete user_data"))?;
            sqlx::query!("DELETE FROM compute_node WHERE workflow_id = $1", id)
                .execute(&mut *conn)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to delete compute_node"))?;
            sqlx::query!(
                "DELETE FROM resource_requirements WHERE workflow_id = $1",
                id
            )
            .execute(&mut *conn)
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to delete resource_requirements"))?;
            sqlx::query!("DELETE FROM failure_handler WHERE workflow_id = $1", id)
                .execute(&mut *conn)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to delete failure_handler"))?;
            sqlx::query!("DELETE FROM local_scheduler WHERE workflow_id = $1", id)
                .execute(&mut *conn)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to delete local_scheduler"))?;
            sqlx::query!("DELETE FROM slurm_scheduler WHERE workflow_id = $1", id)
                .execute(&mut *conn)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to delete slurm_scheduler"))?;
            sqlx::query!(
                "DELETE FROM scheduled_compute_node WHERE workflow_id = $1",
                id
            )
            .execute(&mut *conn)
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to delete scheduled_compute_node"))?;

            // Tier 4: The workflow itself (should be fast now with no children)
            let res = sqlx::query!("DELETE FROM workflow WHERE id = $1", id)
                .execute(&mut *conn)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to delete workflow"))?;

            if res.rows_affected() > 1 {
                error!(
                    "Unexpected number of rows affected when deleting workflow {}: {}",
                    id,
                    res.rows_affected()
                );
                return Err(ApiError(format!(
                    "Database error: Unexpected number of rows affected: {}",
                    res.rows_affected()
                )));
            }

            sqlx::query("COMMIT")
                .execute(&mut *conn)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to commit transaction"))?;

            Ok::<(), ApiError>(())
        }
        .await;

        // On error, ROLLBACK to close the transaction before restoring FK checks.
        // PRAGMA foreign_keys is a no-op inside an open transaction, so we must
        // close it first. If rollback fails, detach the connection to prevent
        // returning it to the pool with an open write lock.
        if let Err(delete_err) = result {
            if let Err(rb_err) = sqlx::query("ROLLBACK").execute(&mut *conn).await {
                error!("Failed to rollback transaction: {rb_err}; dropping connection");
                conn.detach();
            } else {
                // Rollback succeeded. Re-enable FK checks before returning.
                let _ = sqlx::query("PRAGMA foreign_keys = ON")
                    .execute(&mut *conn)
                    .await;
            }
            return Err(delete_err);
        }

        // Success path: re-enable FK checks before returning connection to pool.
        if sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *conn)
            .await
            .is_err()
        {
            error!("Failed to re-enable foreign key checks; dropping connection");
            conn.detach();
        }

        info!(
            "Successfully deleted workflow {} (name: {:?})",
            id, workflow.name
        );
        Ok(DeleteWorkflowResponse::SuccessfulResponse(workflow))
    }

    /// Reset workflow status.
    async fn reset_workflow_status(
        &self,
        id: i64,
        force: Option<bool>,
        context: &C,
    ) -> Result<ResetWorkflowStatusResponse, ApiError> {
        debug!(
            "reset_workflow_status({}, force={:?}) - X-Span-ID: {:?}",
            id,
            force,
            context.get().0.clone()
        );

        // Use force flag from query parameter (defaults to false)
        let force = force.unwrap_or(false);

        let current_workflow = match self.get_workflow(id, context).await? {
            GetWorkflowResponse::SuccessfulResponse(wf) => wf,
            GetWorkflowResponse::ForbiddenErrorResponse(err) => {
                // Return the forbidden error as a default error since this endpoint
                // doesn't have a ForbiddenErrorResponse variant in the OpenAPI spec
                return Ok(ResetWorkflowStatusResponse::DefaultErrorResponse(err));
            }
            GetWorkflowResponse::NotFoundErrorResponse(err) => {
                return Ok(ResetWorkflowStatusResponse::NotFoundErrorResponse(err));
            }
            GetWorkflowResponse::DefaultErrorResponse(_) => {
                error!("Failed to get workflow before reset for workflow_id={}", id);
                return Err(ApiError("Failed to get workflow".to_string()));
            }
        };

        // Check if workflow is archived
        if current_workflow.is_archived.unwrap_or(false) {
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
                return Err(database_error_with_msg(
                    e,
                    "Failed to check for active jobs",
                ));
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
                return Err(database_error_with_msg(e, "Failed to check for active compute nodes"));
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
                return Err(database_error_with_msg(e, "Failed to begin transaction"));
            }
        };

        // Reset clears the lifecycle flags and bumps run_id by one — semantically
        // "start a new run". Folding the bump in here keeps the API surface
        // small and means run_id is only mutated through this endpoint.
        match sqlx::query!(
            r#"
            UPDATE workflow
            SET is_canceled = 0,
                is_archived = 0,
                run_id = run_id + 1
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
                return Err(database_error_with_msg(
                    e,
                    "Failed to reset workflow status",
                ));
            }
        }

        // Commit the transaction
        if let Err(e) = tx.commit().await {
            return Err(database_error_with_msg(e, "Failed to commit transaction"));
        }

        // Return the post-reset workflow row.
        info!("Successfully reset workflow status for ID: {}", id);
        let updated = match self.get_workflow(id, context).await? {
            GetWorkflowResponse::SuccessfulResponse(wf) => wf,
            _ => {
                error!("Failed to get workflow {} after reset", id);
                return Err(ApiError("Failed to get workflow after reset".to_string()));
            }
        };
        let response_json = serde_json::to_value(updated)
            .map_err(|e| ApiError(format!("Failed to serialize workflow: {e}")))?;
        Ok(ResetWorkflowStatusResponse::SuccessfulResponse(
            response_json,
        ))
    }

    /// Retrieve job blocking relationships for a workflow.
    async fn list_job_dependencies(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListJobDependenciesResponse, ApiError> {
        debug!(
            "list_job_dependencies({}, {:?}, {:?}, {:?}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            context.get().0.clone()
        );

        let offset_val = offset.unwrap_or(0);
        let limit_val = limit
            .unwrap_or(MAX_RECORD_TRANSFER_COUNT)
            .min(MAX_RECORD_TRANSFER_COUNT);

        let validated_sort_by = match sort_by.as_deref() {
            Some("job_id") => Some("jb.job_id".to_string()),
            Some("job_name") => Some("j1.name".to_string()),
            Some("depends_on_job_id") => Some("jb.depends_on_job_id".to_string()),
            Some("depends_on_job_name") => Some("j2.name".to_string()),
            Some("workflow_id") => Some("jb.workflow_id".to_string()),
            Some(col) => {
                debug!("Invalid sort column requested: {}", col);
                None
            }
            None => None,
        };

        let query = SqlQueryBuilder::new(
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
            "#
            .to_string(),
        )
        .with_where("jb.workflow_id = ?".to_string())
        .with_pagination_and_sorting(
            offset_val,
            limit_val,
            validated_sort_by,
            reverse_sort,
            "jb.job_id",
            JOB_DEPENDENCY_COLUMNS,
        )
        .build();

        let dependency_rows = match sqlx::query(&query)
            .bind(workflow_id)
            .fetch_all(self.context.pool.as_ref())
            .await
        {
            Ok(deps) => deps,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to list job dependencies",
                ));
            }
        };

        let dependencies: Vec<models::JobDependencyModel> = dependency_rows
            .into_iter()
            .map(|record| models::JobDependencyModel {
                job_id: record.get("job_id"),
                job_name: record.get("job_name"),
                depends_on_job_id: record.get("depends_on_job_id"),
                depends_on_job_name: record.get("depends_on_job_name"),
                workflow_id: record.get("workflow_id"),
            })
            .collect();

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
                return Err(database_error_with_msg(
                    e,
                    "Failed to count job dependencies",
                ));
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
                items: dependencies,
                offset: offset_val,
                max_limit: MAX_RECORD_TRANSFER_COUNT,
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
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListJobFileRelationshipsResponse, ApiError> {
        debug!(
            "list_job_file_relationships({}, {:?}, {:?}, {:?}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            context.get().0.clone()
        );

        let offset_val = offset.unwrap_or(0);
        let limit_val = limit
            .unwrap_or(MAX_RECORD_TRANSFER_COUNT)
            .min(MAX_RECORD_TRANSFER_COUNT);

        let validated_sort_by = match sort_by.as_deref() {
            Some("file_id") => Some("f.id".to_string()),
            Some("file_name") => Some("f.name".to_string()),
            Some("file_path") => Some("f.path".to_string()),
            Some("producer_job_id") => Some("jof.job_id".to_string()),
            Some("producer_job_name") => Some("producer.name".to_string()),
            Some("consumer_job_id") => Some("jif.job_id".to_string()),
            Some("consumer_job_name") => Some("consumer.name".to_string()),
            Some("workflow_id") => Some("f.workflow_id".to_string()),
            Some(col) => {
                debug!("Invalid sort column requested: {}", col);
                None
            }
            None => None,
        };

        let query = SqlQueryBuilder::new(
            r#"
            SELECT
                f.id as file_id,
                f.name as file_name,
                f.path as file_path,
                jof.job_id as producer_job_id,
                producer.name as producer_job_name,
                jif.job_id as consumer_job_id,
                consumer.name as consumer_job_name,
                f.workflow_id as workflow_id
            FROM file f
            LEFT JOIN job_output_file jof ON f.id = jof.file_id
            LEFT JOIN job producer ON jof.job_id = producer.id
            LEFT JOIN job_input_file jif ON f.id = jif.file_id
            LEFT JOIN job consumer ON jif.job_id = consumer.id
            "#
            .to_string(),
        )
        .with_where(
            "f.workflow_id = ? AND (jof.job_id IS NOT NULL OR jif.job_id IS NOT NULL)".to_string(),
        )
        .with_pagination_and_sorting(
            offset_val,
            limit_val,
            validated_sort_by,
            reverse_sort,
            "f.id",
            JOB_FILE_RELATIONSHIP_COLUMNS,
        )
        .build();

        let relationship_rows = match sqlx::query(&query)
            .bind(workflow_id)
            .fetch_all(self.context.pool.as_ref())
            .await
        {
            Ok(rels) => rels,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to list job file relationships",
                ));
            }
        };

        let relationships: Vec<models::JobFileRelationshipModel> = relationship_rows
            .into_iter()
            .map(|record| models::JobFileRelationshipModel {
                file_id: record.get("file_id"),
                file_name: record.get("file_name"),
                file_path: record.get("file_path"),
                producer_job_id: record
                    .try_get::<Option<i64>, _>("producer_job_id")
                    .ok()
                    .flatten(),
                producer_job_name: record
                    .try_get::<Option<String>, _>("producer_job_name")
                    .ok()
                    .flatten(),
                consumer_job_id: record
                    .try_get::<Option<i64>, _>("consumer_job_id")
                    .ok()
                    .flatten(),
                consumer_job_name: record
                    .try_get::<Option<String>, _>("consumer_job_name")
                    .ok()
                    .flatten(),
                workflow_id: record.get("workflow_id"),
            })
            .collect();

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
                return Err(database_error_with_msg(
                    e,
                    "Failed to count job file relationships",
                ));
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
                items: relationships,
                offset: offset_val,
                max_limit: MAX_RECORD_TRANSFER_COUNT,
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
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListJobUserDataRelationshipsResponse, ApiError> {
        debug!(
            "list_job_user_data_relationships({}, {:?}, {:?}, {:?}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            context.get().0.clone()
        );

        let offset_val = offset.unwrap_or(0);
        let limit_val = limit
            .unwrap_or(MAX_RECORD_TRANSFER_COUNT)
            .min(MAX_RECORD_TRANSFER_COUNT);

        let validated_sort_by = match sort_by.as_deref() {
            Some("user_data_id") => Some("ud.id".to_string()),
            Some("user_data_name") => Some("ud.name".to_string()),
            Some("producer_job_id") => Some("joud.job_id".to_string()),
            Some("producer_job_name") => Some("producer.name".to_string()),
            Some("consumer_job_id") => Some("jiud.job_id".to_string()),
            Some("consumer_job_name") => Some("consumer.name".to_string()),
            Some("workflow_id") => Some("ud.workflow_id".to_string()),
            Some(col) => {
                debug!("Invalid sort column requested: {}", col);
                None
            }
            None => None,
        };

        let query = SqlQueryBuilder::new(
            r#"
            SELECT
                ud.id as user_data_id,
                ud.name as user_data_name,
                joud.job_id as producer_job_id,
                producer.name as producer_job_name,
                jiud.job_id as consumer_job_id,
                consumer.name as consumer_job_name,
                ud.workflow_id as workflow_id
            FROM user_data ud
            LEFT JOIN job_output_user_data joud ON ud.id = joud.user_data_id
            LEFT JOIN job producer ON joud.job_id = producer.id
            LEFT JOIN job_input_user_data jiud ON ud.id = jiud.user_data_id
            LEFT JOIN job consumer ON jiud.job_id = consumer.id
            "#
            .to_string(),
        )
        .with_where(
            "ud.workflow_id = ? AND (joud.job_id IS NOT NULL OR jiud.job_id IS NOT NULL)"
                .to_string(),
        )
        .with_pagination_and_sorting(
            offset_val,
            limit_val,
            validated_sort_by,
            reverse_sort,
            "ud.id",
            JOB_USER_DATA_RELATIONSHIP_COLUMNS,
        )
        .build();

        let relationship_rows = match sqlx::query(&query)
            .bind(workflow_id)
            .fetch_all(self.context.pool.as_ref())
            .await
        {
            Ok(rels) => rels,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to list job user data relationships",
                ));
            }
        };

        let relationships: Vec<models::JobUserDataRelationshipModel> = relationship_rows
            .into_iter()
            .map(|record| models::JobUserDataRelationshipModel {
                user_data_id: record.get("user_data_id"),
                user_data_name: record.get("user_data_name"),
                producer_job_id: record
                    .try_get::<Option<i64>, _>("producer_job_id")
                    .ok()
                    .flatten(),
                producer_job_name: record
                    .try_get::<Option<String>, _>("producer_job_name")
                    .ok()
                    .flatten(),
                consumer_job_id: record
                    .try_get::<Option<i64>, _>("consumer_job_id")
                    .ok()
                    .flatten(),
                consumer_job_name: record
                    .try_get::<Option<String>, _>("consumer_job_name")
                    .ok()
                    .flatten(),
                workflow_id: record.get("workflow_id"),
            })
            .collect();

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
                return Err(database_error_with_msg(
                    e,
                    "Failed to count job user data relationships",
                ));
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
                items: relationships,
                offset: offset_val,
                max_limit: MAX_RECORD_TRANSFER_COUNT,
                count: current_count,
                total_count,
                has_more,
            },
        ))
    }
}
