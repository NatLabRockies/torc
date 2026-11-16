/**
 * Wizard data model, workflow-spec generation, and per-step validation. Plain TS (no runes) so the
 * logic is easy to reason about; the components wrap a WizardData object in $state.
 */

export const PRESETS = {
  small: { label: "Small", num_cpus: 1, memory: "1g", num_gpus: 0 },
  medium: { label: "Medium", num_cpus: 4, memory: "10g", num_gpus: 0 },
  large: { label: "Large", num_cpus: 8, memory: "50g", num_gpus: 0 },
  gpu: { label: "GPU", num_cpus: 1, memory: "10g", num_gpus: 1 },
  custom: { label: "Custom", num_cpus: 1, memory: "1g", num_gpus: 0 },
} as const;
export type PresetKey = keyof typeof PRESETS;

export type TriggerType = "on_workflow_start" | "on_jobs_ready" | "on_jobs_complete";
export const TRIGGERS: { id: TriggerType; label: string }[] = [
  { id: "on_workflow_start", label: "When workflow starts" },
  { id: "on_jobs_ready", label: "When jobs become ready" },
  { id: "on_jobs_complete", label: "When jobs complete" },
];

export type WizardJob = {
  id: number;
  name: string;
  command: string;
  /** Ids of other wizard jobs. */
  depends_on: number[];
  resource_preset: PresetKey;
  num_cpus: number;
  memory: string;
  num_gpus: number;
  runtime: string;
  parameters: string;
  parameter_mode: "product" | "zip";
  /** Id of a wizard scheduler, or null for auto. */
  scheduler: number | null;
};

export type WizardScheduler = {
  id: number;
  name: string;
  account: string;
  nodes: number;
  walltime: string;
  partition: string;
  qos: string;
  gres: string;
  mem: string;
  tmp: string;
  extra: string;
};

export type WizardAction = {
  id: number;
  trigger_type: TriggerType;
  /** Id of a wizard scheduler. */
  scheduler: number | null;
  /** Ids of wizard jobs (for job-triggered actions). */
  jobs: number[];
  num_allocations: number;
  max_parallel_jobs: number;
  start_one_worker_per_node: boolean;
};

export type Strategy = "resource_aware" | "queue_depth";

export type WizardData = {
  step: number;
  name: string;
  description: string;
  strategy: Strategy;
  jobs: WizardJob[];
  schedulers: WizardScheduler[];
  actions: WizardAction[];
  monitor: { enabled: boolean; granularity: "summary" | "time_series"; sample_interval_seconds: number };
  nextId: number;
};

export const STEPS = ["Basics", "Jobs", "Schedulers", "Actions", "Monitoring", "Review"] as const;

export function newWizard(): WizardData {
  return {
    step: 1,
    name: "",
    description: "",
    strategy: "resource_aware",
    jobs: [],
    schedulers: [],
    actions: [],
    monitor: { enabled: true, granularity: "summary", sample_interval_seconds: 10 },
    nextId: 1,
  };
}

export function newJob(id: number): WizardJob {
  return {
    id,
    name: "",
    command: "",
    depends_on: [],
    resource_preset: "small",
    num_cpus: 1,
    memory: "1g",
    num_gpus: 0,
    runtime: "PT1H",
    parameters: "",
    parameter_mode: "product",
    scheduler: null,
  };
}

export function newScheduler(id: number, name = ""): WizardScheduler {
  return {
    id,
    name,
    account: "",
    nodes: 1,
    walltime: "01:00:00",
    partition: "",
    qos: "",
    gres: "",
    mem: "",
    tmp: "",
    extra: "",
  };
}

export function newAction(id: number): WizardAction {
  return {
    id,
    trigger_type: "on_workflow_start",
    scheduler: null,
    jobs: [],
    num_allocations: 1,
    max_parallel_jobs: 10,
    start_one_worker_per_node: false,
  };
}

export const jobLabel = (j: WizardJob) => j.name.trim() || "Untitled job";
export const isJobTriggered = (t: TriggerType) => t !== "on_workflow_start";

/**
 * Parse the wizard's compact parameter syntax into the spec's string-valued form:
 *   `i: 1:10`  `x: [1,2,3], y: [a,b]`  `name: "a, b"`
 */
export function parseParameters(text: string): Record<string, string> {
  const s = text.trim();
  const params: Record<string, string> = {};
  let i = 0;
  while (i < s.length) {
    while (i < s.length && (s[i] === " " || s[i] === ",")) i++;
    if (i >= s.length) break;
    const keyStart = i;
    while (i < s.length && /\w/.test(s[i])) i++;
    const key = s.slice(keyStart, i);
    if (!key) break;
    while (i < s.length && (s[i] === " " || s[i] === ":")) i++;
    let value: string;
    if (s[i] === '"' || s[i] === "'") {
      const quote = s[i++];
      const start = i;
      while (i < s.length && s[i] !== quote) i++;
      value = s.slice(start, i++);
    } else if (s[i] === "[") {
      const start = i;
      let depth = 0;
      while (i < s.length) {
        if (s[i] === "[") depth++;
        else if (s[i] === "]" && --depth === 0) {
          i++;
          break;
        }
        i++;
      }
      value = s.slice(start, i);
    } else {
      const start = i;
      while (i < s.length && s[i] !== ",") i++;
      value = s.slice(start, i).trim();
    }
    if (value) params[key] = value;
  }
  return params;
}

/** Number of values one parameter expands to, or null if it can't be determined cheaply. */
function valueCount(v: string): number | null {
  const t = v.trim();
  if (t.startsWith("[")) {
    const inner = t.slice(1, -1).trim();
    return inner ? inner.split(",").length : 0;
  }
  const m = t.match(/^(-?\d+(?:\.\d+)?):(-?\d+(?:\.\d+)?)(?::(-?\d+(?:\.\d+)?))?$/);
  if (m) {
    const [a, b, step] = [Number(m[1]), Number(m[2]), m[3] ? Number(m[3]) : 1];
    if (step <= 0 || b < a) return null;
    return Math.floor((b - a) / step + 1e-9) + 1;
  }
  return 1;
}

/** How many jobs a parameterized job expands to (null if unknown). */
export function expandedCount(job: WizardJob): number | null {
  const counts = Object.values(parseParameters(job.parameters)).map(valueCount);
  if (counts.length === 0) return 1;
  if (counts.some((c) => c === null)) return null;
  const nums = counts as number[];
  return job.parameter_mode === "zip" ? Math.max(...nums) : nums.reduce((a, b) => a * b, 1);
}

const isParameterized = (j: WizardJob) => j.parameters.trim().length > 0;

/** Regex matching every expansion of a templated job name like `work_{i}`. */
export function jobNameToRegex(name: string): string {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return `^${escaped.replace(/\\\{[^}]+\\\}/g, ".*")}$`;
}

type Spec = Record<string, unknown>;

export function generateSpec(w: WizardData): Spec {
  const resourceAware = w.strategy === "resource_aware";
  const jobName = (j: WizardJob) => j.name.trim() || `job_${j.id}`;
  const byId = new Map(w.jobs.map((j) => [j.id, j]));
  const schedName = (id: number | null) =>
    w.schedulers.find((s) => s.id === id)?.name.trim() || "";

  // Split job references into plain names and regexes (for parameterized jobs).
  const refs = (ids: number[]) => {
    const names: string[] = [];
    const regexes: string[] = [];
    for (const id of ids) {
      const j = byId.get(id);
      if (!j) continue;
      if (isParameterized(j)) regexes.push(jobNameToRegex(jobName(j)));
      else names.push(jobName(j));
    }
    return { names, regexes };
  };

  const resKey = (j: WizardJob) => `${j.num_cpus}_${j.memory}_${j.num_gpus}_${j.runtime || "PT1H"}`;
  const resources = new Map<string, Spec>();
  if (resourceAware) {
    for (const j of w.jobs) {
      const runtime = j.runtime || "PT1H";
      if (!resources.has(resKey(j))) {
        resources.set(resKey(j), {
          name: `res_${j.num_cpus}cpu_${j.memory}${j.num_gpus > 0 ? `_${j.num_gpus}gpu` : ""}_${runtime}`,
          num_cpus: j.num_cpus,
          memory: j.memory,
          num_gpus: j.num_gpus,
          num_nodes: 1,
          runtime,
        });
      }
    }
  }

  const jobs = w.jobs.map((j) => {
    const spec: Spec = { name: jobName(j), command: j.command.trim() || 'echo "TODO"' };
    const { names, regexes } = refs(j.depends_on);
    if (names.length) spec.depends_on = names;
    if (regexes.length) spec.depends_on_regexes = regexes;
    if (resourceAware) spec.resource_requirements = resources.get(resKey(j))!.name;
    const sched = schedName(j.scheduler);
    if (sched) spec.scheduler = sched;
    const params = parseParameters(j.parameters);
    if (Object.keys(params).length) {
      spec.parameters = params;
      if (j.parameter_mode === "zip") spec.parameter_mode = "zip";
    }
    return spec;
  });

  const slurm = w.schedulers
    .filter((s) => s.name.trim() && s.account.trim())
    .map((s) => {
      const spec: Spec = {
        name: s.name.trim(),
        account: s.account.trim(),
        nodes: s.nodes || 1,
        walltime: s.walltime.trim() || "01:00:00",
      };
      for (const k of ["partition", "qos", "gres", "mem", "tmp", "extra"] as const) {
        if (s[k].trim()) spec[k] = s[k].trim();
      }
      return spec;
    });

  const actions = w.actions
    .filter((a) => schedName(a.scheduler))
    .map((a) => {
      const spec: Spec = {
        trigger_type: a.trigger_type,
        action_type: "schedule_nodes",
        scheduler: schedName(a.scheduler),
        scheduler_type: "slurm",
        num_allocations: a.num_allocations || 1,
      };
      if (isJobTriggered(a.trigger_type)) {
        const { names, regexes } = refs(a.jobs);
        if (names.length) spec.jobs = names;
        if (regexes.length) spec.job_name_regexes = regexes;
      }
      if (!resourceAware && a.max_parallel_jobs) spec.max_parallel_jobs = a.max_parallel_jobs;
      if (a.start_one_worker_per_node) spec.start_one_worker_per_node = true;
      return spec;
    });

  const spec: Spec = { name: w.name.trim() || "untitled-workflow" };
  if (w.description.trim()) spec.description = w.description.trim();
  spec.jobs = jobs;
  if (resources.size) spec.resource_requirements = [...resources.values()];
  spec.resource_monitor = w.monitor.enabled
    ? {
        enabled: true,
        granularity: w.monitor.granularity,
        sample_interval_seconds: w.monitor.sample_interval_seconds,
      }
    : { enabled: false };
  if (slurm.length) spec.slurm_schedulers = slurm;
  if (actions.length) spec.actions = actions;
  return spec;
}

// ---------- Validation ----------

const MEMORY_RE = /^\d+(\.\d+)?\s*[kmgtKMGT]?[bB]?$/;
const RUNTIME_RE = /^P(?=.)(\d+D)?(T(?=\d)(\d+H)?(\d+M)?(\d+(\.\d+)?S)?)?$/;
const WALLTIME_RE = /^(\d+-)?\d{1,3}:\d{2}:\d{2}$/;

/** Field errors keyed like `job:3:name`, plus step-level messages under `step`. */
export type Errors = Record<string, string>;

function duplicates<T extends { id: number; name: string }>(items: T[]): Set<number> {
  const seen = new Map<string, number>();
  const dups = new Set<number>();
  for (const it of items) {
    const key = it.name.trim().toLowerCase();
    if (!key) continue;
    if (seen.has(key)) {
      dups.add(it.id);
      dups.add(seen.get(key)!);
    } else seen.set(key, it.id);
  }
  return dups;
}

/** Errors that block leaving `step`. */
export function stepErrors(w: WizardData, step: number): Errors {
  const e: Errors = {};
  if (step === 1) {
    if (!w.name.trim()) e.name = "Workflow name is required";
  } else if (step === 2) {
    if (w.jobs.length === 0) e.step = "Add at least one job";
    const dups = duplicates(w.jobs);
    const resourceAware = w.strategy === "resource_aware";
    for (const j of w.jobs) {
      if (!j.name.trim()) e[`job:${j.id}:name`] = "Name is required";
      else if (dups.has(j.id)) e[`job:${j.id}:name`] = "Duplicate job name";
      if (!j.command.trim()) e[`job:${j.id}:command`] = "Command is required";
      if (j.parameters.trim() && Object.keys(parseParameters(j.parameters)).length === 0)
        e[`job:${j.id}:parameters`] = "Could not parse parameters (expected name: value)";
      if (resourceAware) {
        if (!(j.num_cpus >= 1)) e[`job:${j.id}:num_cpus`] = "At least 1";
        if (!(j.num_gpus >= 0)) e[`job:${j.id}:num_gpus`] = "0 or more";
        if (!MEMORY_RE.test(j.memory.trim())) e[`job:${j.id}:memory`] = "e.g. 512m, 4g";
        if (j.runtime.trim() && !RUNTIME_RE.test(j.runtime.trim()))
          e[`job:${j.id}:runtime`] = "ISO8601, e.g. PT30M, PT2H, P1D";
      }
    }
  } else if (step === 3) {
    const dups = duplicates(w.schedulers);
    for (const s of w.schedulers) {
      if (!s.name.trim()) e[`sched:${s.id}:name`] = "Name is required";
      else if (dups.has(s.id)) e[`sched:${s.id}:name`] = "Duplicate scheduler name";
      if (!s.account.trim()) e[`sched:${s.id}:account`] = "Account is required";
      if (!(s.nodes >= 1)) e[`sched:${s.id}:nodes`] = "At least 1";
      if (s.walltime.trim() && !WALLTIME_RE.test(s.walltime.trim()))
        e[`sched:${s.id}:walltime`] = "HH:MM:SS or D-HH:MM:SS";
    }
  } else if (step === 4) {
    for (const a of w.actions) {
      if (!w.schedulers.some((s) => s.id === a.scheduler))
        e[`action:${a.id}:scheduler`] = "Select a scheduler";
      if (isJobTriggered(a.trigger_type) && a.jobs.length === 0)
        e[`action:${a.id}:jobs`] = "Select at least one job";
      if (!(a.num_allocations >= 1)) e[`action:${a.id}:num_allocations`] = "At least 1";
    }
  } else if (step === 5) {
    const n = w.monitor.sample_interval_seconds;
    if (w.monitor.enabled && !(n >= 1 && n <= 300)) e.interval = "Between 1 and 300 seconds";
  }
  return e;
}

/** First step (1-based) with blocking errors, or null if everything is valid. */
export function firstInvalidStep(w: WizardData): number | null {
  for (let s = 1; s < STEPS.length; s++) {
    if (Object.keys(stepErrors(w, s)).length) return s;
  }
  return null;
}

/** Non-blocking hints for a job (e.g. parameterized job without a {param} in its name). */
export function jobWarning(j: WizardJob): string | null {
  if (j.parameters.trim() && !/\{\w+(:[^}]*)?\}/.test(j.name)) {
    return "Use {param} in the name so each expanded job gets a unique name";
  }
  return null;
}
