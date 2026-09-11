# Spec authoring

`docs/src/core/reference/workflow-spec.md` is the authoritative field list. This file covers the
semantics that decide whether a spec behaves as intended.

## Contents

- [Formats and validation](#formats-and-validation)
- [Dependency edges](#dependency-edges)
- [Variables versus parameters](#variables-versus-parameters)
- [Environment and invocation scripts](#environment-and-invocation-scripts)
- [Resource requirements](#resource-requirements)
- [Failure handlers](#failure-handlers)
- [Workflow actions](#workflow-actions)
- [Stdio capture](#stdio-capture)
- [Resource monitoring](#resource-monitoring)

## Formats and validation

YAML, JSON, JSON5, and KDL are all accepted. The format is detected from the file extension, with
fallback parsing when the extension is missing or wrong. `-` reads the spec from stdin, and the
format is detected from the content:

```bash
torc slurm generate --account acct workflow.yaml | torc create -
```

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

Runnable examples for every feature above live in `examples/yaml/`.
