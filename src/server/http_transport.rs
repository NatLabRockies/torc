//! Permanent HTTP transport for the live Torc server.

include!("http_transport/access_control.rs");
include!("http_transport/files.rs");
include!("http_transport/jobs.rs");
include!("http_transport/results.rs");
include!("http_transport/ro_crate.rs");
include!("http_transport/compute_nodes.rs");
include!("http_transport/local_schedulers.rs");
include!("http_transport/resource_requirements.rs");
include!("http_transport/remote_workers.rs");
include!("http_transport/scheduled_compute_nodes.rs");
include!("http_transport/slurm_schedulers.rs");
include!("http_transport/slurm_stats.rs");
include!("http_transport/system.rs");
include!("http_transport/user_data.rs");
include!("http_transport/workflows.rs");

use crate::models;
use crate::openapi_codegen::{OpenApiAppState, PingResponse, VersionResponse};
use crate::server::api_contract::{TransportApi, TransportApiCore};
use crate::server::dashboard::serve_dashboard;
use crate::server::http_server::Server;
use crate::server::response_types::{
    access::*, artifacts::*, events::*, jobs::*, scheduling::*, system::*, workflows::*,
};
use crate::server::transport_types::auth_types::Authorization;
use crate::server::transport_types::context_types::{Has, XSpanIdString};
use axum::body::Body;
use axum::http::header::{CONTENT_TYPE, HeaderValue};
use axum::http::{Method, Request, Response, StatusCode};
use futures::future::BoxFuture;
use http_body::Body as HttpBody;
use http_body_util::BodyExt;
use std::collections::HashMap;
use std::task::{Context, Poll};
use tower::Service;
use url::form_urlencoded;

#[derive(Clone)]
pub struct MakeHttpFallbackService;

impl MakeHttpFallbackService {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MakeHttpFallbackService {
    fn default() -> Self {
        Self::new()
    }
}

impl<Target> Service<Target> for MakeHttpFallbackService
where
    Target: Send,
{
    type Response = HttpFallbackService;
    type Error = std::convert::Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _target: Target) -> Self::Future {
        Box::pin(async move { Ok(HttpFallbackService) })
    }
}

#[derive(Clone)]
pub struct HttpFallbackService;

impl<B, C> Service<(Request<B>, C)> for HttpFallbackService
where
    B: Send + 'static,
    C: Send + 'static,
{
    type Response = Response<Body>;
    type Error = std::convert::Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request_and_context: (Request<B>, C)) -> Self::Future {
        let (request, _context) = request_and_context;
        let response = serve_dashboard(request.uri().path()).unwrap_or_else(not_found_response);
        Box::pin(async move { Ok(response) })
    }
}

#[derive(Clone)]
pub struct MakeHttpTransportService<T, C> {
    inner: T,
    state: OpenApiAppState,
    server: Server<C>,
}

impl<T, C> MakeHttpTransportService<T, C> {
    pub fn new(inner: T, state: OpenApiAppState, server: Server<C>) -> Self {
        Self {
            inner,
            state,
            server,
        }
    }
}

impl<T, C, Target> Service<Target> for MakeHttpTransportService<T, C>
where
    T: Service<Target>,
    T::Future: Send + 'static,
    C: Send + Sync + 'static,
{
    type Response = HttpTransportService<T::Response, C>;
    type Error = T::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, target: Target) -> Self::Future {
        let state = self.state.clone();
        let server = self.server.clone();
        let future = self.inner.call(target);
        Box::pin(async move { Ok(HttpTransportService::new(future.await?, state, server)) })
    }
}

#[derive(Clone)]
pub struct HttpTransportService<T, C> {
    inner: T,
    state: OpenApiAppState,
    server: Server<C>,
}

impl<T, C> HttpTransportService<T, C> {
    fn new(inner: T, state: OpenApiAppState, server: Server<C>) -> Self {
        Self {
            inner,
            state,
            server,
        }
    }

    fn try_intercept<B>(&self, request: &Request<B>) -> Option<Response<Body>> {
        match (request.method(), request.uri().path()) {
            (&Method::GET, "/torc-service/v1/ping") => Some(json_response(&PingResponse {
                status: "ok".to_string(),
            })),
            (&Method::GET, "/torc-service/v1/version") => Some(json_response(&VersionResponse {
                version: self.state.version.clone(),
                api_version: self.state.api_version.clone(),
                git_hash: (!self.state.access_control_enabled)
                    .then_some(self.state.git_hash.clone()),
            })),
            _ => None,
        }
    }
}

impl<T, B, C> Service<(Request<B>, C)> for HttpTransportService<T, C>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    T: Service<(Request<B>, C), Response = Response<Body>>,
    T::Future: Send + 'static,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
    Server<C>: TransportApi<C>,
{
    type Response = Response<Body>;
    type Error = T::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request_and_context: (Request<B>, C)) -> Self::Future {
        let (request, context) = request_and_context;
        if let Some(response) = self.try_intercept(&request) {
            return Box::pin(async move { Ok(response) });
        }

        if request.method() == Method::GET
            && request.uri().path() == "/torc-service/v1/compute_nodes"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_list_compute_nodes(server, request, context).await)
            });
        }

        if request.method() == Method::POST
            && request.uri().path() == "/torc-service/v1/compute_nodes"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_create_compute_node(server, request, context).await)
            });
        }

        if request.method() == Method::DELETE
            && request.uri().path() == "/torc-service/v1/compute_nodes"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_delete_compute_nodes(server, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(id) =
                parse_resource_id(request.uri().path(), "/torc-service/v1/compute_nodes/")
        {
            let server = self.server.clone();
            return Box::pin(async move { Ok(handle_get_compute_node(server, id, context).await) });
        }

        if request.method() == Method::PUT
            && let Some(id) =
                parse_resource_id(request.uri().path(), "/torc-service/v1/compute_nodes/")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_update_compute_node(server, id, request, context).await)
            });
        }

        if request.method() == Method::DELETE
            && let Some(id) =
                parse_resource_id(request.uri().path(), "/torc-service/v1/compute_nodes/")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_delete_compute_node(server, id, request, context).await)
            });
        }

        if request.method() == Method::GET && request.uri().path() == "/torc-service/v1/events" {
            let server = self.server.clone();
            return Box::pin(async move { Ok(handle_list_events(server, request, context).await) });
        }

        if request.method() == Method::POST && request.uri().path() == "/torc-service/v1/events" {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_create_event(server, request, context).await) },
            );
        }

        if request.method() == Method::DELETE && request.uri().path() == "/torc-service/v1/events" {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_delete_events(server, request, context).await) },
            );
        }

        if request.method() == Method::GET
            && let Some(id) = parse_resource_id(request.uri().path(), "/torc-service/v1/events/")
        {
            let server = self.server.clone();
            return Box::pin(async move { Ok(handle_get_event(server, id, context).await) });
        }

        if request.method() == Method::PUT
            && let Some(id) = parse_resource_id(request.uri().path(), "/torc-service/v1/events/")
        {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_update_event(server, id, request, context).await) },
            );
        }

        if request.method() == Method::DELETE
            && let Some(id) = parse_resource_id(request.uri().path(), "/torc-service/v1/events/")
        {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_delete_event(server, id, request, context).await) },
            );
        }

        if request.method() == Method::GET && request.uri().path() == "/torc-service/v1/files" {
            let server = self.server.clone();
            return Box::pin(async move { Ok(handle_list_files(server, request, context).await) });
        }

        if request.method() == Method::POST && request.uri().path() == "/torc-service/v1/files" {
            let server = self.server.clone();
            return Box::pin(async move { Ok(handle_create_file(server, request, context).await) });
        }

        if request.method() == Method::DELETE && request.uri().path() == "/torc-service/v1/files" {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_delete_files(server, request, context).await) },
            );
        }

        if request.method() == Method::GET
            && let Some(id) = parse_resource_id(request.uri().path(), "/torc-service/v1/files/")
        {
            let server = self.server.clone();
            return Box::pin(async move { Ok(handle_get_file(server, id, context).await) });
        }

        if request.method() == Method::PUT
            && let Some(id) = parse_resource_id(request.uri().path(), "/torc-service/v1/files/")
        {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_update_file(server, id, request, context).await) },
            );
        }

        if request.method() == Method::DELETE
            && let Some(id) = parse_resource_id(request.uri().path(), "/torc-service/v1/files/")
        {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_delete_file(server, id, request, context).await) },
            );
        }

        if request.method() == Method::GET
            && request.uri().path() == "/torc-service/v1/local_schedulers"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_list_local_schedulers(server, request, context).await)
            });
        }

        if request.method() == Method::POST
            && request.uri().path() == "/torc-service/v1/local_schedulers"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_create_local_scheduler(server, request, context).await)
            });
        }

        if request.method() == Method::DELETE
            && request.uri().path() == "/torc-service/v1/local_schedulers"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_delete_local_schedulers(server, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(id) =
                parse_resource_id(request.uri().path(), "/torc-service/v1/local_schedulers/")
        {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_get_local_scheduler(server, id, context).await) },
            );
        }

        if request.method() == Method::PUT
            && let Some(id) =
                parse_resource_id(request.uri().path(), "/torc-service/v1/local_schedulers/")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_update_local_scheduler(server, id, request, context).await)
            });
        }

        if request.method() == Method::DELETE
            && let Some(id) =
                parse_resource_id(request.uri().path(), "/torc-service/v1/local_schedulers/")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_delete_local_scheduler(server, id, request, context).await)
            });
        }

        if request.method() == Method::GET && request.uri().path() == "/torc-service/v1/results" {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_list_results(server, request, context).await) },
            );
        }

        if request.method() == Method::POST && request.uri().path() == "/torc-service/v1/results" {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_create_result(server, request, context).await) },
            );
        }

        if request.method() == Method::DELETE && request.uri().path() == "/torc-service/v1/results"
        {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_delete_results(server, request, context).await) },
            );
        }

        if request.method() == Method::GET
            && let Some(id) = parse_resource_id(request.uri().path(), "/torc-service/v1/results/")
        {
            let server = self.server.clone();
            return Box::pin(async move { Ok(handle_get_result(server, id, context).await) });
        }

        if request.method() == Method::PUT
            && let Some(id) = parse_resource_id(request.uri().path(), "/torc-service/v1/results/")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_update_result(server, id, request, context).await)
            });
        }

        if request.method() == Method::DELETE
            && let Some(id) = parse_resource_id(request.uri().path(), "/torc-service/v1/results/")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_delete_result(server, id, request, context).await)
            });
        }

        if request.method() == Method::GET && request.uri().path() == "/torc-service/v1/user_data" {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_list_user_data(server, request, context).await) },
            );
        }

        if request.method() == Method::POST && request.uri().path() == "/torc-service/v1/user_data"
        {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_create_user_data(server, request, context).await) },
            );
        }

        if request.method() == Method::DELETE
            && request.uri().path() == "/torc-service/v1/user_data"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_delete_all_user_data(server, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(id) = parse_resource_id(request.uri().path(), "/torc-service/v1/user_data/")
        {
            let server = self.server.clone();
            return Box::pin(async move { Ok(handle_get_user_data(server, id, context).await) });
        }

        if request.method() == Method::PUT
            && let Some(id) = parse_resource_id(request.uri().path(), "/torc-service/v1/user_data/")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_update_user_data(server, id, request, context).await)
            });
        }

        if request.method() == Method::DELETE
            && let Some(id) = parse_resource_id(request.uri().path(), "/torc-service/v1/user_data/")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_delete_user_data(server, id, request, context).await)
            });
        }

        if request.method() == Method::GET
            && request.uri().path() == "/torc-service/v1/scheduled_compute_nodes"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_list_scheduled_compute_nodes(server, request, context).await)
            });
        }

        if request.method() == Method::POST
            && request.uri().path() == "/torc-service/v1/scheduled_compute_nodes"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_create_scheduled_compute_node(server, request, context).await)
            });
        }

        if request.method() == Method::DELETE
            && request.uri().path() == "/torc-service/v1/scheduled_compute_nodes"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_delete_scheduled_compute_nodes(server, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(id) = parse_resource_id(
                request.uri().path(),
                "/torc-service/v1/scheduled_compute_nodes/",
            )
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_get_scheduled_compute_node(server, id, context).await)
            });
        }

        if request.method() == Method::PUT
            && let Some(id) = parse_resource_id(
                request.uri().path(),
                "/torc-service/v1/scheduled_compute_nodes/",
            )
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_update_scheduled_compute_node(server, id, request, context).await)
            });
        }

        if request.method() == Method::DELETE
            && let Some(id) = parse_resource_id(
                request.uri().path(),
                "/torc-service/v1/scheduled_compute_nodes/",
            )
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_delete_scheduled_compute_node(server, id, request, context).await)
            });
        }

        if request.method() == Method::GET
            && request.uri().path() == "/torc-service/v1/slurm_schedulers"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_list_slurm_schedulers(server, request, context).await)
            });
        }

        if request.method() == Method::POST
            && request.uri().path() == "/torc-service/v1/slurm_schedulers"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_create_slurm_scheduler(server, request, context).await)
            });
        }

        if request.method() == Method::DELETE
            && request.uri().path() == "/torc-service/v1/slurm_schedulers"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_delete_slurm_schedulers(server, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(id) =
                parse_resource_id(request.uri().path(), "/torc-service/v1/slurm_schedulers/")
        {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_get_slurm_scheduler(server, id, context).await) },
            );
        }

        if request.method() == Method::PUT
            && let Some(id) =
                parse_resource_id(request.uri().path(), "/torc-service/v1/slurm_schedulers/")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_update_slurm_scheduler(server, id, request, context).await)
            });
        }

        if request.method() == Method::DELETE
            && let Some(id) =
                parse_resource_id(request.uri().path(), "/torc-service/v1/slurm_schedulers/")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_delete_slurm_scheduler(server, id, request, context).await)
            });
        }

        if request.method() == Method::GET
            && request.uri().path() == "/torc-service/v1/access_groups"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_list_access_groups(server, request, context).await)
            });
        }

        if request.method() == Method::POST
            && request.uri().path() == "/torc-service/v1/access_groups"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_create_access_group(server, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(id) =
                parse_resource_id(request.uri().path(), "/torc-service/v1/access_groups/")
        {
            let server = self.server.clone();
            return Box::pin(async move { Ok(handle_get_access_group(server, id, context).await) });
        }

        if request.method() == Method::DELETE
            && let Some(id) =
                parse_resource_id(request.uri().path(), "/torc-service/v1/access_groups/")
        {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_delete_access_group(server, id, context).await) },
            );
        }

        if request.method() == Method::GET
            && let Some(id) = parse_access_group_members_collection_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_list_group_members(server, id, request, context).await)
            });
        }

        if request.method() == Method::POST
            && let Some(id) = parse_access_group_members_collection_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_add_user_to_group(server, id, request, context).await)
            });
        }

        if request.method() == Method::DELETE
            && let Some((group_id, user_name)) = parse_group_member_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_remove_user_from_group(server, group_id, user_name, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(user_name) = parse_user_groups_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_list_user_groups(server, user_name, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(workflow_id) =
                parse_workflow_access_groups_collection_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_list_workflow_groups(server, workflow_id, request, context).await)
            });
        }

        if request.method() == Method::POST
            && let Some(workflow_id) =
                parse_workflow_access_groups_collection_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_add_workflow_to_group(server, workflow_id, request, context).await)
            });
        }

        if request.method() == Method::POST
            && let Some((workflow_id, group_id)) =
                parse_workflow_access_group_item_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(
                    handle_add_workflow_to_group_by_path(server, workflow_id, group_id, context)
                        .await,
                )
            });
        }

        if request.method() == Method::DELETE
            && let Some((workflow_id, group_id)) =
                parse_workflow_access_group_item_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_remove_workflow_from_group(server, workflow_id, group_id, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some((workflow_id, user_name)) = parse_access_check_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_check_workflow_access(server, workflow_id, user_name, context).await)
            });
        }

        if request.method() == Method::POST
            && request.uri().path() == "/torc-service/v1/admin/reload-auth"
        {
            let server = self.server.clone();
            return Box::pin(async move { Ok(handle_reload_auth(server, context).await) });
        }

        if request.method() == Method::POST && request.uri().path() == "/torc-service/v1/bulk_jobs"
        {
            let server = self.server.clone();
            return Box::pin(async move { Ok(handle_create_jobs(server, request, context).await) });
        }

        if request.method() == Method::POST
            && request.uri().path() == "/torc-service/v1/resource_requirements"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_create_resource_requirements(server, request, context).await)
            });
        }

        if request.method() == Method::GET
            && request.uri().path() == "/torc-service/v1/resource_requirements"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_list_resource_requirements(server, request, context).await)
            });
        }

        if request.method() == Method::DELETE
            && request.uri().path() == "/torc-service/v1/resource_requirements"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_delete_all_resource_requirements(server, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(id) = parse_resource_id(
                request.uri().path(),
                "/torc-service/v1/resource_requirements/",
            )
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_get_resource_requirements(server, id, context).await)
            });
        }

        if request.method() == Method::PUT
            && let Some(id) = parse_resource_id(
                request.uri().path(),
                "/torc-service/v1/resource_requirements/",
            )
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_update_resource_requirements(server, id, request, context).await)
            });
        }

        if request.method() == Method::DELETE
            && let Some(id) = parse_resource_id(
                request.uri().path(),
                "/torc-service/v1/resource_requirements/",
            )
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_delete_resource_requirements(server, id, request, context).await)
            });
        }

        if request.method() == Method::POST
            && request.uri().path() == "/torc-service/v1/failure_handlers"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_create_failure_handler(server, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(id) =
                parse_resource_id(request.uri().path(), "/torc-service/v1/failure_handlers/")
        {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_get_failure_handler(server, id, context).await) },
            );
        }

        if request.method() == Method::DELETE
            && let Some(id) =
                parse_resource_id(request.uri().path(), "/torc-service/v1/failure_handlers/")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_delete_failure_handler(server, id, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(workflow_id) = parse_workflow_failure_handlers_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_list_failure_handlers(server, workflow_id, request, context).await)
            });
        }

        if request.method() == Method::POST
            && request.uri().path() == "/torc-service/v1/slurm_stats"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_create_slurm_stats(server, request, context).await)
            });
        }

        if request.method() == Method::GET && request.uri().path() == "/torc-service/v1/slurm_stats"
        {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_list_slurm_stats(server, request, context).await) },
            );
        }

        if request.method() == Method::POST
            && request.uri().path() == "/torc-service/v1/ro_crate_entities"
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_create_ro_crate_entity(server, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(id) =
                parse_resource_id(request.uri().path(), "/torc-service/v1/ro_crate_entities/")
        {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_get_ro_crate_entity(server, id, context).await) },
            );
        }

        if request.method() == Method::PUT
            && let Some(id) =
                parse_resource_id(request.uri().path(), "/torc-service/v1/ro_crate_entities/")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_update_ro_crate_entity(server, id, request, context).await)
            });
        }

        if request.method() == Method::DELETE
            && let Some(id) =
                parse_resource_id(request.uri().path(), "/torc-service/v1/ro_crate_entities/")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_delete_ro_crate_entity(server, id, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(workflow_id) = parse_workflow_ro_crate_entities_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_list_ro_crate_entities(server, workflow_id, request, context).await)
            });
        }

        if request.method() == Method::DELETE
            && let Some(workflow_id) = parse_workflow_ro_crate_entities_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_delete_ro_crate_entities(server, workflow_id, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(workflow_id) =
                parse_workflow_remote_workers_collection_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_list_remote_workers(server, workflow_id, context).await)
            });
        }

        if request.method() == Method::POST
            && let Some(workflow_id) =
                parse_workflow_remote_workers_collection_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_create_remote_workers(server, workflow_id, request, context).await)
            });
        }

        if request.method() == Method::DELETE
            && let Some((workflow_id, worker)) =
                parse_workflow_remote_worker_item_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_delete_remote_worker(server, workflow_id, worker, context).await)
            });
        }

        if request.method() == Method::GET && request.uri().path() == "/torc-service/v1/jobs" {
            let server = self.server.clone();
            return Box::pin(async move { Ok(handle_list_jobs(server, request, context).await) });
        }

        if request.method() == Method::POST && request.uri().path() == "/torc-service/v1/jobs" {
            let server = self.server.clone();
            return Box::pin(async move { Ok(handle_create_job(server, request, context).await) });
        }

        if request.method() == Method::DELETE && request.uri().path() == "/torc-service/v1/jobs" {
            let server = self.server.clone();
            return Box::pin(async move { Ok(handle_delete_jobs(server, request, context).await) });
        }

        if request.method() == Method::POST
            && let Some((id, status, run_id)) = parse_job_status_run_path(
                request.uri().path(),
                "/torc-service/v1/jobs/",
                "/complete_job/",
            )
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_complete_job(server, id, status, run_id, request, context).await)
            });
        }

        if request.method() == Method::PUT
            && let Some((id, status, run_id)) = parse_job_status_run_path(
                request.uri().path(),
                "/torc-service/v1/jobs/",
                "/manage_status_change/",
            )
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_manage_status_change(server, id, status, run_id, request, context).await)
            });
        }

        if request.method() == Method::PUT
            && let Some((id, run_id, compute_node_id)) = parse_job_start_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_start_job(server, id, run_id, compute_node_id, request, context).await)
            });
        }

        if request.method() == Method::POST
            && let Some((id, run_id)) = parse_job_retry_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_retry_job(server, id, run_id, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(id) = parse_resource_id(request.uri().path(), "/torc-service/v1/jobs/")
        {
            let server = self.server.clone();
            return Box::pin(async move { Ok(handle_get_job(server, id, context).await) });
        }

        if request.method() == Method::PUT
            && let Some(id) = parse_resource_id(request.uri().path(), "/torc-service/v1/jobs/")
        {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_update_job(server, id, request, context).await) },
            );
        }

        if request.method() == Method::DELETE
            && let Some(id) = parse_resource_id(request.uri().path(), "/torc-service/v1/jobs/")
        {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_delete_job(server, id, request, context).await) },
            );
        }

        if request.method() == Method::GET && request.uri().path() == "/torc-service/v1/workflows" {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_list_workflows(server, request, context).await) },
            );
        }

        if request.method() == Method::POST && request.uri().path() == "/torc-service/v1/workflows"
        {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_create_workflow(server, request, context).await) },
            );
        }

        if request.method() == Method::GET
            && let Some(workflow_id) = parse_workflow_actions_collection_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_get_workflow_actions(server, workflow_id, context).await)
            });
        }

        if request.method() == Method::POST
            && let Some(workflow_id) = parse_workflow_actions_collection_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_create_workflow_action(server, workflow_id, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(workflow_id) = parse_workflow_pending_actions_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_get_pending_actions(server, workflow_id, request, context).await)
            });
        }

        if request.method() == Method::POST
            && let Some((workflow_id, action_id)) =
                parse_workflow_action_claim_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_claim_action(server, workflow_id, action_id, request, context).await)
            });
        }

        if request.method() == Method::PUT
            && let Some(id) = parse_workflow_suffix_path(request.uri().path(), "/cancel")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_cancel_workflow(server, id, request, context).await)
            });
        }

        if request.method() == Method::POST
            && let Some((id, limit)) =
                parse_workflow_claim_jobs_resources_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_claim_jobs_based_on_resources(server, id, limit, request, context).await)
            });
        }

        if request.method() == Method::POST
            && let Some(id) = parse_workflow_suffix_path(request.uri().path(), "/claim_next_jobs")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_claim_next_jobs(server, id, request, context).await)
            });
        }

        if request.method() == Method::POST
            && let Some(id) = parse_workflow_suffix_path(request.uri().path(), "/initialize_jobs")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_initialize_jobs(server, id, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(id) = parse_workflow_suffix_path(request.uri().path(), "/is_complete")
        {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_is_workflow_complete(server, id, context).await) },
            );
        }

        if request.method() == Method::GET
            && let Some(id) = parse_workflow_suffix_path(request.uri().path(), "/is_uninitialized")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_is_workflow_uninitialized(server, id, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(id) = parse_workflow_suffix_path(request.uri().path(), "/job_dependencies")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_list_job_dependencies(server, id, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(id) =
                parse_workflow_suffix_path(request.uri().path(), "/job_file_relationships")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_list_job_file_relationships(server, id, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(id) = parse_workflow_suffix_path(request.uri().path(), "/job_ids")
        {
            let server = self.server.clone();
            return Box::pin(async move { Ok(handle_list_job_ids(server, id, context).await) });
        }

        if request.method() == Method::GET
            && let Some(id) =
                parse_workflow_suffix_path(request.uri().path(), "/job_user_data_relationships")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_list_job_user_data_relationships(server, id, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(id) = parse_workflow_suffix_path(request.uri().path(), "/missing_user_data")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_list_missing_user_data(server, id, context).await)
            });
        }

        if request.method() == Method::POST
            && let Some(id) =
                parse_workflow_suffix_path(request.uri().path(), "/process_changed_job_inputs")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_process_changed_job_inputs(server, id, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(id) =
                parse_workflow_suffix_path(request.uri().path(), "/ready_job_requirements")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_get_ready_job_requirements(server, id, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(id) =
                parse_workflow_suffix_path(request.uri().path(), "/required_existing_files")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_list_required_existing_files(server, id, context).await)
            });
        }

        if request.method() == Method::POST
            && let Some(id) = parse_workflow_suffix_path(request.uri().path(), "/reset_job_status")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_reset_job_status(server, id, request, context).await)
            });
        }

        if request.method() == Method::POST
            && let Some(id) = parse_workflow_suffix_path(request.uri().path(), "/reset_status")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_reset_workflow_status(server, id, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(id) = parse_workflow_suffix_path(request.uri().path(), "/status")
        {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_get_workflow_status(server, id, context).await) },
            );
        }

        if request.method() == Method::PUT
            && let Some(id) = parse_workflow_suffix_path(request.uri().path(), "/status")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_update_workflow_status(server, id, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some(id) = parse_workflow_events_stream_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_workflow_events_stream(server, id, request, context).await)
            });
        }

        if request.method() == Method::GET
            && let Some((id, name)) = parse_workflow_dot_graph_path(request.uri().path())
        {
            let server = self.server.clone();
            return Box::pin(
                async move { Ok(handle_get_dot_graph(server, id, name, context).await) },
            );
        }

        if request.method() == Method::GET
            && let Some(id) = parse_resource_id(request.uri().path(), "/torc-service/v1/workflows/")
        {
            let server = self.server.clone();
            return Box::pin(async move { Ok(handle_get_workflow(server, id, context).await) });
        }

        if request.method() == Method::PUT
            && let Some(id) = parse_resource_id(request.uri().path(), "/torc-service/v1/workflows/")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_update_workflow(server, id, request, context).await)
            });
        }

        if request.method() == Method::DELETE
            && let Some(id) = parse_resource_id(request.uri().path(), "/torc-service/v1/workflows/")
        {
            let server = self.server.clone();
            return Box::pin(async move {
                Ok(handle_delete_workflow(server, id, request, context).await)
            });
        }

        if is_known_api_path(request.uri().path()) {
            return Box::pin(async move { Ok(method_not_allowed_response()) });
        }

        Box::pin(self.inner.call((request, context)))
    }
}

include!("http_transport/query_parsing.rs");
include!("http_transport/path_parsing.rs");
include!("http_transport/request_parsing.rs");
include!("http_transport/response_mapping.rs");

#[cfg(test)]
mod http_transport_tests {
    use super::*;
    use crate::models::{ComputeNodeModel, WorkflowModel};
    use crate::server::api_contract::TransportApiCore;
    use crate::server::auth::{SharedCredentialCache, SharedHtpasswd};
    use crate::server::response_types::workflows::CreateWorkflowResponse;
    use crate::server::transport_types::auth_types::AuthData;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use parking_lot::RwLock;
    use serde::de::DeserializeOwned;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::convert::Infallible;
    use std::str::FromStr;
    use std::sync::Arc;

    #[derive(Clone, Default)]
    struct PassthroughService;

    #[derive(Clone, Default)]
    struct TestContext {
        span_id: XSpanIdString,
        auth_data: Option<AuthData>,
        authorization: Option<Authorization>,
    }

    impl Has<XSpanIdString> for TestContext {
        fn get(&self) -> &XSpanIdString {
            &self.span_id
        }

        fn get_mut(&mut self) -> &mut XSpanIdString {
            &mut self.span_id
        }

        fn set(&mut self, value: XSpanIdString) {
            self.span_id = value;
        }
    }

    impl Has<Option<AuthData>> for TestContext {
        fn get(&self) -> &Option<AuthData> {
            &self.auth_data
        }

        fn get_mut(&mut self) -> &mut Option<AuthData> {
            &mut self.auth_data
        }

        fn set(&mut self, value: Option<AuthData>) {
            self.auth_data = value;
        }
    }

    impl Has<Option<Authorization>> for TestContext {
        fn get(&self) -> &Option<Authorization> {
            &self.authorization
        }

        fn get_mut(&mut self) -> &mut Option<Authorization> {
            &mut self.authorization
        }

        fn set(&mut self, value: Option<Authorization>) {
            self.authorization = value;
        }
    }

    impl<B> Service<(Request<B>, TestContext)> for PassthroughService
    where
        B: Send + 'static,
    {
        type Response = Response<Body>;
        type Error = Infallible;
        type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _request: (Request<B>, TestContext)) -> Self::Future {
            Box::pin(async move {
                Ok(Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::empty())
                    .expect("valid passthrough response"))
            })
        }
    }

    #[tokio::test]
    async fn intercepts_ping() {
        let mut service = HttpTransportService::new(
            PassthroughService,
            OpenApiAppState::default(),
            test_server(),
        );
        let response = service
            .call((
                Request::builder()
                    .method(Method::GET)
                    .uri("/torc-service/v1/ping")
                    .body(Body::empty())
                    .expect("valid request"),
                TestContext::default(),
            ))
            .await
            .expect("bridge response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn delegates_non_bridge_routes() {
        let mut service = HttpTransportService::new(
            PassthroughService,
            OpenApiAppState::default(),
            test_server(),
        );
        let response = service
            .call((
                Request::builder()
                    .method(Method::GET)
                    .uri("/torc-service/v1/not-bridged")
                    .body(Body::empty())
                    .expect("valid request"),
                TestContext::default(),
            ))
            .await
            .expect("bridge response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn returns_method_not_allowed_for_known_bridge_path() {
        let mut service = HttpTransportService::new(
            PassthroughService,
            OpenApiAppState::default(),
            test_server(),
        );
        let response = service
            .call((
                Request::builder()
                    .method(Method::POST)
                    .uri("/torc-service/v1/ping")
                    .body(Body::empty())
                    .expect("valid request"),
                TestContext::default(),
            ))
            .await
            .expect("bridge response");

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn create_and_list_compute_nodes_round_trip() {
        let server = test_server_with_schema().await;
        let context = TestContext::default();
        let workflow_id = create_workflow_record(&server, &context).await;
        let mut service =
            HttpTransportService::new(PassthroughService, OpenApiAppState::default(), server);

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

        let create_response = service
            .call((
                Request::builder()
                    .method(Method::POST)
                    .uri("/torc-service/v1/compute_nodes")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&create_body).expect("serialize compute node"),
                    ))
                    .expect("valid request"),
                context.clone(),
            ))
            .await
            .expect("create response");

        assert_eq!(create_response.status(), StatusCode::OK);
        let created: ComputeNodeModel = read_json_body(create_response).await;
        assert_eq!(created.hostname, "node-a");
        assert_eq!(created.workflow_id, workflow_id);

        let list_response = service
            .call((
                Request::builder()
                    .method(Method::GET)
                    .uri(format!(
                        "/torc-service/v1/compute_nodes?workflow_id={workflow_id}"
                    ))
                    .body(Body::empty())
                    .expect("valid request"),
                context,
            ))
            .await
            .expect("list response");

        assert_eq!(list_response.status(), StatusCode::OK);
        let listed: serde_json::Value = read_json_body(list_response).await;
        let items = listed["items"].as_array().expect("list items array");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["hostname"], "node-a");
    }

    #[tokio::test]
    async fn get_workflow_round_trip() {
        let server = test_server_with_schema().await;
        let context = TestContext::default();
        let workflow_id = create_workflow_record(&server, &context).await;
        let mut service =
            HttpTransportService::new(PassthroughService, OpenApiAppState::default(), server);

        let response = service
            .call((
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/torc-service/v1/workflows/{workflow_id}"))
                    .body(Body::empty())
                    .expect("valid request"),
                context,
            ))
            .await
            .expect("workflow response");

        assert_eq!(response.status(), StatusCode::OK);
        let workflow: WorkflowModel = read_json_body(response).await;
        assert_eq!(workflow.id, Some(workflow_id));
        assert_eq!(workflow.name, "transport-workflow");
    }

    #[test]
    fn parses_workflow_events_stream_path_and_level() {
        assert_eq!(
            parse_workflow_events_stream_path("/torc-service/v1/workflows/7/events/stream"),
            Some(7)
        );
        assert_eq!(
            parse_event_stream_level(Some("level=warning")),
            models::EventSeverity::Warning
        );
        assert_eq!(
            parse_event_stream_level(Some("level=invalid")),
            models::EventSeverity::Info
        );
    }

    #[test]
    fn parses_compute_nodes_query() {
        let parsed = parse_compute_nodes_query(Some(
            "workflow_id=7&offset=1&limit=2&sort_by=hostname&reverse_sort=true&hostname=node01&is_active=false&scheduled_compute_node_id=9",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            ComputeNodesQuery {
                workflow_id: 7,
                offset: Some(1),
                limit: Some(2),
                sort_by: Some("hostname".to_string()),
                reverse_sort: Some(true),
                hostname: Some("node01".to_string()),
                is_active: Some(false),
                scheduled_compute_node_id: Some(9),
            }
        );
    }

    #[test]
    fn rejects_missing_workflow_id() {
        let err = parse_compute_nodes_query(Some("limit=2")).expect_err("missing workflow id");
        assert!(err.contains("workflow_id"));
    }

    #[test]
    fn parses_events_query() {
        let parsed = parse_events_query(Some(
            "workflow_id=7&offset=1&limit=2&sort_by=timestamp&reverse_sort=false&category=system&after_timestamp=42",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            EventsQuery {
                workflow_id: 7,
                offset: Some(1),
                limit: Some(2),
                sort_by: Some("timestamp".to_string()),
                reverse_sort: Some(false),
                category: Some("system".to_string()),
                after_timestamp: Some(42),
            }
        );
    }

    #[test]
    fn parses_files_query() {
        let parsed = parse_files_query(Some(
            "workflow_id=7&produced_by_job_id=3&offset=1&limit=2&sort_by=name&reverse_sort=true&name=out.txt&path=%2Ftmp&is_output=false",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            FilesQuery {
                workflow_id: 7,
                produced_by_job_id: Some(3),
                offset: Some(1),
                limit: Some(2),
                sort_by: Some("name".to_string()),
                reverse_sort: Some(true),
                name: Some("out.txt".to_string()),
                path: Some("/tmp".to_string()),
                is_output: Some(false),
            }
        );
    }

    #[test]
    fn parses_local_schedulers_query() {
        let parsed = parse_local_schedulers_query(Some(
            "workflow_id=7&offset=1&limit=2&sort_by=memory&reverse_sort=false&memory=4g&num_cpus=8",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            LocalSchedulersQuery {
                workflow_id: 7,
                offset: Some(1),
                limit: Some(2),
                sort_by: Some("memory".to_string()),
                reverse_sort: Some(false),
                memory: Some("4g".to_string()),
                num_cpus: Some(8),
            }
        );
    }

    #[test]
    fn parses_results_query() {
        let parsed = parse_results_query(Some(
            "workflow_id=7&job_id=3&run_id=5&return_code=0&status=completed&compute_node_id=9&offset=1&limit=2&sort_by=run_id&reverse_sort=true&all_runs=false",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            ResultsQuery {
                workflow_id: 7,
                job_id: Some(3),
                run_id: Some(5),
                return_code: Some(0),
                status: Some(models::JobStatus::Completed),
                compute_node_id: Some(9),
                offset: Some(1),
                limit: Some(2),
                sort_by: Some("run_id".to_string()),
                reverse_sort: Some(true),
                all_runs: Some(false),
            }
        );
    }

    #[test]
    fn parses_user_data_query() {
        let parsed = parse_user_data_query(Some(
            "workflow_id=7&consumer_job_id=3&producer_job_id=5&offset=1&limit=2&sort_by=name&reverse_sort=false&name=blob&is_ephemeral=true",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            UserDataQuery {
                workflow_id: 7,
                consumer_job_id: Some(3),
                producer_job_id: Some(5),
                offset: Some(1),
                limit: Some(2),
                sort_by: Some("name".to_string()),
                reverse_sort: Some(false),
                name: Some("blob".to_string()),
                is_ephemeral: Some(true),
            }
        );
    }

    #[test]
    fn parses_scheduled_compute_nodes_query() {
        let parsed = parse_scheduled_compute_nodes_query(Some(
            "workflow_id=7&offset=1&limit=2&sort_by=status&reverse_sort=true&scheduler_id=sched-1&scheduler_config_id=config-2&status=running",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            ScheduledComputeNodesQuery {
                workflow_id: 7,
                offset: Some(1),
                limit: Some(2),
                sort_by: Some("status".to_string()),
                reverse_sort: Some(true),
                scheduler_id: Some("sched-1".to_string()),
                scheduler_config_id: Some("config-2".to_string()),
                status: Some("running".to_string()),
            }
        );
    }

    #[test]
    fn parses_slurm_schedulers_query() {
        let parsed = parse_slurm_schedulers_query(Some(
            "workflow_id=7&offset=1&limit=2&sort_by=name&reverse_sort=false",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            SlurmSchedulersQuery {
                workflow_id: 7,
                offset: Some(1),
                limit: Some(2),
                sort_by: Some("name".to_string()),
                reverse_sort: Some(false),
            }
        );
    }

    #[test]
    fn parses_access_pagination_query() {
        let parsed = parse_access_pagination_query(Some("offset=3&limit=25")).expect("valid query");

        assert_eq!(
            parsed,
            AccessPaginationQuery {
                offset: Some(3),
                limit: Some(25),
            }
        );
    }

    #[test]
    fn parses_resource_requirements_query() {
        let parsed = parse_resource_requirements_query(Some(
            "workflow_id=7&job_id=3&name=default&memory=16g&num_cpus=4&num_gpus=1&num_nodes=2&runtime=3600&offset=1&limit=10&sort_by=name&reverse_sort=true",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            ResourceRequirementsQuery {
                workflow_id: 7,
                job_id: Some(3),
                name: Some("default".to_string()),
                memory: Some("16g".to_string()),
                num_cpus: Some(4),
                num_gpus: Some(1),
                num_nodes: Some(2),
                runtime: Some(3600),
                offset: Some(1),
                limit: Some(10),
                sort_by: Some("name".to_string()),
                reverse_sort: Some(true),
            }
        );
    }

    #[test]
    fn parses_slurm_stats_query() {
        let parsed = parse_slurm_stats_query(Some(
            "workflow_id=7&job_id=3&run_id=4&attempt_id=5&offset=1&limit=10",
        ))
        .expect("valid query");

        assert_eq!(
            parsed,
            SlurmStatsQuery {
                workflow_id: 7,
                job_id: Some(3),
                run_id: Some(4),
                attempt_id: Some(5),
                offset: Some(1),
                limit: Some(10),
            }
        );
    }

    #[test]
    fn parses_access_control_paths() {
        assert_eq!(
            parse_access_group_members_collection_path("/torc-service/v1/access_groups/12/members"),
            Some(12)
        );
        assert_eq!(
            parse_group_member_path("/torc-service/v1/access_groups/12/members/alice"),
            Some((12, "alice".to_string()))
        );
        assert_eq!(
            parse_user_groups_path("/torc-service/v1/users/alice/groups"),
            Some("alice".to_string())
        );
        assert_eq!(
            parse_workflow_access_groups_collection_path(
                "/torc-service/v1/workflows/7/access_groups",
            ),
            Some(7)
        );
        assert_eq!(
            parse_workflow_access_group_item_path("/torc-service/v1/workflows/7/access_groups/8",),
            Some((7, 8))
        );
        assert_eq!(
            parse_access_check_path("/torc-service/v1/access_check/7/alice"),
            Some((7, "alice".to_string()))
        );
        assert_eq!(
            parse_workflow_failure_handlers_path("/torc-service/v1/workflows/7/failure_handlers"),
            Some(7)
        );
    }

    async fn test_server_with_schema() -> Server<TestContext> {
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

    fn test_server() -> Server<TestContext> {
        futures::executor::block_on(test_server_with_schema())
    }

    async fn create_workflow_record(server: &Server<TestContext>, context: &TestContext) -> i64 {
        let workflow_response = server
            .create_workflow(
                WorkflowModel::new("transport-workflow".to_string(), "test-user".to_string()),
                context,
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
