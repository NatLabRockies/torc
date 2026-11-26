//! The dashboard UI: server-rendered HTML (maud) driven by Datastar (`torc-dash/static/ui/`).
//!
//! The shell page (`/`) holds the sidebar, top bar, and slots for modals, menus, toasts, and the
//! execution panel. Tabs are hash routes (`#/details`); `GET /ui/tab` renders the tab named by
//! the `$tab` signal into `#content`, and re-runs on refresh. Each feature module exposes
//! `routes()` for its own actions and, for tabs, `view()`.

// ponytail: remove once every feature module uses the kit.
#[allow(dead_code)]
pub mod kit;

mod chat;
mod config;
mod create;
mod dag;
mod debugging;
mod details;
mod editor;
mod events;
mod exec;
mod resources;
mod wf_dialogs;
mod workflows;

use std::sync::Arc;

use axum::{
    Router,
    extract::{Query, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
    routing::get,
};
use maud::{DOCTYPE, Markup, html};
use serde_json::Value;

use crate::AppState;
use kit::{G, Sig, Ui, icon};

/// (id, label, shortcut key, icon).
pub const TABS: [(&str, &str, &str, &str); 9] = [
    ("workflows", "Workflows", "1", "layers"),
    ("details", "Details", "2", "list"),
    ("dag", "Graph", "3", "graph"),
    ("events", "Events", "4", "bell"),
    ("debugging", "Debugging", "5", "bug"),
    ("resources", "Resources", "6", "activity"),
    ("editor", "Spec editor", "e", "edit"),
    ("chat", "AI Chat", "7", "chat"),
    ("config", "Settings", "8", "settings"),
];

/// Tabs that work on the selected workflow; the top bar shows the workflow picker for these.
const WORKFLOW_TABS: [&str; 5] = ["details", "dag", "events", "debugging", "resources"];

/// What a tab's `view` gets: the global signals, all signals (for the tab's own namespace, read
/// with `kit::ns`), and whether this render is an automatic refresh.
#[allow(dead_code)] // ponytail: `auto` is for tabs that skip heavy work on auto-refresh.
pub struct Ctx {
    pub g: G,
    pub sig: Value,
    pub auto: bool,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ui/tab", get(tab_handler))
        .merge(workflows::routes())
        .merge(wf_dialogs::routes())
        .merge(exec::routes())
        .merge(details::routes())
        .merge(dag::routes())
        .merge(events::routes())
        .merge(debugging::routes())
        .merge(resources::routes())
        .merge(editor::routes())
        .merge(create::routes())
        .merge(chat::routes())
        .merge(config::routes())
}

// ============== Dashboard token ==============

/// Name of the cookie carrying this dashboard's token. Cookies ignore ports, so dashboards on
/// other ports of the same host each get their own name (derived from their token).
pub fn token_cookie_name(token: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    token.hash(&mut h);
    format!("torc_dash_{:08x}", h.finish() as u32)
}

/// The token in this request's cookie, if any.
pub fn token_from_cookie(headers: &HeaderMap, token: &str) -> Option<String> {
    let name = token_cookie_name(token);
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_string())
}

// ============== Shell ==============

/// `GET /`: the shell. Opened with `?token=`, it stores the token in a cookie (HttpOnly, so page
/// scripts never see it) that every later request carries.
pub async fn index_handler(
    State(st): State<Arc<AppState>>,
    Query(q): Query<std::collections::HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let mut set_cookie = None;
    if let Some(expected) = &st.token {
        let given = q
            .get("token")
            .cloned()
            .or_else(|| token_from_cookie(&headers, expected));
        if !given
            .as_deref()
            .is_some_and(|t| crate::same_secret(t, expected))
        {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                maud::html! {
                    (DOCTYPE)
                    html { head { title { "Torc" } } body style="font-family: system-ui; padding: 40px" {
                        h2 { "Missing or wrong dashboard token" }
                        p { "Open the dashboard with the URL torc-dash printed at startup (it ends in ?token=...)." }
                    } }
                },
            )
                .into_response();
        }
        if q.contains_key("token") {
            set_cookie = Some(format!(
                "{}={}; Path=/; HttpOnly; SameSite=Strict",
                token_cookie_name(expected),
                expected
            ));
        }
    }
    let page = shell(&st).await;
    match set_cookie {
        Some(c) => ([(header::SET_COOKIE, c)], page).into_response(),
        None => page.into_response(),
    }
}

async fn shell(st: &Arc<AppState>) -> Markup {
    let user = torc::get_username();
    let v: Value = kit::json_of(crate::version_handler(State(st.clone())).await)
        .await
        .unwrap_or_default();
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(String::from);
    let version = s("version").unwrap_or_default();
    let server_version = s("server_version");
    let mismatch = s("version_mismatch");
    let severity = s("mismatch_severity");
    let can_stop = v
        .get("can_stop_runs")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let defaults = serde_json::json!({
        "tab": "workflows", "wf": null, "refresh": 30, "allUsers": false, "archived": false,
        "group": "", "outputDir": "torc_output", "theme": "system", "palette": "default",
        "canStopRuns": can_stop, "_t": 0,
    });

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Torc Dashboard" }
                link rel="icon" href="/static/images/torc_icon_32.png";
                link rel="stylesheet" href="/static/ui/app.css";
                @for f in ["workflows", "details", "dag", "events", "debugging", "resources", "editor", "create", "chat", "config", "exec"] {
                    link rel="stylesheet" href=(format!("/static/ui/{f}.css"));
                }
                script src="/static/ui/shell.js" {}
                script type="module" src="/static/ui/datastar.js" {}
            }
            body data-signals=(format!("Object.assign({defaults}, torcPrefs)"))
                data-effect="torcTheme($theme, $palette); torcSave({wf: $wf, refresh: $refresh, allUsers: $allUsers, archived: $archived, group: $group, outputDir: $outputDir, theme: $theme, palette: $palette})"
                data-init="$tab = torcTab(); @get('/ui/tab')"
                data-on:hashchange__window="$tab = torcTab(); @get('/ui/tab')"
                data-on:torc-refresh="@get('/ui/tab')"
                data-on:torc-theme="$theme = ({system: 'light', light: 'dark', dark: 'system'})[$theme] || 'system'"
                data-on:torc-create="@get('/ui/create/open')"
                "data-on-interval__duration.1s"="$refresh > 0 && document.visibilityState === 'visible' && ++$_t >= $refresh && ($_t = 0, @get('/ui/tab?auto=1'))" {
                div.shell {
                    nav.sidebar {
                        div.brand {
                            img src="/static/images/torc_icon_32.png" alt="" width="22" height="22";
                            span { "Torc" }
                        }
                        ul {
                            @for (id, label, _, ic) in TABS {
                                li { a href=(format!("#/{id}")) data-class:active=(format!("$tab === '{id}'")) {
                                    (icon(ic, 16))
                                    span.grow { (label) }
                                    @if id == "events" { (events::badge()) }
                                } }
                            }
                        }
                        div.sidebar-footer {
                            div #conn.status { span.dot {} "Connecting…" }
                            div.meta {
                                span title="User" { (icon("user", 12)) " " (user) }
                                span.warn[mismatch.is_some()] title=(mismatch.clone().unwrap_or_else(|| "Version".into())) {
                                    @if mismatch.is_some() { "⚠ " }
                                    "v" (version)
                                    @if let Some(sv) = &server_version { " / server v" (sv) }
                                }
                            }
                            div.footer-actions {
                                button.btn.btn-ghost.btn-sm title="Theme (T)" data-on:click="el.dispatchEvent(new CustomEvent('torc-theme', {bubbles: true}))" {
                                    span.cap data-text="$theme" { "system" }
                                }
                                button.btn.btn-ghost.btn-sm data-on:click="document.getElementById('shortcuts').showModal()" {
                                    kbd { "?" } " Shortcuts"
                                }
                            }
                        }
                    }
                    div.main {
                        header #topbar.topbar { h1 {} }
                        main #content.content { div.empty { span.spinner {} } }
                    }
                }
                div #modals {}
                div #menu.menu popover="auto" role="menu" {}
                div #exec {}
                div #toasts.toasts aria-live="polite" {
                    @if let (Some(m), Some(sev)) = (&mismatch, &severity) {
                        @if sev != "patch" {
                            div.toast.(if sev == "major" { "error" } else { "warning" }) role="status" {
                                span.msg { (m) }
                                button.btn.btn-ghost.btn-sm.btn-icon aria-label="Dismiss" data-on:click="el.parentElement.remove()" { "✕" }
                            }
                        }
                    }
                }
                (shortcuts_dialog())
            }
        }
    }
}

fn shortcuts_dialog() -> Markup {
    html! {
        dialog #shortcuts.modal.sm data-on:click="evt.target === el && el.close()" {
            header {
                h3 { "Keyboard shortcuts" }
                button.btn.btn-ghost.btn-icon.btn-sm aria-label="Close" data-on:click="el.closest('dialog').close()" { "✕" }
            }
            div.body {
                dl.kv.shortcuts {
                    @for (_, label, key, _) in TABS { dt { kbd { (key.to_uppercase()) } } dd { (label) } }
                    dt { kbd { "G" } } dd { "Graph" }
                    dt { kbd { "⌫" } } dd { "Back from the graph" }
                    dt { kbd { "N" } } dd { "New workflow" }
                    dt { kbd { "R" } } dd { "Refresh" }
                    dt { kbd { "T" } } dd { "Cycle theme (system / light / dark)" }
                    dt { kbd { "?" } } dd { "This help" }
                    dt { kbd { "Esc" } } dd { "Close dialog / leave text field" }
                    dt { kbd { "/" } } dd { "Focus the filter" }
                }
            }
        }
    }
}

// ============== Tabs ==============

#[derive(serde::Deserialize)]
struct TabQuery {
    #[serde(default)]
    auto: Option<String>,
}

/// Render the current tab (`$tab`) into `#content`, plus the top bar and connection status.
async fn tab_handler(
    State(st): State<Arc<AppState>>,
    Query(q): Query<TabQuery>,
    Sig(sig): Sig<Value>,
) -> Ui {
    let mut g: G = serde_json::from_value(sig.clone()).unwrap_or_default();
    let (tab, label) = TABS
        .iter()
        .find(|t| t.0 == g.tab)
        .map(|t| (t.0, t.1))
        .unwrap_or(("workflows", "Workflows"));
    let mut ui = Ui::new();

    let picker = if WORKFLOW_TABS.contains(&tab) {
        let list: Vec<torc::models::WorkflowModel> =
            kit::list_all(&st, "/workflows", &g.workflow_query())
                .await
                .unwrap_or_default();
        let mut list = list;
        list.sort_by_key(|w| std::cmp::Reverse(w.id));
        // Default to the newest workflow so workflow tabs never open empty.
        if g.wf.is_none()
            && let Some(id) = list.first().and_then(|w| w.id)
        {
            g.wf = Some(id);
            ui = ui.signals(serde_json::json!({ "wf": id }));
        }
        Some(workflow_picker(&list, g.wf))
    } else {
        None
    };

    let ping: Result<Value, String> = kit::get(&st, "/ping", &[]).await;
    let conn = html! {
        div #conn.status title=(ping.as_ref().err().cloned().unwrap_or_else(|| "Connected to torc-server".into())) {
            span.dot.ok[ping.is_ok()].bad[ping.is_err()] {}
            (if ping.is_ok() { "Connected" } else { "Disconnected" })
        }
    };

    let cx = Ctx {
        g,
        sig,
        auto: q.auto.is_some(),
    };
    let content = match tab {
        "details" => details::view(&st, &cx).await,
        "dag" => dag::view(&st, &cx).await,
        "events" => events::view(&st, &cx).await,
        "debugging" => debugging::view(&st, &cx).await,
        "resources" => resources::view(&st, &cx).await,
        "editor" => editor::view(&st, &cx).await,
        "chat" => chat::view(&st, &cx).await,
        "config" => config::view(&st, &cx).await,
        _ => workflows::view(&st, &cx).await,
    };

    ui.patch(html! {
        header #topbar.topbar {
            h1 { (label) }
            @if let Some(p) = picker { (p) }
            div.grow {}
            button.btn.btn-ghost.btn-sm title="Refresh (R)" data-on:click="@get('/ui/tab')" data-indicator="_loading" {
                span.spinner.small data-show="$_loading" style="display:none" {}
                span data-show="!$_loading" { (icon("refresh", 14)) }
                " Refresh"
            }
        }
    })
    .patch(html! { main #content.content data-tab=(tab) { (content) } })
    .patch(conn)
}

fn workflow_picker(list: &[torc::models::WorkflowModel], selected: Option<i64>) -> Markup {
    let missing = selected.filter(|id| !list.iter().any(|w| w.id == Some(*id)));
    html! {
        label.picker {
            span.label { "Workflow" }
            select.select data-bind="wf" data-on:change="@get('/ui/tab')" {
                option value="" disabled selected[selected.is_none()] { "Select a workflow…" }
                @if let Some(id) = missing { option value=(id) selected { "#" (id) } }
                @for w in list {
                    @let id = w.id.unwrap_or_default();
                    option value=(id) selected[Some(id) == selected] { "#" (id) " · " (w.name) }
                }
            }
        }
    }
}
