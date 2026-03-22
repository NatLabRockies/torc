use super::*;

impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync,
{
    pub(super) async fn transport_create_access_group(
        &self,
        body: models::AccessGroupModel,
        context: &C,
    ) -> Result<CreateAccessGroupResponse, ApiError> {
        authorize_admin!(self, context, CreateAccessGroupResponse);
        self.access_groups_api
            .create_access_group(body, context)
            .await
    }

    pub(super) async fn transport_get_access_group(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetAccessGroupResponse, ApiError> {
        authorize_admin!(self, context, GetAccessGroupResponse);
        self.access_groups_api.get_access_group(id, context).await
    }

    pub(super) async fn transport_list_access_groups(
        &self,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListAccessGroupsApiResponse, ApiError> {
        authorize_admin!(self, context, ListAccessGroupsApiResponse);
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.access_groups_api
            .list_access_groups(offset, limit, context)
            .await
    }

    pub(super) async fn transport_delete_access_group(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteAccessGroupResponse, ApiError> {
        authorize_admin!(self, context, DeleteAccessGroupResponse);
        match self.authorization_service.is_system_group(id).await {
            Ok(true) => {
                return Ok(DeleteAccessGroupResponse::ForbiddenErrorResponse(
                    forbidden_error!("Cannot delete system groups"),
                ));
            }
            Ok(false) => {}
            Err(e) => {
                return Ok(DeleteAccessGroupResponse::NotFoundErrorResponse(
                    not_found_error!(e),
                ));
            }
        }
        self.access_groups_api
            .delete_access_group(id, context)
            .await
    }

    pub(super) async fn transport_add_user_to_group(
        &self,
        group_id: i64,
        body: models::UserGroupMembershipModel,
        context: &C,
    ) -> Result<AddUserToGroupResponse, ApiError> {
        authorize_group_admin!(self, group_id, context, AddUserToGroupResponse);
        self.access_groups_api
            .add_user_to_group(group_id, body, context)
            .await
    }

    pub(super) async fn transport_remove_user_from_group(
        &self,
        group_id: i64,
        user_name: String,
        context: &C,
    ) -> Result<RemoveUserFromGroupResponse, ApiError> {
        authorize_group_admin!(self, group_id, context, RemoveUserFromGroupResponse);
        self.access_groups_api
            .remove_user_from_group(group_id, &user_name, context)
            .await
    }

    pub(super) async fn transport_list_group_members(
        &self,
        group_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListGroupMembersResponse, ApiError> {
        authorize_admin!(self, context, ListGroupMembersResponse);
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.access_groups_api
            .list_group_members(group_id, offset, limit, context)
            .await
    }

    pub(super) async fn transport_list_user_groups(
        &self,
        user_name: String,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListUserGroupsApiResponse, ApiError> {
        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        let is_self = auth
            .as_ref()
            .map(|a| a.subject == user_name)
            .unwrap_or(false);
        if !is_self {
            authorize_admin!(self, context, ListUserGroupsApiResponse);
        }
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.access_groups_api
            .list_user_groups(&user_name, offset, limit, context)
            .await
    }

    pub(super) async fn transport_add_workflow_to_group(
        &self,
        workflow_id: i64,
        group_id: i64,
        context: &C,
    ) -> Result<AddWorkflowToGroupResponse, ApiError> {
        authorize_workflow_group!(
            self,
            workflow_id,
            group_id,
            context,
            AddWorkflowToGroupResponse
        );
        self.access_groups_api
            .add_workflow_to_group(workflow_id, group_id, context)
            .await
    }

    pub(super) async fn transport_remove_workflow_from_group(
        &self,
        workflow_id: i64,
        group_id: i64,
        context: &C,
    ) -> Result<RemoveWorkflowFromGroupResponse, ApiError> {
        authorize_workflow_group!(
            self,
            workflow_id,
            group_id,
            context,
            RemoveWorkflowFromGroupResponse
        );
        self.access_groups_api
            .remove_workflow_from_group(workflow_id, group_id, context)
            .await
    }

    pub(super) async fn transport_list_workflow_groups(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListWorkflowGroupsResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, ListWorkflowGroupsResponse);
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.access_groups_api
            .list_workflow_groups(workflow_id, offset, limit, context)
            .await
    }

    pub(super) async fn transport_check_workflow_access(
        &self,
        workflow_id: i64,
        user_name: String,
        context: &C,
    ) -> Result<CheckWorkflowAccessResponse, ApiError> {
        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        if self.authorization_service.enforce_access_control() {
            match auth {
                None => {
                    return Ok(CheckWorkflowAccessResponse::ForbiddenErrorResponse(
                        forbidden_error!("Authentication required"),
                    ));
                }
                Some(ref a) => {
                    if a.subject != user_name
                        && !self
                            .authorization_service
                            .check_admin_access(&auth)
                            .await
                            .is_allowed()
                    {
                        return Ok(CheckWorkflowAccessResponse::ForbiddenErrorResponse(
                            forbidden_error!(format!(
                                "Only admins can check access for other users (requester: {}, requested: {})",
                                a.subject, user_name
                            )),
                        ));
                    }
                }
            }
        }

        self.access_groups_api
            .check_workflow_access(workflow_id, &user_name, context)
            .await
    }
}
