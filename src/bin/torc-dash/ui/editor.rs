//! Spec editor tab: a plain `<textarea>` bound to `$editor.text` (format in `$editor.format`),
//! checked on the server as you type (the real spec parser and dry-run validation, with
//! clickable line numbers); templates from examples/yaml; snippets for common blocks; open and
//! save files on the torc-dash host; validate and create. The draft is kept in localStorage.
//!
//! Elements whose content the browser manages (the textarea, gutter, problem list, report, and
//! the AI chat slot) carry `data-ignore-morph`, so re-rendering the tab never resets them; the
//! server updates the slots with `Ui::inner`.

use std::sync::{Arc, LazyLock};

use axum::{
    Json, Router,
    extract::{Query, State},
    routing::{get, post},
};
use maud::{Markup, html};
use regex::Regex;
use serde::Deserialize;
use serde_json::{Value, json};
use torc::client::workflow_spec::WorkflowSpec;

use super::Ctx;
use super::create::{self, Report, report_panel};
use super::kit::{self, Sig, Tone, Ui, icon, js};
use crate::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ui/editor/check", post(check))
        .route("/ui/editor/templates", get(templates))
        .route("/ui/editor/template", post(use_template))
        .route("/ui/editor/insert-menu", get(insert_menu))
        .route("/ui/editor/insert", post(insert))
        .route("/ui/editor/open-local", post(open_local))
        .route("/ui/editor/dialog", get(path_dialog))
        .route("/ui/editor/open-path", post(open_path))
        .route("/ui/editor/save-path", post(save_path))
        .route("/ui/editor/save", post(save))
        .route("/ui/editor/validate", post(validate))
        .route("/ui/editor/create", post(create_workflow))
}

pub const BLANK: &str = "name: my_workflow
description: What this workflow does
jobs:
  - name: hello
    command: echo hello
";

const STORE: &str = "torc-dash.editor";

/// Initial `$editor` signals: defaults, then the draft saved in localStorage.
pub static INIT: LazyLock<String> = LazyLock::new(|| {
    format!(
        "{{editor: Object.assign({{text: {}, format: 'yaml', path: '', saved: '', ai: false, init: true, pathInput: '', cursor: 0, pending: '', incoming: ''}}, (() => {{ try {{ return JSON.parse(localStorage.getItem('{STORE}')) || {{}} }} catch (e) {{ return {{}} }} }})())}}",
        js(BLANK)
    )
});

/// Keep the draft across reloads (a convenience; storage failures are ignored).
const PERSIST: &str = "(() => { try { localStorage.setItem('torc-dash.editor', JSON.stringify({text: $editor.text, format: $editor.format, path: $editor.path, saved: $editor.saved, ai: $editor.ai, init: $editor.init})) } catch (e) {} })()";

/// Tab inserts two spaces instead of leaving the text area (Esc then Tab still moves on).
pub const TAB_KEY: &str = "if (evt.key === 'Tab' && !evt.shiftKey && !evt.metaKey && !evt.ctrlKey && !evt.altKey) { evt.preventDefault(); el.setRangeText('  ', el.selectionStart, el.selectionEnd, 'end'); el.dispatchEvent(new Event('input', {bubbles: true})) }";

/// The editor's signals.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Draft {
    pub text: String,
    pub format: String,
    /// File on the torc-dash machine this text was opened from or saved to ("" = none).
    pub path: String,
    /// Text as last opened from or saved to `path`.
    pub saved: String,
    pub init: bool,
    path_input: String,
    /// Selection start in the text area (UTF-16 units), for inserting snippets.
    cursor: usize,
    #[serde(deserialize_with = "create::obj")]
    incoming: Option<Incoming>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Incoming {
    name: String,
    text: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Sigs {
    editor: Draft,
}

impl Draft {
    fn format(&self) -> &str {
        match self.format.as_str() {
            "json" | "json5" | "kdl" => &self.format,
            _ => "yaml",
        }
    }
}

/// A dialog in a wrapper with its own id: the modal slot morphs by id, so a dialog replacing
/// another (e.g. templates → "Replace?") is inserted fresh and opens, instead of being morphed
/// into the old one (which drops its `open` attribute).
pub fn modal_in(id: &str, title: &str, size: &str, body: Markup, footer: Option<Markup>) -> Markup {
    html! { div id=(id) { (kit::modal(title, size, body, footer)) } }
}

/// Editor format for a file extension (".yml" → "yaml"); YAML when unknown.
pub fn format_of(ext_or_path: &str) -> &'static str {
    let ext = ext_or_path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "json" => "json",
        "json5" => "json5",
        "kdl" => "kdl",
        _ => "yaml",
    }
}

fn known_ext(name: &str) -> bool {
    let lower = name.to_lowercase();
    [".yaml", ".yml", ".json", ".json5", ".kdl"]
        .iter()
        .any(|e| lower.ends_with(e))
}

/// Replace the editor contents and switch to the editor tab, after asking if that would
/// discard edits not saved anywhere. `on_cancel` runs if the user keeps the current text (e.g.
/// reopen the dialog that asked).
pub fn open_in_editor(
    cur: &Draft,
    text: String,
    path: String,
    format: &str,
    on_cancel: Option<&str>,
) -> Ui {
    let unsaved = if cur.path.is_empty() {
        !cur.text.trim().is_empty() && cur.text != BLANK
    } else {
        cur.text != cur.saved
    };
    let saved = if path.is_empty() { "" } else { &text };
    let next = json!({ "text": text, "path": path, "saved": saved, "format": format });
    if unsaved && cur.text != text {
        let cancel = match on_cancel {
            Some(a) => a.to_string(), // replaces this dialog
            None => "el.closest('dialog').close()".into(),
        };
        return Ui::new()
            .signals(json!({ "editor": { "pending": next } }))
            .modal(modal_in("editor-replace-dlg",
                "Replace the spec in the editor?",
                "sm",
                html! { p { "Its unsaved changes will be lost." } },
                Some(html! {
                    button.btn data-on:click=(cancel) { "Cancel" }
                    button.btn.btn-danger-solid
                        data-on:click="const p = $editor.pending; $editor.text = p.text; $editor.path = p.path; $editor.saved = p.saved; $editor.format = p.format; $editor.pending = ''; el.closest('dialog').close(); location.hash = '/editor'" {
                        "Replace"
                    }
                }),
            ));
    }
    // The dialog may hold the button that asked; remove it last or later events are lost.
    Ui::new()
        .signals(json!({ "editor": next }))
        .script("location.hash = '/editor'")
        .close_modal()
}

// ============== View ==============

pub async fn view(_st: &Arc<AppState>, _cx: &Ctx) -> Markup {
    let gutter =
        "Array.from({length: $editor.text.split('\\n').length}, (_, i) => i + 1).join('\\n')";
    let file_label = "($editor.path || 'Unsaved draft') + ($editor.path && $editor.text !== $editor.saved ? ' •' : '')";
    let save_key = "if ((evt.metaKey || evt.ctrlKey) && evt.key === 's' && !document.querySelector('#modals dialog[open]')) { evt.preventDefault(); @post('/ui/editor/save') }";
    let open_local = "const f = el.files[0]; el.value = ''; f && f.text().then(t => { $editor.incoming = {name: f.name, text: t}; @post('/ui/editor/open-local') })";
    html! {
        div.page.spec-page data-signals__ifmissing=(INIT.as_str()) data-effect=(PERSIST)
            data-on:keydown__window=(save_key) {
            div.toolbar {
                button.btn.btn-sm data-attr:disabled="$chat.editing" data-preserve-attr="disabled" data-on:click="@get('/ui/editor/templates')" { (icon("plus", 13)) " New from template" }
                button.btn.btn-sm title="Insert a building block (YAML: into its section; JSON: at the cursor)"
                    data-attr:disabled="$editor.format === 'kdl' || $chat.editing" data-preserve-attr="disabled"
                    data-on:click="torcAnchor(el); @get('/ui/editor/insert-menu')" {
                    "Insert " (icon("chevronDown", 13))
                }
                button.btn.btn-sm title="Open a spec file from this computer" data-attr:disabled="$chat.editing" data-preserve-attr="disabled"
                    data-on:click="document.getElementById('editor-file').click()" { (icon("upload", 13)) " Open local file" }
                button.btn.btn-sm title="Open a file on the machine running torc-dash" data-attr:disabled="$chat.editing" data-preserve-attr="disabled"
                    data-on:click="@get('/ui/editor/dialog?kind=open')" { (icon("file", 13)) " Open path" }
                button.btn.btn-sm title="Save (⌘S / Ctrl+S)" data-on:click="@post('/ui/editor/save')" { (icon("download", 13)) " Save" }
                button.btn.btn-sm.btn-ghost data-show="$editor.path" data-preserve-attr="style" style="display:none"
                    data-on:click="@get('/ui/editor/dialog?kind=save')" { "Save as…" }
                select.select.sm aria-label="Spec format" title="Format of the text" data-bind="editor.format" {
                    option value="yaml" { "YAML" }
                    option value="json" { "JSON" }
                    option value="json5" { "JSON5" }
                    option value="kdl" { "KDL" }
                }
                span.file.muted data-ignore-morph data-text=(file_label) data-attr:title="$editor.path" {}
                span.grow {}
                button.btn.btn-sm data-class:btn-active="$editor.ai" data-preserve-attr="class"
                    data-attr:aria-pressed="$editor.ai" data-on:click="$editor.ai = !$editor.ai" {
                    (icon("chat", 13)) " Assistant"
                }
                label.check { input type="checkbox" data-bind="editor.init"; " Initialize after create" }
                button.btn.btn-sm data-indicator="_evalidating" data-attr:disabled="$_evalidating" data-preserve-attr="disabled"
                    data-on:click="@post('/ui/editor/validate')" {
                    span.spinner.small data-show="$_evalidating" data-preserve-attr="style" style="display:none" {}
                    span data-show="!$_evalidating" data-preserve-attr="style" { (icon("check", 13)) }
                    " Validate"
                }
                button.btn.btn-sm.btn-primary data-indicator="_ecreating"
                    data-attr:disabled="$_ecreating || !$editor.text.trim()" data-preserve-attr="disabled"
                    data-on:click="@post('/ui/editor/create')" {
                    span data-text="$_ecreating ? 'Creating…' : 'Create workflow'" data-ignore-morph { "Create workflow" }
                }
            }
            input #editor-file type="file" accept=".yaml,.yml,.json,.json5,.kdl" hidden data-on:change=(open_local);

            div.spec-body {
                div.spec-main {
                    div.spec-wrap {
                        pre #editor-gutter.gutter aria-hidden="true" data-ignore-morph data-text=(gutter) {}
                        textarea #editor-text data-ignore-morph data-bind="editor.text" data-attr:readonly="$chat.editing" spellcheck="false" wrap="off"
                            aria-label="Workflow spec" autocomplete="off"
                            data-on:keydown=(TAB_KEY)
                            data-on:scroll="document.getElementById('editor-gutter').scrollTop = el.scrollTop" {}
                    }
                    div #editor-problems.problems data-ignore-morph
                        data-init="@post('/ui/editor/check')"
                        "data-on-signal-patch__debounce.400ms"="@post('/ui/editor/check')"
                        data-on-signal-patch-filter="{include: /^editor\\.(text|format)$/}" {}
                }
                aside.spec-aside data-class:ai="$editor.ai" data-preserve-attr="class" {
                    div #editor-report data-ignore-morph {}
                    section.assistant data-show="$editor.ai" data-preserve-attr="style" style="display:none" {
                        header {
                            strong { "Assistant" }
                            span.faint { "reads and edits this spec" }
                            span.grow {}
                            button.btn.btn-sm.btn-ghost.btn-icon title="Close" aria-label="Close" data-on:click="$editor.ai = false" { (icon("x", 13)) }
                        }
                        div #editor-chat data-ignore-morph data-init="@get('/ui/chat/editor-panel')" {}
                    }
                }
            }
            p.faint.tips {
                "Problems are checked as you type; click one to jump to its line · Tab indents · ⌘S / Ctrl+S saves · Validate runs a full dry-run create"
            }
        }
    }
}

// ============== Checking ==============

/// The line (1-based) an error message refers to: its "line N", else the first line that
/// contains a name quoted in the message.
static LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bline (\d+)").unwrap());

fn line_of(msg: &str, text: &str) -> Option<usize> {
    static QUOTED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"'([^'\n]{1,80})'|"([^"\n]{1,80})"|`([^`\n]{1,80})`"#).unwrap()
    });
    let lines = text.lines().count().max(1);
    if let Some(n) = LINE.captures(msg).and_then(|c| c[1].parse::<usize>().ok()) {
        return Some(n.clamp(1, lines));
    }
    QUOTED.captures_iter(msg).find_map(|c| {
        let tok = c.get(1).or(c.get(2)).or(c.get(3))?.as_str();
        text.lines().position(|l| l.contains(tok)).map(|i| i + 1)
    })
}

/// Select line `n` in the text area and scroll it into view.
fn jump(n: usize) -> String {
    format!(
        "const t = document.getElementById('editor-text'), L = t.value.split('\\n'), n = Math.min({n}, L.length), a = L.slice(0, n - 1).join('\\n').length + (n > 1 ? 1 : 0); t.focus(); t.setSelectionRange(a, a + L[n - 1].length); t.scrollTop = Math.max(0, (n - 5) * parseFloat(getComputedStyle(t).lineHeight))"
    )
}

/// One error or warning, as a link to its line when one can be found.
fn problem(msg: &str, text: &str) -> Markup {
    match line_of(msg, text) {
        Some(n) => html! {
            button.problem-link type="button" title="Go to line" data-on:click=(jump(n)) {
                span.ln { "Ln " (n) } span.msg { (msg) }
            }
        },
        None => html! { span.msg { (msg) } },
    }
}

/// Parse error of the spec text deserialized directly (unlike the spec loader, which goes
/// through a JSON value), which carries the line of the offending field.
fn typed_parse_error(text: &str, format: &str) -> Option<String> {
    match format {
        "yaml" => serde_yaml::from_str::<WorkflowSpec>(text)
            .err()
            .map(|e| e.to_string()),
        "json" => serde_json::from_str::<WorkflowSpec>(text)
            .err()
            .map(|e| e.to_string()),
        "json5" => json5::from_str::<WorkflowSpec>(text)
            .err()
            .map(|e| e.to_string()),
        _ => None,
    }
}

/// Validate the text in-process (what `torc create --dry-run` does) from a temp file.
fn check_spec(text: &str, format: &str) -> (Vec<String>, Vec<String>) {
    let path = std::env::temp_dir().join(format!(
        "torc_editor_{}.{format}",
        uuid::Uuid::new_v4().simple()
    ));
    if let Err(e) = std::fs::write(&path, text) {
        return (vec![format!("Could not check the spec: {e}")], vec![]);
    }
    let r = WorkflowSpec::validate_spec(&path);
    let _ = std::fs::remove_file(&path);
    let mut errors = r.errors;
    if let Some(first) = errors.first_mut()
        && first.starts_with("Failed to parse")
        && !LINE.is_match(first)
        && let Some(typed) = typed_parse_error(text, format)
    {
        *first = format!("Failed to parse specification file: {typed}");
    }
    (errors, r.warnings)
}

/// Live check as you type (also clears a validation report, which described older text).
async fn check(Sig(s): Sig<Sigs>) -> Ui {
    let d = s.editor;
    let text = d.text.clone();
    let html = if text.trim().is_empty() {
        html! { div.problems-ok.muted { "Empty spec" } }
    } else {
        let format = d.format().to_string();
        let t = text.clone();
        let (errors, warnings) = tokio::task::spawn_blocking(move || check_spec(&t, &format))
            .await
            .unwrap_or_else(|e| (vec![e.to_string()], vec![]));
        if errors.is_empty() && warnings.is_empty() {
            html! { div.problems-ok { (icon("check", 13)) " No problems found" } }
        } else {
            html! {
                ul.problem-list {
                    @for e in &errors { li.error { (icon("x", 12)) (problem(e, &text)) } }
                    @for w in &warnings { li.warning { "⚠" (problem(w, &text)) } }
                }
            }
        }
    };
    Ui::new()
        .inner("#editor-problems", html)
        .inner("#editor-report", html! {})
}

// ============== Templates ==============

/// Starting points for "New from template": a short, curated set of examples/yaml specs (most
/// examples demo one feature; those are reachable through Insert). (group, name, text)
const TEMPLATES: [(&str, &str, &str); 8] = [
    (
        "Basics",
        "diamond_workflow",
        include_str!("../../../../examples/yaml/diamond_workflow.yaml"),
    ),
    (
        "Many jobs",
        "hundred_jobs_parameterized",
        include_str!("../../../../examples/yaml/hundred_jobs_parameterized.yaml"),
    ),
    (
        "Many jobs",
        "hyperparameter_sweep",
        include_str!("../../../../examples/yaml/hyperparameter_sweep.yaml"),
    ),
    (
        "Many jobs",
        "data_pipeline_parameterized",
        include_str!("../../../../examples/yaml/data_pipeline_parameterized.yaml"),
    ),
    (
        "Slurm",
        "workflow_actions_simple_slurm",
        include_str!("../../../../examples/yaml/workflow_actions_simple_slurm.yaml"),
    ),
    (
        "Slurm",
        "multi_node_slurm",
        include_str!("../../../../examples/yaml/multi_node_slurm.yaml"),
    ),
    (
        "Reliability",
        "simple_retry",
        include_str!("../../../../examples/yaml/simple_retry.yaml"),
    ),
    (
        "Monitoring",
        "resource_monitoring_demo",
        include_str!("../../../../examples/yaml/resource_monitoring_demo.yaml"),
    ),
];

/// All templates in display order, Blank first: (group, name, description, text).
fn all_templates() -> Vec<(&'static str, &'static str, String, &'static str)> {
    let mut v = vec![("Basics", "Blank", "A name and one job".to_string(), BLANK)];
    v.extend(TEMPLATES.iter().map(|(g, n, t)| {
        // A template that doesn't parse still opens; it just has no description.
        let d = serde_yaml::from_str::<serde_yaml::Value>(t)
            .ok()
            .and_then(|v| {
                v.get("description")
                    .and_then(|d| d.as_str())
                    .map(String::from)
            })
            .unwrap_or_default();
        (*g, *n, d, *t)
    }));
    v
}

async fn templates() -> Ui {
    let all = all_templates();
    Ui::new().modal(modal_in("editor-templates-dlg",
        "New from template",
        "xl",
        html! {
            div.templates data-signals="{_tpl: 0}" {
                ul.tpl-list {
                    @for (i, (group, name, desc, _)) in all.iter().enumerate() {
                        @if i == 0 || all[i - 1].0 != *group { li.group { (group) } }
                        li {
                            button data-class:active=(format!("$_tpl === {i}")) data-on:click=(format!("$_tpl = {i}"))
                                data-on:dblclick=(format!("@post('/ui/editor/template?i={i}')")) {
                                strong { (name) } span.muted { (desc) }
                            }
                        }
                    }
                }
                div.tpl-previews {
                    @for (i, (_, _, _, text)) in all.iter().enumerate() {
                        pre.code-block data-show=(format!("$_tpl === {i}")) style=[(i > 0).then_some("display:none")] { (text) }
                    }
                }
            }
        },
        Some(html! {
            span.faint.grow { "Double-click a template to use it" }
            (kit::cancel_button())
            button.btn.btn-primary data-on:click="@post('/ui/editor/template?i=' + $_tpl)" { "Use template" }
        }),
    ))
}

#[derive(Deserialize)]
struct Index {
    i: usize,
}

async fn use_template(Query(q): Query<Index>, Sig(s): Sig<Sigs>) -> Ui {
    let Some((_, _, _, text)) = all_templates().into_iter().nth(q.i) else {
        return Ui::new();
    };
    open_in_editor(
        &s.editor,
        text.to_string(),
        String::new(),
        "yaml",
        Some("@get('/ui/editor/templates')"),
    )
}

// ============== Snippets ==============

/// A spec building block for the Insert menu. `map`: the section is a mapping (keys, not list
/// items), so it can only be inserted once. Text is written at indent 0 and re-indented.
struct Snippet {
    label: &'static str,
    section: &'static str,
    text: &'static str,
    map: bool,
}

const fn snip(label: &'static str, section: &'static str, text: &'static str) -> Snippet {
    Snippet {
        label,
        section,
        text,
        map: false,
    }
}

const SNIPPETS: [Snippet; 11] = [
    snip("Job", "jobs", "- name: my_job\n  command: echo hello\n"),
    snip(
        "Parameterized job",
        "jobs",
        "- name: job_{i:03d}\n  command: python run.py --index {i}\n  parameters:\n    i: \"1:10\"\n",
    ),
    snip(
        "Job with file dependencies",
        "jobs",
        "- name: process\n  command: python process.py --input data/raw.csv --output data/clean.csv\n  input_files: [raw_data]\n  output_files: [clean_data]\n",
    ),
    snip("File", "files", "- name: raw_data\n  path: data/raw.csv\n"),
    snip(
        "Resource requirements",
        "resource_requirements",
        "- name: small\n  num_cpus: 1\n  memory: 2g\n  runtime: PT30M\n",
    ),
    snip(
        "Slurm scheduler",
        "slurm_schedulers",
        "- name: compute\n  account: my_account\n  nodes: 1\n  walltime: \"04:00:00\"\n",
    ),
    Snippet {
        label: "Execution config",
        section: "execution_config",
        text: "mode: direct\nlimit_resources: true\ntermination_signal: SIGTERM\nsigterm_lead_seconds: 30\n",
        map: true,
    },
    Snippet {
        label: "Resource monitoring",
        section: "resource_monitor",
        text: "sample_interval_seconds: 10\ngenerate_plots: true\njobs:\n  enabled: true\n  granularity: summary  # or time_series\ncompute_node:\n  enabled: false\n  granularity: summary  # or time_series\n  # cpu: true\n  # memory: true\n# flush_interval_seconds: 300  # how often time-series samples are written to SQLite\n# live_monitoring: false  # checkpoint after each flush so torc-dash live plots see new samples\n",
        map: true,
    },
    // on_jobs_ready, not on_workflow_start: a selective re-run (reset-status --reinit, then
    // submit) re-schedules only the allocations whose jobs were reset.
    snip(
        "Action: schedule Slurm nodes when jobs are ready",
        "actions",
        "- trigger_type: on_jobs_ready\n  action_type: schedule_nodes\n  jobs: [my_job]\n  scheduler: compute\n  scheduler_type: slurm\n  num_allocations: 1\n",
    ),
    snip(
        "Failure handler (retry)",
        "failure_handlers",
        "- name: retry_on_timeout\n  rules:\n    - exit_codes: [152]\n      max_retries: 3\n",
    ),
    snip(
        "User data",
        "user_data",
        "- name: config\n  data:\n    key: value\n",
    ),
];

fn indent(s: &str, n: usize) -> String {
    s.split('\n')
        .map(|l| {
            if l.is_empty() {
                String::new()
            } else {
                format!("{}{l}", " ".repeat(n))
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Append `sn` to the end of its top-level YAML section, creating the section at the end of the
/// document if missing. Items are indented like the section's existing items (default 2). An
/// empty section (`files: []`, `{}`, `null`, `~`) becomes a block section; a non-empty flow
/// section is an error, since rewriting it could lose comments or formatting. Returns the new
/// text and the inserted byte range.
fn insert_snippet(text: &str, sn: &Snippet) -> Result<(String, usize, usize), String> {
    static AFTER_KEY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^:]+:\s*").unwrap());
    static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(^|\s)#.*$").unwrap());
    static EMPTY: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(\[\s*\]|\{\s*\}|~|null|Null|NULL)$").unwrap());
    static TRAILING_COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s(#.*)$").unwrap());
    static TOP_KEY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^\s#-]").unwrap());
    static CONTENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s+[^\s#]").unwrap());
    static ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*- ").unwrap());
    let head_re = Regex::new(&format!(r"^{}\s*:", regex::escape(sn.section))).unwrap();

    let mut lines: Vec<String> = text.split('\n').map(String::from).collect();
    let mut head = lines.iter().position(|l| head_re.is_match(l));
    if let Some(h) = head {
        let after = AFTER_KEY.replace(&lines[h], "").to_string();
        let value = COMMENT.replace(&after, "").trim().to_string();
        if EMPTY.is_match(&value) {
            lines[h] = match TRAILING_COMMENT.captures(&lines[h]) {
                Some(c) => format!("{}:  {}", sn.section, &c[1]),
                None => format!("{}:", sn.section),
            };
        } else if value.starts_with('[') || value.starts_with('{') {
            return Err(format!(
                "\"{}\" is written in flow style. Convert it to a block list to insert here.",
                sn.section
            ));
        } else if !value.is_empty() {
            head = None; // not a section (e.g. a scalar); append a new one
        }
    }
    let Some(head) = head else {
        let sep = if text.trim().is_empty() || text.ends_with("\n\n") {
            ""
        } else if text.ends_with('\n') {
            "\n"
        } else {
            "\n\n"
        };
        let body = format!("{}:\n{}", sn.section, indent(sn.text, 2));
        let from = text.len() + sep.len();
        return Ok((format!("{text}{sep}{body}"), from, from + body.len()));
    };
    // The section ends at the next top-level key (column 0, not a list dash or comment).
    let mut end = (head + 1..lines.len())
        .find(|i| TOP_KEY.is_match(&lines[*i]))
        .unwrap_or(lines.len());
    while end - 1 > head && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    let section = &lines[head + 1..end];
    if sn.map && section.iter().any(|l| CONTENT.is_match(l)) {
        return Err(format!("\"{}\" already exists. Edit it there.", sn.section));
    }
    let n = section
        .iter()
        .find(|l| ITEM.is_match(l))
        .map(|l| l.len() - l.trim_start().len())
        .unwrap_or(2);
    let body = indent(sn.text, n).trim_end_matches('\n').to_string();
    let before = lines[..end].join("\n");
    let after = lines[end..].join("\n");
    let from = before.len() + 1;
    let rest = if end < lines.len() {
        format!("\n{after}")
    } else {
        "\n".into()
    };
    Ok((format!("{before}\n{body}{rest}"), from, from + body.len()))
}

/// UTF-16 length (JavaScript string offsets) of `s`.
fn u16len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Byte offset of a UTF-16 offset in `s` (clamped to the end).
fn byte_at(s: &str, u16_offset: usize) -> usize {
    let mut n = 0;
    for (i, c) in s.char_indices() {
        if n >= u16_offset {
            return i;
        }
        n += c.len_utf16();
    }
    s.len()
}

async fn insert_menu() -> Ui {
    Ui::new()
        .inner(
            "#menu",
            html! {
                @for (i, s) in SNIPPETS.iter().enumerate() {
                    button role="menuitem" data-on:click=(format!(
                        "document.getElementById('menu').hidePopover(); $editor.cursor = document.getElementById('editor-text')?.selectionStart || 0; @post('/ui/editor/insert?i={i}')"
                    )) {
                        (s.label) span.faint.section { (s.section) }
                    }
                }
            },
        )
        .script("torcMenu()")
}

/// Insert a snippet: YAML into its section, JSON/JSON5 (converted) at the cursor; then select
/// the inserted text.
async fn insert(Query(q): Query<Index>, Sig(s): Sig<Sigs>) -> Ui {
    let Some(sn) = SNIPPETS.get(q.i) else {
        return Ui::new();
    };
    let d = s.editor;
    let r = match d.format() {
        "yaml" => insert_snippet(&d.text, sn),
        "kdl" => Err("Insert works on YAML and JSON specs".to_string()),
        _ => (|| {
            let v: Value = serde_yaml::from_str(sn.text).map_err(|e| e.to_string())?;
            let piece = if sn.map {
                format!(
                    "{}: {}",
                    js(sn.section),
                    serde_json::to_string_pretty(&v).unwrap_or_default()
                )
            } else {
                serde_json::to_string_pretty(&v[0]).unwrap_or_default()
            };
            let at = byte_at(&d.text, d.cursor);
            let text = format!("{}{piece}{}", &d.text[..at], &d.text[at..]);
            Ok((text, at, at + piece.len()))
        })(),
    };
    match r {
        Ok((text, from, to)) => {
            let (f, t) = (u16len(&text[..from]), u16len(&text[..to]));
            Ui::new()
                .signals(json!({ "editor": { "text": text } }))
                .script(&format!(
                    "setTimeout(() => {{ const t = document.getElementById('editor-text'); if (!t) return; t.focus(); t.setSelectionRange({f}, {t}); t.scrollTop = Math.max(0, (t.value.slice(0, {f}).split('\\n').length - 5) * parseFloat(getComputedStyle(t).lineHeight)) }}, 30)"
                ))
        }
        Err(e) => Ui::new().err(e),
    }
}

// ============== Files ==============

/// A file chosen from this computer.
async fn open_local(Sig(s): Sig<Sigs>) -> Ui {
    let d = s.editor;
    let Some(f) = d.incoming.as_ref() else {
        return Ui::new();
    };
    let clear = json!({ "editor": { "incoming": "" } });
    if !known_ext(&f.name) {
        return Ui::new().signals(clear).toast(
            Tone::Info,
            "The editor opens YAML, JSON, JSON5, and KDL files.",
        );
    }
    open_in_editor(&d, f.text.clone(), String::new(), format_of(&f.name), None).signals(clear)
}

#[derive(Deserialize)]
struct Kind {
    kind: String,
}

/// Open/save-as dialog for a path on the machine running torc-dash.
async fn path_dialog(Query(q): Query<Kind>, Sig(s): Sig<Sigs>) -> Ui {
    dialog(q.kind == "open", &s.editor)
}

fn dialog(open: bool, d: &Draft) -> Ui {
    let initial = if open {
        String::new()
    } else if !d.path.is_empty() {
        d.path.clone()
    } else {
        suggest_name(&d.text, d.format())
    };
    let action = if open {
        "@post('/ui/editor/open-path')"
    } else {
        "@post('/ui/editor/save-path')"
    };
    Ui::new()
        .signals(json!({ "editor": { "pathInput": initial } }))
        .modal(modal_in("editor-path-dlg",
            if open { "Open file" } else { "Save as" },
            "md",
            html! {
                form #editor-path-form data-on:submit=(format!("evt.preventDefault(); $editor.pathInput.trim() && {action}")) {
                    label.field {
                        "Path on the machine running torc-dash"
                        input.input data-bind="editor.pathInput" placeholder="workflow.yaml" autofocus autocomplete="off";
                        span.hint { "Relative paths resolve against torc-dash's working directory." }
                    }
                }
            },
            Some(html! {
                (kit::cancel_button())
                button.btn.btn-primary type="submit" form="editor-path-form" data-attr:disabled="!$editor.pathInput.trim()" {
                    (if open { "Open" } else { "Save" })
                }
            }),
        ))
}

fn suggest_name(text: &str, format: &str) -> String {
    static NAME: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)^(?:name:\s*["']?|\s*"?name"?\s*:\s*["']|name\s+")([\w.-]+)"#).unwrap()
    });
    let name = NAME
        .captures(text)
        .map(|c| c[1].to_string())
        .unwrap_or_else(|| "workflow".into());
    format!("{name}.{format}")
}

async fn open_path(Sig(s): Sig<Sigs>) -> Ui {
    let d = s.editor;
    let path = d.path_input.trim().to_string();
    if path.is_empty() {
        return Ui::new();
    }
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Res {
        success: bool,
        content: Option<String>,
        error: Option<String>,
        exists: bool,
    }
    let r: Res = kit::json_of(
        crate::cli_read_file_handler(Json(crate::ReadFileRequest { path: path.clone() })).await,
    )
    .await
    .unwrap_or_default();
    if !r.exists {
        return Ui::new().err(format!("{path} does not exist"));
    }
    match r.content.filter(|_| r.success) {
        Some(text) => open_in_editor(&d, text, path.clone(), format_of(&path), None),
        None => Ui::new().err(r.error.unwrap_or_else(|| "Could not read the file".into())),
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Overwrite {
    overwrite: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct WriteRes {
    success: bool,
    path: Option<String>,
    error: Option<String>,
    exists: bool,
}

async fn write(path: &str, content: &str, overwrite: bool) -> WriteRes {
    kit::json_of(
        crate::cli_write_file_handler(Json(crate::WriteFileRequest {
            path: path.to_string(),
            content: content.to_string(),
            overwrite,
        }))
        .await,
    )
    .await
    .unwrap_or_default()
}

/// Save to the path in the dialog; asks before replacing an existing file.
async fn save_path(Query(q): Query<Overwrite>, Sig(s): Sig<Sigs>) -> Ui {
    let d = s.editor;
    let path = d.path_input.trim().to_string();
    let r = write(&path, &d.text, q.overwrite).await;
    if !r.success && r.exists && !q.overwrite {
        return Ui::new().modal(modal_in("editor-overwrite-dlg",
            &format!("Replace {path}?"),
            "sm",
            html! { p { "The file already exists." } },
            Some(html! {
                button.btn data-on:click="@get('/ui/editor/dialog?kind=save')" { "Cancel" }
                button.btn.btn-danger-solid data-on:click="@post('/ui/editor/save-path?overwrite=true')" { "Replace" }
            }),
        ));
    }
    saved(r, &path, &d.text)
}

fn saved(r: WriteRes, path: &str, text: &str) -> Ui {
    if !r.success {
        return Ui::new().err(r.error.unwrap_or_else(|| "Save failed".into()));
    }
    let path = r.path.unwrap_or_else(|| path.to_string());
    tracing::info!("Saved spec editor draft to {path}");
    Ui::new()
        .signals(json!({ "editor": { "path": path, "saved": text } }))
        .ok(format!("Saved {path}"))
        .close_modal()
}

/// Save to the file the text came from, or ask where.
async fn save(Sig(s): Sig<Sigs>) -> Ui {
    let d = &s.editor;
    if d.path.is_empty() {
        return dialog(false, d);
    }
    let r = write(&d.path, &d.text, true).await;
    saved(r, &d.path, &d.text)
}

// ============== Validate / create ==============

fn body(d: &Draft) -> Value {
    json!({ "spec": d.text, "is_file": false, "file_extension": format!(".{}", d.format()) })
}

fn report_box(r: &Report, text: &str) -> Markup {
    html! {
        div.spec-report {
            (report_panel(r, &|m| problem(m, text)))
            button.btn.btn-sm.btn-ghost data-on:click="document.getElementById('editor-report').replaceChildren()" { "Dismiss" }
        }
    }
}

async fn validate(State(st): State<Arc<AppState>>, Sig(s): Sig<Sigs>) -> Ui {
    let d = s.editor;
    let r = create::validate_spec(&st, body(&d)).await;
    Ui::new().inner("#editor-report", report_box(&r, &d.text))
}

async fn create_workflow(State(st): State<Arc<AppState>>, Sig(s): Sig<Sigs>) -> Ui {
    let d = s.editor;
    if d.text.trim().is_empty() {
        return Ui::new();
    }
    let r = kit::call(&st, crate::cli_create_handler, body(&d))
        .await
        .and_then(kit::cli_result);
    match r {
        Ok(stdout) => {
            let id = crate::extract_workflow_id(&stdout).and_then(|i| i.parse::<i64>().ok());
            let ui = create::created(Ui::new(), id, d.init);
            if id.is_some() {
                ui.script("location.hash = '/workflows'")
            } else {
                ui
            }
        }
        Err(e) => Ui::new()
            .inner("#editor-report", report_box(&Report::failed(e), &d.text))
            .err("Failed to create workflow"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippets() {
        let job = &SNIPPETS[0];
        let (t, a, b) = insert_snippet(BLANK, job).unwrap();
        assert!(t.ends_with("  - name: my_job\n    command: echo hello\n"));
        assert_eq!(&t[a..b], "  - name: my_job\n    command: echo hello");
        let (t, _, _) = insert_snippet("name: x\nfiles: []  # none\n", &SNIPPETS[3]).unwrap();
        assert_eq!(
            t,
            "name: x\nfiles:  # none\n  - name: raw_data\n    path: data/raw.csv\n"
        );
        let (t, _, _) = insert_snippet("name: x", &SNIPPETS[3]).unwrap();
        assert_eq!(
            t,
            "name: x\n\nfiles:\n  - name: raw_data\n    path: data/raw.csv\n"
        );
        assert!(insert_snippet("jobs: [a]\n", job).is_err());
        let ec = &SNIPPETS[6];
        assert!(insert_snippet("execution_config:\n  mode: direct\n", ec).is_err());
        let (t, _, _) = insert_snippet("jobs:\n- name: a\n  command: b\nfiles: []\n", job).unwrap();
        assert!(t.starts_with("jobs:\n- name: a\n  command: b\n- name: my_job\n"));
    }

    #[test]
    fn lines() {
        let text = "name: x\njobs:\n  - name: foo\n";
        assert_eq!(line_of("bad at line 2 column 3", text), Some(2));
        assert_eq!(line_of("job 'foo' depends on missing", text), Some(3));
        assert_eq!(line_of("nothing here", text), None);
        assert_eq!(byte_at("aé😀b", 2), 3);
        assert_eq!(byte_at("aé😀b", 4), 7);
        assert_eq!(u16len("aé😀b"), 5);
    }
}
