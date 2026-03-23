use crate::models;
use crate::openapi_codegen::{OpenApiAppState, PingResponse, VersionResponse};
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
use axum::extract::{Path, Request, State};
use axum::http::header::{HeaderName, HeaderValue};
use axum::http::{Response, StatusCode};
use axum::middleware::{self, Next};
use axum::routing::{delete, get, post, put};
use parking_lot::RwLockReadGuard;
use std::collections::BTreeSet;

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

macro_rules! req_handler {
    ($name:ident, |$server:ident, $request:ident, $context:ident| $body:block) => {
        async fn $name(State(state): State<LiveRouterState>, request: Request) -> Response<Body> {
            let $server = state.server.clone();
            let $request = request;
            let $context = request_context(&$request);
            $body
        }
    };
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

pub fn app_router(state: LiveRouterState) -> Router {
    Router::new()
        .route(
            "/torc-service/v1/access_groups",
            post(create_access_group_route).get(list_access_groups_route),
        )
        .route(
            "/torc-service/v1/access_groups/{id}",
            get(get_access_group_route).delete(delete_access_group_route),
        )
        .route(
            "/torc-service/v1/access_groups/{id}/members",
            post(add_user_to_group_route).get(list_group_members_route),
        )
        .route(
            "/torc-service/v1/access_groups/{id}/members/{user_name}",
            delete(remove_user_from_group_route),
        )
        .route(
            "/torc-service/v1/users/{user_name}/groups",
            get(list_user_groups_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/access_groups",
            get(list_workflow_groups_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/access_groups/{group_id}",
            post(add_workflow_to_group_route).delete(remove_workflow_from_group_route),
        )
        .route(
            "/torc-service/v1/access_check/{workflow_id}/{user_name}",
            get(check_workflow_access_route),
        )
        .route("/torc-service/v1/ping", get(ping_route))
        .route("/torc-service/v1/version", get(version_route))
        .route("/torc-service/v1/bulk_jobs", post(create_jobs_route))
        .route(
            "/torc-service/v1/compute_nodes",
            get(list_compute_nodes_route)
                .post(create_compute_node_route)
                .delete(delete_compute_nodes_route),
        )
        .route(
            "/torc-service/v1/compute_nodes/{id}",
            get(get_compute_node_route)
                .put(update_compute_node_route)
                .delete(delete_compute_node_route),
        )
        .route(
            "/torc-service/v1/events",
            get(list_events_route)
                .post(create_event_route)
                .delete(delete_events_route),
        )
        .route(
            "/torc-service/v1/events/{id}",
            get(get_event_route)
                .put(update_event_route)
                .delete(delete_event_route),
        )
        .route(
            "/torc-service/v1/files",
            get(list_files_route)
                .post(create_file_route)
                .delete(delete_files_route),
        )
        .route(
            "/torc-service/v1/files/{id}",
            get(get_file_route)
                .put(update_file_route)
                .delete(delete_file_route),
        )
        .route(
            "/torc-service/v1/local_schedulers",
            get(list_local_schedulers_route)
                .post(create_local_scheduler_route)
                .delete(delete_local_schedulers_route),
        )
        .route(
            "/torc-service/v1/local_schedulers/{id}",
            get(get_local_scheduler_route)
                .put(update_local_scheduler_route)
                .delete(delete_local_scheduler_route),
        )
        .route(
            "/torc-service/v1/resource_requirements",
            get(list_resource_requirements_route)
                .post(create_resource_requirements_route)
                .delete(delete_all_resource_requirements_route),
        )
        .route(
            "/torc-service/v1/resource_requirements/{id}",
            get(get_resource_requirements_route)
                .put(update_resource_requirements_route)
                .delete(delete_resource_requirements_route),
        )
        .route(
            "/torc-service/v1/failure_handlers",
            post(create_failure_handler_route),
        )
        .route(
            "/torc-service/v1/failure_handlers/{id}",
            get(get_failure_handler_route).delete(delete_failure_handler_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/failure_handlers",
            get(list_failure_handlers_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/actions",
            post(create_workflow_action_route).get(get_workflow_actions_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/actions/pending",
            get(get_pending_actions_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/actions/{action_id}/claim",
            post(claim_action_route),
        )
        .route(
            "/torc-service/v1/jobs",
            get(list_jobs_route)
                .post(create_job_route)
                .delete(delete_jobs_route),
        )
        .route(
            "/torc-service/v1/jobs/{id}",
            get(get_job_route)
                .put(update_job_route)
                .delete(delete_job_route),
        )
        .route(
            "/torc-service/v1/jobs/{id}/complete_job/{status}/{run_id}",
            post(complete_job_route),
        )
        .route(
            "/torc-service/v1/jobs/{id}/manage_status_change/{status}/{run_id}",
            put(manage_status_change_route),
        )
        .route(
            "/torc-service/v1/jobs/{id}/start_job/{run_id}/{compute_node_id}",
            put(start_job_route),
        )
        .route(
            "/torc-service/v1/jobs/{id}/retry/{run_id}",
            post(retry_job_route),
        )
        .route(
            "/torc-service/v1/user_data",
            get(list_user_data_route)
                .post(create_user_data_route)
                .delete(delete_all_user_data_route),
        )
        .route(
            "/torc-service/v1/user_data/{id}",
            get(get_user_data_route)
                .put(update_user_data_route)
                .delete(delete_user_data_route),
        )
        .route(
            "/torc-service/v1/results",
            get(list_results_route)
                .post(create_result_route)
                .delete(delete_results_route),
        )
        .route(
            "/torc-service/v1/results/{id}",
            get(get_result_route)
                .put(update_result_route)
                .delete(delete_result_route),
        )
        .route(
            "/torc-service/v1/scheduled_compute_nodes",
            get(list_scheduled_compute_nodes_route)
                .post(create_scheduled_compute_node_route)
                .delete(delete_scheduled_compute_nodes_route),
        )
        .route(
            "/torc-service/v1/scheduled_compute_nodes/{id}",
            get(get_scheduled_compute_node_route)
                .put(update_scheduled_compute_node_route)
                .delete(delete_scheduled_compute_node_route),
        )
        .route(
            "/torc-service/v1/slurm_schedulers",
            get(list_slurm_schedulers_route)
                .post(create_slurm_scheduler_route)
                .delete(delete_slurm_schedulers_route),
        )
        .route(
            "/torc-service/v1/slurm_schedulers/{id}",
            get(get_slurm_scheduler_route)
                .put(update_slurm_scheduler_route)
                .delete(delete_slurm_scheduler_route),
        )
        .route(
            "/torc-service/v1/slurm_stats",
            get(list_slurm_stats_route).post(create_slurm_stats_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/remote_workers",
            get(list_remote_workers_route).post(create_remote_workers_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/remote_workers/{worker}",
            delete(delete_remote_worker_route),
        )
        .route(
            "/torc-service/v1/ro_crate_entities",
            post(create_ro_crate_entity_route),
        )
        .route(
            "/torc-service/v1/ro_crate_entities/{id}",
            get(get_ro_crate_entity_route)
                .put(update_ro_crate_entity_route)
                .delete(delete_ro_crate_entity_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/ro_crate_entities",
            get(list_ro_crate_entities_route).delete(delete_ro_crate_entities_route),
        )
        .route(
            "/torc-service/v1/admin/reload-auth",
            post(reload_auth_route),
        )
        .route(
            "/torc-service/v1/workflows",
            get(list_workflows_route).post(create_workflow_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}",
            get(get_workflow_route)
                .put(update_workflow_route)
                .delete(delete_workflow_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/cancel",
            put(cancel_workflow_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/initialize_jobs",
            post(initialize_jobs_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/is_complete",
            get(is_workflow_complete_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/is_uninitialized",
            get(is_workflow_uninitialized_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/reset_status",
            post(reset_workflow_status_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/reset_job_status",
            post(reset_job_status_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/status",
            get(get_workflow_status_route).put(update_workflow_status_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/claim_jobs_based_on_resources/{limit}",
            post(claim_jobs_based_on_resources_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/claim_next_jobs",
            post(claim_next_jobs_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/job_dependencies",
            get(list_job_dependencies_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/job_file_relationships",
            get(list_job_file_relationships_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/job_user_data_relationships",
            get(list_job_user_data_relationships_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/job_ids",
            get(list_job_ids_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/missing_user_data",
            get(list_missing_user_data_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/process_changed_job_inputs",
            post(process_changed_job_inputs_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/ready_job_requirements",
            get(get_ready_job_requirements_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/required_existing_files",
            get(list_required_existing_files_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/events/stream",
            get(workflow_events_stream_route),
        )
        .route(
            "/torc-service/v1/workflows/{id}/dot_graph/{name}",
            get(get_dot_graph_route),
        )
        .fallback(dashboard_fallback)
        .layer(middleware::from_fn_with_state(
            state.auth.clone(),
            inject_request_context,
        ))
        .with_state(state)
}

async fn ping_route() -> Response<Body> {
    json_response(&PingResponse {
        status: "ok".to_string(),
    })
}

async fn version_route(State(state): State<LiveRouterState>) -> Response<Body> {
    json_response(&VersionResponse {
        version: state.openapi_state.version.clone(),
        api_version: state.openapi_state.api_version.clone(),
        git_hash: (!state.openapi_state.access_control_enabled)
            .then_some(state.openapi_state.git_hash.clone()),
    })
}

async fn reload_auth_route(
    State(state): State<LiveRouterState>,
    request: Request,
) -> Response<Body> {
    let context = request_context(&request);
    handle_reload_auth(state.server.clone(), context).await
}

async fn dashboard_fallback(request: Request) -> Response<Body> {
    serve_dashboard(request.uri().path()).unwrap_or_else(not_found_response)
}

req_handler!(list_access_groups_route, |server, request, context| {
    handle_list_access_groups(server, request, context).await
});
req_handler!(create_access_group_route, |server, request, context| {
    handle_create_access_group(server, request, context).await
});
path_handler!(
    get_access_group_route,
    i64,
    |id, server, request, context| { handle_get_access_group(server, id, context).await }
);
path_handler!(
    delete_access_group_route,
    i64,
    |id, server, request, context| { handle_delete_access_group(server, id, context).await }
);
path_handler!(
    add_user_to_group_route,
    i64,
    |id, server, request, context| { handle_add_user_to_group(server, id, request, context).await }
);
path_handler!(
    list_group_members_route,
    i64,
    |id, server, request, context| {
        handle_list_group_members(server, id, request, context).await
    }
);
path_handler!(
    remove_user_from_group_route,
    (i64, String),
    |(group_id, user_name), server, request, context| {
        handle_remove_user_from_group(server, group_id, user_name, context).await
    }
);
path_handler!(
    list_user_groups_route,
    String,
    |user_name, server, request, context| {
        handle_list_user_groups(server, user_name, request, context).await
    }
);
path_handler!(
    add_workflow_to_group_route,
    (i64, i64),
    |(workflow_id, group_id), server, request, context| {
        handle_add_workflow_to_group_by_path(server, workflow_id, group_id, context).await
    }
);
path_handler!(
    list_workflow_groups_route,
    i64,
    |workflow_id, server, request, context| {
        handle_list_workflow_groups(server, workflow_id, request, context).await
    }
);
path_handler!(
    remove_workflow_from_group_route,
    (i64, i64),
    |(workflow_id, group_id), server, request, context| {
        handle_remove_workflow_from_group(server, workflow_id, group_id, context).await
    }
);
path_handler!(
    check_workflow_access_route,
    (i64, String),
    |(workflow_id, user_name), server, request, context| {
        handle_check_workflow_access(server, workflow_id, user_name, context).await
    }
);

req_handler!(create_jobs_route, |server, request, context| {
    handle_create_jobs(server, request, context).await
});
req_handler!(list_compute_nodes_route, |server, request, context| {
    handle_list_compute_nodes(server, request, context).await
});
req_handler!(create_compute_node_route, |server, request, context| {
    handle_create_compute_node(server, request, context).await
});
req_handler!(delete_compute_nodes_route, |server, request, context| {
    handle_delete_compute_nodes(server, request, context).await
});
path_handler!(
    get_compute_node_route,
    i64,
    |id, server, request, context| { handle_get_compute_node(server, id, context).await }
);
path_handler!(
    update_compute_node_route,
    i64,
    |id, server, request, context| {
        handle_update_compute_node(server, id, request, context).await
    }
);
path_handler!(
    delete_compute_node_route,
    i64,
    |id, server, request, context| {
        handle_delete_compute_node(server, id, request, context).await
    }
);

req_handler!(list_events_route, |server, request, context| {
    handle_list_events(server, request, context).await
});
req_handler!(create_event_route, |server, request, context| {
    handle_create_event(server, request, context).await
});
req_handler!(delete_events_route, |server, request, context| {
    handle_delete_events(server, request, context).await
});
path_handler!(get_event_route, i64, |id, server, request, context| {
    handle_get_event(server, id, context).await
});
path_handler!(update_event_route, i64, |id, server, request, context| {
    handle_update_event(server, id, request, context).await
});
path_handler!(delete_event_route, i64, |id, server, request, context| {
    handle_delete_event(server, id, request, context).await
});

req_handler!(list_files_route, |server, request, context| {
    handle_list_files(server, request, context).await
});
req_handler!(create_file_route, |server, request, context| {
    handle_create_file(server, request, context).await
});
req_handler!(delete_files_route, |server, request, context| {
    handle_delete_files(server, request, context).await
});
path_handler!(get_file_route, i64, |id, server, request, context| {
    handle_get_file(server, id, context).await
});
path_handler!(update_file_route, i64, |id, server, request, context| {
    handle_update_file(server, id, request, context).await
});
path_handler!(delete_file_route, i64, |id, server, request, context| {
    handle_delete_file(server, id, request, context).await
});

req_handler!(list_local_schedulers_route, |server, request, context| {
    handle_list_local_schedulers(server, request, context).await
});
req_handler!(create_local_scheduler_route, |server, request, context| {
    handle_create_local_scheduler(server, request, context).await
});
req_handler!(delete_local_schedulers_route, |server, request, context| {
    handle_delete_local_schedulers(server, request, context).await
});
path_handler!(
    get_local_scheduler_route,
    i64,
    |id, server, request, context| { handle_get_local_scheduler(server, id, context).await }
);
path_handler!(
    update_local_scheduler_route,
    i64,
    |id, server, request, context| {
        handle_update_local_scheduler(server, id, request, context).await
    }
);
path_handler!(
    delete_local_scheduler_route,
    i64,
    |id, server, request, context| {
        handle_delete_local_scheduler(server, id, request, context).await
    }
);

req_handler!(
    create_resource_requirements_route,
    |server, request, context| {
        handle_create_resource_requirements(server, request, context).await
    }
);
req_handler!(
    list_resource_requirements_route,
    |server, request, context| {
        handle_list_resource_requirements(server, request, context).await
    }
);
req_handler!(
    delete_all_resource_requirements_route,
    |server, request, context| {
        handle_delete_all_resource_requirements(server, request, context).await
    }
);
path_handler!(
    get_resource_requirements_route,
    i64,
    |id, server, request, context| { handle_get_resource_requirements(server, id, context).await }
);
path_handler!(
    update_resource_requirements_route,
    i64,
    |id, server, request, context| {
        handle_update_resource_requirements(server, id, request, context).await
    }
);
path_handler!(
    delete_resource_requirements_route,
    i64,
    |id, server, request, context| {
        handle_delete_resource_requirements(server, id, request, context).await
    }
);

req_handler!(create_failure_handler_route, |server, request, context| {
    handle_create_failure_handler(server, request, context).await
});
path_handler!(
    get_failure_handler_route,
    i64,
    |id, server, request, context| { handle_get_failure_handler(server, id, context).await }
);
path_handler!(
    delete_failure_handler_route,
    i64,
    |id, server, request, context| {
        handle_delete_failure_handler(server, id, request, context).await
    }
);
path_handler!(
    list_failure_handlers_route,
    i64,
    |workflow_id, server, request, context| {
        handle_list_failure_handlers(server, workflow_id, request, context).await
    }
);

path_handler!(
    get_workflow_actions_route,
    i64,
    |workflow_id, server, request, context| {
        handle_get_workflow_actions(server, workflow_id, context).await
    }
);
path_handler!(
    create_workflow_action_route,
    i64,
    |workflow_id, server, request, context| {
        handle_create_workflow_action(server, workflow_id, request, context).await
    }
);
path_handler!(
    get_pending_actions_route,
    i64,
    |workflow_id, server, request, context| {
        handle_get_pending_actions(server, workflow_id, request, context).await
    }
);
path_handler!(
    claim_action_route,
    (i64, i64),
    |(workflow_id, action_id), server, request, context| {
        handle_claim_action(server, workflow_id, action_id, request, context).await
    }
);

req_handler!(list_jobs_route, |server, request, context| {
    handle_list_jobs(server, request, context).await
});
req_handler!(create_job_route, |server, request, context| {
    handle_create_job(server, request, context).await
});
req_handler!(delete_jobs_route, |server, request, context| {
    handle_delete_jobs(server, request, context).await
});
path_handler!(get_job_route, i64, |id, server, request, context| {
    handle_get_job(server, id, context).await
});
path_handler!(update_job_route, i64, |id, server, request, context| {
    handle_update_job(server, id, request, context).await
});
path_handler!(delete_job_route, i64, |id, server, request, context| {
    handle_delete_job(server, id, request, context).await
});
path_handler!(
    complete_job_route,
    (i64, models::JobStatus, i64),
    |(id, status, run_id), server, request, context| {
        handle_complete_job(server, id, status, run_id, request, context).await
    }
);
path_handler!(
    manage_status_change_route,
    (i64, models::JobStatus, i64),
    |(id, status, run_id), server, request, context| {
        handle_manage_status_change(server, id, status, run_id, request, context).await
    }
);
path_handler!(
    start_job_route,
    (i64, i64, i64),
    |(id, run_id, compute_node_id), server, request, context| {
        handle_start_job(server, id, run_id, compute_node_id, request, context).await
    }
);
path_handler!(
    retry_job_route,
    (i64, i64),
    |(id, run_id), server, request, context| {
        handle_retry_job(server, id, run_id, request, context).await
    }
);

req_handler!(list_user_data_route, |server, request, context| {
    handle_list_user_data(server, request, context).await
});
req_handler!(create_user_data_route, |server, request, context| {
    handle_create_user_data(server, request, context).await
});
req_handler!(delete_all_user_data_route, |server, request, context| {
    handle_delete_all_user_data(server, request, context).await
});
path_handler!(get_user_data_route, i64, |id, server, request, context| {
    handle_get_user_data(server, id, context).await
});
path_handler!(
    update_user_data_route,
    i64,
    |id, server, request, context| { handle_update_user_data(server, id, request, context).await }
);
path_handler!(
    delete_user_data_route,
    i64,
    |id, server, request, context| { handle_delete_user_data(server, id, request, context).await }
);

req_handler!(list_results_route, |server, request, context| {
    handle_list_results(server, request, context).await
});
req_handler!(create_result_route, |server, request, context| {
    handle_create_result(server, request, context).await
});
req_handler!(delete_results_route, |server, request, context| {
    handle_delete_results(server, request, context).await
});
path_handler!(get_result_route, i64, |id, server, request, context| {
    handle_get_result(server, id, context).await
});
path_handler!(update_result_route, i64, |id, server, request, context| {
    handle_update_result(server, id, request, context).await
});
path_handler!(delete_result_route, i64, |id, server, request, context| {
    handle_delete_result(server, id, request, context).await
});

req_handler!(
    list_scheduled_compute_nodes_route,
    |server, request, context| {
        handle_list_scheduled_compute_nodes(server, request, context).await
    }
);
req_handler!(
    create_scheduled_compute_node_route,
    |server, request, context| {
        handle_create_scheduled_compute_node(server, request, context).await
    }
);
req_handler!(
    delete_scheduled_compute_nodes_route,
    |server, request, context| {
        handle_delete_scheduled_compute_nodes(server, request, context).await
    }
);
path_handler!(
    get_scheduled_compute_node_route,
    i64,
    |id, server, request, context| { handle_get_scheduled_compute_node(server, id, context).await }
);
path_handler!(
    update_scheduled_compute_node_route,
    i64,
    |id, server, request, context| {
        handle_update_scheduled_compute_node(server, id, request, context).await
    }
);
path_handler!(
    delete_scheduled_compute_node_route,
    i64,
    |id, server, request, context| {
        handle_delete_scheduled_compute_node(server, id, request, context).await
    }
);

req_handler!(list_slurm_schedulers_route, |server, request, context| {
    handle_list_slurm_schedulers(server, request, context).await
});
req_handler!(create_slurm_scheduler_route, |server, request, context| {
    handle_create_slurm_scheduler(server, request, context).await
});
req_handler!(delete_slurm_schedulers_route, |server, request, context| {
    handle_delete_slurm_schedulers(server, request, context).await
});
path_handler!(
    get_slurm_scheduler_route,
    i64,
    |id, server, request, context| { handle_get_slurm_scheduler(server, id, context).await }
);
path_handler!(
    update_slurm_scheduler_route,
    i64,
    |id, server, request, context| {
        handle_update_slurm_scheduler(server, id, request, context).await
    }
);
path_handler!(
    delete_slurm_scheduler_route,
    i64,
    |id, server, request, context| {
        handle_delete_slurm_scheduler(server, id, request, context).await
    }
);

req_handler!(create_slurm_stats_route, |server, request, context| {
    handle_create_slurm_stats(server, request, context).await
});
req_handler!(list_slurm_stats_route, |server, request, context| {
    handle_list_slurm_stats(server, request, context).await
});

path_handler!(
    list_remote_workers_route,
    i64,
    |workflow_id, server, request, context| {
        handle_list_remote_workers(server, workflow_id, context).await
    }
);
path_handler!(
    create_remote_workers_route,
    i64,
    |workflow_id, server, request, context| {
        handle_create_remote_workers(server, workflow_id, request, context).await
    }
);
path_handler!(
    delete_remote_worker_route,
    (i64, String),
    |(workflow_id, worker), server, request, context| {
        handle_delete_remote_worker(server, workflow_id, worker, context).await
    }
);

req_handler!(create_ro_crate_entity_route, |server, request, context| {
    handle_create_ro_crate_entity(server, request, context).await
});
path_handler!(
    get_ro_crate_entity_route,
    i64,
    |id, server, request, context| { handle_get_ro_crate_entity(server, id, context).await }
);
path_handler!(
    update_ro_crate_entity_route,
    i64,
    |id, server, request, context| {
        handle_update_ro_crate_entity(server, id, request, context).await
    }
);
path_handler!(
    delete_ro_crate_entity_route,
    i64,
    |id, server, request, context| {
        handle_delete_ro_crate_entity(server, id, request, context).await
    }
);
path_handler!(
    list_ro_crate_entities_route,
    i64,
    |workflow_id, server, request, context| {
        handle_list_ro_crate_entities(server, workflow_id, request, context).await
    }
);
path_handler!(
    delete_ro_crate_entities_route,
    i64,
    |workflow_id, server, request, context| {
        handle_delete_ro_crate_entities(server, workflow_id, request, context).await
    }
);

req_handler!(list_workflows_route, |server, request, context| {
    handle_list_workflows(server, request, context).await
});
req_handler!(create_workflow_route, |server, request, context| {
    handle_create_workflow(server, request, context).await
});
path_handler!(get_workflow_route, i64, |id, server, request, context| {
    handle_get_workflow(server, id, context).await
});
path_handler!(
    update_workflow_route,
    i64,
    |id, server, request, context| { handle_update_workflow(server, id, request, context).await }
);
path_handler!(
    delete_workflow_route,
    i64,
    |id, server, request, context| { handle_delete_workflow(server, id, request, context).await }
);
path_handler!(
    cancel_workflow_route,
    i64,
    |id, server, request, context| { handle_cancel_workflow(server, id, request, context).await }
);
path_handler!(
    initialize_jobs_route,
    i64,
    |id, server, request, context| { handle_initialize_jobs(server, id, request, context).await }
);
path_handler!(
    is_workflow_complete_route,
    i64,
    |id, server, request, context| { handle_is_workflow_complete(server, id, context).await }
);
path_handler!(
    is_workflow_uninitialized_route,
    i64,
    |id, server, request, context| { handle_is_workflow_uninitialized(server, id, context).await }
);
path_handler!(
    reset_workflow_status_route,
    i64,
    |id, server, request, context| {
        handle_reset_workflow_status(server, id, request, context).await
    }
);
path_handler!(
    reset_job_status_route,
    i64,
    |id, server, request, context| { handle_reset_job_status(server, id, request, context).await }
);
path_handler!(
    get_workflow_status_route,
    i64,
    |id, server, request, context| { handle_get_workflow_status(server, id, context).await }
);
path_handler!(
    update_workflow_status_route,
    i64,
    |id, server, request, context| {
        handle_update_workflow_status(server, id, request, context).await
    }
);
path_handler!(
    claim_jobs_based_on_resources_route,
    (i64, i64),
    |(id, limit), server, request, context| {
        handle_claim_jobs_based_on_resources(server, id, limit, request, context).await
    }
);
path_handler!(
    claim_next_jobs_route,
    i64,
    |id, server, request, context| { handle_claim_next_jobs(server, id, request, context).await }
);
path_handler!(
    list_job_dependencies_route,
    i64,
    |id, server, request, context| {
        handle_list_job_dependencies(server, id, request, context).await
    }
);
path_handler!(
    list_job_file_relationships_route,
    i64,
    |id, server, request, context| {
        handle_list_job_file_relationships(server, id, request, context).await
    }
);
path_handler!(
    list_job_user_data_relationships_route,
    i64,
    |id, server, request, context| {
        handle_list_job_user_data_relationships(server, id, request, context).await
    }
);
path_handler!(list_job_ids_route, i64, |id, server, request, context| {
    handle_list_job_ids(server, id, context).await
});
path_handler!(
    list_missing_user_data_route,
    i64,
    |id, server, request, context| { handle_list_missing_user_data(server, id, context).await }
);
path_handler!(
    process_changed_job_inputs_route,
    i64,
    |id, server, request, context| {
        handle_process_changed_job_inputs(server, id, request, context).await
    }
);
path_handler!(
    get_ready_job_requirements_route,
    i64,
    |id, server, request, context| {
        handle_get_ready_job_requirements(server, id, request, context).await
    }
);
path_handler!(
    list_required_existing_files_route,
    i64,
    |id, server, request, context| {
        handle_list_required_existing_files(server, id, context).await
    }
);
path_handler!(
    workflow_events_stream_route,
    i64,
    |id, server, request, context| {
        handle_workflow_events_stream(server, id, request, context).await
    }
);
path_handler!(
    get_dot_graph_route,
    (i64, String),
    |(id, name), server, request, context| {
        handle_get_dot_graph(server, id, name, context).await
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
    use crate::models::{ComputeNodeModel, WorkflowModel};
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
