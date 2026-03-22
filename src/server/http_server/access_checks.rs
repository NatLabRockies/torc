use super::*;

impl<C> Server<C>
where
    C: Has<Option<Authorization>> + Send + Sync,
{
    /// Helper to extract authorization from context and check workflow access
    pub(super) async fn check_workflow_access_for_context(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> AccessCheckResult {
        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        self.authorization_service
            .check_workflow_access(&auth, workflow_id)
            .await
    }

    /// Helper to extract authorization from context and check job access
    pub(super) async fn check_job_access_for_context(
        &self,
        job_id: i64,
        context: &C,
    ) -> AccessCheckResult {
        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        self.authorization_service
            .check_job_access(&auth, job_id)
            .await
    }

    /// Helper to extract authorization from context and check admin access
    pub(super) async fn check_admin_access_for_context(&self, context: &C) -> AccessCheckResult {
        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        self.authorization_service.check_admin_access(&auth).await
    }

    /// Helper to extract authorization from context and check group admin access
    pub(super) async fn check_group_admin_access_for_context(
        &self,
        group_id: i64,
        context: &C,
    ) -> AccessCheckResult {
        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        self.authorization_service
            .check_group_admin_access(&auth, group_id)
            .await
    }

    /// Helper to extract authorization from context and check workflow group access
    pub(super) async fn check_workflow_group_access_for_context(
        &self,
        workflow_id: i64,
        group_id: i64,
        context: &C,
    ) -> AccessCheckResult {
        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        self.authorization_service
            .check_workflow_group_access(&auth, workflow_id, group_id)
            .await
    }

    /// Helper to extract authorization from context and check resource access
    pub(super) async fn check_resource_access_for_context(
        &self,
        resource_id: i64,
        table_name: &str,
        context: &C,
    ) -> AccessCheckResult {
        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        self.authorization_service
            .check_resource_access(&auth, resource_id, table_name)
            .await
    }
}
