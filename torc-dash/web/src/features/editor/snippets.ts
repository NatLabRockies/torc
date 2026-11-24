/** Spec building blocks for the editor's Insert menu, and where they go in a YAML document. */

/** `map`: the section is a mapping (keys, not list items), so it can only be inserted once. */
export type Snippet = { label: string; section: string; text: string; map?: boolean };

/** List items (or mapping keys), written at indent 0; insertSnippet() re-indents them. */
export const SNIPPETS: Snippet[] = [
  {
    label: "Job",
    section: "jobs",
    text: `- name: my_job
  command: echo hello
`,
  },
  {
    label: "Parameterized job",
    section: "jobs",
    text: `- name: job_{i:03d}
  command: python run.py --index {i}
  parameters:
    i: "1:10"
`,
  },
  {
    label: "Job with file dependencies",
    section: "jobs",
    text: `- name: process
  command: python process.py --input data/raw.csv --output data/clean.csv
  input_files: [raw_data]
  output_files: [clean_data]
`,
  },
  { label: "File", section: "files", text: "- name: raw_data\n  path: data/raw.csv\n" },
  {
    label: "Resource requirements",
    section: "resource_requirements",
    text: `- name: small
  num_cpus: 1
  memory: 2g
  runtime: PT30M
`,
  },
  {
    label: "Slurm scheduler",
    section: "slurm_schedulers",
    text: `- name: compute
  account: my_account
  nodes: 1
  walltime: "04:00:00"
`,
  },
  {
    label: "Execution config",
    section: "execution_config",
    map: true,
    text: `mode: direct
limit_resources: true
termination_signal: SIGTERM
sigterm_lead_seconds: 30
`,
  },
  {
    label: "Resource monitoring",
    section: "resource_monitor",
    map: true,
    text: `sample_interval_seconds: 10
generate_plots: true
jobs:
  enabled: true
  granularity: summary  # or time_series
compute_node:
  enabled: false
  granularity: summary  # or time_series
  # cpu: true
  # memory: true
# flush_interval_seconds: 300  # how often time-series samples are written to SQLite
# live_monitoring: false  # checkpoint after each flush so torc-dash live plots see new samples
`,
  },
  {
    // on_jobs_ready, not on_workflow_start: a selective re-run (reset-status --reinit, then
    // submit) re-schedules only the allocations whose jobs were reset.
    label: "Action: schedule Slurm nodes when jobs are ready",
    section: "actions",
    text: `- trigger_type: on_jobs_ready
  action_type: schedule_nodes
  jobs: [my_job]
  scheduler: compute
  scheduler_type: slurm
  num_allocations: 1
`,
  },
  {
    label: "Failure handler (retry)",
    section: "failure_handlers",
    text: `- name: retry_on_timeout
  rules:
    - exit_codes: [152]
      max_retries: 3
`,
  },
  { label: "User data", section: "user_data", text: "- name: config\n  data:\n    key: value\n" },
];

/**
 * Append `snippet` to the end of its top-level section, creating the section at the end of the
 * document if missing. Items are indented like the section's existing items (default 2). An empty
 * section (`files: []`, `files: {}`, `files: null`, `files: ~`) becomes a block section; a non-empty
 * flow section throws, since rewriting it could lose comments or formatting. Returns the new text and
 * the inserted range.
 */
export function insertSnippet(text: string, snippet: Snippet): { text: string; from: number; to: number } {
  const lines = text.split("\n");
  let head = lines.findIndex((l) => new RegExp(`^${snippet.section}\\s*:`).test(l));
  if (head >= 0) {
    const value = lines[head].replace(/^[^:]+:\s*/, "").replace(/(^|\s)#.*$/, "").trim();
    if (/^(\[\s*\]|\{\s*\}|~|null|Null|NULL)$/.test(value)) {
      const comment = /\s(#.*)$/.exec(lines[head])?.[1];
      lines[head] = comment ? `${snippet.section}:  ${comment}` : `${snippet.section}:`;
    } else if (/^[[{]/.test(value)) {
      throw new Error(`"${snippet.section}" is written in flow style. Convert it to a block list to insert here.`);
    } else if (value) head = -1; // not a section (e.g. a scalar); append a new one
  }
  if (head < 0) {
    const sep = !text.trim() ? "" : text.endsWith("\n\n") ? "" : text.endsWith("\n") ? "\n" : "\n\n";
    const body = `${snippet.section}:\n${indent(snippet.text, 2)}`;
    const from = text.length + sep.length;
    return { text: text + sep + body, from, to: from + body.length };
  }
  // The section ends at the next top-level key (column 0, not a list dash or comment).
  let end = lines.findIndex((l, i) => i > head && /^[^\s#-]/.test(l));
  if (end < 0) end = lines.length;
  while (end - 1 > head && !lines[end - 1].trim()) end--;
  if (snippet.map && lines.slice(head + 1, end).some((l) => /^\s+[^\s#]/.test(l)))
    throw new Error(`"${snippet.section}" already exists. Edit it there.`);
  const item = lines.slice(head + 1, end).find((l) => /^\s*- /.test(l));
  const body = indent(snippet.text, item ? item.length - item.trimStart().length : 2).replace(/\n$/, "");
  const before = lines.slice(0, end).join("\n");
  const from = before.length + 1;
  const after = lines.slice(end).join("\n");
  return { text: `${before}\n${body}${end < lines.length ? "\n" + after : "\n"}`, from, to: from + body.length };
}

const indent = (s: string, n: number) =>
  s
    .split("\n")
    .map((l) => (l ? " ".repeat(n) + l : l))
    .join("\n");
