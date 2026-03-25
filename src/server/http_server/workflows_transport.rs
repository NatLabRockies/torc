use super::*;
use crate::server::api::{
    EventsApi, FailureHandlersApi, ResourceRequirementsApi, WorkflowActionsApi, WorkflowsApi,
};

#[allow(clippy::too_many_arguments)]
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync,
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
        authorize_workflow!(self, workflow_id, context, ListFailureHandlersResponse);
        let (offset, limit) = process_pagination_params(offset, limit)?;
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
            CreateWorkflowResponse::DefaultErrorResponse(err) => {
                Ok(CreateWorkflowResponse::DefaultErrorResponse(err))
            }
        }
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
        info!(
            "cancel_workflow(workflow_id={}) - X-Span-ID: {:?}",
            id,
            Has::<XSpanIdString>::get(context).0.clone()
        );
        authorize_workflow!(self, id, context, CancelWorkflowResponse);
        self.workflows_api.cancel_workflow(id, context).await
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
        authorize_workflow!(self, workflow_id, context, ListEventsResponse);
        let (offset, limit) = process_pagination_params(offset, limit)?;
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

    pub(super) async fn transport_get_workflow_status(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetWorkflowStatusResponse, ApiError> {
        authorize_workflow!(self, id, context, GetWorkflowStatusResponse);
        self.workflows_api.get_workflow_status(id, context).await
    }

    pub(super) async fn transport_is_workflow_complete(
        &self,
        id: i64,
        context: &C,
    ) -> Result<IsWorkflowCompleteResponse, ApiError> {
        authorize_workflow!(self, id, context, IsWorkflowCompleteResponse);
        self.workflows_api.is_workflow_complete(id, context).await
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

    pub(super) async fn transport_update_workflow_status(
        &self,
        id: i64,
        body: models::WorkflowStatusModel,
        context: &C,
    ) -> Result<UpdateWorkflowStatusResponse, ApiError> {
        authorize_workflow!(self, id, context, UpdateWorkflowStatusResponse);
        if body.is_archived == Some(true)
            && let Ok(mut set) = self.workflows_with_failures.write()
        {
            set.remove(&id);
        }
        self.workflows_api
            .update_workflow_status(id, body, context)
            .await
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
        info!(
            "delete_workflow(workflow_id={}) - X-Span-ID: {:?}",
            id,
            Has::<XSpanIdString>::get(context).0.clone()
        );
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
        info!(
            "reset_workflow_status(workflow_id={}, force={:?}) - X-Span-ID: {:?}",
            id,
            force,
            Has::<XSpanIdString>::get(context).0.clone()
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
            let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
            let username = auth
                .map(|a| a.subject)
                .unwrap_or_else(|| "unknown".to_string());

            let event = models::EventModel::new(
                id,
                serde_json::json!({
                    "category": "user_action",
                    "action": "reset_workflow_status",
                    "user": username,
                    "workflow_id": id,
                    "force": force_value,
                }),
            );
            if let Err(e) = self.events_api.create_event(event, context).await {
                error!("Failed to create event for reset_workflow_status: {:?}", e);
            }
        }

        Ok(result)
    }
    pub(super) async fn transport_claim_jobs_based_on_resources(
        &self,
        id: i64,
        body: models::ComputeNodesResources,
        limit: i64,
        strict_scheduler_match: Option<bool>,
        context: &C,
    ) -> Result<ClaimJobsBasedOnResources, ApiError> {
        debug!(
            "claim_jobs_based_on_resources({}, {:?}, {:?}, strict_scheduler_match={:?}) - X-Span-ID: {:?}",
            id,
            body,
            limit,
            strict_scheduler_match,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        authorize_workflow!(self, id, context, ClaimJobsBasedOnResources);

        let status = match self.get_workflow_status(id, context).await {
            Ok(GetWorkflowStatusResponse::SuccessfulResponse(status)) => status,
            Ok(_) => {
                error!(
                    "Unexpected response from get_workflow_status for workflow_id={}",
                    id
                );
                return Err(ApiError(
                    "Unexpected response from get_workflow_status".to_string(),
                ));
            }
            Err(e) => return Err(e),
        };

        if status.is_canceled {
            return Ok(ClaimJobsBasedOnResources::SuccessfulResponse(
                models::ClaimJobsBasedOnResources {
                    jobs: Some(vec![]),
                    reason: Some("Workflow is canceled".to_string()),
                },
            ));
        }

        self.transport_prepare_ready_jobs(id, body, limit, strict_scheduler_match, context)
            .await
    }
}
