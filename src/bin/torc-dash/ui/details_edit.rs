//! Editing a workflow's resource requirements, Slurm schedulers, and per-job settings.
//!
//! Shared definitions are only edited while nothing is running, so running jobs and submitted
//! allocations never disagree with the records that describe them.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::Method,
};
use maud::{Markup, html};
use serde::Deserialize;
use serde_json::{Value, json};

use super::super::kit::{self, G, Sig, Ui, cancel_button, modal};
use super::job::{self, JobSig};
use super::{int, text};
use crate::AppState;

/// Job statuses whose resources can't change: the job is (about to be) running with them.
const JOB_BUSY: [&str; 2] = ["running", "pending"];
const PRE_RUN: [&str; 3] = ["uninitialized", "blocked", "ready"];
const RR_FIELDS: [&str; 5] = ["num_cpus", "memory", "num_gpus", "num_nodes", "runtime"];
const NUM_FIELDS: [&str; 4] = ["num_cpus", "num_gpus", "num_nodes", "nodes"];

/// A number from a signal, which inputs may send as a string.
fn num_of(v: &Value, k: &str) -> Option<i64> {
    match v.get(k)? {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Signal values with numeric fields as numbers.
fn coerce(mut v: Value) -> Value {
    for k in NUM_FIELDS {
        if let Some(n) = num_of(&v, k) {
            v[k] = json!(n);
        }
    }
    v
}

async fn put(st: &AppState, path: &str, body: &Value) -> Result<Value, String> {
    kit::api(st, Method::PUT, path, &[], Some(body)).await
}

async fn post(st: &AppState, path: &str, body: &Value) -> Result<Value, String> {
    kit::api(st, Method::POST, path, &[], Some(body)).await
}

async fn total(st: &AppState, path: &str, wf: i64, q: (&'static str, &str)) -> Result<i64, String> {
    let query = [
        ("workflow_id", wf.to_string()),
        ("limit", "1".into()),
        (q.0, q.1.to_string()),
    ];
    let v: Value = kit::get(st, path, &query).await?;
    Ok(int(&v, "total_count").unwrap_or(0))
}

/// Why the workflow's shared definitions can't be edited now, or None if they can.
async fn edit_blocked_reason(st: &AppState, wf: i64) -> Result<Option<String>, String> {
    let (running, pending, active, queued) = futures::join!(
        total(st, "/jobs", wf, ("status", "running")),
        total(st, "/jobs", wf, ("status", "pending")),
        total(st, "/scheduled_compute_nodes", wf, ("status", "active")),
        total(st, "/scheduled_compute_nodes", wf, ("status", "pending")),
    );
    let (jobs, nodes) = (running? + pending?, active? + queued?);
    Ok(if jobs > 0 {
        Some(format!("{jobs} job(s) are running or pending."))
    } else if nodes > 0 {
        Some(format!("{nodes} Slurm allocation(s) are queued or active."))
    } else {
        None
    })
}

/// How many jobs use a resource requirement.
async fn jobs_using(st: &AppState, wf: i64, rr: i64) -> Result<i64, String> {
    total(
        st,
        "/jobs",
        wf,
        ("resource_requirements_id", &rr.to_string()),
    )
    .await
}

enum CommandEdit {
    Direct,
    Reset,
    /// Why the command can't change.
    No(String),
}

/// How a job's command can be changed. The server accepts command changes only for
/// uninitialized jobs (initialization hashes the command). A blocked or ready job can still be
/// edited while the whole workflow is pre-run: reset every job, edit, and initialize again,
/// losing nothing.
async fn command_edit_mode(st: &AppState, job: &Value) -> CommandEdit {
    if text(job, "status") == "uninitialized" {
        return CommandEdit::Direct;
    }
    let wf = int(job, "workflow_id").unwrap_or_default();
    let path = format!("/workflows/{wf}/status");
    let (status, blocked) = futures::join!(
        kit::get::<Value>(st, &path, &[]),
        edit_blocked_reason(st, wf)
    );
    let check = |e: String| CommandEdit::No(format!("Could not check job status: {e}"));
    let status = match status {
        Ok(s) => s,
        Err(e) => return check(e),
    };
    match blocked {
        Err(e) => return check(e),
        Ok(Some(why)) => return CommandEdit::No(why),
        Ok(None) => {}
    }
    let by = status.get("jobs_by_status").cloned().unwrap_or_default();
    let ran: Vec<String> = kit::JOB_STATUSES
        .iter()
        .filter(|s| !PRE_RUN.contains(s))
        .filter_map(|s| int(&by, s).filter(|n| *n > 0).map(|n| format!("{n} {s}")))
        .collect();
    if ran.is_empty() {
        CommandEdit::Reset
    } else {
        CommandEdit::No(format!(
            "Commands can change only before any job runs ({}). Reset the workflow first.",
            ran.join(", ")
        ))
    }
}

/// Give one job new resource values. A requirement used only by this job is updated in place;
/// a shared one is cloned (as `<name>_job<id>`) so the other jobs keep theirs. Returns whether
/// it was cloned.
async fn set_job_resources(
    st: &AppState,
    job: &Value,
    current: Option<&Value>,
    fields: &Value,
) -> Result<bool, String> {
    let wf = int(job, "workflow_id").unwrap_or_default();
    let id = int(job, "id").unwrap_or_default();
    let mut values = serde_json::Map::new();
    for k in RR_FIELDS {
        values.insert(k.into(), fields.get(k).cloned().unwrap_or(Value::Null));
    }
    if let Some(cur) = current
        && let Some(rr) = int(cur, "id")
        && jobs_using(st, wf, rr).await? <= 1
    {
        let mut body = cur.clone();
        for (k, v) in values {
            body[k] = v;
        }
        put(st, &format!("/resource_requirements/{rr}"), &body).await?;
        return Ok(false);
    }
    let name = current
        .map(|c| text(c, "name"))
        .unwrap_or_else(|| "resources".into());
    let mut body = json!({ "workflow_id": wf, "name": format!("{name}_job{id}") });
    for (k, v) in values {
        body[k] = v;
    }
    let created = post(st, "/resource_requirements", &body).await?;
    let mut j = job.clone();
    j["resource_requirements_id"] = created.get("id").cloned().unwrap_or(Value::Null);
    put(st, &format!("/jobs/{id}"), &j).await?;
    Ok(true)
}

/// Inputs for a resource requirement's CPUs, memory, GPUs, nodes, and runtime, bound to
/// `$<prefix>.<field>`.
fn resource_inputs(prefix: &str, disabled: bool) -> Markup {
    let b = |k: &str| format!("{prefix}.{k}");
    html! {
        div.res-grid {
            label.field { "CPUs" input.input type="number" min="1" data-bind=(b("num_cpus")) disabled[disabled] required; }
            label.field {
                "Memory"
                input.input.mono data-bind=(b("memory")) disabled[disabled] required;
                span.hint { "e.g. 512m, 4g" }
            }
            label.field { "GPUs" input.input type="number" min="0" data-bind=(b("num_gpus")) disabled[disabled] required; }
            label.field { "Nodes" input.input type="number" min="1" data-bind=(b("num_nodes")) disabled[disabled] required; }
            label.field {
                "Runtime"
                input.input.mono data-bind=(b("runtime")) disabled[disabled] required;
                span.hint { "ISO 8601, e.g. PT30M, PT2H, P1DT4H" }
            }
        }
    }
}

fn rr_values(r: Option<&Value>) -> Value {
    json!({
        "num_cpus": r.and_then(|r| int(r, "num_cpus")).unwrap_or(1),
        "memory": r.map(|r| text(r, "memory")).filter(|s| !s.is_empty()).unwrap_or_else(|| "1m".into()),
        "num_gpus": r.and_then(|r| int(r, "num_gpus")).unwrap_or(0),
        "num_nodes": r.and_then(|r| int(r, "num_nodes")).unwrap_or(1),
        "runtime": r.map(|r| text(r, "runtime")).filter(|s| !s.is_empty()).unwrap_or_else(|| "PT1H".into()),
    })
}

// ============== Edit job ==============

/// `GET /ui/details/job/{id}/edit`: edit one job's command, priority, and resource
/// requirements. The job can use another existing requirement or new values; a requirement
/// shared with other jobs is cloned for this job, so the others are not affected.
pub async fn edit_job(State(st): State<Arc<AppState>>, Path(id): Path<i64>) -> Ui {
    let job: Value = match kit::get(&st, &format!("/jobs/{id}"), &[]).await {
        Ok(j) => j,
        Err(e) => return Ui::new().err(format!("Could not load job {id}: {e}")),
    };
    let wf = int(&job, "workflow_id").unwrap_or_default();
    let q = [("workflow_id", wf.to_string())];
    let (records, mode) = futures::join!(
        kit::list_all::<Value>(&st, "/resource_requirements", &q),
        command_edit_mode(&st, &job),
    );
    let (records, records_error) = match records {
        Ok(r) => (r, None),
        Err(e) => (Vec::new(), Some(e)),
    };
    let rr = int(&job, "resource_requirements_id");
    let selected = records.iter().find(|r| int(r, "id") == rr);
    let rr_missing = rr.is_some() && selected.is_none();
    let status = text(&job, "status");
    let busy = JOB_BUSY.contains(&status.as_str());
    let editable = matches!(mode, CommandEdit::Direct | CommandEdit::Reset);
    let name = text(&job, "name");

    let mut ej = rr_values(selected);
    ej["command"] = json!(text(&job, "command"));
    ej["priority"] = json!(int(&job, "priority").unwrap_or(0));
    ej["rr"] = json!(rr);

    let save_disabled = busy || rr_missing;
    let body = html! {
        form #edit-job.form data-on:submit__prevent=(format!("@post('/ui/details/job/{id}/save')")) data-indicator="_ejSaving" {
            @if busy { p.note { "This job is " (status) ". Edit it after it finishes." } }
            label.field {
                "Command"
                textarea.input.mono rows="3" spellcheck="false" data-bind="details.ej.command" disabled[busy || !editable] { (text(&job, "command")) }
                @match &mode {
                    CommandEdit::Reset => span.hint { "Changing it resets the workflow's jobs and initializes it again. Nothing has run yet, so nothing is lost." },
                    CommandEdit::No(why) => span.hint { (why) },
                    CommandEdit::Direct => {},
                }
            }
            label.field {
                "Priority"
                input.input type="number" min="0" step="1" data-bind="details.ej.priority" disabled[busy] required;
                span.hint { "Ready jobs with higher priority are started first." }
            }
            h4 { "Resources" }
            label.field {
                "Resource requirements"
                select.select data-bind="details.ej.rr" disabled[busy || records.is_empty()]
                    data-on:change=(format!("@post('/ui/details/job/{id}/pick')")) {
                    @for r in &records {
                        option value=(text(r, "id")) selected[int(r, "id") == rr] { (text(r, "name")) }
                    }
                }
            }
            (resource_inputs("details.ej", busy))
            @if selected.is_some() {
                p.muted { "If the selected requirements are shared with other jobs and you change their values, this job gets its own copy and the others keep theirs." }
            }
            p.muted { "New values apply the next time the job runs. Reset it to rerun it." }
            @if let Some(e) = records_error {
                p.error-text { "Could not load resource requirements: " (e) }
            } @else if rr_missing && !records.is_empty() {
                p.error-text { "The job's resource requirements (id " (rr.unwrap_or_default()) ") were not found." }
            }
            p #ej-error.error-text {}
        }
    };
    let footer = html! {
        button.btn data-on:click=(format!("@get('/ui/job/{id}')")) { "Cancel" }
        button.btn.btn-primary type="submit" form="edit-job" disabled[save_disabled]
            data-attr:disabled=[(!save_disabled).then_some("$_ejSaving")] {
            span data-show="$_ejSaving" style="display:none" { "Saving…" }
            span data-show="!$_ejSaving" { "Save" }
        }
    };
    Ui::new()
        .signals(json!({ "details": { "ej": ej } }))
        .close_modal()
        .modal(modal(&format!("Edit job {name}"), "lg", body, Some(footer)))
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct EjSig {
    details: EjNs,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct EjNs {
    ej: Value,
}

/// Picking another requirement shows its values, which can then be adjusted.
pub async fn pick_resources(State(st): State<Arc<AppState>>, Sig(s): Sig<EjSig>) -> Ui {
    let Some(rr) = num_of(&s.details.ej, "rr") else {
        return Ui::new();
    };
    match kit::get::<Value>(&st, &format!("/resource_requirements/{rr}"), &[]).await {
        Ok(r) => Ui::new().signals(json!({ "details": { "ej": rr_values(Some(&r)) } })),
        Err(e) => Ui::new().inner("#ej-error", html! { (e) }),
    }
}

/// `POST /ui/details/job/{id}/save`.
pub async fn save_job(
    State(st): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Sig(s): Sig<JobSig>,
) -> Ui {
    let fail = |e: String| Ui::new().inner("#ej-error", html! { (e) });
    let ej = coerce(s.details.ej.clone());
    let get_job = || async { kit::get::<Value>(&st, &format!("/jobs/{id}"), &[]).await };

    // The job may have started since the dialog opened, and the server doesn't stop a running
    // job's requirement from being rewritten.
    let job = match get_job().await {
        Ok(j) => j,
        Err(e) => return fail(e),
    };
    let status = text(&job, "status");
    if JOB_BUSY.contains(&status.as_str()) {
        return fail(format!(
            "This job is now {status}. Edit it after it finishes."
        ));
    }
    let wf = int(&job, "workflow_id").unwrap_or_default();
    let command = ej
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let command_changed = command != text(&job, "command");
    if command_changed && command.is_empty() {
        return fail("The command can't be empty.".into());
    }
    let priority = num_of(&ej, "priority").unwrap_or(0);
    let rr = num_of(&ej, "rr");
    let selected = match rr {
        Some(r) => {
            match kit::get::<Value>(&st, &format!("/resource_requirements/{r}"), &[]).await {
                Ok(v) => Some(v),
                Err(_) => {
                    return fail(
                        "The job's resource requirements haven't loaded; reopen this dialog."
                            .into(),
                    );
                }
            }
        }
        None => None,
    };
    let rr_changed = rr != int(&job, "resource_requirements_id");
    let fields_changed = selected
        .as_ref()
        .is_none_or(|sel| RR_FIELDS.iter().any(|k| sel.get(*k) != ej.get(*k)));

    // Mode may be stale: re-check right before a reset so nothing that started since is lost.
    let reset = command_changed && matches!(command_edit_mode(&st, &job).await, CommandEdit::Reset);
    if command_changed && !reset && status != "uninitialized" {
        return fail("The command can no longer be changed; reopen this dialog to see why.".into());
    }
    let wid = json!({ "workflow_id": wf.to_string() });
    let name = text(&job, "name");

    let result: Result<bool, String> = async {
        if reset {
            kit::call(&st, crate::cli_reset_status_handler, wid.clone())
                .await
                .and_then(kit::cli_result)?;
        }
        if rr_changed && let Some(r) = rr {
            let mut j = get_job().await?;
            j["resource_requirements_id"] = json!(r);
            put(&st, &format!("/jobs/{id}"), &j).await?;
        }
        let mut cloned = false;
        if fields_changed {
            cloned = set_job_resources(&st, &get_job().await?, selected.as_ref(), &ej).await?;
        }
        if command_changed || priority != int(&job, "priority").unwrap_or(0) {
            // Re-read the job: the steps above may have changed its status or requirement.
            let mut j = get_job().await?;
            j["command"] = json!(command);
            j["priority"] = json!(priority);
            put(&st, &format!("/jobs/{id}"), &j).await?;
        }
        Ok(cloned)
    }
    .await;

    let mut ui = Ui::new();
    // Restore the workflow's state even if an edit failed after the reset. It was initialized
    // before, so skip the missing-input-files check that initialization already passed.
    if reset {
        let mut body = wid;
        body["force"] = json!(true);
        if let Err(e) = kit::call(&st, crate::cli_initialize_handler, body)
            .await
            .and_then(kit::cli_result)
        {
            ui = ui.err(format!("Workflow {wf} was reset but not initialized: {e}"));
        }
    }
    match result {
        Ok(cloned) => {
            tracing::info!("Updated job workflow_id={wf} job_id={id}");
            let note = if cloned {
                " (gave it its own resource requirements)"
            } else {
                ""
            };
            let ui = ui.ok(format!("Job {name} updated{note}")).refresh();
            job::job_ui(ui, &st, id, s, false).await
        }
        Err(e) => ui.refresh().inner("#ej-error", html! { (e) }),
    }
}

// ============== Edit resource requirements / Slurm schedulers ==============

#[derive(Deserialize)]
pub struct DefPath {
    kind: String,
    id: String,
}

fn def_title(kind: &str, new: bool, rec: &Value) -> String {
    match (kind, new) {
        ("resources", true) => "Add resource requirements".into(),
        (_, true) => "Add Slurm scheduler".into(),
        ("resources", false) => format!("Edit resource requirements {}", text(rec, "name")),
        _ => {
            let n = text(rec, "name");
            format!(
                "Edit Slurm scheduler {}",
                if n.is_empty() { text(rec, "id") } else { n }
            )
        }
    }
}

/// `GET /ui/details/def/{resources|scheduler}/{id|new}`: add a resource requirement or Slurm
/// scheduler, or edit one in place. Every job (or allocation) that uses an existing record picks
/// up the change, so editing is allowed only while nothing in the workflow is running.
pub async fn edit_definition(
    State(st): State<Arc<AppState>>,
    Path(p): Path<DefPath>,
    Sig(g): Sig<G>,
) -> Ui {
    let resources = p.kind == "resources";
    let new = p.id == "new";
    let rec: Value = if new {
        let Some(wf) = g.wf else {
            return Ui::new().err("Pick a workflow first");
        };
        if resources {
            json!({ "workflow_id": wf, "name": "", "num_cpus": 1, "memory": "1g", "num_gpus": 0, "num_nodes": 1, "runtime": "PT1H" })
        } else {
            json!({ "workflow_id": wf, "name": "", "account": "", "nodes": 1, "walltime": "04:00:00" })
        }
    } else {
        let path = if resources {
            "/resource_requirements"
        } else {
            "/slurm_schedulers"
        };
        match kit::get(&st, &format!("{path}/{}", p.id), &[]).await {
            Ok(r) => r,
            Err(e) => return Ui::new().err(format!("Could not load {}: {e}", p.id)),
        }
    };
    let wf = int(&rec, "workflow_id").unwrap_or_default();
    let blocked = if new {
        None
    } else {
        edit_blocked_reason(&st, wf)
            .await
            .unwrap_or_else(|e| Some(format!("Could not check job status: {e}")))
    };
    let users = match (resources && !new, int(&rec, "id")) {
        (true, Some(rr)) => jobs_using(&st, wf, rr).await.ok(),
        _ => None,
    };
    let disabled = blocked.is_some();
    let f = |label: &str, key: &str, class: &str, placeholder: &str, required: bool| {
        html! {
            label.field {
                (label)
                input.input.(class) data-bind=(format!("details.def.{key}")) disabled[disabled]
                    required[required] placeholder=[(!placeholder.is_empty()).then_some(placeholder)];
            }
        }
    };
    let body = html! {
        form #edit-def.form data-on:submit__prevent=(format!("@post('/ui/details/def/{}/{}/save')", p.kind, p.id)) data-indicator="_defSaving" {
            @if let Some(b) = &blocked {
                p.note { (b) " Edit this after they finish, or cancel the workflow. To change one job's resources, edit the job instead." }
            }
            @if resources {
                (f("Name", "name", "", "", true))
                (resource_inputs("details.def", disabled))
                @if let Some(n) = users {
                    p.muted { "Used by " (n) " job(s). All of them get the new values the next time they run." }
                }
            } @else {
                div.res-grid {
                    (f("Name", "name", "", "", false))
                    (f("Account", "account", "", "", true))
                    (f("Partition", "partition", "", "Default", false))
                    (f("QoS", "qos", "", "", false))
                    label.field { "Nodes" input.input type="number" min="1" data-bind="details.def.nodes" disabled[disabled] required; }
                    (f("Walltime", "walltime", "mono", "04:00:00", true))
                    (f("Memory", "mem", "mono", "e.g. 240G", false))
                    (f("GRES", "gres", "mono", "e.g. gpu:2", false))
                    (f("Tmp", "tmp", "mono", "", false))
                }
                (f("Extra sbatch options", "extra", "mono", "--constraint=…", false))
                @if !new { p.muted { "Applies to allocations submitted from now on." } }
            }
            p #def-error.error-text {}
        }
    };
    let footer = html! {
        (cancel_button())
        button.btn.btn-primary type="submit" form="edit-def" disabled[disabled]
            data-attr:disabled=[(!disabled).then_some("$_defSaving")] {
            (if new { "Add" } else { "Save" })
        }
    };
    // Optional scheduler fields must exist as signals for their inputs to bind.
    let mut rec = rec;
    if !resources {
        for k in ["partition", "qos", "mem", "gres", "tmp", "extra"] {
            if rec.get(k).is_none_or(Value::is_null) {
                rec[k] = json!("");
            }
        }
    }
    let title = def_title(&p.kind, new, &rec);
    Ui::new()
        .signals(json!({ "details": { "def": null } }))
        .signals(json!({ "details": { "def": rec } }))
        .close_modal()
        .modal(modal(&title, "lg", body, Some(footer)))
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct DefSig {
    details: DefNs,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct DefNs {
    def: Value,
}

/// `POST /ui/details/def/{kind}/{id|new}/save`.
pub async fn save_definition(
    State(st): State<Arc<AppState>>,
    Path(p): Path<DefPath>,
    Sig(s): Sig<DefSig>,
) -> Ui {
    let fail = |e: String| Ui::new().inner("#def-error", html! { (e) });
    let new = p.id == "new";
    let mut body = coerce(s.details.def);
    let wf = int(&body, "workflow_id").unwrap_or_default();
    // Work may have started since the dialog opened.
    // ponytail: check-then-update is not atomic; enforce on the server if that window matters.
    if !new {
        match edit_blocked_reason(&st, wf).await {
            Ok(None) => {}
            Ok(Some(b)) => return fail(b),
            Err(e) => return fail(format!("Could not check job status: {e}")),
        }
    }
    let path = if p.kind == "resources" {
        "/resource_requirements"
    } else {
        // Optional text fields: on update the server keeps a null value and clears an empty
        // one; a new scheduler leaves blank ones unset.
        for k in ["partition", "qos", "mem", "gres", "tmp", "extra"] {
            let v = text(&body, k).trim().to_string();
            body[k] = match (v.is_empty(), new) {
                (false, _) => json!(v),
                (true, true) => Value::Null,
                (true, false) => json!(""),
            };
        }
        "/slurm_schedulers"
    };
    let r = if new {
        post(&st, path, &body).await
    } else {
        put(&st, &format!("{path}/{}", p.id), &body).await
    };
    match r {
        Ok(_) => {
            tracing::info!("Saved {} workflow_id={wf} id={}", p.kind, p.id);
            Ui::new()
                .ok(if new { "Added" } else { "Saved" })
                .close_modal()
                .refresh()
        }
        Err(e) => fail(e),
    }
}
