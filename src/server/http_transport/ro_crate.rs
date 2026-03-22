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
