//! Lightweight per-second ring buffer of HTTP request stats.
//!
//! Tracks request count, request/response bytes, and a 2xx/4xx/5xx
//! breakdown for the last hour. Used by the `GET /admin/api-stats`
//! endpoint and the `torc admin api-stats` CLI to answer "how busy is
//! the server right now?" without streaming individual events.
//!
//! Byte counts are tallied by [`CountingBody`], an `http_body::Body`
//! adapter that records stats as data frames flow through. A response
//! body fires a "request started" record on its first poll (so the
//! request count + status appear immediately) and then attributes each
//! data frame's bytes to the wall-clock second it is sent via
//! [`ApiStatsRing::record_bytes_out`]. This captures chunked /
//! streaming payloads — including the long-lived SSE event streams —
//! without deferring all the bytes to the disconnect second, and
//! without the 0-byte counts `Content-Length` headers would report.
//!
//! Stats are in-memory only; the buffer is cleared on server restart.

use bytes::Buf;
use http_body::{Body as HttpBody, Frame, SizeHint};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};

/// Number of 1-second buckets retained. One hour of history.
const BUCKET_COUNT: usize = 3600;

/// Default window the API endpoint reports when none is requested.
pub(crate) const DEFAULT_WINDOW_SECONDS: u64 = 3600;

/// Default aggregation interval for the API endpoint.
pub(crate) const DEFAULT_INTERVAL_SECONDS: u64 = 60;

/// One second's worth of accumulated request stats.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApiStatsBucket {
    /// Bucket start, in Unix epoch milliseconds.
    start_ms: i64,
    /// Total requests handled during this bucket.
    pub(crate) request_count: u64,
    /// Sum of data frame bytes received in inbound requests.
    bytes_in: u64,
    /// Sum of data frame bytes written in outbound responses.
    pub(crate) bytes_out: u64,
    /// Requests that returned a 2xx status.
    status_2xx: u64,
    /// Requests that returned a 4xx status.
    pub(crate) status_4xx: u64,
    /// Requests that returned a 5xx status.
    status_5xx: u64,
    /// Requests that returned anything else (1xx, 3xx).
    status_other: u64,
}

impl ApiStatsBucket {
    fn merge(&mut self, other: &ApiStatsBucket) {
        self.request_count += other.request_count;
        self.bytes_in += other.bytes_in;
        self.bytes_out += other.bytes_out;
        self.status_2xx += other.status_2xx;
        self.status_4xx += other.status_4xx;
        self.status_5xx += other.status_5xx;
        self.status_other += other.status_other;
    }
}

/// Snapshot returned by [`ApiStatsRing::snapshot`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiStatsSnapshot {
    /// Server-side current time in Unix epoch milliseconds.
    now_ms: i64,
    /// Width of each bucket in seconds.
    interval_seconds: u64,
    /// Total span covered by `buckets`, in seconds.
    window_seconds: u64,
    /// Newest first: `buckets[0]` is the most recent interval.
    pub(crate) buckets: Vec<ApiStatsBucket>,
}

/// Mutex-protected ring of per-second counters.
#[derive(Clone)]
pub struct ApiStatsRing {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    /// One bucket per second, indexed by `(unix_seconds % BUCKET_COUNT)`.
    /// `start_ms = 0` means "never written".
    buckets: Box<[ApiStatsBucket]>,
}

impl Inner {
    /// Return the bucket that owns `now_ms`, resetting it first if the
    /// ring slot still holds data from an earlier second.
    fn bucket_mut(&mut self, now_ms: i64) -> &mut ApiStatsBucket {
        let bucket_start_ms = (now_ms / 1000) * 1000;
        let idx = ((now_ms / 1000) as i128).rem_euclid(BUCKET_COUNT as i128) as usize;
        let bucket = &mut self.buckets[idx];
        if bucket.start_ms != bucket_start_ms {
            *bucket = ApiStatsBucket {
                start_ms: bucket_start_ms,
                ..ApiStatsBucket::default()
            };
        }
        bucket
    }
}

impl ApiStatsRing {
    pub(crate) fn new() -> Self {
        let buckets = (0..BUCKET_COUNT)
            .map(|_| ApiStatsBucket::default())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            inner: Arc::new(Mutex::new(Inner { buckets })),
        }
    }

    /// Record a single request against the bucket for `now_ms`,
    /// incrementing the request count and status breakdown.
    ///
    /// For streaming responses this is fired once when the response
    /// body starts (with `bytes_out = 0`); the streamed bytes are
    /// attributed second-by-second via [`Self::record_bytes_out`].
    pub(crate) fn record(&self, now_ms: i64, status: u16, bytes_in: u64, bytes_out: u64) {
        if now_ms <= 0 {
            return;
        }
        let mut guard = self.inner.lock();
        let bucket = guard.bucket_mut(now_ms);
        bucket.request_count += 1;
        bucket.bytes_in += bytes_in;
        bucket.bytes_out += bytes_out;
        match status / 100 {
            2 => bucket.status_2xx += 1,
            4 => bucket.status_4xx += 1,
            5 => bucket.status_5xx += 1,
            _ => bucket.status_other += 1,
        }
    }

    /// Attribute `bytes_out` to the bucket for `now_ms` without
    /// touching the request count or status breakdown. Used to bucket
    /// each streamed response frame by the second it is actually sent,
    /// rather than deferring the whole stream to the disconnect second.
    pub(crate) fn record_bytes_out(&self, now_ms: i64, bytes_out: u64) {
        if now_ms <= 0 || bytes_out == 0 {
            return;
        }
        let mut guard = self.inner.lock();
        guard.bucket_mut(now_ms).bytes_out += bytes_out;
    }

    /// Aggregate the last `window_seconds` of recorded data into
    /// `interval_seconds`-wide buckets, newest first.
    pub(crate) fn snapshot(
        &self,
        now_ms: i64,
        window_seconds: u64,
        interval_seconds: u64,
    ) -> ApiStatsSnapshot {
        let interval_seconds = interval_seconds.max(1);
        let window_seconds = window_seconds
            .max(interval_seconds)
            .min(BUCKET_COUNT as u64);
        let bucket_count = window_seconds.div_ceil(interval_seconds);

        let now_secs = now_ms / 1000;
        // Anchor each output bucket to the floor of `now` aligned to
        // `interval_seconds` so successive snapshots report consistent
        // boundaries.
        let latest_start_secs = (now_secs / interval_seconds as i64) * interval_seconds as i64;

        let mut output = Vec::with_capacity(bucket_count as usize);
        let guard = self.inner.lock();
        for i in 0..bucket_count {
            let start_secs = latest_start_secs - (i as i64) * (interval_seconds as i64);
            let mut agg = ApiStatsBucket {
                start_ms: start_secs * 1000,
                ..ApiStatsBucket::default()
            };
            for offset in 0..interval_seconds as i64 {
                let sec = start_secs + offset;
                let idx = (sec as i128).rem_euclid(BUCKET_COUNT as i128) as usize;
                let stored = &guard.buckets[idx];
                if stored.start_ms / 1000 == sec {
                    agg.merge(stored);
                }
            }
            output.push(agg);
        }
        ApiStatsSnapshot {
            now_ms,
            interval_seconds,
            window_seconds: bucket_count * interval_seconds,
            buckets: output,
        }
    }
}

impl Default for ApiStatsRing {
    fn default() -> Self {
        Self::new()
    }
}

/// `http_body::Body` adapter that counts the bytes of each data frame
/// as it passes through. Wrapping a request body lets us tally actual
/// inbound bytes (chunked uploads included). Wrapping a response body
/// with [`CountingBody::with_recorder`] instead records stats
/// incrementally: `on_start` fires once when the body is first polled
/// (or, failing that, when it is dropped) so the request count and
/// status land in the current bucket immediately, and `on_frame` fires
/// for each data frame so its bytes are attributed to the second it is
/// actually sent — keeping long-lived SSE streams from spiking the
/// whole transfer into the disconnect second.
pub struct CountingBody<B> {
    inner: B,
    /// Set for request bodies: the middleware reads the running total
    /// after the handler returns. Unused for response bodies, which
    /// report each frame through `on_frame` instead.
    counter: Option<Arc<AtomicU64>>,
    on_start: Option<Box<dyn FnOnce() + Send>>,
    on_frame: Option<Box<dyn Fn(u64) + Send>>,
}

impl<B> CountingBody<B> {
    /// Wrap `inner`, accumulating the size of each data frame into
    /// `counter`. Use this for request bodies, where the consuming
    /// middleware reads the final count directly from `counter` after
    /// the handler returns.
    pub(crate) fn new(inner: B, counter: Arc<AtomicU64>) -> Self {
        Self {
            inner,
            counter: Some(counter),
            on_start: None,
            on_frame: None,
        }
    }

    /// Wrap a response `inner`, firing `on_start` exactly once when the
    /// body is first polled (or dropped without being polled) and
    /// `on_frame(len)` for each data frame as it is sent. This records
    /// the request as soon as it starts and attributes streamed bytes
    /// to the second they leave the server, rather than deferring the
    /// whole stream to completion.
    pub(crate) fn with_recorder<S, F>(inner: B, on_start: S, on_frame: F) -> Self
    where
        S: FnOnce() + Send + 'static,
        F: Fn(u64) + Send + 'static,
    {
        Self {
            inner,
            counter: None,
            on_start: Some(Box::new(on_start)),
            on_frame: Some(Box::new(on_frame)),
        }
    }

    fn fire_on_start(&mut self) {
        if let Some(cb) = self.on_start.take() {
            cb();
        }
    }
}

impl<B> HttpBody for CountingBody<B>
where
    B: HttpBody + Unpin,
{
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.as_mut().get_mut();
        // Fire the "request started" record on the first poll, before
        // we know whether any data frames will follow, so empty bodies
        // are still counted.
        this.fire_on_start();
        let polled = Pin::new(&mut this.inner).poll_frame(cx);
        if let Poll::Ready(Some(Ok(frame))) = &polled
            && let Some(data) = frame.data_ref()
        {
            let len = data.remaining() as u64;
            if let Some(counter) = &this.counter {
                counter.fetch_add(len, Ordering::Relaxed);
            }
            if let Some(on_frame) = &this.on_frame {
                on_frame(len);
            }
        }
        polled
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

impl<B> Drop for CountingBody<B> {
    fn drop(&mut self) {
        // Ensure the request is still recorded if the response body was
        // dropped before it was ever polled (e.g. HEAD / 204 responses).
        self.fire_on_start();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_snapshot_single_bucket() {
        let ring = ApiStatsRing::new();
        let t = 1_700_000_000_000;
        ring.record(t, 200, 100, 200);
        ring.record(t + 100, 200, 50, 75);
        ring.record(t + 200, 404, 0, 30);
        ring.record(t + 300, 500, 0, 10);

        let snap = ring.snapshot(t + 500, 60, 60);
        assert_eq!(snap.buckets.len(), 1);
        let bucket = &snap.buckets[0];
        assert_eq!(bucket.request_count, 4);
        assert_eq!(bucket.bytes_in, 150);
        assert_eq!(bucket.bytes_out, 315);
        assert_eq!(bucket.status_2xx, 2);
        assert_eq!(bucket.status_4xx, 1);
        assert_eq!(bucket.status_5xx, 1);
        assert_eq!(bucket.status_other, 0);
    }

    #[test]
    fn snapshot_aggregates_across_seconds() {
        let ring = ApiStatsRing::new();
        let t = 1_700_000_000_000;
        // Two requests in second 0
        ring.record(t, 200, 1, 2);
        ring.record(t + 100, 200, 3, 4);
        // One request 30 seconds later
        ring.record(t + 30_000, 200, 10, 20);

        // 60-second bucket should include all three
        let snap = ring.snapshot(t + 35_000, 60, 60);
        assert_eq!(snap.buckets[0].request_count, 3);
        assert_eq!(snap.buckets[0].bytes_in, 14);
        assert_eq!(snap.buckets[0].bytes_out, 26);
    }

    #[test]
    fn snapshot_returns_separate_buckets_at_finer_interval() {
        let ring = ApiStatsRing::new();
        let t0 = 1_700_000_060_000; // aligned to 60s boundary
        ring.record(t0 + 1_000, 200, 0, 0);
        ring.record(t0 + 2_000, 200, 0, 0);
        ring.record(t0 - 30_000, 200, 0, 0);

        let snap = ring.snapshot(t0 + 5_000, 120, 60);
        assert_eq!(snap.buckets.len(), 2);
        // newest bucket (covering t0..t0+60s) saw 2 requests
        assert_eq!(snap.buckets[0].request_count, 2);
        // previous minute saw 1
        assert_eq!(snap.buckets[1].request_count, 1);
    }

    #[test]
    fn snapshot_ignores_stale_data_from_recycled_slots() {
        let ring = ApiStatsRing::new();
        let t0 = 1_700_000_000_000;
        // Record well outside the window the snapshot will look at.
        ring.record(t0 - 7_200_000, 200, 99, 99);
        // The current minute should report zero, not the stale bucket
        // that happens to live in the same ring slot.
        let snap = ring.snapshot(t0, 60, 60);
        assert_eq!(snap.buckets[0].request_count, 0);
        assert_eq!(snap.buckets[0].bytes_in, 0);
        assert_eq!(snap.buckets[0].bytes_out, 0);
    }

    #[test]
    fn snapshot_clamps_window_to_buffer_size() {
        let ring = ApiStatsRing::new();
        let snap = ring.snapshot(1_700_000_000_000, 1_000_000, 60);
        assert!(snap.window_seconds <= BUCKET_COUNT as u64);
    }

    #[test]
    fn record_bytes_out_adds_without_counting_a_request() {
        let ring = ApiStatsRing::new();
        let t = 1_700_000_000_000;
        ring.record(t, 200, 10, 0);
        ring.record_bytes_out(t + 100, 40);
        ring.record_bytes_out(t + 200, 60);

        let snap = ring.snapshot(t + 500, 60, 60);
        let bucket = &snap.buckets[0];
        // Only the single `record` call counts as a request.
        assert_eq!(bucket.request_count, 1);
        assert_eq!(bucket.status_2xx, 1);
        assert_eq!(bucket.bytes_in, 10);
        // Both `record_bytes_out` calls accumulate into bytes_out.
        assert_eq!(bucket.bytes_out, 100);
    }

    #[test]
    fn record_bytes_out_buckets_by_send_time() {
        let ring = ApiStatsRing::new();
        let t = 1_700_000_000_000;
        ring.record(t, 200, 0, 0);
        // A frame sent 30 seconds into the stream lands in a later second.
        ring.record_bytes_out(t + 30_000, 500);

        let snap = ring.snapshot(t + 35_000, 60, 1);
        // Newest-first: the start second has the request, the +30s
        // second has the streamed bytes — they are not merged into one.
        assert_eq!(snap.buckets[35].request_count, 1);
        assert_eq!(snap.buckets[35].bytes_out, 0);
        assert_eq!(snap.buckets[5].request_count, 0);
        assert_eq!(snap.buckets[5].bytes_out, 500);
    }

    #[test]
    fn record_bytes_out_ignores_zero_and_nonpositive_time() {
        let ring = ApiStatsRing::new();
        let t = 1_700_000_000_000;
        ring.record_bytes_out(t, 0);
        ring.record_bytes_out(0, 100);
        let snap = ring.snapshot(t + 500, 60, 60);
        assert_eq!(snap.buckets[0].bytes_out, 0);
    }

    use http_body_util::{BodyExt, Full};
    use std::sync::atomic::AtomicBool;

    #[tokio::test]
    async fn counting_body_sums_data_frame_bytes() {
        let counter = Arc::new(AtomicU64::new(0));
        let body = CountingBody::new(Full::<bytes::Bytes>::from("hello world"), counter.clone());
        let _ = body.collect().await.expect("collect body");
        assert_eq!(counter.load(Ordering::Relaxed), 11);
    }

    #[tokio::test]
    async fn counting_body_records_start_then_each_frame() {
        let started = Arc::new(AtomicU64::new(0));
        let frame_bytes = Arc::new(AtomicU64::new(0));
        let started_clone = started.clone();
        let frame_clone = frame_bytes.clone();
        let body = CountingBody::with_recorder(
            Full::<bytes::Bytes>::from("abcdef"),
            move || {
                started_clone.fetch_add(1, Ordering::Relaxed);
            },
            move |len| {
                frame_clone.fetch_add(len, Ordering::Relaxed);
            },
        );
        let _ = body.collect().await.expect("collect body");
        assert_eq!(started.load(Ordering::Relaxed), 1);
        assert_eq!(frame_bytes.load(Ordering::Relaxed), 6);
    }

    #[tokio::test]
    async fn counting_body_fires_start_on_drop_when_not_polled() {
        let started = Arc::new(AtomicBool::new(false));
        let started_clone = started.clone();
        {
            let _body = CountingBody::with_recorder(
                Full::<bytes::Bytes>::from("never read"),
                move || {
                    started_clone.store(true, Ordering::Relaxed);
                },
                |_| {},
            );
        }
        assert!(
            started.load(Ordering::Relaxed),
            "on_start should fire from Drop even if the body is never polled",
        );
    }

    #[tokio::test]
    async fn counting_body_fires_start_exactly_once() {
        let count = Arc::new(AtomicU64::new(0));
        let count_clone = count.clone();
        {
            let body = CountingBody::with_recorder(
                Full::<bytes::Bytes>::from("xyz"),
                move || {
                    count_clone.fetch_add(1, Ordering::Relaxed);
                },
                |_| {},
            );
            let _ = body.collect().await.expect("collect body");
            // Body now dropped at end of scope; on_start must not fire again.
        }
        assert_eq!(count.load(Ordering::Relaxed), 1);
    }
}
