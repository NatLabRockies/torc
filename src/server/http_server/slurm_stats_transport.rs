use super::*;
use crate::server::api::SlurmStatsApi;

#[allow(clippy::too_many_arguments)]
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync,
{
    pub(super) async fn transport_create_slurm_stats(
        &self,
        body: models::SlurmStatsModel,
        context: &C,
    ) -> Result<CreateSlurmStatsResponse, ApiError> {
        authorize_workflow!(self, body.workflow_id, context, CreateSlurmStatsResponse);
        self.slurm_stats_api.create_slurm_stats(body, context).await
    }
    pub(super) async fn transport_list_slurm_stats(
        &self,
        workflow_id: i64,
        job_id: Option<i64>,
        run_id: Option<i64>,
        attempt_id: Option<i64>,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListSlurmStatsResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, ListSlurmStatsResponse);
        self.slurm_stats_api
            .list_slurm_stats(
                workflow_id,
                job_id,
                run_id,
                attempt_id,
                offset.unwrap_or(0),
                limit.unwrap_or(MAX_RECORD_TRANSFER_COUNT),
                context,
            )
            .await
    }
}
