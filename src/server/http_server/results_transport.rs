use super::*;
use crate::server::api::ResultsApi;

#[allow(clippy::too_many_arguments)]
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync,
{
    pub(super) async fn transport_create_result(
        &self,
        body: models::ResultModel,
        context: &C,
    ) -> Result<CreateResultResponse, ApiError> {
        authorize_workflow!(self, body.workflow_id, context, CreateResultResponse);
        self.results_api.create_result(body, context).await
    }
    pub(super) async fn transport_delete_results(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteResultsResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteResultsResponse);
        self.results_api.delete_results(workflow_id, context).await
    }
    pub(super) async fn transport_list_results(
        &self,
        workflow_id: i64,
        job_id: Option<i64>,
        run_id: Option<i64>,
        return_code: Option<i64>,
        status: Option<models::JobStatus>,
        compute_node_id: Option<i64>,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        all_runs: Option<bool>,
        context: &C,
    ) -> Result<ListResultsResponse, ApiError> {
        debug!(
            "list_results({}, {:?}, {:?}, {:?}, {:?}, compute_node_id={:?}, {:?}, {:?}, {:?}, {:?}, all_runs={:?}) - X-Span-ID: {:?}",
            workflow_id,
            job_id,
            run_id,
            return_code,
            status,
            compute_node_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            all_runs,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        authorize_workflow!(self, workflow_id, context, ListResultsResponse);

        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.results_api
            .list_results(
                workflow_id,
                job_id,
                run_id,
                return_code,
                status,
                compute_node_id,
                offset,
                limit,
                sort_by,
                reverse_sort,
                all_runs,
                context,
            )
            .await
    }
    pub(super) async fn transport_get_result(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetResultResponse, ApiError> {
        authorize_resource!(self, id, "result", context, GetResultResponse);
        self.results_api.get_result(id, context).await
    }
    pub(super) async fn transport_update_result(
        &self,
        id: i64,
        body: models::ResultModel,
        context: &C,
    ) -> Result<UpdateResultResponse, ApiError> {
        authorize_resource!(self, id, "result", context, UpdateResultResponse);
        self.results_api.update_result(id, body, context).await
    }
    pub(super) async fn transport_delete_result(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteResultResponse, ApiError> {
        authorize_resource!(self, id, "result", context, DeleteResultResponse);
        self.results_api.delete_result(id, context).await
    }
}
