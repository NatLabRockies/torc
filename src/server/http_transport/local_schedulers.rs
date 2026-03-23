use super::*;

pub(crate) async fn handle_list_local_schedulers<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_local_schedulers_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
        .list_local_schedulers(
            query.workflow_id,
            query.offset,
            query.limit,
            query.sort_by,
            query.reverse_sort,
            query.memory,
            query.num_cpus,
            &context,
        )
        .await
    {
        Ok(response) => list_local_schedulers_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_get_local_scheduler<C>(
    server: Server<C>,
    id: i64,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.get_local_scheduler(id, &context).await {
        Ok(response) => get_local_scheduler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_create_local_scheduler<C, B>(
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
    let body = match read_required_json_body::<B, models::LocalSchedulerModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.create_local_scheduler(body, &context).await {
        Ok(response) => create_local_scheduler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_update_local_scheduler<C, B>(
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
    let body = match read_required_json_body::<B, models::LocalSchedulerModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.update_local_scheduler(id, body, &context).await {
        Ok(response) => update_local_scheduler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_delete_local_schedulers<C, B>(
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
        .delete_local_schedulers(query.workflow_id, body, &context)
        .await
    {
        Ok(response) => delete_local_schedulers_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_delete_local_scheduler<C, B>(
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

    match server.delete_local_scheduler(id, body, &context).await {
        Ok(response) => delete_local_scheduler_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}
