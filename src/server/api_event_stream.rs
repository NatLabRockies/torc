//! Broadcast channel and event types for the admin API request inspector.
//!
//! Each HTTP request that flows through the live router emits an
//! [`ApiRequestEvent`] to subscribers of the admin SSE stream. When no
//! subscribers are connected, sends are essentially a no-op so the runtime
//! cost is limited to a clock read and a `Sender::send` on a channel with
//! zero receivers.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::broadcast;

/// Default cap on captured request/response body bytes per direction
/// that are forwarded to subscribers.
pub const DEFAULT_BODY_CAPTURE_LIMIT: usize = 8 * 1024;

/// Environment variable that overrides [`DEFAULT_BODY_CAPTURE_LIMIT`].
pub const BODY_CAPTURE_LIMIT_ENV: &str = "TORC_API_EVENT_BODY_MAX_BYTES";

/// Hard ceiling on bytes the middleware will buffer in memory in order
/// to capture a body. Requests/responses whose advertised length
/// exceeds this are passed through untouched (no body capture).
pub const BODY_CAPTURE_HARD_CAP_BYTES: usize = 1024 * 1024;

/// Resolve the per-direction body display limit at runtime.
pub fn body_capture_limit() -> usize {
    std::env::var(BODY_CAPTURE_LIMIT_ENV)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_BODY_CAPTURE_LIMIT)
}

/// A single captured HTTP request/response, broadcast to admin SSE clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRequestEvent {
    /// Unix epoch milliseconds at which the request finished.
    pub timestamp_ms: i64,
    /// HTTP method (e.g. `GET`, `POST`).
    pub method: String,
    /// URL path component (without query string).
    pub path: String,
    /// URL query string, if any (without the leading `?`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Final HTTP status code returned to the client.
    pub status: u16,
    /// Wall-clock duration spent inside the router, in milliseconds.
    pub latency_ms: u64,
    /// `x-span-id` assigned by `inject_request_context`, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Authenticated subject extracted from the request, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Captured request body when body capture was enabled and the
    /// payload was textual.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_body: Option<CapturedBody>,
    /// Captured response body when body capture was enabled and the
    /// payload was textual.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_body: Option<CapturedBody>,
}

/// Captured payload, possibly truncated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedBody {
    /// Total observed length in bytes (before truncation).
    pub bytes: usize,
    /// Whether `text` was truncated to fit the capture limit.
    pub truncated: bool,
    /// UTF-8 view of the (possibly truncated) body. `None` when the
    /// body was not valid UTF-8 — binary payloads are reported as
    /// metadata only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

impl CapturedBody {
    /// Build a [`CapturedBody`] from raw bytes, truncating to `limit`.
    pub fn from_bytes(bytes: &[u8], limit: usize) -> Self {
        let total = bytes.len();
        let truncated = total > limit;
        let slice = if truncated { &bytes[..limit] } else { bytes };
        let text = std::str::from_utf8(slice).ok().map(|s| s.to_string());
        Self {
            bytes: total,
            truncated,
            text,
        }
    }
}

/// Broadcast channel for [`ApiRequestEvent`]s.
#[derive(Clone)]
pub struct ApiEventBroadcaster {
    sender: Arc<broadcast::Sender<ApiRequestEvent>>,
    body_subscribers: Arc<AtomicUsize>,
}

impl ApiEventBroadcaster {
    /// Create a broadcaster with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender: Arc::new(sender),
            body_subscribers: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Returns the number of currently connected receivers.
    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Returns the number of receivers that asked for body capture.
    pub fn body_subscriber_count(&self) -> usize {
        self.body_subscribers.load(Ordering::Relaxed)
    }

    /// Broadcast an event. Returns `true` if at least one receiver was
    /// notified. Drops silently when there are no subscribers.
    pub fn broadcast(&self, event: ApiRequestEvent) -> bool {
        self.sender.send(event).is_ok()
    }

    /// Subscribe to the channel.
    pub fn subscribe(&self) -> broadcast::Receiver<ApiRequestEvent> {
        self.sender.subscribe()
    }

    /// Register interest in body capture; the returned guard decrements
    /// the body-subscriber count when dropped.
    pub fn body_subscriber_guard(&self) -> BodySubscriberGuard {
        self.body_subscribers.fetch_add(1, Ordering::Relaxed);
        BodySubscriberGuard {
            counter: self.body_subscribers.clone(),
        }
    }
}

impl Default for ApiEventBroadcaster {
    fn default() -> Self {
        Self::new(256)
    }
}

/// RAII guard that keeps the broadcaster's body-subscriber count
/// elevated for as long as it is held.
pub struct BodySubscriberGuard {
    counter: Arc<AtomicUsize>,
}

impl Drop for BodySubscriberGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_event() -> ApiRequestEvent {
        ApiRequestEvent {
            timestamp_ms: 1_700_000_000_000,
            method: "GET".into(),
            path: "/torc-service/v1/ping".into(),
            query: None,
            status: 200,
            latency_ms: 4,
            request_id: Some("span-1".into()),
            user: Some("alice".into()),
            request_body: None,
            response_body: None,
        }
    }

    #[tokio::test]
    async fn broadcast_delivers_to_subscribers() {
        let bus = ApiEventBroadcaster::new(8);
        let mut rx = bus.subscribe();
        assert_eq!(bus.receiver_count(), 1);

        assert!(bus.broadcast(sample_event()));
        let received = rx.recv().await.expect("event");
        assert_eq!(received.path, "/torc-service/v1/ping");
        assert_eq!(received.status, 200);
    }

    #[tokio::test]
    async fn broadcast_with_no_subscribers_is_noop() {
        let bus = ApiEventBroadcaster::new(8);
        assert_eq!(bus.receiver_count(), 0);
        assert!(!bus.broadcast(sample_event()));
    }

    #[test]
    fn body_subscriber_guard_tracks_count() {
        let bus = ApiEventBroadcaster::new(8);
        assert_eq!(bus.body_subscriber_count(), 0);

        let guard1 = bus.body_subscriber_guard();
        let guard2 = bus.body_subscriber_guard();
        assert_eq!(bus.body_subscriber_count(), 2);

        drop(guard1);
        assert_eq!(bus.body_subscriber_count(), 1);
        drop(guard2);
        assert_eq!(bus.body_subscriber_count(), 0);
    }

    #[test]
    fn captured_body_truncates() {
        let body = b"hello world".as_slice();
        let captured = CapturedBody::from_bytes(body, 5);
        assert_eq!(captured.bytes, 11);
        assert!(captured.truncated);
        assert_eq!(captured.text.as_deref(), Some("hello"));
    }

    #[test]
    fn captured_body_full() {
        let body = b"abc".as_slice();
        let captured = CapturedBody::from_bytes(body, 16);
        assert_eq!(captured.bytes, 3);
        assert!(!captured.truncated);
        assert_eq!(captured.text.as_deref(), Some("abc"));
    }

    #[test]
    fn captured_body_binary_drops_text() {
        let body = &[0xff, 0xfe, 0xfd][..];
        let captured = CapturedBody::from_bytes(body, 16);
        assert_eq!(captured.bytes, 3);
        assert!(!captured.truncated);
        assert!(captured.text.is_none());
    }
}
