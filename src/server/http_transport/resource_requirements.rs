use super::*;

pub(crate) async fn handle_create_resource_requirements<C, B>(
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
    let body = match read_required_json_body::<B, models::ResourceRequirementsModel>(request).await
    {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.create_resource_requirements(body, &context).await {
        Ok(response) => create_resource_requirements_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(crate) async fn handle_list_resource_requirements<C, B>(
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

pub(crate) async fn handle_delete_all_resource_requirements<C, B>(
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

pub(crate) async fn handle_get_resource_requirements<C>(
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

pub(crate) async fn handle_update_resource_requirements<C, B>(
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
    let body = match read_required_json_body::<B, models::ResourceRequirementsModel>(request).await
    {
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

pub(crate) async fn handle_delete_resource_requirements<C, B>(
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
