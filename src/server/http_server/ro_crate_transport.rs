use super::*;
use crate::server::api::RoCrateApi;

#[allow(clippy::too_many_arguments)]
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync,
{
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
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListRoCrateEntitiesResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, ListRoCrateEntitiesResponse);
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.ro_crate_api
            .list_ro_crate_entities(workflow_id, offset, limit, sort_by, reverse_sort, context)
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
        context: &C,
    ) -> Result<DeleteRoCrateEntityResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "ro_crate_entity",
            context,
            DeleteRoCrateEntityResponse
        );

        self.ro_crate_api.delete_ro_crate_entity(id, context).await
    }
    pub(super) async fn transport_delete_ro_crate_entities(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteRoCrateEntitiesResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteRoCrateEntitiesResponse);
        self.ro_crate_api
            .delete_ro_crate_entities(workflow_id, context)
            .await
    }
}
