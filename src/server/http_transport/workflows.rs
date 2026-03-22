async fn handle_list_workflows<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_workflows_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
        .list_workflows(
            query.offset,
            query.sort_by,
            query.reverse_sort,
            query.limit,
            query.name,
            query.user,
            query.description,
            query.is_archived,
            &context,
        )
        .await
    {
        Ok(response) => list_workflows_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_create_workflow<C, B>(
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
    let body = match read_required_json_body::<B, models::WorkflowModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.create_workflow(body, &context).await {
        Ok(response) => create_workflow_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_get_workflow<C>(server: Server<C>, id: i64, context: C) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.get_workflow(id, &context).await {
        Ok(response) => get_workflow_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_update_workflow<C, B>(
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
    let body = match read_required_json_body::<B, models::WorkflowModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.update_workflow(id, body, &context).await {
        Ok(response) => update_workflow_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_delete_workflow<C, B>(
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

    match server.delete_workflow(id, body, &context).await {
        Ok(response) => delete_workflow_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_get_workflow_actions<C>(
    server: Server<C>,
    workflow_id: i64,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.get_workflow_actions(workflow_id, &context).await {
        Ok(response) => get_workflow_actions_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_create_workflow_action<C, B>(
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
    let body = match read_required_json_body::<B, serde_json::Value>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server
        .create_workflow_action(workflow_id, body, &context)
        .await
    {
        Ok(response) => create_workflow_action_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_get_pending_actions<C, B>(
    server: Server<C>,
    workflow_id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_pending_actions_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
        .get_pending_actions(workflow_id, query.trigger_type, &context)
        .await
    {
        Ok(response) => get_pending_actions_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_claim_action<C, B>(
    server: Server<C>,
    workflow_id: i64,
    action_id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let body = match read_required_json_body::<B, serde_json::Value>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server
        .claim_action(workflow_id, action_id, body, &context)
        .await
    {
        Ok(response) => claim_action_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_cancel_workflow<C, B>(
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

    match server.cancel_workflow(id, body, &context).await {
        Ok(response) => cancel_workflow_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_claim_jobs_based_on_resources<C, B>(
    server: Server<C>,
    id: i64,
    limit: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_claim_jobs_based_on_resources_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };
    let body = match read_required_json_body::<B, models::ComputeNodesResources>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server
        .claim_jobs_based_on_resources(
            id,
            body,
            limit,
            query.sort_method,
            query.strict_scheduler_match,
            &context,
        )
        .await
    {
        Ok(response) => claim_jobs_based_on_resources_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_claim_next_jobs<C, B>(
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
    let query = match parse_claim_next_jobs_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };
    let body = match read_optional_json_value(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server
        .claim_next_jobs(id, query.limit, body, &context)
        .await
    {
        Ok(response) => claim_next_jobs_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_initialize_jobs<C, B>(
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
    let query = match parse_initialize_jobs_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };
    let body = match read_optional_json_value(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server
        .initialize_jobs(
            id,
            query.only_uninitialized,
            query.clear_ephemeral_user_data,
            body,
            &context,
        )
        .await
    {
        Ok(response) => initialize_jobs_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_is_workflow_complete<C>(server: Server<C>, id: i64, context: C) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.is_workflow_complete(id, &context).await {
        Ok(response) => is_workflow_complete_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_is_workflow_uninitialized<C>(
    server: Server<C>,
    id: i64,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.is_workflow_uninitialized(id, &context).await {
        Ok(response) => is_workflow_uninitialized_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_list_job_dependencies<C, B>(
    server: Server<C>,
    id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_workflow_relationships_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
        .list_job_dependencies(id, query.offset, query.limit, &context)
        .await
    {
        Ok(response) => list_job_dependencies_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_list_job_file_relationships<C, B>(
    server: Server<C>,
    id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_workflow_relationships_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
        .list_job_file_relationships(id, query.offset, query.limit, &context)
        .await
    {
        Ok(response) => list_job_file_relationships_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_list_job_ids<C>(server: Server<C>, id: i64, context: C) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.list_job_ids(id, &context).await {
        Ok(response) => list_job_ids_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_list_job_user_data_relationships<C, B>(
    server: Server<C>,
    id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_workflow_relationships_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
        .list_job_user_data_relationships(id, query.offset, query.limit, &context)
        .await
    {
        Ok(response) => list_job_user_data_relationships_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_list_missing_user_data<C>(server: Server<C>, id: i64, context: C) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.list_missing_user_data(id, &context).await {
        Ok(response) => list_missing_user_data_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_process_changed_job_inputs<C, B>(
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
    let query = match parse_process_changed_job_inputs_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };
    let body = match read_optional_json_value(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server
        .process_changed_job_inputs(id, query.dry_run, body, &context)
        .await
    {
        Ok(response) => process_changed_job_inputs_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_get_ready_job_requirements<C, B>(
    server: Server<C>,
    id: i64,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_get_ready_job_requirements_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
        .get_ready_job_requirements(id, query.scheduler_config_id, &context)
        .await
    {
        Ok(response) => get_ready_job_requirements_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_list_required_existing_files<C>(
    server: Server<C>,
    id: i64,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.list_required_existing_files(id, &context).await {
        Ok(response) => list_required_existing_files_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_reset_job_status<C, B>(
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
    let query = match parse_reset_job_status_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };
    let body = match read_optional_json_value(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server
        .reset_job_status(id, query.failed_only, body, &context)
        .await
    {
        Ok(response) => reset_job_status_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_reset_workflow_status<C, B>(
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
    let query = match parse_reset_workflow_status_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };
    let body = match read_optional_json_value(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server
        .reset_workflow_status(id, query.force, body, &context)
        .await
    {
        Ok(response) => reset_workflow_status_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_get_workflow_status<C>(server: Server<C>, id: i64, context: C) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.get_workflow_status(id, &context).await {
        Ok(response) => get_workflow_status_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_update_workflow_status<C, B>(
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
    let body = match read_required_json_body::<B, models::WorkflowStatusModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.update_workflow_status(id, body, &context).await {
        Ok(response) => update_workflow_status_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

async fn handle_workflow_events_stream<C, B>(
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
    let min_severity = parse_event_stream_level(request.uri().query());

    match server.get_workflow(workflow_id, &context).await {
        Ok(GetWorkflowResponse::SuccessfulResponse(_)) => {}
        Ok(GetWorkflowResponse::ForbiddenErrorResponse(body)) => {
            return json_response_with_status(&body, StatusCode::FORBIDDEN);
        }
        Ok(GetWorkflowResponse::NotFoundErrorResponse(body)) => {
            return json_response_with_status(&body, StatusCode::NOT_FOUND);
        }
        Ok(GetWorkflowResponse::DefaultErrorResponse(body)) => {
            return json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR);
        }
        Err(err) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }

    let mut receiver = server.subscribe_to_events();
    let stream = async_stream::stream! {
        loop {
            match receiver.recv().await {
                Ok(event)
                    if event.workflow_id == workflow_id && event.severity >= min_severity =>
                {
                    let data = serde_json::to_string(&event).unwrap_or_default();
                    yield Ok::<_, std::convert::Infallible>(
                        format!("event: {}\ndata: {}\n\n", event.event_type, data)
                    );
                }
                Ok(_) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                    yield Ok::<_, std::convert::Infallible>(
                        format!("event: warning\ndata: {{\"dropped\": {}}}\n\n", count)
                    );
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("X-Accel-Buffering", "no")
        .body(Body::from_stream(stream))
        .expect("valid SSE response")
}

async fn handle_get_dot_graph<C>(
    server: Server<C>,
    id: i64,
    name: String,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.get_dot_graph(id, name, &context).await {
        Ok(response) => get_dot_graph_response(response),
        Err(err) => error_response(StatusCode::NOT_IMPLEMENTED, err.0),
    }
}
