use super::*;
use crate::server::api::{EventsApi, JobsApi, ResultsApi, WorkflowsApi, database_error_with_msg};

const RESOURCE_CLAIM_ORDER_BY: &str = "\
    ORDER BY \
        job.priority DESC, \
        rr.num_gpus DESC, \
        rr.runtime_s DESC, \
        rr.memory_bytes DESC, \
        rr.num_cpus DESC, \
        job.id ASC";

#[derive(Clone, Copy)]
struct ClaimRemainingResources {
    cpus: i64,
    memory_bytes: i64,
    gpus: i64,
    /// Remaining shared-node capacity after exclusive multi-node reservations.
    nodes: i64,
}

struct ClaimPackingState {
    per_node_cpus: i64,
    per_node_memory: i64,
    per_node_gpus: i64,
    total_nodes: i64,
    consumed_memory_bytes: i64,
    consumed_cpus: i64,
    consumed_gpus: i64,
    exclusive_nodes: i64,
}

impl ClaimPackingState {
    fn new(resources: &models::ComputeNodesResources, memory_bytes: i64) -> Self {
        Self {
            per_node_cpus: resources.num_cpus,
            per_node_memory: memory_bytes,
            per_node_gpus: resources.num_gpus,
            total_nodes: resources.num_nodes.max(1),
            consumed_memory_bytes: 0,
            consumed_cpus: 0,
            consumed_gpus: 0,
            exclusive_nodes: 0,
        }
    }

    fn remaining_resources(&self) -> ClaimRemainingResources {
        let shared_nodes = (self.total_nodes - self.exclusive_nodes).max(0);
        ClaimRemainingResources {
            cpus: shared_nodes
                .saturating_mul(self.per_node_cpus)
                .saturating_sub(self.consumed_cpus),
            memory_bytes: shared_nodes
                .saturating_mul(self.per_node_memory)
                .saturating_sub(self.consumed_memory_bytes),
            gpus: shared_nodes
                .saturating_mul(self.per_node_gpus)
                .saturating_sub(self.consumed_gpus),
            nodes: shared_nodes,
        }
    }

    fn candidate_fits(&self, row: &sqlx::sqlite::SqliteRow) -> bool {
        let job_memory: i64 = row.get("memory_bytes");
        let job_cpus: i64 = row.get("num_cpus");
        let job_gpus: i64 = row.get("num_gpus");
        let job_nodes: i64 = row.get("num_nodes");
        let reserved_nodes = job_nodes.max(1);

        if reserved_nodes > 1 {
            let shared_nodes_after = self.total_nodes - self.exclusive_nodes - reserved_nodes;
            self.exclusive_nodes + reserved_nodes <= self.total_nodes
                && self.consumed_cpus <= shared_nodes_after * self.per_node_cpus
                && self.consumed_memory_bytes <= shared_nodes_after * self.per_node_memory
                && self.consumed_gpus <= shared_nodes_after * self.per_node_gpus
        } else {
            let shared_capacity_cpus =
                (self.total_nodes - self.exclusive_nodes) * self.per_node_cpus;
            let shared_capacity_memory =
                (self.total_nodes - self.exclusive_nodes) * self.per_node_memory;
            let shared_capacity_gpus =
                (self.total_nodes - self.exclusive_nodes) * self.per_node_gpus;
            self.consumed_cpus + job_cpus <= shared_capacity_cpus
                && self.consumed_memory_bytes + job_memory <= shared_capacity_memory
                && self.consumed_gpus + job_gpus <= shared_capacity_gpus
        }
    }

    fn consume_candidate(&mut self, row: &sqlx::sqlite::SqliteRow) {
        let job_memory: i64 = row.get("memory_bytes");
        let job_cpus: i64 = row.get("num_cpus");
        let job_gpus: i64 = row.get("num_gpus");
        let job_nodes: i64 = row.get("num_nodes");
        let reserved_nodes = job_nodes.max(1);

        if reserved_nodes > 1 {
            self.exclusive_nodes += reserved_nodes;
        } else {
            self.consumed_memory_bytes += job_memory;
            self.consumed_cpus += job_cpus;
            self.consumed_gpus += job_gpus;
        }
    }

    fn skip_reason(&self, row: &sqlx::sqlite::SqliteRow) -> String {
        let job_memory: i64 = row.get("memory_bytes");
        let job_cpus: i64 = row.get("num_cpus");
        let job_gpus: i64 = row.get("num_gpus");
        let job_nodes: i64 = row.get("num_nodes");
        let reserved_nodes = job_nodes.max(1);

        if reserved_nodes > 1 {
            let available = self.total_nodes - self.exclusive_nodes;
            format!(
                "multi-node job needs {} free nodes, {} available \
                 (exclusive_nodes={}, shared cpus={}/{})",
                reserved_nodes,
                available,
                self.exclusive_nodes,
                self.consumed_cpus,
                (self.total_nodes - self.exclusive_nodes) * self.per_node_cpus
            )
        } else {
            let shared_nodes = self.total_nodes - self.exclusive_nodes;
            format!(
                "cpus: {}/{}, memory: {}/{}, gpus: {}/{}",
                self.consumed_cpus + job_cpus,
                shared_nodes * self.per_node_cpus,
                self.consumed_memory_bytes + job_memory,
                shared_nodes * self.per_node_memory,
                self.consumed_gpus + job_gpus,
                shared_nodes * self.per_node_gpus
            )
        }
    }
}

struct CompletedJobRecord {
    job: models::JobModel,
    job_id: i64,
    workflow_id: i64,
    status: models::JobStatus,
    result_return_code: i64,
    result_id: i64,
}

enum CompletionMutationError {
    Response(Box<CompleteJobResponse>),
    Transport(ApiError),
}

fn completion_error_message(err: &models::ErrorResponse) -> String {
    err.error
        .get("message")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| {
            serde_json::to_string(&err.error).unwrap_or_else(|_| "unknown error".to_string())
        })
}

struct BackfillClaimParams {
    workflow_id: i64,
    ready_status: i32,
    time_limit_seconds: i64,
    scheduler_config_id: Option<i64>,
    use_scheduler_filter: bool,
    claim_limit: usize,
}

fn claim_candidate_row(
    row: &sqlx::sqlite::SqliteRow,
    packing_state: &mut ClaimPackingState,
    selected_jobs: &mut Vec<models::JobModel>,
    job_ids_to_update: &mut Vec<i64>,
) -> Result<bool, ApiError> {
    if !packing_state.candidate_fits(row) {
        if log::log_enabled!(log::Level::Debug) {
            debug!(
                "Skipping job {} - would exceed resource limits ({})",
                row.get::<i64, _>("job_id"),
                packing_state.skip_reason(row)
            );
        }
        return Ok(false);
    }

    let status = models::JobStatus::from_int(row.get::<i64, _>("status") as i32).map_err(|e| {
        error!("Failed to parse job status: {}", e);
        ApiError("Invalid job status".to_string())
    })?;

    if status != models::JobStatus::Ready {
        error!("Expected job status to be Ready, but got: {}", status);
        return Err(ApiError("Invalid job status in ready queue".to_string()));
    }

    packing_state.consume_candidate(row);

    let job_id: i64 = row.get("job_id");
    job_ids_to_update.push(job_id);
    selected_jobs.push(models::JobModel {
        id: Some(job_id),
        workflow_id: row.get("workflow_id"),
        name: row.get("name"),
        command: row.get("command"),
        env: crate::server::api::deserialize_env_map(row.get("env"), "job env")?,
        invocation_script: row.get("invocation_script"),
        status: Some(models::JobStatus::Pending),
        schedule_compute_nodes: None,
        cancel_on_blocking_job_failure: Some(row.get("cancel_on_blocking_job_failure")),
        supports_termination: Some(row.get("supports_termination")),
        depends_on_job_ids: None,
        input_file_ids: None,
        output_file_ids: None,
        input_user_data_ids: None,
        output_user_data_ids: None,
        resource_requirements_id: Some(row.get("resource_requirements_id")),
        scheduler_id: None,
        failure_handler_id: row.get("failure_handler_id"),
        attempt_id: row.get("attempt_id"),
        priority: Some(row.get("priority")),
    });

    Ok(true)
}

async fn claim_backfill_jobs(
    conn: &mut sqlx::SqliteConnection,
    params: &BackfillClaimParams,
    packing_state: &mut ClaimPackingState,
    selected_jobs: &mut Vec<models::JobModel>,
    job_ids_to_update: &mut Vec<i64>,
) -> Result<(), ApiError> {
    if selected_jobs.len() >= params.claim_limit {
        return Ok(());
    }

    let remaining = packing_state.remaining_resources();
    let remaining_limit = params.claim_limit - selected_jobs.len();
    if remaining_limit == 0 || remaining.nodes <= 0 || remaining.cpus <= 0 {
        return Ok(());
    }

    let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
        r#"
        SELECT
            job.workflow_id,
            job.id AS job_id,
            job.name,
            job.command,
            job.invocation_script,
            job.env,
            job.status,
            job.cancel_on_blocking_job_failure,
            job.supports_termination,
            job.failure_handler_id,
            job.attempt_id,
            job.priority,
            rr.id AS resource_requirements_id,
            rr.memory_bytes,
            rr.num_cpus,
            rr.num_gpus,
            rr.num_nodes,
            rr.runtime_s
        FROM job
        JOIN resource_requirements rr ON job.resource_requirements_id = rr.id
        WHERE job.workflow_id =
        "#,
    );
    builder
        .push_bind(params.workflow_id)
        .push(" AND job.status = ")
        .push_bind(params.ready_status)
        .push(" AND rr.memory_bytes <= ")
        .push_bind(remaining.memory_bytes)
        .push(" AND rr.num_cpus <= ")
        .push_bind(remaining.cpus)
        .push(" AND rr.num_gpus <= ")
        .push_bind(remaining.gpus)
        .push(" AND rr.memory_bytes <= ")
        .push_bind(packing_state.per_node_memory)
        .push(" AND rr.num_cpus <= ")
        .push_bind(packing_state.per_node_cpus)
        .push(" AND rr.num_gpus <= ")
        .push_bind(packing_state.per_node_gpus)
        .push(" AND rr.num_nodes <= ")
        .push_bind(remaining.nodes)
        .push(" AND rr.runtime_s <= ")
        .push_bind(params.time_limit_seconds);

    if params.use_scheduler_filter {
        builder
            .push(" AND (job.scheduler_id IS NULL OR job.scheduler_id = ")
            .push_bind(params.scheduler_config_id)
            .push(")");
    }

    if !job_ids_to_update.is_empty() {
        builder.push(" AND job.id NOT IN (");
        let mut separated = builder.separated(", ");
        for job_id in job_ids_to_update.iter() {
            separated.push_bind(job_id);
        }
        separated.push_unseparated(")");
    }

    builder.push(" ");
    builder.push(RESOURCE_CLAIM_ORDER_BY);
    builder.push(" LIMIT ");
    builder.push_bind(remaining_limit as i64);

    let backfill_rows = builder.build().fetch_all(&mut *conn).await.map_err(|e| {
        error!("Database error in get_ready_jobs backfill query: {}", e);
        ApiError("Database error".to_string())
    })?;

    debug!(
        "get_ready_jobs: Found {} backfill candidates for workflow {} with remaining resources: cpus={}, memory_bytes={}, gpus={}, nodes={}",
        backfill_rows.len(),
        params.workflow_id,
        remaining.cpus,
        remaining.memory_bytes,
        remaining.gpus,
        remaining.nodes
    );

    let primary_selected = selected_jobs.len();
    for row in backfill_rows {
        if selected_jobs.len() >= params.claim_limit {
            break;
        }
        claim_candidate_row(&row, packing_state, selected_jobs, job_ids_to_update)?;
    }
    let remaining_after = packing_state.remaining_resources();
    debug!(
        "get_ready_jobs backfill result: workflow_id={} primary_selected={} backfill_selected={} remaining_after_cpus={} remaining_after_memory_bytes={} remaining_after_gpus={} remaining_after_nodes={}",
        params.workflow_id,
        primary_selected,
        selected_jobs.len().saturating_sub(primary_selected),
        remaining_after.cpus,
        remaining_after.memory_bytes,
        remaining_after.gpus,
        remaining_after.nodes
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    pub(super) async fn transport_create_job(
        &self,
        mut job: models::JobModel,
        context: &C,
    ) -> Result<CreateJobResponse, ApiError> {
        authorize_workflow!(self, job.workflow_id, context, CreateJobResponse);

        if job.resource_requirements_id.is_none() {
            let default_id = self
                .get_default_resource_requirements_id(job.workflow_id, context)
                .await?;
            job.resource_requirements_id = Some(default_id);
        }

        self.jobs_api.create_job(job, context).await
    }

    pub(super) async fn transport_create_jobs(
        &self,
        mut body: models::JobsModel,
        context: &C,
    ) -> Result<CreateJobsResponse, ApiError> {
        if body.jobs.is_empty() {
            return self.jobs_api.create_jobs(body, context).await;
        }

        let first_workflow_id = body.jobs[0].workflow_id;
        for job in &body.jobs {
            if job.workflow_id != first_workflow_id {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!(
                        "All jobs in a batch must have the same workflow_id. Found workflow_ids: {} and {}",
                        first_workflow_id, job.workflow_id
                    )
                }));
                return Ok(CreateJobsResponse::UnprocessableContentErrorResponse(
                    error_response,
                ));
            }
        }

        authorize_workflow!(self, first_workflow_id, context, CreateJobsResponse);

        let default_resource_requirements_id = self
            .get_default_resource_requirements_id(first_workflow_id, context)
            .await?;

        for job in &mut body.jobs {
            if job.resource_requirements_id.is_none() {
                job.resource_requirements_id = Some(default_resource_requirements_id);
            }
        }

        self.jobs_api.create_jobs(body, context).await
    }

    pub(super) async fn transport_initialize_jobs(
        &self,
        id: i64,
        only_uninitialized: Option<bool>,
        clear_ephemeral_user_data: Option<bool>,
        async_: Option<bool>,
        context: &C,
    ) -> Result<InitializeJobsResponse, ApiError> {
        info!(
            "initialize_jobs({}, {:?}, {:?}, async={:?}) - X-Span-ID: {:?}",
            id,
            only_uninitialized,
            clear_ephemeral_user_data,
            async_,
            Has::<XSpanIdString>::get(context).0.clone()
        );
        authorize_workflow!(self, id, context, InitializeJobsResponse);

        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        let username = auth
            .map(|a| a.subject)
            .unwrap_or_else(|| "unknown".to_string());

        if async_.unwrap_or(false) {
            let outcome = match self
                .create_or_get_initialize_jobs_task(
                    id,
                    only_uninitialized,
                    clear_ephemeral_user_data,
                    Some(username.clone()),
                )
                .await
            {
                Ok(outcome) => outcome,
                Err(CreateTaskError::Conflict {
                    existing_task_id,
                    existing_operation,
                    reason,
                }) => {
                    let payload = serde_json::json!({
                        "error": "Conflict",
                        "message": reason,
                        "existing_task_id": existing_task_id,
                        "existing_operation": existing_operation,
                    });
                    return Ok(InitializeJobsResponse::ConflictErrorResponse(
                        models::ErrorResponse::new(payload),
                    ));
                }
                Err(CreateTaskError::Api(err)) => return Err(err),
            };

            let task = match outcome {
                TaskCreation::Created(task) => {
                    let server = self.clone();
                    let task_id = task.id;
                    tokio::spawn(async move {
                        server
                            .run_initialize_jobs_task(
                                task_id,
                                id,
                                only_uninitialized,
                                clear_ephemeral_user_data,
                                username,
                            )
                            .await;
                    });
                    task
                }
                TaskCreation::Existing(task) => task,
            };

            return Ok(InitializeJobsResponse::AcceptedResponse(task));
        }

        self.initialize_jobs_core(id, only_uninitialized, clear_ephemeral_user_data, username)
            .await?;

        Ok(InitializeJobsResponse::SuccessfulResponse(
            serde_json::json!({"message": "Initialized job status"}),
        ))
    }

    pub(super) async fn transport_delete_jobs(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteJobsResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteJobsResponse);
        self.jobs_api.delete_jobs(workflow_id, context).await
    }

    pub(super) async fn transport_list_jobs(
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
        authorize_workflow!(self, workflow_id, context, ListJobsResponse);
        let (processed_offset, processed_limit) = process_pagination_params(offset, limit)?;
        self.jobs_api
            .list_jobs(
                workflow_id,
                status,
                needs_file_id,
                upstream_job_id,
                processed_offset,
                processed_limit,
                sort_by,
                reverse_sort,
                include_relationships,
                active_compute_node_id,
                context,
            )
            .await
    }

    pub(super) async fn transport_get_job(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetJobResponse, ApiError> {
        authorize_job!(self, id, context, GetJobResponse);
        self.jobs_api.get_job(id, context).await
    }

    pub(super) async fn transport_list_job_ids(
        &self,
        id: i64,
        context: &C,
    ) -> Result<ListJobIdsResponse, ApiError> {
        authorize_workflow!(self, id, context, ListJobIdsResponse);
        self.jobs_api.list_job_ids(id, context).await
    }

    pub(super) async fn transport_update_job(
        &self,
        id: i64,
        body: models::JobModel,
        context: &C,
    ) -> Result<UpdateJobResponse, ApiError> {
        authorize_job!(self, id, context, UpdateJobResponse);
        self.jobs_api.update_job(id, body, context).await
    }

    pub(super) async fn transport_claim_next_jobs(
        &self,
        id: i64,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ClaimNextJobsResponse, ApiError> {
        debug!(
            "claim_next_jobs({}, {:?}) - X-Span-ID: {:?}",
            id,
            limit,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        authorize_workflow!(self, id, context, ClaimNextJobsResponse);

        let requested_limit = limit.unwrap_or(10);
        self.jobs_api
            .claim_next_jobs(id, requested_limit, context)
            .await
    }

    pub(super) async fn transport_process_changed_job_inputs(
        &self,
        id: i64,
        dry_run: Option<bool>,
        context: &C,
    ) -> Result<ProcessChangedJobInputsResponse, ApiError> {
        authorize_workflow!(self, id, context, ProcessChangedJobInputsResponse);
        let dry_run_value = dry_run.unwrap_or(false);
        self.jobs_api
            .process_changed_job_inputs(id, dry_run_value, context)
            .await
    }

    pub(super) async fn transport_retry_job(
        &self,
        id: i64,
        run_id: i64,
        max_retries: i32,
        context: &C,
    ) -> Result<RetryJobResponse, ApiError> {
        authorize_job!(self, id, context, RetryJobResponse);
        self.jobs_api
            .retry_job(id, run_id, max_retries, context)
            .await
    }

    pub(super) async fn transport_delete_job(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteJobResponse, ApiError> {
        authorize_job!(self, id, context, DeleteJobResponse);
        self.jobs_api.delete_job(id, context).await
    }

    pub(super) async fn transport_reset_job_status(
        &self,
        id: i64,
        failed_only: Option<bool>,
        context: &C,
    ) -> Result<ResetJobStatusResponse, ApiError> {
        info!(
            "reset_job_status(workflow_id={}, failed_only={:?}) - X-Span-ID: {:?}",
            id,
            failed_only,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        authorize_workflow!(self, id, context, ResetJobStatusResponse);

        let failed_only_value = failed_only.unwrap_or(false);
        let result = self
            .jobs_api
            .reset_job_status(id, failed_only_value, context)
            .await?;

        if let ResetJobStatusResponse::SuccessfulResponse(ref response) = result {
            let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
            let username = auth
                .map(|a| a.subject)
                .unwrap_or_else(|| "unknown".to_string());

            let event = models::EventModel::new(
                id,
                serde_json::json!({
                    "category": "user_action",
                    "action": "reset_job_status",
                    "user": username,
                    "workflow_id": id,
                    "failed_only": failed_only_value,
                    "updated_count": response.updated_count,
                }),
            );
            if let Err(e) = self.events_api.create_event(event, context).await {
                error!("Failed to create event for reset_job_status: {:?}", e);
            }
        }

        Ok(result)
    }

    pub(super) async fn transport_manage_status_change(
        &self,
        id: i64,
        status: models::JobStatus,
        run_id: i64,
        context: &C,
    ) -> Result<ManageStatusChangeResponse, ApiError> {
        debug!(
            "manage_status_change({}, {:?}, {}) - X-Span-ID: {:?}",
            id,
            status,
            run_id,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        if status.is_complete() {
            error!(
                "manage_status_change: cannot set completion status '{}' for job_id={}. Use complete_job instead.",
                status, id
            );
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "Cannot set completion status '{}' via manage_status_change. Use complete_job API instead.",
                    status
                )
            }));
            return Ok(
                ManageStatusChangeResponse::UnprocessableContentErrorResponse(error_response),
            );
        }

        authorize_job!(self, id, context, ManageStatusChangeResponse);

        let mut job = match self.jobs_api.get_job(id, context).await? {
            GetJobResponse::SuccessfulResponse(job) => job,
            GetJobResponse::ForbiddenErrorResponse(err) => {
                return Ok(ManageStatusChangeResponse::DefaultErrorResponse(err));
            }
            GetJobResponse::NotFoundErrorResponse(err) => {
                return Ok(ManageStatusChangeResponse::NotFoundErrorResponse(err));
            }
            GetJobResponse::DefaultErrorResponse(err) => {
                return Ok(ManageStatusChangeResponse::DefaultErrorResponse(err));
            }
        };

        let current_status = *job.status.as_ref().ok_or_else(|| {
            error!("Job status is missing for job_id={}", id);
            ApiError("Job status is required".to_string())
        })?;

        if current_status == status {
            debug!(
                "manage_status_change: job_id={} already has status '{}', no change needed",
                id, status
            );
            return Ok(ManageStatusChangeResponse::SuccessfulResponse(job));
        }

        if let Err(e) = self.validate_run_id(job.workflow_id, run_id).await {
            error!("manage_status_change: job_id={}, {}", id, e);
            let error_response = models::ErrorResponse::new(serde_json::json!({ "message": e }));
            return Ok(
                ManageStatusChangeResponse::UnprocessableContentErrorResponse(error_response),
            );
        }

        job.status = Some(status);

        let new_status_int = status.to_int();
        let current_status_int = current_status.to_int();
        let update_result = sqlx::query!(
            "UPDATE job SET status = ? WHERE id = ? AND status = ?",
            new_status_int,
            id,
            current_status_int,
        )
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| {
            error!("Failed to update job status: {}", e);
            ApiError("Database error".to_string())
        })?;

        if update_result.rows_affected() == 0 {
            let exists = sqlx::query_scalar!("SELECT id FROM job WHERE id = ?", id)
                .fetch_optional(self.pool.as_ref())
                .await
                .map_err(|e| {
                    error!("Failed to check job existence: {}", e);
                    ApiError("Database error".to_string())
                })?;

            if exists.is_none() {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Job not found with ID: {}", id)
                }));
                return Ok(ManageStatusChangeResponse::NotFoundErrorResponse(
                    error_response,
                ));
            }

            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "Job {} status was concurrently modified (expected '{}'), please retry",
                    id, current_status
                )
            }));
            return Ok(
                ManageStatusChangeResponse::UnprocessableContentErrorResponse(error_response),
            );
        }

        let workflow_id = job.workflow_id;

        let updated_job = match self.get_job(id, context).await? {
            GetJobResponse::SuccessfulResponse(fresh_job) => fresh_job,
            _ => {
                job.status = Some(status);
                job
            }
        };

        if current_status.is_complete()
            && status == models::JobStatus::Uninitialized
            && let Err(e) = self.reinitialize_downstream_jobs(id, workflow_id).await
        {
            error!(
                "Failed to reinitialize downstream jobs for job {}: {}",
                id, e
            );
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": "Failed to reinitialize downstream jobs"
            }));
            return Ok(ManageStatusChangeResponse::DefaultErrorResponse(
                error_response,
            ));
        }

        debug!(
            "manage_status_change: successfully changed job_id={} status from '{}' to '{}'",
            id, current_status, status
        );

        Ok(ManageStatusChangeResponse::SuccessfulResponse(updated_job))
    }

    pub(super) async fn transport_start_job(
        &self,
        id: i64,
        run_id: i64,
        compute_node_id: i64,
        context: &C,
    ) -> Result<StartJobResponse, ApiError> {
        debug!(
            "start_job({}, {}, {}) - X-Span-ID: {:?}",
            id,
            run_id,
            compute_node_id,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        authorize_job!(self, id, context, StartJobResponse);

        let mut job = match self.jobs_api.get_job(id, context).await? {
            GetJobResponse::SuccessfulResponse(job) => job,
            GetJobResponse::ForbiddenErrorResponse(err) => {
                error!("Access denied for job {}: {:?}", id, err);
                return Ok(StartJobResponse::ForbiddenErrorResponse(err));
            }
            GetJobResponse::NotFoundErrorResponse(err) => {
                error!("Job not found {}: {:?}", id, err);
                return Ok(StartJobResponse::NotFoundErrorResponse(err));
            }
            GetJobResponse::DefaultErrorResponse(err) => {
                error!("Failed to get job {}: {:?}", id, err);
                return Ok(StartJobResponse::DefaultErrorResponse(err));
            }
        };
        match job.status {
            Some(models::JobStatus::Pending) => {
                job.status = Some(models::JobStatus::Running);
            }
            Some(status) => {
                error!(
                    "start_job: Invalid job status for job_id={}. Expected SubmittedPending, got {:?}",
                    id, status
                );
                return Err(ApiError(format!(
                    "Job {} has invalid status {:?}. Expected SubmittedPending for job start.",
                    id, status
                )));
            }
            None => {
                error!("start_job: Job status not set for job_id={}", id);
                return Err(ApiError(format!(
                    "Job {} has no status set. Expected SubmittedPending for job start.",
                    id
                )));
            }
        }

        if let Err(e) = self.validate_run_id(job.workflow_id, run_id).await {
            error!("start_job: job_id={}, {}", id, e);
            let error_response = models::ErrorResponse::new(serde_json::json!({ "message": e }));
            return Ok(StartJobResponse::UnprocessableContentErrorResponse(
                error_response,
            ));
        }

        let pending_int = models::JobStatus::Pending.to_int();
        let running_int = models::JobStatus::Running.to_int();
        let start_result = sqlx::query!(
            "UPDATE job SET status = ? WHERE id = ? AND status = ?",
            running_int,
            id,
            pending_int,
        )
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| {
            error!("Failed to update job status for start_job: {}", e);
            ApiError("Database error".to_string())
        })?;

        if start_result.rows_affected() == 0 {
            error!(
                "start_job: job_id={} status was concurrently changed from Pending, cannot start",
                id
            );
            return Err(ApiError(format!(
                "Job {} status was concurrently modified, cannot start",
                id
            )));
        }

        match sqlx::query!(
            "UPDATE job_internal SET active_compute_node_id = ? WHERE job_id = ?",
            compute_node_id,
            id
        )
        .execute(self.pool.as_ref())
        .await
        {
            Ok(_) => {
                debug!(
                    "Set active_compute_node_id={} for job_id={}",
                    compute_node_id, id
                );
            }
            Err(e) => {
                error!(
                    "Failed to set active_compute_node_id for job_id={}: {}",
                    id, e
                );
            }
        }

        self.event_broadcaster.broadcast(BroadcastEvent {
            workflow_id: job.workflow_id,
            timestamp: chrono::Utc::now().timestamp_millis(),
            event_type: "job_started".to_string(),
            severity: models::EventSeverity::Info,
            data: serde_json::json!({
                "job_id": id,
                "job_name": job.name,
                "compute_node_id": compute_node_id,
                "run_id": run_id,
            }),
        });
        debug!("Broadcast job_started event for job_id={}", id);

        Ok(StartJobResponse::SuccessfulResponse(job))
    }

    pub(super) async fn transport_complete_job(
        &self,
        id: i64,
        status: models::JobStatus,
        run_id: i64,
        result: models::ResultModel,
        context: &C,
    ) -> Result<CompleteJobResponse, ApiError> {
        debug!(
            "complete_job({}, {:?}, {}, {:?}) - X-Span-ID: {:?}",
            id,
            status,
            run_id,
            result,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        authorize_job!(self, id, context, CompleteJobResponse);

        match self
            .apply_job_completion_state(None, id, status, run_id, result, context)
            .await
        {
            Ok(completion) => {
                let job = completion.job.clone();
                self.finalize_completed_jobs(completion.workflow_id, &[completion], context)
                    .await;
                Ok(CompleteJobResponse::SuccessfulResponse(job))
            }
            Err(CompletionMutationError::Response(response)) => Ok(*response),
            Err(CompletionMutationError::Transport(error)) => Err(error),
        }
    }

    async fn apply_job_completion_state(
        &self,
        expected_workflow_id: Option<i64>,
        id: i64,
        status: models::JobStatus,
        run_id: i64,
        result: models::ResultModel,
        context: &C,
    ) -> Result<CompletedJobRecord, CompletionMutationError> {
        if !status.is_terminal() {
            error!(
                "Attempted to complete job {} with non-terminal status '{}'",
                id, status
            );
            return Err(CompletionMutationError::Transport(ApiError(format!(
                "Status '{}' is not a terminal status for job completion",
                status
            ))));
        }

        let mut job = match self.jobs_api.get_job(id, context).await {
            Ok(response) => match response {
                GetJobResponse::SuccessfulResponse(job) => job,
                GetJobResponse::ForbiddenErrorResponse(err) => {
                    error!("Access denied for job {}: {:?}", id, err);
                    return Err(CompletionMutationError::Response(Box::new(
                        CompleteJobResponse::ForbiddenErrorResponse(err),
                    )));
                }
                GetJobResponse::NotFoundErrorResponse(err) => {
                    error!("Job not found {}: {:?}", id, err);
                    return Err(CompletionMutationError::Response(Box::new(
                        CompleteJobResponse::NotFoundErrorResponse(err),
                    )));
                }
                GetJobResponse::DefaultErrorResponse(err) => {
                    error!("Failed to get job {}: {:?}", id, err);
                    return Err(CompletionMutationError::Response(Box::new(
                        CompleteJobResponse::DefaultErrorResponse(err),
                    )));
                }
            },
            Err(error) => return Err(CompletionMutationError::Transport(error)),
        };

        if let Some(expected_workflow_id) = expected_workflow_id
            && job.workflow_id != expected_workflow_id
        {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "Job {} belongs to workflow {} but batch target is workflow {}",
                    id, job.workflow_id, expected_workflow_id
                )
            }));
            return Err(CompletionMutationError::Response(Box::new(
                CompleteJobResponse::UnprocessableContentErrorResponse(error_response),
            )));
        }

        if let Some(current_status) = &job.status
            && current_status.is_complete()
        {
            error!(
                "Job {} is already complete with status {:?}",
                id, current_status
            );
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Job {} is already complete with status {:?}", id, current_status)
            }));
            return Err(CompletionMutationError::Response(Box::new(
                CompleteJobResponse::UnprocessableContentErrorResponse(error_response),
            )));
        }

        if result.job_id != id {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "ResultModel job_id {} does not match target job_id {}",
                    result.job_id, id
                )
            }));
            return Err(CompletionMutationError::Response(Box::new(
                CompleteJobResponse::UnprocessableContentErrorResponse(error_response),
            )));
        }
        if result.workflow_id != job.workflow_id {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "ResultModel workflow_id {} does not match job's workflow_id {}",
                    result.workflow_id, job.workflow_id
                )
            }));
            return Err(CompletionMutationError::Response(Box::new(
                CompleteJobResponse::UnprocessableContentErrorResponse(error_response),
            )));
        }
        if result.status != status {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "ResultModel status '{}' does not match target status '{}'",
                    result.status, status
                )
            }));
            return Err(CompletionMutationError::Response(Box::new(
                CompleteJobResponse::UnprocessableContentErrorResponse(error_response),
            )));
        }
        if result.run_id != run_id {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "ResultModel run_id {} does not match target run_id {}",
                    result.run_id, run_id
                )
            }));
            return Err(CompletionMutationError::Response(Box::new(
                CompleteJobResponse::UnprocessableContentErrorResponse(error_response),
            )));
        }

        job.status = Some(status);

        match sqlx::query!(
            "UPDATE job_internal SET active_compute_node_id = NULL WHERE job_id = ?",
            id
        )
        .execute(self.pool.as_ref())
        .await
        {
            Ok(_) => {
                debug!("Cleared active_compute_node_id for job_id={}", id);
            }
            Err(e) => {
                error!(
                    "Failed to clear active_compute_node_id for job_id={}: {}",
                    id, e
                );
            }
        }

        let result_return_code = result.return_code;
        let result_response = self
            .results_api
            .create_result(result, context)
            .await
            .map_err(CompletionMutationError::Transport)?;

        let result_id = match result_response {
            CreateResultResponse::SuccessfulResponse(result) => {
                debug!(
                    "complete_job: added result with ID {:?} for job_id={}",
                    result.id, id
                );
                result.id
            }
            CreateResultResponse::ForbiddenErrorResponse(err) => {
                error!("Forbidden to add result for job {}: {:?}", id, err);
                return Err(CompletionMutationError::Response(Box::new(
                    CompleteJobResponse::ForbiddenErrorResponse(err),
                )));
            }
            CreateResultResponse::NotFoundErrorResponse(err) => {
                error!("Failed to add result for job {}: {:?}", id, err);
                return Err(CompletionMutationError::Response(Box::new(
                    CompleteJobResponse::NotFoundErrorResponse(err),
                )));
            }
            CreateResultResponse::DefaultErrorResponse(err) => {
                error!("Failed to add result for job {}: {:?}", id, err);
                return Err(CompletionMutationError::Response(Box::new(
                    CompleteJobResponse::DefaultErrorResponse(err),
                )));
            }
        };

        let workflow_id = job.workflow_id;
        let result_id_value = result_id.ok_or_else(|| {
            error!("Result ID is missing after creating result");
            CompletionMutationError::Transport(ApiError("Result ID is missing".to_string()))
        })?;

        match sqlx::query!(
            r#"
            INSERT OR REPLACE INTO workflow_result (workflow_id, job_id, result_id)
            VALUES (?, ?, ?)
            "#,
            workflow_id,
            id,
            result_id_value
        )
        .execute(self.pool.as_ref())
        .await
        {
            Ok(_) => {
                debug!(
                    "complete_job: added workflow_result record for workflow_id={}, job_id={}, result_id={}",
                    workflow_id, id, result_id_value
                );
            }
            Err(e) => {
                error!(
                    "Failed to insert workflow_result for workflow_id={}, job_id={}, result_id={}: {}",
                    workflow_id, id, result_id_value, e
                );
                return Err(CompletionMutationError::Transport(ApiError(
                    "Database error".to_string(),
                )));
            }
        }

        self.manage_job_status_change(&job, run_id)
            .await
            .map_err(CompletionMutationError::Transport)?;

        Ok(CompletedJobRecord {
            job,
            job_id: id,
            workflow_id,
            status,
            result_return_code,
            result_id: result_id_value,
        })
    }

    async fn apply_job_completion_state_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        expected_workflow_id: Option<i64>,
        id: i64,
        status: models::JobStatus,
        run_id: i64,
        result: models::ResultModel,
        context: &C,
    ) -> Result<CompletedJobRecord, CompletionMutationError> {
        if !status.is_terminal() {
            return Err(CompletionMutationError::Transport(ApiError(format!(
                "Status '{}' is not a terminal status for job completion",
                status
            ))));
        }

        let job = match self.jobs_api.get_job(id, context).await {
            Ok(response) => match response {
                GetJobResponse::SuccessfulResponse(job) => job,
                GetJobResponse::ForbiddenErrorResponse(err) => {
                    return Err(CompletionMutationError::Response(Box::new(
                        CompleteJobResponse::ForbiddenErrorResponse(err),
                    )));
                }
                GetJobResponse::NotFoundErrorResponse(err) => {
                    return Err(CompletionMutationError::Response(Box::new(
                        CompleteJobResponse::NotFoundErrorResponse(err),
                    )));
                }
                GetJobResponse::DefaultErrorResponse(err) => {
                    return Err(CompletionMutationError::Response(Box::new(
                        CompleteJobResponse::DefaultErrorResponse(err),
                    )));
                }
            },
            Err(error) => return Err(CompletionMutationError::Transport(error)),
        };

        if let Some(expected_workflow_id) = expected_workflow_id
            && job.workflow_id != expected_workflow_id
        {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "Job {} belongs to workflow {} but batch target is workflow {}",
                    id, job.workflow_id, expected_workflow_id
                )
            }));
            return Err(CompletionMutationError::Response(Box::new(
                CompleteJobResponse::UnprocessableContentErrorResponse(error_response),
            )));
        }

        if let Some(current_status) = &job.status
            && current_status.is_complete()
        {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Job {} is already complete with status {:?}", id, current_status)
            }));
            return Err(CompletionMutationError::Response(Box::new(
                CompleteJobResponse::UnprocessableContentErrorResponse(error_response),
            )));
        }

        if result.job_id != id {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "ResultModel job_id {} does not match target job_id {}",
                    result.job_id, id
                )
            }));
            return Err(CompletionMutationError::Response(Box::new(
                CompleteJobResponse::UnprocessableContentErrorResponse(error_response),
            )));
        }
        if result.workflow_id != job.workflow_id {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "ResultModel workflow_id {} does not match job's workflow_id {}",
                    result.workflow_id, job.workflow_id
                )
            }));
            return Err(CompletionMutationError::Response(Box::new(
                CompleteJobResponse::UnprocessableContentErrorResponse(error_response),
            )));
        }
        if result.status != status {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "ResultModel status '{}' does not match target status '{}'",
                    result.status, status
                )
            }));
            return Err(CompletionMutationError::Response(Box::new(
                CompleteJobResponse::UnprocessableContentErrorResponse(error_response),
            )));
        }
        if result.run_id != run_id {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "ResultModel run_id {} does not match target run_id {}",
                    result.run_id, run_id
                )
            }));
            return Err(CompletionMutationError::Response(Box::new(
                CompleteJobResponse::UnprocessableContentErrorResponse(error_response),
            )));
        }

        if let Err(e) = self.validate_run_id(job.workflow_id, run_id).await {
            let error_response = models::ErrorResponse::new(serde_json::json!({ "message": e }));
            return Err(CompletionMutationError::Response(Box::new(
                CompleteJobResponse::UnprocessableContentErrorResponse(error_response),
            )));
        }

        if let Err(e) = sqlx::query!(
            "UPDATE job_internal SET active_compute_node_id = NULL WHERE job_id = ?",
            id
        )
        .execute(&mut **tx)
        .await
        {
            error!(
                "Failed to clear active_compute_node_id for job_id={}: {}",
                id, e
            );
        }

        let result_return_code = result.return_code;
        let attempt_id = result.attempt_id.unwrap_or(1);
        let status_int = result.status.to_int();
        let result_row = sqlx::query!(
            r#"
            INSERT INTO result
            (
                job_id
                ,workflow_id
                ,run_id
                ,attempt_id
                ,compute_node_id
                ,return_code
                ,exec_time_minutes
                ,completion_time
                ,status
                ,peak_memory_bytes
                ,avg_memory_bytes
                ,peak_cpu_percent
                ,avg_cpu_percent
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING rowid
        "#,
            result.job_id,
            result.workflow_id,
            result.run_id,
            attempt_id,
            result.compute_node_id,
            result.return_code,
            result.exec_time_minutes,
            result.completion_time,
            status_int,
            result.peak_memory_bytes,
            result.avg_memory_bytes,
            result.peak_cpu_percent,
            result.avg_cpu_percent,
        )
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| {
            CompletionMutationError::Transport(database_error_with_msg(
                e,
                "Failed to create result record",
            ))
        })?;

        let result_id_value = result_row.id;
        sqlx::query!(
            r#"
            INSERT OR REPLACE INTO workflow_result (workflow_id, job_id, result_id)
            VALUES (?, ?, ?)
            "#,
            job.workflow_id,
            id,
            result_id_value
        )
        .execute(&mut **tx)
        .await
        .map_err(|e| {
            CompletionMutationError::Transport(database_error_with_msg(
                e,
                "Failed to create workflow_result record",
            ))
        })?;

        let new_status_int = status.to_int();
        let completed_int = models::JobStatus::Completed.to_int();
        let failed_int = models::JobStatus::Failed.to_int();
        let canceled_int = models::JobStatus::Canceled.to_int();
        let terminated_int = models::JobStatus::Terminated.to_int();
        let disabled_int = models::JobStatus::Disabled.to_int();
        let pending_failed_int = models::JobStatus::PendingFailed.to_int();
        let update_result = sqlx::query!(
            "UPDATE job SET status = ?, unblocking_processed = 0 WHERE id = ? AND status NOT IN (?, ?, ?, ?, ?, ?)",
            new_status_int,
            id,
            completed_int,
            failed_int,
            canceled_int,
            terminated_int,
            disabled_int,
            pending_failed_int,
        )
        .execute(&mut **tx)
        .await
        .map_err(|e| CompletionMutationError::Transport(database_error_with_msg(e, "Failed to update job status")))?;

        if update_result.rows_affected() == 0 {
            let current = sqlx::query_scalar!("SELECT status FROM job WHERE id = ?", id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| {
                    CompletionMutationError::Transport(database_error_with_msg(
                        e,
                        "Failed to re-check job status",
                    ))
                })?;

            match current {
                Some(status_int) => {
                    let current_status = models::JobStatus::from_int(status_int as i32)
                        .unwrap_or(models::JobStatus::Failed);
                    if current_status.is_complete() {
                        let error_response = models::ErrorResponse::new(serde_json::json!({
                            "message": format!("Job {} is already complete with status {:?}", id, current_status)
                        }));
                        return Err(CompletionMutationError::Response(Box::new(
                            CompleteJobResponse::UnprocessableContentErrorResponse(error_response),
                        )));
                    }
                    return Err(CompletionMutationError::Transport(ApiError(format!(
                        "Job {} is in unexpected status {:?}",
                        id, current_status
                    ))));
                }
                None => {
                    return Err(CompletionMutationError::Transport(ApiError(format!(
                        "Job {} not found",
                        id
                    ))));
                }
            }
        }

        let mut completed_job = job.clone();
        completed_job.status = Some(status);

        Ok(CompletedJobRecord {
            job: completed_job,
            job_id: id,
            workflow_id: job.workflow_id,
            status,
            result_return_code,
            result_id: result_id_value,
        })
    }

    async fn finalize_completed_jobs(
        &self,
        workflow_id: i64,
        completions: &[CompletedJobRecord],
        context: &C,
    ) {
        if completions.is_empty() {
            return;
        }

        let mut completed_job_ids = Vec::with_capacity(completions.len());
        for completion in completions {
            let event_type = format!("job_{}", completion.status.to_string().to_lowercase());
            let severity = match completion.status {
                models::JobStatus::Completed => models::EventSeverity::Info,
                models::JobStatus::Failed => models::EventSeverity::Error,
                models::JobStatus::Terminated | models::JobStatus::Canceled => {
                    models::EventSeverity::Warning
                }
                _ => models::EventSeverity::Info,
            };
            self.event_broadcaster.broadcast(BroadcastEvent {
                workflow_id: completion.workflow_id,
                timestamp: chrono::Utc::now().timestamp_millis(),
                event_type,
                severity,
                data: serde_json::json!({
                    "job_id": completion.job_id,
                    "job_name": completion.job.name,
                    "status": completion.status.to_string(),
                    "return_code": completion.result_return_code,
                }),
            });
            debug!(
                "Broadcast job completion event for job_id={}",
                completion.job_id
            );
            debug!(
                "complete_job: successfully completed job_id={} with status={}, result_id={}",
                completion.job_id, completion.status, completion.result_id
            );
            completed_job_ids.push(completion.job_id);
        }

        if let Err(e) = self
            .workflow_actions_api
            .check_and_trigger_actions(
                workflow_id,
                "on_jobs_complete",
                Some(completed_job_ids.clone()),
            )
            .await
        {
            error!(
                "Failed to check_and_trigger_actions for on_jobs_complete: {}",
                e
            );
        }

        match self
            .workflows_api
            .is_workflow_complete(workflow_id, context)
            .await
        {
            Ok(response) => {
                if let IsWorkflowCompleteResponse::SuccessfulResponse(completion_status) = response
                    && completion_status.is_complete
                {
                    debug!(
                        "Workflow {} is complete, triggering on_workflow_complete actions",
                        workflow_id
                    );
                    if let Err(e) = self
                        .workflow_actions_api
                        .check_and_trigger_actions(workflow_id, "on_workflow_complete", None)
                        .await
                    {
                        error!(
                            "Failed to check_and_trigger_actions for on_workflow_complete: {}",
                            e
                        );
                    }
                }
            }
            Err(e) => {
                error!(
                    "Failed to check if workflow {} is complete: {}",
                    workflow_id, e
                );
            }
        }
    }

    pub(super) async fn transport_batch_complete_jobs(
        &self,
        workflow_id: i64,
        body: models::BatchCompleteJobsRequest,
        context: &C,
    ) -> Result<BatchCompleteJobsResponse, ApiError> {
        debug!(
            "batch_complete_jobs(workflow_id={}, count={}) - X-Span-ID: {:?}",
            workflow_id,
            body.completions.len(),
            Has::<XSpanIdString>::get(context).0.clone()
        );

        authorize_workflow!(self, workflow_id, context, BatchCompleteJobsResponse);

        let mut completed = Vec::new();
        let mut errors = Vec::new();
        let mut completion_records = Vec::new();
        let mut tx = self.pool.begin().await.map_err(|e| {
            database_error_with_msg(e, "Failed to begin batch completion transaction")
        })?;

        for entry in body.completions {
            let job_id = entry.job_id;
            match self
                .apply_job_completion_state_tx(
                    &mut tx,
                    Some(workflow_id),
                    job_id,
                    entry.status,
                    entry.run_id,
                    entry.result,
                    context,
                )
                .await
            {
                Ok(completion) => {
                    completed.push(job_id);
                    completion_records.push(completion);
                }
                Err(CompletionMutationError::Response(response)) => match *response {
                    CompleteJobResponse::ForbiddenErrorResponse(err)
                    | CompleteJobResponse::NotFoundErrorResponse(err)
                    | CompleteJobResponse::UnprocessableContentErrorResponse(err)
                    | CompleteJobResponse::DefaultErrorResponse(err) => {
                        let message = completion_error_message(&err);
                        errors.push(models::JobCompletionError { job_id, message });
                    }
                    CompleteJobResponse::SuccessfulResponse(_) => {
                        unreachable!("successful completion should not be returned as an error")
                    }
                },
                Err(CompletionMutationError::Transport(error)) => {
                    let _ = tx.rollback().await;
                    return Err(error);
                }
            }
        }

        tx.commit().await.map_err(|e| {
            database_error_with_msg(e, "Failed to commit batch completion transaction")
        })?;

        if !completion_records.is_empty() {
            self.signal_job_completion();
        }

        self.finalize_completed_jobs(workflow_id, &completion_records, context)
            .await;

        Ok(BatchCompleteJobsResponse::SuccessfulResponse(
            models::BatchCompleteJobsResponse { completed, errors },
        ))
    }

    pub(super) async fn transport_prepare_ready_jobs(
        &self,
        workflow_id: i64,
        resources: models::ComputeNodesResources,
        limit: i64,
        strict_scheduler_match: Option<bool>,
        context: &C,
    ) -> Result<ClaimJobsBasedOnResources, ApiError> {
        let strict_scheduler_match = strict_scheduler_match.unwrap_or(false);
        if limit <= 0 {
            return Ok(ClaimJobsBasedOnResources::SuccessfulResponse(
                models::ClaimJobsBasedOnResources {
                    jobs: Some(Vec::new()),
                    reason: None,
                },
            ));
        }
        let claim_limit = usize::try_from(limit)
            .map_err(|_| ApiError(format!("Limit {} does not fit on this platform", limit)))?;

        let time_limit_seconds = if let Some(ref time_limit) = resources.time_limit {
            match duration_string_to_seconds(time_limit) {
                Ok(seconds) => seconds,
                Err(e) => {
                    let error_response = models::ErrorResponse::new(serde_json::json!({
                        "message": format!("Invalid time_limit format '{}': {}", time_limit, e),
                        "field": "time_limit",
                        "value": time_limit
                    }));
                    return Ok(
                        ClaimJobsBasedOnResources::UnprocessableContentErrorResponse(
                            error_response,
                        ),
                    );
                }
            }
        } else {
            i64::MAX
        };

        let memory_bytes = (resources.memory_gb * 1024.0 * 1024.0 * 1024.0) as i64;
        let ready_status = models::JobStatus::Ready.to_int();

        let mut conn = self.pool.acquire().await.map_err(|e| {
            error!("Failed to acquire database connection: {}", e);
            ApiError("Database connection error".to_string())
        })?;

        debug!(
            "get_ready_jobs: workflow_id={}, limit={}, resources={:?} - X-Span-ID: {:?}",
            workflow_id,
            limit,
            resources,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        // Workflow existence check runs without a transaction. WAL mode allows
        // concurrent reads, so this never contends with productive writes.
        let workflow_exists = sqlx::query("SELECT id FROM workflow WHERE id = $1")
            .bind(workflow_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| {
                error!("Database error checking workflow existence: {}", e);
                ApiError("Database error".to_string())
            })?;

        if workflow_exists.is_none() {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", workflow_id)
            }));
            return Ok(ClaimJobsBasedOnResources::NotFoundErrorResponse(
                error_response,
            ));
        }

        // Lock-free pre-check: skip the BEGIN IMMEDIATE write lock when no
        // ready job in this workflow could possibly fit the runner's resources.
        // We deliberately omit the scheduler filter here so a positive result
        // covers both the strict and lenient code paths below; false positives
        // simply fall through to the normal locked path.
        let pre_check = sqlx::query(
            r#"
            SELECT 1
            FROM job
            JOIN resource_requirements rr ON job.resource_requirements_id = rr.id
            WHERE job.workflow_id = $1
            AND job.status = $2
            AND rr.memory_bytes <= $3
            AND rr.num_cpus <= $4
            AND rr.num_gpus <= $5
            AND rr.num_nodes <= $6
            AND rr.runtime_s <= $7
            LIMIT 1
            "#,
        )
        .bind(workflow_id)
        .bind(ready_status)
        .bind(memory_bytes)
        .bind(resources.num_cpus)
        .bind(resources.num_gpus)
        .bind(resources.num_nodes)
        .bind(time_limit_seconds)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| {
            error!("Database error in claim pre-check: {}", e);
            ApiError("Database error".to_string())
        })?;

        if pre_check.is_none() {
            return Ok(ClaimJobsBasedOnResources::SuccessfulResponse(
                models::ClaimJobsBasedOnResources {
                    jobs: Some(Vec::new()),
                    reason: None,
                },
            ));
        }

        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(|e| {
                error!("Failed to begin immediate transaction: {}", e);
                ApiError("Database lock error".to_string())
            })?;
        let query_with_scheduler = format!(
            r#"
            SELECT
                job.workflow_id,
                job.id AS job_id,
                job.name,
                job.command,
                job.invocation_script,
                job.env,
                job.status,
                job.cancel_on_blocking_job_failure,
                job.supports_termination,
                job.failure_handler_id,
                job.attempt_id,
                job.priority,
                rr.id AS resource_requirements_id,
                rr.memory_bytes,
                rr.num_cpus,
                rr.num_gpus,
                rr.num_nodes,
                rr.runtime_s
            FROM job
            JOIN resource_requirements rr ON job.resource_requirements_id = rr.id
            WHERE job.workflow_id = $1
            AND job.status = $2
            AND rr.memory_bytes <= $3
            AND rr.num_cpus <= $4
            AND rr.num_gpus <= $5
            AND rr.num_nodes <= $6
            AND rr.runtime_s <= $7
            AND (job.scheduler_id IS NULL OR job.scheduler_id = $8)
            {}
            LIMIT $9
            "#,
            RESOURCE_CLAIM_ORDER_BY
        );

        let mut used_scheduler_filter = true;
        let mut rows = match sqlx::query(&query_with_scheduler)
            .bind(workflow_id)
            .bind(ready_status)
            .bind(memory_bytes)
            .bind(resources.num_cpus)
            .bind(resources.num_gpus)
            .bind(resources.num_nodes)
            .bind(time_limit_seconds)
            .bind(resources.scheduler_config_id)
            .bind(limit)
            .fetch_all(&mut *conn)
            .await
        {
            Ok(rows) => rows,
            Err(e) => {
                error!("Database error in get_ready_jobs: {}", e);
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                return Err(ApiError("Database error".to_string()));
            }
        };

        if rows.is_empty() && !strict_scheduler_match {
            let query_without_scheduler = format!(
                r#"
                SELECT
                    job.workflow_id,
                    job.id AS job_id,
                    job.name,
                    job.command,
                    job.invocation_script,
                    job.env,
                    job.status,
                    job.cancel_on_blocking_job_failure,
                    job.supports_termination,
                    job.failure_handler_id,
                    job.attempt_id,
                    job.priority,
                    rr.id AS resource_requirements_id,
                    rr.memory_bytes,
                    rr.num_cpus,
                    rr.num_gpus,
                    rr.num_nodes,
                    rr.runtime_s
                FROM job
                JOIN resource_requirements rr ON job.resource_requirements_id = rr.id
                WHERE job.workflow_id = $1
                AND job.status = $2
                AND rr.memory_bytes <= $3
                AND rr.num_cpus <= $4
                AND rr.num_gpus <= $5
                AND rr.num_nodes <= $6
                AND rr.runtime_s <= $7
                {}
                LIMIT $8
                "#,
                RESOURCE_CLAIM_ORDER_BY
            );

            rows = match sqlx::query(&query_without_scheduler)
                .bind(workflow_id)
                .bind(ready_status)
                .bind(memory_bytes)
                .bind(resources.num_cpus)
                .bind(resources.num_gpus)
                .bind(resources.num_nodes)
                .bind(time_limit_seconds)
                .bind(limit)
                .fetch_all(&mut *conn)
                .await
            {
                Ok(rows) => rows,
                Err(e) => {
                    error!(
                        "Database error in get_ready_jobs (no scheduler filter): {}",
                        e
                    );
                    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                    return Err(ApiError("Database error".to_string()));
                }
            };

            if !rows.is_empty() {
                info!(
                    "Worker with scheduler_config_id={:?} found {} ready jobs after removing scheduler filter \
                     (strict_scheduler_match=false).",
                    resources.scheduler_config_id,
                    rows.len()
                );
            }
            used_scheduler_filter = false;
        }

        let mut packing_state = ClaimPackingState::new(&resources, memory_bytes);
        let mut selected_jobs = Vec::new();
        let mut job_ids_to_update = Vec::new();

        debug!(
            "get_ready_jobs: Found {} potential jobs for workflow {} with resources: \
             per_node(cpus={}, memory_bytes={}, gpus={}), nodes={}, time_limit={:?}",
            rows.len(),
            workflow_id,
            packing_state.per_node_cpus,
            packing_state.per_node_memory,
            packing_state.per_node_gpus,
            packing_state.total_nodes,
            resources.time_limit
        );

        for row in rows {
            if selected_jobs.len() >= claim_limit {
                break;
            }
            if let Err(e) = claim_candidate_row(
                &row,
                &mut packing_state,
                &mut selected_jobs,
                &mut job_ids_to_update,
            ) {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                return Err(e);
            }
        }

        let backfill_params = BackfillClaimParams {
            workflow_id,
            ready_status,
            time_limit_seconds,
            scheduler_config_id: resources.scheduler_config_id,
            use_scheduler_filter: used_scheduler_filter,
            claim_limit,
        };
        if let Err(e) = claim_backfill_jobs(
            &mut conn,
            &backfill_params,
            &mut packing_state,
            &mut selected_jobs,
            &mut job_ids_to_update,
        )
        .await
        {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            return Err(e);
        }

        let mut output_files_map: std::collections::HashMap<i64, Vec<i64>> =
            std::collections::HashMap::new();
        let mut output_user_data_map: std::collections::HashMap<i64, Vec<i64>> =
            std::collections::HashMap::new();

        if !job_ids_to_update.is_empty() {
            let output_files = match sqlx::query(
                "SELECT job_id, file_id FROM job_output_file WHERE workflow_id = $1",
            )
            .bind(workflow_id)
            .fetch_all(&mut *conn)
            .await
            {
                Ok(rows) => rows,
                Err(e) => {
                    error!("Failed to query output files: {}", e);
                    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                    return Err(ApiError("Database query error".to_string()));
                }
            };

            for row in output_files {
                let job_id: i64 = row.get("job_id");
                let file_id: i64 = row.get("file_id");
                if job_ids_to_update.contains(&job_id) {
                    output_files_map.entry(job_id).or_default().push(file_id);
                }
            }

            let output_user_data = match sqlx::query("SELECT job_id, user_data_id FROM job_output_user_data WHERE job_id IN (SELECT id FROM job WHERE workflow_id = $1)")
                .bind(workflow_id)
                .fetch_all(&mut *conn)
                .await
            {
                Ok(rows) => rows,
                Err(e) => {
                    error!("Failed to query output user_data: {}", e);
                    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                    return Err(ApiError("Database query error".to_string()));
                }
            };

            for row in output_user_data {
                let job_id: i64 = row.get("job_id");
                let user_data_id: i64 = row.get("user_data_id");
                if job_ids_to_update.contains(&job_id) {
                    output_user_data_map
                        .entry(job_id)
                        .or_default()
                        .push(user_data_id);
                }
            }
        }

        for job in &mut selected_jobs {
            if let Some(job_id) = job.id {
                job.output_file_ids = output_files_map.get(&job_id).cloned();
                job.output_user_data_ids = output_user_data_map.get(&job_id).cloned();
            }
        }

        if !job_ids_to_update.is_empty() {
            let pending = models::JobStatus::Pending.to_int();
            let job_ids_str = job_ids_to_update
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "UPDATE job SET status = {} WHERE id IN ({})",
                pending, job_ids_str
            );
            if let Err(e) = sqlx::query(&sql).execute(&mut *conn).await {
                error!("Failed to update job status: {}", e);
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                return Err(ApiError("Database update error".to_string()));
            }

            debug!(
                "Updated {} jobs to pending status for workflow {}",
                job_ids_to_update.len(),
                workflow_id
            );
        }

        if let Err(e) = sqlx::query("COMMIT").execute(&mut *conn).await {
            error!("Failed to commit transaction: {}", e);
            if let Err(rollback_err) = sqlx::query("ROLLBACK").execute(&mut *conn).await {
                error!("Failed to rollback after commit failure: {}", rollback_err);
            }
            return Err(ApiError("Database commit error".to_string()));
        }

        let response = models::ClaimJobsBasedOnResources {
            jobs: Some(selected_jobs),
            reason: None,
        };

        Ok(ClaimJobsBasedOnResources::SuccessfulResponse(response))
    }
}
