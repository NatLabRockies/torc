//! Workflow action-related API endpoints

use crate::server::transport_types::context_types::{ApiError, Has, XSpanIdString};
use async_trait::async_trait;
use log::{debug, error, info, warn};
use sqlx::Row;
use std::collections::{HashMap, HashSet};

use crate::server::api_responses::{
    ClaimActionResponse, CreateWorkflowActionResponse, DeleteWorkflowActionResponse,
    GetPendingActionsResponse, GetWorkflowActionsResponse, ReportActionResultResponse,
    ResetActionResponse, SuppressActionResponse, UpdateWorkflowActionResponse,
};

use crate::models;
use crate::models::JobStatus;

use super::{
    ApiContext, database_error_with_msg, message_error_response, resource_not_found_response,
};

/// Build a [`models::WorkflowActionModel`] from a `workflow_action` row that selected every column.
fn action_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<models::WorkflowActionModel, String> {
    let action_config_str: String = row.get("action_config");
    let action_config: serde_json::Value = serde_json::from_str(&action_config_str)
        .map_err(|e| format!("Failed to parse action_config: {}", e))?;
    // Deserialize job_ids from JSON string to Vec<i64>
    let job_ids_str: Option<String> = row.get("job_ids");
    let job_ids: Option<Vec<i64>> = job_ids_str.and_then(|s| serde_json::from_str(&s).ok());
    Ok(models::WorkflowActionModel {
        id: Some(row.get("id")),
        workflow_id: row.get("workflow_id"),
        trigger_type: row.get("trigger_type"),
        action_type: row.get("action_type"),
        action_config,
        job_ids,
        trigger_count: row.get("trigger_count"),
        required_triggers: row.get("required_triggers"),
        executed: row.get::<i32, _>("executed") != 0,
        executed_at: row.get("executed_at"),
        executed_by: row.get("executed_by"),
        persistent: row.get::<i32, _>("persistent") != 0,
        is_recovery: row.get::<i32, _>("is_recovery") != 0,
        state: row.get("state"),
        error: row.get("error"),
    })
}

/// Validate action_config based on action_type
fn validate_action_config(
    action_type: &str,
    action_config: &serde_json::Value,
) -> Result<(), String> {
    let config_obj = action_config
        .as_object()
        .ok_or("action_config must be an object")?;

    match action_type {
        "run_commands" => {
            // Only "commands" field is allowed
            if config_obj.len() != 1 || !config_obj.contains_key("commands") {
                return Err(
                    "For action_type 'run_commands', only 'commands' field is allowed".to_string(),
                );
            }

            // Must be an array of strings
            let commands = config_obj
                .get("commands")
                .ok_or("'commands' field is required")?;
            let commands_array = commands.as_array().ok_or("'commands' must be an array")?;

            // Cannot be empty
            if commands_array.is_empty() {
                return Err("'commands' array cannot be empty".to_string());
            }

            // All elements must be strings
            for (i, cmd) in commands_array.iter().enumerate() {
                if !cmd.is_string() {
                    return Err(format!("'commands[{}]' must be a string", i));
                }
            }

            Ok(())
        }
        "schedule_nodes" => {
            let allowed_fields = [
                "scheduler_id",
                "scheduler_type",
                "num_allocations",
                "start_one_worker_per_node",
                "max_parallel_jobs",
            ];

            // Check for unsupported fields
            for key in config_obj.keys() {
                if !allowed_fields.contains(&key.as_str()) {
                    return Err(format!(
                        "Unsupported field '{}' for action_type 'schedule_nodes'. Allowed fields: {}",
                        key,
                        allowed_fields.join(", ")
                    ));
                }
            }

            // Validate field types if present
            if let Some(scheduler_id) = config_obj.get("scheduler_id")
                && !scheduler_id.is_i64()
                && !scheduler_id.is_u64()
            {
                return Err("'scheduler_id' must be an integer".to_string());
            }

            if let Some(scheduler_type) = config_obj.get("scheduler_type")
                && !scheduler_type.is_string()
            {
                return Err("'scheduler_type' must be a string".to_string());
            }

            if let Some(num_allocations) = config_obj.get("num_allocations")
                && !num_allocations.is_i64()
                && !num_allocations.is_u64()
            {
                return Err("'num_allocations' must be an integer".to_string());
            }

            if let Some(start_one_worker_per_node) = config_obj.get("start_one_worker_per_node")
                && !start_one_worker_per_node.is_boolean()
            {
                return Err("'start_one_worker_per_node' must be a boolean".to_string());
            }

            if let Some(max_parallel_jobs) = config_obj.get("max_parallel_jobs")
                && !max_parallel_jobs.is_i64()
                && !max_parallel_jobs.is_u64()
            {
                return Err("'max_parallel_jobs' must be an integer".to_string());
            }

            Ok(())
        }
        _ => {
            // For other action types, we don't validate the config
            Ok(())
        }
    }
}

/// Trait defining workflow action-related API operations
#[async_trait]
pub trait WorkflowActionsApi<C> {
    /// Create a workflow action
    async fn create_workflow_action(
        &self,
        workflow_id: i64,
        body: models::WorkflowActionModel,
        context: &C,
    ) -> Result<CreateWorkflowActionResponse, ApiError>;

    /// Get all workflow actions for a workflow
    async fn get_workflow_actions(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<GetWorkflowActionsResponse, ApiError>;

    /// Get pending (unexecuted) workflow actions for a workflow
    async fn get_pending_actions(
        &self,
        workflow_id: i64,
        trigger_types: Option<Vec<String>>,
        context: &C,
    ) -> Result<GetPendingActionsResponse, ApiError>;

    /// Atomically claim a workflow action for execution
    /// compute_node_id is optional - if None, executed_by will be NULL (used for submission from login nodes)
    async fn claim_action(
        &self,
        workflow_id: i64,
        action_id: i64,
        compute_node_id: Option<i64>,
        context: &C,
    ) -> Result<ClaimActionResponse, ApiError>;

    /// Mark an action suppressed so it is never claimed, without executing it.
    async fn suppress_action(
        &self,
        workflow_id: i64,
        action_id: i64,
        context: &C,
    ) -> Result<SuppressActionResponse, ApiError>;

    /// Record the outcome of a claimed action.
    async fn report_action_result(
        &self,
        workflow_id: i64,
        action_id: i64,
        body: models::WorkflowActionResultRequest,
        context: &C,
    ) -> Result<ReportActionResultResponse, ApiError>;

    /// Re-arm a failed, suppressed, or claimed action so it can be claimed again.
    async fn reset_action(
        &self,
        workflow_id: i64,
        action_id: i64,
        context: &C,
    ) -> Result<ResetActionResponse, ApiError>;

    /// Update action_config for an existing workflow action (partial update)
    async fn update_workflow_action(
        &self,
        workflow_id: i64,
        action_id: i64,
        updates: serde_json::Value,
        context: &C,
    ) -> Result<UpdateWorkflowActionResponse, ApiError>;

    /// Delete a single workflow action
    async fn delete_workflow_action(
        &self,
        workflow_id: i64,
        action_id: i64,
        context: &C,
    ) -> Result<DeleteWorkflowActionResponse, ApiError>;
}

/// Implementation of workflow actions API for the server
#[derive(Clone)]
pub struct WorkflowActionsApiImpl {
    pub context: ApiContext,
}

impl WorkflowActionsApiImpl {
    pub fn new(context: ApiContext) -> Self {
        Self { context }
    }
}

#[async_trait]
impl<C> WorkflowActionsApi<C> for WorkflowActionsApiImpl
where
    C: Has<XSpanIdString> + Send + Sync,
{
    /// Create a workflow action
    async fn create_workflow_action(
        &self,
        workflow_id: i64,
        mut body: models::WorkflowActionModel,
        context: &C,
    ) -> Result<CreateWorkflowActionResponse, ApiError> {
        debug!(
            "create_workflow_action(workflow_id={}) - X-Span-ID: {:?}",
            workflow_id,
            context.get().0.clone()
        );

        // Ensure workflow_id in body matches parameter
        body.workflow_id = workflow_id;

        // Validate action_config based on action_type
        if let Err(validation_error) =
            validate_action_config(&body.action_type, &body.action_config)
        {
            error!("Invalid action_config: {}", validation_error);
            return Ok(
                CreateWorkflowActionResponse::UnprocessableContentErrorResponse(
                    message_error_response(format!("Invalid action_config: {}", validation_error)),
                ),
            );
        }

        // Convert action_config to JSON string for storage
        let action_config_str = body.action_config.to_string();

        // Calculate required_triggers based on trigger type and job_ids
        let required_triggers =
            if body.trigger_type == "on_jobs_ready" || body.trigger_type == "on_jobs_complete" {
                // For job-based triggers, count the number of job IDs
                if let Some(ref ids) = body.job_ids {
                    ids.len() as i64
                } else {
                    1 // Default if no job_ids specified
                }
            } else {
                // For workflow/worker triggers, only need 1 trigger
                1
            };

        // Convert job_ids to JSON array format for database storage
        let job_ids_json: Option<String> = body
            .job_ids
            .as_ref()
            .map(|ids| serde_json::to_string(ids).expect("Failed to serialize job_ids"));

        let result = sqlx::query(
            "INSERT INTO workflow_action (workflow_id, trigger_type, action_type, action_config, job_ids, trigger_count, required_triggers, executed, persistent, is_recovery)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(body.workflow_id)
        .bind(&body.trigger_type)
        .bind(&body.action_type)
        .bind(&action_config_str)
        .bind(job_ids_json.as_ref())
        .bind(body.trigger_count)
        .bind(required_triggers)
        .bind(if body.executed { 1 } else { 0 })
        .bind(if body.persistent { 1 } else { 0 })
        .bind(if body.is_recovery { 1 } else { 0 })
        .execute(self.context.pool.as_ref())
        .await;

        match result {
            Ok(result) => {
                let id = result.last_insert_rowid();
                debug!("Created workflow action with id={}", id);

                body.id = Some(id);

                Ok(CreateWorkflowActionResponse::SuccessfulResponse(body))
            }
            Err(e) => {
                error!("Failed to create workflow action: {}", e);
                Ok(CreateWorkflowActionResponse::DefaultErrorResponse(
                    message_error_response(format!("Failed to create workflow action: {}", e)),
                ))
            }
        }
    }

    /// Get all workflow actions for a workflow
    async fn get_workflow_actions(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<GetWorkflowActionsResponse, ApiError> {
        debug!(
            "get_workflow_actions(workflow_id={}) - X-Span-ID: {:?}",
            workflow_id,
            context.get().0.clone()
        );

        let rows = sqlx::query(
            "SELECT id, workflow_id, trigger_type, action_type, action_config, job_ids, trigger_count, required_triggers, executed, executed_at, executed_by, persistent, is_recovery, state, error
             FROM workflow_action
             WHERE workflow_id = ?
             ORDER BY id"
        )
        .bind(workflow_id)
        .fetch_all(self.context.pool.as_ref())
        .await;

        match rows {
            Ok(rows) => {
                let actions: Result<Vec<models::WorkflowActionModel>, String> =
                    rows.into_iter().map(|row| action_from_row(&row)).collect();

                match actions {
                    Ok(actions) => Ok(GetWorkflowActionsResponse::SuccessfulResponse(actions)),
                    Err(e) => {
                        error!("Failed to parse workflow actions: {}", e);
                        Ok(GetWorkflowActionsResponse::DefaultErrorResponse(
                            message_error_response(format!(
                                "Failed to parse workflow actions: {}",
                                e
                            )),
                        ))
                    }
                }
            }
            Err(e) => {
                error!("Failed to get workflow actions: {}", e);
                Ok(GetWorkflowActionsResponse::DefaultErrorResponse(
                    message_error_response(format!("Failed to get workflow actions: {}", e)),
                ))
            }
        }
    }

    /// Get pending (unexecuted) workflow actions for a workflow
    async fn get_pending_actions(
        &self,
        workflow_id: i64,
        trigger_types: Option<Vec<String>>,
        context: &C,
    ) -> Result<GetPendingActionsResponse, ApiError> {
        debug!(
            "get_pending_actions(workflow_id={}, trigger_types={:?}) - X-Span-ID: {:?}",
            workflow_id,
            trigger_types,
            context.get().0.clone()
        );

        // Build query with optional trigger_type filter
        let (_query_str, rows) = if let Some(ref types) = trigger_types {
            if types.is_empty() {
                // If empty list provided, return no results
                (String::new(), Ok(Vec::new()))
            } else {
                // Build IN clause with placeholders
                let placeholders = types.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
                let query_str = format!(
                    "SELECT id, workflow_id, trigger_type, action_type, action_config, job_ids, trigger_count, required_triggers, executed, executed_at, executed_by, persistent, is_recovery, state, error
                     FROM workflow_action
                     WHERE workflow_id = ? AND trigger_count >= required_triggers AND executed = 0 AND trigger_type IN ({})
                     ORDER BY id",
                    placeholders
                );

                let mut query = sqlx::query(sqlx::AssertSqlSafe(&*query_str)).bind(workflow_id);
                for trigger_type in types {
                    query = query.bind(trigger_type);
                }

                (
                    query_str.clone(),
                    query.fetch_all(self.context.pool.as_ref()).await,
                )
            }
        } else {
            // No filter - get all pending actions
            let query_str = "SELECT id, workflow_id, trigger_type, action_type, action_config, job_ids, trigger_count, required_triggers, executed, executed_at, executed_by, persistent, is_recovery, state, error
                 FROM workflow_action
                 WHERE workflow_id = ? AND trigger_count >= required_triggers AND executed = 0
                 ORDER BY id".to_string();
            (
                query_str.clone(),
                sqlx::query(sqlx::AssertSqlSafe(&*query_str))
                    .bind(workflow_id)
                    .fetch_all(self.context.pool.as_ref())
                    .await,
            )
        };

        let rows = rows;

        match rows {
            Ok(rows) => {
                let actions: Result<Vec<models::WorkflowActionModel>, String> =
                    rows.into_iter().map(|row| action_from_row(&row)).collect();

                match actions {
                    Ok(actions) => Ok(GetPendingActionsResponse::SuccessfulResponse(actions)),
                    Err(e) => {
                        error!("Failed to parse pending actions: {}", e);
                        Ok(GetPendingActionsResponse::DefaultErrorResponse(
                            message_error_response(format!(
                                "Failed to parse pending actions: {}",
                                e
                            )),
                        ))
                    }
                }
            }
            Err(e) => {
                error!("Failed to get pending actions: {}", e);
                Ok(GetPendingActionsResponse::DefaultErrorResponse(
                    message_error_response(format!("Failed to get pending actions: {}", e)),
                ))
            }
        }
    }

    /// Atomically claim a workflow action for execution
    async fn claim_action(
        &self,
        workflow_id: i64,
        action_id: i64,
        compute_node_id: Option<i64>,
        context: &C,
    ) -> Result<ClaimActionResponse, ApiError> {
        debug!(
            "claim_action(workflow_id={}, action_id={}, compute_node_id={:?}) - X-Span-ID: {:?}",
            workflow_id,
            action_id,
            compute_node_id,
            context.get().0.clone()
        );

        // Verify action exists and belongs to this workflow
        let action_check = sqlx::query(
            "SELECT workflow_id, executed, persistent, state FROM workflow_action WHERE id = ?",
        )
        .bind(action_id)
        .fetch_optional(self.context.pool.as_ref())
        .await;

        let is_persistent = match action_check {
            Ok(Some(record)) => {
                let workflow_id_col: i64 = record.get("workflow_id");
                let executed_col: i32 = record.get("executed");
                let persistent_col: i32 = record.get("persistent");
                let state_col: Option<String> = record.get("state");

                if workflow_id_col != workflow_id {
                    return Ok(ClaimActionResponse::NotFoundErrorResponse(
                        message_error_response(format!(
                            "Action {} does not belong to workflow {}",
                            action_id, workflow_id
                        )),
                    ));
                }
                // A persistent action keeps executed = 0 across claims, so executed = 1 means it
                // was suppressed; a non-persistent one was claimed or suppressed.
                if executed_col != 0 {
                    return Ok(ClaimActionResponse::ConflictResponse(claim_conflict(
                        action_id,
                        state_col.as_deref(),
                    )));
                }
                persistent_col != 0
            }
            Ok(None) => {
                return Ok(ClaimActionResponse::NotFoundErrorResponse(
                    resource_not_found_response("Action", action_id),
                ));
            }
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to check workflow action",
                ));
            }
        };

        // Atomically claim the action
        let now = chrono::Utc::now().to_rfc3339();

        // For persistent actions, don't mark as executed so other workers can claim it
        // For non-persistent actions, mark as executed
        // Both statements require executed = 0, so a concurrent suppress wins or loses atomically.
        let result = if is_persistent {
            // For persistent actions, record the claim but keep executed=0 so other workers can
            // claim it. state/error hold the latest claim's outcome.
            // Note: This simple implementation doesn't track which compute nodes have claimed it
            // A more robust implementation would use a separate workflow_action_claims table
            sqlx::query(
                "UPDATE workflow_action
                 SET executed_at = ?, state = 'claimed', error = NULL
                 WHERE id = ? AND executed = 0",
            )
            .bind(&now)
            .bind(action_id)
            .execute(self.context.pool.as_ref())
            .await
        } else {
            sqlx::query(
                "UPDATE workflow_action
                 SET executed = 1, executed_at = ?, executed_by = ?, state = 'claimed',
                     error = NULL
                 WHERE id = ? AND executed = 0",
            )
            .bind(&now)
            .bind(compute_node_id)
            .bind(action_id)
            .execute(self.context.pool.as_ref())
            .await
        };

        match result {
            Ok(result) => {
                let claimed = result.rows_affected() > 0;
                if claimed {
                    info!(
                        "Successfully claimed action {} for compute node {:?} (persistent={})",
                        action_id, compute_node_id, is_persistent
                    );
                    let response = serde_json::json!({"claimed": true, "action_id": action_id});
                    Ok(ClaimActionResponse::SuccessfulResponse(response))
                } else {
                    // Race condition: action was claimed or suppressed between check and update
                    let state = self
                        .fetch_action(workflow_id, action_id)
                        .await?
                        .and_then(|a| a.state);
                    Ok(ClaimActionResponse::ConflictResponse(claim_conflict(
                        action_id,
                        state.as_deref(),
                    )))
                }
            }
            Err(e) => {
                error!("Failed to claim action: {}", e);
                Ok(ClaimActionResponse::DefaultErrorResponse(
                    message_error_response(format!("Failed to claim action: {}", e)),
                ))
            }
        }
    }

    /// Mark an action suppressed (`executed = 1, state = suppressed`). Idempotent. A persistent
    /// action can be suppressed in any state, since a claim never consumes it; a non-persistent one
    /// only while unclaimed.
    async fn suppress_action(
        &self,
        workflow_id: i64,
        action_id: i64,
        context: &C,
    ) -> Result<SuppressActionResponse, ApiError> {
        debug!(
            "suppress_action(workflow_id={}, action_id={}) - X-Span-ID: {:?}",
            workflow_id,
            action_id,
            context.get().0.clone()
        );
        let result = sqlx::query(
            "UPDATE workflow_action SET executed = 1, state = 'suppressed', error = NULL
             WHERE id = ? AND workflow_id = ?
             AND (persistent = 1 OR executed = 0 OR state = 'suppressed')",
        )
        .bind(action_id)
        .bind(workflow_id)
        .execute(self.context.pool.as_ref())
        .await
        .map_err(|e| database_error_with_msg(e, "Failed to suppress workflow action"))?;
        let Some(action) = self.fetch_action(workflow_id, action_id).await? else {
            return Ok(SuppressActionResponse::NotFoundErrorResponse(
                resource_not_found_response("Action", action_id),
            ));
        };
        if result.rows_affected() == 0 {
            return Ok(SuppressActionResponse::ConflictResponse(
                message_error_response(format!(
                    "Action {} cannot be suppressed: it is already {}",
                    action_id,
                    action.state.as_deref().unwrap_or("claimed")
                )),
            ));
        }
        info!(
            "Suppressed workflow action workflow_id={} action_id={}",
            workflow_id, action_id
        );
        Ok(SuppressActionResponse::SuccessfulResponse(action))
    }

    /// Record a claimed action's outcome: `succeeded`, or `failed` with an error. For persistent
    /// actions this is the latest outcome. Rejected unless the action has been claimed.
    async fn report_action_result(
        &self,
        workflow_id: i64,
        action_id: i64,
        body: models::WorkflowActionResultRequest,
        context: &C,
    ) -> Result<ReportActionResultResponse, ApiError> {
        debug!(
            "report_action_result(workflow_id={}, action_id={}, success={}) - X-Span-ID: {:?}",
            workflow_id,
            action_id,
            body.success,
            context.get().0.clone()
        );
        if body.success && body.error.is_some() {
            return Ok(
                ReportActionResultResponse::UnprocessableContentErrorResponse(
                    message_error_response(
                        "error is only allowed when success is false".to_string(),
                    ),
                ),
            );
        }
        let state = if body.success { "succeeded" } else { "failed" };
        let result = sqlx::query(
            "UPDATE workflow_action SET state = ?, error = ?
             WHERE id = ? AND workflow_id = ? AND state IN ('claimed', 'succeeded', 'failed')",
        )
        .bind(state)
        .bind(&body.error)
        .bind(action_id)
        .bind(workflow_id)
        .execute(self.context.pool.as_ref())
        .await
        .map_err(|e| database_error_with_msg(e, "Failed to record workflow action result"))?;
        let Some(action) = self.fetch_action(workflow_id, action_id).await? else {
            return Ok(ReportActionResultResponse::NotFoundErrorResponse(
                resource_not_found_response("Action", action_id),
            ));
        };
        if result.rows_affected() == 0 {
            return Ok(ReportActionResultResponse::ConflictResponse(
                message_error_response(format!(
                    "Action {} has no claim to report a result for (state: {})",
                    action_id,
                    action.state.as_deref().unwrap_or("none")
                )),
            ));
        }
        if body.success {
            info!(
                "Action succeeded workflow_id={} action_id={}",
                workflow_id, action_id
            );
        } else {
            warn!(
                "Action failed workflow_id={} action_id={} error={}",
                workflow_id,
                action_id,
                body.error.as_deref().unwrap_or("")
            );
        }
        Ok(ReportActionResultResponse::SuccessfulResponse(action))
    }

    /// Re-arm an action: clear `executed`, `state`, `error`, and the claim record so it can be
    /// claimed again. Rejected for a succeeded action; a no-op for one that was never claimed.
    async fn reset_action(
        &self,
        workflow_id: i64,
        action_id: i64,
        context: &C,
    ) -> Result<ResetActionResponse, ApiError> {
        debug!(
            "reset_action(workflow_id={}, action_id={}) - X-Span-ID: {:?}",
            workflow_id,
            action_id,
            context.get().0.clone()
        );
        let result = sqlx::query(
            "UPDATE workflow_action
             SET executed = 0, executed_at = NULL, executed_by = NULL, state = NULL, error = NULL
             WHERE id = ? AND workflow_id = ? AND (state IS NULL OR state != 'succeeded')",
        )
        .bind(action_id)
        .bind(workflow_id)
        .execute(self.context.pool.as_ref())
        .await
        .map_err(|e| database_error_with_msg(e, "Failed to reset workflow action"))?;
        let Some(action) = self.fetch_action(workflow_id, action_id).await? else {
            return Ok(ResetActionResponse::NotFoundErrorResponse(
                resource_not_found_response("Action", action_id),
            ));
        };
        if result.rows_affected() == 0 {
            return Ok(ResetActionResponse::ConflictResponse(
                message_error_response(format!(
                    "Action {} already succeeded and cannot be reset",
                    action_id
                )),
            ));
        }
        info!(
            "Reset workflow action workflow_id={} action_id={}",
            workflow_id, action_id
        );
        Ok(ResetActionResponse::SuccessfulResponse(action))
    }

    /// Update action_config for an existing workflow action (partial update)
    async fn update_workflow_action(
        &self,
        workflow_id: i64,
        action_id: i64,
        updates: serde_json::Value,
        context: &C,
    ) -> Result<UpdateWorkflowActionResponse, ApiError> {
        debug!(
            "update_workflow_action(workflow_id={}, action_id={}) - X-Span-ID: {:?}",
            workflow_id,
            action_id,
            context.get().0.clone()
        );

        // Build merge map for the partial update
        let update_map = match updates.as_object() {
            Some(map) => map.clone(),
            None => {
                return Ok(
                    UpdateWorkflowActionResponse::UnprocessableContentErrorResponse(
                        message_error_response("updates body must be a JSON object".to_string()),
                    ),
                );
            }
        };

        if update_map.is_empty() {
            return Ok(
                UpdateWorkflowActionResponse::UnprocessableContentErrorResponse(
                    message_error_response("updates body has no fields to merge".to_string()),
                ),
            );
        }

        // Fetch the existing action (the full row so we can return the updated model
        // without an extra round trip).
        let existing = sqlx::query(
            "SELECT id, workflow_id, trigger_type, action_type, action_config, job_ids, \
             trigger_count, required_triggers, executed, executed_at, executed_by, \
             persistent, is_recovery, state, error \
             FROM workflow_action WHERE id = ? AND workflow_id = ?",
        )
        .bind(action_id)
        .bind(workflow_id)
        .fetch_optional(self.context.pool.as_ref())
        .await
        .map_err(|e| database_error_with_msg(e, "Failed to fetch workflow action"))?;

        let row = match existing {
            Some(row) => row,
            None => {
                return Ok(UpdateWorkflowActionResponse::NotFoundErrorResponse(
                    resource_not_found_response("Action", action_id),
                ));
            }
        };

        let action_type: String = row.get("action_type");
        let existing_config_str: String = row.get("action_config");
        // A malformed/non-object stored config is server-side data corruption, not a
        // client request problem -- fail loudly with a 500 (and log) rather than
        // silently treating it as an empty object, which would drop every existing
        // field on the merge.
        let mut merged: serde_json::Value = match serde_json::from_str(&existing_config_str) {
            Ok(value) => value,
            Err(e) => {
                error!(
                    "stored action_config is not valid JSON for workflow_id={} action_id={}: {}",
                    workflow_id, action_id, e
                );
                return Ok(UpdateWorkflowActionResponse::DefaultErrorResponse(
                    message_error_response("stored action_config is not valid JSON".to_string()),
                ));
            }
        };

        // Merge update fields on top of existing config
        match merged.as_object_mut() {
            Some(existing_obj) => {
                for (key, value) in update_map {
                    existing_obj.insert(key, value);
                }
            }
            None => {
                error!(
                    "stored action_config is not a JSON object for workflow_id={} action_id={}",
                    workflow_id, action_id
                );
                return Ok(UpdateWorkflowActionResponse::DefaultErrorResponse(
                    message_error_response("stored action_config is not a JSON object".to_string()),
                ));
            }
        }

        // Validate the merged config against the action_type
        if let Err(validation_error) = validate_action_config(&action_type, &merged) {
            return Ok(
                UpdateWorkflowActionResponse::UnprocessableContentErrorResponse(
                    message_error_response(format!("Invalid action_config: {}", validation_error)),
                ),
            );
        }

        // Persist the updated config
        let merged_str = merged.to_string();
        if let Err(e) = sqlx::query(
            "UPDATE workflow_action SET action_config = ? WHERE id = ? AND workflow_id = ?",
        )
        .bind(&merged_str)
        .bind(action_id)
        .bind(workflow_id)
        .execute(self.context.pool.as_ref())
        .await
        {
            error!("Failed to update workflow action: {}", e);
            return Ok(UpdateWorkflowActionResponse::DefaultErrorResponse(
                message_error_response(format!("Failed to update workflow action: {}", e)),
            ));
        }

        // Build the updated model from the row we already have plus the merged config.
        let mut updated = match action_from_row(&row) {
            Ok(action) => action,
            Err(e) => {
                return Ok(UpdateWorkflowActionResponse::DefaultErrorResponse(
                    message_error_response(e),
                ));
            }
        };
        updated.action_config = merged;

        Ok(UpdateWorkflowActionResponse::SuccessfulResponse(updated))
    }

    /// Delete a single workflow action
    async fn delete_workflow_action(
        &self,
        workflow_id: i64,
        action_id: i64,
        context: &C,
    ) -> Result<DeleteWorkflowActionResponse, ApiError> {
        debug!(
            "delete_workflow_action(workflow_id={}, action_id={}) - X-Span-ID: {:?}",
            workflow_id,
            action_id,
            context.get().0.clone()
        );

        // Scope the delete to the workflow so an action_id from another workflow
        // cannot be removed (and so a mismatch reports not-found rather than 0 rows).
        let result =
            match sqlx::query("DELETE FROM workflow_action WHERE id = ? AND workflow_id = ?")
                .bind(action_id)
                .bind(workflow_id)
                .execute(self.context.pool.as_ref())
                .await
            {
                Ok(result) => result,
                Err(e) => {
                    return Err(database_error_with_msg(
                        e,
                        "Failed to delete workflow action",
                    ));
                }
            };

        if result.rows_affected() == 0 {
            return Ok(DeleteWorkflowActionResponse::NotFoundErrorResponse(
                resource_not_found_response("Action", action_id),
            ));
        }

        info!(
            "Deleted workflow action workflow_id={} action_id={}",
            workflow_id, action_id
        );
        Ok(DeleteWorkflowActionResponse::SuccessfulResponse(
            serde_json::json!({"message": "Workflow action deleted successfully"}),
        ))
    }
}

/// 409 body for a claim that lost: the action is already claimed, finished, or suppressed.
fn claim_conflict(action_id: i64, state: Option<&str>) -> models::ErrorResponse {
    let state = state.unwrap_or("claimed");
    models::ErrorResponse::new(serde_json::json!({
        "message": format!("Action {} already {}", action_id, state),
        "claimed": false,
        "state": state,
    }))
}

/// Helper methods for workflow actions (not part of the trait)
impl WorkflowActionsApiImpl {
    /// Fetch one action of a workflow, or None if it does not exist in that workflow.
    async fn fetch_action(
        &self,
        workflow_id: i64,
        action_id: i64,
    ) -> Result<Option<models::WorkflowActionModel>, ApiError> {
        let row = sqlx::query(
            "SELECT id, workflow_id, trigger_type, action_type, action_config, job_ids, \
             trigger_count, required_triggers, executed, executed_at, executed_by, \
             persistent, is_recovery, state, error \
             FROM workflow_action WHERE id = ? AND workflow_id = ?",
        )
        .bind(action_id)
        .bind(workflow_id)
        .fetch_optional(self.context.pool.as_ref())
        .await
        .map_err(|e| database_error_with_msg(e, "Failed to fetch workflow action"))?;
        row.map(|r| action_from_row(&r))
            .transpose()
            .map_err(ApiError)
    }

    /// Check and trigger workflow actions based on trigger type and job state changes
    /// This is called by other API endpoints when state changes occur
    ///
    /// `max_action_id` limits the check to actions with `id <= max_action_id`. Callers that commit
    /// a state change before calling this pass the highest action id seen inside that transaction:
    /// an action created after the commit (ids are AUTOINCREMENT, so it has a larger id) was armed
    /// from the committed state and must not count the change again.
    pub async fn check_and_trigger_actions(
        &self,
        workflow_id: i64,
        trigger_type: &str,
        job_ids: Option<Vec<i64>>,
        max_action_id: Option<i64>,
    ) -> Result<(), ApiError> {
        debug!(
            "check_and_trigger_actions(workflow_id={}, trigger_type={}, job_ids={:?})",
            workflow_id, trigger_type, job_ids
        );

        // Get all actions of this trigger type for this workflow that haven't reached required triggers yet
        let actions = match sqlx::query(
            "SELECT id, job_ids, trigger_count, required_triggers FROM workflow_action
             WHERE workflow_id = ? AND trigger_type = ? AND trigger_count < required_triggers
             AND (? IS NULL OR id <= ?)",
        )
        .bind(workflow_id)
        .bind(trigger_type)
        .bind(max_action_id)
        .bind(max_action_id)
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(actions) => actions,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to fetch untriggered actions",
                ));
            }
        };

        for action_row in actions {
            let action_id: i64 = action_row.get("id");
            let action_job_ids_str: Option<String> = action_row.get("job_ids");

            // For trigger types that require job_ids, check if conditions are met
            // Returns the number of triggers to increment (usually 1, but can be > 1 for job-based triggers)
            let trigger_increment = match trigger_type {
                "on_workflow_start"
                | "on_workflow_complete"
                | "on_worker_start"
                | "on_worker_complete" => {
                    // These triggers fire based on workflow/worker state, not individual jobs
                    1
                }
                "on_jobs_ready" | "on_jobs_complete" => {
                    // Check if the action has job_ids specified
                    if let Some(action_job_ids_str) = action_job_ids_str {
                        // Deserialize the JSON array of job IDs from database
                        let action_job_ids: Vec<i64> =
                            match serde_json::from_str(&action_job_ids_str) {
                                Ok(ids) => ids,
                                Err(e) => {
                                    error!(
                                        "Failed to parse job_ids JSON '{}': {}",
                                        action_job_ids_str, e
                                    );
                                    vec![]
                                }
                            };

                        if action_job_ids.is_empty() {
                            0
                        } else {
                            // Check if any of the changed job_ids are in this action's job_ids
                            if let Some(ref changed_job_ids) = job_ids {
                                // Count how many of the changed jobs are in the action's job_ids
                                let overlap_count = action_job_ids
                                    .iter()
                                    .filter(|id| changed_job_ids.contains(id))
                                    .count()
                                    as i64;

                                if overlap_count == 0 {
                                    // None of the changed jobs are relevant to this action
                                    continue;
                                }
                                // Increment by the number of jobs that transitioned
                                overlap_count
                            } else {
                                // No job_ids specified in the call, check current state of all action jobs
                                if self.check_jobs_state(&action_job_ids, trigger_type).await? {
                                    // All jobs are in the required state, increment by the total count
                                    action_job_ids.len() as i64
                                } else {
                                    0
                                }
                            }
                        }
                    } else {
                        0
                    }
                }
                _ => 0,
            };

            if trigger_increment > 0 {
                // Increment trigger_count by the calculated amount
                match sqlx::query(
                    "UPDATE workflow_action SET trigger_count = trigger_count + ? WHERE id = ?",
                )
                .bind(trigger_increment)
                .bind(action_id)
                .execute(self.context.pool.as_ref())
                .await
                {
                    Ok(_) => {
                        info!(
                            "Incremented trigger_count by {} for action {} (trigger_type={}) for workflow {}",
                            trigger_increment, action_id, trigger_type, workflow_id
                        );
                    }
                    Err(e) => {
                        error!(
                            "Failed to increment trigger_count for action {}: {}",
                            action_id, e
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if all jobs meet the required state for the trigger type
    async fn check_jobs_state(
        &self,
        job_ids: &[i64],
        trigger_type: &str,
    ) -> Result<bool, ApiError> {
        let job_ids_json = serde_json::to_string(job_ids).unwrap_or_else(|_| "[]".to_string());
        Ok(
            count_jobs_in_satisfied_state(self.context.pool.as_ref(), &job_ids_json, trigger_type)
                .await?
                == job_ids.len() as i64,
        )
    }

    /// Reset workflow actions for reinitialization, inside the initialize transaction.
    ///
    /// First deletes any recovery actions (created by `torc slurm regenerate`), then, for each
    /// remaining action, recomputes `trigger_count` from current job states and decides whether to
    /// **re-arm** it (clear `executed`/`state` so it can fire again in the new run) or **keep** its
    /// `executed`/`state` (including `suppressed`). A never-fired action keeps `executed = 0` either
    /// way.
    ///
    /// Per trigger:
    ///
    /// - `on_jobs_ready` / `on_jobs_complete` → **re-arm iff this initialize reset at least one of
    ///   the action's gate jobs** (`reset_gate_job_ids`, computed by the caller with
    ///   [`reset_gate_job_ids`]: gates the initialize touched that end `Ready` or `Blocked`, i.e.
    ///   jobs that will become ready again in the new run). Gates left untouched — still complete, still `Ready`
    ///   from before, disabled — mean the event is not recurring, and re-firing would duplicate its
    ///   side effect (a second allocation, a re-run command). This holds for a full initialize too:
    ///   `torc workflows init` resets every job first, so every gate shows up as reset.
    /// - `on_workflow_start` → **keep** on a partial reinitialize (`only_uninitialized = true`): a
    ///   reinitialize is not a new start. A full initialize re-arms it.
    /// - `on_workflow_complete`, `on_worker_start`, `on_worker_complete` → **re-arm**; they recur
    ///   every run.
    pub async fn reset_actions_for_reinitialize(
        &self,
        conn: &mut sqlx::SqliteConnection,
        workflow_id: i64,
        only_uninitialized: bool,
        reset_gate_job_ids: &HashSet<i64>,
    ) -> Result<(), ApiError> {
        debug!(
            "reset_actions_for_reinitialize(workflow_id={}, only_uninitialized={}, reset_gates={})",
            workflow_id,
            only_uninitialized,
            reset_gate_job_ids.len()
        );

        let deleted =
            sqlx::query("DELETE FROM workflow_action WHERE workflow_id = ? AND is_recovery = 1")
                .bind(workflow_id)
                .execute(&mut *conn)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to delete recovery actions"))?
                .rows_affected();
        if deleted > 0 {
            info!(
                "Deleted {} recovery action(s) for workflow_id={}",
                deleted, workflow_id
            );
        }

        let actions = sqlx::query(
            "SELECT id, trigger_type, action_type, job_ids FROM workflow_action WHERE workflow_id = ?",
        )
        .bind(workflow_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| database_error_with_msg(e, "Failed to fetch workflow actions"))?;

        for action_row in actions {
            let action_id: i64 = action_row.get("id");
            let trigger_type: String = action_row.get("trigger_type");
            let action_type: String = action_row.get("action_type");
            let job_ids_str: Option<String> = action_row.get("job_ids");

            let is_job_gated =
                matches!(trigger_type.as_str(), "on_jobs_ready" | "on_jobs_complete");
            let job_ids: Vec<i64> = match job_ids_str.as_deref() {
                Some(s) if is_job_gated => serde_json::from_str(s).unwrap_or_else(|e| {
                    error!(
                        "Failed to parse job_ids JSON '{}' for action {}: {}",
                        s, action_id, e
                    );
                    vec![]
                }),
                _ => vec![],
            };

            // For on_jobs_ready a Ready gate counts; for on_jobs_complete only terminal gates count.
            let trigger_count = match job_ids_str.as_deref() {
                Some(s) if is_job_gated && !job_ids.is_empty() => {
                    count_jobs_in_satisfied_state(&mut *conn, s, &trigger_type).await?
                }
                _ => 0,
            };

            let keep_executed = match trigger_type.as_str() {
                "on_workflow_start" => only_uninitialized,
                "on_jobs_ready" | "on_jobs_complete" => {
                    !job_ids.is_empty() && !job_ids.iter().any(|id| reset_gate_job_ids.contains(id))
                }
                _ => false,
            };

            let query = if keep_executed {
                sqlx::query("UPDATE workflow_action SET trigger_count = ? WHERE id = ?")
            } else {
                sqlx::query(
                    "UPDATE workflow_action SET executed = 0, executed_by = NULL, state = NULL, \
                     error = NULL, trigger_count = ? WHERE id = ?",
                )
            };
            query
                .bind(trigger_count)
                .bind(action_id)
                .execute(&mut *conn)
                .await
                .map_err(|e| {
                    database_error_with_msg(
                        e,
                        "Failed to update workflow action during reinitialize",
                    )
                })?;
            debug!(
                "Reinit action {} (trigger_type={}, action_type={}) workflow_id={}: trigger_count={}, kept_executed={}",
                action_id, trigger_type, action_type, workflow_id, trigger_count, keep_executed
            );
        }

        Ok(())
    }
}

/// Count how many of the jobs in `job_ids_json` (a JSON array) currently satisfy `trigger_type`:
/// for `on_jobs_ready` the job is Ready or terminal (it passed through ready); for
/// `on_jobs_complete` it is terminal. Duplicate ids count once per occurrence.
async fn count_jobs_in_satisfied_state<'e, E>(
    executor: E,
    job_ids_json: &str,
    trigger_type: &str,
) -> Result<i64, ApiError>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let terminal = [
        JobStatus::Completed,
        JobStatus::Failed,
        JobStatus::Canceled,
        JobStatus::Terminated,
    ];
    let statuses: Vec<String> = match trigger_type {
        "on_jobs_ready" => std::iter::once(JobStatus::Ready)
            .chain(terminal)
            .map(|s| s.to_int().to_string())
            .collect(),
        "on_jobs_complete" => terminal.iter().map(|s| s.to_int().to_string()).collect(),
        _ => return Ok(0),
    };
    let sql = format!(
        "SELECT COUNT(*) FROM json_each(?) g JOIN job j ON j.id = g.value WHERE j.status IN ({})",
        statuses.join(",")
    );
    // Only integer status constants are interpolated.
    sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(sql))
        .bind(job_ids_json)
        .fetch_one(executor)
        .await
        .map_err(|e| database_error_with_msg(e, "Failed to count satisfied gate jobs"))
}

/// Current status of every gate job of this workflow's job-gated actions, keyed by job id.
/// `initialize_jobs` reads this before and during its status changes to work out which gates the
/// initialize reset.
pub async fn gate_job_statuses(
    conn: &mut sqlx::SqliteConnection,
    workflow_id: i64,
) -> Result<HashMap<i64, i64>, ApiError> {
    let rows = sqlx::query_as::<_, (i64, i64)>(
        "SELECT DISTINCT j.id, j.status FROM workflow_action a \
         JOIN json_each(a.job_ids) g \
         JOIN job j ON j.id = g.value \
         WHERE a.workflow_id = ? AND a.job_ids IS NOT NULL \
         AND a.trigger_type IN ('on_jobs_ready', 'on_jobs_complete')",
    )
    .bind(workflow_id)
    .fetch_all(conn)
    .await
    .map_err(|e| database_error_with_msg(e, "Failed to read action gate jobs"))?;
    Ok(rows.into_iter().collect())
}

/// Gate jobs this initialize reset: they end `Ready` or `Blocked` (they will become ready in the
/// new run) and either the uninitialize step touched them (`uninitialized_mid`: gates that were
/// `Uninitialized` after it, which includes transitive descendants of reset jobs) or their status
/// changed (`before` vs `after`, e.g. a `Ready` gate pushed back to `Blocked`, or a running gate
/// swept back to `Ready`). An untouched gate, whatever its status, is not reset.
pub fn reset_gate_job_ids(
    before: &HashMap<i64, i64>,
    uninitialized_mid: &HashSet<i64>,
    after: &HashMap<i64, i64>,
) -> HashSet<i64> {
    let runnable = [
        JobStatus::Ready.to_int() as i64,
        JobStatus::Blocked.to_int() as i64,
    ];
    after
        .iter()
        .filter(|(id, status)| {
            runnable.contains(status)
                && (uninitialized_mid.contains(id) || before.get(id) != Some(status))
        })
        .map(|(id, _)| *id)
        .collect()
}
