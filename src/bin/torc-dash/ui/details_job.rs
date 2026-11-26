//! The job details modal (`GET /ui/job/{id}`): summary, command, and sub-tabs for results, logs,
//! files, user data, resources, and dependencies. Loads with per-job queries, so it stays fast on
//! workflows with 100k jobs.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use maud::{Markup, html};
use serde::Deserialize;
use serde_json::{Value, json};

use super::super::kit::{self, G, Sig, Ui, js, modal, seg_tabs, status_badge};
use super::view::{self, PartQuery};
use super::{DetailsNs, RERUN_MESSAGE, fixed, float, int, return_code, text};
use crate::AppState;

pub struct JobDetails {
    pub job: Value,
    workflow: Option<Value>,
    results: Vec<Value>,
    pub resource_req: Option<Value>,
    input_files: Vec<Value>,
    output_files: Vec<Value>,
    input_user_data: Vec<Value>,
    output_user_data: Vec<Value>,
    blocked_by: Vec<Value>,
    blocks: Vec<Value>,
}

/// Up to this many ids are fetched one by one; beyond that, list the workflow and filter.
const MAX_INDIVIDUAL: usize = 50;
/// Ids per `/jobs?ids=` request: keeps the URL well under server limits on large fan-ins.
const IDS_PER_REQUEST: usize = 1000;
/// Rows shown per list in the modal.
const MAX_ROWS: usize = 500;

fn ids(v: &Value, k: &str) -> Vec<i64> {
    v.get(k)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_i64).collect())
        .unwrap_or_default()
}

/// The jobs of workflow `wf` among `ids`.
pub async fn jobs_by_ids(st: &AppState, wf: i64, ids: &[i64]) -> Result<Vec<Value>, String> {
    let mut jobs = Vec::new();
    for chunk in ids.chunks(IDS_PER_REQUEST) {
        let mut q = vec![("workflow_id", wf.to_string())];
        q.extend(chunk.iter().map(|i| ("ids", i.to_string())));
        jobs.extend(kit::list_all::<Value>(st, "/jobs", &q).await?);
    }
    Ok(jobs)
}

async fn files_by_ids(st: &AppState, wf: i64, ids: &[i64]) -> Result<Vec<Value>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    if ids.len() <= MAX_INDIVIDUAL {
        return futures::future::try_join_all(
            ids.iter()
                .map(|id| async move { kit::get::<Value>(st, &format!("/files/{id}"), &[]).await }),
        )
        .await;
    }
    let all: Vec<Value> = kit::list_all(st, "/files", &[("workflow_id", wf.to_string())]).await?;
    Ok(all
        .into_iter()
        .filter(|f| int(f, "id").is_some_and(|i| ids.contains(&i)))
        .collect())
}

/// Every item of a workflow's list endpoint, filtered by `q` (`("all_runs", 1)` means true).
async fn list(
    st: &AppState,
    path: &str,
    wf: i64,
    q: Vec<(&'static str, i64)>,
) -> Result<Vec<Value>, String> {
    let mut query = vec![("workflow_id", wf.to_string())];
    for (k, v) in q {
        query.push((
            k,
            if k == "all_runs" {
                "true".into()
            } else {
                v.to_string()
            },
        ));
    }
    kit::list_all(st, path, &query).await
}

pub async fn load_job(st: &AppState, id: i64) -> Result<JobDetails, String> {
    let job: Value = kit::get(st, &format!("/jobs/{id}"), &[]).await?;
    let wf = int(&job, "workflow_id").unwrap_or_default();
    let rr_id = int(&job, "resource_requirements_id");
    let (wf_path, input_ids, dep_ids) = (
        format!("/workflows/{wf}"),
        ids(&job, "input_file_ids"),
        ids(&job, "depends_on_job_ids"),
    );
    let (workflow, results, rr, inputs, outputs, ud_in, ud_out, blocked_by, blocks) = futures::join!(
        kit::get::<Value>(st, &wf_path, &[]),
        list(st, "/results", wf, vec![("job_id", id), ("all_runs", 1)]),
        async {
            match rr_id {
                Some(r) => kit::get::<Value>(st, &format!("/resource_requirements/{r}"), &[])
                    .await
                    .ok(),
                None => None,
            }
        },
        files_by_ids(st, wf, &input_ids),
        list(st, "/files", wf, vec![("produced_by_job_id", id)]),
        list(st, "/user_data", wf, vec![("consumer_job_id", id)]),
        list(st, "/user_data", wf, vec![("producer_job_id", id)]),
        jobs_by_ids(st, wf, &dep_ids),
        list(st, "/jobs", wf, vec![("upstream_job_id", id)]),
    );
    let mut results = results?;
    results.sort_by_key(|r| (int(r, "run_id"), int(r, "attempt_id").unwrap_or(1)));
    Ok(JobDetails {
        job,
        workflow: workflow.ok(),
        results,
        resource_req: rr,
        input_files: inputs?,
        output_files: outputs?,
        input_user_data: ud_in?,
        output_user_data: ud_out?,
        blocked_by: blocked_by?,
        blocks: blocks?,
    })
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct JobSig {
    #[serde(flatten)]
    pub g: G,
    pub details: DetailsNs,
}

/// `GET /ui/job/{id}`: the job details modal. `?part=true` re-renders only its body (sub-tab,
/// run, stream, or find text changed).
pub async fn job_modal(
    State(st): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(q): Query<PartQuery>,
    Sig(s): Sig<JobSig>,
) -> Ui {
    job_ui(Ui::new(), &st, id, s, q.part).await
}

pub async fn job_ui(ui: Ui, st: &AppState, id: i64, mut s: JobSig, part: bool) -> Ui {
    if !part {
        s.details = DetailsNs {
            jtab: "results".into(),
            stream: "stdout".into(),
            ..Default::default()
        };
    }
    let (title, body) = render(st, id, &s).await;
    if part {
        return ui.patch(body);
    }
    let footer = html! {
        button.btn data-on:click=(format!("@get('/ui/details/job/{id}/edit')")) { "Edit…" }
        button.btn data-on:click=(format!("confirm({}) && @post('/ui/details/job/{id}/rerun')",
            js(&format!("Reset job {id} for rerun?\n\n{RERUN_MESSAGE}")))) { "Reset for rerun" }
        button.btn data-on:click="el.closest('dialog').close()" { "Close" }
    };
    ui.signals(
        json!({ "details": { "jtab": "results", "q": "", "stream": "stdout", "run": null } }),
    )
    .close_modal()
    .modal(modal(&title, "xl", body, Some(footer)))
}

/// The modal's title and body.
async fn render(st: &AppState, id: i64, s: &JobSig) -> (String, Markup) {
    match load_job(st, id).await {
        Ok(d) => (format!("Job {}", text(&d.job, "name")), body(&d, s).await),
        Err(e) => (
            format!("Job {id}"),
            html! { div #job-body.empty.error-text { "Error loading job details: " (e) } },
        ),
    }
}

async fn body(d: &JobDetails, s: &JobSig) -> Markup {
    let job = &d.job;
    let id = int(job, "id").unwrap_or_default();
    let wf = int(job, "workflow_id").unwrap_or_default();
    let reload = format!("@get('/ui/job/{id}?part=true')");
    let tab = match s.details.jtab.as_str() {
        t @ ("logs" | "files" | "user_data" | "resources" | "dependencies") => t,
        _ => "results",
    };
    let count = |n: usize| Some(n);
    let tabs = [
        ("results", "Results", count(d.results.len())),
        ("logs", "Logs", None),
        (
            "files",
            "Files",
            count(d.input_files.len() + d.output_files.len()),
        ),
        (
            "user_data",
            "User Data",
            count(d.input_user_data.len() + d.output_user_data.len()),
        ),
        ("resources", "Resources", None),
        (
            "dependencies",
            "Dependencies",
            count(d.blocked_by.len() + d.blocks.len()),
        ),
    ];
    let labels = job
        .get("labels")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|l| {
                    l.as_str()
                        .map(String::from)
                        .unwrap_or_else(|| l.to_string())
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty());
    let origin = text(job, "origin");
    let wf_name = d
        .workflow
        .as_ref()
        .map(|w| text(w, "name"))
        .unwrap_or_else(|| "Workflow".into());
    let open_file = |f: &Value| {
        format!(
            "@get({})",
            js(&view::file_url(
                &text(f, "path"),
                Some(&text(f, "name")),
                Some(id)
            ))
        )
    };
    let open_job = |j: &Value| format!("@get('/ui/job/{}')", int(j, "id").unwrap_or_default());

    let content = match tab {
        "logs" => logs_tab(d, s, &reload).await,
        "files" => html! {
            (section("Input files", "No input files", &d.input_files, file_head(), file_cells, Some(&open_file)))
            (section("Output files", "No output files", &d.output_files, file_head(), file_cells, Some(&open_file)))
        },
        "user_data" => html! {
            (section("Input user data", "No input user data", &d.input_user_data, ud_head(), ud_cells, None))
            (section("Output user data", "No output user data", &d.output_user_data, ud_head(), ud_cells, None))
        },
        "resources" => match &d.resource_req {
            None => html! { div.empty { "No resource requirement assigned to this job" } },
            Some(r) => html! {
                dl.kv.card.card-body {
                    dt { "ID" } dd { code { (text(r, "id")) } }
                    dt { "Name" } dd { (text(r, "name")) }
                    dt { "CPUs" } dd { (text(r, "num_cpus")) }
                    dt { "Memory" } dd { (text(r, "memory")) }
                    dt { "GPUs" } dd { (text(r, "num_gpus")) }
                    dt { "Nodes" } dd { (text(r, "num_nodes")) }
                    dt { "Runtime" } dd { code { (text(r, "runtime")) } }
                }
            },
        },
        "dependencies" => html! {
            (section("Blocked by", "This job has no dependencies", &d.blocked_by, job_head(), job_cells, Some(&open_job)))
            (section("Blocks", "No jobs depend on this job", &d.blocks, job_head(), job_cells, Some(&open_job)))
        },
        _ => section(
            "Results",
            "No results for this job",
            &d.results,
            result_head(),
            result_cells,
            None,
        ),
    };

    html! {
        div #job-body.job-wrap {
            div.job-summary {
                dl.kv {
                    dt { "ID" } dd { code { (id) } }
                    dt { "Name" } dd { (text(job, "name")) }
                    dt { "Status" } dd { (status_badge(&text(job, "status"))) }
                }
                dl.kv {
                    dt { "Workflow" }
                    dd { button.link data-on:click=(format!("$wf = {wf}; location.hash = '/details'; el.closest('dialog').close()")) { (wf_name) " (" (wf) ")" } }
                    dt { "Compute node" } dd { code { (kit::or_dash(int(job, "compute_node_id"))) } }
                    dt { "Start time" } dd { (kit::fmt_datetime(Some(&text(job, "start_time")))) }
                }
                dl.kv {
                    dt { "Attempt" }
                    dd { (int(job, "attempt_id").unwrap_or(1)) @if !origin.is_empty() { span.faint { " · " (origin) } } }
                    dt { "Priority" } dd { (int(job, "priority").unwrap_or(0)) }
                    dt { "Labels" } dd { (labels.unwrap_or_else(|| "—".into())) }
                    dt { "Resources" } dd { (d.resource_req.as_ref().map(|r| text(r, "name")).unwrap_or_else(|| "—".into())) }
                }
            }
            pre.code-block.command { (text(job, "command")) }
            (seg_tabs("details.jtab", tab, &tabs, &reload))
            div.job-tab { (content) }
        }
    }
}

async fn logs_tab(d: &JobDetails, s: &JobSig, reload: &str) -> Markup {
    let n = d.results.len();
    if n == 0 {
        return html! { div.empty { "No results yet. Logs are available after the job runs." } };
    }
    let idx = s
        .details
        .run
        .and_then(|r| usize::try_from(r).ok())
        .filter(|r| *r < n)
        .unwrap_or(n - 1);
    let run = &d.results[idx];
    let sub = d.workflow.as_ref().map(|w| text(w, "submission_directory"));
    let base = format!(
        "{}/job_stdio/job_wf{}_j{}_r{}_a{}",
        kit::resolve_log_base_dir(&s.g.output_dir, sub.as_deref()),
        text(run, "workflow_id"),
        text(run, "job_id"),
        text(run, "run_id"),
        int(run, "attempt_id").unwrap_or(1)
    );
    html! {
        div.toolbar {
            (view::output_dir_field(reload))
            @if n > 1 {
                label.field.outdir {
                    "Run"
                    select.select data-on:change=(format!("$details.run = +el.value; {reload}")) {
                        @for (i, r) in d.results.iter().enumerate() {
                            option value=(i) selected[i == idx] {
                                "Run " (text(r, "run_id")) " · attempt " (int(r, "attempt_id").unwrap_or(1)) " · rc " (text(r, "return_code"))
                            }
                        }
                    }
                }
            }
        }
        (view::log_pane(
            &[format!("{base}.o"), format!("{base}.log")],
            &[format!("{base}.e"), format!("{base}.log")],
            &s.details.stream,
            &s.details.q,
            reload,
        ).await)
    }
}

// ============== Short tables ==============

type Cells = fn(&Value) -> Markup;

/// A titled, unsorted table for short lists (capped at [`MAX_ROWS`]).
fn section(
    title: &str,
    empty: &str,
    rows: &[Value],
    head: Markup,
    cells: Cells,
    click: Option<&dyn Fn(&Value) -> String>,
) -> Markup {
    html! {
        section.job-section {
            h4 { (title) " " span.faint { (rows.len()) } }
            @if rows.is_empty() {
                p.muted { (empty) }
            } @else {
                div.table-wrap.mini {
                    div.table-scroll {
                        table {
                            thead { tr { (head) } }
                            tbody {
                                @for r in rows.iter().take(MAX_ROWS) {
                                    @let action = click.map(|f| f(r));
                                    tr.clickable[action.is_some()] tabindex=[action.as_ref().map(|_| 0)] data-on:click=[action] { (cells(r)) }
                                }
                            }
                        }
                    }
                    @if rows.len() > MAX_ROWS {
                        div.table-footer { "Showing " (MAX_ROWS) " of " (rows.len()) }
                    }
                }
            }
        }
    }
}

fn th(label: &str, width: Option<&str>) -> Markup {
    html! { th style=[width.map(|w| format!("width:{w}"))] { (label) } }
}

fn result_head() -> Markup {
    html! {
        (th("Run", Some("60px"))) (th("Attempt", Some("75px"))) (th("Return", Some("75px")))
        (th("Status", Some("120px"))) (th("Exec (min)", None)) (th("Peak Mem", None))
        (th("Peak CPU %", None)) (th("Completed", Some("180px")))
    }
}

fn result_cells(r: &Value) -> Markup {
    html! {
        td.mono { (text(r, "run_id")) }
        td.mono { (int(r, "attempt_id").unwrap_or(1)) }
        td.mono { (return_code(r)) }
        td { (status_badge(&text(r, "status"))) }
        td.mono { (fixed(float(r, "exec_time_minutes"), 2)) }
        td.mono { (kit::fmt_bytes(float(r, "peak_memory_bytes"))) }
        td.mono { (fixed(float(r, "peak_cpu_percent"), 1)) }
        td { (kit::fmt_datetime(Some(&text(r, "completion_time")))) }
    }
}

fn file_head() -> Markup {
    html! { (th("ID", Some("70px"))) (th("Name", Some("30%"))) (th("Path", None)) (th("Modified", Some("180px"))) }
}

fn file_cells(f: &Value) -> Markup {
    html! {
        td.mono { (text(f, "id")) }
        td title=(text(f, "name")) { (text(f, "name")) }
        td.mono title=(text(f, "path")) { (text(f, "path")) }
        td { (kit::fmt_datetime(Some(&text(f, "st_mtime")))) }
    }
}

fn ud_head() -> Markup {
    html! { (th("ID", Some("70px"))) (th("Name", Some("30%"))) (th("Data", None)) }
}

fn ud_cells(u: &Value) -> Markup {
    let data = u.get("data").map(Value::to_string).unwrap_or_default();
    html! { td.mono { (text(u, "id")) } td { (text(u, "name")) } td.mono title=(data) { (data) } }
}

fn job_head() -> Markup {
    html! { (th("ID", Some("80px"))) (th("Name", None)) (th("Status", Some("130px"))) }
}

fn job_cells(j: &Value) -> Markup {
    html! { td.mono { (text(j, "id")) } td { (text(j, "name")) } td { (status_badge(&text(j, "status"))) } }
}

// ============== Actions ==============

/// `torc jobs reset-status --reinit`: reset the jobs to uninitialized and reinitialize the
/// workflow, which also resets their downstream jobs.
pub async fn rerun(st: &Arc<AppState>, ids: &[i64]) -> Ui {
    if ids.is_empty() {
        return Ui::new();
    }
    let what = if ids.len() == 1 {
        format!("job {}", ids[0])
    } else {
        format!("{} jobs", ids.len())
    };
    match kit::call(st, crate::cli_reset_jobs_handler, json!({ "job_ids": ids }))
        .await
        .and_then(kit::cli_result)
    {
        Ok(_) => Ui::new().ok(format!("Reset {what} for rerun")).refresh(),
        Err(e) => Ui::new().err(format!("Reset failed: {e}")),
    }
}

pub async fn rerun_one(
    State(st): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Sig(s): Sig<JobSig>,
) -> Ui {
    let ui = rerun(&st, &[id]).await;
    ui.patch(render(&st, id, &s).await.1)
}
