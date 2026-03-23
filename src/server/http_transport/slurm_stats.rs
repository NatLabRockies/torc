use super::*;

pub(crate) async fn handle_create_slurm_stats<C, B>(
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

pub(crate) async fn handle_list_slurm_stats<C, B>(
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
