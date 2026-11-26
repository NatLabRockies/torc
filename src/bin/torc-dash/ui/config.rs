//! Settings: preferences (bound to the global signals the shell persists), version info, and the
//! torc-server process managed by torc-dash.

use std::sync::Arc;

use axum::{Router, extract::State, routing::post};
use maud::{Markup, html};
use serde::Deserialize;
use serde_json::{Value, json};

use super::Ctx;
use super::kit::{self, Sig, Ui, icon, js};
use crate::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ui/config/test", post(test_connection))
        .route("/ui/config/server/start", post(start_server))
        .route("/ui/config/server/stop", post(stop_server))
}

const THEMES: [(&str, &str, &str); 3] = [
    ("system", "System", "monitor"),
    ("light", "Light", "sun"),
    ("dark", "Dark", "moon"),
];

const PALETTES: [(&str, &str); 9] = [
    ("default", "Default"),
    ("warm", "Warm Gray"),
    ("neutral", "Neutral"),
    ("dimmed", "Dimmed"),
    ("nord", "Nord"),
    ("dracula", "Dracula"),
    ("midnight", "Midnight Blue"),
    ("high-contrast", "High Contrast"),
    ("monokai", "Monokai"),
];

const REFRESH_OPTIONS: [u32; 8] = [0, 5, 10, 15, 30, 60, 120, 300];

/// `$config`: the server start form.
#[derive(Deserialize)]
#[serde(default)]
struct ServerForm {
    port: Value,
    level: String,
    db: String,
}

impl Default for ServerForm {
    fn default() -> Self {
        // The dashboard talks to its configured API URL (default port 8080), so 8080 is the
        // useful default.
        Self {
            port: json!(8080),
            level: "info".into(),
            db: String::new(),
        }
    }
}

async fn version(st: &Arc<AppState>) -> Value {
    kit::json_of(crate::version_handler(State(st.clone())).await)
        .await
        .unwrap_or_default()
}

pub async fn view(st: &Arc<AppState>, cx: &Ctx) -> Markup {
    let v = version(st).await;
    let s = |k: &str| v.get(k).and_then(Value::as_str);
    // Keep a custom value (e.g. set before this list existed) selectable.
    let mut refresh = REFRESH_OPTIONS.to_vec();
    if !refresh.contains(&cx.g.refresh) {
        refresh.push(cx.g.refresh);
        refresh.sort_unstable();
    }

    html! {
        div.page.config data-signals__ifmissing=(json!({"config": {"port": 8080, "level": "info", "db": ""}}).to_string()) {
            div.page-header { h2 { "Settings" } }
            div.config-grid {
                section.card.card-body.stack {
                    h3 { "Appearance & behavior" }
                    div.field {
                        span { "Theme" }
                        div.seg role="radiogroup" aria-label="Theme" {
                            @for (id, label, ic) in THEMES {
                                @let is = format!("$theme === '{id}'");
                                button role="radio" data-class:active=(is) data-attr:aria-checked=(is)
                                    data-on:click=(format!("$theme = '{id}'")) {
                                    (icon(ic, 14)) (label)
                                }
                            }
                        }
                        span.hint { "Press " kbd { "T" } " anywhere to cycle." }
                    }
                    label.field data-show="$theme !== 'light'" {
                        span { "Dark palette" }
                        select.select data-bind="palette" {
                            @for (id, label) in PALETTES { option value=(id) { (label) } }
                        }
                    }
                    label.field {
                        span { "Auto-refresh" }
                        select.select data-bind="refresh" {
                            @for s in &refresh {
                                option value=(s) {
                                    @if *s == 0 { "Off" }
                                    @else if *s < 60 { "Every " (s) "s" }
                                    @else { "Every " (s / 60) " min" }
                                }
                            }
                        }
                        span.hint { "Paused while the window is hidden. Press " kbd { "R" } " to refresh now." }
                    }
                    label.field {
                        span { "Job output directory" }
                        input.input.mono data-bind="outputDir" placeholder="torc_output" spellcheck="false";
                        span.hint { "Where jobs write their logs (torc run's output directory). A relative path is resolved against each workflow's submission directory." }
                    }
                    label.check {
                        input type="checkbox" data-bind="allUsers";
                        " Show workflows from all users by default"
                    }
                    (notify_setting())
                }

                section.card.card-body.stack {
                    h3 { "Version" }
                    @if v.is_null() {
                        p.error-text { "Could not read the version." }
                    } @else {
                        @if let Some(m) = s("version_mismatch") {
                            div.banner.major[s("mismatch_severity") == Some("major")] { (m) }
                        }
                        dl.kv {
                            dt { "Dashboard" }
                            dd.mono { "v" (s("version").unwrap_or("?")) " " span.faint { "(API " (s("api_version").unwrap_or("?")) ")" } }
                            dt { "Server" }
                            dd.mono {
                                @if let Some(sv) = s("server_version") {
                                    "v" (sv)
                                    @if let Some(a) = s("server_api_version") { " " span.faint { "(API " (a) ")" } }
                                } @else {
                                    span.muted { "unreachable" }
                                }
                            }
                            dt { "Server URL" }
                            dd.mono { (s("api_url").unwrap_or("")) }
                            dt { "User" }
                            dd { (torc::get_username()) }
                        }
                    }
                    div.test {
                        button.btn.btn-sm data-indicator="_testing" data-attr:disabled="$_testing"
                            data-on:click="@post('/ui/config/test')" {
                            span.spinner.small data-show="$_testing" style="display:none" {}
                            " Test connection"
                        }
                        span #config-test {}
                    }
                }

                (server_panel(st).await)
            }
        }
    }
}

/// Desktop notifications when a followed workflow finishes: a per-browser choice kept in
/// localStorage (`torc-dash.notify` = "1"), which the events module reads.
fn notify_setting() -> Markup {
    let init = "{_notify: (() => { try { return localStorage.getItem('torc-dash.notify') === '1' } catch { return false } })(), \
                _perm: typeof Notification === 'undefined' ? 'unsupported' : Notification.permission}";
    html! {
        div.field data-signals=(init) {
            label.check {
                input type="checkbox" data-bind="_notify" data-attr:disabled="$_perm === 'unsupported'"
                    data-on:change="try { el.checked ? localStorage.setItem('torc-dash.notify', '1') : localStorage.removeItem('torc-dash.notify') } catch {}; \
                                    el.checked && $_perm === 'default' && Notification.requestPermission().then(p => $_perm = p)";
                " Notify me when a workflow finishes"
            }
            span.hint data-show="$_perm === 'unsupported'" style="display:none" { "This browser does not support notifications." }
            span.hint data-show="$_perm !== 'unsupported' && $_notify && $_perm === 'denied'" style="display:none" {
                "Notifications are blocked for this site; allow them in the browser's site settings."
            }
            span.hint data-show="$_perm !== 'unsupported' && !($_notify && $_perm === 'denied')" {
                "For the selected workflow and those you run, watch, or submit here, while this window is in the background."
            }
        }
    }
}

async fn server_panel(st: &Arc<AppState>) -> Markup {
    let status: Value = kit::json_of(crate::server_status_handler(State(st.clone())).await)
        .await
        .unwrap_or_default();
    let managed = status["managed"].as_bool().unwrap_or(false);
    let running = status["running"].as_bool().unwrap_or(false) && managed;
    let connected = kit::get::<Value>(st, "/ping", &[]).await.is_ok();
    let output: Vec<&str> = status["output_lines"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let stop_msg = format!(
        "Stop torc-server?\n\nThis kills the managed server process (PID {}). The dashboard loses its connection until a server is running again.",
        status["pid"]
    );

    html! {
        section.card.card-body.stack {
            h3 { "Server management" }
            div.server-status.on[running] {
                span.dot {}
                @if running {
                    "Managed server running "
                    span.muted.mono { "PID " (status["pid"]) " · port " (status["port"]) }
                } @else if managed {
                    "Managed server stopped"
                } @else {
                    "No server managed by this dashboard "
                    @if connected { span.muted { "(an external server is connected)" } }
                }
            }
            @if !running {
                div.row {
                    label.field {
                        span { "Port" }
                        input.input type="number" min="0" max="65535" data-bind="config.port";
                    }
                    label.field {
                        span { "Log level" }
                        select.select data-bind="config.level" {
                            @for l in ["error", "warn", "info", "debug", "trace"] { option value=(l) { (l) } }
                        }
                    }
                }
                label.field {
                    span { "Database path" }
                    input.input.mono data-bind="config.db" placeholder="db/sqlite/dev.db (default)";
                    span.hint { "The dashboard talks to its configured API URL; use the matching port (0 picks a free port)." }
                }
            }
            div.toolbar data-indicator="_server" {
                @if running {
                    button.btn.btn-danger data-attr:disabled="$_server"
                        data-on:click=(format!("confirm({}) && @post('/ui/config/server/stop')", js(&stop_msg))) {
                        span.spinner.small data-show="$_server" style="display:none" {}
                        span data-show="!$_server" { (icon("stop", 14)) }
                        " Stop server"
                    }
                } @else {
                    button.btn.btn-primary data-attr:disabled="$_server" data-on:click="@post('/ui/config/server/start')" {
                        span.spinner.small data-show="$_server" style="display:none" {}
                        span data-show="!$_server" { (icon("play", 14)) }
                        " Start server"
                    }
                }
            }
            @if !output.is_empty() {
                div.field {
                    span { "Server output" }
                    pre.code-block.server-output { (output.join("\n")) }
                }
            }
        }
    }
}

async fn test_connection(State(st): State<Arc<AppState>>) -> Ui {
    let url = version(&st).await["api_url"]
        .as_str()
        .unwrap_or("torc-server")
        .to_string();
    let r = kit::get::<Value>(&st, "/ping", &[]).await;
    Ui::new().patch(html! {
        span #config-test .ok-text[r.is_ok()] .error-text[r.is_err()] {
            @match &r {
                Ok(_) => { "Connected to " (url) }
                Err(e) => { "Connection to " (url) " failed: " (e) }
            }
        }
    })
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ConfigSig {
    config: ServerForm,
}

async fn start_server(State(st): State<Arc<AppState>>, Sig(sig): Sig<ConfigSig>) -> Ui {
    let f = sig.config;
    // A number input binds a number, but an emptied one sends "".
    let port = match &f.port {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
    .unwrap_or(0);
    if port > 65535 {
        return Ui::new().err("Port must be between 0 and 65535");
    }
    // A managed server that exited still holds its PID, and start refuses while it does: clear it.
    let status: Value = kit::json_of(crate::server_status_handler(State(st.clone())).await)
        .await
        .unwrap_or_default();
    if status["managed"] == true && status["running"] != true {
        let _ = crate::server_stop_handler(State(st.clone())).await;
    }
    let db = f.db.trim();
    let r: Result<Value, String> = kit::call(
        &st,
        crate::server_start_handler,
        json!({"port": port, "database": (!db.is_empty()).then_some(db), "log_level": f.level}),
    )
    .await;
    match r {
        Ok(v) if v["success"] == true => {
            // Give the server a moment to accept connections before the re-render pings it.
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            Ui::new()
                .ok(v["message"].as_str().unwrap_or("Server started"))
                .refresh()
        }
        Ok(v) => Ui::new().err(v["message"].as_str().unwrap_or("Failed to start server")),
        Err(e) => Ui::new().err(e),
    }
}

async fn stop_server(State(st): State<Arc<AppState>>) -> Ui {
    let v: Value = kit::json_of(crate::server_stop_handler(State(st.clone())).await)
        .await
        .unwrap_or_default();
    let msg = v["message"].as_str().unwrap_or("Failed to stop server");
    let ui = if v["success"] == true {
        Ui::new().ok(msg)
    } else {
        Ui::new().err(msg)
    };
    ui.refresh()
}
