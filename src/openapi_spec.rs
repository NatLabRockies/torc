//! Live-server-owned OpenAPI document and parity checks.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use utoipa::OpenApi;
use utoipa::ToSchema;

use crate::api_version::HTTP_API_VERSION;
use crate::models::{
    AccessCheckResponse, AccessGroupModel, BatchCompleteJobsRequest, BatchCompleteJobsResponse,
    ClaimActionRequest, ClaimActionResponse, ClaimJobsBasedOnResources, ClaimNextJobsResponse,
    ComputeNodeModel, ComputeNodesResources, CreateJobsResponse, DeleteCountResponse,
    DeleteRoCrateEntitiesResponse, EventModel, FailureHandlerModel, FileModel,
    GetReadyJobRequirementsResponse, IsCompleteResponse, IsUninitializedResponse,
    JobCompletionEntry, JobCompletionError, JobDependencyModel, JobFileRelationshipModel, JobModel,
    JobStatus, JobUserDataRelationshipModel, JobsModel, ListAccessGroupsResponse,
    ListComputeNodesResponse, ListEventsResponse, ListFailureHandlersResponse, ListFilesResponse,
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

#[allow(unused_imports)]
mod openapi_job_paths {
    pub use crate::server::live_router::{
        __path_complete_job, __path_create_job, __path_delete_job, __path_delete_jobs,
        __path_get_job, __path_list_jobs, __path_manage_status_change, __path_retry_job,
        __path_start_job, __path_update_job, complete_job, create_job, delete_job, delete_jobs,
        get_job, list_jobs, manage_status_change, retry_job, start_job, update_job,
    };
}

#[allow(unused_imports)]
mod openapi_system_paths {
    pub use crate::server::live_router::{__path_ping, __path_version, ping, version};
}

#[allow(unused_imports)]
mod openapi_access_control_paths {
    pub use crate::server::live_router::{
        __path_add_user_to_group, __path_add_workflow_to_group, __path_check_workflow_access,
        __path_create_access_group, __path_delete_access_group, __path_get_access_group,
        __path_list_access_groups, __path_list_group_members, __path_list_user_groups,
        __path_list_workflow_groups, __path_reload_auth, __path_remove_user_from_group,
        __path_remove_workflow_from_group, add_user_to_group, add_workflow_to_group,
        check_workflow_access, create_access_group, delete_access_group, get_access_group,
        list_access_groups, list_group_members, list_user_groups, list_workflow_groups,
        reload_auth, remove_user_from_group, remove_workflow_from_group,
    };
}

#[allow(unused_imports)]
mod openapi_bulk_job_paths {
    pub use crate::server::live_router::{__path_create_jobs, create_jobs};
}

#[allow(unused_imports)]
mod openapi_compute_node_paths {
    pub use crate::server::live_router::{
        __path_create_compute_node, __path_delete_compute_node, __path_delete_compute_nodes,
        __path_get_compute_node, __path_list_compute_nodes, __path_update_compute_node,
        create_compute_node, delete_compute_node, delete_compute_nodes, get_compute_node,
        list_compute_nodes, update_compute_node,
    };
}

#[allow(unused_imports)]
mod openapi_file_paths {
    pub use crate::server::live_router::{
        __path_create_file, __path_delete_file, __path_delete_files, __path_get_file,
        __path_list_files, __path_update_file, create_file, delete_file, delete_files, get_file,
        list_files, update_file,
    };
}

#[allow(unused_imports)]
mod openapi_local_scheduler_paths {
    pub use crate::server::live_router::{
        __path_create_local_scheduler, __path_delete_local_scheduler,
        __path_delete_local_schedulers, __path_get_local_scheduler, __path_list_local_schedulers,
        __path_update_local_scheduler, create_local_scheduler, delete_local_scheduler,
        delete_local_schedulers, get_local_scheduler, list_local_schedulers,
        update_local_scheduler,
    };
}

#[allow(unused_imports)]
mod openapi_event_paths {
    pub use crate::server::live_router::{
        __path_create_event, __path_delete_event, __path_delete_events, __path_get_event,
        __path_list_events, __path_update_event, create_event, delete_event, delete_events,
        get_event, list_events, update_event,
    };
}

#[allow(unused_imports)]
mod openapi_result_paths {
    pub use crate::server::live_router::{
        __path_create_result, __path_delete_result, __path_delete_results, __path_get_result,
        __path_list_results, __path_update_result, create_result, delete_result, delete_results,
        get_result, list_results, update_result,
    };
}

#[allow(unused_imports)]
mod openapi_user_data_paths {
    pub use crate::server::live_router::{
        __path_create_user_data, __path_delete_all_user_data, __path_delete_user_data,
        __path_get_user_data, __path_list_user_data, __path_update_user_data, create_user_data,
        delete_all_user_data, delete_user_data, get_user_data, list_user_data, update_user_data,
    };
}

#[allow(unused_imports)]
mod openapi_workflow_action_paths {
    pub use crate::server::live_router::{
        __path_claim_action, __path_create_workflow_action, __path_get_pending_actions,
        __path_get_workflow_actions, claim_action, create_workflow_action, get_pending_actions,
        get_workflow_actions,
    };
}

#[allow(unused_imports)]
mod openapi_workflow_paths {
    pub use crate::server::live_router::{
        __path_batch_complete_jobs, __path_cancel_workflow, __path_claim_jobs_based_on_resources,
        __path_claim_next_jobs, __path_create_workflow, __path_delete_workflow,
        __path_get_active_task_for_workflow, __path_get_ready_job_requirements,
        __path_get_workflow, __path_get_workflow_status, __path_initialize_jobs,
        __path_is_workflow_complete, __path_is_workflow_uninitialized,
        __path_list_job_dependencies, __path_list_job_file_relationships, __path_list_job_ids,
        __path_list_job_user_data_relationships, __path_list_missing_user_data,
        __path_list_required_existing_files, __path_list_workflows,
        __path_process_changed_job_inputs, __path_reset_job_status, __path_reset_workflow_status,
        __path_update_workflow, __path_update_workflow_status, batch_complete_jobs,
        cancel_workflow, claim_jobs_based_on_resources, claim_next_jobs, create_workflow,
        delete_workflow, get_active_task_for_workflow, get_ready_job_requirements, get_workflow,
        get_workflow_status, initialize_jobs, is_workflow_complete, is_workflow_uninitialized,
        list_job_dependencies, list_job_file_relationships, list_job_ids,
        list_job_user_data_relationships, list_missing_user_data, list_required_existing_files,
        list_workflows, process_changed_job_inputs, reset_job_status, reset_workflow_status,
        update_workflow, update_workflow_status,
    };
}

#[allow(unused_imports)]
mod openapi_resource_requirements_paths {
    pub use crate::server::live_router::{
        __path_create_resource_requirements, __path_delete_all_resource_requirements,
        __path_delete_resource_requirements, __path_get_resource_requirements,
        __path_list_resource_requirements, __path_update_resource_requirements,
        create_resource_requirements, delete_all_resource_requirements,
        delete_resource_requirements, get_resource_requirements, list_resource_requirements,
        update_resource_requirements,
    };
}

#[allow(unused_imports)]
mod openapi_failure_handler_paths {
    pub use crate::server::live_router::{
        __path_create_failure_handler, __path_delete_failure_handler, __path_get_failure_handler,
        __path_list_failure_handlers, create_failure_handler, delete_failure_handler,
        get_failure_handler, list_failure_handlers,
    };
}

#[allow(unused_imports)]
mod openapi_slurm_stats_paths {
    pub use crate::server::live_router::{
        __path_create_slurm_stats, __path_list_slurm_stats, create_slurm_stats, list_slurm_stats,
    };
}

#[allow(unused_imports)]
mod openapi_scheduled_compute_node_paths {
    pub use crate::server::live_router::{
        __path_create_scheduled_compute_node, __path_delete_scheduled_compute_node,
        __path_delete_scheduled_compute_nodes, __path_get_scheduled_compute_node,
        __path_list_scheduled_compute_nodes, __path_update_scheduled_compute_node,
        create_scheduled_compute_node, delete_scheduled_compute_node,
        delete_scheduled_compute_nodes, get_scheduled_compute_node, list_scheduled_compute_nodes,
        update_scheduled_compute_node,
    };
}

#[allow(unused_imports)]
mod openapi_slurm_scheduler_paths {
    pub use crate::server::live_router::{
        __path_create_slurm_scheduler, __path_delete_slurm_scheduler,
        __path_delete_slurm_schedulers, __path_get_slurm_scheduler, __path_list_slurm_schedulers,
        __path_update_slurm_scheduler, create_slurm_scheduler, delete_slurm_scheduler,
        delete_slurm_schedulers, get_slurm_scheduler, list_slurm_schedulers,
        update_slurm_scheduler,
    };
}

#[allow(unused_imports)]
mod openapi_remote_worker_paths {
    pub use crate::server::live_router::{
        __path_create_remote_workers, __path_delete_remote_worker, __path_list_remote_workers,
        create_remote_workers, delete_remote_worker, list_remote_workers,
    };
}

#[allow(unused_imports)]
mod openapi_ro_crate_paths {
    pub use crate::server::live_router::{
        __path_create_ro_crate_entity, __path_delete_ro_crate_entities,
        __path_delete_ro_crate_entity, __path_get_ro_crate_entity, __path_list_ro_crate_entities,
        __path_update_ro_crate_entity, create_ro_crate_entity, delete_ro_crate_entities,
        delete_ro_crate_entity, get_ro_crate_entity, list_ro_crate_entities,
        update_ro_crate_entity,
    };
}

#[allow(unused_imports)]
mod openapi_task_paths {
    pub use crate::server::live_router::{__path_get_task, get_task};
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PingResponse {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VersionResponse {
    pub version: String,
    pub api_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_hash: Option<String>,
}

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
                format!("{} ({})", env!("CARGO_PKG_VERSION"), git_hash)
            },
            api_version: HTTP_API_VERSION.to_string(),
            git_hash: option_env!("GIT_HASH").unwrap_or("unknown").to_string(),
            access_control_enabled: false,
        }
    }
}

fn check_operation_id(
    source: &str,
    emitted: &Value,
    path: &str,
    method: &str,
    expected: &str,
    issues: &mut Vec<String>,
) {
    let source_operation_id = source_operation_id(source, path, method);
    let emitted_operation_id = emitted
        .get("paths")
        .and_then(|paths| paths.get(path))
        .and_then(|path_item| path_item.get(method))
        .and_then(|op| op.get("operationId"))
        .and_then(Value::as_str);

    if source_operation_id != Some(expected) {
        issues.push(format!(
            "source operationId mismatch for {} {}: expected {}, found {:?}",
            method, path, expected, source_operation_id
        ));
    }

    if emitted_operation_id != Some(expected) {
        issues.push(format!(
            "emitted operationId mismatch for {} {}: expected {}, found {:?}",
            method, path, expected, emitted_operation_id
        ));
    }
}

fn check_schema_properties(
    emitted: &Value,
    path: &str,
    method: &str,
    expected_properties: &[&str],
    issues: &mut Vec<String>,
) {
    let schema = emitted
        .get("paths")
        .and_then(|paths| paths.get(path))
        .and_then(|path_item| path_item.get(method))
        .and_then(|op| op.get("responses"))
        .and_then(|responses| responses.get("200"))
        .and_then(|response| response.get("content"))
        .and_then(|content| content.get("application/json"))
        .and_then(|json_content| json_content.get("schema"));

    let Some(schema) = schema else {
        issues.push(format!(
            "emitted schema missing for {} {} 200 response",
            method, path
        ));
        return;
    };

    let properties = resolve_schema_properties(emitted, schema);
    let Some(properties) = properties else {
        issues.push(format!(
            "unable to resolve emitted properties for {} {}",
            method, path
        ));
        return;
    };

    for property in expected_properties {
        if !properties.contains_key(*property) {
            issues.push(format!(
                "emitted schema for {} {} missing property {}",
                method, path, property
            ));
        }
    }
}

fn check_component_properties(
    document: &Value,
    schema_name: &str,
    expected_properties: &[&str],
    issues: &mut Vec<String>,
) {
    let properties = document
        .get("components")
        .and_then(|components| components.get("schemas"))
        .and_then(|schemas| schemas.get(schema_name))
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object);

    let Some(properties) = properties else {
        issues.push(format!("emitted component schema missing: {schema_name}"));
        return;
    };

    for property in expected_properties {
        if !properties.contains_key(*property) {
            issues.push(format!(
                "emitted component {schema_name} missing property {}",
                property
            ));
        }
    }
}

fn source_operation_id<'a>(source: &'a str, path: &str, method: &str) -> Option<&'a str> {
    let path_line = format!("  {path}:");
    let start = source.find(&path_line)?;
    let remaining = &source[start..];
    let end = remaining[1..]
        .find("\n  /")
        .map(|index| index + 1)
        .unwrap_or(remaining.len());
    let section = &remaining[..end];

    let mut current_method: Option<&str> = None;

    for line in section.lines() {
        if let Some(method_name) = line
            .strip_prefix("    ")
            .and_then(|rest| rest.strip_suffix(':'))
            .filter(|value| matches!(*value, "get" | "post" | "put" | "delete" | "patch"))
        {
            current_method = Some(method_name);
            continue;
        }

        if current_method == Some(method)
            && let Some(value) = line.trim().strip_prefix("operationId: ")
        {
            return Some(value.trim());
        }
    }

    None
}

fn resolve_schema_properties<'a>(
    document: &'a Value,
    schema: &'a Value,
) -> Option<&'a Map<String, Value>> {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let schema_name = reference.rsplit('/').next()?;
        return document
            .get("components")
            .and_then(|components| components.get("schemas"))
            .and_then(|schemas| schemas.get(schema_name))
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object);
    }

    schema.get("properties").and_then(Value::as_object)
}

#[derive(OpenApi)]
#[openapi(
    servers((url = "/torc-service/v1", description = "Versioned Torc API base path")),
    paths(
        openapi_access_control_paths::create_access_group,
        openapi_access_control_paths::list_access_groups,
        openapi_access_control_paths::get_access_group,
        openapi_access_control_paths::delete_access_group,
        openapi_access_control_paths::add_user_to_group,
        openapi_access_control_paths::list_group_members,
        openapi_access_control_paths::remove_user_from_group,
        openapi_access_control_paths::list_user_groups,
        openapi_access_control_paths::add_workflow_to_group,
        openapi_access_control_paths::list_workflow_groups,
        openapi_access_control_paths::remove_workflow_from_group,
        openapi_access_control_paths::check_workflow_access,
        openapi_system_paths::ping,
        openapi_system_paths::version,
        openapi_bulk_job_paths::create_jobs,
        openapi_compute_node_paths::create_compute_node,
        openapi_compute_node_paths::delete_compute_nodes,
        openapi_compute_node_paths::list_compute_nodes,
        openapi_compute_node_paths::delete_compute_node,
        openapi_compute_node_paths::get_compute_node,
        openapi_compute_node_paths::update_compute_node,
        openapi_event_paths::create_event,
        openapi_event_paths::delete_events,
        openapi_event_paths::list_events,
        openapi_event_paths::delete_event,
        openapi_event_paths::get_event,
        openapi_event_paths::update_event,
        openapi_file_paths::create_file,
        openapi_file_paths::delete_files,
        openapi_file_paths::list_files,
        openapi_file_paths::delete_file,
        openapi_file_paths::get_file,
        openapi_file_paths::update_file,
        openapi_job_paths::create_job,
        openapi_job_paths::delete_jobs,
        openapi_job_paths::list_jobs,
        openapi_job_paths::delete_job,
        openapi_job_paths::get_job,
        openapi_job_paths::update_job,
        openapi_job_paths::complete_job,
        openapi_job_paths::manage_status_change,
        openapi_job_paths::start_job,
        openapi_job_paths::retry_job,
        openapi_local_scheduler_paths::create_local_scheduler,
        openapi_local_scheduler_paths::delete_local_schedulers,
        openapi_local_scheduler_paths::list_local_schedulers,
        openapi_local_scheduler_paths::delete_local_scheduler,
        openapi_local_scheduler_paths::get_local_scheduler,
        openapi_local_scheduler_paths::update_local_scheduler,
        openapi_resource_requirements_paths::create_resource_requirements,
        openapi_resource_requirements_paths::delete_all_resource_requirements,
        openapi_resource_requirements_paths::list_resource_requirements,
        openapi_resource_requirements_paths::delete_resource_requirements,
        openapi_resource_requirements_paths::get_resource_requirements,
        openapi_resource_requirements_paths::update_resource_requirements,
        openapi_failure_handler_paths::create_failure_handler,
        openapi_failure_handler_paths::get_failure_handler,
        openapi_failure_handler_paths::delete_failure_handler,
        openapi_failure_handler_paths::list_failure_handlers,
        openapi_workflow_action_paths::create_workflow_action,
        openapi_workflow_action_paths::get_workflow_actions,
        openapi_workflow_action_paths::get_pending_actions,
        openapi_workflow_action_paths::claim_action,
        openapi_result_paths::create_result,
        openapi_result_paths::delete_results,
        openapi_result_paths::list_results,
        openapi_result_paths::delete_result,
        openapi_result_paths::get_result,
        openapi_result_paths::update_result,
        openapi_scheduled_compute_node_paths::create_scheduled_compute_node,
        openapi_scheduled_compute_node_paths::delete_scheduled_compute_nodes,
        openapi_scheduled_compute_node_paths::list_scheduled_compute_nodes,
        openapi_scheduled_compute_node_paths::delete_scheduled_compute_node,
        openapi_scheduled_compute_node_paths::get_scheduled_compute_node,
        openapi_scheduled_compute_node_paths::update_scheduled_compute_node,
        openapi_slurm_scheduler_paths::create_slurm_scheduler,
        openapi_slurm_scheduler_paths::delete_slurm_schedulers,
        openapi_slurm_scheduler_paths::list_slurm_schedulers,
        openapi_slurm_scheduler_paths::delete_slurm_scheduler,
        openapi_slurm_scheduler_paths::get_slurm_scheduler,
        openapi_slurm_scheduler_paths::update_slurm_scheduler,
        openapi_slurm_stats_paths::create_slurm_stats,
        openapi_slurm_stats_paths::list_slurm_stats,
        openapi_remote_worker_paths::create_remote_workers,
        openapi_remote_worker_paths::list_remote_workers,
        openapi_remote_worker_paths::delete_remote_worker,
        openapi_ro_crate_paths::create_ro_crate_entity,
        openapi_ro_crate_paths::get_ro_crate_entity,
        openapi_ro_crate_paths::update_ro_crate_entity,
        openapi_ro_crate_paths::delete_ro_crate_entity,
        openapi_ro_crate_paths::list_ro_crate_entities,
        openapi_ro_crate_paths::delete_ro_crate_entities,
        openapi_access_control_paths::reload_auth,
        openapi_workflow_paths::list_workflows,
        openapi_workflow_paths::create_workflow,
        openapi_workflow_paths::delete_workflow,
        openapi_workflow_paths::get_workflow,
        openapi_workflow_paths::update_workflow,
        openapi_workflow_paths::cancel_workflow,
        openapi_workflow_paths::initialize_jobs,
        openapi_workflow_paths::is_workflow_complete,
        openapi_workflow_paths::is_workflow_uninitialized,
        openapi_workflow_paths::reset_workflow_status,
        openapi_workflow_paths::reset_job_status,
        openapi_workflow_paths::get_workflow_status,
        openapi_workflow_paths::update_workflow_status,
        openapi_workflow_paths::claim_jobs_based_on_resources,
        openapi_workflow_paths::claim_next_jobs,
        openapi_workflow_paths::batch_complete_jobs,
        openapi_workflow_paths::list_job_dependencies,
        openapi_workflow_paths::list_job_file_relationships,
        openapi_workflow_paths::list_job_user_data_relationships,
        openapi_workflow_paths::list_job_ids,
        openapi_workflow_paths::list_missing_user_data,
        openapi_workflow_paths::process_changed_job_inputs,
        openapi_workflow_paths::get_ready_job_requirements,
        openapi_workflow_paths::list_required_existing_files,
        openapi_workflow_paths::get_active_task_for_workflow,
        openapi_user_data_paths::create_user_data,
        openapi_user_data_paths::delete_all_user_data,
        openapi_user_data_paths::list_user_data,
        openapi_user_data_paths::delete_user_data,
        openapi_user_data_paths::get_user_data,
        openapi_user_data_paths::update_user_data,
        openapi_task_paths::get_task
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
        BatchCompleteJobsRequest,
        BatchCompleteJobsResponse,
        JobCompletionEntry,
        JobCompletionError,
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
    doc
}

const ENV_VAR_NAME_PATTERN: &str = "^[A-Za-z_][A-Za-z0-9_]*$";

/// Inject `propertyNames.pattern` on the `env` map property of the named schemas so the
/// OpenAPI contract advertises the same key constraint the server enforces at runtime.
fn apply_env_property_name_pattern_json(value: &mut Value) {
    let Some(schemas) = value
        .get_mut("components")
        .and_then(|components| components.get_mut("schemas"))
        .and_then(|schemas| schemas.as_object_mut())
    else {
        return;
    };

    for schema_name in ["JobModel", "WorkflowModel"] {
        let Some(env) = schemas
            .get_mut(schema_name)
            .and_then(|schema| schema.get_mut("properties"))
            .and_then(|props| props.get_mut("env"))
            .and_then(|env| env.as_object_mut())
        else {
            continue;
        };

        let mut property_names = Map::new();
        property_names.insert("type".to_string(), Value::String("string".to_string()));
        property_names.insert(
            "pattern".to_string(),
            Value::String(ENV_VAR_NAME_PATTERN.to_string()),
        );
        env.insert("propertyNames".to_string(), Value::Object(property_names));
    }
}

fn apply_env_property_name_pattern_yaml(value: &mut serde_yaml::Value) {
    let Some(schemas) = value
        .get_mut("components")
        .and_then(|components| components.get_mut("schemas"))
        .and_then(|schemas| schemas.as_mapping_mut())
    else {
        return;
    };

    for schema_name in ["JobModel", "WorkflowModel"] {
        let Some(env) = schemas
            .get_mut(serde_yaml::Value::String(schema_name.to_string()))
            .and_then(|schema| schema.get_mut("properties"))
            .and_then(|props| props.get_mut("env"))
            .and_then(|env| env.as_mapping_mut())
        else {
            continue;
        };

        let mut property_names = serde_yaml::Mapping::new();
        property_names.insert(
            serde_yaml::Value::String("type".to_string()),
            serde_yaml::Value::String("string".to_string()),
        );
        property_names.insert(
            serde_yaml::Value::String("pattern".to_string()),
            serde_yaml::Value::String(ENV_VAR_NAME_PATTERN.to_string()),
        );
        env.insert(
            serde_yaml::Value::String("propertyNames".to_string()),
            serde_yaml::Value::Mapping(property_names),
        );
    }
}

pub fn openapi_value() -> Value {
    let mut value = serde_json::to_value(openapi_doc()).expect("OpenAPI document should serialize");
    apply_env_property_name_pattern_json(&mut value);
    value
}

pub fn render_openapi_yaml() -> Result<String, serde_yaml::Error> {
    let mut value = serde_yaml::to_value(openapi_doc())?;
    apply_env_property_name_pattern_yaml(&mut value);
    serde_yaml::to_string(&value)
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
        "/workflows/{id}/access_groups/{group_id}",
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
