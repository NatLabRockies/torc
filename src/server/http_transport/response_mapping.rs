use super::*;

macro_rules! map_response_std {
    ($name:ident, $ty:path, $success:ident) => {
        pub(crate) fn $name(response: $ty) -> Response<Body> {
            use $ty::*;
            match response {
                $success(body) => json_response_with_status(&body, StatusCode::OK),
                ForbiddenErrorResponse(body) => {
                    json_response_with_status(&body, StatusCode::FORBIDDEN)
                }
                NotFoundErrorResponse(body) => {
                    json_response_with_status(&body, StatusCode::NOT_FOUND)
                }
                DefaultErrorResponse(body) => {
                    json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
    };
}

macro_rules! map_response_not_found_only {
    ($name:ident, $ty:path, $success:ident) => {
        pub(crate) fn $name(response: $ty) -> Response<Body> {
            use $ty::*;
            match response {
                $success(body) => json_response_with_status(&body, StatusCode::OK),
                NotFoundErrorResponse(body) => {
                    json_response_with_status(&body, StatusCode::NOT_FOUND)
                }
                DefaultErrorResponse(body) => {
                    json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
    };
}

macro_rules! map_response_conflict {
    ($name:ident, $ty:path, $success:ident, $conflict:ident) => {
        pub(crate) fn $name(response: $ty) -> Response<Body> {
            use $ty::*;
            match response {
                $success(body) => json_response_with_status(&body, StatusCode::OK),
                ForbiddenErrorResponse(body) => {
                    json_response_with_status(&body, StatusCode::FORBIDDEN)
                }
                NotFoundErrorResponse(body) => {
                    json_response_with_status(&body, StatusCode::NOT_FOUND)
                }
                $conflict(body) => json_response_with_status(&body, StatusCode::CONFLICT),
                DefaultErrorResponse(body) => {
                    json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
    };
}

macro_rules! map_response_accepted_conflict {
    ($name:ident, $ty:path, $success:ident, $accepted:ident, $conflict:ident) => {
        pub(crate) fn $name(response: $ty) -> Response<Body> {
            use $ty::*;
            match response {
                $success(body) => json_response_with_status(&body, StatusCode::OK),
                $accepted(body) => json_response_with_status(&body, StatusCode::ACCEPTED),
                ForbiddenErrorResponse(body) => {
                    json_response_with_status(&body, StatusCode::FORBIDDEN)
                }
                NotFoundErrorResponse(body) => {
                    json_response_with_status(&body, StatusCode::NOT_FOUND)
                }
                $conflict(body) => json_response_with_status(&body, StatusCode::CONFLICT),
                DefaultErrorResponse(body) => {
                    json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
    };
}

macro_rules! map_response_unprocessable {
    ($name:ident, $ty:path, $success:ident) => {
        pub(crate) fn $name(response: $ty) -> Response<Body> {
            use $ty::*;
            match response {
                $success(body) => json_response_with_status(&body, StatusCode::OK),
                ForbiddenErrorResponse(body) => {
                    json_response_with_status(&body, StatusCode::FORBIDDEN)
                }
                NotFoundErrorResponse(body) => {
                    json_response_with_status(&body, StatusCode::NOT_FOUND)
                }
                UnprocessableContentErrorResponse(body) => {
                    json_response_with_status(&body, StatusCode::UNPROCESSABLE_ENTITY)
                }
                DefaultErrorResponse(body) => {
                    json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
    };
}

map_response_std!(
    list_compute_nodes_response,
    ListComputeNodesResponse,
    SuccessfulResponse
);
map_response_std!(
    create_compute_node_response,
    CreateComputeNodeResponse,
    SuccessfulResponse
);
map_response_std!(
    create_event_response,
    CreateEventResponse,
    SuccessfulResponse
);
map_response_std!(create_file_response, CreateFileResponse, SuccessfulResponse);
map_response_std!(
    create_local_scheduler_response,
    CreateLocalSchedulerResponse,
    SuccessfulResponse
);
map_response_std!(
    create_result_response,
    CreateResultResponse,
    SuccessfulResponse
);
map_response_std!(
    create_user_data_response,
    CreateUserDataResponse,
    SuccessfulResponse
);
map_response_std!(
    create_scheduled_compute_node_response,
    CreateScheduledComputeNodeResponse,
    SuccessfulResponse
);
map_response_std!(
    create_slurm_scheduler_response,
    CreateSlurmSchedulerResponse,
    SuccessfulResponse
);
map_response_conflict!(
    create_access_group_response,
    CreateAccessGroupResponse,
    SuccessfulResponse,
    ConflictErrorResponse
);
map_response_unprocessable!(create_jobs_response, CreateJobsResponse, SuccessfulResponse);
map_response_std!(
    create_failure_handler_response,
    CreateFailureHandlerResponse,
    SuccessfulResponse
);
map_response_unprocessable!(
    create_resource_requirements_response,
    CreateResourceRequirementsResponse,
    SuccessfulResponse
);
map_response_std!(
    create_slurm_stats_response,
    CreateSlurmStatsResponse,
    SuccessfulResponse
);
map_response_std!(
    create_ro_crate_entity_response,
    CreateRoCrateEntityResponse,
    SuccessfulResponse
);
map_response_std!(
    create_remote_workers_response,
    CreateRemoteWorkersResponse,
    SuccessfulResponse
);
map_response_std!(
    update_compute_node_response,
    UpdateComputeNodeResponse,
    SuccessfulResponse
);
map_response_std!(
    update_event_response,
    UpdateEventResponse,
    SuccessfulResponse
);
map_response_std!(update_file_response, UpdateFileResponse, SuccessfulResponse);
map_response_std!(
    update_local_scheduler_response,
    UpdateLocalSchedulerResponse,
    SuccessfulResponse
);
map_response_std!(
    update_result_response,
    UpdateResultResponse,
    SuccessfulResponse
);
map_response_std!(
    update_user_data_response,
    UpdateUserDataResponse,
    SuccessfulResponse
);
map_response_std!(
    update_scheduled_compute_node_response,
    UpdateScheduledComputeNodeResponse,
    ScheduledComputeNodeUpdatedInTheTable
);
map_response_std!(
    update_slurm_scheduler_response,
    UpdateSlurmSchedulerResponse,
    SuccessfulResponse
);
map_response_unprocessable!(
    update_resource_requirements_response,
    UpdateResourceRequirementsResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_compute_nodes_response,
    DeleteComputeNodesResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_events_response,
    DeleteEventsResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_files_response,
    DeleteFilesResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_local_schedulers_response,
    DeleteLocalSchedulersResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_results_response,
    DeleteResultsResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_all_user_data_response,
    DeleteAllUserDataResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_scheduled_compute_nodes_response,
    DeleteScheduledComputeNodesResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_slurm_schedulers_response,
    DeleteSlurmSchedulersResponse,
    Message
);
map_response_std!(
    delete_access_group_response,
    DeleteAccessGroupResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_all_resource_requirements_response,
    DeleteAllResourceRequirementsResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_failure_handler_response,
    DeleteFailureHandlerResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_resource_requirements_response,
    DeleteResourceRequirementsResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_ro_crate_entity_response,
    DeleteRoCrateEntityResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_ro_crate_entities_response,
    DeleteRoCrateEntitiesResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_remote_worker_response,
    DeleteRemoteWorkerResponse,
    SuccessfulResponse
);
map_response_std!(list_events_response, ListEventsResponse, SuccessfulResponse);
map_response_std!(list_files_response, ListFilesResponse, SuccessfulResponse);
map_response_std!(
    list_local_schedulers_response,
    ListLocalSchedulersResponse,
    HTTP
);
map_response_std!(
    list_results_response,
    ListResultsResponse,
    SuccessfulResponse
);
map_response_std!(
    list_user_data_response,
    ListUserDataResponse,
    SuccessfulResponse
);
map_response_std!(
    list_scheduled_compute_nodes_response,
    ListScheduledComputeNodesResponse,
    SuccessfulResponse
);
map_response_std!(
    list_slurm_schedulers_response,
    ListSlurmSchedulersResponse,
    SuccessfulResponse
);
map_response_std!(
    list_access_groups_response,
    ListAccessGroupsApiResponse,
    SuccessfulResponse
);
map_response_std!(
    list_group_members_response,
    ListGroupMembersResponse,
    SuccessfulResponse
);
map_response_std!(
    list_user_groups_response,
    ListUserGroupsApiResponse,
    SuccessfulResponse
);
map_response_std!(
    list_workflow_groups_response,
    ListWorkflowGroupsResponse,
    SuccessfulResponse
);
map_response_std!(
    list_failure_handlers_response,
    ListFailureHandlersResponse,
    SuccessfulResponse
);
map_response_std!(
    list_resource_requirements_response,
    ListResourceRequirementsResponse,
    SuccessfulResponse
);
map_response_std!(
    list_slurm_stats_response,
    ListSlurmStatsResponse,
    SuccessfulResponse
);
map_response_std!(
    list_ro_crate_entities_response,
    ListRoCrateEntitiesResponse,
    SuccessfulResponse
);
map_response_std!(
    list_remote_workers_response,
    ListRemoteWorkersResponse,
    SuccessfulResponse
);
map_response_std!(
    get_compute_node_response,
    GetComputeNodeResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_compute_node_response,
    DeleteComputeNodeResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_event_response,
    DeleteEventResponse,
    SuccessfulResponse
);
map_response_std!(delete_file_response, DeleteFileResponse, SuccessfulResponse);
map_response_std!(
    delete_local_scheduler_response,
    DeleteLocalSchedulerResponse,
    LocalComputeNodeConfigurationStoredInTheTable
);
map_response_std!(
    delete_result_response,
    DeleteResultResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_user_data_response,
    DeleteUserDataResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_scheduled_compute_node_response,
    DeleteScheduledComputeNodeResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_slurm_scheduler_response,
    DeleteSlurmSchedulerResponse,
    SuccessfulResponse
);
map_response_std!(get_event_response, GetEventResponse, SuccessfulResponse);
map_response_std!(get_file_response, GetFileResponse, SuccessfulResponse);
map_response_std!(
    get_local_scheduler_response,
    GetLocalSchedulerResponse,
    SuccessfulResponse
);
map_response_std!(get_result_response, GetResultResponse, SuccessfulResponse);
map_response_std!(
    get_user_data_response,
    GetUserDataResponse,
    SuccessfulResponse
);
map_response_std!(
    get_scheduled_compute_node_response,
    GetScheduledComputeNodeResponse,
    HTTP
);
map_response_std!(
    get_slurm_scheduler_response,
    GetSlurmSchedulerResponse,
    SuccessfulResponse
);
map_response_std!(
    get_access_group_response,
    GetAccessGroupResponse,
    SuccessfulResponse
);
map_response_std!(
    get_failure_handler_response,
    GetFailureHandlerResponse,
    SuccessfulResponse
);
map_response_std!(
    get_resource_requirements_response,
    GetResourceRequirementsResponse,
    SuccessfulResponse
);
map_response_std!(
    get_ro_crate_entity_response,
    GetRoCrateEntityResponse,
    SuccessfulResponse
);
map_response_conflict!(
    add_user_to_group_response,
    AddUserToGroupResponse,
    SuccessfulResponse,
    ConflictErrorResponse
);
map_response_std!(
    remove_user_from_group_response,
    RemoveUserFromGroupResponse,
    SuccessfulResponse
);
map_response_conflict!(
    add_workflow_to_group_response,
    AddWorkflowToGroupResponse,
    SuccessfulResponse,
    ConflictErrorResponse
);
map_response_std!(
    remove_workflow_from_group_response,
    RemoveWorkflowFromGroupResponse,
    SuccessfulResponse
);
map_response_std!(
    check_workflow_access_response,
    CheckWorkflowAccessResponse,
    SuccessfulResponse
);
map_response_std!(reload_auth_response, ReloadAuthResponse, SuccessfulResponse);
map_response_std!(
    update_ro_crate_entity_response,
    UpdateRoCrateEntityResponse,
    SuccessfulResponse
);
map_response_unprocessable!(create_job_response, CreateJobResponse, SuccessfulResponse);
map_response_std!(list_jobs_response, ListJobsResponse, SuccessfulResponse);
map_response_std!(delete_jobs_response, DeleteJobsResponse, SuccessfulResponse);
map_response_std!(get_job_response, GetJobResponse, SuccessfulResponse);
map_response_unprocessable!(update_job_response, UpdateJobResponse, SuccessfulResponse);
map_response_std!(delete_job_response, DeleteJobResponse, SuccessfulResponse);
map_response_unprocessable!(
    complete_job_response,
    CompleteJobResponse,
    SuccessfulResponse
);
map_response_std!(
    batch_complete_jobs_response,
    BatchCompleteJobsResponse,
    SuccessfulResponse
);
map_response_unprocessable!(
    manage_status_change_response,
    ManageStatusChangeResponse,
    SuccessfulResponse
);
map_response_unprocessable!(start_job_response, StartJobResponse, SuccessfulResponse);
map_response_unprocessable!(retry_job_response, RetryJobResponse, SuccessfulResponse);
map_response_unprocessable!(
    create_workflow_response,
    CreateWorkflowResponse,
    SuccessfulResponse
);
map_response_std!(
    list_workflows_response,
    ListWorkflowsResponse,
    SuccessfulResponse
);
map_response_std!(
    get_workflow_response,
    GetWorkflowResponse,
    SuccessfulResponse
);
map_response_unprocessable!(
    update_workflow_response,
    UpdateWorkflowResponse,
    SuccessfulResponse
);
map_response_std!(
    delete_workflow_response,
    DeleteWorkflowResponse,
    SuccessfulResponse
);
map_response_unprocessable!(
    create_workflow_action_response,
    CreateWorkflowActionResponse,
    SuccessfulResponse
);
map_response_std!(
    get_workflow_actions_response,
    GetWorkflowActionsResponse,
    SuccessfulResponse
);
map_response_std!(
    get_pending_actions_response,
    GetPendingActionsResponse,
    SuccessfulResponse
);
map_response_conflict!(
    claim_action_response,
    ClaimActionResponse,
    SuccessfulResponse,
    ConflictResponse
);
map_response_std!(
    cancel_workflow_response,
    CancelWorkflowResponse,
    SuccessfulResponse
);
map_response_unprocessable!(
    claim_jobs_based_on_resources_response,
    ClaimJobsBasedOnResources,
    SuccessfulResponse
);
map_response_std!(
    claim_next_jobs_response,
    ClaimNextJobsResponse,
    SuccessfulResponse
);
map_response_not_found_only!(get_task_response, GetTaskResponse, SuccessfulResponse);

map_response_not_found_only!(
    get_active_task_response,
    GetActiveTaskResponse,
    SuccessfulResponse
);
map_response_accepted_conflict!(
    initialize_jobs_response,
    InitializeJobsResponse,
    SuccessfulResponse,
    AcceptedResponse,
    ConflictErrorResponse
);
map_response_std!(
    is_workflow_complete_response,
    IsWorkflowCompleteResponse,
    SuccessfulResponse
);
map_response_std!(
    is_workflow_uninitialized_response,
    IsWorkflowUninitializedResponse,
    SuccessfulResponse
);
map_response_std!(
    list_job_dependencies_response,
    ListJobDependenciesResponse,
    SuccessfulResponse
);
map_response_std!(
    list_job_file_relationships_response,
    ListJobFileRelationshipsResponse,
    SuccessfulResponse
);
map_response_std!(
    list_job_ids_response,
    ListJobIdsResponse,
    SuccessfulResponse
);
map_response_std!(
    list_job_user_data_relationships_response,
    ListJobUserDataRelationshipsResponse,
    SuccessfulResponse
);
map_response_std!(
    list_missing_user_data_response,
    ListMissingUserDataResponse,
    SuccessfulResponse
);
map_response_std!(
    process_changed_job_inputs_response,
    ProcessChangedJobInputsResponse,
    SuccessfulResponse
);
map_response_std!(
    get_ready_job_requirements_response,
    GetReadyJobRequirementsResponse,
    SuccessfulResponse
);
map_response_std!(
    list_required_existing_files_response,
    ListRequiredExistingFilesResponse,
    SuccessfulResponse
);
map_response_std!(
    reset_job_status_response,
    ResetJobStatusResponse,
    SuccessfulResponse
);
map_response_unprocessable!(
    reset_workflow_status_response,
    ResetWorkflowStatusResponse,
    SuccessfulResponse
);
map_response_std!(
    get_workflow_status_response,
    GetWorkflowStatusResponse,
    SuccessfulResponse
);
map_response_std!(
    update_workflow_status_response,
    UpdateWorkflowStatusResponse,
    SuccessfulResponse
);
pub(crate) fn json_response<T>(body: &T) -> Response<Body>
where
    T: serde::Serialize,
{
    json_response_with_status(body, StatusCode::OK)
}

pub(crate) fn json_response_with_status<T>(body: &T, status: StatusCode) -> Response<Body>
where
    T: serde::Serialize,
{
    let payload = serde_json::to_vec(body).expect("live bridge response should serialize");
    let mut response = Response::new(Body::from(payload));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

pub(crate) fn error_response(status: StatusCode, message: String) -> Response<Body> {
    json_response_with_status(
        &models::ErrorResponse::new(serde_json::json!({
            "error": status
                .canonical_reason()
                .unwrap_or("Error")
                .replace(' ', ""),
            "message": message,
        })),
        status,
    )
}

pub(crate) fn not_found_response() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .expect("valid not-found response")
}
