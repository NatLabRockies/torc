//! Dashboard redirect module.
//!
//! The full dashboard is provided by the `torc-dash` binary.
//! This module serves a simple message directing users to use `torc-dash`.

use hyper::{Body, Response, StatusCode};

const INFO_PAGE: &str = r#"<!DOCTYPE html>
<html>
<head><title>Torc Server</title></head>
<body style="font-family: sans-serif; max-width: 600px; margin: 80px auto; text-align: center;">
<h1>Torc API Server</h1>
<p>The API server is running. To access the web dashboard, start <code>torc-dash</code>:</p>
<pre style="background: #f4f4f4; padding: 12px; border-radius: 4px;">torc-dash --url http://&lt;this-server&gt;:&lt;port&gt;</pre>
<p style="color: #666; font-size: 0.9em;">See <code>torc-dash --help</code> for options.</p>
</body>
</html>"#;

/// Serve an informational page for non-API routes.
///
/// Returns `Some(Response)` for root/dashboard paths,
/// or `None` if the request should be handled by the API.
pub fn serve_dashboard(path: &str) -> Option<Response<Body>> {
    let path = path.trim_start_matches('/');

    if path.is_empty() || path == "dashboard" || path == "dashboard/" {
        Some(
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "text/html; charset=utf-8")
                .body(Body::from(INFO_PAGE))
                .expect("Failed to build response"),
        )
    } else {
        None
    }
}
