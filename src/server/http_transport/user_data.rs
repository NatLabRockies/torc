use super::*;

pub(crate) async fn handle_list_user_data<C, B>(
    server: Server<C>,
    request: Request<B>,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    let query = match parse_user_data_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };

    match server
        .list_user_data(
            query.workflow_id,
            query.consumer_job_id,
            query.producer_job_id,
            query.offset,
            query.limit,
            query.sort_by,
            query.reverse_sort,
            query.name,
            query.is_ephemeral,
            &context,
        )
        .await
    {
        Ok(response) => list_user_data_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_get_user_data<C>(
    server: Server<C>,
    id: i64,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.get_user_data(id, &context).await {
        Ok(response) => get_user_data_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_create_user_data<C, B>(
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
    let associations = match parse_user_data_create_query(request.uri().query()) {
        Ok(query) => query,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };
    let body = match read_required_json_body::<B, models::UserDataModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server
        .create_user_data(
            body,
            associations.consumer_job_id,
            associations.producer_job_id,
            &context,
        )
        .await
    {
        Ok(response) => create_user_data_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_update_user_data<C, B>(
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
    let body = match read_required_json_body::<B, models::UserDataModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.update_user_data(id, body, &context).await {
        Ok(response) => update_user_data_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_delete_all_user_data<C, B>(
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
        .delete_all_user_data(query.workflow_id, body, &context)
        .await
    {
        Ok(response) => delete_all_user_data_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_delete_user_data<C, B>(
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

    match server.delete_user_data(id, body, &context).await {
        Ok(response) => delete_user_data_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}
