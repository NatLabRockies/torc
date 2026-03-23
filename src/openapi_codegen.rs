//! Code-first OpenAPI scaffold used to migrate away from generated server bindings.

mod access_control;
mod admin_resources;
mod compute_nodes;
mod events;
mod files;
mod final_surfaces;
mod helpers;
mod jobs;
mod local_schedulers;
mod results;
mod scheduled_compute_nodes;
mod slurm_schedulers;
mod system;
mod user_data;
mod workflows;

use axum::{Router, routing::get};
use serde_json::Value;
use utoipa::OpenApi;

pub use system::{PingResponse, VersionResponse};

use crate::api_models::{
    AccessCheckResponse, AccessGroupModel, ClaimActionRequest, ClaimActionResponse,
    ClaimJobsBasedOnResources, ClaimNextJobsResponse, ComputeNodeModel, ComputeNodesResources,
    CreateJobsResponse, DeleteCountResponse, DeleteRoCrateEntitiesResponse, EventModel,
    FailureHandlerModel, FileModel, GetReadyJobRequirementsResponse, IsCompleteResponse,
    IsUninitializedResponse, JobDependencyModel, JobFileRelationshipModel, JobModel, JobStatus,
    JobUserDataRelationshipModel, JobsModel, ListAccessGroupsResponse, ListComputeNodesResponse,
    ListEventsResponse, ListFailureHandlersResponse, ListFilesResponse,
    ListJobDependenciesResponse, ListJobFileRelationshipsResponse, ListJobIdsResponse,
    ListJobUserDataRelationshipsResponse, ListJobsResponse, ListLocalSchedulersResponse,
    ListMissingUserDataResponse, ListRequiredExistingFilesResponse,
    ListResourceRequirementsResponse, ListResultsResponse, ListRoCrateEntitiesResponse,
    ListScheduledComputeNodesResponse, ListSlurmSchedulersResponse, ListSlurmStatsResponse,
    ListUserDataResponse, ListUserGroupMembershipsResponse, ListWorkflowsResponse,
    LocalSchedulerModel, MessageResponse, ProcessChangedJobInputsResponse, ReloadAuthResponse,
    RemoteWorkerModel, ResetJobStatusResponse, ResourceRequirementsModel, ResultModel,
    RoCrateEntityModel, ScheduledComputeNodesModel, SlurmSchedulerModel, SlurmStatsModel,
    UserDataModel, UserGroupMembershipModel, WorkflowAccessGroupModel, WorkflowActionModel,
    WorkflowModel, WorkflowStatusModel,
};
use crate::api_version::HTTP_API_VERSION;
use helpers::{check_component_properties, check_operation_id, check_schema_properties};

#[derive(Debug, Clone)]
pub struct OpenApiAppState {
    pub version: String,
    pub api_version: String,
    pub git_hash: String,
    pub access_control_enabled: bool,
}

impl Default for OpenApiAppState {
    fn default() -> Self {
        Self {
            version: {
                let git_hash = option_env!("GIT_HASH").unwrap_or("unknown");
                let git_dirty = option_env!("GIT_DIRTY").unwrap_or("");
                format!("{} ({}{})", env!("CARGO_PKG_VERSION"), git_hash, git_dirty)
            },
            api_version: HTTP_API_VERSION.to_string(),
            git_hash: option_env!("GIT_HASH").unwrap_or("unknown").to_string(),
            access_control_enabled: false,
        }
    }
}

pub fn app_router(state: OpenApiAppState) -> Router {
    Router::new()
        .route(
            "/torc-service/v1/access_groups",
            axum::routing::post(access_control::create_access_group)
                .get(access_control::list_access_groups),
        )
        .route(
            "/torc-service/v1/access_groups/{id}",
            get(access_control::get_access_group).delete(access_control::delete_access_group),
        )
        .route(
            "/torc-service/v1/access_groups/{id}/members",
            axum::routing::post(access_control::add_user_to_group)
                .get(access_control::list_group_members),
        )
        .route(
            "/torc-service/v1/access_groups/{id}/members/{user_name}",
            axum::routing::delete(access_control::remove_user_from_group),
        )
        .route(
            "/torc-service/v1/users/{user_name}/groups",
            get(access_control::list_user_groups),
        )
        .route(
            "/torc-service/v1/workflows/{id}/access_groups",
            axum::routing::post(access_control::add_workflow_to_group)
                .get(access_control::list_workflow_groups),
        )
        .route(
            "/torc-service/v1/workflows/{id}/access_groups/{group_id}",
            axum::routing::delete(access_control::remove_workflow_from_group),
        )
        .route(
            "/torc-service/v1/access_check/{workflow_id}/{user_name}",
            get(access_control::check_workflow_access),
        )
        .route("/torc-service/v1/ping", get(system::ping))
        .route("/torc-service/v1/version", get(system::version))
        .route(
            "/torc-service/v1/bulk_jobs",
            axum::routing::post(admin_resources::create_jobs),
        )
        .route(
            "/torc-service/v1/compute_nodes",
            axum::routing::post(compute_nodes::create_compute_node)
                .get(compute_nodes::list_compute_nodes)
                .delete(compute_nodes::delete_compute_nodes),
        )
        .route(
            "/torc-service/v1/compute_nodes/{id}",
            get(compute_nodes::get_compute_node)
                .put(compute_nodes::update_compute_node)
                .delete(compute_nodes::delete_compute_node),
        )
        .route(
            "/torc-service/v1/events",
            axum::routing::post(events::create_event)
                .get(events::list_events)
                .delete(events::delete_events),
        )
        .route(
            "/torc-service/v1/events/{id}",
            get(events::get_event)
                .put(events::update_event)
                .delete(events::delete_event),
        )
        .route(
            "/torc-service/v1/files",
            axum::routing::post(files::create_file)
                .get(files::list_files)
                .delete(files::delete_files),
        )
        .route(
            "/torc-service/v1/files/{id}",
            get(files::get_file)
                .put(files::update_file)
                .delete(files::delete_file),
        )
        .route(
            "/torc-service/v1/local_schedulers",
            axum::routing::post(local_schedulers::create_local_scheduler)
                .get(local_schedulers::list_local_schedulers)
                .delete(local_schedulers::delete_local_schedulers),
        )
        .route(
            "/torc-service/v1/local_schedulers/{id}",
            get(local_schedulers::get_local_scheduler)
                .put(local_schedulers::update_local_scheduler)
                .delete(local_schedulers::delete_local_scheduler),
        )
        .route(
            "/torc-service/v1/resource_requirements",
            axum::routing::post(admin_resources::create_resource_requirements)
                .get(admin_resources::list_resource_requirements)
                .delete(admin_resources::delete_resource_requirements),
        )
        .route(
            "/torc-service/v1/resource_requirements/{id}",
            get(admin_resources::get_resource_requirements)
                .put(admin_resources::update_resource_requirements)
                .delete(admin_resources::delete_resource_requirement),
        )
        .route(
            "/torc-service/v1/failure_handlers",
            axum::routing::post(admin_resources::create_failure_handler),
        )
        .route(
            "/torc-service/v1/failure_handlers/{id}",
            get(admin_resources::get_failure_handler)
                .delete(admin_resources::delete_failure_handler),
        )
        .route(
            "/torc-service/v1/workflows/{id}/failure_handlers",
            get(admin_resources::list_failure_handlers),
        )
        .route(
            "/torc-service/v1/workflows/{id}/actions",
            axum::routing::post(final_surfaces::create_workflow_action)
                .get(final_surfaces::get_workflow_actions),
        )
        .route(
            "/torc-service/v1/workflows/{id}/actions/pending",
            get(final_surfaces::get_pending_actions),
        )
        .route(
            "/torc-service/v1/workflows/{id}/actions/{action_id}/claim",
            axum::routing::post(final_surfaces::claim_action),
        )
        .route(
            "/torc-service/v1/jobs",
            axum::routing::post(jobs::create_job)
                .get(jobs::list_jobs)
                .delete(jobs::delete_jobs),
        )
        .route(
            "/torc-service/v1/jobs/{id}",
            get(jobs::get_job)
                .put(jobs::update_job)
                .delete(jobs::delete_job),
        )
        .route(
            "/torc-service/v1/jobs/{id}/complete_job/{status}/{run_id}",
            axum::routing::post(jobs::complete_job),
        )
        .route(
            "/torc-service/v1/jobs/{id}/manage_status_change/{status}/{run_id}",
            axum::routing::put(jobs::manage_status_change),
        )
        .route(
            "/torc-service/v1/jobs/{id}/start_job/{run_id}/{compute_node_id}",
            axum::routing::put(jobs::start_job),
        )
        .route(
            "/torc-service/v1/jobs/{id}/retry/{run_id}",
            axum::routing::post(jobs::retry_job),
        )
        .route(
            "/torc-service/v1/user_data",
            axum::routing::post(user_data::create_user_data)
                .get(user_data::list_user_data)
                .delete(user_data::delete_all_user_data),
        )
        .route(
            "/torc-service/v1/user_data/{id}",
            get(user_data::get_user_data)
                .put(user_data::update_user_data)
                .delete(user_data::delete_user_data),
        )
        .route(
            "/torc-service/v1/results",
            axum::routing::post(results::create_result)
                .get(results::list_results)
                .delete(results::delete_results),
        )
        .route(
            "/torc-service/v1/results/{id}",
            get(results::get_result)
                .put(results::update_result)
                .delete(results::delete_result),
        )
        .route(
            "/torc-service/v1/scheduled_compute_nodes",
            axum::routing::post(scheduled_compute_nodes::create_scheduled_compute_node)
                .get(scheduled_compute_nodes::list_scheduled_compute_nodes)
                .delete(scheduled_compute_nodes::delete_scheduled_compute_nodes),
        )
        .route(
            "/torc-service/v1/scheduled_compute_nodes/{id}",
            get(scheduled_compute_nodes::get_scheduled_compute_node)
                .put(scheduled_compute_nodes::update_scheduled_compute_node)
                .delete(scheduled_compute_nodes::delete_scheduled_compute_node),
        )
        .route(
            "/torc-service/v1/slurm_schedulers",
            axum::routing::post(slurm_schedulers::create_slurm_scheduler)
                .get(slurm_schedulers::list_slurm_schedulers)
                .delete(slurm_schedulers::delete_slurm_schedulers),
        )
        .route(
            "/torc-service/v1/slurm_schedulers/{id}",
            get(slurm_schedulers::get_slurm_scheduler)
                .put(slurm_schedulers::update_slurm_scheduler)
                .delete(slurm_schedulers::delete_slurm_scheduler),
        )
        .route(
            "/torc-service/v1/slurm_stats",
            axum::routing::post(admin_resources::create_slurm_stats)
                .get(admin_resources::list_slurm_stats),
        )
        .route(
            "/torc-service/v1/workflows/{id}/remote_workers",
            axum::routing::post(final_surfaces::create_remote_workers)
                .get(final_surfaces::list_remote_workers),
        )
        .route(
            "/torc-service/v1/workflows/{id}/remote_workers/{worker}",
            axum::routing::delete(final_surfaces::delete_remote_worker),
        )
        .route(
            "/torc-service/v1/ro_crate_entities",
            axum::routing::post(final_surfaces::create_ro_crate_entity),
        )
        .route(
            "/torc-service/v1/ro_crate_entities/{id}",
            get(final_surfaces::get_ro_crate_entity)
                .put(final_surfaces::update_ro_crate_entity)
                .delete(final_surfaces::delete_ro_crate_entity),
        )
        .route(
            "/torc-service/v1/workflows/{id}/ro_crate_entities",
            get(final_surfaces::list_ro_crate_entities)
                .delete(final_surfaces::delete_ro_crate_entities),
        )
        .route(
            "/torc-service/v1/admin/reload-auth",
            axum::routing::post(final_surfaces::reload_auth),
        )
        .route(
            "/torc-service/v1/workflows",
            axum::routing::get(workflows::list_workflows).post(workflows::create_workflow),
        )
        .route(
            "/torc-service/v1/workflows/{id}",
            get(workflows::get_workflow)
                .put(workflows::update_workflow)
                .delete(workflows::delete_workflow),
        )
        .route(
            "/torc-service/v1/workflows/{id}/cancel",
            axum::routing::put(workflows::cancel_workflow),
        )
        .route(
            "/torc-service/v1/workflows/{id}/initialize_jobs",
            axum::routing::post(workflows::initialize_jobs),
        )
        .route(
            "/torc-service/v1/workflows/{id}/is_complete",
            get(workflows::is_workflow_complete),
        )
        .route(
            "/torc-service/v1/workflows/{id}/is_uninitialized",
            get(workflows::is_workflow_uninitialized),
        )
        .route(
            "/torc-service/v1/workflows/{id}/reset_status",
            axum::routing::post(workflows::reset_workflow_status),
        )
        .route(
            "/torc-service/v1/workflows/{id}/reset_job_status",
            axum::routing::post(workflows::reset_job_status),
        )
        .route(
            "/torc-service/v1/workflows/{id}/status",
            get(workflows::get_workflow_status).put(workflows::update_workflow_status),
        )
        .route(
            "/torc-service/v1/workflows/{id}/claim_jobs_based_on_resources/{limit}",
            axum::routing::post(workflows::claim_jobs_based_on_resources),
        )
        .route(
            "/torc-service/v1/workflows/{id}/claim_next_jobs",
            axum::routing::post(workflows::claim_next_jobs),
        )
        .route(
            "/torc-service/v1/workflows/{id}/job_dependencies",
            get(workflows::list_job_dependencies),
        )
        .route(
            "/torc-service/v1/workflows/{id}/job_file_relationships",
            get(workflows::list_job_file_relationships),
        )
        .route(
            "/torc-service/v1/workflows/{id}/job_user_data_relationships",
            get(workflows::list_job_user_data_relationships),
        )
        .route(
            "/torc-service/v1/workflows/{id}/job_ids",
            get(workflows::list_job_ids),
        )
        .route(
            "/torc-service/v1/workflows/{id}/missing_user_data",
            get(workflows::list_missing_user_data),
        )
        .route(
            "/torc-service/v1/workflows/{id}/process_changed_job_inputs",
            axum::routing::post(workflows::process_changed_job_inputs),
        )
        .route(
            "/torc-service/v1/workflows/{id}/ready_job_requirements",
            get(workflows::get_ready_job_requirements),
        )
        .route(
            "/torc-service/v1/workflows/{id}/required_existing_files",
            get(workflows::list_required_existing_files),
        )
        .with_state(state)
}

#[derive(OpenApi)]
#[openapi(
    servers((url = "/torc-service/v1", description = "Versioned Torc API base path")),
    paths(
        access_control::create_access_group,
        access_control::list_access_groups,
        access_control::get_access_group,
        access_control::delete_access_group,
        access_control::add_user_to_group,
        access_control::list_group_members,
        access_control::remove_user_from_group,
        access_control::list_user_groups,
        access_control::add_workflow_to_group,
        access_control::list_workflow_groups,
        access_control::remove_workflow_from_group,
        access_control::check_workflow_access,
        system::ping,
        system::version,
        admin_resources::create_jobs,
        compute_nodes::create_compute_node,
        compute_nodes::delete_compute_nodes,
        compute_nodes::list_compute_nodes,
        compute_nodes::delete_compute_node,
        compute_nodes::get_compute_node,
        compute_nodes::update_compute_node,
        events::create_event,
        events::delete_events,
        events::list_events,
        events::delete_event,
        events::get_event,
        events::update_event,
        files::create_file,
        files::delete_files,
        files::list_files,
        files::delete_file,
        files::get_file,
        files::update_file,
        jobs::create_job,
        jobs::delete_jobs,
        jobs::list_jobs,
        jobs::delete_job,
        jobs::get_job,
        jobs::update_job,
        jobs::complete_job,
        jobs::manage_status_change,
        jobs::start_job,
        jobs::retry_job,
        local_schedulers::create_local_scheduler,
        local_schedulers::delete_local_schedulers,
        local_schedulers::list_local_schedulers,
        local_schedulers::delete_local_scheduler,
        local_schedulers::get_local_scheduler,
        local_schedulers::update_local_scheduler,
        admin_resources::create_resource_requirements,
        admin_resources::delete_resource_requirements,
        admin_resources::list_resource_requirements,
        admin_resources::delete_resource_requirement,
        admin_resources::get_resource_requirements,
        admin_resources::update_resource_requirements,
        admin_resources::create_failure_handler,
        admin_resources::get_failure_handler,
        admin_resources::delete_failure_handler,
        admin_resources::list_failure_handlers,
        final_surfaces::create_workflow_action,
        final_surfaces::get_workflow_actions,
        final_surfaces::get_pending_actions,
        final_surfaces::claim_action,
        results::create_result,
        results::delete_results,
        results::list_results,
        results::delete_result,
        results::get_result,
        results::update_result,
        scheduled_compute_nodes::create_scheduled_compute_node,
        scheduled_compute_nodes::delete_scheduled_compute_nodes,
        scheduled_compute_nodes::list_scheduled_compute_nodes,
        scheduled_compute_nodes::delete_scheduled_compute_node,
        scheduled_compute_nodes::get_scheduled_compute_node,
        scheduled_compute_nodes::update_scheduled_compute_node,
        slurm_schedulers::create_slurm_scheduler,
        slurm_schedulers::delete_slurm_schedulers,
        slurm_schedulers::list_slurm_schedulers,
        slurm_schedulers::delete_slurm_scheduler,
        slurm_schedulers::get_slurm_scheduler,
        slurm_schedulers::update_slurm_scheduler,
        admin_resources::create_slurm_stats,
        admin_resources::list_slurm_stats,
        final_surfaces::create_remote_workers,
        final_surfaces::list_remote_workers,
        final_surfaces::delete_remote_worker,
        final_surfaces::create_ro_crate_entity,
        final_surfaces::get_ro_crate_entity,
        final_surfaces::update_ro_crate_entity,
        final_surfaces::delete_ro_crate_entity,
        final_surfaces::list_ro_crate_entities,
        final_surfaces::delete_ro_crate_entities,
        final_surfaces::reload_auth,
        workflows::list_workflows,
        workflows::create_workflow,
        workflows::delete_workflow,
        workflows::get_workflow,
        workflows::update_workflow,
        workflows::cancel_workflow,
        workflows::initialize_jobs,
        workflows::is_workflow_complete,
        workflows::is_workflow_uninitialized,
        workflows::reset_workflow_status,
        workflows::reset_job_status,
        workflows::get_workflow_status,
        workflows::update_workflow_status,
        workflows::claim_jobs_based_on_resources,
        workflows::claim_next_jobs,
        workflows::list_job_dependencies,
        workflows::list_job_file_relationships,
        workflows::list_job_user_data_relationships,
        workflows::list_job_ids,
        workflows::list_missing_user_data,
        workflows::process_changed_job_inputs,
        workflows::get_ready_job_requirements,
        workflows::list_required_existing_files,
        user_data::create_user_data,
        user_data::delete_all_user_data,
        user_data::list_user_data,
        user_data::delete_user_data,
        user_data::get_user_data,
        user_data::update_user_data
    ),
    components(schemas(
        PingResponse,
        VersionResponse,
        AccessGroupModel,
        UserGroupMembershipModel,
        WorkflowAccessGroupModel,
        ListAccessGroupsResponse,
        ListUserGroupMembershipsResponse,
        AccessCheckResponse,
        JobsModel,
        CreateJobsResponse,
        ComputeNodeModel,
        ListComputeNodesResponse,
        DeleteCountResponse,
        EventModel,
        ListEventsResponse,
        FileModel,
        ListFilesResponse,
        JobModel,
        ListJobsResponse,
        LocalSchedulerModel,
        ListLocalSchedulersResponse,
        ResourceRequirementsModel,
        ListResourceRequirementsResponse,
        FailureHandlerModel,
        ListFailureHandlersResponse,
        WorkflowActionModel,
        ClaimActionRequest,
        ClaimActionResponse,
        JobStatus,
        ResultModel,
        ListResultsResponse,
        ScheduledComputeNodesModel,
        ListScheduledComputeNodesResponse,
        SlurmSchedulerModel,
        ListSlurmSchedulersResponse,
        SlurmStatsModel,
        ListSlurmStatsResponse,
        RemoteWorkerModel,
        RoCrateEntityModel,
        ListRoCrateEntitiesResponse,
        MessageResponse,
        DeleteRoCrateEntitiesResponse,
        ReloadAuthResponse,
        WorkflowModel,
        ListWorkflowsResponse,
        ComputeNodesResources,
        ClaimJobsBasedOnResources,
        ClaimNextJobsResponse,
        JobDependencyModel,
        ListJobDependenciesResponse,
        JobFileRelationshipModel,
        ListJobFileRelationshipsResponse,
        JobUserDataRelationshipModel,
        ListJobUserDataRelationshipsResponse,
        ListJobIdsResponse,
        ListMissingUserDataResponse,
        ProcessChangedJobInputsResponse,
        GetReadyJobRequirementsResponse,
        ListRequiredExistingFilesResponse,
        WorkflowStatusModel,
        IsCompleteResponse,
        IsUninitializedResponse,
        ResetJobStatusResponse,
        UserDataModel,
        ListUserDataResponse
    )),
    info(
        title = "torc",
        version = env!("CARGO_PKG_VERSION"),
        description = "Rust-owned OpenAPI surface for Torc."
    )
)]
pub struct TorcOpenApi;

fn openapi_doc() -> utoipa::openapi::OpenApi {
    let mut doc = TorcOpenApi::openapi();
    doc.info.version = HTTP_API_VERSION.to_string();

    let workflow_action_required = vec![
        "id".to_string(),
        "workflow_id".to_string(),
        "trigger_type".to_string(),
        "action_type".to_string(),
        "action_config".to_string(),
        "trigger_count".to_string(),
        "required_triggers".to_string(),
        "executed".to_string(),
        "persistent".to_string(),
        "is_recovery".to_string(),
    ];

    if let Some(components) = doc.components.as_mut()
        && let Some(schema) = components.schemas.get_mut("WorkflowActionModel")
        && let utoipa::openapi::RefOr::T(utoipa::openapi::schema::Schema::Object(object)) = schema
    {
        object.required = workflow_action_required;
    }

    doc
}

pub fn openapi_value() -> Value {
    serde_json::to_value(openapi_doc()).expect("OpenAPI document should serialize")
}

pub fn render_openapi_yaml() -> Result<String, serde_yaml::Error> {
    serde_yaml::to_string(&openapi_doc())
}

pub fn parity_report(source: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let emitted = openapi_value();
    let mut issues = Vec::new();

    check_operation_id(source, &emitted, "/ping", "get", "ping", &mut issues);
    check_operation_id(
        source,
        &emitted,
        "/version",
        "get",
        "get_version",
        &mut issues,
    );
    check_schema_properties(&emitted, "/ping", "get", &["status"], &mut issues);
    check_schema_properties(
        &emitted,
        "/version",
        "get",
        &["version", "api_version", "git_hash"],
        &mut issues,
    );

    check_operation_id(
        source,
        &emitted,
        "/bulk_jobs",
        "post",
        "create_jobs",
        &mut issues,
    );
    check_component_properties(&emitted, "JobsModel", &["jobs"], &mut issues);
    check_component_properties(&emitted, "CreateJobsResponse", &["jobs"], &mut issues);

    check_operation_id(
        source,
        &emitted,
        "/access_groups",
        "post",
        "create_access_group",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/access_groups",
        "get",
        "list_access_groups",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/access_groups/{id}",
        "get",
        "get_access_group",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/access_groups/{id}",
        "delete",
        "delete_access_group",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/access_groups/{id}/members",
        "post",
        "add_user_to_group",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/access_groups/{id}/members",
        "get",
        "list_group_members",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/access_groups/{id}/members/{user_name}",
        "delete",
        "remove_user_from_group",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/users/{user_name}/groups",
        "get",
        "list_user_groups",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/access_groups",
        "post",
        "add_workflow_to_group",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/access_groups",
        "get",
        "list_workflow_groups",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/access_groups/{group_id}",
        "delete",
        "remove_workflow_from_group",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/access_check/{workflow_id}/{user_name}",
        "get",
        "check_workflow_access",
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "AccessGroupModel",
        &["id", "name", "description", "created_at"],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "UserGroupMembershipModel",
        &["id", "user_name", "group_id", "role", "created_at"],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "WorkflowAccessGroupModel",
        &["workflow_id", "group_id", "created_at"],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListAccessGroupsResponse",
        &["items", "offset", "limit", "total_count", "has_more"],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListUserGroupMembershipsResponse",
        &["items", "offset", "limit", "total_count", "has_more"],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "AccessCheckResponse",
        &["has_access", "user_name", "workflow_id", "reason"],
        &mut issues,
    );

    check_operation_id(
        source,
        &emitted,
        "/compute_nodes",
        "post",
        "create_compute_node",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/compute_nodes",
        "delete",
        "delete_compute_nodes",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/compute_nodes",
        "get",
        "list_compute_nodes",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/compute_nodes/{id}",
        "delete",
        "delete_compute_node",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/compute_nodes/{id}",
        "get",
        "get_compute_node",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/compute_nodes/{id}",
        "put",
        "update_compute_node",
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ComputeNodeModel",
        &[
            "id",
            "workflow_id",
            "hostname",
            "pid",
            "start_time",
            "duration_seconds",
            "is_active",
            "num_cpus",
            "memory_gb",
            "num_gpus",
            "num_nodes",
            "time_limit",
            "scheduler_config_id",
            "compute_node_type",
            "scheduler",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListComputeNodesResponse",
        &[
            "items",
            "offset",
            "max_limit",
            "count",
            "total_count",
            "has_more",
        ],
        &mut issues,
    );

    check_operation_id(
        source,
        &emitted,
        "/events",
        "post",
        "create_event",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/events",
        "delete",
        "delete_events",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/events",
        "get",
        "list_events",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/events/{id}",
        "delete",
        "delete_event",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/events/{id}",
        "get",
        "get_event",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/events/{id}",
        "put",
        "update_event",
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "EventModel",
        &["id", "workflow_id", "timestamp", "data"],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListEventsResponse",
        &[
            "items",
            "offset",
            "max_limit",
            "count",
            "total_count",
            "has_more",
        ],
        &mut issues,
    );

    check_operation_id(
        source,
        &emitted,
        "/files",
        "post",
        "create_file",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/files",
        "delete",
        "delete_files",
        &mut issues,
    );
    check_operation_id(source, &emitted, "/files", "get", "list_files", &mut issues);
    check_operation_id(
        source,
        &emitted,
        "/files/{id}",
        "delete",
        "delete_file",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/files/{id}",
        "get",
        "get_file",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/files/{id}",
        "put",
        "update_file",
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "FileModel",
        &["id", "workflow_id", "name", "path", "st_mtime"],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListFilesResponse",
        &[
            "items",
            "offset",
            "max_limit",
            "count",
            "total_count",
            "has_more",
        ],
        &mut issues,
    );

    check_operation_id(source, &emitted, "/jobs", "post", "create_job", &mut issues);
    check_operation_id(
        source,
        &emitted,
        "/jobs",
        "delete",
        "delete_jobs",
        &mut issues,
    );
    check_operation_id(source, &emitted, "/jobs", "get", "list_jobs", &mut issues);
    check_operation_id(
        source,
        &emitted,
        "/jobs/{id}",
        "delete",
        "delete_job",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/jobs/{id}",
        "get",
        "get_job",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/jobs/{id}",
        "put",
        "update_job",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/jobs/{id}/complete_job/{status}/{run_id}",
        "post",
        "complete_job",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/jobs/{id}/manage_status_change/{status}/{run_id}",
        "put",
        "manage_status_change",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/jobs/{id}/start_job/{run_id}/{compute_node_id}",
        "put",
        "start_job",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/jobs/{id}/retry/{run_id}",
        "post",
        "retry_job",
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "JobModel",
        &[
            "id",
            "workflow_id",
            "name",
            "command",
            "invocation_script",
            "status",
            "cancel_on_blocking_job_failure",
            "supports_termination",
            "depends_on_job_ids",
            "input_file_ids",
            "output_file_ids",
            "input_user_data_ids",
            "output_user_data_ids",
            "resource_requirements_id",
            "scheduler_id",
            "failure_handler_id",
            "attempt_id",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListJobsResponse",
        &[
            "items",
            "offset",
            "max_limit",
            "count",
            "total_count",
            "has_more",
        ],
        &mut issues,
    );

    check_operation_id(
        source,
        &emitted,
        "/local_schedulers",
        "post",
        "create_local_scheduler",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/local_schedulers",
        "delete",
        "delete_local_schedulers",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/local_schedulers",
        "get",
        "list_local_schedulers",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/local_schedulers/{id}",
        "delete",
        "delete_local_scheduler",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/local_schedulers/{id}",
        "get",
        "get_local_scheduler",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/local_schedulers/{id}",
        "put",
        "update_local_scheduler",
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "LocalSchedulerModel",
        &["id", "workflow_id", "name", "memory", "num_cpus"],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListLocalSchedulersResponse",
        &[
            "items",
            "offset",
            "max_limit",
            "count",
            "total_count",
            "has_more",
        ],
        &mut issues,
    );

    check_operation_id(
        source,
        &emitted,
        "/resource_requirements",
        "post",
        "create_resource_requirements",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/resource_requirements",
        "delete",
        "delete_resource_requirements",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/resource_requirements",
        "get",
        "list_resource_requirements",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/resource_requirements/{id}",
        "delete",
        "delete_resource_requirement",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/resource_requirements/{id}",
        "get",
        "get_resource_requirements",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/resource_requirements/{id}",
        "put",
        "update_resource_requirements",
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ResourceRequirementsModel",
        &[
            "id",
            "workflow_id",
            "name",
            "num_cpus",
            "num_gpus",
            "num_nodes",
            "memory",
            "runtime",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListResourceRequirementsResponse",
        &[
            "items",
            "offset",
            "max_limit",
            "count",
            "total_count",
            "has_more",
        ],
        &mut issues,
    );

    check_operation_id(
        source,
        &emitted,
        "/failure_handlers",
        "post",
        "create_failure_handler",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/failure_handlers/{id}",
        "get",
        "get_failure_handler",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/failure_handlers/{id}",
        "delete",
        "delete_failure_handler",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/failure_handlers",
        "get",
        "list_failure_handlers",
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "FailureHandlerModel",
        &["id", "workflow_id", "name", "rules"],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListFailureHandlersResponse",
        &[
            "items",
            "offset",
            "max_limit",
            "count",
            "total_count",
            "has_more",
        ],
        &mut issues,
    );

    check_operation_id(
        source,
        &emitted,
        "/results",
        "post",
        "create_result",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/results",
        "delete",
        "delete_results",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/results",
        "get",
        "list_results",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/results/{id}",
        "delete",
        "delete_result",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/results/{id}",
        "get",
        "get_result",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/results/{id}",
        "put",
        "update_result",
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ResultModel",
        &[
            "id",
            "job_id",
            "workflow_id",
            "run_id",
            "attempt_id",
            "compute_node_id",
            "return_code",
            "exec_time_minutes",
            "completion_time",
            "peak_memory_bytes",
            "avg_memory_bytes",
            "peak_cpu_percent",
            "avg_cpu_percent",
            "status",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListResultsResponse",
        &[
            "items",
            "offset",
            "max_limit",
            "count",
            "total_count",
            "has_more",
        ],
        &mut issues,
    );

    check_operation_id(
        source,
        &emitted,
        "/scheduled_compute_nodes",
        "post",
        "create_scheduled_compute_node",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/scheduled_compute_nodes",
        "delete",
        "delete_scheduled_compute_nodes",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/scheduled_compute_nodes",
        "get",
        "list_scheduled_compute_nodes",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/scheduled_compute_nodes/{id}",
        "delete",
        "delete_scheduled_compute_node",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/scheduled_compute_nodes/{id}",
        "get",
        "get_scheduled_compute_node",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/scheduled_compute_nodes/{id}",
        "put",
        "update_scheduled_compute_node",
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ScheduledComputeNodesModel",
        &[
            "id",
            "workflow_id",
            "scheduler_id",
            "scheduler_config_id",
            "scheduler_type",
            "status",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListScheduledComputeNodesResponse",
        &[
            "items",
            "offset",
            "max_limit",
            "count",
            "total_count",
            "has_more",
        ],
        &mut issues,
    );

    check_operation_id(
        source,
        &emitted,
        "/slurm_schedulers",
        "post",
        "create_slurm_scheduler",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/slurm_schedulers",
        "delete",
        "delete_slurm_schedulers",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/slurm_schedulers",
        "get",
        "list_slurm_schedulers",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/slurm_schedulers/{id}",
        "delete",
        "delete_slurm_scheduler",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/slurm_schedulers/{id}",
        "get",
        "get_slurm_scheduler",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/slurm_schedulers/{id}",
        "put",
        "update_slurm_scheduler",
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "SlurmSchedulerModel",
        &[
            "id",
            "workflow_id",
            "name",
            "account",
            "gres",
            "mem",
            "nodes",
            "ntasks_per_node",
            "partition",
            "qos",
            "tmp",
            "walltime",
            "extra",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListSlurmSchedulersResponse",
        &[
            "items",
            "offset",
            "max_limit",
            "count",
            "total_count",
            "has_more",
        ],
        &mut issues,
    );

    check_operation_id(
        source,
        &emitted,
        "/slurm_stats",
        "post",
        "create_slurm_stats",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/slurm_stats",
        "get",
        "list_slurm_stats",
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "SlurmStatsModel",
        &[
            "id",
            "workflow_id",
            "job_id",
            "run_id",
            "attempt_id",
            "slurm_job_id",
            "max_rss_bytes",
            "max_vm_size_bytes",
            "max_disk_read_bytes",
            "max_disk_write_bytes",
            "ave_cpu_seconds",
            "node_list",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListSlurmStatsResponse",
        &[
            "items",
            "offset",
            "max_limit",
            "count",
            "total_count",
            "has_more",
        ],
        &mut issues,
    );

    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/actions",
        "post",
        "create_workflow_action",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/actions",
        "get",
        "get_workflow_actions",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/actions/pending",
        "get",
        "get_pending_actions",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/actions/{action_id}/claim",
        "post",
        "claim_action",
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "WorkflowActionModel",
        &[
            "id",
            "workflow_id",
            "trigger_type",
            "action_type",
            "action_config",
            "job_ids",
            "trigger_count",
            "required_triggers",
            "executed",
            "executed_at",
            "executed_by",
            "persistent",
            "is_recovery",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ClaimActionRequest",
        &["compute_node_id"],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ClaimActionResponse",
        &["action_id", "success"],
        &mut issues,
    );

    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/remote_workers",
        "post",
        "create_remote_workers",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/remote_workers",
        "get",
        "list_remote_workers",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/remote_workers/{worker}",
        "delete",
        "delete_remote_worker",
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "RemoteWorkerModel",
        &["worker", "workflow_id"],
        &mut issues,
    );

    check_operation_id(
        source,
        &emitted,
        "/ro_crate_entities",
        "post",
        "create_ro_crate_entity",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/ro_crate_entities/{id}",
        "get",
        "get_ro_crate_entity",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/ro_crate_entities/{id}",
        "put",
        "update_ro_crate_entity",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/ro_crate_entities/{id}",
        "delete",
        "delete_ro_crate_entity",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/ro_crate_entities",
        "get",
        "list_ro_crate_entities",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/ro_crate_entities",
        "delete",
        "delete_ro_crate_entities",
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "RoCrateEntityModel",
        &[
            "id",
            "workflow_id",
            "file_id",
            "entity_id",
            "entity_type",
            "metadata",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListRoCrateEntitiesResponse",
        &[
            "items",
            "offset",
            "max_limit",
            "count",
            "total_count",
            "has_more",
        ],
        &mut issues,
    );
    check_component_properties(&emitted, "MessageResponse", &["message"], &mut issues);
    check_component_properties(
        &emitted,
        "DeleteRoCrateEntitiesResponse",
        &["message", "deleted_count"],
        &mut issues,
    );

    check_operation_id(
        source,
        &emitted,
        "/admin/reload-auth",
        "post",
        "reload_auth",
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ReloadAuthResponse",
        &["message", "user_count"],
        &mut issues,
    );

    check_operation_id(
        source,
        &emitted,
        "/workflows",
        "get",
        "list_workflows",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows",
        "post",
        "create_workflow",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}",
        "delete",
        "delete_workflow",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}",
        "get",
        "get_workflow",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}",
        "put",
        "update_workflow",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/cancel",
        "put",
        "cancel_workflow",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/initialize_jobs",
        "post",
        "initialize_jobs",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/is_complete",
        "get",
        "is_workflow_complete",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/is_uninitialized",
        "get",
        "is_workflow_uninitialized",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/reset_status",
        "post",
        "reset_workflow_status",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/reset_job_status",
        "post",
        "reset_job_status",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/status",
        "get",
        "get_workflow_status",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/status",
        "put",
        "update_workflow_status",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/claim_jobs_based_on_resources/{limit}",
        "post",
        "claim_jobs_based_on_resources",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/claim_next_jobs",
        "post",
        "claim_next_jobs",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/job_dependencies",
        "get",
        "list_job_dependencies",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/job_file_relationships",
        "get",
        "list_job_file_relationships",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/job_user_data_relationships",
        "get",
        "list_job_user_data_relationships",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/job_ids",
        "get",
        "list_job_ids",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/missing_user_data",
        "get",
        "list_missing_user_data",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/process_changed_job_inputs",
        "post",
        "process_changed_job_inputs",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/ready_job_requirements",
        "get",
        "get_ready_job_requirements",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/workflows/{id}/required_existing_files",
        "get",
        "list_required_existing_files",
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "WorkflowModel",
        &[
            "id",
            "name",
            "user",
            "description",
            "timestamp",
            "project",
            "metadata",
            "compute_node_expiration_buffer_seconds",
            "compute_node_wait_for_new_jobs_seconds",
            "compute_node_ignore_workflow_completion",
            "compute_node_wait_for_healthy_database_minutes",
            "compute_node_min_time_for_new_jobs_seconds",
            "resource_monitor_config",
            "slurm_defaults",
            "use_pending_failed",
            "enable_ro_crate",
            "status_id",
            "slurm_config",
            "execution_config",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListWorkflowsResponse",
        &[
            "items",
            "offset",
            "max_limit",
            "count",
            "total_count",
            "has_more",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "WorkflowStatusModel",
        &[
            "id",
            "is_canceled",
            "is_archived",
            "run_id",
            "has_detected_need_to_run_completion_script",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "IsCompleteResponse",
        &[
            "is_canceled",
            "is_complete",
            "needs_to_run_completion_script",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "IsUninitializedResponse",
        &["is_uninitialized"],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ResetJobStatusResponse",
        &["workflow_id", "updated_count", "status", "reset_type"],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ComputeNodesResources",
        &[
            "id",
            "num_cpus",
            "memory_gb",
            "num_gpus",
            "num_nodes",
            "time_limit",
            "scheduler_config_id",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ClaimJobsBasedOnResources",
        &["jobs", "reason"],
        &mut issues,
    );
    check_component_properties(&emitted, "ClaimNextJobsResponse", &["jobs"], &mut issues);
    check_component_properties(
        &emitted,
        "JobDependencyModel",
        &[
            "job_id",
            "job_name",
            "depends_on_job_id",
            "depends_on_job_name",
            "workflow_id",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListJobDependenciesResponse",
        &[
            "items",
            "offset",
            "max_limit",
            "count",
            "total_count",
            "has_more",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "JobFileRelationshipModel",
        &[
            "file_id",
            "file_name",
            "file_path",
            "producer_job_id",
            "producer_job_name",
            "consumer_job_id",
            "consumer_job_name",
            "workflow_id",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListJobFileRelationshipsResponse",
        &[
            "items",
            "offset",
            "max_limit",
            "count",
            "total_count",
            "has_more",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "JobUserDataRelationshipModel",
        &[
            "user_data_id",
            "user_data_name",
            "producer_job_id",
            "producer_job_name",
            "consumer_job_id",
            "consumer_job_name",
            "workflow_id",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListJobUserDataRelationshipsResponse",
        &[
            "items",
            "offset",
            "max_limit",
            "count",
            "total_count",
            "has_more",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListJobIdsResponse",
        &["job_ids", "count"],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListMissingUserDataResponse",
        &["user_data"],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ProcessChangedJobInputsResponse",
        &["reinitialized_jobs"],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "GetReadyJobRequirementsResponse",
        &[
            "num_jobs",
            "num_cpus",
            "num_gpus",
            "memory_gb",
            "max_num_nodes",
            "max_runtime",
        ],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListRequiredExistingFilesResponse",
        &["files"],
        &mut issues,
    );

    check_operation_id(
        source,
        &emitted,
        "/user_data",
        "post",
        "create_user_data",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/user_data",
        "delete",
        "delete_all_user_data",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/user_data",
        "get",
        "list_user_data",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/user_data/{id}",
        "delete",
        "delete_user_data",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/user_data/{id}",
        "get",
        "get_user_data",
        &mut issues,
    );
    check_operation_id(
        source,
        &emitted,
        "/user_data/{id}",
        "put",
        "update_user_data",
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "UserDataModel",
        &["id", "workflow_id", "is_ephemeral", "name", "data"],
        &mut issues,
    );
    check_component_properties(
        &emitted,
        "ListUserDataResponse",
        &[
            "items",
            "offset",
            "max_limit",
            "count",
            "total_count",
            "has_more",
        ],
        &mut issues,
    );

    Ok(issues)
}

#[cfg(test)]
mod tests {
    use super::{parity_report, render_openapi_yaml};

    #[test]
    fn generated_yaml_contains_scaffold_paths() {
        let yaml = render_openapi_yaml().expect("openapi yaml should render");

        assert!(yaml.contains("/access_groups"));
        assert!(yaml.contains("/access_check/{workflow_id}/{user_name}"));
        assert!(yaml.contains("/ping"));
        assert!(yaml.contains("/version"));
        assert!(yaml.contains("/bulk_jobs"));
        assert!(yaml.contains("/compute_nodes"));
        assert!(yaml.contains("/events"));
        assert!(yaml.contains("/files"));
        assert!(yaml.contains("/jobs"));
        assert!(yaml.contains("/local_schedulers"));
        assert!(yaml.contains("/resource_requirements"));
        assert!(yaml.contains("/failure_handlers"));
        assert!(yaml.contains("/workflows/{id}/failure_handlers"));
        assert!(yaml.contains("/workflows/{id}/actions"));
        assert!(yaml.contains("/workflows/{id}/actions/pending"));
        assert!(yaml.contains("/workflows/{id}/actions/{action_id}/claim"));
        assert!(yaml.contains("/results"));
        assert!(yaml.contains("/scheduled_compute_nodes"));
        assert!(yaml.contains("/slurm_schedulers"));
        assert!(yaml.contains("/slurm_stats"));
        assert!(yaml.contains("/workflows/{id}/remote_workers"));
        assert!(yaml.contains("/ro_crate_entities"));
        assert!(yaml.contains("/workflows/{id}/ro_crate_entities"));
        assert!(yaml.contains("/admin/reload-auth"));
        assert!(yaml.contains("/user_data"));
        assert!(yaml.contains("/workflows"));
        assert!(yaml.contains("/workflows/{id}/status"));
        assert!(yaml.contains("/workflows/{id}/claim_jobs_based_on_resources/{limit}"));
        assert!(yaml.contains("/workflows/{id}/claim_next_jobs"));
        assert!(yaml.contains("/workflows/{id}/job_dependencies"));
        assert!(yaml.contains("/workflows/{id}/job_file_relationships"));
        assert!(yaml.contains("/workflows/{id}/job_user_data_relationships"));
        assert!(yaml.contains("/workflows/{id}/job_ids"));
        assert!(yaml.contains("/workflows/{id}/missing_user_data"));
        assert!(yaml.contains("/workflows/{id}/process_changed_job_inputs"));
        assert!(yaml.contains("/workflows/{id}/ready_job_requirements"));
        assert!(yaml.contains("/workflows/{id}/required_existing_files"));
        assert!(yaml.contains("create_access_group"));
        assert!(yaml.contains("check_workflow_access"));
        assert!(yaml.contains("get_version"));
        assert!(yaml.contains("create_jobs"));
        assert!(yaml.contains("list_compute_nodes"));
        assert!(yaml.contains("list_events"));
        assert!(yaml.contains("list_files"));
        assert!(yaml.contains("list_jobs"));
        assert!(yaml.contains("list_local_schedulers"));
        assert!(yaml.contains("list_resource_requirements"));
        assert!(yaml.contains("list_failure_handlers"));
        assert!(yaml.contains("create_workflow_action"));
        assert!(yaml.contains("claim_action"));
        assert!(yaml.contains("list_results"));
        assert!(yaml.contains("list_scheduled_compute_nodes"));
        assert!(yaml.contains("list_slurm_schedulers"));
        assert!(yaml.contains("list_slurm_stats"));
        assert!(yaml.contains("create_remote_workers"));
        assert!(yaml.contains("create_ro_crate_entity"));
        assert!(yaml.contains("reload_auth"));
        assert!(yaml.contains("list_user_data"));
        assert!(yaml.contains("list_workflows"));
        assert!(yaml.contains("get_workflow_status"));
        assert!(yaml.contains("claim_jobs_based_on_resources"));
        assert!(yaml.contains("claim_next_jobs"));
        assert!(yaml.contains("list_job_dependencies"));
        assert!(yaml.contains("list_job_file_relationships"));
        assert!(yaml.contains("list_job_user_data_relationships"));
        assert!(yaml.contains("list_job_ids"));
        assert!(yaml.contains("list_missing_user_data"));
        assert!(yaml.contains("process_changed_job_inputs"));
        assert!(yaml.contains("get_ready_job_requirements"));
        assert!(yaml.contains("list_required_existing_files"));
        assert!(yaml.contains("status:"));
    }

    #[test]
    fn parity_check_accepts_current_system_endpoints() {
        let source = include_str!("../api/openapi.yaml");
        let issues = parity_report(source).expect("parity report should run");
        assert!(issues.is_empty(), "unexpected parity issues: {issues:?}");
    }
}
