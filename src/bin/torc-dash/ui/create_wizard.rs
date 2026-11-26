//! The create-workflow wizard: its data model (the `$create.w` signals), workflow-spec
//! generation, per-step validation, and rendering. Jobs, schedulers, and actions are keyed
//! objects (`j3`, `s4`, `a5`) so a removed item can be deleted from the signals with `null`.
//!
//! Inputs bind to signals and carry no `value` attribute, so re-rendering the wizard (after
//! every edit, for the live preview) never overwrites what is being typed.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use maud::{Markup, html};
use regex::Regex;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Value, json};

use super::highlight_json;
use crate::ui::kit::{icon, js};

pub const STEPS: [&str; 6] = [
    "Basics",
    "Jobs",
    "Schedulers",
    "Actions",
    "Monitoring",
    "Review",
];
pub const LAST: usize = STEPS.len();

/// (key, label, cpus, memory, gpus)
const PRESETS: [(&str, &str, i64, &str, i64); 5] = [
    ("small", "Small", 1, "1g", 0),
    ("medium", "Medium", 4, "10g", 0),
    ("large", "Large", 8, "50g", 0),
    ("gpu", "GPU", 1, "10g", 1),
    ("custom", "Custom", 1, "1g", 0),
];

const TRIGGERS: [(&str, &str); 3] = [
    ("on_workflow_start", "When workflow starts"),
    ("on_jobs_ready", "When jobs become ready"),
    ("on_jobs_complete", "When jobs complete"),
];

/// Focus the first control of the current step and scroll the dialog to the top.
pub const FOCUS_FIRST: &str = "setTimeout(() => { const s = document.getElementById('create-wiz-step'); s?.closest('.body')?.scrollTo({top: 0}); s?.querySelector('input:not([type=checkbox]):not([type=radio]):not(:disabled), textarea, select, .empty-add')?.focus() }, 30)";

/// Reveal and focus the first visible field error.
const FOCUS_ERROR: &str = "setTimeout(() => { const e = document.querySelector('#create-wizard [role=alert]'); const d = e?.closest('details'); if (d) d.open = true; e?.closest('.field')?.querySelector('input, select, button')?.focus() }, 30)";

// ============== Model ==============

/// A number field as the input left it (a number, or text while being edited), passed back
/// unchanged so the signal keeps what the user typed.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(transparent)]
pub struct Num(Value);

impl Num {
    fn new(n: i64) -> Self {
        Num(json!(n))
    }
    /// The value as a number; NaN when it isn't one (so `!(x >= 1)` checks fail like JS).
    fn f(&self) -> f64 {
        match &self.0 {
            Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
            Value::String(s) if !s.trim().is_empty() => s.trim().parse().unwrap_or(f64::NAN),
            _ => f64::NAN,
        }
    }
    /// A number no less than `min` (false when not a number).
    fn at_least(&self, min: f64) -> bool {
        self.f() >= min
    }
    /// For the generated spec: an integer when whole, else the number, else `fallback`.
    fn json(&self, fallback: i64) -> Value {
        let f = self.f();
        if !f.is_finite() || f == 0.0 && fallback != 0 {
            json!(fallback)
        } else if f.fract() == 0.0 {
            json!(f as i64)
        } else {
            json!(f)
        }
    }
}

impl Default for Num {
    fn default() -> Self {
        Num::new(0)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(default)]
pub struct Wiz {
    pub step: usize,
    name: String,
    description: String,
    strategy: String,
    jobs: BTreeMap<String, Job>,
    scheds: BTreeMap<String, Sched>,
    actions: BTreeMap<String, Act>,
    monitor: Monitor,
    next: i64,
    /// Steps the user tried to leave; their "required" errors become visible.
    attempted: BTreeMap<String, bool>,
    preview: bool,
}

impl Default for Wiz {
    fn default() -> Self {
        Wiz {
            step: 1,
            name: String::new(),
            description: String::new(),
            strategy: "resource_aware".into(),
            jobs: BTreeMap::new(),
            scheds: BTreeMap::new(),
            actions: BTreeMap::new(),
            monitor: Monitor::default(),
            next: 1,
            attempted: BTreeMap::new(),
            preview: true,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(default)]
struct Monitor {
    enabled: bool,
    granularity: String,
    sample_interval_seconds: Num,
}

impl Default for Monitor {
    fn default() -> Self {
        Monitor {
            enabled: true,
            granularity: "summary".into(),
            sample_interval_seconds: Num::new(10),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(default)]
struct Job {
    id: i64,
    name: String,
    command: String,
    depends_on: Vec<i64>,
    resource_preset: String,
    num_cpus: Num,
    memory: String,
    num_gpus: Num,
    runtime: String,
    parameters: String,
    parameter_mode: String,
    /// Id of a wizard scheduler as text ("" = auto, from a `<select>`).
    scheduler: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(default)]
struct Sched {
    id: i64,
    name: String,
    account: String,
    nodes: Num,
    walltime: String,
    partition: String,
    qos: String,
    gres: String,
    mem: String,
    tmp: String,
    extra: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(default)]
struct Act {
    id: i64,
    trigger_type: String,
    /// Id of a wizard scheduler as text ("" = none).
    scheduler: String,
    jobs: Vec<i64>,
    num_allocations: Num,
    max_parallel_jobs: Num,
    start_one_worker_per_node: bool,
}

fn new_job(id: i64) -> Job {
    Job {
        id,
        resource_preset: "small".into(),
        num_cpus: Num::new(1),
        memory: "1g".into(),
        num_gpus: Num::new(0),
        runtime: "PT1H".into(),
        parameter_mode: "product".into(),
        ..Default::default()
    }
}

fn new_sched(id: i64, name: String) -> Sched {
    Sched {
        id,
        name,
        nodes: Num::new(1),
        walltime: "01:00:00".into(),
        ..Default::default()
    }
}

fn new_action(id: i64) -> Act {
    Act {
        id,
        trigger_type: "on_workflow_start".into(),
        num_allocations: Num::new(1),
        max_parallel_jobs: Num::new(10),
        ..Default::default()
    }
}

fn sorted<T: Clone>(m: &BTreeMap<String, T>, id: impl Fn(&T) -> i64) -> Vec<T> {
    let mut v: Vec<T> = m.values().cloned().collect();
    v.sort_by_key(|x| id(x));
    v
}

impl Wiz {
    fn jobs(&self) -> Vec<Job> {
        sorted(&self.jobs, |j| j.id)
    }
    fn scheds(&self) -> Vec<Sched> {
        sorted(&self.scheds, |s| s.id)
    }
    fn actions(&self) -> Vec<Act> {
        sorted(&self.actions, |a| a.id)
    }
    fn job(&self, id: i64) -> Option<&Job> {
        self.jobs.get(&format!("j{id}"))
    }
    /// Name of the scheduler with this id ("" if none or unnamed).
    fn sched_name(&self, id: &str) -> String {
        self.scheds
            .get(&format!("s{id}"))
            .map(|s| s.name.trim().to_string())
            .unwrap_or_default()
    }
    fn resource_aware(&self) -> bool {
        self.strategy != "queue_depth"
    }
    fn take_id(&mut self) -> i64 {
        let id = self.next.max(1);
        self.next = id + 1;
        id
    }
}

fn job_label(j: &Job) -> String {
    let n = j.name.trim();
    if n.is_empty() {
        "Untitled job".into()
    } else {
        n.into()
    }
}

fn is_job_triggered(t: &str) -> bool {
    t != "on_workflow_start"
}

/// The signals patch turning `old` into `new`: the whole new state, plus `null` for removed
/// items (a merge patch would otherwise keep them).
pub fn patch(old: &Wiz, new: &Wiz) -> Value {
    let mut v = serde_json::to_value(new).unwrap_or_default();
    for (coll, keys) in [
        ("jobs", old.jobs.keys().collect::<Vec<_>>()),
        ("scheds", old.scheds.keys().collect()),
        ("actions", old.actions.keys().collect()),
    ] {
        for k in keys {
            if v[coll].get(k).is_none() {
                v[coll][k] = Value::Null;
            }
        }
    }
    v
}

// ============== Parameters ==============

/// Parse the wizard's compact parameter syntax into the spec's string-valued form:
/// `i: 1:10`, `x: [1,2,3], y: [a,b]`, `name: "a, b"`. Keeps the order written.
fn parse_parameters(text: &str) -> Vec<(String, String)> {
    let s: Vec<char> = text.trim().chars().collect();
    let mut params: Vec<(String, String)> = Vec::new();
    let mut i = 0;
    let slice = |a: usize, b: usize| s[a..b.min(s.len())].iter().collect::<String>();
    while i < s.len() {
        while i < s.len() && (s[i] == ' ' || s[i] == ',') {
            i += 1;
        }
        if i >= s.len() {
            break;
        }
        let key_start = i;
        while i < s.len() && (s[i].is_alphanumeric() || s[i] == '_') {
            i += 1;
        }
        let key = slice(key_start, i);
        if key.is_empty() {
            break;
        }
        while i < s.len() && (s[i] == ' ' || s[i] == ':') {
            i += 1;
        }
        let value = if i < s.len() && (s[i] == '"' || s[i] == '\'') {
            let quote = s[i];
            i += 1;
            let start = i;
            while i < s.len() && s[i] != quote {
                i += 1;
            }
            let v = slice(start, i);
            i += 1;
            v
        } else if i < s.len() && s[i] == '[' {
            let start = i;
            let mut depth = 0;
            while i < s.len() {
                if s[i] == '[' {
                    depth += 1;
                } else if s[i] == ']' {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                i += 1;
            }
            slice(start, i)
        } else {
            let start = i;
            while i < s.len() && s[i] != ',' {
                i += 1;
            }
            slice(start, i).trim().to_string()
        };
        if !value.is_empty() {
            match params.iter_mut().find(|(k, _)| *k == key) {
                Some(p) => p.1 = value,
                None => params.push((key, value)),
            }
        }
    }
    params
}

/// Number of values one parameter expands to, or None if it can't be determined cheaply.
fn value_count(v: &str) -> Option<usize> {
    static RANGE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(-?\d+(?:\.\d+)?):(-?\d+(?:\.\d+)?)(?::(-?\d+(?:\.\d+)?))?$").unwrap()
    });
    let t = v.trim();
    if let Some(inner) = t.strip_prefix('[') {
        let inner = inner.strip_suffix(']').unwrap_or(inner).trim();
        return Some(if inner.is_empty() {
            0
        } else {
            inner.split(',').count()
        });
    }
    if let Some(c) = RANGE.captures(t) {
        let n = |i: usize| c.get(i).and_then(|m| m.as_str().parse::<f64>().ok());
        let (a, b, step) = (n(1)?, n(2)?, n(3).unwrap_or(1.0));
        if step <= 0.0 || b < a {
            return None;
        }
        return Some(((b - a) / step + 1e-9).floor() as usize + 1);
    }
    Some(1)
}

/// How many jobs a parameterized job expands to (None if unknown).
fn expanded_count(j: &Job) -> Option<usize> {
    let counts: Vec<Option<usize>> = parse_parameters(&j.parameters)
        .iter()
        .map(|(_, v)| value_count(v))
        .collect();
    if counts.is_empty() {
        return Some(1);
    }
    let nums: Option<Vec<usize>> = counts.into_iter().collect();
    let nums = nums?;
    Some(if j.parameter_mode == "zip" {
        nums.into_iter().max().unwrap_or(0)
    } else {
        nums.into_iter().product()
    })
}

fn is_parameterized(j: &Job) -> bool {
    !j.parameters.trim().is_empty()
}

/// Regex matching every expansion of a templated job name like `work_{i}`.
fn job_name_to_regex(name: &str) -> String {
    static TEMPLATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\\{[^}]+\\\}").unwrap());
    format!("^{}$", TEMPLATE.replace_all(&regex::escape(name), ".*"))
}

// ============== Spec generation ==============

/// A map that serializes in insertion order.
struct Ordered(Vec<(String, String)>);

impl Serialize for Ordered {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}

fn none_if_empty<T>(v: Vec<T>) -> Option<Vec<T>> {
    (!v.is_empty()).then_some(v)
}

#[derive(Serialize)]
pub struct Spec {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    jobs: Vec<SpecJob>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resource_requirements: Option<Vec<SpecRes>>,
    resource_monitor: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    slurm_schedulers: Option<Vec<SpecSched>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    actions: Option<Vec<SpecAction>>,
}

#[derive(Serialize)]
struct SpecJob {
    name: String,
    command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    depends_on: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    depends_on_regexes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resource_requirements: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduler: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<Ordered>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameter_mode: Option<&'static str>,
}

#[derive(Serialize, Clone)]
struct SpecRes {
    name: String,
    num_cpus: Value,
    memory: String,
    num_gpus: Value,
    num_nodes: i64,
    runtime: String,
}

#[derive(Serialize)]
struct SpecSched {
    name: String,
    account: String,
    nodes: Value,
    walltime: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    partition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    qos: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gres: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mem: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tmp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra: Option<String>,
}

#[derive(Serialize)]
struct SpecAction {
    trigger_type: String,
    action_type: &'static str,
    scheduler: String,
    scheduler_type: &'static str,
    num_allocations: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    jobs: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    job_name_regexes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_parallel_jobs: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_one_worker_per_node: Option<bool>,
}

fn opt(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// The workflow spec the wizard describes.
pub fn generate(w: &Wiz) -> Spec {
    let resource_aware = w.resource_aware();
    let job_name = |j: &Job| opt(&j.name).unwrap_or_else(|| format!("job_{}", j.id));
    // Job references split into plain names and regexes (for parameterized jobs).
    let refs = |ids: &[i64]| {
        let (mut names, mut regexes) = (Vec::new(), Vec::new());
        for j in ids.iter().filter_map(|id| w.job(*id)) {
            if is_parameterized(j) {
                regexes.push(job_name_to_regex(&job_name(j)));
            } else {
                names.push(job_name(j));
            }
        }
        (names, regexes)
    };
    let runtime = |j: &Job| opt(&j.runtime).unwrap_or_else(|| "PT1H".into());
    let res_key = |j: &Job| {
        format!(
            "{}_{}_{}_{}",
            j.num_cpus.json(0),
            j.memory,
            j.num_gpus.json(0),
            runtime(j)
        )
    };
    let jobs = w.jobs();
    let mut resources: Vec<(String, SpecRes)> = Vec::new();
    if resource_aware {
        for j in &jobs {
            if resources.iter().any(|(k, _)| *k == res_key(j)) {
                continue;
            }
            let gpus = j.num_gpus.json(0);
            let gpu = if gpus.as_f64().unwrap_or(0.0) > 0.0 {
                format!("_{gpus}gpu")
            } else {
                String::new()
            };
            resources.push((
                res_key(j),
                SpecRes {
                    name: format!(
                        "res_{}cpu_{}{gpu}_{}",
                        j.num_cpus.json(0),
                        j.memory,
                        runtime(j)
                    ),
                    num_cpus: j.num_cpus.json(0),
                    memory: j.memory.clone(),
                    num_gpus: gpus,
                    num_nodes: 1,
                    runtime: runtime(j),
                },
            ));
        }
    }

    let spec_jobs = jobs
        .iter()
        .map(|j| {
            let (names, regexes) = refs(&j.depends_on);
            let params = parse_parameters(&j.parameters);
            SpecJob {
                name: job_name(j),
                command: opt(&j.command).unwrap_or_else(|| "echo \"TODO\"".into()),
                depends_on: none_if_empty(names),
                depends_on_regexes: none_if_empty(regexes),
                resource_requirements: resource_aware
                    .then(|| {
                        resources
                            .iter()
                            .find(|(k, _)| *k == res_key(j))
                            .map(|(_, r)| r.name.clone())
                    })
                    .flatten(),
                scheduler: opt(&w.sched_name(&j.scheduler)),
                parameter_mode: (!params.is_empty() && j.parameter_mode == "zip").then_some("zip"),
                parameters: (!params.is_empty()).then_some(Ordered(params)),
            }
        })
        .collect();

    let slurm: Vec<SpecSched> = w
        .scheds()
        .iter()
        .filter(|s| !s.name.trim().is_empty() && !s.account.trim().is_empty())
        .map(|s| SpecSched {
            name: s.name.trim().into(),
            account: s.account.trim().into(),
            nodes: s.nodes.json(1),
            walltime: opt(&s.walltime).unwrap_or_else(|| "01:00:00".into()),
            partition: opt(&s.partition),
            qos: opt(&s.qos),
            gres: opt(&s.gres),
            mem: opt(&s.mem),
            tmp: opt(&s.tmp),
            extra: opt(&s.extra),
        })
        .collect();

    let actions: Vec<SpecAction> = w
        .actions()
        .iter()
        .filter(|a| !w.sched_name(&a.scheduler).is_empty())
        .map(|a| {
            let (names, regexes) = if is_job_triggered(&a.trigger_type) {
                refs(&a.jobs)
            } else {
                (vec![], vec![])
            };
            let max_par = a.max_parallel_jobs.f();
            SpecAction {
                trigger_type: a.trigger_type.clone(),
                action_type: "schedule_nodes",
                scheduler: w.sched_name(&a.scheduler),
                scheduler_type: "slurm",
                num_allocations: a.num_allocations.json(1),
                jobs: none_if_empty(names),
                job_name_regexes: none_if_empty(regexes),
                max_parallel_jobs: (!resource_aware && max_par.is_finite() && max_par != 0.0)
                    .then(|| a.max_parallel_jobs.json(0)),
                start_one_worker_per_node: a.start_one_worker_per_node.then_some(true),
            }
        })
        .collect();

    Spec {
        name: opt(&w.name).unwrap_or_else(|| "untitled-workflow".into()),
        description: opt(&w.description),
        jobs: spec_jobs,
        resource_requirements: none_if_empty(resources.into_iter().map(|(_, r)| r).collect()),
        resource_monitor: if w.monitor.enabled {
            json!({
                "enabled": true,
                "granularity": w.monitor.granularity,
                "sample_interval_seconds": w.monitor.sample_interval_seconds.json(10),
            })
        } else {
            json!({ "enabled": false })
        },
        slurm_schedulers: none_if_empty(slurm),
        actions: none_if_empty(actions),
    }
}

// ============== Validation ==============

static MEMORY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d+(\.\d+)?\s*[kmgtKMGT]?[bB]?$").unwrap());
static RUNTIME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^P(\d+D)?(T(\d+H)?(\d+M)?(\d+(\.\d+)?S)?)?$").unwrap());
static WALLTIME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+-)?\d{1,3}:\d{2}:\d{2}$").unwrap());

/// ISO8601 duration: something after P, and T followed by a time.
fn valid_runtime(s: &str) -> bool {
    RUNTIME_RE.is_match(s) && s != "P" && !s.ends_with('T')
}

/// Ids of items sharing a (case-insensitive) name.
fn duplicates<'a>(items: impl Iterator<Item = (i64, &'a str)>) -> Vec<i64> {
    let mut seen: BTreeMap<String, i64> = BTreeMap::new();
    let mut dups = Vec::new();
    for (id, name) in items {
        let key = name.trim().to_lowercase();
        if key.is_empty() {
            continue;
        }
        match seen.get(&key) {
            Some(first) => dups.extend([id, *first]),
            None => {
                seen.insert(key, id);
            }
        }
    }
    dups
}

/// Errors that block leaving `step`, keyed like `job:3:name`, plus `step` for step-level ones.
fn step_errors(w: &Wiz, step: usize) -> BTreeMap<String, &'static str> {
    let mut e = BTreeMap::new();
    match step {
        1 if w.name.trim().is_empty() => {
            e.insert("name".into(), "Workflow name is required");
        }
        2 => {
            if w.jobs.is_empty() {
                e.insert("step".into(), "Add at least one job");
            }
            let jobs = w.jobs();
            let dups = duplicates(jobs.iter().map(|j| (j.id, j.name.as_str())));
            for j in &jobs {
                let k = |f: &str| format!("job:{}:{f}", j.id);
                if j.name.trim().is_empty() {
                    e.insert(k("name"), "Name is required");
                } else if dups.contains(&j.id) {
                    e.insert(k("name"), "Duplicate job name");
                }
                if j.command.trim().is_empty() {
                    e.insert(k("command"), "Command is required");
                }
                if is_parameterized(j) && parse_parameters(&j.parameters).is_empty() {
                    e.insert(
                        k("parameters"),
                        "Could not parse parameters (expected name: value)",
                    );
                }
                if w.resource_aware() {
                    if !j.num_cpus.at_least(1.0) {
                        e.insert(k("num_cpus"), "At least 1");
                    }
                    if !j.num_gpus.at_least(0.0) {
                        e.insert(k("num_gpus"), "0 or more");
                    }
                    if !MEMORY_RE.is_match(j.memory.trim()) {
                        e.insert(k("memory"), "e.g. 512m, 4g");
                    }
                    if !j.runtime.trim().is_empty() && !valid_runtime(j.runtime.trim()) {
                        e.insert(k("runtime"), "ISO8601, e.g. PT30M, PT2H, P1D");
                    }
                }
            }
        }
        3 => {
            let scheds = w.scheds();
            let dups = duplicates(scheds.iter().map(|s| (s.id, s.name.as_str())));
            for s in &scheds {
                let k = |f: &str| format!("sched:{}:{f}", s.id);
                if s.name.trim().is_empty() {
                    e.insert(k("name"), "Name is required");
                } else if dups.contains(&s.id) {
                    e.insert(k("name"), "Duplicate scheduler name");
                }
                if s.account.trim().is_empty() {
                    e.insert(k("account"), "Account is required");
                }
                if !s.nodes.at_least(1.0) {
                    e.insert(k("nodes"), "At least 1");
                }
                if !s.walltime.trim().is_empty() && !WALLTIME_RE.is_match(s.walltime.trim()) {
                    e.insert(k("walltime"), "HH:MM:SS or D-HH:MM:SS");
                }
            }
        }
        4 => {
            for a in w.actions() {
                let k = |f: &str| format!("action:{}:{f}", a.id);
                if !w.scheds.contains_key(&format!("s{}", a.scheduler)) {
                    e.insert(k("scheduler"), "Select a scheduler");
                }
                if is_job_triggered(&a.trigger_type) && a.jobs.is_empty() {
                    e.insert(k("jobs"), "Select at least one job");
                }
                if !a.num_allocations.at_least(1.0) {
                    e.insert(k("num_allocations"), "At least 1");
                }
            }
        }
        5 => {
            let n = w.monitor.sample_interval_seconds.f();
            if w.monitor.enabled && !(1.0..=300.0).contains(&n) {
                e.insert("interval".into(), "Between 1 and 300 seconds");
            }
        }
        _ => {}
    }
    e
}

/// First step with blocking errors.
fn first_invalid_step(w: &Wiz) -> Option<usize> {
    (1..LAST).find(|s| !step_errors(w, *s).is_empty())
}

/// Non-blocking hint: a parameterized job needs `{param}` in its name.
fn job_warning(j: &Job) -> Option<&'static str> {
    static PARAM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\w+(:[^}]*)?\}").unwrap());
    (is_parameterized(j) && !PARAM.is_match(&j.name))
        .then_some("Use {param} in the name so each expanded job gets a unique name")
}

fn attempted(w: &Wiz, step: usize) -> bool {
    w.attempted.get(&step.to_string()) == Some(&true)
}

/// Before creating: jump to the first invalid step and reveal its errors. Returns the script
/// that focuses the first error, or None when everything is valid.
pub fn ensure_valid(w: &mut Wiz) -> Option<&'static str> {
    let bad = first_invalid_step(w)?;
    w.attempted.insert(bad.to_string(), true);
    w.step = bad;
    Some(FOCUS_ERROR)
}

// ============== Edits ==============

#[derive(Default)]
pub struct Outcome {
    /// Item to render expanded (just added).
    pub opened: Option<i64>,
    pub info: Option<String>,
    pub script: Option<&'static str>,
}

fn toggle(v: &mut Vec<i64>, id: i64) {
    if let Some(i) = v.iter().position(|x| *x == id) {
        v.remove(i);
    } else {
        v.push(id);
    }
}

fn go_to(w: &mut Wiz, step: usize) -> &'static str {
    if step > w.step {
        // Moving forward requires every step before the target to be valid.
        for s in w.step..step {
            if !step_errors(w, s).is_empty() {
                w.attempted.insert(s.to_string(), true);
                w.step = s;
                return FOCUS_ERROR;
            }
        }
    }
    w.step = step.clamp(1, LAST);
    FOCUS_FIRST
}

/// Apply a wizard edit. `id` is the item, `other` a referenced item, `key` a preset name.
pub fn apply(w: &mut Wiz, op: &str, id: i64, other: i64, key: &str) -> Outcome {
    let mut out = Outcome::default();
    // A new card renders with `data-new`; focus its first input.
    let focus_new =
        "setTimeout(() => document.querySelector('#create-wizard [data-new] input')?.focus(), 30)";
    match op {
        "add-job" => {
            let id = w.take_id();
            w.jobs.insert(format!("j{id}"), new_job(id));
            out.opened = Some(id);
            out.script = Some(focus_new);
        }
        "rm-job" => {
            w.jobs.remove(&format!("j{id}"));
            for j in w.jobs.values_mut() {
                j.depends_on.retain(|d| *d != id);
            }
            for a in w.actions.values_mut() {
                a.jobs.retain(|d| *d != id);
            }
        }
        "preset" => {
            if let Some(j) = w.jobs.get_mut(&format!("j{id}")) {
                j.resource_preset = key.to_string();
                if let Some(p) = PRESETS.iter().find(|p| p.0 == key && key != "custom") {
                    j.num_cpus = Num::new(p.2);
                    j.memory = p.3.into();
                    j.num_gpus = Num::new(p.4);
                }
            }
        }
        "dep" => {
            if let Some(j) = w.jobs.get_mut(&format!("j{id}")) {
                toggle(&mut j.depends_on, other);
            }
        }
        "new-sched" => {
            let sid = w.take_id();
            let name = format!("scheduler-{}", w.scheds.len() + 1);
            out.info = Some(format!(
                "Scheduler \"{name}\" created. Configure it in the Schedulers step."
            ));
            w.scheds.insert(format!("s{sid}"), new_sched(sid, name));
            if let Some(j) = w.jobs.get_mut(&format!("j{id}")) {
                j.scheduler = sid.to_string();
            }
        }
        "add-sched" => {
            let id = w.take_id();
            w.scheds
                .insert(format!("s{id}"), new_sched(id, String::new()));
            out.opened = Some(id);
            out.script = Some(focus_new);
        }
        "rm-sched" => {
            w.scheds.remove(&format!("s{id}"));
            let sid = id.to_string();
            for j in w.jobs.values_mut().filter(|j| j.scheduler == sid) {
                j.scheduler.clear();
            }
            for a in w.actions.values_mut().filter(|a| a.scheduler == sid) {
                a.scheduler.clear();
            }
        }
        "add-action" => {
            let id = w.take_id();
            let mut a = new_action(id);
            let named: Vec<Sched> = w
                .scheds()
                .into_iter()
                .filter(|s| !s.name.trim().is_empty())
                .collect();
            if named.len() == 1 {
                a.scheduler = named[0].id.to_string();
            }
            w.actions.insert(format!("a{id}"), a);
            out.opened = Some(id);
        }
        "rm-action" => {
            w.actions.remove(&format!("a{id}"));
        }
        "act-job" => {
            if let Some(a) = w.actions.get_mut(&format!("a{id}")) {
                toggle(&mut a.jobs, other);
            }
        }
        "next" => {
            w.attempted.insert(w.step.to_string(), true);
            out.script = Some(if step_errors(w, w.step).is_empty() {
                go_to(w, w.step + 1)
            } else {
                FOCUS_ERROR
            });
        }
        "back" if w.step > 1 => out.script = Some(go_to(w, w.step - 1)),
        "goto" => out.script = Some(go_to(w, other as usize)),
        _ => {}
    }
    out
}

// ============== Rendering ==============

pub fn render(w: &Wiz, slurm: Option<(&str, &str)>) -> Markup {
    render_opening(w, slurm, None)
}

/// The wizard; `opened` is an item to render expanded. Cards keep their open state across
/// re-renders (`data-preserve-attr`).
pub fn render_opening(w: &Wiz, slurm: Option<(&str, &str)>, opened: Option<i64>) -> Markup {
    let errors = step_errors(w, w.step);
    // Format and duplicate errors show at once; "missing value" errors wait until the user
    // tries to leave the step, so a fresh form isn't covered in red.
    let err = |key: &str| -> Option<&'static str> {
        let msg = *errors.get(key)?;
        let missing = msg.contains("required")
            || msg.starts_with("Select")
            || msg.starts_with("Add at least");
        (!missing || attempted(w, w.step)).then_some(msg)
    };
    let spec_json = serde_json::to_string_pretty(&generate(w)).unwrap_or_default();
    let with_preview = w.preview && w.step != LAST;
    html! {
        div #create-wizard.wizard "data-on:input__debounce.300ms"="@post('/ui/create/render')" {
            ol.steps aria-label="Wizard steps" {
                @for (i, label) in STEPS.iter().enumerate() {
                    @let n = i + 1;
                    @let state = if n == w.step { "current" }
                        else if attempted(w, n) && !step_errors(w, n).is_empty() { "error" }
                        else if n < w.step { "done" } else { "todo" };
                    li.(state) {
                        button type="button" title=(format!("Step {n}: {label}"))
                            aria-current=[(state == "current").then_some("step")]
                            data-on:click=(format!("@post('/ui/create/wiz?op=goto&other={n}')")) {
                            span.num {
                                @match state { "done" => (icon("check", 12)), "error" => "!", _ => (n) }
                            }
                            span.label { (label) }
                        }
                    }
                }
            }
            div.cols.with-preview[with_preview] {
                section #create-wiz-step.step {
                    @match w.step {
                        1 => (basics(&err)),
                        2 => (jobs_step(w, &err, opened)),
                        3 => (scheds_step(w, &err, opened, slurm.is_some())),
                        4 => (actions_step(w, &err, opened, slurm.is_some())),
                        5 => (monitor_step(w, &err)),
                        _ => (review(w, slurm, &spec_json)),
                    }
                }
                @if w.step != LAST {
                    @if w.preview {
                        aside.preview {
                            div.preview-head {
                                strong { "Live spec" }
                                span.grow {}
                                (copy_button("Copy"))
                                button.btn.btn-sm.btn-ghost.btn-icon type="button" aria-label="Hide preview" title="Hide preview"
                                    data-on:click="$create.w.preview = false; @post('/ui/create/render')" { (icon("x", 14)) }
                            }
                            pre #create-spec-json.code-block.hl { (highlight_json(&spec_json)) }
                        }
                    } @else {
                        button.show-preview.btn.btn-sm.btn-ghost type="button"
                            data-on:click="$create.w.preview = true; @post('/ui/create/render')" { "Show spec preview" }
                    }
                }
            }
        }
    }
}

fn copy_button(label: &str) -> Markup {
    html! {
        button.btn.btn-sm.btn-ghost type="button"
            data-on:click=(format!("navigator.clipboard.writeText(document.getElementById('create-spec-json').textContent).then(() => {{ el.textContent = 'Copied'; setTimeout(() => el.textContent = {}, 1500) }})", js(label))) {
            (label)
        }
    }
}

type ErrFn<'a> = dyn Fn(&str) -> Option<&'static str> + 'a;

/// A labeled control with an inline error or hint.
fn field(
    label: &str,
    required: bool,
    wide: bool,
    error: Option<&str>,
    hint: Option<&str>,
    control: Markup,
) -> Markup {
    html! {
        label.field.invalid[error.is_some()].wide[wide] {
            span { (label) @if required { span.req aria-hidden="true" { " *" } } }
            (control)
            @if let Some(e) = error { span.err role="alert" { (e) } }
            @else if let Some(h) = hint { span.hint { (h) } }
        }
    }
}

/// A text input bound to `create.w.<path>`.
fn text(path: &str, placeholder: &str, mono: bool) -> Markup {
    html! { input.input.mono[mono] data-bind=(format!("create.w.{path}")) placeholder=(placeholder) autocomplete="off"; }
}

fn number(path: &str, min: i64) -> Markup {
    html! { input.input type="number" min=(min) data-bind=(format!("create.w.{path}")); }
}

/// Multi-select as toggle chips.
fn chips(
    label: &str,
    options: &[(i64, String)],
    selected: &[i64],
    empty: &str,
    error: Option<&str>,
    toggle: impl Fn(i64) -> String,
) -> Markup {
    html! {
        div.field.wide.invalid[error.is_some()] role="group" aria-label=(label) {
            span { (label) }
            @if options.is_empty() {
                span.hint { (empty) }
            } @else {
                div.chips {
                    @for (id, l) in options {
                        @let on = selected.contains(id);
                        button.chip type="button" aria-pressed=(on) data-on:click=(toggle(*id)) {
                            @if on { "✓ " } (l)
                        }
                    }
                }
            }
            @if let Some(e) = error { span.err role="alert" { (e) } }
        }
    }
}

/// Collapsible card for one job / scheduler / action.
#[allow(clippy::too_many_arguments)]
fn card(
    dom_id: &str,
    index: usize,
    title: &str,
    subtitle: &str,
    open: bool,
    invalid: bool,
    remove: String,
    body: Markup,
) -> Markup {
    html! {
        details.card.item #(dom_id) .invalid[invalid] open[open] data-new[open] data-preserve-attr="open" {
            summary {
                span.chev { (icon("chevronRight", 14)) }
                span.idx { (index) }
                span.title { (title) }
                @if !subtitle.is_empty() { span.sub { (subtitle) } }
                @if invalid { span.flag { "Needs attention" } }
                button.btn.btn-ghost.btn-sm.btn-icon.remove type="button" aria-label=(format!("Remove {title}")) title="Remove"
                    data-on:click=(format!("evt.preventDefault(); {remove}")) { (icon("trash", 14)) }
            }
            div.content { (body) }
        }
    }
}

fn op(op: &str, id: i64) -> String {
    format!("@post('/ui/create/wiz?op={op}&id={id}')")
}

fn head(title: &str, sub: Markup, add_label: &str, add: &str, disabled: bool) -> Markup {
    html! {
        div.whead {
            div { h4 { (title) } p.muted { (sub) } }
            button.btn.btn-sm type="button" disabled[disabled] data-on:click=(add) {
                (icon("plus", 14)) " " (add_label)
            }
        }
    }
}

fn basics(err: &ErrFn) -> Markup {
    let strategy = |value: &str, title: Markup, text: Markup| {
        html! {
            label.strategy {
                input type="radio" name="create-strategy" value=(value) data-bind="create.w.strategy";
                strong { (title) }
                small { (text) }
            }
        }
    };
    html! {
        div.form {
            (field("Workflow name", true, false, err("name"), None, text("name", "my-data-pipeline", false)))
            (field("Description", false, false, None, None, text("description", "Optional: what does this workflow do?", false)))
            div.field role="radiogroup" aria-label="Parallelization strategy" {
                span { "Parallelization strategy" }
                div.strategies {
                    (strategy("resource_aware", html! { "Resource-aware " span.tag { "Recommended" } },
                        html! { "Define per-job CPU, memory, and GPU needs. The runner packs jobs onto available resources." }))
                    (strategy("queue_depth", html! { "Queue-depth" },
                        html! { "Cap concurrent jobs with " code { "--max-parallel-jobs" } ". Best for homogeneous jobs." }))
                }
                span.hint {
                    a href="https://natlabrockies.github.io/torc/latest/core/concepts/parallelization.html" target="_blank" rel="noreferrer" {
                        "Learn more about parallelization"
                    }
                }
            }
        }
    }
}

fn jobs_step(w: &Wiz, err: &ErrFn, opened: Option<i64>) -> Markup {
    let jobs = w.jobs();
    let named_scheds: Vec<Sched> = w
        .scheds()
        .into_iter()
        .filter(|s| !s.name.trim().is_empty())
        .collect();
    html! {
        (head("Jobs", html! {
            "Need file-based dependencies or complex pipelines? Use a spec file instead ("
            a href="https://natlabrockies.github.io/torc/latest/core/workflows/creating-workflows.html" target="_blank" rel="noreferrer" { "guide" }
            ")."
        }, "Add job", "@post('/ui/create/wiz?op=add-job')", false))
        @if let Some(e) = err("step") { p.error-text { (e) } }
        div.list {
            @for (i, j) in jobs.iter().enumerate() {
                @let p = format!("jobs.j{}", j.id);
                @let e = |f: &str| err(&format!("job:{}:{f}", j.id));
                @let invalid = ["name", "command", "parameters", "num_cpus", "num_gpus", "memory", "runtime"].iter().any(|f| e(f).is_some());
                @let count = if is_parameterized(j) { expanded_count(j) } else { Some(1) };
                @let subtitle = {
                    let mut parts = Vec::new();
                    if count != Some(1) { parts.push(count.map(|n| format!("×{n}")).unwrap_or_else(|| "parameterized".into())); }
                    if !j.command.trim().is_empty() { parts.push(j.command.trim().to_string()); }
                    parts.join(" · ")
                };
                @let others: Vec<(i64, String)> = jobs.iter().filter(|o| o.id != j.id).map(|o| (o.id, job_label(o))).collect();
                (card(&format!("wj-{}", j.id), i + 1, &job_label(j), &subtitle, opened == Some(j.id), invalid, op("rm-job", j.id), html! {
                    (field("Name", true, false, e("name"), job_warning(j), text(&format!("{p}.name"), "e.g. process-data", false)))
                    (field("Scheduler", false, false, None, Some("Auto lets torc pick a matching scheduler"), html! {
                        select.select data-bind=(format!("create.w.{p}.scheduler"))
                            data-on:change=(format!("evt.target.value === 'new' && {}", op("new-sched", j.id))) {
                            option value="" selected[j.scheduler.is_empty()] { "Auto" }
                            @for s in &named_scheds {
                                option value=(s.id) selected[j.scheduler == s.id.to_string()] { (s.name) }
                            }
                            option value="new" { "+ New Slurm scheduler…" }
                        }
                    }))
                    (field("Command", true, true, e("command"), None, text(&format!("{p}.command"), "e.g. python process.py --input data.csv", true)))
                    (chips("Depends on", &others, &j.depends_on, "Add more jobs to define dependencies.", None,
                        |o| format!("@post('/ui/create/wiz?op=dep&id={}&other={o}')", j.id)))
                    (field("Parameters", false, false, e("parameters"), Some(&match count {
                        None => "Expands into multiple jobs. Use {param} in the name and command.".to_string(),
                        Some(n) => format!("Expands into {n} job{}.", if n == 1 { "" } else { "s" }),
                    }), text(&format!("{p}.parameters"), "i: 1:10   or   x: [1,2,3], y: [a,b,c]", true)))
                    (field("Parameter mode", false, false, None, Some("Product: every combination. Zip: pair values by position."), html! {
                        select.select data-bind=(format!("create.w.{p}.parameter_mode")) disabled[!is_parameterized(j)] {
                            option value="product" selected[j.parameter_mode != "zip"] { "Product" }
                            option value="zip" selected[j.parameter_mode == "zip"] { "Zip" }
                        }
                    }))
                    @if w.resource_aware() {
                        div.field.wide role="group" aria-label="Resources" {
                            span { "Resources" }
                            div.presets {
                                @for (key, label, cpus, mem, gpus) in PRESETS {
                                    button.preset type="button" aria-pressed=(j.resource_preset == key)
                                        data-on:click=(format!("@post('/ui/create/wiz?op=preset&id={}&key={key}')", j.id)) {
                                        strong { (label) }
                                        @if key != "custom" {
                                            small { (cpus) " CPU · " (mem) @if gpus > 0 { " · " (gpus) " GPU" } }
                                        } @else { small { "Set your own" } }
                                    }
                                }
                            }
                        }
                        @if j.resource_preset == "custom" {
                            div.res.wide {
                                (field("CPUs", false, false, e("num_cpus"), None, number(&format!("{p}.num_cpus"), 1)))
                                (field("Memory", false, false, e("memory"), None, text(&format!("{p}.memory"), "4g", false)))
                                (field("GPUs", false, false, e("num_gpus"), None, number(&format!("{p}.num_gpus"), 0)))
                                (field("Runtime", false, false, e("runtime"), Some("ISO8601, e.g. PT1H"), text(&format!("{p}.runtime"), "PT1H", false)))
                            }
                        } @else {
                            (field("Runtime", false, false, e("runtime"), Some("ISO8601 duration, e.g. PT30M, PT2H"), text(&format!("{p}.runtime"), "PT1H", false)))
                        }
                    }
                }))
            }
            @if jobs.is_empty() {
                button.empty-add type="button" data-on:click="@post('/ui/create/wiz?op=add-job')" {
                    (icon("plus", 20))
                    span { "Add your first job" }
                    small.faint { "A job is a command plus its dependencies and resources." }
                }
            }
        }
    }
}

fn scheds_step(w: &Wiz, err: &ErrFn, opened: Option<i64>, slurm_auto: bool) -> Markup {
    let scheds = w.scheds();
    html! {
        (head("Slurm schedulers", html! { "Optional. Configure Slurm allocations for running jobs on compute nodes." },
            "Add scheduler", "@post('/ui/create/wiz?op=add-sched')", false))
        div.list {
            @for (i, s) in scheds.iter().enumerate() {
                @let p = format!("scheds.s{}", s.id);
                @let e = |f: &str| err(&format!("sched:{}:{f}", s.id));
                @let used = w.jobs.values().filter(|j| j.scheduler == s.id.to_string()).count();
                @let nodes = s.nodes.f();
                @let subtitle = [
                    s.account.clone(),
                    format!("{} node{}", s.nodes.json(0), if nodes == 1.0 { "" } else { "s" }),
                    s.walltime.clone(),
                    if used > 0 { format!("{used} job{}", if used == 1 { "" } else { "s" }) } else { String::new() },
                ].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ");
                @let title = if s.name.trim().is_empty() { "Untitled scheduler".to_string() } else { s.name.trim().to_string() };
                @let invalid = ["name", "account", "nodes", "walltime"].iter().any(|f| e(f).is_some());
                (card(&format!("ws-{}", s.id), i + 1, &title, &subtitle, opened == Some(s.id), invalid, op("rm-sched", s.id), html! {
                    (field("Name", true, false, e("name"), None, text(&format!("{p}.name"), "e.g. compute", false)))
                    (field("Account", true, false, e("account"), None, text(&format!("{p}.account"), "e.g. my_project", false)))
                    (field("Nodes", false, false, e("nodes"), None, number(&format!("{p}.nodes"), 1)))
                    (field("Wall time", false, false, e("walltime"), Some("HH:MM:SS"), text(&format!("{p}.walltime"), "01:00:00", false)))
                    (field("Partition", false, false, None, None, text(&format!("{p}.partition"), "e.g. compute", false)))
                    (field("QoS", false, false, None, None, text(&format!("{p}.qos"), "e.g. normal", false)))
                    (field("GRES", false, false, None, None, text(&format!("{p}.gres"), "e.g. gpu:2", false)))
                    (field("Memory", false, false, None, None, text(&format!("{p}.mem"), "e.g. 256G", false)))
                    (field("Temporary storage", false, false, None, None, text(&format!("{p}.tmp"), "e.g. 100G", false)))
                    (field("Extra Slurm options", false, false, None, None, text(&format!("{p}.extra"), "e.g. --exclusive", false)))
                }))
            }
            @if scheds.is_empty() {
                div.note {
                    @if slurm_auto {
                        strong { "Schedulers will be generated automatically." }
                        span { "\"Generate Slurm schedulers\" is on, so schedulers are created from your jobs' resource requirements." }
                    } @else {
                        strong { "No schedulers defined." }
                        span { "Not needed for local runs. Add one to run on Slurm, or enable \"Generate Slurm schedulers\" under Options." }
                    }
                }
            }
        }
    }
}

fn actions_step(w: &Wiz, err: &ErrFn, opened: Option<i64>, slurm_auto: bool) -> Markup {
    let named: Vec<Sched> = w
        .scheds()
        .into_iter()
        .filter(|s| !s.name.trim().is_empty())
        .collect();
    let actions = w.actions();
    let job_opts: Vec<(i64, String)> = w.jobs().iter().map(|j| (j.id, job_label(j))).collect();
    html! {
        (head("Actions", html! { "Optional. Schedule Slurm nodes automatically when the workflow starts, or when jobs become ready or complete." },
            "Add action", "@post('/ui/create/wiz?op=add-action')", named.is_empty()))
        div.list {
            @if named.is_empty() && !slurm_auto {
                div.note {
                    strong { "No schedulers available." }
                    span { "Define a scheduler in the previous step, or enable \"Generate Slurm schedulers\" under Options." }
                }
            } @else if actions.is_empty() {
                div.note {
                    @if slurm_auto {
                        strong { "Actions will be generated automatically." }
                        span { "\"Generate Slurm schedulers\" is on, so actions are created to schedule nodes when jobs are ready." }
                    } @else {
                        strong { "No actions defined." }
                        span { "Actions are optional for local execution." }
                    }
                }
            }
            @for (i, a) in actions.iter().enumerate() {
                @let p = format!("actions.a{}", a.id);
                @let e = |f: &str| err(&format!("action:{}:{f}", a.id));
                @let invalid = ["scheduler", "jobs", "num_allocations"].iter().any(|f| e(f).is_some());
                @let title = TRIGGERS.iter().find(|t| t.0 == a.trigger_type).map(|t| t.1).unwrap_or("");
                (card(&format!("wa-{}", a.id), i + 1, title, &w.sched_name(&a.scheduler), opened == Some(a.id), invalid, op("rm-action", a.id), html! {
                    (field("Trigger", true, false, None, None, html! {
                        select.select data-bind=(format!("create.w.{p}.trigger_type"))
                            data-on:change=(format!("evt.target.value === 'on_workflow_start' && ($create.w.{p}.jobs = []); @post('/ui/create/render')")) {
                            @for (id, label) in TRIGGERS { option value=(id) selected[a.trigger_type == id] { (label) } }
                        }
                    }))
                    (field("Scheduler", true, false, e("scheduler"), None, html! {
                        select.select data-bind=(format!("create.w.{p}.scheduler")) {
                            option value="" selected[a.scheduler.is_empty()] { "Select scheduler…" }
                            @for s in &named { option value=(s.id) selected[a.scheduler == s.id.to_string()] { (s.name) } }
                        }
                    }))
                    @if is_job_triggered(&a.trigger_type) {
                        (chips("Jobs *", &job_opts, &a.jobs, "No jobs defined.", e("jobs"),
                            |o| format!("@post('/ui/create/wiz?op=act-job&id={}&other={o}')", a.id)))
                    }
                    (field("Number of allocations", false, false, e("num_allocations"), None, number(&format!("{p}.num_allocations"), 1)))
                    @if !w.resource_aware() {
                        (field("Max parallel jobs", false, false, None, None, number(&format!("{p}.max_parallel_jobs"), 1)))
                    } @else { span {} }
                    label.check {
                        input type="checkbox" data-bind=(format!("create.w.{p}.start_one_worker_per_node"));
                        "Start one worker per node"
                    }
                }))
            }
        }
    }
}

fn monitor_step(w: &Wiz, err: &ErrFn) -> Markup {
    let gran = |value: &str, title: &str, text: &str| {
        html! {
            label.strategy {
                input type="radio" name="create-granularity" value=(value) data-bind="create.w.monitor.granularity";
                strong { (title) }
                small { (text) }
            }
        }
    };
    html! {
        div.form {
            div {
                h4 { "Resource monitoring" }
                p.muted.sub { "Track CPU, memory, and process usage while jobs run." }
            }
            label.check {
                input type="checkbox" data-bind="create.w.monitor.enabled" data-on:change="@post('/ui/create/render')";
                "Enable resource monitoring"
            }
            @if w.monitor.enabled {
                div.field role="radiogroup" aria-label="Granularity" {
                    span { "Granularity" }
                    div.strategies {
                        (gran("summary", "Summary", "Peak and average values only."))
                        (gran("time_series", "Time series", "Samples at regular intervals. More detail, more storage."))
                    }
                }
                div.narrow {
                    (field("Sample interval (seconds)", false, false, err("interval"), Some("How often to sample resource usage (1–300)."), html! {
                        input.input type="number" min="1" max="300" data-bind="create.w.monitor.sample_interval_seconds";
                    }))
                }
            }
        }
    }
}

fn review(w: &Wiz, slurm: Option<(&str, &str)>, spec_json: &str) -> Markup {
    let total: Option<usize> = w
        .jobs
        .values()
        .map(expanded_count)
        .try_fold(0, |n, c| c.map(|c| n + c));
    html! {
        div.review {
            div.summary {
                div { span.v.plain { (opt(&w.name).unwrap_or_else(|| "untitled-workflow".into())) } span.l { "Name" } }
                div {
                    span.v { (w.jobs.len()) }
                    span.l { "Job specs" @if let Some(t) = total.filter(|t| *t != w.jobs.len()) { " → " (t) " jobs" } }
                }
                div { span.v { (w.scheds.len()) } span.l { "Schedulers" } }
                div { span.v { (w.actions.len()) } span.l { "Actions" } }
                div {
                    span.v { (if w.monitor.enabled { w.monitor.granularity.replace('_', " ") } else { "off".into() }) }
                    span.l { "Monitoring" }
                }
            }
            @if let Some((account, profile)) = slurm {
                div.note-info {
                    "Slurm schedulers and actions will be generated when the workflow is created, based on job resource requirements (account "
                    code { (if account.is_empty() { "(not set)" } else { account }) } ", profile " code { (profile) } ")."
                }
            }
            div #create-wiz-report data-init="@post('/ui/create/wiz-validate')" {
                div.vpanel { span.spinner {} " " span.muted { "Validating…" } }
            }
            div.preview-head {
                strong { "Generated spec" }
                (copy_button("Copy JSON"))
            }
            pre #create-spec-json.code-block.big.hl { (highlight_json(spec_json)) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameters() {
        let p = parse_parameters(r#"x: [1,2,3], y: [a,b], name: "a, b", i: 1:10"#);
        assert_eq!(
            p,
            vec![
                ("x".to_string(), "[1,2,3]".to_string()),
                ("y".into(), "[a,b]".into()),
                ("name".into(), "a, b".into()),
                ("i".into(), "1:10".into()),
            ]
        );
        assert_eq!(value_count("1:10"), Some(10));
        assert_eq!(value_count("0:100:10"), Some(11));
        assert_eq!(value_count("[a,b]"), Some(2));
        let mut j = new_job(1);
        j.parameters = "x: [1,2,3], y: [a,b]".into();
        assert_eq!(expanded_count(&j), Some(6));
        j.parameter_mode = "zip".into();
        assert_eq!(expanded_count(&j), Some(3));
        assert_eq!(job_name_to_regex("work_{i:03d}.x"), r"^work_.*\.x$");
        assert!(valid_runtime("PT30M") && valid_runtime("P1D") && !valid_runtime("PT"));
        assert!(!valid_runtime("P"));
    }

    #[test]
    fn generate_and_validate() {
        let mut w = Wiz {
            name: "wf".into(),
            ..Default::default()
        };
        assert_eq!(first_invalid_step(&w), Some(2));
        apply(&mut w, "add-job", 0, 0, "");
        apply(&mut w, "add-job", 0, 0, "");
        for (k, j) in w.jobs.iter_mut() {
            j.name = format!("n{k}");
            j.command = "echo".into();
        }
        apply(&mut w, "dep", 2, 1, "");
        apply(&mut w, "preset", 2, 0, "gpu");
        assert_eq!(first_invalid_step(&w), None);
        let v = serde_json::to_value(generate(&w)).unwrap();
        assert_eq!(v["jobs"][1]["depends_on"], json!(["nj1"]));
        assert_eq!(v["resource_requirements"].as_array().unwrap().len(), 2);
        assert_eq!(
            v["jobs"][1]["resource_requirements"],
            "res_1cpu_10g_1gpu_PT1H"
        );
        apply(&mut w, "rm-job", 1, 0, "");
        let p = patch(
            &serde_json::from_value(json!({"jobs": {"j1": {}, "j2": {}}})).unwrap(),
            &w,
        );
        assert_eq!(p["jobs"]["j1"], Value::Null);
        assert!(p["jobs"]["j2"].is_object());
    }
}
