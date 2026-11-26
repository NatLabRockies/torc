//! Events tab, the sidebar's unread-events badge, and desktop notifications.
//!
//! The badge ([`badge`], in the shell's Events link) hosts the app-wide watcher, so it runs on
//! every tab:
//! - every refresh interval (30 s when auto-refresh is off, and also while the page is hidden) it
//!   calls `/ui/events/poll`, which sets the unread count (`$_evCount`) and notifies about followed
//!   workflows that finished;
//! - with live updates on (`$events.live`, remembered in localStorage) it keeps
//!   `/ui/events/stream` open: a proxy of torc-server's event stream for the selected workflow.
//!   Those broadcast events (job_started, job_completed, ...) are never stored, so they are kept
//!   here per connection and merged into the table. Datastar closes the stream while the page is
//!   hidden and reopens it when visible.
//!
//! Stored events come from `/events` (the newest [`MAX`]) on each render.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::{
    Router,
    extract::{Query, State},
    response::{IntoResponse, Response},
    routing::get,
};
use futures::StreamExt;
use maud::{Markup, html};
use serde::Deserialize;
use serde_json::{Value, json};
use torc::models::{EventModel, WorkflowStatusResponse};

use super::Ctx;
use super::kit::{self, Col, G, Sig, SortKey, Table, TableState, Tone, Ui, badge as tone_badge};
use crate::AppState;

const RELOAD: &str = "@get('/ui/tab?content=1')";
/// Most events shown (and kept per live connection).
const MAX: usize = 1000;
/// Where the live-updates choice is remembered (as in the Svelte dashboard).
const LIVE_KEY: &str = "torc-dash.events.live";
/// Saves the live-updates choice (localStorage may be unavailable, e.g. in private mode).
const SAVE: &str = "try { localStorage.setItem('torc-dash.events.live', $events.live ? '1' : '0') } catch (e) {}; 0";
/// Every signal of the `$events` namespace, plus the watcher's local ones.
const SIGNALS: &str = "{events: {filter: '', sort: '', desc: false, page: 0, live: false, notify: false, \
    seenWf: 0, seen: 0, seenLive: 0, clearWf: 0, clear: 0, clearLive: 0, conn: 0}, \
    _evCount: 0, _evRows: 0, _evStatus: 'off', _evAt: 0}";

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ui/events/poll", get(poll))
        .route("/ui/events/stream", get(stream))
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct EvSig {
    live: bool,
    notify: bool,
    /// Workflow the `seen*` marks belong to; the badge counts events newer than them.
    seen_wf: i64,
    seen: i64,
    seen_live: u64,
    /// "Clear" hides events up to these marks (for `clear_wf`).
    clear_wf: i64,
    clear: i64,
    clear_live: u64,
    /// This page's live-stream buffer (see [`LIVE`]).
    conn: u64,
}

// ============== Rows ==============

#[derive(Clone)]
struct Row {
    /// Stored: the event id; live: minus the sequence number.
    key: i64,
    /// Milliseconds since the epoch.
    ts: i64,
    severity: String,
    kind: String,
    job_id: Option<i64>,
    job_name: Option<String>,
    message: String,
    data: Value,
}

fn text(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// A one-line summary: the `message`, or the remaining fields (type and job have their own
/// columns).
fn summarize(data: &Value) -> String {
    const HIDDEN: [&str; 6] = [
        "event_type",
        "type",
        "action",
        "job_id",
        "job_name",
        "workflow_id",
    ];
    match data {
        Value::Null => String::new(),
        Value::Object(o) => text(o.get("message")).unwrap_or_else(|| {
            o.iter()
                .filter(|(k, _)| !HIDDEN.contains(&k.as_str()))
                .map(|(k, v)| match v {
                    Value::String(s) => format!("{k}={s}"),
                    v => format!("{k}={v}"),
                })
                .collect::<Vec<_>>()
                .join("  ")
        }),
        Value::String(s) => s.clone(),
        v => v.to_string(),
    }
}

fn from_stored(e: EventModel) -> Row {
    let d = &e.data;
    let field = |k: &str| text(d.get(k));
    Row {
        key: e.id.unwrap_or_default(),
        ts: e.timestamp,
        severity: field("severity")
            .or_else(|| field("level"))
            .unwrap_or_else(|| "info".into()),
        kind: ["event_type", "type", "action", "category"]
            .iter()
            .find_map(|k| field(k))
            .unwrap_or_else(|| "event".into()),
        job_id: d.get("job_id").and_then(Value::as_i64),
        job_name: field("job_name"),
        message: summarize(d),
        data: e.data,
    }
}

/// A frame of torc-server's event stream (`event: <type>` + `data: <json>`).
fn from_frame(frame: &str, seq: u64) -> Option<Row> {
    let mut kind = "message";
    let mut data = String::new();
    for line in frame.lines() {
        if let Some(v) = line.strip_prefix("event:") {
            kind = v.trim();
        } else if let Some(v) = line.strip_prefix("data:") {
            data.push_str(v.trim_start());
        }
    }
    let e: Value = serde_json::from_str(&data).ok()?;
    let d = e.get("data").filter(|d| !d.is_null()).unwrap_or(&e).clone();
    Some(Row {
        key: -(seq as i64),
        ts: e
            .get("timestamp")
            .and_then(Value::as_i64)
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
        severity: text(e.get("severity")).unwrap_or_else(|| "info".into()),
        kind: text(e.get("event_type")).unwrap_or_else(|| kind.to_string()),
        job_id: d.get("job_id").and_then(Value::as_i64),
        job_name: text(d.get("job_name")),
        message: summarize(&d),
        data: d,
    })
}

// ============== Live stream ==============

/// Live events per page connection: (workflow, newest first). Kept after the stream closes, so a
/// page that was hidden for a while (Datastar closes the stream then) still shows them.
// ponytail: the 16 most recent connections, a per-page TTL if many dashboards share one torc-dash.
#[allow(clippy::type_complexity)]
static LIVE: Mutex<BTreeMap<u64, (i64, VecDeque<(u64, Row)>)>> = Mutex::new(BTreeMap::new());
static SEQ: AtomicU64 = AtomicU64::new(0);

fn live_rows(conn: u64, wf: i64) -> Vec<(u64, Row)> {
    LIVE.lock()
        .unwrap()
        .get(&conn)
        .filter(|(w, _)| *w == wf)
        .map(|(_, rows)| rows.iter().cloned().collect())
        .unwrap_or_default()
}

/// `GET /ui/events/stream`: relay the selected workflow's live events while `$events.live` is
/// on. Re-requested (which cancels the previous request) whenever `$wf` or `$events.live`
/// changes. Batches arrive at most once a second as a `torc-events` window event, which makes
/// the badge poll (and refresh the table when the Events tab is open).
async fn stream(State(st): State<Arc<AppState>>, Sig(sig): Sig<Value>) -> Response {
    let g: G = serde_json::from_value(sig.clone()).unwrap_or_default();
    let ev: EvSig = kit::ns(&sig, "events");
    let Some(wf) = g.wf.filter(|_| ev.live) else {
        return Ui::new()
            .signals(json!({"_evStatus": "off"}))
            .into_response();
    };
    let conn = {
        let mut live = LIVE.lock().unwrap();
        if live.get(&ev.conn).is_some_and(|(w, _)| *w == wf) {
            ev.conn
        } else {
            let c = SEQ.fetch_add(1, Ordering::Relaxed) + 1;
            live.insert(c, (wf, VecDeque::new()));
            while live.len() > 16 {
                live.pop_first();
            }
            c
        }
    };
    let s = async_stream::stream! {
        yield Ui::new().signals(json!({"events": {"conn": conn}, "_evStatus": "connecting"}));
        loop {
            let url = format!("{}/workflows/{wf}/events/stream", st.api_url);
            let resp = crate::with_server_auth(st.client.get(&url), &st.server_auth)
                .send()
                .await;
            match resp {
                Ok(r) if r.status().is_success() => {
                    yield Ui::new().signals(json!({"_evStatus": "live"}));
                    let mut body = r.bytes_stream();
                    let mut buf: Vec<u8> = Vec::new();
                    let mut pending = false;
                    let mut last = Instant::now();
                    loop {
                        let wait = if pending {
                            Duration::from_secs(1).saturating_sub(last.elapsed())
                        } else {
                            Duration::from_secs(3600)
                        };
                        match tokio::time::timeout(wait, body.next()).await {
                            Ok(Some(Ok(chunk))) => {
                                buf.extend_from_slice(&chunk);
                                while let Some(i) = buf.windows(2).position(|w| w == b"\n\n") {
                                    let frame = String::from_utf8_lossy(&buf[..i]).into_owned();
                                    buf.drain(..i + 2);
                                    let seq = SEQ.fetch_add(1, Ordering::Relaxed) + 1;
                                    if let Some(row) = from_frame(&frame, seq) {
                                        let mut live = LIVE.lock().unwrap();
                                        if let Some((_, rows)) = live.get_mut(&conn) {
                                            rows.push_front((seq, row));
                                            rows.truncate(MAX);
                                        }
                                        pending = true;
                                    }
                                }
                            }
                            Ok(_) => break,
                            Err(_) => {}
                        }
                        if pending && last.elapsed() >= Duration::from_secs(1) {
                            pending = false;
                            last = Instant::now();
                            yield Ui::new().script("dispatchEvent(new CustomEvent('torc-events'))");
                        }
                    }
                }
                // Refused (deleted workflow, no access): stop, like an EventSource would.
                Ok(r) if r.status().is_client_error() => {
                    tracing::warn!("Event stream refused for workflow_id={wf}: {}", r.status());
                    yield Ui::new().signals(json!({"_evStatus": "off"}));
                    return;
                }
                _ => {}
            }
            yield Ui::new().signals(json!({"_evStatus": "reconnecting"}));
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    };
    kit::stream(s)
}

// ============== Badge, poll, notifications ==============

/// The sidebar's Events badge, which also runs the app-wide watcher. The shell renders it in
/// place of an empty `span#events-badge`.
pub fn badge() -> Markup {
    // The Settings tab's "Notify me when a workflow finishes" checkbox owns this key; read it
    // before every poll so the server only checks workflows when notifications are on.
    let poll = |url: &str| {
        format!(
            "$events.notify = (() => {{ try {{ return localStorage.getItem('torc-dash.notify') === '1' }} catch (e) {{ return false }} }})(), @get('{url}', {{openWhenHidden: true}})"
        )
    };
    let load = format!(
        "try {{ $events.live = localStorage.getItem('{LIVE_KEY}') === '1' }} catch (e) {{}}; $_evAt = Date.now(); {}",
        poll("/ui/events/poll")
    );
    html! {
        span #events-badge data-signals__ifmissing=(SIGNALS)
            data-init=(load)
            data-effect="$wf; $events.live; @get('/ui/events/stream')"
            // Time-based, not tick-counting: hidden pages throttle intervals hard.
            "data-on-interval__duration.1s"=(format!("Date.now() - $_evAt >= ($refresh || 30) * 1000 && ($_evAt = Date.now(), {})", poll("/ui/events/poll")))
            data-on:torc-events__window=(poll("/ui/events/poll?live=1"))
            data-class:nav-badge="$_evCount > 0"
            data-text="$_evCount > 99 ? '99+' : ($_evCount || '')" {}
    }
}

/// Workflows to notify about, and the last-seen completion of each; a false → true change
/// notifies.
// ponytail: shared by every page of this torc-dash, so with two pages open only the first to
// poll notifies; key it by page if that matters.
#[allow(clippy::type_complexity)]
static WATCH: Mutex<Option<(HashSet<i64>, HashMap<i64, bool>)>> = Mutex::new(None);

/// Follow a workflow: notify the user when it finishes (called when it is run or submitted, so
/// it counts as running even if it finishes before the next poll).
pub fn follow(workflow_id: i64) {
    let mut w = WATCH.lock().unwrap();
    let (followed, complete) = w.get_or_insert_default();
    followed.insert(workflow_id);
    complete.insert(workflow_id, false);
}

/// Desktop notifications for followed workflows (and the selected one) that just finished.
/// Shown only while the dashboard is hidden or unfocused; otherwise the page shows the change.
async fn notify_finished(st: &AppState, selected: Option<i64>, mut ui: Ui) -> Ui {
    let ids: Vec<i64> = {
        let mut w = WATCH.lock().unwrap();
        let (followed, _) = w.get_or_insert_default();
        followed
            .iter()
            .copied()
            .chain(selected)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    };
    for id in ids {
        let Ok(s) =
            kit::get::<WorkflowStatusResponse>(st, &format!("/workflows/{id}/status"), &[]).await
        else {
            continue; // Deleted or unreachable; try again next time.
        };
        let was = WATCH
            .lock()
            .unwrap()
            .get_or_insert_default()
            .1
            .insert(id, s.is_complete);
        if was != Some(false) || !s.is_complete {
            continue;
        }
        let c = &s.jobs_by_status;
        let bad = c.failed + c.terminated + c.canceled;
        let title = format!(
            "Workflow {id} {}",
            if bad > 0 {
                "finished with failures"
            } else {
                "completed"
            }
        );
        let body = if bad > 0 {
            format!(
                "{}: {} failed, {} terminated, {} canceled",
                s.workflow_name, c.failed, c.terminated, c.canceled
            )
        } else {
            format!(
                "{}: {} of {} jobs completed",
                s.workflow_name, c.completed, s.total_jobs
            )
        };
        tracing::info!("Notifying that workflow_id={id} finished");
        // Torc Desktop polyfills Notification; some browsers (Android) throw outside a worker.
        let q = |s: &str| kit::js(s).replace('<', "\\u003c");
        ui = ui.script(&format!(
            "try {{ (document.hidden || !document.hasFocus()) && localStorage.getItem('torc-dash.notify') === '1' && typeof Notification !== 'undefined' && Notification.permission === 'granted' && new Notification({}, {{body: {}}}) }} catch (e) {{}}",
            q(&title),
            q(&body)
        ));
    }
    ui
}

#[derive(Deserialize)]
struct PollQuery {
    #[serde(default)]
    live: Option<String>,
}

/// `GET /ui/events/poll`: the unread count, notifications, and (after live events, on the Events
/// tab) a fresh table.
async fn poll(
    State(st): State<Arc<AppState>>,
    Query(q): Query<PollQuery>,
    Sig(sig): Sig<Value>,
) -> Ui {
    let g: G = serde_json::from_value(sig.clone()).unwrap_or_default();
    let ev: EvSig = kit::ns(&sig, "events");
    let mut ui = Ui::new();
    if ev.notify {
        ui = notify_finished(&st, g.wf, ui).await;
    }
    let Some(wf) = g.wf else {
        return ui.signals(json!({"_evCount": 0}));
    };
    let Ok(latest) = newest(&st, wf, 100).await else {
        return ui;
    };
    let max_id = latest.first().and_then(|e| e.id).unwrap_or(0);
    let live = live_rows(ev.conn, wf);
    let max_live = live.first().map(|r| r.0).unwrap_or(0);
    if ev.seen_wf != wf || g.tab == "events" {
        // A newly selected workflow starts with nothing unread; the Events tab marks all seen.
        ui = ui.signals(json!({
            "events": {"seenWf": wf, "seen": max_id, "seenLive": max_live},
            "_evCount": 0,
        }));
        if q.live.is_some() && g.tab == "events" {
            let t: TableState = kit::ns(&sig, "events");
            ui = ui.patch(body(&st, wf, &ev, &t).await);
        }
        return ui;
    }
    let n = latest
        .iter()
        .filter(|e| e.id.unwrap_or(0) > ev.seen)
        .count()
        + live.iter().filter(|r| r.0 > ev.seen_live).count();
    ui.signals(json!({"_evCount": n}))
}

#[derive(Deserialize)]
struct EventPage {
    items: Vec<EventModel>,
}

/// The workflow's newest stored events, newest first.
async fn newest(st: &AppState, wf: i64, limit: usize) -> Result<Vec<EventModel>, String> {
    let q = [
        ("workflow_id", wf.to_string()),
        ("sort_by", "id".to_string()),
        ("reverse_sort", "true".to_string()),
        ("offset", "0".to_string()),
        ("limit", limit.to_string()),
    ];
    Ok(kit::get::<EventPage>(st, "/events", &q).await?.items)
}

// ============== Tab ==============

pub async fn view(st: &Arc<AppState>, cx: &Ctx) -> Markup {
    let ev: EvSig = kit::ns(&cx.sig, "events");
    let t: TableState = kit::ns(&cx.sig, "events");
    let status = "$events.live ? ({off: 'Paused', connecting: 'Connecting…', live: 'Live', reconnecting: 'Reconnecting…'})[$_evStatus] : 'Live updates'";
    html! {
        div.page data-signals__ifmissing=(SIGNALS) {
            div.page-header {
                h2 { "Events" }
                @if cx.g.wf.is_some() {
                    label.stream title="Stream live events from the server (server-sent events). Paused while this browser tab is hidden."
                        data-class:live="$events.live && $_evStatus === 'live'"
                        data-class:waiting="$events.live && ($_evStatus === 'connecting' || $_evStatus === 'reconnecting')" {
                        input type="checkbox" data-bind="events.live" data-on:change=(SAVE);
                        span.dot {}
                        span data-text=(status) { "Live updates" }
                    }
                    (kit::filter_box("events", RELOAD, "Filter events  /"))
                    button.btn title="Clear the displayed events" data-attr:disabled="!$_evRows"
                        data-on:click=(format!("$events.clearWf = $events.seenWf; $events.clear = $events.seen; $events.clearLive = $events.seenLive; $events.page = 0; {RELOAD}")) {
                        "Clear"
                    }
                }
            }
            @match cx.g.wf {
                None => div.card.empty { "Pick a workflow in the top bar to see its events." },
                Some(wf) => (body(st, wf, &ev, &t).await),
            }
        }
    }
}

/// The table (and the Clear button's marks), patched on its own after live events.
async fn body(st: &AppState, wf: i64, ev: &EvSig, t: &TableState) -> Markup {
    let (stored, error) = match newest(st, wf, MAX).await {
        Ok(r) => (r, None),
        Err(e) => (Vec::new(), Some(e)),
    };
    let live = live_rows(ev.conn, wf);
    let max_id = stored.first().and_then(|e| e.id).unwrap_or(0);
    let max_live = live.first().map(|r| r.0).unwrap_or(0);
    let (clear, clear_live) = if ev.clear_wf == wf {
        (ev.clear, ev.clear_live)
    } else {
        (0, 0)
    };
    let mut rows: Vec<Row> = stored
        .into_iter()
        .filter(|e| e.id.unwrap_or(0) > clear)
        .map(from_stored)
        .chain(live.into_iter().filter(|r| r.0 > clear_live).map(|r| r.1))
        .collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.ts));
    rows.truncate(MAX);

    let cols = vec![
        Col::text("timestamp", "Time", |r: &Row| r.ts.to_string())
            .width("210px")
            .sort_by(|r| SortKey::Num(r.ts as f64))
            .cell(|r| {
                let ts = r.ts.to_string();
                let iso = chrono::DateTime::from_timestamp_millis(r.ts)
                    .map(|d| d.to_rfc3339())
                    .unwrap_or_default();
                html! { span title=(iso) { (kit::fmt_datetime(Some(&ts))) } }
            }),
        Col::text("severity", "Level", |r: &Row| r.severity.clone())
            .width("96px")
            .cell(|r| {
                let tone = match r.severity.to_lowercase().as_str() {
                    "error" => Tone::Danger,
                    "warning" => Tone::Warning,
                    "info" => Tone::Info,
                    _ => Tone::Neutral,
                };
                tone_badge(tone, &r.severity)
            }),
        Col::text("type", "Type", |r: &Row| r.kind.clone())
            .width("200px")
            .mono(),
        Col::text("job", "Job", |r: &Row| {
            r.job_id
                .map(|id| format!("{id} {}", r.job_name.as_deref().unwrap_or("")))
                .unwrap_or_default()
        })
        .width("200px")
        .cell(|r| match r.job_id {
            Some(id) => html! {
                button.link title=(format!("Open job {id}"))
                    data-on:click=(format!("evt.stopPropagation(); @get('/ui/job/{id}')")) {
                    span.mono { "#" (id) }
                    @if let Some(n) = &r.job_name { span.job-name { (n) } }
                }
            },
            None => html! { span.faint { "—" } },
        }),
        Col::text("message", "Details", |r: &Row| r.message.clone()).cell(|r| {
            let full = serde_json::to_string_pretty(&r.data).unwrap_or_default();
            html! { span.details title=(full) { (r.message) } }
        }),
    ];
    let mut table = Table::new("events", RELOAD, |r: &Row| r.key, cols);
    table.on_click = Some(Box::new(|r| match r.job_id {
        Some(id) => format!("@get('/ui/job/{id}')"),
        None => "void 0".into(),
    }));
    table.empty = if ev.live {
        "No events yet. Waiting for new events…".into()
    } else {
        "No events yet.".into()
    };
    let n = rows.len();
    html! {
        div #events-body.events-body {
            @if let (Some(e), true) = (&error, rows.is_empty()) {
                p.error-text { "Failed to load events: " (e) }
            }
            div.table-host { (table.render(&rows, t)) }
            // Being on this tab marks everything seen (which is also what Clear hides). A new id
            // per mark re-runs data-init.
            div id=(format!("events-seen-{wf}-{max_id}-{max_live}-{n}")) hidden
                data-init=(format!("$events.seenWf = {wf}; $events.seen = {max_id}; $events.seenLive = {max_live}; $_evRows = {n}; $_evCount = 0")) {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_from_stored_and_live() {
        let r = from_stored(EventModel {
            id: Some(7),
            workflow_id: 1,
            timestamp: 1_700_000_000_000,
            data: json!({"event_type": "job_failed", "job_id": 3, "job_name": "a", "return_code": 2}),
        });
        assert_eq!(
            (r.kind.as_str(), r.job_id, r.severity.as_str()),
            ("job_failed", Some(3), "info")
        );
        assert_eq!(r.message, "return_code=2");

        let frame = "event: job_started\ndata: {\"workflow_id\":1,\"timestamp\":5,\"event_type\":\"job_started\",\"severity\":\"info\",\"data\":{\"job_id\":4,\"message\":\"go\"}}";
        let r = from_frame(frame, 9).unwrap();
        assert_eq!(
            (r.key, r.ts, r.job_id, r.message.as_str()),
            (-9, 5, Some(4), "go")
        );
        let r = from_frame("event: warning\ndata: {\"dropped\": 3}", 10).unwrap();
        assert_eq!(
            (r.kind.as_str(), r.message.as_str()),
            ("warning", "dropped=3")
        );
    }
}
