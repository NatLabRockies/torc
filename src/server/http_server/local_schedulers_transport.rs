use super::*;
use crate::server::api::SchedulersApi;

#[allow(clippy::too_many_arguments)]
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync,
{
    pub(super) async fn transport_create_local_scheduler(
        &self,
        body: models::LocalSchedulerModel,
        context: &C,
    ) -> Result<CreateLocalSchedulerResponse, ApiError> {
        authorize_workflow!(
            self,
            body.workflow_id,
            context,
            CreateLocalSchedulerResponse
        );
        self.schedulers_api
            .create_local_scheduler(body, context)
            .await
    }
    pub(super) async fn transport_delete_local_schedulers(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteLocalSchedulersResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteLocalSchedulersResponse);
        self.schedulers_api
            .delete_local_schedulers(workflow_id, context)
            .await
    }
    pub(super) async fn transport_list_local_schedulers(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        memory: Option<String>,
        num_cpus: Option<i64>,
        context: &C,
    ) -> Result<ListLocalSchedulersResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, ListLocalSchedulersResponse);
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.schedulers_api
            .list_local_schedulers(
                workflow_id,
                offset,
                limit,
                sort_by,
                reverse_sort,
                memory,
                num_cpus,
                context,
            )
            .await
    }
    pub(super) async fn transport_get_local_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetLocalSchedulerResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "local_scheduler",
            context,
            GetLocalSchedulerResponse
        );
        self.schedulers_api.get_local_scheduler(id, context).await
    }
    pub(super) async fn transport_update_local_scheduler(
        &self,
        id: i64,
        body: models::LocalSchedulerModel,
        context: &C,
    ) -> Result<UpdateLocalSchedulerResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "local_scheduler",
            context,
            UpdateLocalSchedulerResponse
        );
        self.schedulers_api
            .update_local_scheduler(id, body, context)
            .await
    }
    pub(super) async fn transport_delete_local_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteLocalSchedulerResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "local_scheduler",
            context,
            DeleteLocalSchedulerResponse
        );
        self.schedulers_api
            .delete_local_scheduler(id, context)
            .await
    }
}
