use super::jobs_transport::find_dependency_cycle;
use super::*;
use crate::server::api::{
    EventsApi, FailureHandlersApi, ResourceRequirementsApi, WorkflowActionsApi, WorkflowsApi,
    begin_immediate, database_lock_aware_error, message_error_response,
    resource_not_found_response,
};

#[allow(clippy::too_many_arguments)]
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    pub(super) async fn transport_create_event(
        &self,
        body: models::EventModel,
        context: &C,
    ) -> Result<CreateEventResponse, ApiError> {
        authorize_workflow!(self, body.workflow_id, context, CreateEventResponse);
        self.events_api.create_event(body, context).await
    }

    pub(super) async fn transport_create_failure_handler(
        &self,
        body: models::FailureHandlerModel,
        context: &C,
    ) -> Result<CreateFailureHandlerResponse, ApiError> {
        authorize_workflow!(
            self,
            body.workflow_id,
            context,
            CreateFailureHandlerResponse
        );
        self.failure_handlers_api
            .create_failure_handler(body, context)
            .await
    }

    pub(super) async fn transport_get_failure_handler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetFailureHandlerResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "failure_handler",
            context,
            GetFailureHandlerResponse
        );
        self.failure_handlers_api
            .get_failure_handler(id, context)
            .await
    }

    pub(super) async fn transport_list_failure_handlers(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListFailureHandlersResponse, ApiError> {
        let (offset, limit) = authorize_workflow_and_paginate!(
            self,
            workflow_id,
            context,
            ListFailureHandlersResponse,
            offset,
            limit
        );
        self.failure_handlers_api
            .list_failure_handlers(workflow_id, offset, limit, context)
            .await
    }

    pub(super) async fn transport_delete_failure_handler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteFailureHandlerResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "failure_handler",
            context,
            DeleteFailureHandlerResponse
        );
        self.failure_handlers_api
            .delete_failure_handler(id, context)
            .await
    }

    pub(super) async fn transport_create_workflow(
        &self,
        mut body: models::WorkflowModel,
        context: &C,
    ) -> Result<CreateWorkflowResponse, ApiError> {
        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        if let Some(username) = AuthorizationService::get_username(&auth) {
            if body.user != username {
                info!(
                    "Workflow user field '{}' overwritten with authenticated user '{}'",
                    body.user, username
                );
            }
            body.user = username.to_string();
        }

        let response = self.workflows_api.create_workflow(body, context).await?;
        match response {
            CreateWorkflowResponse::SuccessfulResponse(w) => {
                let rr = models::ResourceRequirementsModel {
                    id: None,
                    workflow_id: w.id.expect("Failed to get workflow ID"),
                    name: "default".to_string(),
                    num_cpus: 1,
                    num_gpus: 0,
                    num_nodes: 1,
                    memory: "1m".to_string(),
                    runtime: "P0DT1M".to_string(),
                };
                let _result = self
                    .resource_requirements_api
                    .create_resource_requirements(rr, context)
                    .await?;
                Ok(CreateWorkflowResponse::SuccessfulResponse(w))
            }
            CreateWorkflowResponse::ForbiddenErrorResponse(err) => {
                Ok(CreateWorkflowResponse::ForbiddenErrorResponse(err))
            }
            CreateWorkflowResponse::NotFoundErrorResponse(err) => {
                Ok(CreateWorkflowResponse::NotFoundErrorResponse(err))
            }
            CreateWorkflowResponse::UnprocessableContentErrorResponse(err) => Ok(
                CreateWorkflowResponse::UnprocessableContentErrorResponse(err),
            ),
            CreateWorkflowResponse::DefaultErrorResponse(err) => {
                Ok(CreateWorkflowResponse::DefaultErrorResponse(err))
            }
        }
    }

    /// Validate a draft workflow and clear `is_draft` if it passes. The checks and the flip run
    /// in one `BEGIN IMMEDIATE` transaction, so no write (another batch, the draft cleanup task)
    /// can slip between them.
    pub(super) async fn transport_finalize_workflow(
        &self,
        id: i64,
        force: Option<bool>,
        context: &C,
    ) -> Result<FinalizeWorkflowResponse, ApiError> {
        authorize_workflow!(self, id, context, FinalizeWorkflowResponse);
        let mut tx = begin_immediate(self.pool.as_ref())
            .await
            .map_err(|e| database_lock_aware_error(e, "Failed to begin transaction"))?;
        let row: Option<(i64, Option<String>)> =
            sqlx::query_as("SELECT is_draft, execution_config FROM workflow WHERE id = ?")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| database_lock_aware_error(e, "Failed to read workflow"))?;
        let Some((is_draft, execution_config)) = row else {
            return Ok(FinalizeWorkflowResponse::NotFoundErrorResponse(
                resource_not_found_response("Workflow", id),
            ));
        };
        if is_draft == 0 {
            return Ok(FinalizeWorkflowResponse::SuccessfulResponse(
                models::FinalizeWorkflowResponse {
                    finalized: true,
                    errors: vec![],
                    warnings: vec![],
                },
            ));
        }

        let execution_config = match execution_config.as_deref().map(serde_json::from_str) {
            Some(Ok(ec)) => Some(ec),
            Some(Err(e)) => {
                return Err(ApiError(format!(
                    "Failed to parse execution_config of workflow_id={}: {}",
                    id, e
                )));
            }
            None => None,
        };
        let (input, mut errors) = load_check_input(&mut tx, id, execution_config).await?;
        let findings = crate::workflow_validation::check_workflow(&input);
        errors.extend(findings.errors);
        if let Some(cycle) = find_dependency_cycle(&mut *tx, id).await? {
            errors.push(named_cycle_message(&mut tx, &cycle).await?);
        }
        let warnings = findings.warnings;
        let finalized = errors.is_empty() && (warnings.is_empty() || force.unwrap_or(false));
        if finalized {
            sqlx::query("UPDATE workflow SET is_draft = 0 WHERE id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| database_lock_aware_error(e, "Failed to finalize workflow"))?;
            tx.commit()
                .await
                .map_err(|e| database_lock_aware_error(e, "Failed to commit finalize"))?;
        }
        info!(
            "finalize_workflow workflow_id={} finalized={} errors={} warnings={}",
            id,
            finalized,
            errors.len(),
            warnings.len()
        );
        Ok(FinalizeWorkflowResponse::SuccessfulResponse(
            models::FinalizeWorkflowResponse {
                finalized,
                errors,
                warnings,
            },
        ))
    }

    pub(super) async fn transport_create_workflow_action(
        &self,
        workflow_id: i64,
        action_model: models::WorkflowActionModel,
        context: &C,
    ) -> Result<CreateWorkflowActionResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, CreateWorkflowActionResponse);
        self.workflow_actions_api
            .create_workflow_action(workflow_id, action_model, context)
            .await
    }

    pub(super) async fn transport_get_workflow_actions(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<GetWorkflowActionsResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, GetWorkflowActionsResponse);
        self.workflow_actions_api
            .get_workflow_actions(workflow_id, context)
            .await
    }

    pub(super) async fn transport_get_pending_actions(
        &self,
        workflow_id: i64,
        trigger_types: Option<Vec<String>>,
        context: &C,
    ) -> Result<GetPendingActionsResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, GetPendingActionsResponse);
        self.workflow_actions_api
            .get_pending_actions(workflow_id, trigger_types, context)
            .await
    }

    pub(super) async fn transport_claim_action(
        &self,
        workflow_id: i64,
        action_id: i64,
        body: models::ClaimActionRequest,
        context: &C,
    ) -> Result<ClaimActionResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, ClaimActionResponse);
        self.workflow_actions_api
            .claim_action(workflow_id, action_id, body.compute_node_id, context)
            .await
    }

    pub(super) async fn transport_suppress_action(
        &self,
        workflow_id: i64,
        action_id: i64,
        context: &C,
    ) -> Result<SuppressActionResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, SuppressActionResponse);
        self.workflow_actions_api
            .suppress_action(workflow_id, action_id, context)
            .await
    }

    pub(super) async fn transport_report_action_result(
        &self,
        workflow_id: i64,
        action_id: i64,
        body: models::WorkflowActionResultRequest,
        context: &C,
    ) -> Result<ReportActionResultResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, ReportActionResultResponse);
        self.workflow_actions_api
            .report_action_result(workflow_id, action_id, body, context)
            .await
    }

    pub(super) async fn transport_reset_action(
        &self,
        workflow_id: i64,
        action_id: i64,
        context: &C,
    ) -> Result<ResetActionResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, ResetActionResponse);
        self.workflow_actions_api
            .reset_action(workflow_id, action_id, context)
            .await
    }

    pub(super) async fn transport_update_workflow_action(
        &self,
        workflow_id: i64,
        action_id: i64,
        updates: serde_json::Value,
        context: &C,
    ) -> Result<UpdateWorkflowActionResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, UpdateWorkflowActionResponse);
        self.workflow_actions_api
            .update_workflow_action(workflow_id, action_id, updates, context)
            .await
    }

    pub(super) async fn transport_delete_workflow_action(
        &self,
        workflow_id: i64,
        action_id: i64,
        context: &C,
    ) -> Result<DeleteWorkflowActionResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteWorkflowActionResponse);
        self.workflow_actions_api
            .delete_workflow_action(workflow_id, action_id, context)
            .await
    }

    pub(super) async fn transport_list_workflows(
        &self,
        offset: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        limit: Option<i64>,
        name: Option<String>,
        user: Option<String>,
        description: Option<String>,
        is_archived: Option<bool>,
        access_group: Option<String>,
        context: &C,
    ) -> Result<ListWorkflowsResponse, ApiError> {
        let (offset, limit) = process_pagination_params(offset, limit)?;

        let accessible_ids = if self.authorization_service.is_enforced() && user.is_none() {
            let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
            match self
                .authorization_service
                .get_accessible_workflow_ids(&auth)
                .await
            {
                Ok(ids) => ids,
                Err(e) => {
                    return Err(ApiError(format!(
                        "Failed to get accessible workflows: {}",
                        e
                    )));
                }
            }
        } else {
            None
        };

        self.workflows_api
            .list_workflows_filtered(
                offset,
                sort_by,
                reverse_sort,
                limit,
                name,
                user,
                description,
                is_archived,
                access_group,
                accessible_ids,
                context,
            )
            .await
    }

    pub(super) async fn transport_cancel_workflow(
        &self,
        id: i64,
        context: &C,
    ) -> Result<CancelWorkflowResponse, ApiError> {
        log_call!(info, context, "cancel_workflow(workflow_id={})", id);
        authorize_workflow!(self, id, context, CancelWorkflowResponse);
        // Note: cancel_workflow already records a persistent "workflow_canceled" event inside
        // its transaction (see workflows_api::cancel_workflow), so no event is recorded here.
        let response = self.workflows_api.cancel_workflow(id, context).await?;
        Ok(response)
    }

    pub(super) async fn transport_archive_workflow(
        &self,
        id: i64,
        body: models::ArchiveWorkflowRequest,
        context: &C,
    ) -> Result<ArchiveWorkflowResponse, ApiError> {
        log_call!(
            info,
            context,
            "archive_workflow(workflow_id={}, is_archived={})",
            id,
            body.is_archived,
        );
        authorize_workflow!(self, id, context, ArchiveWorkflowResponse);
        // When archiving, drop the workflow from the failures cache (mirrors
        // the previous behavior of the now-removed update_workflow_status flow).
        if body.is_archived
            && let Ok(mut set) = self.workflows_with_failures.write()
        {
            set.remove(&id);
        }
        let is_archived = body.is_archived;
        let result = self
            .workflows_api
            .archive_workflow(id, body, context)
            .await?;
        if let ArchiveWorkflowResponse::SuccessfulResponse(_) = result {
            self.record_user_action_event(
                id,
                "archive_workflow",
                serde_json::json!({
                    "workflow_id": id,
                    "is_archived": is_archived,
                }),
                context,
            )
            .await;
        }
        Ok(result)
    }

    pub(super) async fn transport_delete_events(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteEventsResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteEventsResponse);
        self.events_api.delete_events(workflow_id, context).await
    }

    pub(super) async fn transport_list_events(
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
        let (offset, limit) = authorize_workflow_and_paginate!(
            self,
            workflow_id,
            context,
            ListEventsResponse,
            offset,
            limit
        );
        self.events_api
            .list_events(
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

    pub(super) async fn transport_list_job_dependencies(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListJobDependenciesResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, ListJobDependenciesResponse);
        self.workflows_api
            .list_job_dependencies(workflow_id, offset, limit, sort_by, reverse_sort, context)
            .await
    }

    pub(super) async fn transport_list_job_file_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListJobFileRelationshipsResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, ListJobFileRelationshipsResponse);
        self.workflows_api
            .list_job_file_relationships(workflow_id, offset, limit, sort_by, reverse_sort, context)
            .await
    }

    pub(super) async fn transport_list_job_user_data_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListJobUserDataRelationshipsResponse, ApiError> {
        authorize_workflow!(
            self,
            workflow_id,
            context,
            ListJobUserDataRelationshipsResponse
        );
        self.workflows_api
            .list_job_user_data_relationships(
                workflow_id,
                offset,
                limit,
                sort_by,
                reverse_sort,
                context,
            )
            .await
    }

    pub(super) async fn transport_get_event(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetEventResponse, ApiError> {
        authorize_resource!(self, id, "event", context, GetEventResponse);
        self.events_api.get_event(id, context).await
    }

    pub(super) async fn transport_get_workflow(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetWorkflowResponse, ApiError> {
        authorize_workflow!(self, id, context, GetWorkflowResponse);
        self.workflows_api.get_workflow(id, context).await
    }

    pub(super) async fn transport_is_workflow_complete(
        &self,
        id: i64,
        context: &C,
    ) -> Result<IsWorkflowCompleteResponse, ApiError> {
        authorize_workflow!(self, id, context, IsWorkflowCompleteResponse);
        self.workflows_api.is_workflow_complete(id, context).await
    }

    pub(super) async fn transport_get_workflow_status(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetWorkflowStatusResponse, ApiError> {
        authorize_workflow!(self, id, context, GetWorkflowStatusResponse);
        self.workflows_api.get_workflow_status(id, context).await
    }

    pub(super) async fn transport_get_slurm_job_correlations(
        &self,
        id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<GetSlurmJobCorrelationsResponse, ApiError> {
        let (offset, limit) = authorize_workflow_and_paginate!(
            self,
            id,
            context,
            GetSlurmJobCorrelationsResponse,
            offset,
            limit
        );
        self.workflows_api
            .get_slurm_job_correlations(id, offset, limit, context)
            .await
    }

    pub(super) async fn transport_get_running_jobs(
        &self,
        id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<GetRunningJobsResponse, ApiError> {
        let (offset, limit) = authorize_workflow_and_paginate!(
            self,
            id,
            context,
            GetRunningJobsResponse,
            offset,
            limit
        );
        self.workflows_api
            .get_running_jobs(id, offset, limit, context)
            .await
    }

    pub(super) async fn transport_is_workflow_uninitialized(
        &self,
        id: i64,
        context: &C,
    ) -> Result<IsWorkflowUninitializedResponse, ApiError> {
        authorize_workflow!(self, id, context, IsWorkflowUninitializedResponse);
        self.workflows_api
            .is_workflow_uninitialized(id, context)
            .await
    }

    pub(super) async fn transport_update_event(
        &self,
        id: i64,
        body: serde_json::Value,
        context: &C,
    ) -> Result<UpdateEventResponse, ApiError> {
        authorize_resource!(self, id, "event", context, UpdateEventResponse);
        self.events_api.update_event(id, body, context).await
    }

    pub(super) async fn transport_update_workflow(
        &self,
        id: i64,
        body: models::WorkflowModel,
        context: &C,
    ) -> Result<UpdateWorkflowResponse, ApiError> {
        authorize_workflow!(self, id, context, UpdateWorkflowResponse);
        self.workflows_api.update_workflow(id, body, context).await
    }

    pub(super) async fn transport_delete_event(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteEventResponse, ApiError> {
        authorize_resource!(self, id, "event", context, DeleteEventResponse);
        self.events_api.delete_event(id, context).await
    }

    pub(super) async fn transport_delete_workflow(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteWorkflowResponse, ApiError> {
        log_call!(info, context, "delete_workflow(workflow_id={})", id);
        authorize_workflow!(self, id, context, DeleteWorkflowResponse);
        if let Ok(mut set) = self.workflows_with_failures.write() {
            set.remove(&id);
        }
        self.workflows_api.delete_workflow(id, context).await
    }

    pub(super) async fn transport_reset_workflow_status(
        &self,
        id: i64,
        force: Option<bool>,
        context: &C,
    ) -> Result<ResetWorkflowStatusResponse, ApiError> {
        log_call!(
            info,
            context,
            "reset_workflow_status(workflow_id={}, force={:?})",
            id,
            force,
        );

        authorize_workflow!(self, id, context, ResetWorkflowStatusResponse);
        if let Ok(mut set) = self.workflows_with_failures.write() {
            set.remove(&id);
        }

        let force_value = force.unwrap_or(false);
        let result = self
            .workflows_api
            .reset_workflow_status(id, force, context)
            .await?;

        if let ResetWorkflowStatusResponse::SuccessfulResponse(_) = result {
            self.record_user_action_event(
                id,
                "reset_workflow_status",
                serde_json::json!({
                    "workflow_id": id,
                    "force": force_value,
                }),
                context,
            )
            .await;
        }

        Ok(result)
    }
    pub(super) async fn transport_claim_jobs_based_on_resources(
        &self,
        id: i64,
        body: models::ComputeNodesResources,
        limit: i64,
        strict_scheduler_match: Option<bool>,
        run_id: Option<i64>,
        compute_node_id: Option<i64>,
        context: &C,
    ) -> Result<ClaimJobsBasedOnResources, ApiError> {
        log_call!(
            debug,
            context,
            "claim_jobs_based_on_resources({}, {:?}, {:?}, strict_scheduler_match={:?}, run_id={:?}, compute_node_id={:?})",
            id,
            body,
            limit,
            strict_scheduler_match,
            run_id,
            compute_node_id,
        );

        authorize_workflow!(self, id, context, ClaimJobsBasedOnResources);

        if let Err(err) = models::validate_labels(body.labels.as_deref()) {
            return Ok(
                ClaimJobsBasedOnResources::UnprocessableContentErrorResponse(
                    message_error_response(err),
                ),
            );
        }

        let workflow = match self.get_workflow(id, context).await {
            Ok(GetWorkflowResponse::SuccessfulResponse(workflow)) => workflow,
            Ok(_) => {
                error!(
                    "Unexpected response from get_workflow for workflow_id={}",
                    id
                );
                return Err(ApiError(
                    "Unexpected response from get_workflow".to_string(),
                ));
            }
            Err(e) => return Err(e),
        };

        let start = match crate::server::api::jobs::ClaimStart::from_query(run_id, compute_node_id)
        {
            Ok(start) => start,
            Err(e) => {
                return Ok(
                    ClaimJobsBasedOnResources::UnprocessableContentErrorResponse(
                        message_error_response(e),
                    ),
                );
            }
        };

        if workflow.is_canceled.unwrap_or(false) {
            // Nothing is claimed, but still reject a stale run so its runner stops.
            if start.is_some() {
                let mut conn = self.pool.acquire().await.map_err(|e| {
                    error!("Failed to acquire database connection: {}", e);
                    ApiError("Database connection error".to_string())
                })?;
                let valid = crate::server::api::jobs::claim_start_is_valid(&mut conn, id, start)
                    .await
                    .map_err(|e| {
                        error!("Database error validating claim start: {}", e);
                        ApiError("Database error".to_string())
                    })?;
                if !valid {
                    return Ok(
                        ClaimJobsBasedOnResources::UnprocessableContentErrorResponse(
                            message_error_response(crate::CLAIM_START_MISMATCH),
                        ),
                    );
                }
            }
            return Ok(ClaimJobsBasedOnResources::SuccessfulResponse(
                models::ClaimJobsBasedOnResources {
                    jobs: Some(vec![]),
                    reason: Some("Workflow is canceled".to_string()),
                },
            ));
        }

        if workflow.is_draft.unwrap_or(false) {
            return Ok(ClaimJobsBasedOnResources::SuccessfulResponse(
                models::ClaimJobsBasedOnResources {
                    jobs: Some(vec![]),
                    reason: Some("Workflow is a draft".to_string()),
                },
            ));
        }

        let response = self
            .transport_prepare_ready_jobs(id, body, limit, strict_scheduler_match, start, context)
            .await?;
        if let (Some(start), ClaimJobsBasedOnResources::SuccessfulResponse(r)) = (start, &response)
        {
            self.broadcast_jobs_started(r.jobs.as_deref().unwrap_or_default(), start);
        }
        Ok(response)
    }
}

/// Build the shared-check input for a workflow from its rows. Also returns errors for workflow
/// actions whose `scheduler_id` is not a Slurm scheduler of the workflow (action configs are
/// JSON, so no foreign key catches them).
async fn load_check_input(
    conn: &mut sqlx::SqliteConnection,
    workflow_id: i64,
    execution_config: Option<models::ExecutionConfig>,
) -> Result<(crate::workflow_validation::WorkflowCheckInput, Vec<String>), ApiError> {
    use crate::workflow_validation::{
        ActionInfo, JobInfo, ResourceInfo, SchedulerInfo, WorkflowCheckInput,
    };
    use std::collections::HashMap;
    let db_err = |what: &'static str| {
        move |e: sqlx::Error| database_lock_aware_error(e, format!("Failed to load {}", what))
    };

    type SchedRow = (
        i64,
        Option<String>,
        i64,
        String,
        Option<String>,
        Option<String>,
    );
    let sched_rows: Vec<SchedRow> = sqlx::query_as(
        "SELECT id, name, nodes, walltime, mem, gres FROM slurm_scheduler WHERE workflow_id = ?",
    )
    .bind(workflow_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(db_err("slurm schedulers"))?;
    let mut sched_idx = HashMap::new();
    let mut schedulers = Vec::with_capacity(sched_rows.len());
    for (i, (id, name, nodes, walltime, mem, gres)) in sched_rows.into_iter().enumerate() {
        sched_idx.insert(id, i);
        schedulers.push(SchedulerInfo {
            name: name.unwrap_or_else(|| format!("id={}", id)),
            nodes,
            walltime,
            mem,
            gres,
        });
    }

    let rr_rows: Vec<(i64, String, i64, i64, String, String)> = sqlx::query_as(
        "SELECT id, name, num_nodes, num_gpus, memory, runtime \
         FROM resource_requirements WHERE workflow_id = ?",
    )
    .bind(workflow_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(db_err("resource requirements"))?;
    let mut rr_idx = HashMap::new();
    let mut resource_requirements = Vec::with_capacity(rr_rows.len());
    for (i, (id, name, num_nodes, num_gpus, memory, runtime)) in rr_rows.into_iter().enumerate() {
        rr_idx.insert(id, i);
        resource_requirements.push(ResourceInfo {
            name,
            num_nodes,
            num_gpus,
            memory,
            runtime,
        });
    }

    // Only the scheduler and resource checks look at jobs, and only when there are schedulers.
    let jobs = if schedulers.is_empty() {
        Vec::new()
    } else {
        let job_rows: Vec<(String, Option<i64>, Option<i64>)> = sqlx::query_as(
            "SELECT name, resource_requirements_id, scheduler_id FROM job WHERE workflow_id = ?",
        )
        .bind(workflow_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(db_err("jobs"))?;
        job_rows
            .into_iter()
            .map(|(name, rr_id, sched_id)| JobInfo {
                name,
                resource_requirements: rr_id.and_then(|id| rr_idx.get(&id).copied()),
                scheduler: sched_id.and_then(|id| sched_idx.get(&id).copied()),
            })
            .collect()
    };

    let action_rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT id, action_type, action_config FROM workflow_action WHERE workflow_id = ?",
    )
    .bind(workflow_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(db_err("workflow actions"))?;
    let mut errors = Vec::new();
    let mut actions = Vec::with_capacity(action_rows.len());
    for (id, action_type, config) in action_rows {
        let config: serde_json::Value = serde_json::from_str(&config).unwrap_or_default();
        let scheduler_type = config["scheduler_type"].as_str().map(str::to_string);
        let mut scheduler = None;
        if action_type == "schedule_nodes"
            && scheduler_type.as_deref() == Some("slurm")
            && let Some(sched_id) = config["scheduler_id"].as_i64()
        {
            scheduler = sched_idx.get(&sched_id).copied();
            if scheduler.is_none() {
                errors.push(format!(
                    "Workflow action {} references slurm scheduler_id={}, which is not a \
                     scheduler of workflow_id={}",
                    id, sched_id, workflow_id
                ));
            }
        }
        actions.push(ActionInfo {
            action_type,
            scheduler_type,
            scheduler,
            start_one_worker_per_node: config["start_one_worker_per_node"]
                .as_bool()
                .unwrap_or(false),
        });
    }

    // Action job_ids are a JSON column, so no foreign key keeps them inside the workflow.
    let foreign_job_ids: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT a.id, CAST(j.value AS INTEGER) FROM workflow_action a, json_each(a.job_ids) j \
         WHERE a.workflow_id = ?1 AND a.job_ids IS NOT NULL AND NOT EXISTS \
         (SELECT 1 FROM job WHERE job.id = j.value AND job.workflow_id = ?1)",
    )
    .bind(workflow_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(db_err("workflow action job ids"))?;
    for (action_id, job_id) in foreign_job_ids {
        errors.push(format!(
            "Workflow action {} references job_id={}, which is not a job of workflow_id={}",
            action_id, job_id, workflow_id
        ));
    }

    Ok((
        WorkflowCheckInput {
            execution_config,
            schedulers,
            resource_requirements,
            jobs,
            actions,
        },
        errors,
    ))
}

/// Cycle message naming the jobs, since the ids of a draft that a client then deletes are of
/// no use to the user.
async fn named_cycle_message(
    conn: &mut sqlx::SqliteConnection,
    cycle: &[i64],
) -> Result<String, ApiError> {
    let ids_json = serde_json::to_string(cycle)
        .map_err(|e| ApiError(format!("Failed to serialize job ids: {}", e)))?;
    let names: std::collections::HashMap<i64, String> =
        sqlx::query_as("SELECT id, name FROM job WHERE id IN (SELECT value FROM json_each(?))")
            .bind(ids_json)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| database_lock_aware_error(e, "Failed to load job names"))?
            .into_iter()
            .collect();
    let path = cycle
        .iter()
        .chain(cycle.first())
        .map(|id| match names.get(id) {
            Some(name) => format!("'{}'", name),
            None => format!("job_id={}", id),
        })
        .collect::<Vec<_>>()
        .join(" -> ");
    Ok(format!(
        "Job dependency graph contains a cycle (job depends on job): {}",
        path
    ))
}

/// Delete draft workflows created more than `timeout_secs` ago, every `timeout_secs / 4`
/// seconds (between 1 second and 5 minutes). A draft is abandoned when the client that was
/// building it died before calling `finalize_workflow`.
pub(super) async fn background_draft_cleanup_task<C>(server: Server<C>, timeout_secs: u64)
where
    C: Has<XSpanIdString> + Send + Sync,
{
    let interval = std::time::Duration::from_secs((timeout_secs / 4).clamp(1, 300));
    info!(
        "Starting abandoned draft workflow cleanup (timeout = {}s, interval = {:?})",
        timeout_secs, interval
    );
    loop {
        tokio::time::sleep(interval).await;
        if let Err(e) = delete_abandoned_drafts(&server, timeout_secs).await {
            error!("Failed to delete abandoned draft workflows: {}", e);
        }
    }
}

async fn delete_abandoned_drafts<C>(server: &Server<C>, timeout_secs: u64) -> Result<(), ApiError>
where
    C: Has<XSpanIdString> + Send + Sync,
{
    // workflow.timestamp is the creation time, formatted so that string order is time order.
    let cutoff = (chrono::Utc::now()
        - chrono::Duration::seconds(i64::try_from(timeout_secs).unwrap_or(i64::MAX)))
    .format("%Y-%m-%dT%H:%M:%S%.3fZ")
    .to_string();
    let ids: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM workflow WHERE is_draft = 1 AND timestamp < ?")
            .bind(&cutoff)
            .fetch_all(server.pool.as_ref())
            .await
            .map_err(|e| database_lock_aware_error(e, "Failed to list abandoned drafts"))?;
    for id in ids {
        match server
            .workflows_api
            .delete_workflow_rows(id, Some(&cutoff))
            .await
        {
            Ok(true) => info!("Deleted abandoned draft workflow_id={}", id),
            Ok(false) => {} // finalized or deleted since the scan
            Err(e) => error!("Failed to delete abandoned draft workflow_id={}: {}", id, e),
        }
    }
    Ok(())
}
