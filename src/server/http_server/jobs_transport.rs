use super::*;
use crate::server::api::{
    JobsApi, ResultsApi, WorkflowsApi, begin_immediate, database_lock_aware_error,
    message_error_response, parse_job_status, resource_not_found_response,
};

const RESOURCE_CLAIM_ORDER_BY: &str = "\
    ORDER BY \
        job.priority DESC, \
        rr.num_gpus DESC, \
        rr.runtime_s DESC, \
        rr.memory_bytes DESC, \
        rr.num_cpus DESC, \
        job.id ASC";

/// Detect a cycle in a directed graph given as adjacency lists (node -> deps).
/// Used to reject self-referential `spawn_jobs` batches.
fn has_cycle(adjacency: &std::collections::HashMap<&str, Vec<&str>>) -> bool {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Visiting,
        Done,
    }
    fn dfs<'a>(
        node: &'a str,
        adjacency: &std::collections::HashMap<&'a str, Vec<&'a str>>,
        marks: &mut std::collections::HashMap<&'a str, Mark>,
    ) -> bool {
        match marks.get(node) {
            Some(Mark::Visiting) => return true,
            Some(Mark::Done) => return false,
            None => {}
        }
        marks.insert(node, Mark::Visiting);
        for next in adjacency.get(node).map(|v| v.as_slice()).unwrap_or(&[]) {
            if dfs(next, adjacency, marks) {
                return true;
            }
        }
        marks.insert(node, Mark::Done);
        false
    }
    let mut marks: std::collections::HashMap<&str, Mark> = std::collections::HashMap::new();
    adjacency
        .keys()
        .any(|node| dfs(node, adjacency, &mut marks))
}

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

/// Translate a `GetJobResponse` into `Ok(JobModel)` on success, or `Err(<target>)` carrying the
/// matching error variant of `$target_enum`. Logs each non-success variant with the job id so
/// individual call sites do not have to repeat identical match arms. Only valid where the target
/// enum has `ForbiddenErrorResponse`/`NotFoundErrorResponse`/`DefaultErrorResponse` variants that
/// take the same `ErrorResponse` payload as `GetJobResponse`.
macro_rules! translate_get_job_response {
    ($source:expr, $id:expr, $target:ident) => {
        match $source {
            GetJobResponse::SuccessfulResponse(job) => Ok(job),
            GetJobResponse::ForbiddenErrorResponse(err) => {
                error!("Access denied job_id={} error={:?}", $id, err);
                Err($target::ForbiddenErrorResponse(err))
            }
            GetJobResponse::NotFoundErrorResponse(err) => {
                error!("Job not found job_id={} error={:?}", $id, err);
                Err($target::NotFoundErrorResponse(err))
            }
            GetJobResponse::DefaultErrorResponse(err) => {
                error!("Failed to get job job_id={} error={:?}", $id, err);
                Err($target::DefaultErrorResponse(err))
            }
        }
    };
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

/// Verify that a `ResultModel` matches the target identifiers a caller is completing against.
/// Returns the validation error as a 422 `ErrorResponse` so the two completion paths
/// (`apply_job_completion_state` and `apply_job_completion_state_tx`) can wrap it identically.
fn validate_result_matches_target(
    id: i64,
    job_workflow_id: i64,
    status: models::JobStatus,
    run_id: i64,
    result: &models::ResultModel,
) -> Result<(), models::ErrorResponse> {
    if result.job_id != id {
        return Err(message_error_response(format!(
            "ResultModel job_id={} does not match target job_id={}",
            result.job_id, id
        )));
    }
    if result.workflow_id != job_workflow_id {
        return Err(message_error_response(format!(
            "ResultModel workflow_id={} does not match job's workflow_id={}",
            result.workflow_id, job_workflow_id
        )));
    }
    if result.status != status {
        return Err(message_error_response(format!(
            "ResultModel status='{}' does not match target status='{}'",
            result.status, status
        )));
    }
    if result.run_id != run_id {
        return Err(message_error_response(format!(
            "ResultModel run_id={} does not match target run_id={}",
            result.run_id, run_id
        )));
    }
    Ok(())
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

    let status = parse_job_status(
        row.get::<i64, _>("status") as i32,
        row.get::<i64, _>("job_id"),
    )?;

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
        // Empty batches have no workflow_id to authorize against and create no rows,
        // so the only sensible response is 422 — the alternative is letting an
        // unauthenticated caller hit the route and get a 200. The api impl still
        // defends in depth.
        if body.jobs.is_empty() {
            return Ok(CreateJobsResponse::UnprocessableContentErrorResponse(
                message_error_response(
                    "Bulk job creation requires a non-empty `jobs` array".to_string(),
                ),
            ));
        }

        let first_workflow_id = body.jobs[0].workflow_id;
        for job in &body.jobs {
            if job.workflow_id != first_workflow_id {
                return Ok(CreateJobsResponse::UnprocessableContentErrorResponse(
                    message_error_response(format!(
                        "All jobs in a batch must have the same workflow_id. Found workflow_ids: {} and {}",
                        first_workflow_id, job.workflow_id
                    )),
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
        log_call!(
            info,
            context,
            "initialize_jobs({}, {:?}, {:?}, async={:?})",
            id,
            only_uninitialized,
            clear_ephemeral_user_data,
            async_,
        );
        authorize_workflow!(self, id, context, InitializeJobsResponse);

        let username = username_from_context(context);

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
        let (offset, limit) = authorize_workflow_and_paginate!(
            self,
            workflow_id,
            context,
            ListJobsResponse,
            offset,
            limit
        );
        self.jobs_api
            .list_jobs(
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
        log_call!(debug, context, "claim_next_jobs({}, {:?})", id, limit);

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
        log_call!(
            info,
            context,
            "reset_job_status(workflow_id={}, failed_only={:?})",
            id,
            failed_only,
        );

        authorize_workflow!(self, id, context, ResetJobStatusResponse);

        let failed_only_value = failed_only.unwrap_or(false);
        let result = self
            .jobs_api
            .reset_job_status(id, failed_only_value, context)
            .await?;

        if let ResetJobStatusResponse::SuccessfulResponse(ref response) = result {
            self.record_user_action_event(
                id,
                "reset_job_status",
                serde_json::json!({
                    "workflow_id": id,
                    "failed_only": failed_only_value,
                    "updated_count": response.updated_count,
                }),
                context,
            )
            .await;
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
        log_call!(
            debug,
            context,
            "manage_status_change({}, {:?}, {})",
            id,
            status,
            run_id,
        );

        if status.is_complete() {
            error!(
                "manage_status_change: cannot set completion status '{}' for job_id={}. Use complete_job instead.",
                status, id
            );
            return Ok(
                ManageStatusChangeResponse::UnprocessableContentErrorResponse(
                    message_error_response(format!(
                        "Cannot set completion status '{}' via manage_status_change. Use complete_job API instead.",
                        status
                    )),
                ),
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
            return Ok(
                ManageStatusChangeResponse::UnprocessableContentErrorResponse(
                    message_error_response(e),
                ),
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
                return Ok(ManageStatusChangeResponse::NotFoundErrorResponse(
                    resource_not_found_response("Job", id),
                ));
            }

            return Ok(
                ManageStatusChangeResponse::UnprocessableContentErrorResponse(
                    message_error_response(format!(
                        "job_id={} status was concurrently modified (expected status='{}'), please retry",
                        id, current_status
                    )),
                ),
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
            return Ok(ManageStatusChangeResponse::DefaultErrorResponse(
                message_error_response("Failed to reinitialize downstream jobs"),
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
        log_call!(
            debug,
            context,
            "start_job({}, {}, {})",
            id,
            run_id,
            compute_node_id,
        );

        authorize_job!(self, id, context, StartJobResponse);

        let mut job = match translate_get_job_response!(
            self.jobs_api.get_job(id, context).await?,
            id,
            StartJobResponse
        ) {
            Ok(job) => job,
            Err(err_response) => return Ok(err_response),
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
                    "job_id={} has invalid status={:?}. Expected SubmittedPending for job start.",
                    id, status
                )));
            }
            None => {
                error!("start_job: Job status not set for job_id={}", id);
                return Err(ApiError(format!(
                    "job_id={} has no status set. Expected SubmittedPending for job start.",
                    id
                )));
            }
        }

        if let Err(e) = self.validate_run_id(job.workflow_id, run_id).await {
            error!("start_job: job_id={}, {}", id, e);
            return Ok(StartJobResponse::UnprocessableContentErrorResponse(
                message_error_response(e),
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
                "job_id={} status was concurrently modified, cannot start",
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
        log_call!(
            debug,
            context,
            "complete_job({}, {:?}, {}, {:?})",
            id,
            status,
            run_id,
            result,
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
            Ok(response) => match translate_get_job_response!(response, id, CompleteJobResponse) {
                Ok(job) => job,
                Err(err_response) => {
                    return Err(CompletionMutationError::Response(Box::new(err_response)));
                }
            },
            Err(error) => return Err(CompletionMutationError::Transport(error)),
        };

        if let Some(expected_workflow_id) = expected_workflow_id
            && job.workflow_id != expected_workflow_id
        {
            return Err(CompletionMutationError::Response(Box::new(
                CompleteJobResponse::UnprocessableContentErrorResponse(message_error_response(
                    format!(
                        "job_id={} belongs to workflow_id={} but batch target is workflow_id={}",
                        id, job.workflow_id, expected_workflow_id
                    ),
                )),
            )));
        }

        if let Some(current_status) = &job.status
            && current_status.is_complete()
        {
            error!(
                "job_id={} is already complete with status={:?}",
                id, current_status
            );
            return Err(CompletionMutationError::Response(Box::new(
                CompleteJobResponse::UnprocessableContentErrorResponse(message_error_response(
                    format!(
                        "job_id={} is already complete with status={:?}",
                        id, current_status
                    ),
                )),
            )));
        }

        if let Err(error_response) =
            validate_result_matches_target(id, job.workflow_id, status, run_id, &result)
        {
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
    ) -> Result<CompletedJobRecord, CompletionMutationError> {
        if !status.is_terminal() {
            return Err(CompletionMutationError::Transport(ApiError(format!(
                "Status '{}' is not a terminal status for job completion",
                status
            ))));
        }

        // Read the job through the same transaction as the writes below. Going through
        // jobs_api.get_job (which uses a fresh pool connection) deadlocks under shared-cache
        // SQLite once an earlier iteration in this batch has written via tx: the new
        // connection's SELECT blocks on the table-level lock that tx holds, while the only
        // tokio worker is awaiting that SELECT before tx can release it.
        let job_row = match sqlx::query!(
            "SELECT workflow_id, name, command, status FROM job WHERE id = ?",
            id
        )
        .fetch_optional(&mut **tx)
        .await
        {
            Ok(Some(row)) => row,
            Ok(None) => {
                return Err(CompletionMutationError::Response(Box::new(
                    CompleteJobResponse::NotFoundErrorResponse(resource_not_found_response(
                        "Job", id,
                    )),
                )));
            }
            Err(e) => {
                return Err(CompletionMutationError::Transport(
                    database_lock_aware_error(e, "Failed to fetch job for completion"),
                ));
            }
        };

        let job_workflow_id = job_row.workflow_id;
        let job_name = job_row.name;
        let job_command = job_row.command;
        let status_i32 = i32::try_from(job_row.status).map_err(|e| {
            CompletionMutationError::Transport(ApiError(format!(
                "job_id={} has out-of-range status value={} in database: {}",
                id, job_row.status, e
            )))
        })?;
        let current_status =
            parse_job_status(status_i32, id).map_err(CompletionMutationError::Transport)?;

        if let Some(expected_workflow_id) = expected_workflow_id
            && job_workflow_id != expected_workflow_id
        {
            return Err(CompletionMutationError::Response(Box::new(
                CompleteJobResponse::UnprocessableContentErrorResponse(message_error_response(
                    format!(
                        "job_id={} belongs to workflow_id={} but batch target is workflow_id={}",
                        id, job_workflow_id, expected_workflow_id
                    ),
                )),
            )));
        }

        if current_status.is_complete() {
            return Err(CompletionMutationError::Response(Box::new(
                CompleteJobResponse::UnprocessableContentErrorResponse(message_error_response(
                    format!(
                        "job_id={} is already complete with status={:?}",
                        id, current_status
                    ),
                )),
            )));
        }

        if let Err(error_response) =
            validate_result_matches_target(id, job_workflow_id, status, run_id, &result)
        {
            return Err(CompletionMutationError::Response(Box::new(
                CompleteJobResponse::UnprocessableContentErrorResponse(error_response),
            )));
        }

        // Inline run_id validation against tx for the same reason: validate_run_id uses a
        // fresh pool connection and would deadlock against the in-flight transaction.
        let workflow_run_id_row =
            sqlx::query!("SELECT run_id FROM workflow WHERE id = ?", job_workflow_id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| {
                    CompletionMutationError::Transport(database_lock_aware_error(
                        e,
                        "Failed to fetch workflow run_id",
                    ))
                })?;
        match workflow_run_id_row {
            Some(row) if row.run_id == run_id => {}
            Some(row) => {
                return Err(CompletionMutationError::Response(Box::new(
                    CompleteJobResponse::UnprocessableContentErrorResponse(message_error_response(
                        format!(
                            "Run ID mismatch: provided {} but workflow status has {}",
                            run_id, row.run_id
                        ),
                    )),
                )));
            }
            None => {
                return Err(CompletionMutationError::Response(Box::new(
                    CompleteJobResponse::UnprocessableContentErrorResponse(message_error_response(
                        format!(
                            "Workflow status not found for workflow ID: {}",
                            job_workflow_id
                        ),
                    )),
                )));
            }
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
            CompletionMutationError::Transport(database_lock_aware_error(
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
            job_workflow_id,
            id,
            result_id_value
        )
        .execute(&mut **tx)
        .await
        .map_err(|e| {
            CompletionMutationError::Transport(database_lock_aware_error(
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
        .map_err(|e| CompletionMutationError::Transport(database_lock_aware_error(e, "Failed to update job status")))?;

        if update_result.rows_affected() == 0 {
            let current = sqlx::query_scalar!("SELECT status FROM job WHERE id = ?", id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| {
                    CompletionMutationError::Transport(database_lock_aware_error(
                        e,
                        "Failed to re-check job status",
                    ))
                })?;

            match current {
                Some(status_int) => {
                    let current_status = models::JobStatus::from_int(status_int as i32)
                        .unwrap_or(models::JobStatus::Failed);
                    if current_status.is_complete() {
                        return Err(CompletionMutationError::Response(Box::new(
                            CompleteJobResponse::UnprocessableContentErrorResponse(
                                message_error_response(format!(
                                    "job_id={} is already complete with status={:?}",
                                    id, current_status
                                )),
                            ),
                        )));
                    }
                    return Err(CompletionMutationError::Transport(ApiError(format!(
                        "job_id={} is in unexpected status={:?}",
                        id, current_status
                    ))));
                }
                None => {
                    return Err(CompletionMutationError::Transport(ApiError(format!(
                        "job_id={} not found",
                        id
                    ))));
                }
            }
        }

        // Construct a JobModel for the completion record from the row we already fetched
        // through `tx`. Relationships and optional metadata are not needed by downstream
        // consumers on the batch path (finalize_completed_jobs only reads `name`); leaving
        // them unset avoids extra cross-table reads while keeping the populated scalar
        // fields (id, workflow_id, name, command, status) accurate.
        let completed_job = models::JobModel {
            id: Some(id),
            workflow_id: job_workflow_id,
            name: job_name,
            command: job_command,
            invocation_script: None,
            env: None,
            status: Some(status),
            schedule_compute_nodes: None,
            cancel_on_blocking_job_failure: None,
            supports_termination: None,
            depends_on_job_ids: None,
            input_file_ids: None,
            output_file_ids: None,
            input_user_data_ids: None,
            output_user_data_ids: None,
            resource_requirements_id: None,
            scheduler_id: None,
            failure_handler_id: None,
            attempt_id: None,
            priority: None,
        };

        Ok(CompletedJobRecord {
            job: completed_job,
            job_id: id,
            workflow_id: job_workflow_id,
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
        log_call!(
            debug,
            context,
            "batch_complete_jobs(workflow_id={}, count={})",
            workflow_id,
            body.completions.len(),
        );

        authorize_workflow!(self, workflow_id, context, BatchCompleteJobsResponse);

        let mut completed = Vec::new();
        let mut errors = Vec::new();
        let mut completion_records = Vec::new();
        // Use BEGIN IMMEDIATE: apply_job_completion_state_tx reads from `job` before
        // writing, and a deferred transaction's read snapshot can be invalidated by a
        // concurrent committer, surfacing SQLITE_BUSY_SNAPSHOT (517) which busy_timeout
        // does not retry. See server/api.rs::begin_immediate.
        let mut tx = begin_immediate(&self.pool).await.map_err(|e| {
            database_lock_aware_error(e, "Failed to begin batch completion transaction")
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
            database_lock_aware_error(e, "Failed to commit batch completion transaction")
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

    /// Add a batch of new jobs to an initialized workflow, all blocked on the
    /// calling job. The calling job is **not** completed here — the runner
    /// completes it when its subprocess exits, and the normal unblock cascade
    /// promotes the spawned jobs. Per-lineage state and counter are persisted
    /// in the same transaction. See `docs/plans/dynamic-jobs-design.md`.
    pub(super) async fn transport_spawn_jobs(
        &self,
        id: i64,
        body: models::SpawnJobsRequest,
        context: &C,
    ) -> Result<SpawnJobsResponse, ApiError> {
        log_call!(
            debug,
            context,
            "spawn_jobs(job_id={}, jobs_count={})",
            id,
            body.jobs.len(),
        );

        authorize_job!(self, id, context, SpawnJobsResponse);

        /// Per-lineage cap applied when the workflow leaves it unset.
        const DEFAULT_MAX_SPAWN_ITERATIONS: i64 = 1000;

        let models::SpawnJobsRequest {
            lineage,
            jobs,
            state,
        } = body;

        let mut tx = begin_immediate(&self.pool)
            .await
            .map_err(|e| database_lock_aware_error(e, "Failed to begin spawn_jobs transaction"))?;

        // --- Resolve the calling job and its workflow --------------------
        let job_row = match sqlx::query("SELECT workflow_id, name, status FROM job WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
        {
            Ok(Some(row)) => row,
            Ok(None) => {
                let _ = tx.rollback().await;
                return Ok(SpawnJobsResponse::NotFoundErrorResponse(
                    resource_not_found_response("Job", id),
                ));
            }
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(database_lock_aware_error(e, "Failed to fetch calling job"));
            }
        };
        let workflow_id: i64 = job_row.get("workflow_id");
        let caller_name: String = job_row.get("name");
        let caller_status_int: i64 = job_row.get("status");
        let caller_status = parse_job_status(i32::try_from(caller_status_int).unwrap_or(0), id)
            .map_err(|e| ApiError(format!("Failed to parse caller status: {}", e.0)))?;
        // The orchestrator must be Running. If the caller is already terminal
        // its unblock has already been processed, so spawned children would
        // sit Blocked forever (the cascade fires on completions, not inserts).
        if caller_status != models::JobStatus::Running {
            let _ = tx.rollback().await;
            return Ok(SpawnJobsResponse::UnprocessableContentErrorResponse(
                message_error_response(format!(
                    "spawn_jobs requires the calling job to be Running (job_id={} is {:?}); \
                     spawned children blocked on an already-processed caller would never unblock",
                    id, caller_status
                )),
            ));
        }
        let lineage = lineage.unwrap_or(caller_name);

        // --- Reject duplicate names in the batch up front ---------------
        // If two entries share a name the second INSERT overwrites the first
        // in `name_to_id`, corrupting the dependency-edge wiring below.
        {
            let mut seen: std::collections::HashSet<&str> =
                std::collections::HashSet::with_capacity(jobs.len());
            for job in &jobs {
                if !seen.insert(job.name.as_str()) {
                    let _ = tx.rollback().await;
                    return Ok(SpawnJobsResponse::UnprocessableContentErrorResponse(
                        message_error_response(format!(
                            "duplicate name '{}' in spawn batch",
                            job.name
                        )),
                    ));
                }
            }
        }

        // --- Validate priorities (mirror create_job's >= 0 rule) --------
        for job in &jobs {
            if let Some(p) = job.priority
                && p < 0
            {
                let _ = tx.rollback().await;
                return Ok(SpawnJobsResponse::UnprocessableContentErrorResponse(
                    message_error_response(format!(
                        "spawn job '{}' has invalid priority {}: must be >= 0",
                        job.name, p
                    )),
                ));
            }
        }

        let wf_row =
            match sqlx::query("SELECT max_spawn_iterations_per_lineage FROM workflow WHERE id = ?")
                .bind(workflow_id)
                .fetch_optional(&mut *tx)
                .await
            {
                Ok(Some(row)) => row,
                Ok(None) => {
                    let _ = tx.rollback().await;
                    return Ok(SpawnJobsResponse::NotFoundErrorResponse(
                        resource_not_found_response("Workflow", workflow_id),
                    ));
                }
                Err(e) => {
                    let _ = tx.rollback().await;
                    return Err(database_lock_aware_error(e, "Failed to fetch workflow"));
                }
            };
        let max_iterations: i64 = wf_row
            .get::<Option<i64>, _>("max_spawn_iterations_per_lineage")
            .unwrap_or(DEFAULT_MAX_SPAWN_ITERATIONS);

        // --- Existing jobs in the workflow (name -> id) ------------------
        // Only fetch the names that the batch actually references: the spawn
        // job names themselves (for the idempotency / overlap check) plus
        // each spawn job's depends_on entries. Loading the entire workflow's
        // job table would inflate the BEGIN IMMEDIATE transaction on large
        // workflows.
        let mut needed_names: std::collections::HashSet<&str> =
            jobs.iter().map(|j| j.name.as_str()).collect();
        for job in &jobs {
            for dep in job.depends_on.as_deref().unwrap_or(&[]) {
                needed_names.insert(dep.as_str());
            }
        }
        let mut name_to_id: std::collections::HashMap<String, i64> =
            std::collections::HashMap::with_capacity(needed_names.len());
        if !needed_names.is_empty() {
            // Build an IN-clause-friendly query. sqlx::QueryBuilder is the
            // idiomatic way; here we hand-roll because the binds are simple.
            let placeholders = needed_names
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "SELECT id, name FROM job WHERE workflow_id = ? AND name IN ({})",
                placeholders
            );
            let mut q = sqlx::query(&sql).bind(workflow_id);
            for name in &needed_names {
                q = q.bind(*name);
            }
            let rows = q.fetch_all(&mut *tx).await.map_err(|e| {
                database_lock_aware_error(e, "Failed to look up referenced workflow jobs")
            })?;
            for r in rows {
                name_to_id.insert(r.get::<String, _>("name"), r.get::<i64, _>("id"));
            }
        }

        // --- Idempotency: detect a replayed spawn -----------------------
        let already_present = jobs
            .iter()
            .filter(|j| name_to_id.contains_key(&j.name))
            .count();
        let is_replay = !jobs.is_empty() && already_present == jobs.len();
        if !jobs.is_empty() && already_present != 0 && !is_replay {
            let _ = tx.rollback().await;
            return Ok(SpawnJobsResponse::UnprocessableContentErrorResponse(
                message_error_response(
                    "Inconsistent replay: some but not all jobs already exist".to_string(),
                ),
            ));
        }

        // --- Derive this lineage's spawn counter from its append-only
        //     per-generation state records -------------------------------
        // Each spawn generation N is an immutable user_data record named
        // `__torc_lineage__<lineage>__g<NNNNNN>`. spawn_count == the highest
        // generation present. The converged final state (no-spawn call that
        // carries state) is a single `__torc_lineage__<lineage>__final` record.
        //
        // Scope the query to this lineage's prefix so we don't drag every
        // user_data row through the BEGIN IMMEDIATE transaction. The prefix
        // contains `_`, which is a LIKE wildcard, so we escape with `\`.
        let gen_prefix = format!("__torc_lineage__{}__g", lineage);
        let escaped_like = gen_prefix.replace('\\', "\\\\").replace('_', "\\_") + "%";
        let lineage_names = sqlx::query(
            "SELECT name FROM user_data WHERE workflow_id = ? AND name LIKE ? ESCAPE '\\'",
        )
        .bind(workflow_id)
        .bind(&escaped_like)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| database_lock_aware_error(e, "Failed to read lineage state"))?;
        let mut spawn_count: i64 = lineage_names
            .iter()
            .filter_map(|r| {
                let name: String = r.get("name");
                name.strip_prefix(&gen_prefix)
                    .and_then(|s| s.parse::<i64>().ok())
            })
            .max()
            .unwrap_or(0);

        // --- Validate the batch ------------------------------------------
        let will_spawn = !jobs.is_empty() && !is_replay;
        if will_spawn && spawn_count + 1 > max_iterations {
            let _ = tx.rollback().await;
            return Ok(SpawnJobsResponse::UnprocessableContentErrorResponse(
                message_error_response(format!(
                    "Per-lineage iteration cap reached for lineage '{}': {} (max_spawn_iterations_per_lineage={})",
                    lineage, spawn_count, max_iterations
                )),
            ));
        }

        // Resolve resource_requirements names -> ids.
        let mut rr_ids: Vec<Option<i64>> = Vec::with_capacity(jobs.len());
        if will_spawn {
            for job in &jobs {
                match &job.resource_requirements {
                    None => rr_ids.push(None),
                    Some(rr_name) => {
                        let rr_id: Option<i64> = sqlx::query_scalar(
                            "SELECT id FROM resource_requirements WHERE workflow_id = ? AND name = ?",
                        )
                        .bind(workflow_id)
                        .bind(rr_name)
                        .fetch_optional(&mut *tx)
                        .await
                        .map_err(|e| {
                            database_lock_aware_error(e, "Failed to resolve resource_requirements")
                        })?;
                        match rr_id {
                            Some(rid) => rr_ids.push(Some(rid)),
                            None => {
                                let _ = tx.rollback().await;
                                return Ok(SpawnJobsResponse::UnprocessableContentErrorResponse(
                                    message_error_response(format!(
                                        "Unknown resource_requirements '{}' for spawn job '{}'",
                                        rr_name, job.name
                                    )),
                                ));
                            }
                        }
                    }
                }
            }

            // Validate the intra-batch dependency DAG (cycle detection over
            // sibling names; edges to pre-existing jobs cannot close a cycle).
            let batch_names: std::collections::HashSet<&str> =
                jobs.iter().map(|j| j.name.as_str()).collect();
            let adjacency: std::collections::HashMap<&str, Vec<&str>> = jobs
                .iter()
                .map(|j| {
                    let edges = j
                        .depends_on
                        .as_deref()
                        .unwrap_or(&[])
                        .iter()
                        .filter(|d| batch_names.contains(d.as_str()))
                        .map(|d| d.as_str())
                        .collect();
                    (j.name.as_str(), edges)
                })
                .collect();
            if has_cycle(&adjacency) {
                let _ = tx.rollback().await;
                return Ok(SpawnJobsResponse::UnprocessableContentErrorResponse(
                    message_error_response(
                        "spawn jobs dependency graph contains a cycle".to_string(),
                    ),
                ));
            }

            // Every dependency name must resolve to an existing job or a sibling.
            for job in &jobs {
                for dep in job.depends_on.as_deref().unwrap_or(&[]) {
                    if !name_to_id.contains_key(dep) && !batch_names.contains(dep.as_str()) {
                        let _ = tx.rollback().await;
                        return Ok(SpawnJobsResponse::UnprocessableContentErrorResponse(
                            message_error_response(format!(
                                "spawn job '{}' depends on unknown job '{}'",
                                job.name, dep
                            )),
                        ));
                    }
                }
            }
        }

        // --- Insert the new jobs, then their dependency edges -----------
        // Every spawned job is created `blocked`, with an auto-injected edge
        // to the calling job. When the runner completes the caller on script
        // exit, the normal unblock cascade promotes the spawned jobs.
        let mut spawned_job_ids: Vec<i64> = Vec::with_capacity(jobs.len());
        if is_replay {
            spawned_job_ids = jobs
                .iter()
                .filter_map(|j| name_to_id.get(&j.name).copied())
                .collect();
        } else if will_spawn {
            let blocked_int = i64::from(models::JobStatus::Blocked.to_int());
            // Mirror the workflow-env merge that normal job creation does
            // (JobsApiImpl::fetch_workflow_env + merge_env): start from the
            // workflow-level env, then layer the lineage var on top so it
            // wins in any unlikely collision.
            let workflow_env_json: Option<String> =
                sqlx::query_scalar("SELECT env FROM workflow WHERE id = ?")
                    .bind(workflow_id)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|e| database_lock_aware_error(e, "Failed to fetch workflow env"))?
                    .flatten();
            let mut effective_env: std::collections::HashMap<String, String> = workflow_env_json
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();
            effective_env.insert("TORC_ORCHESTRATOR_LINEAGE_ID".to_string(), lineage.clone());
            let lineage_env = serde_json::to_string(&effective_env)
                .map_err(|e| ApiError(format!("Failed to serialize spawned job env: {}", e)))?;
            for (job, rr_id) in jobs.iter().zip(rr_ids.iter()) {
                let new_id: i64 = sqlx::query(
                    r#"
                    INSERT INTO job
                    (workflow_id, name, command, cancel_on_blocking_job_failure,
                     supports_termination, resource_requirements_id, status, priority, env)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                    RETURNING id
                    "#,
                )
                .bind(workflow_id)
                .bind(&job.name)
                .bind(&job.command)
                .bind(job.cancel_on_blocking_job_failure.unwrap_or(true))
                .bind(false)
                .bind(*rr_id)
                .bind(blocked_int)
                .bind(job.priority.unwrap_or(0))
                .bind(&lineage_env)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| database_lock_aware_error(e, "Failed to insert spawned job"))?
                .get("id");
                name_to_id.insert(job.name.clone(), new_id);
                spawned_job_ids.push(new_id);
            }

            for job in &jobs {
                let job_id = name_to_id[&job.name];
                // Collect explicit deps + the implicit edge on the caller.
                let mut dep_ids: std::collections::BTreeSet<i64> = job
                    .depends_on
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .map(|n| name_to_id[n])
                    .collect();
                dep_ids.insert(id);
                for dep_id in dep_ids {
                    sqlx::query(
                        "INSERT INTO job_depends_on (job_id, depends_on_job_id, workflow_id) VALUES (?, ?, ?)",
                    )
                    .bind(job_id)
                    .bind(dep_id)
                    .bind(workflow_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| {
                        database_lock_aware_error(e, "Failed to insert spawned dependency")
                    })?;
                }
            }
        }

        // --- Persist lineage state (append-only) ------------------------
        // A spawning generation appends a NEW immutable record
        // `__torc_lineage__<lineage>__g<NNNNNN>`. A non-spawning call that
        // carries state (convergence) upserts the single `__final` record.
        // Replays append nothing because they're detected upstream.
        if will_spawn {
            spawn_count += 1;
            let gen_name = format!("__torc_lineage__{}__g{:06}", lineage, spawn_count);
            let payload = serde_json::json!({
                "generation": spawn_count,
                "spawn_count": spawn_count,
                "state": state.unwrap_or(serde_json::Value::Null),
            });
            let payload_str = serde_json::to_string(&payload)
                .map_err(|e| ApiError(format!("Failed to serialize lineage state: {}", e)))?;
            sqlx::query(
                "INSERT INTO user_data (workflow_id, name, is_ephemeral, data) VALUES (?, ?, 1, ?)",
            )
            .bind(workflow_id)
            .bind(&gen_name)
            .bind(&payload_str)
            .execute(&mut *tx)
            .await
            .map_err(|e| database_lock_aware_error(e, "Failed to append lineage generation"))?;
        } else if !is_replay && state.is_some() {
            let final_name = format!("__torc_lineage__{}__final", lineage);
            let payload = serde_json::json!({
                "generation": spawn_count,
                "spawn_count": spawn_count,
                "final": true,
                "state": state.unwrap_or(serde_json::Value::Null),
            });
            let payload_str = serde_json::to_string(&payload)
                .map_err(|e| ApiError(format!("Failed to serialize lineage state: {}", e)))?;
            let existing: Option<i64> =
                sqlx::query_scalar("SELECT id FROM user_data WHERE workflow_id = ? AND name = ?")
                    .bind(workflow_id)
                    .bind(&final_name)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|e| {
                        database_lock_aware_error(e, "Failed to read final lineage state")
                    })?;
            match existing {
                Some(ud_id) => {
                    sqlx::query("UPDATE user_data SET data = ? WHERE id = ?")
                        .bind(&payload_str)
                        .bind(ud_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| {
                            database_lock_aware_error(e, "Failed to update final lineage state")
                        })?;
                }
                None => {
                    sqlx::query(
                        "INSERT INTO user_data (workflow_id, name, is_ephemeral, data) VALUES (?, ?, 1, ?)",
                    )
                    .bind(workflow_id)
                    .bind(&final_name)
                    .bind(&payload_str)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| {
                        database_lock_aware_error(e, "Failed to insert final lineage state")
                    })?;
                }
            }
        }

        tx.commit()
            .await
            .map_err(|e| database_lock_aware_error(e, "Failed to commit spawn_jobs transaction"))?;

        Ok(SpawnJobsResponse::SuccessfulResponse(
            models::SpawnJobsResponse {
                spawned_job_ids,
                iteration: spawn_count,
            },
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

        log_call!(
            debug,
            context,
            "get_ready_jobs: workflow_id={}, limit={}, resources={:?}",
            workflow_id,
            limit,
            resources,
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
            return Ok(ClaimJobsBasedOnResources::NotFoundErrorResponse(
                resource_not_found_response("Workflow", workflow_id),
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
