use super::*;

pub(super) async fn handle_list_access_groups<C, B>(
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

pub(super) async fn handle_create_access_group<C, B>(
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

pub(super) async fn handle_get_access_group<C>(
    server: Server<C>,
    id: i64,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.get_access_group(id, &context).await {
        Ok(response) => get_access_group_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(super) async fn handle_delete_access_group<C>(
    server: Server<C>,
    id: i64,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.delete_access_group(id, &context).await {
        Ok(response) => delete_access_group_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(super) async fn handle_add_user_to_group<C, B>(
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
    let body = match read_required_json_body::<B, models::UserGroupMembershipModel>(request).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    match server.add_user_to_group(group_id, body, &context).await {
        Ok(response) => add_user_to_group_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(super) async fn handle_list_group_members<C, B>(
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

pub(super) async fn handle_remove_user_from_group<C>(
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

pub(super) async fn handle_list_user_groups<C, B>(
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

pub(super) async fn handle_add_workflow_to_group<C, B>(
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
    let body = match read_required_json_body::<B, models::WorkflowAccessGroupModel>(request).await {
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

pub(super) async fn handle_add_workflow_to_group_by_path<C>(
    server: Server<C>,
    workflow_id: i64,
    group_id: i64,
    context: C,
) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server
        .add_workflow_to_group(workflow_id, group_id, &context)
        .await
    {
        Ok(response) => add_workflow_to_group_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}

pub(super) async fn handle_list_workflow_groups<C, B>(
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

pub(super) async fn handle_remove_workflow_from_group<C>(
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

pub(super) async fn handle_check_workflow_access<C>(
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
