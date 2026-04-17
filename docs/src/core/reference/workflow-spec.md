# Workflow Specification Reference

This page documents all data models used in workflow specification files. Workflow specs can be
written in YAML, JSON, JSON5, or KDL formats.

## WorkflowSpec

The top-level container for a complete workflow definition.

| Name                                             | Type                                                    | Default      | Description                                                               |
| ------------------------------------------------ | ------------------------------------------------------- | ------------ | ------------------------------------------------------------------------- |
| `name`                                           | string                                                  | _required_   | Name of the workflow                                                      |
| `user`                                           | string                                                  | current user | User who owns this workflow                                               |
| `description`                                    | string                                                  | none         | Description of the workflow                                               |
| `project`                                        | string                                                  | none         | Project name or identifier for grouping workflows                         |
| `metadata`                                       | string                                                  | none         | Arbitrary metadata as JSON string                                         |
| `parameters`                                     | map\<string, string\>                                   | none         | Shared parameters that can be used by jobs and files via `use_parameters` |
| `jobs`                                           | [[JobSpec](#jobspec)]                                   | _required_   | Jobs that make up this workflow                                           |
| `files`                                          | [[FileSpec](#filespec)]                                 | none         | Files associated with this workflow                                       |
| `user_data`                                      | [[UserDataSpec](#userdataspec)]                         | none         | User data associated with this workflow                                   |
| `resource_requirements`                          | [[ResourceRequirementsSpec](#resourcerequirementsspec)] | none         | Resource requirements available for this workflow                         |
| `failure_handlers`                               | [[FailureHandlerSpec](#failurehandlerspec)]             | none         | Failure handlers available for this workflow                              |
| `slurm_schedulers`                               | [[SlurmSchedulerSpec](#slurmschedulerspec)]             | none         | Slurm schedulers available for this workflow                              |
| `slurm_defaults`                                 | [SlurmDefaultsSpec](#slurmdefaultsspec)                 | none         | Default Slurm parameters to apply to all schedulers                       |
| `resource_monitor`                               | [ResourceMonitorConfig](#resourcemonitorconfig)         | none         | Resource monitoring configuration                                         |
| `actions`                                        | [[WorkflowActionSpec](#workflowactionspec)]             | none         | Actions to execute based on workflow/job state transitions                |
| `use_pending_failed`                             | boolean                                                 | false        | Use PendingFailed status for failed jobs (enables AI-assisted recovery)   |
| `execution_config`                               | [ExecutionConfig](#executionconfig)                     | none         | Execution mode and termination settings                                   |
| `compute_node_wait_for_new_jobs_seconds`         | integer                                                 | none         | Compute nodes wait for new jobs this long before exiting                  |
| `compute_node_ignore_workflow_completion`        | boolean                                                 | false        | Compute nodes hold allocations even after workflow completes              |
| `compute_node_wait_for_healthy_database_minutes` | integer                                                 | none         | Compute nodes wait this many minutes for database recovery                |
| `enable_ro_crate`                                | boolean                                                 | false        | Enable automatic [RO-Crate](../concepts/ro-crate.md) provenance tracking  |

### Examples with project and metadata

The `project` and `metadata` fields are useful for organizing and categorizing workflows. For more
detailed guidance on organizing workflows, see
[Organizing and Managing Workflows](../workflows/organizing-workflows.md).

**YAML example:**

```yaml
name: "ml_training_workflow"
project: "customer-churn-prediction"
metadata: '{"environment":"staging","version":"1.0.0","team":"ml-engineering"}'
description: "Train and evaluate churn prediction model"
jobs:
  - name: "preprocess"
    command: "python preprocess.py"
  - name: "train"
    command: "python train.py"
    depends_on: ["preprocess"]
```

**JSON example:**

```json
{
  "name": "data_pipeline",
  "project": "analytics-platform",
  "metadata": "{\"cost_center\":\"eng-data\",\"priority\":\"high\"}",
  "description": "Daily data processing pipeline",
  "jobs": [
    {
      "name": "extract",
      "command": "python extract.py"
    }
  ]
}
```

## JobSpec

Defines a single computational task within a workflow.

| Name                             | Type                        | Default     | Description                                                            |
| -------------------------------- | --------------------------- | ----------- | ---------------------------------------------------------------------- |
| `name`                           | string                      | _required_  | Name of the job                                                        |
| `command`                        | string                      | _required_  | Command to execute for this job                                        |
| `priority`                       | integer                     | `0`         | Scheduling priority; higher values are claimed before lower values     |
| `invocation_script`              | string                      | none        | Optional script for job invocation                                     |
| `resource_requirements`          | string                      | none        | Name of a [ResourceRequirementsSpec](#resourcerequirementsspec) to use |
| `failure_handler`                | string                      | none        | Name of a [FailureHandlerSpec](#failurehandlerspec) to use             |
| `scheduler`                      | string                      | none        | Name of the scheduler to use for this job                              |
| `cancel_on_blocking_job_failure` | boolean                     | false       | Cancel this job if a blocking job fails                                |
| `depends_on`                     | [string]                    | none        | Job names that must complete before this job runs (exact matches)      |
| `depends_on_regexes`             | [string]                    | none        | Regex patterns for job dependencies                                    |
| `input_files`                    | [string]                    | none        | File names this job reads (exact matches)                              |
| `input_file_regexes`             | [string]                    | none        | Regex patterns for input files                                         |
| `output_files`                   | [string]                    | none        | File names this job produces (exact matches)                           |
| `output_file_regexes`            | [string]                    | none        | Regex patterns for output files                                        |
| `input_user_data`                | [string]                    | none        | User data names this job reads (exact matches)                         |
| `input_user_data_regexes`        | [string]                    | none        | Regex patterns for input user data                                     |
| `output_user_data`               | [string]                    | none        | User data names this job produces (exact matches)                      |
| `output_user_data_regexes`       | [string]                    | none        | Regex patterns for output user data                                    |
| `parameters`                     | map\<string, string\>       | none        | Local parameters for generating multiple jobs                          |
| `parameter_mode`                 | string                      | `"product"` | How to combine parameters: `"product"` (Cartesian) or `"zip"`          |
| `use_parameters`                 | [string]                    | none        | Workflow parameter names to use for this job                           |
| `stdio`                          | [StdioConfig](#stdioconfig) | none        | Per-job override for stdout/stderr capture (overrides workflow-level)  |

## FileSpec

Defines input/output file artifacts that establish implicit job dependencies.

| Name             | Type                  | Default     | Description                                                   |
| ---------------- | --------------------- | ----------- | ------------------------------------------------------------- |
| `name`           | string                | _required_  | Name of the file (used for referencing in jobs)               |
| `path`           | string                | _required_  | File system path                                              |
| `parameters`     | map\<string, string\> | none        | Parameters for generating multiple files                      |
| `parameter_mode` | string                | `"product"` | How to combine parameters: `"product"` (Cartesian) or `"zip"` |
| `use_parameters` | [string]              | none        | Workflow parameter names to use for this file                 |

## UserDataSpec

Arbitrary JSON data that can establish dependencies between jobs.

| Name           | Type    | Default | Description                                          |
| -------------- | ------- | ------- | ---------------------------------------------------- |
| `name`         | string  | none    | Name of the user data (used for referencing in jobs) |
| `data`         | JSON    | none    | The data content as a JSON value                     |
| `is_ephemeral` | boolean | false   | Whether the user data is ephemeral                   |

## ResourceRequirementsSpec

Defines compute resource requirements for jobs.

| Name        | Type    | Default    | Description                                                                                 |
| ----------- | ------- | ---------- | ------------------------------------------------------------------------------------------- |
| `name`      | string  | _required_ | Name of this resource configuration (referenced by jobs)                                    |
| `num_cpus`  | integer | _required_ | Number of CPUs required                                                                     |
| `memory`    | string  | _required_ | Memory requirement (e.g., `"1m"`, `"2g"`, `"512k"`)                                         |
| `num_gpus`  | integer | `0`        | Number of GPUs required                                                                     |
| `num_nodes` | integer | `1`        | Number of nodes per job (`srun --nodes`); allocation size is set via Slurm scheduler config |
| `runtime`   | string  | `"PT1H"`   | Runtime limit in ISO8601 duration format (e.g., `"PT30M"`, `"PT2H"`)                        |

## FailureHandlerSpec

Defines error recovery strategies for jobs.

| Name    | Type                                                | Default    | Description                                      |
| ------- | --------------------------------------------------- | ---------- | ------------------------------------------------ |
| `name`  | string                                              | _required_ | Name of the failure handler (referenced by jobs) |
| `rules` | [[FailureHandlerRuleSpec](#failurehandlerrulespec)] | _required_ | Rules for handling different exit codes          |

## FailureHandlerRuleSpec

A single rule within a failure handler for handling specific exit codes.

| Name                   | Type      | Default | Description                             |
| ---------------------- | --------- | ------- | --------------------------------------- |
| `exit_codes`           | [integer] | `[]`    | Exit codes that trigger this rule       |
| `match_all_exit_codes` | boolean   | `false` | If true, matches any non-zero exit code |
| `recovery_script`      | string    | none    | Optional script to run before retrying  |
| `max_retries`          | integer   | `3`     | Maximum number of retry attempts        |

## SlurmSchedulerSpec

Defines a Slurm HPC job scheduler configuration.

| Name              | Type    | Default      | Description                                  |
| ----------------- | ------- | ------------ | -------------------------------------------- |
| `name`            | string  | none         | Name of the scheduler (used for referencing) |
| `account`         | string  | _required_   | Slurm account                                |
| `partition`       | string  | none         | Slurm partition name                         |
| `nodes`           | integer | `1`          | Number of nodes to allocate                  |
| `walltime`        | string  | `"01:00:00"` | Wall time limit                              |
| `mem`             | string  | none         | Memory specification                         |
| `gres`            | string  | none         | Generic resources (e.g., GPUs)               |
| `qos`             | string  | none         | Quality of service                           |
| `ntasks_per_node` | integer | none         | Number of tasks per node                     |
| `tmp`             | string  | none         | Temporary storage specification              |
| `extra`           | string  | none         | Additional Slurm parameters                  |

## ExecutionConfig

Controls how jobs are executed and terminated. Fields are grouped by which execution mode they apply
to. Setting a field that doesn't match the effective mode produces a validation error at workflow
creation time.

### Shared fields (both modes)

| Name                       | Type                        | Default    | Description                                            |
| -------------------------- | --------------------------- | ---------- | ------------------------------------------------------ |
| `mode`                     | string                      | `"direct"` | Execution mode: `"direct"`, `"slurm"`, or `"auto"`     |
| `sigkill_headroom_seconds` | integer                     | `60`       | Seconds before end_time for SIGKILL or srun --time     |
| `timeout_exit_code`        | integer                     | `152`      | Exit code for timed-out jobs (matches Slurm TIMEOUT)   |
| `staggered_start`          | boolean                     | `true`     | Stagger job runner startup to mitigate thundering herd |
| `stdio`                    | [StdioConfig](#stdioconfig) | see below  | Workflow-level default for stdout/stderr capture       |

### Direct mode fields

These fields only apply when the effective mode is `direct`. Setting them with `mode: slurm`
produces a validation error. When `mode: auto`, validation checks the effective mode based on
whether Slurm schedulers are present in the spec.

| Name                   | Type    | Default     | Description                                           |
| ---------------------- | ------- | ----------- | ----------------------------------------------------- |
| `limit_resources`      | boolean | `true`      | Monitor memory/CPU and kill jobs that exceed limits   |
| `termination_signal`   | string  | `"SIGTERM"` | Signal to send before SIGKILL for graceful shutdown   |
| `sigterm_lead_seconds` | integer | `30`        | Seconds before SIGKILL to send the termination signal |
| `oom_exit_code`        | integer | `137`       | Exit code for OOM-killed jobs (128 + SIGKILL)         |

### Slurm mode fields

These fields only apply when the effective mode is `slurm`. Setting them with `mode: direct`
produces a validation error. When `mode: auto`, validation checks the effective mode based on
whether Slurm schedulers are present in the spec.

| Name                      | Type    | Default | Description                             |
| ------------------------- | ------- | ------- | --------------------------------------- |
| `srun_termination_signal` | string  | none    | Signal spec for `srun --signal=<value>` |
| `enable_cpu_bind`         | boolean | `false` | Allow Slurm CPU binding (`--cpu-bind`)  |

### Worker-per-node Slurm launch field

This field applies only when `execution_config.mode: direct` is combined with a `schedule_nodes`
action that sets `start_one_worker_per_node: true`. In that mode, Torc launches one
`torc-slurm-job-runner` per allocated node with an outer `srun`, and this field is passed to that
launcher command.

| Name       | Type   | Default | Description                                                        |
| ---------- | ------ | ------- | ------------------------------------------------------------------ |
| `srun_mpi` | string | none    | MPI mode for the outer job-runner launch: `srun --mpi=<value> ...` |

### StdioConfig

Controls how stdout and stderr are captured for job processes.

| Name                | Type                    | Default      | Description                                             |
| ------------------- | ----------------------- | ------------ | ------------------------------------------------------- |
| `mode`              | [StdioMode](#stdiomode) | `"separate"` | How to capture stdout/stderr                            |
| `delete_on_success` | boolean                 | `false`      | Delete captured files when a job completes successfully |

### StdioMode

| Value       | Description                                                      |
| ----------- | ---------------------------------------------------------------- |
| `separate`  | Separate stdout (`.o`) and stderr (`.e`) files per job (default) |
| `combined`  | Combine stdout and stderr into a single `.log` file per job      |
| `no_stdout` | Discard stdout (`/dev/null`); capture stderr only                |
| `no_stderr` | Discard stderr (`/dev/null`); capture stdout only                |
| `none`      | Discard both stdout and stderr                                   |

Per-job overrides can be set via the `stdio` field on individual [JobSpec](#jobspec) entries, which
takes precedence over the workflow-level setting.

#### Stdio Examples

Combine stdout and stderr into a single file, and delete it on success:

```yaml
execution_config:
  stdio:
    mode: combined
    delete_on_success: true
```

Suppress stdout for most jobs, but keep separate files for a specific job:

```yaml
execution_config:
  stdio:
    mode: no_stdout

jobs:
  - name: preprocess
    command: python preprocess.py
  - name: train
    command: python train.py
    stdio:
      mode: separate
```

### Execution Modes

| Mode     | Description                                                             |
| -------- | ----------------------------------------------------------------------- |
| `direct` | Torc manages job execution directly (default). Works everywhere         |
| `slurm`  | Jobs wrapped with `srun`. Slurm manages resource limits and termination |
| `auto`   | Selects `slurm` if `SLURM_JOB_ID` is set, otherwise `direct`            |

> **Warning**: `auto` will silently select slurm mode when running inside a Slurm allocation. Prefer
> setting the mode explicitly to avoid unexpected behavior.

### Direct Mode Example

```yaml
execution_config:
  mode: direct
  limit_resources: true
  termination_signal: SIGTERM
  sigterm_lead_seconds: 30
  sigkill_headroom_seconds: 60
  timeout_exit_code: 152
  oom_exit_code: 137
```

### Direct Mode Worker-Per-Node Example

```yaml
execution_config:
  mode: direct
  srun_mpi: "none"

actions:
  - trigger_type: on_workflow_start
    action_type: schedule_nodes
    scheduler: multi_node
    scheduler_type: slurm
    start_one_worker_per_node: true
```

This launches the job runners with:

```console
srun --ntasks-per-node=1 --mpi=none torc-slurm-job-runner ...
```

### Slurm Mode Example

```yaml
execution_config:
  mode: slurm
  srun_termination_signal: "TERM@120"
  sigkill_headroom_seconds: 180
  enable_cpu_bind: false
```

### Termination Timeline (Direct Mode)

With `sigkill_headroom_seconds=60` and `sigterm_lead_seconds=30`:

1. `end_time - 90s`: Send SIGTERM (or configured `termination_signal`)
2. `end_time - 60s`: Send SIGKILL to remaining jobs, set exit code to `timeout_exit_code`
3. `end_time`: Job runner exits

### Slurm Mode Headroom

In Slurm mode, `sigkill_headroom_seconds` controls `srun --time`. The step time limit is set to
`remaining_time - sigkill_headroom_seconds`, allowing the job runner to detect completion before the
allocation expires.

## SlurmDefaultsSpec

Workflow-level default parameters applied to all Slurm schedulers. This is a map of parameter names
to values.

Any valid sbatch long option can be specified (without the leading `--`), except for parameters
managed by torc: `partition`, `nodes`, `walltime`, `time`, `mem`, `gres`, `name`, `job-name`.

The `account` parameter is allowed as a workflow-level default.

**Example:**

```yaml
slurm_defaults:
  qos: "high"
  constraint: "cpu"
  mail-user: "user@example.com"
  mail-type: "END,FAIL"
```

## WorkflowActionSpec

Defines conditional actions triggered by workflow or job state changes.

| Name                        | Type     | Default    | Description                                                                                               |
| --------------------------- | -------- | ---------- | --------------------------------------------------------------------------------------------------------- |
| `trigger_type`              | string   | _required_ | When to trigger: `"on_workflow_start"`, `"on_workflow_complete"`, `"on_jobs_ready"`, `"on_jobs_complete"` |
| `action_type`               | string   | _required_ | What to do: `"run_commands"`, `"schedule_nodes"`                                                          |
| `jobs`                      | [string] | none       | For job triggers: exact job names to match                                                                |
| `job_name_regexes`          | [string] | none       | For job triggers: regex patterns to match job names                                                       |
| `commands`                  | [string] | none       | For `run_commands`: commands to execute                                                                   |
| `scheduler`                 | string   | none       | For `schedule_nodes`: scheduler name                                                                      |
| `scheduler_type`            | string   | none       | For `schedule_nodes`: scheduler type (`"slurm"`, `"local"`)                                               |
| `num_allocations`           | integer  | none       | For `schedule_nodes`: number of node allocations                                                          |
| `start_one_worker_per_node` | boolean  | false      | For `schedule_nodes`: launch one worker per node (direct mode only)                                       |
| `max_parallel_jobs`         | integer  | none       | For `schedule_nodes`: maximum parallel jobs                                                               |
| `persistent`                | boolean  | false      | Whether the action persists and can be claimed by multiple workers                                        |

## ResourceMonitorConfig

Configuration for resource usage monitoring.

| Name                      | Type                                      | Default     | Description                            |
| ------------------------- | ----------------------------------------- | ----------- | -------------------------------------- |
| `enabled`                 | boolean                                   | `false`     | Enable resource monitoring             |
| `granularity`             | [MonitorGranularity](#monitorgranularity) | `"Summary"` | Level of detail for metrics collection |
| `sample_interval_seconds` | integer                                   | `10`        | Sampling interval in seconds           |
| `generate_plots`          | boolean                                   | `false`     | Generate resource usage plots          |

## MonitorGranularity

Enum specifying the level of detail for resource monitoring.

| Value        | Description                       |
| ------------ | --------------------------------- |
| `Summary`    | Collect summary statistics only   |
| `TimeSeries` | Collect detailed time series data |

## Job Priority

Use `priority` when some ready jobs should be claimed before others.

- Higher values are preferred over lower values
- The default is `0`
- `claim_next_jobs` uses a stable tie-breaker for jobs with the same priority
- `claim_jobs_based_on_resources` prefers GPU jobs first within the same priority
- Priority affects both `claim_next_jobs` and `claim_jobs_based_on_resources`

Example:

```yaml
jobs:
  - name: urgent_step
    command: ./run_urgent.sh
    priority: 100

  - name: background_step
    command: ./run_background.sh
    priority: 10
```

## Parameter Formats

Parameters support several formats for generating multiple jobs or files:

| Format                  | Example                      | Description                   |
| ----------------------- | ---------------------------- | ----------------------------- |
| Integer range           | `"1:100"`                    | Inclusive range from 1 to 100 |
| Integer range with step | `"0:100:10"`                 | Range with step size          |
| Float range             | `"0.0:1.0:0.1"`              | Float range with step         |
| Integer list            | `"[1,5,10,100]"`             | Explicit list of integers     |
| Float list              | `"[0.1,0.5,0.9]"`            | Explicit list of floats       |
| String list             | `"['adam','sgd','rmsprop']"` | Explicit list of strings      |

**Template substitution in strings:**

- Basic: `{param_name}` - Replace with parameter value
- Formatted integer: `{i:03d}` - Zero-padded (001, 042, 100)
- Formatted float: `{lr:.4f}` - Precision (0.0010, 0.1000)

See the [Job Parameterization](./parameterization.md) reference for more details.
