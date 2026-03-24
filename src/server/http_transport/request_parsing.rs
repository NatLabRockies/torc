#![allow(dead_code)]

use super::*;
use axum::http::header::CONTENT_LENGTH;
use futures::future::poll_fn;
use hyper::body::{Buf, Bytes};
use std::sync::OnceLock;

/// Default maximum allowed request body size (200 MiB).
/// Override at runtime with TORC_MAX_REQUEST_BODY_MB (value in MiB).
const DEFAULT_MAX_REQUEST_BODY_BYTES: u64 = 200 * 1024 * 1024;

fn max_request_body_bytes() -> u64 {
    static CACHED: OnceLock<u64> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("TORC_MAX_REQUEST_BODY_MB")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .and_then(|mb| mb.checked_mul(1024 * 1024))
            .unwrap_or(DEFAULT_MAX_REQUEST_BODY_BYTES)
    })
}

async fn read_body_bytes_limited<B>(request: Request<B>) -> Result<Bytes, Response<Body>>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
{
    let (parts, body) = request.into_parts();
    if let Some(cl) = parts.headers.get(CONTENT_LENGTH) {
        match cl.to_str().ok().and_then(|s| s.parse::<u64>().ok()) {
            Some(len) if len > max_request_body_bytes() => {
                return Err(error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "Request body too large".to_string(),
                ));
            }
            None => {
                return Err(error_response(
                    StatusCode::BAD_REQUEST,
                    "Invalid Content-Length header".to_string(),
                ));
            }
            _ => {}
        }
    }

    let limit = max_request_body_bytes().min(usize::MAX as u64) as usize;
    let mut buffer = Vec::new();
    let mut body = std::pin::pin!(body);

    while let Some(frame) = poll_fn(|cx| body.as_mut().poll_frame(cx)).await {
        let frame = match frame {
            Ok(frame) => frame,
            Err(err) => return Err(error_response(StatusCode::BAD_REQUEST, err.to_string())),
        };

        if let Ok(data) = frame.into_data() {
            let mut data = data;
            let remaining = data.remaining();
            if buffer.len().saturating_add(remaining) > limit {
                return Err(error_response(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "Request body too large".to_string(),
                ));
            }
            buffer.extend_from_slice(data.copy_to_bytes(remaining).as_ref());
        }
    }

    Ok(buffer.into())
}

pub(super) async fn read_required_json_body<B, T>(request: Request<B>) -> Result<T, Response<Body>>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
    T: serde::de::DeserializeOwned,
{
    let bytes = read_body_bytes_limited(request).await?;

    if bytes.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Request body is required".to_string(),
        ));
    }

    serde_json::from_slice::<T>(&bytes)
        .map_err(|err| error_response(StatusCode::BAD_REQUEST, err.to_string()))
}

pub(super) async fn read_optional_json_value<B>(
    request: Request<B>,
) -> Result<Option<serde_json::Value>, Response<Body>>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: std::fmt::Display,
{
    let bytes = read_body_bytes_limited(request).await?;

    if bytes.is_empty() {
        return Ok(None);
    }

    Ok(serde_json::from_slice::<serde_json::Value>(&bytes).ok())
}

#[cfg(test)]
mod request_parsing_tests {
    use super::*;
    use axum::body::Body;

    #[tokio::test]
    async fn required_body_rejects_invalid_content_length() {
        let request = Request::builder()
            .header(CONTENT_LENGTH, "not-a-number")
            .body(Body::from("{}"))
            .expect("request");

        let response = read_required_json_body::<_, serde_json::Value>(request)
            .await
            .expect_err("invalid content length should fail");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn required_body_rejects_oversized_content_length() {
        let request = Request::builder()
            .header(
                CONTENT_LENGTH,
                (DEFAULT_MAX_REQUEST_BODY_BYTES + 1).to_string(),
            )
            .body(Body::from("{}"))
            .expect("request");

        let response = read_required_json_body::<_, serde_json::Value>(request)
            .await
            .expect_err("oversized body should fail");
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn optional_body_treats_malformed_json_as_absent() {
        let request = Request::builder().body(Body::from("{")).expect("request");

        let parsed = read_optional_json_value(request)
            .await
            .expect("malformed optional body should not error");
        assert_eq!(parsed, None);
    }
}
