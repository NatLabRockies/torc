fn list_compute_nodes_response(response: ListComputeNodesResponse) -> Response<Body> {
    match response {
        ListComputeNodesResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListComputeNodesResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListComputeNodesResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListComputeNodesResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_compute_node_response(response: CreateComputeNodeResponse) -> Response<Body> {
    match response {
        CreateComputeNodeResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CreateComputeNodeResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CreateComputeNodeResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CreateComputeNodeResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_event_response(response: CreateEventResponse) -> Response<Body> {
    match response {
        CreateEventResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CreateEventResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CreateEventResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CreateEventResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_file_response(response: CreateFileResponse) -> Response<Body> {
    match response {
        CreateFileResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CreateFileResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CreateFileResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CreateFileResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_local_scheduler_response(response: CreateLocalSchedulerResponse) -> Response<Body> {
    match response {
        CreateLocalSchedulerResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CreateLocalSchedulerResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CreateLocalSchedulerResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CreateLocalSchedulerResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_result_response(response: CreateResultResponse) -> Response<Body> {
    match response {
        CreateResultResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CreateResultResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CreateResultResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CreateResultResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_user_data_response(response: CreateUserDataResponse) -> Response<Body> {
    match response {
        CreateUserDataResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CreateUserDataResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CreateUserDataResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CreateUserDataResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_scheduled_compute_node_response(
    response: CreateScheduledComputeNodeResponse,
) -> Response<Body> {
    match response {
        CreateScheduledComputeNodeResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CreateScheduledComputeNodeResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CreateScheduledComputeNodeResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CreateScheduledComputeNodeResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_slurm_scheduler_response(response: CreateSlurmSchedulerResponse) -> Response<Body> {
    match response {
        CreateSlurmSchedulerResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CreateSlurmSchedulerResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CreateSlurmSchedulerResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CreateSlurmSchedulerResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_access_group_response(response: CreateAccessGroupResponse) -> Response<Body> {
    match response {
        CreateAccessGroupResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CreateAccessGroupResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CreateAccessGroupResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CreateAccessGroupResponse::ConflictErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::CONFLICT)
        }
        CreateAccessGroupResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_jobs_response(response: CreateJobsResponse) -> Response<Body> {
    match response {
        CreateJobsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CreateJobsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CreateJobsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CreateJobsResponse::UnprocessableContentErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::UNPROCESSABLE_ENTITY)
        }
        CreateJobsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_failure_handler_response(response: CreateFailureHandlerResponse) -> Response<Body> {
    match response {
        CreateFailureHandlerResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CreateFailureHandlerResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CreateFailureHandlerResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CreateFailureHandlerResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_resource_requirements_response(
    response: CreateResourceRequirementsResponse,
) -> Response<Body> {
    match response {
        CreateResourceRequirementsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CreateResourceRequirementsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CreateResourceRequirementsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CreateResourceRequirementsResponse::UnprocessableContentErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::UNPROCESSABLE_ENTITY)
        }
        CreateResourceRequirementsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_slurm_stats_response(response: CreateSlurmStatsResponse) -> Response<Body> {
    match response {
        CreateSlurmStatsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CreateSlurmStatsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CreateSlurmStatsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CreateSlurmStatsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_ro_crate_entity_response(response: CreateRoCrateEntityResponse) -> Response<Body> {
    match response {
        CreateRoCrateEntityResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CreateRoCrateEntityResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CreateRoCrateEntityResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CreateRoCrateEntityResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_remote_workers_response(response: CreateRemoteWorkersResponse) -> Response<Body> {
    match response {
        CreateRemoteWorkersResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CreateRemoteWorkersResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CreateRemoteWorkersResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CreateRemoteWorkersResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn update_compute_node_response(response: UpdateComputeNodeResponse) -> Response<Body> {
    match response {
        UpdateComputeNodeResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        UpdateComputeNodeResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        UpdateComputeNodeResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        UpdateComputeNodeResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn update_event_response(response: UpdateEventResponse) -> Response<Body> {
    match response {
        UpdateEventResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        UpdateEventResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        UpdateEventResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        UpdateEventResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn update_file_response(response: UpdateFileResponse) -> Response<Body> {
    match response {
        UpdateFileResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        UpdateFileResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        UpdateFileResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        UpdateFileResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn update_local_scheduler_response(response: UpdateLocalSchedulerResponse) -> Response<Body> {
    match response {
        UpdateLocalSchedulerResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        UpdateLocalSchedulerResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        UpdateLocalSchedulerResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        UpdateLocalSchedulerResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn update_result_response(response: UpdateResultResponse) -> Response<Body> {
    match response {
        UpdateResultResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        UpdateResultResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        UpdateResultResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        UpdateResultResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn update_user_data_response(response: UpdateUserDataResponse) -> Response<Body> {
    match response {
        UpdateUserDataResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        UpdateUserDataResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        UpdateUserDataResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        UpdateUserDataResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn update_scheduled_compute_node_response(
    response: UpdateScheduledComputeNodeResponse,
) -> Response<Body> {
    match response {
        UpdateScheduledComputeNodeResponse::ScheduledComputeNodeUpdatedInTheTable(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        UpdateScheduledComputeNodeResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        UpdateScheduledComputeNodeResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        UpdateScheduledComputeNodeResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn update_slurm_scheduler_response(response: UpdateSlurmSchedulerResponse) -> Response<Body> {
    match response {
        UpdateSlurmSchedulerResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        UpdateSlurmSchedulerResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        UpdateSlurmSchedulerResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        UpdateSlurmSchedulerResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn update_resource_requirements_response(
    response: UpdateResourceRequirementsResponse,
) -> Response<Body> {
    match response {
        UpdateResourceRequirementsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        UpdateResourceRequirementsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        UpdateResourceRequirementsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        UpdateResourceRequirementsResponse::UnprocessableContentErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::UNPROCESSABLE_ENTITY)
        }
        UpdateResourceRequirementsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_compute_nodes_response(response: DeleteComputeNodesResponse) -> Response<Body> {
    match response {
        DeleteComputeNodesResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteComputeNodesResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteComputeNodesResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteComputeNodesResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_events_response(response: DeleteEventsResponse) -> Response<Body> {
    match response {
        DeleteEventsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteEventsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteEventsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteEventsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_files_response(response: DeleteFilesResponse) -> Response<Body> {
    match response {
        DeleteFilesResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteFilesResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteFilesResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteFilesResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_local_schedulers_response(response: DeleteLocalSchedulersResponse) -> Response<Body> {
    match response {
        DeleteLocalSchedulersResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteLocalSchedulersResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteLocalSchedulersResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteLocalSchedulersResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_results_response(response: DeleteResultsResponse) -> Response<Body> {
    match response {
        DeleteResultsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteResultsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteResultsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteResultsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_all_user_data_response(response: DeleteAllUserDataResponse) -> Response<Body> {
    match response {
        DeleteAllUserDataResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteAllUserDataResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteAllUserDataResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteAllUserDataResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_scheduled_compute_nodes_response(
    response: DeleteScheduledComputeNodesResponse,
) -> Response<Body> {
    match response {
        DeleteScheduledComputeNodesResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteScheduledComputeNodesResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteScheduledComputeNodesResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteScheduledComputeNodesResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_slurm_schedulers_response(response: DeleteSlurmSchedulersResponse) -> Response<Body> {
    match response {
        DeleteSlurmSchedulersResponse::Message(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteSlurmSchedulersResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteSlurmSchedulersResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteSlurmSchedulersResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_access_group_response(response: DeleteAccessGroupResponse) -> Response<Body> {
    match response {
        DeleteAccessGroupResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteAccessGroupResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteAccessGroupResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteAccessGroupResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_all_resource_requirements_response(
    response: DeleteAllResourceRequirementsResponse,
) -> Response<Body> {
    match response {
        DeleteAllResourceRequirementsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteAllResourceRequirementsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteAllResourceRequirementsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteAllResourceRequirementsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_failure_handler_response(response: DeleteFailureHandlerResponse) -> Response<Body> {
    match response {
        DeleteFailureHandlerResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteFailureHandlerResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteFailureHandlerResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteFailureHandlerResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_resource_requirements_response(
    response: DeleteResourceRequirementsResponse,
) -> Response<Body> {
    match response {
        DeleteResourceRequirementsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteResourceRequirementsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteResourceRequirementsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteResourceRequirementsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_ro_crate_entity_response(response: DeleteRoCrateEntityResponse) -> Response<Body> {
    match response {
        DeleteRoCrateEntityResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteRoCrateEntityResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteRoCrateEntityResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteRoCrateEntityResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_ro_crate_entities_response(response: DeleteRoCrateEntitiesResponse) -> Response<Body> {
    match response {
        DeleteRoCrateEntitiesResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteRoCrateEntitiesResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteRoCrateEntitiesResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteRoCrateEntitiesResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_remote_worker_response(response: DeleteRemoteWorkerResponse) -> Response<Body> {
    match response {
        DeleteRemoteWorkerResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteRemoteWorkerResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteRemoteWorkerResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteRemoteWorkerResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_events_response(response: ListEventsResponse) -> Response<Body> {
    match response {
        ListEventsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListEventsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListEventsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListEventsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_files_response(response: ListFilesResponse) -> Response<Body> {
    match response {
        ListFilesResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListFilesResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListFilesResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListFilesResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_local_schedulers_response(response: ListLocalSchedulersResponse) -> Response<Body> {
    match response {
        ListLocalSchedulersResponse::HTTP(body) => json_response_with_status(&body, StatusCode::OK),
        ListLocalSchedulersResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListLocalSchedulersResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListLocalSchedulersResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_results_response(response: ListResultsResponse) -> Response<Body> {
    match response {
        ListResultsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListResultsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListResultsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListResultsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_user_data_response(response: ListUserDataResponse) -> Response<Body> {
    match response {
        ListUserDataResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListUserDataResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListUserDataResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListUserDataResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_scheduled_compute_nodes_response(
    response: ListScheduledComputeNodesResponse,
) -> Response<Body> {
    match response {
        ListScheduledComputeNodesResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListScheduledComputeNodesResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListScheduledComputeNodesResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListScheduledComputeNodesResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_slurm_schedulers_response(response: ListSlurmSchedulersResponse) -> Response<Body> {
    match response {
        ListSlurmSchedulersResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListSlurmSchedulersResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListSlurmSchedulersResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListSlurmSchedulersResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_access_groups_response(response: ListAccessGroupsApiResponse) -> Response<Body> {
    match response {
        ListAccessGroupsApiResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListAccessGroupsApiResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListAccessGroupsApiResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListAccessGroupsApiResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_group_members_response(response: ListGroupMembersResponse) -> Response<Body> {
    match response {
        ListGroupMembersResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListGroupMembersResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListGroupMembersResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListGroupMembersResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_user_groups_response(response: ListUserGroupsApiResponse) -> Response<Body> {
    match response {
        ListUserGroupsApiResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListUserGroupsApiResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListUserGroupsApiResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListUserGroupsApiResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_workflow_groups_response(response: ListWorkflowGroupsResponse) -> Response<Body> {
    match response {
        ListWorkflowGroupsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListWorkflowGroupsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListWorkflowGroupsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListWorkflowGroupsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_failure_handlers_response(response: ListFailureHandlersResponse) -> Response<Body> {
    match response {
        ListFailureHandlersResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListFailureHandlersResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListFailureHandlersResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListFailureHandlersResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_resource_requirements_response(
    response: ListResourceRequirementsResponse,
) -> Response<Body> {
    match response {
        ListResourceRequirementsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListResourceRequirementsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListResourceRequirementsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListResourceRequirementsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_slurm_stats_response(response: ListSlurmStatsResponse) -> Response<Body> {
    match response {
        ListSlurmStatsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListSlurmStatsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListSlurmStatsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListSlurmStatsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_ro_crate_entities_response(response: ListRoCrateEntitiesResponse) -> Response<Body> {
    match response {
        ListRoCrateEntitiesResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListRoCrateEntitiesResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListRoCrateEntitiesResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListRoCrateEntitiesResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_remote_workers_response(response: ListRemoteWorkersResponse) -> Response<Body> {
    match response {
        ListRemoteWorkersResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListRemoteWorkersResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListRemoteWorkersResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListRemoteWorkersResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn get_compute_node_response(response: GetComputeNodeResponse) -> Response<Body> {
    match response {
        GetComputeNodeResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        GetComputeNodeResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        GetComputeNodeResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        GetComputeNodeResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_compute_node_response(response: DeleteComputeNodeResponse) -> Response<Body> {
    match response {
        DeleteComputeNodeResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteComputeNodeResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteComputeNodeResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteComputeNodeResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_event_response(response: DeleteEventResponse) -> Response<Body> {
    match response {
        DeleteEventResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteEventResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteEventResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteEventResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_file_response(response: DeleteFileResponse) -> Response<Body> {
    match response {
        DeleteFileResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteFileResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteFileResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteFileResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_local_scheduler_response(response: DeleteLocalSchedulerResponse) -> Response<Body> {
    match response {
        DeleteLocalSchedulerResponse::LocalComputeNodeConfigurationStoredInTheTable(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteLocalSchedulerResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteLocalSchedulerResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteLocalSchedulerResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_result_response(response: DeleteResultResponse) -> Response<Body> {
    match response {
        DeleteResultResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteResultResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteResultResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteResultResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_user_data_response(response: DeleteUserDataResponse) -> Response<Body> {
    match response {
        DeleteUserDataResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteUserDataResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteUserDataResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteUserDataResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_scheduled_compute_node_response(
    response: DeleteScheduledComputeNodeResponse,
) -> Response<Body> {
    match response {
        DeleteScheduledComputeNodeResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteScheduledComputeNodeResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteScheduledComputeNodeResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteScheduledComputeNodeResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_slurm_scheduler_response(response: DeleteSlurmSchedulerResponse) -> Response<Body> {
    match response {
        DeleteSlurmSchedulerResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteSlurmSchedulerResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteSlurmSchedulerResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteSlurmSchedulerResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn get_event_response(response: GetEventResponse) -> Response<Body> {
    match response {
        GetEventResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        GetEventResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        GetEventResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        GetEventResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn get_file_response(response: GetFileResponse) -> Response<Body> {
    match response {
        GetFileResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        GetFileResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        GetFileResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        GetFileResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn get_local_scheduler_response(response: GetLocalSchedulerResponse) -> Response<Body> {
    match response {
        GetLocalSchedulerResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        GetLocalSchedulerResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        GetLocalSchedulerResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        GetLocalSchedulerResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn get_result_response(response: GetResultResponse) -> Response<Body> {
    match response {
        GetResultResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        GetResultResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        GetResultResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        GetResultResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn get_user_data_response(response: GetUserDataResponse) -> Response<Body> {
    match response {
        GetUserDataResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        GetUserDataResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        GetUserDataResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        GetUserDataResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn get_scheduled_compute_node_response(
    response: GetScheduledComputeNodeResponse,
) -> Response<Body> {
    match response {
        GetScheduledComputeNodeResponse::HTTP(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        GetScheduledComputeNodeResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        GetScheduledComputeNodeResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        GetScheduledComputeNodeResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn get_slurm_scheduler_response(response: GetSlurmSchedulerResponse) -> Response<Body> {
    match response {
        GetSlurmSchedulerResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        GetSlurmSchedulerResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        GetSlurmSchedulerResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        GetSlurmSchedulerResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn get_access_group_response(response: GetAccessGroupResponse) -> Response<Body> {
    match response {
        GetAccessGroupResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        GetAccessGroupResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        GetAccessGroupResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        GetAccessGroupResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn get_failure_handler_response(response: GetFailureHandlerResponse) -> Response<Body> {
    match response {
        GetFailureHandlerResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        GetFailureHandlerResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        GetFailureHandlerResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        GetFailureHandlerResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn get_resource_requirements_response(response: GetResourceRequirementsResponse) -> Response<Body> {
    match response {
        GetResourceRequirementsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        GetResourceRequirementsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        GetResourceRequirementsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        GetResourceRequirementsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn get_ro_crate_entity_response(response: GetRoCrateEntityResponse) -> Response<Body> {
    match response {
        GetRoCrateEntityResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        GetRoCrateEntityResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        GetRoCrateEntityResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        GetRoCrateEntityResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn add_user_to_group_response(response: AddUserToGroupResponse) -> Response<Body> {
    match response {
        AddUserToGroupResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        AddUserToGroupResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        AddUserToGroupResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        AddUserToGroupResponse::ConflictErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::CONFLICT)
        }
        AddUserToGroupResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn remove_user_from_group_response(response: RemoveUserFromGroupResponse) -> Response<Body> {
    match response {
        RemoveUserFromGroupResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        RemoveUserFromGroupResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        RemoveUserFromGroupResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        RemoveUserFromGroupResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn add_workflow_to_group_response(response: AddWorkflowToGroupResponse) -> Response<Body> {
    match response {
        AddWorkflowToGroupResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        AddWorkflowToGroupResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        AddWorkflowToGroupResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        AddWorkflowToGroupResponse::ConflictErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::CONFLICT)
        }
        AddWorkflowToGroupResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn remove_workflow_from_group_response(
    response: RemoveWorkflowFromGroupResponse,
) -> Response<Body> {
    match response {
        RemoveWorkflowFromGroupResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        RemoveWorkflowFromGroupResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        RemoveWorkflowFromGroupResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        RemoveWorkflowFromGroupResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn check_workflow_access_response(response: CheckWorkflowAccessResponse) -> Response<Body> {
    match response {
        CheckWorkflowAccessResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CheckWorkflowAccessResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CheckWorkflowAccessResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CheckWorkflowAccessResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn reload_auth_response(response: ReloadAuthResponse) -> Response<Body> {
    match response {
        ReloadAuthResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ReloadAuthResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ReloadAuthResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ReloadAuthResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn update_ro_crate_entity_response(response: UpdateRoCrateEntityResponse) -> Response<Body> {
    match response {
        UpdateRoCrateEntityResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        UpdateRoCrateEntityResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        UpdateRoCrateEntityResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        UpdateRoCrateEntityResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_job_response(response: CreateJobResponse) -> Response<Body> {
    match response {
        CreateJobResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CreateJobResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CreateJobResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CreateJobResponse::UnprocessableContentErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::UNPROCESSABLE_ENTITY)
        }
        CreateJobResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_jobs_response(response: ListJobsResponse) -> Response<Body> {
    match response {
        ListJobsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListJobsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListJobsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListJobsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_jobs_response(response: DeleteJobsResponse) -> Response<Body> {
    match response {
        DeleteJobsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteJobsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteJobsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteJobsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn get_job_response(response: GetJobResponse) -> Response<Body> {
    match response {
        GetJobResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        GetJobResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        GetJobResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        GetJobResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn update_job_response(response: UpdateJobResponse) -> Response<Body> {
    match response {
        UpdateJobResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        UpdateJobResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        UpdateJobResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        UpdateJobResponse::UnprocessableContentErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::UNPROCESSABLE_ENTITY)
        }
        UpdateJobResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_job_response(response: DeleteJobResponse) -> Response<Body> {
    match response {
        DeleteJobResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteJobResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteJobResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteJobResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn complete_job_response(response: CompleteJobResponse) -> Response<Body> {
    match response {
        CompleteJobResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CompleteJobResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CompleteJobResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CompleteJobResponse::UnprocessableContentErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::UNPROCESSABLE_ENTITY)
        }
        CompleteJobResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn manage_status_change_response(response: ManageStatusChangeResponse) -> Response<Body> {
    match response {
        ManageStatusChangeResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ManageStatusChangeResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ManageStatusChangeResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ManageStatusChangeResponse::UnprocessableContentErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::UNPROCESSABLE_ENTITY)
        }
        ManageStatusChangeResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn start_job_response(response: StartJobResponse) -> Response<Body> {
    match response {
        StartJobResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        StartJobResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        StartJobResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        StartJobResponse::UnprocessableContentErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::UNPROCESSABLE_ENTITY)
        }
        StartJobResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn retry_job_response(response: RetryJobResponse) -> Response<Body> {
    match response {
        RetryJobResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        RetryJobResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        RetryJobResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        RetryJobResponse::UnprocessableContentErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::UNPROCESSABLE_ENTITY)
        }
        RetryJobResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_workflow_response(response: CreateWorkflowResponse) -> Response<Body> {
    match response {
        CreateWorkflowResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CreateWorkflowResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CreateWorkflowResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CreateWorkflowResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_workflows_response(response: ListWorkflowsResponse) -> Response<Body> {
    match response {
        ListWorkflowsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListWorkflowsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListWorkflowsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListWorkflowsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn get_workflow_response(response: GetWorkflowResponse) -> Response<Body> {
    match response {
        GetWorkflowResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        GetWorkflowResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        GetWorkflowResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        GetWorkflowResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn update_workflow_response(response: UpdateWorkflowResponse) -> Response<Body> {
    match response {
        UpdateWorkflowResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        UpdateWorkflowResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        UpdateWorkflowResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        UpdateWorkflowResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn delete_workflow_response(response: DeleteWorkflowResponse) -> Response<Body> {
    match response {
        DeleteWorkflowResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        DeleteWorkflowResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        DeleteWorkflowResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        DeleteWorkflowResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_workflow_action_response(response: CreateWorkflowActionResponse) -> Response<Body> {
    match response {
        CreateWorkflowActionResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CreateWorkflowActionResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CreateWorkflowActionResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CreateWorkflowActionResponse::UnprocessableContentErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::UNPROCESSABLE_ENTITY)
        }
        CreateWorkflowActionResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn get_workflow_actions_response(response: GetWorkflowActionsResponse) -> Response<Body> {
    match response {
        GetWorkflowActionsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        GetWorkflowActionsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        GetWorkflowActionsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        GetWorkflowActionsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn get_pending_actions_response(response: GetPendingActionsResponse) -> Response<Body> {
    match response {
        GetPendingActionsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        GetPendingActionsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        GetPendingActionsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        GetPendingActionsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn claim_action_response(response: ClaimActionResponse) -> Response<Body> {
    match response {
        ClaimActionResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ClaimActionResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ClaimActionResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ClaimActionResponse::ConflictResponse(body) => {
            json_response_with_status(&body, StatusCode::CONFLICT)
        }
        ClaimActionResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn cancel_workflow_response(response: CancelWorkflowResponse) -> Response<Body> {
    match response {
        CancelWorkflowResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        CancelWorkflowResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        CancelWorkflowResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        CancelWorkflowResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn claim_jobs_based_on_resources_response(response: ClaimJobsBasedOnResources) -> Response<Body> {
    match response {
        ClaimJobsBasedOnResources::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ClaimJobsBasedOnResources::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ClaimJobsBasedOnResources::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ClaimJobsBasedOnResources::UnprocessableContentErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::UNPROCESSABLE_ENTITY)
        }
        ClaimJobsBasedOnResources::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn claim_next_jobs_response(response: ClaimNextJobsResponse) -> Response<Body> {
    match response {
        ClaimNextJobsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ClaimNextJobsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ClaimNextJobsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ClaimNextJobsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn initialize_jobs_response(response: InitializeJobsResponse) -> Response<Body> {
    match response {
        InitializeJobsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        InitializeJobsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        InitializeJobsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        InitializeJobsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn is_workflow_complete_response(response: IsWorkflowCompleteResponse) -> Response<Body> {
    match response {
        IsWorkflowCompleteResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        IsWorkflowCompleteResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        IsWorkflowCompleteResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        IsWorkflowCompleteResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn is_workflow_uninitialized_response(response: IsWorkflowUninitializedResponse) -> Response<Body> {
    match response {
        IsWorkflowUninitializedResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        IsWorkflowUninitializedResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        IsWorkflowUninitializedResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        IsWorkflowUninitializedResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_job_dependencies_response(response: ListJobDependenciesResponse) -> Response<Body> {
    match response {
        ListJobDependenciesResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListJobDependenciesResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListJobDependenciesResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListJobDependenciesResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_job_file_relationships_response(
    response: ListJobFileRelationshipsResponse,
) -> Response<Body> {
    match response {
        ListJobFileRelationshipsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListJobFileRelationshipsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListJobFileRelationshipsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListJobFileRelationshipsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_job_ids_response(response: ListJobIdsResponse) -> Response<Body> {
    match response {
        ListJobIdsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListJobIdsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListJobIdsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListJobIdsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_job_user_data_relationships_response(
    response: ListJobUserDataRelationshipsResponse,
) -> Response<Body> {
    match response {
        ListJobUserDataRelationshipsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListJobUserDataRelationshipsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListJobUserDataRelationshipsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListJobUserDataRelationshipsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_missing_user_data_response(response: ListMissingUserDataResponse) -> Response<Body> {
    match response {
        ListMissingUserDataResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListMissingUserDataResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListMissingUserDataResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListMissingUserDataResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn process_changed_job_inputs_response(
    response: ProcessChangedJobInputsResponse,
) -> Response<Body> {
    match response {
        ProcessChangedJobInputsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ProcessChangedJobInputsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ProcessChangedJobInputsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ProcessChangedJobInputsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn get_ready_job_requirements_response(
    response: GetReadyJobRequirementsResponse,
) -> Response<Body> {
    match response {
        GetReadyJobRequirementsResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        GetReadyJobRequirementsResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        GetReadyJobRequirementsResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        GetReadyJobRequirementsResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn list_required_existing_files_response(
    response: ListRequiredExistingFilesResponse,
) -> Response<Body> {
    match response {
        ListRequiredExistingFilesResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ListRequiredExistingFilesResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ListRequiredExistingFilesResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ListRequiredExistingFilesResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn reset_job_status_response(response: ResetJobStatusResponse) -> Response<Body> {
    match response {
        ResetJobStatusResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ResetJobStatusResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ResetJobStatusResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ResetJobStatusResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn reset_workflow_status_response(response: ResetWorkflowStatusResponse) -> Response<Body> {
    match response {
        ResetWorkflowStatusResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        ResetWorkflowStatusResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        ResetWorkflowStatusResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        ResetWorkflowStatusResponse::UnprocessableContentErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::UNPROCESSABLE_ENTITY)
        }
        ResetWorkflowStatusResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn get_workflow_status_response(response: GetWorkflowStatusResponse) -> Response<Body> {
    match response {
        GetWorkflowStatusResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        GetWorkflowStatusResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        GetWorkflowStatusResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        GetWorkflowStatusResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn update_workflow_status_response(response: UpdateWorkflowStatusResponse) -> Response<Body> {
    match response {
        UpdateWorkflowStatusResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        UpdateWorkflowStatusResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        UpdateWorkflowStatusResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        UpdateWorkflowStatusResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn get_dot_graph_response(
    response: crate::server::api_responses::GetDotGraphResponse,
) -> Response<Body> {
    match response {
        crate::server::api_responses::GetDotGraphResponse::SuccessfulResponse(body) => {
            json_response_with_status(&body, StatusCode::OK)
        }
        crate::server::api_responses::GetDotGraphResponse::ForbiddenErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::FORBIDDEN)
        }
        crate::server::api_responses::GetDotGraphResponse::NotFoundErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::NOT_FOUND)
        }
        crate::server::api_responses::GetDotGraphResponse::DefaultErrorResponse(body) => {
            json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn json_response<T>(body: &T) -> Response<Body>
where
    T: serde::Serialize,
{
    json_response_with_status(body, StatusCode::OK)
}

fn json_response_with_status<T>(body: &T, status: StatusCode) -> Response<Body>
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

fn error_response(status: StatusCode, message: String) -> Response<Body> {
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

fn method_not_allowed_response() -> Response<Body> {
    Response::builder()
        .status(StatusCode::METHOD_NOT_ALLOWED)
        .body(Body::empty())
        .expect("valid method-not-allowed response")
}

fn not_found_response() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .expect("valid not-found response")
}
