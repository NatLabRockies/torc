#![allow(
    missing_docs,
    clippy::derive_partial_eq_without_eq,
    clippy::large_enum_variant
)]

//! Owned transport response enums for the server contract and HTTP mapping layer.

use crate::models;
use serde::{Deserialize, Serialize};

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
    /// Unprocessable content (e.g., invalid priority)
    UnprocessableContentErrorResponse(models::ErrorResponse),
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
pub enum CreateRoCrateEntityResponse {
    /// Successful response
    SuccessfulResponse(models::RoCrateEntityModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetRoCrateEntityResponse {
    /// Successful response
    SuccessfulResponse(models::RoCrateEntityModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ListRoCrateEntitiesResponse {
    /// Successful response
    SuccessfulResponse(models::ListRoCrateEntitiesResponse),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum UpdateRoCrateEntityResponse {
    /// Successful response
    SuccessfulResponse(models::RoCrateEntityModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum DeleteRoCrateEntityResponse {
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
pub enum DeleteRoCrateEntitiesResponse {
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
pub enum CreateSlurmStatsResponse {
    /// Successful response
    SuccessfulResponse(models::SlurmStatsModel),
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
    /// Unprocessable content error response
    UnprocessableContentErrorResponse(models::ErrorResponse),
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
pub enum ListSlurmStatsResponse {
    /// Successful response
    SuccessfulResponse(models::ListSlurmStatsResponse),
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
    /// Accepted - initialization task created
    AcceptedResponse(models::TaskModel),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Conflict error response
    ConflictErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetTaskResponse {
    /// Successful response
    SuccessfulResponse(models::TaskModel),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

/// Response for `GET /workflows/{id}/active_task`. The body is an `ActiveTaskResponse`
/// with `task: Option<TaskModel>`, so the response is always a JSON object even when no
/// active task exists.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum GetActiveTaskResponse {
    /// 200 OK with an `ActiveTaskResponse` body.
    SuccessfulResponse(models::ActiveTaskResponse),
    /// Workflow not found or caller is not authorized.
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response.
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
    /// Unprocessable content error response
    UnprocessableContentErrorResponse(models::ErrorResponse),
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
    /// Unprocessable content error response
    UnprocessableContentErrorResponse(models::ErrorResponse),
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
pub enum BatchCompleteJobsResponse {
    /// Successful response. Per-completion failures are reported in the body's `errors` field.
    SuccessfulResponse(models::BatchCompleteJobsResponse),
    /// Forbidden - user does not have access to the workflow
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Workflow not found
    NotFoundErrorResponse(models::ErrorResponse),
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

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum ReloadAuthResponse {
    /// Successful response
    SuccessfulResponse(serde_json::Value),
    /// Forbidden - user does not have access
    ForbiddenErrorResponse(models::ErrorResponse),
    /// Not found error response
    NotFoundErrorResponse(models::ErrorResponse),
    /// Default error response
    DefaultErrorResponse(models::ErrorResponse),
}

// Module declarations removed - these are now handled in src/server/mod.rs and src/lib.rs
