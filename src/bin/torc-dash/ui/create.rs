//! Create-workflow modal: upload a spec file, point at a server-side path, paste a spec, list
//! commands to run, or build one with the wizard. Optional Slurm scheduler generation (with an
//! allocation-plan preview) and initialize-after-create.
//!
//! State lives in the `$create` signals. The source panels, options, and footer are rendered
//! once when the modal opens and react on the client; the wizard (`#create-wizard`) is rendered
//! on the server from `$create.w` and re-rendered after each change. Its spec model, generation,
//! and validation are in `create_wizard`.

use std::sync::Arc;

use axum::{
    Router,
    extract::{Query, State},
    routing::{get, post},
};
use maud::{Markup, html};
use serde::Deserialize;
use serde_json::{Value, json};

use super::editor::{self, Draft};
use super::kit::{self, Sig, Tone, Ui, icon};
use crate::AppState;
use wizard::{self as wiz, LAST, Wiz};

#[path = "create_wizard.rs"]
mod wizard;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ui/create/open", get(open))
        .route("/ui/create/render", post(render))
        .route("/ui/create/wiz", post(wiz_op))
        .route("/ui/create/wiz-validate", post(wiz_validate))
        .route("/ui/create/detect", post(detect))
        .route("/ui/create/validate", post(validate))
        .route("/ui/create/plan", post(plan))
        .route("/ui/create/create", post(create))
        .route("/ui/create/to-editor", post(to_editor))
}

const SOURCES: [(&str, &str); 5] = [
    ("upload", "Upload file"),
    ("path", "File path"),
    ("inline", "Paste spec"),
    ("commands", "Commands"),
    ("wizard", "Wizard"),
];
const FORMATS: [(&str, &str); 5] = [
    ("auto", "Auto-detect"),
    (".yaml", "YAML"),
    (".json", "JSON"),
    (".json5", "JSON5"),
    (".kdl", "KDL"),
];

// ============== Signals ==============

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Form {
    source: String,
    #[serde(deserialize_with = "obj")]
    upload: Option<Upload>,
    path: String,
    inline: String,
    inline_format: String,
    commands: String,
    commands_name: String,
    initialize: bool,
    use_slurm: bool,
    profile: String,
    account: String,
    profiles: Vec<Profile>,
    detected: String,
    w: Wiz,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct Upload {
    name: String,
    content: String,
    ext: String,
}

#[derive(Deserialize, Default, Clone, serde::Serialize)]
#[serde(default)]
struct Profile {
    name: String,
    display_name: String,
    is_detected: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Sigs {
    create: Form,
    editor: Draft,
}

/// An object-valued signal, or None for anything else (unset signals read back as "").
pub fn obj<'de, D: serde::Deserializer<'de>, T: serde::de::DeserializeOwned>(
    d: D,
) -> Result<Option<T>, D::Error> {
    let v = Value::deserialize(d)?;
    Ok(if v.is_object() {
        serde_json::from_value(v).ok()
    } else {
        None
    })
}

impl Form {
    /// Slurm generation is on: (account, profile). The Commands source has no resource
    /// requirements to generate from.
    fn slurm(&self) -> Option<(&str, &str)> {
        (!self.profiles.is_empty()
            && self.source != "commands"
            && self.use_slurm
            && !self.profile.is_empty())
        .then_some((self.account.trim(), self.profile.as_str()))
    }

    /// Request body of the create/validate handlers for the current source, or what's missing.
    fn spec_body(&self) -> Result<Value, String> {
        match self.source.as_str() {
            "path" => {
                let p = self.path.trim();
                if p.is_empty() {
                    return Err("Enter a spec file path".into());
                }
                Ok(json!({ "spec": p, "is_file": true }))
            }
            "inline" => {
                if self.inline.trim().is_empty() {
                    return Err("Paste a workflow spec".into());
                }
                let ext = if self.inline_format == "auto" || self.inline_format.is_empty() {
                    detect_format(&self.inline)
                } else {
                    &self.inline_format
                };
                Ok(json!({ "spec": self.inline, "is_file": false, "file_extension": ext }))
            }
            "commands" => commands_spec(&self.commands, &self.commands_name)
                .map(|s| json!({ "spec": s, "is_file": false, "file_extension": ".json" }))
                .ok_or_else(|| "Enter at least one command".into()),
            "wizard" => Ok(json!({
                "spec": serde_json::to_string_pretty(&wiz::generate(&self.w)).unwrap_or_default(),
                "is_file": false,
                "file_extension": ".json",
            })),
            _ => self
                .upload
                .as_ref()
                .map(|u| json!({ "spec": u.content, "is_file": false, "file_extension": u.ext }))
                .ok_or_else(|| "Choose a workflow spec file to upload".into()),
        }
    }
}

/// Guess the format of pasted text (as the file extension the CLI expects).
fn detect_format(text: &str) -> &'static str {
    static KDL: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r#"(?m)^\s*(name|description|job|file)\s+""#).unwrap()
    });
    static YAML_KEY: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"(?m)^\s*\w+:\s").unwrap());
    let t = text.trim();
    if serde_json::from_str::<Value>(t).is_ok() {
        ".json"
    } else if t.starts_with('{') {
        ".json5"
    } else if KDL.is_match(t) && !YAML_KEY.is_match(t) {
        ".kdl"
    } else {
        ".yaml"
    }
}

/// A spec (JSON text) with one independent job per command line, like `torc exec -C file`.
/// Blank lines and `#` comments are skipped. None when there are no commands.
fn commands_spec(text: &str, name: &str) -> Option<String> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if lines.is_empty() {
        return None;
    }
    let width = lines.len().to_string().len().max(3);
    let name = if name.trim().is_empty() {
        "commands"
    } else {
        name.trim()
    };
    let jobs: Vec<Value> = lines
        .iter()
        .enumerate()
        .map(|(i, c)| json!({ "name": format!("job_{:0width$}", i + 1), "command": c }))
        .collect();
    serde_json::to_string_pretty(&json!({ "name": name, "jobs": jobs })).ok()
}

// ============== Validation report (shared with the spec editor) ==============

/// Parsed stdout of `torc -f json create --dry-run`.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Report {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    summary: Option<Summary>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Summary {
    workflow_name: Option<String>,
    job_count: Option<i64>,
    job_count_before_expansion: Option<i64>,
    file_count: Option<i64>,
    user_data_count: Option<i64>,
    resource_requirements_count: Option<i64>,
    slurm_scheduler_count: Option<i64>,
    action_count: Option<i64>,
}

impl Report {
    pub fn failed(msg: String) -> Self {
        Report {
            errors: vec![msg],
            ..Default::default()
        }
    }
}

/// Dry-run create through the validate handler. An invalid spec is a report with errors.
pub async fn validate_spec(st: &Arc<AppState>, body: Value) -> Report {
    let res: Result<crate::CliResponse, String> =
        kit::call(st, crate::cli_validate_handler, body).await;
    match res {
        Ok(r) => match serde_json::from_str::<Report>(&r.stdout) {
            Ok(rep) => rep,
            Err(_) => {
                let msg = [r.stderr.trim(), r.stdout.trim()]
                    .into_iter()
                    .find(|s| !s.is_empty())
                    .unwrap_or("Validation failed")
                    .to_string();
                Report {
                    valid: r.success,
                    errors: if r.success { vec![] } else { vec![msg] },
                    ..Default::default()
                }
            }
        },
        Err(e) => Report {
            errors: vec![e],
            ..Default::default()
        },
    }
}

/// Result panel of a dry run. `item` renders one error or warning (the editor links lines).
pub fn report_panel(r: &Report, item: &dyn Fn(&str) -> Markup) -> Markup {
    let s = r.summary.as_ref();
    let stats: Vec<(&str, Option<i64>, Option<i64>)> = s
        .map(|s| {
            vec![
                ("Jobs", s.job_count, s.job_count_before_expansion),
                ("Files", s.file_count, None),
                ("User data", s.user_data_count, None),
                ("Resource reqs", s.resource_requirements_count, None),
                ("Schedulers", s.slurm_scheduler_count, None),
                ("Actions", s.action_count, None),
            ]
        })
        .unwrap_or_default()
        .into_iter()
        .filter(|(l, v, _)| *l == "Jobs" || v.unwrap_or(0) > 0)
        .collect();
    html! {
        div.vpanel.ok[r.valid].bad[!r.valid] aria-live="polite" {
            div.vtitle {
                span.vicon { (icon(if r.valid { "check" } else { "x" }, 14)) }
                strong { (if r.valid { "Spec is valid" } else { "Spec has errors" }) }
                @if let Some(n) = s.and_then(|s| s.workflow_name.as_deref()).filter(|n| !n.is_empty()) {
                    span.muted { "· " (n) }
                }
            }
            @if !r.errors.is_empty() { ul.verrors { @for e in &r.errors { li { (item(e)) } } } }
            @if !r.warnings.is_empty() { ul.vwarnings { @for w in &r.warnings { li { (item(w)) } } } }
            @if !stats.is_empty() {
                div.vstats {
                    @for (label, v, before) in &stats {
                        div {
                            span.v { (v.map(|v| v.to_string()).unwrap_or_else(|| "?".into())) }
                            span.l {
                                (label)
                                @if let Some(b) = before.filter(|b| Some(*b) != *v) {
                                    span.faint { " (from " (b) " specs)" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn plain(s: &str) -> Markup {
    html! { (s) }
}

// ============== Modal ==============

async fn hpc_profiles(st: &Arc<AppState>) -> (Vec<Profile>, String) {
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Res {
        success: bool,
        profiles: Vec<Profile>,
        detected_profile: Option<String>,
    }
    let r: Res = kit::json_of(crate::cli_hpc_profiles_handler(State(st.clone())).await)
        .await
        .unwrap_or_default();
    if !r.success {
        return (Vec::new(), String::new());
    }
    (r.profiles, r.detected_profile.unwrap_or_default())
}

/// `GET /ui/create/open` (the `n` key and Create buttons): the modal. Other inputs are kept
/// from last time until a create succeeds; the wizard starts over.
async fn open(State(st): State<Arc<AppState>>, Sig(s): Sig<Sigs>) -> Ui {
    let f = s.create;
    let (profiles, detected) = hpc_profiles(&st).await;
    let mut profile = f.profile.clone();
    if profile.is_empty() || !profiles.iter().any(|p| p.name == profile) {
        profile = if detected.is_empty() {
            profiles.first().map(|p| p.name.clone()).unwrap_or_default()
        } else {
            detected.clone()
        };
    }
    let fresh = Wiz::default();
    let defaults = json!({ "create": {
        "source": "upload", "upload": "", "path": "", "inline": "", "inlineFormat": "auto",
        "commands": "", "commandsName": "", "initialize": true, "useSlurm": false,
        "profile": "", "account": "", "accountError": false,
    }});
    let mut patch = json!({ "create": {
        "profiles": profiles, "detected": detected, "profile": profile, "accountError": false,
        "w": wiz::patch(&f.w, &fresh),
    }});
    if profile.is_empty() {
        patch["create"]["useSlurm"] = json!(false);
    }
    let source = if f.source.is_empty() {
        "upload".to_string()
    } else {
        f.source.clone()
    };
    let form = Form {
        profiles: serde_json::from_value(patch["create"]["profiles"].clone()).unwrap_or_default(),
        w: fresh,
        ..f
    };
    let body = modal_body(&form);
    let dialog = kit::modal("Create workflow", "xl", body, Some(footer()));
    Ui::new()
        .signals(patch)
        .modal(html! {
            div #create-host data-signals__ifmissing=(defaults.to_string()) { (dialog) }
        })
        .script(&focus_first_js(&source))
}

/// Focus the first control of the current source's panel.
fn focus_first_js(source: &str) -> String {
    if source == "wizard" {
        return wiz::FOCUS_FIRST.to_string();
    }
    format!(
        "setTimeout(() => document.querySelector('[data-panel={source}] :is(.dropzone, input:not([type=file]), textarea)')?.focus(), 30)"
    )
}

/// Keyboard shortcuts inside the modal: ←/→ or 1–5 switch source, [ ] (or ↑ ↓) wizard steps,
/// Enter focuses the panel (or creates on the review step), ⌘/Ctrl+Enter is the primary button.
const KEYS: &str = r#"
const t = evt.target;
if (evt.defaultPrevented || !t.closest || !t.closest('dialog')?.contains(el)) return;
const click = (id) => document.getElementById(id)?.click();
if (evt.key === 'Enter' && (evt.metaKey || evt.ctrlKey)) { evt.preventDefault(); click('create-primary'); return; }
if (evt.metaKey || evt.ctrlKey || evt.altKey || t.closest('input, textarea, select, [contenteditable]')) return;
const S = ['upload', 'path', 'inline', 'commands', 'wizard'];
const i = S.indexOf($create.source), k = evt.key, wz = $create.source === 'wizard';
let s = null;
if (k === 'ArrowLeft' || k === 'ArrowRight') s = S[(i + (k === 'ArrowLeft' ? S.length - 1 : 1)) % S.length];
else if (k >= '1' && k <= '5') s = S[+k - 1];
else if (wz && (k === '[' || k === 'ArrowUp')) click('create-back');
else if (wz && (k === ']' || k === 'ArrowDown')) click('create-primary');
else if (k === 'Enter' && !t.closest('button, a, summary, label')) {
  if (wz && $create.w.step === 6) click('create-primary');
  else document.querySelector('[data-panel=' + $create.source + '] :is(.dropzone, input:not([type=file]), textarea, select, .empty-add)')?.focus();
} else return;
evt.preventDefault();
if (s) { $create.source = s; setTimeout(() => document.querySelector('[data-panel=' + s + '] :is(.dropzone, input:not([type=file]), textarea, select, .empty-add)')?.focus(), 30); }
"#;

/// Read a chosen or dropped file into `$create.upload`.
const READ_FILE: &str = "const f = el.files[0]; f && f.text().then(t => { const d = f.name.lastIndexOf('.'); $create.upload = {name: f.name, size: f.size, content: t, ext: d >= 0 ? f.name.slice(d).toLowerCase() : '.json'} })";

fn modal_body(f: &Form) -> Markup {
    html! {
        div #create-root.create data-on:keydown__window=(KEYS)
            data-signals__ifmissing=(editor::INIT.as_str()) {
            div.top {
                div.seg role="tablist" {
                    @for (id, label) in SOURCES {
                        button role="tab" data-class:active=(format!("$create.source === '{id}'"))
                            data-on:click=(format!("$create.source = '{id}'; {}", focus_first_js(id))) { (label) }
                    }
                }
                span.faint.hint { kbd { "←" } kbd { "→" } " or " kbd { "1" } "–" kbd { "5" } " switch source" }
            }

            // Results describe the inputs they were computed from.
            div.panels data-on:input="document.querySelectorAll('#create-report, #create-error, #create-plan').forEach(e => e.replaceChildren())" {
                div.panel data-panel="upload" data-show="$create.source === 'upload'" {
                    button.dropzone type="button"
                        data-class:has-file="$create.upload.name"
                        data-on:click="document.getElementById('create-file').click()"
                        data-on:dragover="evt.preventDefault(); el.classList.add('dragging')"
                        data-on:dragleave="el.classList.remove('dragging')"
                        data-on:drop="evt.preventDefault(); el.classList.remove('dragging'); const i = document.getElementById('create-file'); i.files = evt.dataTransfer.files; i.dispatchEvent(new Event('change'))" {
                        span data-show="!$create.upload.name" { (icon("upload", 28)) }
                        span data-show="$create.upload.name" { (icon("file", 28)) }
                        strong data-show="!$create.upload.name" { "Drop a workflow spec here, or click to choose" }
                        span.muted data-show="!$create.upload.name" { "From this computer · YAML, JSON, JSON5, or KDL" }
                        strong data-show="$create.upload.name" data-text="$create.upload.name" {}
                        span.muted data-show="$create.upload.name"
                            data-text="($create.upload.size < 1024 ? $create.upload.size + ' B' : ($create.upload.size / 1024).toFixed(1) + ' KB') + ' · click or drop to replace'" {}
                    }
                    input #create-file type="file" accept=".yaml,.yml,.json,.json5,.kdl" hidden data-on:change=(READ_FILE);
                    div.upload-preview data-show="$create.upload.name" {
                        div.row {
                            strong { "Contents" }
                            span.grow {}
                            button.btn.btn-sm.btn-ghost type="button"
                                data-on:click="$create.inline = $create.upload.content; $create.inlineFormat = $create.upload.ext === '.yml' ? '.yaml' : $create.upload.ext; $create.source = 'inline'; @post('/ui/create/detect')" {
                                "Edit as pasted spec"
                            }
                            button.btn.btn-sm.btn-ghost type="button" data-on:click="@post('/ui/create/to-editor?from=upload')" { "Open in spec editor" }
                            button.btn.btn-sm.btn-ghost type="button"
                                data-on:click="$create.upload = ''; document.getElementById('create-file').value = ''" { "Clear" }
                        }
                        pre.code-block data-text="$create.upload.content" {}
                    }
                }

                form.panel.field data-panel="path" data-show="$create.source === 'path'" style="display:none"
                    data-on:submit="evt.preventDefault(); document.getElementById('create-primary').click()" {
                    label for="create-spec-path" { "Spec file path" }
                    input #create-spec-path.input.mono data-bind="create.path" placeholder="/path/to/workflow.yaml" autocomplete="off";
                    span.hint { "A path on the machine running torc-dash (relative paths resolve from its working directory)." }
                }

                div.panel data-panel="inline" data-show="$create.source === 'inline'" style="display:none" {
                    div.field.grow-field {
                        div.row {
                            label for="create-spec-text" { "Workflow spec" }
                            span.grow {}
                            select.select.sm aria-label="Spec format" data-bind="create.inlineFormat" {
                                @for (ext, label) in FORMATS {
                                    @if ext == "auto" { (auto_option(&f.inline)) } @else { option value=(ext) { (label) } }
                                }
                            }
                        }
                        textarea #create-spec-text.input.editor data-bind="create.inline" spellcheck="false"
                            placeholder="name: my-workflow\njobs:\n  - name: hello\n    command: echo \"hello\""
                            data-on:keydown=(editor::TAB_KEY)
                            "data-on:input__debounce.400ms"="@post('/ui/create/detect')" {}
                    }
                }

                div.panel data-panel="commands" data-show="$create.source === 'commands'" style="display:none" {
                    label.field {
                        "Workflow name"
                        input.input data-bind="create.commandsName" placeholder="commands" autocomplete="off";
                    }
                    div.field.grow-field {
                        label for="create-commands" { "Commands, one per line" }
                        textarea #create-commands.input.editor data-bind="create.commands" spellcheck="false"
                            placeholder="python train.py --seed 1\npython train.py --seed 2\n# Lines starting with # are skipped" {}
                        span.hint {
                            "Each line becomes an independent job (job_001, job_002, …) with default resources. Use the wizard or a spec for dependencies and resource requirements."
                        }
                    }
                }

                div.panel data-panel="wizard" data-show="$create.source === 'wizard'" style="display:none" {
                    (wiz::render(&f.w, f.slurm()))
                }
            }

            div #create-report {}
            div #create-error {}

            fieldset.options {
                legend { "Options" }
                label.check {
                    input type="checkbox" data-bind="create.initialize";
                    "Initialize workflow after creation"
                }
                @if !f.profiles.is_empty() {
                    // Slurm generation needs resource requirements, which Commands doesn't define.
                    div.slurm data-show="$create.source !== 'commands'"
                        data-on:change="$create.source === 'wizard' && @post('/ui/create/render')" {
                        div.row {
                            label.check data-class:disabled="!$create.profile" {
                                input type="checkbox" data-bind="create.useSlurm" data-attr:disabled="!$create.profile";
                                "Generate Slurm schedulers for"
                            }
                            select.select.sm aria-label="HPC profile" data-bind="create.profile" {
                                @for p in &f.profiles {
                                    option value=(p.name) { (p.display_name) @if p.is_detected { " (this system)" } }
                                }
                            }
                        }
                        @if f.detected.is_empty() {
                            span.faint.small { "No HPC system detected here; generation uses the chosen profile's partitions." }
                        }
                        div.slurm data-show="$create.useSlurm" {
                            label.field.account data-class:invalid="$create.accountError && !$create.account.trim()" {
                                span { "Slurm account " span.req { "*" } }
                                input #create-slurm-account.input data-bind="create.account" placeholder="e.g. myproject"
                                    data-on:input="$create.accountError = false; document.getElementById('create-plan').replaceChildren()";
                                span.err role="alert" data-show="$create.accountError && !$create.account.trim()" { "Enter the Slurm account to charge" }
                                span.hint data-show="!($create.accountError && !$create.account.trim())" { "Used for all generated job submissions." }
                            }
                            div.row {
                                button.btn.btn-sm type="button" data-indicator="_planning" data-attr:disabled="$_planning"
                                    data-on:click="@post('/ui/create/plan')" {
                                    span.spinner.small data-show="$_planning" style="display:none" {}
                                    " Preview allocation plan"
                                }
                            }
                            div #create-plan {}
                        }
                    }
                }
            }
        }
    }
}

/// The "Auto-detect" format option, naming what the pasted text looks like.
fn auto_option(text: &str) -> Markup {
    let detected = (!text.trim().is_empty()).then(|| detect_format(text));
    let label = detected.and_then(|d| FORMATS.iter().find(|f| f.0 == d).map(|f| f.1));
    html! {
        option #create-auto-opt value="auto" {
            "Auto-detect" @if let Some(l) = label { " (" (l) ")" }
        }
    }
}

fn footer() -> Markup {
    let mac = "navigator.platform.includes('Mac') ? '⌘' : 'Ctrl'";
    html! {
        div.create-foot {
            button #create-back.btn type="button" data-show="$create.source === 'wizard'"
                data-attr:disabled="$create.w.step <= 1" data-on:click="@post('/ui/create/wiz?op=back')" { "Back" }
            span.faint.keys data-show="$create.source === 'wizard'" { kbd { "[" } " " kbd { "]" } " steps" }
            button.btn.btn-ghost type="button" data-show=(format!("$create.source === 'wizard' && $create.w.step === {LAST}"))
                data-on:click="@post('/ui/create/to-editor?from=wizard')" { "Edit as YAML" }
            span.grow {}
            span.faint.keys { kbd data-text=(mac) { "Ctrl" } kbd { "↵" } }
            button.btn type="button" data-indicator="_validating" data-attr:disabled="$_validating || $_creating"
                data-on:click="@post('/ui/create/validate')" {
                span.spinner.small data-show="$_validating" style="display:none" {}
                " Validate"
            }
            (kit::cancel_button())
            button #create-primary.btn.btn-primary type="button" data-indicator="_creating" data-attr:disabled="$_creating"
                data-on:click="$create.source === 'wizard' ? @post('/ui/create/wiz?op=next') : @post('/ui/create/create')" {
                span data-show="$_creating" style="display:none" { span.spinner.small {} " Creating…" }
                span data-show=(format!("!$_creating && $create.source === 'wizard' && $create.w.step < {LAST}")) style="display:none" {
                    "Next " (icon("chevronRight", 14))
                }
                span data-show=(format!("!$_creating && ($create.source !== 'wizard' || $create.w.step >= {LAST})")) {
                    (icon("plus", 14)) " Create"
                }
            }
        }
    }
}

// ============== Handlers ==============

/// Re-render the wizard (live preview, card summaries, errors) after an input changed.
async fn render(Sig(s): Sig<Sigs>) -> Ui {
    Ui::new().patch(wiz::render(&s.create.w, s.create.slurm()))
}

#[derive(Deserialize)]
struct WizQuery {
    op: String,
    #[serde(default)]
    id: i64,
    #[serde(default)]
    other: i64,
    #[serde(default)]
    key: String,
}

/// A wizard edit (add/remove items, presets, chips, navigation): apply it to `$create.w` and
/// send back the new state and the re-rendered wizard.
async fn wiz_op(
    State(st): State<Arc<AppState>>,
    Query(q): Query<WizQuery>,
    Sig(s): Sig<Sigs>,
) -> Ui {
    let f = s.create;
    if q.op == "next" && f.w.step >= LAST {
        return create_workflow(&st, f).await;
    }
    let mut w = f.w.clone();
    let out = wiz::apply(&mut w, &q.op, q.id, q.other, &q.key);
    // The patch may remove the element that sent this request (a card's remove button), so it
    // goes last; scripts wait a moment, so they still see the new wizard.
    let mut ui = Ui::new().signals(json!({ "create": { "w": wiz::patch(&f.w, &w) } }));
    if let Some(msg) = out.info {
        ui = ui.toast(Tone::Info, msg);
    }
    if let Some(js) = out.script {
        ui = ui.script(js);
    }
    ui.patch(wiz::render_opening(&w, f.slurm(), out.opened))
}

/// Dry run of the wizard's spec on the review step.
async fn wiz_validate(State(st): State<Arc<AppState>>, Sig(s): Sig<Sigs>) -> Ui {
    let body = json!({
        "spec": serde_json::to_string_pretty(&wiz::generate(&s.create.w)).unwrap_or_default(),
        "is_file": false,
        "file_extension": ".json",
    });
    let r = validate_spec(&st, body).await;
    Ui::new().inner("#create-wiz-report", report_panel(&r, &plain))
}

/// Update the "Auto-detect (…)" label for the pasted spec.
async fn detect(Sig(s): Sig<Sigs>) -> Ui {
    Ui::new().patch(auto_option(&s.create.inline))
}

/// Before validating, planning, or creating from the wizard: jump to the first invalid step.
fn wizard_invalid(f: &Form) -> Option<Ui> {
    if f.source != "wizard" {
        return None;
    }
    let mut w = f.w.clone();
    let out = wiz::ensure_valid(&mut w)?;
    Some(
        Ui::new()
            .signals(json!({ "create": { "w": wiz::patch(&f.w, &w) } }))
            .script(out)
            .patch(wiz::render(&w, f.slurm())),
    )
}

async fn validate(State(st): State<Arc<AppState>>, Sig(s): Sig<Sigs>) -> Ui {
    let f = s.create;
    if let Some(ui) = wizard_invalid(&f) {
        return ui;
    }
    match f.spec_body() {
        Ok(body) => {
            let r = validate_spec(&st, body).await;
            Ui::new().inner("#create-report", report_panel(&r, &plain))
        }
        Err(msg) => Ui::new().toast(Tone::Info, msg),
    }
}

/// Recommend Slurm allocations for the spec (`torc slurm plan-allocations`).
async fn plan(State(st): State<Arc<AppState>>, Sig(s): Sig<Sigs>) -> Ui {
    let f = s.create;
    if let Some(ui) = wizard_invalid(&f) {
        return ui;
    }
    let body = match f.spec_body() {
        Ok(b) => b,
        Err(msg) => return Ui::new().toast(Tone::Info, msg),
    };
    let mut params = if body["is_file"] == json!(true) {
        json!({ "spec_path": body["spec"] })
    } else {
        json!({ "spec": body["spec"], "file_extension": body["file_extension"] })
    };
    if !f.account.trim().is_empty() {
        params["account"] = json!(f.account.trim());
    }
    if !f.profile.is_empty() {
        params["profile"] = json!(f.profile);
    }
    // Live sinfo/squeue only make sense on the cluster the profile describes.
    params["offline"] = json!(f.profile != f.detected);
    let html = match kit::op(&st, "plan-allocations", params).await {
        Ok(p) => allocation_plan(&p),
        Err(e) => html! { span.error-text.small { (e) } },
    };
    Ui::new().inner("#create-plan", html)
}

/// Result of `torc slurm plan-allocations`: recommended allocations per job group.
fn allocation_plan(p: &Value) -> Markup {
    let s = |v: &Value| match v {
        Value::Null => "?".to_string(),
        Value::String(s) => s.clone(),
        v => v.to_string(),
    };
    let groups = p["resource_groups"].as_array().cloned().unwrap_or_default();
    let partition = |r: &Value| -> String {
        if let Some(p) = r["partition"].as_str() {
            return p.into();
        }
        groups
            .iter()
            .find(|g| g["name"] == r["group_name"])
            .and_then(|g| g["partition"].as_str())
            .unwrap_or("auto")
            .into()
    };
    let a = &p["workflow_analysis"];
    let recs = p["recommendations"].as_array().cloned().unwrap_or_default();
    html! {
        div.plan {
            @if a.is_object() {
                p.muted {
                    (s(&a["total_instances"])) " job(s), dependency depth " (s(&a["dependency_depth"]))
                    ", up to " (s(&a["max_parallelism"])) " in parallel"
                    @if let Some(n) = p["profile_display_name"].as_str() { " · " (n) }
                }
            }
            @if !recs.is_empty() {
                table {
                    thead { tr { th { "Jobs" } th { "Partition" } th { "Allocations" } th { "Nodes each" } th { "Walltime" } } }
                    tbody {
                        @for r in &recs {
                            tr title=(r["reason"].as_str().unwrap_or("")) {
                                td { (s(&r["group_name"])) " " span.muted { "(" (s(&r["job_count"])) ")" } }
                                td.mono { (partition(r)) }
                                td { (s(&r["num_allocations"])) }
                                td { (s(&r["nodes_per_allocation"])) }
                                td.mono { (r["walltime"].as_str().unwrap_or("—")) }
                            }
                        }
                    }
                }
            }
            @for w in p["warnings"].as_array().into_iter().flatten() {
                p.warn { (w.as_str().unwrap_or_default()) }
            }
        }
    }
}

async fn create(State(st): State<Arc<AppState>>, Sig(s): Sig<Sigs>) -> Ui {
    create_workflow(&st, s.create).await
}

/// Create the workflow from the current source; on success close the modal, select the new
/// workflow, and initialize it if asked.
async fn create_workflow(st: &Arc<AppState>, f: Form) -> Ui {
    if let Some(ui) = wizard_invalid(&f) {
        return ui;
    }
    let mut body = match f.spec_body() {
        Ok(b) => b,
        Err(msg) => return Ui::new().toast(Tone::Info, msg),
    };
    let r = match f.slurm() {
        Some(("", _)) => {
            return Ui::new()
                .signals(json!({ "create": { "accountError": true } }))
                .script("document.getElementById('create-slurm-account')?.focus()");
        }
        Some((account, profile)) => {
            body["account"] = json!(account);
            body["profile"] = json!(profile);
            kit::call(st, crate::cli_create_slurm_handler, body).await
        }
        None => kit::call(st, crate::cli_create_handler, body).await,
    }
    .and_then(kit::cli_result);
    match r {
        Ok(stdout) => {
            let id = crate::extract_workflow_id(&stdout).and_then(|i| i.parse::<i64>().ok());
            let clear = json!({ "create": {
                "upload": "", "path": "", "inline": "", "commands": "", "commandsName": "",
            }});
            created(Ui::new().signals(clear), id, f.initialize)
        }
        Err(e) => Ui::new()
            .inner(
                "#create-error",
                html! { div.create-error role="alert" { strong { "Create failed" } pre { (e) } } },
            )
            .err("Failed to create workflow"),
    }
}

/// After a create: toast, close the dialog, select the workflow, refresh, and initialize it
/// (through the Workflows tab's initialize, which first checks for existing output files).
/// The dialog patch goes last: it may remove the button that sent the request, after which
/// Datastar drops the rest of the response.
pub fn created(ui: Ui, id: Option<i64>, initialize: bool) -> Ui {
    let Some(id) = id else {
        return ui.ok("Workflow created").refresh().close_modal();
    };
    tracing::info!("Created workflow_id={id} from the dashboard");
    let slot = if initialize {
        html! { div hidden data-init=(format!("@post('/ui/wf/{id}/initialize')")) {} }
    } else {
        html! {}
    };
    ui.ok(format!("Created workflow {id}"))
        .signals(json!({ "wf": id }))
        .refresh()
        .modal(slot)
}

#[derive(Deserialize)]
struct ToEditorQuery {
    from: String,
}

/// Hand the uploaded spec, or the wizard's spec as YAML, to the spec editor.
async fn to_editor(Query(q): Query<ToEditorQuery>, Sig(s): Sig<Sigs>) -> Ui {
    let f = s.create;
    let (text, format) = if q.from == "wizard" {
        (
            serde_yaml::to_string(&wiz::generate(&f.w)).unwrap_or_default(),
            "yaml",
        )
    } else {
        let Some(u) = f.upload else {
            return Ui::new();
        };
        (u.content, editor::format_of(&u.ext))
    };
    editor::open_in_editor(
        &s.editor,
        text,
        String::new(),
        format,
        Some("@get('/ui/create/open')"),
    )
}

/// Escape text for HTML (for the hand-built highlighted spec).
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escape JSON and wrap its tokens in spans (`.hl` in app.css): .k key, .s string, .n number,
/// .b bool/null, .p punctuation.
pub fn highlight_json(json: &str) -> Markup {
    static TOKEN: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(
            r#"("(?:\\.|[^"\\])*")(\s*:)?|\b(true|false|null)\b|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)|([{}\[\]])"#,
        )
        .unwrap()
    });
    let mut out = String::new();
    let mut last = 0;
    for c in TOKEN.captures_iter(json) {
        let m = c.get(0).unwrap();
        out.push_str(&esc(&json[last..m.start()]));
        if let Some(s) = c.get(1) {
            match c.get(2) {
                Some(colon) => out.push_str(&format!(
                    "<span class=\"k\">{}</span>{}",
                    esc(s.as_str()),
                    colon.as_str()
                )),
                None => out.push_str(&format!("<span class=\"s\">{}</span>", esc(s.as_str()))),
            }
        } else if let Some(b) = c.get(3) {
            out.push_str(&format!("<span class=\"b\">{}</span>", b.as_str()));
        } else if let Some(n) = c.get(4) {
            out.push_str(&format!("<span class=\"n\">{}</span>", n.as_str()));
        } else {
            out.push_str(&format!("<span class=\"p\">{}</span>", m.as_str()));
        }
        last = m.end();
    }
    out.push_str(&esc(&json[last..]));
    maud::PreEscaped(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_and_detect() {
        let s = commands_spec("a\n# skip\n\n b \n", "").unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["name"], "commands");
        assert_eq!(v["jobs"][1]["name"], "job_002");
        assert_eq!(v["jobs"][1]["command"], "b");
        assert!(commands_spec("# only\n", "x").is_none());
        assert_eq!(detect_format("{\"a\": 1}"), ".json");
        assert_eq!(detect_format("{a: 1}"), ".json5");
        assert_eq!(detect_format("name \"x\"\njob \"a\" {}"), ".kdl");
        assert_eq!(detect_format("name: x\n"), ".yaml");
    }
}
