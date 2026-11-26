//! Resources tab: resource-monitoring databases of the selected workflow, plotted after the run
//! (`torc plot-resources`) or live while it runs (samples read from the databases). Charts are
//! server-rendered SVG ([`chart`]) themed with CSS variables.
//!
//! Generated plots fill `data-ignore-morph` slots keyed by the workflow and its database list, so
//! tab refreshes keep them and a workflow switch (or a new database) drops them.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{Json, Router, extract::State, routing::post};
use maud::{Markup, html};
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};

use super::Ctx;
use super::kit::{self, G, Sig, Ui, js, seg_tabs};
use crate::AppState;

const RELOAD: &str = "@get('/ui/tab')";
const DEFAULT_DIR: &str = "torc_output/resource_utilization";
const POLL_SECS: u32 = 5;
const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
const WINDOWS: [(i64, &str); 4] = [
    (900, "15 min"),
    (3600, "1 hour"),
    (6 * 3600, "6 hours"),
    (0, "All"),
];

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/ui/res/plots", post(plots))
}

/// `$res`: this tab's signals.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Res {
    mode: String,
    dir: String,
    show_all: bool,
    /// Checked databases, keyed `p<hash(path)>`. A key set to false was unchecked by the user.
    sel: HashMap<String, Value>,
    #[serde(deserialize_with = "lenient_i64")]
    win: i64,
    nodes: bool,
    /// Live: checked jobs, keyed `j<hash(db path)>_<job id>`.
    jobs: HashMap<String, Value>,
}

/// A number from a `<select>` or number input (which may arrive as a string).
fn lenient_i64<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Number(n) => n.as_i64().unwrap_or(0),
        Value::String(s) => s.trim().parse().unwrap_or(0),
        _ => 0,
    })
}

const INIT: &str = "{res: {mode: 'plots', dir: '', showAll: false, sel: {}, win: 3600, \
    nodes: true, paused: false, jobs: {}, jf: '', plot: 0}}";

fn hash(s: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    format!("{:08x}", h.finish() as u32)
}

fn on(m: &HashMap<String, Value>, k: &str) -> bool {
    m.get(k) == Some(&Value::Bool(true))
}

#[derive(Deserialize, Clone)]
struct Db {
    path: String,
    name: String,
    #[serde(default)]
    size_bytes: f64,
    #[serde(default)]
    modified: String,
    workflow_id: Option<i64>,
}

impl Db {
    fn key(&self) -> String {
        format!("p{}", hash(&self.path))
    }
    fn label(&self) -> &str {
        let n = self
            .name
            .strip_prefix("resource_metrics_")
            .unwrap_or(&self.name);
        n.strip_suffix(".db").unwrap_or(n)
    }
}

/// The scan: databases in the resolved directory, those shown for the workflow, the selected
/// ones, whether the selection is the automatic one (a single database), and the slot key.
struct Scan {
    dir: String,
    all: Result<Vec<Db>, String>,
    visible: Vec<Db>,
    selected: Vec<Db>,
    auto: bool,
    slot: String,
}

async fn scan(st: &AppState, g: &G, r: &Res) -> Scan {
    // Relative dirs resolve against the workflow's submission directory, not torc-dash's cwd.
    // If the workflow can't be read, fall back to cwd-relative.
    let sub = match g.wf {
        Some(id) => super::workflows::submission_directory(st, id).await,
        None => None,
    };
    let dir = r.dir.trim();
    let dir = kit::resolve_log_base_dir(
        if dir.is_empty() { DEFAULT_DIR } else { dir },
        sub.as_deref(),
    );
    let resp: Result<Value, String> = kit::json_of(
        crate::cli_list_resource_dbs_handler(Json(crate::ListResourceDbsRequest {
            base_dir: dir.clone(),
        }))
        .await,
    )
    .await;
    let all = resp.and_then(|v| {
        if v.get("success").and_then(Value::as_bool) == Some(true) {
            serde_json::from_value::<Vec<Db>>(v["databases"].clone()).map_err(|e| e.to_string())
        } else {
            Err(v["error"].as_str().unwrap_or("Scan failed").to_string())
        }
    });
    let visible: Vec<Db> = match (&all, g.wf) {
        (Ok(a), Some(id)) if !r.show_all => a
            .iter()
            .filter(|d| d.workflow_id == Some(id))
            .cloned()
            .collect(),
        (Ok(a), _) => a.clone(),
        _ => Vec::new(),
    };
    let mut selected: Vec<Db> = visible
        .iter()
        .filter(|d| on(&r.sel, &d.key()))
        .cloned()
        .collect();
    // If exactly one database matches and the user hasn't touched it, select it.
    let auto = visible.len() == 1 && !r.sel.contains_key(&visible[0].key());
    if auto {
        selected = visible.clone();
    }
    let paths: Vec<&str> = visible.iter().map(|d| d.path.as_str()).collect();
    let slot = hash(&format!("{:?}|{}", g.wf, paths.join("|")));
    Scan {
        dir,
        all,
        visible,
        selected,
        auto,
        slot,
    }
}

pub async fn view(st: &Arc<AppState>, cx: &Ctx) -> Markup {
    let r: Res = kit::ns(&cx.sig, "res");
    let live = r.mode == "live";
    let s = scan(st, &cx.g, &r).await;
    html! {
        div.page.res data-signals__ifmissing=(INIT) {
            div.page-header {
                h2 { "Resource plots" }
                (seg_tabs("res.mode", if live { "live" } else { "plots" }, &[("plots", "Plots", None), ("live", "Live", None)], RELOAD))
            }
            div.res-layout {
                aside.side {
                    section.card { div.card-body.stack {
                        form.scan data-on:submit=(format!("evt.preventDefault(); {RELOAD}")) {
                            label.field.grow {
                                span { "Database directory" }
                                input.input data-bind="res.dir" placeholder=(DEFAULT_DIR) title=(s.dir);
                            }
                            button.btn type="submit" data-indicator="_resScan" {
                                span.spinner.small data-show="$_resScan" style="display:none" {}
                                "Scan"
                            }
                        }
                        (db_list(&s, cx.g.wf, &r, live))
                    } }
                    div.plots-wrap hidden[live] {
                        div id=(format!("res-list-{}", s.slot)) data-ignore-morph {}
                    }
                }
                section.card.res-main {
                    div.plots-wrap hidden[live] data-class:stale="$_resGen" {
                        div id=(format!("res-main-{}", s.slot)) data-ignore-morph
                            data-init=[(s.auto && !live).then_some("@post('/ui/res/plots')")] {
                            div.empty {
                                @if s.auto { span.spinner {} }
                                @else { div { "Select one or more resource databases and click " strong { "Generate plots" } "." } }
                            }
                        }
                    }
                    @if live { (live_view(st, &s, &r).await) }
                }
            }
        }
    }
}

fn db_list(s: &Scan, wf: Option<i64>, r: &Res, live: bool) -> Markup {
    let all = match &s.all {
        Err(e) => return kit::error_box(e),
        Ok(a) => a,
    };
    if all.is_empty() {
        return html! { div.muted { "No resource databases found in this directory." } };
    }
    let sel: Vec<&str> = s.selected.iter().map(|d| d.path.as_str()).collect();
    html! {
        div.db-head {
            span.muted {
                @match (wf, r.show_all) {
                    (None, _) => "All databases",
                    (Some(_), true) => "All workflows",
                    (Some(id), false) => { "Workflow " (id) }
                }
            }
            @if wf.is_some() {
                label.check { input type="checkbox" data-bind="res.showAll" data-on:change=(RELOAD); "Show all" }
            }
        }
        @if s.visible.is_empty() {
            div.muted { "No databases for workflow " (wf.unwrap_or_default()) "." }
        } @else {
            div.db-list {
                @for db in &s.visible {
                    @let checked = sel.contains(&db.path.as_str());
                    label.db.on[checked] {
                        input type="checkbox" checked[checked]
                            data-on:change=(format!("$res.sel.{} = el.checked; {RELOAD}", db.key()));
                        span.db-info {
                            span.db-name title=(db.path) { (db.name) }
                            span.faint {
                                (db.workflow_id.map(|w| format!("Workflow {w}")).unwrap_or_else(|| "Workflow unknown".into()))
                                " · " (kit::fmt_bytes(Some(db.size_bytes))) " · " (db.modified)
                            }
                        }
                    }
                }
            }
        }
        @if !live && !s.visible.is_empty() {
            button.btn.btn-primary disabled[s.selected.is_empty()] data-indicator="_resGen"
                data-attr:disabled=(format!("$_resGen || {}", s.selected.is_empty()))
                data-on:click="@post('/ui/res/plots')" {
                span data-show="$_resGen" style="display:none" { span.spinner.small {} " Generating…" }
                span data-show="!$_resGen" { "Generate plots" }
            }
        }
    }
}

// ============== Plots ==============

fn friendly_name(file: &str) -> String {
    let n = file.strip_suffix(".json").unwrap_or(file);
    let n = n.strip_prefix("resource_plot").unwrap_or(n);
    let n = n.strip_prefix('_').unwrap_or(n);
    if n.is_empty() {
        return "Summary".into();
    }
    n.split('_')
        .map(|w| {
            if w.eq_ignore_ascii_case("cpu") || w.eq_ignore_ascii_case("gpu") {
                w.to_uppercase()
            } else {
                let mut c = w.chars();
                c.next()
                    .map(|f| f.to_uppercase().chain(c).collect())
                    .unwrap_or_default()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Overview plots first, then per-job plots in numeric order.
fn plot_order(name: &str) -> (u8, usize, String) {
    let rank = if name.contains("summary") {
        0
    } else if name.contains("all_jobs") {
        1
    } else {
        2
    };
    (rank, name.len(), name.to_string())
}

/// Generate plots for the selected databases into the plot slots.
async fn plots(State(st): State<Arc<AppState>>, Sig(sig): Sig<Value>) -> Ui {
    let g: G = serde_json::from_value(sig.clone()).unwrap_or_default();
    let r: Res = kit::ns(&sig, "res");
    let s = scan(&st, &g, &r).await;
    let main = format!("#res-main-{}", s.slot);
    let list = format!("#res-list-{}", s.slot);
    if s.selected.is_empty() {
        return Ui::new();
    }
    let paths: Vec<&str> = s.selected.iter().map(|d| d.path.as_str()).collect();
    let resp: Result<Value, String> = kit::call(
        &st,
        crate::cli_plot_resources_handler,
        json!({ "db_paths": paths, "prefix": "resource_plot" }),
    )
    .await;
    let mut plots = match resp {
        Ok(v) if v.get("success").and_then(Value::as_bool) == Some(true) => {
            v["plots"].as_array().cloned().unwrap_or_default()
        }
        Ok(v) => {
            let e = v["error"].as_str().unwrap_or("Plot generation failed");
            return Ui::new()
                .inner(&main, html! { div.empty.error-text { (e) } })
                .inner(&list, html! {});
        }
        Err(e) => {
            return Ui::new()
                .inner(&main, html! { div.empty.error-text { (e) } })
                .inner(&list, html! {});
        }
    };
    plots.sort_by_key(|p| plot_order(p["name"].as_str().unwrap_or("")));
    if plots.is_empty() {
        return Ui::new()
            .inner(
                &main,
                html! { div.empty { "No plots generated. The database may not contain any resource data." } },
            )
            .inner(&list, html! {});
    }
    // Every plot is rendered now; the list switches between them without a request.
    // ponytail: all charts in one response; render on demand if per-job plots get heavy.
    Ui::new()
        .signals(json!({ "res": { "plot": 0 } }))
        .inner(
            &list,
            html! {
                section.card.plot-list {
                    @for (i, p) in plots.iter().enumerate() {
                        button.plot-item data-class:active=(format!("$res.plot == {i}"))
                            data-on:click=(format!("$res.plot = {i}")) {
                            (friendly_name(p["name"].as_str().unwrap_or("")))
                        }
                    }
                }
            },
        )
        .inner(
            &main,
            html! {
                @for (i, p) in plots.iter().enumerate() {
                    div.figure data-show=(format!("$res.plot == {i}")) style=[(i > 0).then_some("display:none")] {
                        @if p["data"].is_object() { (figure(&p["data"])) }
                        @else { div.empty { "No data available for this plot." } }
                    }
                }
            },
        )
}

/// Strip Plotly's inline HTML (`<br>`, `<sub>`) from a title: (title, subtitle).
fn plotly_title(t: &str) -> (String, String) {
    let strip = |s: &str| {
        let mut out = String::new();
        let mut tag = false;
        for c in s.chars() {
            match c {
                '<' => tag = true,
                '>' => tag = false,
                c if !tag => out.push(c),
                _ => {}
            }
        }
        out.trim().to_string()
    };
    match t.split_once("<br>") {
        Some((a, b)) => (strip(a), strip(b)),
        None => (strip(t), String::new()),
    }
}

fn axis_title(layout: &Value, axis: &str) -> String {
    let a = &layout[axis]["title"];
    a["text"].as_str().or(a.as_str()).unwrap_or("").to_string()
}

/// A figure from `torc plot-resources`: one panel per y axis (overlaid axes become stacked
/// panels sharing the time axis; side-by-side bar subplots become stacked bar panels).
fn figure(fig: &Value) -> Markup {
    let layout = &fig["layout"];
    let (title, sub) = plotly_title(
        layout["title"]["text"]
            .as_str()
            .or(layout["title"].as_str())
            .unwrap_or(""),
    );
    let mut panels: Vec<(String, chart::Panel)> = Vec::new();
    let mut x_title = String::new();
    for (i, t) in fig["data"].as_array().into_iter().flatten().enumerate() {
        let y_axis = match t["yaxis"].as_str().unwrap_or("y") {
            "y" | "y1" => "yaxis".to_string(),
            a => format!("yaxis{}", &a[1..]),
        };
        let x_axis = match t["xaxis"].as_str().unwrap_or("x") {
            "x" | "x1" => "xaxis".to_string(),
            a => format!("xaxis{}", &a[1..]),
        };
        let bar = t["type"].as_str() == Some("bar");
        let xs = t["x"].as_array().cloned().unwrap_or_default();
        let ys = t["y"].as_array().cloned().unwrap_or_default();
        let cats: Option<Vec<String>> = bar.then(|| {
            xs.iter()
                .map(|v| {
                    v.as_str()
                        .map(String::from)
                        .unwrap_or_else(|| v.to_string())
                })
                .collect()
        });
        let pts: Vec<(f64, f64)> = xs
            .iter()
            .zip(&ys)
            .enumerate()
            .filter_map(|(j, (x, y))| {
                let x = if bar { Some(j as f64) } else { x.as_f64() };
                Some((x?, y.as_f64()?))
            })
            .collect();
        if x_title.is_empty() || bar {
            x_title = axis_title(layout, &x_axis);
        }
        let series = chart::Series {
            name: t["name"].as_str().unwrap_or("").to_string(),
            color: i,
            pts,
            notes: Vec::new(),
        };
        match panels.iter_mut().find(|(k, _)| *k == y_axis) {
            Some((_, p)) => p.series.push(series),
            None => panels.push((
                y_axis.clone(),
                chart::Panel {
                    title: axis_title(layout, &y_axis),
                    series: vec![series],
                    max: None,
                    cats,
                },
            )),
        }
    }
    let panels: Vec<chart::Panel> = panels.into_iter().map(|(_, p)| p).collect();
    let bars = panels.iter().all(|p| p.cats.is_some());
    html! {
        div.fig-head {
            h3 { (title) }
            @if !sub.is_empty() { div.muted { (sub) } }
        }
        (chart::chart(&panels, if bars { "" } else { &x_title }, &|x| chart::num(x)))
        @if bars && !x_title.is_empty() { div.chart-xtitle { (x_title) } }
    }
}

// ============== Live ==============

/// Samples of one series, as returned by the resource-samples handler.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Samples {
    timestamp: Vec<f64>,
    cpu_percent: Vec<f64>,
    memory_bytes: Vec<f64>,
    total_memory_bytes: Vec<f64>,
}

impl Samples {
    fn newest(&self) -> f64 {
        self.timestamp.last().copied().unwrap_or(0.0)
    }
}

struct Node {
    db: Db,
    system: Samples,
    jobs: Vec<(i64, Samples)>,
    names: Vec<(i64, String)>,
    error: Option<String>,
    newest: f64,
}

async fn read_node(db: &Db, r: &Res) -> Node {
    let dk = hash(&db.path);
    let jobs: Vec<i64> = r
        .jobs
        .iter()
        .filter(|(_, v)| **v == Value::Bool(true))
        .filter_map(|(k, _)| {
            k.strip_prefix(&format!("j{dk}_"))
                .and_then(|id| id.parse().ok())
        })
        .collect();
    // Re-read the whole window each poll; the server anchors it on the newest sample.
    // ponytail: no incremental merge; add `since` cursors if large windows get slow.
    let req = crate::ResourceSamplesRequest {
        db_path: db.path.clone(),
        since: 0,
        jobs: jobs.iter().map(|id| (*id, 0)).collect(),
        window: r.win,
    };
    let resp: Result<Value, String> =
        kit::json_of(crate::cli_resource_samples_handler(Json(req)).await).await;
    let mut node = Node {
        db: db.clone(),
        system: Samples::default(),
        jobs: Vec::new(),
        names: Vec::new(),
        error: None,
        newest: 0.0,
    };
    match resp {
        Ok(v) if v.get("success").and_then(Value::as_bool) == Some(true) => {
            node.system = serde_json::from_value(v["system"].clone()).unwrap_or_default();
            let mut js: Vec<(i64, Samples)> = jobs
                .iter()
                .map(|id| {
                    let s = serde_json::from_value(v["jobs"][id.to_string()].clone())
                        .unwrap_or_default();
                    (*id, s)
                })
                .collect();
            js.sort_by_key(|(id, _)| *id);
            node.jobs = js;
            node.names = serde_json::from_value(v["job_names"].clone()).unwrap_or_default();
            node.names.reverse();
            node.newest = node
                .jobs
                .iter()
                .map(|(_, s)| s.newest())
                .fold(node.system.newest(), f64::max);
        }
        Ok(v) => {
            node.error = Some(
                v["error"]
                    .as_str()
                    .unwrap_or("Failed to read samples")
                    .to_string(),
            )
        }
        Err(e) => node.error = Some(e),
    }
    node
}

async fn live_view(_st: &Arc<AppState>, s: &Scan, r: &Res) -> Markup {
    let nodes = futures::future::join_all(s.selected.iter().map(|d| read_node(d, r))).await;
    let has_data = nodes.iter().any(|n| {
        !n.system.timestamp.is_empty() || n.jobs.iter().any(|(_, s)| !s.timestamp.is_empty())
    });
    let now = chrono::Utc::now().timestamp() as f64;
    let poll = "!$res.paused && document.visibilityState === 'visible' && @get('/ui/tab?auto=1')";
    html! {
        div.live {
            div.toolbar.live-bar {
                label.field {
                    span { "Window" }
                    select.select data-bind="res.win" data-on:change=(RELOAD) {
                        @for (secs, label) in WINDOWS { option value=(secs) selected[secs == r.win] { (label) } }
                    }
                }
                label.check { input type="checkbox" data-bind="res.nodes" data-on:change=(RELOAD); "Node utilization" }
                button.btn data-on:click="$res.paused = !$res.paused" {
                    span data-show="!$res.paused" { "Pause" }
                    span data-show="$res.paused" style="display:none" { "Resume" }
                }
                span.faint { "Updates every " (POLL_SECS) "s" }
            }
            @if s.selected.is_empty() {
                div.empty { "Select one or more compute node databases to monitor." }
            } @else {
                div.live-body "data-on-interval__duration.5s"=(poll) {
                    aside.jobs {
                        input.input placeholder="Filter jobs" data-bind="res.jf";
                        @for n in &nodes {
                            @let dk = hash(&n.db.path);
                            div.node {
                                div.node-head title=(n.db.path) { (n.db.label()) }
                                @if let Some(e) = &n.error { div.error-text { (e) } }
                                @else if n.newest == 0.0 { div.faint { "no samples yet" } }
                                @else { div.faint { "last sample " (kit::fmt_duration(Some((now - n.newest).max(0.0).round()))) " ago" } }
                                @for (id, name) in &n.names {
                                    @let key = format!("j{dk}_{id}");
                                    @let hay = format!("{} {id}", name.to_lowercase());
                                    label.check.job data-show=(format!("!$res.jf || {}.includes($res.jf.trim().toLowerCase())", js(&hay))) {
                                        input type="checkbox" checked[on(&r.jobs, &key)]
                                            data-on:change=(format!("$res.jobs.{key} = el.checked; {RELOAD}"));
                                        span title=(name) { (name) " " span.faint { "(" (id) ")" } }
                                    }
                                }
                            }
                        }
                    }
                    div.live-chart {
                        @if has_data { (live_chart(&nodes, r.nodes)) }
                        @else {
                            div.empty {
                                div { "No samples yet." }
                                div.faint {
                                    "Live plots need " code { "resource_monitor.live_monitoring: true" }
                                    " with time-series monitoring, and new samples appear every "
                                    code { "flush_interval_seconds" } " (default 300). See the resource monitoring docs."
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Node utilization is a percentage of the node's capacity; job CPU is per process (100% = one
/// core), so the two get separate panels instead of sharing a misleading axis. Each node/job has
/// one color across its CPU and memory panels.
fn live_chart(nodes: &[Node], show_nodes: bool) -> Markup {
    use chart::{Panel, Series};
    let panel = |title: &str, max: Option<f64>| Panel {
        title: title.into(),
        series: Vec::new(),
        max,
        cats: None,
    };
    let mut node_cpu = panel("Node CPU %", Some(100.0));
    let mut node_mem = panel("Node memory %", Some(100.0));
    let mut job_cpu = panel("Job CPU % (100 = 1 core)", None);
    let mut job_mem = panel("Job memory (GiB)", None);
    let mut color = 0;
    let ts = |s: &Samples| s.timestamp.clone();
    for n in nodes {
        let sys = &n.system;
        if show_nodes && !sys.timestamp.is_empty() {
            let name = format!("{} (node)", n.db.label());
            node_cpu.series.push(Series {
                name: name.clone(),
                color,
                pts: ts(sys).into_iter().zip(sys.cpu_percent.clone()).collect(),
                notes: Vec::new(),
            });
            let (pts, notes): (Vec<_>, Vec<_>) = sys
                .timestamp
                .iter()
                .zip(&sys.memory_bytes)
                .zip(&sys.total_memory_bytes)
                .filter(|(_, total)| **total > 0.0)
                .map(|((t, b), total)| ((*t, 100.0 * b / total), format!("{:.1} GiB", b / GIB)))
                .unzip();
            node_mem.series.push(Series {
                name,
                color,
                pts,
                notes,
            });
            color += 1;
        }
        let names: HashMap<i64, &str> = n.names.iter().map(|(i, s)| (*i, s.as_str())).collect();
        for (id, s) in &n.jobs {
            let name = format!(
                "{} ({id}){}",
                names.get(id).unwrap_or(&"job"),
                if nodes.len() > 1 {
                    format!(" · {}", n.db.label())
                } else {
                    String::new()
                }
            );
            job_cpu.series.push(Series {
                name: name.clone(),
                color,
                pts: ts(s).into_iter().zip(s.cpu_percent.clone()).collect(),
                notes: Vec::new(),
            });
            job_mem.series.push(Series {
                name,
                color,
                pts: ts(s)
                    .into_iter()
                    .zip(s.memory_bytes.iter().map(|b| b / GIB))
                    .collect(),
                notes: Vec::new(),
            });
            color += 1;
        }
    }
    let panels: Vec<Panel> = [node_cpu, node_mem, job_cpu, job_mem]
        .into_iter()
        .filter(|p| !p.series.is_empty())
        .collect();
    let span = panels
        .iter()
        .flat_map(|p| p.series.iter().flat_map(|s| s.pts.iter().map(|p| p.0)))
        .fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(x), b.max(x)));
    let long = span.1 - span.0 > 86400.0;
    chart::chart(&panels, "", &move |t| {
        chrono::DateTime::from_timestamp(t as i64, 0)
            .map(|d| {
                d.with_timezone(&chrono::Local)
                    .format(if long { "%m-%d %H:%M" } else { "%H:%M:%S" })
                    .to_string()
            })
            .unwrap_or_default()
    })
}

/// Small server-rendered SVG charts: stacked panels of line series (sharing one x range) or
/// grouped bars, colored by the `--chart-N` CSS variables, with a legend. Points and bars carry
/// their hover text in `data-x` / `data-tip`; `shell.js` shows it (all series at the pointer's x).
pub mod chart {
    use maud::{Markup, html};

    pub struct Series {
        pub name: String,
        /// Palette index; series with the same index share a color (and a legend entry).
        pub color: usize,
        /// (x, y); for bar panels x is the category index.
        pub pts: Vec<(f64, f64)>,
        /// Optional extra tooltip text per point.
        pub notes: Vec<String>,
    }

    pub struct Panel {
        pub title: String,
        pub series: Vec<Series>,
        /// Fixed top of the y axis (e.g. 100 for percentages); None scales to the data.
        pub max: Option<f64>,
        /// Category labels: a grouped bar panel instead of lines.
        pub cats: Option<Vec<String>>,
    }

    const COLORS: usize = 10;
    const W: f64 = 760.0;
    const H: f64 = 190.0;
    const L: f64 = 56.0;
    const R: f64 = 14.0;
    const T: f64 = 24.0;
    const B: f64 = 24.0;
    /// Points kept per line series (the highest of each bucket, so peaks survive).
    const MAX_POINTS: usize = 400;

    /// A compact number: 3 significant-ish digits.
    pub fn num(v: f64) -> String {
        let a = v.abs();
        let s = if a >= 100.0 || a == a.trunc() {
            format!("{v:.0}")
        } else if a >= 10.0 {
            format!("{v:.1}")
        } else {
            format!("{v:.2}")
        };
        if s == "-0" { "0".into() } else { s }
    }

    /// Axis top and tick values from 0 to it, at a 1/2/2.5/5 × 10ⁿ step.
    fn ticks(max: f64) -> (f64, Vec<f64>) {
        let max = if max > 0.0 && max.is_finite() {
            max
        } else {
            1.0
        };
        let raw = max / 4.0;
        let mag = 10f64.powf(raw.log10().floor());
        let step = mag
            * match raw / mag {
                n if n <= 1.0 => 1.0,
                n if n <= 2.0 => 2.0,
                n if n <= 2.5 => 2.5,
                n if n <= 5.0 => 5.0,
                _ => 10.0,
            };
        let top = (max / step - 1e-9).ceil() * step;
        let n = (top / step).round() as usize;
        (top, (0..=n).map(|i| i as f64 * step).collect())
    }

    /// Keep at most `MAX_POINTS` (with their notes): the highest point of each bucket.
    fn thin(s: &Series) -> Vec<(f64, f64, Option<&str>)> {
        let pts: Vec<(f64, f64, Option<&str>)> = s
            .pts
            .iter()
            .enumerate()
            .map(|(i, p)| (p.0, p.1, s.notes.get(i).map(String::as_str)))
            .collect();
        if pts.len() <= MAX_POINTS {
            return pts;
        }
        pts.chunks(pts.len().div_ceil(MAX_POINTS))
            .filter_map(|c| c.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1)))
            .collect()
    }

    pub fn chart(panels: &[Panel], x_title: &str, xfmt: &dyn Fn(f64) -> String) -> Markup {
        let (x0, x1) = panels
            .iter()
            .filter(|p| p.cats.is_none())
            .flat_map(|p| p.series.iter().flat_map(|s| s.pts.iter().map(|p| p.0)))
            .fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(x), b.max(x)));
        let (x0, x1) = if x0 > x1 {
            (0.0, 1.0)
        } else if x1 == x0 {
            (x0, x0 + 1.0)
        } else {
            (x0, x1)
        };
        let mut legend: Vec<(&str, usize)> = Vec::new();
        for s in panels.iter().flat_map(|p| &p.series) {
            if !legend.iter().any(|(n, _)| *n == s.name) {
                legend.push((&s.name, s.color));
            }
        }
        html! {
            div.chart {
                @for p in panels { (panel(p, x0, x1, xfmt)) }
                @if !x_title.is_empty() { div.chart-xtitle { (x_title) } }
                div.chart-legend {
                    @for (name, c) in legend { span { i class=(format!("sw c{}", c % COLORS)) {} (name) } }
                }
            }
        }
    }

    fn panel(p: &Panel, x0: f64, x1: f64, xfmt: &dyn Fn(f64) -> String) -> Markup {
        let (pw, ph) = (W - L - R, H - T - B);
        let data_max = p
            .series
            .iter()
            .flat_map(|s| s.pts.iter().map(|p| p.1))
            .fold(0.0, f64::max);
        let (top, yt) = ticks(p.max.unwrap_or(data_max));
        let sy = |y: f64| T + ph - y.clamp(0.0, top) / top * ph;
        let sx = |x: f64| L + (x - x0) / (x1 - x0) * pw;
        html! {
            svg.chart-panel viewBox=(format!("0 0 {W} {H}")) role="img" aria-label=(p.title) {
                text.ptitle x=(L) y="14" { (p.title) }
                @for t in &yt {
                    line.grid x1=(L) x2=(W - R) y1=(sy(*t)) y2=(sy(*t)) {}
                    text.tick x=(L - 6.0) y=(sy(*t) + 4.0) text-anchor="end" { (num(*t)) }
                }
                @if let Some(cats) = &p.cats {
                    @let band = pw / cats.len().max(1) as f64;
                    @let n = p.series.len().max(1) as f64;
                    @let bw = band * 0.8 / n;
                    @for (ci, c) in cats.iter().enumerate() {
                        text.tick x=(L + band * (ci as f64 + 0.5)) y=(H - 6.0) text-anchor="middle" { (c) }
                        @for (si, s) in p.series.iter().enumerate() {
                            @if let Some((_, y)) = s.pts.iter().find(|pt| pt.0 as usize == ci) {
                                rect class=(format!("bar c{}", s.color % COLORS))
                                    x=(L + band * ci as f64 + band * 0.1 + bw * si as f64) y=(sy(*y))
                                    width=(bw.max(1.0)) height=((T + ph - sy(*y)).max(0.0))
                                    data-x=(c) data-tip=(format!("{}: {}", s.name, num(*y))) {}
                            }
                        }
                    }
                } @else {
                    @for i in 0..=5 {
                        @let x = x0 + (x1 - x0) * i as f64 / 5.0;
                        text.tick x=(sx(x)) y=(H - 6.0) text-anchor=(match i { 0 => "start", 5 => "end", _ => "middle" }) { (xfmt(x)) }
                    }
                    @for s in &p.series {
                        @let pts = thin(s);
                        @let d = pts.iter().enumerate().map(|(i, (x, y, _))| format!("{}{:.1},{:.1}", if i == 0 { "M" } else { "L" }, sx(*x), sy(*y))).collect::<String>();
                        g class=(format!("series c{}", s.color % COLORS)) {
                            path.line d=(d) {}
                            @for (x, y, note) in &pts {
                                circle.pt cx=(format!("{:.1}", sx(*x))) cy=(format!("{:.1}", sy(*y))) r="3.5"
                                    data-x=(xfmt(*x))
                                    data-tip=(format!("{}: {}{}", s.name, num(*y), note.map(|n| format!(" ({n})")).unwrap_or_default())) {}
                            }
                        }
                    }
                }
                line.axis x1=(L) x2=(W - R) y1=(T + ph) y2=(T + ph) {}
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn nice_ticks() {
            assert_eq!(ticks(100.0), (100.0, vec![0.0, 25.0, 50.0, 75.0, 100.0]));
            assert_eq!(ticks(277.7).0, 300.0);
            assert_eq!(ticks(0.0).0, 1.0);
            assert_eq!(num(0.1234), "0.12");
            assert_eq!(num(12.34), "12.3");
            assert_eq!(num(250.0), "250");
        }

        #[test]
        fn thinning_keeps_peaks() {
            let s = Series {
                name: "a".into(),
                color: 0,
                pts: (0..1000)
                    .map(|i| (i as f64, if i == 517 { 99.0 } else { 1.0 }))
                    .collect(),
                notes: Vec::new(),
            };
            let t = thin(&s);
            assert!(t.len() <= MAX_POINTS);
            assert!(t.iter().any(|p| p.1 == 99.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plot_names() {
        assert_eq!(
            friendly_name("resource_plot_cpu_all_jobs.json"),
            "CPU All Jobs"
        );
        assert_eq!(friendly_name("resource_plot_summary.json"), "Summary");
        assert_eq!(friendly_name("resource_plot.json"), "Summary");
        let mut v = vec!["job_10", "job_2", "summary", "cpu_all_jobs"];
        v.sort_by_key(|n| plot_order(n));
        assert_eq!(v, ["summary", "cpu_all_jobs", "job_2", "job_10"]);
        assert_eq!(
            plotly_title("A<br><sub>Peak: 1</sub>"),
            ("A".to_string(), "Peak: 1".to_string())
        );
    }
}
