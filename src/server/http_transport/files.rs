use super::*;

pub(crate) async fn handle_list_files<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_files_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
        .list_files(
            query.workflow_id,
            query.produced_by_job_id,
            query.offset,
            query.limit,
            query.sort_by,
            query.reverse_sort,
            query.name,
            query.path,
            query.is_output,
            &context,
        )
        .await
    {
        Ok(response) => list_files_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_get_file<C>(server: Server<C>, id: i64, context: C) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.get_file(id, &context).await {
        Ok(response) => get_file_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_create_file<C, B>(
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
    let body = match read_required_json_body::<B, models::FileModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.create_file(body, &context).await {
        Ok(response) => create_file_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_update_file<C, B>(
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
    let body = match read_required_json_body::<B, models::FileModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.update_file(id, body, &context).await {
        Ok(response) => update_file_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_delete_files<C, B>(
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

    match server.delete_files(query.workflow_id, body, &context).await {
        Ok(response) => delete_files_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_delete_file<C, B>(
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

    match server.delete_file(id, body, &context).await {
        Ok(response) => delete_file_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}
