//! Building blocks shared by the server-rendered dashboard views: Datastar responses, signal
//! extraction, torc-server API calls, and small HTML components.
//!
//! How a view works: the browser holds UI state in Datastar signals (`$tab`, `$wf`, and one
//! namespace per tab such as `$wfs.filter`). Every `@get`/`@post` sends all non-underscore
//! signals; a handler reads the ones it needs with [`Sig`], renders HTML with maud, and answers
//! with a [`Ui`], which patches elements by id (morphing, so focus and scroll survive).

use std::sync::Arc;

use axum::{
    Json,
    body::Body,
    extract::{FromRequest, Request, State},
    http::{Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use futures::{Stream, StreamExt};
use maud::{Markup, PreEscaped, html};
use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use serde_json::Value;

use crate::AppState;

// ============== Responses ==============

/// A Datastar answer: a list of SSE events that patch elements or signals. Return it from a
/// handler, or send several over time with [`stream`].
#[derive(Default)]
pub struct Ui(String);

impl Ui {
    pub fn new() -> Self {
        Self::default()
    }

    fn elements(mut self, selector: Option<&str>, mode: &str, html: &str) -> Self {
        self.0.push_str("event: datastar-patch-elements\n");
        if let Some(s) = selector {
            self.0.push_str(&format!("data: selector {s}\n"));
        }
        if mode != "outer" {
            self.0.push_str(&format!("data: mode {mode}\n"));
        }
        for line in html.lines() {
            self.0.push_str("data: elements ");
            self.0.push_str(line);
            self.0.push('\n');
        }
        self.0.push('\n');
        self
    }

    /// Morph the top-level element(s) of `html` into the page elements with the same ids.
    pub fn patch(self, html: Markup) -> Self {
        self.elements(None, "outer", &html.into_string())
    }

    /// Replace the children of `selector` with `html`.
    pub fn inner(self, selector: &str, html: Markup) -> Self {
        self.elements(Some(selector), "inner", &html.into_string())
    }

    pub fn append(self, selector: &str, html: Markup) -> Self {
        self.elements(Some(selector), "append", &html.into_string())
    }

    pub fn remove(self, selector: &str) -> Self {
        self.elements(Some(selector), "remove", "")
    }

    /// Merge `signals` into the page's signals; a `null` value deletes a signal.
    pub fn signals(mut self, signals: Value) -> Self {
        self.0.push_str("event: datastar-patch-signals\n");
        self.0.push_str(&format!("data: signals {signals}\n\n"));
        self
    }

    /// Run a line of JavaScript in the page (e.g. `history.back()`).
    pub fn script(self, js: &str) -> Self {
        let tag = html! { script data-effect="el.remove()" { (PreEscaped(js)) } };
        self.append("body", tag)
    }

    pub fn toast(self, tone: Tone, message: impl AsRef<str>) -> Self {
        // Errors stay until dismissed would be annoying for routine failures; give them longer.
        let ms = match tone {
            Tone::Danger => 12000,
            Tone::Warning => 8000,
            _ => 4000,
        };
        let t = html! {
            div.toast.(tone.class()) role="status" data-init=(format!("setTimeout(() => el.remove(), {ms})")) {
                span.msg { (message.as_ref()) }
                button.btn.btn-ghost.btn-sm.btn-icon aria-label="Dismiss" data-on:click="el.parentElement.remove()" { "✕" }
            }
        };
        self.append("#toasts", t)
    }

    pub fn ok(self, msg: impl AsRef<str>) -> Self {
        self.toast(Tone::Success, msg)
    }

    pub fn err(self, msg: impl AsRef<str>) -> Self {
        self.toast(Tone::Danger, msg)
    }

    /// Show `dialog` (from [`modal`]) in the modal slot.
    pub fn modal(self, dialog: Markup) -> Self {
        // Empty the slot first: morphing an open dialog into a new one drops its `open` state.
        self.close_modal().inner("#modals", dialog)
    }

    pub fn close_modal(self) -> Self {
        self.inner("#modals", html! {})
    }

    /// Ask the page to re-render the current tab (after an action changed server state).
    pub fn refresh(self) -> Self {
        self.script("document.body.dispatchEvent(new CustomEvent('torc-refresh'))")
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl IntoResponse for Ui {
    fn into_response(self) -> Response {
        (
            [
                (header::CONTENT_TYPE, "text/event-stream"),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            self.0,
        )
            .into_response()
    }
}

/// A long-lived SSE response that sends each [`Ui`] as it arrives (live output, chat replies).
pub fn stream(s: impl Stream<Item = Ui> + Send + 'static) -> Response {
    let body = Body::from_stream(s.map(|ui| Ok::<_, std::convert::Infallible>(ui.into_string())));
    Response::builder()
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("x-accel-buffering", "no")
        .body(body)
        .unwrap()
}

// ============== Signals ==============

/// The signals sent with a Datastar request, deserialized into `T` (unknown fields ignored).
/// GET requests carry them in the `datastar` query parameter, others in the JSON body.
pub struct Sig<T>(pub T);

impl<T: DeserializeOwned, S: Send + Sync> FromRequest<S> for Sig<T> {
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let bad = |e: String| (StatusCode::BAD_REQUEST, e).into_response();
        let json = if req.method() == Method::GET || req.method() == Method::DELETE {
            url::form_urlencoded::parse(req.uri().query().unwrap_or("").as_bytes())
                .find(|(k, _)| k == "datastar")
                .map(|(_, v)| v.into_owned())
                .unwrap_or_default()
        } else {
            let bytes = axum::body::Bytes::from_request(req, state)
                .await
                .map_err(|e| bad(e.to_string()))?;
            String::from_utf8_lossy(&bytes).into_owned()
        };
        let json = if json.trim().is_empty() { "{}" } else { &json };
        serde_json::from_str(json)
            .map(Sig)
            .map_err(|e| bad(format!("bad signals: {e}")))
    }
}

/// Signals every view can read: the open tab, the selected workflow, and global preferences.
/// Flatten it into a view's own signal struct: `#[serde(flatten)] g: G`.
#[derive(Deserialize, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct G {
    pub tab: String,
    #[serde(deserialize_with = "lenient_id")]
    pub wf: Option<i64>,
    /// Auto-refresh interval in seconds; 0 = off.
    pub refresh: u32,
    pub all_users: bool,
    pub archived: bool,
    /// Access-group filter for workflow lists.
    pub group: String,
    /// Base directory of job logs (Debugging tab and log downloads).
    pub output_dir: String,
}

impl G {
    /// Query for the workflow lists: the user's own (or every user's, or an access group's),
    /// archived ones only when asked for.
    pub fn workflow_query(&self) -> Vec<(&'static str, String)> {
        let mut q = Vec::new();
        let group = self.group.trim();
        if !group.is_empty() {
            q.push(("access_group", group.to_string()));
        } else if !self.all_users {
            q.push(("user", torc::get_username()));
        }
        if !self.archived {
            q.push(("is_archived", "false".into()));
        }
        q
    }
}

/// Accepts an id as a number, a numeric string (from a `<select>`), or empty/null.
pub fn lenient_id<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().trim_start_matches('#').parse().ok(),
        _ => None,
    })
}

/// Ids of the rows checked in a [`Table`] with selection (`{"r12": true, "r13": false}`).
pub fn selected_ids(sel: &std::collections::HashMap<String, bool>) -> Vec<i64> {
    let mut ids: Vec<i64> = sel
        .iter()
        .filter(|(_, v)| **v)
        .filter_map(|(k, _)| k.trim_start_matches('r').parse().ok())
        .collect();
    ids.sort_unstable();
    ids
}

// ============== torc-server API ==============

/// The JSON error message in a torc-server error body, or the body itself.
fn error_message(status: reqwest::StatusCode, body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    for key in ["message", "error", "detail"] {
        match v.get(key) {
            Some(Value::String(s)) => return s.clone(),
            Some(o @ Value::Object(_)) => return o.to_string(),
            _ => {}
        }
    }
    if body.trim().is_empty() {
        format!("HTTP {status}")
    } else {
        body.trim().to_string()
    }
}

/// Call torc-server (`path` is relative to the API root, e.g. `/workflows/3`) and parse JSON.
pub async fn api<T: DeserializeOwned>(
    st: &AppState,
    method: Method,
    path: &str,
    query: &[(&str, String)],
    body: Option<&Value>,
) -> Result<T, String> {
    let url = format!("{}{}", st.api_url, path);
    let mut req = st.client.request(method, &url).query(query);
    req = crate::with_server_auth(req, &st.server_auth);
    if let Some(b) = body {
        req = req.json(b);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("Cannot reach torc-server: {e}"))?;
    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(crate::unauthorized_message(st, &url, resp.url().as_str()));
    }
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(error_message(status, &text));
    }
    // Some endpoints answer with an empty body; let `()`/Value callers accept that.
    let text = if text.trim().is_empty() {
        "null"
    } else {
        &text
    };
    serde_json::from_str(text).map_err(|e| format!("Unexpected response from {path}: {e}"))
}

pub async fn get<T: DeserializeOwned>(
    st: &AppState,
    path: &str,
    query: &[(&str, String)],
) -> Result<T, String> {
    api(st, Method::GET, path, query, None).await
}

#[derive(Deserialize)]
struct Page<T> {
    items: Vec<T>,
    #[serde(default)]
    has_more: bool,
}

/// Every item of a paginated list endpoint.
pub async fn list_all<T: DeserializeOwned>(
    st: &AppState,
    path: &str,
    query: &[(&str, String)],
) -> Result<Vec<T>, String> {
    let mut all = Vec::new();
    loop {
        let mut q = query.to_vec();
        q.push(("offset", all.len().to_string()));
        q.push(("limit", "10000".into()));
        let page: Page<T> = get(st, path, &q).await?;
        let n = page.items.len();
        all.extend(page.items);
        if !page.has_more || n == 0 {
            return Ok(all);
        }
    }
}

/// Call one of torc-dash's JSON handlers in-process and parse its JSON reply, e.g.
/// `call(&st, crate::cli_submit_handler, json!({"workflow_id": "3"}))`.
pub async fn call<R, F, Fut, O>(st: &Arc<AppState>, handler: F, body: Value) -> Result<O, String>
where
    R: DeserializeOwned,
    F: FnOnce(State<Arc<AppState>>, Json<R>) -> Fut,
    Fut: std::future::Future,
    Fut::Output: IntoResponse,
    O: DeserializeOwned,
{
    let req: R = serde_json::from_value(body).map_err(|e| e.to_string())?;
    json_of(handler(State(st.clone()), Json(req)).await).await
}

/// Parse the JSON body of a handler's response.
pub async fn json_of<O: DeserializeOwned>(resp: impl IntoResponse) -> Result<O, String> {
    let bytes = axum::body::to_bytes(resp.into_response().into_body(), usize::MAX)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

/// This tab's namespace of the signals (e.g. `ns(sig, "wfs")`), or its default.
pub fn ns<T: DeserializeOwned + Default>(sig: &Value, name: &str) -> T {
    sig.get(name)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

/// Result of `torc` run through a CLI handler: Ok(stdout) or Err(the command's error text).
pub fn cli_result(r: crate::CliResponse) -> Result<String, String> {
    if r.success {
        Ok(r.stdout)
    } else {
        let e = if r.stderr.trim().is_empty() {
            r.stdout
        } else {
            r.stderr
        };
        Err(if e.trim().is_empty() {
            "Command failed".into()
        } else {
            e.trim().to_string()
        })
    }
}

/// Envelope of `/api/cli/op` and the JSON CLI wrappers: Ok(data) or Err(error).
pub fn op_result(v: Value) -> Result<Value, String> {
    if v.get("success").and_then(Value::as_bool) == Some(true) {
        Ok(v.get("data").cloned().unwrap_or(Value::Null))
    } else {
        Err(v
            .get("error")
            .and_then(Value::as_str)
            .or_else(|| v.get("stderr").and_then(Value::as_str))
            .unwrap_or("Command failed")
            .trim()
            .to_string())
    }
}

/// Run a whitelisted torc command (see `build_op`) and return its parsed JSON output.
pub async fn op(st: &Arc<AppState>, name: &str, mut params: Value) -> Result<Value, String> {
    params["op"] = Value::String(name.into());
    op_result(call(st, crate::cli_op_handler, params).await?)
}

// ============== Formatting ==============

pub fn to_datetime(v: &str) -> Option<chrono::DateTime<chrono::Local>> {
    use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
    if let Ok(n) = v.parse::<f64>() {
        let secs = if n < 1e12 { n } else { n / 1000.0 };
        return Local.timestamp_opt(secs as i64, 0).single();
    }
    DateTime::parse_from_rfc3339(v)
        .map(|d| d.with_timezone(&Local))
        .ok()
        .or_else(|| {
            NaiveDateTime::parse_from_str(v, "%Y-%m-%d %H:%M:%S%.f")
                .ok()
                .and_then(|n| Local.from_local_datetime(&n).single())
        })
}

pub fn fmt_datetime(v: Option<&str>) -> String {
    v.and_then(to_datetime)
        .map(|d| d.format("%b %-d, %Y %H:%M:%S").to_string())
        .unwrap_or_else(|| "—".into())
}

pub fn fmt_relative(v: Option<&str>) -> String {
    let Some(d) = v.and_then(to_datetime) else {
        return "—".into();
    };
    let s = (chrono::Local::now() - d).num_seconds();
    match s {
        ..=4 => "just now".into(),
        5..60 => format!("{s}s ago"),
        60..3600 => format!("{}m ago", s / 60),
        3600..86400 => format!("{}h ago", s / 3600),
        _ => format!("{}d ago", s / 86400),
    }
}

/// Seconds → "1h 02m 03s" / "4m 05s" / "6.2s".
pub fn fmt_duration(seconds: Option<f64>) -> String {
    let Some(s) = seconds.filter(|s| s.is_finite()) else {
        return "—".into();
    };
    if s < 60.0 {
        return if s < 10.0 {
            format!("{s:.1}s")
        } else {
            format!("{}s", s.round())
        };
    }
    let t = s.round() as i64;
    let (h, m, sec) = (t / 3600, (t % 3600) / 60, t % 60);
    if h > 0 {
        format!("{h}h {m:02}m {sec:02}s")
    } else {
        format!("{m}m {sec:02}s")
    }
}

pub fn fmt_bytes(bytes: Option<f64>) -> String {
    let Some(mut v) = bytes.filter(|b| b.is_finite()) else {
        return "—".into();
    };
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut i = 0;
    while v.abs() >= 1024.0 && i < units.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if v < 10.0 && i > 0 {
        format!("{v:.1} {}", units[i])
    } else {
        format!("{} {}", v.round(), units[i])
    }
}

/// "pending_failed" → "pending failed".
pub fn status_label(s: &str) -> String {
    s.replace('_', " ")
}

/// A model enum (e.g. `JobStatus`) as its wire name ("pending_failed").
pub fn wire_name<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

/// Quote `s` as a JavaScript string literal for use inside a Datastar expression.
pub fn js(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

// ============== Components ==============

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Info,
    Accent,
    Success,
    Warning,
    Danger,
}

impl Tone {
    pub fn class(self) -> &'static str {
        match self {
            Tone::Neutral => "neutral",
            Tone::Info => "info",
            Tone::Accent => "accent",
            Tone::Success => "success",
            Tone::Warning => "warning",
            Tone::Danger => "danger",
        }
    }
}

pub const JOB_STATUSES: [&str; 11] = [
    "uninitialized",
    "blocked",
    "ready",
    "pending",
    "running",
    "completed",
    "failed",
    "canceled",
    "terminated",
    "disabled",
    "pending_failed",
];

pub fn status_tone(status: &str) -> Tone {
    match status {
        "ready" => Tone::Info,
        "pending" | "running" => Tone::Accent,
        "completed" => Tone::Success,
        "failed" | "terminated" => Tone::Danger,
        "canceled" | "pending_failed" => Tone::Warning,
        _ => Tone::Neutral,
    }
}

pub fn badge(tone: Tone, text: &str) -> Markup {
    html! { span.badge.(tone.class()) { (text) } }
}

/// Colored pill for a job status (wire name); running/pending pulse.
pub fn status_badge(status: &str) -> Markup {
    let tone = status_tone(status);
    html! {
        span.badge.(tone.class()) {
            @if tone == Tone::Accent { span.pulse {} }
            (status_label(status))
        }
    }
}

/// Stroke icon (Lucide-style paths, ISC license). Unknown names render nothing.
pub fn icon(name: &str, size: u32) -> Markup {
    let d = match name {
        "layers" => "M12 2 2 7l10 5 10-5-10-5Z M2 17l10 5 10-5 M2 12l10 5 10-5",
        "list" => "M8 6h13 M8 12h13 M8 18h13 M3 6h.01 M3 12h.01 M3 18h.01",
        "graph" => {
            "M18 8a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z M6 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z M18 22a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z M8.6 13.5l6.8 4 M15.4 6.5l-6.8 4"
        }
        "bell" => "M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9 M10.3 21a1.94 1.94 0 0 0 3.4 0",
        "bug" => {
            "M8 2l1.9 1.9 M16 2l-1.9 1.9 M9 7.1V6a3 3 0 0 1 6 0v1.1 M12 20c-3.3 0-6-2.7-6-6v-3a4 4 0 0 1 4-4h4a4 4 0 0 1 4 4v3c0 3.3-2.7 6-6 6Z M12 20v-9 M6.5 9C4.6 8.8 3 7.1 3 5 M6 13H2 M3 21c0-2.1 1.7-3.9 3.8-4 M20.97 5c0 2.1-1.6 3.8-3.5 4 M22 13h-4 M17.2 17c2.1.1 3.8 1.9 3.8 4"
        }
        "activity" => "M22 12h-4l-3 9L9 3l-3 9H2",
        "chat" => "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z",
        "settings" => {
            "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.6 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.6a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1Z"
        }
        "sun" => {
            "M12 17a5 5 0 1 0 0-10 5 5 0 0 0 0 10Z M12 1v2 M12 21v2 M4.22 4.22l1.42 1.42 M18.36 18.36l1.42 1.42 M1 12h2 M21 12h2 M4.22 19.78l1.42-1.42 M18.36 5.64l1.42-1.42"
        }
        "moon" => "M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79Z",
        "monitor" => "M2 3h20v14H2z M8 21h8 M12 17v4",
        "refresh" => {
            "M21 2v6h-6 M3 12a9 9 0 0 1 15-6.7L21 8 M3 22v-6h6 M21 12a9 9 0 0 1-15 6.7L3 16"
        }
        "plus" => "M12 5v14 M5 12h14",
        "play" => "M6 3l14 9-14 9V3z",
        "stop" => "M6 6h12v12H6z",
        "trash" => {
            "M3 6h18 M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6 M10 11v6 M14 11v6 M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"
        }
        "download" => "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4 M7 10l5 5 5-5 M12 15V3",
        "upload" => "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4 M17 8l-5-5-5 5 M12 3v12",
        "edit" => "M12 20h9 M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z",
        "file" => "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z M14 2v6h6",
        "search" => "M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16Z M21 21l-4.35-4.35",
        "x" => "M18 6 6 18 M6 6l12 12",
        "check" => "M20 6 9 17l-5-5",
        "chevronDown" => "M6 9l6 6 6-6",
        "chevronRight" => "M9 18l6-6-6-6",
        "more" => {
            "M12 13a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z M19 13a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z M5 13a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z"
        }
        "send" => "M22 2 11 13 M22 2l-7 20-4-9-9-4 20-7z",
        "user" => "M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2 M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z",
        "external" => {
            "M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6 M15 3h6v6 M10 14 21 3"
        }
        _ => "",
    };
    html! {
        svg width=(size) height=(size) viewBox="0 0 24 24" fill="none" stroke="currentColor"
            stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" {
            @for (i, p) in d.split(" M").enumerate() {
                path d=(if i == 0 { p.to_string() } else { format!("M{p}") }) {}
            }
        }
    }
}

/// A native `<dialog>` for [`Ui::modal`]. It opens itself, closes on Esc, the ✕ button, or a
/// backdrop click, and empties the modal slot when closed. `size`: sm | md | lg | xl.
pub fn modal(title: &str, size: &str, body: Markup, footer: Option<Markup>) -> Markup {
    html! {
        dialog.modal.(size) data-init="el.showModal()"
            data-on:mousedown="el._down = evt.target === el"
            data-on:click="el._down && evt.target === el && el.close()"
            data-on:close="document.getElementById('modals').replaceChildren()" {
            header {
                h3 { (title) }
                button.btn.btn-ghost.btn-icon.btn-sm aria-label="Close" data-on:click="el.closest('dialog').close()" { "✕" }
            }
            div.body { (body) }
            @if let Some(f) = footer { footer { (f) } }
        }
    }
}

/// A Cancel button that closes the enclosing dialog.
pub fn cancel_button() -> Markup {
    html! { button.btn data-on:click="el.closest('dialog').close()" { "Cancel" } }
}

/// A segmented sub-tab switcher bound to the string signal `signal`. Clicking sets it and runs
/// `action` (e.g. `@get('/ui/details')`).
pub fn seg_tabs(
    signal: &str,
    current: &str,
    tabs: &[(&str, &str, Option<usize>)],
    action: &str,
) -> Markup {
    html! {
        div.seg role="tablist" {
            @for (id, label, count) in tabs {
                button role="tab" .active[*id == current] aria-selected=(*id == current)
                    data-on:click=(format!("${signal} = {}; {action}", js(id))) {
                    (label)
                    @if let Some(c) = count { span.count { (c) } }
                }
            }
        }
    }
}

/// A key/value grid (`<dl class="kv">`), skipping empty values.
pub fn kv(rows: &[(&str, Markup)]) -> Markup {
    html! { dl.kv { @for (k, v) in rows { dt { (k) } dd { (v) } } } }
}

/// Text or an em dash when empty.
pub fn or_dash(v: Option<impl std::fmt::Display>) -> String {
    v.map(|v| v.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "—".into())
}

pub fn error_box(msg: &str) -> Markup {
    html! { p.error-text { (msg) } }
}

// ============== Table ==============

/// Sort/filter/paging state of a [`Table`], kept in the signals under the table's namespace
/// (`{filter, sort, desc, page}`).
#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct TableState {
    pub filter: String,
    pub sort: String,
    pub desc: bool,
    pub page: usize,
    /// Checked rows, as `{"r<key>": true}`.
    pub sel: std::collections::HashMap<String, bool>,
}

/// Value a column sorts by.
pub enum SortKey {
    Num(f64),
    Text(String),
    None,
}

impl SortKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering::*;
        match (self, other) {
            (SortKey::None, SortKey::None) => Equal,
            (SortKey::None, _) => Greater,
            (_, SortKey::None) => Less,
            (SortKey::Num(a), SortKey::Num(b)) => a.partial_cmp(b).unwrap_or(Equal),
            (a, b) => natural_cmp(&a.text(), &b.text()),
        }
    }
    fn text(&self) -> String {
        match self {
            SortKey::Num(n) => n.to_string(),
            SortKey::Text(s) => s.clone(),
            SortKey::None => String::new(),
        }
    }
}

/// Case-insensitive comparison that orders embedded numbers numerically ("job2" < "job10").
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, _) => return std::cmp::Ordering::Less,
            (_, None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut s = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        s.push(c);
                        it.next();
                    }
                    s.trim_start_matches('0').to_string()
                };
                let (na, nb) = (take(&mut a), take(&mut b));
                let o = na.len().cmp(&nb.len()).then(na.cmp(&nb));
                if o.is_ne() {
                    return o;
                }
            }
            (Some(x), Some(y)) => {
                let o = x.to_lowercase().cmp(y.to_lowercase());
                if o.is_ne() {
                    return o;
                }
                a.next();
                b.next();
            }
        }
    }
}

/// A boxed per-row function.
pub type RowFn<'a, T, R> = Box<dyn Fn(&T) -> R + 'a>;

pub struct Col<'a, T> {
    pub key: &'a str,
    pub label: &'a str,
    /// CSS width such as "90px" or "20%".
    pub width: Option<&'a str>,
    pub mono: bool,
    /// Text used for display (when no `cell`) and for filtering.
    pub text: RowFn<'a, T, String>,
    /// Sort value; None makes the column unsortable.
    pub sort: Option<RowFn<'a, T, SortKey>>,
    pub cell: Option<RowFn<'a, T, Markup>>,
}

impl<'a, T> Col<'a, T> {
    /// A sortable text column.
    pub fn text(key: &'a str, label: &'a str, f: impl Fn(&T) -> String + 'a) -> Self {
        let f = std::rc::Rc::new(f);
        let g = f.clone();
        Col {
            key,
            label,
            width: None,
            mono: false,
            text: Box::new(move |r| f(r)),
            sort: Some(Box::new(move |r| {
                let s = g(r);
                if s.is_empty() {
                    SortKey::None
                } else {
                    SortKey::Text(s)
                }
            })),
            cell: None,
        }
    }

    /// A sortable numeric column.
    pub fn num(key: &'a str, label: &'a str, f: impl Fn(&T) -> Option<f64> + 'a) -> Self {
        let f = std::rc::Rc::new(f);
        let g = f.clone();
        Col {
            key,
            label,
            width: None,
            mono: true,
            text: Box::new(move |r| f(r).map(|v| v.to_string()).unwrap_or_default()),
            sort: Some(Box::new(move |r| {
                g(r).map(SortKey::Num).unwrap_or(SortKey::None)
            })),
            cell: None,
        }
    }

    pub fn width(mut self, w: &'a str) -> Self {
        self.width = Some(w);
        self
    }
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }
    pub fn cell(mut self, f: impl Fn(&T) -> Markup + 'a) -> Self {
        self.cell = Some(Box::new(f));
        self
    }
    pub fn sort_by(mut self, f: impl Fn(&T) -> SortKey + 'a) -> Self {
        self.sort = Some(Box::new(f));
        self
    }
    pub fn unsortable(mut self) -> Self {
        self.sort = None;
        self
    }
}

/// A server-side sorted, filtered, paged table. Its controls write `$<ns>.sort` etc. and call
/// `reload` (a Datastar action that re-renders the view, e.g. `@get('/ui/tab')`).
pub struct Table<'a, T> {
    /// Element id, and the signal namespace for its state (e.g. "wfs").
    pub ns: &'a str,
    pub reload: &'a str,
    pub cols: Vec<Col<'a, T>>,
    /// Row key for selection and `active`.
    pub key: RowFn<'a, T, i64>,
    pub selectable: bool,
    /// Datastar expression run on a row click, given the row.
    pub on_click: Option<RowFn<'a, T, String>>,
    pub active: Option<i64>,
    pub empty: String,
    pub page_size: usize,
    pub row_class: Option<RowFn<'a, T, &'static str>>,
}

impl<'a, T> Table<'a, T> {
    pub fn new(
        ns: &'a str,
        reload: &'a str,
        key: impl Fn(&T) -> i64 + 'a,
        cols: Vec<Col<'a, T>>,
    ) -> Self {
        Table {
            ns,
            reload,
            cols,
            key: Box::new(key),
            selectable: false,
            on_click: None,
            active: None,
            empty: "No data".into(),
            page_size: 200,
            row_class: None,
        }
    }

    /// Filter, sort, and page `rows` per `state`, and render.
    pub fn render(&self, rows: &[T], state: &TableState) -> Markup {
        let ns = self.ns;
        let q = state.filter.trim().to_lowercase();
        let mut view: Vec<&T> = rows
            .iter()
            .filter(|r| {
                q.is_empty()
                    || self
                        .cols
                        .iter()
                        .any(|c| (c.text)(r).to_lowercase().contains(&q))
            })
            .collect();
        if let Some(sort) = self
            .cols
            .iter()
            .find(|c| c.key == state.sort)
            .and_then(|c| c.sort.as_ref())
        {
            let mut keyed: Vec<(SortKey, &T)> = view.iter().map(|r| (sort(r), *r)).collect();
            keyed.sort_by(|a, b| {
                let o = a.0.cmp(&b.0);
                // Empties stay last either way.
                if state.desc && !matches!(a.0, SortKey::None) && !matches!(b.0, SortKey::None) {
                    o.reverse()
                } else {
                    o
                }
            });
            view = keyed.into_iter().map(|(_, r)| r).collect();
        }
        let total = view.len();
        let pages = total.div_ceil(self.page_size).max(1);
        let page = state.page.min(pages - 1);
        let shown =
            &view[page * self.page_size..(page * self.page_size + self.page_size).min(total)];
        let selected = state.sel.values().filter(|v| **v).count();
        let go = |p: usize| format!("${ns}.page = {p}; {}", self.reload);

        html! {
            div.table-wrap id=(ns) {
                div.table-scroll {
                    table {
                        colgroup {
                            @if self.selectable { col style="width:40px" {} }
                            @for c in &self.cols { col style=[c.width.map(|w| format!("width:{w}"))] {} }
                        }
                        thead { tr {
                            @if self.selectable {
                                th.check-col {
                                    input type="checkbox" aria-label="Select all"
                                        data-on:change=(format!("@setAll(el.checked, {{include: /^{ns}\\.sel\\./}})"));
                                }
                            }
                            @for c in &self.cols {
                                @let sorted = state.sort == c.key && !c.key.is_empty();
                                @if c.sort.is_some() {
                                    th.sortable aria-sort=[sorted.then_some(if state.desc { "descending" } else { "ascending" })]
                                        data-on:click=(format!(
                                            "{}; ${ns}.page = 0; {}",
                                            if !sorted { format!("${ns}.sort = {}; ${ns}.desc = false", js(c.key)) }
                                            else if !state.desc { format!("${ns}.desc = true") }
                                            else { format!("${ns}.sort = ''; ${ns}.desc = false") },
                                            self.reload)) {
                                        (c.label)
                                        @if sorted { span.arrow { (if state.desc { "▼" } else { "▲" }) } }
                                    }
                                } @else {
                                    th { (c.label) }
                                }
                            }
                        } }
                        tbody {
                            @for r in shown {
                                @let k = (self.key)(r);
                                @let click = self.on_click.as_ref().map(|f| f(r));
                                tr id=(format!("{ns}-r{k}"))
                                    class=(format!("{}{}{}",
                                        self.row_class.as_ref().map(|f| f(r)).unwrap_or(""),
                                        if click.is_some() { " clickable" } else { "" },
                                        if self.active == Some(k) { " active" } else { "" }))
                                    tabindex=[click.as_ref().map(|_| 0)]
                                    data-on:click=[click.clone()]
                                    data-on:keydown=[click.as_ref().map(|_| "evt.target === el && evt.key === 'Enter' && el.click()")] {
                                    @if self.selectable {
                                        td.check-col data-on:click="evt.stopPropagation()" {
                                            input type="checkbox" aria-label="Select row"
                                                data-bind=(format!("{ns}.sel.r{k}"));
                                        }
                                    }
                                    @for c in &self.cols {
                                        td.mono[c.mono] {
                                            @if let Some(cell) = &c.cell { (cell(r)) }
                                            @else {
                                                @let t = (c.text)(r);
                                                @if t.is_empty() { "—" } @else { (t) }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    @if total == 0 {
                        div.empty { (if rows.is_empty() || q.is_empty() { self.empty.as_str() } else { "No matching rows" }) }
                    }
                }
                div.table-footer {
                    span {
                        @if total != rows.len() { (total) " of " }
                        (rows.len()) (if rows.len() == 1 { " row" } else { " rows" })
                    }
                    @if self.selectable && selected > 0 { span { "· " (selected) " selected" } }
                    @if pages > 1 {
                        span.pager {
                            button.btn.btn-ghost.btn-sm disabled[page == 0] data-on:click=(go(page.saturating_sub(1))) { "‹ Prev" }
                            span { "Page " (page + 1) " of " (pages) }
                            button.btn.btn-ghost.btn-sm disabled[page + 1 >= pages] data-on:click=(go(page + 1)) { "Next ›" }
                        }
                    }
                }
            }
        }
    }
}

/// A search box bound to `$<ns>.filter` that reloads as you type. `/` focuses it.
pub fn filter_box(ns: &str, reload: &str, placeholder: &str) -> Markup {
    html! {
        div.search {
            (icon("search", 14))
            input.input.filter type="search" placeholder=(placeholder) aria-label=(placeholder)
                data-bind=(format!("{ns}.filter"))
                "data-on:input__debounce.250ms"=(format!("${ns}.page = 0; {reload}"));
        }
    }
}

/// Join a relative output dir to the workflow's submission directory (where logs are written),
/// so paths resolve regardless of torc-dash's working directory. Absolute paths (POSIX, `~/`,
/// Windows drive, UNC) are returned unchanged.
pub fn resolve_log_base_dir(output_dir: &str, submission_dir: Option<&str>) -> String {
    let d = if output_dir.trim().is_empty() {
        "torc_output"
    } else {
        output_dir.trim()
    };
    let bytes = d.as_bytes();
    let absolute = d.starts_with('/')
        || d == "~"
        || d.starts_with("~/")
        || d.starts_with("\\\\")
        || (bytes.len() > 2
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\'));
    match submission_dir.filter(|s| !s.is_empty()) {
        Some(sub) if !absolute => format!("{}/{d}", sub.trim_end_matches(['/', '\\'])),
        _ => d.to_string(),
    }
}

/// Row of summary numbers: (label, value, tone).
pub fn stats(items: &[(&str, i64, Option<Tone>)]) -> Markup {
    html! {
        div.stats {
            @for (label, value, tone) in items {
                div.stat {
                    div.value.(tone.map(Tone::class).unwrap_or("")) { (value) }
                    div.label { (label) }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_dir_resolution() {
        assert_eq!(
            resolve_log_base_dir("out", Some("/home/u/wf/")),
            "/home/u/wf/out"
        );
        assert_eq!(resolve_log_base_dir("/abs", Some("/x")), "/abs");
        assert_eq!(resolve_log_base_dir("~/o", Some("/x")), "~/o");
        assert_eq!(resolve_log_base_dir("C:\\o", Some("/x")), "C:\\o");
        assert_eq!(resolve_log_base_dir("", None), "torc_output");
    }

    #[test]
    fn natural_order() {
        assert!(natural_cmp("job2", "job10").is_lt());
        assert!(natural_cmp("Job1", "job1").is_eq());
        assert!(natural_cmp("a", "b").is_lt());
    }

    #[test]
    fn ui_events_are_sse() {
        let s = Ui::new()
            .patch(html! { div #x { "a" } })
            .signals(serde_json::json!({"a": 1}))
            .into_string();
        assert!(
            s.contains("event: datastar-patch-elements\ndata: elements <div id=\"x\">a</div>\n\n")
        );
        assert!(s.contains("event: datastar-patch-signals\ndata: signals {\"a\":1}\n\n"));
    }
}
