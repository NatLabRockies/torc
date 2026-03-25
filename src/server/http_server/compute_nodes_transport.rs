use super::*;
use crate::server::api::ComputeNodesApi;

#[allow(clippy::too_many_arguments)]
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync,
{
    pub(super) async fn transport_create_compute_node(
        &self,
        body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<CreateComputeNodeResponse, ApiError> {
        authorize_workflow!(self, body.workflow_id, context, CreateComputeNodeResponse);

        let result = self
            .compute_nodes_api
            .create_compute_node(body.clone(), context)
            .await?;

        if let CreateComputeNodeResponse::SuccessfulResponse(ref created) = result {
            self.event_broadcaster.broadcast(BroadcastEvent {
                workflow_id: body.workflow_id,
                timestamp: chrono::Utc::now().timestamp_millis(),
                event_type: "compute_node_started".to_string(),
                severity: models::EventSeverity::Info,
                data: serde_json::json!({
                    "compute_node_id": created.id,
                    "hostname": body.hostname,
                    "pid": body.pid,
                    "num_cpus": body.num_cpus,
                    "memory_gb": body.memory_gb,
                    "num_gpus": body.num_gpus,
                    "compute_node_type": body.compute_node_type,
                }),
            });
        }

        Ok(result)
    }
    pub(super) async fn transport_delete_compute_nodes(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteComputeNodesResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteComputeNodesResponse);
        self.compute_nodes_api
            .delete_compute_nodes(workflow_id, context)
            .await
    }
    pub(super) async fn transport_list_compute_nodes(
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
        authorize_workflow!(self, workflow_id, context, ListComputeNodesResponse);
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.compute_nodes_api
            .list_compute_nodes(
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
    pub(super) async fn transport_get_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetComputeNodeResponse, ApiError> {
        authorize_resource!(self, id, "compute_node", context, GetComputeNodeResponse);
        self.compute_nodes_api.get_compute_node(id, context).await
    }
    pub(super) async fn transport_update_compute_node(
        &self,
        id: i64,
        body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<UpdateComputeNodeResponse, ApiError> {
        authorize_resource!(self, id, "compute_node", context, UpdateComputeNodeResponse);
        let result = self
            .compute_nodes_api
            .update_compute_node(id, body.clone(), context)
            .await?;
        if let UpdateComputeNodeResponse::SuccessfulResponse(ref _updated) = result
            && body.is_active == Some(false)
        {
            self.event_broadcaster.broadcast(BroadcastEvent {
                workflow_id: body.workflow_id,
                timestamp: chrono::Utc::now().timestamp_millis(),
                event_type: "compute_node_stopped".to_string(),
                severity: models::EventSeverity::Info,
                data: serde_json::json!({
                    "compute_node_id": id,
                    "hostname": body.hostname,
                    "pid": body.pid,
                    "duration_seconds": body.duration_seconds,
                    "compute_node_type": body.compute_node_type,
                }),
            });
        }
        Ok(result)
    }
    pub(super) async fn transport_delete_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteComputeNodeResponse, ApiError> {
        authorize_resource!(self, id, "compute_node", context, DeleteComputeNodeResponse);
        self.compute_nodes_api
            .delete_compute_node(id, context)
            .await
    }
}
