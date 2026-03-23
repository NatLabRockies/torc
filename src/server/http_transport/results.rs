use super::*;

pub(crate) async fn handle_list_results<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_results_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
        .list_results(
            query.workflow_id,
            query.job_id,
            query.run_id,
            query.return_code,
            query.status,
            query.compute_node_id,
            query.offset,
            query.limit,
            query.sort_by,
            query.reverse_sort,
            query.all_runs,
            &context,
        )
        .await
    {
        Ok(response) => list_results_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_get_result<C>(server: Server<C>, id: i64, context: C) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.get_result(id, &context).await {
        Ok(response) => get_result_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_create_result<C, B>(
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
    let body = match read_required_json_body::<B, models::ResultModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.create_result(body, &context).await {
        Ok(response) => create_result_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_update_result<C, B>(
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
    let body = match read_required_json_body::<B, models::ResultModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.update_result(id, body, &context).await {
        Ok(response) => update_result_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_delete_results<C, B>(
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
        .delete_results(query.workflow_id, body, &context)
        .await
    {
        Ok(response) => delete_results_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_delete_result<C, B>(
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

    match server.delete_result(id, body, &context).await {
        Ok(response) => delete_result_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}
