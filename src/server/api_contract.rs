//! Server-owned API contracts implemented by the live transport layer.

use crate::models;
use crate::server::api::{
    AccessGroupsApi, ComputeNodesApi, EventsApi, FailureHandlersApi, FilesApi, JobsApi,
    RemoteWorkersApi, ResourceRequirementsApi, ResultsApi, RoCrateApi, SchedulersApi,
    SlurmStatsApi, UserDataApi, WorkflowActionsApi, WorkflowsApi,
};
use crate::server::event_broadcast::BroadcastEvent;
use crate::server::response_types::{
    access::*, artifacts::*, events::*, jobs::*, scheduling::*, system::*, workflows::*,
};
use crate::server::transport_types::context_types::ApiError;
use async_trait::async_trait;
use std::error::Error;
use std::task::{Context, Poll};
use tokio::sync::broadcast;

/// Domain contract for artifact-centric APIs.
pub trait ArtifactDomainApi<C: Send + Sync>:
    FilesApi<C> + ResultsApi<C> + RoCrateApi<C> + UserDataApi<C>
{
}

impl<T, C> ArtifactDomainApi<C> for T
where
    C: Send + Sync,
    T: FilesApi<C> + ResultsApi<C> + RoCrateApi<C> + UserDataApi<C>,
{
}

/// Domain contract for scheduler and execution-resource APIs.
pub trait SchedulingDomainApi<C: Send + Sync>:
    ComputeNodesApi<C>
    + RemoteWorkersApi<C>
    + ResourceRequirementsApi<C>
    + SchedulersApi<C>
    + SlurmStatsApi<C>
{
}

impl<T, C> SchedulingDomainApi<C> for T
where
    C: Send + Sync,
    T: ComputeNodesApi<C>
        + RemoteWorkersApi<C>
        + ResourceRequirementsApi<C>
        + SchedulersApi<C>
        + SlurmStatsApi<C>,
{
}

/// Domain contract for workflow and access-control APIs.
pub trait WorkflowDomainApi<C: Send + Sync>:
    AccessGroupsApi<C> + WorkflowActionsApi<C> + WorkflowsApi<C>
{
}

impl<T, C> WorkflowDomainApi<C> for T
where
    C: Send + Sync,
    T: AccessGroupsApi<C> + WorkflowActionsApi<C> + WorkflowsApi<C>,
{
}

/// Domain contract for jobs and workflow execution state changes.
pub trait JobDomainApi<C: Send + Sync>: JobsApi<C> {}

impl<T, C> JobDomainApi<C> for T
where
    C: Send + Sync,
    T: JobsApi<C>,
{
}

/// Domain contract for event and failure-handler APIs.
pub trait EventDomainApi<C: Send + Sync>: EventsApi<C> + FailureHandlersApi<C> {}

impl<T, C> EventDomainApi<C> for T
where
    C: Send + Sync,
    T: EventsApi<C> + FailureHandlersApi<C>,
{
}

/// Small shared surface for service-level behavior that is not tied to one resource family.
#[async_trait]
pub trait SystemApi<C: Send + Sync> {
    fn poll_ready(
        &self,
        _cx: &mut Context,
    ) -> Poll<Result<(), Box<dyn Error + Send + Sync + 'static>>> {
        Poll::Ready(Ok(()))
    }

    /// Subscribe to the event broadcast channel for SSE streaming.
    fn subscribe_to_events(&self) -> broadcast::Receiver<BroadcastEvent>;

    /// Return the version of the service.
    async fn get_version(&self, context: &C) -> Result<GetVersionResponse, ApiError>;

    /// Check if the service is running.
    async fn ping(&self, context: &C) -> Result<PingResponse, ApiError>;

    /// Reload the htpasswd file from disk (admin only).
    async fn reload_auth(&self, context: &C) -> Result<ReloadAuthResponse, ApiError>;
}

/// Internal transport trait that keeps the concrete live-server method surface in one place.
///
/// The public transport contract below is composed from domain-specific traits, but the live
/// `Server<C>` implementation is still most naturally expressed against one shared core trait.
#[async_trait]
#[allow(clippy::too_many_arguments, clippy::ptr_arg)]
pub trait TransportApiCore<C: Send + Sync> {
    fn poll_ready(
        &self,
        _cx: &mut Context,
    ) -> Poll<Result<(), Box<dyn Error + Send + Sync + 'static>>> {
        Poll::Ready(Ok(()))
    }

    /// Store a compute node.
    async fn create_compute_node(
        &self,
        body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<CreateComputeNodeResponse, ApiError>;

    /// Store an event.
    async fn create_event(
        &self,
        body: models::EventModel,
        context: &C,
    ) -> Result<CreateEventResponse, ApiError>;

    /// Store a file.
    async fn create_file(
        &self,
        body: models::FileModel,
        context: &C,
    ) -> Result<CreateFileResponse, ApiError>;

    /// Store a job.
    async fn create_job(
        &self,
        body: models::JobModel,
        context: &C,
    ) -> Result<CreateJobResponse, ApiError>;

    /// Create jobs in bulk.
    async fn create_jobs(
        &self,
        body: models::JobsModel,
        context: &C,
    ) -> Result<CreateJobsResponse, ApiError>;

    /// Store a local scheduler.
    async fn create_local_scheduler(
        &self,
        body: models::LocalSchedulerModel,
        context: &C,
    ) -> Result<CreateLocalSchedulerResponse, ApiError>;

    /// Store a failure handler.
    async fn create_failure_handler(
        &self,
        body: models::FailureHandlerModel,
        context: &C,
    ) -> Result<CreateFailureHandlerResponse, ApiError>;

    /// Retrieve a failure handler by ID.
    async fn get_failure_handler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetFailureHandlerResponse, ApiError>;

    /// Retrieve all failure handlers for one workflow.
    async fn list_failure_handlers(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListFailureHandlersResponse, ApiError>;

    /// Delete a failure handler.
    async fn delete_failure_handler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteFailureHandlerResponse, ApiError>;

    /// Store an RO-Crate entity.
    async fn create_ro_crate_entity(
        &self,
        body: models::RoCrateEntityModel,
        context: &C,
    ) -> Result<CreateRoCrateEntityResponse, ApiError>;

    /// Retrieve an RO-Crate entity by ID.
    async fn get_ro_crate_entity(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetRoCrateEntityResponse, ApiError>;

    /// Retrieve all RO-Crate entities for one workflow.
    async fn list_ro_crate_entities(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListRoCrateEntitiesResponse, ApiError>;

    /// Update an RO-Crate entity.
    async fn update_ro_crate_entity(
        &self,
        id: i64,
        body: models::RoCrateEntityModel,
        context: &C,
    ) -> Result<UpdateRoCrateEntityResponse, ApiError>;

    /// Delete an RO-Crate entity.
    async fn delete_ro_crate_entity(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteRoCrateEntityResponse, ApiError>;

    /// Delete all RO-Crate entities for a workflow.
    async fn delete_ro_crate_entities(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteRoCrateEntitiesResponse, ApiError>;

    /// Store one resource requirements record.
    async fn create_resource_requirements(
        &self,
        body: models::ResourceRequirementsModel,
        context: &C,
    ) -> Result<CreateResourceRequirementsResponse, ApiError>;

    /// Store a job result.
    async fn create_result(
        &self,
        body: models::ResultModel,
        context: &C,
    ) -> Result<CreateResultResponse, ApiError>;

    /// Store a scheduled compute node.
    async fn create_scheduled_compute_node(
        &self,
        body: models::ScheduledComputeNodesModel,
        context: &C,
    ) -> Result<CreateScheduledComputeNodeResponse, ApiError>;

    /// Store a Slurm compute node configuration.
    async fn create_slurm_scheduler(
        &self,
        body: models::SlurmSchedulerModel,
        context: &C,
    ) -> Result<CreateSlurmSchedulerResponse, ApiError>;

    /// Store Slurm accounting stats for a job step.
    async fn create_slurm_stats(
        &self,
        body: models::SlurmStatsModel,
        context: &C,
    ) -> Result<CreateSlurmStatsResponse, ApiError>;

    /// List Slurm accounting stats.
    async fn list_slurm_stats(
        &self,
        workflow_id: i64,
        job_id: Option<i64>,
        run_id: Option<i64>,
        attempt_id: Option<i64>,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListSlurmStatsResponse, ApiError>;

    /// Store remote workers for a workflow.
    async fn create_remote_workers(
        &self,
        workflow_id: i64,
        workers: Vec<String>,
        context: &C,
    ) -> Result<CreateRemoteWorkersResponse, ApiError>;

    /// List remote workers for a workflow.
    async fn list_remote_workers(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<ListRemoteWorkersResponse, ApiError>;

    /// Delete a remote worker from a workflow.
    async fn delete_remote_worker(
        &self,
        workflow_id: i64,
        worker: String,
        context: &C,
    ) -> Result<DeleteRemoteWorkerResponse, ApiError>;

    /// Store a user data record.
    async fn create_user_data(
        &self,
        body: models::UserDataModel,
        consumer_job_id: Option<i64>,
        producer_job_id: Option<i64>,
        context: &C,
    ) -> Result<CreateUserDataResponse, ApiError>;

    /// Store a workflow.
    async fn create_workflow(
        &self,
        body: models::WorkflowModel,
        context: &C,
    ) -> Result<CreateWorkflowResponse, ApiError>;

    /// Create a workflow action.
    async fn create_workflow_action(
        &self,
        workflow_id: i64,
        body: models::WorkflowActionModel,
        context: &C,
    ) -> Result<CreateWorkflowActionResponse, ApiError>;

    /// Get all workflow actions for a workflow.
    async fn get_workflow_actions(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<GetWorkflowActionsResponse, ApiError>;

    /// Get pending (unexecuted) workflow actions for a workflow.
    async fn get_pending_actions(
        &self,
        workflow_id: i64,
        trigger_types: Option<Vec<String>>,
        context: &C,
    ) -> Result<GetPendingActionsResponse, ApiError>;

    /// Atomically claim a workflow action for execution.
    async fn claim_action(
        &self,
        workflow_id: i64,
        action_id: i64,
        body: models::ClaimActionRequest,
        context: &C,
    ) -> Result<ClaimActionResponse, ApiError>;

    /// Delete all compute node records for one workflow.
    async fn delete_compute_nodes(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteComputeNodesResponse, ApiError>;

    /// Delete all events for one workflow.
    async fn delete_events(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteEventsResponse, ApiError>;

    /// Delete all files for one workflow.
    async fn delete_files(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteFilesResponse, ApiError>;

    /// Delete all jobs for one workflow.
    async fn delete_jobs(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteJobsResponse, ApiError>;

    /// Delete all local schedulers for one workflow.
    async fn delete_local_schedulers(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteLocalSchedulersResponse, ApiError>;

    /// Delete all resource requirements records for one workflow.
    async fn delete_all_resource_requirements(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteAllResourceRequirementsResponse, ApiError>;

    /// Delete all job results for one workflow.
    async fn delete_results(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteResultsResponse, ApiError>;

    /// Delete all scheduled compute node records for one workflow.
    async fn delete_scheduled_compute_nodes(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodesResponse, ApiError>;

    /// Retrieve all Slurm compute node configurations for one workflow.
    async fn delete_slurm_schedulers(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteSlurmSchedulersResponse, ApiError>;

    /// Delete all user data records for one workflow.
    async fn delete_all_user_data(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteAllUserDataResponse, ApiError>;

    /// Return the version of the service.
    async fn get_version(&self, context: &C) -> Result<GetVersionResponse, ApiError>;

    /// Retrieve all compute node records for one workflow.
    async fn list_compute_nodes(
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
    ) -> Result<ListComputeNodesResponse, ApiError>;

    /// Retrieve all events for one workflow.
    async fn list_events(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        category: Option<String>,
        after_timestamp: Option<i64>,
        context: &C,
    ) -> Result<ListEventsResponse, ApiError>;

    /// Retrieve all files for one workflow.
    async fn list_files(
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
    ) -> Result<ListFilesResponse, ApiError>;

    /// Retrieve all jobs for one workflow.
    async fn list_jobs(
        &self,
        workflow_id: i64,
        status: Option<models::JobStatus>,
        needs_file_id: Option<i64>,
        upstream_job_id: Option<i64>,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        include_relationships: Option<bool>,
        active_compute_node_id: Option<i64>,
        context: &C,
    ) -> Result<ListJobsResponse, ApiError>;

    /// Retrieve all job dependencies for one workflow.
    async fn list_job_dependencies(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListJobDependenciesResponse, ApiError>;

    /// Retrieve job-file relationships for one workflow.
    async fn list_job_file_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListJobFileRelationshipsResponse, ApiError>;

    /// Retrieve job-user_data relationships for one workflow.
    async fn list_job_user_data_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListJobUserDataRelationshipsResponse, ApiError>;

    /// Retrieve local schedulers for one workflow.
    async fn list_local_schedulers(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        memory: Option<String>,
        num_cpus: Option<i64>,
        context: &C,
    ) -> Result<ListLocalSchedulersResponse, ApiError>;

    /// Retrieve all resource requirements records for one workflow.
    async fn list_resource_requirements(
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
    ) -> Result<ListResourceRequirementsResponse, ApiError>;

    /// Retrieve all job results for one workflow.
    async fn list_results(
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
    ) -> Result<ListResultsResponse, ApiError>;

    /// Retrieve scheduled compute node records for one workflow.
    async fn list_scheduled_compute_nodes(
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
    ) -> Result<ListScheduledComputeNodesResponse, ApiError>;

    /// Retrieve a Slurm compute node configuration.
    async fn list_slurm_schedulers(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        name: Option<String>,
        account: Option<String>,
        gres: Option<String>,
        mem: Option<String>,
        nodes: Option<i64>,
        partition: Option<String>,
        qos: Option<String>,
        tmp: Option<String>,
        walltime: Option<String>,
        context: &C,
    ) -> Result<ListSlurmSchedulersResponse, ApiError>;

    /// Retrieve all user data records for one workflow.
    async fn list_user_data(
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
    ) -> Result<ListUserDataResponse, ApiError>;

    /// Retrieve all workflows.
    async fn list_workflows(
        &self,
        offset: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        limit: Option<i64>,
        name: Option<String>,
        user: Option<String>,
        description: Option<String>,
        is_archived: Option<bool>,
        context: &C,
    ) -> Result<ListWorkflowsResponse, ApiError>;

    /// Check if the service is running.
    async fn ping(&self, context: &C) -> Result<PingResponse, ApiError>;

    /// Cancel a workflow. Workers will detect the status change and cancel jobs.
    async fn cancel_workflow(
        &self,
        id: i64,
        context: &C,
    ) -> Result<CancelWorkflowResponse, ApiError>;

    /// Retrieve a compute node by ID.
    async fn get_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetComputeNodeResponse, ApiError>;

    /// Retrieve an event by ID.
    async fn get_event(&self, id: i64, context: &C) -> Result<GetEventResponse, ApiError>;

    /// Retrieve a file.
    async fn get_file(&self, id: i64, context: &C) -> Result<GetFileResponse, ApiError>;

    /// Retrieve a job.
    async fn get_job(&self, id: i64, context: &C) -> Result<GetJobResponse, ApiError>;

    /// Retrieve a local scheduler.
    async fn get_local_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetLocalSchedulerResponse, ApiError>;

    /// Return the resource requirements for jobs with a status of ready.
    async fn get_ready_job_requirements(
        &self,
        id: i64,
        scheduler_config_id: Option<i64>,
        context: &C,
    ) -> Result<GetReadyJobRequirementsResponse, ApiError>;

    /// Retrieve one resource requirements record.
    async fn get_resource_requirements(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetResourceRequirementsResponse, ApiError>;

    /// Retrieve a job result.
    async fn get_result(&self, id: i64, context: &C) -> Result<GetResultResponse, ApiError>;

    /// Retrieve a scheduled compute node.
    async fn get_scheduled_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetScheduledComputeNodeResponse, ApiError>;

    /// Retrieve a Slurm compute node configuration.
    async fn get_slurm_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetSlurmSchedulerResponse, ApiError>;

    /// Retrieve a user data record.
    async fn get_user_data(&self, id: i64, context: &C) -> Result<GetUserDataResponse, ApiError>;

    /// Retrieve a workflow.
    async fn get_workflow(&self, id: i64, context: &C) -> Result<GetWorkflowResponse, ApiError>;

    /// Return the workflow status.
    async fn get_workflow_status(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetWorkflowStatusResponse, ApiError>;

    /// Initialize job relationships based on file and user_data relationships.
    async fn initialize_jobs(
        &self,
        id: i64,
        only_uninitialized: Option<bool>,
        clear_ephemeral_user_data: Option<bool>,
        context: &C,
    ) -> Result<InitializeJobsResponse, ApiError>;

    /// Return true if all jobs in the workflow are complete.
    async fn is_workflow_complete(
        &self,
        id: i64,
        context: &C,
    ) -> Result<IsWorkflowCompleteResponse, ApiError>;

    /// Return true if all jobs in the workflow are uninitialized or disabled.
    async fn is_workflow_uninitialized(
        &self,
        id: i64,
        context: &C,
    ) -> Result<IsWorkflowUninitializedResponse, ApiError>;

    /// Retrieve all job IDs for one workflow.
    async fn list_job_ids(&self, id: i64, context: &C) -> Result<ListJobIdsResponse, ApiError>;

    /// List missing user data that should exist.
    async fn list_missing_user_data(
        &self,
        id: i64,
        context: &C,
    ) -> Result<ListMissingUserDataResponse, ApiError>;

    /// List files that must exist.
    async fn list_required_existing_files(
        &self,
        id: i64,
        context: &C,
    ) -> Result<ListRequiredExistingFilesResponse, ApiError>;

    /// Update a compute node.
    async fn update_compute_node(
        &self,
        id: i64,
        body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<UpdateComputeNodeResponse, ApiError>;

    /// Update an event.
    async fn update_event(
        &self,
        id: i64,
        body: serde_json::Value,
        context: &C,
    ) -> Result<UpdateEventResponse, ApiError>;

    /// Update a file.
    async fn update_file(
        &self,
        id: i64,
        body: models::FileModel,
        context: &C,
    ) -> Result<UpdateFileResponse, ApiError>;

    /// Update a job.
    async fn update_job(
        &self,
        id: i64,
        body: models::JobModel,
        context: &C,
    ) -> Result<UpdateJobResponse, ApiError>;

    /// Update a local scheduler.
    async fn update_local_scheduler(
        &self,
        id: i64,
        body: models::LocalSchedulerModel,
        context: &C,
    ) -> Result<UpdateLocalSchedulerResponse, ApiError>;

    /// Update one resource requirements record.
    async fn update_resource_requirements(
        &self,
        id: i64,
        body: models::ResourceRequirementsModel,
        context: &C,
    ) -> Result<UpdateResourceRequirementsResponse, ApiError>;

    /// Update a job result.
    async fn update_result(
        &self,
        id: i64,
        body: models::ResultModel,
        context: &C,
    ) -> Result<UpdateResultResponse, ApiError>;

    /// Update a scheduled compute node.
    async fn update_scheduled_compute_node(
        &self,
        id: i64,
        body: models::ScheduledComputeNodesModel,
        context: &C,
    ) -> Result<UpdateScheduledComputeNodeResponse, ApiError>;

    /// Update a Slurm compute node configuration.
    async fn update_slurm_scheduler(
        &self,
        id: i64,
        body: models::SlurmSchedulerModel,
        context: &C,
    ) -> Result<UpdateSlurmSchedulerResponse, ApiError>;

    /// Update a user data record.
    async fn update_user_data(
        &self,
        id: i64,
        body: models::UserDataModel,
        context: &C,
    ) -> Result<UpdateUserDataResponse, ApiError>;

    /// Update a workflow.
    async fn update_workflow(
        &self,
        id: i64,
        body: models::WorkflowModel,
        context: &C,
    ) -> Result<UpdateWorkflowResponse, ApiError>;

    /// Update the workflow status.
    async fn update_workflow_status(
        &self,
        id: i64,
        body: models::WorkflowStatusModel,
        context: &C,
    ) -> Result<UpdateWorkflowStatusResponse, ApiError>;

    /// Return jobs that are ready for submission and meet worker resource. Set status to pending.
    async fn claim_jobs_based_on_resources(
        &self,
        id: i64,
        body: models::ComputeNodesResources,
        limit: i64,
        strict_scheduler_match: Option<bool>,
        context: &C,
    ) -> Result<ClaimJobsBasedOnResources, ApiError>;

    /// Return user-requested number of jobs that are ready for submission. Sets status to pending.
    async fn claim_next_jobs(
        &self,
        id: i64,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ClaimNextJobsResponse, ApiError>;

    /// Check for changed job inputs and update status accordingly.
    async fn process_changed_job_inputs(
        &self,
        id: i64,
        dry_run: Option<bool>,
        context: &C,
    ) -> Result<ProcessChangedJobInputsResponse, ApiError>;

    /// Delete a compute node.
    async fn delete_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteComputeNodeResponse, ApiError>;

    /// Delete an event.
    async fn delete_event(&self, id: i64, context: &C) -> Result<DeleteEventResponse, ApiError>;

    /// Delete a file.
    async fn delete_file(&self, id: i64, context: &C) -> Result<DeleteFileResponse, ApiError>;

    /// Delete a job.
    async fn delete_job(&self, id: i64, context: &C) -> Result<DeleteJobResponse, ApiError>;

    /// Delete a local scheduler.
    async fn delete_local_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteLocalSchedulerResponse, ApiError>;

    /// Delete a resource requirements record.
    async fn delete_resource_requirements(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteResourceRequirementsResponse, ApiError>;

    /// Delete a job result.
    async fn delete_result(&self, id: i64, context: &C) -> Result<DeleteResultResponse, ApiError>;

    /// Delete a scheduled compute node.
    async fn delete_scheduled_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodeResponse, ApiError>;

    /// Delete Slurm compute node configuration.
    async fn delete_slurm_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteSlurmSchedulerResponse, ApiError>;

    /// Delete a user data record.
    async fn delete_user_data(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteUserDataResponse, ApiError>;

    /// Delete a workflow.
    async fn delete_workflow(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteWorkflowResponse, ApiError>;

    /// Reset status for jobs to uninitialized.
    async fn reset_job_status(
        &self,
        id: i64,
        failed_only: Option<bool>,
        context: &C,
    ) -> Result<ResetJobStatusResponse, ApiError>;

    /// Reset worklow status.
    async fn reset_workflow_status(
        &self,
        id: i64,
        force: Option<bool>,
        context: &C,
    ) -> Result<ResetWorkflowStatusResponse, ApiError>;

    /// Change the status of a job and manage side effects.
    async fn manage_status_change(
        &self,
        id: i64,
        status: models::JobStatus,
        run_id: i64,
        context: &C,
    ) -> Result<ManageStatusChangeResponse, ApiError>;

    /// Start a job and manage side effects.
    async fn start_job(
        &self,
        id: i64,
        run_id: i64,
        compute_node_id: i64,
        context: &C,
    ) -> Result<StartJobResponse, ApiError>;

    /// Complete a job, connect it to a result, and manage side effects.
    async fn complete_job(
        &self,
        id: i64,
        status: models::JobStatus,
        run_id: i64,
        body: models::ResultModel,
        context: &C,
    ) -> Result<CompleteJobResponse, ApiError>;

    /// Retry a failed job by resetting it to ready status and incrementing attempt_id.
    async fn retry_job(
        &self,
        id: i64,
        run_id: i64,
        max_retries: i32,
        context: &C,
    ) -> Result<RetryJobResponse, ApiError>;

    /// Get ready jobs that fit within the specified resource constraints.
    async fn prepare_ready_jobs(
        &self,
        workflow_id: i64,
        resources: models::ComputeNodesResources,
        limit: i64,
        strict_scheduler_match: Option<bool>,
        context: &C,
    ) -> Result<ClaimJobsBasedOnResources, ApiError>;

    // Access Groups API

    /// Create an access group.
    async fn create_access_group(
        &self,
        body: models::AccessGroupModel,
        context: &C,
    ) -> Result<CreateAccessGroupResponse, ApiError>;

    /// Get an access group by ID.
    async fn get_access_group(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetAccessGroupResponse, ApiError>;

    /// List all access groups.
    async fn list_access_groups(
        &self,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListAccessGroupsApiResponse, ApiError>;

    /// Delete an access group.
    async fn delete_access_group(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteAccessGroupResponse, ApiError>;

    /// Add a user to an access group.
    async fn add_user_to_group(
        &self,
        group_id: i64,
        body: models::UserGroupMembershipModel,
        context: &C,
    ) -> Result<AddUserToGroupResponse, ApiError>;

    /// Remove a user from an access group.
    async fn remove_user_from_group(
        &self,
        group_id: i64,
        user_name: String,
        context: &C,
    ) -> Result<RemoveUserFromGroupResponse, ApiError>;

    /// List members of an access group.
    async fn list_group_members(
        &self,
        group_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListGroupMembersResponse, ApiError>;

    /// List groups a user belongs to.
    async fn list_user_groups(
        &self,
        user_name: String,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListUserGroupsApiResponse, ApiError>;

    /// Add a workflow to an access group.
    async fn add_workflow_to_group(
        &self,
        workflow_id: i64,
        group_id: i64,
        context: &C,
    ) -> Result<AddWorkflowToGroupResponse, ApiError>;

    /// Remove a workflow from an access group.
    async fn remove_workflow_from_group(
        &self,
        workflow_id: i64,
        group_id: i64,
        context: &C,
    ) -> Result<RemoveWorkflowFromGroupResponse, ApiError>;

    /// List access groups for a workflow.
    async fn list_workflow_groups(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListWorkflowGroupsResponse, ApiError>;

    /// Check if a user can access a workflow.
    async fn check_workflow_access(
        &self,
        workflow_id: i64,
        user_name: String,
        context: &C,
    ) -> Result<CheckWorkflowAccessResponse, ApiError>;

    /// Subscribe to the event broadcast channel for SSE streaming.
    /// Returns a broadcast receiver that will receive all future events.
    fn subscribe_to_events(&self) -> broadcast::Receiver<BroadcastEvent>;

    /// Reload the htpasswd file from disk (admin only).
    async fn reload_auth(&self, context: &C) -> Result<ReloadAuthResponse, ApiError>;
}

/// Transport contract for artifact-related HTTP endpoints.
pub trait ArtifactTransportApi<C: Send + Sync>: TransportApiCore<C> {}
impl<T, C> ArtifactTransportApi<C> for T
where
    C: Send + Sync,
    T: TransportApiCore<C>,
{
}

/// Transport contract for scheduler and compute-resource HTTP endpoints.
pub trait SchedulingTransportApi<C: Send + Sync>: TransportApiCore<C> {}
impl<T, C> SchedulingTransportApi<C> for T
where
    C: Send + Sync,
    T: TransportApiCore<C>,
{
}

/// Transport contract for workflow and workflow-action HTTP endpoints.
pub trait WorkflowTransportApi<C: Send + Sync>: TransportApiCore<C> {}
impl<T, C> WorkflowTransportApi<C> for T
where
    C: Send + Sync,
    T: TransportApiCore<C>,
{
}

/// Transport contract for job lifecycle and claiming HTTP endpoints.
pub trait JobTransportApi<C: Send + Sync>: TransportApiCore<C> {}
impl<T, C> JobTransportApi<C> for T
where
    C: Send + Sync,
    T: TransportApiCore<C>,
{
}

/// Transport contract for event and failure-handler HTTP endpoints.
pub trait EventTransportApi<C: Send + Sync>: TransportApiCore<C> {}
impl<T, C> EventTransportApi<C> for T
where
    C: Send + Sync,
    T: TransportApiCore<C>,
{
}

/// Transport contract for access-control and authorization HTTP endpoints.
pub trait AccessTransportApi<C: Send + Sync>: TransportApiCore<C> {}
impl<T, C> AccessTransportApi<C> for T
where
    C: Send + Sync,
    T: TransportApiCore<C>,
{
}

/// Public transport contract used by the HTTP layer.
pub trait TransportApi<C: Send + Sync>:
    TransportApiCore<C>
    + ArtifactTransportApi<C>
    + SchedulingTransportApi<C>
    + WorkflowTransportApi<C>
    + JobTransportApi<C>
    + EventTransportApi<C>
    + AccessTransportApi<C>
{
}

impl<T, C> TransportApi<C> for T
where
    C: Send + Sync,
    T: TransportApiCore<C>
        + ArtifactTransportApi<C>
        + SchedulingTransportApi<C>
        + WorkflowTransportApi<C>
        + JobTransportApi<C>
        + EventTransportApi<C>
        + AccessTransportApi<C>,
{
}

/// Composed live server contract used by higher-level transport code.
pub trait Api<C: Send + Sync>:
    TransportApi<C>
    + SystemApi<C>
    + ArtifactDomainApi<C>
    + SchedulingDomainApi<C>
    + WorkflowDomainApi<C>
    + JobDomainApi<C>
    + EventDomainApi<C>
{
}

impl<T, C> Api<C> for T
where
    C: Send + Sync,
    T: TransportApi<C>
        + SystemApi<C>
        + ArtifactDomainApi<C>
        + SchedulingDomainApi<C>
        + WorkflowDomainApi<C>
        + JobDomainApi<C>
        + EventDomainApi<C>,
{
}
