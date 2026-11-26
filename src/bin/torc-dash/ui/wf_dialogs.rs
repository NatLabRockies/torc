//! Per-workflow dialogs opened from the workflow actions: execution plan, recover, sync Slurm
//! status, schedule Slurm nodes, export, edit & share; plus the import dialog.
//!
//! Dialogs keep their form state in `$dlg` (reset by each dialog's `data-signals`). Slow
//! commands answer with a stream: the dialog with a spinner first, then its `#dlg-body` and
//! `#dlg-foot` once the command returns.

use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, Query, State},
    response::Response,
    routing::{get, post},
};
use base64::Engine;
use futures::{Future, StreamExt, stream};
use maud::{Markup, html};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};

use super::kit::{self, G, Sig, Tone, Ui, cancel_button, icon, js, modal, stats};
use super::workflows::submission_directory;
use crate::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ui/wf/{id}/plan", get(plan))
        .route("/ui/wf/{id}/recover", get(recover))
        .route("/ui/wf/{id}/recover/preview", post(recover_preview))
        .route("/ui/wf/{id}/recover/apply", post(recover_apply))
        .route("/ui/wf/{id}/sync", get(sync))
        .route("/ui/wf/{id}/sync/apply", post(sync_apply))
        .route("/ui/wf/{id}/schedule", get(schedule).post(schedule_submit))
        .route("/ui/wf/{id}/export", get(export))
        .route("/ui/wf/{id}/edit", get(edit).post(edit_save))
        .route("/ui/wf/{id}/group", post(change_group))
        .route("/ui/workflows/import", get(import).post(import_submit))
}

// ============== Helpers ==============

/// A dialog whose body and footer can be replaced later with [`content`]. `extra` goes above
/// the body (e.g. an options form that stays while the body reloads).
fn dialog(title: &str, size: &str, extra: Markup, body: Markup, foot: Markup) -> Markup {
    modal(
        title,
        size,
        html! { (extra) div #dlg-body.dlg { (body) } },
        Some(html! { div #dlg-foot style="display:contents" { (foot) } }),
    )
}

fn content(body: Markup, foot: Markup) -> Ui {
    Ui::new()
        .patch(html! { div #dlg-body.dlg { (body) } })
        .patch(html! { div #dlg-foot style="display:contents" { (foot) } })
}

fn loading(msg: &str) -> Markup {
    html! { div.empty { span.spinner {} (msg) } }
}

fn close_button(label: &str) -> Markup {
    html! { button.btn data-on:click="el.closest('dialog').close()" { (label) } }
}

/// Close the dialog a few seconds after it shows the result.
fn auto_close() -> &'static str {
    "setTimeout(() => el.closest('dialog')?.close(), 3000)"
}

/// Send `first` now and the result of `rest` when it is ready.
fn two_step(first: Ui, rest: impl Future<Output = Ui> + Send + 'static) -> Response {
    kit::stream(stream::iter([first]).chain(stream::once(rest)))
}

fn wid(id: i64) -> Value {
    json!({ "workflow_id": id.to_string() })
}

fn int(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}

fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

fn list<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// A string or number field as text, or "—".
fn shown(v: &Value, k: &str) -> String {
    match v.get(k) {
        Some(Value::String(s)) if !s.is_empty() => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => "—".into(),
    }
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// Accepts a number, a numeric string, or empty (from number inputs and selects).
fn lenient_f64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    })
}

/// Toast and error text for a failed form submit, shown in the dialog's `#dlg-err`.
fn form_error(msg: &str) -> Ui {
    Ui::new().patch(html! { div #dlg-err { p.error-text { (msg) } } })
}

// ============== Execution plan ==============

async fn plan(State(st): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    let first = Ui::new().modal(dialog(
        &format!("Execution plan · workflow {id}"),
        "xl",
        html! {},
        loading("Loading execution plan…"),
        close_button("Close"),
    ));
    two_step(first, async move {
        let r: Result<Value, String> =
            kit::call(&st, crate::cli_execution_plan_handler, wid(id)).await;
        let body = match r.and_then(kit::op_result) {
            Ok(p) => plan_body(&p),
            Err(e) => kit::error_box(&e),
        };
        content(body, close_button("Close"))
    })
}

fn trigger_text(t: &Value) -> String {
    match text(t, "type") {
        "WorkflowStart" => "Workflow Start".into(),
        "JobsComplete" => {
            let jobs: Vec<&str> = t
                .get("data")
                .map(|d| list(d, "jobs"))
                .unwrap_or(&[])
                .iter()
                .filter_map(Value::as_str)
                .collect();
            match jobs.len() {
                0 => "Jobs Complete".into(),
                1 => format!("When job '{}' completes", jobs[0]),
                2 | 3 => format!(
                    "When jobs complete: {}",
                    jobs.iter()
                        .map(|j| format!("'{j}'"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                n => format!("When {n} jobs complete ('{}', '{}'...)", jobs[0], jobs[1]),
            }
        }
        _ => "Unknown trigger".into(),
    }
}

/// Events in breadth-first order from the roots, then anything unreachable.
fn plan_body(p: &Value) -> Markup {
    let events = list(p, "events");
    let by_id = |id: &str| events.iter().find(|e| text(e, "id") == id);
    let ids = |v: &Value, k: &str| -> Vec<String> {
        list(v, k)
            .iter()
            .filter_map(|x| x.as_str().map(String::from))
            .collect()
    };
    let roots = ids(p, "root_events");
    let leaves = ids(p, "leaf_events");
    let mut seen = std::collections::HashSet::new();
    let mut ordered: Vec<&Value> = Vec::new();
    let mut queue: std::collections::VecDeque<String> = roots.iter().cloned().collect();
    while let Some(id) = queue.pop_front() {
        let Some(ev) = by_id(&id) else { continue };
        if !seen.insert(id) {
            continue;
        }
        ordered.push(ev);
        queue.extend(ids(ev, "unlocks_events"));
    }
    for ev in events {
        if !seen.contains(text(ev, "id")) {
            ordered.push(ev);
        }
    }
    if ordered.is_empty() {
        return html! { div.empty { "No execution events computed" } };
    }
    let total: usize = ordered
        .iter()
        .map(|e| list(e, "jobs_becoming_ready").len())
        .sum();
    let job = |j: &Value| {
        j.as_str()
            .map(String::from)
            .unwrap_or_else(|| j.to_string())
    };
    html! {
        p.muted style="margin: 0 0 12px" {
            strong { (plural(ordered.len(), "event")) } " · " strong { (plural(total, "job")) }
        }
        ol.plan-events {
            @for ev in &ordered {
                @let id = text(ev, "id");
                @let root = roots.iter().any(|r| r == id);
                @let leaf = leaves.iter().any(|r| r == id);
                @let jobs = list(ev, "jobs_becoming_ready");
                @let allocs = list(ev, "scheduler_allocations");
                @let unlocks = list(ev, "unlocks_events");
                @let desc = text(ev, "trigger_description");
                li.root[root].leaf[leaf] {
                    div.head {
                        span.marker { (if root { "▶" } else if leaf { "◆" } else { "→" }) }
                        span { (if desc.is_empty() { trigger_text(ev.get("trigger").unwrap_or(&Value::Null)) } else { desc.to_string() }) }
                    }
                    @if !allocs.is_empty() {
                        h5 { "Scheduler allocations" }
                        ul { @for s in allocs {
                            li { strong { (text(s, "scheduler")) } " (" (text(s, "scheduler_type")) ") — "
                                (plural(int(s, "num_allocations") as usize, "allocation")) }
                        } }
                    }
                    h5 { "Jobs becoming ready (" (jobs.len()) ")" }
                    ul { @for j in jobs.iter().take(10) { li { (job(j)) } } }
                    @if jobs.len() > 10 {
                        details {
                            summary { "… and " (jobs.len() - 10) " more" }
                            ul { @for j in &jobs[10..] { li { (job(j)) } } }
                        }
                    }
                    @if !unlocks.is_empty() {
                        div.flow { "↓ unlocks " (plural(unlocks.len(), "event")) }
                    }
                }
            }
        }
    }
}

// ============== Recover ==============

/// Recovery options. `out` is where the jobs' logs are, relative to the submission directory.
#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
struct RecoverOpts {
    #[serde(deserialize_with = "lenient_f64")]
    mem: Option<f64>,
    #[serde(deserialize_with = "lenient_f64")]
    rt: Option<f64>,
    out: String,
    unknown: bool,
}

impl Default for RecoverOpts {
    fn default() -> Self {
        RecoverOpts {
            mem: Some(1.5),
            rt: Some(1.5),
            out: "torc_output".into(),
            unknown: false,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RecoverSig {
    dlg: RecoverOpts,
}

fn recover_form(id: i64) -> Markup {
    html! {
        form #dlg-opts.dlg-options data-indicator="_dlgBusy"
            data-on:submit__prevent=(format!("@post('/ui/wf/{id}/recover/preview')")) {
            label.field {
                "Memory multiplier"
                input.input type="number" min="1" step="0.1" data-bind="dlg.mem";
                span.hint { "For jobs that ran out of memory" }
            }
            label.field {
                "Runtime multiplier"
                input.input type="number" min="1" step="0.1" data-bind="dlg.rt";
                span.hint { "For jobs that timed out" }
            }
            label.field {
                "Output dir"
                input.input.mono placeholder="torc_output" spellcheck="false" data-bind="dlg.out";
                span.hint { "Relative to the submission directory" }
            }
            label.check {
                input type="checkbox" data-bind="dlg.unknown";
                " Also retry jobs that failed for unknown reasons"
            }
            button.btn.btn-sm type="submit" data-attr:disabled="$_dlgBusy" { "Update preview" }
        }
    }
}

async fn recover(State(st): State<Arc<AppState>>, Path(id): Path<i64>, Sig(g): Sig<G>) -> Response {
    let opts = RecoverOpts {
        out: g.output_dir.clone(),
        ..Default::default()
    };
    let first = Ui::new().modal(html! {
        div data-signals=(json!({ "dlg": opts }).to_string()) {
            (dialog(
                &format!("Recover workflow {id}"),
                "lg",
                recover_form(id),
                loading("Analyzing workflow for recovery…"),
                close_button("Close"),
            ))
        }
    });
    two_step(first, recover_preview_ui(st, id, opts))
}

async fn recover_preview(
    State(st): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Sig(s): Sig<RecoverSig>,
) -> Response {
    let first = content(
        loading("Analyzing workflow for recovery…"),
        close_button("Close"),
    );
    two_step(first, recover_preview_ui(st, id, s.dlg))
}

/// Run `torc recover` (a dry run unless `apply`) with the options.
async fn run_recover(
    st: &Arc<AppState>,
    id: i64,
    o: &RecoverOpts,
    apply: bool,
) -> Result<Value, String> {
    let sub = submission_directory(st, id).await;
    let out = if o.out.trim().is_empty() {
        "torc_output"
    } else {
        o.out.trim()
    };
    let body = json!({
        "workflow_id": id.to_string(),
        "dry_run": !apply,
        "memory_multiplier": o.mem.unwrap_or(1.5),
        "runtime_multiplier": o.rt.unwrap_or(1.5),
        "retry_unknown": o.unknown,
        "output_dir": kit::resolve_log_base_dir(out, sub.as_deref()),
    });
    let data = kit::op_result(kit::call(st, crate::cli_recover_handler, body).await?)?;
    if data.get("result").is_none_or(Value::is_null) {
        return Err("No recovery data available.".into());
    }
    Ok(data)
}

async fn recover_preview_ui(st: Arc<AppState>, id: i64, o: RecoverOpts) -> Ui {
    let data = match run_recover(&st, id, &o, false).await {
        Ok(d) => d,
        Err(e) => return content(kit::error_box(&e), close_button("Close")),
    };
    let r = &data["result"];
    let retry = list(r, "jobs_to_retry").len();
    let (oom, timeout, other) = (
        int(r, "oom_fixed"),
        int(r, "timeout_fixed"),
        int(r, "other_failures"),
    );
    let failed = data
        .get("diagnosis")
        .map(|d| list(d, "failed_jobs"))
        .unwrap_or(&[]);
    let adjustments = list(r, "adjustments");
    let slurm = r.get("slurm_dry_run").unwrap_or(&Value::Null);
    let planned = list(slurm, "planned_schedulers");
    let code = |a: &Value, k: &str| html! { code { (text(a, k)) } };

    let body = html! {
        section {
            h4 { "Summary" }
            (stats(&[
                ("Jobs to retry", retry as i64, (retry > 0).then_some(Tone::Warning)),
                ("OOM fixes", oom, (oom > 0).then_some(Tone::Danger)),
                ("Timeout fixes", timeout, (timeout > 0).then_some(Tone::Warning)),
                ("Unknown failures", other, None),
            ]))
        }
        @if !failed.is_empty() {
            section {
                h4 { "Failed jobs" }
                table.dlg-table {
                    thead { tr { th { "Job" } th { "Return code" } th { "Reason" } th { "Details" } } }
                    tbody { @for j in failed {
                        @let (why, tone, details) = failure_reason(j);
                        tr {
                            td { code { (text(j, "job_name")) } }
                            td { (shown(j, "return_code")) }
                            td.(tone) { (why) }
                            td { (details) }
                        }
                    } }
                }
            }
        }
        @if !adjustments.is_empty() {
            section {
                h4 { "Resource adjustments" }
                table.dlg-table {
                    thead { tr { th { "Jobs" } th { "Memory" } th { "Runtime" } } }
                    tbody { @for a in adjustments {
                        @let names: Vec<&str> = list(a, "job_names").iter().filter_map(Value::as_str).collect();
                        tr {
                            td {
                                (names.iter().take(3).copied().collect::<Vec<_>>().join(", "))
                                @if names.len() > 3 { " " em.muted { "(+" (names.len() - 3) " more)" } }
                            }
                            td { @if a.get("memory_adjusted") == Some(&Value::Bool(true)) {
                                (code(a, "original_memory")) " → " (code(a, "new_memory"))
                            } @else { "—" } }
                            td { @if a.get("runtime_adjusted") == Some(&Value::Bool(true)) {
                                (code(a, "original_runtime")) " → " (code(a, "new_runtime"))
                            } @else { "—" } }
                        }
                    } }
                }
            }
        }
        @if !planned.is_empty() {
            section {
                h4 { "Slurm allocations to create" }
                @for s in planned {
                    div.dlg-sched {
                        strong { (text(s, "name")) }
                        span.muted { " (" (int(s, "job_count")) " job(s), " (int(s, "num_allocations")) " allocation(s))" }
                        (kit::kv(&[
                            ("Account", html! { (shown(s, "account")) }),
                            ("Partition", html! { (if text(s, "partition").is_empty() { "default" } else { text(s, "partition") }) }),
                            ("Walltime", html! { (shown(s, "walltime")) }),
                            ("Memory", html! { (shown(s, "mem")) }),
                            ("Nodes", html! { (int(s, "nodes").max(1)) }),
                        ]))
                    }
                }
                p.muted { "Total: " (int(slurm, "total_allocations")) " allocation(s) would be submitted" }
            }
        }
        @if retry == 0 {
            p.dlg-note {
                @if other > 0 {
                    (other) " job(s) failed with unknown causes. Check “Also retry jobs that failed for unknown reasons” and update the preview to retry them."
                } @else {
                    "No recoverable jobs found. The workflow may have completed successfully or all failures require manual intervention."
                }
            }
        }
    };
    // Apply uses the options as previewed, so it does what the preview showed.
    let mut q = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in [("mem", o.mem), ("rt", o.rt)] {
        if let Some(v) = v {
            q.append_pair(k, &v.to_string());
        }
    }
    let query = q
        .append_pair("out", &o.out)
        .append_pair("unknown", &o.unknown.to_string())
        .finish();
    let foot = html! {
        @if retry > 0 {
            (cancel_button())
            button.btn.btn-primary data-on:click=(format!("@post('/ui/wf/{id}/recover/apply?{query}')")) { "Apply recovery" }
        } @else {
            (close_button("Close"))
        }
    };
    content(body, foot)
}

/// (reason, css class, details) of a failed job.
fn failure_reason(j: &Value) -> (&'static str, &'static str, String) {
    let yes = |k: &str| j.get(k).and_then(Value::as_bool) == Some(true);
    if yes("likely_oom") {
        let peak = text(j, "peak_memory_formatted");
        let peak = if peak.is_empty() || peak == "0.0 MB" {
            String::new()
        } else {
            format!(" (peak: {peak})")
        };
        ("OOM", "danger", format!("{}{peak}", text(j, "oom_reason")))
    } else if yes("likely_timeout") {
        ("Timeout", "warning", text(j, "timeout_reason").to_string())
    } else {
        ("Unknown", "", String::new())
    }
}

async fn recover_apply(
    State(st): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(o): Query<RecoverOpts>,
) -> Response {
    let first = content(loading("Applying recovery…"), close_button("Close"))
        .patch(html! { div #dlg-opts {} });
    two_step(first, async move {
        info_log("recover", id);
        match run_recover(&st, id, &o, true).await {
            Ok(data) => {
                let r = &data["result"];
                let retry = list(r, "jobs_to_retry").len();
                let body = html! {
                    section.dlg-done data-init=(auto_close()) {
                        h4 { "✓ Recovery complete" }
                        ul {
                            @let n = int(r, "oom_fixed");
                            @if n > 0 { li { (n) " job(s) had memory increased" } }
                            @let n = int(r, "timeout_fixed");
                            @if n > 0 { li { (n) " job(s) had runtime increased" } }
                            @let n = int(r, "unknown_retried");
                            @if n > 0 { li { (n) " job(s) with unknown failures reset" } }
                            @if retry > 0 {
                                li { "Reset " (retry) " job(s)" }
                                li { "Slurm schedulers regenerated and submitted" }
                            }
                        }
                    }
                };
                content(body, close_button("Close"))
                    .ok("Recovery complete")
                    .refresh()
            }
            Err(e) => content(
                kit::error_box(&format!("Recovery failed: {e}")),
                close_button("Close"),
            )
            .err("Recovery failed"),
        }
    })
}

fn info_log(what: &str, id: i64) {
    tracing::info!("Dashboard {what} workflow_id={id}");
}

// ============== Sync Slurm status ==============

#[derive(Deserialize, Default)]
#[serde(default)]
struct RetryQuery {
    /// initialize | reinitialize: offered after cleanup when the dialog was opened because it
    /// was blocked by jobs that look active.
    retry: String,
}

impl RetryQuery {
    fn retry(&self) -> Option<&str> {
        ["initialize", "reinitialize"]
            .into_iter()
            .find(|r| *r == self.retry)
    }
}

async fn run_sync(st: &Arc<AppState>, id: i64, apply: bool) -> Result<Value, String> {
    let body = json!({ "workflow_id": id.to_string(), "dry_run": !apply });
    kit::op_result(kit::call(st, crate::cli_sync_status_handler, body).await?)
}

async fn sync(
    State(st): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(q): Query<RetryQuery>,
) -> Response {
    let first = Ui::new().modal(dialog(
        &format!("Sync status · workflow {id}"),
        "xl",
        html! {},
        loading("Checking Slurm job statuses…"),
        close_button("Close"),
    ));
    two_step(first, async move {
        let d = match run_sync(&st, id, false).await {
            Ok(d) => d,
            Err(e) => return content(kit::error_box(&e), close_button("Close")),
        };
        let (slurm, pending, running) = (
            int(&d, "slurm_jobs_failed"),
            int(&d, "pending_allocations_cleaned"),
            int(&d, "running_jobs_failed"),
        );
        let orphans = slurm + pending + running > 0;
        let tone = |n: i64, bad: Tone| Some(if n > 0 { bad } else { Tone::Success });
        let details = list(&d, "failed_job_details");
        let body = html! {
            section {
                h4 { "Summary" }
                (stats(&[
                    ("Slurm jobs failed", slurm, tone(slurm, Tone::Danger)),
                    ("Pending allocations cleaned", pending, tone(pending, Tone::Warning)),
                    ("Running jobs failed", running, tone(running, Tone::Danger)),
                ]))
            }
            @if !details.is_empty() {
                section {
                    h4 { "Affected jobs" }
                    table.dlg-table {
                        thead { tr { th { "Job ID" } th { "Job name" } th { "Reason" } th { "Slurm job ID" } } }
                        tbody { @for j in details {
                            tr {
                                td { code { (shown(j, "job_id")) } }
                                td { (shown(j, "job_name")) }
                                td { (shown(j, "reason")) }
                                td { code { (shown(j, "slurm_job_id")) } }
                            }
                        } }
                    }
                }
            }
            @if !orphans {
                p.dlg-note { "No orphaned jobs found. All running jobs have active Slurm allocations." }
            }
        };
        let foot = html! {
            @if orphans {
                (cancel_button())
                button.btn.btn-primary data-indicator="_dlgBusy" data-attr:disabled="$_dlgBusy"
                    data-on:click=(format!("@post('/ui/wf/{id}/sync/apply?retry={}')", q.retry().unwrap_or(""))) {
                    "Apply cleanup"
                }
            } @else {
                (close_button("Close"))
            }
        };
        content(body, foot)
    })
}

async fn sync_apply(
    State(st): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(q): Query<RetryQuery>,
) -> Response {
    let first = content(
        loading("Applying sync status cleanup…"),
        close_button("Close"),
    );
    two_step(first, async move {
        info_log("sync-status", id);
        match run_sync(&st, id, true).await {
            Ok(d) => {
                let retry = q.retry();
                let body = html! {
                    section.dlg-done data-init=[retry.is_none().then(auto_close)] {
                        h4 { "Sync complete" }
                        ul {
                            @let n = int(&d, "slurm_jobs_failed");
                            @if n > 0 { li { (n) " Slurm job(s) marked as failed" } }
                            @let n = int(&d, "pending_allocations_cleaned");
                            @if n > 0 { li { (n) " pending allocation(s) cleaned up" } }
                            @let n = int(&d, "running_jobs_failed");
                            @if n > 0 { li { (n) " running job(s) marked as failed" } }
                        }
                    }
                };
                let foot = html! {
                    (close_button("Close"))
                    @if let Some(r) = retry {
                        button.btn.btn-primary
                            data-on:click=(format!("el.closest('dialog').close(); @post('/ui/wf/{id}/{r}')")) {
                            (if r == "initialize" { "Initialize" } else { "Reinitialize" })
                        }
                    }
                };
                content(body, foot).ok("Sync status complete").refresh()
            }
            Err(e) => content(
                kit::error_box(&format!("Sync failed: {e}")),
                close_button("Close"),
            )
            .err("Sync status failed"),
        }
    })
}

// ============== Schedule Slurm nodes ==============

async fn schedule(State(st): State<Arc<AppState>>, Path(id): Path<i64>, Sig(g): Sig<G>) -> Ui {
    let title = format!("Schedule Slurm nodes for workflow {id}");
    let scheds: Result<Vec<Value>, String> =
        kit::list_all(&st, "/slurm_schedulers", &[("workflow_id", id.to_string())]).await;
    let scheds = match scheds {
        Ok(s) => s,
        Err(e) => {
            return Ui::new().modal(dialog(
                &title,
                "md",
                html! {},
                kit::error_box(&e),
                cancel_button(),
            ));
        }
    };
    if scheds.is_empty() {
        return Ui::new().modal(dialog(
            &title,
            "md",
            html! {},
            html! { p.muted {
                "This workflow has no Slurm schedulers. Add " code { "slurm_schedulers" }
                " to the spec, or create the workflow with “Generate Slurm schedulers”."
            } },
            cancel_button(),
        ));
    }
    let signals = json!({ "dlg": {
        "sched": int(&scheds[0], "id"), "n": 1, "maxp": "", "out": g.output_dir,
        "suppress": true, "perNode": false,
    } });
    let form = html! {
        form #dlg-form.dlg-form data-signals=(signals.to_string()) data-indicator="_dlgBusy"
            data-on:submit__prevent=(format!("@post('/ui/wf/{id}/schedule')")) {
            p.muted {
                "Submit Slurm allocations that run this workflow's ready jobs. Use this to add capacity or to run jobs again after resetting them. Uninitialized workflows are initialized first."
            }
            label.field {
                "Scheduler"
                select.input data-bind="dlg.sched" {
                    @for s in &scheds {
                        @let sid = int(s, "id");
                        option value=(sid) {
                            @if text(s, "name").is_empty() { "Scheduler " (sid) } @else { (text(s, "name")) }
                        }
                    }
                }
                @for s in &scheds {
                    span.hint.mono data-show=(format!("$dlg.sched == {}", int(s, "id"))) {
                        "account=" (text(s, "account")) " nodes=" (shown(s, "nodes")) " walltime=" (text(s, "walltime"))
                        @if !text(s, "partition").is_empty() { " partition=" (text(s, "partition")) }
                        @if !text(s, "mem").is_empty() { " mem=" (text(s, "mem")) }
                    }
                }
            }
            div.dlg-row {
                label.field {
                    "Allocations"
                    input.input type="number" min="1" required data-bind="dlg.n";
                }
                label.field {
                    "Max parallel jobs"
                    input.input type="number" min="1" placeholder="Unlimited" data-bind="dlg.maxp";
                }
            }
            label.field {
                "Output dir"
                input.input.mono placeholder="torc_output" spellcheck="false" data-bind="dlg.out";
                span.hint { "Relative to the submission directory" }
            }
            label.check {
                input type="checkbox" data-bind="dlg.suppress";
                " Skip the workflow's pending schedule_nodes actions"
            }
            span.dlg-indent {
                "Otherwise the new workers may also fire actions re-armed by a reset and submit extra allocations."
            }
            label.check {
                input type="checkbox" data-bind="dlg.perNode";
                " Start one worker per node"
            }
            div #dlg-err {}
        }
    };
    Ui::new().modal(dialog(
        &title,
        "md",
        html! {},
        form,
        html! {
            (cancel_button())
            button.btn.btn-primary type="submit" form="dlg-form" data-attr:disabled="$_dlgBusy"
                data-text="$_dlgBusy ? 'Submitting…' : 'Submit allocations'" { "Submit allocations" }
        },
    ))
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ScheduleForm {
    #[serde(deserialize_with = "kit::lenient_id")]
    sched: Option<i64>,
    #[serde(deserialize_with = "kit::lenient_id")]
    n: Option<i64>,
    #[serde(deserialize_with = "kit::lenient_id")]
    maxp: Option<i64>,
    out: String,
    suppress: bool,
    per_node: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ScheduleSig {
    dlg: ScheduleForm,
}

async fn schedule_submit(
    State(st): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Sig(s): Sig<ScheduleSig>,
) -> Ui {
    let f = s.dlg;
    let Some(sched) = f.sched else {
        return form_error("Choose a scheduler");
    };
    let n = f.n.unwrap_or(1).max(1);
    let sub = submission_directory(&st, id).await;
    let out = if f.out.trim().is_empty() {
        "torc_output"
    } else {
        f.out.trim()
    };
    let r = kit::op(
        &st,
        "schedule-nodes",
        json!({
            "workflow_id": id,
            "scheduler_config_id": sched,
            "num_allocations": n,
            "max_parallel_jobs": f.maxp.filter(|m| *m > 0),
            "suppress_actions": f.suppress,
            "start_one_worker_per_node": f.per_node,
            "output_dir": kit::resolve_log_base_dir(out, sub.as_deref()),
            "cwd": sub,
        }),
    )
    .await;
    match r {
        Ok(_) => Ui::new()
            .close_modal()
            .ok(format!(
                "Submitted {n} Slurm allocation(s) for workflow {id}"
            ))
            .refresh(),
        Err(e) => form_error(&e),
    }
}

// ============== Export ==============

async fn export(State(st): State<Arc<AppState>>, Path(id): Path<i64>) -> Ui {
    let name = kit::get::<Value>(&st, &format!("/workflows/{id}"), &[])
        .await
        .map(|w| text(&w, "name").to_string())
        .unwrap_or_default();
    let file = if name.is_empty() {
        format!("workflow_{id}.json")
    } else {
        // `<name>_<id>.json`, with only safe characters.
        let safe: String = name
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("_")
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        format!("{safe}_{id}.json")
    };
    let signals = json!({ "dlg": { "file": file, "results": false, "events": false } });
    let download = format!(
        "torcDownload('/api/download/export?workflow_id={id}&filename=' + encodeURIComponent($dlg.file.trim() || {}) + '&include_results=' + $dlg.results + '&include_events=' + $dlg.events); el.closest('dialog').close()",
        js(&format!("workflow_{id}.json"))
    );
    Ui::new().modal(dialog(
        &format!("Export workflow {id}"),
        "md",
        html! {},
        html! {
            form #dlg-form.dlg-form data-signals=(signals.to_string()) data-on:submit__prevent=(download) {
                p.muted { "Download the workflow as a JSON file that can be imported into this or another torc server." }
                label.field { "File name" input.input data-bind="dlg.file"; }
                label.check { input type="checkbox" data-bind="dlg.results"; " Include job results" }
                label.check { input type="checkbox" data-bind="dlg.events"; " Include events" }
            }
        },
        html! {
            (cancel_button())
            button.btn.btn-primary type="submit" form="dlg-form" { (icon("download", 13)) " Download" }
        },
    ))
}

// ============== Edit & share ==============

async fn edit(State(st): State<Arc<AppState>>, Path(id): Path<i64>) -> Ui {
    let wf = kit::get::<Value>(&st, &format!("/workflows/{id}"), &[]).await;
    let loaded = wf.is_ok();
    let w = wf.as_ref().cloned().unwrap_or(Value::Null);
    let signals = json!({ "dlg": {
        "name": text(&w, "name"), "desc": text(&w, "description"),
        "project": text(&w, "project"), "group": "",
    } });
    Ui::new().modal(dialog(
        &format!("Edit workflow {id}"),
        "md",
        html! {},
        html! {
            form #dlg-form.dlg-form data-signals=(signals.to_string()) data-indicator="_dlgBusy"
                data-on:submit__prevent=(format!("@post('/ui/wf/{id}/edit')")) {
                label.field { "Name" input.input required disabled[!loaded] data-bind="dlg.name"; }
                label.field { "Description" textarea.input rows="3" disabled[!loaded] data-bind="dlg.desc" {} }
                label.field { "Project" input.input disabled[!loaded] data-bind="dlg.project"; }
                div #dlg-err { @if let Err(e) = &wf { p.error-text { (e) } } }
            }
            (groups(&st, id).await)
        },
        html! {
            (cancel_button())
            button.btn.btn-primary type="submit" form="dlg-form" disabled[!loaded]
                data-attr:disabled=[loaded.then_some("$_dlgBusy")]
                data-text="$_dlgBusy ? 'Saving…' : 'Save'" { "Save" }
        },
    ))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct EditForm {
    name: String,
    desc: String,
    project: String,
    #[serde(deserialize_with = "kit::lenient_id")]
    group: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct EditSig {
    dlg: EditForm,
}

async fn edit_save(
    State(st): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Sig(s): Sig<EditSig>,
) -> Ui {
    let f = s.dlg;
    if f.name.trim().is_empty() {
        return Ui::new();
    }
    let r = kit::op(
        &st,
        "update-workflow",
        json!({ "workflow_id": id, "name": f.name, "description": f.desc, "project": f.project }),
    )
    .await;
    match r {
        Ok(_) => Ui::new()
            .close_modal()
            .ok(format!("Workflow {id} updated"))
            .refresh(),
        Err(e) => form_error(&e),
    }
}

/// The access groups the workflow is shared with, and a picker to share it with another.
async fn groups(st: &Arc<AppState>, id: i64) -> Markup {
    let items = |r: Result<Value, String>| -> Result<Vec<Value>, String> {
        r.map(|v| list(&v, "items").to_vec())
    };
    let all = items(kit::op(st, "list-access-groups", json!({})).await);
    let mine = items(kit::op(st, "list-workflow-groups", json!({ "workflow_id": id })).await);
    html! {
        section #dlg-groups.dlg-groups.dlg {
            h4 { "Shared with access groups" }
            @match (all, mine) {
                (Ok(all), Ok(mine)) => {
                    @let available: Vec<&Value> = all.iter().filter(|g| !mine.iter().any(|m| m["id"] == g["id"])).collect();
                    @if mine.is_empty() {
                        p.muted { "Not shared with any group. Only the owner (and admins) can see it." }
                    } @else {
                        ul { @for g in &mine {
                            li {
                                span { strong { (text(g, "name")) } " " span.muted { (text(g, "description")) } }
                                button.btn.btn-ghost.btn-sm
                                    data-on:click=(format!("@post('/ui/wf/{id}/group?verb=remove&group={}')", int(g, "id"))) { "Remove" }
                            }
                        } }
                    }
                    @if !available.is_empty() {
                        div.add {
                            select.input aria-label="Access group to add" data-bind="dlg.group" {
                                option value="" { "Choose a group…" }
                                @for g in &available { option value=(int(g, "id")) { (text(g, "name")) } }
                            }
                            button.btn.btn-sm data-attr:disabled="!$dlg.group"
                                data-on:click=(format!("@post('/ui/wf/{id}/group?verb=add')")) { "Share" }
                        }
                    }
                }
                (Err(e), _) | (_, Err(e)) => {
                    p.muted { "Access groups are unavailable: " (e) }
                }
            }
        }
    }
}

#[derive(Deserialize)]
struct GroupQuery {
    verb: String,
    group: Option<i64>,
}

async fn change_group(
    State(st): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(q): Query<GroupQuery>,
    Sig(s): Sig<EditSig>,
) -> Ui {
    let verb = if q.verb == "add" { "add" } else { "remove" };
    let Some(group) = q.group.or(s.dlg.group) else {
        return Ui::new();
    };
    let r = kit::op(
        &st,
        &format!("{verb}-workflow-group"),
        json!({ "workflow_id": id, "group_id": group }),
    )
    .await;
    let ui = match r {
        Ok(_) => Ui::new().signals(json!({ "dlg": { "group": "" } })),
        Err(e) => Ui::new().err(format!("Could not {verb} group: {e}")),
    };
    ui.patch(groups(&st, id).await)
}

// ============== Import ==============

async fn import() -> Ui {
    let signals = json!({ "dlg": {
        "src": "path", "path": "", "file": [], "name": "", "skipResults": false, "skipEvents": false,
    } });
    let seg = |id: &str, label: &str| {
        html! {
            button type="button" role="tab" data-class:active=(format!("$dlg.src === {}", js(id)))
                data-on:click=(format!("$dlg.src = {}", js(id))) { (label) }
        }
    };
    Ui::new().modal(dialog(
        "Import workflow",
        "md",
        html! {},
        html! {
            form #dlg-form.dlg-form data-signals=(signals.to_string()) data-indicator="_dlgBusy"
                data-on:submit__prevent="@post('/ui/workflows/import')" {
                p.muted { "Import a previously exported workflow JSON file." }
                div.seg role="tablist" { (seg("path", "Server path")) (seg("upload", "Upload file")) }
                label.field data-show="$dlg.src === 'path'" {
                    "Path to exported JSON file on the server"
                    input.input placeholder="/path/to/workflow_export.json" autofocus data-bind="dlg.path";
                }
                div.dlg-drop data-show="$dlg.src === 'upload'" style="display:none" {
                    input type="file" accept=".json,application/json" aria-label="Workflow JSON file" data-bind="dlg.file";
                    (icon("upload", 22))
                    span.loaded data-show="$dlg.file.length"
                        data-text="$dlg.file.length ? 'Loaded ' + $dlg.file[0].name + ' (' + ($dlg.file[0].contents.length * 0.75 / 1024).toFixed(1) + ' KB)' : ''" {}
                    span.faint data-show="$dlg.file.length" { "Drop or click to choose a different file" }
                    span data-show="!$dlg.file.length" { "Drop an exported workflow JSON file here or click to select" }
                }
                label.field {
                    "Name override (optional)"
                    input.input placeholder="Leave empty to use original name" data-bind="dlg.name";
                }
                label.check { input type="checkbox" data-bind="dlg.skipResults"; " Skip job results" }
                label.check { input type="checkbox" data-bind="dlg.skipEvents"; " Skip events" }
                div #dlg-err {}
            }
        },
        html! {
            (cancel_button())
            button.btn.btn-primary type="submit" form="dlg-form" data-attr:disabled="$_dlgBusy"
                data-text="$_dlgBusy ? 'Importing…' : 'Import'" { "Import" }
        },
    ))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Upload {
    contents: String,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ImportForm {
    src: String,
    path: String,
    file: Vec<Upload>,
    name: String,
    skip_results: bool,
    skip_events: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ImportSig {
    dlg: ImportForm,
}

async fn import_submit(State(st): State<Arc<AppState>>, Sig(s): Sig<ImportSig>) -> Ui {
    let f = s.dlg;
    let name = f.name.trim();
    let mut body = json!({
        "name": (!name.is_empty()).then_some(name),
        "skip_results": f.skip_results,
        "skip_events": f.skip_events,
    });
    if f.src == "upload" {
        let Some(up) = f.file.first() else {
            return Ui::new().toast(
                Tone::Warning,
                "Please select a workflow JSON file to upload",
            );
        };
        let content = base64::engine::general_purpose::STANDARD
            .decode(&up.contents)
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .filter(|c| serde_json::from_str::<Value>(c).is_ok());
        let Some(content) = content else {
            return Ui::new().err("Invalid JSON file");
        };
        body["content"] = Value::String(content);
    } else {
        if f.path.trim().is_empty() {
            return Ui::new().toast(Tone::Warning, "Please enter a file path");
        }
        body["file_path"] = Value::String(f.path.trim().to_string());
    }
    let r = kit::call(&st, crate::cli_import_handler, body)
        .await
        .and_then(kit::cli_result);
    match r {
        Ok(out) => {
            let id = serde_json::from_str::<Value>(&out)
                .ok()
                .and_then(|v| v.get("workflow_id").cloned())
                .filter(|v| !v.is_null());
            let msg = match id {
                Some(id) => format!("Workflow imported with ID {id}"),
                None => "Workflow imported successfully".into(),
            };
            Ui::new().close_modal().ok(msg).refresh()
        }
        Err(e) => form_error(&e).err("Import failed"),
    }
}
