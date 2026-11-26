//! Graph tab: a workflow's dependency graph (jobs, optionally with their files or user data),
//! laid out on the server (Sugiyama layering via `rust-sugiyama`) and drawn as inline SVG themed
//! with CSS variables. Pan and zoom are a transform on the SVG's root group held in the local
//! signal `$_dag`; clicking a job opens its details, clicking a file opens the file viewer.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use axum::Router;
use maud::{Markup, html};
use serde::Deserialize;
use torc::models::{
    FileModel, JobDependencyModel, JobFileRelationshipModel, JobModel,
    JobUserDataRelationshipModel, UserDataModel,
};

use super::Ctx;
use super::kit::{self, JOB_STATUSES, icon, js};
use crate::AppState;

const RELOAD: &str = "@get('/ui/tab')";
/// Above this many nodes: lay out left-to-right straight away and draw straight edges.
const LARGE: usize = 1000;
/// Above this many nodes, ask before rendering (a multi-megabyte SVG is slow to lay out and draw).
const HUGE: usize = 5000;
const NODE_H: f64 = 26.0;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct DagSig {
    /// jobs | files | userdata
    mode: String,
    /// "<wf>:<mode>" the user chose to render despite its size.
    force: String,
}

/// CSS color of a job status (matches the badges).
fn status_var(s: &str) -> &'static str {
    match s {
        "blocked" => "--text-muted",
        "ready" => "--info",
        "pending" | "running" => "--accent",
        "completed" => "--success",
        "failed" | "terminated" => "--danger",
        "canceled" | "pending_failed" => "--warning",
        "disabled" => "--border-strong",
        _ => "--text-faint",
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Job,
    File,
    UserData,
}

struct Node {
    kind: Kind,
    id: i64,
    name: String,
    label: String,
    status: &'static str,
    path: String,
}

/// Edge styles: plain dependency, job → file/user data it produces, file → job consuming it.
#[derive(Clone, Copy, PartialEq)]
enum Rel {
    Dep,
    Output,
    Input,
}

struct Graph {
    nodes: Vec<Node>,
    edges: Vec<(u32, u32, Rel)>,
    counts: BTreeMap<&'static str, usize>,
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    } else {
        s.to_string()
    }
}

async fn load_graph(st: &AppState, wf: i64, mode: &str) -> Result<Graph, String> {
    let q = [("workflow_id", wf.to_string())];
    let file_rels = format!("/workflows/{wf}/job_file_relationships");
    let ud_rels = format!("/workflows/{wf}/job_user_data_relationships");
    let deps_path = format!("/workflows/{wf}/job_dependencies");
    let jobs = kit::list_all::<JobModel>(st, "/jobs", &q);

    let mut g = Graph {
        nodes: Vec::new(),
        edges: Vec::new(),
        counts: BTreeMap::new(),
    };
    let mut index: HashMap<i64, u32> = HashMap::new();
    let add_jobs = |g: &mut Graph, index: &mut HashMap<i64, u32>, jobs: Vec<JobModel>| {
        for j in jobs {
            let id = j.id.unwrap_or_default();
            let status = j.status.map(|s| kit::wire_name(&s)).unwrap_or_default();
            let status = JOB_STATUSES
                .iter()
                .copied()
                .find(|s| *s == status)
                .unwrap_or("uninitialized");
            *g.counts.entry(status).or_default() += 1;
            let name = if j.name.is_empty() {
                format!("Job {id}")
            } else {
                j.name
            };
            index.insert(id, g.nodes.len() as u32);
            g.nodes.push(Node {
                kind: Kind::Job,
                id,
                label: truncate(&name, 40),
                name,
                status,
                path: String::new(),
            });
        }
    };

    if mode == "jobs" {
        // Explicit dependencies plus implicit ones from file/user-data producer → consumer links
        // (those only land in job_dependencies once the workflow is initialized).
        let (jobs, deps, frels, urels) = futures::try_join!(
            jobs,
            kit::list_all::<JobDependencyModel>(st, &deps_path, &[]),
            kit::list_all::<JobFileRelationshipModel>(st, &file_rels, &[]),
            kit::list_all::<JobUserDataRelationshipModel>(st, &ud_rels, &[]),
        )?;
        add_jobs(&mut g, &mut index, jobs);
        let links = deps
            .iter()
            .map(|d| (Some(d.depends_on_job_id), Some(d.job_id)))
            .chain(frels.iter().map(|r| (r.producer_job_id, r.consumer_job_id)))
            .chain(urels.iter().map(|r| (r.producer_job_id, r.consumer_job_id)));
        let mut seen = HashSet::new();
        for (from, to) in links {
            if let (Some(a), Some(b)) = (
                from.and_then(|f| index.get(&f)),
                to.and_then(|t| index.get(&t)),
            ) && a != b
                && seen.insert((*a, *b))
            {
                g.edges.push((*a, *b, Rel::Dep));
            }
        }
        return Ok(g);
    }

    // Items as (id, name, path), and (item id, producer, consumer) links.
    type Link = (i64, Option<i64>, Option<i64>);
    let (items, rels): (Vec<(i64, String, String)>, Vec<Link>) = if mode == "files" {
        let (jobs, files, rels) = futures::try_join!(
            jobs,
            kit::list_all::<FileModel>(st, "/files", &q),
            kit::list_all::<JobFileRelationshipModel>(st, &file_rels, &[]),
        )?;
        add_jobs(&mut g, &mut index, jobs);
        (
            files
                .into_iter()
                .map(|f| (f.id.unwrap_or_default(), f.name, f.path))
                .collect(),
            rels.into_iter()
                .map(|r| (r.file_id, r.producer_job_id, r.consumer_job_id))
                .collect(),
        )
    } else {
        let (jobs, uds, rels) = futures::try_join!(
            jobs,
            kit::list_all::<UserDataModel>(st, "/user_data", &q),
            kit::list_all::<JobUserDataRelationshipModel>(st, &ud_rels, &[]),
        )?;
        add_jobs(&mut g, &mut index, jobs);
        (
            uds.into_iter()
                .map(|u| (u.id.unwrap_or_default(), u.name, String::new()))
                .collect(),
            rels.into_iter()
                .map(|r| (r.user_data_id, r.producer_job_id, r.consumer_job_id))
                .collect(),
        )
    };
    let kind = if mode == "files" {
        Kind::File
    } else {
        Kind::UserData
    };
    let mut item_index = HashMap::new();
    for (id, name, path) in items {
        let name = if !name.is_empty() {
            name
        } else if kind == Kind::File {
            format!("File {id}")
        } else {
            format!("UserData {id}")
        };
        item_index.insert(id, g.nodes.len() as u32);
        g.nodes.push(Node {
            kind,
            id,
            label: truncate(&name, 28),
            name,
            status: "",
            path,
        });
    }
    let mut seen = HashSet::new();
    for (item, producer, consumer) in rels {
        let Some(&node) = item_index.get(&item) else {
            continue;
        };
        if let Some(&p) = producer.and_then(|p| index.get(&p))
            && seen.insert((p, node))
        {
            g.edges.push((p, node, Rel::Output));
        }
        if let Some(&c) = consumer.and_then(|c| index.get(&c))
            && seen.insert((node, c))
        {
            g.edges.push((node, c, Rel::Input));
        }
    }
    Ok(g)
}

// ============== Layout ==============

/// Node centers plus the drawing's size.
#[derive(Clone)]
struct Layout {
    pos: Vec<(f64, f64)>,
    w: f64,
    h: f64,
    lr: bool,
}

/// Approximate rendered width of a label at 11px (no text metrics on the server).
fn node_width(label: &str) -> f64 {
    label.chars().count() as f64 * 6.4 + 22.0
}

/// Lay out a DAG top-to-bottom (or left-to-right with `lr`). Each connected part is laid out on
/// its own (the crate's own splitting is quadratic in the number of parts), then the parts are
/// packed into rows.
fn layout(sizes: &[(f64, f64)], edges: &[(u32, u32)], lr: bool) -> Layout {
    const GAP: f64 = 40.0;
    let cfg = rust_sugiyama::configure::Config {
        vertex_spacing: 16.0,
        // Transposition sweeps are what make big layered graphs slow; barycenter alone is fine.
        transpose: sizes.len() <= LARGE,
        ..Default::default()
    };

    // Connected parts (union-find), in order of their first node.
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let mut parent: Vec<usize> = (0..sizes.len()).collect();
    for &(a, b) in edges {
        let (ra, rb) = (root(&mut parent, a as usize), root(&mut parent, b as usize));
        parent[ra.max(rb)] = ra.min(rb);
    }
    let mut members: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..sizes.len() {
        let r = root(&mut parent, i);
        members.entry(r).or_default().push(i);
    }
    let mut part_edges: HashMap<usize, Vec<(u32, u32)>> = HashMap::new();
    for &(a, b) in edges {
        let r = root(&mut parent, a as usize);
        part_edges.entry(r).or_default().push((a, b));
    }

    let mut pos = vec![(0.0, 0.0); sizes.len()];
    let mut parts = Vec::new();
    for (r, nodes) in &members {
        if nodes.len() > 1 {
            // The crate lays out top-to-bottom: size.0 is along a layer, size.1 across layers.
            // Pad the across-layer size so ranks get a readable gap.
            let local: HashMap<usize, u32> = nodes
                .iter()
                .enumerate()
                .map(|(k, &i)| (i, k as u32))
                .collect();
            let verts: Vec<(u32, (f64, f64))> = nodes
                .iter()
                .enumerate()
                .map(|(k, &i)| {
                    let (w, h) = sizes[i];
                    (k as u32, if lr { (h, w + 40.0) } else { (w, h + 40.0) })
                })
                .collect();
            let es: Vec<(u32, u32)> = part_edges[r]
                .iter()
                .map(|(a, b)| (local[&(*a as usize)], local[&(*b as usize)]))
                .collect();
            for (part, _, _) in rust_sugiyama::from_vertices_and_edges(&verts, &es, &cfg) {
                for (k, (a, b)) in part {
                    pos[nodes[k]] = if lr { (b, a) } else { (a, b) };
                }
            }
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for &i in nodes {
            let ((x, y), (w, h)) = (pos[i], sizes[i]);
            x0 = x0.min(x - w / 2.0);
            x1 = x1.max(x + w / 2.0);
            y0 = y0.min(y - h / 2.0);
            y1 = y1.max(y + h / 2.0);
        }
        parts.push((nodes, (x0, y0, x1 - x0, y1 - y0)));
    }
    let boxes: Vec<_> = parts.iter().map(|p| p.1).collect();
    // Shelf-pack the parts into rows about as wide as the whole is tall.
    let area: f64 = boxes.iter().map(|b| (b.2 + GAP) * (b.3 + GAP)).sum();
    let widest = boxes.iter().map(|b| b.2).fold(0.0, f64::max);
    let row_w = (area.sqrt() * 1.6).max(widest);
    let (mut cx, mut cy, mut row_h, mut w, mut h) = (0.0, 0.0, 0.0f64, 0.0f64, 0.0f64);
    let mut shift = Vec::new();
    for b in &boxes {
        if cx > 0.0 && cx + b.2 > row_w {
            cx = 0.0;
            cy += row_h + GAP;
            row_h = 0.0;
        }
        shift.push((cx - b.0, cy - b.1));
        w = w.max(cx + b.2);
        h = h.max(cy + b.3);
        row_h = row_h.max(b.3);
        cx += b.2 + GAP;
    }
    for ((nodes, _), (dx, dy)) in parts.iter().zip(shift) {
        for &i in nodes.iter() {
            pos[i].0 += dx;
            pos[i].1 += dy;
        }
    }
    Layout { pos, w, h, lr }
}

/// Last layout, so auto-refreshes of an unchanged graph don't lay it out again.
static LAST: Mutex<Option<(u64, Layout)>> = Mutex::new(None);

/// Lay out off the async runtime; None if the layout failed (the crate panicked).
async fn layout_cached(sizes: Vec<(f64, f64)>, edges: Vec<(u32, u32)>) -> Option<Layout> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    sizes.len().hash(&mut hasher);
    for (w, _) in &sizes {
        w.to_bits().hash(&mut hasher);
    }
    edges.hash(&mut hasher);
    let key = hasher.finish();
    if let Some((k, l)) = LAST.lock().unwrap().as_ref()
        && *k == key
    {
        return Some(l.clone());
    }
    // The crate recurses deeply on big graphs, so it gets a thread with a large stack.
    let job = move || {
        let started = std::time::Instant::now();
        let large = sizes.len() > LARGE;
        // Big workflows are usually wide parameter sweeps, so start those left-to-right.
        let mut l = layout(&sizes, &edges, large);
        // Wide, shallow graphs read better left-to-right too.
        if !large && l.w > 4.0 * l.h {
            l = layout(&sizes, &edges, true);
        }
        tracing::debug!(
            "Graph layout of {} nodes, {} edges took {:?}",
            sizes.len(),
            edges.len(),
            started.elapsed()
        );
        l
    };
    let l = tokio::task::spawn_blocking(move || {
        std::thread::Builder::new()
            .stack_size(512 << 20)
            .spawn(job)
            .ok()?
            .join()
            .ok()
    })
    .await
    .ok()
    .flatten()?;
    *LAST.lock().unwrap() = Some((key, l.clone()));
    Some(l)
}

// ============== View ==============

pub async fn view(st: &Arc<AppState>, cx: &Ctx) -> Markup {
    let sig: DagSig = kit::ns(&cx.sig, "dag");
    let mode = match sig.mode.as_str() {
        m @ ("files" | "userdata") => m,
        _ => "jobs",
    };
    let body = match cx.g.wf {
        None => {
            html! { div.card.empty { "Pick a workflow in the top bar to see its dependency graph." } }
        }
        Some(wf) => canvas(st, wf, mode, &sig).await,
    };
    html! {
        div.page.dag-page data-signals__ifmissing="{dag: {mode: 'jobs', force: ''}, _dag: {s: 1, x: 0, y: 0, hover: ''}}" {
            div.page-header {
                button.btn.btn-sm.btn-ghost title="Back (Backspace)" data-show="history.length > 1"
                    data-on:click="history.back()" { "← Back" }
                h2 { "Dependency graph" }
                (kit::seg_tabs("dag.mode", mode, &[
                    ("jobs", "Jobs", None),
                    ("files", "Jobs + files", None),
                    ("userdata", "Jobs + user data", None),
                ], RELOAD))
            }
            (body)
        }
    }
}

async fn canvas(st: &AppState, wf: i64, mode: &str, sig: &DagSig) -> Markup {
    let g = match load_graph(st, wf, mode).await {
        Ok(g) => g,
        Err(e) => {
            return html! { div.card.dag-card { div.overlay { span.error-text { "Failed to load graph: " (e) } } } };
        }
    };
    let n = g.nodes.len();
    let key = format!("{wf}:{mode}");
    if n == 0 {
        return html! { div.card.dag-card { div.overlay.muted { "This workflow has no jobs." } } };
    }
    if n > HUGE && sig.force != key {
        return html! {
            div.card.dag-card { div.overlay { div.empty {
                div {
                    "This graph has " (n) " nodes and " (g.edges.len())
                    " edges. Laying it out and drawing it may take a while."
                }
                button.btn data-on:click=(format!("$dag.force = {}; {RELOAD}", js(&key))) { "Render anyway" }
            } } }
        };
    }

    let sizes: Vec<(f64, f64)> = g
        .nodes
        .iter()
        .map(|n| (node_width(&n.label), NODE_H))
        .collect();
    let edges: Vec<(u32, u32)> = g.edges.iter().map(|e| (e.0, e.1)).collect();
    let Some(l) = layout_cached(sizes.clone(), edges).await else {
        return html! { div.card.dag-card { div.overlay { span.error-text { "Failed to lay out the graph." } } } };
    };
    let (gw, gh) = (l.w.max(1.0), l.h.max(1.0));
    let fit = |svg: &str| {
        format!(
            "const r = {svg}.getBoundingClientRect(), s = Math.max(0.001, Math.min(1.2, (r.width - 48) / {gw}, (r.height - 48) / {gh})); \
             $_dag.s = s; $_dag.x = (r.width - {gw} * s) / 2; $_dag.y = (r.height - {gh} * s) / 2"
        )
    };
    let zoom = |f: f64| {
        format!(
            "const r = document.getElementById('dag-svg').getBoundingClientRect(), s = Math.min(3, Math.max(0.001, $_dag.s * {f})); \
             $_dag.x = r.width / 2 - (r.width / 2 - $_dag.x) * s / $_dag.s; $_dag.y = r.height / 2 - (r.height / 2 - $_dag.y) * s / $_dag.s; $_dag.s = s"
        )
    };
    let large = n > LARGE;
    let edge_path = |a: usize, b: usize| {
        let ((x1, y1), (x2, y2)) = (l.pos[a], l.pos[b]);
        let ((w1, h1), (w2, h2)) = (sizes[a], sizes[b]);
        if l.lr {
            let (sx, tx) = (x1 + w1 / 2.0, x2 - w2 / 2.0);
            if large {
                format!("M{sx:.1} {y1:.1}L{tx:.1} {y2:.1}")
            } else {
                let m = (sx + tx) / 2.0;
                format!("M{sx:.1} {y1:.1}C{m:.1} {y1:.1} {m:.1} {y2:.1} {tx:.1} {y2:.1}")
            }
        } else {
            let (sy, ty) = (y1 + h1 / 2.0, y2 - h2 / 2.0);
            if large {
                format!("M{x1:.1} {sy:.1}L{x2:.1} {ty:.1}")
            } else {
                let m = (sy + ty) / 2.0;
                format!("M{x1:.1} {sy:.1}C{x1:.1} {m:.1} {x2:.1} {m:.1} {x2:.1} {ty:.1}")
            }
        }
    };

    html! {
        div.card.dag-card {
            svg #dag-svg.dag-svg xmlns="http://www.w3.org/2000/svg"
                data-on:wheel__prevent="const r = el.getBoundingClientRect(), px = evt.clientX - r.left, py = evt.clientY - r.top, s = Math.min(3, Math.max(0.001, $_dag.s * Math.exp(-evt.deltaY * 0.002))); $_dag.x = px - (px - $_dag.x) * s / $_dag.s; $_dag.y = py - (py - $_dag.y) * s / $_dag.s; $_dag.s = s"
                data-on:pointerdown="el._d = {x: evt.clientX - $_dag.x, y: evt.clientY - $_dag.y, cx: evt.clientX, cy: evt.clientY}; el._m = false"
                data-on:pointermove="el._d && (el._m = el._m || Math.abs(evt.clientX - el._d.cx) + Math.abs(evt.clientY - el._d.cy) > 4) && (el.hasPointerCapture(evt.pointerId) || el.setPointerCapture(evt.pointerId), $_dag.x = evt.clientX - el._d.x, $_dag.y = evt.clientY - el._d.y)"
                data-on:pointerup="el._d = null"
                data-on:pointercancel="el._d = null"
                data-on:click="const n = evt.target.closest('[data-href]'); !el._m && n && @get(n.dataset.href)"
                data-on:mouseover="$_dag.hover = evt.target.closest('.n')?.dataset.tip || ''"
                data-on:mouseleave="$_dag.hover = ''" {
                defs {
                    @for (id, class) in [("dag-arrow", "dep"), ("dag-arrow-output", "output"), ("dag-arrow-input", "input")] {
                        marker id=(id) class=(class) viewBox="0 0 10 10" refX="9" refY="5"
                            markerWidth="7" markerHeight="7" orient="auto-start-reverse" {
                            path d="M0 0L10 5L0 10z" {}
                        }
                    }
                }
                // A new id per graph/size: the morph swaps the group in, so its data-init fits
                // the new graph, while refreshes of the same graph keep the user's pan and zoom.
                g id=(format!("dag-view-{wf}-{mode}-{n}-{}", g.edges.len())) data-init=(fit("el.ownerSVGElement"))
                    data-attr:transform="`translate(${$_dag.x} ${$_dag.y}) scale(${$_dag.s})`" {
                    g.edges {
                        @for (a, b, rel) in &g.edges {
                            @let (class, marker) = match rel {
                                Rel::Dep => ("e", "url(#dag-arrow)"),
                                Rel::Output => ("e output", "url(#dag-arrow-output)"),
                                Rel::Input => ("e input", "url(#dag-arrow-input)"),
                            };
                            path class=(class) d=(edge_path(*a as usize, *b as usize)) marker-end=(marker) {}
                        }
                    }
                    g.nodes {
                        @for (i, node) in g.nodes.iter().enumerate() {
                            @let (w, h) = sizes[i];
                            @let (x, y) = l.pos[i];
                            (node_markup(node, w, h, x - w / 2.0, y - h / 2.0))
                        }
                    }
                }
            }
            div.controls {
                button.btn.btn-sm.btn-icon title="Zoom in" aria-label="Zoom in" data-on:click=(zoom(1.25)) { (icon("plus", 14)) }
                button.btn.btn-sm.btn-icon title="Zoom out" aria-label="Zoom out" data-on:click=(zoom(0.8)) {
                    svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true" { path d="M5 12h14" {} }
                }
                button.btn.btn-sm title="Fit to screen" data-on:click=(fit("document.getElementById('dag-svg')")) { "Fit" }
            }
            div.info {
                strong data-show="$_dag.hover" data-text="$_dag.hover" style="display:none" {}
                span.faint data-show="!$_dag.hover" { (n) " nodes · " (g.edges.len()) " edges" }
            }
            div.legend {
                @for (s, c) in &g.counts {
                    span.item {
                        span.swatch style=(format!("--c: var({})", status_var(s))) {}
                        (kit::status_label(s))
                        span.faint { (c) }
                    }
                }
                @if mode == "files" { span.item { span.swatch.file {} "file" } }
                @if mode == "userdata" { span.item { span.swatch.ud {} "user data" } }
                @if mode != "jobs" {
                    span.item { span.line {} "produces" }
                    span.item { span.line.dashed {} "consumed by" }
                }
            }
        }
    }
}

fn node_markup(n: &Node, w: f64, h: f64, x: f64, y: f64) -> Markup {
    let (class, href, tip) = match n.kind {
        Kind::Job => (
            format!("n job s-{}", n.status),
            Some(format!("/ui/job/{}", n.id)),
            format!("{} · {}", n.name, kit::status_label(n.status)),
        ),
        Kind::File => (
            "n file".to_string(),
            Some(format!(
                "/ui/file?path={}&title={}",
                urlencode(&n.path),
                urlencode(&n.name)
            )),
            format!("{} · file", n.name),
        ),
        Kind::UserData => ("n ud".to_string(), None, format!("{} · user data", n.name)),
    };
    let shape = match n.kind {
        Kind::Job => html! { rect width=(w) height=(h) rx="6" {} },
        // A parallelogram for files, a tag for user data (as in the old cytoscape styling).
        Kind::File => {
            html! { polygon points=(format!("6,0 {w},0 {},{h} 0,{h}", w - 6.0)) {} }
        }
        Kind::UserData => html! {
            polygon points=(format!("0,0 {},0 {w},{} {},{h} 0,{h}", w - 8.0, h / 2.0, w - 8.0)) {}
        },
    };
    html! {
        g class=(class) transform=(format!("translate({x:.1} {y:.1})")) data-href=[href] data-tip=(tip) {
            title { (tip) }
            (shape)
            text x=(format!("{:.1}", w / 2.0)) y=(h / 2.0) { (n.label) }
        }
    }
}

fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Layout time for a big parameter sweep (fan-out → N workers → fan-in) and a chain-ish
    /// graph. Run with `--nocapture` to see the numbers.
    #[test]
    fn layout_scales() {
        for n in [100usize, 2000] {
            let sizes = vec![(160.0, NODE_H); n + 2];
            let mut edges = Vec::new();
            for i in 0..n as u32 {
                edges.push((0, i + 2));
                edges.push((i + 2, 1));
            }
            let t = std::time::Instant::now();
            let l = layout(&sizes, &edges, n > LARGE);
            println!("sweep {n}: {:?} ({:.0}x{:.0})", t.elapsed(), l.w, l.h);
            assert_eq!(l.pos.len(), n + 2);

            // Layers of 10 with each job depending on two of the previous layer.
            let edges: Vec<(u32, u32)> = (10..n as u32)
                .flat_map(|i| [(i - 10, i), (i - 10 + (i + 3) % 10 - i % 10, i)])
                .filter(|(a, b)| a != b)
                .collect();
            let sizes = vec![(160.0, NODE_H); n];
            let t = std::time::Instant::now();
            layout(&sizes, &edges, n > LARGE);
            println!("layered {n}: {:?}", t.elapsed());

            // Independent jobs: one part per job, packed into rows.
            let t = std::time::Instant::now();
            let l = layout(&sizes, &[], false);
            println!("independent {n}: {:?} ({:.0}x{:.0})", t.elapsed(), l.w, l.h);
            assert!(l.w < l.h * 4.0);
        }
    }
}
