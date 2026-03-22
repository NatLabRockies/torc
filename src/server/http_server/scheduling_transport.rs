use super::*;
use crate::server::api::{
    ComputeNodesApi, EventsApi, RemoteWorkersApi, ResourceRequirementsApi, SchedulersApi,
    SlurmStatsApi,
};

#[allow(clippy::too_many_arguments)]
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync,
{
    pub(super) async fn transport_create_compute_node(
        &self,
        body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<CreateComputeNodeResponse, ApiError> {
        authorize_workflow!(self, body.workflow_id, context, CreateComputeNodeResponse);

        let result = self
            .compute_nodes_api
            .create_compute_node(body.clone(), context)
            .await?;

        if let CreateComputeNodeResponse::SuccessfulResponse(ref created) = result {
            self.event_broadcaster.broadcast(BroadcastEvent {
                workflow_id: body.workflow_id,
                timestamp: chrono::Utc::now().timestamp_millis(),
                event_type: "compute_node_started".to_string(),
                severity: models::EventSeverity::Info,
                data: serde_json::json!({
                    "compute_node_id": created.id,
                    "hostname": body.hostname,
                    "pid": body.pid,
                    "num_cpus": body.num_cpus,
                    "memory_gb": body.memory_gb,
                    "num_gpus": body.num_gpus,
                    "compute_node_type": body.compute_node_type,
                }),
            });
        }

        Ok(result)
    }

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

    pub(super) async fn transport_create_resource_requirements(
        &self,
        body: models::ResourceRequirementsModel,
        context: &C,
    ) -> Result<CreateResourceRequirementsResponse, ApiError> {
        if body.name == "default" {
            error!(
                "Attempt to create resource requirement with reserved name 'default' via external API for workflow_id={}",
                body.workflow_id
            );
            return Err(ApiError(
                "Cannot create resource requirement named 'default' via external API".to_string(),
            ));
        }

        authorize_workflow!(
            self,
            body.workflow_id,
            context,
            CreateResourceRequirementsResponse
        );
        self.resource_requirements_api
            .create_resource_requirements(body, context)
            .await
    }

    pub(super) async fn transport_create_scheduled_compute_node(
        &self,
        body: models::ScheduledComputeNodesModel,
        context: &C,
    ) -> Result<CreateScheduledComputeNodeResponse, ApiError> {
        authorize_workflow!(
            self,
            body.workflow_id,
            context,
            CreateScheduledComputeNodeResponse
        );

        let workflow_id = body.workflow_id;
        let scheduler_id = body.scheduler_id;
        let scheduler_config_id = body.scheduler_config_id;
        let scheduler_type = body.scheduler_type.clone();

        let result = self
            .schedulers_api
            .create_scheduled_compute_node(body, context)
            .await?;

        if let CreateScheduledComputeNodeResponse::SuccessfulResponse(ref created) = result {
            self.event_broadcaster.broadcast(BroadcastEvent {
                workflow_id,
                timestamp: chrono::Utc::now().timestamp_millis(),
                event_type: "scheduler_node_created".to_string(),
                severity: models::EventSeverity::Info,
                data: serde_json::json!({
                    "category": "scheduler",
                    "scheduled_compute_node_id": created.id,
                    "scheduler_id": scheduler_id,
                    "scheduler_config_id": scheduler_config_id,
                    "scheduler_type": scheduler_type,
                    "status": created.status,
                }),
            });
        }

        Ok(result)
    }

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

    pub(super) async fn transport_create_remote_workers(
        &self,
        workflow_id: i64,
        workers: Vec<String>,
        context: &C,
    ) -> Result<CreateRemoteWorkersResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, CreateRemoteWorkersResponse);
        self.remote_workers_api
            .create_remote_workers(workflow_id, workers, context)
            .await
    }

    pub(super) async fn transport_list_remote_workers(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<ListRemoteWorkersResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, ListRemoteWorkersResponse);
        self.remote_workers_api
            .list_remote_workers(workflow_id, context)
            .await
    }

    pub(super) async fn transport_delete_remote_worker(
        &self,
        workflow_id: i64,
        worker: String,
        context: &C,
    ) -> Result<DeleteRemoteWorkerResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteRemoteWorkerResponse);
        self.remote_workers_api
            .delete_remote_worker(workflow_id, worker, context)
            .await
    }

    pub(super) async fn transport_delete_compute_nodes(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteComputeNodesResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteComputeNodesResponse);
        self.compute_nodes_api
            .delete_compute_nodes(workflow_id, body, context)
            .await
    }

    pub(super) async fn transport_delete_local_schedulers(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteLocalSchedulersResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteLocalSchedulersResponse);
        self.schedulers_api
            .delete_local_schedulers(workflow_id, body, context)
            .await
    }

    pub(super) async fn transport_delete_all_resource_requirements(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteAllResourceRequirementsResponse, ApiError> {
        authorize_workflow!(
            self,
            workflow_id,
            context,
            DeleteAllResourceRequirementsResponse
        );
        self.resource_requirements_api
            .delete_all_resource_requirements(workflow_id, body, context)
            .await
    }

    pub(super) async fn transport_delete_scheduled_compute_nodes(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodesResponse, ApiError> {
        authorize_workflow!(
            self,
            workflow_id,
            context,
            DeleteScheduledComputeNodesResponse
        );
        self.schedulers_api
            .delete_scheduled_compute_nodes(workflow_id, body, context)
            .await
    }

    pub(super) async fn transport_delete_slurm_schedulers(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteSlurmSchedulersResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteSlurmSchedulersResponse);
        self.schedulers_api
            .delete_slurm_schedulers(workflow_id, body, context)
            .await
    }

    pub(super) async fn transport_list_compute_nodes(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        hostname: Option<String>,
        is_active: Option<bool>,
        scheduled_compute_node_id: Option<i64>,
        context: &C,
    ) -> Result<ListComputeNodesResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, ListComputeNodesResponse);
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.compute_nodes_api
            .list_compute_nodes(
                workflow_id,
                offset,
                limit,
                sort_by,
                reverse_sort,
                hostname,
                is_active,
                scheduled_compute_node_id,
                context,
            )
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

    pub(super) async fn transport_list_resource_requirements(
        &self,
        workflow_id: i64,
        job_id: Option<i64>,
        name: Option<String>,
        memory: Option<String>,
        num_cpus: Option<i64>,
        num_gpus: Option<i64>,
        num_nodes: Option<i64>,
        runtime: Option<i64>,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListResourceRequirementsResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, ListResourceRequirementsResponse);
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.resource_requirements_api
            .list_resource_requirements(
                workflow_id,
                job_id,
                name,
                memory,
                num_cpus,
                num_gpus,
                num_nodes,
                runtime,
                offset,
                limit,
                sort_by,
                reverse_sort,
                context,
            )
            .await
    }

    pub(super) async fn transport_list_scheduled_compute_nodes(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        scheduler_id: Option<String>,
        scheduler_config_id: Option<String>,
        status: Option<String>,
        context: &C,
    ) -> Result<ListScheduledComputeNodesResponse, ApiError> {
        authorize_workflow!(
            self,
            workflow_id,
            context,
            ListScheduledComputeNodesResponse
        );
        let (offset, limit) = process_pagination_params(offset, limit)?;
        self.schedulers_api
            .list_scheduled_compute_nodes(
                workflow_id,
                offset,
                limit,
                sort_by,
                reverse_sort,
                scheduler_id,
                scheduler_config_id,
                status,
                context,
            )
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

    pub(super) async fn transport_get_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetComputeNodeResponse, ApiError> {
        authorize_resource!(self, id, "compute_node", context, GetComputeNodeResponse);
        self.compute_nodes_api.get_compute_node(id, context).await
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

    pub(super) async fn transport_get_ready_job_requirements(
        &self,
        id: i64,
        scheduler_config_id: Option<i64>,
        context: &C,
    ) -> Result<GetReadyJobRequirementsResponse, ApiError> {
        debug!(
            "get_ready_job_requirements({}, {:?}) - X-Span-ID: {:?}",
            id,
            scheduler_config_id,
            Has::<XSpanIdString>::get(context).0.clone()
        );
        authorize_workflow!(self, id, context, GetReadyJobRequirementsResponse);
        error!("get_ready_job_requirements operation is not implemented");
        Err(ApiError("Api-Error: Operation is NOT implemented".into()))
    }

    pub(super) async fn transport_get_resource_requirements(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetResourceRequirementsResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "resource_requirements",
            context,
            GetResourceRequirementsResponse
        );
        self.resource_requirements_api
            .get_resource_requirements(id, context)
            .await
    }

    pub(super) async fn transport_get_scheduled_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetScheduledComputeNodeResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "scheduled_compute_node",
            context,
            GetScheduledComputeNodeResponse
        );
        self.schedulers_api
            .get_scheduled_compute_node(id, context)
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

    pub(super) async fn transport_update_compute_node(
        &self,
        id: i64,
        body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<UpdateComputeNodeResponse, ApiError> {
        authorize_resource!(self, id, "compute_node", context, UpdateComputeNodeResponse);
        let result = self
            .compute_nodes_api
            .update_compute_node(id, body.clone(), context)
            .await?;
        if let UpdateComputeNodeResponse::SuccessfulResponse(ref _updated) = result
            && body.is_active == Some(false)
        {
            self.event_broadcaster.broadcast(BroadcastEvent {
                workflow_id: body.workflow_id,
                timestamp: chrono::Utc::now().timestamp_millis(),
                event_type: "compute_node_stopped".to_string(),
                severity: models::EventSeverity::Info,
                data: serde_json::json!({
                    "compute_node_id": id,
                    "hostname": body.hostname,
                    "pid": body.pid,
                    "duration_seconds": body.duration_seconds,
                    "compute_node_type": body.compute_node_type,
                }),
            });
        }
        Ok(result)
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

    pub(super) async fn transport_update_resource_requirements(
        &self,
        id: i64,
        body: models::ResourceRequirementsModel,
        context: &C,
    ) -> Result<UpdateResourceRequirementsResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "resource_requirements",
            context,
            UpdateResourceRequirementsResponse
        );

        let result = self
            .resource_requirements_api
            .update_resource_requirements(id, body, context)
            .await?;

        if let UpdateResourceRequirementsResponse::SuccessfulResponse(ref rr) = result {
            let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
            let username = auth
                .map(|a| a.subject)
                .unwrap_or_else(|| "unknown".to_string());

            let event = models::EventModel::new(
                rr.workflow_id,
                serde_json::json!({
                    "category": "user_action",
                    "action": "update_resource_requirements",
                    "user": username,
                    "resource_requirements_id": id,
                    "name": rr.name,
                    "num_cpus": rr.num_cpus,
                    "num_gpus": rr.num_gpus,
                    "num_nodes": rr.num_nodes,
                    "memory": rr.memory,
                    "runtime": rr.runtime,
                }),
            );
            if let Err(e) = self.events_api.create_event(event, context).await {
                error!(
                    "Failed to create event for update_resource_requirements: {:?}",
                    e
                );
            }
        }

        Ok(result)
    }

    pub(super) async fn transport_update_scheduled_compute_node(
        &self,
        id: i64,
        body: models::ScheduledComputeNodesModel,
        context: &C,
    ) -> Result<UpdateScheduledComputeNodeResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "scheduled_compute_node",
            context,
            UpdateScheduledComputeNodeResponse
        );
        self.schedulers_api
            .update_scheduled_compute_node(id, body, context)
            .await
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

    pub(super) async fn transport_delete_compute_node(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteComputeNodeResponse, ApiError> {
        authorize_resource!(self, id, "compute_node", context, DeleteComputeNodeResponse);
        self.compute_nodes_api
            .delete_compute_node(id, body, context)
            .await
    }

    pub(super) async fn transport_delete_local_scheduler(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
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
            .delete_local_scheduler(id, body, context)
            .await
    }

    pub(super) async fn transport_delete_resource_requirements(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteResourceRequirementsResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "resource_requirements",
            context,
            DeleteResourceRequirementsResponse
        );
        self.resource_requirements_api
            .delete_resource_requirements(id, body, context)
            .await
    }

    pub(super) async fn transport_delete_scheduled_compute_node(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodeResponse, ApiError> {
        authorize_resource!(
            self,
            id,
            "scheduled_compute_node",
            context,
            DeleteScheduledComputeNodeResponse
        );
        self.schedulers_api
            .delete_scheduled_compute_node(id, body, context)
            .await
    }

    pub(super) async fn transport_delete_slurm_scheduler(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
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
            .delete_slurm_scheduler(id, body, context)
            .await
    }

    pub(super) async fn transport_claim_jobs_based_on_resources(
        &self,
        id: i64,
        body: models::ComputeNodesResources,
        limit: i64,
        sort_method: Option<models::ClaimJobsSortMethod>,
        strict_scheduler_match: Option<bool>,
        context: &C,
    ) -> Result<ClaimJobsBasedOnResources, ApiError> {
        debug!(
            "claim_jobs_based_on_resources({}, {:?}, {:?}, {:?}, strict_scheduler_match={:?}) - X-Span-ID: {:?}",
            id,
            body,
            sort_method,
            limit,
            strict_scheduler_match,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        authorize_workflow!(self, id, context, ClaimJobsBasedOnResources);

        let status = match self.get_workflow_status(id, context).await {
            Ok(GetWorkflowStatusResponse::SuccessfulResponse(status)) => status,
            Ok(_) => {
                error!(
                    "Unexpected response from get_workflow_status for workflow_id={}",
                    id
                );
                return Err(ApiError(
                    "Unexpected response from get_workflow_status".to_string(),
                ));
            }
            Err(e) => return Err(e),
        };

        if status.is_canceled {
            return Ok(ClaimJobsBasedOnResources::SuccessfulResponse(
                models::ClaimJobsBasedOnResources {
                    jobs: Some(vec![]),
                    reason: Some("Workflow is canceled".to_string()),
                },
            ));
        }

        self.transport_prepare_ready_jobs(
            id,
            body,
            sort_method,
            limit,
            strict_scheduler_match,
            context,
        )
        .await
    }
}
