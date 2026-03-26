//! Main library entry point for torc implementation.

use crate::MAX_RECORD_TRANSFER_COUNT;
use crate::models;
use crate::server::authorization::{AccessCheckResult, AuthorizationService};
use crate::server::event_broadcast::{BroadcastEvent, EventBroadcaster};
use crate::server::htpasswd::HtpasswdFile;
use crate::server::transport_types::auth_types::Authorization;
use crate::server::transport_types::context_types::{Has, XSpanIdString};
use async_trait::async_trait;
use log::{debug, error, info, warn};
use sqlx::Row;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tracing::instrument;

use sqlx::sqlite::SqlitePool;

const TORC_VERSION: &str = env!("CARGO_PKG_VERSION");
const GIT_HASH: &str = env!("GIT_HASH");
const GIT_DIRTY: &str = env!("GIT_DIRTY");

/// Returns the full version string including git hash (e.g., "0.8.0 (abc1234)")
fn full_version() -> String {
    format!("{} ({}{})", TORC_VERSION, GIT_HASH, GIT_DIRTY)
}

macro_rules! forbidden_error {
    ($reason:expr) => {
        models::ErrorResponse::new(serde_json::json!({
            "error": "Forbidden",
            "message": $reason
        }))
    };
}

macro_rules! not_found_error {
    ($reason:expr) => {
        models::ErrorResponse::new(serde_json::json!({
            "error": "NotFound",
            "message": $reason
        }))
    };
}

macro_rules! authorize_workflow {
    ($self:ident, $workflow_id:expr, $context:expr, $response_enum:ident) => {
        match $self
            .check_workflow_access_for_context($workflow_id, $context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok($response_enum::ForbiddenErrorResponse(forbidden_error!(
                    reason
                )));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok($response_enum::NotFoundErrorResponse(not_found_error!(
                    reason
                )));
            }
            AccessCheckResult::InternalError(reason) => {
                return Err(ApiError(reason));
            }
        }
    };
}

macro_rules! authorize_resource {
    ($self:ident, $resource_id:expr, $table_name:expr, $context:expr, $response_enum:ident) => {
        match $self
            .check_resource_access_for_context($resource_id, $table_name, $context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok($response_enum::ForbiddenErrorResponse(forbidden_error!(
                    reason
                )));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok($response_enum::NotFoundErrorResponse(not_found_error!(
                    reason
                )));
            }
            AccessCheckResult::InternalError(reason) => {
                return Err(ApiError(reason));
            }
        }
    };
}

macro_rules! authorize_job {
    ($self:ident, $job_id:expr, $context:expr, $response_enum:ident) => {
        match $self.check_job_access_for_context($job_id, $context).await {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok($response_enum::ForbiddenErrorResponse(forbidden_error!(
                    reason
                )));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok($response_enum::NotFoundErrorResponse(not_found_error!(
                    reason
                )));
            }
            AccessCheckResult::InternalError(reason) => {
                return Err(ApiError(reason));
            }
        }
    };
}

macro_rules! authorize_group_admin {
    ($self:ident, $group_id:expr, $context:expr, $response_enum:ident) => {
        match $self
            .check_group_admin_access_for_context($group_id, $context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok($response_enum::ForbiddenErrorResponse(forbidden_error!(
                    reason
                )));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok($response_enum::NotFoundErrorResponse(not_found_error!(
                    reason
                )));
            }
            AccessCheckResult::InternalError(reason) => {
                return Err(ApiError(reason));
            }
        }
    };
}

macro_rules! authorize_admin {
    ($self:ident, $context:expr, $response_enum:ident) => {
        match $self.check_admin_access_for_context($context).await {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok($response_enum::ForbiddenErrorResponse(forbidden_error!(
                    reason
                )));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok($response_enum::NotFoundErrorResponse(not_found_error!(
                    reason
                )));
            }
            AccessCheckResult::InternalError(reason) => {
                return Err(ApiError(reason));
            }
        }
    };
}

macro_rules! authorize_workflow_group {
    ($self:ident, $workflow_id:expr, $group_id:expr, $context:expr, $response_enum:ident) => {
        match $self
            .check_workflow_group_access_for_context($workflow_id, $group_id, $context)
            .await
        {
            AccessCheckResult::Allowed => {}
            AccessCheckResult::Denied(reason) => {
                return Ok($response_enum::ForbiddenErrorResponse(forbidden_error!(
                    reason
                )));
            }
            AccessCheckResult::NotFound(reason) => {
                return Ok($response_enum::NotFoundErrorResponse(not_found_error!(
                    reason
                )));
            }
            AccessCheckResult::InternalError(reason) => {
                return Err(ApiError(reason));
            }
        }
    };
}

mod access_checks;
mod access_control_transport;
mod bootstrap;
mod compute_nodes_transport;
mod files_transport;
mod jobs_transport;
mod lifecycle_support;
mod local_schedulers_transport;
mod remote_workers_transport;
mod resource_requirements_transport;
mod results_transport;
mod ro_crate_transport;
mod runtime_support;
mod scheduled_compute_nodes_transport;
mod slurm_schedulers_transport;
mod slurm_stats_transport;
mod system_transport;
mod unblock_processing;
mod user_data_transport;
mod workflows_transport;

/// Process optional offset and limit parameters and return concrete values.
/// Returns (offset, limit) where:
/// - offset defaults to 0 if not provided
/// - limit defaults to [`MAX_RECORD_TRANSFER_COUNT`] if not provided
/// - Returns an error if limit exceeds [`MAX_RECORD_TRANSFER_COUNT`]
fn process_pagination_params(
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<(i64, i64), ApiError> {
    let processed_offset = offset.unwrap_or(0);
    let processed_limit = limit.unwrap_or(MAX_RECORD_TRANSFER_COUNT);

    if processed_limit > MAX_RECORD_TRANSFER_COUNT {
        error!(
            "Limit exceeds maximum allowed value: {} > {}",
            processed_limit, MAX_RECORD_TRANSFER_COUNT
        );
        return Err(ApiError(format!(
            "Limit cannot exceed {}",
            MAX_RECORD_TRANSFER_COUNT
        )));
    }

    Ok((processed_offset, processed_limit))
}

/// Sync the admin group with configured admin users
///
/// Creates the "admin" system group if it doesn't exist and ensures
/// all configured admin users are members with admin role.
/// Creates and starts the HTTP(S) server.
///
/// When `https` is true, `tls_cert` and `tls_key` must provide paths to the
/// TLS certificate chain and private key files (PEM format).
///
/// Returns the actual port the server bound to (useful when port 0 is specified for auto-detection).
#[allow(clippy::too_many_arguments)]
pub async fn create(
    addr: &str,
    https: bool,
    pool: SqlitePool,
    htpasswd: Option<HtpasswdFile>,
    require_auth: bool,
    credential_cache_ttl_secs: u64,
    enforce_access_control: bool,
    completion_check_interval_secs: f64,
    admin_users: Vec<String>,
    #[allow(unused_variables)] tls_cert: Option<String>,
    #[allow(unused_variables)] tls_key: Option<String>,
    auth_file_path: Option<String>,
) -> u16 {
    bootstrap::create_server(
        addr,
        https,
        pool,
        htpasswd,
        require_auth,
        credential_cache_ttl_secs,
        enforce_access_control,
        completion_check_interval_secs,
        admin_users,
        tls_cert,
        tls_key,
        auth_file_path,
    )
    .await
}

pub struct Server<C> {
    marker: PhantomData<C>,
    shared: Arc<LiveServerState>,
}

impl<C> Clone for Server<C> {
    fn clone(&self) -> Self {
        Self {
            marker: PhantomData,
            shared: self.shared.clone(),
        }
    }
}

impl<C> Deref for Server<C> {
    type Target = LiveServerState;

    fn deref(&self) -> &Self::Target {
        self.shared.as_ref()
    }
}

impl<C> Server<C> {
    pub fn new(
        pool: SqlitePool,
        enforce_access_control: bool,
        htpasswd: crate::server::auth::SharedHtpasswd,
        auth_file_path: Option<String>,
        credential_cache: crate::server::auth::SharedCredentialCache,
    ) -> Self {
        Server {
            marker: PhantomData,
            shared: Arc::new(LiveServerState::new(
                pool,
                enforce_access_control,
                htpasswd,
                auth_file_path,
                credential_cache,
            )),
        }
    }

    /// Signal that a job has completed. This wakes up the background unblock task.
    fn signal_job_completion(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(1);
        self.last_completion_time.store(now, Ordering::Release);
    }

    /// Get a reference to the event broadcaster for SSE subscriptions.
    pub fn get_event_broadcaster(&self) -> &EventBroadcaster {
        &self.event_broadcaster
    }

    pub fn shared_state(&self) -> Arc<LiveServerState> {
        self.shared.clone()
    }

    #[cfg(feature = "openapi-codegen")]
    pub fn openapi_app_state(&self) -> crate::openapi_spec::OpenApiAppState {
        self.shared.openapi_app_state(
            full_version(),
            API_VERSION.to_string(),
            GIT_HASH.to_string(),
        )
    }
}

use crate::server::api_constants::API_VERSION;
use crate::server::api_contract::TransportApiCore;
use crate::server::response_types::{
    access::*, artifacts::*, events::*, jobs::*, scheduling::*, system::*, workflows::*,
};
use crate::server::transport_types::context_types::ApiError;
use crate::time_utils::duration_string_to_seconds;
use std::ops::Deref;

use crate::server::live_state::LiveServerState;

#[async_trait]
impl<C> TransportApiCore<C> for Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync,
{
    /// Store a compute node.
    async fn create_compute_node(
        &self,
        body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<CreateComputeNodeResponse, ApiError> {
        self.transport_create_compute_node(body, context).await
    }

    /// Store an event.
    async fn create_event(
        &self,
        body: models::EventModel,
        context: &C,
    ) -> Result<CreateEventResponse, ApiError> {
        self.transport_create_event(body, context).await
    }

    /// Store a file.
    async fn create_file(
        &self,
        file: models::FileModel,
        context: &C,
    ) -> Result<CreateFileResponse, ApiError> {
        self.transport_create_file(file, context).await
    }

    /// Store a job.
    async fn create_job(
        &self,
        job: models::JobModel,
        context: &C,
    ) -> Result<CreateJobResponse, ApiError> {
        self.transport_create_job(job, context).await
    }

    /// Create jobs in bulk.
    async fn create_jobs(
        &self,
        body: models::JobsModel,
        context: &C,
    ) -> Result<CreateJobsResponse, ApiError> {
        self.transport_create_jobs(body, context).await
    }

    /// Store a local scheduler.
    async fn create_local_scheduler(
        &self,
        body: models::LocalSchedulerModel,
        context: &C,
    ) -> Result<CreateLocalSchedulerResponse, ApiError> {
        self.transport_create_local_scheduler(body, context).await
    }

    /// Store a failure handler.
    async fn create_failure_handler(
        &self,
        body: models::FailureHandlerModel,
        context: &C,
    ) -> Result<CreateFailureHandlerResponse, ApiError> {
        self.transport_create_failure_handler(body, context).await
    }

    /// Retrieve a failure handler by ID.
    async fn get_failure_handler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetFailureHandlerResponse, ApiError> {
        self.transport_get_failure_handler(id, context).await
    }

    /// Retrieve all failure handlers for one workflow.
    async fn list_failure_handlers(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListFailureHandlersResponse, ApiError> {
        self.transport_list_failure_handlers(workflow_id, offset, limit, context)
            .await
    }

    /// Delete a failure handler.
    async fn delete_failure_handler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteFailureHandlerResponse, ApiError> {
        self.transport_delete_failure_handler(id, context).await
    }

    /// Store an RO-Crate entity.
    async fn create_ro_crate_entity(
        &self,
        body: models::RoCrateEntityModel,
        context: &C,
    ) -> Result<CreateRoCrateEntityResponse, ApiError> {
        self.transport_create_ro_crate_entity(body, context).await
    }

    /// Retrieve an RO-Crate entity by ID.
    async fn get_ro_crate_entity(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetRoCrateEntityResponse, ApiError> {
        self.transport_get_ro_crate_entity(id, context).await
    }

    /// Retrieve all RO-Crate entities for one workflow.
    async fn list_ro_crate_entities(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListRoCrateEntitiesResponse, ApiError> {
        self.transport_list_ro_crate_entities(
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            context,
        )
        .await
    }

    /// Update an RO-Crate entity.
    async fn update_ro_crate_entity(
        &self,
        id: i64,
        body: models::RoCrateEntityModel,
        context: &C,
    ) -> Result<UpdateRoCrateEntityResponse, ApiError> {
        self.transport_update_ro_crate_entity(id, body, context)
            .await
    }

    /// Delete an RO-Crate entity.
    async fn delete_ro_crate_entity(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteRoCrateEntityResponse, ApiError> {
        self.transport_delete_ro_crate_entity(id, context).await
    }

    /// Delete all RO-Crate entities for a workflow.
    async fn delete_ro_crate_entities(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteRoCrateEntitiesResponse, ApiError> {
        self.transport_delete_ro_crate_entities(workflow_id, context)
            .await
    }

    /// Store one resource requirements record.
    async fn create_resource_requirements(
        &self,
        body: models::ResourceRequirementsModel,
        context: &C,
    ) -> Result<CreateResourceRequirementsResponse, ApiError> {
        self.transport_create_resource_requirements(body, context)
            .await
    }

    /// Store a job result.
    async fn create_result(
        &self,
        body: models::ResultModel,
        context: &C,
    ) -> Result<CreateResultResponse, ApiError> {
        self.transport_create_result(body, context).await
    }

    /// Store a scheduled compute node.
    async fn create_scheduled_compute_node(
        &self,
        body: models::ScheduledComputeNodesModel,
        context: &C,
    ) -> Result<CreateScheduledComputeNodeResponse, ApiError> {
        self.transport_create_scheduled_compute_node(body, context)
            .await
    }

    /// Store a Slurm compute node configuration.
    async fn create_slurm_scheduler(
        &self,
        body: models::SlurmSchedulerModel,
        context: &C,
    ) -> Result<CreateSlurmSchedulerResponse, ApiError> {
        self.transport_create_slurm_scheduler(body, context).await
    }

    /// Store Slurm accounting stats for a job step.
    async fn create_slurm_stats(
        &self,
        body: models::SlurmStatsModel,
        context: &C,
    ) -> Result<CreateSlurmStatsResponse, ApiError> {
        self.transport_create_slurm_stats(body, context).await
    }

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
    ) -> Result<ListSlurmStatsResponse, ApiError> {
        self.transport_list_slurm_stats(
            workflow_id,
            job_id,
            run_id,
            attempt_id,
            offset,
            limit,
            context,
        )
        .await
    }

    /// Store remote workers for a workflow.
    async fn create_remote_workers(
        &self,
        workflow_id: i64,
        workers: Vec<String>,
        context: &C,
    ) -> Result<CreateRemoteWorkersResponse, ApiError> {
        self.transport_create_remote_workers(workflow_id, workers, context)
            .await
    }

    /// List remote workers for a workflow.
    async fn list_remote_workers(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<ListRemoteWorkersResponse, ApiError> {
        self.transport_list_remote_workers(workflow_id, context)
            .await
    }

    /// Delete a remote worker from a workflow.
    async fn delete_remote_worker(
        &self,
        workflow_id: i64,
        worker: String,
        context: &C,
    ) -> Result<DeleteRemoteWorkerResponse, ApiError> {
        self.transport_delete_remote_worker(workflow_id, worker, context)
            .await
    }

    /// Store a user data record.
    async fn create_user_data(
        &self,
        body: models::UserDataModel,
        consumer_job_id: Option<i64>,
        producer_job_id: Option<i64>,
        context: &C,
    ) -> Result<CreateUserDataResponse, ApiError> {
        self.transport_create_user_data(body, consumer_job_id, producer_job_id, context)
            .await
    }

    /// Store a workflow.
    async fn create_workflow(
        &self,
        body: models::WorkflowModel,
        context: &C,
    ) -> Result<CreateWorkflowResponse, ApiError> {
        self.transport_create_workflow(body, context).await
    }

    /// Create a workflow action.
    async fn create_workflow_action(
        &self,
        workflow_id: i64,
        body: models::WorkflowActionModel,
        context: &C,
    ) -> Result<CreateWorkflowActionResponse, ApiError> {
        self.transport_create_workflow_action(workflow_id, body, context)
            .await
    }

    async fn get_workflow_actions(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<GetWorkflowActionsResponse, ApiError> {
        self.transport_get_workflow_actions(workflow_id, context)
            .await
    }

    #[instrument(level = "debug", skip(self, context), fields(workflow_id))]
    async fn get_pending_actions(
        &self,
        workflow_id: i64,
        trigger_types: Option<Vec<String>>,
        context: &C,
    ) -> Result<GetPendingActionsResponse, ApiError> {
        self.transport_get_pending_actions(workflow_id, trigger_types, context)
            .await
    }

    async fn claim_action(
        &self,
        workflow_id: i64,
        action_id: i64,
        body: models::ClaimActionRequest,
        context: &C,
    ) -> Result<ClaimActionResponse, ApiError> {
        self.transport_claim_action(workflow_id, action_id, body, context)
            .await
    }

    /// Return the version of the service.
    async fn get_version(&self, context: &C) -> Result<GetVersionResponse, ApiError> {
        self.transport_get_version(context).await
    }

    async fn reload_auth(&self, context: &C) -> Result<ReloadAuthResponse, ApiError> {
        self.transport_reload_auth(context).await
    }

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
    ) -> Result<ListWorkflowsResponse, ApiError> {
        self.transport_list_workflows(
            offset,
            sort_by,
            reverse_sort,
            limit,
            name,
            user,
            description,
            is_archived,
            context,
        )
        .await
    }

    /// Check if the service is running.
    async fn ping(&self, context: &C) -> Result<PingResponse, ApiError> {
        self.transport_ping(context).await
    }

    async fn cancel_workflow(
        &self,
        id: i64,
        context: &C,
    ) -> Result<CancelWorkflowResponse, ApiError> {
        self.transport_cancel_workflow(id, context).await
    }

    async fn delete_compute_nodes(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteComputeNodesResponse, ApiError> {
        self.transport_delete_compute_nodes(workflow_id, context)
            .await
    }

    async fn delete_events(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteEventsResponse, ApiError> {
        self.transport_delete_events(workflow_id, context).await
    }

    async fn delete_files(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteFilesResponse, ApiError> {
        self.transport_delete_files(workflow_id, context).await
    }

    async fn delete_jobs(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteJobsResponse, ApiError> {
        self.transport_delete_jobs(workflow_id, context).await
    }

    async fn delete_local_schedulers(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteLocalSchedulersResponse, ApiError> {
        self.transport_delete_local_schedulers(workflow_id, context)
            .await
    }

    async fn delete_all_resource_requirements(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteAllResourceRequirementsResponse, ApiError> {
        self.transport_delete_all_resource_requirements(workflow_id, context)
            .await
    }

    async fn delete_results(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteResultsResponse, ApiError> {
        self.transport_delete_results(workflow_id, context).await
    }

    async fn delete_scheduled_compute_nodes(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodesResponse, ApiError> {
        self.transport_delete_scheduled_compute_nodes(workflow_id, context)
            .await
    }

    async fn delete_slurm_schedulers(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteSlurmSchedulersResponse, ApiError> {
        self.transport_delete_slurm_schedulers(workflow_id, context)
            .await
    }

    async fn delete_all_user_data(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteAllUserDataResponse, ApiError> {
        self.transport_delete_all_user_data(workflow_id, context)
            .await
    }

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
    ) -> Result<ListComputeNodesResponse, ApiError> {
        self.transport_list_compute_nodes(
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
    ) -> Result<ListEventsResponse, ApiError> {
        self.transport_list_events(
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            category,
            after_timestamp,
            context,
        )
        .await
    }

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
    ) -> Result<ListFilesResponse, ApiError> {
        self.transport_list_files(
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
    ) -> Result<ListJobsResponse, ApiError> {
        self.transport_list_jobs(
            workflow_id,
            status,
            needs_file_id,
            upstream_job_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            include_relationships,
            active_compute_node_id,
            context,
        )
        .await
    }

    /// Retrieve all job dependencies for one workflow.
    async fn list_job_dependencies(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListJobDependenciesResponse, ApiError> {
        self.transport_list_job_dependencies(
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            context,
        )
        .await
    }

    async fn list_job_file_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListJobFileRelationshipsResponse, ApiError> {
        self.transport_list_job_file_relationships(
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            context,
        )
        .await
    }

    async fn list_job_user_data_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListJobUserDataRelationshipsResponse, ApiError> {
        self.transport_list_job_user_data_relationships(
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            context,
        )
        .await
    }

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
    ) -> Result<ListLocalSchedulersResponse, ApiError> {
        self.transport_list_local_schedulers(
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
    ) -> Result<ListResourceRequirementsResponse, ApiError> {
        self.transport_list_resource_requirements(
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
    ) -> Result<ListResultsResponse, ApiError> {
        self.transport_list_results(
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
    ) -> Result<ListScheduledComputeNodesResponse, ApiError> {
        self.transport_list_scheduled_compute_nodes(
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

    async fn list_slurm_schedulers(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        _: Option<String>,
        _: Option<String>,
        _: Option<String>,
        _: Option<String>,
        _: Option<i64>,
        _: Option<String>,
        _: Option<String>,
        _: Option<String>,
        _: Option<String>,
        context: &C,
    ) -> Result<ListSlurmSchedulersResponse, ApiError> {
        self.transport_list_slurm_schedulers(
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            context,
        )
        .await
    }

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
    ) -> Result<ListUserDataResponse, ApiError> {
        self.transport_list_user_data(
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

    async fn get_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetComputeNodeResponse, ApiError> {
        self.transport_get_compute_node(id, context).await
    }

    async fn get_event(&self, id: i64, context: &C) -> Result<GetEventResponse, ApiError> {
        self.transport_get_event(id, context).await
    }

    async fn get_file(&self, id: i64, context: &C) -> Result<GetFileResponse, ApiError> {
        self.transport_get_file(id, context).await
    }

    async fn get_job(&self, id: i64, context: &C) -> Result<GetJobResponse, ApiError> {
        self.transport_get_job(id, context).await
    }

    /// Retrieve a local scheduler.
    async fn get_local_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetLocalSchedulerResponse, ApiError> {
        self.transport_get_local_scheduler(id, context).await
    }

    /// Return the resource requirements for jobs with a status of ready.
    #[instrument(level = "debug", skip(self, context), fields(workflow_id = id, scheduler_config_id = ?scheduler_config_id))]
    async fn get_ready_job_requirements(
        &self,
        id: i64,
        scheduler_config_id: Option<i64>,
        context: &C,
    ) -> Result<GetReadyJobRequirementsResponse, ApiError> {
        self.transport_get_ready_job_requirements(id, scheduler_config_id, context)
            .await
    }

    /// Retrieve one resource requirements record.
    async fn get_resource_requirements(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetResourceRequirementsResponse, ApiError> {
        self.transport_get_resource_requirements(id, context).await
    }

    /// Retrieve a job result.
    async fn get_result(&self, id: i64, context: &C) -> Result<GetResultResponse, ApiError> {
        self.transport_get_result(id, context).await
    }

    /// Retrieve a scheduled compute node.
    async fn get_scheduled_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetScheduledComputeNodeResponse, ApiError> {
        self.transport_get_scheduled_compute_node(id, context).await
    }

    /// Retrieve a Slurm compute node configuration.
    async fn get_slurm_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetSlurmSchedulerResponse, ApiError> {
        self.transport_get_slurm_scheduler(id, context).await
    }

    /// Retrieve a user data record.
    async fn get_user_data(&self, id: i64, context: &C) -> Result<GetUserDataResponse, ApiError> {
        self.transport_get_user_data(id, context).await
    }

    /// Retrieve a workflow.
    async fn get_workflow(&self, id: i64, context: &C) -> Result<GetWorkflowResponse, ApiError> {
        self.transport_get_workflow(id, context).await
    }

    async fn get_workflow_status(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetWorkflowStatusResponse, ApiError> {
        self.transport_get_workflow_status(id, context).await
    }

    /// Initialize job relationships based on file and user_data relationships.
    ///
    /// This operation wraps all initialization steps in a transaction to ensure atomicity.
    /// If any step fails, all changes will be rolled back.
    ///
    /// This function can be called multiple times (e.g., for workflow reruns). It will:
    /// - Reset all job statuses to uninitialized
    /// - Delete workflow_result records for incomplete jobs
    /// - Re-initialize job statuses based on dependencies
    async fn initialize_jobs(
        &self,
        id: i64,
        only_uninitialized: Option<bool>,
        clear_ephemeral_user_data: Option<bool>,
        context: &C,
    ) -> Result<InitializeJobsResponse, ApiError> {
        self.transport_initialize_jobs(id, only_uninitialized, clear_ephemeral_user_data, context)
            .await
    }

    /// Return true if all jobs in the workflow are complete.
    #[instrument(level = "debug", skip(self, context), fields(workflow_id = id))]
    async fn is_workflow_complete(
        &self,
        id: i64,
        context: &C,
    ) -> Result<IsWorkflowCompleteResponse, ApiError> {
        self.transport_is_workflow_complete(id, context).await
    }

    async fn is_workflow_uninitialized(
        &self,
        id: i64,
        context: &C,
    ) -> Result<IsWorkflowUninitializedResponse, ApiError> {
        self.transport_is_workflow_uninitialized(id, context).await
    }

    /// Retrieve all job IDs for one workflow.
    async fn list_job_ids(&self, id: i64, context: &C) -> Result<ListJobIdsResponse, ApiError> {
        self.transport_list_job_ids(id, context).await
    }

    /// List missing user data that should exist.
    async fn list_missing_user_data(
        &self,
        id: i64,
        context: &C,
    ) -> Result<ListMissingUserDataResponse, ApiError> {
        self.transport_list_missing_user_data(id, context).await
    }

    /// List files that must exist.
    async fn list_required_existing_files(
        &self,
        id: i64,
        context: &C,
    ) -> Result<ListRequiredExistingFilesResponse, ApiError> {
        self.transport_list_required_existing_files(id, context)
            .await
    }

    /// Update a compute node.
    async fn update_compute_node(
        &self,
        id: i64,
        body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<UpdateComputeNodeResponse, ApiError> {
        self.transport_update_compute_node(id, body, context).await
    }

    /// Update an event.
    async fn update_event(
        &self,
        id: i64,
        body: serde_json::Value,
        context: &C,
    ) -> Result<UpdateEventResponse, ApiError> {
        self.transport_update_event(id, body, context).await
    }

    /// Update a file.
    async fn update_file(
        &self,
        id: i64,
        body: models::FileModel,
        context: &C,
    ) -> Result<UpdateFileResponse, ApiError> {
        self.transport_update_file(id, body, context).await
    }

    /// Update a job.
    async fn update_job(
        &self,
        id: i64,
        body: models::JobModel,
        context: &C,
    ) -> Result<UpdateJobResponse, ApiError> {
        self.transport_update_job(id, body, context).await
    }

    async fn update_local_scheduler(
        &self,
        id: i64,
        body: models::LocalSchedulerModel,
        context: &C,
    ) -> Result<UpdateLocalSchedulerResponse, ApiError> {
        self.transport_update_local_scheduler(id, body, context)
            .await
    }

    /// Update one resource requirements record.
    async fn update_resource_requirements(
        &self,
        id: i64,
        body: models::ResourceRequirementsModel,
        context: &C,
    ) -> Result<UpdateResourceRequirementsResponse, ApiError> {
        self.transport_update_resource_requirements(id, body, context)
            .await
    }

    /// Update a job result.
    async fn update_result(
        &self,
        id: i64,
        body: models::ResultModel,
        context: &C,
    ) -> Result<UpdateResultResponse, ApiError> {
        self.transport_update_result(id, body, context).await
    }

    /// Update a scheduled compute node.
    async fn update_scheduled_compute_node(
        &self,
        id: i64,
        body: models::ScheduledComputeNodesModel,
        context: &C,
    ) -> Result<UpdateScheduledComputeNodeResponse, ApiError> {
        self.transport_update_scheduled_compute_node(id, body, context)
            .await
    }

    /// Update a Slurm compute node configuration.
    async fn update_slurm_scheduler(
        &self,
        id: i64,
        body: models::SlurmSchedulerModel,
        context: &C,
    ) -> Result<UpdateSlurmSchedulerResponse, ApiError> {
        self.transport_update_slurm_scheduler(id, body, context)
            .await
    }

    /// Update a user data record.
    async fn update_user_data(
        &self,
        id: i64,
        body: models::UserDataModel,
        context: &C,
    ) -> Result<UpdateUserDataResponse, ApiError> {
        self.transport_update_user_data(id, body, context).await
    }

    /// Update a workflow.
    async fn update_workflow(
        &self,
        id: i64,
        body: models::WorkflowModel,
        context: &C,
    ) -> Result<UpdateWorkflowResponse, ApiError> {
        self.transport_update_workflow(id, body, context).await
    }

    /// Update the workflow status.
    async fn update_workflow_status(
        &self,
        id: i64,
        body: models::WorkflowStatusModel,
        context: &C,
    ) -> Result<UpdateWorkflowStatusResponse, ApiError> {
        self.transport_update_workflow_status(id, body, context)
            .await
    }

    /// Return jobs that are ready for submission and meet worker resource requirements. Set status to pending.
    #[instrument(level = "debug", skip(self, context), fields(workflow_id = id, limit))]
    async fn claim_jobs_based_on_resources(
        &self,
        id: i64,
        body: models::ComputeNodesResources,
        limit: i64,
        strict_scheduler_match: Option<bool>,
        context: &C,
    ) -> Result<ClaimJobsBasedOnResources, ApiError> {
        self.transport_claim_jobs_based_on_resources(
            id,
            body,
            limit,
            strict_scheduler_match,
            context,
        )
        .await
    }

    /// Return user-requested number of jobs that are ready for submission. Sets status to pending.
    #[instrument(level = "debug", skip(self, context), fields(workflow_id = id, limit = ?limit))]
    async fn claim_next_jobs(
        &self,
        id: i64,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ClaimNextJobsResponse, ApiError> {
        self.transport_claim_next_jobs(id, limit, context).await
    }

    /// Check for changed job inputs and update status accordingly.
    #[instrument(level = "debug", skip(self, context), fields(workflow_id = id, dry_run = ?dry_run))]
    async fn process_changed_job_inputs(
        &self,
        id: i64,
        dry_run: Option<bool>,
        context: &C,
    ) -> Result<ProcessChangedJobInputsResponse, ApiError> {
        self.transport_process_changed_job_inputs(id, dry_run, context)
            .await
    }

    /// Delete a compute node.
    async fn delete_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteComputeNodeResponse, ApiError> {
        self.transport_delete_compute_node(id, context).await
    }

    /// Delete an event.
    async fn delete_event(&self, id: i64, context: &C) -> Result<DeleteEventResponse, ApiError> {
        self.transport_delete_event(id, context).await
    }

    /// Delete a file.
    async fn delete_file(&self, id: i64, context: &C) -> Result<DeleteFileResponse, ApiError> {
        self.transport_delete_file(id, context).await
    }

    /// Delete a job.
    async fn delete_job(&self, id: i64, context: &C) -> Result<DeleteJobResponse, ApiError> {
        self.transport_delete_job(id, context).await
    }

    /// Delete a local scheduler.
    async fn delete_local_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteLocalSchedulerResponse, ApiError> {
        self.transport_delete_local_scheduler(id, context).await
    }

    /// Delete a resource requirements record.
    async fn delete_resource_requirements(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteResourceRequirementsResponse, ApiError> {
        self.transport_delete_resource_requirements(id, context)
            .await
    }

    /// Delete a job result.
    async fn delete_result(&self, id: i64, context: &C) -> Result<DeleteResultResponse, ApiError> {
        self.transport_delete_result(id, context).await
    }

    /// Delete a scheduled compute node.
    async fn delete_scheduled_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodeResponse, ApiError> {
        self.transport_delete_scheduled_compute_node(id, context)
            .await
    }

    /// Delete Slurm compute node configuration.
    async fn delete_slurm_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteSlurmSchedulerResponse, ApiError> {
        self.transport_delete_slurm_scheduler(id, context).await
    }

    /// Delete a user data record.
    async fn delete_user_data(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteUserDataResponse, ApiError> {
        self.transport_delete_user_data(id, context).await
    }

    async fn delete_workflow(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteWorkflowResponse, ApiError> {
        self.transport_delete_workflow(id, context).await
    }

    /// Reset status for jobs to uninitialized.
    /// If failed_only is true, only jobs with a failed result will be reset.
    /// If failed_only is false, all jobs will be reset.
    async fn reset_job_status(
        &self,
        id: i64,
        failed_only: Option<bool>,
        context: &C,
    ) -> Result<ResetJobStatusResponse, ApiError> {
        self.transport_reset_job_status(id, failed_only, context)
            .await
    }

    /// Reset worklow status.
    /// Rules:
    /// - Not allowed if any job is running or SubmittedPending (unless force=true)
    /// Actions:
    /// - Reset fields in WorkflowStatusModel
    async fn reset_workflow_status(
        &self,
        id: i64,
        force: Option<bool>,
        context: &C,
    ) -> Result<ResetWorkflowStatusResponse, ApiError> {
        self.transport_reset_workflow_status(id, force, context)
            .await
    }

    /// Change the status of a job and manage side effects.
    #[instrument(level = "debug", skip(self, context), fields(job_id = id, status = ?status, run_id))]
    async fn manage_status_change(
        &self,
        id: i64,
        status: models::JobStatus,
        run_id: i64,
        context: &C,
    ) -> Result<ManageStatusChangeResponse, ApiError> {
        self.transport_manage_status_change(id, status, run_id, context)
            .await
    }

    /// Start a job and manage side effects.
    #[instrument(level = "debug", skip(self, context), fields(job_id = id, run_id, compute_node_id))]
    async fn start_job(
        &self,
        id: i64,
        run_id: i64,
        compute_node_id: i64,
        context: &C,
    ) -> Result<StartJobResponse, ApiError> {
        self.transport_start_job(id, run_id, compute_node_id, context)
            .await
    }

    /// Complete a job, connect it to a result, and manage side effects.
    #[instrument(level = "debug", skip(self, result, context), fields(job_id = id, status = ?status, run_id))]
    async fn complete_job(
        &self,
        id: i64,
        status: models::JobStatus,
        run_id: i64,
        result: models::ResultModel,
        context: &C,
    ) -> Result<CompleteJobResponse, ApiError> {
        self.transport_complete_job(id, status, run_id, result, context)
            .await
    }

    /// Retry a failed job by resetting it to ready status and incrementing attempt_id.
    async fn retry_job(
        &self,
        id: i64,
        run_id: i64,
        max_retries: i32,
        context: &C,
    ) -> Result<RetryJobResponse, ApiError> {
        self.transport_retry_job(id, run_id, max_retries, context)
            .await
    }

    /// Get ready jobs that fit within the specified resource constraints.
    ///
    /// This function performs the following operations:
    /// 1. Queries job and resource_requirements tables for ready jobs
    /// 2. Filters jobs based on resource constraints:
    ///    - memory_bytes <= resources.memory_gb (converted from GiB to bytes)
    ///    - num_cpus <= resources.num_cpus
    ///    - num_gpus <= resources.num_gpus
    ///    - num_nodes <= resources.num_nodes (only multi-node jobs consume dedicated nodes)
    ///    - runtime_s < resources.time_limit (converted to seconds using duration_string_to_seconds)
    /// 3. Sorts results by job priority descending
    /// 4. Loops through returned records and accumulates resource consumption
    /// 5. Selects jobs that can fit within total available resources
    /// 6. Atomically updates selected jobs to "pending" status
    ///
    /// # Parameters
    /// - `workflow_id`: ID of the workflow to get jobs for
    /// - `resources`: Available compute resources (CPUs, memory, GPUs, nodes, time limit)
    /// - `limit`: Maximum number of jobs to return
    ///
    /// # Returns
    /// A `ClaimJobsBasedOnResources` containing the list of jobs that were selected and updated,
    /// or an error if the operation fails. The `reason` field is set to an empty string.
    ///
    /// # Implementation Notes
    /// - Uses SQLite BEGIN IMMEDIATE TRANSACTION to acquire a database write lock
    /// - This ensures thread-safe access at the database level, preventing race conditions
    /// - The lock prevents concurrent job selection and ensures consistent resource accounting
    /// - Leverages the time_utils::duration_string_to_seconds function for time parsing
    /// - All selected jobs are changed from "ready" to "pending" status atomically
    #[instrument(
        level = "debug",
        skip(self, resources, context),
        fields(workflow_id, limit)
    )]
    async fn prepare_ready_jobs(
        &self,
        workflow_id: i64,
        resources: models::ComputeNodesResources,
        limit: i64,
        strict_scheduler_match: Option<bool>,
        context: &C,
    ) -> Result<ClaimJobsBasedOnResources, ApiError> {
        self.transport_prepare_ready_jobs(
            workflow_id,
            resources,
            limit,
            strict_scheduler_match,
            context,
        )
        .await
    }

    // Access Groups API

    async fn create_access_group(
        &self,
        body: models::AccessGroupModel,
        context: &C,
    ) -> Result<CreateAccessGroupResponse, ApiError> {
        self.transport_create_access_group(body, context).await
    }

    async fn get_access_group(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetAccessGroupResponse, ApiError> {
        self.transport_get_access_group(id, context).await
    }

    async fn list_access_groups(
        &self,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListAccessGroupsApiResponse, ApiError> {
        self.transport_list_access_groups(offset, limit, context)
            .await
    }

    async fn delete_access_group(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteAccessGroupResponse, ApiError> {
        self.transport_delete_access_group(id, context).await
    }

    async fn add_user_to_group(
        &self,
        group_id: i64,
        body: models::UserGroupMembershipModel,
        context: &C,
    ) -> Result<AddUserToGroupResponse, ApiError> {
        self.transport_add_user_to_group(group_id, body, context)
            .await
    }

    async fn remove_user_from_group(
        &self,
        group_id: i64,
        user_name: String,
        context: &C,
    ) -> Result<RemoveUserFromGroupResponse, ApiError> {
        self.transport_remove_user_from_group(group_id, user_name, context)
            .await
    }

    async fn list_group_members(
        &self,
        group_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListGroupMembersResponse, ApiError> {
        self.transport_list_group_members(group_id, offset, limit, context)
            .await
    }

    async fn list_user_groups(
        &self,
        user_name: String,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListUserGroupsApiResponse, ApiError> {
        self.transport_list_user_groups(user_name, offset, limit, context)
            .await
    }

    async fn add_workflow_to_group(
        &self,
        workflow_id: i64,
        group_id: i64,
        context: &C,
    ) -> Result<AddWorkflowToGroupResponse, ApiError> {
        self.transport_add_workflow_to_group(workflow_id, group_id, context)
            .await
    }

    async fn remove_workflow_from_group(
        &self,
        workflow_id: i64,
        group_id: i64,
        context: &C,
    ) -> Result<RemoveWorkflowFromGroupResponse, ApiError> {
        self.transport_remove_workflow_from_group(workflow_id, group_id, context)
            .await
    }

    async fn list_workflow_groups(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListWorkflowGroupsResponse, ApiError> {
        self.transport_list_workflow_groups(workflow_id, offset, limit, context)
            .await
    }

    async fn check_workflow_access(
        &self,
        workflow_id: i64,
        user_name: String,
        context: &C,
    ) -> Result<CheckWorkflowAccessResponse, ApiError> {
        self.transport_check_workflow_access(workflow_id, user_name, context)
            .await
    }

    /// Subscribe to the event broadcast channel for SSE streaming.
    fn subscribe_to_events(&self) -> tokio::sync::broadcast::Receiver<BroadcastEvent> {
        self.event_broadcaster.subscribe()
    }
}

// Helper methods for Server (not part of the Api trait)
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Send + Sync,
{
    // No additional helper methods needed
}
