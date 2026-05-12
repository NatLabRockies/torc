use super::*;
use crate::server::api::{UserDataApi, message_error_response};

#[allow(clippy::too_many_arguments)]
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync,
{
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
    pub(super) async fn transport_create_user_data_list(
        &self,
        body: models::UserDataListModel,
        context: &C,
    ) -> Result<CreateUserDataListResponse, ApiError> {
        if body.user_data.is_empty() {
            return self
                .user_data_api
                .create_user_data_list(body, context)
                .await;
        }

        let first_workflow_id = body.user_data[0].workflow_id;
        for entry in &body.user_data {
            if entry.workflow_id != first_workflow_id {
                return Ok(
                    CreateUserDataListResponse::UnprocessableContentErrorResponse(
                        message_error_response(format!(
                            "All user_data entries in a batch must have the same workflow_id. Found workflow_ids: {} and {}",
                            first_workflow_id, entry.workflow_id
                        )),
                    ),
                );
            }
        }

        authorize_workflow!(self, first_workflow_id, context, CreateUserDataListResponse);
        self.user_data_api
            .create_user_data_list(body, context)
            .await
    }
    pub(super) async fn transport_delete_all_user_data(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteAllUserDataResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteAllUserDataResponse);
        self.user_data_api
            .delete_all_user_data(workflow_id, context)
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
        let (offset, limit) = authorize_workflow_and_paginate!(
            self,
            workflow_id,
            context,
            ListUserDataResponse,
            offset,
            limit
        );
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
    pub(super) async fn transport_update_user_data(
        &self,
        id: i64,
        body: models::UserDataModel,
        context: &C,
    ) -> Result<UpdateUserDataResponse, ApiError> {
        authorize_resource!(self, id, "user_data", context, UpdateUserDataResponse);
        self.user_data_api.update_user_data(id, body, context).await
    }
    pub(super) async fn transport_delete_user_data(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteUserDataResponse, ApiError> {
        authorize_resource!(self, id, "user_data", context, DeleteUserDataResponse);
        self.user_data_api.delete_user_data(id, context).await
    }
}
