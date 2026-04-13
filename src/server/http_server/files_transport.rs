use super::*;
use crate::server::api::FilesApi;

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
    pub(super) async fn transport_delete_files(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteFilesResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteFilesResponse);
        self.files_api.delete_files(workflow_id, context).await
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
    pub(super) async fn transport_get_file(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetFileResponse, ApiError> {
        authorize_resource!(self, id, "file", context, GetFileResponse);
        self.files_api.get_file(id, context).await
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
    pub(super) async fn transport_delete_file(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteFileResponse, ApiError> {
        authorize_resource!(self, id, "file", context, DeleteFileResponse);
        self.files_api.delete_file(id, context).await
    }
}
