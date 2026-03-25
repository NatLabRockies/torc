use super::*;
use crate::server::api::SchedulersApi;

#[allow(clippy::too_many_arguments)]
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync,
{
    pub(super) async fn transport_create_scheduled_compute_node(
        &self,
        body: models::ScheduledComputeNodesModel,
        context: &C,
    ) -> Result<CreateScheduledComputeNodeResponse, ApiError> {
        authorize_workflow!(
            self,
            body.workflow_id,
            context,
            CreateScheduledComputeNodeResponse
        );

        let workflow_id = body.workflow_id;
        let scheduler_id = body.scheduler_id;
        let scheduler_config_id = body.scheduler_config_id;
        let scheduler_type = body.scheduler_type.clone();

        let result = self
            .schedulers_api
            .create_scheduled_compute_node(body, context)
            .await?;

        if let CreateScheduledComputeNodeResponse::SuccessfulResponse(ref created) = result {
            self.event_broadcaster.broadcast(BroadcastEvent {
                workflow_id,
                timestamp: chrono::Utc::now().timestamp_millis(),
                event_type: "scheduler_node_created".to_string(),
                severity: models::EventSeverity::Info,
                data: serde_json::json!({
                    "category": "scheduler",
                    "scheduled_compute_node_id": created.id,
                    "scheduler_id": scheduler_id,
                    "scheduler_config_id": scheduler_config_id,
                    "scheduler_type": scheduler_type,
                    "status": created.status,
                }),
            });
        }

        Ok(result)
    }
    pub(super) async fn transport_delete_scheduled_compute_nodes(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodesResponse, ApiError> {
        authorize_workflow!(
            self,
            workflow_id,
            context,
            DeleteScheduledComputeNodesResponse
        );
        self.schedulers_api
            .delete_scheduled_compute_nodes(workflow_id, context)
            .await
    }
    pub(super) async fn transport_list_scheduled_compute_nodes(
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
        authorize_workflow!(
            self,
            workflow_id,
            context,
            ListScheduledComputeNodesResponse
        );
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.schedulers_api
            .list_scheduled_compute_nodes(
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
    pub(super) async fn transport_get_scheduled_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetScheduledComputeNodeResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "scheduled_compute_node",
            context,
            GetScheduledComputeNodeResponse
        );
        self.schedulers_api
            .get_scheduled_compute_node(id, context)
            .await
    }
    pub(super) async fn transport_update_scheduled_compute_node(
        &self,
        id: i64,
        body: models::ScheduledComputeNodesModel,
        context: &C,
    ) -> Result<UpdateScheduledComputeNodeResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "scheduled_compute_node",
            context,
            UpdateScheduledComputeNodeResponse
        );
        self.schedulers_api
            .update_scheduled_compute_node(id, body, context)
            .await
    }
    pub(super) async fn transport_delete_scheduled_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodeResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "scheduled_compute_node",
            context,
            DeleteScheduledComputeNodeResponse
        );
        self.schedulers_api
            .delete_scheduled_compute_node(id, context)
            .await
    }
}
