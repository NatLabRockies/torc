//! AI Chat: the AI Chat tab and the spec editor's assistant panel (`GET /ui/chat/editor-panel`,
//! rendered into the editor's `#editor-chat` slot). Both talk to `chat_handler`, which runs the
//! model and its tool calls and streams SSE events (`text`, `tool_use`, `tool_result`,
//! `spec_edit`, `error`, `done`).
//!
//! Conversations live here, server-side, keyed by an id the page keeps in `$chat.id` (tab) or
//! `$chat.eid` (editor). A reply runs in a background task that updates the conversation, so it
//! keeps going across tab switches and auto-refresh re-renders, which just show its current state.
//! While a reply runs, the thread holds a watcher element whose `@get('/ui/chat/live')` streams
//! the thread as it changes; leaving the tab removes the watcher (and its request), and coming
//! back renders a new one.
//!
//! In the editor, the request carries `$editor.text`; each AI edit is applied to a server copy of
//! it and sent back as `$editor.text`. `$chat.editing` is true while the editor's assistant runs
//! (the editor should be read-only then, as before).

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Instant;

use axum::{
    Form, Json, Router,
    extract::{Query, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use futures::StreamExt;
use maud::{Markup, PreEscaped, html};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::watch;

use super::Ctx;
use super::kit::{self, G, Sig, Ui, icon, js};
use crate::AppState;
use tracing::info;

const DOCS: &str =
    "https://natlabrockies.github.io/torc/v0.20.4/core/monitoring/dashboard.html#ai-chat-tab";
/// Longest tool result shown.
const RESULT_LIMIT: usize = 5000;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ui/chat/editor-panel", get(editor_panel))
        .route("/ui/chat/send", post(send))
        .route("/ui/chat/stop", post(stop))
        .route("/ui/chat/clear", post(clear))
        .route("/ui/chat/live", get(live))
        .route("/ui/chat/setup", get(setup))
        .route("/ui/chat/configure", post(configure))
}

// ============== Panels ==============

/// Which conversation: the AI Chat tab's (about the selected workflow) or the spec editor's.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    Tab,
    Editor,
}

#[derive(Deserialize)]
struct PanelQuery {
    #[serde(default)]
    panel: String,
    #[serde(default)]
    provider: Option<String>,
}

impl PanelQuery {
    fn panel(&self) -> Panel {
        if self.panel == "editor" {
            Panel::Editor
        } else {
            Panel::Tab
        }
    }
}

impl Panel {
    /// Used in element ids and URLs.
    fn name(self) -> &'static str {
        match self {
            Panel::Tab => "tab",
            Panel::Editor => "editor",
        }
    }
    /// Keys in `$chat`: conversation id, composer text, busy flag, provider form open.
    fn keys(self) -> (&'static str, &'static str, &'static str, &'static str) {
        match self {
            Panel::Tab => ("id", "draft", "busy", "setup"),
            Panel::Editor => ("eid", "edraft", "editing", "esetup"),
        }
    }
    fn url(self, action: &str) -> String {
        format!("/ui/chat/{action}?panel={}", self.name())
    }
    /// Datastar expression that sends the composer text.
    fn send_expr(self) -> String {
        let (_, draft, busy, _) = self.keys();
        format!(
            "!$chat.{busy} && $chat.{draft}.trim() && (document.getElementById('chat-{p}-scroller')._up = false, @post('{}'))",
            self.url("send"),
            p = self.name()
        )
    }
}

/// `$chat`, as sent by the page.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ChatSig {
    id: String,
    eid: String,
    draft: String,
    edraft: String,
    setup: bool,
    esetup: bool,
}

impl ChatSig {
    fn conv_id(&self, p: Panel) -> &str {
        match p {
            Panel::Tab => &self.id,
            Panel::Editor => &self.eid,
        }
    }
}

/// `$chat` defaults for `data-signals__ifmissing`; fresh conversation ids each time, used only
/// when the page has none yet.
fn chat_defaults() -> String {
    let id = || uuid::Uuid::new_v4().simple().to_string();
    json!({"chat": {
        "id": id(), "eid": id(), "draft": "", "edraft": "",
        "busy": false, "editing": false, "setup": false, "esetup": false,
    }})
    .to_string()
}

/// Host the page was served from; links to it are dropped from replies.
fn host_of(headers: &HeaderMap) -> String {
    headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

// ============== Conversations ==============

enum Part {
    Text(String),
    Tool {
        id: String,
        name: String,
        input: Value,
        result: Option<String>,
        error: bool,
    },
    Note {
        text: String,
        error: bool,
    },
}

enum Msg {
    User(String),
    Assistant(Vec<Part>),
}

#[derive(Default)]
struct Conv {
    msgs: Vec<Msg>,
    /// What is sent to the model: plain-text turns, alternating user/assistant.
    history: Vec<Value>,
    streaming: bool,
    /// Text of the reply in progress.
    reply: String,
    task: Option<tokio::task::AbortHandle>,
    /// Editor conversations: the spec as the assistant has edited it.
    spec: Option<String>,
    /// `spec` changed since it was last sent to the page.
    spec_unsent: bool,
    /// Host the page was served from; links to it are dropped from replies.
    host: String,
}

impl Conv {
    fn parts(&mut self) -> Option<&mut Vec<Part>> {
        match self.msgs.last_mut() {
            Some(Msg::Assistant(p)) => Some(p),
            _ => None,
        }
    }
    fn note(&mut self, text: String, error: bool) {
        if let Some(p) = self.parts() {
            p.push(Part::Note { text, error });
        }
    }
}

struct Entry {
    conv: Mutex<Conv>,
    /// Bumped on every change; live views wait on it.
    tx: watch::Sender<u64>,
    used: Mutex<Instant>,
}

impl Entry {
    fn bump(&self) {
        self.tx.send_modify(|v| *v += 1);
    }
}

static CONVS: LazyLock<Mutex<HashMap<String, Arc<Entry>>>> = LazyLock::new(Default::default);

/// The conversation with this id, created if new.
fn entry(id: &str) -> Arc<Entry> {
    let mut all = CONVS.lock().unwrap();
    // ponytail: keeps the 64 most recently used conversations (they vanish on reload anyway).
    if all.len() >= 64 && !all.contains_key(id) {
        let oldest = all
            .iter()
            .filter(|(_, e)| !e.conv.lock().unwrap().streaming)
            .min_by_key(|(_, e)| *e.used.lock().unwrap())
            .map(|(k, _)| k.clone());
        if let Some(k) = oldest {
            all.remove(&k);
        }
    }
    let e = all
        .entry(id.to_string())
        .or_insert_with(|| {
            Arc::new(Entry {
                conv: Mutex::default(),
                tx: watch::Sender::new(0),
                used: Mutex::new(Instant::now()),
            })
        })
        .clone();
    *e.used.lock().unwrap() = Instant::now();
    e
}

/// Ids are generated by the page; refuse anything else so the map can't be stuffed.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

async fn available(st: &Arc<AppState>) -> (bool, Option<String>) {
    let v: Value = kit::json_of(crate::chat_status_handler(State(st.clone())).await)
        .await
        .unwrap_or_default();
    (
        v.get("available").and_then(Value::as_bool).unwrap_or(false),
        v.get("reason").and_then(Value::as_str).map(String::from),
    )
}

// ============== Views ==============

const TAB_EXAMPLES: [&str; 4] = [
    "Show me the failed jobs",
    "Why did my last workflow fail?",
    "Help me create a workflow",
    "Summarize resource usage",
];

const EDITOR_EXAMPLES: [&str; 4] = [
    "Write a workflow that runs 10 simulations, then aggregates the results",
    "Add a Slurm scheduler and schedule nodes at workflow start",
    "Explain this spec",
    "Validate this spec and fix any problems",
];

pub async fn view(st: &Arc<AppState>, cx: &Ctx) -> Markup {
    let c: ChatSig = kit::ns(&cx.sig, "chat");
    let (ok, reason) = available(st).await;
    let show_setup = !ok || c.setup;
    let conv = valid_id(&c.id).then(|| entry(&c.id));
    let streaming = conv
        .as_ref()
        .is_some_and(|e| e.conv.lock().unwrap().streaming);

    html! {
        div.page.chat data-signals__ifmissing=(chat_defaults()) {
            span hidden data-signals=(json!({"chat": {"busy": streaming}}).to_string()) {}
            div.page-header {
                h2 { "AI Assistant" }
                @if let Some(wf) = cx.g.wf {
                    span.ctx title="The assistant is told which workflow is selected" {
                        "Context: workflow " span.mono { "#" (wf) }
                    }
                }
                div.toolbar {
                    @if ok && !c.setup {
                        button.btn.btn-sm.btn-ghost data-on:click="$chat.setup = true; @get('/ui/tab')" { "Provider" }
                    }
                    a.btn.btn-sm.btn-ghost href=(DOCS) target="_blank" rel="noopener" title="AI Chat documentation" {
                        (icon("external", 13)) " Docs"
                    }
                    button.btn.btn-sm data-attr:disabled="$chat.busy" data-on:click=(format!("@post('{}')", Panel::Tab.url("clear"))) {
                        (icon("trash", 13)) " Clear"
                    }
                }
            }
            @if show_setup {
                div.chat-setup-wrap { (setup_form(Panel::Tab, &setup_provider(None), reason.as_deref(), ok, None)) }
            } @else {
                (conversation(Panel::Tab, conv.as_deref()))
            }
        }
    }
}

/// `GET /ui/chat/editor-panel`: the spec assistant, into the editor's `#editor-chat`.
async fn editor_panel(State(st): State<Arc<AppState>>, Sig(sig): Sig<Value>) -> Ui {
    let c: ChatSig = kit::ns(&sig, "chat");
    render_editor_panel(&st, &c.eid, c.esetup, Ui::new()).await
}

/// The editor panel for conversation `eid`, added to `ui`.
async fn render_editor_panel(st: &Arc<AppState>, eid: &str, setup_open: bool, mut ui: Ui) -> Ui {
    let (ok, reason) = available(st).await;
    let conv = valid_id(eid).then(|| entry(eid));
    let mut streaming = false;
    if let Some(e) = &conv {
        let mut cv = e.conv.lock().unwrap();
        streaming = cv.streaming;
        // Edits made while the editor was closed.
        if cv.spec_unsent
            && let Some(s) = &cv.spec
        {
            ui = ui.signals(json!({"editor": {"text": s}}));
            cv.spec_unsent = false;
        }
    }
    let p = Panel::Editor;
    let panel = html! {
        section.chat-panel data-signals__ifmissing=(chat_defaults()) {
            header {
                strong { "Assistant" }
                span.faint { "reads and edits this spec" }
                span.grow {}
                @if ok && !setup_open {
                    button.btn.btn-sm.btn-ghost title="AI provider (shared with the AI Chat tab)"
                        data-attr:disabled="$chat.editing"
                        data-on:click=(format!("$chat.esetup = true; @get('{}')", "/ui/chat/editor-panel")) { "Provider" }
                }
                button.btn.btn-sm.btn-ghost.btn-icon title="Clear conversation" aria-label="Clear conversation"
                    data-attr:disabled="$chat.editing"
                    data-on:click=(format!("@post('{}')", p.url("clear"))) { (icon("trash", 13)) }
            }
            @if !ok || setup_open {
                div.chat-setup-wrap { (setup_form(p, &setup_provider(None), reason.as_deref(), ok, None)) }
            } @else {
                (conversation(p, conv.as_deref()))
            }
        }
    };
    ui.signals(json!({"chat": {"editing": streaming}}))
        .inner("#editor-chat", panel)
}

/// Thread and composer.
fn conversation(p: Panel, conv: Option<&Entry>) -> Markup {
    let (_, draft, busy, _) = p.keys();
    let n = p.name();
    let placeholder = match p {
        Panel::Tab => "Ask about your workflows…  (Enter to send, Shift+Enter for a new line)",
        Panel::Editor => "Ask for a change…",
    };
    html! {
        // Follow new content only while the user is at the bottom.
        div.chat-scroller id=(format!("chat-{n}-scroller"))
            data-on:scroll="el._up = el.scrollHeight - el.scrollTop - el.clientHeight >= 40"
            data-init="new ResizeObserver(() => el._up || (el.scrollTop = el.scrollHeight)).observe(el.firstElementChild)" {
            @match conv {
                Some(e) => (thread(p, &e.conv.lock().unwrap())),
                None => (thread(p, &Conv::default())),
            }
        }
        div.composer {
            textarea rows="1" placeholder=(placeholder) aria-label="Chat message"
                data-bind=(format!("chat.{draft}"))
                data-effect=(format!("$chat.{draft}; el.style.height = 'auto'; el.style.height = Math.min(el.scrollHeight, 200) + 'px'"))
                data-on:keydown=(format!("evt.key === 'Enter' && !evt.shiftKey && !evt.isComposing && (evt.preventDefault(), {})", p.send_expr())) {}
            button.btn.btn-danger data-show=(format!("$chat.{busy}")) style="display:none"
                data-on:click=(format!("@post('{}')", p.url("stop"))) {
                (icon("stop", 14)) " Stop"
            }
            button.btn.btn-primary data-show=(format!("!$chat.{busy}"))
                data-attr:disabled=(format!("!$chat.{draft}.trim()"))
                data-on:click=(p.send_expr()) {
                (icon("send", 14)) " Send"
            }
        }
    }
}

fn thread(p: Panel, cv: &Conv) -> Markup {
    let n = p.name();
    let (_, draft, _, _) = p.keys();
    let (title, subtitle, examples) = match p {
        Panel::Tab => (
            "Torc AI Assistant",
            "Ask about your workflows, job status, logs, and more.",
            TAB_EXAMPLES,
        ),
        Panel::Editor => (
            "Spec assistant",
            "Describe the workflow you want or the change to make. Edits appear in the editor as they happen.",
            EDITOR_EXAMPLES,
        ),
    };
    let last = cv.msgs.len().saturating_sub(1);
    html! {
        div.chat-thread id=(format!("chat-{n}-thread")) {
            @if cv.msgs.is_empty() {
                div.welcome {
                    div.chat-icon { (icon("chat", 24)) }
                    h3 { (title) }
                    p.muted { (subtitle) }
                    div.examples {
                        @for ex in examples {
                            button.btn.btn-sm data-on:click=(format!("$chat.{draft} = {}; {}", js(ex), p.send_expr())) { (ex) }
                        }
                    }
                }
            }
            @for (i, m) in cv.msgs.iter().enumerate() {
                @match m {
                    Msg::User(t) => div.msg.user { div.bubble { (t) } },
                    Msg::Assistant(parts) => {
                        @let live = cv.streaming && i == last;
                        div.msg.assistant {
                            div.avatar { (icon("chat", 14)) }
                            div.bubble {
                                @for part in parts {
                                    div.part { (render_part(part, live, &cv.host)) }
                                }
                                @if live {
                                    span.dots aria-label="Working" { span {} span {} span {} }
                                }
                            }
                        }
                    }
                }
            }
            @if cv.streaming {
                div hidden id=(format!("chat-{n}-live"))
                    data-init=(format!("@get('{}', {{requestCancellation: 'cleanup'}})", p.url("live"))) {}
            }
        }
    }
}

fn render_part(part: &Part, live: bool, host: &str) -> Markup {
    match part {
        Part::Text(t) => html! { div.md { (PreEscaped(markdown(t, host))) } },
        Part::Note { text, error } => html! { div.error-text[*error].muted[!*error] { (text) } },
        Part::Tool {
            name,
            input,
            result,
            error,
            ..
        } => html! {
            // Keep the user's open/closed choice across re-renders.
            details.tool.error[*error] data-preserve-attr="open" {
                summary {
                    span.chev { (icon("chevronRight", 12)) }
                    span.mono.name { (name) }
                    @match result {
                        None => { @if live { span.spinner.small {} } }
                        Some(_) if *error => span.state.err { "failed" },
                        Some(_) => span.state.ok { (icon("check", 12)) },
                    }
                }
                div.body {
                    div.label { "Input" }
                    pre.code-block { (serde_json::to_string_pretty(input).unwrap_or_default()) }
                    @if let Some(r) = result {
                        div.label { "Result" }
                        pre.code-block.err-block[*error] { (if r.is_empty() { "(empty)" } else { r }) }
                    }
                }
            }
        },
    }
}

// ============== Sending ==============

/// `POST /ui/chat/send?panel=`: start a reply to the composer text.
async fn send(
    State(st): State<Arc<AppState>>,
    Query(q): Query<PanelQuery>,
    headers: HeaderMap,
    Sig(sig): Sig<Value>,
) -> Ui {
    let p = q.panel();
    let c: ChatSig = kit::ns(&sig, "chat");
    let id = c.conv_id(p);
    let text = match p {
        Panel::Tab => c.draft.trim(),
        Panel::Editor => c.edraft.trim(),
    }
    .to_string();
    if !valid_id(id) {
        return Ui::new().err("This chat has no conversation id; reload the page.");
    }
    if text.is_empty() {
        return Ui::new();
    }
    let e = entry(id);
    let (_, draft, busy, _) = p.keys();
    {
        let mut cv = e.conv.lock().unwrap();
        if cv.streaming {
            return Ui::new();
        }
        cv.msgs.push(Msg::User(text.clone()));
        cv.history.push(json!({"role": "user", "content": text}));
        cv.msgs.push(Msg::Assistant(Vec::new()));
        cv.streaming = true;
        cv.reply.clear();
        let g: G = serde_json::from_value(sig.clone()).unwrap_or_default();
        cv.spec = (p == Panel::Editor).then(|| {
            sig.pointer("/editor/text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        });
        cv.spec_unsent = false;
        cv.host = host_of(&headers);
        let body = json!({
            "messages": cv.history,
            "workflow_id": if p == Panel::Tab { g.wf } else { None },
            "spec": cv.spec,
        });
        cv.task = Some(tokio::spawn(run(st, e.clone(), headers.clone(), body)).abort_handle());
    }
    e.bump();
    let cv = e.conv.lock().unwrap();
    Ui::new()
        .signals(json!({"chat": {draft: "", busy: true}}))
        .patch(thread(p, &cv))
}

/// Run `chat_handler` and fold its events into the conversation.
async fn run(st: Arc<AppState>, e: Arc<Entry>, headers: HeaderMap, body: Value) {
    let resp = match serde_json::from_value(body) {
        Ok(req) => crate::chat_handler(State(st), headers, Json(req))
            .await
            .into_response(),
        Err(err) => {
            e.conv.lock().unwrap().note(format!("Error: {err}"), true);
            return finish(&e, false);
        }
    };
    let status = resp.status();
    if !status.is_success() {
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .map(|b| String::from_utf8_lossy(&b).trim().to_string())
            .unwrap_or_default();
        let msg = if body.is_empty() {
            format!("HTTP {status}")
        } else {
            body
        };
        e.conv.lock().unwrap().note(format!("Error: {msg}"), true);
        return finish(&e, false);
    }
    let mut body = resp.into_body().into_data_stream();
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = body.next().await {
        match chunk {
            Ok(b) => buf.extend_from_slice(&b),
            Err(err) => {
                e.conv.lock().unwrap().note(format!("Error: {err}"), true);
                break;
            }
        }
        while let Some(i) = buf.windows(2).position(|w| w == b"\n\n") {
            let block: Vec<u8> = buf.drain(..i + 2).collect();
            if let Some((kind, data)) = parse_event(&String::from_utf8_lossy(&block)) {
                apply_event(&mut e.conv.lock().unwrap(), &kind, &data);
                e.bump();
            }
        }
    }
    finish(&e, false);
}

/// One SSE block → (event type, data). Comments (keep-alives) give None.
fn parse_event(block: &str) -> Option<(String, String)> {
    let mut kind = "message".to_string();
    let mut data: Vec<&str> = Vec::new();
    for line in block.lines() {
        if let Some(t) = line.strip_prefix("event:") {
            kind = t.trim().to_string();
        } else if let Some(d) = line.strip_prefix("data:") {
            data.push(d.strip_prefix(' ').unwrap_or(d));
        }
    }
    (!data.is_empty()).then(|| (kind, data.join("\n")))
}

fn apply_event(cv: &mut Conv, kind: &str, data: &str) {
    match kind {
        "text" | "message" => {
            let t: String = serde_json::from_str(data).unwrap_or_else(|_| data.to_string());
            cv.reply.push_str(&t);
            if let Some(parts) = cv.parts() {
                match parts.last_mut() {
                    Some(Part::Text(s)) => s.push_str(&t),
                    _ => parts.push(Part::Text(t)),
                }
            }
        }
        "tool_use" => {
            let v: Value = serde_json::from_str(data).unwrap_or_default();
            let s = |k: &str| v[k].as_str().unwrap_or_default().to_string();
            let (id, name, input) = (s("id"), s("name"), v["input"].clone());
            if let Some(parts) = cv.parts() {
                parts.push(Part::Tool {
                    id,
                    name,
                    input,
                    result: None,
                    error: false,
                });
            }
        }
        "tool_result" => {
            let v: Value = serde_json::from_str(data).unwrap_or_default();
            let mut r = v["result"].as_str().unwrap_or_default().to_string();
            if r.len() > RESULT_LIMIT {
                let mut cut = RESULT_LIMIT;
                while !r.is_char_boundary(cut) {
                    cut -= 1;
                }
                r.truncate(cut);
                r.push_str("\n… (truncated)");
            }
            let is_error = v["is_error"].as_bool().unwrap_or(false);
            if let Some(parts) = cv.parts()
                && let Some(Part::Tool { result, error, .. }) = parts
                    .iter_mut()
                    .find(|p| matches!(p, Part::Tool { id, .. } if v["id"] == id.as_str()))
            {
                *result = Some(r);
                *error = is_error;
            }
        }
        "spec_edit" => {
            let edit: Value = serde_json::from_str(data).unwrap_or_default();
            let tool = if edit.get("content").is_some() {
                "replace_spec"
            } else {
                "edit_spec"
            };
            let ok = cv
                .spec
                .as_mut()
                .is_some_and(|buf| crate::apply_spec_edit(buf, tool, &edit).is_ok());
            if ok {
                cv.spec_unsent = true;
            } else {
                cv.note("Could not apply an edit: the spec changed.".into(), true);
            }
        }
        "error" => cv.note(format!("Error: {data}"), true),
        _ => {}
    }
}

/// End the reply in progress (finished, failed, or stopped).
fn finish(e: &Entry, stopped: bool) {
    let mut cv = e.conv.lock().unwrap();
    if !cv.streaming {
        return;
    }
    if stopped {
        cv.note("Stopped.".into(), false);
    }
    if cv.reply.is_empty() {
        // Keep user/assistant alternation for the next request.
        cv.history.pop();
    } else {
        let reply = std::mem::take(&mut cv.reply);
        cv.history
            .push(json!({"role": "assistant", "content": reply}));
    }
    cv.streaming = false;
    cv.task = None;
    drop(cv);
    e.bump();
}

async fn stop(Query(q): Query<PanelQuery>, Sig(sig): Sig<Value>) -> Ui {
    let p = q.panel();
    let c: ChatSig = kit::ns(&sig, "chat");
    if !valid_id(c.conv_id(p)) {
        return Ui::new();
    }
    let e = entry(c.conv_id(p));
    if let Some(t) = e.conv.lock().unwrap().task.take() {
        t.abort();
    }
    finish(&e, true);
    let cv = e.conv.lock().unwrap();
    Ui::new()
        .signals(json!({"chat": {p.keys().2: false}}))
        .patch(thread(p, &cv))
}

async fn clear(Query(q): Query<PanelQuery>, Sig(sig): Sig<Value>) -> Ui {
    let p = q.panel();
    let c: ChatSig = kit::ns(&sig, "chat");
    if !valid_id(c.conv_id(p)) {
        return Ui::new();
    }
    let e = entry(c.conv_id(p));
    let mut cv = e.conv.lock().unwrap();
    if cv.streaming {
        return Ui::new();
    }
    *cv = Conv::default();
    Ui::new().patch(thread(p, &cv))
}

/// `GET /ui/chat/live?panel=`: stream the thread while a reply runs.
async fn live(Query(q): Query<PanelQuery>, Sig(sig): Sig<Value>) -> Response {
    let p = q.panel();
    let c: ChatSig = kit::ns(&sig, "chat");
    if !valid_id(c.conv_id(p)) {
        return Ui::new().into_response();
    }
    let e = entry(c.conv_id(p));
    let mut rx = e.tx.subscribe();
    kit::stream(async_stream::stream! {
        loop {
            let (ui, done) = {
                let mut cv = e.conv.lock().unwrap();
                // Signals first: the last patch removes the watcher, which ends this request.
                let mut ui = Ui::new().signals(json!({"chat": {p.keys().2: cv.streaming}}));
                if cv.spec_unsent && let Some(s) = &cv.spec {
                    ui = ui.signals(json!({"editor": {"text": s}}));
                    cv.spec_unsent = false;
                }
                (ui.patch(thread(p, &cv)), !cv.streaming)
            };
            yield ui;
            if done || rx.changed().await.is_err() {
                break;
            }
        }
    })
}

// ============== Provider setup ==============

const PROVIDERS: [(&str, &str); 6] = [
    ("anthropic", "Anthropic (Direct API)"),
    ("openai", "OpenAI"),
    ("foundry", "Azure AI Foundry"),
    ("ollama", "Ollama (Local)"),
    ("github", "GitHub Models"),
    ("custom", "Custom via gateway (LiteLLM, proxy)"),
];

fn key_placeholder(p: &str) -> &'static str {
    match p {
        "anthropic" => "sk-ant-…",
        "openai" => "sk-… (OpenAI API key)",
        "foundry" => "Foundry API key",
        "github" => "ghp_… (GitHub token)",
        "custom" => "API key or token",
        _ => "",
    }
}

fn url_placeholder(p: &str) -> Option<&'static str> {
    match p {
        "openai" => Some("https://api.openai.com/v1"),
        "ollama" => Some("http://localhost:11434/v1"),
        "github" => Some("https://models.inference.ai.azure.com"),
        "custom" => Some("https://litellm.example.org"),
        _ => None,
    }
}

fn model_placeholder(p: &str) -> &'static str {
    match p {
        "anthropic" | "foundry" => "claude-opus-5-5",
        "custom" => "Model name on the gateway",
        "openai" | "github" => "gpt-4o",
        "ollama" => "llama3.2",
        _ => "",
    }
}

/// Everything but the key, remembered per provider for the form.
#[derive(Clone, Default)]
struct Fields {
    base_url: String,
    model: String,
    foundry_resource: String,
    auth_header: String,
}

/// The last provider connected and each provider's fields (torc-dash memory only; the key is
/// never kept here).
static SETUP: LazyLock<Mutex<(String, HashMap<String, Fields>)>> = LazyLock::new(Default::default);

/// The provider to show: the one asked for, else the last one connected, else Anthropic.
fn setup_provider(asked: Option<&str>) -> String {
    let p = asked
        .map(String::from)
        .unwrap_or_else(|| SETUP.lock().unwrap().0.clone());
    if PROVIDERS.iter().any(|(id, _)| *id == p) {
        p
    } else {
        "anthropic".into()
    }
}

fn setup_form(
    p: Panel,
    provider: &str,
    reason: Option<&str>,
    available: bool,
    error: Option<&str>,
) -> Markup {
    let n = p.name();
    let f = SETUP
        .lock()
        .unwrap()
        .1
        .get(provider)
        .cloned()
        .unwrap_or_default();
    let auth = if f.auth_header.is_empty() {
        "x-api-key"
    } else {
        &f.auth_header
    };
    // Ids per provider, so switching providers doesn't carry typed values across.
    let fid = |k: &str| format!("chat-{n}-{provider}-{k}");
    let cancel = match p {
        Panel::Tab => "$chat.setup = false; @get('/ui/tab')".to_string(),
        Panel::Editor => "$chat.esetup = false; @get('/ui/chat/editor-panel')".to_string(),
    };
    html! {
        form.card.chat-setup id=(format!("chat-{n}-setup")) autocomplete="off"
            data-indicator="_chatConnecting"
            data-on:submit=(format!("@post('{}', {{contentType: 'form'}})", p.url("configure"))) {
            div.chat-icon { (icon("chat", 22)) }
            h3 { "Connect an AI provider" }
            // The conversation to re-render after connecting (form posts carry no signals).
            input type="hidden" name="conv" data-attr:value=(format!("$chat.{}", p.keys().0));
            @if let Some(r) = reason { p.muted { (r) } }
            label.field {
                span { "Provider" }
                select.select name="provider"
                    data-on:change=(format!("@get('{}&provider=' + el.value)", p.url("setup"))) {
                    @for (id, label) in PROVIDERS {
                        option value=(id) selected[id == provider] { (label) }
                    }
                }
            }
            @if provider == "foundry" {
                label.field {
                    span { "Foundry resource name" }
                    input.input id=(fid("res")) name="foundry_resource" value=(f.foundry_resource) placeholder="my-resource";
                }
            }
            @if provider == "custom" {
                label.field {
                    span { "Base URL " span.hint { "(a gateway with the Anthropic Messages API, as LiteLLM provides for any model)" } }
                    input.input id=(fid("url")) name="base_url" value=(f.base_url) placeholder=[url_placeholder(provider)];
                }
            }
            @if provider != "ollama" {
                label.field {
                    span { (if provider == "github" { "GitHub token" } else { "API key" }) }
                    input.input id=(fid("key")) type="password" name="api_key" autocomplete="off" autofocus
                        placeholder=(key_placeholder(provider));
                }
            }
            label.field {
                span { "Model " span.hint { "(optional)" } }
                input.input id=(fid("model")) name="model" value=(f.model) placeholder=(model_placeholder(provider));
            }
            @if provider != "custom" && let Some(ph) = url_placeholder(provider) {
                label.field {
                    span {
                        (match provider { "openai" => "API URL", "ollama" => "Ollama URL", _ => "GitHub Models URL" })
                        " " span.hint { "(optional" (if provider == "openai" { ", for compatible services" } else { "" }) ")" }
                    }
                    input.input id=(fid("url")) name="base_url" value=(f.base_url) placeholder=(ph);
                }
            }
            @if provider == "custom" {
                label.field {
                    span { "Auth header" }
                    input.input id=(fid("auth")) name="auth_header" value=(auth) list=(format!("chat-{n}-auth-headers")) placeholder="x-api-key";
                    datalist id=(format!("chat-{n}-auth-headers")) {
                        option value="x-api-key" { "API key, like ANTHROPIC_API_KEY" }
                        option value="Authorization" { "Bearer token, like ANTHROPIC_AUTH_TOKEN" }
                    }
                }
            }
            @if let Some(e) = error { p.error-text { (e) } }
            div.actions {
                @if available {
                    button.btn type="button" data-on:click=(cancel) { "Cancel" }
                }
                button.btn.btn-primary type="submit" data-attr:disabled="$_chatConnecting" {
                    span.spinner.small data-show="$_chatConnecting" style="display:none" {}
                    " Connect"
                }
            }
            p.faint.note {
                "Credentials are kept in memory for this session only. For persistent configuration, use CLI flags or environment variables. "
                a href=(DOCS) target="_blank" rel="noopener" { "Documentation" }
            }
        }
    }
}

/// `GET /ui/chat/setup?panel=&provider=`: the form for another provider.
async fn setup(State(st): State<Arc<AppState>>, Query(q): Query<PanelQuery>) -> Ui {
    let (ok, reason) = available(&st).await;
    let provider = setup_provider(q.provider.as_deref());
    Ui::new().patch(setup_form(
        q.panel(),
        &provider,
        reason.as_deref(),
        ok,
        None,
    ))
}

async fn setup_error(st: &Arc<AppState>, p: Panel, provider: &str, msg: &str) -> Ui {
    let (ok, reason) = available(st).await;
    Ui::new().patch(setup_form(p, provider, reason.as_deref(), ok, Some(msg)))
}

/// `POST /ui/chat/configure?panel=` (form fields): connect a provider for all conversations.
async fn configure(
    State(st): State<Arc<AppState>>,
    Query(q): Query<PanelQuery>,
    headers: HeaderMap,
    Form(f): Form<HashMap<String, String>>,
) -> Ui {
    let p = q.panel();
    let get = |k: &str| f.get(k).map(|s| s.trim()).unwrap_or_default().to_string();
    let provider = setup_provider(Some(&get("provider")));
    let fields = Fields {
        base_url: get("base_url"),
        model: get("model"),
        foundry_resource: get("foundry_resource"),
        auth_header: get("auth_header"),
    };
    let key = get("api_key");
    let invalid = if provider != "ollama" && key.is_empty() {
        Some(format!(
            "Please enter {}.",
            if provider == "github" {
                "a GitHub token"
            } else {
                "an API key"
            }
        ))
    } else if provider == "foundry" && fields.foundry_resource.is_empty() {
        Some("Please enter a Foundry resource name.".into())
    } else if provider == "custom" && fields.base_url.is_empty() {
        Some("Please enter a base URL.".into())
    } else {
        None
    };
    if let Some(msg) = invalid {
        return setup_error(&st, p, &provider, &msg).await;
    }

    let mut body = json!({
        "provider": provider,
        "api_key": if provider == "ollama" { "ollama" } else { key.as_str() },
    });
    if !fields.model.is_empty() {
        body["model"] = json!(fields.model);
    }
    if provider == "foundry" {
        body["foundry_resource"] = json!(fields.foundry_resource);
    }
    if !fields.base_url.is_empty() && url_placeholder(&provider).is_some() {
        body["base_url"] = json!(fields.base_url);
    }
    if provider == "custom" {
        body["auth_header"] = json!(if fields.auth_header.is_empty() {
            "x-api-key"
        } else {
            &fields.auth_header
        });
    }
    let req = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => return setup_error(&st, p, &provider, &e.to_string()).await,
    };
    let resp = crate::configure_chat_handler(State(st.clone()), headers.clone(), Json(req))
        .await
        .into_response();
    if !resp.status().is_success() {
        let status = resp.status();
        let text = axum::body::to_bytes(resp.into_body(), 1 << 16)
            .await
            .map(|b| String::from_utf8_lossy(&b).trim().to_string())
            .unwrap_or_default();
        let msg = if text.is_empty() {
            format!("Failed to configure provider (HTTP {status})")
        } else {
            text
        };
        return setup_error(&st, p, &provider, &msg).await;
    }
    {
        let mut s = SETUP.lock().unwrap();
        s.0 = provider.clone();
        s.1.insert(provider, fields);
    }
    info!("AI Chat provider configured from the dashboard");
    // The provider is torc-dash-wide: close both forms.
    let ui = Ui::new().signals(json!({"chat": {"setup": false, "esetup": false}}));
    match p {
        Panel::Tab => ui.refresh(),
        Panel::Editor => render_editor_panel(&st, &get("conv"), false, ui).await,
    }
}

// ============== Markdown ==============

/// Render assistant Markdown (untrusted) to HTML. Raw HTML is dropped. Nothing that fetches is
/// emitted (images become links labeled with their alt text), and links are kept only to
/// http(s)/mailto URLs on another host, so a reply can't make or link a dashboard request.
fn markdown(text: &str, host: &str) -> String {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut out: Vec<Event> = Vec::new();
    // Per open link/image: whether an <a> was emitted for it.
    let mut links: Vec<bool> = Vec::new();
    // Image whose alt text hasn't appeared yet: its URL, to label the link with.
    let mut alt_missing: Option<String> = None;
    for ev in Parser::new_ext(text, opts) {
        match ev {
            Event::Html(_) | Event::InlineHtml(_) => {}
            Event::SoftBreak => out.push(Event::HardBreak),
            Event::Start(Tag::Link { dest_url, .. }) => {
                links.push(open_link(&mut out, &dest_url, host));
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                links.push(open_link(&mut out, &dest_url, host));
                alt_missing = Some(dest_url.to_string());
            }
            Event::End(TagEnd::Link) => {
                if links.pop() == Some(true) {
                    out.push(Event::InlineHtml("</a>".into()));
                }
            }
            Event::End(TagEnd::Image) => {
                if let Some(u) = alt_missing.take() {
                    out.push(Event::Text(u.into()));
                }
                if links.pop() == Some(true) {
                    out.push(Event::InlineHtml("</a>".into()));
                }
            }
            Event::Start(Tag::CodeBlock(_)) => {
                out.push(Event::Html("<pre class=\"code-block\"><code>".into()))
            }
            Event::End(TagEnd::CodeBlock) => out.push(Event::Html("</code></pre>\n".into())),
            Event::Text(t) => {
                alt_missing = None;
                out.push(Event::Text(t));
            }
            ev => out.push(ev),
        }
    }
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, out.into_iter());
    html
}

/// Push `<a …>` for a safe URL; returns whether it did.
fn open_link(out: &mut Vec<pulldown_cmark::Event>, url: &str, host: &str) -> bool {
    let ok = safe_href(url, host);
    if ok {
        out.push(pulldown_cmark::Event::InlineHtml(
            format!(
                "<a href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer\">",
                escape_attr(url)
            )
            .into(),
        ));
    }
    ok
}

fn safe_href(url: &str, host: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    if u.starts_with("mailto:") {
        return true;
    }
    let Some(rest) = u
        .strip_prefix("https://")
        .or_else(|| u.strip_prefix("http://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let target = authority.rsplit('@').next().unwrap_or_default();
    !target.is_empty() && !target.eq_ignore_ascii_case(host)
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_is_sanitized() {
        let h = markdown(
            "Hi <script>alert(1)</script> **b**\n\n<img src=x onerror=alert(1)>\n\n[x](javascript:alert(1)) [ok](https://example.org/a?b=\"c\") [self](http://localhost:8106/ui/x) ![alt](https://example.org/i.png) ![](https://example.org/j.png)\n\n```yaml\nname: <b>\n```",
            "localhost:8106",
        );
        assert!(!h.contains("<script"), "{h}");
        assert!(!h.contains("<img"), "{h}");
        assert!(!h.contains("onerror"), "{h}");
        assert!(!h.contains("javascript:"), "{h}");
        assert!(!h.contains("localhost:8106"), "{h}");
        assert!(h.contains("<strong>b</strong>"), "{h}");
        assert!(h.contains(
            "<a href=\"https://example.org/a?b=&quot;c&quot;\" target=\"_blank\" rel=\"noopener noreferrer\">ok</a>"
        ), "{h}");
        assert!(h.contains(">alt</a>"), "{h}");
        assert!(h.contains(">https://example.org/j.png</a>"), "{h}");
        assert!(
            h.contains("<pre class=\"code-block\"><code>name: &lt;b&gt;\n</code></pre>"),
            "{h}"
        );
    }

    #[test]
    fn sse_events_fold_into_the_conversation() {
        assert_eq!(
            parse_event("event: text\ndata: \"hi\""),
            Some(("text".into(), "\"hi\"".into()))
        );
        assert_eq!(parse_event(":keep-alive"), None);
        let mut cv = Conv {
            msgs: vec![Msg::User("q".into()), Msg::Assistant(Vec::new())],
            spec: Some("x: 1\n".into()),
            ..Default::default()
        };
        apply_event(&mut cv, "text", "\"Hello\"");
        apply_event(
            &mut cv,
            "tool_use",
            r#"{"id":"t1","name":"get_workflow","input":{}}"#,
        );
        apply_event(
            &mut cv,
            "tool_result",
            r#"{"id":"t1","result":"ok","is_error":false}"#,
        );
        apply_event(&mut cv, "spec_edit", r#"{"old_str":"1","new_str":"2"}"#);
        assert_eq!(cv.reply, "Hello");
        assert_eq!(cv.spec.as_deref(), Some("x: 2\n"));
        assert!(cv.spec_unsent);
        let Some(Msg::Assistant(parts)) = cv.msgs.last() else {
            panic!()
        };
        assert!(matches!(&parts[1], Part::Tool { result: Some(r), .. } if r == "ok"));
    }
}
