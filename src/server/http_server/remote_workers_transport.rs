use super::*;
use crate::server::api::RemoteWorkersApi;

#[allow(clippy::too_many_arguments)]
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync,
{
    pub(super) async fn transport_create_remote_workers(
        &self,
        workflow_id: i64,
        workers: Vec<String>,
        context: &C,
    ) -> Result<CreateRemoteWorkersResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, CreateRemoteWorkersResponse);
        self.remote_workers_api
            .create_remote_workers(workflow_id, workers, context)
            .await
    }
    pub(super) async fn transport_list_remote_workers(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<ListRemoteWorkersResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, ListRemoteWorkersResponse);
        self.remote_workers_api
            .list_remote_workers(workflow_id, context)
            .await
    }
    pub(super) async fn transport_delete_remote_worker(
        &self,
        workflow_id: i64,
        worker: String,
        context: &C,
    ) -> Result<DeleteRemoteWorkerResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteRemoteWorkerResponse);
        self.remote_workers_api
            .delete_remote_worker(workflow_id, worker, context)
            .await
    }
}
