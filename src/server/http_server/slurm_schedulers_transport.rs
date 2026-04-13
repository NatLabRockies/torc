use super::*;
use crate::server::api::SchedulersApi;

#[allow(clippy::too_many_arguments)]
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync,
{
    pub(super) async fn transport_create_slurm_scheduler(
        &self,
        body: models::SlurmSchedulerModel,
        context: &C,
    ) -> Result<CreateSlurmSchedulerResponse, ApiError> {
        authorize_workflow!(
            self,
            body.workflow_id,
            context,
            CreateSlurmSchedulerResponse
        );
        self.schedulers_api
            .create_slurm_scheduler(body, context)
            .await
    }
    pub(super) async fn transport_delete_slurm_schedulers(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteSlurmSchedulersResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteSlurmSchedulersResponse);
        self.schedulers_api
            .delete_slurm_schedulers(workflow_id, context)
            .await
    }
    pub(super) async fn transport_list_slurm_schedulers(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListSlurmSchedulersResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, ListSlurmSchedulersResponse);
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.schedulers_api
            .list_slurm_schedulers(workflow_id, offset, limit, sort_by, reverse_sort, context)
            .await
    }
    pub(super) async fn transport_get_slurm_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetSlurmSchedulerResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "slurm_scheduler",
            context,
            GetSlurmSchedulerResponse
        );
        self.schedulers_api.get_slurm_scheduler(id, context).await
    }
    pub(super) async fn transport_update_slurm_scheduler(
        &self,
        id: i64,
        body: models::SlurmSchedulerModel,
        context: &C,
    ) -> Result<UpdateSlurmSchedulerResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "slurm_scheduler",
            context,
            UpdateSlurmSchedulerResponse
        );
        self.schedulers_api
            .update_slurm_scheduler(id, body, context)
            .await
    }
    pub(super) async fn transport_delete_slurm_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteSlurmSchedulerResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "slurm_scheduler",
            context,
            DeleteSlurmSchedulerResponse
        );
        self.schedulers_api
            .delete_slurm_scheduler(id, context)
            .await
    }
}
