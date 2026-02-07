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
| `compute_node_expiration_buffer_seconds`         | integer                                                 | none         | Shut down compute nodes this many seconds before expiration               |
| `compute_node_wait_for_new_jobs_seconds`         | integer                                                 | none         | Compute nodes wait for new jobs this long before exiting                  |
| `compute_node_ignore_workflow_completion`        | boolean                                                 | false        | Compute nodes hold allocations even after workflow completes              |
| `compute_node_wait_for_healthy_database_minutes` | integer                                                 | none         | Compute nodes wait this many minutes for database recovery                |
| `jobs_sort_method`                               | [ClaimJobsSortMethod](#claimjobssortmethod)             | `none`       | Method for sorting jobs when claiming them                                |

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

| Name                             | Type                  | Default     | Description                                                            |
| -------------------------------- | --------------------- | ----------- | ---------------------------------------------------------------------- |
| `name`                           | string                | _required_  | Name of the job                                                        |
| `command`                        | string                | _required_  | Command to execute for this job                                        |
| `invocation_script`              | string                | none        | Optional script for job invocation                                     |
| `resource_requirements`          | string                | none        | Name of a [ResourceRequirementsSpec](#resourcerequirementsspec) to use |
| `failure_handler`                | string                | none        | Name of a [FailureHandlerSpec](#failurehandlerspec) to use             |
| `scheduler`                      | string                | none        | Name of the scheduler to use for this job                              |
| `cancel_on_blocking_job_failure` | boolean               | false       | Cancel this job if a blocking job fails                                |
| `supports_termination`           | boolean               | false       | Whether this job supports graceful termination                         |
| `depends_on`                     | [string]              | none        | Job names that must complete before this job runs (exact matches)      |
| `depends_on_regexes`             | [string]              | none        | Regex patterns for job dependencies                                    |
| `input_files`                    | [string]              | none        | File names this job reads (exact matches)                              |
| `input_file_regexes`             | [string]              | none        | Regex patterns for input files                                         |
| `output_files`                   | [string]              | none        | File names this job produces (exact matches)                           |
| `output_file_regexes`            | [string]              | none        | Regex patterns for output files                                        |
| `input_user_data`                | [string]              | none        | User data names this job reads (exact matches)                         |
| `input_user_data_regexes`        | [string]              | none        | Regex patterns for input user data                                     |
| `output_user_data`               | [string]              | none        | User data names this job produces (exact matches)                      |
| `output_user_data_regexes`       | [string]              | none        | Regex patterns for output user data                                    |
| `parameters`                     | map\<string, string\> | none        | Local parameters for generating multiple jobs                          |
| `parameter_mode`                 | string                | `"product"` | How to combine parameters: `"product"` (Cartesian) or `"zip"`          |
| `use_parameters`                 | [string]              | none        | Workflow parameter names to use for this job                           |

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

| Name        | Type    | Default    | Description                                                          |
| ----------- | ------- | ---------- | -------------------------------------------------------------------- |
| `name`      | string  | _required_ | Name of this resource configuration (referenced by jobs)             |
| `num_cpus`  | integer | _required_ | Number of CPUs required                                              |
| `memory`    | string  | _required_ | Memory requirement (e.g., `"1m"`, `"2g"`, `"512k"`)                  |
| `num_gpus`  | integer | `0`        | Number of GPUs required                                              |
| `num_nodes` | integer | `1`        | Number of nodes required                                             |
| `runtime`   | string  | `"PT1H"`   | Runtime limit in ISO8601 duration format (e.g., `"PT30M"`, `"PT2H"`) |

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
| `start_one_worker_per_node` | boolean  | none       | For `schedule_nodes`: start one worker per allocated node                                                 |
| `max_parallel_jobs`         | integer  | none       | For `schedule_nodes`: maximum parallel jobs                                                               |
| `persistent`                | boolean  | false      | Whether the action persists and can be claimed by multiple workers                                        |

## ResourceMonitorConfig

Configuration for resource usage monitoring.

| Name                      | Type                                      | Default     | Description                            |
| ------------------------- | ----------------------------------------- | ----------- | -------------------------------------- |
| `enabled`                 | boolean                                   | `false`     | Enable resource monitoring             |
| `granularity`             | [MonitorGranularity](#monitorgranularity) | `"Summary"` | Level of detail for metrics collection |
| `sample_interval_seconds` | integer                                   | `5`         | Sampling interval in seconds           |
| `generate_plots`          | boolean                                   | `false`     | Generate resource usage plots          |

## MonitorGranularity

Enum specifying the level of detail for resource monitoring.

| Value        | Description                       |
| ------------ | --------------------------------- |
| `Summary`    | Collect summary statistics only   |
| `TimeSeries` | Collect detailed time series data |

## ClaimJobsSortMethod

Enum specifying how jobs are sorted when being claimed by workers.

| Value                 | Description                             |
| --------------------- | --------------------------------------- |
| `none`                | No sorting (default)                    |
| `gpus_runtime_memory` | Sort by GPUs, then runtime, then memory |
| `gpus_memory_runtime` | Sort by GPUs, then memory, then runtime |

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
