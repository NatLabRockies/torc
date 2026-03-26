use super::*;

pub(crate) async fn handle_workflow_events_stream<C, B>(
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
    let min_severity = parse_event_stream_level(request.uri().query());

    match server.get_workflow(workflow_id, &context).await {
        Ok(GetWorkflowResponse::SuccessfulResponse(_)) => {}
        Ok(GetWorkflowResponse::ForbiddenErrorResponse(body)) => {
            return json_response_with_status(&body, StatusCode::FORBIDDEN);
        }
        Ok(GetWorkflowResponse::NotFoundErrorResponse(body)) => {
            return json_response_with_status(&body, StatusCode::NOT_FOUND);
        }
        Ok(GetWorkflowResponse::DefaultErrorResponse(body)) => {
            return json_response_with_status(&body, StatusCode::INTERNAL_SERVER_ERROR);
        }
        Err(err) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, err.0),
    }

    let mut receiver = server.subscribe_to_events();
    let stream = async_stream::stream! {
        loop {
            match receiver.recv().await {
                Ok(event)
                    if event.workflow_id == workflow_id && event.severity >= min_severity =>
                {
                    let data = serde_json::to_string(&event).unwrap_or_default();
                    yield Ok::<_, std::convert::Infallible>(
                        format!("event: {}\ndata: {}\n\n", event.event_type, data)
                    );
                }
                Ok(_) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                    yield Ok::<_, std::convert::Infallible>(
                        format!("event: warning\ndata: {{\"dropped\": {}}}\n\n", count)
                    );
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("X-Accel-Buffering", "no")
        .body(Body::from_stream(stream))
        .expect("valid SSE response")
}
