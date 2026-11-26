//! Workflows tab: the workflow list, bulk actions, and the per-workflow action menu and actions
//! (also used by the Details tab header via [`actions`]).
//!
//! Dialog actions (plan, recover, sync, export, schedule, edit, import) live in `wf_dialogs`,
//! and run/watch output in `exec`.

use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use maud::{Markup, html};
use serde::Deserialize;
use serde_json::{Value, json};
use torc::models::WorkflowModel;

use super::Ctx;
use super::kit::{
    self, Col, Sig, SortKey, Table, TableState, Tone, Ui, badge, cancel_button, filter_box, icon,
    js, modal, selected_ids,
};
use crate::AppState;

const RELOAD: &str = "@get('/ui/tab')";

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ui/wf/{id}/menu", get(menu))
        .route("/ui/wf/{id}/submit", post(submit))
        .route("/ui/wf/{id}/initialize", post(initialize))
        .route("/ui/wf/{id}/reinitialize", post(reinitialize))
        .route("/ui/wf/{id}/reset", post(reset))
        .route("/ui/wf/{id}/cancel", post(cancel))
        .route("/ui/wf/{id}/delete", post(delete_one))
        .route("/ui/wf/{id}/archive", post(archive_one))
        .route("/ui/wf/{id}/stop-run", post(stop_run))
        .route("/ui/wf/{id}/logs", post(download_logs))
        .route("/ui/workflows/archive", post(bulk_archive))
        .route("/ui/workflows/delete", post(bulk_delete))
        .route(
            "/ui/workflows/delete-confirmed",
            post(bulk_delete_confirmed),
        )
}

pub async fn view(st: &Arc<AppState>, cx: &Ctx) -> Markup {
    // The first render comes before the page's `$wfs` signals exist: start newest first, as
    // they do.
    let t: TableState = match cx.sig.get("wfs") {
        Some(_) => kit::ns(&cx.sig, "wfs"),
        None => TableState {
            sort: "id".into(),
            desc: true,
            ..Default::default()
        },
    };
    let (rows, error) =
        match kit::list_all::<WorkflowModel>(st, "/workflows", &cx.g.workflow_query()).await {
            Ok(r) => (r, None),
            Err(e) => (Vec::new(), Some(e)),
        };
    let running = local_runs(st).await;

    let cols = vec![
        Col::num("id", "ID", |w: &WorkflowModel| w.id.map(|i| i as f64)).width("72px"),
        Col::text("name", "Name", |w: &WorkflowModel| w.name.clone()).cell(|w| {
            html! {
                span.wf-name {
                    span.ellipsis title=(w.name) { (if w.name.is_empty() { "Unnamed" } else { &w.name }) }
                    @if w.is_canceled == Some(true) { (kit::status_badge("canceled")) }
                    @if w.is_archived == Some(true) { (badge(Tone::Neutral, "archived")) }
                }
            }
        }),
        Col::text("user", "User", |w: &WorkflowModel| w.user.clone()).width("12%"),
        Col::text("description", "Description", |w: &WorkflowModel| w.description.clone().unwrap_or_default())
            .width("28%")
            .cell(|w| {
                let d = w.description.as_deref().unwrap_or("");
                html! { span.muted title=(d) { (if d.is_empty() { "—" } else { d }) } }
            }),
        Col::text("timestamp", "Created", |w: &WorkflowModel| w.timestamp.clone().unwrap_or_default())
            .width("130px")
            .sort_by(|w| {
                w.timestamp
                    .as_deref()
                    .and_then(kit::to_datetime)
                    .map(|d| SortKey::Num(d.timestamp() as f64))
                    .unwrap_or(SortKey::None)
            })
            .cell(|w| {
                let ts = w.timestamp.as_deref();
                html! { span title=(kit::fmt_datetime(ts)) { (kit::fmt_relative(ts)) } }
            }),
        Col::text("actions", "", |_: &WorkflowModel| String::new())
            .width("124px")
            .unsortable()
            .cell(|w| {
                let id = w.id.unwrap_or_default();
                actions(id, true, w.is_archived == Some(true), running.contains(&id))
            }),
    ];
    let mut table = Table::new(
        "wfs",
        RELOAD,
        |w: &WorkflowModel| w.id.unwrap_or_default(),
        cols,
    );
    table.selectable = true;
    table.active = cx.g.wf;
    table.on_click = Some(Box::new(|w| {
        format!(
            "$wf = {}; location.hash = '/details'",
            w.id.unwrap_or_default()
        )
    }));
    table.empty = if cx.g.group.trim().is_empty() {
        "No workflows yet. Create one to get started.".into()
    } else {
        format!("No workflows shared with group \"{}\"", cx.g.group.trim())
    };

    let any_selected = "Object.values($wfs.sel || {}).some(Boolean)";
    html! {
        div.page data-signals__ifmissing="{wfs: {filter: '', sort: 'id', desc: true, page: 0, sel: {}}}" {
            div.page-header {
                (filter_box("wfs", RELOAD, "Filter workflows…  /"))
                input.input.group type="search" placeholder="Access group" aria-label="Access group"
                    title="Show workflows shared with this access group (press Enter)"
                    data-bind="group" data-on:change=(RELOAD);
                label.check title="Show workflows owned by every user" {
                    input type="checkbox" data-bind="allUsers" data-on:change=(RELOAD);
                    "All users"
                }
                label.check title="Include archived workflows" {
                    input type="checkbox" data-bind="archived" data-on:change=(RELOAD);
                    "Archived"
                }
                div.grow {}
                button.btn data-on:click="@get('/ui/workflows/import')" { (icon("download", 14)) " Import" }
                button.btn.btn-primary title="New workflow (N)"
                    data-on:click="el.dispatchEvent(new CustomEvent('torc-create', {bubbles: true}))" {
                    (icon("plus", 14)) " Create workflow"
                }
            }
            div.bulk data-show=(any_selected) style="display:none" {
                strong data-text="Object.values($wfs.sel || {}).filter(Boolean).length + ' selected'" {}
                button.btn.btn-sm data-on:click="@post('/ui/workflows/archive?archived=true')" { "Archive" }
                @if cx.g.archived {
                    button.btn.btn-sm data-on:click="@post('/ui/workflows/archive?archived=false')" { "Unarchive" }
                }
                button.btn.btn-sm.btn-danger data-on:click="@post('/ui/workflows/delete')" {
                    (icon("trash", 13)) " Delete selected"
                }
                button.btn.btn-sm.btn-ghost data-on:click="@setAll(false, {include: /^wfs\\.sel\\./})" { "Clear selection" }
            }
            @if let Some(e) = error { p.error-text { "Error loading workflows: " (e) } }
            div.table-host { (table.render(&rows, &t)) }
        }
    }
}

/// Ids of workflows with a `torc run` started from this dashboard still running.
pub async fn local_runs(st: &AppState) -> Vec<i64> {
    st.local_runs
        .lock()
        .await
        .iter()
        .filter(|(_, pids)| !pids.is_empty())
        .filter_map(|(id, _)| id.parse().ok())
        .collect()
}

/// Buttons for one workflow. compact: Run and Submit icons plus the "more" menu (table rows);
/// otherwise labeled primary buttons plus the menu (Details tab header). `running`: a local run
/// started from this dashboard is active.
pub fn actions(id: i64, compact: bool, archived: bool, running: bool) -> Markup {
    let run = format!("@post('/ui/exec/{id}/run')");
    let submit = format!(
        "confirm({}) && @post('/ui/wf/{id}/submit')",
        js(&format!(
            "Submit workflow {id}?\n\nSubmit this workflow to the scheduler (e.g., Slurm)."
        ))
    );
    let more = format!(
        "torcAnchor(el); @get('/ui/wf/{id}/menu?compact={}&archived={}')",
        compact as u8, archived as u8
    );
    // Clicks must not reach the table row (which opens the workflow).
    html! {
        div.wf-actions.compact[compact] data-on:click="evt.stopPropagation()" {
            @if compact {
                button.btn.btn-ghost.btn-icon.btn-sm.run title=(if running { "Running…" } else { "Run locally" })
                    aria-label="Run locally" disabled[running] data-on:click=(run) {
                    @if running { span.spinner.small {} } @else { (icon("play", 14)) }
                }
                button.btn.btn-ghost.btn-icon.btn-sm.submit title="Submit to scheduler" aria-label="Submit to scheduler"
                    data-on:click=(submit) { (icon("upload", 14)) }
                button.btn.btn-ghost.btn-icon.btn-sm title="More actions" aria-label="More actions" data-on:click=(more) {
                    (icon("more", 14))
                }
            } @else {
                button.btn.btn-sm.exec disabled[running] data-on:click=(run) {
                    @if running { span.spinner.small {} " Running…" } @else { (icon("play", 13)) " Run" }
                }
                button.btn.btn-sm.exec data-on:click=(submit) { (icon("upload", 13)) " Submit" }
                span.sep aria-hidden="true" {}
                button.btn.btn-sm data-on:click=(format!("@post('/ui/wf/{id}/initialize')")) { "Initialize" }
                button.btn.btn-sm data-on:click=(format!("@post('/ui/wf/{id}/reinitialize')")) { "Reinitialize" }
                button.btn.btn-sm data-on:click=(format!("@get('/ui/wf/{id}/plan')")) { "Execution plan" }
                button.btn.btn-sm.btn-icon title="More actions" aria-label="More actions" data-on:click=(more) {
                    (icon("more", 14))
                }
            }
        }
    }
}

#[derive(Deserialize)]
struct MenuQuery {
    #[serde(default)]
    compact: u8,
    #[serde(default)]
    archived: u8,
}

/// (label, icon, danger, Datastar action)
type Item = (&'static str, &'static str, bool, String);

/// The "more actions" menu for a workflow, opened under the button that asked for it.
async fn menu(Path(id): Path<i64>, Query(q): Query<MenuQuery>) -> Ui {
    let post = |a: &str| format!("@post('/ui/wf/{id}/{a}')");
    let dialog = |d: &str| format!("@get('/ui/wf/{id}/{d}')");
    let confirm = |msg: String, a: &str| format!("confirm({}) && {}", js(&msg), post(a));

    let init: Item = ("Initialize", "", false, post("initialize"));
    let reinit: Item = ("Reinitialize", "", false, post("reinitialize"));
    let reset: Item = (
        "Reset status",
        "",
        false,
        confirm(
            format!("Reset workflow {id}?\n\nThis sets all jobs back to the uninitialized state."),
            "reset",
        ),
    );
    let watch: Item = ("Watch", "", false, format!("@post('/ui/exec/{id}/watch')"));
    let watch_recover: Item = (
        "Watch with auto-recovery",
        "",
        false,
        format!("@post('/ui/exec/{id}/watch-recover')"),
    );
    let stop: Item = ("Stop local run", "stop", false, post("stop-run"));
    let graph: Item = (
        "View graph",
        "graph",
        false,
        format!("$wf = {id}; location.hash = '/dag'"),
    );
    let plan: Item = ("Execution plan", "", false, dialog("plan"));
    let schedule: Item = ("Schedule Slurm nodes…", "", false, dialog("schedule"));
    let recover: Item = ("Recover failed jobs…", "", false, dialog("recover"));
    let sync: Item = ("Sync Slurm status…", "", false, dialog("sync"));
    let edit: Item = ("Edit & share…", "", false, dialog("edit"));
    let export: Item = ("Export…", "download", false, dialog("export"));
    let logs: Item = ("Download logs", "download", false, post("logs"));
    let archive: Item = if q.archived == 1 {
        (
            "Unarchive",
            "",
            false,
            format!("@post('/ui/wf/{id}/archive?archived=false')"),
        )
    } else {
        (
            "Archive",
            "",
            false,
            format!("@post('/ui/wf/{id}/archive?archived=true')"),
        )
    };
    let cancel: Item = (
        "Cancel workflow",
        "stop",
        true,
        confirm(
            format!("Cancel workflow {id}?\n\nThis will terminate all running Slurm jobs."),
            "cancel",
        ),
    );
    let delete: Item = (
        "Delete",
        "trash",
        true,
        confirm(
            format!("Delete workflow {id}?\n\nThis action cannot be undone."),
            "delete",
        ),
    );

    // Stop can't end a run cleanly on a Windows host, so it isn't offered there.
    let mut runs = vec![watch, watch_recover];
    if cfg!(unix) {
        runs.push(stop);
    }
    let groups: Vec<Vec<Item>> = if q.compact == 1 {
        vec![
            vec![init, reinit, reset],
            runs,
            vec![graph, plan],
            vec![schedule, recover, sync],
            vec![edit, export, logs, archive],
            vec![cancel, delete],
        ]
    } else {
        vec![
            runs,
            vec![reset],
            vec![schedule, recover, sync],
            vec![edit, export, logs, archive],
            vec![cancel, delete],
        ]
    };

    Ui::new()
        .inner(
            "#menu",
            html! {
                @for (gi, group) in groups.iter().enumerate() {
                    @if gi > 0 { hr; }
                    @for (label, ic, danger, action) in group {
                        button role="menuitem" .danger[*danger]
                            data-on:click=(format!("document.getElementById('menu').hidePopover(); {action}")) {
                            span.ico { (icon(ic, 14)) }
                            (label)
                        }
                    }
                }
            },
        )
        .script("torcMenu()")
}

fn wid(id: i64) -> Value {
    json!({ "workflow_id": id.to_string() })
}

/// Toast the outcome of a CLI action and refresh on success.
fn done(r: Result<String, String>, what: &str, ok: String) -> Ui {
    match r {
        Ok(_) => Ui::new().ok(ok).refresh(),
        Err(e) => Ui::new().err(format!("{what} failed: {e}")),
    }
}

async fn submit(State(st): State<Arc<AppState>>, Path(id): Path<i64>) -> Ui {
    super::events::follow(id);
    let r = kit::call(&st, crate::cli_submit_handler, wid(id))
        .await
        .and_then(kit::cli_result);
    done(r, "Submit", format!("Workflow {id} submitted"))
}

async fn reset(State(st): State<Arc<AppState>>, Path(id): Path<i64>) -> Ui {
    let r = kit::call(&st, crate::cli_reset_status_handler, wid(id))
        .await
        .and_then(kit::cli_result);
    done(r, "Reset", format!("Workflow {id} status reset"))
}

async fn cancel(State(st): State<Arc<AppState>>, Path(id): Path<i64>) -> Ui {
    let r = kit::call(&st, crate::cli_cancel_handler, wid(id))
        .await
        .and_then(kit::cli_result);
    done(r, "Cancel", format!("Workflow {id} canceled"))
}

async fn stop_run(State(st): State<Arc<AppState>>, Path(id): Path<i64>) -> Ui {
    let r: Result<Value, String> = kit::call(&st, crate::cli_stop_run_handler, wid(id)).await;
    match r {
        Ok(v) if v.get("stopped").and_then(Value::as_bool) == Some(true) => Ui::new()
            .ok(format!("Stopping the local run of workflow {id}"))
            .refresh(),
        Ok(v) if v.get("success").and_then(Value::as_bool) == Some(false) => {
            Ui::new().err(format!(
                "Stop failed: {}",
                v.get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown error")
            ))
        }
        Ok(_) => Ui::new().toast(
            Tone::Info,
            format!("No local run of workflow {id} started from this dashboard is active"),
        ),
        Err(e) => Ui::new().err(format!("Stop failed: {e}")),
    }
}

#[derive(Deserialize)]
struct ArchiveQuery {
    archived: bool,
}

async fn archive(st: &Arc<AppState>, ids: &[i64], archived: bool) -> Ui {
    if ids.is_empty() {
        return Ui::new();
    }
    let noun = if ids.len() == 1 {
        format!("Workflow {}", ids[0])
    } else {
        format!("{} workflows", ids.len())
    };
    let verb = if archived { "archived" } else { "unarchived" };
    match kit::op(
        st,
        "archive",
        json!({ "workflow_ids": ids, "archived": archived }),
    )
    .await
    {
        Ok(_) => Ui::new().ok(format!("{noun} {verb}")).refresh(),
        Err(e) => Ui::new().err(format!(
            "{} failed: {e}",
            if archived { "Archive" } else { "Unarchive" }
        )),
    }
}

async fn archive_one(
    State(st): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(q): Query<ArchiveQuery>,
) -> Ui {
    archive(&st, &[id], q.archived).await
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct BulkSig {
    wfs: TableState,
    #[serde(deserialize_with = "kit::lenient_id")]
    wf: Option<i64>,
}

/// Uncheck the given rows.
fn clear_selection(ids: &[i64]) -> Value {
    let sel: serde_json::Map<String, Value> = ids
        .iter()
        .map(|i| (format!("r{i}"), Value::Bool(false)))
        .collect();
    json!({ "wfs": { "sel": sel } })
}

async fn bulk_archive(
    State(st): State<Arc<AppState>>,
    Query(q): Query<ArchiveQuery>,
    Sig(s): Sig<BulkSig>,
) -> Ui {
    let ids = selected_ids(&s.wfs.sel);
    archive(&st, &ids, q.archived)
        .await
        .signals(clear_selection(&ids))
}

/// Delete workflows, toasting the outcome.
async fn delete(st: &Arc<AppState>, ids: &[i64], selected_wf: Option<i64>) -> Ui {
    let results = futures::future::join_all(ids.iter().map(|id| async move {
        (
            *id,
            kit::call(st, crate::cli_delete_handler, wid(*id))
                .await
                .and_then(kit::cli_result),
        )
    }))
    .await;
    let deleted: Vec<i64> = results
        .iter()
        .filter(|(_, r)| r.is_ok())
        .map(|(i, _)| *i)
        .collect();
    let failed: Vec<String> = results
        .iter()
        .filter_map(|(i, r)| r.as_ref().err().map(|e| format!("{i}: {e}")))
        .collect();
    let mut ui = Ui::new();
    match deleted.len() {
        0 => {}
        1 => ui = ui.ok(format!("Deleted workflow {}", deleted[0])),
        n => ui = ui.ok(format!("Deleted {n} workflows")),
    }
    if !failed.is_empty() {
        ui = ui.err(format!("Failed to delete {}", failed.join("; ")));
    }
    if selected_wf.is_some_and(|w| deleted.contains(&w)) {
        ui = ui.signals(json!({ "wf": null }));
    }
    ui.signals(clear_selection(&deleted)).refresh()
}

async fn delete_one(
    State(st): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Sig(s): Sig<BulkSig>,
) -> Ui {
    delete(&st, &[id], s.wf).await
}

async fn bulk_delete(Sig(s): Sig<BulkSig>) -> Ui {
    let ids = selected_ids(&s.wfs.sel);
    if ids.is_empty() {
        return Ui::new();
    }
    let noun = if ids.len() == 1 {
        format!("workflow {}", ids[0])
    } else {
        format!("{} workflows", ids.len())
    };
    // Confirm on the server's side so the dialog can name what is selected.
    Ui::new().modal(modal(
        &format!("Delete {noun}?"),
        "sm",
        html! { p { "This action cannot be undone." } },
        Some(html! {
            (cancel_button())
            button.btn.btn-danger-solid data-on:click="el.closest('dialog').close(); @post('/ui/workflows/delete-confirmed')" { "Delete" }
        }),
    ))
}

// ============== Initialize / reinitialize ==============

/// Server refusals for a reset while jobs or Slurm allocations still look active.
fn active_work(msg: &str) -> bool {
    msg.contains("jobs are currently running or pending")
        || msg.contains("compute nodes are currently pending or active")
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct InitQuery {
    force: bool,
    /// Skip the existing-outputs check (the user already confirmed it).
    checked: bool,
}

async fn initialize(
    State(st): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(q): Query<InitQuery>,
) -> Ui {
    // If output files from an earlier run exist, say so first: initializing keeps them.
    if !q.checked && !q.force {
        let check = kit::call(&st, crate::cli_check_initialize_handler, wid(id))
            .await
            .and_then(kit::cli_result)
            .ok()
            .and_then(|out| serde_json::from_str::<Value>(&out).ok());
        let count = check
            .as_ref()
            .and_then(|d| d.get("existing_output_file_count"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if count > 0 {
            let files: Vec<String> = check
                .as_ref()
                .and_then(|d| d.get("existing_output_files"))
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|f| f.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            return Ui::new().modal(modal(
                "Output files already exist",
                "md",
                html! {
                    p { "Workflow " (id) " has " (count) " output file(s) from an earlier run. Initializing does not delete them:" }
                    ul.file-list {
                        @for f in files.iter().take(10) { li.mono { (f) } }
                        @if files.len() > 10 { li { "… and " (files.len() - 10) " more" } }
                    }
                },
                Some(html! {
                    (cancel_button())
                    button.btn.btn-primary data-on:click=(format!("el.closest('dialog').close(); @post('/ui/wf/{id}/initialize?checked=true')")) { "Initialize" }
                }),
            ));
        }
    }
    run_init(&st, id, "initialize", q.force).await
}

async fn reinitialize(
    State(st): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(q): Query<InitQuery>,
) -> Ui {
    run_init(&st, id, "reinitialize", q.force).await
}

/// Run initialize/reinitialize. If it fails only because input files are missing, offer to
/// retry with `--force`. If jobs or allocations still look active (e.g. after `scancel`), offer
/// to check Slurm status, which can mark the orphaned jobs failed.
async fn run_init(st: &Arc<AppState>, id: i64, cmd: &str, force: bool) -> Ui {
    let body = json!({ "workflow_id": id.to_string(), "force": force });
    let r = if cmd == "initialize" {
        kit::call(st, crate::cli_initialize_handler, body).await
    } else {
        kit::call(st, crate::cli_reinitialize_handler, body).await
    }
    .and_then(kit::cli_result);
    let label = if cmd == "initialize" {
        "Initialize"
    } else {
        "Reinitialize"
    };
    match r {
        Ok(_) => Ui::new().ok(format!("Workflow {id} {cmd}d")).refresh(),
        Err(msg) if active_work(&msg) => Ui::new().modal(modal(
            "Workflow still has active jobs",
            "md",
            html! {
                p { "Workflow " (id) " has jobs marked running or Slurm allocations marked active, so it can't be " (cmd) "d yet." }
                p { "If those Slurm jobs were canceled (scancel), timed out, or died, check Slurm status to mark the orphaned jobs as failed, then " (label.to_lowercase()) "." }
            },
            Some(html! {
                (cancel_button())
                button.btn.btn-primary data-on:click=(format!("el.closest('dialog').close(); @get('/ui/wf/{id}/sync?retry={cmd}')")) { "Check Slurm status…" }
            }),
        )),
        Err(msg) if !force && msg.contains("Missing required files") => Ui::new().modal(modal(
            "Required input files are missing",
            "md",
            html! {
                pre.code-block { (msg) }
                p { (label) " anyway? Jobs that need these files will fail unless they appear before the jobs run." }
            },
            Some(html! {
                (cancel_button())
                button.btn.btn-danger-solid data-on:click=(format!("el.closest('dialog').close(); @post('/ui/wf/{id}/{cmd}?force=true')")) { (label) " anyway" }
            }),
        )),
        Err(msg) => Ui::new().err(format!("{label} failed: {msg}")),
    }
}

/// Download a .tar.gz of the workflow's logs from the Debugging tab's output dir, resolved
/// against the workflow's submission directory.
async fn download_logs(
    State(st): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Sig(g): Sig<kit::G>,
) -> Ui {
    let submission_dir = submission_directory(&st, id).await;
    let dir = kit::resolve_log_base_dir(&g.output_dir, submission_dir.as_deref());
    let url = format!(
        "/api/download/logs?workflow_id={id}&output_dir={}",
        url::form_urlencoded::byte_serialize(dir.as_bytes()).collect::<String>()
    );
    Ui::new()
        .toast(Tone::Info, format!("Bundling logs from {dir}…"))
        .script(&format!("torcDownload({})", js(&url)))
}

/// The directory the workflow was submitted from (where relative log paths resolve).
pub async fn submission_directory(st: &AppState, id: i64) -> Option<String> {
    let v: Value = kit::get(st, &format!("/workflows/{id}"), &[]).await.ok()?;
    v.get("submission_directory")
        .and_then(Value::as_str)
        .map(String::from)
}

/// After the delete confirmation.
async fn bulk_delete_confirmed(State(st): State<Arc<AppState>>, Sig(s): Sig<BulkSig>) -> Ui {
    let ids = selected_ids(&s.wfs.sel);
    delete(&st, &ids, s.wf).await
}
