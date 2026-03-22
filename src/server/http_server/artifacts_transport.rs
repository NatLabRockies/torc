use super::*;
use crate::server::api::{FilesApi, ResultsApi, RoCrateApi, UserDataApi};

#[allow(clippy::too_many_arguments)]
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync,
{
    pub(super) async fn transport_create_file(
        &self,
        file: models::FileModel,
        context: &C,
    ) -> Result<CreateFileResponse, ApiError> {
        authorize_workflow!(self, file.workflow_id, context, CreateFileResponse);
        self.files_api.create_file(file, context).await
    }

    pub(super) async fn transport_create_ro_crate_entity(
        &self,
        body: models::RoCrateEntityModel,
        context: &C,
    ) -> Result<CreateRoCrateEntityResponse, ApiError> {
        authorize_workflow!(self, body.workflow_id, context, CreateRoCrateEntityResponse);
        self.ro_crate_api
            .create_ro_crate_entity(body, context)
            .await
    }

    pub(super) async fn transport_get_ro_crate_entity(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetRoCrateEntityResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "ro_crate_entity",
            context,
            GetRoCrateEntityResponse
        );

        self.ro_crate_api.get_ro_crate_entity(id, context).await
    }

    pub(super) async fn transport_list_ro_crate_entities(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListRoCrateEntitiesResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, ListRoCrateEntitiesResponse);
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.ro_crate_api
            .list_ro_crate_entities(workflow_id, offset, limit, context)
            .await
    }

    pub(super) async fn transport_update_ro_crate_entity(
        &self,
        id: i64,
        body: models::RoCrateEntityModel,
        context: &C,
    ) -> Result<UpdateRoCrateEntityResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "ro_crate_entity",
            context,
            UpdateRoCrateEntityResponse
        );

        self.ro_crate_api
            .update_ro_crate_entity(id, body, context)
            .await
    }

    pub(super) async fn transport_delete_ro_crate_entity(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteRoCrateEntityResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "ro_crate_entity",
            context,
            DeleteRoCrateEntityResponse
        );

        self.ro_crate_api
            .delete_ro_crate_entity(id, body, context)
            .await
    }

    pub(super) async fn transport_delete_ro_crate_entities(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteRoCrateEntitiesResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteRoCrateEntitiesResponse);
        self.ro_crate_api
            .delete_ro_crate_entities(workflow_id, body, context)
            .await
    }

    pub(super) async fn transport_create_result(
        &self,
        body: models::ResultModel,
        context: &C,
    ) -> Result<CreateResultResponse, ApiError> {
        authorize_workflow!(self, body.workflow_id, context, CreateResultResponse);
        self.results_api.create_result(body, context).await
    }

    pub(super) async fn transport_create_user_data(
        &self,
        body: models::UserDataModel,
        consumer_job_id: Option<i64>,
        producer_job_id: Option<i64>,
        context: &C,
    ) -> Result<CreateUserDataResponse, ApiError> {
        authorize_workflow!(self, body.workflow_id, context, CreateUserDataResponse);
        self.user_data_api
            .create_user_data(body, consumer_job_id, producer_job_id, context)
            .await
    }

    pub(super) async fn transport_delete_files(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteFilesResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteFilesResponse);
        self.files_api
            .delete_files(workflow_id, body, context)
            .await
    }

    pub(super) async fn transport_delete_results(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteResultsResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteResultsResponse);
        self.results_api
            .delete_results(workflow_id, body, context)
            .await
    }

    pub(super) async fn transport_delete_all_user_data(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteAllUserDataResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteAllUserDataResponse);
        self.user_data_api
            .delete_all_user_data(workflow_id, body, context)
            .await
    }

    pub(super) async fn transport_list_files(
        &self,
        workflow_id: i64,
        produced_by_job_id: Option<i64>,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        name: Option<String>,
        path: Option<String>,
        is_output: Option<bool>,
        context: &C,
    ) -> Result<ListFilesResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, ListFilesResponse);
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.files_api
            .list_files(
                workflow_id,
                produced_by_job_id,
                offset,
                limit,
                sort_by,
                reverse_sort,
                name,
                path,
                is_output,
                context,
            )
            .await
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

    pub(super) async fn transport_list_user_data(
        &self,
        workflow_id: i64,
        consumer_job_id: Option<i64>,
        producer_job_id: Option<i64>,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        name: Option<String>,
        is_ephemeral: Option<bool>,
        context: &C,
    ) -> Result<ListUserDataResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, ListUserDataResponse);

        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.user_data_api
            .list_user_data(
                workflow_id,
                consumer_job_id,
                producer_job_id,
                offset,
                limit,
                sort_by,
                reverse_sort,
                name,
                is_ephemeral,
                context,
            )
            .await
    }

    pub(super) async fn transport_get_file(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetFileResponse, ApiError> {
        authorize_resource!(self, id, "file", context, GetFileResponse);
        self.files_api.get_file(id, context).await
    }

    pub(super) async fn transport_get_result(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetResultResponse, ApiError> {
        authorize_resource!(self, id, "result", context, GetResultResponse);
        self.results_api.get_result(id, context).await
    }

    pub(super) async fn transport_get_user_data(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetUserDataResponse, ApiError> {
        authorize_resource!(self, id, "user_data", context, GetUserDataResponse);
        self.user_data_api.get_user_data(id, context).await
    }

    pub(super) async fn transport_list_missing_user_data(
        &self,
        id: i64,
        context: &C,
    ) -> Result<ListMissingUserDataResponse, ApiError> {
        authorize_workflow!(self, id, context, ListMissingUserDataResponse);
        self.user_data_api.list_missing_user_data(id, context).await
    }

    pub(super) async fn transport_list_required_existing_files(
        &self,
        id: i64,
        context: &C,
    ) -> Result<ListRequiredExistingFilesResponse, ApiError> {
        authorize_workflow!(self, id, context, ListRequiredExistingFilesResponse);
        self.files_api
            .list_required_existing_files(id, context)
            .await
    }

    pub(super) async fn transport_update_file(
        &self,
        id: i64,
        body: models::FileModel,
        context: &C,
    ) -> Result<UpdateFileResponse, ApiError> {
        authorize_resource!(self, id, "file", context, UpdateFileResponse);
        self.files_api.update_file(id, body, context).await
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

    pub(super) async fn transport_update_user_data(
        &self,
        id: i64,
        body: models::UserDataModel,
        context: &C,
    ) -> Result<UpdateUserDataResponse, ApiError> {
        authorize_resource!(self, id, "user_data", context, UpdateUserDataResponse);
        self.user_data_api.update_user_data(id, body, context).await
    }

    pub(super) async fn transport_delete_file(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteFileResponse, ApiError> {
        authorize_resource!(self, id, "file", context, DeleteFileResponse);
        self.files_api.delete_file(id, body, context).await
    }

    pub(super) async fn transport_delete_result(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteResultResponse, ApiError> {
        authorize_resource!(self, id, "result", context, DeleteResultResponse);
        self.results_api.delete_result(id, body, context).await
    }

    pub(super) async fn transport_delete_user_data(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteUserDataResponse, ApiError> {
        authorize_resource!(self, id, "user_data", context, DeleteUserDataResponse);
        self.user_data_api.delete_user_data(id, body, context).await
    }
}
