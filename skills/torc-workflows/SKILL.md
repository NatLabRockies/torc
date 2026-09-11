---
name: torc-workflows
description: >
  Author Torc workflow specifications and run them locally, on Slurm/HPC, or across SSH remote
  workers. Use for torc create/run/exec/submit, YAML/JSON5/JSON/KDL specs, job dependencies, file
  and user-data edges, parameter sweeps, resource requirements, workflow actions, invocation
  scripts, failure handlers, standalone mode, or reruns after a partial failure. Do not use for
  generic Slurm, SSH, or shell questions with no Torc workflow involved.
license: BSD-3-Clause
---

# Torc workflows

Torc turns one declarative spec into a dependency graph of jobs that a runner claims and executes.
The server owns all state in SQLite; the CLI is a client. Nothing runs until a workflow exists on a
server and has been initialized.

Add control in layers and stop at the first layer that solves the problem:

```text
dry-run validation -> local standalone smoke test -> shared-server local run -> Slurm submit -> remote workers
```

## Task router

Read only the reference that matches the task.

| Task                                                             | Read                               |
| ---------------------------------------------------------------- | ---------------------------------- |
| Write or review a spec: dependencies, files, parameters, actions | `references/spec-authoring.md`     |
| Run locally, pick standalone vs shared server, use `torc exec`   | `references/local-execution.md`    |
| Generate schedulers, submit to Slurm, size allocations           | `references/slurm.md`              |
| Run jobs on SSH-reachable machines                               | `references/remote-workers.md`     |
| Rerun part of a workflow, reset state, resume after edits        | `references/rerun-and-recovery.md` |
| Command preconditions, exit codes, stream routing, prompting     | `references/command-behavior.md`   |

## Core loop

1. **Confirm the target.** Identify the server (`torc ping`), the workflow spec or ID, and the
   execution mode. `torc --help` groups commands; most subcommand groups are hidden from the top
   level, so use `torc <group> --help` for the full list.
2. **Validate offline first.** `torc create --dry-run <spec>` parses the spec, expands parameters,
   and reports the resulting job/file/action counts without a server. It exits non-zero when
   validation fails. Always check the expanded job count before creating a sweep.
3. **Smoke test small.** Run locally with `-s` (standalone) into a scratch output directory and a
   low `--max-parallel-jobs` before scaling. Never smoke test into the directory that holds real
   artifacts.
4. **Create, then execute.** `torc run` and `torc submit` accept either a spec path or an existing
   workflow ID, so a create step is optional. Use an explicit `torc create` when the workflow must
   be reviewed or scheduled later.
5. **Verify from server state.** After a run, check `torc status`, `torc results list`, and job
   status counts. A zero exit from `torc run` does not mean the jobs succeeded.
6. **Report evidence.** Give the mode, exact commands, workflow ID, expanded counts, output
   directory, and the observed job status counts.

## Critical behavior

- `torc run` exits 0 even when jobs failed. It reports `had_failures=true` in its log line but does
  not propagate that to the exit status. Always confirm with `torc status <id>` or
  `torc results list <id> --failed`. `torc exec` does exit 1 when a job fails.
- `torc run` and `torc exec` write runner logs to **stdout** in table format, and to **stderr** when
  `-f json` is used, so JSON output stays parseable. Every other command logs to stderr and prints
  data to stdout.
- `torc submit` requires a `schedule_nodes` action in the workflow. A spec without one is
  local-only; `torc create --dry-run` states which case applies.
- Commands that take an optional workflow ID prompt interactively when it is omitted, and exit 1
  when the prompt cannot be satisfied. Always pass the ID in scripts and agent runs.
- The workflow's `run_id` increments on `torc workflows reinit`, and log filenames embed it. Do not
  reinitialize while looking for logs from a previous run.
- Torc records the submission directory at create/run/submit time and exposes it to jobs as
  `TORC_WORKFLOW_SUBMISSION_DIR`. Relative paths in job commands resolve against the runner's
  working directory, not the submission directory.

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

- Dependencies come from `depends_on` (explicit) or from file and user-data edges (implicit). Both
  are resolved by name at create time and converted to IDs.
- `{name}` tokens are workflow `variables` (constants, substituted once) or `parameters` (sweep
  dimensions, expand the graph). An undefined `{name}` is rejected as a typo.
- Workflow-level `env` and job-level `env` values are literal strings. They are not evaluated by a
  shell, so `$(...)` and `${VAR:-default}` do not work there. Put shell logic in the job command or
  an `invocation_script`.
- Resource requirements are named blocks referenced by jobs. They are required for Slurm scheduler
  generation and are what OOM/timeout recovery adjusts.

Details, formats, and action semantics: `references/spec-authoring.md`.

## Mode selection

| Situation                                               | Mode                                                              |
| ------------------------------------------------------- | ----------------------------------------------------------------- |
| One machine, no server running, quick validation        | `torc -s run spec.yaml`                                           |
| One machine, shared server, durable workflow history    | `torc run spec.yaml`                                              |
| Ad-hoc commands or a parallel batch with no spec file   | `torc -s exec -c '<cmd>' -j <n>`                                  |
| Slurm cluster, spec already has `slurm_schedulers`      | `torc submit spec.yaml`                                           |
| Slurm cluster, portable spec with resource requirements | `torc slurm generate --account <acct> spec.yaml \| torc submit -` |
| SSH-reachable machines, no scheduler                    | `torc remote add-workers` then `torc remote run`                  |

Submit from a login node only. Do not run builds, installs, solvers, or payload smoke tests there;
login nodes are for spec validation, discovery (`torc hpc detect`, `torc hpc partitions`), and
submission.

## Output

Report the execution mode, the commands run, the workflow ID and expanded job/file counts, the
output directory, the final job status counts from server state, and any blocker with the evidence
that identified it.
