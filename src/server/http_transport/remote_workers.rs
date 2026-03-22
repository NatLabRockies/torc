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
