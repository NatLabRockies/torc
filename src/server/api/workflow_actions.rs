//! Workflow action-related API endpoints

use crate::server::transport_types::context_types::{ApiError, Has, XSpanIdString};
use async_trait::async_trait;
use log::{debug, error, info};
use sqlx::Row;

use crate::server::api_responses::{
    ClaimActionResponse, CreateWorkflowActionResponse, GetPendingActionsResponse,
    GetWorkflowActionsResponse,
};

use crate::models;
use crate::models::JobStatus;

use super::{
    ApiContext, database_error_with_msg, message_error_response, resource_not_found_response,
};

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
            "SELECT id, workflow_id, trigger_type, action_type, action_config, job_ids, trigger_count, required_triggers, executed, executed_at, executed_by, persistent, is_recovery
             FROM workflow_action
             WHERE workflow_id = ?
             ORDER BY id"
        )
        .bind(workflow_id)
        .fetch_all(self.context.pool.as_ref())
        .await;

        match rows {
            Ok(rows) => {
                let actions: Result<Vec<models::WorkflowActionModel>, String> = rows
                    .into_iter()
                    .map(|row| {
                        let action_config_str: String = row.get("action_config");
                        let action_config: serde_json::Value =
                            serde_json::from_str(&action_config_str)
                                .map_err(|e| format!("Failed to parse action_config: {}", e))?;

                        // Deserialize job_ids from JSON string to Vec<i64>
                        let job_ids_str: Option<String> = row.get("job_ids");
                        let job_ids: Option<Vec<i64>> =
                            job_ids_str.and_then(|s| serde_json::from_str(&s).ok());

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
                        })
                    })
                    .collect();

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
                    "SELECT id, workflow_id, trigger_type, action_type, action_config, job_ids, trigger_count, required_triggers, executed, executed_at, executed_by, persistent, is_recovery
                     FROM workflow_action
                     WHERE workflow_id = ? AND trigger_count >= required_triggers AND executed = 0 AND trigger_type IN ({})
                     ORDER BY id",
                    placeholders
                );

                let mut query = sqlx::query(&query_str).bind(workflow_id);
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
            let query_str = "SELECT id, workflow_id, trigger_type, action_type, action_config, job_ids, trigger_count, required_triggers, executed, executed_at, executed_by, persistent, is_recovery
                 FROM workflow_action
                 WHERE workflow_id = ? AND trigger_count >= required_triggers AND executed = 0
                 ORDER BY id".to_string();
            (
                query_str.clone(),
                sqlx::query(&query_str)
                    .bind(workflow_id)
                    .fetch_all(self.context.pool.as_ref())
                    .await,
            )
        };

        let rows = rows;

        match rows {
            Ok(rows) => {
                let actions: Result<Vec<models::WorkflowActionModel>, String> = rows
                    .into_iter()
                    .map(|row| {
                        let action_config_str: String = row.get("action_config");
                        let action_config: serde_json::Value =
                            serde_json::from_str(&action_config_str)
                                .map_err(|e| format!("Failed to parse action_config: {}", e))?;

                        // Deserialize job_ids from JSON string to Vec<i64>
                        let job_ids_str: Option<String> = row.get("job_ids");
                        let job_ids: Option<Vec<i64>> =
                            job_ids_str.and_then(|s| serde_json::from_str(&s).ok());

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
                        })
                    })
                    .collect();

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
            "SELECT workflow_id, executed, persistent FROM workflow_action WHERE id = ?",
        )
        .bind(action_id)
        .fetch_optional(self.context.pool.as_ref())
        .await;

        let is_persistent = match action_check {
            Ok(Some(record)) => {
                let workflow_id_col: i64 = record.get("workflow_id");
                let executed_col: i32 = record.get("executed");
                let persistent_col: i32 = record.get("persistent");

                if workflow_id_col != workflow_id {
                    return Ok(ClaimActionResponse::NotFoundErrorResponse(
                        message_error_response(format!(
                            "Action {} does not belong to workflow {}",
                            action_id, workflow_id
                        )),
                    ));
                }
                // For non-persistent actions, check if already executed
                let is_persistent = persistent_col != 0;
                if !is_persistent && executed_col != 0 {
                    // Action already executed
                    let error_response = models::ErrorResponse::new(serde_json::json!({
                        "message": format!("Action {} already claimed", action_id),
                        "claimed": false
                    }));
                    return Ok(ClaimActionResponse::ConflictResponse(error_response));
                }
                is_persistent
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
        let result = if is_persistent {
            // For persistent actions, just record the execution timestamp
            // Keep executed=0 so other workers can claim it
            // Note: This simple implementation doesn't track which compute nodes have claimed it
            // A more robust implementation would use a separate workflow_action_claims table
            sqlx::query(
                "UPDATE workflow_action
                 SET executed_at = ?
                 WHERE id = ?",
            )
            .bind(&now)
            .bind(action_id)
            .execute(self.context.pool.as_ref())
            .await
        } else {
            sqlx::query(
                "UPDATE workflow_action
                 SET executed = 1, executed_at = ?, executed_by = ?
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
                    // Race condition: action was claimed by another node between check and update
                    let error_response = models::ErrorResponse::new(serde_json::json!({
                        "message": format!("Action {} already claimed", action_id),
                        "claimed": false
                    }));
                    Ok(ClaimActionResponse::ConflictResponse(error_response))
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
}

/// Helper methods for workflow actions (not part of the trait)
impl WorkflowActionsApiImpl {
    /// Check and trigger workflow actions based on trigger type and job state changes
    /// This is called by other API endpoints when state changes occur
    pub async fn check_and_trigger_actions(
        &self,
        workflow_id: i64,
        trigger_type: &str,
        job_ids: Option<Vec<i64>>,
    ) -> Result<(), ApiError> {
        debug!(
            "check_and_trigger_actions(workflow_id={}, trigger_type={}, job_ids={:?})",
            workflow_id, trigger_type, job_ids
        );

        // Get all actions of this trigger type for this workflow that haven't reached required triggers yet
        let actions = match sqlx::query(
            "SELECT id, job_ids, trigger_count, required_triggers FROM workflow_action
             WHERE workflow_id = ? AND trigger_type = ? AND trigger_count < required_triggers",
        )
        .bind(workflow_id)
        .bind(trigger_type)
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
                                if self
                                    .check_jobs_state(workflow_id, &action_job_ids, trigger_type)
                                    .await?
                                {
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
        workflow_id: i64,
        job_ids: &[i64],
        trigger_type: &str,
    ) -> Result<bool, ApiError> {
        Ok(self
            .count_jobs_in_satisfied_state(workflow_id, job_ids, trigger_type)
            .await?
            == job_ids.len() as i64)
    }

    /// Count how many jobs currently satisfy the condition for the trigger type.
    /// This is used to properly set trigger_count after reinitialize, when some jobs
    /// may already be in a satisfied state (e.g., job2 is already Completed when job1
    /// transitions to Ready after reinitialize).
    async fn count_jobs_in_satisfied_state(
        &self,
        workflow_id: i64,
        job_ids: &[i64],
        trigger_type: &str,
    ) -> Result<i64, ApiError> {
        let mut count = 0i64;

        for job_id in job_ids {
            let job_status =
                match sqlx::query_scalar::<_, i64>("SELECT status FROM job WHERE id = ?")
                    .bind(job_id)
                    .fetch_optional(self.context.pool.as_ref())
                    .await
                {
                    Ok(Some(status)) => status,
                    Ok(None) => {
                        debug!("job_id={} not found in workflow_id={}", job_id, workflow_id);
                        continue;
                    }
                    Err(e) => {
                        return Err(database_error_with_msg(e, "Failed to fetch job status"));
                    }
                };

            // Check if job meets the required state
            // For on_jobs_ready: job must be Ready OR have already completed (passed through ready)
            // For on_jobs_complete: job must be in a terminal state
            let meets_condition = match trigger_type {
                "on_jobs_ready" => {
                    job_status == JobStatus::Ready.to_int() as i64
                        || job_status == JobStatus::Completed.to_int() as i64
                        || job_status == JobStatus::Failed.to_int() as i64
                        || job_status == JobStatus::Canceled.to_int() as i64
                        || job_status == JobStatus::Terminated.to_int() as i64
                }
                "on_jobs_complete" => {
                    job_status == JobStatus::Completed.to_int() as i64
                        || job_status == JobStatus::Failed.to_int() as i64
                        || job_status == JobStatus::Canceled.to_int() as i64
                        || job_status == JobStatus::Terminated.to_int() as i64
                }
                _ => false,
            };

            if meets_condition {
                count += 1;
            }
        }

        Ok(count)
    }

    /// Reset workflow actions for reinitialization.
    ///
    /// First deletes any recovery actions (created by `torc slurm regenerate`), then, for each
    /// remaining (non-recovery) action, recomputes `trigger_count` from current job states and
    /// decides whether to **re-arm** the action (clear `executed`/`executed_by` so it can fire again
    /// on the new run) or **keep** its `executed` flag (leave it suppressed). For on_jobs_ready and
    /// on_jobs_complete actions, trigger_count is set to the number of jobs already in a satisfied
    /// state (e.g., Completed jobs count toward on_jobs_ready); for other action types it is 0.
    ///
    /// The keep-vs-re-arm decision follows one rule, independent of `action_type`: **re-arm an
    /// action iff its triggering condition will genuinely re-occur in the new run.** A user who
    /// reinitializes after resetting a subset of jobs expects only those jobs to re-run; actions tied
    /// to events that are not happening again must not re-fire (a re-fired action re-submits its
    /// Slurm allocation or re-runs its commands — a duplicate side effect). Per trigger:
    ///
    /// - `on_workflow_start` → **keep**. The workflow starts exactly once in its lifetime; a
    ///   reinitialize is not a new start. (A never-fired action keeps `executed = 0` and still fires,
    ///   so first-time initialize is unaffected.) This is what stops plain `reinit` /
    ///   `reset-status --reinit` from re-running start-time setup or re-submitting the original node
    ///   count.
    /// - `on_jobs_ready` / `on_jobs_complete` → **keep iff the gating jobs are still all terminal**
    ///   (the *subset* re-run: gates untouched, the event already happened and is not recurring).
    ///   Re-arm when any gate was reset (the *full* re-run: that job will run again, so its action
    ///   should fire again). "Still terminal" is measured with the on_jobs_complete notion (terminal
    ///   jobs only: completed/failed/canceled/terminated), NOT `trigger_count`: a reset
    ///   `on_jobs_ready` gate returns to `Ready`, and `Ready` satisfies `on_jobs_ready`, so
    ///   `trigger_count` cannot tell a freshly-reset gate from one that already completed — using it
    ///   would wrongly suppress the action on a full re-run and stall the workflow.
    /// - `on_workflow_complete`, `on_worker_start`, `on_worker_complete` → **re-arm**. These recur
    ///   every run (the workflow will complete again; new workers start), so they should fire again.
    ///
    /// The keep logic above applies only to a **partial** reinitialize (`only_uninitialized = true`:
    /// `reinit`, `reset-status --reinit`, `recover`, `regenerate`, `watch`), which is the case that
    /// must not resurrect already-satisfied actions. A **full** initialize (`only_uninitialized =
    /// false`, i.e. `torc workflows init`, which first resets every job to uninitialized) is a clean
    /// slate: every action is re-armed, including `on_workflow_start`. Otherwise re-running
    /// `workflows init` on a workflow that already ran would leave its `on_workflow_start` actions
    /// suppressed forever.
    pub async fn reset_actions_for_reinitialize(
        &self,
        workflow_id: i64,
        only_uninitialized: bool,
    ) -> Result<(), ApiError> {
        debug!(
            "reset_actions_for_reinitialize(workflow_id={}, only_uninitialized={})",
            workflow_id, only_uninitialized
        );

        // First, delete all recovery actions (ephemeral actions created during recovery)
        match sqlx::query("DELETE FROM workflow_action WHERE workflow_id = ? AND is_recovery = 1")
            .bind(workflow_id)
            .execute(self.context.pool.as_ref())
            .await
        {
            Ok(result) => {
                let deleted = result.rows_affected();
                if deleted > 0 {
                    info!(
                        "Deleted {} recovery action(s) for workflow {}",
                        deleted, workflow_id
                    );
                }
            }
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to delete recovery actions",
                ));
            }
        }

        // Get all remaining (non-recovery) actions. We decide per-action whether to re-arm, so
        // there is no blanket "clear executed" step: an already-satisfied job-gated schedule_nodes
        // action must keep its executed flag.
        let actions = match sqlx::query(
            "SELECT id, trigger_type, action_type, job_ids, required_triggers FROM workflow_action WHERE workflow_id = ?",
        )
        .bind(workflow_id)
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(actions) => actions,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to fetch workflow actions",
                ));
            }
        };

        for action_row in actions {
            let action_id: i64 = action_row.get("id");
            let trigger_type: String = action_row.get("trigger_type");
            let action_type: String = action_row.get("action_type");
            let job_ids_str: Option<String> = action_row.get("job_ids");
            let required_triggers: i64 = action_row.get("required_triggers");

            let is_job_gated =
                matches!(trigger_type.as_str(), "on_jobs_ready" | "on_jobs_complete");

            // Parse the gating job_ids once (job-gated triggers only).
            let job_ids: Vec<i64> = match job_ids_str.as_deref() {
                Some(s) if is_job_gated => serde_json::from_str(s).unwrap_or_else(|e| {
                    error!(
                        "Failed to parse job_ids JSON '{}' for action {}: {}",
                        s, action_id, e
                    );
                    vec![]
                }),
                // Other trigger types (on_workflow_start, etc.) have no gating jobs.
                _ => vec![],
            };

            // trigger_count drives whether the action becomes pending again on the new run. For
            // on_jobs_ready a Ready gate counts; for on_jobs_complete only terminal gates count.
            let trigger_count = if is_job_gated && !job_ids.is_empty() {
                self.count_jobs_in_satisfied_state(workflow_id, &job_ids, &trigger_type)
                    .await?
            } else {
                0
            };

            // Decide whether to re-arm (clear `executed` so the action can fire again on the new
            // run) or keep the `executed` flag (leave it suppressed). Guiding rule: re-arm an action
            // iff its triggering condition will genuinely re-occur in the new run. "Keep" only
            // preserves the flag -- a never-fired action keeps `executed = 0` and stays fireable. See
            // the function doc comment for the full rationale and the per-trigger reasoning.
            //
            // A full initialize (only_uninitialized = false, e.g. `torc workflows init`) is a clean
            // slate: it reset every job, so re-arm every action (never keep). The per-trigger keep
            // logic below applies only to a partial reinitialize.
            let keep_executed = only_uninitialized
                && match trigger_type.as_str() {
                    // The workflow starts exactly once per run-from-the-top; a *partial* reinitialize
                    // is NOT a new start. Keep on_workflow_start actions suppressed so reinit never
                    // re-runs start-time setup (run_commands) or re-submits start-time allocations
                    // (schedule_nodes).
                    "on_workflow_start" => true,

                    // Job-gated: the condition re-occurs iff the gating jobs will run again, i.e. they
                    // are no longer all in a terminal state. Measured with the terminal-state
                    // (on_jobs_complete) notion for BOTH triggers, NOT trigger_count: a reset
                    // on_jobs_ready gate returns to Ready, and Ready would spuriously satisfy
                    // on_jobs_ready and wrongly keep the action suppressed on a full re-run. This is
                    // action-type-agnostic on purpose -- the hazard is a duplicate side effect (a
                    // re-submitted allocation, a re-run command), not anything specific to
                    // schedule_nodes.
                    "on_jobs_ready" | "on_jobs_complete" => {
                        if job_ids.is_empty() {
                            false
                        } else {
                            let already_ran = self
                                .count_jobs_in_satisfied_state(
                                    workflow_id,
                                    &job_ids,
                                    "on_jobs_complete",
                                )
                                .await?;
                            already_ran >= required_triggers
                        }
                    }

                    // on_workflow_complete (the workflow will complete again once the re-run finishes)
                    // and on_worker_start / on_worker_complete (new workers run in the new run)
                    // genuinely recur each run, so they are always re-armed.
                    _ => false,
                };

            let result = if keep_executed {
                // Leave executed/executed_by intact; only refresh trigger_count.
                sqlx::query("UPDATE workflow_action SET trigger_count = ? WHERE id = ?")
                    .bind(trigger_count)
                    .bind(action_id)
                    .execute(self.context.pool.as_ref())
                    .await
            } else {
                sqlx::query(
                    "UPDATE workflow_action SET executed = 0, executed_by = NULL, trigger_count = ? WHERE id = ?",
                )
                .bind(trigger_count)
                .bind(action_id)
                .execute(self.context.pool.as_ref())
                .await
            };

            match result {
                Ok(_) => {
                    debug!(
                        "Reinit action {} (trigger_type={}, action_type={}) in workflow {}: trigger_count={}, kept_executed={}",
                        action_id,
                        trigger_type,
                        action_type,
                        workflow_id,
                        trigger_count,
                        keep_executed
                    );
                }
                Err(e) => {
                    // Propagate rather than log-and-continue: a swallowed failure here would
                    // leave the workflow partially reset (some actions still executed, some with a
                    // stale trigger_count) while the caller believes reinitialize succeeded. The
                    // recompute is idempotent, so a failed reinit can be safely retried. This
                    // matches the DELETE/SELECT steps above and the executed-clear in the previous
                    // implementation.
                    error!(
                        "Failed to update action {} during reinitialize: {}",
                        action_id, e
                    );
                    return Err(database_error_with_msg(
                        e,
                        "Failed to update workflow action during reinitialize",
                    ));
                }
            }
        }

        Ok(())
    }
}
