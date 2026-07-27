//! Shared live server state that can be reused by multiple HTTP frontends.

use crate::server::api::{
    AccessGroupsApiImpl, ApiContext, ComputeNodesApiImpl, EventsApiImpl, FailureHandlersApiImpl,
    FilesApiImpl, JobsApiImpl, RemoteWorkersApiImpl, ResourceRequirementsApiImpl, ResultsApiImpl,
    RoCrateApiImpl, SchedulersApiImpl, SlurmStatsApiImpl, UserDataApiImpl, WorkflowActionsApiImpl,
    WorkflowsApiImpl,
};
use crate::server::api_event_stream::ApiEventBroadcaster;
use crate::server::api_stats::ApiStatsRing;
use crate::server::auth::{SharedCredentialCache, SharedHtpasswd};
use crate::server::authorization::AuthorizationService;
use crate::server::event_broadcast::EventBroadcaster;
use sqlx::sqlite::SqlitePool;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

/// Operator-controlled availability of the admin raw-SQL endpoint
/// (`POST /admin/sql`). Both halves default to enabled; the `torc-server`
/// `--disable-admin-sql` / `--disable-admin-sql-writes` flags turn them off.
/// The audit-log listing is intentionally not gated here so past activity stays
/// reviewable even after the feature is disabled.
#[derive(Debug, Clone, Copy)]
pub struct AdminSqlConfig {
    /// When false, both read and write `admin_sql` calls are rejected (403).
    pub reads_enabled: bool,
    /// When false, write-path `admin_sql` calls are rejected (403); reads are
    /// unaffected. Implied false whenever `reads_enabled` is false.
    pub writes_enabled: bool,
}

impl Default for AdminSqlConfig {
    fn default() -> Self {
        Self {
            reads_enabled: true,
            writes_enabled: true,
        }
    }
}

#[derive(Clone)]
pub struct LiveServerState {
    pub(crate) pool: Arc<SqlitePool>,
    pub(crate) last_completion_time: Arc<AtomicU64>,
    pub(crate) workflows_with_failures: Arc<std::sync::RwLock<HashSet<i64>>>,
    pub(crate) authorization_service: AuthorizationService,
    pub(crate) event_broadcaster: EventBroadcaster,
    pub(crate) api_event_broadcaster: ApiEventBroadcaster,
    pub(crate) api_stats: ApiStatsRing,
    pub(crate) htpasswd: SharedHtpasswd,
    pub(crate) auth_file_path: Option<String>,
    pub(crate) credential_cache: SharedCredentialCache,
    pub(crate) admin_sql: AdminSqlConfig,
    pub(crate) access_groups_api: AccessGroupsApiImpl,
    pub(crate) compute_nodes_api: ComputeNodesApiImpl,
    pub(crate) events_api: EventsApiImpl,
    pub(crate) failure_handlers_api: FailureHandlersApiImpl,
    pub(crate) files_api: FilesApiImpl,
    pub(crate) jobs_api: JobsApiImpl,
    pub(crate) remote_workers_api: RemoteWorkersApiImpl,
    pub(crate) resource_requirements_api: ResourceRequirementsApiImpl,
    pub(crate) results_api: ResultsApiImpl,
    pub(crate) ro_crate_api: RoCrateApiImpl,
    pub(crate) schedulers_api: SchedulersApiImpl,
    pub(crate) slurm_stats_api: SlurmStatsApiImpl,
    pub(crate) user_data_api: UserDataApiImpl,
    pub(crate) workflow_actions_api: WorkflowActionsApiImpl,
    pub(crate) workflows_api: WorkflowsApiImpl,
}

impl LiveServerState {
    pub(crate) fn new(
        pool: SqlitePool,
        enforce_access_control: bool,
        htpasswd: SharedHtpasswd,
        auth_file_path: Option<String>,
        credential_cache: SharedCredentialCache,
        admin_sql: AdminSqlConfig,
    ) -> Self {
        let pool_arc = Arc::new(pool);
        let api_context = ApiContext::new(pool_arc.as_ref().clone());
        let authorization_service =
            AuthorizationService::new(pool_arc.clone(), enforce_access_control);

        Self {
            pool: pool_arc,
            last_completion_time: Arc::new(AtomicU64::new(1)),
            workflows_with_failures: Arc::new(std::sync::RwLock::new(HashSet::new())),
            authorization_service,
            event_broadcaster: EventBroadcaster::new(512),
            api_event_broadcaster: ApiEventBroadcaster::default(),
            api_stats: ApiStatsRing::new(),
            htpasswd,
            auth_file_path,
            credential_cache,
            admin_sql,
            access_groups_api: AccessGroupsApiImpl::new(api_context.clone()),
            compute_nodes_api: ComputeNodesApiImpl::new(api_context.clone()),
            events_api: EventsApiImpl::new(api_context.clone()),
            failure_handlers_api: FailureHandlersApiImpl::new(api_context.clone()),
            files_api: FilesApiImpl::new(api_context.clone()),
            jobs_api: JobsApiImpl::new(api_context.clone()),
            remote_workers_api: RemoteWorkersApiImpl::new(api_context.clone()),
            resource_requirements_api: ResourceRequirementsApiImpl::new(api_context.clone()),
            results_api: ResultsApiImpl::new(api_context.clone()),
            ro_crate_api: RoCrateApiImpl::new(api_context.clone()),
            schedulers_api: SchedulersApiImpl::new(api_context.clone()),
            slurm_stats_api: SlurmStatsApiImpl::new(api_context.clone()),
            user_data_api: UserDataApiImpl::new(api_context.clone()),
            workflow_actions_api: WorkflowActionsApiImpl::new(api_context.clone()),
            workflows_api: WorkflowsApiImpl::new(api_context.clone()),
        }
    }

    #[cfg(feature = "openapi-codegen")]
    pub(crate) fn openapi_app_state(
        &self,
        version: String,
        api_version: String,
        git_hash: String,
    ) -> crate::openapi_spec::OpenApiAppState {
        crate::openapi_spec::OpenApiAppState {
            version,
            api_version,
            git_hash,
            access_control_enabled: self.authorization_service.enforce_access_control(),
        }
    }
}
