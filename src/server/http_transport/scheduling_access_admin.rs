async fn handle_list_scheduled_compute_nodes<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_scheduled_compute_nodes_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
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

async fn handle_create_scheduled_compute_node<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body =
        match read_required_json_body::<B, models::ScheduledComputeNodesModel>(request).await {
            Ok(body) => body,
            Err(response) => return response,
        };

    match server.create_scheduled_compute_node(body, &context).await {
        Ok(response) => create_scheduled_compute_node_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_get_scheduled_compute_node<C>(
    server: Server<C>,
    id: i64,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.get_scheduled_compute_node(id, &context).await {
        Ok(response) => get_scheduled_compute_node_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_update_scheduled_compute_node<C, B>(
    server: Server<C>,
    id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body =
        match read_required_json_body::<B, models::ScheduledComputeNodesModel>(request).await {
            Ok(body) => body,
            Err(response) => return response,
        };

    match server
        .update_scheduled_compute_node(id, body, &context)
        .await
    {
        Ok(response) => update_scheduled_compute_node_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_delete_scheduled_compute_nodes<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_delete_compute_nodes_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };
    let body = match read_optional_json_value(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server
        .delete_scheduled_compute_nodes(query.workflow_id, body, &context)
        .await
    {
        Ok(response) => delete_scheduled_compute_nodes_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_delete_scheduled_compute_node<C, B>(
    server: Server<C>,
    id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body = match read_optional_json_value(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server
        .delete_scheduled_compute_node(id, body, &context)
        .await
    {
        Ok(response) => delete_scheduled_compute_node_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_list_slurm_schedulers<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_slurm_schedulers_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
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

async fn handle_create_slurm_scheduler<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body = match read_required_json_body::<B, models::SlurmSchedulerModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.create_slurm_scheduler(body, &context).await {
        Ok(response) => create_slurm_scheduler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_get_slurm_scheduler<C>(server: Server<C>, id: i64, context: C) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.get_slurm_scheduler(id, &context).await {
        Ok(response) => get_slurm_scheduler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_update_slurm_scheduler<C, B>(
    server: Server<C>,
    id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body = match read_required_json_body::<B, models::SlurmSchedulerModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.update_slurm_scheduler(id, body, &context).await {
        Ok(response) => update_slurm_scheduler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_delete_slurm_schedulers<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_delete_compute_nodes_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };
    let body = match read_optional_json_value(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server
        .delete_slurm_schedulers(query.workflow_id, body, &context)
        .await
    {
        Ok(response) => delete_slurm_schedulers_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_delete_slurm_scheduler<C, B>(
    server: Server<C>,
    id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body = match read_optional_json_value(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.delete_slurm_scheduler(id, body, &context).await {
        Ok(response) => delete_slurm_scheduler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_list_access_groups<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_access_pagination_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
        .list_access_groups(query.offset, query.limit, &context)
        .await
    {
        Ok(response) => list_access_groups_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_create_access_group<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body = match read_required_json_body::<B, models::AccessGroupModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.create_access_group(body, &context).await {
        Ok(response) => create_access_group_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_get_access_group<C>(server: Server<C>, id: i64, context: C) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.get_access_group(id, &context).await {
        Ok(response) => get_access_group_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_delete_access_group<C>(server: Server<C>, id: i64, context: C) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.delete_access_group(id, &context).await {
        Ok(response) => delete_access_group_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_add_user_to_group<C, B>(
    server: Server<C>,
    group_id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body =
        match read_required_json_body::<B, models::UserGroupMembershipModel>(request).await {
            Ok(body) => body,
            Err(response) => return response,
        };

    match server.add_user_to_group(group_id, body, &context).await {
        Ok(response) => add_user_to_group_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_list_group_members<C, B>(
    server: Server<C>,
    group_id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_access_pagination_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
        .list_group_members(group_id, query.offset, query.limit, &context)
        .await
    {
        Ok(response) => list_group_members_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_remove_user_from_group<C>(
    server: Server<C>,
    group_id: i64,
    user_name: String,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server
        .remove_user_from_group(group_id, user_name, &context)
        .await
    {
        Ok(response) => remove_user_from_group_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_list_user_groups<C, B>(
    server: Server<C>,
    user_name: String,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_access_pagination_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
        .list_user_groups(user_name, query.offset, query.limit, &context)
        .await
    {
        Ok(response) => list_user_groups_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_add_workflow_to_group<C, B>(
    server: Server<C>,
    workflow_id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body =
        match read_required_json_body::<B, models::WorkflowAccessGroupModel>(request).await {
            Ok(body) => body,
            Err(response) => return response,
        };

    match server
        .add_workflow_to_group(workflow_id, body.group_id, &context)
        .await
    {
        Ok(response) => add_workflow_to_group_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_add_workflow_to_group_by_path<C>(
    server: Server<C>,
    workflow_id: i64,
    group_id: i64,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.add_workflow_to_group(workflow_id, group_id, &context).await {
        Ok(response) => add_workflow_to_group_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_list_workflow_groups<C, B>(
    server: Server<C>,
    workflow_id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_access_pagination_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
        .list_workflow_groups(workflow_id, query.offset, query.limit, &context)
        .await
    {
        Ok(response) => list_workflow_groups_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_remove_workflow_from_group<C>(
    server: Server<C>,
    workflow_id: i64,
    group_id: i64,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server
        .remove_workflow_from_group(workflow_id, group_id, &context)
        .await
    {
        Ok(response) => remove_workflow_from_group_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_check_workflow_access<C>(
    server: Server<C>,
    workflow_id: i64,
    user_name: String,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server
        .check_workflow_access(workflow_id, user_name, &context)
        .await
    {
        Ok(response) => check_workflow_access_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_reload_auth<C>(server: Server<C>, context: C) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.reload_auth(&context).await {
        Ok(response) => reload_auth_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_create_jobs<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body = match read_required_json_body::<B, models::JobsModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.create_jobs(body, &context).await {
        Ok(response) => create_jobs_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_create_resource_requirements<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body =
        match read_required_json_body::<B, models::ResourceRequirementsModel>(request).await {
            Ok(body) => body,
            Err(response) => return response,
        };

    match server.create_resource_requirements(body, &context).await {
        Ok(response) => create_resource_requirements_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_list_resource_requirements<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_resource_requirements_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
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

async fn handle_delete_all_resource_requirements<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_delete_compute_nodes_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };
    let body = match read_optional_json_value(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server
        .delete_all_resource_requirements(query.workflow_id, body, &context)
        .await
    {
        Ok(response) => delete_all_resource_requirements_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_get_resource_requirements<C>(
    server: Server<C>,
    id: i64,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.get_resource_requirements(id, &context).await {
        Ok(response) => get_resource_requirements_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_update_resource_requirements<C, B>(
    server: Server<C>,
    id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body =
        match read_required_json_body::<B, models::ResourceRequirementsModel>(request).await {
            Ok(body) => body,
            Err(response) => return response,
        };

    match server
        .update_resource_requirements(id, body, &context)
        .await
    {
        Ok(response) => update_resource_requirements_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_delete_resource_requirements<C, B>(
    server: Server<C>,
    id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body = match read_optional_json_value(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server
        .delete_resource_requirements(id, body, &context)
        .await
    {
        Ok(response) => delete_resource_requirements_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_create_failure_handler<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body = match read_required_json_body::<B, models::FailureHandlerModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.create_failure_handler(body, &context).await {
        Ok(response) => create_failure_handler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_get_failure_handler<C>(server: Server<C>, id: i64, context: C) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.get_failure_handler(id, &context).await {
        Ok(response) => get_failure_handler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_delete_failure_handler<C, B>(
    server: Server<C>,
    id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body = match read_optional_json_value(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.delete_failure_handler(id, body, &context).await {
        Ok(response) => delete_failure_handler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_list_failure_handlers<C, B>(
    server: Server<C>,
    workflow_id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_access_pagination_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
        .list_failure_handlers(workflow_id, query.offset, query.limit, &context)
        .await
    {
        Ok(response) => list_failure_handlers_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_create_slurm_stats<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body = match read_required_json_body::<B, models::SlurmStatsModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.create_slurm_stats(body, &context).await {
        Ok(response) => create_slurm_stats_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_list_slurm_stats<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_slurm_stats_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
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

async fn handle_create_ro_crate_entity<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body = match read_required_json_body::<B, models::RoCrateEntityModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.create_ro_crate_entity(body, &context).await {
        Ok(response) => create_ro_crate_entity_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_get_ro_crate_entity<C>(server: Server<C>, id: i64, context: C) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.get_ro_crate_entity(id, &context).await {
        Ok(response) => get_ro_crate_entity_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_update_ro_crate_entity<C, B>(
    server: Server<C>,
    id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body = match read_required_json_body::<B, models::RoCrateEntityModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.update_ro_crate_entity(id, body, &context).await {
        Ok(response) => update_ro_crate_entity_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_delete_ro_crate_entity<C, B>(
    server: Server<C>,
    id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body = match read_optional_json_value(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.delete_ro_crate_entity(id, body, &context).await {
        Ok(response) => delete_ro_crate_entity_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_list_ro_crate_entities<C, B>(
    server: Server<C>,
    workflow_id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_access_pagination_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
        .list_ro_crate_entities(workflow_id, query.offset, query.limit, &context)
        .await
    {
        Ok(response) => list_ro_crate_entities_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_delete_ro_crate_entities<C, B>(
    server: Server<C>,
    workflow_id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body = match read_optional_json_value(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server
        .delete_ro_crate_entities(workflow_id, body, &context)
        .await
    {
        Ok(response) => delete_ro_crate_entities_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_list_remote_workers<C>(
    server: Server<C>,
    workflow_id: i64,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.list_remote_workers(workflow_id, &context).await {
        Ok(response) => list_remote_workers_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_create_remote_workers<C, B>(
    server: Server<C>,
    workflow_id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let workers = match read_required_json_body::<B, Vec<String>>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server
        .create_remote_workers(workflow_id, workers, &context)
        .await
    {
        Ok(response) => create_remote_workers_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_delete_remote_worker<C>(
    server: Server<C>,
    workflow_id: i64,
    worker: String,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server
        .delete_remote_worker(workflow_id, worker, &context)
        .await
    {
        Ok(response) => delete_remote_worker_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}
