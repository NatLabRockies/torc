//! Debugging tab: job results with their stdout/stderr, live tail of running jobs' output,
//! scheduling diagnosis, resource checks and right-sizing, Slurm log analysis, and Slurm
//! accounting.
//!
//! Results that come from explicit actions (Slurm analysis, sacct, the right-sizing preview) and
//! the live-tail output are filled into `data-ignore-morph` slots keyed by workflow (or file), so
//! tab refreshes keep them and a workflow switch drops them.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Query, State},
    routing::{get, post},
};
use maud::{Markup, html};
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};
use torc::models::{ExecutionConfig, JobModel, ResultModel, WorkflowModel};

use super::Ctx;
use super::kit::{
    self, Col, G, Sig, Table, TableState, Tone, Ui, badge, cancel_button, filter_box, icon, js,
    modal, seg_tabs, status_badge,
};
use crate::AppState;

const RELOAD: &str = "@get('/ui/tab')";
/// Characters of live-tail output kept per pane.
const TAIL_MAX_CHARS: usize = 512 * 1024;
/// Find-in-log matches highlighted.
const MAX_MATCHES: usize = 10_000;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ui/debug/tail", post(tail))
        .route("/ui/debug/correct", post(correct_preview))
        .route("/ui/debug/correct/confirm", get(correct_confirm))
        .route("/ui/debug/correct/apply", post(correct_apply))
        .route("/ui/debug/slurm-logs", post(slurm_logs))
        .route("/ui/debug/sacct", post(sacct))
}

/// `$debug`: this tab's signals.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Dbg {
    view: String,
    failed_only: bool,
    all_runs: bool,
    /// Key of the selected job-log row.
    #[serde(deserialize_with = "kit::lenient_id")]
    sel: Option<i64>,
    stream: String,
    find: String,
    jobs: TableState,
    tail: TableState,
    running_only: bool,
    check: TableState,
    actions: TableState,
    include_failed: bool,
    show_all: bool,
    #[serde(deserialize_with = "lenient_f64")]
    mem: f64,
    #[serde(deserialize_with = "lenient_f64")]
    cpu: f64,
    #[serde(deserialize_with = "lenient_f64")]
    rt: f64,
    no_downsize: bool,
    errors_only: bool,
    /// Live tail: offsets and per-pane pause, keyed by pane id.
    /// (Values may be "": Datastar creates signals referenced before they are set.)
    off: HashMap<String, Value>,
    pp: HashMap<String, Value>,
}

/// A number from a number input (which may arrive as a string); 0 when unparsable.
fn lenient_f64<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) => s.trim().parse().unwrap_or(0.0),
        _ => 0.0,
    })
}

const INIT: &str = "{debug: {view: 'jobs', failedOnly: true, allRuns: false, sel: null, \
    stream: 'stdout', find: '', jobs: {filter: '', sort: '', desc: false, page: 0}, \
    tail: {filter: '', sort: '', desc: false, page: 0, sel: {}}, runningOnly: true, \
    check: {filter: '', sort: '', desc: false, page: 0}, \
    actions: {filter: '', sort: '', desc: false, page: 0}, includeFailed: false, showAll: false, \
    mem: 1.2, cpu: 1.2, rt: 1.2, noDownsize: false, errorsOnly: false, paused: false, off: {}, pp: {}}}";

/// Short stable hash for element ids.
fn hash(s: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    format!("{:08x}", h.finish() as u32)
}

fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

fn str_of<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

fn int_of(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}

/// `{base}/job_stdio/job_wf{wf}_j{job}_r{run}_a{attempt}` (append .o, .e, or .log).
fn stdio_base(base: &str, wf: i64, job: i64, run: i64, attempt: i64) -> String {
    format!("{base}/job_stdio/job_wf{wf}_j{job}_r{run}_a{attempt}")
}

/// Stdio mode of a job (wire name), honoring per-job overrides.
fn stdio_mode(wf: Option<&WorkflowModel>, job: &str) -> String {
    let ec = wf
        .map(ExecutionConfig::from_workflow_model)
        .unwrap_or_default();
    kit::wire_name(&ec.stdio_for_job(job).mode)
}

pub async fn view(st: &Arc<AppState>, cx: &Ctx) -> Markup {
    let d: Dbg = kit::ns(&cx.sig, "debug");
    let view = if d.view.is_empty() { "jobs" } else { &d.view };
    let tailed = kit::selected_ids(&d.tail.sel);
    let body = match cx.g.wf {
        None => html! { div.empty { "Pick a workflow in the top bar to debug its jobs." } },
        Some(id) => {
            let (body, count) = match view {
                "tail" => (tail_view(st, &cx.g, id, &d, &tailed).await, None),
                "scheduling" => (scheduling_view(st, id, &d).await, None),
                "resources" => (resource_check_view(st, id, &d).await, None),
                "slurm-logs" | "sacct" => (slurm_view(view, id), None),
                _ => {
                    let (m, n) = jobs_view(st, &cx.g, id, &d).await;
                    (m, n)
                }
            };
            let tail_count = (!tailed.is_empty()).then_some(tailed.len());
            html! {
                div.toolbar {
                    (seg_tabs("debug.view", view, &[
                        ("jobs", "Job logs", count),
                        ("tail", "Live tail", tail_count),
                        ("scheduling", "Scheduling", None),
                        ("resources", "Resource check", None),
                        ("slurm-logs", "Slurm log analysis", None),
                        ("sacct", "Slurm accounting", None),
                    ], RELOAD))
                    @if view == "jobs" || view == "tail" { div.grow {} }
                    @if view == "jobs" {
                        label.check { input type="checkbox" data-bind="debug.failedOnly" data-on:change=(RELOAD); "Failed only" }
                        label.check { input type="checkbox" data-bind="debug.allRuns" data-on:change=(RELOAD); "All runs" }
                        (filter_box("debug.jobs", RELOAD, "Filter…  /"))
                    }
                    @if view == "tail" { (filter_box("debug.tail", RELOAD, "Filter…  /")) }
                }
                (body)
            }
        }
    };
    html! {
        div.page.debug data-signals__ifmissing=(INIT) {
            div.page-header {
                h2 { "Debugging" }
                label.field.inline {
                    span { "Output dir" }
                    input.input.mono data-bind="outputDir" placeholder="torc_output" spellcheck="false"
                        data-on:change=(RELOAD);
                }
            }
            (body)
        }
    }
}

// ============== Job logs ==============

struct Row {
    key: i64,
    job: JobModel,
    result: Option<ResultModel>,
}

async fn load_wf(st: &AppState, id: i64) -> Result<WorkflowModel, String> {
    kit::get(st, &format!("/workflows/{id}"), &[]).await
}

async fn jobs_view(st: &Arc<AppState>, g: &G, id: i64, d: &Dbg) -> (Markup, Option<usize>) {
    let wid = [("workflow_id", id.to_string())];
    let mut rq = vec![("workflow_id", id.to_string())];
    if d.all_runs {
        rq.push(("all_runs", "true".into()));
    }
    let (wf, jobs, results) = tokio::join!(
        load_wf(st, id),
        kit::list_all::<JobModel>(st, "/jobs", &wid),
        kit::list_all::<ResultModel>(st, "/results", &rq),
    );
    let (jobs, results) = match (jobs, results) {
        (Ok(j), Ok(r)) => (j, r),
        (Err(e), _) | (_, Err(e)) => return (kit::error_box(&e), None),
    };
    let base = kit::resolve_log_base_dir(
        &g.output_dir,
        wf.ok().and_then(|w| w.submission_directory).as_deref(),
    );

    let mut by_job: HashMap<i64, Vec<ResultModel>> = HashMap::new();
    for r in results {
        by_job.entry(r.job_id).or_default().push(r);
    }
    let mut rows = Vec::new();
    for job in jobs {
        let jid = job.id.unwrap_or_default();
        let rs = by_job.remove(&jid).unwrap_or_default();
        if rs.is_empty() {
            if !d.failed_only {
                rows.push(Row {
                    key: -jid,
                    job,
                    result: None,
                });
            }
            continue;
        }
        let skip = if d.all_runs { 0 } else { rs.len() - 1 };
        for r in rs.into_iter().skip(skip) {
            if d.failed_only && r.return_code == 0 {
                continue;
            }
            rows.push(Row {
                key: r.id.unwrap_or_default(),
                job: job.clone(),
                result: Some(r),
            });
        }
    }
    // Keep a selection so the log viewer always has something useful to show.
    let selected = rows.iter().find(|r| Some(r.key) == d.sel).or(rows.first());

    // Await before building the table: its row closures aren't Send.
    let logs = match selected {
        None => html! { div.empty { "Select a job to view its logs." } },
        Some(Row {
            job, result: None, ..
        }) => {
            html! { div.empty { strong { (job.name) } " has no results yet, so there are no logs." } }
        }
        Some(Row {
            job,
            result: Some(r),
            ..
        }) => {
            let p = stdio_base(
                &base,
                r.workflow_id,
                r.job_id,
                r.run_id,
                r.attempt_id.unwrap_or(1),
            );
            html! {
                div.logs-head {
                    strong { (job.name) }
                    span.faint.mono { "#" (job.id.unwrap_or_default()) }
                }
                (log_viewer(&p, d).await)
            }
        }
    };
    let cols = vec![
        Col::num("id", "ID", |r: &Row| r.job.id.map(|i| i as f64)).width("70px"),
        Col::text("name", "Name", |r: &Row| r.job.name.clone()),
        Col::text("status", "Status", |r: &Row| row_status(r))
            .width("115px")
            .cell(|r| status_badge(&row_status(r))),
        Col::text("attempt", "Run/Att", |r: &Row| {
            r.result
                .as_ref()
                .map(|x| format!("{}/{}", x.run_id, x.attempt_id.unwrap_or(1)))
                .unwrap_or_default()
        })
        .width("75px")
        .mono(),
        Col::num("rc", "RC", |r: &Row| {
            r.result.as_ref().map(|x| x.return_code as f64)
        })
        .width("60px")
        .cell(|r| {
            let rc = r.result.as_ref().map(|x| x.return_code);
            html! { span.mono.bad[rc.is_some_and(|c| c != 0)] { (kit::or_dash(rc)) } }
        }),
        Col::text("open", "", |_: &Row| String::new())
            .width("90px")
            .unsortable()
            .cell(|r| {
                html! {
                    button.btn.btn-ghost.btn-sm
                        data-on:click=(format!("evt.stopPropagation(); @get('/ui/job/{}')", r.job.id.unwrap_or_default())) { "Details" }
                }
            }),
    ];
    let mut table = Table::new("debug.jobs", RELOAD, |r: &Row| r.key, cols);
    table.active = selected.map(|r| r.key);
    table.on_click = Some(Box::new(|r| format!("$debug.sel = {}; {RELOAD}", r.key)));
    table.empty = if d.failed_only {
        "No failed jobs. Uncheck “Failed only” to see all jobs.".into()
    } else {
        "No jobs in this workflow.".into()
    };

    let n = rows.len();
    (
        html! {
            div.dbg-split {
                div.pane { (table.render(&rows, &d.jobs)) }
                div.pane.dbg-logs.card { (logs) }
            }
        },
        Some(n),
    )
}

fn row_status(r: &Row) -> String {
    match &r.result {
        Some(x) => kit::wire_name(&x.status),
        None => r
            .job
            .status
            .as_ref()
            .map(kit::wire_name)
            .unwrap_or_default(),
    }
}

async fn read_file(path: &str) -> Value {
    kit::json_of(
        crate::cli_read_file_handler(Json(crate::ReadFileRequest {
            path: path.to_string(),
        }))
        .await,
    )
    .await
    .unwrap_or_else(|e| json!({ "success": false, "error": e, "exists": true }))
}

/// stdout/stderr of one result (`base` + .o/.e, or the combined .log), with find-in-log.
async fn log_viewer(base: &str, d: &Dbg) -> Markup {
    let stream = if d.stream == "stderr" {
        "stderr"
    } else {
        "stdout"
    };
    let path = format!("{base}.{}", if stream == "stderr" { "e" } else { "o" });
    // With stdio mode "combined", the runner writes one .log instead of .o and .e.
    let mut shown = path.clone();
    let mut r = read_file(&path).await;
    if r.get("exists") == Some(&Value::Bool(false)) {
        let alt = format!("{base}.log");
        let r2 = read_file(&alt).await;
        if r2.get("exists") != Some(&Value::Bool(false)) {
            (shown, r) = (alt, r2);
        }
    }
    let content = r
        .get("content")
        .and_then(Value::as_str)
        .filter(|c| !c.trim().is_empty());
    let (note, is_err) = if r.get("exists") == Some(&Value::Bool(false)) {
        (Some("File does not exist".to_string()), false)
    } else if r.get("success") != Some(&Value::Bool(true)) {
        (
            Some(str_of(&r, "error").to_string()).filter(|e| !e.is_empty()),
            true,
        )
    } else if content.is_none() {
        (Some("(empty)".to_string()), false)
    } else {
        (None, false)
    };
    let (body, matches) = content
        .map(|c| highlight(c, &d.find))
        .unwrap_or((html! {}, 0));
    let file_url = format!("/ui/file?path={}&title={stream}", enc(&shown));
    let step = |delta: i32| {
        format!(
            "(p => {{ if (!p) return; const ms = p.querySelectorAll('mark'); if (!ms.length) return; \
             p._i = ((p._i ?? -1) + {delta} + ms.length) % ms.length; \
             ms.forEach((m, i) => m.classList.toggle('cur', i === p._i)); \
             ms[p._i].scrollIntoView({{block: 'center', inline: 'nearest'}}); $_dbgCur = p._i + 1 }})(document.querySelector('.log-view'))"
        )
    };
    html! {
        div.log-bar {
            (seg_tabs("debug.stream", stream, &[("stdout", "stdout", None), ("stderr", "stderr", None)], RELOAD))
            code.path title=(shown) { (shown) }
            div.log-search {
                input.input type="search" placeholder="Find…" aria-label="Find in log" data-bind="debug.find"
                    "data-on:input__debounce.250ms"=(format!("$_dbgCur = 0; {RELOAD}"))
                    data-on:keydown=(format!(
                        "if (evt.key === 'Enter') {{ evt.preventDefault(); evt.shiftKey ? {} : {} }} \
                         else if (evt.key === 'Escape' && $debug.find) {{ evt.stopPropagation(); evt.preventDefault(); $debug.find = ''; $_dbgCur = 0; {RELOAD} }}",
                        step(-1), step(1)));
                @if !d.find.is_empty() {
                    // `$_dbgCur` may be left over from another file; show it only when it fits.
                    span.count data-text=(format!("($_dbgCur && $_dbgCur <= {matches} ? $_dbgCur + '/' : '') + '{matches}{}'", if matches >= MAX_MATCHES { "+" } else { "" })) { (matches) }
                    button.btn.btn-sm.btn-ghost.btn-icon title="Previous (Shift+Enter)" disabled[matches == 0] data-on:click=(step(-1)) { "↑" }
                    button.btn.btn-sm.btn-ghost.btn-icon title="Next (Enter)" disabled[matches == 0] data-on:click=(step(1)) { "↓" }
                }
            }
            button.btn.btn-sm title="Copy log contents" disabled[content.is_none()]
                data-on:click="navigator.clipboard.writeText(document.querySelector('.log-view')?.textContent ?? '').then(() => { el.textContent = 'Copied'; setTimeout(() => el.textContent = 'Copy', 1500) })" { "Copy" }
            button.btn.btn-sm.btn-ghost.btn-icon title="Open in file viewer" aria-label="Open in file viewer"
                data-on:click=(format!("@get({})", js(&file_url))) { (icon("external", 13)) }
        }
        @if content.is_some() {
            // Keyed by file, so a newly shown file starts scrolled to the end.
            pre.log-view.stderr[stream == "stderr"] id=(format!("dbg-log-{}", hash(&shown)))
                data-init="el.scrollTop = el.scrollHeight" { (body) }
        } @else {
            div.empty.log-note.error-text[is_err] { (note.unwrap_or_default()) }
        }
    }
}

/// `text` with case-insensitive (ASCII) matches of `q` wrapped in `<mark>`, and the match count.
fn highlight(text: &str, q: &str) -> (Markup, usize) {
    if q.is_empty() {
        return (html! { (text) }, 0);
    }
    // ASCII lowercasing keeps byte offsets aligned with `text`.
    let (hay, needle) = (text.to_ascii_lowercase(), q.to_ascii_lowercase());
    let mut parts = Vec::new();
    let mut last = 0;
    for (i, _) in hay.match_indices(&needle).take(MAX_MATCHES) {
        parts.push((&text[last..i], &text[i..i + needle.len()]));
        last = i + needle.len();
    }
    let n = parts.len();
    (
        html! { @for (before, m) in parts { (before) mark { (m) } } (&text[last..]) },
        n,
    )
}

// ============== Live tail ==============

async fn tail_view(st: &Arc<AppState>, g: &G, id: i64, d: &Dbg, tailed: &[i64]) -> Markup {
    let wid = [("workflow_id", id.to_string())];
    let (wf, jobs) = tokio::join!(
        load_wf(st, id),
        kit::list_all::<JobModel>(st, "/jobs", &wid),
    );
    let jobs = match jobs {
        Ok(j) => j,
        Err(e) => return kit::error_box(&e),
    };
    let wf = wf.ok();
    let base = kit::resolve_log_base_dir(
        &g.output_dir,
        wf.as_ref().and_then(|w| w.submission_directory.as_deref()),
    );
    let is_tailed = |j: &JobModel| tailed.contains(&j.id.unwrap_or_default());
    let rows: Vec<JobModel> = jobs
        .iter()
        .filter(|j| {
            !d.running_only
                || is_tailed(j)
                || j.status.as_ref().map(kit::wire_name).as_deref() == Some("running")
        })
        .cloned()
        .collect();
    let wf_ref = wf.as_ref();
    let cols = vec![
        Col::num("id", "ID", |j: &JobModel| j.id.map(|i| i as f64)).width("70px"),
        Col::text("name", "Name", |j: &JobModel| j.name.clone()),
        Col::text("status", "Status", |j: &JobModel| {
            j.status.as_ref().map(kit::wire_name).unwrap_or_default()
        })
        .width("115px")
        .cell(|j| status_badge(&j.status.as_ref().map(kit::wire_name).unwrap_or_default())),
        Col::text("stdio", "Stdio", move |j: &JobModel| {
            stdio_mode(wf_ref, &j.name)
        })
        .width("95px"),
    ];
    let mut table = Table::new(
        "debug.tail",
        RELOAD,
        |j: &JobModel| j.id.unwrap_or_default(),
        cols,
    );
    table.selectable = true;
    table.on_click = Some(Box::new(|j| {
        let k = format!("$debug.tail.sel.r{}", j.id.unwrap_or_default());
        format!("{k} = !{k}; {RELOAD}")
    }));
    table.empty = if d.running_only {
        "No running jobs. Uncheck “Running only” to pick any job.".into()
    } else {
        "No jobs in this workflow.".into()
    };

    // One pane per file each tailed job's current attempt writes.
    let panes: Vec<(&JobModel, Vec<(&str, String)>)> = match &wf {
        Some(w) => jobs
            .iter()
            .filter(|j| is_tailed(j))
            .map(|j| {
                let p = stdio_base(
                    &base,
                    id,
                    j.id.unwrap_or_default(),
                    w.run_id.unwrap_or(1),
                    j.attempt_id.unwrap_or(1),
                );
                let mode = stdio_mode(Some(w), &j.name);
                let mut files = Vec::new();
                if mode == "combined" {
                    files.push(("combined", format!("{p}.log")));
                }
                if mode == "separate" || mode == "no_stderr" {
                    files.push(("stdout", format!("{p}.o")));
                }
                if mode == "separate" || mode == "no_stdout" {
                    files.push(("stderr", format!("{p}.e")));
                }
                (j, files)
            })
            .collect(),
        None => Vec::new(),
    };
    let n = panes.len();
    let poll = "document.visibilityState === 'visible' && !$debug.paused && document.querySelector('.tail-pane') \
        && @post('/ui/debug/tail?panes=' + encodeURIComponent(JSON.stringify(Object.fromEntries(\
        [...document.querySelectorAll('.tail-pane')].map(p => [p.id, p.dataset.path])))))";
    html! {
        div.dbg-split.tail {
            div.side {
                div.toolbar {
                    label.check { input type="checkbox" data-bind="debug.runningOnly" data-on:change=(RELOAD); "Running only" }
                }
                div.table-host data-on:change=(RELOAD) { (table.render(&rows, &d.tail)) }
            }
            div.side {
                // Re-render every 5s so a job's retry (a new attempt, a new file) gets its pane.
                div.toolbar "data-on-interval__duration.5s"=[(n > 0).then_some("document.visibilityState === 'visible' && @get('/ui/tab?auto=1')")] {
                    span.faint { (n) " job" (if n == 1 { "" } else { "s" }) " tailed" }
                    div.grow {}
                    button.btn.btn-sm disabled[n == 0] data-on:click="$debug.paused = !$debug.paused" {
                        span data-show="!$debug.paused" { (icon("stop", 14)) " Pause all" }
                        span data-show="$debug.paused" style="display:none" { (icon("play", 14)) " Resume all" }
                    }
                    button.btn.btn-sm disabled[n == 0]
                        data-on:click="document.querySelectorAll('.tail-pane pre').forEach(p => p.replaceChildren())" {
                        (icon("trash", 14)) " Clear all"
                    }
                    button.btn.btn-sm disabled[n == 0]
                        data-on:click=(format!("@setAll(false, {{include: /^debug\\.tail\\.sel\\./}}); {RELOAD}")) {
                        (icon("x", 14)) " Close all"
                    }
                }
                @if n == 0 {
                    div.empty { "Check jobs on the left to follow their output as it is written." }
                } @else {
                    div.panes "data-on-interval__duration.1s"=(poll) {
                        @for (job, files) in &panes {
                            @let title = format!("{} #{}", job.name, job.id.unwrap_or_default());
                            @for (stream, path) in files {
                                (tail_pane(&title, stream, path))
                            }
                            @if files.is_empty() {
                                div.card.none {
                                    strong { (job.name) } " #" (job.id.unwrap_or_default())
                                    ": output is not captured (stdio mode " code { "none" } ")."
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One followed file. Its id is keyed by the path, so a new attempt's file gets a fresh pane; the
/// pane ignores tab re-renders, and `tail` appends to it.
fn tail_pane(title: &str, stream: &str, path: &str) -> Markup {
    let pid = format!("tp{}", hash(path));
    let paused = format!("$debug.pp.{pid}");
    html! {
        div.card.tail-pane id=(pid) data-path=(path) data-ignore-morph {
            div.head {
                strong.title title=(title) { (title) }
                span.tag.err[stream == "stderr"] { (stream) }
                span.faint data-show=(format!("$debug.paused || {paused}")) style="display:none" { "paused" }
                div.grow {}
                button.btn.btn-sm.btn-icon data-attr:disabled="$debug.paused"
                    data-attr:title=(format!("{paused} ? 'Resume' : 'Pause'"))
                    data-on:click=(format!("{paused} = !{paused}")) {
                    span data-show=(format!("!{paused}")) { (icon("stop", 14)) }
                    span data-show=(paused) style="display:none" { (icon("play", 14)) }
                }
                button.btn.btn-sm.btn-icon title="Clear" data-on:click=(format!("document.getElementById('{pid}-out').replaceChildren()")) {
                    (icon("trash", 14))
                }
            }
            code.path title=(path) { (path) }
            div.note id=(format!("{pid}-note")) { span { "Loading…" } }
            pre.code-block.out.stderr[stream == "stderr"] id=(format!("{pid}-out")) {}
        }
    }
}

#[derive(Deserialize)]
struct TailQuery {
    /// JSON object: pane id → path.
    panes: String,
}

/// Poll the live-tail panes: append what each file gained since its offset.
async fn tail(Query(q): Query<TailQuery>, Sig(sig): Sig<Value>) -> Ui {
    let d: Dbg = kit::ns(&sig, "debug");
    let panes: HashMap<String, String> = serde_json::from_str(&q.panes).unwrap_or_default();
    let mut ui = Ui::new();
    let mut offsets = serde_json::Map::new();
    for (pid, path) in panes {
        if d.pp.get(&pid) == Some(&Value::Bool(true)) || !pid.starts_with("tp") {
            continue;
        }
        let prev = d.off.get(&pid).and_then(Value::as_u64);
        let (note, err) = match crate::tail_file(&path, prev).await {
            Ok(r) => {
                offsets.insert(pid.clone(), json!(r.offset));
                if !r.content.is_empty() {
                    // Trim the oldest chunks past the cap, and follow the end unless scrolled up.
                    let init = format!(
                        "const p = el.parentNode; \
                         while (p.textContent.length > {TAIL_MAX_CHARS} && p.childElementCount > 1) p.firstElementChild.remove(); \
                         if (p.scrollHeight - el.offsetHeight - p.scrollTop - p.clientHeight < 24) p.scrollTop = p.scrollHeight"
                    );
                    ui = ui.append(
                        &format!("#{pid}-out"),
                        html! { span data-init=(init) { (r.content) } },
                    );
                }
                match r.exists {
                    true => (String::new(), false),
                    false if prev.is_some_and(|o| o > 0) => ("File no longer exists".into(), false),
                    false => ("Waiting for file…".into(), false),
                }
            }
            // Keep our offset so a fresh tail still starts near the end once readable.
            Err(e) => (format!("Failed to read file: {e}"), true),
        };
        // Morph patches are skipped inside the ignore-morph pane; append and drop the old note.
        ui = ui.append(
            &format!("#{pid}-note"),
            html! { span.error-text[err] data-init="while (el.previousSibling) el.previousSibling.remove()" { (note) } },
        );
    }
    if offsets.is_empty() {
        ui
    } else {
        ui.signals(json!({ "debug": { "off": offsets } }))
    }
}

// ============== Scheduling ==============

fn range(d: &Value) -> String {
    if d.is_null() {
        return "—".into();
    }
    let s = |k: &str| kit::fmt_duration(d.get(k).and_then(Value::as_f64));
    format!("{} – {}", s("min_seconds"), s("max_seconds"))
}

fn action_config(a: &Value) -> String {
    let c = a.get("action_config").cloned().unwrap_or(json!({}));
    let c = if c.is_null() { json!({}) } else { c };
    if str_of(a, "action_type") == "schedule_nodes" {
        let n = c.get("num_allocations").cloned().unwrap_or(json!(1));
        let t = c
            .get("scheduler_type")
            .and_then(Value::as_str)
            .unwrap_or("scheduler");
        let sid = c
            .get("scheduler_id")
            .map(|v| v.to_string())
            .unwrap_or_default();
        return format!("{n} × {t} {sid}");
    }
    match c.get("command") {
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => c.to_string(),
    }
}

async fn scheduling_view(st: &Arc<AppState>, id: i64, d: &Dbg) -> Markup {
    let (diag, actions) = tokio::join!(
        kit::op(st, "diagnose", json!({ "workflow_id": id })),
        kit::op(st, "list-actions", json!({ "workflow_id": id })),
    );
    let error = diag.as_ref().err().or(actions.as_ref().err()).cloned();
    let actions: Option<Vec<Value>> = actions.ok().map(|a| {
        a.get("actions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    });
    let warn = |n: i64| (n > 0).then_some(Tone::Warning);
    html! {
        div.section {
            @if let Some(e) = error { (kit::error_box(&e)) }
            h4 { "Allocation packing" }
            @if let Ok(d) = &diag {
                @let blocked = int_of(d, "runtime_blocked_jobs");
                @let short = int_of(d, "allocations_too_short_for_longest");
                @let ready = int_of(d, "ready_jobs");
                @let active = int_of(d, "active_allocations");
                (kit::stats(&[
                    ("Active allocations", active, None),
                    ("Ready jobs", ready, None),
                    ("Blocked by walltime", blocked, warn(blocked)),
                    ("Allocations too short", short, warn(short)),
                ]))
                (kit::kv(&[
                    ("Remaining allocation walltime", html! { (range(&d["allocation_remaining"])) }),
                    ("Runtime of ready jobs", html! { (range(&d["ready_job_runtime"])) }),
                ]))
                @if d.get("runtime_blocked").and_then(Value::as_bool) == Some(true) {
                    p.note.warn {
                        (blocked) " ready job(s) need more time than any running allocation has left, so torc won't start them there (they would be killed mid-run). They will run on a new allocation: schedule more Slurm nodes, or lower the jobs' runtime if it is overestimated."
                    }
                } @else if ready > 0 && active == 0 {
                    p.note {
                        (ready) " job(s) are ready but no allocation is active. Submit the workflow or schedule Slurm nodes to run them."
                    }
                }
            }
            h4 { "Workflow actions" }
            @match &actions {
                None => {}
                Some(a) if a.is_empty() => p.muted { "This workflow has no actions." },
                Some(a) => {
                    p.muted { "An action runs once its trigger has fired for all of its jobs (Triggers shows progress)." }
                    div.dbg-table { (actions_table(a, &d.actions)) }
                }
            }
        }
    }
}

fn actions_table(rows: &[Value], state: &TableState) -> Markup {
    let cols = vec![
        Col::num("id", "ID", |a: &Value| a.get("id").and_then(Value::as_f64)).width("60px"),
        Col::text("trigger_type", "Trigger", |a: &Value| {
            str_of(a, "trigger_type").into()
        })
        .mono(),
        Col::text("action_type", "Action", |a: &Value| {
            str_of(a, "action_type").into()
        })
        .mono(),
        Col::text("config", "Config", action_config),
        Col::num("jobs", "Jobs", |a: &Value| {
            Some(a.get("job_ids").and_then(Value::as_array).map_or(0, Vec::len) as f64)
        })
        .width("70px"),
        Col::text("progress", "Triggers", |a: &Value| {
            format!(
                "{}/{}",
                int_of(a, "trigger_count"),
                int_of(a, "required_triggers")
            )
        })
        .width("90px")
        .mono(),
        Col::text("executed", "Status", |a: &Value| {
            if a.get("executed").and_then(Value::as_bool) == Some(true) {
                "executed".into()
            } else {
                "pending".into()
            }
        })
        .width("130px")
        .cell(|a| {
            html! {
                @if a.get("executed").and_then(Value::as_bool) == Some(true) { (badge(Tone::Success, "executed")) }
                @else { (badge(Tone::Neutral, "pending")) }
                @if a.get("is_recovery").and_then(Value::as_bool) == Some(true) { " " (badge(Tone::Warning, "recovery")) }
            }
        }),
    ];
    Table::new("debug.actions", RELOAD, |a: &Value| int_of(a, "id"), cols).render(rows, state)
}

// ============== Resource check ==============

async fn resource_check_view(st: &Arc<AppState>, id: i64, d: &Dbg) -> Markup {
    let check = kit::op(
        st,
        "check-resources",
        json!({ "workflow_id": id, "include_failed": d.include_failed, "all": d.show_all }),
    )
    .await;
    // A preview belongs to one workflow and one set of options; editing them drops it.
    let drop_preview = format!("document.getElementById('dbg-preview-{id}')?.replaceChildren()");
    html! {
        div.section {
            div.toolbar {
                h4.grow { "Jobs that exceeded their requested resources" }
                label.check { input type="checkbox" data-bind="debug.includeFailed" data-on:change=(RELOAD); "Include failed jobs" }
                label.check { input type="checkbox" data-bind="debug.showAll" data-on:change=(RELOAD); "All jobs" }
            }
            @match &check {
                Err(e) => (kit::error_box(e)),
                Ok(c) => {
                    @let rows: Vec<Value> = ["violations", "within_limits"].iter().flat_map(|k| c.get(*k).and_then(Value::as_array).cloned().unwrap_or_default()).collect();
                    p.muted { (int_of(c, "over_utilization_count")) " violation(s) across " (int_of(c, "total_results")) " result(s)." }
                    @if !rows.is_empty() { div.dbg-table { (violations_table(&rows, &d.check)) } }
                }
            }
            h4 { "Right-size resource requirements" }
            p.muted {
                "Set each requirement from its jobs' measured peak usage times a safety multiplier. Raises requests that were exceeded and, unless disabled, lowers ones that were far too large. Only future runs are affected."
            }
            div.options data-on:change=(drop_preview) {
                label.field { "Memory ×" input.input type="number" min="1" step="0.1" data-bind="debug.mem"; }
                label.field { "CPU ×" input.input type="number" min="1" step="0.1" data-bind="debug.cpu"; }
                label.field { "Runtime ×" input.input type="number" min="1" step="0.1" data-bind="debug.rt"; }
                label.check { input type="checkbox" data-bind="debug.noDownsize"; "Never lower requests" }
                button.btn data-indicator="_dbgCorrect" data-attr:disabled="$_dbgCorrect"
                    data-on:click="@post('/ui/debug/correct')" {
                    span.spinner.small data-show="$_dbgCorrect" style="display:none" {}
                    " Preview changes"
                }
            }
            div id=(format!("dbg-preview-{id}")) data-ignore-morph {}
        }
    }
}

/// Violations keyed by position (a job can appear once per resource type).
fn violations_table(rows: &[Value], state: &TableState) -> Markup {
    let rows: Vec<(i64, &Value)> = (0..).zip(rows).collect();
    let s = |k: &'static str| move |v: &(i64, &Value)| str_of(v.1, k).to_string();
    let cols = vec![
        Col::num("job_id", "Job", |v: &(i64, &Value)| {
            v.1.get("job_id").and_then(Value::as_f64)
        })
        .width("70px"),
        Col::text("job_name", "Name", s("job_name")),
        Col::text("resource_type", "Resource", s("resource_type")).width("110px"),
        Col::text("specified", "Requested", s("specified")).mono(),
        Col::text("peak_used", "Peak used", s("peak_used")).mono(),
        Col::text("over_utilization", "Over", s("over_utilization"))
            .mono()
            .width("100px"),
    ];
    let mut t = Table::new("debug.check", RELOAD, |v: &(i64, &Value)| v.0, cols);
    t.on_click = Some(Box::new(|v| {
        format!("@get('/ui/job/{}')", int_of(v.1, "job_id"))
    }));
    t.render(&rows, state)
}

#[derive(Deserialize)]
struct CorrectParams {
    workflow_id: i64,
    mem: f64,
    cpu: f64,
    rt: f64,
    no_downsize: bool,
    #[serde(default)]
    n: i64,
}

impl CorrectParams {
    fn op(&self, dry_run: bool) -> Value {
        json!({
            "workflow_id": self.workflow_id, "memory_multiplier": self.mem,
            "cpu_multiplier": self.cpu, "runtime_multiplier": self.rt,
            "no_downsize": self.no_downsize, "dry_run": dry_run,
        })
    }
    fn query(&self, n: i64) -> String {
        format!(
            "workflow_id={}&mem={}&cpu={}&rt={}&no_downsize={}&n={n}",
            self.workflow_id, self.mem, self.cpu, self.rt, self.no_downsize
        )
    }
}

fn change(a: &Value, from: &str, to: &str) -> String {
    let (f, t) = (&a[from], &a[to]);
    if f.is_null() && t.is_null() {
        return "—".into();
    }
    let s = |v: &Value| match v {
        Value::Null => "?".to_string(),
        Value::String(s) => s.clone(),
        v => v.to_string(),
    };
    format!("{} → {}", s(f), s(t))
}

/// Preview right-sizing (dry run) into the preview slot; Apply reuses exactly these options.
async fn correct_preview(State(st): State<Arc<AppState>>, Sig(sig): Sig<Value>) -> Ui {
    let g: G = serde_json::from_value(sig.clone()).unwrap_or_default();
    let d: Dbg = kit::ns(&sig, "debug");
    let Some(id) = g.wf else { return Ui::new() };
    let p = CorrectParams {
        workflow_id: id,
        mem: d.mem,
        cpu: d.cpu,
        rt: d.rt,
        no_downsize: d.no_downsize,
        n: 0,
    };
    let slot = format!("#dbg-preview-{id}");
    let r = match kit::op(&st, "correct-resources", p.op(true)).await {
        Ok(r) => r,
        Err(e) => return Ui::new().inner(&slot, kit::error_box(&e)),
    };
    let adj = r
        .get("adjustments")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let n = int_of(&r, "resource_requirements_updated");
    let adjusted = |a: &Value, k: &str| a.get(k).and_then(Value::as_bool) == Some(true);
    let body = if adj.is_empty() {
        html! { p.muted { (r.get("message").and_then(Value::as_str).unwrap_or("No changes needed.")) } }
    } else {
        html! {
            table.plain {
                thead { tr { th { "Requirement" } th { "Jobs" } th { "Memory" } th { "CPUs" } th { "Runtime" } } }
                tbody {
                    @for a in &adj {
                        @let names: Vec<&str> = a["job_names"].as_array().map(|v| v.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
                        tr {
                            td.mono { "#" (int_of(a, "resource_requirements_id")) " " span.muted { (str_of(a, "direction")) } }
                            td {
                                (names.iter().take(3).copied().collect::<Vec<_>>().join(", "))
                                @if names.len() > 3 { " " em.muted { "(+" (names.len() - 3) " more)" } }
                            }
                            td.mono { (if adjusted(a, "memory_adjusted") { change(a, "original_memory", "new_memory") } else { "—".into() }) }
                            td.mono { (if adjusted(a, "cpu_adjusted") { change(a, "original_cpus", "new_cpus") } else { "—".into() }) }
                            td.mono { (if adjusted(a, "runtime_adjusted") { change(a, "original_runtime", "new_runtime") } else { "—".into() }) }
                        }
                    }
                }
            }
            div.toolbar {
                div.grow {}
                button.btn.btn-primary data-on:click=(format!("@get('/ui/debug/correct/confirm?{}')", p.query(n))) {
                    "Apply " (n) " change(s)"
                }
            }
        }
    };
    Ui::new().inner(&slot, body)
}

async fn correct_confirm(Query(p): Query<CorrectParams>) -> Ui {
    Ui::new().modal(modal(
        "Update resource requirements?",
        "sm",
        html! {
            p { "This updates " (p.n) " resource requirement(s) for future runs. Jobs are not reset or rerun; reset or recover them to run again with the new values." }
        },
        Some(html! {
            (cancel_button())
            button.btn.btn-primary data-on:click=(format!("el.closest('dialog').close(); @post('/ui/debug/correct/apply?{}')", p.query(p.n))) { "Apply" }
        }),
    ))
}

async fn correct_apply(State(st): State<Arc<AppState>>, Query(p): Query<CorrectParams>) -> Ui {
    let slot = format!("#dbg-preview-{}", p.workflow_id);
    match kit::op(&st, "correct-resources", p.op(false)).await {
        Ok(r) => Ui::new()
            .ok(format!(
                "Updated {} resource requirement(s)",
                r.get("resource_requirements_updated")
                    .and_then(Value::as_i64)
                    .unwrap_or(p.n)
            ))
            .inner(&slot, html! {})
            .refresh(),
        Err(e) => Ui::new().inner(&slot, kit::error_box(&e)),
    }
}

// ============== Slurm ==============

fn slurm_view(view: &str, id: i64) -> Markup {
    if view == "slurm-logs" {
        html! {
            div.section {
                div.toolbar {
                    p.muted.grow { "Parse Slurm log files for known error patterns (OOM, time limits, node failures, etc.)." }
                    label.check { input type="checkbox" data-bind="debug.errorsOnly"; "Errors only" }
                    button.btn.btn-primary data-indicator="_dbgSlurm" data-attr:disabled="$_dbgSlurm"
                        data-on:click="@post('/ui/debug/slurm-logs')" {
                        span.spinner.small data-show="$_dbgSlurm" style="display:none" {}
                        " Analyze Slurm logs"
                    }
                }
                div.stack id=(format!("dbg-slurm-{id}")) data-ignore-morph {
                    div.empty { "Click “Analyze Slurm logs” to scan for issues." }
                }
            }
        }
    } else {
        html! {
            div.section {
                div.toolbar {
                    p.muted.grow { "Collect sacct data for all Slurm jobs in this workflow." }
                    button.btn.btn-primary data-indicator="_dbgSacct" data-attr:disabled="$_dbgSacct"
                        data-on:click="@post('/ui/debug/sacct')" {
                        span.spinner.small data-show="$_dbgSacct" style="display:none" {}
                        " Collect sacct data"
                    }
                }
                div.stack id=(format!("dbg-sacct-{id}")) data-ignore-morph {
                    div.empty { "Click “Collect sacct data” to retrieve job accounting information." }
                }
            }
        }
    }
}

/// The selected workflow and its log base dir (relative dirs resolve against its submission
/// directory; if it can't be read, cwd-relative).
async fn wf_and_base(st: &AppState, sig: &Value) -> Option<(i64, String)> {
    let g: G = serde_json::from_value(sig.clone()).unwrap_or_default();
    let id = g.wf?;
    let sub = super::workflows::submission_directory(st, id).await;
    Some((id, kit::resolve_log_base_dir(&g.output_dir, sub.as_deref())))
}

/// `{success, data, error}` from the Slurm handlers.
fn envelope(v: Result<Value, String>) -> Result<Value, String> {
    let v = v?;
    if v.get("success").and_then(Value::as_bool) == Some(true) {
        Ok(v.get("data")
            .cloned()
            .filter(|d| !d.is_null())
            .unwrap_or(json!({})))
    } else {
        Err(v
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("Unknown error")
            .to_string())
    }
}

async fn slurm_logs(State(st): State<Arc<AppState>>, Sig(sig): Sig<Value>) -> Ui {
    let Some((id, base)) = wf_and_base(&st, &sig).await else {
        return Ui::new();
    };
    let d: Dbg = kit::ns(&sig, "debug");
    let r = envelope(
        kit::call(
            &st,
            crate::cli_slurm_parse_logs_handler,
            json!({ "workflow_id": id, "output_dir": base, "errors_only": d.errors_only }),
        )
        .await,
    );
    let body = match r {
        Err(e) => kit::error_box(&e),
        Ok(data) if int_of(&data, "total_issues") == 0 => html! {
            div.empty {
                (data.get("message").and_then(Value::as_str).map(String::from).unwrap_or_else(||
                    format!("No issues found in Slurm logs (scanned {} file(s)).", int_of(&data, "files_scanned"))))
            }
        },
        Ok(data) => {
            let mut groups: Vec<(String, Vec<&Value>)> = Vec::new();
            for i in data["issues"].as_array().into_iter().flatten() {
                let k = Some(str_of(i, "slurm_job_id"))
                    .filter(|s| !s.is_empty())
                    .unwrap_or("unknown")
                    .to_string();
                match groups.iter_mut().find(|(g, _)| *g == k) {
                    Some((_, v)) => v.push(i),
                    None => groups.push((k, vec![i])),
                }
            }
            html! {
                p {
                    strong { (int_of(&data, "errors")) } " error(s), "
                    strong { (int_of(&data, "warnings")) } " warning(s) in "
                    (int_of(&data, "files_scanned")) " file(s)"
                }
                @for (job, issues) in &groups {
                    div.card.group {
                        div.head { strong { "Slurm job " (job) } span.muted { (issues.len()) " issue(s)" } }
                        @if let Some(aff) = issues[0]["affected_jobs"].as_array().filter(|a| !a.is_empty()) {
                            div.affected {
                                "Affected Torc jobs:"
                                @for j in aff { span.tag { (str_of(j, "job_name")) " (" (int_of(j, "job_id")) ")" } }
                            }
                        }
                        @for issue in issues {
                            div.issue {
                                div.title {
                                    (badge(if str_of(issue, "severity") == "error" { Tone::Danger } else { Tone::Warning }, str_of(issue, "severity")))
                                    (str_of(issue, "pattern_description"))
                                    @if let Some(node) = issue["node"].as_str() { span.muted { " (node: " (node) ")" } }
                                }
                                pre.code-block { (str_of(issue, "line")) }
                                div.faint.loc { (str_of(issue, "file")) ":" (int_of(issue, "line_number")) }
                            }
                        }
                    }
                }
            }
        }
    };
    Ui::new().inner(&format!("#dbg-slurm-{id}"), body)
}

fn sacct_tone(state: &str) -> Tone {
    match state.to_uppercase().as_str() {
        "COMPLETED" => Tone::Success,
        "RUNNING" => Tone::Accent,
        "PENDING" => Tone::Info,
        "FAILED" | "NODE_FAIL" | "OUT_OF_MEMORY" => Tone::Danger,
        "CANCELLED" | "TIMEOUT" | "PREEMPTED" => Tone::Warning,
        _ => Tone::Neutral,
    }
}

async fn sacct(State(st): State<Arc<AppState>>, Sig(sig): Sig<Value>) -> Ui {
    let Some((id, base)) = wf_and_base(&st, &sig).await else {
        return Ui::new();
    };
    let r = envelope(
        kit::call(
            &st,
            crate::cli_slurm_sacct_handler,
            json!({ "workflow_id": id, "output_dir": base }),
        )
        .await,
    );
    let body = match r {
        Err(e) => kit::error_box(&e),
        Ok(data) if int_of(&data, "total_slurm_jobs") == 0 => html! {
            div.empty { (data.get("message").and_then(Value::as_str).unwrap_or("No Slurm scheduled compute nodes found for this workflow.")) }
        },
        Ok(data) => {
            // Totals are a convenience; the table is the main result.
            let usage = kit::op(&st, "slurm-usage", json!({ "workflow_id": id }))
                .await
                .ok();
            let summary = data["summary"].as_array().cloned().unwrap_or_default();
            let errors = data["errors"].as_array().cloned().unwrap_or_default();
            let cols = [
                ("slurm_job_id", "Slurm job"),
                ("job_step", "Step"),
                ("state", "State"),
                ("exit_code", "Exit code"),
                ("elapsed", "Elapsed"),
                ("max_rss", "Max RSS"),
                ("cpu_time", "CPU time"),
                ("nodes", "Nodes"),
            ];
            html! {
                p {
                    strong { (int_of(&data, "total_slurm_jobs")) } " Slurm job(s), "
                    strong { (summary.len()) } " job step(s)"
                    @if let Some(u) = &usage {
                        " · " strong { (int_of(u, "total_nodes")) } " node(s), "
                        strong { (str_of(u, "total_node_time")) } " node time, "
                        strong { (str_of(u, "total_cpu_time")) } " CPU time"
                    }
                }
                @if !summary.is_empty() {
                    div.table-wrap.static { div.table-scroll { table {
                        thead { tr { @for (_, l) in cols { th { (l) } } } }
                        tbody { @for row in &summary { tr {
                            @for (k, _) in cols {
                                @if k == "state" {
                                    td { (badge(sacct_tone(str_of(row, k)), Some(str_of(row, k)).filter(|s| !s.is_empty()).unwrap_or("—"))) }
                                } @else {
                                    td.mono[k != "nodes"] { (kit::or_dash(Some(str_of(row, k)))) }
                                }
                            }
                        } } }
                    } } }
                }
                @if !errors.is_empty() {
                    div {
                        strong { "Errors" }
                        ul { @for e in &errors { li.error-text { (e.as_str().map(String::from).unwrap_or_else(|| e.to_string())) } } }
                    }
                }
            }
        }
    };
    Ui::new().inner(&format!("#dbg-sacct-{id}"), body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_case_insensitively() {
        let (m, n) = highlight("Error: an error<x>", "ERROR");
        assert_eq!(n, 2);
        assert_eq!(
            m.into_string(),
            "<mark>Error</mark>: an <mark>error</mark>&lt;x&gt;"
        );
        assert_eq!(highlight("abc", "").1, 0);
    }
}
