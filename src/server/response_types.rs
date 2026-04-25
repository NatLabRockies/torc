//! Owned facade over server transport response enums.
//!
//! The concrete enums still come from `api_responses.rs`, but callers import them through
//! domain-grouped modules so the rest of the server no longer depends directly on one large
//! response barrel.

pub mod access {
    pub use crate::server::api_responses::{
        AddUserToGroupResponse, AddWorkflowToGroupResponse, CheckWorkflowAccessResponse,
        CreateAccessGroupResponse, DeleteAccessGroupResponse, GetAccessGroupResponse,
        ListAccessGroupsApiResponse, ListGroupMembersResponse, ListUserGroupsApiResponse,
        ListWorkflowGroupsResponse, RemoveUserFromGroupResponse, RemoveWorkflowFromGroupResponse,
    };
}

pub mod artifacts {
    pub use crate::server::api_responses::{
        CreateFileResponse, CreateResultResponse, CreateRoCrateEntityResponse,
        CreateUserDataResponse, DeleteAllUserDataResponse, DeleteFileResponse, DeleteFilesResponse,
        DeleteResultResponse, DeleteResultsResponse, DeleteRoCrateEntitiesResponse,
        DeleteRoCrateEntityResponse, DeleteUserDataResponse, GetFileResponse, GetResultResponse,
        GetRoCrateEntityResponse, GetUserDataResponse, ListFilesResponse,
        ListMissingUserDataResponse, ListRequiredExistingFilesResponse, ListResultsResponse,
        ListRoCrateEntitiesResponse, ListUserDataResponse, UpdateFileResponse,
        UpdateResultResponse, UpdateRoCrateEntityResponse, UpdateUserDataResponse,
    };
}

pub mod events {
    pub use crate::server::api_responses::{
        CreateEventResponse, CreateFailureHandlerResponse, DeleteEventResponse,
        DeleteEventsResponse, DeleteFailureHandlerResponse, GetEventResponse,
        GetFailureHandlerResponse, ListEventsResponse, ListFailureHandlersResponse,
        UpdateEventResponse,
    };
}

pub mod jobs {
    pub use crate::server::api_responses::{
        BatchCompleteJobsResponse, ClaimJobsBasedOnResources, ClaimNextJobsResponse,
        CompleteJobResponse, CreateJobResponse, CreateJobsResponse, DeleteJobResponse,
        DeleteJobsResponse, GetJobResponse, GetReadyJobRequirementsResponse,
        InitializeJobsResponse, ListJobDependenciesResponse, ListJobFileRelationshipsResponse,
        ListJobIdsResponse, ListJobUserDataRelationshipsResponse, ListJobsResponse,
        ManageStatusChangeResponse, ProcessChangedJobInputsResponse, ResetJobStatusResponse,
        RetryJobResponse, StartJobResponse, UpdateJobResponse,
    };
}

pub mod scheduling {
    pub use crate::server::api_responses::{
        CreateComputeNodeResponse, CreateLocalSchedulerResponse, CreateRemoteWorkersResponse,
        CreateResourceRequirementsResponse, CreateScheduledComputeNodeResponse,
        CreateSlurmSchedulerResponse, CreateSlurmStatsResponse,
        DeleteAllResourceRequirementsResponse, DeleteComputeNodeResponse,
        DeleteComputeNodesResponse, DeleteLocalSchedulerResponse, DeleteLocalSchedulersResponse,
        DeleteRemoteWorkerResponse, DeleteResourceRequirementsResponse,
        DeleteScheduledComputeNodeResponse, DeleteScheduledComputeNodesResponse,
        DeleteSlurmSchedulerResponse, DeleteSlurmSchedulersResponse, GetComputeNodeResponse,
        GetLocalSchedulerResponse, GetResourceRequirementsResponse,
        GetScheduledComputeNodeResponse, GetSlurmSchedulerResponse, ListComputeNodesResponse,
        ListLocalSchedulersResponse, ListRemoteWorkersResponse, ListResourceRequirementsResponse,
        ListScheduledComputeNodesResponse, ListSlurmSchedulersResponse, ListSlurmStatsResponse,
        UpdateComputeNodeResponse, UpdateLocalSchedulerResponse,
        UpdateResourceRequirementsResponse, UpdateScheduledComputeNodeResponse,
        UpdateSlurmSchedulerResponse,
    };
}

pub mod system {
    pub use crate::server::api_responses::{
        GetTaskResponse, GetVersionResponse, PingResponse, ReloadAuthResponse,
    };
}

pub mod workflows {
    pub use crate::server::api_responses::{
        CancelWorkflowResponse, ClaimActionResponse, CreateWorkflowActionResponse,
        CreateWorkflowResponse, DeleteWorkflowResponse, GetActiveTaskResponse,
        GetPendingActionsResponse, GetWorkflowActionsResponse, GetWorkflowResponse,
        GetWorkflowStatusResponse, IsWorkflowCompleteResponse, IsWorkflowUninitializedResponse,
        ListWorkflowsResponse, ResetWorkflowStatusResponse, UpdateWorkflowResponse,
        UpdateWorkflowStatusResponse,
    };
}
