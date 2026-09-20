# Spec authoring

`docs/src/core/reference/workflow-spec.md` is the authoritative field list. This file covers the
semantics that decide whether a spec behaves as intended.

## Contents

- [Formats and validation](#formats-and-validation)
- [Dependency edges](#dependency-edges)
- [Variables versus parameters](#variables-versus-parameters)
- [Parameter tables from CSV or JSON](#parameter-tables-from-csv-or-json)
- [Environment and invocation scripts](#environment-and-invocation-scripts)
- [Resource requirements](#resource-requirements)
- [Failure handlers](#failure-handlers)
- [Workflow actions](#workflow-actions)
- [Execution config](#execution-config)
- [Stdio capture](#stdio-capture)
- [Resource monitoring](#resource-monitoring)
- [Slurm scheduler fields](#slurm-scheduler-fields)
- [Dynamic jobs](#dynamic-jobs)
- [Sharing a workflow](#sharing-a-workflow)

## Formats and validation

YAML, JSON, JSON5, and KDL are all accepted. The format is detected from the file extension, with
fallback parsing when the extension is missing or wrong. `-` reads the spec from stdin, and the
format is detected from the content:

```bash
torc slurm generate --account acct workflow.yaml | torc create -
```

Only `name` and `jobs` are required. Optional top-level organizational fields: `user` (defaults to
the current user), `description`, `project` for grouping related workflows, and `metadata` for
arbitrary key/value tagging. `metadata` is a **map**, not a JSON string:

```yaml
project: customer-churn
metadata:
  environment: staging
  team: ml-engineering
```

Both are queryable later, which is what makes a long-lived workflow findable.

Validate before creating anything. `torc create --dry-run` needs no server, expands parameters, and
prints the counts that will be created:

```bash
torc create --dry-run workflow.yaml
torc -f json create --dry-run workflow.yaml
```

It reports errors such as `Job 'a' depends_on non-existent job 'nonexistent'` and exits 1. It also
states whether the spec is submittable (`schedule_nodes` action present) or local-only. Check the
expanded job count here, before a bad range creates thousands of jobs.

`torc create --skip-checks` bypasses validation checks such as scheduler node requirements. Resource
validation warnings prompt for confirmation on a TTY and refuse to proceed when stdin is not a TTY.

## Dependency edges

Three independent mechanisms create edges. All are resolved by name at create time.

| Mechanism | Fields                                                           | Use when                                                 |
| --------- | ---------------------------------------------------------------- | -------------------------------------------------------- |
| Explicit  | `depends_on`, `depends_on_regexes`                               | Ordering has no artifact, or the artifact is not tracked |
| Files     | `input_files`, `output_files`, and `*_file_regexes`              | A job consumes a file another job produces               |
| User data | `input_user_data`, `output_user_data`, and `*_user_data_regexes` | A job consumes structured data another job produces      |

Prefer file and user-data edges over hidden shell ordering: they document the artifact, drive change
detection, and let Torc rerun only the affected jobs after an input changes.

Regex forms match against names, so a fan-in job can consume an entire parameterized family:

```yaml
- name: summarize
  command: python summarize.py
  input_file_regexes: ["^result_T\\d+_P\\d+$"]
```

A `FileSpec` needs `name` and `path`, and takes an optional `identifier` to override the RO-Crate
`@id` for a published input (a DOI or URN), which requires `enable_ro_crate: true`. A `UserDataSpec`
needs `name` plus a `data` object, whose string values are also parameter-substituted recursively;
set `is_ephemeral: true` for intermediate data that should be cleared between runs rather than
persisted.

`cancel_on_blocking_job_failure: true` cancels a job when a blocking job fails instead of leaving it
blocked.

## Variables versus parameters

Both use `{name}` tokens, and they compose in one string.

- `variables` are constants. Each reference is substituted once; the job count does not change.
  Values must be plain literals. A `{name}` inside a variable's value is rejected. Shell-style
  `${HOME}` is preserved verbatim and expanded at runtime.
- `parameters` are sweep dimensions. They expand jobs, files, and user data. `parameter_mode`
  selects `product` (Cartesian, default) or `zip`.
- Workflow-level `parameters` are shared and require `use_parameters` on each job or file that
  should inherit them.

Validation rejects any `{name}` token that is neither a variable nor a parameter, so typos fail
fast. Tokens with non-identifier contents (`find ... {} \;`) and `${...}` shell expansions are left
alone.

```yaml
variables:
  data_root: /scratch/proj42
jobs:
  - name: "train_{i:02d}"
    command: "python train.py --shard {i} --in {data_root}/clean"
    parameters:
      i: "1:4"
```

Format specifiers (`{i:02d}`) work in names, commands, and paths. Use the same tokens in a file's
`name` and `path` so each expanded job gets its own file entity.

## Parameter tables from CSV or JSON

`parameters` builds combinations from independent axes. When you already have an explicit table of
combinations, generated by another tool or an irregular set that is not a full grid, point at it
with `parameters_file` instead. Each CSV row or JSON object becomes exactly one generated instance,
and its columns become substitution tokens.

```yaml
jobs:
  - name: train_{model}_{dataset}_bs{batch_size}
    command: python train.py --model {model} --lr {lr} --bs {batch_size}
    parameters_file: sweeps/sweep.csv
```

- Format comes from the extension: `.csv`, `.json` (an array of objects), or `.jsonl` / `.ndjson`
  (one object per line).
- Relative paths resolve against the current working directory, not the spec file.
- CSV cells are inferred integer, then float, then string, so numeric columns work with specifiers
  like `{lr:.4f}`. JSON keeps native types; nested or non-scalar values are stringified.
- Mutually exclusive with `parameters`, `parameter_mode`, and `use_parameters` on the same entity.
- Available on jobs, files, and user data; a workflow-level `parameters_file` supplies a shared
  table.

Use this when the sweep comes from elsewhere or is not a Cartesian product, and `parameters` when
the axes are independent and you want a product or zip.

## Environment and invocation scripts

`env` maps exist at the workflow level and per job. Job-level values win on key conflicts, and the
merged map is stored on the job at create time.

Values are literal strings. Nothing in `env` is evaluated by a shell, so `$(hostname)`, `$TMPDIR`,
and `${VAR:-default}` do not expand there. Put anything dynamic in the job command or in an
`invocation_script`.

An `invocation_script` wraps the job command, which is the right place for module loads, conda
activation, and interpreter selection. End the wrapper with `exec "$@"` so signals and exit codes
reach the real process.

Torc exports these variables to every job: `TORC_WORKFLOW_ID`, `TORC_RUN_ID`, `TORC_JOB_ID`,
`TORC_JOB_NAME`, `TORC_ATTEMPT_ID`, `TORC_API_URL`, `TORC_OUTPUT_DIR`, and
`TORC_WORKFLOW_SUBMISSION_DIR` when the workflow recorded one. Recovery scripts additionally get
`TORC_RETURN_CODE`.

## Resource requirements

```yaml
resource_requirements:
  - name: compute
    num_cpus: 32
    memory: 64g
    runtime: PT2H
```

- `num_cpus` and `memory` are required; `memory` uses suffixed strings (`512k`, `8g`).
- `runtime` is an ISO8601 duration and defaults to `PT1H`.
- `num_gpus` defaults to 0, `num_nodes` to 1. `num_nodes` sets `srun --nodes` for the job; the
  allocation size comes from the Slurm scheduler config.

Resource requirements matter beyond bookkeeping:

- The local runner packs jobs against available CPU/memory/GPU, so wrong values change parallelism.
- `torc slurm generate` maps requirements to partitions and derives walltime from them.
- Runtime gates node packing: a ready job whose runtime exceeds an allocation's remaining walltime
  will not start. `torc workflows diagnose` reports exactly this case.
- Recovery multiplies memory and runtime on OOM and timeout failures, so a job with no resource
  requirements cannot be auto-recovered.

A local-only workflow generally needs no resource requirements at all.

## Failure handlers

A handler is a named set of rules matched against a job's exit code:

```yaml
failure_handlers:
  - name: retry_transient
    rules:
      - exit_codes: [1, 137]
        max_retries: 3
        recovery_script: bash cleanup_partial.sh
jobs:
  - name: flaky
    command: ./flaky.sh
    failure_handler: retry_transient
```

`match_all_exit_codes: true` matches any non-zero exit. `max_retries` defaults to 3. A retry
increments `TORC_ATTEMPT_ID`, which appears in log filenames, so attempts do not overwrite each
other.

A job that fails with no matching rule normally becomes `failed`. With `use_pending_failed: true` at
the workflow level it becomes `pending_failed` instead, awaiting classification.

## Workflow actions

Actions react to state transitions. They are not dependency edges: an action failure does not block
downstream jobs. If setup must gate downstream work, model it as a real job with an output file.

| Field                                               | Notes                                                                            |
| --------------------------------------------------- | -------------------------------------------------------------------------------- |
| `trigger_type`                                      | `on_workflow_start`, `on_workflow_complete`, `on_jobs_ready`, `on_jobs_complete` |
| `action_type`                                       | `run_commands` or `schedule_nodes`                                               |
| `jobs` / `job_name_regexes`                         | Which jobs a job-scoped trigger matches                                          |
| `scheduler`, `num_allocations`, `max_parallel_jobs` | `schedule_nodes` parameters                                                      |

For `schedule_nodes`, prefer `on_jobs_ready` gated on the jobs the allocation runs, even for root
jobs. Root jobs are ready at init, so the action still fires at the start, but tying it to jobs
makes a selective rerun re-schedule only the reset jobs. An `on_workflow_start` `schedule_nodes`
action is kept across reinitialize and `torc submit` cannot re-fire it.

Use `on_workflow_complete` only for narrowly scoped cleanup.

Two more action fields: `scheduler_type` selects `slurm` or `local`, and `persistent: true` keeps
the action claimable by multiple workers instead of firing once.

## Execution config

`execution_config.mode` selects `direct` (the default), `slurm`, or `auto`. Under `auto` the
effective mode is resolved **by the runner from its own environment**: `slurm` when `SLURM_JOB_ID`
is set, `direct` otherwise. It does not look at the spec's schedulers, so the same spec can run
direct locally and under `srun` inside an allocation. The remaining fields are mode-gated, and
setting one that does not match the effective mode is a validation error at creation, not a silently
ignored value.

| Field                      | Mode   | Default   | Purpose                                              |
| -------------------------- | ------ | --------- | ---------------------------------------------------- |
| `limit_resources`          | direct | `true`    | Monitor and kill jobs exceeding declared limits      |
| `termination_signal`       | direct | `SIGTERM` | Signal sent before SIGKILL                           |
| `sigterm_lead_seconds`     | direct | `30`      | Lead time before SIGKILL                             |
| `oom_exit_code`            | direct | `137`     | Exit code recorded for OOM-killed jobs               |
| `srun_termination_signal`  | slurm  | none      | Passed to `srun --signal=<value>`                    |
| `enable_cpu_bind`          | slurm  | `false`   | Allow Slurm CPU binding (`--cpu-bind`)               |
| `srun_mpi`                 | slurm  | none      | `srun --mpi=<value>` for worker-per-node launches    |
| `sigkill_headroom_seconds` | both   | `60`      | Headroom before end time for SIGKILL / `srun --time` |
| `timeout_exit_code`        | both   | `152`     | Exit code for timed-out jobs (matches Slurm TIMEOUT) |
| `staggered_start`          | both   | `true`    | Stagger runner startup to avoid a thundering herd    |
| `stdio`                    | both   | see below | Workflow-level stdout/stderr capture                 |

`srun_mpi` applies only when `mode: direct` is combined with a `schedule_nodes` action setting
`start_one_worker_per_node: true`, since that is the only path with an outer `srun` launching
runners.

`srun_termination_signal` (for example `"TERM@300"`) is what makes graceful checkpointing possible:
the job catches SIGTERM, saves state, and exits 0. See `failure-analysis.md` for why that reads as
`completed` rather than `terminated`.

## Stdio capture

`execution_config.stdio` sets the workflow default; a job's `stdio` overrides it.

| Mode        | Files produced per attempt |
| ----------- | -------------------------- |
| `separate`  | `.o` and `.e` (default)    |
| `combined`  | one `.log`                 |
| `no_stdout` | `.e` only                  |
| `no_stderr` | `.o` only                  |
| `none`      | nothing                    |

`delete_on_success: true` removes the captured files when a job exits 0. Both of these change what a
later debugging session can find, so do not enable them on a workflow you expect to debug.

## Resource monitoring

`resource_monitor` controls per-job and compute-node sampling:

```yaml
resource_monitor:
  sample_interval_seconds: 5
  jobs:
    enabled: true
    granularity: summary
  compute_node:
    enabled: true
    granularity: time_series
```

`summary` records peak and average CPU/memory per job, which is what `torc results list` and
`torc workflows check-resources` display. `time_series` writes samples into a SQLite database and is
required for `torc plot-resources`. Both scopes share `sample_interval_seconds`; time-series samples
flush every `flush_interval_seconds` (default 300).

## Slurm scheduler fields

A `slurm_schedulers` entry carries `name`, `account` (required), `partition`, `nodes` (default 1),
`walltime` (default `01:00:00`), `mem`, `gres`, `qos`, `ntasks_per_node`, `tmp`, `extra`, and
`serialize_allocations`. Jobs bind to one by name with the job's `scheduler` field.

`extra` passes additional sbatch text through. `serialize_allocations` chains a scheduler's
allocations one at a time via `--dependency=singleton`; see `optimization.md`.

For options that should apply to every scheduler, including generated ones, use `slurm_defaults`. It
is a free-form map of sbatch long option names without the leading dashes:

```yaml
slurm_defaults:
  account: my_project
  qos: high
  constraint: "cpu"
  mail-type: "END,FAIL"
  mail-user: me@example.gov
```

Torc manages `partition`, `nodes`, `walltime`/`time`, `mem`, `gres`, and `name`/`job-name` itself,
so putting any of those in `slurm_defaults` is rejected with the offending keys listed. `account` is
deliberately allowed there as a workflow-level default. Anything else valid for sbatch is accepted
and not validated by Torc, so a typo surfaces as an sbatch rejection at submit time.

`torc create --dry-run` runs this check too, so a clean dry-run does cover `slurm_defaults`.

## Dynamic jobs

When the iteration count is only known at runtime, a job can extend the workflow while it runs by
calling `spawn_jobs` through the API or a client. This covers loops that run until convergence,
which a static DAG cannot express.

```yaml
dynamic_jobs:
  max_iterations: 20
```

`max_iterations` caps `spawn_jobs` calls per orchestrator lineage, which is the guard against a
runaway loop; omit it to take the server default. It must be at least 1, and the spec loader rejects
0 or a negative value up front rather than letting the first `spawn_jobs` call fail confusingly. The
field is runtime-immutable. Prefer a static graph when the work is known, and reach for this only
when it genuinely is not.

## Sharing a workflow

`access_groups` grants named groups access at creation time:

```yaml
access_groups:
  - naerm
  - data-team
```

Every name must match an existing group, and the create fails if one does not, leaving no workflow
row behind. This is a declarative shortcut for creation only. Use `torc access-groups add-workflow`
and `remove-workflow` for changes afterward.

Runnable examples for every feature above live in `examples/yaml/`, and `examples/parameter_tables/`
holds CSV and JSON sweep tables.
