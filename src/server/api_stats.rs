//! Lightweight per-second ring buffer of HTTP request stats.
//!
//! Tracks request count, request/response bytes (from `Content-Length`
//! headers — see caveats below), and a 2xx/4xx/5xx breakdown for the
//! last hour. Used by the `GET /admin/api-stats` endpoint and the
//! `torc admin api-stats` CLI to answer "how busy is the server right
//! now?" without streaming individual events.
//!
//! ## Caveats
//!
//! * Byte counts come from `Content-Length` request and response headers
//!   only. Chunked / streaming responses (notably the SSE event streams)
//!   do not advertise a length and will be reported as 0 bytes; the
//!   request *count* is still recorded.
//! * Stats are in-memory only. The buffer is cleared on server restart.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Number of 1-second buckets retained. One hour of history.
pub const BUCKET_COUNT: usize = 3600;

/// Default window the API endpoint reports when none is requested.
pub const DEFAULT_WINDOW_SECONDS: u64 = 3600;

/// Default aggregation interval for the API endpoint.
pub const DEFAULT_INTERVAL_SECONDS: u64 = 60;

/// One second's worth of accumulated request stats.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApiStatsBucket {
    /// Bucket start, in Unix epoch milliseconds.
    pub start_ms: i64,
    /// Total requests handled during this bucket.
    pub request_count: u64,
    /// Sum of `Content-Length` from inbound requests.
    pub bytes_in: u64,
    /// Sum of `Content-Length` from outbound responses.
    pub bytes_out: u64,
    /// Requests that returned a 2xx status.
    pub status_2xx: u64,
    /// Requests that returned a 4xx status.
    pub status_4xx: u64,
    /// Requests that returned a 5xx status.
    pub status_5xx: u64,
    /// Requests that returned anything else (1xx, 3xx).
    pub status_other: u64,
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
    pub now_ms: i64,
    /// Width of each bucket in seconds.
    pub interval_seconds: u64,
    /// Total span covered by `buckets`, in seconds.
    pub window_seconds: u64,
    /// Newest first: `buckets[0]` is the most recent interval.
    pub buckets: Vec<ApiStatsBucket>,
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

impl ApiStatsRing {
    pub fn new() -> Self {
        let buckets = (0..BUCKET_COUNT)
            .map(|_| ApiStatsBucket::default())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            inner: Arc::new(Mutex::new(Inner { buckets })),
        }
    }

    /// Record a single completed request.
    pub fn record(&self, now_ms: i64, status: u16, bytes_in: u64, bytes_out: u64) {
        if now_ms <= 0 {
            return;
        }
        let bucket_start_ms = (now_ms / 1000) * 1000;
        let idx = ((now_ms / 1000) as i128).rem_euclid(BUCKET_COUNT as i128) as usize;
        let mut guard = self.inner.lock();
        let bucket = &mut guard.buckets[idx];
        if bucket.start_ms != bucket_start_ms {
            *bucket = ApiStatsBucket {
                start_ms: bucket_start_ms,
                ..ApiStatsBucket::default()
            };
        }
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

    /// Aggregate the last `window_seconds` of recorded data into
    /// `interval_seconds`-wide buckets, newest first.
    pub fn snapshot(
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
}
