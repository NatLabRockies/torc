use super::*;
use crate::server::api::{EventsApi, ResourceRequirementsApi};

#[allow(clippy::too_many_arguments)]
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync,
{
    pub(super) async fn transport_create_resource_requirements(
        &self,
        body: models::ResourceRequirementsModel,
        context: &C,
    ) -> Result<CreateResourceRequirementsResponse, ApiError> {
        if body.name == "default" {
            error!(
                "Attempt to create resource requirement with reserved name 'default' via external API for workflow_id={}",
                body.workflow_id
            );
            return Err(ApiError(
                "Cannot create resource requirement named 'default' via external API".to_string(),
            ));
        }

        authorize_workflow!(
            self,
            body.workflow_id,
            context,
            CreateResourceRequirementsResponse
        );
        self.resource_requirements_api
            .create_resource_requirements(body, context)
            .await
    }
    pub(super) async fn transport_delete_all_resource_requirements(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteAllResourceRequirementsResponse, ApiError> {
        authorize_workflow!(
            self,
            workflow_id,
            context,
            DeleteAllResourceRequirementsResponse
        );
        self.resource_requirements_api
            .delete_all_resource_requirements(workflow_id, context)
            .await
    }
    pub(super) async fn transport_list_resource_requirements(
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
        authorize_workflow!(self, workflow_id, context, ListResourceRequirementsResponse);
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.resource_requirements_api
            .list_resource_requirements(
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
    pub(super) async fn transport_get_ready_job_requirements(
        &self,
        id: i64,
        scheduler_config_id: Option<i64>,
        context: &C,
    ) -> Result<GetReadyJobRequirementsResponse, ApiError> {
        debug!(
            "get_ready_job_requirements({}, {:?}) - X-Span-ID: {:?}",
            id,
            scheduler_config_id,
            Has::<XSpanIdString>::get(context).0.clone()
        );
        authorize_workflow!(self, id, context, GetReadyJobRequirementsResponse);
        error!("get_ready_job_requirements operation is not implemented");
        Err(ApiError("Api-Error: Operation is NOT implemented".into()))
    }
    pub(super) async fn transport_get_resource_requirements(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetResourceRequirementsResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "resource_requirements",
            context,
            GetResourceRequirementsResponse
        );
        self.resource_requirements_api
            .get_resource_requirements(id, context)
            .await
    }
    pub(super) async fn transport_update_resource_requirements(
        &self,
        id: i64,
        body: models::ResourceRequirementsModel,
        context: &C,
    ) -> Result<UpdateResourceRequirementsResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "resource_requirements",
            context,
            UpdateResourceRequirementsResponse
        );

        let result = self
            .resource_requirements_api
            .update_resource_requirements(id, body, context)
            .await?;

        if let UpdateResourceRequirementsResponse::SuccessfulResponse(ref rr) = result {
            let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
            let username = auth
                .map(|a| a.subject)
                .unwrap_or_else(|| "unknown".to_string());

            let event = models::EventModel::new(
                rr.workflow_id,
                serde_json::json!({
                    "category": "user_action",
                    "action": "update_resource_requirements",
                    "user": username,
                    "resource_requirements_id": id,
                    "name": rr.name,
                    "num_cpus": rr.num_cpus,
                    "num_gpus": rr.num_gpus,
                    "num_nodes": rr.num_nodes,
                    "memory": rr.memory,
                    "runtime": rr.runtime,
                }),
            );
            if let Err(e) = self.events_api.create_event(event, context).await {
                error!(
                    "Failed to create event for update_resource_requirements: {:?}",
                    e
                );
            }
        }

        Ok(result)
    }
    pub(super) async fn transport_delete_resource_requirements(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteResourceRequirementsResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "resource_requirements",
            context,
            DeleteResourceRequirementsResponse
        );
        self.resource_requirements_api
            .delete_resource_requirements(id, context)
            .await
    }
}
