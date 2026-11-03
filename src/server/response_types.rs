//! Owned facade over server transport response enums.
//!
//! The concrete enums still come from `api_responses.rs`, but callers import them through
//! domain-grouped modules so the rest of the server no longer depends directly on one large
//! response barrel.

pub(crate) mod access {
    pub(crate) use crate::server::api_responses::{
        AddUserToGroupResponse, AddWorkflowToGroupResponse, CheckWorkflowAccessResponse,
        CreateAccessGroupResponse, DeleteAccessGroupResponse, GetAccessGroupResponse,
        ListAccessGroupsApiResponse, ListGroupMembersResponse, ListUserGroupsApiResponse,
        ListWorkflowGroupsResponse, RemoveUserFromGroupResponse, RemoveWorkflowFromGroupResponse,
    };
}

pub(crate) mod artifacts {
    pub(crate) use crate::server::api_responses::{
        CreateFileResponse, CreateFilesResponse, CreateResultResponse, CreateRoCrateEntityResponse,
        CreateUserDataListResponse, CreateUserDataResponse, DeleteAllUserDataResponse,
        DeleteFileResponse, DeleteFilesResponse, DeleteResultResponse, DeleteResultsResponse,
        DeleteRoCrateEntitiesResponse, DeleteRoCrateEntityResponse, DeleteUserDataResponse,
        GetFileResponse, GetResultResponse, GetRoCrateEntityResponse, GetUserDataResponse,
        ListFilesResponse, ListMissingUserDataResponse, ListRequiredExistingFilesResponse,
        ListResultsResponse, ListRoCrateEntitiesResponse, ListUserDataResponse, UpdateFileResponse,
        UpdateResultResponse, UpdateRoCrateEntityResponse, UpdateUserDataResponse,
    };
}

pub(crate) mod events {
    pub(crate) use crate::server::api_responses::{
        CreateEventResponse, CreateFailureHandlerResponse, DeleteEventResponse,
        DeleteEventsResponse, DeleteFailureHandlerResponse, GetEventResponse,
        GetFailureHandlerResponse, ListEventsResponse, ListFailureHandlersResponse,
        UpdateEventResponse,
    };
}

pub(crate) mod jobs {
    pub(crate) use crate::server::api_responses::{
        BatchCompleteJobsResponse, ClaimJobsBasedOnResources, ClaimNextJobsResponse,
        CompleteJobResponse, CreateJobResponse, CreateJobsResponse, DeleteJobResponse,
        DeleteJobsResponse, GetJobResponse, GetReadyJobRequirementsResponse,
        InitializeJobsResponse, ListJobDependenciesResponse, ListJobFileRelationshipsResponse,
        ListJobIdsResponse, ListJobUserDataRelationshipsResponse, ListJobsResponse,
        ManageStatusChangeResponse, ProcessChangedJobInputsResponse, ResetJobStatusResponse,
        RetryJobResponse, SpawnJobsResponse, StartJobResponse, UpdateJobResponse,
    };
}

pub(crate) mod scheduling {
    pub(crate) use crate::server::api_responses::{
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

pub(crate) mod system {
    pub(crate) use crate::server::api_responses::{
        AdminSqlResponse, GetTaskResponse, GetVersionResponse, ListAdminAuditLogResponse,
        PingResponse, ReloadAuthResponse,
    };
}

pub(crate) mod workflows {
    pub(crate) use crate::server::api_responses::{
        ArchiveWorkflowResponse, CancelWorkflowResponse, ClaimActionResponse,
        CreateWorkflowActionResponse, CreateWorkflowResponse, DeleteWorkflowActionResponse,
        DeleteWorkflowResponse, GetActiveTaskResponse, GetPendingActionsResponse,
        GetRunningJobsResponse, GetSlurmJobCorrelationsResponse, GetWorkflowActionsResponse,
        GetWorkflowResponse, GetWorkflowStatusResponse, IsWorkflowCompleteResponse,
        IsWorkflowUninitializedResponse, ListWorkflowsResponse, ResetWorkflowStatusResponse,
        UpdateWorkflowActionResponse, UpdateWorkflowResponse,
    };
}
