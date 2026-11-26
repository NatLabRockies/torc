//! Viewing files: the file viewer modal (`GET /ui/file`), the stdout/stderr log pane used by the
//! job details and Slurm logs modals, find-in-text, and JSON/YAML syntax highlighting (the `.hl`
//! token classes in app.css).

use std::sync::{Arc, LazyLock};

use axum::{
    Json,
    extract::{Path, Query, State},
};
use maud::{Markup, PreEscaped, html};
use regex::Regex;
use serde::Deserialize;
use serde_json::{Value, json};

use super::super::kit::{self, G, Sig, Ui, icon, js, modal, seg_tabs};
use super::DetailsSig;
use crate::AppState;

// ============== Highlighting ==============

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

static JSON_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"("(?:\\.|[^"\\])*")(\s*:)?|\b(true|false|null)\b|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)|([{}\[\]])"#,
    )
    .unwrap()
});

/// Escape and wrap JSON tokens in spans (.k key, .s string, .n number, .b bool/null, .p punct).
pub fn highlight_json(json: &str) -> String {
    let mut out = String::with_capacity(json.len() * 2);
    let mut last = 0;
    for c in JSON_TOKEN.captures_iter(json) {
        let m = c.get(0).unwrap();
        out.push_str(&escape(&json[last..m.start()]));
        if let Some(s) = c.get(1) {
            match c.get(2) {
                Some(colon) => out.push_str(&format!(
                    r#"<span class="k">{}</span>{}"#,
                    escape(s.as_str()),
                    colon.as_str()
                )),
                None => out.push_str(&format!(r#"<span class="s">{}</span>"#, escape(s.as_str()))),
            }
        } else if let Some(l) = c.get(3) {
            out.push_str(&format!(r#"<span class="b">{}</span>"#, l.as_str()));
        } else if let Some(n) = c.get(4) {
            out.push_str(&format!(r#"<span class="n">{}</span>"#, n.as_str()));
        } else {
            out.push_str(&format!(r#"<span class="p">{}</span>"#, m.as_str()));
        }
        last = m.end();
    }
    out.push_str(&escape(&json[last..]));
    out
}

// YAML: per line, `indent [- ]key: value # comment`; block scalars (| or >) continue as strings.
// The regex crate has no lookahead, so the key's trailing `(?=\s|$)` consumes one character;
// the value starts at the end of group 3.
static YAML_KEY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^(\s*)((?:- +)*)((?:"(?:\\.|[^"\\])*"|'(?:[^']|'')*'|[^\s#'"\[{-][^:#]*?|-[^\s:#][^:#]*?)\s*:)(?:\s|$)"#,
    )
    .unwrap()
});
static YAML_SCALAR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?:(true|false|null|yes|no|on|off|~)|(-?(?:\d[\d_]*(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?|0x[\da-f]+)|([|>][-+\d]*)|([&*!]\S+))$",
    )
    .unwrap()
});
static YAML_DASHES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\s*)((?:- +|-$)*)").unwrap());
static YAML_BLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*[|>][-+\d]*\s*$").unwrap());

/// Byte index of a `#` comment start outside quotes.
fn comment_start(s: &str) -> Option<usize> {
    let mut quote = None;
    let mut prev_ws = true;
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        match quote {
            Some(q) => {
                if c == '\\' && q == '"' {
                    chars.next();
                } else if c == q {
                    quote = None;
                }
            }
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '#' && prev_ws => return Some(i),
            None => {}
        }
        prev_ws = c.is_whitespace();
    }
    None
}

fn yaml_value(v: &str) -> String {
    let t = v.trim();
    if t.is_empty() {
        return escape(v);
    }
    let lead = &v[..v.find(t).unwrap_or(0)];
    let tail = &v[lead.len() + t.len()..];
    let body = if t.starts_with('[') || t.starts_with('{') {
        highlight_json(t)
    } else {
        let class = match YAML_SCALAR.captures(t) {
            Some(m) if m.get(1).is_some() => "b",
            Some(m) if m.get(2).is_some() => "n",
            Some(_) => "p",
            None => "s",
        };
        format!(r#"<span class="{class}">{}</span>"#, escape(t))
    };
    format!("{}{body}{}", escape(lead), escape(tail))
}

/// Escape and wrap YAML tokens in the same span classes as [`highlight_json`] (plus .c comment).
pub fn highlight_yaml(yaml: &str) -> String {
    let mut block_indent: Option<usize> = None; // inside a | or > block scalar
    let mut lines = Vec::new();
    for line in yaml.split('\n') {
        let indent = line.len() - line.trim_start().len();
        if let Some(b) = block_indent {
            if line.trim().is_empty() || indent > b {
                lines.push(format!(r#"<span class="s">{}</span>"#, escape(line)));
                continue;
            }
            block_indent = None;
        }
        let trimmed = line.trim_end();
        if trimmed == "---" || trimmed == "..." {
            lines.push(format!(r#"<span class="p">{}</span>"#, escape(line)));
            continue;
        }
        let (code, comment) = match comment_start(line) {
            Some(h) => (
                &line[..h],
                format!(r#"<span class="c">{}</span>"#, escape(&line[h..])),
            ),
            None => (line, String::new()),
        };
        let (mut out, value) = if let Some(m) = YAML_KEY.captures(code) {
            let (ws, dashes, key) = (&m[1], &m[2], &m[3]);
            let colon = key.rfind(':').unwrap_or(key.len());
            let mut o = ws.to_string();
            if !dashes.is_empty() {
                o.push_str(&format!(r#"<span class="p">{dashes}</span>"#));
            }
            o.push_str(&format!(
                r#"<span class="k">{}</span>{}"#,
                escape(&key[..colon]),
                escape(&key[colon..])
            ));
            (o, &code[m.get(3).unwrap().end()..])
        } else {
            let d = YAML_DASHES.captures(code).unwrap();
            let mut o = d[1].to_string();
            if !d[2].is_empty() {
                o.push_str(&format!(r#"<span class="p">{}</span>"#, &d[2]));
            }
            (o, &code[d.get(0).unwrap().end()..])
        };
        if YAML_BLOCK.is_match(value) {
            block_indent = Some(indent);
        }
        out.push_str(&yaml_value(value));
        out.push_str(&comment);
        lines.push(out);
    }
    lines.join("\n")
}

/// Above this size, highlighting costs too much; show plain text.
pub const MAX_HIGHLIGHT: usize = 1_000_000;

/// Highlighted HTML for a spec or data file by extension; None: show plain.
pub fn highlight_by_name(text: &str, name: &str) -> Option<String> {
    if text.len() > MAX_HIGHLIGHT {
        return None;
    }
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_lowercase());
    match ext.as_deref() {
        Some("yaml" | "yml") => Some(highlight_yaml(text)),
        Some("json" | "json5") => Some(highlight_json(text)),
        _ => None,
    }
}

/// Syntax-highlighted JSON in a scrollable `<pre>`; large documents render as plain text.
pub fn json_view(json: &str) -> Markup {
    html! {
        @if json.len() > MAX_HIGHLIGHT { pre.json { (json) } }
        @else { pre.json.hl { (PreEscaped(highlight_json(json))) } }
    }
}

// ============== Find in text ==============

/// Most matches marked; beyond this the search stops.
const MAX_MATCHES: usize = 10_000;

/// `text` escaped, with case-insensitive matches of `q` wrapped in `<mark>`, and the match count.
fn mark_matches(text: &str, q: &str) -> (String, usize) {
    let Ok(re) = regex::RegexBuilder::new(&regex::escape(q))
        .case_insensitive(true)
        .build()
    else {
        return (escape(text), 0);
    };
    let mut out = String::with_capacity(text.len() + 64);
    let (mut last, mut n) = (0, 0);
    for m in re.find_iter(text).take(MAX_MATCHES) {
        out.push_str(&escape(&text[last..m.start()]));
        out.push_str("<mark>");
        out.push_str(&escape(m.as_str()));
        out.push_str("</mark>");
        last = m.end();
        n += 1;
    }
    out.push_str(&escape(&text[last..]));
    (out, n)
}

/// A find box bound to `$details.q` that runs `reload` as you type. Enter / Shift+Enter step
/// through the `<mark>`s inside the closest `.finder` element.
pub fn search_box(reload: &str) -> Markup {
    let step = "evt.key === 'Enter' && (evt.preventDefault(), el._m = el.closest('.finder').querySelectorAll('mark'), \
        el._m.length && (el._i = ((el._i ?? (evt.shiftKey ? 0 : -1)) + (evt.shiftKey ? el._m.length - 1 : 1)) % el._m.length, \
        el._m.forEach((m, i) => m.classList.toggle('cur', i === el._i)), el._m[el._i].scrollIntoView({block: 'center'})))";
    html! {
        div.search.find {
            (icon("search", 13))
            input.input type="search" placeholder="Find…" aria-label="Find in text"
                data-bind="details.q" data-on:keydown=(step)
                "data-on:input__debounce.300ms"=(format!("el._i = null; {reload}"));
        }
    }
}

/// Text for a `<pre>`: line endings normalized (a bare `\r` would also end an SSE line).
fn clean(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// The body of a text `<pre>`: search marks when searching, else highlighted (if `hl`), else
/// plain. Returns (html, Some(match count) when searching).
fn text_body(text: &str, q: &str, hl: Option<&str>) -> (Markup, Option<usize>) {
    let text = clean(text);
    let q = q.trim();
    if !q.is_empty() {
        let (h, n) = mark_matches(&text, q);
        return (PreEscaped(h), Some(n));
    }
    match hl.and_then(|name| highlight_by_name(&text, name)) {
        Some(h) => (PreEscaped(h), None),
        None => (html! { (text) }, None),
    }
}

fn match_count(n: Option<usize>) -> Markup {
    html! {
        @if let Some(n) = n {
            span.faint.matches { (if n >= MAX_MATCHES { format!("{n}+") } else { n.to_string() }) (if n == 1 { " match" } else { " matches" }) }
        }
    }
}

// ============== Reading files ==============

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ReadFile {
    pub success: bool,
    pub content: Option<String>,
    pub error: Option<String>,
    pub is_json: bool,
    pub exists: bool,
}

pub async fn read_file(path: &str) -> Result<ReadFile, String> {
    let req = crate::ReadFileRequest { path: path.into() };
    kit::json_of(crate::cli_read_file_handler(Json(req)).await).await
}

/// Read the first candidate path that exists (e.g. `.o`, then the combined `.log`).
async fn read_first(paths: &[String]) -> (String, Result<ReadFile, String>) {
    let mut last = Ok(ReadFile::default());
    for p in paths {
        last = read_file(p).await;
        if last.as_ref().map(|r| r.exists).unwrap_or(true) {
            return (p.clone(), last);
        }
    }
    (paths.first().cloned().unwrap_or_default(), last)
}

// ============== Log pane ==============

/// stdout/stderr viewer. Each stream takes candidate paths; the first that exists is shown (job
/// logs fall back from .o/.e to the combined .log). `reload` re-renders the pane (it reads
/// `$details.stream` and `$details.q`).
pub async fn log_pane(
    stdout: &[String],
    stderr: &[String],
    stream: &str,
    q: &str,
    reload: &str,
) -> Markup {
    let stream = if stream == "stderr" {
        "stderr"
    } else {
        "stdout"
    };
    let (path, res) = read_first(if stream == "stderr" { stderr } else { stdout }).await;
    let note = match &res {
        Err(e) => Some(format!("Error loading file: {e}")),
        Ok(r) if !r.exists => Some("File does not exist".into()),
        Ok(r) if !r.success => Some(format!(
            "Error: {}",
            r.error.as_deref().unwrap_or("unknown error")
        )),
        Ok(r) if r.content.as_deref().unwrap_or("").trim().is_empty() => Some("(empty)".into()),
        Ok(_) => None,
    };
    let content = res.ok().and_then(|r| r.content).unwrap_or_default();
    let (body, n) = text_body(&content, q, None);
    html! {
        div.logs.finder {
            div.toolbar {
                (seg_tabs("details.stream", stream, &[("stdout", "stdout", None), ("stderr", "stderr", None)], reload))
                code.path.grow title=(path) { bdi { (path) } }
                (match_count(n))
                (search_box(reload))
                button.btn.btn-sm.btn-icon title="Reload" aria-label="Reload" data-on:click=(reload) { (icon("refresh", 14)) }
            }
            @if let Some(note) = note { div.empty { (note) } }
            @else { pre.code-block.out.stderr[stream == "stderr"] { (body) } }
        }
    }
}

/// Signals that reset a viewer's find box and stream when a modal opens.
pub fn fresh_viewer() -> Value {
    json!({ "details": { "q": "", "stream": "stdout" } })
}

// ============== File viewer ==============

#[derive(Deserialize)]
pub struct FileQuery {
    path: String,
    #[serde(default)]
    title: Option<String>,
    /// Job whose details modal opened this viewer: offer a way back.
    #[serde(default)]
    back: Option<i64>,
    /// Re-render only the content (find box changed).
    #[serde(default)]
    pub part: bool,
}

/// URL of the file viewer for `path` (optionally titled, and returning to job `back`).
pub fn file_url(path: &str, title: Option<&str>, back: Option<i64>) -> String {
    let mut q = url::form_urlencoded::Serializer::new(String::new());
    q.append_pair("path", path);
    if let Some(t) = title {
        q.append_pair("title", t);
    }
    if let Some(b) = back {
        q.append_pair("back", &b.to_string());
    }
    format!("/ui/file?{}", q.finish())
}

/// `GET /ui/file?path=…&title=…`: the file viewer modal.
pub async fn file_viewer(Query(q): Query<FileQuery>, Sig(s): Sig<DetailsSig>) -> Ui {
    let reload = format!(
        "@get({})",
        js(&format!("{}&part=true", file_url(&q.path, None, None)))
    );
    let (content, search) = match read_file(&q.path).await {
        Err(e) => (
            html! { div.empty.error-text { "Error loading file: " (e) } },
            false,
        ),
        Ok(r) if !r.exists => (html! { div.empty { "File does not exist" } }, false),
        Ok(r) if !r.success => (
            html! { div.empty.error-text { "Error: " (r.error.as_deref().unwrap_or("unknown error")) } },
            false,
        ),
        Ok(r) if r.is_json => (
            html! { div.content { (json_view(&clean(r.content.as_deref().unwrap_or("")))) } },
            false,
        ),
        Ok(r) => match r.content.as_deref().filter(|c| !c.is_empty()) {
            None => (html! { div.empty { "(empty)" } }, false),
            Some(c) => {
                let q_text = if q.part { s.details.q.as_str() } else { "" };
                let (body, n) = text_body(c, q_text, Some(&q.path));
                (
                    html! {
                        (match_count(n))
                        pre.content.plain.hl { (body) }
                    },
                    true,
                )
            }
        },
    };
    let content = html! { div #fv-content.fv-content { (content) } };
    if q.part {
        return Ui::new().patch(content);
    }
    let path = &q.path;
    let body = html! {
        div.viewer.finder {
            div.pathbar {
                code.grow title=(path) { (path) }
                @if search { (search_box(&reload)) }
                button.btn.btn-sm data-on:click=(format!(
                    "navigator.clipboard.writeText({}).then(() => el.textContent = 'Copied', () => el.textContent = 'Copy failed')",
                    js(path))) { "Copy path" }
            }
            (content)
        }
    };
    let title = q
        .title
        .as_deref()
        .filter(|t| !t.is_empty())
        .unwrap_or("File contents");
    let footer = q.back.map(|id| {
        html! {
            button.btn data-on:click=(format!("@get('/ui/job/{id}')")) { "‹ Back to job" }
            button.btn data-on:click="el.closest('dialog').close()" { "Close" }
        }
    });
    Ui::new()
        .signals(fresh_viewer())
        .close_modal()
        .modal(modal(title, "xl", body, footer))
}

// ============== Slurm logs ==============

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct PartQuery {
    pub part: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct SlurmSig {
    #[serde(flatten)]
    g: G,
    #[serde(flatten)]
    d: DetailsSig,
}

/// `GET /ui/details/slurm-logs/{wf}/{scheduler_id}`: Slurm allocation stdout/stderr,
/// `{output_dir}/slurm_output_wf{wf}_sl{scheduler_id}.o/.e`.
pub async fn slurm_logs(
    State(st): State<Arc<AppState>>,
    Path((wf, sid)): Path<(i64, String)>,
    Query(q): Query<PartQuery>,
    Sig(s): Sig<SlurmSig>,
) -> Ui {
    let sub = super::super::workflows::submission_directory(&st, wf).await;
    let base = format!(
        "{}/slurm_output_wf{wf}_sl{sid}",
        kit::resolve_log_base_dir(&s.g.output_dir, sub.as_deref())
    );
    let reload = format!("@get('/ui/details/slurm-logs/{wf}/{sid}?part=true')");
    let mut d = s.d;
    if !q.part {
        d.details.q.clear();
        d.details.stream = "stdout".into();
    }
    let pane = html! {
        div #slurm-logs.grow-col {
            (log_pane(&[format!("{base}.o")], &[format!("{base}.e")], &d.details.stream, &d.details.q, &reload).await)
        }
    };
    if q.part {
        return Ui::new().patch(pane);
    }
    let body = html! {
        div.dlg-col {
            (output_dir_field(&reload))
            (pane)
        }
    };
    Ui::new().signals(fresh_viewer()).close_modal().modal(modal(
        &format!("Slurm job {sid} logs"),
        "xl",
        body,
        None,
    ))
}

/// The log output directory, bound to the global `$outputDir`; applied on change (Enter/blur).
pub fn output_dir_field(reload: &str) -> Markup {
    html! {
        label.field.outdir {
            "Output directory"
            input.input placeholder="torc_output" data-bind="outputDir" data-on:change=(reload);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_tokens() {
        let h = highlight_json(r#"{"a": "x<y", "n": -1.5e3, "t": true}"#);
        assert!(h.contains(r#"<span class="k">"a"</span>:"#));
        assert!(h.contains(r#"<span class="s">"x&lt;y"</span>"#));
        assert!(h.contains(r#"<span class="n">-1.5e3</span>"#));
        assert!(h.contains(r#"<span class="b">true</span>"#));
        assert!(h.contains(r#"<span class="p">{</span>"#));
    }

    #[test]
    fn yaml_tokens() {
        let h = highlight_yaml(
            "name: test # c\njobs:\n  - name: a\n    cmd: |\n      echo 1\n    n: 3",
        );
        assert!(h.contains(
            r#"<span class="k">name</span>: <span class="s">test</span> <span class="c"># c</span>"#
        ));
        assert!(h.contains(r#"<span class="p">- </span><span class="k">name</span>"#));
        assert!(h.contains(r#"<span class="s">      echo 1</span>"#));
        assert!(h.contains(r#"<span class="k">n</span>: <span class="n">3</span>"#));
        assert_eq!(comment_start("a: 'x # y' # z"), Some(11));
    }

    #[test]
    fn marks() {
        let (h, n) = mark_matches("Error <x> error", "ERROR");
        assert_eq!(n, 2);
        assert_eq!(h, "<mark>Error</mark> &lt;x&gt; <mark>error</mark>");
    }
}
