use crate::models;
use crate::openapi_spec::{OpenApiAppState, PingResponse, VersionResponse};
use crate::server::api_contract::TransportApiCore;
use crate::server::auth::{SharedCredentialCache, SharedHtpasswd};
use crate::server::credential_cache::CredentialCache;
use crate::server::dashboard::serve_dashboard;
use crate::server::htpasswd::HtpasswdFile;
use crate::server::http_server::Server;
use crate::server::http_transport::*;
use crate::server::transport_types::auth_types::{AuthData, Authorization, Scopes, from_headers};
use crate::server::transport_types::context_types::{EmptyContext, Push, XSpanIdString};
use axum::Router;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::header::{HeaderName, HeaderValue};
use axum::http::{Response, StatusCode};
use axum::middleware::{self, Next};
use axum::routing::{delete, get, post, put};
use axum::{Extension, Json};
use parking_lot::RwLockReadGuard;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::sync::OnceLock;
use url::form_urlencoded;
use utoipa::IntoParams;

#[derive(Clone)]
pub struct LiveRouterState {
    pub openapi_state: OpenApiAppState,
    pub server: Server<EmptyContext>,
    pub auth: LiveAuthState,
}

#[derive(Clone)]
pub struct LiveAuthState {
    pub htpasswd: SharedHtpasswd,
    pub require_auth: bool,
    pub credential_cache: SharedCredentialCache,
}

macro_rules! path_handler {
    ($name:ident, $ptype:ty, |$path:pat_param, $server:ident, $request:ident, $context:ident| $body:block) => {
        async fn $name(
            State(state): State<LiveRouterState>,
            Path($path): Path<$ptype>,
            request: Request,
        ) -> Response<Body> {
            let $server = state.server.clone();
            let $request = request;
            let $context = request_context(&$request);
            $body
        }
    };
}

/// Default maximum allowed request body size for bulk job creation (200 MiB).
/// Override at runtime with TORC_MAX_REQUEST_BODY_MB (value in MiB).
const DEFAULT_MAX_BULK_REQUEST_BODY_BYTES: usize = 200 * 1024 * 1024;

fn max_bulk_request_body_bytes() -> usize {
    static CACHED: OnceLock<usize> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("TORC_MAX_REQUEST_BODY_MB")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .and_then(|mb| mb.checked_mul(1024 * 1024))
            .unwrap_or(DEFAULT_MAX_BULK_REQUEST_BODY_BYTES)
    })
}

pub fn app_router(state: LiveRouterState) -> Router {
    Router::new()
        .merge(
            Router::new()
                // Axum defaults JSON request bodies to 2 MiB, so bulk job creation needs an
                // explicit override to preserve large workflow submissions.
                .route("/torc-service/v1/bulk_jobs", post(create_jobs))
                .layer(DefaultBodyLimit::max(max_bulk_request_body_bytes())),
        )
        .route(
            "/torc-service/v1/access_groups",
            post(create_access_group).get(list_access_groups),
        )
        .route(
            "/torc-service/v1/access_groups/{id}",
            get(get_access_group).delete(delete_access_group),
        )
        .route(
            "/torc-service/v1/access_groups/{id}/members",
            post(add_user_to_group).get(list_group_members),
        )
        .route(
            "/torc-service/v1/access_groups/{id}/members/{user_name}",
            delete(remove_user_from_group),
        )
        .route(
            "/torc-service/v1/users/{user_name}/groups",
            get(list_user_groups),
        )
        .route(
            "/torc-service/v1/workflows/{id}/access_groups",
            get(list_workflow_groups),
        )
        .route(
            "/torc-service/v1/workflows/{id}/access_groups/{group_id}",
            post(add_workflow_to_group).delete(remove_workflow_from_group),
        )
        .route(
            "/torc-service/v1/access_check/{workflow_id}/{user_name}",
            get(check_workflow_access),
        )
        .route("/torc-service/v1/ping", get(ping))
        .route("/torc-service/v1/version", get(version))
        .route(
            "/torc-service/v1/compute_nodes",
            get(list_compute_nodes)
                .post(create_compute_node)
                .delete(delete_compute_nodes),
        )
        .route(
            "/torc-service/v1/compute_nodes/{id}",
            get(get_compute_node)
                .put(update_compute_node)
                .delete(delete_compute_node),
        )
        .route(
            "/torc-service/v1/events",
            get(list_events).post(create_event).delete(delete_events),
        )
        .route(
            "/torc-service/v1/events/{id}",
            get(get_event).put(update_event).delete(delete_event),
        )
        .route(
            "/torc-service/v1/files",
            get(list_files).post(create_file).delete(delete_files),
        )
        .route(
            "/torc-service/v1/files/{id}",
            get(get_file).put(update_file).delete(delete_file),
        )
        .route(
            "/torc-service/v1/local_schedulers",
            get(list_local_schedulers)
                .post(create_local_scheduler)
                .delete(delete_local_schedulers),
        )
        .route(
            "/torc-service/v1/local_schedulers/{id}",
            get(get_local_scheduler)
                .put(update_local_scheduler)
                .delete(delete_local_scheduler),
        )
        .route(
            "/torc-service/v1/resource_requirements",
            get(list_resource_requirements)
                .post(create_resource_requirements)
                .delete(delete_all_resource_requirements),
        )
        .route(
            "/torc-service/v1/resource_requirements/{id}",
            get(get_resource_requirements)
                .put(update_resource_requirements)
                .delete(delete_resource_requirements),
        )
        .route(
            "/torc-service/v1/failure_handlers",
            post(create_failure_handler),
        )
        .route(
            "/torc-service/v1/failure_handlers/{id}",
            get(get_failure_handler).delete(delete_failure_handler),
        )
        .route(
            "/torc-service/v1/workflows/{id}/failure_handlers",
            get(list_failure_handlers),
        )
        .route(
            "/torc-service/v1/workflows/{id}/actions",
            post(create_workflow_action).get(get_workflow_actions),
        )
        .route(
            "/torc-service/v1/workflows/{id}/actions/pending",
            get(get_pending_actions),
        )
        .route(
            "/torc-service/v1/workflows/{id}/actions/{action_id}/claim",
            post(claim_action),
        )
        .route(
            "/torc-service/v1/jobs",
            get(list_jobs).post(create_job).delete(delete_jobs),
        )
        .route(
            "/torc-service/v1/jobs/{id}",
            get(get_job).put(update_job).delete(delete_job),
        )
        .route(
            "/torc-service/v1/jobs/{id}/complete_job/{status}/{run_id}",
            post(complete_job),
        )
        .route(
            "/torc-service/v1/jobs/{id}/manage_status_change/{status}/{run_id}",
            put(manage_status_change),
        )
        .route(
            "/torc-service/v1/jobs/{id}/start_job/{run_id}/{compute_node_id}",
            put(start_job),
        )
        .route("/torc-service/v1/jobs/{id}/retry/{run_id}", post(retry_job))
        .route(
            "/torc-service/v1/user_data",
            get(list_user_data)
                .post(create_user_data)
                .delete(delete_all_user_data),
        )
        .route(
            "/torc-service/v1/user_data/{id}",
            get(get_user_data)
                .put(update_user_data)
                .delete(delete_user_data),
        )
        .route(
            "/torc-service/v1/results",
            get(list_results).post(create_result).delete(delete_results),
        )
        .route(
            "/torc-service/v1/results/{id}",
            get(get_result).put(update_result).delete(delete_result),
        )
        .route(
            "/torc-service/v1/scheduled_compute_nodes",
            get(list_scheduled_compute_nodes)
                .post(create_scheduled_compute_node)
                .delete(delete_scheduled_compute_nodes),
        )
        .route(
            "/torc-service/v1/scheduled_compute_nodes/{id}",
            get(get_scheduled_compute_node)
                .put(update_scheduled_compute_node)
                .delete(delete_scheduled_compute_node),
        )
        .route(
            "/torc-service/v1/slurm_schedulers",
            get(list_slurm_schedulers)
                .post(create_slurm_scheduler)
                .delete(delete_slurm_schedulers),
        )
        .route(
            "/torc-service/v1/slurm_schedulers/{id}",
            get(get_slurm_scheduler)
                .put(update_slurm_scheduler)
                .delete(delete_slurm_scheduler),
        )
        .route(
            "/torc-service/v1/slurm_stats",
            get(list_slurm_stats).post(create_slurm_stats),
        )
        .route(
            "/torc-service/v1/workflows/{id}/remote_workers",
            get(list_remote_workers).post(create_remote_workers),
        )
        .route(
            "/torc-service/v1/workflows/{id}/remote_workers/{worker}",
            delete(delete_remote_worker),
        )
        .route(
            "/torc-service/v1/ro_crate_entities",
            post(create_ro_crate_entity),
        )
        .route(
            "/torc-service/v1/ro_crate_entities/{id}",
            get(get_ro_crate_entity)
                .put(update_ro_crate_entity)
                .delete(delete_ro_crate_entity),
        )
        .route(
            "/torc-service/v1/workflows/{id}/ro_crate_entities",
            get(list_ro_crate_entities).delete(delete_ro_crate_entities),
        )
        .route("/torc-service/v1/admin/reload-auth", post(reload_auth))
        .route(
            "/torc-service/v1/workflows",
            get(list_workflows).post(create_workflow),
        )
        .route(
            "/torc-service/v1/workflows/{id}",
            get(get_workflow)
                .put(update_workflow)
                .delete(delete_workflow),
        )
        .route(
            "/torc-service/v1/workflows/{id}/cancel",
            put(cancel_workflow),
        )
        .route(
            "/torc-service/v1/workflows/{id}/initialize_jobs",
            post(initialize_jobs),
        )
        .route(
            "/torc-service/v1/workflows/{id}/is_complete",
            get(is_workflow_complete),
        )
        .route(
            "/torc-service/v1/workflows/{id}/is_uninitialized",
            get(is_workflow_uninitialized),
        )
        .route(
            "/torc-service/v1/workflows/{id}/reset_status",
            post(reset_workflow_status),
        )
        .route(
            "/torc-service/v1/workflows/{id}/reset_job_status",
            post(reset_job_status),
        )
        .route(
            "/torc-service/v1/workflows/{id}/status",
            get(get_workflow_status).put(update_workflow_status),
        )
        .route(
            "/torc-service/v1/workflows/{id}/claim_jobs_based_on_resources/{limit}",
            post(claim_jobs_based_on_resources),
        )
        .route(
            "/torc-service/v1/workflows/{id}/claim_next_jobs",
            post(claim_next_jobs),
        )
        .route(
            "/torc-service/v1/workflows/{id}/job_dependencies",
            get(list_job_dependencies),
        )
        .route(
            "/torc-service/v1/workflows/{id}/job_file_relationships",
            get(list_job_file_relationships),
        )
        .route(
            "/torc-service/v1/workflows/{id}/job_user_data_relationships",
            get(list_job_user_data_relationships),
        )
        .route("/torc-service/v1/workflows/{id}/job_ids", get(list_job_ids))
        .route(
            "/torc-service/v1/workflows/{id}/missing_user_data",
            get(list_missing_user_data),
        )
        .route(
            "/torc-service/v1/workflows/{id}/process_changed_job_inputs",
            post(process_changed_job_inputs),
        )
        .route(
            "/torc-service/v1/workflows/{id}/ready_job_requirements",
            get(get_ready_job_requirements),
        )
        .route(
            "/torc-service/v1/workflows/{id}/required_existing_files",
            get(list_required_existing_files),
        )
        .route(
            "/torc-service/v1/workflows/{id}/events/stream",
            get(workflow_events_stream_route),
        )
        .fallback(dashboard_fallback)
        .layer(middleware::from_fn_with_state(
            state.auth.clone(),
            inject_request_context,
        ))
        .with_state(state)
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct AccessPaginationQuery {
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PendingActionsQuery {
    #[param(nullable = true)]
    pub trigger_type: Option<Vec<String>>,
}

fn parse_pending_actions_query(query: Option<&str>) -> PendingActionsQuery {
    let trigger_type: Vec<String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .filter_map(|(key, value)| (key == "trigger_type").then_some(value))
        .collect();

    PendingActionsQuery {
        trigger_type: (!trigger_type.is_empty()).then_some(trigger_type),
    }
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct WorkflowsListQuery {
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
    #[param(nullable = true)]
    pub sort_by: Option<String>,
    #[param(nullable = true)]
    pub reverse_sort: Option<bool>,
    #[param(nullable = true)]
    pub name: Option<String>,
    #[param(nullable = true)]
    pub user: Option<String>,
    #[param(nullable = true)]
    pub description: Option<String>,
    #[param(nullable = true)]
    pub is_archived: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct InitializeJobsQuery {
    #[param(nullable = true)]
    pub only_uninitialized: Option<bool>,
    #[param(nullable = true)]
    pub clear_ephemeral_user_data: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ResetWorkflowStatusQuery {
    #[param(nullable = true)]
    pub force: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ResetJobStatusQuery {
    #[param(nullable = true)]
    pub failed_only: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ClaimJobsBasedOnResourcesQuery {
    #[param(nullable = true)]
    pub strict_scheduler_match: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ClaimNextJobsQuery {
    #[param(nullable = true)]
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct WorkflowRelationshipsQuery {
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
    #[param(nullable = true)]
    pub sort_by: Option<String>,
    #[param(nullable = true)]
    pub reverse_sort: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ProcessChangedJobInputsQuery {
    #[param(nullable = true)]
    pub dry_run: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ReadyJobRequirementsQuery {
    #[param(nullable = true)]
    pub scheduler_config_id: Option<i64>,
}

#[utoipa::path(
    get,
    tag = "system",
    path = "/ping",
    operation_id = "ping",
    responses((status = 200, body = PingResponse))
)]
pub async fn ping() -> Response<Body> {
    json_response(&PingResponse {
        status: "ok".to_string(),
    })
}

#[utoipa::path(
    get,
    tag = "system",
    path = "/version",
    operation_id = "get_version",
    responses((status = 200, body = VersionResponse))
)]
pub async fn version(State(state): State<LiveRouterState>) -> Response<Body> {
    json_response(&VersionResponse {
        version: state.openapi_state.version.clone(),
        api_version: state.openapi_state.api_version.clone(),
        git_hash: (!state.openapi_state.access_control_enabled)
            .then_some(state.openapi_state.git_hash.clone()),
    })
}

#[utoipa::path(
    post,
    tag = "access_control",
    path = "/admin/reload-auth",
    operation_id = "reload_auth",
    responses((status = 200, body = models::ReloadAuthResponse))
)]
pub async fn reload_auth(
    State(state): State<LiveRouterState>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.reload_auth(&context).await {
        Ok(response) => reload_auth_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn dashboard_fallback(request: Request) -> Response<Body> {
    serve_dashboard(request.uri().path()).unwrap_or_else(not_found_response)
}

#[utoipa::path(
    post,
    tag = "jobs",
    path = "/bulk_jobs",
    operation_id = "create_jobs",
    request_body = models::JobsModel,
    responses((status = 200, body = models::CreateJobsResponse))
)]
pub async fn create_jobs(
    State(state): State<LiveRouterState>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::JobsModel>,
) -> Response<Body> {
    match state.server.create_jobs(body, &context).await {
        Ok(response) => create_jobs_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "access_control",
    path = "/access_groups",
    operation_id = "create_access_group",
    request_body = models::AccessGroupModel,
    responses((status = 200, body = models::AccessGroupModel))
)]
pub async fn create_access_group(
    State(state): State<LiveRouterState>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::AccessGroupModel>,
) -> Response<Body> {
    match state.server.create_access_group(body, &context).await {
        Ok(response) => create_access_group_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "access_control",
    path = "/access_groups",
    operation_id = "list_access_groups",
    params(AccessPaginationQuery),
    responses((status = 200, body = models::ListAccessGroupsResponse))
)]
pub async fn list_access_groups(
    State(state): State<LiveRouterState>,
    Query(query): Query<AccessPaginationQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_access_groups(query.offset, query.limit, &context)
        .await
    {
        Ok(response) => list_access_groups_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "access_control",
    path = "/access_groups/{id}",
    operation_id = "get_access_group",
    params(("id" = i64, Path, description = "Access group ID")),
    responses((status = 200, body = models::AccessGroupModel))
)]
pub async fn get_access_group(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.get_access_group(id, &context).await {
        Ok(response) => get_access_group_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "access_control",
    path = "/access_groups/{id}",
    operation_id = "delete_access_group",
    params(("id" = i64, Path, description = "Access group ID")),
    responses((status = 200, body = models::AccessGroupModel))
)]
pub async fn delete_access_group(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.delete_access_group(id, &context).await {
        Ok(response) => delete_access_group_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "access_control",
    path = "/access_groups/{id}/members",
    operation_id = "add_user_to_group",
    params(("id" = i64, Path, description = "Access group ID")),
    request_body = models::UserGroupMembershipModel,
    responses((status = 200, body = models::UserGroupMembershipModel))
)]
pub async fn add_user_to_group(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::UserGroupMembershipModel>,
) -> Response<Body> {
    match state.server.add_user_to_group(id, body, &context).await {
        Ok(response) => add_user_to_group_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "access_control",
    path = "/access_groups/{id}/members",
    operation_id = "list_group_members",
    params(("id" = i64, Path, description = "Access group ID"), AccessPaginationQuery),
    responses((status = 200, body = models::ListUserGroupMembershipsResponse))
)]
pub async fn list_group_members(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Query(query): Query<AccessPaginationQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_group_members(id, query.offset, query.limit, &context)
        .await
    {
        Ok(response) => list_group_members_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "access_control",
    path = "/access_groups/{id}/members/{user_name}",
    operation_id = "remove_user_from_group",
    params(
        ("id" = i64, Path, description = "Access group ID"),
        ("user_name" = String, Path, description = "Username")
    ),
    responses((status = 200, body = models::UserGroupMembershipModel))
)]
pub async fn remove_user_from_group(
    State(state): State<LiveRouterState>,
    Path((group_id, user_name)): Path<(i64, String)>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .remove_user_from_group(group_id, user_name, &context)
        .await
    {
        Ok(response) => remove_user_from_group_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "access_control",
    path = "/users/{user_name}/groups",
    operation_id = "list_user_groups",
    params(
        ("user_name" = String, Path, description = "Username"),
        AccessPaginationQuery
    ),
    responses((status = 200, body = models::ListAccessGroupsResponse))
)]
pub async fn list_user_groups(
    State(state): State<LiveRouterState>,
    Path(user_name): Path<String>,
    Query(query): Query<AccessPaginationQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_user_groups(user_name, query.offset, query.limit, &context)
        .await
    {
        Ok(response) => list_user_groups_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "access_control",
    path = "/workflows/{id}/access_groups/{group_id}",
    operation_id = "add_workflow_to_group",
    params(
        ("id" = i64, Path, description = "Workflow ID"),
        ("group_id" = i64, Path, description = "Access group ID")
    ),
    responses((status = 200, body = models::WorkflowAccessGroupModel))
)]
pub async fn add_workflow_to_group(
    State(state): State<LiveRouterState>,
    Path((workflow_id, group_id)): Path<(i64, i64)>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .add_workflow_to_group(workflow_id, group_id, &context)
        .await
    {
        Ok(response) => add_workflow_to_group_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "access_control",
    path = "/workflows/{id}/access_groups",
    operation_id = "list_workflow_groups",
    params(("id" = i64, Path, description = "Workflow ID"), AccessPaginationQuery),
    responses((status = 200, body = models::ListAccessGroupsResponse))
)]
pub async fn list_workflow_groups(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Query(query): Query<AccessPaginationQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_workflow_groups(id, query.offset, query.limit, &context)
        .await
    {
        Ok(response) => list_workflow_groups_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "access_control",
    path = "/workflows/{id}/access_groups/{group_id}",
    operation_id = "remove_workflow_from_group",
    params(
        ("id" = i64, Path, description = "Workflow ID"),
        ("group_id" = i64, Path, description = "Access group ID")
    ),
    responses((status = 200, body = models::WorkflowAccessGroupModel))
)]
pub async fn remove_workflow_from_group(
    State(state): State<LiveRouterState>,
    Path((workflow_id, group_id)): Path<(i64, i64)>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .remove_workflow_from_group(workflow_id, group_id, &context)
        .await
    {
        Ok(response) => remove_workflow_from_group_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "access_control",
    path = "/access_check/{workflow_id}/{user_name}",
    operation_id = "check_workflow_access",
    params(
        ("workflow_id" = i64, Path, description = "Workflow ID"),
        ("user_name" = String, Path, description = "Username")
    ),
    responses((status = 200, body = models::AccessCheckResponse))
)]
pub async fn check_workflow_access(
    State(state): State<LiveRouterState>,
    Path((workflow_id, user_name)): Path<(i64, String)>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .check_workflow_access(workflow_id, user_name, &context)
        .await
    {
        Ok(response) => check_workflow_access_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ComputeNodesQuery {
    pub workflow_id: i64,
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
    #[param(nullable = true)]
    pub sort_by: Option<String>,
    #[param(nullable = true)]
    pub reverse_sort: Option<bool>,
    #[param(nullable = true)]
    pub hostname: Option<String>,
    #[param(nullable = true)]
    pub is_active: Option<bool>,
    #[param(nullable = true)]
    pub scheduled_compute_node_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct DeleteComputeNodesQuery {
    pub workflow_id: i64,
}

#[utoipa::path(
    get,
    tag = "compute_nodes",
    path = "/compute_nodes",
    operation_id = "list_compute_nodes",
    params(ComputeNodesQuery),
    responses(
        (status = 200, description = "Successful response", body = models::ListComputeNodesResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn list_compute_nodes(
    State(state): State<LiveRouterState>,
    Query(query): Query<ComputeNodesQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_compute_nodes(
            query.workflow_id,
            query.offset,
            query.limit,
            query.sort_by,
            query.reverse_sort,
            query.hostname,
            query.is_active,
            query.scheduled_compute_node_id,
            &context,
        )
        .await
    {
        Ok(response) => list_compute_nodes_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "compute_nodes",
    path = "/compute_nodes",
    operation_id = "create_compute_node",
    request_body = models::ComputeNodeModel,
    responses(
        (status = 200, description = "Successful response", body = models::ComputeNodeModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn create_compute_node(
    State(state): State<LiveRouterState>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::ComputeNodeModel>,
) -> Response<Body> {
    match state.server.create_compute_node(body, &context).await {
        Ok(response) => create_compute_node_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "compute_nodes",
    path = "/compute_nodes",
    operation_id = "delete_compute_nodes",
    params(DeleteComputeNodesQuery),
    responses(
        (status = 200, description = "Successful response", body = models::DeleteCountResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_compute_nodes(
    State(state): State<LiveRouterState>,
    Query(query): Query<DeleteComputeNodesQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .delete_compute_nodes(query.workflow_id, &context)
        .await
    {
        Ok(response) => delete_compute_nodes_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "compute_nodes",
    path = "/compute_nodes/{id}",
    operation_id = "get_compute_node",
    params(("id" = i64, Path, description = "ID of the compute node record")),
    responses(
        (status = 200, description = "Successful response", body = models::ComputeNodeModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn get_compute_node(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.get_compute_node(id, &context).await {
        Ok(response) => get_compute_node_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    put,
    tag = "compute_nodes",
    path = "/compute_nodes/{id}",
    operation_id = "update_compute_node",
    params(("id" = i64, Path, description = "ID of the compute node.")),
    request_body = models::ComputeNodeModel,
    responses(
        (status = 200, description = "Successful response", body = models::ComputeNodeModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn update_compute_node(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::ComputeNodeModel>,
) -> Response<Body> {
    match state.server.update_compute_node(id, body, &context).await {
        Ok(response) => update_compute_node_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "compute_nodes",
    path = "/compute_nodes/{id}",
    operation_id = "delete_compute_node",
    params(("id" = i64, Path, description = "Compute node ID")),
    responses(
        (status = 200, description = "Successful response", body = models::ComputeNodeModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_compute_node(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.delete_compute_node(id, &context).await {
        Ok(response) => delete_compute_node_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct EventsQuery {
    pub workflow_id: i64,
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
    #[param(nullable = true)]
    pub sort_by: Option<String>,
    #[param(nullable = true)]
    pub reverse_sort: Option<bool>,
    #[param(nullable = true)]
    pub category: Option<String>,
    #[param(nullable = true)]
    pub after_timestamp: Option<i64>,
}

#[utoipa::path(
    get,
    tag = "events",
    path = "/events",
    operation_id = "list_events",
    params(EventsQuery),
    responses(
        (status = 200, description = "Successful response", body = models::ListEventsResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn list_events(
    State(state): State<LiveRouterState>,
    Query(query): Query<EventsQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_events(
            query.workflow_id,
            query.offset,
            query.limit,
            query.sort_by,
            query.reverse_sort,
            query.category,
            query.after_timestamp,
            &context,
        )
        .await
    {
        Ok(response) => list_events_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "events",
    path = "/events",
    operation_id = "create_event",
    request_body = models::EventModel,
    responses(
        (status = 200, description = "Successful response", body = models::EventModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn create_event(
    State(state): State<LiveRouterState>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::EventModel>,
) -> Response<Body> {
    match state.server.create_event(body, &context).await {
        Ok(response) => create_event_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "events",
    path = "/events",
    operation_id = "delete_events",
    params(DeleteComputeNodesQuery),
    responses(
        (status = 200, description = "Successful response", body = Value),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_events(
    State(state): State<LiveRouterState>,
    Query(query): Query<DeleteComputeNodesQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .delete_events(query.workflow_id, &context)
        .await
    {
        Ok(response) => delete_events_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "events",
    path = "/events/{id}",
    operation_id = "get_event",
    params(("id" = i64, Path, description = "ID of the event record.")),
    responses(
        (status = 200, description = "Successful response", body = models::EventModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn get_event(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.get_event(id, &context).await {
        Ok(response) => get_event_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    put,
    tag = "events",
    path = "/events/{id}",
    operation_id = "update_event",
    params(("id" = i64, Path, description = "ID of the event.")),
    request_body = Value,
    responses(
        (status = 200, description = "Successful response", body = models::EventModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn update_event(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<Value>,
) -> Response<Body> {
    match state.server.update_event(id, body, &context).await {
        Ok(response) => update_event_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "events",
    path = "/events/{id}",
    operation_id = "delete_event",
    params(("id" = i64, Path, description = "ID of the event record.")),
    responses(
        (status = 200, description = "Successful response", body = models::EventModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_event(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.delete_event(id, &context).await {
        Ok(response) => delete_event_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct FilesQuery {
    pub workflow_id: i64,
    #[param(nullable = true)]
    pub produced_by_job_id: Option<i64>,
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
    #[param(nullable = true)]
    pub sort_by: Option<String>,
    #[param(nullable = true)]
    pub reverse_sort: Option<bool>,
    #[param(nullable = true)]
    pub name: Option<String>,
    #[param(nullable = true)]
    pub path: Option<String>,
    #[param(nullable = true)]
    pub is_output: Option<bool>,
}

#[utoipa::path(
    get,
    tag = "files",
    path = "/files",
    operation_id = "list_files",
    params(FilesQuery),
    responses(
        (status = 200, description = "Successful response", body = models::ListFilesResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn list_files(
    State(state): State<LiveRouterState>,
    Query(query): Query<FilesQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_files(
            query.workflow_id,
            query.produced_by_job_id,
            query.offset,
            query.limit,
            query.sort_by,
            query.reverse_sort,
            query.name,
            query.path,
            query.is_output,
            &context,
        )
        .await
    {
        Ok(response) => list_files_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "files",
    path = "/files",
    operation_id = "create_file",
    request_body = models::FileModel,
    responses(
        (status = 200, description = "Successful response", body = models::FileModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn create_file(
    State(state): State<LiveRouterState>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::FileModel>,
) -> Response<Body> {
    match state.server.create_file(body, &context).await {
        Ok(response) => create_file_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "files",
    path = "/files",
    operation_id = "delete_files",
    params(DeleteComputeNodesQuery),
    responses(
        (status = 200, description = "Successful response", body = models::DeleteCountResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_files(
    State(state): State<LiveRouterState>,
    Query(query): Query<DeleteComputeNodesQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.delete_files(query.workflow_id, &context).await {
        Ok(response) => delete_files_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "files",
    path = "/files/{id}",
    operation_id = "get_file",
    params(("id" = i64, Path, description = "ID of the file record")),
    responses(
        (status = 200, description = "Successful response", body = models::FileModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn get_file(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.get_file(id, &context).await {
        Ok(response) => get_file_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    put,
    tag = "files",
    path = "/files/{id}",
    operation_id = "update_file",
    params(("id" = i64, Path, description = "ID of the file.")),
    request_body = models::FileModel,
    responses(
        (status = 200, description = "Successful response", body = models::FileModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn update_file(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::FileModel>,
) -> Response<Body> {
    match state.server.update_file(id, body, &context).await {
        Ok(response) => update_file_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "files",
    path = "/files/{id}",
    operation_id = "delete_file",
    params(("id" = i64, Path, description = "File ID")),
    responses(
        (status = 200, description = "Successful response", body = models::FileModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_file(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.delete_file(id, &context).await {
        Ok(response) => delete_file_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct LocalSchedulersQuery {
    pub workflow_id: i64,
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
    #[param(nullable = true)]
    pub sort_by: Option<String>,
    #[param(nullable = true)]
    pub reverse_sort: Option<bool>,
    #[param(nullable = true)]
    pub memory: Option<String>,
    #[param(nullable = true)]
    pub num_cpus: Option<i64>,
}

#[utoipa::path(
    get,
    tag = "local_schedulers",
    path = "/local_schedulers",
    operation_id = "list_local_schedulers",
    params(LocalSchedulersQuery),
    responses(
        (status = 200, description = "Successful response", body = models::ListLocalSchedulersResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn list_local_schedulers(
    State(state): State<LiveRouterState>,
    Query(query): Query<LocalSchedulersQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_local_schedulers(
            query.workflow_id,
            query.offset,
            query.limit,
            query.sort_by,
            query.reverse_sort,
            query.memory,
            query.num_cpus,
            &context,
        )
        .await
    {
        Ok(response) => list_local_schedulers_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "local_schedulers",
    path = "/local_schedulers",
    operation_id = "create_local_scheduler",
    request_body = models::LocalSchedulerModel,
    responses(
        (status = 200, description = "Successful response", body = models::LocalSchedulerModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn create_local_scheduler(
    State(state): State<LiveRouterState>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::LocalSchedulerModel>,
) -> Response<Body> {
    match state.server.create_local_scheduler(body, &context).await {
        Ok(response) => create_local_scheduler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "local_schedulers",
    path = "/local_schedulers",
    operation_id = "delete_local_schedulers",
    params(DeleteComputeNodesQuery),
    responses(
        (status = 200, description = "Successful response", body = models::DeleteCountResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_local_schedulers(
    State(state): State<LiveRouterState>,
    Query(query): Query<DeleteComputeNodesQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .delete_local_schedulers(query.workflow_id, &context)
        .await
    {
        Ok(response) => delete_local_schedulers_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "local_schedulers",
    path = "/local_schedulers/{id}",
    operation_id = "get_local_scheduler",
    params(("id" = i64, Path, description = "ID of the local scheduler record")),
    responses(
        (status = 200, description = "Successful response", body = models::LocalSchedulerModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn get_local_scheduler(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.get_local_scheduler(id, &context).await {
        Ok(response) => get_local_scheduler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    put,
    tag = "local_schedulers",
    path = "/local_schedulers/{id}",
    operation_id = "update_local_scheduler",
    params(("id" = i64, Path, description = "ID of the local scheduler.")),
    request_body = models::LocalSchedulerModel,
    responses(
        (status = 200, description = "Successful response", body = models::LocalSchedulerModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn update_local_scheduler(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::LocalSchedulerModel>,
) -> Response<Body> {
    match state
        .server
        .update_local_scheduler(id, body, &context)
        .await
    {
        Ok(response) => update_local_scheduler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "local_schedulers",
    path = "/local_schedulers/{id}",
    operation_id = "delete_local_scheduler",
    params(("id" = i64, Path, description = "Local scheduler ID")),
    responses(
        (status = 200, description = "Successful response", body = models::LocalSchedulerModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_local_scheduler(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.delete_local_scheduler(id, &context).await {
        Ok(response) => delete_local_scheduler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ResourceRequirementsQuery {
    pub workflow_id: i64,
    #[param(nullable = true)]
    pub job_id: Option<i64>,
    #[param(nullable = true)]
    pub name: Option<String>,
    #[param(nullable = true)]
    pub memory: Option<String>,
    #[param(nullable = true)]
    pub num_cpus: Option<i64>,
    #[param(nullable = true)]
    pub num_gpus: Option<i64>,
    #[param(nullable = true)]
    pub num_nodes: Option<i64>,
    #[param(nullable = true)]
    pub runtime: Option<i64>,
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
    #[param(nullable = true)]
    pub sort_by: Option<String>,
    #[param(nullable = true)]
    pub reverse_sort: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct FailureHandlersListQuery {
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct SlurmStatsQuery {
    pub workflow_id: i64,
    #[param(nullable = true)]
    pub job_id: Option<i64>,
    #[param(nullable = true)]
    pub run_id: Option<i64>,
    #[param(nullable = true)]
    pub attempt_id: Option<i64>,
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
}

#[utoipa::path(
    post,
    tag = "resource_requirements",
    path = "/resource_requirements",
    operation_id = "create_resource_requirements",
    request_body = models::ResourceRequirementsModel,
    responses(
        (status = 200, description = "Successful response", body = models::ResourceRequirementsModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 422, description = "Unprocessable content", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn create_resource_requirements(
    State(state): State<LiveRouterState>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::ResourceRequirementsModel>,
) -> Response<Body> {
    match state
        .server
        .create_resource_requirements(body, &context)
        .await
    {
        Ok(response) => create_resource_requirements_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "resource_requirements",
    path = "/resource_requirements",
    operation_id = "list_resource_requirements",
    params(ResourceRequirementsQuery),
    responses(
        (status = 200, description = "Successful response", body = models::ListResourceRequirementsResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn list_resource_requirements(
    State(state): State<LiveRouterState>,
    Query(query): Query<ResourceRequirementsQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_resource_requirements(
            query.workflow_id,
            query.job_id,
            query.name,
            query.memory,
            query.num_cpus,
            query.num_gpus,
            query.num_nodes,
            query.runtime,
            query.offset,
            query.limit,
            query.sort_by,
            query.reverse_sort,
            &context,
        )
        .await
    {
        Ok(response) => list_resource_requirements_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "resource_requirements",
    path = "/resource_requirements",
    operation_id = "delete_resource_requirements",
    params(DeleteComputeNodesQuery),
    responses(
        (status = 200, description = "Successful response", body = Value),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_all_resource_requirements(
    State(state): State<LiveRouterState>,
    Query(query): Query<DeleteComputeNodesQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .delete_all_resource_requirements(query.workflow_id, &context)
        .await
    {
        Ok(response) => delete_all_resource_requirements_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "resource_requirements",
    path = "/resource_requirements/{id}",
    operation_id = "get_resource_requirements",
    params(("id" = i64, Path, description = "Resource requirements ID")),
    responses(
        (status = 200, description = "Successful response", body = models::ResourceRequirementsModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn get_resource_requirements(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.get_resource_requirements(id, &context).await {
        Ok(response) => get_resource_requirements_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    put,
    tag = "resource_requirements",
    path = "/resource_requirements/{id}",
    operation_id = "update_resource_requirements",
    params(("id" = i64, Path, description = "Resource requirements ID")),
    request_body = models::ResourceRequirementsModel,
    responses(
        (status = 200, description = "Successful response", body = models::ResourceRequirementsModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 422, description = "Unprocessable content", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn update_resource_requirements(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::ResourceRequirementsModel>,
) -> Response<Body> {
    match state
        .server
        .update_resource_requirements(id, body, &context)
        .await
    {
        Ok(response) => update_resource_requirements_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "resource_requirements",
    path = "/resource_requirements/{id}",
    operation_id = "delete_resource_requirement",
    params(("id" = i64, Path, description = "Resource requirements ID")),
    responses(
        (status = 200, description = "Successful response", body = models::ResourceRequirementsModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_resource_requirements(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .delete_resource_requirements(id, &context)
        .await
    {
        Ok(response) => delete_resource_requirements_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "failure_handlers",
    path = "/failure_handlers",
    operation_id = "create_failure_handler",
    request_body = models::FailureHandlerModel,
    responses(
        (status = 200, description = "Successful response", body = models::FailureHandlerModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn create_failure_handler(
    State(state): State<LiveRouterState>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::FailureHandlerModel>,
) -> Response<Body> {
    match state.server.create_failure_handler(body, &context).await {
        Ok(response) => create_failure_handler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "failure_handlers",
    path = "/failure_handlers/{id}",
    operation_id = "get_failure_handler",
    params(("id" = i64, Path, description = "Failure handler ID")),
    responses(
        (status = 200, description = "Successful response", body = models::FailureHandlerModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn get_failure_handler(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.get_failure_handler(id, &context).await {
        Ok(response) => get_failure_handler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "failure_handlers",
    path = "/failure_handlers/{id}",
    operation_id = "delete_failure_handler",
    params(("id" = i64, Path, description = "Failure handler ID")),
    responses(
        (status = 200, description = "Successful response", body = models::FailureHandlerModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_failure_handler(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.delete_failure_handler(id, &context).await {
        Ok(response) => delete_failure_handler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "failure_handlers",
    path = "/workflows/{id}/failure_handlers",
    operation_id = "list_failure_handlers",
    params(("id" = i64, Path, description = "Workflow ID"), FailureHandlersListQuery),
    responses(
        (status = 200, description = "Successful response", body = models::ListFailureHandlersResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn list_failure_handlers(
    State(state): State<LiveRouterState>,
    Path(workflow_id): Path<i64>,
    Query(query): Query<FailureHandlersListQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_failure_handlers(workflow_id, query.offset, query.limit, &context)
        .await
    {
        Ok(response) => list_failure_handlers_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "slurm_stats",
    path = "/slurm_stats",
    operation_id = "create_slurm_stats",
    request_body = models::SlurmStatsModel,
    responses(
        (status = 200, description = "Successful response", body = models::SlurmStatsModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn create_slurm_stats(
    State(state): State<LiveRouterState>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::SlurmStatsModel>,
) -> Response<Body> {
    match state.server.create_slurm_stats(body, &context).await {
        Ok(response) => create_slurm_stats_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "slurm_stats",
    path = "/slurm_stats",
    operation_id = "list_slurm_stats",
    params(SlurmStatsQuery),
    responses(
        (status = 200, description = "Successful response", body = models::ListSlurmStatsResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn list_slurm_stats(
    State(state): State<LiveRouterState>,
    Query(query): Query<SlurmStatsQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_slurm_stats(
            query.workflow_id,
            query.job_id,
            query.run_id,
            query.attempt_id,
            query.offset,
            query.limit,
            &context,
        )
        .await
    {
        Ok(response) => list_slurm_stats_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct JobsListQuery {
    pub workflow_id: i64,
    #[param(nullable = true)]
    pub status: Option<models::JobStatus>,
    #[param(nullable = true)]
    pub needs_file_id: Option<i64>,
    #[param(nullable = true)]
    pub upstream_job_id: Option<i64>,
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
    #[param(nullable = true)]
    pub sort_by: Option<String>,
    #[param(nullable = true)]
    pub reverse_sort: Option<bool>,
    #[param(nullable = true)]
    pub include_relationships: Option<bool>,
    #[param(nullable = true)]
    pub active_compute_node_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct DeleteJobsQuery {
    pub workflow_id: i64,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct RetryJobQuery {
    pub max_retries: i32,
}

#[utoipa::path(
    get,
    tag = "jobs",
    path = "/jobs",
    operation_id = "list_jobs",
    params(JobsListQuery),
    responses(
        (status = 200, description = "Successful response", body = models::ListJobsResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn list_jobs(
    State(state): State<LiveRouterState>,
    Query(query): Query<JobsListQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_jobs(
            query.workflow_id,
            query.status,
            query.needs_file_id,
            query.upstream_job_id,
            query.offset,
            query.limit,
            query.sort_by,
            query.reverse_sort,
            query.include_relationships,
            query.active_compute_node_id,
            &context,
        )
        .await
    {
        Ok(response) => list_jobs_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "jobs",
    path = "/jobs",
    operation_id = "create_job",
    request_body = models::JobModel,
    responses(
        (status = 200, description = "Successful response", body = models::JobModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 422, description = "Unprocessable content (e.g., invalid priority)", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn create_job(
    State(state): State<LiveRouterState>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::JobModel>,
) -> Response<Body> {
    match state.server.create_job(body, &context).await {
        Ok(response) => create_job_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "jobs",
    path = "/jobs",
    operation_id = "delete_jobs",
    params(DeleteJobsQuery),
    responses(
        (status = 200, description = "Successful response", body = models::DeleteCountResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_jobs(
    State(state): State<LiveRouterState>,
    Query(query): Query<DeleteJobsQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.delete_jobs(query.workflow_id, &context).await {
        Ok(response) => delete_jobs_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "jobs",
    path = "/jobs/{id}",
    operation_id = "get_job",
    params(("id" = i64, Path, description = "ID of the job record")),
    responses(
        (status = 200, description = "Successful response", body = models::JobModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn get_job(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.get_job(id, &context).await {
        Ok(response) => get_job_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    put,
    tag = "jobs",
    path = "/jobs/{id}",
    operation_id = "update_job",
    params(("id" = i64, Path, description = "ID of the job.")),
    request_body = models::JobModel,
    responses(
        (status = 200, description = "Successful response", body = models::JobModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn update_job(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::JobModel>,
) -> Response<Body> {
    match state.server.update_job(id, body, &context).await {
        Ok(response) => update_job_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "jobs",
    path = "/jobs/{id}",
    operation_id = "delete_job",
    params(("id" = i64, Path, description = "Job ID")),
    responses(
        (status = 200, description = "Successful response", body = models::JobModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_job(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.delete_job(id, &context).await {
        Ok(response) => delete_job_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "jobs",
    path = "/jobs/{id}/complete_job/{status}/{run_id}",
    operation_id = "complete_job",
    params(
        ("id" = i64, Path, description = "Job ID"),
        ("status" = models::JobStatus, Path, description = "New job status."),
        ("run_id" = i64, Path, description = "Current job run ID")
    ),
    request_body = models::ResultModel,
    responses(
        (status = 200, description = "Successful response", body = models::JobModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn complete_job(
    State(state): State<LiveRouterState>,
    Path((id, status, run_id)): Path<(i64, models::JobStatus, i64)>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::ResultModel>,
) -> Response<Body> {
    match state
        .server
        .complete_job(id, status, run_id, body, &context)
        .await
    {
        Ok(response) => complete_job_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    put,
    tag = "jobs",
    path = "/jobs/{id}/manage_status_change/{status}/{run_id}",
    operation_id = "manage_status_change",
    params(
        ("id" = i64, Path, description = "Job ID"),
        ("status" = models::JobStatus, Path, description = "New job status"),
        ("run_id" = i64, Path, description = "Current job run ID")
    ),
    responses(
        (status = 200, description = "Successful response", body = models::JobModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn manage_status_change(
    State(state): State<LiveRouterState>,
    Path((id, status, run_id)): Path<(i64, models::JobStatus, i64)>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .manage_status_change(id, status, run_id, &context)
        .await
    {
        Ok(response) => manage_status_change_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    put,
    tag = "jobs",
    path = "/jobs/{id}/start_job/{run_id}/{compute_node_id}",
    operation_id = "start_job",
    params(
        ("id" = i64, Path, description = "Job ID"),
        ("run_id" = i64, Path, description = "Current job run ID"),
        ("compute_node_id" = i64, Path, description = "Compute node ID that started the job")
    ),
    responses(
        (status = 200, description = "Successful response", body = models::JobModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn start_job(
    State(state): State<LiveRouterState>,
    Path((id, run_id, compute_node_id)): Path<(i64, i64, i64)>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .start_job(id, run_id, compute_node_id, &context)
        .await
    {
        Ok(response) => start_job_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "jobs",
    path = "/jobs/{id}/retry/{run_id}",
    operation_id = "retry_job",
    params(
        ("id" = i64, Path, description = "Job ID"),
        ("run_id" = i64, Path, description = "Current workflow run ID"),
        RetryJobQuery
    ),
    responses(
        (status = 200, description = "Successful response", body = models::JobModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 422, description = "Unprocessable content", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn retry_job(
    State(state): State<LiveRouterState>,
    Path((id, run_id)): Path<(i64, i64)>,
    Query(query): Query<RetryJobQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .retry_job(id, run_id, query.max_retries, &context)
        .await
    {
        Ok(response) => retry_job_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct CreateUserDataQuery {
    #[param(nullable = true)]
    pub consumer_job_id: Option<i64>,
    #[param(nullable = true)]
    pub producer_job_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct UserDataQuery {
    pub workflow_id: i64,
    #[param(nullable = true)]
    pub consumer_job_id: Option<i64>,
    #[param(nullable = true)]
    pub producer_job_id: Option<i64>,
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
    #[param(nullable = true)]
    pub sort_by: Option<String>,
    #[param(nullable = true)]
    pub reverse_sort: Option<bool>,
    #[param(nullable = true)]
    pub name: Option<String>,
    #[param(nullable = true)]
    pub is_ephemeral: Option<bool>,
}

#[utoipa::path(
    get,
    tag = "user_data",
    path = "/user_data",
    operation_id = "list_user_data",
    params(UserDataQuery),
    responses(
        (status = 200, description = "Successful response", body = models::ListUserDataResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn list_user_data(
    State(state): State<LiveRouterState>,
    Query(query): Query<UserDataQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_user_data(
            query.workflow_id,
            query.consumer_job_id,
            query.producer_job_id,
            query.offset,
            query.limit,
            query.sort_by,
            query.reverse_sort,
            query.name,
            query.is_ephemeral,
            &context,
        )
        .await
    {
        Ok(response) => list_user_data_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "user_data",
    path = "/user_data",
    operation_id = "create_user_data",
    params(CreateUserDataQuery),
    request_body = models::UserDataModel,
    responses(
        (status = 200, description = "Successful response", body = models::UserDataModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn create_user_data(
    State(state): State<LiveRouterState>,
    Query(query): Query<CreateUserDataQuery>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::UserDataModel>,
) -> Response<Body> {
    match state
        .server
        .create_user_data(body, query.consumer_job_id, query.producer_job_id, &context)
        .await
    {
        Ok(response) => create_user_data_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "user_data",
    path = "/user_data",
    operation_id = "delete_all_user_data",
    params(DeleteComputeNodesQuery),
    responses(
        (status = 200, description = "Successful response", body = Value),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_all_user_data(
    State(state): State<LiveRouterState>,
    Query(query): Query<DeleteComputeNodesQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .delete_all_user_data(query.workflow_id, &context)
        .await
    {
        Ok(response) => delete_all_user_data_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "user_data",
    path = "/user_data/{id}",
    operation_id = "get_user_data",
    params(("id" = i64, Path, description = "User data record ID")),
    responses(
        (status = 200, description = "Successful response", body = models::UserDataModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn get_user_data(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.get_user_data(id, &context).await {
        Ok(response) => get_user_data_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    put,
    tag = "user_data",
    path = "/user_data/{id}",
    operation_id = "update_user_data",
    params(("id" = i64, Path, description = "User data record ID")),
    request_body = models::UserDataModel,
    responses(
        (status = 200, description = "Successful response", body = models::UserDataModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn update_user_data(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::UserDataModel>,
) -> Response<Body> {
    match state.server.update_user_data(id, body, &context).await {
        Ok(response) => update_user_data_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "user_data",
    path = "/user_data/{id}",
    operation_id = "delete_user_data",
    params(("id" = i64, Path, description = "User data record ID")),
    responses(
        (status = 200, description = "Successful response", body = models::UserDataModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_user_data(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.delete_user_data(id, &context).await {
        Ok(response) => delete_user_data_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ResultsQuery {
    pub workflow_id: i64,
    #[param(nullable = true)]
    pub job_id: Option<i64>,
    #[param(nullable = true)]
    pub run_id: Option<i64>,
    #[param(nullable = true)]
    pub return_code: Option<i64>,
    #[param(nullable = true)]
    pub status: Option<models::JobStatus>,
    #[param(nullable = true)]
    pub compute_node_id: Option<i64>,
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
    #[param(nullable = true)]
    pub sort_by: Option<String>,
    #[param(nullable = true)]
    pub reverse_sort: Option<bool>,
    #[param(nullable = true)]
    pub all_runs: Option<bool>,
}

#[utoipa::path(
    get,
    tag = "results",
    path = "/results",
    operation_id = "list_results",
    params(ResultsQuery),
    responses(
        (status = 200, description = "Successful response", body = models::ListResultsResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn list_results(
    State(state): State<LiveRouterState>,
    Query(query): Query<ResultsQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_results(
            query.workflow_id,
            query.job_id,
            query.run_id,
            query.return_code,
            query.status,
            query.compute_node_id,
            query.offset,
            query.limit,
            query.sort_by,
            query.reverse_sort,
            query.all_runs,
            &context,
        )
        .await
    {
        Ok(response) => list_results_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "results",
    path = "/results",
    operation_id = "create_result",
    request_body = models::ResultModel,
    responses(
        (status = 200, description = "Successful response", body = models::ResultModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn create_result(
    State(state): State<LiveRouterState>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::ResultModel>,
) -> Response<Body> {
    match state.server.create_result(body, &context).await {
        Ok(response) => create_result_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "results",
    path = "/results",
    operation_id = "delete_results",
    params(DeleteComputeNodesQuery),
    responses(
        (status = 200, description = "Successful response", body = Value),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_results(
    State(state): State<LiveRouterState>,
    Query(query): Query<DeleteComputeNodesQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .delete_results(query.workflow_id, &context)
        .await
    {
        Ok(response) => delete_results_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "results",
    path = "/results/{id}",
    operation_id = "get_result",
    params(("id" = i64, Path, description = "Results ID")),
    responses(
        (status = 200, description = "Successful response", body = models::ResultModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn get_result(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.get_result(id, &context).await {
        Ok(response) => get_result_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    put,
    tag = "results",
    path = "/results/{id}",
    operation_id = "update_result",
    params(("id" = i64, Path, description = "Result ID")),
    request_body = models::ResultModel,
    responses(
        (status = 200, description = "Successful response", body = models::ResultModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn update_result(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::ResultModel>,
) -> Response<Body> {
    match state.server.update_result(id, body, &context).await {
        Ok(response) => update_result_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "results",
    path = "/results/{id}",
    operation_id = "delete_result",
    params(("id" = i64, Path, description = "Results ID")),
    responses(
        (status = 200, description = "Successful response", body = models::ResultModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_result(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.delete_result(id, &context).await {
        Ok(response) => delete_result_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ScheduledComputeNodesQuery {
    pub workflow_id: i64,
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
    #[param(nullable = true)]
    pub sort_by: Option<String>,
    #[param(nullable = true)]
    pub reverse_sort: Option<bool>,
    #[param(nullable = true)]
    pub scheduler_id: Option<String>,
    #[param(nullable = true)]
    pub scheduler_config_id: Option<String>,
    #[param(nullable = true)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct SlurmSchedulersQuery {
    pub workflow_id: i64,
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
    #[param(nullable = true)]
    pub sort_by: Option<String>,
    #[param(nullable = true)]
    pub reverse_sort: Option<bool>,
}

#[utoipa::path(
    get,
    tag = "scheduled_compute_nodes",
    path = "/scheduled_compute_nodes",
    operation_id = "list_scheduled_compute_nodes",
    params(ScheduledComputeNodesQuery),
    responses(
        (status = 200, description = "Successful response", body = models::ListScheduledComputeNodesResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn list_scheduled_compute_nodes(
    State(state): State<LiveRouterState>,
    Query(query): Query<ScheduledComputeNodesQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_scheduled_compute_nodes(
            query.workflow_id,
            query.offset,
            query.limit,
            query.sort_by,
            query.reverse_sort,
            query.scheduler_id,
            query.scheduler_config_id,
            query.status,
            &context,
        )
        .await
    {
        Ok(response) => list_scheduled_compute_nodes_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "scheduled_compute_nodes",
    path = "/scheduled_compute_nodes",
    operation_id = "create_scheduled_compute_node",
    request_body = models::ScheduledComputeNodesModel,
    responses(
        (status = 200, description = "Successful response", body = models::ScheduledComputeNodesModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn create_scheduled_compute_node(
    State(state): State<LiveRouterState>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::ScheduledComputeNodesModel>,
) -> Response<Body> {
    match state
        .server
        .create_scheduled_compute_node(body, &context)
        .await
    {
        Ok(response) => create_scheduled_compute_node_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "scheduled_compute_nodes",
    path = "/scheduled_compute_nodes",
    operation_id = "delete_scheduled_compute_nodes",
    params(DeleteComputeNodesQuery),
    responses(
        (status = 200, description = "Successful response", body = models::DeleteCountResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_scheduled_compute_nodes(
    State(state): State<LiveRouterState>,
    Query(query): Query<DeleteComputeNodesQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .delete_scheduled_compute_nodes(query.workflow_id, &context)
        .await
    {
        Ok(response) => delete_scheduled_compute_nodes_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "scheduled_compute_nodes",
    path = "/scheduled_compute_nodes/{id}",
    operation_id = "get_scheduled_compute_node",
    params(("id" = i64, Path, description = "ID of the scheduled_compute_nodes record")),
    responses(
        (status = 200, description = "HTTP 200 OK.", body = models::ScheduledComputeNodesModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn get_scheduled_compute_node(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.get_scheduled_compute_node(id, &context).await {
        Ok(response) => get_scheduled_compute_node_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    put,
    tag = "scheduled_compute_nodes",
    path = "/scheduled_compute_nodes/{id}",
    operation_id = "update_scheduled_compute_node",
    params(("id" = i64, Path, description = "Scheduled compute node ID")),
    request_body = models::ScheduledComputeNodesModel,
    responses(
        (status = 200, description = "scheduled compute node updated in the table.", body = models::ScheduledComputeNodesModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn update_scheduled_compute_node(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::ScheduledComputeNodesModel>,
) -> Response<Body> {
    match state
        .server
        .update_scheduled_compute_node(id, body, &context)
        .await
    {
        Ok(response) => update_scheduled_compute_node_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "scheduled_compute_nodes",
    path = "/scheduled_compute_nodes/{id}",
    operation_id = "delete_scheduled_compute_node",
    params(("id" = i64, Path, description = "Scheduled compute node ID")),
    responses(
        (status = 200, description = "Successful response", body = models::ScheduledComputeNodesModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_scheduled_compute_node(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .delete_scheduled_compute_node(id, &context)
        .await
    {
        Ok(response) => delete_scheduled_compute_node_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "slurm_schedulers",
    path = "/slurm_schedulers",
    operation_id = "list_slurm_schedulers",
    params(SlurmSchedulersQuery),
    responses(
        (status = 200, description = "Successful response", body = models::ListSlurmSchedulersResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn list_slurm_schedulers(
    State(state): State<LiveRouterState>,
    Query(query): Query<SlurmSchedulersQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_slurm_schedulers(
            query.workflow_id,
            query.offset,
            query.limit,
            query.sort_by,
            query.reverse_sort,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            &context,
        )
        .await
    {
        Ok(response) => list_slurm_schedulers_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "slurm_schedulers",
    path = "/slurm_schedulers",
    operation_id = "create_slurm_scheduler",
    request_body = models::SlurmSchedulerModel,
    responses(
        (status = 200, description = "Successful response", body = models::SlurmSchedulerModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn create_slurm_scheduler(
    State(state): State<LiveRouterState>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::SlurmSchedulerModel>,
) -> Response<Body> {
    match state.server.create_slurm_scheduler(body, &context).await {
        Ok(response) => create_slurm_scheduler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "slurm_schedulers",
    path = "/slurm_schedulers",
    operation_id = "delete_slurm_schedulers",
    params(DeleteComputeNodesQuery),
    responses(
        (status = 200, description = "Successful response", body = models::DeleteCountResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_slurm_schedulers(
    State(state): State<LiveRouterState>,
    Query(query): Query<DeleteComputeNodesQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .delete_slurm_schedulers(query.workflow_id, &context)
        .await
    {
        Ok(response) => delete_slurm_schedulers_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "slurm_schedulers",
    path = "/slurm_schedulers/{id}",
    operation_id = "get_slurm_scheduler",
    params(("id" = i64, Path, description = "Slurm compute node configuration ID")),
    responses(
        (status = 200, description = "Successful response", body = models::SlurmSchedulerModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn get_slurm_scheduler(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.get_slurm_scheduler(id, &context).await {
        Ok(response) => get_slurm_scheduler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    put,
    tag = "slurm_schedulers",
    path = "/slurm_schedulers/{id}",
    operation_id = "update_slurm_scheduler",
    params(("id" = i64, Path, description = "Slurm compute node configuration ID")),
    request_body = models::SlurmSchedulerModel,
    responses(
        (status = 200, description = "Successful response", body = models::SlurmSchedulerModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn update_slurm_scheduler(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::SlurmSchedulerModel>,
) -> Response<Body> {
    match state
        .server
        .update_slurm_scheduler(id, body, &context)
        .await
    {
        Ok(response) => update_slurm_scheduler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "slurm_schedulers",
    path = "/slurm_schedulers/{id}",
    operation_id = "delete_slurm_scheduler",
    params(("id" = i64, Path, description = "Slurm compute node configuration ID")),
    responses(
        (status = 200, description = "Successful response", body = models::SlurmSchedulerModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_slurm_scheduler(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.delete_slurm_scheduler(id, &context).await {
        Ok(response) => delete_slurm_scheduler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "workflow_actions",
    path = "/workflows/{id}/actions",
    operation_id = "create_workflow_action",
    params(("id" = i64, Path, description = "Workflow ID")),
    request_body = models::WorkflowActionModel,
    responses((status = 200, body = models::WorkflowActionModel))
)]
pub async fn create_workflow_action(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::WorkflowActionModel>,
) -> Response<Body> {
    match state
        .server
        .create_workflow_action(id, body, &context)
        .await
    {
        Ok(response) => create_workflow_action_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "workflow_actions",
    path = "/workflows/{id}/actions",
    operation_id = "get_workflow_actions",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses((status = 200, body = [models::WorkflowActionModel]))
)]
pub async fn get_workflow_actions(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.get_workflow_actions(id, &context).await {
        Ok(response) => get_workflow_actions_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "workflow_actions",
    path = "/workflows/{id}/actions/pending",
    operation_id = "get_pending_actions",
    params(("id" = i64, Path, description = "Workflow ID"), PendingActionsQuery),
    responses((status = 200, body = [models::WorkflowActionModel]))
)]
pub async fn get_pending_actions(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    request: Request,
) -> Response<Body> {
    // Preserve the legacy transport semantics for repeated `trigger_type` query params.
    let query = parse_pending_actions_query(request.uri().query());
    let context = request_context(&request);

    match state
        .server
        .get_pending_actions(id, query.trigger_type, &context)
        .await
    {
        Ok(response) => get_pending_actions_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "workflow_actions",
    path = "/workflows/{id}/actions/{action_id}/claim",
    operation_id = "claim_action",
    params(
        ("id" = i64, Path, description = "Workflow ID"),
        ("action_id" = i64, Path, description = "Action ID")
    ),
    request_body = models::ClaimActionRequest,
    responses((status = 200, body = models::ClaimActionResponse))
)]
pub async fn claim_action(
    State(state): State<LiveRouterState>,
    Path((workflow_id, action_id)): Path<(i64, i64)>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::ClaimActionRequest>,
) -> Response<Body> {
    match state
        .server
        .claim_action(workflow_id, action_id, body, &context)
        .await
    {
        Ok(response) => claim_action_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "workflows",
    path = "/workflows",
    operation_id = "list_workflows",
    params(WorkflowsListQuery),
    responses((status = 200, body = models::ListWorkflowsResponse))
)]
pub async fn list_workflows(
    State(state): State<LiveRouterState>,
    Query(query): Query<WorkflowsListQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_workflows(
            query.offset,
            query.sort_by,
            query.reverse_sort,
            query.limit,
            query.name,
            query.user,
            query.description,
            query.is_archived,
            &context,
        )
        .await
    {
        Ok(response) => list_workflows_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "workflows",
    path = "/workflows",
    operation_id = "create_workflow",
    request_body = models::WorkflowModel,
    responses((status = 200, body = models::WorkflowModel))
)]
pub async fn create_workflow(
    State(state): State<LiveRouterState>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::WorkflowModel>,
) -> Response<Body> {
    match state.server.create_workflow(body, &context).await {
        Ok(response) => create_workflow_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "workflows",
    path = "/workflows/{id}",
    operation_id = "get_workflow",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses((status = 200, body = models::WorkflowModel))
)]
pub async fn get_workflow(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.get_workflow(id, &context).await {
        Ok(response) => get_workflow_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    put,
    tag = "workflows",
    path = "/workflows/{id}",
    operation_id = "update_workflow",
    params(("id" = i64, Path, description = "Workflow ID")),
    request_body = models::WorkflowModel,
    responses((status = 200, body = models::WorkflowModel))
)]
pub async fn update_workflow(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::WorkflowModel>,
) -> Response<Body> {
    match state.server.update_workflow(id, body, &context).await {
        Ok(response) => update_workflow_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "workflows",
    path = "/workflows/{id}",
    operation_id = "delete_workflow",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses((status = 200, body = models::WorkflowModel))
)]
pub async fn delete_workflow(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.delete_workflow(id, &context).await {
        Ok(response) => delete_workflow_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    put,
    tag = "workflows",
    path = "/workflows/{id}/cancel",
    operation_id = "cancel_workflow",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses((status = 200, body = Value))
)]
pub async fn cancel_workflow(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.cancel_workflow(id, &context).await {
        Ok(response) => cancel_workflow_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "workflows",
    path = "/workflows/{id}/initialize_jobs",
    operation_id = "initialize_jobs",
    params(("id" = i64, Path, description = "Workflow ID"), InitializeJobsQuery),
    responses((status = 200, body = Value))
)]
pub async fn initialize_jobs(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Query(query): Query<InitializeJobsQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .initialize_jobs(
            id,
            query.only_uninitialized,
            query.clear_ephemeral_user_data,
            &context,
        )
        .await
    {
        Ok(response) => initialize_jobs_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "workflows",
    path = "/workflows/{id}/is_complete",
    operation_id = "is_workflow_complete",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses((status = 200, body = models::IsCompleteResponse))
)]
pub async fn is_workflow_complete(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.is_workflow_complete(id, &context).await {
        Ok(response) => is_workflow_complete_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "workflows",
    path = "/workflows/{id}/is_uninitialized",
    operation_id = "is_workflow_uninitialized",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses((status = 200, body = models::IsUninitializedResponse))
)]
pub async fn is_workflow_uninitialized(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.is_workflow_uninitialized(id, &context).await {
        Ok(response) => is_workflow_uninitialized_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "workflows",
    path = "/workflows/{id}/reset_status",
    operation_id = "reset_workflow_status",
    params(("id" = i64, Path, description = "Workflow ID"), ResetWorkflowStatusQuery),
    responses((status = 200, body = Value))
)]
pub async fn reset_workflow_status(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Query(query): Query<ResetWorkflowStatusQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .reset_workflow_status(id, query.force, &context)
        .await
    {
        Ok(response) => reset_workflow_status_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "workflows",
    path = "/workflows/{id}/reset_job_status",
    operation_id = "reset_job_status",
    params(("id" = i64, Path, description = "Workflow ID"), ResetJobStatusQuery),
    responses((status = 200, body = models::ResetJobStatusResponse))
)]
pub async fn reset_job_status(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Query(query): Query<ResetJobStatusQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .reset_job_status(id, query.failed_only, &context)
        .await
    {
        Ok(response) => reset_job_status_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "workflows",
    path = "/workflows/{id}/status",
    operation_id = "get_workflow_status",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses((status = 200, body = models::WorkflowStatusModel))
)]
pub async fn get_workflow_status(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.get_workflow_status(id, &context).await {
        Ok(response) => get_workflow_status_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    put,
    tag = "workflows",
    path = "/workflows/{id}/status",
    operation_id = "update_workflow_status",
    params(("id" = i64, Path, description = "Workflow ID")),
    request_body = models::WorkflowStatusModel,
    responses((status = 200, body = models::WorkflowStatusModel))
)]
pub async fn update_workflow_status(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::WorkflowStatusModel>,
) -> Response<Body> {
    match state
        .server
        .update_workflow_status(id, body, &context)
        .await
    {
        Ok(response) => update_workflow_status_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "workflows",
    path = "/workflows/{id}/claim_jobs_based_on_resources/{limit}",
    operation_id = "claim_jobs_based_on_resources",
    params(
        ("id" = i64, Path, description = "Workflow ID"),
        ClaimJobsBasedOnResourcesQuery,
        ("limit" = i64, Path, description = "Maximum number of jobs to claim")
    ),
    request_body = models::ComputeNodesResources,
    responses((status = 200, body = models::ClaimJobsBasedOnResources))
)]
pub async fn claim_jobs_based_on_resources(
    State(state): State<LiveRouterState>,
    Path((id, limit)): Path<(i64, i64)>,
    Query(query): Query<ClaimJobsBasedOnResourcesQuery>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::ComputeNodesResources>,
) -> Response<Body> {
    match state
        .server
        .claim_jobs_based_on_resources(id, body, limit, query.strict_scheduler_match, &context)
        .await
    {
        Ok(response) => claim_jobs_based_on_resources_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "workflows",
    path = "/workflows/{id}/claim_next_jobs",
    operation_id = "claim_next_jobs",
    params(("id" = i64, Path, description = "Workflow ID"), ClaimNextJobsQuery),
    responses((status = 200, body = models::ClaimNextJobsResponse))
)]
pub async fn claim_next_jobs(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Query(query): Query<ClaimNextJobsQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .claim_next_jobs(id, query.limit, &context)
        .await
    {
        Ok(response) => claim_next_jobs_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "workflows",
    path = "/workflows/{id}/job_dependencies",
    operation_id = "list_job_dependencies",
    params(("id" = i64, Path, description = "Workflow ID"), WorkflowRelationshipsQuery),
    responses((status = 200, body = models::ListJobDependenciesResponse))
)]
pub async fn list_job_dependencies(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Query(query): Query<WorkflowRelationshipsQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_job_dependencies(
            id,
            query.offset,
            query.limit,
            query.sort_by,
            query.reverse_sort,
            &context,
        )
        .await
    {
        Ok(response) => list_job_dependencies_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "workflows",
    path = "/workflows/{id}/job_file_relationships",
    operation_id = "list_job_file_relationships",
    params(("id" = i64, Path, description = "Workflow ID"), WorkflowRelationshipsQuery),
    responses((status = 200, body = models::ListJobFileRelationshipsResponse))
)]
pub async fn list_job_file_relationships(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Query(query): Query<WorkflowRelationshipsQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_job_file_relationships(
            id,
            query.offset,
            query.limit,
            query.sort_by,
            query.reverse_sort,
            &context,
        )
        .await
    {
        Ok(response) => list_job_file_relationships_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "workflows",
    path = "/workflows/{id}/job_user_data_relationships",
    operation_id = "list_job_user_data_relationships",
    params(("id" = i64, Path, description = "Workflow ID"), WorkflowRelationshipsQuery),
    responses((status = 200, body = models::ListJobUserDataRelationshipsResponse))
)]
pub async fn list_job_user_data_relationships(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Query(query): Query<WorkflowRelationshipsQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_job_user_data_relationships(
            id,
            query.offset,
            query.limit,
            query.sort_by,
            query.reverse_sort,
            &context,
        )
        .await
    {
        Ok(response) => list_job_user_data_relationships_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "workflows",
    path = "/workflows/{id}/job_ids",
    operation_id = "list_job_ids",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses((status = 200, body = models::ListJobIdsResponse))
)]
pub async fn list_job_ids(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.list_job_ids(id, &context).await {
        Ok(response) => list_job_ids_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "workflows",
    path = "/workflows/{id}/missing_user_data",
    operation_id = "list_missing_user_data",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses((status = 200, body = models::ListMissingUserDataResponse))
)]
pub async fn list_missing_user_data(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.list_missing_user_data(id, &context).await {
        Ok(response) => list_missing_user_data_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "workflows",
    path = "/workflows/{id}/process_changed_job_inputs",
    operation_id = "process_changed_job_inputs",
    params(("id" = i64, Path, description = "Workflow ID"), ProcessChangedJobInputsQuery),
    responses((status = 200, body = models::ProcessChangedJobInputsResponse))
)]
pub async fn process_changed_job_inputs(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Query(query): Query<ProcessChangedJobInputsQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .process_changed_job_inputs(id, query.dry_run, &context)
        .await
    {
        Ok(response) => process_changed_job_inputs_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "workflows",
    path = "/workflows/{id}/ready_job_requirements",
    operation_id = "get_ready_job_requirements",
    params(("id" = i64, Path, description = "Workflow ID"), ReadyJobRequirementsQuery),
    responses((status = 200, body = models::GetReadyJobRequirementsResponse))
)]
pub async fn get_ready_job_requirements(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Query(query): Query<ReadyJobRequirementsQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .get_ready_job_requirements(id, query.scheduler_config_id, &context)
        .await
    {
        Ok(response) => get_ready_job_requirements_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "workflows",
    path = "/workflows/{id}/required_existing_files",
    operation_id = "list_required_existing_files",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses((status = 200, body = models::ListRequiredExistingFilesResponse))
)]
pub async fn list_required_existing_files(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_required_existing_files(id, &context)
        .await
    {
        Ok(response) => list_required_existing_files_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct RoCrateEntitiesQuery {
    #[param(nullable = true)]
    pub offset: Option<i64>,
    #[param(nullable = true)]
    pub limit: Option<i64>,
    #[param(nullable = true)]
    pub sort_by: Option<String>,
    #[param(nullable = true)]
    pub reverse_sort: Option<bool>,
}

#[utoipa::path(
    get,
    tag = "remote_workers",
    path = "/workflows/{id}/remote_workers",
    operation_id = "list_remote_workers",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses(
        (status = 200, description = "Successful response", body = [models::RemoteWorkerModel]),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn list_remote_workers(
    State(state): State<LiveRouterState>,
    Path(workflow_id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_remote_workers(workflow_id, &context)
        .await
    {
        Ok(response) => list_remote_workers_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "remote_workers",
    path = "/workflows/{id}/remote_workers",
    operation_id = "create_remote_workers",
    params(("id" = i64, Path, description = "Workflow ID")),
    request_body = Vec<String>,
    responses(
        (status = 200, description = "Successful response", body = [models::RemoteWorkerModel]),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 422, description = "Unprocessable content", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn create_remote_workers(
    State(state): State<LiveRouterState>,
    Path(workflow_id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<Vec<String>>,
) -> Response<Body> {
    match state
        .server
        .create_remote_workers(workflow_id, body, &context)
        .await
    {
        Ok(response) => create_remote_workers_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "remote_workers",
    path = "/workflows/{id}/remote_workers/{worker}",
    operation_id = "delete_remote_worker",
    params(
        ("id" = i64, Path, description = "Workflow ID"),
        ("worker" = String, Path, description = "Worker address")
    ),
    responses(
        (status = 200, description = "Successful response", body = models::RemoteWorkerModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_remote_worker(
    State(state): State<LiveRouterState>,
    Path((workflow_id, worker)): Path<(i64, String)>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .delete_remote_worker(workflow_id, worker, &context)
        .await
    {
        Ok(response) => delete_remote_worker_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    post,
    tag = "ro_crate_entities",
    path = "/ro_crate_entities",
    operation_id = "create_ro_crate_entity",
    request_body = models::RoCrateEntityModel,
    responses(
        (status = 200, description = "Successful response", body = models::RoCrateEntityModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 422, description = "Unprocessable content", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn create_ro_crate_entity(
    State(state): State<LiveRouterState>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::RoCrateEntityModel>,
) -> Response<Body> {
    match state.server.create_ro_crate_entity(body, &context).await {
        Ok(response) => create_ro_crate_entity_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "ro_crate_entities",
    path = "/ro_crate_entities/{id}",
    operation_id = "get_ro_crate_entity",
    params(("id" = i64, Path, description = "Entity ID")),
    responses(
        (status = 200, description = "Successful response", body = models::RoCrateEntityModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn get_ro_crate_entity(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.get_ro_crate_entity(id, &context).await {
        Ok(response) => get_ro_crate_entity_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    put,
    tag = "ro_crate_entities",
    path = "/ro_crate_entities/{id}",
    operation_id = "update_ro_crate_entity",
    params(("id" = i64, Path, description = "Entity ID")),
    request_body = models::RoCrateEntityModel,
    responses(
        (status = 200, description = "Successful response", body = models::RoCrateEntityModel),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 422, description = "Unprocessable content", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn update_ro_crate_entity(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
    Json(body): Json<models::RoCrateEntityModel>,
) -> Response<Body> {
    match state
        .server
        .update_ro_crate_entity(id, body, &context)
        .await
    {
        Ok(response) => update_ro_crate_entity_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "ro_crate_entities",
    path = "/ro_crate_entities/{id}",
    operation_id = "delete_ro_crate_entity",
    params(("id" = i64, Path, description = "Entity ID")),
    responses(
        (status = 200, description = "Successful response", body = models::MessageResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_ro_crate_entity(
    State(state): State<LiveRouterState>,
    Path(id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state.server.delete_ro_crate_entity(id, &context).await {
        Ok(response) => delete_ro_crate_entity_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    get,
    tag = "ro_crate_entities",
    path = "/workflows/{id}/ro_crate_entities",
    operation_id = "list_ro_crate_entities",
    params(
        ("id" = i64, Path, description = "Workflow ID"),
        RoCrateEntitiesQuery
    ),
    responses(
        (status = 200, description = "Successful response", body = models::ListRoCrateEntitiesResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn list_ro_crate_entities(
    State(state): State<LiveRouterState>,
    Path(workflow_id): Path<i64>,
    Query(query): Query<RoCrateEntitiesQuery>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .list_ro_crate_entities(
            workflow_id,
            query.offset,
            query.limit,
            query.sort_by,
            query.reverse_sort,
            &context,
        )
        .await
    {
        Ok(response) => list_ro_crate_entities_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

#[utoipa::path(
    delete,
    tag = "ro_crate_entities",
    path = "/workflows/{id}/ro_crate_entities",
    operation_id = "delete_ro_crate_entities",
    params(("id" = i64, Path, description = "Workflow ID")),
    responses(
        (status = 200, description = "Successful response", body = models::DeleteRoCrateEntitiesResponse),
        (status = 403, description = "Forbidden", body = models::ErrorResponse),
        (status = 404, description = "Not found", body = models::ErrorResponse),
        (status = 500, description = "Internal server error", body = models::ErrorResponse)
    )
)]
pub async fn delete_ro_crate_entities(
    State(state): State<LiveRouterState>,
    Path(workflow_id): Path<i64>,
    Extension(context): Extension<EmptyContext>,
) -> Response<Body> {
    match state
        .server
        .delete_ro_crate_entities(workflow_id, &context)
        .await
    {
        Ok(response) => delete_ro_crate_entities_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

path_handler!(
    workflow_events_stream_route,
    i64,
    |id, server, request, context| {
        handle_workflow_events_stream(server, id, request, context).await
    }
);
fn request_context(request: &Request) -> EmptyContext {
    request
        .extensions()
        .get::<EmptyContext>()
        .cloned()
        .unwrap_or_else(|| {
            EmptyContext::default()
                .push(XSpanIdString::get_or_generate(request))
                .push(None::<AuthData>)
                .push(None::<Authorization>)
        })
}

async fn inject_request_context(
    State(state): State<LiveAuthState>,
    mut request: Request,
    next: Next,
) -> Response<Body> {
    let span_id = XSpanIdString::get_or_generate(&request);
    let authorization = resolve_authorization(
        request.headers(),
        &state.htpasswd,
        state.require_auth,
        &state.credential_cache,
    );

    if state.require_auth && authorization.is_none() {
        let mut response = Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header("WWW-Authenticate", "Basic realm=\"Torc\"")
            .body(Body::from("Unauthorized"))
            .unwrap();
        add_standard_response_headers(&mut response, &span_id);
        return response;
    }

    let context = EmptyContext::default()
        .push(span_id.clone())
        .push(None::<AuthData>)
        .push(authorization);
    request.extensions_mut().insert(context);

    let mut response = next.run(request).await;
    add_standard_response_headers(&mut response, &span_id);
    response
}

fn add_standard_response_headers(response: &mut Response<Body>, span_id: &XSpanIdString) {
    response.headers_mut().insert(
        HeaderName::from_static("x-span-id"),
        HeaderValue::from_str(&span_id.0).expect("span id should be a valid header value"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
}

fn resolve_authorization(
    headers: &axum::http::HeaderMap,
    htpasswd: &SharedHtpasswd,
    require_auth: bool,
    credential_cache: &SharedCredentialCache,
) -> Option<Authorization> {
    let basic_auth = from_headers(headers);
    let htpasswd_guard = htpasswd.read();

    match &*htpasswd_guard {
        Some(htpasswd_file) => match basic_auth {
            Some(basic) => {
                let password = basic.password.as_deref().unwrap_or("");
                if verify_with_cache(credential_cache, htpasswd_file, &basic.username, password) {
                    Some(Authorization {
                        subject: basic.username.clone(),
                        scopes: Scopes::All,
                        issuer: None,
                    })
                } else {
                    None
                }
            }
            None if require_auth => None,
            None => Some(anonymous_authorization()),
        },
        None if require_auth => None,
        None => Some(anonymous_authorization()),
    }
}

fn verify_with_cache(
    credential_cache: &SharedCredentialCache,
    htpasswd: &HtpasswdFile,
    username: &str,
    password: &str,
) -> bool {
    if is_cached(credential_cache.read(), username, password) {
        return true;
    }

    if htpasswd.verify(username, password) {
        let cache_guard = credential_cache.read();
        if let Some(ref cache) = *cache_guard {
            cache.cache_success(username, password);
        }
        true
    } else {
        false
    }
}

fn is_cached(
    cache_guard: RwLockReadGuard<'_, Option<CredentialCache>>,
    username: &str,
    password: &str,
) -> bool {
    match &*cache_guard {
        Some(cache) => cache.is_cached(username, password),
        None => false,
    }
}

fn anonymous_authorization() -> Authorization {
    Authorization {
        subject: "anonymous".to_string(),
        scopes: Scopes::Some(BTreeSet::new()),
        issuer: None,
    }
}

#[cfg(test)]
mod live_router_tests {
    use super::*;
    use crate::models::{ComputeNodeModel, JobModel, JobsModel, WorkflowModel};
    use crate::server::api_contract::TransportApiCore;
    use crate::server::auth::{SharedCredentialCache, SharedHtpasswd};
    use crate::server::response_types::workflows::CreateWorkflowResponse;
    use axum::http::Request;
    use axum::http::header::CONTENT_TYPE;
    use http_body_util::BodyExt;
    use parking_lot::RwLock;
    use serde::de::DeserializeOwned;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;
    use std::sync::Arc;
    use tower::ServiceExt;

    #[tokio::test]
    async fn router_serves_ping() {
        let router = test_router(test_server_with_schema().await);
        let response = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/torc-service/v1/ping")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get("x-span-id").is_some());
        assert_eq!(
            response.headers().get("x-content-type-options"),
            Some(&HeaderValue::from_static("nosniff"))
        );
        assert_eq!(
            response.headers().get("x-frame-options"),
            Some(&HeaderValue::from_static("DENY"))
        );
    }

    #[tokio::test]
    async fn router_returns_method_not_allowed_for_known_path() {
        let router = test_router(test_server_with_schema().await);
        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/torc-service/v1/ping")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn router_falls_back_for_unknown_path() {
        let router = test_router(test_server_with_schema().await);
        let response = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/torc-service/v1/not-bridged")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn compute_nodes_round_trip_via_router() {
        let server = test_server_with_schema().await;
        let workflow_id = create_workflow_record(&server).await;
        let router = test_router(server);

        let create_body = ComputeNodeModel::new(
            workflow_id,
            "node-a".to_string(),
            1234,
            chrono::Utc::now().to_rfc3339(),
            8,
            16.0,
            0,
            1,
            "local".to_string(),
            None,
        );

        let create_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/torc-service/v1/compute_nodes")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&create_body).expect("serialize compute node"),
                    ))
                    .expect("valid request"),
            )
            .await
            .expect("create response");

        assert_eq!(create_response.status(), StatusCode::OK);
        let created: ComputeNodeModel = read_json_body(create_response).await;
        assert_eq!(created.hostname, "node-a");
        assert_eq!(created.workflow_id, workflow_id);

        let list_response = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/torc-service/v1/compute_nodes?workflow_id={workflow_id}"
                    ))
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("list response");

        assert_eq!(list_response.status(), StatusCode::OK);
        let listed: serde_json::Value = read_json_body(list_response).await;
        let items = listed["items"].as_array().expect("list items array");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["hostname"], "node-a");
    }

    #[tokio::test]
    async fn get_workflow_round_trip_via_router() {
        let server = test_server_with_schema().await;
        let workflow_id = create_workflow_record(&server).await;
        let router = test_router(server);

        let response = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/torc-service/v1/workflows/{workflow_id}"))
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("workflow response");

        assert_eq!(response.status(), StatusCode::OK);
        let workflow: WorkflowModel = read_json_body(response).await;
        assert_eq!(workflow.id, Some(workflow_id));
        assert_eq!(workflow.name, "transport-workflow");
    }

    #[tokio::test]
    async fn bulk_jobs_route_accepts_body_larger_than_default_limit() {
        let server = test_server_with_schema().await;
        let workflow_id = create_workflow_record(&server).await;
        let router = test_router(server);

        let create_body = JobsModel::new(vec![JobModel::new(
            workflow_id,
            "large-job".to_string(),
            "x".repeat(3 * 1024 * 1024),
        )]);

        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/torc-service/v1/bulk_jobs")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&create_body).expect("serialize jobs"),
                    ))
                    .expect("valid request"),
            )
            .await
            .expect("bulk jobs response");

        assert_eq!(response.status(), StatusCode::OK);
        let created: serde_json::Value = read_json_body(response).await;
        let jobs = created["jobs"].as_array().expect("jobs array");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0]["name"], "large-job");
    }

    #[tokio::test]
    async fn non_bulk_json_route_still_uses_default_body_limit() {
        let server = test_server_with_schema().await;
        let workflow_id = create_workflow_record(&server).await;
        let router = test_router(server);

        let create_body = ComputeNodeModel::new(
            workflow_id,
            "n".repeat(3 * 1024 * 1024),
            1234,
            chrono::Utc::now().to_rfc3339(),
            8,
            16.0,
            0,
            1,
            "local".to_string(),
            None,
        );

        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/torc-service/v1/compute_nodes")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&create_body).expect("serialize compute node"),
                    ))
                    .expect("valid request"),
            )
            .await
            .expect("compute nodes response");

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    fn test_router(server: Server<EmptyContext>) -> Router {
        app_router(LiveRouterState {
            openapi_state: server.openapi_app_state(),
            server,
            auth: LiveAuthState {
                htpasswd: Arc::new(RwLock::new(None)),
                require_auth: false,
                credential_cache: Arc::new(RwLock::new(None)),
            },
        })
    }

    async fn test_server_with_schema() -> Server<EmptyContext> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::from_str("sqlite::memory:")
                    .expect("sqlite memory connection")
                    .create_if_missing(true),
            )
            .await
            .expect("in-memory pool");
        sqlx::migrate!("./torc-server/migrations")
            .run(&pool)
            .await
            .expect("migrations");

        let htpasswd: SharedHtpasswd = Arc::new(RwLock::new(None));
        let credential_cache: SharedCredentialCache = Arc::new(RwLock::new(None));
        Server::new(pool, false, htpasswd, None, credential_cache)
    }

    async fn create_workflow_record(server: &Server<EmptyContext>) -> i64 {
        let workflow_response = server
            .create_workflow(
                WorkflowModel::new("transport-workflow".to_string(), "test-user".to_string()),
                &EmptyContext::default(),
            )
            .await
            .expect("create workflow");

        match workflow_response {
            CreateWorkflowResponse::SuccessfulResponse(workflow) => {
                workflow.id.expect("workflow id")
            }
            other => panic!("unexpected workflow response: {other:?}"),
        }
    }

    async fn read_json_body<T: DeserializeOwned>(response: Response<Body>) -> T {
        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect body")
            .to_bytes();
        serde_json::from_slice(&body).expect("json body")
    }
}
