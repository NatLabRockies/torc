async fn read_required_json_body<B, T>(request: Request<B>) -> Result<T, Response<Body>>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    T: serde::de::DeserializeOwned,
{
    let bytes = match request.into_body().collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(err) => return Err(error_response(StatusCode::BAD_REQUEST, err.to_string())),
    };

    if bytes.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Request body is required".to_string(),
        ));
    }

    serde_json::from_slice::<T>(&bytes)
        .map_err(|err| error_response(StatusCode::BAD_REQUEST, err.to_string()))
}

async fn read_optional_json_value<B>(
    request: Request<B>,
) -> Result<Option<serde_json::Value>, Response<Body>>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
{
    let bytes = match request.into_body().collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(err) => return Err(error_response(StatusCode::BAD_REQUEST, err.to_string())),
    };

    if bytes.is_empty() {
        return Ok(None);
    }

    serde_json::from_slice::<serde_json::Value>(&bytes)
        .map(Some)
        .map_err(|err| error_response(StatusCode::BAD_REQUEST, err.to_string()))
}
