---
name: torc
description: >
  Author, run, optimize, inspect, and debug Torc workflows through the torc CLI or the Torc MCP
  server. Use for workflow specs in YAML/JSON5/JSON/KDL, local runs, torc exec, Slurm submission and
  allocation sizing, node packing and parallel-jobs-per-node decisions, remote SSH workers, reruns
  and recovery, status/results/log queries, JSON and CSV scripting, MCP tools, configuration, and log
  levels. Do not use for generic Slurm, SSH, or shell questions with no Torc workflow involved, or
  for contributing to the Torc codebase itself.
license: BSD-3-Clause
---

# Torc

Torc turns one declarative spec into a dependency graph of jobs that runners claim and execute. The
server owns all state in SQLite; the CLI is a client. Nothing runs until a workflow exists on a
server and has been initialized.

Add control in layers and stop at the first layer that solves the problem:

```text
dry-run validation -> local standalone smoke test -> shared-server run -> Slurm submit -> tuned allocations
```

## Task router

Read only the reference that matches the task. Do not preload them.

| Task                                                                   | Read                               |
| ---------------------------------------------------------------------- | ---------------------------------- |
| Write or review a spec: dependencies, files, parameters, actions       | `references/spec-authoring.md`     |
| Run locally, choose standalone vs shared server, use `torc exec`       | `references/local-execution.md`    |
| Choose local, remote workers, or Slurm; size a machine or worker pool  | `references/execution-modes.md`    |
| Generate schedulers, submit to Slurm, understand exit codes            | `references/slurm.md`              |
| Pack jobs per node, size allocations, tune walltime and throughput     | `references/optimization.md`       |
| Run jobs on SSH-reachable machines                                     | `references/remote-workers.md`     |
| Rerun part of a workflow, reset state, recover from failures           | `references/rerun-and-recovery.md` |
| Command preconditions, exit codes, stream routing, prompting           | `references/command-behavior.md`   |
| Query workflow, job, result, dependency, or resource state             | `references/query-map.md`          |
| Parse output with jq or Nushell, build reports and CSV                 | `references/scripting.md`          |
| Watch a live workflow: watch, TUI, dashboard, events, plots            | `references/live-monitoring.md`    |
| Diagnose a failed or killed job                                        | `references/failure-analysis.md`   |
| Find logs, bundle them, scan for error patterns                        | `references/log-map.md`            |
| Nothing is progressing: blocked, unclaimed, orphaned, or empty results | `references/stuck-workflows.md`    |
| Server unreachable, auth or TLS failure, results lost to an outage     | `references/connectivity.md`       |
| Configure settings: files, precedence, environment variables           | `references/settings.md`           |
| Set log levels or find log destinations                                | `references/logging.md`            |
| Define or override an HPC profile for a cluster                        | `references/hpc-profiles.md`       |
| Drive Torc from an AI assistant over MCP                               | `references/mcp-tools.md`          |

## Core loop

1. **Pick the surface.** If Torc MCP tools are connected, prefer them for inspection, log reading,
   resource analysis, and recovery, and do not shell out to `torc` for what a tool already does. MCP
   cannot start work, generate schedulers, or drive remote workers, so those stay on the CLI. See
   `references/mcp-tools.md`.
2. **Confirm the target.** Identify the server (`torc ping`), the workflow spec or ID, and the
   execution mode. `torc --help` groups commands, but most subcommand groups are hidden from the
   top-level list, so use `torc <group> --help` for the real inventory.
3. **Validate offline first.** `torc create --dry-run <spec>` parses the spec, expands parameters,
   and reports resulting job/file/action counts without a server. It exits non-zero on failure.
   Always check the expanded job count before creating a sweep.
4. **Smoke test small.** Run locally with `-s` into a scratch output directory and a low
   `--max-parallel-jobs` before scaling. Never smoke test into a directory holding real artifacts.
5. **Size the work before submitting.** On Slurm, packing and allocation count follow directly from
   resource requirements. Check them with `torc slurm generate` and `torc slurm plan-allocations`
   before submitting; see `references/optimization.md`.
6. **Verify from server state.** After a run, check `torc status`, `torc results list`, and job
   status counts. A zero exit from `torc run` does not mean the jobs succeeded.
7. **Report evidence.** Give the mode, exact commands, workflow ID, expanded counts, output
   directory, and observed job status counts.

## Critical behavior

- `torc run` exits 0 even when jobs failed. It logs `had_failures=true` but does not propagate that
  to the exit status. Confirm with `torc status <id>` or `torc results list <id> --failed`.
  `torc exec` and `torc watch` do exit non-zero.
- `torc run` and `torc exec` write runner logs to **stdout** in table format, and to **stderr** with
  `-f json` so JSON stays parseable. Every other command logs to stderr and prints data to stdout.
- Commands that take an optional workflow ID prompt interactively when it is omitted, printing a
  selection table to **stdout** that corrupts `-f json` output, and exit 1 on EOF. With exactly one
  workflow, one is chosen silently. Always pass the ID explicitly.
- **All resource values are per node.** `num_cpus: 32` and `memory: 128g` describe what one node
  provides for that job, never a total across nodes.
- `torc submit` requires a `schedule_nodes` action. A spec without one is local-only;
  `torc create --dry-run` says which case applies.
- `torc workflows reinit` increments `run_id`, and log filenames embed it. Collect logs before
  reinitializing, or query with `--all-runs`.
- Torc records the submission directory and exposes it as `TORC_WORKFLOW_SUBMISSION_DIR`. Relative
  paths in job commands resolve against the runner's working directory, not that one.

## Spec fundamentals

A minimal spec needs only `name` and `jobs`:

```yaml
name: sweep
jobs:
  - name: prepare
    command: python prepare.py --out /data/config.json
    output_files: [config]
  - name: simulate_{temp}
    command: ./sim --config /data/config.json --temp {temp}
    input_files: [config]
    parameters:
      temp: "250:400:50"
files:
  - name: config
    path: /data/config.json
```

- Dependencies come from `depends_on` or from file and user-data edges. Prefer artifact edges: they
  document intent, drive change detection, and let Torc rerun only affected jobs.
- `{name}` tokens are workflow `variables` (constants, substituted once) or `parameters` (sweep
  dimensions that expand the graph). An undefined `{name}` is rejected as a typo.
- Workflow-level and job-level `env` values are literal strings. They never see a shell, so `$(...)`
  and `${VAR:-default}` do not expand. Put shell logic in the command or an `invocation_script`
  ending in `exec "$@"`.
- Resource requirements are named blocks referenced by jobs. They drive local packing, Slurm
  scheduler generation, node packing, and OOM/timeout recovery. A job with none cannot be
  auto-recovered.

## Choosing an execution mode

| Situation                                               | Mode                                                           |
| ------------------------------------------------------- | -------------------------------------------------------------- |
| One machine, no server, quick validation                | `torc -s run spec.yaml`                                        |
| One machine, shared server, durable history             | `torc run spec.yaml`                                           |
| Ad-hoc commands or a parallel batch, no spec file       | `torc -s exec -c '<cmd>' -j <n>`                               |
| Slurm cluster, spec already has `slurm_schedulers`      | `torc submit spec.yaml`                                        |
| Slurm cluster, portable spec with resource requirements | `torc slurm generate --account <a> spec.yaml \| torc submit -` |
| SSH-reachable machines, no scheduler                    | `torc remote add-workers` then `torc remote run`               |

Submit from a login node only. Login nodes are for spec validation, discovery (`torc hpc detect`,
`torc hpc partitions`), and submission, never builds, solvers, or payload smoke tests. Mode
trade-offs and sizing are in `references/execution-modes.md`.

## Optimizing throughput

Three numbers decide Slurm cost and time-to-solution, and all follow from resource requirements:

```text
concurrent_jobs_per_node = max(1, min(node_cpus / job_cpus, node_mem / job_mem, node_gpus / job_gpus))
time_slots               = max(1, allocation_walltime / job_runtime)
allocations              = ceil(job_count / (concurrent_jobs_per_node * time_slots)) * nodes_per_job
```

So a memory-heavy job caps how many runs share a node, a long walltime trades queue wait for more
sequential reuse of each allocation, and mixing dissimilar jobs in one partition group inflates
allocation counts because the generator takes the maximum of each dimension. Read
`references/optimization.md` before sizing a large campaign; it covers node sharing, splitting by
memory and time-to-solution, jobs-per-node versus more nodes, and the verification commands.

## Output

Report the execution mode, the commands run, the workflow ID and expanded job/file counts, the
output directory, the final job status counts from server state, and any blocker with the evidence
that identified it. Distinguish observations from inferences.
