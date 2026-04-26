//! Main library entry point for torc implementation.

use crate::MAX_RECORD_TRANSFER_COUNT;
use crate::models;
use crate::server::authorization::{AccessCheckResult, AuthorizationService};
use crate::server::event_broadcast::{BroadcastEvent, EventBroadcaster};
use crate::server::htpasswd::HtpasswdFile;
use crate::server::transport_types::auth_types::Authorization;
use crate::server::transport_types::context_types::{Has, XSpanIdString};
use async_trait::async_trait;
use log::{debug, error, info, warn};
use sqlx::Row;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tracing::instrument;

use sqlx::sqlite::SqlitePool;

const TORC_VERSION: &str = env!("CARGO_PKG_VERSION");
const GIT_HASH: &str = env!("GIT_HASH");

/// Returns the full version string including git hash (e.g., "0.8.0 (abc1234)")
fn full_version() -> String {
    format!("{} ({})", TORC_VERSION, GIT_HASH)
}

#[derive(Debug)]
enum CreateTaskError {
    /// An async task is already active and the new request is incompatible with it.
    /// Reasons: a different operation is in-flight, or the same operation was requested
    /// with different parameters.
    Conflict {
        existing_task_id: i64,
        existing_operation: String,
        reason: String,
    },
    Api(ApiError),
}

/// Result of `create_or_get_initialize_jobs_task`.
pub(super) enum TaskCreation {
    /// A new task was inserted; the caller must spawn the background work.
    Created(models::TaskModel),
    /// An identical task is already active; returned idempotently. No work spawned.
    Existing(models::TaskModel),
}
macro_rules! forbidden_error {
    ($reason:expr) => {
        models::ErrorResponse::new(serde_json::json!({
            "error": "Forbidden",
            "message": $reason
        }))
    };
}

macro_rules! not_found_error {
    ($reason:expr) => {
        models::ErrorResponse::new(serde_json::json!({
            "error": "NotFound",
            "message": $reason
        }))
    };
}

macro_rules! authorize_workflow {
    ($self:ident, $workflow_id:expr, $context:expr, $response_enum:ident) => {
        match $self
            .check_workflow_access_for_context($workflow_id, $context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok($response_enum::ForbiddenErrorResponse(forbidden_error!(
                    reason
                )));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok($response_enum::NotFoundErrorResponse(not_found_error!(
                    reason
                )));
            }
            AccessCheckResult::InternalError(reason) => {
                return Err(ApiError(reason));
            }
        }
    };
}

macro_rules! authorize_resource {
    ($self:ident, $resource_id:expr, $table_name:expr, $context:expr, $response_enum:ident) => {
        match $self
            .check_resource_access_for_context($resource_id, $table_name, $context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok($response_enum::ForbiddenErrorResponse(forbidden_error!(
                    reason
                )));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok($response_enum::NotFoundErrorResponse(not_found_error!(
                    reason
                )));
            }
            AccessCheckResult::InternalError(reason) => {
                return Err(ApiError(reason));
            }
        }
    };
}

macro_rules! authorize_job {
    ($self:ident, $job_id:expr, $context:expr, $response_enum:ident) => {
        match $self.check_job_access_for_context($job_id, $context).await {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok($response_enum::ForbiddenErrorResponse(forbidden_error!(
                    reason
                )));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok($response_enum::NotFoundErrorResponse(not_found_error!(
                    reason
                )));
            }
            AccessCheckResult::InternalError(reason) => {
                return Err(ApiError(reason));
            }
        }
    };
}

macro_rules! authorize_group_admin {
    ($self:ident, $group_id:expr, $context:expr, $response_enum:ident) => {
        match $self
            .check_group_admin_access_for_context($group_id, $context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok($response_enum::ForbiddenErrorResponse(forbidden_error!(
                    reason
                )));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok($response_enum::NotFoundErrorResponse(not_found_error!(
                    reason
                )));
            }
            AccessCheckResult::InternalError(reason) => {
                return Err(ApiError(reason));
            }
        }
    };
}

macro_rules! authorize_admin {
    ($self:ident, $context:expr, $response_enum:ident) => {
        match $self.check_admin_access_for_context($context).await {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok($response_enum::ForbiddenErrorResponse(forbidden_error!(
                    reason
                )));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok($response_enum::NotFoundErrorResponse(not_found_error!(
                    reason
                )));
            }
            AccessCheckResult::InternalError(reason) => {
                return Err(ApiError(reason));
            }
        }
    };
}

macro_rules! authorize_workflow_group {
    ($self:ident, $workflow_id:expr, $group_id:expr, $context:expr, $response_enum:ident) => {
        match $self
            .check_workflow_group_access_for_context($workflow_id, $group_id, $context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok($response_enum::ForbiddenErrorResponse(forbidden_error!(
                    reason
                )));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok($response_enum::NotFoundErrorResponse(not_found_error!(
                    reason
                )));
            }
            AccessCheckResult::InternalError(reason) => {
                return Err(ApiError(reason));
            }
        }
    };
}

mod access_checks;
mod access_control_transport;
mod bootstrap;
mod compute_nodes_transport;
mod files_transport;
mod jobs_transport;
mod lifecycle_support;
mod local_schedulers_transport;
mod remote_workers_transport;
mod resource_requirements_transport;
mod results_transport;
mod ro_crate_transport;
mod runtime_support;
mod scheduled_compute_nodes_transport;
mod slurm_schedulers_transport;
mod slurm_stats_transport;
mod system_transport;
mod unblock_processing;
mod user_data_transport;
mod workflows_transport;

/// Process optional offset and limit parameters and return concrete values.
/// Returns (offset, limit) where:
/// - offset defaults to 0 if not provided
/// - limit defaults to [`MAX_RECORD_TRANSFER_COUNT`] if not provided
/// - Returns an error if limit exceeds [`MAX_RECORD_TRANSFER_COUNT`]
fn process_pagination_params(
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<(i64, i64), ApiError> {
    let processed_offset = offset.unwrap_or(0);
    let processed_limit = limit.unwrap_or(MAX_RECORD_TRANSFER_COUNT);

    if processed_limit > MAX_RECORD_TRANSFER_COUNT {
        error!(
            "Limit exceeds maximum allowed value: {} > {}",
            processed_limit, MAX_RECORD_TRANSFER_COUNT
        );
        return Err(ApiError(format!(
            "Limit cannot exceed {}",
            MAX_RECORD_TRANSFER_COUNT
        )));
    }

    Ok((processed_offset, processed_limit))
}

/// Sync the admin group with configured admin users
///
/// Creates the "admin" system group if it doesn't exist and ensures
/// all configured admin users are members with admin role.
/// Creates and starts the HTTP(S) server.
///
/// When `https` is true, `tls_cert` and `tls_key` must provide paths to the
/// TLS certificate chain and private key files (PEM format).
///
/// Returns the actual port the server bound to (useful when port 0 is specified for auto-detection).
#[allow(clippy::too_many_arguments)]
pub async fn create(
    addr: &str,
    https: bool,
    pool: SqlitePool,
    htpasswd: Option<HtpasswdFile>,
    require_auth: bool,
    credential_cache_ttl_secs: u64,
    enforce_access_control: bool,
    completion_check_interval_secs: f64,
    admin_users: Vec<String>,
    #[allow(unused_variables)] tls_cert: Option<String>,
    #[allow(unused_variables)] tls_key: Option<String>,
    auth_file_path: Option<String>,
    shutdown_on_stdin_eof: bool,
) -> u16 {
    bootstrap::create_server(
        addr,
        https,
        pool,
        htpasswd,
        require_auth,
        credential_cache_ttl_secs,
        enforce_access_control,
        completion_check_interval_secs,
        admin_users,
        tls_cert,
        tls_key,
        auth_file_path,
        shutdown_on_stdin_eof,
    )
    .await
}

pub struct Server<C> {
    marker: PhantomData<C>,
    shared: Arc<LiveServerState>,
}

impl<C> Clone for Server<C> {
    fn clone(&self) -> Self {
        Self {
            marker: PhantomData,
            shared: self.shared.clone(),
        }
    }
}

impl<C> Deref for Server<C> {
    type Target = LiveServerState;

    fn deref(&self) -> &Self::Target {
        self.shared.as_ref()
    }
}

impl<C> Server<C> {
    pub fn new(
        pool: SqlitePool,
        enforce_access_control: bool,
        htpasswd: crate::server::auth::SharedHtpasswd,
        auth_file_path: Option<String>,
        credential_cache: crate::server::auth::SharedCredentialCache,
    ) -> Self {
        Server {
            marker: PhantomData,
            shared: Arc::new(LiveServerState::new(
                pool,
                enforce_access_control,
                htpasswd,
                auth_file_path,
                credential_cache,
            )),
        }
    }

    /// Signal that a job has completed. This wakes up the background unblock task.
    fn signal_job_completion(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(1);
        self.last_completion_time.store(now, Ordering::Release);
    }

    /// Get a reference to the event broadcaster for SSE subscriptions.
    pub fn get_event_broadcaster(&self) -> &EventBroadcaster {
        &self.event_broadcaster
    }

    pub fn shared_state(&self) -> Arc<LiveServerState> {
        self.shared.clone()
    }

    fn now_ms() -> i64 {
        chrono::Utc::now().timestamp_millis()
    }

    /// Load the single active async task for a workflow, if any.
    pub(super) async fn get_active_task(
        &self,
        workflow_id: i64,
    ) -> Result<Option<models::TaskModel>, ApiError> {
        Ok(self
            .get_active_task_with_request(workflow_id)
            .await?
            .map(|(task, _)| task))
    }

    /// Like `get_active_task` but also returns the serialized request parameters that
    /// created the task, so callers can detect parameter mismatches on idempotent returns.
    async fn get_active_task_with_request(
        &self,
        workflow_id: i64,
    ) -> Result<Option<(models::TaskModel, Option<String>)>, ApiError> {
        let row = sqlx::query(
            r#"
            SELECT id, workflow_id, operation, status, created_at_ms, started_at_ms, finished_at_ms, request_json
            FROM async_handle
            WHERE workflow_id = ?1
              AND status IN ('queued', 'running')
            ORDER BY id DESC
            LIMIT 1
            "#,
        )
        .bind(workflow_id)
        .fetch_optional(self.pool.as_ref())
        .await
        .map_err(|e| ApiError(format!("Database error: {}", e)))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let status = row
            .get::<String, _>("status")
            .parse::<models::TaskStatus>()
            .map_err(|e| {
                error!(
                    "Invalid async_handle.status for workflow_id={}: {}",
                    workflow_id, e
                );
                ApiError("Database error".to_string())
            })?;

        let task = models::TaskModel {
            id: row.get("id"),
            workflow_id: row.get("workflow_id"),
            operation: row.get("operation"),
            status,
            created_at_ms: row.get("created_at_ms"),
            started_at_ms: row.get("started_at_ms"),
            finished_at_ms: row.get("finished_at_ms"),
            error: None,
        };
        Ok(Some((task, row.get("request_json"))))
    }

    async fn create_or_get_initialize_jobs_task(
        &self,
        workflow_id: i64,
        only_uninitialized: Option<bool>,
        clear_ephemeral_user_data: Option<bool>,
        requested_by: Option<String>,
    ) -> Result<TaskCreation, CreateTaskError> {
        let new_params = serde_json::json!({
            "only_uninitialized": only_uninitialized,
            "clear_ephemeral_user_data": clear_ephemeral_user_data,
        });
        let request_json = new_params.to_string();

        // Retry once to close a narrow race: if the conflicting task transitions out of
        // (queued, running) between our failed INSERT and the follow-up SELECT, the partial
        // unique index no longer blocks us and the second INSERT will succeed.
        for attempt in 0..2 {
            // Capture created_at_ms once per attempt so the persisted row and the returned
            // TaskModel agree.
            let created_at_ms = Self::now_ms();
            let insert_result = sqlx::query(
                r#"
                INSERT INTO async_handle
                  (workflow_id, operation, status, created_at_ms, requested_by, request_json)
                VALUES
                  (?1, 'initialize_jobs', 'queued', ?2, ?3, ?4)
                "#,
            )
            .bind(workflow_id)
            .bind(created_at_ms)
            .bind(requested_by.clone())
            .bind(&request_json)
            .execute(self.pool.as_ref())
            .await;

            match insert_result {
                Ok(result) => {
                    return Ok(TaskCreation::Created(models::TaskModel::new(
                        result.last_insert_rowid(),
                        workflow_id,
                        "initialize_jobs".to_string(),
                        models::TaskStatus::Queued,
                        created_at_ms,
                    )));
                }
                Err(e) if Self::is_sqlite_unique_constraint(&e) => {
                    // Another active task exists for this workflow. Return it idempotently only
                    // if it is the same operation AND was started with the same parameters —
                    // otherwise the new caller would silently get someone else's task.
                    let Some((existing, existing_request_json)) = self
                        .get_active_task_with_request(workflow_id)
                        .await
                        .map_err(CreateTaskError::Api)?
                    else {
                        // The conflicting task finished between INSERT failure and this SELECT.
                        // Retry the INSERT once; if it still fails there's a deeper problem.
                        if attempt == 0 {
                            continue;
                        }
                        return Err(CreateTaskError::Api(ApiError(
                            "unique constraint violated but no active task found after retry"
                                .to_string(),
                        )));
                    };

                    if existing.operation != "initialize_jobs" {
                        return Err(CreateTaskError::Conflict {
                            existing_task_id: existing.id,
                            existing_operation: existing.operation,
                            reason: "different async operation is already active".to_string(),
                        });
                    }

                    let existing_params = existing_request_json
                        .as_deref()
                        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                        .unwrap_or(serde_json::Value::Null);
                    if existing_params != new_params {
                        return Err(CreateTaskError::Conflict {
                            existing_task_id: existing.id,
                            existing_operation: existing.operation,
                            reason: format!(
                                "initialize_jobs task already active with different parameters: existing={}, requested={}",
                                existing_params, new_params
                            ),
                        });
                    }

                    return Ok(TaskCreation::Existing(existing));
                }
                Err(e) => {
                    return Err(CreateTaskError::Api(ApiError(format!(
                        "Database error: {}",
                        e
                    ))));
                }
            }
        }

        // Loop can only fall through if both attempts hit the "no active task after unique
        // constraint" branch, which is already handled above. Keep this as a belt-and-suspenders
        // guard so the function always returns explicitly.
        Err(CreateTaskError::Api(ApiError(
            "create_or_get_initialize_jobs_task: exhausted retries".to_string(),
        )))
    }

    fn is_sqlite_unique_constraint(err: &sqlx::Error) -> bool {
        let sqlx::Error::Database(db_err) = err else {
            return false;
        };

        // SQLite extended error code for UNIQUE constraint is typically 2067.
        // Prefer matching on code when available, but also match message for robustness.
        if let Some(code) = db_err.code()
            && (code == "2067" || code == "1555")
        {
            return true;
        }

        db_err.message().contains("UNIQUE constraint failed")
    }

    async fn update_task_running(&self, task_id: i64) -> Result<(), ApiError> {
        sqlx::query(
            r#"
            UPDATE async_handle
            SET status = 'running', started_at_ms = ?2
            WHERE id = ?1
            "#,
        )
        .bind(task_id)
        .bind(Self::now_ms())
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| ApiError(format!("Database error: {}", e)))?;

        Ok(())
    }

    async fn update_task_finished(
        &self,
        task_id: i64,
        succeeded: bool,
        error: Option<String>,
    ) -> Result<(), ApiError> {
        let (status, result_json) = if succeeded {
            (
                "succeeded",
                Some(serde_json::json!({"message": "initialize_jobs completed"}).to_string()),
            )
        } else {
            ("failed", None)
        };

        sqlx::query(
            r#"
            UPDATE async_handle
            SET status = ?2,
                finished_at_ms = ?3,
                result_json = ?4,
                error = ?5
            WHERE id = ?1
            "#,
        )
        .bind(task_id)
        .bind(status)
        .bind(Self::now_ms())
        .bind(result_json)
        .bind(error)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| ApiError(format!("Database error: {}", e)))?;

        Ok(())
    }

    async fn run_initialize_jobs_task(
        &self,
        task_id: i64,
        workflow_id: i64,
        only_uninitialized: Option<bool>,
        clear_ephemeral_user_data: Option<bool>,
        requested_by: String,
    ) {
        if let Err(e) = self.update_task_running(task_id).await {
            error!("Failed to mark task {} running: {}", task_id, e);
        }

        let result = self
            .initialize_jobs_core(
                workflow_id,
                only_uninitialized,
                clear_ephemeral_user_data,
                requested_by.clone(),
            )
            .await;

        match result {
            Ok(()) => {
                if let Err(e) = self.update_task_finished(task_id, true, None).await {
                    error!("Failed to mark task {} succeeded: {}", task_id, e);
                }

                self.event_broadcaster.broadcast(BroadcastEvent {
                    workflow_id,
                    timestamp: Self::now_ms(),
                    event_type: "task_completed".to_string(),
                    severity: models::EventSeverity::Info,
                    data: serde_json::json!({
                        "category": "tasks",
                        "type": "task_completed",
                        "task_id": task_id,
                        "operation": "initialize_jobs",
                        "status": "succeeded",
                        "user": requested_by,
                        "message": format!("task {} succeeded", task_id),
                    }),
                });
            }
            Err(e) => {
                let error_msg = e.0;
                if let Err(e) = self
                    .update_task_finished(task_id, false, Some(error_msg.clone()))
                    .await
                {
                    error!("Failed to mark task {} failed: {}", task_id, e);
                }

                self.event_broadcaster.broadcast(BroadcastEvent {
                    workflow_id,
                    timestamp: Self::now_ms(),
                    event_type: "task_completed".to_string(),
                    severity: models::EventSeverity::Error,
                    data: serde_json::json!({
                        "category": "tasks",
                        "type": "task_completed",
                        "task_id": task_id,
                        "operation": "initialize_jobs",
                        "status": "failed",
                        "user": requested_by,
                        "error": error_msg,
                        "message": format!("task {} failed", task_id),
                    }),
                });
            }
        }
    }

    async fn initialize_jobs_core(
        &self,
        id: i64,
        only_uninitialized: Option<bool>,
        clear_ephemeral_user_data: Option<bool>,
        username: String,
    ) -> Result<(), ApiError> {
        // Clear in-memory failure tracking for this workflow when (re)initializing
        if let Ok(mut set) = self.workflows_with_failures.write() {
            set.remove(&id);
        }

        // Begin a transaction to ensure all initialization steps are atomic
        let mut tx = match self.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                error!("Failed to begin transaction for initialize_jobs: {}", e);
                return Err(ApiError("Database error".to_string()));
            }
        };

        // Step 1: Add depends-on associations based on file dependencies
        if let Err(e) = self
            .add_depends_on_associations_from_files(&mut *tx, id)
            .await
        {
            error!("Failed to add depends-on associations from files: {}", e);
            let _ = tx.rollback().await;
            return Err(e);
        }

        // Step 1b: Add depends-on associations from user_data
        if let Err(e) = self
            .add_depends_on_associations_from_user_data(&mut *tx, id)
            .await
        {
            error!(
                "Failed to add depends-on associations from user_data: {}",
                e
            );
            let _ = tx.rollback().await;
            return Err(e);
        }

        // Step 2: Uninitialize blocked jobs (only needed during reinitialization)
        // This is skipped during initial workflow start because Step 3 will set all job statuses anyway.
        // During reinitialization, this ensures jobs transitively blocked by reset jobs are also reset.
        let only_uninit = only_uninitialized.unwrap_or(false);
        if only_uninit && let Err(e) = self.uninitialize_blocked_jobs(&mut *tx, id).await {
            error!("Failed to uninitialize blocked jobs: {}", e);
            let _ = tx.rollback().await;
            return Err(e);
        }

        // Step 3: Initialize blocked jobs to blocked status
        if let Err(e) = self
            .initialize_blocked_jobs_to_blocked(&mut *tx, id, only_uninit)
            .await
        {
            error!("Failed to initialize blocked jobs to blocked: {}", e);
            let _ = tx.rollback().await;
            return Err(e);
        }

        // Step 4: Initialize unblocked jobs to ready status
        if let Err(e) = self.initialize_unblocked_jobs(&mut *tx, id).await {
            error!("Failed to initialize unblocked jobs: {}", e);
            let _ = tx.rollback().await;
            return Err(e);
        }

        // TODO: helper function
        // Step 5: Delete workflow_result records for jobs that are not complete
        // This is done after steps 1-4 to be future-proof in case those steps reset job completion statuses
        // Complete statuses are: Completed (5), Failed (6), Canceled (7), Terminated (8)
        let completed_status = models::JobStatus::Completed.to_int();
        let failed_status = models::JobStatus::Failed.to_int();
        let canceled_status = models::JobStatus::Canceled.to_int();
        let terminated_status = models::JobStatus::Terminated.to_int();

        match sqlx::query!(
            r#"
            DELETE FROM workflow_result
            WHERE workflow_id = $1
            AND job_id IN (
                SELECT id FROM job
                WHERE workflow_id = $1
                AND status NOT IN ($2, $3, $4, $5)
            )
            "#,
            id,
            completed_status,
            failed_status,
            canceled_status,
            terminated_status
        )
        .execute(&mut *tx)
        .await
        {
            Ok(result) => {
                debug!(
                    "Deleted {} workflow_result records for incomplete jobs in workflow {}",
                    result.rows_affected(),
                    id
                );
            }
            Err(e) => {
                error!("Database error deleting workflow_result records: {}", e);
                let _ = tx.rollback().await;
                return Err(ApiError("Database error".to_string()));
            }
        }

        // This endpoint currently accepts forward-compatible parameters that may be unused.
        let _ = clear_ephemeral_user_data;

        // Commit the transaction
        // Hash computation must happen AFTER this commit so that compute_job_input_hash
        // can see the job_depends_on relationships that were inserted in this transaction.
        if let Err(e) = tx.commit().await {
            error!("Failed to commit transaction for initialize_jobs: {}", e);
            return Err(ApiError("Database error".to_string()));
        }

        // Compute and store input hashes for all jobs in the workflow.
        // IMPORTANT: This must happen after the transaction commits so the hash computation sees
        // the committed job_depends_on relationships.
        self.jobs_api.compute_and_store_all_input_hashes(id).await?;

        match sqlx::query!("SELECT enable_ro_crate FROM workflow WHERE id = ?", id)
            .fetch_optional(self.pool.as_ref())
            .await
        {
            Ok(Some(row)) if row.enable_ro_crate == Some(1) => {
                debug!(
                    "enable_ro_crate is true for workflow {}, creating input file entities",
                    id
                );
                if let Err(e) = self.ro_crate_api.create_entities_for_input_files(id).await {
                    // Non-blocking: log warning but don't fail initialization
                    warn!("Failed to create RO-Crate entities for input files: {}", e);
                }
            }
            Ok(_) => {}
            Err(e) => {
                // Non-blocking: log warning but don't fail initialization
                warn!("Failed to check enable_ro_crate flag: {}", e);
            }
        }

        // Always create SoftwareApplication entity for torc-server
        if let Err(e) = self.ro_crate_api.create_server_software_entity(id).await {
            warn!("Failed to create torc-server software entity: {}", e);
        }

        debug!(
            "Successfully initialized jobs for workflow {} with transaction",
            id
        );

        // Reset workflow actions for reinitialization
        if let Err(e) = self
            .workflow_actions_api
            .reset_actions_for_reinitialize(id)
            .await
        {
            error!(
                "Failed to reset workflow actions for workflow {}: {}",
                id, e
            );
        }

        // Activate on_workflow_start actions (workflow has started with initialization)
        if let Err(e) = self
            .workflow_actions_api
            .check_and_trigger_actions(id, "on_workflow_start", None)
            .await
        {
            error!(
                "Failed to check_and_trigger_actions for on_workflow_start: {}",
                e
            );
        }

        // Activate on_worker_start and on_worker_complete actions immediately
        for trigger_type in &["on_worker_start", "on_worker_complete"] {
            match sqlx::query(
                "UPDATE workflow_action SET trigger_count = required_triggers WHERE workflow_id = ? AND trigger_type = ?"
            )
            .bind(id)
            .bind(trigger_type)
            .execute(self.pool.as_ref())
            .await
            {
                Ok(result) => {
                    let count = result.rows_affected();
                    if count > 0 {
                        debug!("Activated {} {} actions for workflow {}", count, trigger_type, id);
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to activate {} actions for workflow {}: {}",
                        trigger_type, id, e
                    );
                }
            }
        }

        // Check if any on_jobs_ready actions should be triggered based on job states
        if let Err(e) = self
            .workflow_actions_api
            .check_and_trigger_actions(id, "on_jobs_ready", None)
            .await
        {
            error!(
                "Failed to check_and_trigger_actions for on_jobs_ready: {}",
                e
            );
        }

        // Broadcast SSE event for workflow initialization
        let event_type = if only_uninitialized.unwrap_or(false) {
            "workflow_started"
        } else {
            "workflow_reinitialized"
        };

        self.event_broadcaster.broadcast(BroadcastEvent {
            workflow_id: id,
            timestamp: Self::now_ms(),
            event_type: event_type.to_string(),
            severity: models::EventSeverity::Info,
            data: serde_json::json!({
                "category": "workflow",
                "type": event_type,
                "user": username,
                "message": format!("{} workflow {}", event_type.replace('_', " "), id),
            }),
        });

        Ok(())
    }

    #[cfg(feature = "openapi-codegen")]
    pub fn openapi_app_state(&self) -> crate::openapi_spec::OpenApiAppState {
        self.shared.openapi_app_state(
            full_version(),
            API_VERSION.to_string(),
            GIT_HASH.to_string(),
        )
    }
}

use crate::server::api_constants::API_VERSION;
use crate::server::api_contract::TransportApiCore;
use crate::server::response_types::{
    access::*, artifacts::*, events::*, jobs::*, scheduling::*, system::*, workflows::*,
};
use crate::server::transport_types::context_types::ApiError;
use crate::time_utils::duration_string_to_seconds;
use std::ops::Deref;

use crate::server::live_state::LiveServerState;

#[async_trait]
impl<C> TransportApiCore<C> for Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    /// Store a compute node.
    async fn create_compute_node(
        &self,
        body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<CreateComputeNodeResponse, ApiError> {
        self.transport_create_compute_node(body, context).await
    }

    /// Store an event.
    async fn create_event(
        &self,
        body: models::EventModel,
        context: &C,
    ) -> Result<CreateEventResponse, ApiError> {
        self.transport_create_event(body, context).await
    }

    /// Store a file.
    async fn create_file(
        &self,
        file: models::FileModel,
        context: &C,
    ) -> Result<CreateFileResponse, ApiError> {
        self.transport_create_file(file, context).await
    }

    /// Store a job.
    async fn create_job(
        &self,
        job: models::JobModel,
        context: &C,
    ) -> Result<CreateJobResponse, ApiError> {
        self.transport_create_job(job, context).await
    }

    /// Create jobs in bulk.
    async fn create_jobs(
        &self,
        body: models::JobsModel,
        context: &C,
    ) -> Result<CreateJobsResponse, ApiError> {
        self.transport_create_jobs(body, context).await
    }

    /// Store a local scheduler.
    async fn create_local_scheduler(
        &self,
        body: models::LocalSchedulerModel,
        context: &C,
    ) -> Result<CreateLocalSchedulerResponse, ApiError> {
        self.transport_create_local_scheduler(body, context).await
    }

    /// Store a failure handler.
    async fn create_failure_handler(
        &self,
        body: models::FailureHandlerModel,
        context: &C,
    ) -> Result<CreateFailureHandlerResponse, ApiError> {
        self.transport_create_failure_handler(body, context).await
    }

    /// Retrieve a failure handler by ID.
    async fn get_failure_handler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetFailureHandlerResponse, ApiError> {
        self.transport_get_failure_handler(id, context).await
    }

    /// Retrieve all failure handlers for one workflow.
    async fn list_failure_handlers(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListFailureHandlersResponse, ApiError> {
        self.transport_list_failure_handlers(workflow_id, offset, limit, context)
            .await
    }

    /// Delete a failure handler.
    async fn delete_failure_handler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteFailureHandlerResponse, ApiError> {
        self.transport_delete_failure_handler(id, context).await
    }

    /// Store an RO-Crate entity.
    async fn create_ro_crate_entity(
        &self,
        body: models::RoCrateEntityModel,
        context: &C,
    ) -> Result<CreateRoCrateEntityResponse, ApiError> {
        self.transport_create_ro_crate_entity(body, context).await
    }

    /// Retrieve an RO-Crate entity by ID.
    async fn get_ro_crate_entity(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetRoCrateEntityResponse, ApiError> {
        self.transport_get_ro_crate_entity(id, context).await
    }

    /// Retrieve all RO-Crate entities for one workflow.
    async fn list_ro_crate_entities(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListRoCrateEntitiesResponse, ApiError> {
        self.transport_list_ro_crate_entities(
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            context,
        )
        .await
    }

    /// Update an RO-Crate entity.
    async fn update_ro_crate_entity(
        &self,
        id: i64,
        body: models::RoCrateEntityModel,
        context: &C,
    ) -> Result<UpdateRoCrateEntityResponse, ApiError> {
        self.transport_update_ro_crate_entity(id, body, context)
            .await
    }

    /// Delete an RO-Crate entity.
    async fn delete_ro_crate_entity(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteRoCrateEntityResponse, ApiError> {
        self.transport_delete_ro_crate_entity(id, context).await
    }

    /// Delete all RO-Crate entities for a workflow.
    async fn delete_ro_crate_entities(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteRoCrateEntitiesResponse, ApiError> {
        self.transport_delete_ro_crate_entities(workflow_id, context)
            .await
    }

    /// Store one resource requirements record.
    async fn create_resource_requirements(
        &self,
        body: models::ResourceRequirementsModel,
        context: &C,
    ) -> Result<CreateResourceRequirementsResponse, ApiError> {
        self.transport_create_resource_requirements(body, context)
            .await
    }

    /// Store a job result.
    async fn create_result(
        &self,
        body: models::ResultModel,
        context: &C,
    ) -> Result<CreateResultResponse, ApiError> {
        self.transport_create_result(body, context).await
    }

    /// Store a scheduled compute node.
    async fn create_scheduled_compute_node(
        &self,
        body: models::ScheduledComputeNodesModel,
        context: &C,
    ) -> Result<CreateScheduledComputeNodeResponse, ApiError> {
        self.transport_create_scheduled_compute_node(body, context)
            .await
    }

    /// Store a Slurm compute node configuration.
    async fn create_slurm_scheduler(
        &self,
        body: models::SlurmSchedulerModel,
        context: &C,
    ) -> Result<CreateSlurmSchedulerResponse, ApiError> {
        self.transport_create_slurm_scheduler(body, context).await
    }

    /// Store Slurm accounting stats for a job step.
    async fn create_slurm_stats(
        &self,
        body: models::SlurmStatsModel,
        context: &C,
    ) -> Result<CreateSlurmStatsResponse, ApiError> {
        self.transport_create_slurm_stats(body, context).await
    }

    /// List Slurm accounting stats.
    async fn list_slurm_stats(
        &self,
        workflow_id: i64,
        job_id: Option<i64>,
        run_id: Option<i64>,
        attempt_id: Option<i64>,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListSlurmStatsResponse, ApiError> {
        self.transport_list_slurm_stats(
            workflow_id,
            job_id,
            run_id,
            attempt_id,
            offset,
            limit,
            context,
        )
        .await
    }

    /// Store remote workers for a workflow.
    async fn create_remote_workers(
        &self,
        workflow_id: i64,
        workers: Vec<String>,
        context: &C,
    ) -> Result<CreateRemoteWorkersResponse, ApiError> {
        self.transport_create_remote_workers(workflow_id, workers, context)
            .await
    }

    /// List remote workers for a workflow.
    async fn list_remote_workers(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<ListRemoteWorkersResponse, ApiError> {
        self.transport_list_remote_workers(workflow_id, context)
            .await
    }

    /// Delete a remote worker from a workflow.
    async fn delete_remote_worker(
        &self,
        workflow_id: i64,
        worker: String,
        context: &C,
    ) -> Result<DeleteRemoteWorkerResponse, ApiError> {
        self.transport_delete_remote_worker(workflow_id, worker, context)
            .await
    }

    /// Store a user data record.
    async fn create_user_data(
        &self,
        body: models::UserDataModel,
        consumer_job_id: Option<i64>,
        producer_job_id: Option<i64>,
        context: &C,
    ) -> Result<CreateUserDataResponse, ApiError> {
        self.transport_create_user_data(body, consumer_job_id, producer_job_id, context)
            .await
    }

    /// Store a workflow.
    async fn create_workflow(
        &self,
        body: models::WorkflowModel,
        context: &C,
    ) -> Result<CreateWorkflowResponse, ApiError> {
        self.transport_create_workflow(body, context).await
    }

    /// Create a workflow action.
    async fn create_workflow_action(
        &self,
        workflow_id: i64,
        body: models::WorkflowActionModel,
        context: &C,
    ) -> Result<CreateWorkflowActionResponse, ApiError> {
        self.transport_create_workflow_action(workflow_id, body, context)
            .await
    }

    async fn get_workflow_actions(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<GetWorkflowActionsResponse, ApiError> {
        self.transport_get_workflow_actions(workflow_id, context)
            .await
    }

    #[instrument(level = "debug", skip(self, context), fields(workflow_id))]
    async fn get_pending_actions(
        &self,
        workflow_id: i64,
        trigger_types: Option<Vec<String>>,
        context: &C,
    ) -> Result<GetPendingActionsResponse, ApiError> {
        self.transport_get_pending_actions(workflow_id, trigger_types, context)
            .await
    }

    async fn claim_action(
        &self,
        workflow_id: i64,
        action_id: i64,
        body: models::ClaimActionRequest,
        context: &C,
    ) -> Result<ClaimActionResponse, ApiError> {
        self.transport_claim_action(workflow_id, action_id, body, context)
            .await
    }

    /// Return the version of the service.
    async fn get_version(&self, context: &C) -> Result<GetVersionResponse, ApiError> {
        self.transport_get_version(context).await
    }

    async fn reload_auth(&self, context: &C) -> Result<ReloadAuthResponse, ApiError> {
        self.transport_reload_auth(context).await
    }

    async fn list_workflows(
        &self,
        offset: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        limit: Option<i64>,
        name: Option<String>,
        user: Option<String>,
        description: Option<String>,
        is_archived: Option<bool>,
        context: &C,
    ) -> Result<ListWorkflowsResponse, ApiError> {
        self.transport_list_workflows(
            offset,
            sort_by,
            reverse_sort,
            limit,
            name,
            user,
            description,
            is_archived,
            context,
        )
        .await
    }

    /// Check if the service is running.
    async fn ping(&self, context: &C) -> Result<PingResponse, ApiError> {
        self.transport_ping(context).await
    }

    async fn cancel_workflow(
        &self,
        id: i64,
        context: &C,
    ) -> Result<CancelWorkflowResponse, ApiError> {
        self.transport_cancel_workflow(id, context).await
    }

    async fn delete_compute_nodes(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteComputeNodesResponse, ApiError> {
        self.transport_delete_compute_nodes(workflow_id, context)
            .await
    }

    async fn delete_events(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteEventsResponse, ApiError> {
        self.transport_delete_events(workflow_id, context).await
    }

    async fn delete_files(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteFilesResponse, ApiError> {
        self.transport_delete_files(workflow_id, context).await
    }

    async fn delete_jobs(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteJobsResponse, ApiError> {
        self.transport_delete_jobs(workflow_id, context).await
    }

    async fn delete_local_schedulers(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteLocalSchedulersResponse, ApiError> {
        self.transport_delete_local_schedulers(workflow_id, context)
            .await
    }

    async fn delete_all_resource_requirements(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteAllResourceRequirementsResponse, ApiError> {
        self.transport_delete_all_resource_requirements(workflow_id, context)
            .await
    }

    async fn delete_results(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteResultsResponse, ApiError> {
        self.transport_delete_results(workflow_id, context).await
    }

    async fn delete_scheduled_compute_nodes(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodesResponse, ApiError> {
        self.transport_delete_scheduled_compute_nodes(workflow_id, context)
            .await
    }

    async fn delete_slurm_schedulers(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteSlurmSchedulersResponse, ApiError> {
        self.transport_delete_slurm_schedulers(workflow_id, context)
            .await
    }

    async fn delete_all_user_data(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteAllUserDataResponse, ApiError> {
        self.transport_delete_all_user_data(workflow_id, context)
            .await
    }

    async fn list_compute_nodes(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        hostname: Option<String>,
        is_active: Option<bool>,
        scheduled_compute_node_id: Option<i64>,
        context: &C,
    ) -> Result<ListComputeNodesResponse, ApiError> {
        self.transport_list_compute_nodes(
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            hostname,
            is_active,
            scheduled_compute_node_id,
            context,
        )
        .await
    }

    async fn list_events(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        category: Option<String>,
        after_timestamp: Option<i64>,
        context: &C,
    ) -> Result<ListEventsResponse, ApiError> {
        self.transport_list_events(
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            category,
            after_timestamp,
            context,
        )
        .await
    }

    async fn list_files(
        &self,
        workflow_id: i64,
        produced_by_job_id: Option<i64>,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        name: Option<String>,
        path: Option<String>,
        is_output: Option<bool>,
        context: &C,
    ) -> Result<ListFilesResponse, ApiError> {
        self.transport_list_files(
            workflow_id,
            produced_by_job_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            name,
            path,
            is_output,
            context,
        )
        .await
    }

    /// Retrieve all jobs for one workflow.
    async fn list_jobs(
        &self,
        workflow_id: i64,
        status: Option<models::JobStatus>,
        needs_file_id: Option<i64>,
        upstream_job_id: Option<i64>,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        include_relationships: Option<bool>,
        active_compute_node_id: Option<i64>,
        context: &C,
    ) -> Result<ListJobsResponse, ApiError> {
        self.transport_list_jobs(
            workflow_id,
            status,
            needs_file_id,
            upstream_job_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            include_relationships,
            active_compute_node_id,
            context,
        )
        .await
    }

    /// Retrieve all job dependencies for one workflow.
    async fn list_job_dependencies(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListJobDependenciesResponse, ApiError> {
        self.transport_list_job_dependencies(
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            context,
        )
        .await
    }

    async fn list_job_file_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListJobFileRelationshipsResponse, ApiError> {
        self.transport_list_job_file_relationships(
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            context,
        )
        .await
    }

    async fn list_job_user_data_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListJobUserDataRelationshipsResponse, ApiError> {
        self.transport_list_job_user_data_relationships(
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            context,
        )
        .await
    }

    async fn list_local_schedulers(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        memory: Option<String>,
        num_cpus: Option<i64>,
        context: &C,
    ) -> Result<ListLocalSchedulersResponse, ApiError> {
        self.transport_list_local_schedulers(
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            memory,
            num_cpus,
            context,
        )
        .await
    }

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
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListResourceRequirementsResponse, ApiError> {
        self.transport_list_resource_requirements(
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
            context,
        )
        .await
    }

    async fn list_results(
        &self,
        workflow_id: i64,
        job_id: Option<i64>,
        run_id: Option<i64>,
        return_code: Option<i64>,
        status: Option<models::JobStatus>,
        compute_node_id: Option<i64>,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        all_runs: Option<bool>,
        context: &C,
    ) -> Result<ListResultsResponse, ApiError> {
        self.transport_list_results(
            workflow_id,
            job_id,
            run_id,
            return_code,
            status,
            compute_node_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            all_runs,
            context,
        )
        .await
    }

    async fn list_scheduled_compute_nodes(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        scheduler_id: Option<String>,
        scheduler_config_id: Option<String>,
        status: Option<String>,
        context: &C,
    ) -> Result<ListScheduledComputeNodesResponse, ApiError> {
        self.transport_list_scheduled_compute_nodes(
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            scheduler_id,
            scheduler_config_id,
            status,
            context,
        )
        .await
    }

    async fn list_slurm_schedulers(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        _: Option<String>,
        _: Option<String>,
        _: Option<String>,
        _: Option<String>,
        _: Option<i64>,
        _: Option<String>,
        _: Option<String>,
        _: Option<String>,
        _: Option<String>,
        context: &C,
    ) -> Result<ListSlurmSchedulersResponse, ApiError> {
        self.transport_list_slurm_schedulers(
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            context,
        )
        .await
    }

    async fn list_user_data(
        &self,
        workflow_id: i64,
        consumer_job_id: Option<i64>,
        producer_job_id: Option<i64>,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        name: Option<String>,
        is_ephemeral: Option<bool>,
        context: &C,
    ) -> Result<ListUserDataResponse, ApiError> {
        self.transport_list_user_data(
            workflow_id,
            consumer_job_id,
            producer_job_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            name,
            is_ephemeral,
            context,
        )
        .await
    }

    async fn get_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetComputeNodeResponse, ApiError> {
        self.transport_get_compute_node(id, context).await
    }

    async fn get_event(&self, id: i64, context: &C) -> Result<GetEventResponse, ApiError> {
        self.transport_get_event(id, context).await
    }

    async fn get_file(&self, id: i64, context: &C) -> Result<GetFileResponse, ApiError> {
        self.transport_get_file(id, context).await
    }

    async fn get_job(&self, id: i64, context: &C) -> Result<GetJobResponse, ApiError> {
        self.transport_get_job(id, context).await
    }

    /// Retrieve a local scheduler.
    async fn get_local_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetLocalSchedulerResponse, ApiError> {
        self.transport_get_local_scheduler(id, context).await
    }

    /// Return the resource requirements for jobs with a status of ready.
    #[instrument(level = "debug", skip(self, context), fields(workflow_id = id, scheduler_config_id = ?scheduler_config_id))]
    async fn get_ready_job_requirements(
        &self,
        id: i64,
        scheduler_config_id: Option<i64>,
        context: &C,
    ) -> Result<GetReadyJobRequirementsResponse, ApiError> {
        self.transport_get_ready_job_requirements(id, scheduler_config_id, context)
            .await
    }

    /// Retrieve one resource requirements record.
    async fn get_resource_requirements(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetResourceRequirementsResponse, ApiError> {
        self.transport_get_resource_requirements(id, context).await
    }

    /// Retrieve a job result.
    async fn get_result(&self, id: i64, context: &C) -> Result<GetResultResponse, ApiError> {
        self.transport_get_result(id, context).await
    }

    /// Retrieve a scheduled compute node.
    async fn get_scheduled_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetScheduledComputeNodeResponse, ApiError> {
        self.transport_get_scheduled_compute_node(id, context).await
    }

    /// Retrieve a Slurm compute node configuration.
    async fn get_slurm_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetSlurmSchedulerResponse, ApiError> {
        self.transport_get_slurm_scheduler(id, context).await
    }

    /// Retrieve a user data record.
    async fn get_user_data(&self, id: i64, context: &C) -> Result<GetUserDataResponse, ApiError> {
        self.transport_get_user_data(id, context).await
    }

    /// Retrieve a workflow.
    async fn get_workflow(&self, id: i64, context: &C) -> Result<GetWorkflowResponse, ApiError> {
        self.transport_get_workflow(id, context).await
    }

    async fn get_workflow_status(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetWorkflowStatusResponse, ApiError> {
        self.transport_get_workflow_status(id, context).await
    }

    async fn get_active_task_for_workflow(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<GetActiveTaskResponse, ApiError> {
        match self
            .check_workflow_access_for_context(workflow_id, context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(_) | AccessCheckResult::NotFound(_) => {
                return Ok(GetActiveTaskResponse::NotFoundErrorResponse(
                    not_found_error!("Workflow not found".to_string()),
                ));
            }
            AccessCheckResult::InternalError(reason) => {
                return Err(ApiError(reason));
            }
        }

        let task = self.get_active_task(workflow_id).await?;
        Ok(GetActiveTaskResponse::SuccessfulResponse(
            models::ActiveTaskResponse { task },
        ))
    }

    async fn get_task(&self, id: i64, context: &C) -> Result<GetTaskResponse, ApiError> {
        let row = sqlx::query(
            r#"
            SELECT id, workflow_id, operation, status, created_at_ms, started_at_ms, finished_at_ms, error
            FROM async_handle
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .fetch_optional(self.pool.as_ref())
        .await
        .map_err(|e| ApiError(format!("Database error: {}", e)))?;

        let row = match row {
            Some(r) => r,
            None => {
                return Ok(GetTaskResponse::NotFoundErrorResponse(not_found_error!(
                    "Task not found".to_string()
                )));
            }
        };

        // Avoid task ID enumeration: return 404 both for "no such task" and "not authorized".
        match self
            .check_workflow_access_for_context(row.get("workflow_id"), context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(_) | AccessCheckResult::NotFound(_) => {
                return Ok(GetTaskResponse::NotFoundErrorResponse(not_found_error!(
                    "Task not found".to_string()
                )));
            }
            AccessCheckResult::InternalError(reason) => {
                return Err(ApiError(reason));
            }
        }

        let status = row
            .get::<String, _>("status")
            .parse::<models::TaskStatus>()
            .map_err(|e| {
                error!("Invalid async_handle.status for task_id={}: {}", id, e);
                ApiError("Database error".to_string())
            })?;

        Ok(GetTaskResponse::SuccessfulResponse(models::TaskModel {
            id: row.get("id"),
            workflow_id: row.get("workflow_id"),
            operation: row.get("operation"),
            status,
            created_at_ms: row.get("created_at_ms"),
            started_at_ms: row.get("started_at_ms"),
            finished_at_ms: row.get("finished_at_ms"),
            error: row.get("error"),
        }))
    }

    /// Initialize job relationships based on file and user_data relationships.
    ///
    /// This operation wraps all initialization steps in a transaction to ensure atomicity.
    /// If any step fails, all changes will be rolled back.
    ///
    /// This function can be called multiple times (e.g., for workflow reruns). It will:
    /// - Reset all job statuses to uninitialized
    /// - Delete workflow_result records for incomplete jobs
    /// - Re-initialize job statuses based on dependencies
    async fn initialize_jobs(
        &self,
        id: i64,
        only_uninitialized: Option<bool>,
        clear_ephemeral_user_data: Option<bool>,
        async_: Option<bool>,
        context: &C,
    ) -> Result<InitializeJobsResponse, ApiError> {
        self.transport_initialize_jobs(
            id,
            only_uninitialized,
            clear_ephemeral_user_data,
            async_,
            context,
        )
        .await
    }

    /// Return true if all jobs in the workflow are complete.
    #[instrument(level = "debug", skip(self, context), fields(workflow_id = id))]
    async fn is_workflow_complete(
        &self,
        id: i64,
        context: &C,
    ) -> Result<IsWorkflowCompleteResponse, ApiError> {
        self.transport_is_workflow_complete(id, context).await
    }

    async fn is_workflow_uninitialized(
        &self,
        id: i64,
        context: &C,
    ) -> Result<IsWorkflowUninitializedResponse, ApiError> {
        self.transport_is_workflow_uninitialized(id, context).await
    }

    /// Retrieve all job IDs for one workflow.
    async fn list_job_ids(&self, id: i64, context: &C) -> Result<ListJobIdsResponse, ApiError> {
        self.transport_list_job_ids(id, context).await
    }

    /// List missing user data that should exist.
    async fn list_missing_user_data(
        &self,
        id: i64,
        context: &C,
    ) -> Result<ListMissingUserDataResponse, ApiError> {
        self.transport_list_missing_user_data(id, context).await
    }

    /// List files that must exist.
    async fn list_required_existing_files(
        &self,
        id: i64,
        context: &C,
    ) -> Result<ListRequiredExistingFilesResponse, ApiError> {
        self.transport_list_required_existing_files(id, context)
            .await
    }

    /// Update a compute node.
    async fn update_compute_node(
        &self,
        id: i64,
        body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<UpdateComputeNodeResponse, ApiError> {
        self.transport_update_compute_node(id, body, context).await
    }

    /// Update an event.
    async fn update_event(
        &self,
        id: i64,
        body: serde_json::Value,
        context: &C,
    ) -> Result<UpdateEventResponse, ApiError> {
        self.transport_update_event(id, body, context).await
    }

    /// Update a file.
    async fn update_file(
        &self,
        id: i64,
        body: models::FileModel,
        context: &C,
    ) -> Result<UpdateFileResponse, ApiError> {
        self.transport_update_file(id, body, context).await
    }

    /// Update a job.
    async fn update_job(
        &self,
        id: i64,
        body: models::JobModel,
        context: &C,
    ) -> Result<UpdateJobResponse, ApiError> {
        self.transport_update_job(id, body, context).await
    }

    async fn update_local_scheduler(
        &self,
        id: i64,
        body: models::LocalSchedulerModel,
        context: &C,
    ) -> Result<UpdateLocalSchedulerResponse, ApiError> {
        self.transport_update_local_scheduler(id, body, context)
            .await
    }

    /// Update one resource requirements record.
    async fn update_resource_requirements(
        &self,
        id: i64,
        body: models::ResourceRequirementsModel,
        context: &C,
    ) -> Result<UpdateResourceRequirementsResponse, ApiError> {
        self.transport_update_resource_requirements(id, body, context)
            .await
    }

    /// Update a job result.
    async fn update_result(
        &self,
        id: i64,
        body: models::ResultModel,
        context: &C,
    ) -> Result<UpdateResultResponse, ApiError> {
        self.transport_update_result(id, body, context).await
    }

    /// Update a scheduled compute node.
    async fn update_scheduled_compute_node(
        &self,
        id: i64,
        body: models::ScheduledComputeNodesModel,
        context: &C,
    ) -> Result<UpdateScheduledComputeNodeResponse, ApiError> {
        self.transport_update_scheduled_compute_node(id, body, context)
            .await
    }

    /// Update a Slurm compute node configuration.
    async fn update_slurm_scheduler(
        &self,
        id: i64,
        body: models::SlurmSchedulerModel,
        context: &C,
    ) -> Result<UpdateSlurmSchedulerResponse, ApiError> {
        self.transport_update_slurm_scheduler(id, body, context)
            .await
    }

    /// Update a user data record.
    async fn update_user_data(
        &self,
        id: i64,
        body: models::UserDataModel,
        context: &C,
    ) -> Result<UpdateUserDataResponse, ApiError> {
        self.transport_update_user_data(id, body, context).await
    }

    /// Update a workflow.
    async fn update_workflow(
        &self,
        id: i64,
        body: models::WorkflowModel,
        context: &C,
    ) -> Result<UpdateWorkflowResponse, ApiError> {
        self.transport_update_workflow(id, body, context).await
    }

    /// Update the workflow status.
    async fn update_workflow_status(
        &self,
        id: i64,
        body: models::WorkflowStatusModel,
        context: &C,
    ) -> Result<UpdateWorkflowStatusResponse, ApiError> {
        self.transport_update_workflow_status(id, body, context)
            .await
    }

    /// Return jobs that are ready for submission and meet worker resource requirements. Set status to pending.
    #[instrument(level = "debug", skip(self, context), fields(workflow_id = id, limit))]
    async fn claim_jobs_based_on_resources(
        &self,
        id: i64,
        body: models::ComputeNodesResources,
        limit: i64,
        strict_scheduler_match: Option<bool>,
        context: &C,
    ) -> Result<ClaimJobsBasedOnResources, ApiError> {
        self.transport_claim_jobs_based_on_resources(
            id,
            body,
            limit,
            strict_scheduler_match,
            context,
        )
        .await
    }

    /// Return user-requested number of jobs that are ready for submission. Sets status to pending.
    #[instrument(level = "debug", skip(self, context), fields(workflow_id = id, limit = ?limit))]
    async fn claim_next_jobs(
        &self,
        id: i64,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ClaimNextJobsResponse, ApiError> {
        self.transport_claim_next_jobs(id, limit, context).await
    }

    /// Check for changed job inputs and update status accordingly.
    #[instrument(level = "debug", skip(self, context), fields(workflow_id = id, dry_run = ?dry_run))]
    async fn process_changed_job_inputs(
        &self,
        id: i64,
        dry_run: Option<bool>,
        context: &C,
    ) -> Result<ProcessChangedJobInputsResponse, ApiError> {
        self.transport_process_changed_job_inputs(id, dry_run, context)
            .await
    }

    /// Delete a compute node.
    async fn delete_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteComputeNodeResponse, ApiError> {
        self.transport_delete_compute_node(id, context).await
    }

    /// Delete an event.
    async fn delete_event(&self, id: i64, context: &C) -> Result<DeleteEventResponse, ApiError> {
        self.transport_delete_event(id, context).await
    }

    /// Delete a file.
    async fn delete_file(&self, id: i64, context: &C) -> Result<DeleteFileResponse, ApiError> {
        self.transport_delete_file(id, context).await
    }

    /// Delete a job.
    async fn delete_job(&self, id: i64, context: &C) -> Result<DeleteJobResponse, ApiError> {
        self.transport_delete_job(id, context).await
    }

    /// Delete a local scheduler.
    async fn delete_local_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteLocalSchedulerResponse, ApiError> {
        self.transport_delete_local_scheduler(id, context).await
    }

    /// Delete a resource requirements record.
    async fn delete_resource_requirements(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteResourceRequirementsResponse, ApiError> {
        self.transport_delete_resource_requirements(id, context)
            .await
    }

    /// Delete a job result.
    async fn delete_result(&self, id: i64, context: &C) -> Result<DeleteResultResponse, ApiError> {
        self.transport_delete_result(id, context).await
    }

    /// Delete a scheduled compute node.
    async fn delete_scheduled_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodeResponse, ApiError> {
        self.transport_delete_scheduled_compute_node(id, context)
            .await
    }

    /// Delete Slurm compute node configuration.
    async fn delete_slurm_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteSlurmSchedulerResponse, ApiError> {
        self.transport_delete_slurm_scheduler(id, context).await
    }

    /// Delete a user data record.
    async fn delete_user_data(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteUserDataResponse, ApiError> {
        self.transport_delete_user_data(id, context).await
    }

    async fn delete_workflow(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteWorkflowResponse, ApiError> {
        self.transport_delete_workflow(id, context).await
    }

    /// Reset status for jobs to uninitialized.
    /// If failed_only is true, only jobs with a failed result will be reset.
    /// If failed_only is false, all jobs will be reset.
    async fn reset_job_status(
        &self,
        id: i64,
        failed_only: Option<bool>,
        context: &C,
    ) -> Result<ResetJobStatusResponse, ApiError> {
        self.transport_reset_job_status(id, failed_only, context)
            .await
    }

    /// Reset worklow status.
    /// Rules:
    /// - Not allowed if any job is running or SubmittedPending (unless force=true)
    /// Actions:
    /// - Reset fields in WorkflowStatusModel
    async fn reset_workflow_status(
        &self,
        id: i64,
        force: Option<bool>,
        context: &C,
    ) -> Result<ResetWorkflowStatusResponse, ApiError> {
        self.transport_reset_workflow_status(id, force, context)
            .await
    }

    /// Change the status of a job and manage side effects.
    #[instrument(level = "debug", skip(self, context), fields(job_id = id, status = ?status, run_id))]
    async fn manage_status_change(
        &self,
        id: i64,
        status: models::JobStatus,
        run_id: i64,
        context: &C,
    ) -> Result<ManageStatusChangeResponse, ApiError> {
        self.transport_manage_status_change(id, status, run_id, context)
            .await
    }

    /// Start a job and manage side effects.
    #[instrument(level = "debug", skip(self, context), fields(job_id = id, run_id, compute_node_id))]
    async fn start_job(
        &self,
        id: i64,
        run_id: i64,
        compute_node_id: i64,
        context: &C,
    ) -> Result<StartJobResponse, ApiError> {
        self.transport_start_job(id, run_id, compute_node_id, context)
            .await
    }

    /// Complete a job, connect it to a result, and manage side effects.
    #[instrument(level = "debug", skip(self, result, context), fields(job_id = id, status = ?status, run_id))]
    async fn complete_job(
        &self,
        id: i64,
        status: models::JobStatus,
        run_id: i64,
        result: models::ResultModel,
        context: &C,
    ) -> Result<CompleteJobResponse, ApiError> {
        self.transport_complete_job(id, status, run_id, result, context)
            .await
    }

    /// Complete a batch of jobs in a single request.
    #[instrument(level = "debug", skip(self, body, context), fields(workflow_id, completion_count = body.completions.len()))]
    async fn batch_complete_jobs(
        &self,
        workflow_id: i64,
        body: models::BatchCompleteJobsRequest,
        context: &C,
    ) -> Result<BatchCompleteJobsResponse, ApiError> {
        self.transport_batch_complete_jobs(workflow_id, body, context)
            .await
    }

    /// Retry a failed job by resetting it to ready status and incrementing attempt_id.
    async fn retry_job(
        &self,
        id: i64,
        run_id: i64,
        max_retries: i32,
        context: &C,
    ) -> Result<RetryJobResponse, ApiError> {
        self.transport_retry_job(id, run_id, max_retries, context)
            .await
    }

    /// Get ready jobs that fit within the specified resource constraints.
    ///
    /// This function performs the following operations:
    /// 1. Queries job and resource_requirements tables for ready jobs
    /// 2. Filters jobs based on resource constraints:
    ///    - memory_bytes <= resources.memory_gb (converted from GiB to bytes)
    ///    - num_cpus <= resources.num_cpus
    ///    - num_gpus <= resources.num_gpus
    ///    - num_nodes <= resources.num_nodes (only multi-node jobs consume dedicated nodes)
    ///    - runtime_s < resources.time_limit (converted to seconds using duration_string_to_seconds)
    /// 3. Sorts results by job priority descending, then favors GPU jobs within
    ///    the same priority to avoid starving GPU-capable work behind CPU-only
    ///    jobs
    /// 4. Loops through returned records and accumulates resource consumption
    /// 5. Selects jobs that can fit within total available resources
    /// 6. Atomically updates selected jobs to "pending" status
    ///
    /// # Parameters
    /// - `workflow_id`: ID of the workflow to get jobs for
    /// - `resources`: Available compute resources (CPUs, memory, GPUs, nodes, time limit)
    /// - `limit`: Maximum number of jobs to return
    ///
    /// # Returns
    /// A `ClaimJobsBasedOnResources` containing the list of jobs that were selected and updated,
    /// or an error if the operation fails. The `reason` field is set to an empty string.
    ///
    /// # Implementation Notes
    /// - Uses SQLite BEGIN IMMEDIATE TRANSACTION to acquire a database write lock
    /// - This ensures thread-safe access at the database level, preventing race conditions
    /// - The lock prevents concurrent job selection and ensures consistent resource accounting
    /// - Leverages the time_utils::duration_string_to_seconds function for time parsing
    /// - All selected jobs are changed from "ready" to "pending" status atomically
    #[instrument(
        level = "debug",
        skip(self, resources, context),
        fields(workflow_id, limit)
    )]
    async fn prepare_ready_jobs(
        &self,
        workflow_id: i64,
        resources: models::ComputeNodesResources,
        limit: i64,
        strict_scheduler_match: Option<bool>,
        context: &C,
    ) -> Result<ClaimJobsBasedOnResources, ApiError> {
        self.transport_prepare_ready_jobs(
            workflow_id,
            resources,
            limit,
            strict_scheduler_match,
            context,
        )
        .await
    }

    // Access Groups API

    async fn create_access_group(
        &self,
        body: models::AccessGroupModel,
        context: &C,
    ) -> Result<CreateAccessGroupResponse, ApiError> {
        self.transport_create_access_group(body, context).await
    }

    async fn get_access_group(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetAccessGroupResponse, ApiError> {
        self.transport_get_access_group(id, context).await
    }

    async fn list_access_groups(
        &self,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListAccessGroupsApiResponse, ApiError> {
        self.transport_list_access_groups(offset, limit, context)
            .await
    }

    async fn delete_access_group(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteAccessGroupResponse, ApiError> {
        self.transport_delete_access_group(id, context).await
    }

    async fn add_user_to_group(
        &self,
        group_id: i64,
        body: models::UserGroupMembershipModel,
        context: &C,
    ) -> Result<AddUserToGroupResponse, ApiError> {
        self.transport_add_user_to_group(group_id, body, context)
            .await
    }

    async fn remove_user_from_group(
        &self,
        group_id: i64,
        user_name: String,
        context: &C,
    ) -> Result<RemoveUserFromGroupResponse, ApiError> {
        self.transport_remove_user_from_group(group_id, user_name, context)
            .await
    }

    async fn list_group_members(
        &self,
        group_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListGroupMembersResponse, ApiError> {
        self.transport_list_group_members(group_id, offset, limit, context)
            .await
    }

    async fn list_user_groups(
        &self,
        user_name: String,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListUserGroupsApiResponse, ApiError> {
        self.transport_list_user_groups(user_name, offset, limit, context)
            .await
    }

    async fn add_workflow_to_group(
        &self,
        workflow_id: i64,
        group_id: i64,
        context: &C,
    ) -> Result<AddWorkflowToGroupResponse, ApiError> {
        self.transport_add_workflow_to_group(workflow_id, group_id, context)
            .await
    }

    async fn remove_workflow_from_group(
        &self,
        workflow_id: i64,
        group_id: i64,
        context: &C,
    ) -> Result<RemoveWorkflowFromGroupResponse, ApiError> {
        self.transport_remove_workflow_from_group(workflow_id, group_id, context)
            .await
    }

    async fn list_workflow_groups(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListWorkflowGroupsResponse, ApiError> {
        self.transport_list_workflow_groups(workflow_id, offset, limit, context)
            .await
    }

    async fn check_workflow_access(
        &self,
        workflow_id: i64,
        user_name: String,
        context: &C,
    ) -> Result<CheckWorkflowAccessResponse, ApiError> {
        self.transport_check_workflow_access(workflow_id, user_name, context)
            .await
    }

    /// Subscribe to the event broadcast channel for SSE streaming.
    fn subscribe_to_events(&self) -> tokio::sync::broadcast::Receiver<BroadcastEvent> {
        self.event_broadcaster.subscribe()
    }
}

// Helper methods for Server (not part of the Api trait)
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Send + Sync,
{
    // No additional helper methods needed
}
