use super::*;

/// Define a `pub(crate) fn $name(response: $ty) -> Response<Body>` that maps a
/// transport response enum onto an HTTP response.
///
/// Every `*Response` enum here shares a `Forbidden / NotFound / Default` tail,
/// so this macro hard-codes those three terminal arms and lets call sites
/// declare just the success variant plus any extra arms (Conflict, Accepted,
/// Unprocessable, etc.) as `Variant => StatusCode::XYZ` pairs.
///
/// Use [`map_response_no_forbidden!`] for the rare enums (currently `GetTask*`)
/// that omit the Forbidden variant.
macro_rules! map_response {
    ($name:ident, $ty:path, $success:ident $(, $extra_variant:ident => $extra_status:expr)* $(,)?) => {
        pub(crate) fn $name(response: $ty) -> Response<Body> {
            use $ty::*;
            match response {
                $success(body) => json_response_with_status(&body, StatusCode::OK),
                $($extra_variant(body) => json_response_with_status(&body, $extra_status),)*
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

/// Variant of [`map_response!`] for response enums whose contract does not
/// include `ForbiddenErrorResponse` (e.g. `GetTaskResponse`).
macro_rules! map_response_no_forbidden {
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

map_response!(
    list_compute_nodes_response,
    ListComputeNodesResponse,
    SuccessfulResponse
);
map_response!(
    create_compute_node_response,
    CreateComputeNodeResponse,
    SuccessfulResponse
);
map_response!(
    create_event_response,
    CreateEventResponse,
    SuccessfulResponse
);
map_response!(create_file_response, CreateFileResponse, SuccessfulResponse);
map_response!(
    create_local_scheduler_response,
    CreateLocalSchedulerResponse,
    SuccessfulResponse
);
map_response!(
    create_result_response,
    CreateResultResponse,
    SuccessfulResponse
);
map_response!(
    create_user_data_response,
    CreateUserDataResponse,
    SuccessfulResponse
);
map_response!(
    create_scheduled_compute_node_response,
    CreateScheduledComputeNodeResponse,
    SuccessfulResponse
);
map_response!(
    create_slurm_scheduler_response,
    CreateSlurmSchedulerResponse,
    SuccessfulResponse
);
map_response!(
    create_access_group_response,
    CreateAccessGroupResponse,
    SuccessfulResponse,
    ConflictErrorResponse => StatusCode::CONFLICT
);
map_response!(create_jobs_response, CreateJobsResponse, SuccessfulResponse, UnprocessableContentErrorResponse => StatusCode::UNPROCESSABLE_ENTITY);
map_response!(
    create_failure_handler_response,
    CreateFailureHandlerResponse,
    SuccessfulResponse
);
map_response!(
    create_resource_requirements_response,
    CreateResourceRequirementsResponse,
    SuccessfulResponse,
    UnprocessableContentErrorResponse => StatusCode::UNPROCESSABLE_ENTITY
);
map_response!(
    create_slurm_stats_response,
    CreateSlurmStatsResponse,
    SuccessfulResponse
);
map_response!(
    create_ro_crate_entity_response,
    CreateRoCrateEntityResponse,
    SuccessfulResponse,
    UnprocessableContentErrorResponse => StatusCode::UNPROCESSABLE_ENTITY
);
map_response!(
    create_remote_workers_response,
    CreateRemoteWorkersResponse,
    SuccessfulResponse
);
map_response!(
    update_compute_node_response,
    UpdateComputeNodeResponse,
    SuccessfulResponse
);
map_response!(
    update_event_response,
    UpdateEventResponse,
    SuccessfulResponse
);
map_response!(update_file_response, UpdateFileResponse, SuccessfulResponse);
map_response!(
    update_local_scheduler_response,
    UpdateLocalSchedulerResponse,
    SuccessfulResponse
);
map_response!(
    update_result_response,
    UpdateResultResponse,
    SuccessfulResponse
);
map_response!(
    update_user_data_response,
    UpdateUserDataResponse,
    SuccessfulResponse
);
map_response!(
    update_scheduled_compute_node_response,
    UpdateScheduledComputeNodeResponse,
    ScheduledComputeNodeUpdatedInTheTable
);
map_response!(
    update_slurm_scheduler_response,
    UpdateSlurmSchedulerResponse,
    SuccessfulResponse
);
map_response!(
    update_resource_requirements_response,
    UpdateResourceRequirementsResponse,
    SuccessfulResponse,
    UnprocessableContentErrorResponse => StatusCode::UNPROCESSABLE_ENTITY
);
map_response!(
    delete_compute_nodes_response,
    DeleteComputeNodesResponse,
    SuccessfulResponse
);
map_response!(
    delete_events_response,
    DeleteEventsResponse,
    SuccessfulResponse
);
map_response!(
    delete_files_response,
    DeleteFilesResponse,
    SuccessfulResponse
);
map_response!(
    delete_local_schedulers_response,
    DeleteLocalSchedulersResponse,
    SuccessfulResponse
);
map_response!(
    delete_results_response,
    DeleteResultsResponse,
    SuccessfulResponse
);
map_response!(
    delete_all_user_data_response,
    DeleteAllUserDataResponse,
    SuccessfulResponse
);
map_response!(
    delete_scheduled_compute_nodes_response,
    DeleteScheduledComputeNodesResponse,
    SuccessfulResponse
);
map_response!(
    delete_slurm_schedulers_response,
    DeleteSlurmSchedulersResponse,
    Message
);
map_response!(
    delete_access_group_response,
    DeleteAccessGroupResponse,
    SuccessfulResponse
);
map_response!(
    delete_all_resource_requirements_response,
    DeleteAllResourceRequirementsResponse,
    SuccessfulResponse
);
map_response!(
    delete_failure_handler_response,
    DeleteFailureHandlerResponse,
    SuccessfulResponse
);
map_response!(
    delete_resource_requirements_response,
    DeleteResourceRequirementsResponse,
    SuccessfulResponse
);
map_response!(
    delete_ro_crate_entity_response,
    DeleteRoCrateEntityResponse,
    SuccessfulResponse
);
map_response!(
    delete_ro_crate_entities_response,
    DeleteRoCrateEntitiesResponse,
    SuccessfulResponse
);
map_response!(
    delete_remote_worker_response,
    DeleteRemoteWorkerResponse,
    SuccessfulResponse
);
map_response!(list_events_response, ListEventsResponse, SuccessfulResponse);
map_response!(list_files_response, ListFilesResponse, SuccessfulResponse);
map_response!(
    list_local_schedulers_response,
    ListLocalSchedulersResponse,
    HTTP
);
map_response!(
    list_results_response,
    ListResultsResponse,
    SuccessfulResponse
);
map_response!(
    list_user_data_response,
    ListUserDataResponse,
    SuccessfulResponse
);
map_response!(
    list_scheduled_compute_nodes_response,
    ListScheduledComputeNodesResponse,
    SuccessfulResponse
);
map_response!(
    list_slurm_schedulers_response,
    ListSlurmSchedulersResponse,
    SuccessfulResponse
);
map_response!(
    list_access_groups_response,
    ListAccessGroupsApiResponse,
    SuccessfulResponse
);
map_response!(
    list_group_members_response,
    ListGroupMembersResponse,
    SuccessfulResponse
);
map_response!(
    list_user_groups_response,
    ListUserGroupsApiResponse,
    SuccessfulResponse
);
map_response!(
    list_workflow_groups_response,
    ListWorkflowGroupsResponse,
    SuccessfulResponse
);
map_response!(
    list_failure_handlers_response,
    ListFailureHandlersResponse,
    SuccessfulResponse
);
map_response!(
    list_resource_requirements_response,
    ListResourceRequirementsResponse,
    SuccessfulResponse
);
map_response!(
    list_slurm_stats_response,
    ListSlurmStatsResponse,
    SuccessfulResponse
);
map_response!(
    list_ro_crate_entities_response,
    ListRoCrateEntitiesResponse,
    SuccessfulResponse
);
map_response!(
    list_remote_workers_response,
    ListRemoteWorkersResponse,
    SuccessfulResponse
);
map_response!(
    get_compute_node_response,
    GetComputeNodeResponse,
    SuccessfulResponse
);
map_response!(
    delete_compute_node_response,
    DeleteComputeNodeResponse,
    SuccessfulResponse
);
map_response!(
    delete_event_response,
    DeleteEventResponse,
    SuccessfulResponse
);
map_response!(delete_file_response, DeleteFileResponse, SuccessfulResponse);
map_response!(
    delete_local_scheduler_response,
    DeleteLocalSchedulerResponse,
    LocalComputeNodeConfigurationStoredInTheTable
);
map_response!(
    delete_result_response,
    DeleteResultResponse,
    SuccessfulResponse
);
map_response!(
    delete_user_data_response,
    DeleteUserDataResponse,
    SuccessfulResponse
);
map_response!(
    delete_scheduled_compute_node_response,
    DeleteScheduledComputeNodeResponse,
    SuccessfulResponse
);
map_response!(
    delete_slurm_scheduler_response,
    DeleteSlurmSchedulerResponse,
    SuccessfulResponse
);
map_response!(get_event_response, GetEventResponse, SuccessfulResponse);
map_response!(get_file_response, GetFileResponse, SuccessfulResponse);
map_response!(
    get_local_scheduler_response,
    GetLocalSchedulerResponse,
    SuccessfulResponse
);
map_response!(get_result_response, GetResultResponse, SuccessfulResponse);
map_response!(
    get_user_data_response,
    GetUserDataResponse,
    SuccessfulResponse
);
map_response!(
    get_scheduled_compute_node_response,
    GetScheduledComputeNodeResponse,
    HTTP
);
map_response!(
    get_slurm_scheduler_response,
    GetSlurmSchedulerResponse,
    SuccessfulResponse
);
map_response!(
    get_access_group_response,
    GetAccessGroupResponse,
    SuccessfulResponse
);
map_response!(
    get_failure_handler_response,
    GetFailureHandlerResponse,
    SuccessfulResponse
);
map_response!(
    get_resource_requirements_response,
    GetResourceRequirementsResponse,
    SuccessfulResponse
);
map_response!(
    get_ro_crate_entity_response,
    GetRoCrateEntityResponse,
    SuccessfulResponse
);
map_response!(
    add_user_to_group_response,
    AddUserToGroupResponse,
    SuccessfulResponse,
    ConflictErrorResponse => StatusCode::CONFLICT
);
map_response!(
    remove_user_from_group_response,
    RemoveUserFromGroupResponse,
    SuccessfulResponse
);
map_response!(
    add_workflow_to_group_response,
    AddWorkflowToGroupResponse,
    SuccessfulResponse,
    ConflictErrorResponse => StatusCode::CONFLICT
);
map_response!(
    remove_workflow_from_group_response,
    RemoveWorkflowFromGroupResponse,
    SuccessfulResponse
);
map_response!(
    check_workflow_access_response,
    CheckWorkflowAccessResponse,
    SuccessfulResponse
);
map_response!(reload_auth_response, ReloadAuthResponse, SuccessfulResponse);
map_response!(
    update_ro_crate_entity_response,
    UpdateRoCrateEntityResponse,
    SuccessfulResponse,
    UnprocessableContentErrorResponse => StatusCode::UNPROCESSABLE_ENTITY
);
map_response!(create_job_response, CreateJobResponse, SuccessfulResponse, UnprocessableContentErrorResponse => StatusCode::UNPROCESSABLE_ENTITY);
map_response!(list_jobs_response, ListJobsResponse, SuccessfulResponse);
map_response!(delete_jobs_response, DeleteJobsResponse, SuccessfulResponse);
map_response!(get_job_response, GetJobResponse, SuccessfulResponse);
map_response!(update_job_response, UpdateJobResponse, SuccessfulResponse, UnprocessableContentErrorResponse => StatusCode::UNPROCESSABLE_ENTITY);
map_response!(delete_job_response, DeleteJobResponse, SuccessfulResponse);
map_response!(
    complete_job_response,
    CompleteJobResponse,
    SuccessfulResponse,
    UnprocessableContentErrorResponse => StatusCode::UNPROCESSABLE_ENTITY
);
map_response!(
    batch_complete_jobs_response,
    BatchCompleteJobsResponse,
    SuccessfulResponse
);
map_response!(
    manage_status_change_response,
    ManageStatusChangeResponse,
    SuccessfulResponse,
    UnprocessableContentErrorResponse => StatusCode::UNPROCESSABLE_ENTITY
);
map_response!(start_job_response, StartJobResponse, SuccessfulResponse, UnprocessableContentErrorResponse => StatusCode::UNPROCESSABLE_ENTITY);
map_response!(retry_job_response, RetryJobResponse, SuccessfulResponse, UnprocessableContentErrorResponse => StatusCode::UNPROCESSABLE_ENTITY);
map_response!(
    create_workflow_response,
    CreateWorkflowResponse,
    SuccessfulResponse,
    UnprocessableContentErrorResponse => StatusCode::UNPROCESSABLE_ENTITY
);
map_response!(
    list_workflows_response,
    ListWorkflowsResponse,
    SuccessfulResponse
);
map_response!(
    get_workflow_response,
    GetWorkflowResponse,
    SuccessfulResponse
);
map_response!(
    update_workflow_response,
    UpdateWorkflowResponse,
    SuccessfulResponse,
    UnprocessableContentErrorResponse => StatusCode::UNPROCESSABLE_ENTITY
);
map_response!(
    delete_workflow_response,
    DeleteWorkflowResponse,
    SuccessfulResponse
);
map_response!(
    create_workflow_action_response,
    CreateWorkflowActionResponse,
    SuccessfulResponse,
    UnprocessableContentErrorResponse => StatusCode::UNPROCESSABLE_ENTITY
);
map_response!(
    get_workflow_actions_response,
    GetWorkflowActionsResponse,
    SuccessfulResponse
);
map_response!(
    get_pending_actions_response,
    GetPendingActionsResponse,
    SuccessfulResponse
);
map_response!(
    claim_action_response,
    ClaimActionResponse,
    SuccessfulResponse,
    ConflictResponse => StatusCode::CONFLICT
);
map_response!(
    cancel_workflow_response,
    CancelWorkflowResponse,
    SuccessfulResponse
);
map_response!(
    archive_workflow_response,
    ArchiveWorkflowResponse,
    SuccessfulResponse
);
map_response!(
    claim_jobs_based_on_resources_response,
    ClaimJobsBasedOnResources,
    SuccessfulResponse,
    UnprocessableContentErrorResponse => StatusCode::UNPROCESSABLE_ENTITY
);
map_response!(
    claim_next_jobs_response,
    ClaimNextJobsResponse,
    SuccessfulResponse
);
map_response_no_forbidden!(get_task_response, GetTaskResponse, SuccessfulResponse);

map_response_no_forbidden!(
    get_active_task_response,
    GetActiveTaskResponse,
    SuccessfulResponse
);
map_response!(
    initialize_jobs_response,
    InitializeJobsResponse,
    SuccessfulResponse,
    AcceptedResponse => StatusCode::ACCEPTED,
    ConflictErrorResponse => StatusCode::CONFLICT
);
map_response!(
    is_workflow_complete_response,
    IsWorkflowCompleteResponse,
    SuccessfulResponse
);
map_response!(
    is_workflow_uninitialized_response,
    IsWorkflowUninitializedResponse,
    SuccessfulResponse
);
map_response!(
    list_job_dependencies_response,
    ListJobDependenciesResponse,
    SuccessfulResponse
);
map_response!(
    list_job_file_relationships_response,
    ListJobFileRelationshipsResponse,
    SuccessfulResponse
);
map_response!(
    list_job_ids_response,
    ListJobIdsResponse,
    SuccessfulResponse
);
map_response!(
    list_job_user_data_relationships_response,
    ListJobUserDataRelationshipsResponse,
    SuccessfulResponse
);
map_response!(
    list_missing_user_data_response,
    ListMissingUserDataResponse,
    SuccessfulResponse
);
map_response!(
    process_changed_job_inputs_response,
    ProcessChangedJobInputsResponse,
    SuccessfulResponse
);
map_response!(
    get_ready_job_requirements_response,
    GetReadyJobRequirementsResponse,
    SuccessfulResponse
);
map_response!(
    list_required_existing_files_response,
    ListRequiredExistingFilesResponse,
    SuccessfulResponse
);
map_response!(
    reset_job_status_response,
    ResetJobStatusResponse,
    SuccessfulResponse
);
map_response!(
    reset_workflow_status_response,
    ResetWorkflowStatusResponse,
    SuccessfulResponse,
    UnprocessableContentErrorResponse => StatusCode::UNPROCESSABLE_ENTITY
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
