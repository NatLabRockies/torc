use super::Server;
use crate::models;
use crate::server::api::ComputeNodesApi;
use crate::server::api::compute_nodes::update_response_from_get;
use crate::server::api_responses::{GetComputeNodeResponse, UpdateComputeNodeResponse};
use crate::server::transport_types::auth_types::Authorization;
use crate::server::transport_types::context_types::{ApiError, Has, XSpanIdString};
use log::warn;

impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    /// Deactivate a compute node and fail its running jobs in one transaction. Safe to call
    /// again: a second call finds no running job and sends no event.
    pub(super) async fn fail_compute_node_as_server(
        &self,
        id: i64,
        context: &C,
    ) -> Result<UpdateComputeNodeResponse, ApiError> {
        let deactivated = self.fail_compute_node_jobs(id, context).await?;

        // Read the node after the commit: inside the transaction this pool read deadlocks.
        let node = match self.compute_nodes_api.get_compute_node(id, context).await? {
            GetComputeNodeResponse::SuccessfulResponse(node) => node,
            other => return update_response_from_get(other),
        };

        if deactivated {
            warn!(
                "Failed compute node workflow_id={} compute_node_id={} hostname={} pid={}",
                node.workflow_id, id, node.hostname, node.pid
            );
            self.broadcast_compute_node_stopped(
                node.workflow_id,
                models::EventSeverity::Warning,
                serde_json::json!({
                    "compute_node_id": id,
                    "hostname": node.hostname,
                    "pid": node.pid,
                    "reason": "failed",
                }),
            );
        }
        Ok(UpdateComputeNodeResponse::SuccessfulResponse(node))
    }
}
