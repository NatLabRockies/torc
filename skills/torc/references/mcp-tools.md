# MCP tools

Torc ships an MCP server (`torc-mcp-server`) that exposes workflow operations to an AI assistant
over the Model Context Protocol. It is a client of the same HTTP API as the CLI, so it never sees
state the CLI cannot reach, and everything this skill says about server behavior applies to both.

Prefer whichever surface the session actually has. If the tools are connected, use them; do not
shell out to `torc` for something a connected tool already does, and do not describe tools to
someone driving the CLI.

## Contents

- [What MCP does and does not cover](#what-mcp-does-and-does-not-cover)
- [Tool to command map](#tool-to-command-map)
- [MCP-only capabilities](#mcp-only-capabilities)
- [Mutating tools and dry_run](#mutating-tools-and-dry_run)
- [Creating specs](#creating-specs)
- [Docs and examples](#docs-and-examples)
- [Setup](#setup)

## What MCP does and does not cover

Covered: inspection, log reading, resource analysis, failure diagnosis, recovery, allocation
planning, and spec authoring.

Not covered, and CLI-only:

- Starting work: `torc run`, `torc submit`, `torc exec`.
- Scheduler generation and submission: `torc slurm generate`, `schedule-nodes`, `regenerate`.
- State transitions other than recovery: `init`, `reinit`, `reset-status`, `jobs reset-status`,
  `cancel`, `delete`.
- Remote workers: the whole `torc remote` group.
- Configuration, HPC profiles, export/import, RO-Crate, and admin.

An assistant holding only MCP tools can diagnose and adjust a workflow but cannot start one. Say
that plainly instead of implying a tool exists; several tool descriptions end by telling the user
which `torc` command to run next, which is the intended pattern.

## Tool to command map

| MCP tool                        | CLI equivalent                                              | Reference               |
| ------------------------------- | ----------------------------------------------------------- | ----------------------- |
| `create_workflow`               | `torc create`                                               | `spec-authoring.md`     |
| `get_execution_plan`            | `torc workflows execution-plan`                             | `query-map.md`          |
| `get_workflow_status`           | `torc status`                                               | `query-map.md`          |
| `get_workflow_summary`          | `torc status`                                               | `query-map.md`          |
| `get_job_details`               | `torc jobs get`                                             | `query-map.md`          |
| `list_jobs_by_status`           | `torc jobs list -s <status>`                                | `query-map.md`          |
| `list_failed_jobs`              | `torc jobs list -s failed`                                  | `failure-analysis.md`   |
| `list_results`                  | `torc results list`                                         | `query-map.md`          |
| `get_job_logs`                  | `torc results list --include-logs`, then read the file      | `log-map.md`            |
| `analyze_workflow_logs`         | `torc logs analyze`                                         | `log-map.md`            |
| `check_resource_utilization`    | `torc workflows check-resources`                            | `optimization.md`       |
| `update_job_resources`          | `torc resource-requirements update`                         | `optimization.md`       |
| `recover_workflow`              | `torc recover`                                              | `rerun-and-recovery.md` |
| `list_pending_failed_jobs`      | `torc jobs list -s pending_failed`                          | `failure-analysis.md`   |
| `classify_and_resolve_failures` | none                                                        | `failure-analysis.md`   |
| `get_slurm_sacct`               | `torc slurm sacct`                                          | `slurm.md`              |
| `plan_allocations`              | `torc slurm plan-allocations`                               | `optimization.md`       |
| `check_offline_journals`        | inspect `offline_journal/`, then `torc workflows reconcile` | `connectivity.md`       |
| `analyze_resource_usage`        | none                                                        | `optimization.md`       |
| `regroup_job_resources`         | none                                                        | `optimization.md`       |
| `list_examples` / `get_example` | browse and read `examples/`                                 | `spec-authoring.md`     |
| `get_docs`                      | read the published docs                                     | —                       |

Filters mirror the CLI: `list_results` takes `failed_only`, `get_job_logs` takes a stream and an
optional line count, and `list_jobs_by_status` takes the same status names documented in
`query-map.md`.

## MCP-only capabilities

Three things have no CLI equivalent, and two of them matter for the optimization work in
`optimization.md`.

**`analyze_resource_usage`** returns per-job peak memory, peak CPU percent, and execution time
grouped by resource requirement, with min/max/mean/median per group alongside the configured limits.
`torc workflows check-resources` only reports jobs that exceeded their limits. The distribution is
what reveals that one requirement is hiding two different workloads, which is the signal for the
memory-band split described in `optimization.md`. `completed_only` restricts it to finished jobs.

**`regroup_job_resources`** creates new resource requirement records and reassigns jobs to them.
Each group carries `memory`, `num_cpus`, `runtime`, optional `num_gpus`/`num_nodes`, a name, and
explicit `job_ids`. Jobs left out of every group keep their current requirement, each job may appear
in only one group, and existing requirements are never modified or deleted. This is the mechanized
form of the split that the CLI can only approximate: `correct-resources` adjusts existing
requirements and `--group-by resource-requirements` separates existing scheduler groups, but neither
synthesizes new groups from observed clusters.

The intended loop:

1. `analyze_resource_usage` to see the actual distribution per requirement.
2. Identify clusters, for example jobs using 2 GB sitting next to jobs using 20 GB in one group.
3. `regroup_job_resources` with `dry_run: true`, and show the before/after per job.
4. Apply after confirmation, then regenerate schedulers and compare allocation counts.

**`classify_and_resolve_failures`** decides whether each `pending_failed` job is transient (`retry`,
optionally with a new `memory` or `runtime`) or permanent (`fail`), with a `reason` recorded for
each. No CLI command makes that judgement; `torc recover --ai-recovery` invokes an agent to do it
through this tool. Pair it with `list_pending_failed_jobs`, which returns the jobs and their stderr.

## Mutating tools and dry_run

Four tools change workflow state: `update_job_resources`, `recover_workflow`,
`regroup_job_resources`, and `classify_and_resolve_failures`. The last three take a required
`dry_run`, and their descriptions are explicit that the first call should always be `dry_run: true`,
with the diff shown to the user before applying.

Treat that as binding. Call with `dry_run: true`, present the before/after, get confirmation, then
call again with `dry_run: false`.

`recover_workflow` accepts `memory_multiplier` (default 1.5 for OOM), `runtime_multiplier` (default
1.5 for timeout), and `retry_unknown` (default false), matching `torc recover`. When updating
resources after `check_resource_utilization`, update every over-utilized job rather than only the
failed ones.

## Creating specs

`create_workflow` takes `spec_json` as a JSON object and an `action` that decides what happens:

| Action            | Effect                                        |
| ----------------- | --------------------------------------------- |
| `validate`        | Check for errors without saving or creating   |
| `save_spec_file`  | Write the spec to a file for the user to edit |
| `create_workflow` | Create it in the database                     |

Default to `save_spec_file`. A generated spec is a template with placeholder commands that the user
must customize, so creating it immediately is usually wrong. Reserve `create_workflow` for an
explicit "run", "submit", or "execute".

Ask whether the target is local or Slurm before generating, because `workflow_type` depends on it
and Slurm additionally needs `account` (and possibly `hpc_profile` if detection fails). After
saving, tell the user to replace the placeholder commands, confirm the input files exist, and run
`torc run <file>` for local or `torc submit <file>` for Slurm. Do not invent commands or flags.

When the request mentions files or data flow, express it as a `files` section with `input_files` and
`output_files` on the jobs rather than as ordering, for the reasons in `spec-authoring.md`.

## Docs and examples

`get_docs` takes a topic, including `workflow-spec`, `dependencies`, `parameterization`, `slurm`,
`job-states`, `actions`, `failure-handlers`, `recovery`, `ai-recovery`, `resource-monitoring`,
`cli`, `quick-start`, `architecture`, `checkpointing`, `hpc-profiles`, `workflow-formats`,
`allocation-strategies`, and `tutorials`. `list_examples` and `get_example` reach the runnable specs
in `examples/`.

The same content is exposed as MCP resources at `torc://docs/{topic}` and `torc://examples/{name}`.

Without `TORC_DOCS_DIR` and `TORC_EXAMPLES_DIR` pointing at local copies, these fetch from GitHub
and need network access.

## Setup

`torc-mcp-server` is a feature-gated binary; install it alongside the CLI
(`cargo install torc --features "server-bin,mcp-server,dash,slurm-runner"`) or take it from a
release archive.

```json
{
  "mcpServers": {
    "torc": {
      "command": "/path/to/torc-mcp-server",
      "env": {
        "TORC_API_URL": "http://localhost:8080/torc-service/v1",
        "TORC_OUTPUT_DIR": "/path/to/output"
      }
    }
  }
}
```

`TORC_API_URL` and `TORC_OUTPUT_DIR` carry the same meaning and the same failure modes as for the
CLI: the URL must include `/torc-service/v1` and be reachable from wherever the MCP server process
runs, and the output directory must match the one used during execution or `get_job_logs` will miss.
`TORC_PASSWORD` applies if the server requires authentication.

On HPC, run the MCP server on the cluster next to the Torc server and reach it through an editor's
remote session, so no ports need exposing.

Two surface-level differences from the CLI worth knowing: MCP returns structured JSON with no table
rendering, and it has no interactive workflow-selection prompt, so the stdout-corruption hazard in
`command-behavior.md` does not arise there. There is also no exit status, so the `torc run` exit-0
trap does not apply.
