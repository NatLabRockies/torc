//! The execution panel: live output of a `torc run` or `torc watch [--recover]` started from the
//! dashboard, docked bottom-right in `#exec`.
//!
//! `POST /ui/exec/{id}/{mode}` renders the panel, whose `#exec-stream` element opens the output
//! stream (`…/stream`) and owns it: removing that element (Close, Stop of a watch, or starting
//! another execution) aborts the request. The stream runs the old `/api/cli/run-stream` handler
//! in-process and translates its SSE events into line appends. That handler kills a watch when
//! its stream is dropped, and lets a run keep going (stoppable through `local_runs`).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};

use axum::{
    Router,
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
    routing::post,
};
use futures::StreamExt;
use maud::{Markup, html};
use serde::Deserialize;
use serde_json::{Value, json};

use super::kit::{self, Ui, icon};
use crate::AppState;

// ponytail: keeps the last 5000 lines in the DOM; virtualize the output if longer logs matter.
const MAX_LINES: usize = 5000;

/// Workflows whose run the user asked to stop, so its exit reads as stopped, not failed.
static STOPPING: LazyLock<Mutex<HashSet<i64>>> = LazyLock::new(Default::default);

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ui/exec/{id}/stop", post(stop))
        .route("/ui/exec/{id}/{mode}", post(start))
        .route("/ui/exec/{id}/{mode}/stream", post(stream))
}

fn title(mode: &str) -> Option<&'static str> {
    match mode {
        "run" => Some("Run"),
        "watch" => Some("Watch"),
        "watch-recover" => Some("Watch (recovery)"),
        _ => None,
    }
}

/// The panel header. `state`: running | success | failed | stopped.
fn head(id: i64, mode: &str, state: &str) -> Markup {
    let (dot, text) = match state {
        "success" => ("ok", "Completed"),
        "failed" => ("bad", "Failed"),
        "stopped" => ("warn", "Stopped"),
        _ => ("", "running"),
    };
    let running = state == "running";
    let run = mode == "run";
    html! {
        header #exec-head {
            button.title data-on:click="$exec._collapsed = !$exec._collapsed"
                data-attr:aria-expanded="!$exec._collapsed"
                data-attr:title="$exec._collapsed ? 'Expand' : 'Collapse'" {
                span data-show="$exec._collapsed" style="display:none" { (icon("chevronRight", 14)) }
                span data-show="!$exec._collapsed" { (icon("chevronDown", 14)) }
                @if running { span.spinner.small {} } @else { span.dot.(dot) {} }
                span { (title(mode).unwrap_or("Run")) " · workflow " (id) }
                span.faint { (text) }
            }
            div.grow {}
            button.btn.btn-ghost.btn-sm title="Open workflow details"
                data-on:click=(format!("$wf = {id}; location.hash = '/details'")) { "Details" }
            @if running {
                button.btn.btn-danger.btn-sm
                    title=(if run { "Stop the run and terminate its jobs" } else { "Stop watching" })
                    data-show=[run.then_some("$canStopRuns")]
                    data-on:click=(format!("@post('/ui/exec/{id}/stop?mode={mode}')")) {
                    (icon("stop", 12)) " Stop"
                }
            }
            button.btn.btn-ghost.btn-icon.btn-sm aria-label="Close" title="Close"
                data-on:click="document.getElementById('exec').replaceChildren()" { (icon("x", 14)) }
        }
    }
}

/// Output lines, each `(css class, text)`.
fn lines(lines: &[(&str, String)]) -> Markup {
    html! { @for (kind, text) in lines { span.(kind) { (text) "\n" } } }
}

async fn start(Path((id, mode)): Path<(i64, String)>) -> Ui {
    if title(&mode).is_none() {
        return Ui::new().err(format!("Unknown execution mode '{mode}'"));
    }
    if mode == "run" {
        super::events::follow(id);
    }
    STOPPING.lock().unwrap().remove(&id);
    let first = if mode == "run" {
        format!("Starting workflow {id}…")
    } else {
        format!("Watching workflow {id}…")
    };
    // Empty the slot first so a previous panel's stream element is removed (its request aborted)
    // rather than morphed into this one.
    Ui::new().inner("#exec", html! {}).inner(
        "#exec",
        html! {
            section #exec-panel.exec-panel aria-label="Execution output"
                data-signals="{exec: {_collapsed: false}}"
                data-class:collapsed="$exec._collapsed" {
                (head(id, &mode, "running"))
                // Follow the tail unless the user scrolled up.
                pre #exec-out data-show="!$exec._collapsed"
                    data-init="new MutationObserver(() => el._stick !== false && (el.scrollTop = el.scrollHeight)).observe(el, {childList: true})"
                    data-on:scroll="el._stick = el.scrollHeight - el.scrollTop - el.clientHeight < 24" {
                    (lines(&[("info", first)]))
                }
                // No retries: a retried POST would start a second run.
                div #exec-stream hidden data-init=(format!(
                    "@post('/ui/exec/{id}/{mode}/stream', {{requestCancellation: 'cleanup', retryMaxCount: 0}})"
                )) {}
            }
        },
    )
}

async fn stream(
    State(st): State<Arc<AppState>>,
    Path((id, mode)): Path<(i64, String)>,
) -> Response {
    let params = HashMap::from([
        ("workflow_id".to_string(), id.to_string()),
        ("mode".to_string(), mode.clone()),
    ]);
    let resp = crate::cli_run_stream_handler(State(st), Query(params))
        .await
        .into_response();
    let ok = resp.status().is_success();
    let mut body = resp.into_body().into_data_stream().ready_chunks(64);
    let watching = mode != "run";

    kit::stream(async_stream::stream! {
        if !ok {
            yield finish(id, &mode, "failed", vec![("error", "Could not start the command".into())]);
            return;
        }
        let mut buf = String::new();
        let mut count = 1usize;
        let mut end: Option<String> = None;
        while let Some(chunks) = body.next().await {
            for c in chunks.into_iter().flatten() {
                buf.push_str(&String::from_utf8_lossy(&c));
            }
            let mut out: Vec<(&str, String)> = Vec::new();
            while let Some(i) = buf.find("\n\n") {
                let block: String = buf.drain(..i + 2).collect();
                let (mut event, mut data) = ("message", Vec::new());
                for l in block.lines() {
                    if let Some(e) = l.strip_prefix("event:") {
                        event = match e.trim() {
                            "start" => "start",
                            "stdout" => "stdout",
                            "stderr" => "stderr",
                            "status" => "status",
                            "error" => "error",
                            "end" => "end",
                            _ => "other",
                        };
                    } else if let Some(d) = l.strip_prefix("data:") {
                        data.push(d.strip_prefix(' ').unwrap_or(d));
                    }
                }
                let data = data.join("\n").replace('\r', "");
                match event {
                    "start" => out.push(("info", data)),
                    "stdout" => out.push(("stdout", data)),
                    "stderr" => out.push(("stderr", data)),
                    "status" => out.push(("info", format!("[Status] {data}"))),
                    "error" => out.push(("error", format!("Error: {data}"))),
                    "end" => end = Some(data),
                    _ => {}
                }
            }
            if out.is_empty() {
                continue;
            }
            let first = count == 1;
            count += out.len();
            let mut ui = Ui::new().append("#exec-out", lines(&out));
            // The run is registered by now: show it as running in the workflow actions.
            if first && !watching {
                ui = ui.refresh();
            }
            if count > MAX_LINES {
                ui = ui.remove(&format!("#exec-out > span:nth-child(-n+{})", count - MAX_LINES));
                count = MAX_LINES;
            }
            yield ui;
        }

        let stopping = STOPPING.lock().unwrap().remove(&id);
        yield match end.as_deref() {
            _ if stopping => finish(id, &mode, "stopped", vec![("warning", "Run stopped.".into())]),
            Some("success") => finish(id, &mode, "success", vec![(
                "success",
                if watching { "✓ Watch finished: workflow complete" } else { "✓ Workflow completed successfully" }.into(),
            )])
            .ok(format!("Workflow {id} completed successfully")),
            Some(status) => finish(id, &mode, "failed", vec![("error", format!("✗ Workflow {status}"))])
                .err(format!("Workflow {id} {status}")),
            None => finish(id, &mode, "failed", vec![("error", "Connection error".into())]),
        };
    })
}

/// Final lines, the header in its end state, and a refresh of the tab.
fn finish(id: i64, mode: &str, state: &str, last: Vec<(&str, String)>) -> Ui {
    Ui::new()
        .append("#exec-out", lines(&last))
        .patch(head(id, mode, state))
        .refresh()
}

#[derive(Deserialize)]
struct StopQuery {
    mode: String,
}

/// Stop. A `torc run` is sent SIGTERM (its jobs are terminated) and the output keeps streaming
/// until it exits. A `torc watch` ends when its stream closes; the workflow is not affected.
async fn stop(
    State(st): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(q): Query<StopQuery>,
) -> Ui {
    if q.mode != "run" {
        return finish(
            id,
            &q.mode,
            "stopped",
            vec![(
                "warning",
                "Stopped watching. The workflow itself is not affected.".into(),
            )],
        )
        .remove("#exec-stream");
    }
    STOPPING.lock().unwrap().insert(id);
    let ui = Ui::new().append(
        "#exec-out",
        lines(&[(
            "warning",
            "Stopping the run and terminating its jobs…".into(),
        )]),
    );
    let r: Result<Value, String> = kit::call(
        &st,
        crate::cli_stop_run_handler,
        json!({ "workflow_id": id.to_string() }),
    )
    .await
    .and_then(|v: Value| {
        if v.get("success").and_then(Value::as_bool) == Some(false) {
            Err(v
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("unknown error")
                .to_string())
        } else {
            Ok(v)
        }
    });
    match r {
        Ok(v) if v.get("stopped").and_then(Value::as_bool) == Some(true) => ui,
        Ok(_) => {
            STOPPING.lock().unwrap().remove(&id);
            ui.append(
                "#exec-out",
                lines(&[("warning", "The run is no longer active.".into())]),
            )
            .patch(head(id, &q.mode, "stopped"))
            .remove("#exec-stream")
            .refresh()
        }
        Err(e) => {
            STOPPING.lock().unwrap().remove(&id);
            ui.append(
                "#exec-out",
                lines(&[("error", format!("Stop failed: {e}"))]),
            )
        }
    }
}
