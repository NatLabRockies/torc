use super::*;
use crate::server::api::{
    EventsApi, JobsApi, ResultsApi, WorkflowsApi, begin_immediate_transaction,
    rollback_immediate_transaction,
};
use std::collections::HashMap;

#[allow(clippy::too_many_arguments)]
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync,
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
        context: &C,
    ) -> Result<InitializeJobsResponse, ApiError> {
        info!(
            "initialize_jobs({}, {:?}, {:?}) - X-Span-ID: {:?}",
            id,
            only_uninitialized,
            clear_ephemeral_user_data,
            Has::<XSpanIdString>::get(context).0.clone()
        );
        authorize_workflow!(self, id, context, InitializeJobsResponse);

        if let Ok(mut set) = self.workflows_with_failures.write() {
            set.remove(&id);
        }

        let mut tx = match self.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                error!("Failed to begin transaction for initialize_jobs: {}", e);
                return Err(ApiError("Database error".to_string()));
            }
        };

        if let Err(e) = self
            .add_depends_on_associations_from_files(&mut *tx, id)
            .await
        {
            error!("Failed to add depends-on associations from files: {}", e);
            let _ = tx.rollback().await;
            return Err(e);
        }

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

        let only_uninit = only_uninitialized.unwrap_or(false);
        if only_uninit && let Err(e) = self.uninitialize_blocked_jobs(&mut *tx, id).await {
            error!("Failed to uninitialize blocked jobs: {}", e);
            let _ = tx.rollback().await;
            return Err(e);
        }

        if let Err(e) = self
            .initialize_blocked_jobs_to_blocked(&mut *tx, id, only_uninit)
            .await
        {
            error!("Failed to initialize blocked jobs to blocked: {}", e);
            let _ = tx.rollback().await;
            return Err(e);
        }

        if let Err(e) = self.initialize_unblocked_jobs(&mut *tx, id).await {
            error!("Failed to initialize unblocked jobs: {}", e);
            let _ = tx.rollback().await;
            return Err(e);
        }

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
                error!(
                    "Failed to delete workflow_result records for incomplete jobs: {}",
                    e
                );
                let _ = tx.rollback().await;
                return Err(ApiError("Database error".to_string()));
            }
        }

        if let Err(e) = tx.commit().await {
            error!("Failed to commit transaction for initialize_jobs: {}", e);
            return Err(ApiError("Database error".to_string()));
        }

        self.jobs_api.compute_and_store_all_input_hashes(id).await?;

        match sqlx::query!("SELECT enable_ro_crate FROM workflow WHERE id = $1", id)
            .fetch_optional(self.pool.as_ref())
            .await
        {
            Ok(Some(row)) if row.enable_ro_crate == Some(1) => {
                debug!(
                    "enable_ro_crate is true for workflow {}, creating input file entities",
                    id
                );
                if let Err(e) = self.ro_crate_api.create_entities_for_input_files(id).await {
                    warn!("Failed to create RO-Crate entities for input files: {}", e);
                }
            }
            Ok(_) => {}
            Err(e) => warn!("Failed to check enable_ro_crate flag: {}", e),
        }

        if let Err(e) = self.ro_crate_api.create_server_software_entity(id).await {
            warn!("Failed to create torc-server software entity: {}", e);
        }

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
                    error!("Failed to activate {} actions for workflow {}: {}", trigger_type, id, e);
                }
            }
        }

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

        let event_type = if only_uninitialized.unwrap_or(false) {
            "workflow_started"
        } else {
            "workflow_reinitialized"
        };

        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        let username = auth
            .map(|a| a.subject)
            .unwrap_or_else(|| "unknown".to_string());

        self.event_broadcaster.broadcast(BroadcastEvent {
            workflow_id: id,
            timestamp: chrono::Utc::now().timestamp_millis(),
            event_type: event_type.to_string(),
            severity: models::EventSeverity::Info,
            data: serde_json::json!({
                "category": "workflow",
                "type": event_type,
                "user": username,
                "message": format!("{} workflow {}", event_type.replace('_', " "), id),
            }),
        });

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

        self.jobs_api
            .claim_next_jobs(id, limit.unwrap_or(10), context)
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

        if !status.is_terminal() {
            error!(
                "Attempted to complete job {} with non-terminal status '{}'",
                id, status
            );
            return Err(ApiError(format!(
                "Status '{}' is not a terminal status for job completion",
                status
            )));
        }

        let mut job = match self.jobs_api.get_job(id, context).await? {
            GetJobResponse::SuccessfulResponse(job) => job,
            GetJobResponse::ForbiddenErrorResponse(err) => {
                error!("Access denied for job {}: {:?}", id, err);
                return Ok(CompleteJobResponse::ForbiddenErrorResponse(err));
            }
            GetJobResponse::NotFoundErrorResponse(err) => {
                error!("Job not found {}: {:?}", id, err);
                return Ok(CompleteJobResponse::NotFoundErrorResponse(err));
            }
            GetJobResponse::DefaultErrorResponse(err) => {
                error!("Failed to get job {}: {:?}", id, err);
                return Ok(CompleteJobResponse::DefaultErrorResponse(err));
            }
        };

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
            return Ok(CompleteJobResponse::UnprocessableContentErrorResponse(
                error_response,
            ));
        }

        if result.job_id != id {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "ResultModel job_id {} does not match target job_id {}",
                    result.job_id, id
                )
            }));
            return Ok(CompleteJobResponse::UnprocessableContentErrorResponse(
                error_response,
            ));
        }
        if result.workflow_id != job.workflow_id {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "ResultModel workflow_id {} does not match job's workflow_id {}",
                    result.workflow_id, job.workflow_id
                )
            }));
            return Ok(CompleteJobResponse::UnprocessableContentErrorResponse(
                error_response,
            ));
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
        let result_response = self.results_api.create_result(result, context).await?;

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
                return Ok(CompleteJobResponse::ForbiddenErrorResponse(err));
            }
            CreateResultResponse::NotFoundErrorResponse(err) => {
                error!("Failed to add result for job {}: {:?}", id, err);
                return Ok(CompleteJobResponse::NotFoundErrorResponse(err));
            }
            CreateResultResponse::DefaultErrorResponse(err) => {
                error!("Failed to add result for job {}: {:?}", id, err);
                return Ok(CompleteJobResponse::DefaultErrorResponse(err));
            }
        };

        let workflow_id = job.workflow_id;
        let result_id_value = result_id.ok_or_else(|| {
            error!("Result ID is missing after creating result");
            ApiError("Result ID is missing".to_string())
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
                return Err(ApiError("Database error".to_string()));
            }
        }

        self.manage_job_status_change(&job, run_id).await?;

        let event_type = format!("job_{}", status.to_string().to_lowercase());
        let severity = match status {
            models::JobStatus::Completed => models::EventSeverity::Info,
            models::JobStatus::Failed => models::EventSeverity::Error,
            models::JobStatus::Terminated | models::JobStatus::Canceled => {
                models::EventSeverity::Warning
            }
            _ => models::EventSeverity::Info,
        };
        self.event_broadcaster.broadcast(BroadcastEvent {
            workflow_id: job.workflow_id,
            timestamp: chrono::Utc::now().timestamp_millis(),
            event_type,
            severity,
            data: serde_json::json!({
                "job_id": id,
                "job_name": job.name,
                "status": status.to_string(),
                "return_code": result_return_code,
            }),
        });
        debug!("Broadcast job completion event for job_id={}", id);

        debug!(
            "complete_job: successfully completed job_id={} with status={}, result_id={:?}",
            id, status, result_id
        );

        if let Err(e) = self
            .workflow_actions_api
            .check_and_trigger_actions(workflow_id, "on_jobs_complete", Some(vec![id]))
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

        Ok(CompleteJobResponse::SuccessfulResponse(job))
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
        let mut conn = self.pool.acquire().await.map_err(|e| {
            error!("Failed to acquire database connection: {}", e);
            ApiError("Database connection error".to_string())
        })?;

        begin_immediate_transaction(&mut conn).await.map_err(|e| {
            error!("Failed to begin immediate transaction: {}", e);
            ApiError("Database lock error".to_string())
        })?;

        let result: Result<ClaimJobsBasedOnResources, ApiError> = async {
            debug!(
                "get_ready_jobs: workflow_id={}, limit={}, resources={:?} - X-Span-ID: {:?}",
                workflow_id,
                limit,
                resources,
                Has::<XSpanIdString>::get(context).0.clone()
            );

            let workflow_record =
                sqlx::query("SELECT id, execution_config FROM workflow WHERE id = $1")
                    .bind(workflow_id)
                    .fetch_optional(&mut *conn)
                    .await
                    .map_err(|e| {
                        error!("Database error checking workflow existence: {}", e);
                        ApiError("Database error".to_string())
                    })?;

            let Some(workflow_record) = workflow_record else {
                rollback_immediate_transaction(&mut conn, "prepare_ready_jobs workflow missing")
                    .await;

                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Workflow not found with ID: {}", workflow_id)
                }));
                return Ok(ClaimJobsBasedOnResources::NotFoundErrorResponse(
                    error_response,
                ));
            };

            let downstream_buffer_multiplier = workflow_record
                .get::<Option<String>, _>("execution_config")
                .as_deref()
                .and_then(|json| {
                    serde_json::from_str::<crate::client::workflow_spec::ExecutionConfig>(json).ok()
                })
                .and_then(|config| config.downstream_buffer_multiplier());

            let time_limit_seconds = if let Some(ref time_limit) = resources.time_limit {
                match duration_string_to_seconds(time_limit) {
                    Ok(seconds) => seconds,
                    Err(e) => {
                        rollback_immediate_transaction(
                            &mut conn,
                            "prepare_ready_jobs invalid time_limit",
                        )
                        .await;

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

            const CLAIM_JOBS_PAGE_SIZE: i64 = 256;

            let ready_status = models::JobStatus::Ready.to_int();
            let order_by_clause = "ORDER BY job.priority DESC, job.id ASC";
            let selection_limit =
                usize::try_from(limit).map_err(|_| ApiError("Invalid limit".to_string()))?;

            let query_with_scheduler = format!(
            r#"
            SELECT
                job.workflow_id,
                job.id AS job_id,
                job.name,
                job.command,
                job.invocation_script,
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
            OFFSET $10
            "#,
            order_by_clause
        );
            let query_without_scheduler = format!(
            r#"
            SELECT
                job.workflow_id,
                job.id AS job_id,
                job.name,
                job.command,
                job.invocation_script,
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
            OFFSET $9
            "#,
            order_by_clause
        );

            let per_node_cpus = resources.num_cpus;
            let per_node_memory = memory_bytes;
            let per_node_gpus = resources.num_gpus;
            let total_nodes = resources.num_nodes.max(1);

            let mut consumed_memory_bytes = 0i64;
            let mut consumed_cpus = 0i64;
            let mut consumed_gpus = 0i64;
            let mut exclusive_nodes = 0i64;
            let mut selected_jobs = Vec::new();
            let mut job_ids_to_update = Vec::new();
            let mut job_family_cache: HashMap<i64, Vec<DownstreamFamilyKey>> = HashMap::new();
            let mut family_state_cache: HashMap<DownstreamFamilyKey, Option<DownstreamBufferState>> =
                HashMap::new();

        #[derive(Clone, Debug, Eq, Hash, PartialEq)]
        struct DownstreamFamilyKey {
            resource_requirements_id: i64,
            scheduler_id: Option<i64>,
        }

        #[derive(Clone, Debug)]
        struct DownstreamBufferState {
            occupancy: usize,
            buffer_limit: usize,
        }

        fn calculate_downstream_capacity(
            total_cpus: i64,
            total_memory_bytes: i64,
            total_gpus: i64,
            total_nodes: i64,
            rr_cpus: i64,
            rr_memory_bytes: i64,
            rr_gpus: i64,
            rr_nodes: i64,
        ) -> usize {
            let mut capacities = Vec::new();
            if rr_cpus > 0 {
                capacities.push(total_cpus / rr_cpus);
            }
            if rr_memory_bytes > 0 {
                capacities.push(total_memory_bytes / rr_memory_bytes);
            }
            if rr_gpus > 0 {
                capacities.push(total_gpus / rr_gpus);
            }
            if rr_nodes > 1 {
                capacities.push(total_nodes / rr_nodes);
            }
            capacities.into_iter().min().unwrap_or(0).max(0) as usize
        }

        async fn load_job_families(
            conn: &mut sqlx::SqliteConnection,
            workflow_id: i64,
            job_id: i64,
        ) -> Result<Vec<DownstreamFamilyKey>, ApiError> {
            let rows = sqlx::query(
                r#"
                SELECT DISTINCT
                    downstream.resource_requirements_id AS resource_requirements_id,
                    downstream.scheduler_id AS scheduler_id
                FROM job_depends_on dep
                JOIN job downstream ON downstream.id = dep.job_id
                WHERE dep.workflow_id = $1
                AND dep.depends_on_job_id = $2
                AND downstream.resource_requirements_id IS NOT NULL
                "#,
            )
            .bind(workflow_id)
            .bind(job_id)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| {
                error!(
                    "Database error loading downstream families workflow_id={} job_id={}: {}",
                    workflow_id, job_id, e
                );
                ApiError("Database error".to_string())
            })?;

            Ok(rows
                .into_iter()
                .map(|row| DownstreamFamilyKey {
                    resource_requirements_id: row.get("resource_requirements_id"),
                    scheduler_id: row.get("scheduler_id"),
                })
                .collect())
        }

        async fn load_family_state(
            conn: &mut sqlx::SqliteConnection,
            workflow_id: i64,
            family: &DownstreamFamilyKey,
            downstream_buffer_multiplier: usize,
        ) -> Result<Option<DownstreamBufferState>, ApiError> {
            let rr_row = sqlx::query(
                r#"
                SELECT memory_bytes, num_cpus, num_gpus, num_nodes
                FROM resource_requirements
                WHERE id = $1
                "#,
            )
            .bind(family.resource_requirements_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| {
                error!(
                    "Database error loading downstream resource requirements rr_id={}: {}",
                    family.resource_requirements_id, e
                );
                ApiError("Database error".to_string())
            })?;

            let Some(rr_row) = rr_row else {
                return Ok(None);
            };

            let capacity_row = if let Some(scheduler_id) = family.scheduler_id {
                sqlx::query(
                    r#"
                    SELECT
                        COUNT(*) AS active_count,
                        COALESCE(SUM(num_cpus), 0) AS total_cpus,
                        COALESCE(SUM(memory_gb), 0.0) AS total_memory_gb,
                        COALESCE(SUM(num_gpus), 0) AS total_gpus,
                        COALESCE(SUM(num_nodes), 0) AS total_nodes
                    FROM compute_node
                    WHERE workflow_id = $1
                    AND is_active = 1
                    AND scheduler_config_id = $2
                    "#,
                )
                .bind(workflow_id)
                .bind(scheduler_id)
                .fetch_one(&mut *conn)
                .await
            } else {
                sqlx::query(
                    r#"
                    SELECT
                        COUNT(*) AS active_count,
                        COALESCE(SUM(num_cpus), 0) AS total_cpus,
                        COALESCE(SUM(memory_gb), 0.0) AS total_memory_gb,
                        COALESCE(SUM(num_gpus), 0) AS total_gpus,
                        COALESCE(SUM(num_nodes), 0) AS total_nodes
                    FROM compute_node
                    WHERE workflow_id = $1
                    AND is_active = 1
                    "#,
                )
                .bind(workflow_id)
                .fetch_one(&mut *conn)
                .await
            }
            .map_err(|e| {
                error!(
                    "Database error loading downstream capacity workflow_id={} family={:?}: {}",
                    workflow_id, family, e
                );
                ApiError("Database error".to_string())
            })?;

            let active_count: i64 = capacity_row.get("active_count");
            if active_count == 0 {
                return Ok(None);
            }

            let downstream_capacity = calculate_downstream_capacity(
                capacity_row.get("total_cpus"),
                (capacity_row.get::<f64, _>("total_memory_gb") * 1024.0 * 1024.0 * 1024.0) as i64,
                capacity_row.get("total_gpus"),
                capacity_row.get("total_nodes"),
                rr_row.get("num_cpus"),
                rr_row.get("memory_bytes"),
                rr_row.get("num_gpus"),
                rr_row.get("num_nodes"),
            );

            let upstream_status_rows = if let Some(scheduler_id) = family.scheduler_id {
                sqlx::query(
                    r#"
                    SELECT COUNT(DISTINCT upstream.id) AS count
                    FROM job upstream
                    JOIN job_depends_on dep ON dep.depends_on_job_id = upstream.id
                    JOIN job downstream ON downstream.id = dep.job_id
                    WHERE upstream.workflow_id = $1
                    AND dep.workflow_id = $1
                    AND downstream.workflow_id = $1
                    AND downstream.resource_requirements_id = $2
                    AND downstream.scheduler_id = $3
                    AND upstream.status IN ($4, $5)
                    "#,
                )
                .bind(workflow_id)
                .bind(family.resource_requirements_id)
                .bind(scheduler_id)
                .bind(models::JobStatus::Pending.to_int())
                .bind(models::JobStatus::Running.to_int())
                .fetch_one(&mut *conn)
                .await
            } else {
                sqlx::query(
                    r#"
                    SELECT COUNT(DISTINCT upstream.id) AS count
                    FROM job upstream
                    JOIN job_depends_on dep ON dep.depends_on_job_id = upstream.id
                    JOIN job downstream ON downstream.id = dep.job_id
                    WHERE upstream.workflow_id = $1
                    AND dep.workflow_id = $1
                    AND downstream.workflow_id = $1
                    AND downstream.resource_requirements_id = $2
                    AND downstream.scheduler_id IS NULL
                    AND upstream.status IN ($3, $4)
                    "#,
                )
                .bind(workflow_id)
                .bind(family.resource_requirements_id)
                .bind(models::JobStatus::Pending.to_int())
                .bind(models::JobStatus::Running.to_int())
                .fetch_one(&mut *conn)
                .await
            }
            .map_err(|e| {
                error!(
                    "Database error loading upstream occupancy workflow_id={} family={:?}: {}",
                    workflow_id, family, e
                );
                ApiError("Database error".to_string())
            })?;

            let downstream_status_rows = if let Some(scheduler_id) = family.scheduler_id {
                sqlx::query(
                    r#"
                    SELECT COUNT(*) AS count
                    FROM job downstream
                    WHERE downstream.workflow_id = $1
                    AND downstream.resource_requirements_id = $2
                    AND downstream.scheduler_id = $3
                    AND downstream.status IN ($4, $5, $6)
                    "#,
                )
                .bind(workflow_id)
                .bind(family.resource_requirements_id)
                .bind(scheduler_id)
                .bind(models::JobStatus::Ready.to_int())
                .bind(models::JobStatus::Pending.to_int())
                .bind(models::JobStatus::Running.to_int())
                .fetch_one(&mut *conn)
                .await
            } else {
                sqlx::query(
                    r#"
                    SELECT COUNT(*) AS count
                    FROM job downstream
                    WHERE downstream.workflow_id = $1
                    AND downstream.resource_requirements_id = $2
                    AND downstream.scheduler_id IS NULL
                    AND downstream.status IN ($3, $4, $5)
                    "#,
                )
                .bind(workflow_id)
                .bind(family.resource_requirements_id)
                .bind(models::JobStatus::Ready.to_int())
                .bind(models::JobStatus::Pending.to_int())
                .bind(models::JobStatus::Running.to_int())
                .fetch_one(&mut *conn)
                .await
            }
            .map_err(|e| {
                error!(
                    "Database error loading downstream occupancy workflow_id={} family={:?}: {}",
                    workflow_id, family, e
                );
                ApiError("Database error".to_string())
            })?;

            let occupancy = upstream_status_rows.get::<i64, _>("count")
                + downstream_status_rows.get::<i64, _>("count");
            Ok(Some(DownstreamBufferState {
                occupancy: occupancy.max(0) as usize,
                buffer_limit: downstream_capacity.saturating_mul(downstream_buffer_multiplier),
            }))
        }

        async fn process_rows(
            conn: &mut sqlx::SqliteConnection,
            rows: Vec<sqlx::sqlite::SqliteRow>,
            per_node_cpus: i64,
            per_node_memory: i64,
            per_node_gpus: i64,
            total_nodes: i64,
            selection_limit: usize,
            workflow_id: i64,
            downstream_buffer_multiplier: Option<usize>,
            consumed_memory_bytes: &mut i64,
            consumed_cpus: &mut i64,
            consumed_gpus: &mut i64,
            exclusive_nodes: &mut i64,
            selected_jobs: &mut Vec<models::JobModel>,
            job_ids_to_update: &mut Vec<i64>,
            job_family_cache: &mut HashMap<i64, Vec<DownstreamFamilyKey>>,
            family_state_cache: &mut HashMap<DownstreamFamilyKey, Option<DownstreamBufferState>>,
        ) -> Result<(usize, usize), ApiError> {
            let mut rows_scanned = 0usize;
            let mut rows_skipped = 0usize;

            for row in rows {
                rows_scanned += 1;

                let job_memory: i64 = row.get("memory_bytes");
                let job_cpus: i64 = row.get("num_cpus");
                let job_gpus: i64 = row.get("num_gpus");
                let job_nodes: i64 = row.get("num_nodes");
                let reserved_nodes = job_nodes.max(1);

                let fits = if reserved_nodes > 1 {
                    let shared_nodes_after = total_nodes - *exclusive_nodes - reserved_nodes;
                    *exclusive_nodes + reserved_nodes <= total_nodes
                        && *consumed_cpus <= shared_nodes_after * per_node_cpus
                        && *consumed_memory_bytes <= shared_nodes_after * per_node_memory
                        && *consumed_gpus <= shared_nodes_after * per_node_gpus
                } else {
                    let shared_capacity_cpus = (total_nodes - *exclusive_nodes) * per_node_cpus;
                    let shared_capacity_memory = (total_nodes - *exclusive_nodes) * per_node_memory;
                    let shared_capacity_gpus = (total_nodes - *exclusive_nodes) * per_node_gpus;
                    *consumed_cpus + job_cpus <= shared_capacity_cpus
                        && *consumed_memory_bytes + job_memory <= shared_capacity_memory
                        && *consumed_gpus + job_gpus <= shared_capacity_gpus
                };

                let job_id: i64 = row.get("job_id");
                if fits {
                    let downstream_families = if downstream_buffer_multiplier.is_some() {
                        if let Some(families) = job_family_cache.get(&job_id) {
                            families.clone()
                        } else {
                            let families = load_job_families(conn, workflow_id, job_id).await?;
                            job_family_cache.insert(job_id, families.clone());
                            families
                        }
                    } else {
                        Vec::new()
                    };

                    let mut blocked_by_downstream_buffer = None;
                    if let Some(multiplier) = downstream_buffer_multiplier {
                        for family in &downstream_families {
                            if !family_state_cache.contains_key(family) {
                                let state =
                                    load_family_state(conn, workflow_id, family, multiplier)
                                        .await?;
                                family_state_cache.insert(family.clone(), state);
                            }

                            if let Some(Some(state)) = family_state_cache.get(family)
                                && state.occupancy >= state.buffer_limit
                            {
                                blocked_by_downstream_buffer =
                                    Some((family.clone(), state.occupancy, state.buffer_limit));
                                break;
                            }
                        }
                    }

                    if let Some((family, occupancy, buffer_limit)) = blocked_by_downstream_buffer {
                        rows_skipped += 1;
                        debug!(
                            "Skipping job {} - downstream buffer full for rr_id={} scheduler_id={:?} ({}/{})",
                            job_id,
                            family.resource_requirements_id,
                            family.scheduler_id,
                            occupancy,
                            buffer_limit
                        );
                        continue;
                    }

                    if reserved_nodes > 1 {
                        *exclusive_nodes += reserved_nodes;
                    } else {
                        *consumed_memory_bytes += job_memory;
                        *consumed_cpus += job_cpus;
                        *consumed_gpus += job_gpus;
                    }

                    for family in &downstream_families {
                        if let Some(Some(state)) = family_state_cache.get_mut(family) {
                            state.occupancy += 1;
                        }
                    }

                    job_ids_to_update.push(job_id);

                    let status = models::JobStatus::from_int(row.get::<i64, _>("status") as i32)
                        .map_err(|e| {
                            error!("Failed to parse job status: {}", e);
                            ApiError("Invalid job status".to_string())
                        })?;

                    if status != models::JobStatus::Ready {
                        return Err(ApiError("Invalid job status in ready queue".to_string()));
                    }
                    let job = models::JobModel {
                        id: Some(job_id),
                        workflow_id: row.get("workflow_id"),
                        name: row.get("name"),
                        command: row.get("command"),
                        invocation_script: row.get("invocation_script"),
                        status: Some(models::JobStatus::Pending),
                        schedule_compute_nodes: None,
                        cancel_on_blocking_job_failure: Some(
                            row.get("cancel_on_blocking_job_failure"),
                        ),
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
                    };

                    selected_jobs.push(job);
                    if selected_jobs.len() >= selection_limit {
                        break;
                    }
                } else {
                    rows_skipped += 1;

                    let reason = if reserved_nodes > 1 {
                        let available = total_nodes - *exclusive_nodes;
                        format!(
                            "multi-node job needs {} free nodes, {} available \
                             (exclusive_nodes={}, shared cpus={}/{})",
                            reserved_nodes,
                            available,
                            *exclusive_nodes,
                            *consumed_cpus,
                            (total_nodes - *exclusive_nodes) * per_node_cpus
                        )
                    } else {
                        let shared_nodes = total_nodes - *exclusive_nodes;
                        format!(
                            "cpus: {}/{}, memory: {}/{}, gpus: {}/{}",
                            *consumed_cpus + job_cpus,
                            shared_nodes * per_node_cpus,
                            *consumed_memory_bytes + job_memory,
                            shared_nodes * per_node_memory,
                            *consumed_gpus + job_gpus,
                            shared_nodes * per_node_gpus
                        )
                    };

                    debug!(
                        "Skipping job {} - would exceed resource limits ({})",
                        job_id, reason
                    );
                }
            }

            Ok((rows_scanned, rows_skipped))
        }

            let mut scheduler_rows_scanned = 0usize;
            let mut scheduler_rows_skipped = 0usize;
            let mut scheduler_page = 0i64;
            while selected_jobs.len() < selection_limit {
            let offset = scheduler_page * CLAIM_JOBS_PAGE_SIZE;
            let rows = match sqlx::query(&query_with_scheduler)
                .bind(workflow_id)
                .bind(ready_status)
                .bind(memory_bytes)
                .bind(resources.num_cpus)
                .bind(resources.num_gpus)
                .bind(resources.num_nodes)
                .bind(time_limit_seconds)
                .bind(resources.scheduler_config_id)
                .bind(CLAIM_JOBS_PAGE_SIZE)
                .bind(offset)
                .fetch_all(&mut *conn)
                .await
            {
                Ok(rows) => rows,
                Err(e) => {
                    error!("Database error in get_ready_jobs: {}", e);
                    rollback_immediate_transaction(&mut conn, "prepare_ready_jobs scheduler query")
                        .await;
                    return Err(ApiError("Database error".to_string()));
                }
            };

            if rows.is_empty() {
                break;
            }

            let page_len = rows.len();
            let (rows_scanned, rows_skipped) = match process_rows(
                &mut conn,
                rows,
                per_node_cpus,
                per_node_memory,
                per_node_gpus,
                total_nodes,
                selection_limit,
                workflow_id,
                downstream_buffer_multiplier,
                &mut consumed_memory_bytes,
                &mut consumed_cpus,
                &mut consumed_gpus,
                &mut exclusive_nodes,
                &mut selected_jobs,
                &mut job_ids_to_update,
                &mut job_family_cache,
                &mut family_state_cache,
            )
            .await
                {
                    Ok(stats) => stats,
                    Err(err) => {
                        error!("Expected job status to be Ready during resource claim");
                        rollback_immediate_transaction(
                            &mut conn,
                            "prepare_ready_jobs scheduler row processing",
                        )
                        .await;
                        return Err(err);
                    }
                };
            scheduler_rows_scanned += rows_scanned;
            scheduler_rows_skipped += rows_skipped;

            if page_len < CLAIM_JOBS_PAGE_SIZE as usize {
                break;
            }
            scheduler_page += 1;
        }

            debug!(
            "get_ready_jobs: workflow_id={} scheduler_filter=true page_size={} rows_scanned={} selected={} skipped_for_fit={} \
             per_node(cpus={}, memory_bytes={}, gpus={}) nodes={} time_limit={:?}",
            workflow_id,
            CLAIM_JOBS_PAGE_SIZE,
            scheduler_rows_scanned,
            selected_jobs.len(),
            scheduler_rows_skipped,
            per_node_cpus,
            per_node_memory,
            per_node_gpus,
            total_nodes,
            resources.time_limit
        );

            if scheduler_rows_scanned == 0 && !strict_scheduler_match {
            let mut fallback_rows_scanned = 0usize;
            let mut fallback_rows_skipped = 0usize;
            let mut fallback_page = 0i64;

            while selected_jobs.len() < selection_limit {
                let offset = fallback_page * CLAIM_JOBS_PAGE_SIZE;
                let rows = match sqlx::query(&query_without_scheduler)
                    .bind(workflow_id)
                    .bind(ready_status)
                    .bind(memory_bytes)
                    .bind(resources.num_cpus)
                    .bind(resources.num_gpus)
                    .bind(resources.num_nodes)
                    .bind(time_limit_seconds)
                    .bind(CLAIM_JOBS_PAGE_SIZE)
                    .bind(offset)
                    .fetch_all(&mut *conn)
                    .await
                {
                    Ok(rows) => rows,
                    Err(e) => {
                        error!(
                            "Database error in get_ready_jobs (no scheduler filter): {}",
                            e
                        );
                        rollback_immediate_transaction(
                            &mut conn,
                            "prepare_ready_jobs fallback query",
                        )
                        .await;
                        return Err(ApiError("Database error".to_string()));
                    }
                };

                if rows.is_empty() {
                    break;
                }

                let page_len = rows.len();
                let (rows_scanned, rows_skipped) = match process_rows(
                    &mut conn,
                    rows,
                    per_node_cpus,
                    per_node_memory,
                    per_node_gpus,
                    total_nodes,
                    selection_limit,
                    workflow_id,
                    downstream_buffer_multiplier,
                    &mut consumed_memory_bytes,
                    &mut consumed_cpus,
                    &mut consumed_gpus,
                    &mut exclusive_nodes,
                    &mut selected_jobs,
                    &mut job_ids_to_update,
                    &mut job_family_cache,
                    &mut family_state_cache,
                )
                .await
                {
                    Ok(stats) => stats,
                    Err(err) => {
                        error!("Expected job status to be Ready during resource claim");
                        rollback_immediate_transaction(
                            &mut conn,
                            "prepare_ready_jobs fallback row processing",
                        )
                        .await;
                        return Err(err);
                    }
                };
                fallback_rows_scanned += rows_scanned;
                fallback_rows_skipped += rows_skipped;

                if page_len < CLAIM_JOBS_PAGE_SIZE as usize {
                    break;
                }
                fallback_page += 1;
            }

            if fallback_rows_scanned > 0 {
                info!(
                    "Worker with scheduler_config_id={:?} found {} ready jobs after removing scheduler filter \
                     (strict_scheduler_match=false).",
                    resources.scheduler_config_id, fallback_rows_scanned
                );
            }

            debug!(
                "get_ready_jobs: workflow_id={} scheduler_filter=false page_size={} rows_scanned={} selected={} skipped_for_fit={} \
                 per_node(cpus={}, memory_bytes={}, gpus={}) nodes={} time_limit={:?}",
                workflow_id,
                CLAIM_JOBS_PAGE_SIZE,
                fallback_rows_scanned,
                selected_jobs.len(),
                fallback_rows_skipped,
                per_node_cpus,
                per_node_memory,
                per_node_gpus,
                total_nodes,
                resources.time_limit
            );
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
                    rollback_immediate_transaction(&mut conn, "prepare_ready_jobs output files")
                        .await;
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
                    rollback_immediate_transaction(&mut conn, "prepare_ready_jobs output user_data")
                        .await;
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
                rollback_immediate_transaction(&mut conn, "prepare_ready_jobs update status")
                    .await;
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
                rollback_immediate_transaction(&mut conn, "prepare_ready_jobs commit").await;
                return Err(ApiError("Database commit error".to_string()));
            }

            let response = models::ClaimJobsBasedOnResources {
                jobs: Some(selected_jobs),
                reason: None,
            };

            Ok(ClaimJobsBasedOnResources::SuccessfulResponse(response))
        }
        .await;

        if result.is_err() {
            rollback_immediate_transaction(&mut conn, "prepare_ready_jobs error exit").await;
        }

        result
    }
}
