#![allow(
    missing_docs,
    trivial_casts,
    unused_variables,
    unused_mut,
    unused_imports,
    unused_extern_crates,
    unused_attributes,
    non_camel_case_types
)]
#![allow(
    clippy::derive_partial_eq_without_eq,
    clippy::disallowed_names,
    clippy::large_enum_variant
)]

//! OpenAPI-generated API types and trait definitions

use crate::models;
use crate::server::event_broadcast::BroadcastEvent;
use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::error::Error;
use std::task::{Context, Poll};
use swagger::auth::Authorization;
use swagger::{ApiError, ContextWrapper};
use tokio::sync::broadcast;

pub type ServiceError = Box<dyn Error + Send + Sync + 'static>;

pub const BASE_PATH: &str = "/torc-service/v1";
pub const API_VERSION: &str = "0.8.0";

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum CreateComputeNodeResponse {
    /// Successful response
    SuccessfulResponse(models::ComputeNodeModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum CreateEventResponse {
    /// Successful response
    SuccessfulResponse(models::EventModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum CreateFileResponse {
    /// Successful response
    SuccessfulResponse(models::FileModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum CreateJobResponse {
    /// Successful response
    SuccessfulResponse(models::JobModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum CreateJobsResponse {
    /// Successful response
    SuccessfulResponse(models::CreateJobsResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Workflow not found
    NotFoundErrorResponse(models::ErrorResponse),
    /// Unprocessable content (e.g., jobs have different workflow_ids)
    UnprocessableContentErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum CreateLocalSchedulerResponse {
    /// Successful response
    SuccessfulResponse(models::LocalSchedulerModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum CreateFailureHandlerResponse {
    /// Successful response
    SuccessfulResponse(models::FailureHandlerModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetFailureHandlerResponse {
    /// Successful response
    SuccessfulResponse(models::FailureHandlerModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListFailureHandlersResponse {
    /// Successful response
    SuccessfulResponse(models::ListFailureHandlersResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteFailureHandlerResponse {
    /// Successful response
    SuccessfulResponse(serde_json::Value),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum RetryJobResponse {
    /// Successful response
    SuccessfulResponse(models::JobModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Unprocessable content error response
    UnprocessableContentErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum CreateResourceRequirementsResponse {
    /// Successful response
    SuccessfulResponse(models::ResourceRequirementsModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Unprocessable content error response
    UnprocessableContentErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum CreateResultResponse {
    /// Successful response
    SuccessfulResponse(models::ResultModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum CreateScheduledComputeNodeResponse {
    /// Successful response
    SuccessfulResponse(models::ScheduledComputeNodesModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum CreateSlurmSchedulerResponse {
    /// Response from posting an instance of Slurm compute node configuration.
    SuccessfulResponse(models::SlurmSchedulerModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum CreateUserDataResponse {
    /// Successful response
    SuccessfulResponse(models::UserDataModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum CreateWorkflowResponse {
    /// Successful response
    SuccessfulResponse(models::WorkflowModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum CreateWorkflowActionResponse {
    /// Successful response
    SuccessfulResponse(models::WorkflowActionModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Unprocessable content error response
    UnprocessableContentErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetWorkflowActionsResponse {
    /// Successful response
    SuccessfulResponse(Vec<models::WorkflowActionModel>),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetPendingActionsResponse {
    /// Successful response
    SuccessfulResponse(Vec<models::WorkflowActionModel>),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ClaimActionResponse {
    /// Successful response - action was claimed
    SuccessfulResponse(serde_json::Value),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Conflict - action already claimed
    ConflictResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteComputeNodesResponse {
    /// Successful response
    SuccessfulResponse(serde_json::Value),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteEventsResponse {
    /// Successful response
    SuccessfulResponse(serde_json::Value),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteFilesResponse {
    /// Successful response
    SuccessfulResponse(serde_json::Value),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteJobsResponse {
    /// Successful response
    SuccessfulResponse(serde_json::Value),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteLocalSchedulersResponse {
    /// Successful response
    SuccessfulResponse(serde_json::Value),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteAllResourceRequirementsResponse {
    /// Successful response
    SuccessfulResponse(serde_json::Value),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteResultsResponse {
    /// Successful response
    SuccessfulResponse(serde_json::Value),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteScheduledComputeNodesResponse {
    /// Successful response
    SuccessfulResponse(serde_json::Value),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteSlurmSchedulersResponse {
    /// message
    Message(serde_json::Value),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteAllUserDataResponse {
    /// Successful response
    SuccessfulResponse(serde_json::Value),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetVersionResponse {
    /// Successful response
    SuccessfulResponse(serde_json::Value),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListComputeNodesResponse {
    /// Successful response
    SuccessfulResponse(models::ListComputeNodesResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListEventsResponse {
    /// Successful response
    SuccessfulResponse(models::ListEventsResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListFilesResponse {
    /// Successful response
    SuccessfulResponse(models::ListFilesResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListJobsResponse {
    /// Successful response
    SuccessfulResponse(models::ListJobsResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListJobDependenciesResponse {
    /// Successful response
    SuccessfulResponse(models::ListJobDependenciesResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListJobFileRelationshipsResponse {
    /// Successful response
    SuccessfulResponse(models::ListJobFileRelationshipsResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListJobUserDataRelationshipsResponse {
    /// Successful response
    SuccessfulResponse(models::ListJobUserDataRelationshipsResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListLocalSchedulersResponse {
    /// HTTP 200 OK.
    HTTP(models::ListLocalSchedulersResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListResourceRequirementsResponse {
    /// Successful response
    SuccessfulResponse(models::ListResourceRequirementsResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListResultsResponse {
    /// Successful response
    SuccessfulResponse(models::ListResultsResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListScheduledComputeNodesResponse {
    /// Successful response
    SuccessfulResponse(models::ListScheduledComputeNodesResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListSlurmSchedulersResponse {
    /// Successful response
    SuccessfulResponse(models::ListSlurmSchedulersResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListUserDataResponse {
    /// Successful response
    SuccessfulResponse(models::ListUserDataResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListWorkflowsResponse {
    /// Successful response
    SuccessfulResponse(models::ListWorkflowsResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum PingResponse {
    /// Successful response
    SuccessfulResponse(serde_json::Value),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum CancelWorkflowResponse {
    /// Successful response
    SuccessfulResponse(serde_json::Value),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetComputeNodeResponse {
    /// Successful response
    SuccessfulResponse(models::ComputeNodeModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetEventResponse {
    /// Successful response
    SuccessfulResponse(models::EventModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetFileResponse {
    /// Successful response
    SuccessfulResponse(models::FileModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetJobResponse {
    /// Successful response
    SuccessfulResponse(models::JobModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetLocalSchedulerResponse {
    /// Successful response
    SuccessfulResponse(models::LocalSchedulerModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetReadyJobRequirementsResponse {
    /// Successful response
    SuccessfulResponse(models::GetReadyJobRequirementsResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetResourceRequirementsResponse {
    /// Successful response
    SuccessfulResponse(models::ResourceRequirementsModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetResultResponse {
    /// Successful response
    SuccessfulResponse(models::ResultModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetScheduledComputeNodeResponse {
    /// HTTP 200 OK.
    HTTP(models::ScheduledComputeNodesModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetSlurmSchedulerResponse {
    /// Successful response
    SuccessfulResponse(models::SlurmSchedulerModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetUserDataResponse {
    /// Successful response
    SuccessfulResponse(models::UserDataModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetWorkflowResponse {
    /// Successful response
    SuccessfulResponse(models::WorkflowModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetWorkflowStatusResponse {
    /// Successful response
    SuccessfulResponse(models::WorkflowStatusModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum InitializeJobsResponse {
    /// Successful response
    SuccessfulResponse(serde_json::Value),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum IsWorkflowCompleteResponse {
    /// Successful response
    SuccessfulResponse(models::IsCompleteResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum IsWorkflowUninitializedResponse {
    /// Successful response
    SuccessfulResponse(serde_json::Value),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListJobIdsResponse {
    /// Successful response
    SuccessfulResponse(models::ListJobIdsResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListMissingUserDataResponse {
    /// Successful response
    SuccessfulResponse(models::ListMissingUserDataResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListRequiredExistingFilesResponse {
    /// Successful response
    SuccessfulResponse(models::ListRequiredExistingFilesResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum UpdateComputeNodeResponse {
    /// Successful response
    SuccessfulResponse(models::ComputeNodeModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum UpdateEventResponse {
    /// Successful response
    SuccessfulResponse(models::EventModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum UpdateFileResponse {
    /// Successful response
    SuccessfulResponse(models::FileModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum UpdateJobResponse {
    /// Successful response
    SuccessfulResponse(models::JobModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Unprocessable content error response
    UnprocessableContentErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum UpdateLocalSchedulerResponse {
    /// Successful response
    SuccessfulResponse(models::LocalSchedulerModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum UpdateResourceRequirementsResponse {
    /// Successful response
    SuccessfulResponse(models::ResourceRequirementsModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum UpdateResultResponse {
    /// Successful response
    SuccessfulResponse(models::ResultModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum UpdateScheduledComputeNodeResponse {
    /// scheduled compute node updated in the table.
    ScheduledComputeNodeUpdatedInTheTable(models::ScheduledComputeNodesModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum UpdateSlurmSchedulerResponse {
    /// Successful response
    SuccessfulResponse(models::SlurmSchedulerModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum UpdateUserDataResponse {
    /// Successful response
    SuccessfulResponse(models::UserDataModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum UpdateWorkflowResponse {
    /// Successful response
    SuccessfulResponse(models::WorkflowModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum UpdateWorkflowStatusResponse {
    /// Successful response
    SuccessfulResponse(models::WorkflowStatusModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ClaimJobsBasedOnResources {
    /// Successful response
    SuccessfulResponse(models::ClaimJobsBasedOnResources),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Unprocessable content error response
    UnprocessableContentErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ClaimNextJobsResponse {
    /// Successful response
    SuccessfulResponse(models::ClaimNextJobsResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ProcessChangedJobInputsResponse {
    /// Successful response
    SuccessfulResponse(models::ProcessChangedJobInputsResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteComputeNodeResponse {
    /// Successful response
    SuccessfulResponse(models::ComputeNodeModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteEventResponse {
    /// Successful response
    SuccessfulResponse(models::EventModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteFileResponse {
    /// Successful response
    SuccessfulResponse(models::FileModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteJobResponse {
    /// Successful response
    SuccessfulResponse(models::JobModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteLocalSchedulerResponse {
    /// local compute node configuration stored in the table.
    LocalComputeNodeConfigurationStoredInTheTable(models::LocalSchedulerModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteResourceRequirementsResponse {
    /// Successful response
    SuccessfulResponse(models::ResourceRequirementsModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteResultResponse {
    /// Successful response
    SuccessfulResponse(models::ResultModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteScheduledComputeNodeResponse {
    /// Successful response
    SuccessfulResponse(models::ScheduledComputeNodesModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum CreateRemoteWorkersResponse {
    /// Successful response
    SuccessfulResponse(Vec<models::RemoteWorkerModel>),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response (workflow not found)
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListRemoteWorkersResponse {
    /// Successful response
    SuccessfulResponse(Vec<models::RemoteWorkerModel>),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response (workflow not found)
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteRemoteWorkerResponse {
    /// Successful response
    SuccessfulResponse(models::RemoteWorkerModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response (workflow or worker not found)
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteSlurmSchedulerResponse {
    /// Successful response
    SuccessfulResponse(models::SlurmSchedulerModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteUserDataResponse {
    /// Successful response
    SuccessfulResponse(models::UserDataModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteWorkflowResponse {
    /// Successful response
    SuccessfulResponse(models::WorkflowModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ResetJobStatusResponse {
    /// Successful response
    SuccessfulResponse(models::ResetJobStatusResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ResetWorkflowStatusResponse {
    /// Successful response
    SuccessfulResponse(serde_json::Value),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Unprocessable content error response
    UnprocessableContentErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetDotGraphResponse {
    /// Successful response
    SuccessfulResponse(models::GetDotGraphResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ManageStatusChangeResponse {
    /// Successful response
    SuccessfulResponse(models::JobModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Unprocessable content error response
    UnprocessableContentErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum StartJobResponse {
    /// Successful response
    SuccessfulResponse(models::JobModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Unprocessable content error response
    UnprocessableContentErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum CompleteJobResponse {
    /// Successful response
    SuccessfulResponse(models::JobModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Unprocessable content error response
    UnprocessableContentErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum CreateAccessGroupResponse {
    /// Successful response
    SuccessfulResponse(models::AccessGroupModel),
    /// Forbidden error response
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Conflict error response - group already exists
    ConflictErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetAccessGroupResponse {
    /// Successful response
    SuccessfulResponse(models::AccessGroupModel),
    /// Forbidden error response
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListAccessGroupsApiResponse {
    /// Successful response
    SuccessfulResponse(models::ListAccessGroupsResponse),
    /// Forbidden error response
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteAccessGroupResponse {
    /// Successful response
    SuccessfulResponse(models::AccessGroupModel),
    /// Forbidden error response
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum AddUserToGroupResponse {
    /// Successful response
    SuccessfulResponse(models::UserGroupMembershipModel),
    /// Forbidden error response
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Conflict error response - user already in group
    ConflictErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum RemoveUserFromGroupResponse {
    /// Successful response
    SuccessfulResponse(models::UserGroupMembershipModel),
    /// Forbidden error response
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListGroupMembersResponse {
    /// Successful response
    SuccessfulResponse(models::ListUserGroupMembershipsResponse),
    /// Forbidden error response
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response - group not found
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListUserGroupsApiResponse {
    /// Successful response
    SuccessfulResponse(models::ListAccessGroupsResponse),
    /// Forbidden error response
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum AddWorkflowToGroupResponse {
    /// Successful response
    SuccessfulResponse(models::WorkflowAccessGroupModel),
    /// Forbidden error response
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Conflict error response - association already exists
    ConflictErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum RemoveWorkflowFromGroupResponse {
    /// Successful response
    SuccessfulResponse(models::WorkflowAccessGroupModel),
    /// Forbidden error response
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListWorkflowGroupsResponse {
    /// Successful response
    SuccessfulResponse(models::ListAccessGroupsResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response - workflow not found
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum CheckWorkflowAccessResponse {
    /// Successful response
    SuccessfulResponse(models::AccessCheckResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

/// API
#[async_trait]
#[allow(clippy::too_many_arguments, clippy::ptr_arg)]
pub trait Api<C: Send + Sync> {
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

    /// Create jobs in bulk. Recommended max job count of 10,000.
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
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteFailureHandlerResponse, ApiError>;

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
        body: serde_json::Value,
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
        body: serde_json::Value,
        context: &C,
    ) -> Result<ClaimActionResponse, ApiError>;

    /// Delete all compute node records for one workflow.
    async fn delete_compute_nodes(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteComputeNodesResponse, ApiError>;

    /// Delete all events for one workflow.
    async fn delete_events(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteEventsResponse, ApiError>;

    /// Delete all files for one workflow.
    async fn delete_files(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteFilesResponse, ApiError>;

    /// Delete all jobs for one workflow.
    async fn delete_jobs(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteJobsResponse, ApiError>;

    /// Delete all local schedulers for one workflow.
    async fn delete_local_schedulers(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteLocalSchedulersResponse, ApiError>;

    /// Delete all resource requirements records for one workflow.
    async fn delete_all_resource_requirements(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteAllResourceRequirementsResponse, ApiError>;

    /// Delete all job results for one workflow.
    async fn delete_results(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteResultsResponse, ApiError>;

    /// Delete all scheduled compute node records for one workflow.
    async fn delete_scheduled_compute_nodes(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodesResponse, ApiError>;

    /// Retrieve all Slurm compute node configurations for one workflow.
    async fn delete_slurm_schedulers(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteSlurmSchedulersResponse, ApiError>;

    /// Delete all user data records for one workflow.
    async fn delete_all_user_data(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
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
        context: &C,
    ) -> Result<ListJobDependenciesResponse, ApiError>;

    /// Retrieve job-file relationships for one workflow.
    async fn list_job_file_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ListJobFileRelationshipsResponse, ApiError>;

    /// Retrieve job-user_data relationships for one workflow.
    async fn list_job_user_data_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
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
        body: Option<serde_json::Value>,
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
        body: Option<serde_json::Value>,
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
        sort_method: Option<models::ClaimJobsSortMethod>,
        strict_scheduler_match: Option<bool>,
        context: &C,
    ) -> Result<ClaimJobsBasedOnResources, ApiError>;

    /// Return user-requested number of jobs that are ready for submission. Sets status to pending.
    async fn claim_next_jobs(
        &self,
        id: i64,
        limit: Option<i64>,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<ClaimNextJobsResponse, ApiError>;

    /// Check for changed job inputs and update status accordingly.
    async fn process_changed_job_inputs(
        &self,
        id: i64,
        dry_run: Option<bool>,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<ProcessChangedJobInputsResponse, ApiError>;

    /// Delete a compute node.
    async fn delete_compute_node(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteComputeNodeResponse, ApiError>;

    /// Delete an event.
    async fn delete_event(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteEventResponse, ApiError>;

    /// Delete a file.
    async fn delete_file(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteFileResponse, ApiError>;

    /// Delete a job.
    async fn delete_job(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteJobResponse, ApiError>;

    /// Delete a local scheduler.
    async fn delete_local_scheduler(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteLocalSchedulerResponse, ApiError>;

    /// Delete a resource requirements record.
    async fn delete_resource_requirements(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteResourceRequirementsResponse, ApiError>;

    /// Delete a job result.
    async fn delete_result(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteResultResponse, ApiError>;

    /// Delete a scheduled compute node.
    async fn delete_scheduled_compute_node(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodeResponse, ApiError>;

    /// Delete Slurm compute node configuration.
    async fn delete_slurm_scheduler(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteSlurmSchedulerResponse, ApiError>;

    /// Delete a user data record.
    async fn delete_user_data(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteUserDataResponse, ApiError>;

    /// Delete a workflow.
    async fn delete_workflow(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteWorkflowResponse, ApiError>;

    /// Reset status for jobs to uninitialized.
    async fn reset_job_status(
        &self,
        id: i64,
        failed_only: Option<bool>,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<ResetJobStatusResponse, ApiError>;

    /// Reset worklow status.
    async fn reset_workflow_status(
        &self,
        id: i64,
        force: Option<bool>,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<ResetWorkflowStatusResponse, ApiError>;

    /// Build a string for a DOT graph.
    async fn get_dot_graph(
        &self,
        id: i64,
        name: String,
        context: &C,
    ) -> Result<GetDotGraphResponse, ApiError>;

    /// Change the status of a job and manage side effects.
    async fn manage_status_change(
        &self,
        id: i64,
        status: models::JobStatus,
        run_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<ManageStatusChangeResponse, ApiError>;

    /// Start a job and manage side effects.
    async fn start_job(
        &self,
        id: i64,
        run_id: i64,
        compute_node_id: i64,
        body: Option<serde_json::Value>,
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
        sort_method: Option<models::ClaimJobsSortMethod>,
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
}

/// API where `Context` isn't passed on every API call
#[async_trait]
#[allow(clippy::too_many_arguments, clippy::ptr_arg)]
pub trait ApiNoContext<C: Send + Sync> {
    fn poll_ready(
        &self,
        _cx: &mut Context,
    ) -> Poll<Result<(), Box<dyn Error + Send + Sync + 'static>>>;

    fn context(&self) -> &C;

    /// Store a compute node.
    async fn create_compute_node(
        &self,
        body: models::ComputeNodeModel,
    ) -> Result<CreateComputeNodeResponse, ApiError>;

    /// Store an event.
    async fn create_event(&self, body: models::EventModel)
    -> Result<CreateEventResponse, ApiError>;

    /// Store a file.
    async fn create_file(&self, body: models::FileModel) -> Result<CreateFileResponse, ApiError>;

    /// Store a job.
    async fn create_job(&self, body: models::JobModel) -> Result<CreateJobResponse, ApiError>;

    /// Create jobs in bulk. Recommended max job count of 10,000.
    async fn create_jobs(&self, body: models::JobsModel) -> Result<CreateJobsResponse, ApiError>;

    /// Store a local scheduler.
    async fn create_local_scheduler(
        &self,
        body: models::LocalSchedulerModel,
    ) -> Result<CreateLocalSchedulerResponse, ApiError>;

    /// Store one resource requirements record.
    async fn create_resource_requirements(
        &self,
        body: models::ResourceRequirementsModel,
    ) -> Result<CreateResourceRequirementsResponse, ApiError>;

    /// Store a job result.
    async fn create_result(
        &self,
        body: models::ResultModel,
    ) -> Result<CreateResultResponse, ApiError>;

    /// Store a scheduled compute node.
    async fn create_scheduled_compute_node(
        &self,
        body: models::ScheduledComputeNodesModel,
    ) -> Result<CreateScheduledComputeNodeResponse, ApiError>;

    /// Store a Slurm compute node configuration.
    async fn create_slurm_scheduler(
        &self,
        body: models::SlurmSchedulerModel,
    ) -> Result<CreateSlurmSchedulerResponse, ApiError>;

    /// Store a user data record.
    async fn create_user_data(
        &self,
        body: models::UserDataModel,
        consumer_job_id: Option<i64>,
        producer_job_id: Option<i64>,
    ) -> Result<CreateUserDataResponse, ApiError>;

    /// Store a workflow.
    async fn create_workflow(
        &self,
        body: models::WorkflowModel,
    ) -> Result<CreateWorkflowResponse, ApiError>;

    /// Delete all compute node records for one workflow.
    async fn delete_compute_nodes(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteComputeNodesResponse, ApiError>;

    /// Delete all events for one workflow.
    async fn delete_events(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteEventsResponse, ApiError>;

    /// Delete all files for one workflow.
    async fn delete_files(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteFilesResponse, ApiError>;

    /// Delete all jobs for one workflow.
    async fn delete_jobs(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteJobsResponse, ApiError>;

    /// Delete all local schedulers for one workflow.
    async fn delete_local_schedulers(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteLocalSchedulersResponse, ApiError>;

    /// Delete all resource requirements records for one workflow.
    async fn delete_all_resource_requirements(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteAllResourceRequirementsResponse, ApiError>;

    /// Delete all job results for one workflow.
    async fn delete_results(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteResultsResponse, ApiError>;

    /// Delete all scheduled compute node records for one workflow.
    async fn delete_scheduled_compute_nodes(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteScheduledComputeNodesResponse, ApiError>;

    /// Retrieve all Slurm compute node configurations for one workflow.
    async fn delete_slurm_schedulers(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteSlurmSchedulersResponse, ApiError>;

    /// Delete all user data records for one workflow.
    async fn delete_all_user_data(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteAllUserDataResponse, ApiError>;

    /// Return the version of the service.
    async fn get_version(&self) -> Result<GetVersionResponse, ApiError>;

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
    ) -> Result<ListJobsResponse, ApiError>;

    /// Retrieve all job dependencies for one workflow.
    async fn list_job_dependencies(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
    ) -> Result<ListJobDependenciesResponse, ApiError>;

    /// Retrieve job-file relationships for one workflow.
    async fn list_job_file_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
    ) -> Result<ListJobFileRelationshipsResponse, ApiError>;

    /// Retrieve job-user_data relationships for one workflow.
    async fn list_job_user_data_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
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
    ) -> Result<ListWorkflowsResponse, ApiError>;

    /// Check if the service is running.
    async fn ping(&self) -> Result<PingResponse, ApiError>;

    /// Cancel a workflow. Workers will detect the status change and cancel jobs.
    async fn cancel_workflow(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<CancelWorkflowResponse, ApiError>;

    /// Retrieve a compute node by ID.
    async fn get_compute_node(&self, id: i64) -> Result<GetComputeNodeResponse, ApiError>;

    /// Retrieve an event by ID.
    async fn get_event(&self, id: i64) -> Result<GetEventResponse, ApiError>;

    /// Retrieve a file.
    async fn get_file(&self, id: i64) -> Result<GetFileResponse, ApiError>;

    /// Retrieve a job.
    async fn get_job(&self, id: i64) -> Result<GetJobResponse, ApiError>;

    /// Retrieve a local scheduler.
    async fn get_local_scheduler(&self, id: i64) -> Result<GetLocalSchedulerResponse, ApiError>;

    /// Return the resource requirements for jobs with a status of ready.
    async fn get_ready_job_requirements(
        &self,
        id: i64,
        scheduler_config_id: Option<i64>,
    ) -> Result<GetReadyJobRequirementsResponse, ApiError>;

    /// Retrieve one resource requirements record.
    async fn get_resource_requirements(
        &self,
        id: i64,
    ) -> Result<GetResourceRequirementsResponse, ApiError>;

    /// Retrieve a job result.
    async fn get_result(&self, id: i64) -> Result<GetResultResponse, ApiError>;

    /// Retrieve a scheduled compute node.
    async fn get_scheduled_compute_node(
        &self,
        id: i64,
    ) -> Result<GetScheduledComputeNodeResponse, ApiError>;

    /// Retrieve a Slurm compute node configuration.
    async fn get_slurm_scheduler(&self, id: i64) -> Result<GetSlurmSchedulerResponse, ApiError>;

    /// Retrieve a user data record.
    async fn get_user_data(&self, id: i64) -> Result<GetUserDataResponse, ApiError>;

    /// Retrieve a workflow.
    async fn get_workflow(&self, id: i64) -> Result<GetWorkflowResponse, ApiError>;

    /// Return the workflow status.
    async fn get_workflow_status(&self, id: i64) -> Result<GetWorkflowStatusResponse, ApiError>;

    /// Initialize job relationships based on file and user_data relationships.
    async fn initialize_jobs(
        &self,
        id: i64,
        only_uninitialized: Option<bool>,
        clear_ephemeral_user_data: Option<bool>,
        body: Option<serde_json::Value>,
    ) -> Result<InitializeJobsResponse, ApiError>;

    /// Return true if all jobs in the workflow are complete.
    async fn is_workflow_complete(&self, id: i64) -> Result<IsWorkflowCompleteResponse, ApiError>;

    /// Return true if all jobs in the workflow are uninitialized or disabled.
    async fn is_workflow_uninitialized(
        &self,
        id: i64,
    ) -> Result<IsWorkflowUninitializedResponse, ApiError>;

    /// Retrieve all job IDs for one workflow.
    async fn list_job_ids(&self, id: i64) -> Result<ListJobIdsResponse, ApiError>;

    /// List missing user data that should exist.
    async fn list_missing_user_data(
        &self,
        id: i64,
    ) -> Result<ListMissingUserDataResponse, ApiError>;

    /// List files that must exist.
    async fn list_required_existing_files(
        &self,
        id: i64,
    ) -> Result<ListRequiredExistingFilesResponse, ApiError>;

    /// Update a compute node.
    async fn update_compute_node(
        &self,
        id: i64,
        body: models::ComputeNodeModel,
    ) -> Result<UpdateComputeNodeResponse, ApiError>;

    /// Update an event.
    async fn update_event(
        &self,
        id: i64,
        body: serde_json::Value,
    ) -> Result<UpdateEventResponse, ApiError>;

    /// Update a file.
    async fn update_file(
        &self,
        id: i64,
        body: models::FileModel,
    ) -> Result<UpdateFileResponse, ApiError>;

    /// Update a job.
    async fn update_job(
        &self,
        id: i64,
        body: models::JobModel,
    ) -> Result<UpdateJobResponse, ApiError>;

    /// Update a local scheduler.
    async fn update_local_scheduler(
        &self,
        id: i64,
        body: models::LocalSchedulerModel,
    ) -> Result<UpdateLocalSchedulerResponse, ApiError>;

    /// Update one resource requirements record.
    async fn update_resource_requirements(
        &self,
        id: i64,
        body: models::ResourceRequirementsModel,
    ) -> Result<UpdateResourceRequirementsResponse, ApiError>;

    /// Update a job result.
    async fn update_result(
        &self,
        id: i64,
        body: models::ResultModel,
    ) -> Result<UpdateResultResponse, ApiError>;

    /// Update a scheduled compute node.
    async fn update_scheduled_compute_node(
        &self,
        id: i64,
        body: models::ScheduledComputeNodesModel,
    ) -> Result<UpdateScheduledComputeNodeResponse, ApiError>;

    /// Update a Slurm compute node configuration.
    async fn update_slurm_scheduler(
        &self,
        id: i64,
        body: models::SlurmSchedulerModel,
    ) -> Result<UpdateSlurmSchedulerResponse, ApiError>;

    /// Update a user data record.
    async fn update_user_data(
        &self,
        id: i64,
        body: models::UserDataModel,
    ) -> Result<UpdateUserDataResponse, ApiError>;

    /// Update a workflow.
    async fn update_workflow(
        &self,
        id: i64,
        body: models::WorkflowModel,
    ) -> Result<UpdateWorkflowResponse, ApiError>;

    /// Update the workflow status.
    async fn update_workflow_status(
        &self,
        id: i64,
        body: models::WorkflowStatusModel,
    ) -> Result<UpdateWorkflowStatusResponse, ApiError>;

    /// Return jobs that are ready for submission and meet worker resource. Set status to pending.
    async fn claim_jobs_based_on_resources(
        &self,
        id: i64,
        body: models::ComputeNodesResources,
        limit: i64,
        sort_method: Option<models::ClaimJobsSortMethod>,
        strict_scheduler_match: Option<bool>,
    ) -> Result<ClaimJobsBasedOnResources, ApiError>;

    /// Return user-requested number of jobs that are ready for submission. Sets status to pending.
    async fn claim_next_jobs(
        &self,
        id: i64,
        limit: Option<i64>,
        body: Option<serde_json::Value>,
    ) -> Result<ClaimNextJobsResponse, ApiError>;

    /// Check for changed job inputs and update status accordingly.
    async fn process_changed_job_inputs(
        &self,
        id: i64,
        dry_run: Option<bool>,
        body: Option<serde_json::Value>,
    ) -> Result<ProcessChangedJobInputsResponse, ApiError>;

    /// Delete a compute node.
    async fn delete_compute_node(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteComputeNodeResponse, ApiError>;

    /// Delete an event.
    async fn delete_event(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteEventResponse, ApiError>;

    /// Delete a file.
    async fn delete_file(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteFileResponse, ApiError>;

    /// Delete a job.
    async fn delete_job(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteJobResponse, ApiError>;

    /// Delete a local scheduler.
    async fn delete_local_scheduler(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteLocalSchedulerResponse, ApiError>;

    /// Delete a resource requirements record.
    async fn delete_resource_requirements(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteResourceRequirementsResponse, ApiError>;

    /// Delete a job result.
    async fn delete_result(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteResultResponse, ApiError>;

    /// Delete a scheduled compute node.
    async fn delete_scheduled_compute_node(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteScheduledComputeNodeResponse, ApiError>;

    /// Delete Slurm compute node configuration.
    async fn delete_slurm_scheduler(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteSlurmSchedulerResponse, ApiError>;

    /// Delete a user data record.
    async fn delete_user_data(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteUserDataResponse, ApiError>;

    /// Delete a workflow.
    async fn delete_workflow(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteWorkflowResponse, ApiError>;

    /// Reset status for jobs to uninitialized.
    async fn reset_job_status(
        &self,
        id: i64,
        failed_only: Option<bool>,
        body: Option<serde_json::Value>,
    ) -> Result<ResetJobStatusResponse, ApiError>;

    /// Reset worklow status.
    async fn reset_workflow_status(
        &self,
        id: i64,
        force: Option<bool>,
        body: Option<serde_json::Value>,
    ) -> Result<ResetWorkflowStatusResponse, ApiError>;

    /// Build a string for a DOT graph.
    async fn get_dot_graph(&self, id: i64, name: String) -> Result<GetDotGraphResponse, ApiError>;

    /// Change the status of a job and manage side effects.
    async fn manage_status_change(
        &self,
        id: i64,
        status: models::JobStatus,
        run_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<ManageStatusChangeResponse, ApiError>;

    /// Start a job and manage side effects.
    async fn start_job(
        &self,
        id: i64,
        run_id: i64,
        compute_node_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<StartJobResponse, ApiError>;

    /// Complete a job, connect it to a result, and manage side effects.
    async fn complete_job(
        &self,
        id: i64,
        status: models::JobStatus,
        run_id: i64,
        body: models::ResultModel,
    ) -> Result<CompleteJobResponse, ApiError>;

    /// Retry a failed job by resetting it to ready status and incrementing attempt_id.
    async fn retry_job(
        &self,
        id: i64,
        run_id: i64,
        max_retries: i32,
    ) -> Result<RetryJobResponse, ApiError>;

    /// Get ready jobs that fit within the specified resource constraints.
    async fn get_ready_jobs(
        &self,
        workflow_id: i64,
        resources: models::ComputeNodesResources,
        sort_method: Option<models::ClaimJobsSortMethod>,
        limit: i64,
        strict_scheduler_match: Option<bool>,
    ) -> Result<ClaimJobsBasedOnResources, ApiError>;

    // Access Groups API

    /// Create an access group.
    async fn create_access_group(
        &self,
        body: models::AccessGroupModel,
    ) -> Result<CreateAccessGroupResponse, ApiError>;

    /// Get an access group by ID.
    async fn get_access_group(&self, id: i64) -> Result<GetAccessGroupResponse, ApiError>;

    /// List all access groups.
    async fn list_access_groups(
        &self,
        offset: Option<i64>,
        limit: Option<i64>,
    ) -> Result<ListAccessGroupsApiResponse, ApiError>;

    /// Delete an access group.
    async fn delete_access_group(&self, id: i64) -> Result<DeleteAccessGroupResponse, ApiError>;

    /// Add a user to an access group.
    async fn add_user_to_group(
        &self,
        group_id: i64,
        body: models::UserGroupMembershipModel,
    ) -> Result<AddUserToGroupResponse, ApiError>;

    /// Remove a user from an access group.
    async fn remove_user_from_group(
        &self,
        group_id: i64,
        user_name: String,
    ) -> Result<RemoveUserFromGroupResponse, ApiError>;

    /// List members of an access group.
    async fn list_group_members(
        &self,
        group_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
    ) -> Result<ListGroupMembersResponse, ApiError>;

    /// List groups a user belongs to.
    async fn list_user_groups(
        &self,
        user_name: String,
        offset: Option<i64>,
        limit: Option<i64>,
    ) -> Result<ListUserGroupsApiResponse, ApiError>;

    /// Add a workflow to an access group.
    async fn add_workflow_to_group(
        &self,
        workflow_id: i64,
        group_id: i64,
    ) -> Result<AddWorkflowToGroupResponse, ApiError>;

    /// Remove a workflow from an access group.
    async fn remove_workflow_from_group(
        &self,
        workflow_id: i64,
        group_id: i64,
    ) -> Result<RemoveWorkflowFromGroupResponse, ApiError>;

    /// List access groups for a workflow.
    async fn list_workflow_groups(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
    ) -> Result<ListWorkflowGroupsResponse, ApiError>;

    /// Check if a user can access a workflow.
    async fn check_workflow_access(
        &self,
        workflow_id: i64,
        user_name: String,
    ) -> Result<CheckWorkflowAccessResponse, ApiError>;
}

/// Trait to extend an API to make it easy to bind it to a context.
pub trait ContextWrapperExt<C: Send + Sync>
where
    Self: Sized,
{
    /// Binds this API to a context.
    fn with_context(self, context: C) -> ContextWrapper<Self, C>;
}

impl<T: Api<C> + Send + Sync, C: Clone + Send + Sync> ContextWrapperExt<C> for T {
    fn with_context(self: T, context: C) -> ContextWrapper<T, C> {
        ContextWrapper::<T, C>::new(self, context)
    }
}

#[async_trait]
impl<T: Api<C> + Send + Sync, C: Clone + Send + Sync> ApiNoContext<C> for ContextWrapper<T, C> {
    fn poll_ready(&self, cx: &mut Context) -> Poll<Result<(), ServiceError>> {
        self.api().poll_ready(cx)
    }

    fn context(&self) -> &C {
        ContextWrapper::context(self)
    }

    /// Store a compute node.
    async fn create_compute_node(
        &self,
        body: models::ComputeNodeModel,
    ) -> Result<CreateComputeNodeResponse, ApiError> {
        let context = self.context().clone();
        self.api().create_compute_node(body, &context).await
    }

    /// Store an event.
    async fn create_event(
        &self,
        body: models::EventModel,
    ) -> Result<CreateEventResponse, ApiError> {
        let context = self.context().clone();
        self.api().create_event(body, &context).await
    }

    /// Store a file.
    async fn create_file(&self, body: models::FileModel) -> Result<CreateFileResponse, ApiError> {
        let context = self.context().clone();
        self.api().create_file(body, &context).await
    }

    /// Store a job.
    async fn create_job(&self, body: models::JobModel) -> Result<CreateJobResponse, ApiError> {
        let context = self.context().clone();
        self.api().create_job(body, &context).await
    }

    /// Create jobs in bulk. Recommended max job count of 10,000.
    async fn create_jobs(&self, body: models::JobsModel) -> Result<CreateJobsResponse, ApiError> {
        let context = self.context().clone();
        self.api().create_jobs(body, &context).await
    }

    /// Store a local scheduler.
    async fn create_local_scheduler(
        &self,
        body: models::LocalSchedulerModel,
    ) -> Result<CreateLocalSchedulerResponse, ApiError> {
        let context = self.context().clone();
        self.api().create_local_scheduler(body, &context).await
    }

    /// Store one resource requirements record.
    async fn create_resource_requirements(
        &self,
        body: models::ResourceRequirementsModel,
    ) -> Result<CreateResourceRequirementsResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .create_resource_requirements(body, &context)
            .await
    }

    /// Store a job result.
    async fn create_result(
        &self,
        body: models::ResultModel,
    ) -> Result<CreateResultResponse, ApiError> {
        let context = self.context().clone();
        self.api().create_result(body, &context).await
    }

    /// Store a scheduled compute node.
    async fn create_scheduled_compute_node(
        &self,
        body: models::ScheduledComputeNodesModel,
    ) -> Result<CreateScheduledComputeNodeResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .create_scheduled_compute_node(body, &context)
            .await
    }

    /// Store a Slurm compute node configuration.
    async fn create_slurm_scheduler(
        &self,
        body: models::SlurmSchedulerModel,
    ) -> Result<CreateSlurmSchedulerResponse, ApiError> {
        let context = self.context().clone();
        self.api().create_slurm_scheduler(body, &context).await
    }

    /// Store a user data record.
    async fn create_user_data(
        &self,
        body: models::UserDataModel,
        consumer_job_id: Option<i64>,
        producer_job_id: Option<i64>,
    ) -> Result<CreateUserDataResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .create_user_data(body, consumer_job_id, producer_job_id, &context)
            .await
    }

    /// Store a workflow.
    async fn create_workflow(
        &self,
        body: models::WorkflowModel,
    ) -> Result<CreateWorkflowResponse, ApiError> {
        let context = self.context().clone();
        self.api().create_workflow(body, &context).await
    }

    /// Delete all compute node records for one workflow.
    async fn delete_compute_nodes(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteComputeNodesResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .delete_compute_nodes(workflow_id, body, &context)
            .await
    }

    /// Delete all events for one workflow.
    async fn delete_events(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteEventsResponse, ApiError> {
        let context = self.context().clone();
        self.api().delete_events(workflow_id, body, &context).await
    }

    /// Delete all files for one workflow.
    async fn delete_files(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteFilesResponse, ApiError> {
        let context = self.context().clone();
        self.api().delete_files(workflow_id, body, &context).await
    }

    /// Delete all jobs for one workflow.
    async fn delete_jobs(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteJobsResponse, ApiError> {
        let context = self.context().clone();
        self.api().delete_jobs(workflow_id, body, &context).await
    }

    /// Delete all local schedulers for one workflow.
    async fn delete_local_schedulers(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteLocalSchedulersResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .delete_local_schedulers(workflow_id, body, &context)
            .await
    }

    /// Delete all resource requirements records for one workflow.
    async fn delete_all_resource_requirements(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteAllResourceRequirementsResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .delete_all_resource_requirements(workflow_id, body, &context)
            .await
    }

    /// Delete all job results for one workflow.
    async fn delete_results(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteResultsResponse, ApiError> {
        let context = self.context().clone();
        self.api().delete_results(workflow_id, body, &context).await
    }

    /// Delete all scheduled compute node records for one workflow.
    async fn delete_scheduled_compute_nodes(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteScheduledComputeNodesResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .delete_scheduled_compute_nodes(workflow_id, body, &context)
            .await
    }

    /// Retrieve all Slurm compute node configurations for one workflow.
    async fn delete_slurm_schedulers(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteSlurmSchedulersResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .delete_slurm_schedulers(workflow_id, body, &context)
            .await
    }

    /// Delete all user data records for one workflow.
    async fn delete_all_user_data(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteAllUserDataResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .delete_all_user_data(workflow_id, body, &context)
            .await
    }

    /// Return the version of the service.
    async fn get_version(&self) -> Result<GetVersionResponse, ApiError> {
        let context = self.context().clone();
        self.api().get_version(&context).await
    }

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
    ) -> Result<ListComputeNodesResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .list_compute_nodes(
                workflow_id,
                offset,
                limit,
                sort_by,
                reverse_sort,
                hostname,
                is_active,
                scheduled_compute_node_id,
                &context,
            )
            .await
    }

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
    ) -> Result<ListEventsResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .list_events(
                workflow_id,
                offset,
                limit,
                sort_by,
                reverse_sort,
                category,
                after_timestamp,
                &context,
            )
            .await
    }

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
    ) -> Result<ListFilesResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .list_files(
                workflow_id,
                produced_by_job_id,
                offset,
                limit,
                sort_by,
                reverse_sort,
                name,
                path,
                is_output,
                &context,
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
    ) -> Result<ListJobsResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .list_jobs(
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
                &context,
            )
            .await
    }

    /// Retrieve all job dependencies for one workflow.
    async fn list_job_dependencies(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
    ) -> Result<ListJobDependenciesResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .list_job_dependencies(workflow_id, offset, limit, &context)
            .await
    }

    /// Retrieve job-file relationships for one workflow.
    async fn list_job_file_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
    ) -> Result<ListJobFileRelationshipsResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .list_job_file_relationships(workflow_id, offset, limit, &context)
            .await
    }

    /// Retrieve job-user_data relationships for one workflow.
    async fn list_job_user_data_relationships(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
    ) -> Result<ListJobUserDataRelationshipsResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .list_job_user_data_relationships(workflow_id, offset, limit, &context)
            .await
    }

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
    ) -> Result<ListLocalSchedulersResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .list_local_schedulers(
                workflow_id,
                offset,
                limit,
                sort_by,
                reverse_sort,
                memory,
                num_cpus,
                &context,
            )
            .await
    }

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
    ) -> Result<ListResourceRequirementsResponse, ApiError> {
        let context = self.context().clone();
        self.api()
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
                &context,
            )
            .await
    }

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
    ) -> Result<ListResultsResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .list_results(
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
                &context,
            )
            .await
    }

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
    ) -> Result<ListScheduledComputeNodesResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .list_scheduled_compute_nodes(
                workflow_id,
                offset,
                limit,
                sort_by,
                reverse_sort,
                scheduler_id,
                scheduler_config_id,
                status,
                &context,
            )
            .await
    }

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
    ) -> Result<ListSlurmSchedulersResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .list_slurm_schedulers(
                workflow_id,
                offset,
                limit,
                sort_by,
                reverse_sort,
                name,
                account,
                gres,
                mem,
                nodes,
                partition,
                qos,
                tmp,
                walltime,
                &context,
            )
            .await
    }

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
    ) -> Result<ListUserDataResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .list_user_data(
                workflow_id,
                consumer_job_id,
                producer_job_id,
                offset,
                limit,
                sort_by,
                reverse_sort,
                name,
                is_ephemeral,
                &context,
            )
            .await
    }

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
    ) -> Result<ListWorkflowsResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .list_workflows(
                offset,
                sort_by,
                reverse_sort,
                limit,
                name,
                user,
                description,
                is_archived,
                &context,
            )
            .await
    }

    /// Check if the service is running.
    async fn ping(&self) -> Result<PingResponse, ApiError> {
        let context = self.context().clone();
        self.api().ping(&context).await
    }

    /// Cancel a workflow. Workers will detect the status change and cancel jobs.
    async fn cancel_workflow(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<CancelWorkflowResponse, ApiError> {
        let context = self.context().clone();
        self.api().cancel_workflow(id, body, &context).await
    }

    /// Retrieve a compute node by ID.
    async fn get_compute_node(&self, id: i64) -> Result<GetComputeNodeResponse, ApiError> {
        let context = self.context().clone();
        self.api().get_compute_node(id, &context).await
    }

    /// Retrieve an event by ID.
    async fn get_event(&self, id: i64) -> Result<GetEventResponse, ApiError> {
        let context = self.context().clone();
        self.api().get_event(id, &context).await
    }

    /// Retrieve a file.
    async fn get_file(&self, id: i64) -> Result<GetFileResponse, ApiError> {
        let context = self.context().clone();
        self.api().get_file(id, &context).await
    }

    /// Retrieve a job.
    async fn get_job(&self, id: i64) -> Result<GetJobResponse, ApiError> {
        let context = self.context().clone();
        self.api().get_job(id, &context).await
    }

    /// Retrieve a local scheduler.
    async fn get_local_scheduler(&self, id: i64) -> Result<GetLocalSchedulerResponse, ApiError> {
        let context = self.context().clone();
        self.api().get_local_scheduler(id, &context).await
    }

    /// Return the resource requirements for jobs with a status of ready.
    async fn get_ready_job_requirements(
        &self,
        id: i64,
        scheduler_config_id: Option<i64>,
    ) -> Result<GetReadyJobRequirementsResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .get_ready_job_requirements(id, scheduler_config_id, &context)
            .await
    }

    /// Retrieve one resource requirements record.
    async fn get_resource_requirements(
        &self,
        id: i64,
    ) -> Result<GetResourceRequirementsResponse, ApiError> {
        let context = self.context().clone();
        self.api().get_resource_requirements(id, &context).await
    }

    /// Retrieve a job result.
    async fn get_result(&self, id: i64) -> Result<GetResultResponse, ApiError> {
        let context = self.context().clone();
        self.api().get_result(id, &context).await
    }

    /// Retrieve a scheduled compute node.
    async fn get_scheduled_compute_node(
        &self,
        id: i64,
    ) -> Result<GetScheduledComputeNodeResponse, ApiError> {
        let context = self.context().clone();
        self.api().get_scheduled_compute_node(id, &context).await
    }

    /// Retrieve a Slurm compute node configuration.
    async fn get_slurm_scheduler(&self, id: i64) -> Result<GetSlurmSchedulerResponse, ApiError> {
        let context = self.context().clone();
        self.api().get_slurm_scheduler(id, &context).await
    }

    /// Retrieve a user data record.
    async fn get_user_data(&self, id: i64) -> Result<GetUserDataResponse, ApiError> {
        let context = self.context().clone();
        self.api().get_user_data(id, &context).await
    }

    /// Retrieve a workflow.
    async fn get_workflow(&self, id: i64) -> Result<GetWorkflowResponse, ApiError> {
        let context = self.context().clone();
        self.api().get_workflow(id, &context).await
    }

    /// Return the workflow status.
    async fn get_workflow_status(&self, id: i64) -> Result<GetWorkflowStatusResponse, ApiError> {
        let context = self.context().clone();
        self.api().get_workflow_status(id, &context).await
    }

    /// Initialize job relationships based on file and user_data relationships.
    async fn initialize_jobs(
        &self,
        id: i64,
        only_uninitialized: Option<bool>,
        clear_ephemeral_user_data: Option<bool>,
        body: Option<serde_json::Value>,
    ) -> Result<InitializeJobsResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .initialize_jobs(
                id,
                only_uninitialized,
                clear_ephemeral_user_data,
                body,
                &context,
            )
            .await
    }

    /// Return true if all jobs in the workflow are complete.
    async fn is_workflow_complete(&self, id: i64) -> Result<IsWorkflowCompleteResponse, ApiError> {
        let context = self.context().clone();
        self.api().is_workflow_complete(id, &context).await
    }

    /// Return true if all jobs in the workflow are uninitialized or disabled.
    async fn is_workflow_uninitialized(
        &self,
        id: i64,
    ) -> Result<IsWorkflowUninitializedResponse, ApiError> {
        let context = self.context().clone();
        self.api().is_workflow_uninitialized(id, &context).await
    }

    /// Retrieve all job IDs for one workflow.
    async fn list_job_ids(&self, id: i64) -> Result<ListJobIdsResponse, ApiError> {
        let context = self.context().clone();
        self.api().list_job_ids(id, &context).await
    }

    /// List missing user data that should exist.
    async fn list_missing_user_data(
        &self,
        id: i64,
    ) -> Result<ListMissingUserDataResponse, ApiError> {
        let context = self.context().clone();
        self.api().list_missing_user_data(id, &context).await
    }

    /// List files that must exist.
    async fn list_required_existing_files(
        &self,
        id: i64,
    ) -> Result<ListRequiredExistingFilesResponse, ApiError> {
        let context = self.context().clone();
        self.api().list_required_existing_files(id, &context).await
    }

    /// Update a compute node.
    async fn update_compute_node(
        &self,
        id: i64,
        body: models::ComputeNodeModel,
    ) -> Result<UpdateComputeNodeResponse, ApiError> {
        let context = self.context().clone();
        self.api().update_compute_node(id, body, &context).await
    }

    /// Update an event.
    async fn update_event(
        &self,
        id: i64,
        body: serde_json::Value,
    ) -> Result<UpdateEventResponse, ApiError> {
        let context = self.context().clone();
        self.api().update_event(id, body, &context).await
    }

    /// Update a file.
    async fn update_file(
        &self,
        id: i64,
        body: models::FileModel,
    ) -> Result<UpdateFileResponse, ApiError> {
        let context = self.context().clone();
        self.api().update_file(id, body, &context).await
    }

    /// Update a job.
    async fn update_job(
        &self,
        id: i64,
        body: models::JobModel,
    ) -> Result<UpdateJobResponse, ApiError> {
        let context = self.context().clone();
        self.api().update_job(id, body, &context).await
    }

    /// Update a local scheduler.
    async fn update_local_scheduler(
        &self,
        id: i64,
        body: models::LocalSchedulerModel,
    ) -> Result<UpdateLocalSchedulerResponse, ApiError> {
        let context = self.context().clone();
        self.api().update_local_scheduler(id, body, &context).await
    }

    /// Update one resource requirements record.
    async fn update_resource_requirements(
        &self,
        id: i64,
        body: models::ResourceRequirementsModel,
    ) -> Result<UpdateResourceRequirementsResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .update_resource_requirements(id, body, &context)
            .await
    }

    /// Update a job result.
    async fn update_result(
        &self,
        id: i64,
        body: models::ResultModel,
    ) -> Result<UpdateResultResponse, ApiError> {
        let context = self.context().clone();
        self.api().update_result(id, body, &context).await
    }

    /// Update a scheduled compute node.
    async fn update_scheduled_compute_node(
        &self,
        id: i64,
        body: models::ScheduledComputeNodesModel,
    ) -> Result<UpdateScheduledComputeNodeResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .update_scheduled_compute_node(id, body, &context)
            .await
    }

    /// Update a Slurm compute node configuration.
    async fn update_slurm_scheduler(
        &self,
        id: i64,
        body: models::SlurmSchedulerModel,
    ) -> Result<UpdateSlurmSchedulerResponse, ApiError> {
        let context = self.context().clone();
        self.api().update_slurm_scheduler(id, body, &context).await
    }

    /// Update a user data record.
    async fn update_user_data(
        &self,
        id: i64,
        body: models::UserDataModel,
    ) -> Result<UpdateUserDataResponse, ApiError> {
        let context = self.context().clone();
        self.api().update_user_data(id, body, &context).await
    }

    /// Update a workflow.
    async fn update_workflow(
        &self,
        id: i64,
        body: models::WorkflowModel,
    ) -> Result<UpdateWorkflowResponse, ApiError> {
        let context = self.context().clone();
        self.api().update_workflow(id, body, &context).await
    }

    /// Update the workflow status.
    async fn update_workflow_status(
        &self,
        id: i64,
        body: models::WorkflowStatusModel,
    ) -> Result<UpdateWorkflowStatusResponse, ApiError> {
        let context = self.context().clone();
        self.api().update_workflow_status(id, body, &context).await
    }

    /// Return jobs that are ready for submission and meet worker resource. Set status to pending.
    async fn claim_jobs_based_on_resources(
        &self,
        id: i64,
        body: models::ComputeNodesResources,
        limit: i64,
        sort_method: Option<models::ClaimJobsSortMethod>,
        strict_scheduler_match: Option<bool>,
    ) -> Result<ClaimJobsBasedOnResources, ApiError> {
        let context = self.context().clone();
        self.api()
            .claim_jobs_based_on_resources(
                id,
                body,
                limit,
                sort_method,
                strict_scheduler_match,
                &context,
            )
            .await
    }

    /// Return user-requested number of jobs that are ready for submission. Sets status to pending.
    async fn claim_next_jobs(
        &self,
        id: i64,
        limit: Option<i64>,
        body: Option<serde_json::Value>,
    ) -> Result<ClaimNextJobsResponse, ApiError> {
        let context = self.context().clone();
        self.api().claim_next_jobs(id, limit, body, &context).await
    }

    /// Check for changed job inputs and update status accordingly.
    async fn process_changed_job_inputs(
        &self,
        id: i64,
        dry_run: Option<bool>,
        body: Option<serde_json::Value>,
    ) -> Result<ProcessChangedJobInputsResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .process_changed_job_inputs(id, dry_run, body, &context)
            .await
    }

    /// Delete a compute node.
    async fn delete_compute_node(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteComputeNodeResponse, ApiError> {
        let context = self.context().clone();
        self.api().delete_compute_node(id, body, &context).await
    }

    /// Delete an event.
    async fn delete_event(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteEventResponse, ApiError> {
        let context = self.context().clone();
        self.api().delete_event(id, body, &context).await
    }

    /// Delete a file.
    async fn delete_file(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteFileResponse, ApiError> {
        let context = self.context().clone();
        self.api().delete_file(id, body, &context).await
    }

    /// Delete a job.
    async fn delete_job(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteJobResponse, ApiError> {
        let context = self.context().clone();
        self.api().delete_job(id, body, &context).await
    }

    /// Delete a local scheduler.
    async fn delete_local_scheduler(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteLocalSchedulerResponse, ApiError> {
        let context = self.context().clone();
        self.api().delete_local_scheduler(id, body, &context).await
    }

    /// Delete a resource requirements record.
    async fn delete_resource_requirements(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteResourceRequirementsResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .delete_resource_requirements(id, body, &context)
            .await
    }

    /// Delete a job result.
    async fn delete_result(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteResultResponse, ApiError> {
        let context = self.context().clone();
        self.api().delete_result(id, body, &context).await
    }

    /// Delete a scheduled compute node.
    async fn delete_scheduled_compute_node(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteScheduledComputeNodeResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .delete_scheduled_compute_node(id, body, &context)
            .await
    }

    /// Delete Slurm compute node configuration.
    async fn delete_slurm_scheduler(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteSlurmSchedulerResponse, ApiError> {
        let context = self.context().clone();
        self.api().delete_slurm_scheduler(id, body, &context).await
    }

    /// Delete a user data record.
    async fn delete_user_data(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteUserDataResponse, ApiError> {
        let context = self.context().clone();
        self.api().delete_user_data(id, body, &context).await
    }

    /// Delete a workflow.
    async fn delete_workflow(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<DeleteWorkflowResponse, ApiError> {
        let context = self.context().clone();
        self.api().delete_workflow(id, body, &context).await
    }

    /// Reset status for jobs to uninitialized.
    async fn reset_job_status(
        &self,
        id: i64,
        failed_only: Option<bool>,
        body: Option<serde_json::Value>,
    ) -> Result<ResetJobStatusResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .reset_job_status(id, failed_only, body, &context)
            .await
    }

    /// Reset worklow status.
    async fn reset_workflow_status(
        &self,
        id: i64,
        force: Option<bool>,
        body: Option<serde_json::Value>,
    ) -> Result<ResetWorkflowStatusResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .reset_workflow_status(id, force, body, &context)
            .await
    }

    /// Build a string for a DOT graph.
    async fn get_dot_graph(&self, id: i64, name: String) -> Result<GetDotGraphResponse, ApiError> {
        let context = self.context().clone();
        self.api().get_dot_graph(id, name, &context).await
    }

    /// Change the status of a job and manage side effects.
    async fn manage_status_change(
        &self,
        id: i64,
        status: models::JobStatus,
        run_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<ManageStatusChangeResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .manage_status_change(id, status, run_id, body, &context)
            .await
    }

    /// Start a job and manage side effects.
    async fn start_job(
        &self,
        id: i64,
        run_id: i64,
        compute_node_id: i64,
        body: Option<serde_json::Value>,
    ) -> Result<StartJobResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .start_job(id, run_id, compute_node_id, body, &context)
            .await
    }

    /// Complete a job, connect it to a result, and manage side effects.
    async fn complete_job(
        &self,
        id: i64,
        status: models::JobStatus,
        run_id: i64,
        body: models::ResultModel,
    ) -> Result<CompleteJobResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .complete_job(id, status, run_id, body, &context)
            .await
    }

    /// Retry a failed job by resetting it to ready status and incrementing attempt_id.
    async fn retry_job(
        &self,
        id: i64,
        run_id: i64,
        max_retries: i32,
    ) -> Result<RetryJobResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .retry_job(id, run_id, max_retries, &context)
            .await
    }

    /// Get ready jobs that fit within the specified resource constraints.
    async fn get_ready_jobs(
        &self,
        workflow_id: i64,
        resources: models::ComputeNodesResources,
        sort_method: Option<models::ClaimJobsSortMethod>,
        limit: i64,
        strict_scheduler_match: Option<bool>,
    ) -> Result<ClaimJobsBasedOnResources, ApiError> {
        let context = self.context().clone();
        self.api()
            .prepare_ready_jobs(
                workflow_id,
                resources,
                sort_method,
                limit,
                strict_scheduler_match,
                &context,
            )
            .await
    }

    // Access Groups API

    async fn create_access_group(
        &self,
        body: models::AccessGroupModel,
    ) -> Result<CreateAccessGroupResponse, ApiError> {
        let context = self.context().clone();
        self.api().create_access_group(body, &context).await
    }

    async fn get_access_group(&self, id: i64) -> Result<GetAccessGroupResponse, ApiError> {
        let context = self.context().clone();
        self.api().get_access_group(id, &context).await
    }

    async fn list_access_groups(
        &self,
        offset: Option<i64>,
        limit: Option<i64>,
    ) -> Result<ListAccessGroupsApiResponse, ApiError> {
        let context = self.context().clone();
        self.api().list_access_groups(offset, limit, &context).await
    }

    async fn delete_access_group(&self, id: i64) -> Result<DeleteAccessGroupResponse, ApiError> {
        let context = self.context().clone();
        self.api().delete_access_group(id, &context).await
    }

    async fn add_user_to_group(
        &self,
        group_id: i64,
        body: models::UserGroupMembershipModel,
    ) -> Result<AddUserToGroupResponse, ApiError> {
        let context = self.context().clone();
        self.api().add_user_to_group(group_id, body, &context).await
    }

    async fn remove_user_from_group(
        &self,
        group_id: i64,
        user_name: String,
    ) -> Result<RemoveUserFromGroupResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .remove_user_from_group(group_id, user_name, &context)
            .await
    }

    async fn list_group_members(
        &self,
        group_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
    ) -> Result<ListGroupMembersResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .list_group_members(group_id, offset, limit, &context)
            .await
    }

    async fn list_user_groups(
        &self,
        user_name: String,
        offset: Option<i64>,
        limit: Option<i64>,
    ) -> Result<ListUserGroupsApiResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .list_user_groups(user_name, offset, limit, &context)
            .await
    }

    async fn add_workflow_to_group(
        &self,
        workflow_id: i64,
        group_id: i64,
    ) -> Result<AddWorkflowToGroupResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .add_workflow_to_group(workflow_id, group_id, &context)
            .await
    }

    async fn remove_workflow_from_group(
        &self,
        workflow_id: i64,
        group_id: i64,
    ) -> Result<RemoveWorkflowFromGroupResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .remove_workflow_from_group(workflow_id, group_id, &context)
            .await
    }

    async fn list_workflow_groups(
        &self,
        workflow_id: i64,
        offset: Option<i64>,
        limit: Option<i64>,
    ) -> Result<ListWorkflowGroupsResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .list_workflow_groups(workflow_id, offset, limit, &context)
            .await
    }

    async fn check_workflow_access(
        &self,
        workflow_id: i64,
        user_name: String,
    ) -> Result<CheckWorkflowAccessResponse, ApiError> {
        let context = self.context().clone();
        self.api()
            .check_workflow_access(workflow_id, user_name, &context)
            .await
    }
}

// Module declarations removed - these are now handled in src/server/mod.rs and src/lib.rs
