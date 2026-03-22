async fn handle_reload_auth<C>(server: Server<C>, context: C) -> Response<Body>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    match server.reload_auth(&context).await {
        Ok(response) => reload_auth_response(response),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }
}
