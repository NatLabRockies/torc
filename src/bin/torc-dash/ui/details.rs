//! Details tab: the selected workflow's summary header plus sub-tabs listing its jobs, results,
//! files, events, and so on. Also owns the job details modal (`GET /ui/job/{id}`, in
//! `details_job`), the file viewer (`GET /ui/file`, in `details_view`), and the edit dialogs for
//! jobs, resource requirements, and Slurm schedulers (`details_edit`).
//!
//! Tables are sorted, filtered, and paged on the server. Besides plain substring search, the
//! filter accepts `field<op>value` (see [`parse_operator`]); right-clicking a cell filters by its
//! value.

#[path = "details_edit.rs"]
mod edit;
#[path = "details_job.rs"]
mod job;
#[path = "details_view.rs"]
pub mod view;

use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Mutex},
};

use axum::{
    Router,
    extract::State,
    routing::{get, post},
};
use maud::{Markup, html};
use serde::Deserialize;
use serde_json::{Value, json};

use super::Ctx;
use super::kit::{
    self, Col, Sig, SortKey, Table, TableState, Tone, Ui, badge, filter_box, icon, js,
    selected_ids, status_badge,
};
use crate::AppState;

/// Re-render after a table interaction; big tables reuse their cached rows (see [`rows_for`]).
const RELOAD: &str = "@get('/ui/tab?auto=1')";

/// (id, label, list endpoint)
const SUBTABS: [(&str, &str, &str); 11] = [
    ("jobs", "Jobs", "/jobs"),
    ("running", "Running", ""),
    ("results", "Results", "/results"),
    ("events", "Events", "/events"),
    ("files", "Files", "/files"),
    ("user_data", "User Data", "/user_data"),
    ("resources", "Resources", "/resource_requirements"),
    ("schedulers", "Schedulers", "/slurm_schedulers"),
    ("compute_nodes", "Compute Nodes", "/compute_nodes"),
    (
        "scheduled_nodes",
        "Scheduled Nodes",
        "/scheduled_compute_nodes",
    ),
    ("slurm_stats", "Slurm Stats", "/slurm_stats"),
];

/// (sub-tab, label, filter, title)
const QUICK_FILTERS: [(&str, &str, &str, &str); 8] = [
    ("jobs", "Failed", "status=failed", "Show only failed jobs"),
    (
        "jobs",
        "Running",
        "status=running",
        "Show only running jobs",
    ),
    ("jobs", "Ready", "status=ready", "Show only ready jobs"),
    (
        "jobs",
        "Blocked",
        "status=blocked",
        "Show only blocked jobs",
    ),
    (
        "results",
        "Errors",
        "return_code!=0",
        "Results with a non-zero return code",
    ),
    (
        "results",
        "Success",
        "return_code=0",
        "Results with return code 0",
    ),
    ("results", "Failed", "status=failed", "Failed results"),
    ("events", "Errors", "error", "Events mentioning errors"),
];

/// Auto-refresh (and sorting/paging) reuses the cached rows of tables bigger than this; a manual
/// refresh always reloads.
// ponytail: whole-table reload; fetch only changed rows if large tables need live updates.
const AUTO_REFRESH_MAX_ROWS: usize = 10_000;

const RERUN_MESSAGE: &str = "Resets the selected jobs to uninitialized and reinitializes the workflow, which also \
    resets the jobs that depend on them. Then run or submit the workflow to execute them.";

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ui/job/{id}", get(job::job_modal))
        .route("/ui/details/job/{id}/rerun", post(job::rerun_one))
        .route("/ui/details/job/{id}/edit", get(edit::edit_job))
        .route("/ui/details/job/{id}/pick", post(edit::pick_resources))
        .route("/ui/details/job/{id}/save", post(edit::save_job))
        .route("/ui/details/def/{kind}/{id}", get(edit::edit_definition))
        .route(
            "/ui/details/def/{kind}/{id}/save",
            post(edit::save_definition),
        )
        .route("/ui/details/rerun", post(rerun_selected))
        .route("/ui/file", get(view::file_viewer))
        .route("/ui/details/slurm-logs/{wf}/{sid}", get(view::slurm_logs))
}

/// The `$details` signals shared by this tab and its modals. Sub-tab table states
/// (`$details.jobs`, …) are read from the raw signals.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct DetailsNs {
    pub sub: String,
    /// Selected event (Events sub-tab).
    #[serde(deserialize_with = "kit::lenient_id")]
    pub event: Option<i64>,
    /// Viewers: find text and log stream (stdout/stderr).
    pub q: String,
    pub stream: String,
    /// Job details modal: sub-tab and the result (run) index whose logs are shown.
    pub jtab: String,
    #[serde(deserialize_with = "kit::lenient_id")]
    pub run: Option<i64>,
    /// Edit dialogs' form values.
    pub ej: Value,
    pub def: Value,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct DetailsSig {
    pub details: DetailsNs,
}

// ============== Value helpers ==============

/// A field as display text ("" when missing or null).
pub fn text(v: &Value, k: &str) -> String {
    match v.get(k) {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(o) => o.to_string(),
    }
}

pub fn int(v: &Value, k: &str) -> Option<i64> {
    v.get(k).and_then(Value::as_i64)
}

pub fn float(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(Value::as_f64)
}

/// 12345 → "12,345".
pub fn thousands(n: i64) -> String {
    let s = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

pub fn fixed(v: Option<f64>, digits: usize) -> String {
    v.map(|v| format!("{v:.digits$}"))
        .unwrap_or_else(|| "—".into())
}

fn percent(v: Option<f64>) -> String {
    v.filter(|v| v.is_finite())
        .map(|v| format!("{v:.1}%"))
        .unwrap_or_else(|| "—".into())
}

fn positive(v: Option<f64>) -> Option<f64> {
    v.filter(|v| *v > 0.0)
}

fn elapsed(start: &str) -> String {
    match kit::to_datetime(start) {
        Some(d) => kit::fmt_duration(Some(
            (chrono::Local::now() - d).num_milliseconds() as f64 / 1000.0,
        )),
        None => "—".into(),
    }
}

fn time_key(v: &Value, k: &str) -> SortKey {
    let t = text(v, k);
    kit::to_datetime(&t)
        .map(|d| SortKey::Num(d.timestamp_millis() as f64))
        .unwrap_or(SortKey::None)
}

/// Green 0 / red non-zero return code.
pub fn return_code(v: &Value) -> Markup {
    let rc = int(v, "return_code");
    html! { span.rc.ok[rc == Some(0)].bad[rc != Some(0)] { (text(v, "return_code")) } }
}

/// Best one-line summary of an event's data.
fn event_message(data: &Value) -> String {
    let s = |k: &str| data.get(k).filter(|v| !v.is_null() && *v != "");
    match data {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Object(_) => {
            if let Some(Value::String(m)) = data.get("message") {
                m.clone()
            } else if s("job_name").is_some() {
                format!("Job: {}", text(data, "job_name"))
            } else if s("action").is_some() {
                match s("user") {
                    Some(_) => format!("{} by {}", text(data, "action"), text(data, "user")),
                    None => text(data, "action"),
                }
            } else if s("category").is_some() && s("type").is_some() {
                format!("{}: {}", text(data, "category"), text(data, "type"))
            } else {
                data.to_string()
            }
        }
        o => o.to_string(),
    }
}

// ============== Operator filter ==============

/// A `field<op>value` filter (ops `= != > < >= <= ~ :`, where `~`/`:` mean contains).
#[derive(Debug, PartialEq)]
pub struct OperatorFilter {
    field: String,
    op: &'static str,
    value: String,
}

/// Parse the filter text as `field<op>value`; None means plain substring search.
pub fn parse_operator(text: &str) -> Option<OperatorFilter> {
    let t = text.trim().to_lowercase();
    let end = t
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|&e| e > 0)?;
    let field = &t[..end];
    let rest = t[end..].trim_start();
    let op = ["!=", ">=", "<=", ">", "<", "=", "~", ":"]
        .into_iter()
        .find(|o| rest.starts_with(o))?;
    let value = rest[op.len()..].trim_start();
    if value.is_empty() {
        return None;
    }
    let field = match field {
        "exec_time" => "exec_time_minutes",
        "peak_mem" => "peak_memory_bytes",
        "avg_cpu" => "avg_cpu_percent",
        "modified" => "st_mtime",
        f => f,
    };
    Some(OperatorFilter {
        field: field.into(),
        op,
        value: value.into(),
    })
}

/// The whole string as a finite number (unlike a prefix parse: "1_train" is not 1).
fn to_number(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|n| n.is_finite())
}

pub fn matches_operator(row: &Value, f: &OperatorFilter) -> bool {
    let raw = row.get(&f.field).unwrap_or(&Value::Null);
    let s = text(row, &f.field).to_lowercase();
    let v = f.value.as_str();

    if f.field == "status" {
        // Statuses display with spaces ("pending failed"), so a value copied from a cell works.
        let v = v.replace(' ', "_");
        return match f.op {
            "=" => s == v,
            "!=" => s != v,
            "~" | ":" => s.contains(&v),
            _ => true,
        };
    }

    // Numeric only when both sides are entirely numbers.
    let item_num = raw.as_f64().or_else(|| to_number(&s));
    if let (Some(n), Some(i)) = (to_number(v), item_num) {
        match f.op {
            "=" => return i == n,
            "!=" => return i != n,
            ">" => return i > n,
            "<" => return i < n,
            ">=" => return i >= n,
            "<=" => return i <= n,
            _ => {}
        }
    }
    match f.op {
        "=" => s == v,
        "!=" => s != v,
        _ => s.contains(v),
    }
}

// ============== Rows ==============

/// Last rows per sub-tab of one workflow.
type Rows = Arc<Vec<Value>>;
static CACHE: LazyLock<Mutex<(i64, HashMap<String, Rows>)>> =
    LazyLock::new(|| Mutex::new((0, HashMap::new())));

/// Rows for a sub-tab. An automatic refresh or a table interaction reuses big cached tables.
async fn rows_for(
    st: &Arc<AppState>,
    sub: &str,
    wf: i64,
    reuse: bool,
) -> Result<Arc<Vec<Value>>, String> {
    let cached = {
        let c = CACHE.lock().unwrap();
        (c.0 == wf).then(|| c.1.get(sub).cloned()).flatten()
    };
    if reuse && let Some(rows) = cached.filter(|r| r.len() > AUTO_REFRESH_MAX_ROWS) {
        return Ok(rows);
    }
    let rows = Arc::new(load_rows(st, sub, wf).await?);
    let mut c = CACHE.lock().unwrap();
    if c.0 != wf {
        *c = (wf, HashMap::new());
    }
    c.1.insert(sub.to_string(), rows.clone());
    Ok(rows)
}

async fn load_rows(st: &Arc<AppState>, sub: &str, wf: i64) -> Result<Vec<Value>, String> {
    let q = [("workflow_id", wf.to_string())];
    match sub {
        "running" => kit::list_all(st, &format!("/workflows/{wf}/running_jobs"), &[]).await,
        "slurm_stats" => {
            // CPU% = ave_cpu_seconds / exec time, joined from the results of every run.
            let all_runs = [("workflow_id", wf.to_string()), ("all_runs", "true".into())];
            let (stats, results) = futures::join!(
                kit::list_all::<Value>(st, "/slurm_stats", &q),
                kit::list_all::<Value>(st, "/results", &all_runs),
            );
            let mut stats = stats?;
            let key = |v: &Value, attempt: i64| {
                (
                    int(v, "job_id"),
                    int(v, "run_id"),
                    int(v, "attempt_id").unwrap_or(attempt),
                )
            };
            let minutes: HashMap<_, f64> = results
                .unwrap_or_default()
                .iter()
                .filter_map(|r| Some((key(r, 1), float(r, "exec_time_minutes")?)))
                .collect();
            for s in &mut stats {
                let cpu = match (
                    float(s, "ave_cpu_seconds").filter(|c| *c != 0.0),
                    minutes.get(&key(s, 0)).filter(|m| **m != 0.0),
                ) {
                    (Some(c), Some(m)) => json!(c / (m * 60.0) * 100.0),
                    _ => Value::Null,
                };
                s["cpu_percent"] = cpu;
            }
            Ok(stats)
        }
        _ => {
            let path = SUBTABS
                .iter()
                .find(|t| t.0 == sub)
                .map(|t| t.2)
                .unwrap_or("/jobs");
            kit::list_all(st, path, &q).await
        }
    }
}

/// Row counts for the sub-tabs not covered by the workflow status (one tiny request each).
async fn load_counts(st: &Arc<AppState>, wf: i64) -> HashMap<&'static str, i64> {
    let q = [
        ("workflow_id", wf.to_string()),
        ("offset", "0".into()),
        ("limit", "1".into()),
    ];
    let calls = SUBTABS.iter().filter(|t| !t.2.is_empty() && t.0 != "jobs");
    let res = futures::future::join_all(calls.map(|t| {
        let q = &q;
        async move {
            let v: Result<Value, String> = kit::get(st, t.2, q).await;
            (t.0, v.ok().and_then(|v| int(&v, "total_count")))
        }
    }))
    .await;
    res.into_iter()
        .filter_map(|(k, n)| n.map(|n| (k, n)))
        .collect()
}

// ============== View ==============

fn table_defaults() -> Value {
    let mut d = serde_json::Map::new();
    d.insert("sub".into(), json!("jobs"));
    d.insert("event".into(), json!(0));
    for (id, ..) in SUBTABS {
        let sort = if id == "running" { "start_time" } else { "" };
        d.insert(
            id.into(),
            json!({"filter": "", "sort": sort, "desc": false, "page": 0, "sel": {}}),
        );
    }
    json!({ "details": d })
}

pub async fn view(st: &Arc<AppState>, cx: &Ctx) -> Markup {
    let ns: DetailsNs = kit::ns(&cx.sig, "details");
    let sub = SUBTABS
        .iter()
        .map(|t| t.0)
        .find(|t| *t == ns.sub)
        .unwrap_or("jobs");
    let mut state: TableState = cx
        .sig
        .get("details")
        .and_then(|d| d.get(sub))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_else(|| TableState {
            sort: if sub == "running" {
                "start_time".into()
            } else {
                String::new()
            },
            ..Default::default()
        });
    // Selections from another workflow are cleared by the guard below; don't count them.
    let sel_wf = cx.sig.pointer("/details/selWf").and_then(Value::as_i64);
    if sel_wf != cx.g.wf {
        state.sel.clear();
    }

    let body = match cx.g.wf {
        None => html! { div.empty { "Pick a workflow in the top bar to see its details." } },
        Some(wf) => {
            let (wf_path, status_path) = (
                format!("/workflows/{wf}"),
                format!("/workflows/{wf}/status"),
            );
            let (workflow, status, counts, rows, running) = futures::join!(
                kit::get::<Value>(st, &wf_path, &[]),
                kit::get::<Value>(st, &status_path, &[]),
                load_counts(st, wf),
                rows_for(st, sub, wf, cx.auto),
                super::workflows::local_runs(st),
            );
            let status = status.ok();
            let header = match &workflow {
                Ok(w) => summary(w, status.as_ref(), running.contains(&wf)),
                Err(e) => html! { div.error-text { "Could not load workflow " (wf) ": " (e) } },
            };
            let st_count = |k: &str| {
                status
                    .as_ref()
                    .and_then(|s| s.get("jobs_by_status"))
                    .and_then(|j| int(j, k))
            };
            let tabs: Vec<(&str, &str, Option<usize>)> = SUBTABS
                .iter()
                .map(|(id, label, _)| {
                    let n = match *id {
                        "jobs" => status.as_ref().and_then(|s| int(s, "total_jobs")),
                        "running" => st_count("running"),
                        _ => counts.get(id).copied(),
                    };
                    (*id, *label, n.map(|n| n as usize))
                })
                .collect();
            html! {
                (header)
                (kit::seg_tabs("details.sub", sub, &tabs, "@get('/ui/tab')"))
                (sub_tab(sub, wf, &state, &ns, rows))
            }
        }
    };

    html! {
        div.page.details data-signals__ifmissing=(table_defaults()) {
            // Job selections belong to one workflow: clear them when another is picked.
            @if let Some(wf) = cx.g.wf {
                div #details-sel-guard hidden data-init=(format!(
                    "$details.selWf !== {wf} && ($details.selWf = {wf}, @setAll(false, {{include: /^details\\.jobs\\.sel\\./}}))"
                )) {}
            }
            (body)
        }
    }
}

/// Header: workflow identity, actions, and a clickable job-status breakdown.
fn summary(w: &Value, status: Option<&Value>, running: bool) -> Markup {
    let id = int(w, "id").unwrap_or_default();
    let total = status.and_then(|s| int(s, "total_jobs")).unwrap_or(0);
    let counts: Vec<(&str, i64)> = kit::JOB_STATUSES
        .iter()
        .map(|s| {
            (
                *s,
                status
                    .and_then(|st| st.get("jobs_by_status"))
                    .and_then(|j| int(j, s))
                    .unwrap_or(0),
            )
        })
        .filter(|c| c.1 > 0)
        .collect();
    let color = |s: &str| match kit::status_tone(s) {
        Tone::Neutral => "var(--border-strong)",
        Tone::Info => "var(--info)",
        Tone::Accent => "var(--accent)",
        Tone::Success => "var(--success)",
        Tone::Warning => "var(--warning)",
        Tone::Danger => "var(--danger)",
    };
    let flag = |k: &str| status.and_then(|s| s.get(k)).and_then(Value::as_bool) == Some(true);
    let exec_min = status
        .and_then(|s| float(s, "total_exec_time_minutes"))
        .unwrap_or(0.0);
    let walltime = status.and_then(|s| float(s, "walltime_seconds"));
    let project = text(w, "project");
    let description = text(w, "description");
    html! {
        header.card.wf-summary {
            div.top {
                div.title.grow {
                    h2 { (text(w, "name")) }
                    div.meta.muted {
                        span { "#" (id) }
                        span { (text(w, "user")) }
                        @if !project.is_empty() { span { (project) } }
                        span title="Created" { (kit::fmt_datetime(Some(&text(w, "timestamp")))) }
                        @if flag("is_complete") { span.done { "complete" } }
                        @if flag("is_canceled") { span.canceled { "canceled" } }
                    }
                    @if !description.is_empty() { p.desc { (description) } }
                }
                div.toolbar {
                    button.btn.btn-sm title="Show the job graph" data-on:click="location.hash = '/dag'" {
                        (icon("graph", 14)) " Graph"
                    }
                    (super::workflows::actions(id, false, w.get("is_archived").and_then(Value::as_bool) == Some(true), running))
                }
            }
            @if total > 0 {
                div.bar role="img" aria-label="Job status breakdown" {
                    @for (s, n) in &counts {
                        span style=(format!("flex-grow:{n};background:{}", color(s))) title=(format!("{}: {n}", kit::status_label(s))) {}
                    }
                }
                div.legend {
                    span.muted { (thousands(total)) " jobs" }
                    @for (s, n) in &counts {
                        button.chip title=(format!("Show {} jobs", kit::status_label(s)))
                            data-on:click=(format!("$details.jobs.filter = 'status={s}'; $details.jobs.page = 0; $details.sub = 'jobs'; {RELOAD}")) {
                            i style=(format!("background:{}", color(s))) {}
                            (kit::status_label(s)) " " b { (thousands(*n)) }
                        }
                    }
                    @if exec_min > 0.0 {
                        span.muted.right {
                            "Exec time " (kit::fmt_duration(Some(exec_min * 60.0)))
                            @if let Some(w) = walltime.filter(|w| *w > 0.0) { " · walltime " (kit::fmt_duration(Some(w))) }
                        }
                    }
                }
            }
        }
    }
}

fn sub_tab(
    sub: &str,
    wf: i64,
    state: &TableState,
    ns: &DetailsNs,
    rows: Result<Arc<Vec<Value>>, String>,
) -> Markup {
    let tns = format!("details.{sub}");
    let (rows, error) = match rows {
        Ok(r) => (r, None),
        Err(e) => (Arc::new(Vec::new()), Some(e)),
    };
    // Operator filters are applied here; plain text is left to the table's own search.
    let op = parse_operator(&state.filter);
    let filtered: Vec<Value>;
    let shown: &[Value] = match &op {
        Some(f) => {
            filtered = rows
                .iter()
                .filter(|r| matches_operator(r, f))
                .cloned()
                .collect();
            &filtered
        }
        None => &rows,
    };
    let mut tstate = state.clone();
    if op.is_some() {
        tstate.filter.clear();
    }

    let set_filter = |f: &str| format!("${tns}.filter = {}; ${tns}.page = 0; {RELOAD}", js(f));
    let sel_count = "Object.values($details.jobs.sel || {}).filter(Boolean).length";
    let toolbar = html! {
        div.toolbar {
            (filter_box(&tns, RELOAD, "Filter…  e.g. name:work  status=failed  id>5  (press /)"))
            @if !state.filter.is_empty() {
                button.btn.btn-sm.btn-ghost data-on:click=(set_filter("")) { "Clear" }
            }
            @for (_, label, f, title) in QUICK_FILTERS.iter().filter(|q| q.0 == sub) {
                @let on = state.filter == *f;
                button.btn.btn-sm.on[on] title=(title) data-on:click=(set_filter(if on { "" } else { f })) { (label) }
            }
            @if sub == "resources" {
                button.btn.btn-sm data-on:click="@get('/ui/details/def/resources/new')" { "Add resource requirements…" }
            } @else if sub == "schedulers" {
                button.btn.btn-sm data-on:click="@get('/ui/details/def/scheduler/new')" { "Add Slurm scheduler…" }
            }
            @if sub == "jobs" {
                span.selection data-show=(format!("{sel_count} > 0")) style="display:none" {
                    span data-text=(format!("{sel_count}.toLocaleString() + ' selected'")) {}
                    button.btn.btn-sm.btn-primary data-on:click=(format!(
                        "el._n = {sel_count}, confirm('Reset ' + (el._n === 1 ? '1 job' : el._n + ' jobs') + ' for rerun?\\n\\n' + {}) && @post('/ui/details/rerun')",
                        js(RERUN_MESSAGE))) { "Reset for rerun" }
                    button.btn.btn-sm.btn-ghost data-on:click="@setAll(false, {include: /^details\\.jobs\\.sel\\./})" { "Clear" }
                }
            }
            @if let Some(e) = &error { span.error-text { "Error loading " (sub.replace('_', " ")) ": " (e) } }
        }
    };

    let mut table = Table::new(&tns, RELOAD, row_key, columns(sub, wf));
    table.empty = empty_text(sub).into();
    let open_job = |k: &'static str| -> Box<dyn Fn(&Value) -> String> {
        Box::new(move |v: &Value| format!("@get('/ui/job/{}')", int(v, k).unwrap_or_default()))
    };
    match sub {
        "jobs" => {
            table.selectable = true;
            table.on_click = Some(open_job("id"));
        }
        "running" | "results" | "slurm_stats" => {
            table.on_click = Some(open_job("job_id"));
        }
        "events" => {
            table.active = ns.event;
            table.on_click = Some(Box::new(|v: &Value| {
                format!(
                    "$details.event = {}; {RELOAD}",
                    int(v, "id").unwrap_or_default()
                )
            }));
        }
        "files" => {
            table.on_click = Some(Box::new(|v: &Value| {
                format!(
                    "@get({})",
                    js(&view::file_url(
                        &text(v, "path"),
                        Some(&text(v, "name")),
                        None
                    ))
                )
            }));
        }
        "resources" => {
            table.on_click = Some(Box::new(|v: &Value| {
                format!(
                    "@get('/ui/details/def/resources/{}')",
                    int(v, "id").unwrap_or_default()
                )
            }));
        }
        "schedulers" => {
            table.on_click = Some(Box::new(|v: &Value| {
                format!(
                    "@get('/ui/details/def/scheduler/{}')",
                    int(v, "id").unwrap_or_default()
                )
            }));
        }
        "scheduled_nodes" => {
            table.on_click = Some(Box::new(move |v: &Value| {
                if is_slurm(v) {
                    slurm_logs_action(wf, v)
                } else {
                    String::new()
                }
            }));
        }
        _ => {}
    }
    // Columns whose cell shows the raw value can be filtered by right-clicking a cell.
    let mut cols: Vec<&str> = table
        .cols
        .iter()
        .map(|c| {
            if c.cell.is_none() || ["status", "return_code"].contains(&c.key) {
                c.key
            } else {
                ""
            }
        })
        .collect();
    if table.selectable {
        cols.insert(0, "");
    }
    let cell_filter = format!(
        "el._c = evt.target.closest('td'), el._k = el._c && el.dataset.cols.split(',')[el._c.cellIndex], \
         el._v = el._c && el._c.textContent.trim(), \
         el._k && el._v && el._v !== '—' && (evt.preventDefault(), ${tns}.filter = el._k + '=' + el._v, ${tns}.page = 0, {RELOAD})"
    );
    let rendered = table.render(shown, &tstate);
    let host = html! {
        div.table-host data-cols=(cols.join(",")) data-on:contextmenu=(cell_filter) { (rendered) }
    };

    html! {
        (toolbar)
        @if op.is_some() && shown.len() != rows.len() {
            div.hint.faint { (thousands(shown.len() as i64)) " of " (thousands(rows.len() as i64)) " rows match " code { (state.filter.trim()) } }
        }
        @if sub == "events" {
            div.split {
                div.grow { (host) }
                aside.preview.card {
                    header { "Event data" }
                    @match ns.event.and_then(|id| rows.iter().find(|e| int(e, "id") == Some(id))) {
                        Some(e) => (view::json_view(&serde_json::to_string_pretty(e.get("data").unwrap_or(&Value::Null)).unwrap_or_default())),
                        None => div.empty { "Select an event to see its data" },
                    }
                }
            }
        } @else {
            (host)
        }
        div.hint.faint.cell-hint { "Right-click a cell to filter by its value" }
    }
}

fn row_key(v: &Value) -> i64 {
    int(v, "id")
        .or_else(|| int(v, "job_id"))
        .unwrap_or_default()
}

fn empty_text(sub: &str) -> &'static str {
    match sub {
        "jobs" => "No jobs in this workflow",
        "running" => "No running jobs",
        "results" => "No results in this workflow",
        "events" => "No events in this workflow",
        "files" => "No files in this workflow",
        "user_data" => "No user data in this workflow",
        "resources" => "No resource requirements in this workflow",
        "schedulers" => "No Slurm schedulers configured for this workflow",
        "compute_nodes" => "No compute nodes in this workflow",
        "scheduled_nodes" => "No scheduled compute nodes in this workflow",
        _ => "No Slurm stats in this workflow",
    }
}

fn is_slurm(v: &Value) -> bool {
    text(v, "scheduler_type").eq_ignore_ascii_case("slurm")
}

fn slurm_logs_action(wf: i64, v: &Value) -> String {
    format!(
        "@get('/ui/details/slurm-logs/{wf}/{}')",
        text(v, "scheduler_id")
    )
}

fn node_tone(status: &str) -> Tone {
    match status.to_lowercase().as_str() {
        "running" | "active" => Tone::Accent,
        "pending" | "scheduled" => Tone::Info,
        "completed" | "done" | "complete" => Tone::Success,
        "failed" | "error" => Tone::Danger,
        _ => Tone::Neutral,
    }
}

type C = Col<'static, Value>;

fn t(key: &'static str, label: &'static str) -> C {
    Col::text(key, label, move |v: &Value| text(v, key))
}

fn n(key: &'static str, label: &'static str) -> C {
    Col::num(key, label, move |v: &Value| float(v, key))
}

fn id_col(key: &'static str, label: &'static str, width: &'static str) -> C {
    n(key, label).width(width)
}

fn status_col(width: &'static str) -> C {
    t("status", "Status")
        .width(width)
        .cell(|v| status_badge(&text(v, "status")))
}

fn time_col(key: &'static str, label: &'static str, width: &'static str) -> C {
    t(key, label)
        .width(width)
        .sort_by(move |v| time_key(v, key))
        .cell(move |v| html! { (kit::fmt_datetime(Some(&text(v, key)))) })
}

fn bytes_col(key: &'static str, label: &'static str, positive_only: bool) -> C {
    let get = move |v: &Value| {
        let b = float(v, key);
        if positive_only { positive(b) } else { b }
    };
    Col::num(key, label, get).cell(move |v| html! { (kit::fmt_bytes(get(v))) })
}

fn json_col(key: &'static str, label: &'static str) -> C {
    Col::text(key, label, move |v: &Value| {
        v.get(key).map(Value::to_string).unwrap_or_default()
    })
    .mono()
}

fn columns(sub: &str, wf: i64) -> Vec<C> {
    match sub {
        "jobs" => vec![
            id_col("id", "ID", "80px"),
            t("name", "Name").width("22%"),
            status_col("130px"),
            id_col("compute_node_id", "Node", "70px"),
            Col::text("start_time", "Elapsed", |v: &Value| {
                if text(v, "status") == "running" {
                    text(v, "start_time")
                } else {
                    String::new()
                }
            })
            .width("100px")
            .mono()
            .sort_by(|v| {
                if text(v, "status") == "running" {
                    time_key(v, "start_time")
                } else {
                    SortKey::None
                }
            })
            .cell(|v| {
                html! { @if text(v, "status") == "running" { (elapsed(&text(v, "start_time"))) } @else { "—" } }
            }),
            t("command", "Command").mono(),
        ],
        "running" => vec![
            id_col("job_id", "Job ID", "80px"),
            t("job_name", "Name"),
            t("compute_node_name", "Compute Node"),
            t("start_time", "Elapsed")
                .width("110px")
                .mono()
                .sort_by(|v| time_key(v, "start_time"))
                .cell(|v| html! { (elapsed(&text(v, "start_time"))) }),
            t("scheduler_type", "Scheduler").width("110px"),
            t("scheduler_job_id", "Scheduler Job ID")
                .width("150px")
                .mono(),
        ],
        "results" => vec![
            id_col("job_id", "Job ID", "80px"),
            t("job_name", "Job Name"),
            n("run_id", "Run").width("60px"),
            Col::num("attempt_id", "Attempt", |v: &Value| {
                Some(float(v, "attempt_id").unwrap_or(1.0))
            })
            .width("75px")
            .cell(|v| html! { (int(v, "attempt_id").unwrap_or(1)) }),
            n("return_code", "Return").width("75px").cell(return_code),
            status_col("130px"),
            n("exec_time_minutes", "Exec Time (min)")
                .width("130px")
                .cell(|v| html! { (fixed(float(v, "exec_time_minutes"), 2)) }),
            bytes_col("peak_memory_bytes", "Peak Mem", false).width("100px"),
            n("peak_cpu_percent", "Peak CPU %")
                .width("100px")
                .cell(|v| html! { (fixed(float(v, "peak_cpu_percent"), 1)) }),
        ],
        "events" => vec![
            id_col("id", "ID", "70px"),
            time_col("timestamp", "Timestamp", "190px"),
            Col::text("message", "Message", |v: &Value| {
                event_message(v.get("data").unwrap_or(&Value::Null))
            })
            .cell(|v| html! { (event_message(v.get("data").unwrap_or(&Value::Null))) }),
            json_col("data", "Data"),
        ],
        "files" => vec![
            id_col("id", "ID", "80px"),
            t("name", "Name").width("25%"),
            t("path", "Path").mono(),
            time_col("st_mtime", "Modified", "190px"),
        ],
        "user_data" => vec![
            id_col("id", "ID", "80px"),
            t("name", "Name").width("25%"),
            json_col("data", "Data").cell(|v| html! { (v.get("data").map(Value::to_string).unwrap_or_default()) }),
        ],
        "resources" => vec![
            id_col("id", "ID", "80px"),
            t("name", "Name"),
            n("num_cpus", "CPUs").width("80px"),
            t("memory", "Memory").width("100px"),
            n("num_gpus", "GPUs").width("80px"),
            n("num_nodes", "Nodes").width("80px"),
            t("runtime", "Runtime").width("120px").mono(),
        ],
        "schedulers" => vec![
            id_col("id", "ID", "80px"),
            t("name", "Name"),
            t("account", "Account"),
            t("partition", "Partition"),
            t("qos", "QoS").width("100px"),
            t("walltime", "Walltime").width("110px").mono(),
            n("nodes", "Nodes").width("70px"),
            t("mem", "Mem").width("90px"),
            t("gres", "GRES").width("110px"),
        ],
        "compute_nodes" => vec![
            id_col("id", "ID", "70px"),
            t("hostname", "Hostname"),
            t("compute_node_type", "Type").width("80px"),
            n("num_cpus", "CPUs").width("70px"),
            n("memory_gb", "Mem (GB)").width("90px"),
            n("num_gpus", "GPUs").width("70px"),
            Col::text("is_active", "Active", |v: &Value| {
                match v.get("is_active").and_then(Value::as_bool) {
                    Some(true) => "Yes".into(),
                    Some(false) => "No".into(),
                    None => String::new(),
                }
            })
            .width("70px")
            .cell(|v| {
                html! { (match v.get("is_active").and_then(Value::as_bool) { Some(true) => "Yes", Some(false) => "No", None => "—" }) }
            }),
            time_col("start_time", "Started", "180px"),
            n("peak_cpu_percent", "CPU peak / avg").width("130px").cell(|v| {
                let (p, a) = (float(v, "peak_cpu_percent"), float(v, "avg_cpu_percent"));
                html! { @if p.is_some() && a.is_some() { (percent(p)) " / " (percent(a)) } @else { "—" } }
            }),
            n("peak_memory_bytes", "Mem peak / avg").width("150px").cell(|v| {
                let (p, a) = (float(v, "peak_memory_bytes"), float(v, "avg_memory_bytes"));
                html! { @if p.is_some() && a.is_some() { (kit::fmt_bytes(p)) " / " (kit::fmt_bytes(a)) } @else { "—" } }
            }),
        ],
        "scheduled_nodes" => vec![
            id_col("id", "ID", "80px"),
            t("scheduler_id", "Scheduler ID").mono(),
            t("scheduler_config_id", "Config ID").mono(),
            t("scheduler_type", "Type"),
            t("status", "Status").width("130px").cell(|v| {
                let s = text(v, "status");
                badge(node_tone(&s), &s)
            }),
            Col::text("logs", "Logs", |_: &Value| String::new())
                .width("110px")
                .unsortable()
                .cell(move |v| {
                    html! {
                        @if is_slurm(v) {
                            button.btn.btn-sm data-on:click=(format!("evt.stopPropagation(); {}", slurm_logs_action(wf, v))) { "View logs" }
                        } @else { span.faint { "—" } }
                    }
                }),
        ],
        _ => vec![
            id_col("job_id", "Job ID", "80px"),
            n("run_id", "Run").width("60px"),
            n("attempt_id", "Attempt").width("75px"),
            t("slurm_job_id", "Slurm Job").mono(),
            bytes_col("max_rss_bytes", "Max RSS", true),
            bytes_col("max_vm_size_bytes", "Max VM", true),
            Col::num("ave_cpu_seconds", "Ave CPU (s)", |v: &Value| {
                positive(float(v, "ave_cpu_seconds"))
            })
            .cell(|v| html! { (fixed(positive(float(v, "ave_cpu_seconds")), 1)) }),
            n("cpu_percent", "CPU %").cell(|v| html! { (percent(float(v, "cpu_percent"))) }),
            t("node_list", "Nodes"),
        ],
    }
}

// ============== Actions ==============

#[derive(Deserialize, Default)]
#[serde(default)]
struct RerunSig {
    #[serde(flatten)]
    g: kit::G,
    details: RerunNs,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RerunNs {
    jobs: TableState,
}

/// Reset the checked jobs (of the selected workflow) for rerun.
async fn rerun_selected(State(st): State<Arc<AppState>>, Sig(s): Sig<RerunSig>) -> Ui {
    let Some(wf) = s.g.wf else {
        return Ui::new();
    };
    let checked = selected_ids(&s.details.jobs.sel);
    // Only jobs that belong to this workflow, whatever stale selections the page holds.
    let ids: Vec<i64> = match job::jobs_by_ids(&st, wf, &checked).await {
        Ok(jobs) => jobs.iter().filter_map(|j| int(j, "id")).collect(),
        Err(e) => return Ui::new().err(format!("Reset failed: {e}")),
    };
    let clear: serde_json::Map<String, Value> = checked
        .iter()
        .map(|i| (format!("r{i}"), Value::Bool(false)))
        .collect();
    job::rerun(&st, &ids)
        .await
        .signals(json!({ "details": { "jobs": { "sel": clear } } }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operator_filters() {
        let f = parse_operator("Status = Failed").unwrap();
        assert_eq!(
            (f.field.as_str(), f.op, f.value.as_str()),
            ("status", "=", "failed")
        );
        assert!(parse_operator("plain text").is_none());
        assert_eq!(
            parse_operator("exec_time>=2").unwrap().field,
            "exec_time_minutes"
        );

        let row = json!({"id": 7, "name": "1_train", "status": "pending_failed", "rc": 0});
        let m = |f: &str| matches_operator(&row, &parse_operator(f).unwrap());
        assert!(m("id>5") && !m("id<5") && m("id=7") && m("id!=8"));
        assert!(m("status=pending failed") && m("status~pend") && !m("status!=pending_failed"));
        assert!(m("name=1_train") && !m("name=1") && m("name:train"));
        assert!(m("rc=0") && !m("rc!=0"));
    }

    #[test]
    fn number_format() {
        assert_eq!(thousands(1234567), "1,234,567");
        assert_eq!(thousands(12), "12");
        assert_eq!(thousands(-1000), "-1,000");
    }
}
