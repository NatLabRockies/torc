# CLI Reference

This document contains the help content for the `torc` command-line program.

**Command Overview:**

- [`torc`](#torc)
- [`torc run`](#torc-run)
- [`torc submit`](#torc-submit)
- [`torc submit-slurm`](#torc-submit-slurm)
- [`torc watch`](#torc-watch)
- [`torc recover`](#torc-recover)
- [`torc workflows`](#torc-workflows)
- [`torc compute-nodes`](#torc-compute-nodes)
- [`torc files`](#torc-files)
- [`torc jobs`](#torc-jobs)
- [`torc job-dependencies`](#torc-job-dependencies)
- [`torc resource-requirements`](#torc-resource-requirements)
- [`torc events`](#torc-events)
- [`torc results`](#torc-results)
- [`torc user-data`](#torc-user-data)
- [`torc slurm`](#torc-slurm)
- [`torc remote`](#torc-remote)
- [`torc scheduled-compute-nodes`](#torc-scheduled-compute-nodes)
- [`torc hpc`](#torc-hpc)
- [`torc reports`](#torc-reports)
- [`torc config`](#torc-config)
- [`torc tui`](#torc-tui)
- [`torc plot-resources`](#torc-plot-resources)
- [`torc completions`](#torc-completions)

## `torc`

Torc workflow orchestration system

**Usage:** `torc [OPTIONS] <COMMAND>`

###### **Subcommands:**

- `run` — Run a workflow locally (create from spec file or run existing workflow by ID)
- `submit` — Submit a workflow to scheduler (create from spec file or submit existing workflow by
  ID)
- `submit-slurm` — Submit a workflow to Slurm with auto-generated schedulers
- `watch` — Watch a workflow and automatically recover from failures
- `recover` — Recover a Slurm workflow from failures (one-shot)
- `workflows` — Workflow management commands
- `compute-nodes` — Compute node management commands
- `files` — File management commands
- `jobs` — Job management commands
- `job-dependencies` — Job dependency and relationship queries
- `resource-requirements` — Resource requirements management commands
- `events` — Event management commands
- `results` — Result management commands
- `user-data` — User data management commands
- `slurm` — Slurm scheduler commands
- `remote` — Remote worker execution commands (SSH-based distributed execution)
- `scheduled-compute-nodes` — Scheduled compute node management commands
- `hpc` — HPC system profiles and partition information
- `reports` — Generate reports and analytics
- `config` — Manage configuration files and settings
- `tui` — Interactive terminal UI for managing workflows
- `plot-resources` — Generate interactive HTML plots from resource monitoring data
- `completions` — Generate shell completions

###### **Options:**

- `--log-level <LOG_LEVEL>` — Log level (error, warn, info, debug, trace)
- `-f`, `--format <FORMAT>` — Output format (table or json). Default: `table`
- `--url <URL>` — URL of torc server
- `--username <USERNAME>` — Username for basic authentication
- `--password <PASSWORD>` — Password for basic authentication (will prompt if username provided but
  password not)

## `torc run`

Run a workflow locally (create from spec file or run existing workflow by ID)

**Usage:** `torc run [OPTIONS] <WORKFLOW_SPEC_OR_ID>`

###### **Arguments:**

- `<WORKFLOW_SPEC_OR_ID>` — Path to workflow spec file (JSON/JSON5/YAML) or workflow ID

###### **Options:**

- `--max-parallel-jobs <MAX_PARALLEL_JOBS>` — Maximum number of parallel jobs to run concurrently
- `--num-cpus <NUM_CPUS>` — Number of CPUs available
- `--memory-gb <MEMORY_GB>` — Memory in GB
- `--num-gpus <NUM_GPUS>` — Number of GPUs available
- `-p`, `--poll-interval <POLL_INTERVAL>` — Job completion poll interval in seconds
- `-o`, `--output-dir <OUTPUT_DIR>` — Output directory for jobs
- `--skip-checks` — Skip validation checks (e.g., scheduler node requirements). Use with caution.
  Default: `false`

## `torc submit`

Submit a workflow to scheduler (create from spec file or submit existing workflow by ID)

Requires workflow to have an on_workflow_start action with schedule_nodes. For Slurm workflows
without pre-configured schedulers, use `submit-slurm` instead.

**Usage:** `torc submit [OPTIONS] <WORKFLOW_SPEC_OR_ID>`

###### **Arguments:**

- `<WORKFLOW_SPEC_OR_ID>` — Path to workflow spec file (JSON/JSON5/YAML) or workflow ID

###### **Options:**

- `-i`, `--ignore-missing-data` — Ignore missing data. Default: `false`
- `--skip-checks` — Skip validation checks (e.g., scheduler node requirements). Use with caution.
  Default: `false`

## `torc submit-slurm`

Submit a workflow to Slurm with auto-generated schedulers

Automatically generates Slurm schedulers based on job resource requirements and HPC profile.

WARNING: This command uses heuristics to generate schedulers and workflow actions. For complex
workflows with unusual dependency patterns, the generated configuration may not be optimal and could
waste allocation time.

RECOMMENDED: Preview the generated configuration first with:

```bash
torc slurm generate --account <account> workflow.yaml
```

Review the schedulers and actions to ensure they are appropriate for your workflow before
submitting. You can save the output and submit manually:

```bash
torc slurm generate --account <account> -o workflow_with_schedulers.yaml workflow.yaml
torc submit workflow_with_schedulers.yaml
```

**Usage:** `torc submit-slurm [OPTIONS] --account <ACCOUNT> <WORKFLOW_SPEC>`

###### **Arguments:**

- `<WORKFLOW_SPEC>` — Path to workflow spec file (JSON/JSON5/YAML/KDL)

###### **Options:**

- `--account <ACCOUNT>` — Slurm account to use for allocations
- `--hpc-profile <HPC_PROFILE>` — HPC profile to use (auto-detected if not specified)
- `--single-allocation` — Bundle all nodes into a single Slurm allocation per scheduler. By default,
  creates one Slurm allocation per node (N×1 mode), which allows jobs to start as nodes become
  available and provides better fault tolerance. With this flag, creates one large allocation with
  all nodes (1×N mode), which requires all nodes to be available simultaneously but uses a single
  sbatch.
- `-i`, `--ignore-missing-data` — Ignore missing data. Default: `false`
- `--skip-checks` — Skip validation checks (e.g., scheduler node requirements). Use with caution.
  Default: `false`

## `torc watch`

Watch a workflow and automatically recover from failures.

Monitors a workflow until completion. With `--recover`, automatically diagnoses failures, adjusts
resource requirements, and resubmits jobs.

**Usage:** `torc watch [OPTIONS] <WORKFLOW_ID>`

### Usage Modes

1. **Basic monitoring** (no recovery):

   ```bash
   torc watch 123
   ```

   Reports failures and exits. Use for manual intervention or AI-assisted recovery.

2. **With automatic recovery** (`--recover`):

   ```bash
   torc watch 123 --recover
   ```

   Automatically diagnoses OOM/timeout failures, adjusts resources, and retries. Runs until all jobs
   complete or max retries exceeded.

3. **With auto-scheduling** (`--auto-schedule`):

   ```bash
   torc watch 123 --auto-schedule
   ```

   Automatically submits new Slurm allocations when retry jobs are waiting. Essential for workflows
   using failure handlers that create retry jobs.

### Arguments

- `<WORKFLOW_ID>` — Workflow ID to watch

### Options

**Polling:**

- `-p`, `--poll-interval <POLL_INTERVAL>` — Poll interval in seconds. Default: `60`
- `-o`, `--output-dir <OUTPUT_DIR>` — Output directory for job files. Default: `output`
- `-s`, `--show-job-counts` — Show job counts by status during polling. WARNING: Can cause high
  server load for large workflows.

**Recovery:**

- `-r`, `--recover` — Enable automatic failure recovery
- `-m`, `--max-retries <MAX_RETRIES>` — Maximum number of recovery attempts. Default: `3`
- `--memory-multiplier <MEMORY_MULTIPLIER>` — Memory multiplier for OOM failures. Default: `1.5`
- `--runtime-multiplier <RUNTIME_MULTIPLIER>` — Runtime multiplier for timeout failures. Default:
  `1.5`
- `--retry-unknown` — Also retry jobs with unknown failure causes (not just OOM or timeout)
- `--recovery-hook <RECOVERY_HOOK>` — Custom recovery script for unknown failures. The workflow ID
  is passed as an argument and via `TORC_WORKFLOW_ID` environment variable.

**Auto-scheduling:**

- `--auto-schedule` — Automatically schedule new compute nodes when needed
- `--auto-schedule-threshold <N>` — Minimum retry jobs before auto-scheduling when schedulers exist.
  Default: `5`
- `--auto-schedule-cooldown <SECONDS>` — Cooldown between auto-schedule attempts. Default: `1800`
  (30 min)
- `--auto-schedule-stranded-timeout <SECONDS>` — Schedule stranded jobs after this timeout even if
  below threshold. Default: `7200` (2 hrs). Set to `0` to disable.

### Auto-Scheduling Behavior

When `--auto-schedule` is enabled:

1. **No schedulers available**: Immediately submits new allocations if ready jobs exist.
2. **Threshold exceeded**: If retry jobs (attempt_id > 1) exceed `--auto-schedule-threshold` while
   schedulers are running, submits additional allocations after cooldown.
3. **Stranded jobs**: If retry jobs are below threshold but waiting longer than
   `--auto-schedule-stranded-timeout`, schedules anyway to prevent indefinite waiting.

### Examples

```bash
# Basic: watch until completion, report failures
torc watch 123

# Recovery: automatically fix OOM/timeout failures
torc watch 123 --recover

# Recovery with aggressive resource increases
torc watch 123 --recover --memory-multiplier 2.0 --runtime-multiplier 2.0

# Recovery including unknown failures (transient errors)
torc watch 123 --recover --retry-unknown

# Auto-schedule: ensure retry jobs get scheduled
torc watch 123 --auto-schedule

# Full production setup: recovery + auto-scheduling
torc watch 123 --recover --auto-schedule

# Custom auto-schedule settings
torc watch 123 --auto-schedule \
    --auto-schedule-threshold 10 \
    --auto-schedule-cooldown 3600 \
    --auto-schedule-stranded-timeout 14400
```

### See Also

- [`torc recover`](#torc-recover) — One-shot recovery (no continuous monitoring)
- [Automatic Failure Recovery](../../specialized/fault-tolerance/automatic-recovery.md) — Detailed
  guide

## `torc recover`

Recover a Slurm workflow from failures (one-shot).

Diagnoses job failures (OOM, timeout), adjusts resource requirements, and resubmits jobs. Use after
a workflow has completed with failures. For continuous monitoring, use `torc watch --recover`
instead.

**Usage:** `torc recover [OPTIONS] <WORKFLOW_ID>`

### Arguments

- `<WORKFLOW_ID>` — Workflow ID to recover

### Options

- `-o`, `--output-dir <OUTPUT_DIR>` — Output directory for job files. Default: `output`
- `--memory-multiplier <MEMORY_MULTIPLIER>` — Memory multiplier for OOM failures. Default: `1.5`
- `--runtime-multiplier <RUNTIME_MULTIPLIER>` — Runtime multiplier for timeout failures. Default:
  `1.4`
- `--retry-unknown` — Also retry jobs with unknown failure causes
- `--recovery-hook <RECOVERY_HOOK>` — Custom recovery script for unknown failures
- `--dry-run` — Show what would be done without making any changes

### When to Use

Use `torc recover` for:

- One-shot recovery after a workflow has completed with failures
- Manual investigation before retrying (use `--dry-run` first)
- Workflows where you want to inspect failures before retrying

Use `torc watch --recover` instead for:

- Continuous monitoring of long-running workflows
- Fully automated recovery without manual intervention
- Production workflows that should self-heal

### Examples

```bash
# Basic recovery
torc recover 123

# Dry run to preview changes without modifying anything
torc recover 123 --dry-run

# Custom resource multipliers
torc recover 123 --memory-multiplier 2.0 --runtime-multiplier 1.5

# Also retry unknown failures (not just OOM/timeout)
torc recover 123 --retry-unknown

# With custom recovery hook for domain-specific fixes
torc recover 123 --recovery-hook 'bash fix-cluster.sh'
```

### See Also

- [`torc watch --recover`](#torc-watch) — Continuous monitoring with automatic recovery
- [Automatic Failure Recovery](../../specialized/fault-tolerance/automatic-recovery.md) — Detailed
  guide

## `torc workflows`

Workflow management commands

**Usage:** `torc workflows <COMMAND>`

###### **Subcommands:**

- `create` — Create a workflow from a specification file (supports JSON, JSON5, YAML, and KDL
  formats)
- `create-slurm` — Create a workflow with auto-generated Slurm schedulers
- `new` — Create a new empty workflow
- `list` — List workflows
- `get` — Get a specific workflow by ID
- `update` — Update an existing workflow
- `cancel` — Cancel a workflow and all associated Slurm jobs
- `delete` — Delete one or more workflows
- `archive` — Archive or unarchive one or more workflows
- `submit` — Submit a workflow: initialize if needed and schedule nodes for on_workflow_start
  actions. This command requires the workflow to have an on_workflow_start action with
  schedule_nodes
- `run` — Run a workflow locally on the current node
- `initialize` — Initialize a workflow, including all job statuses
- `reinitialize` — Reinitialize a workflow. This will reinitialize all jobs with a status of
  canceled, submitting, pending, or terminated. Jobs with a status of done will also be
  reinitialized if an input_file or user_data record has changed
- `status` — Get workflow status
- `reset-status` — Reset workflow and job status
- `execution-plan` — Show the execution plan for a workflow specification or existing workflow
- `list-actions` — List workflow actions and their statuses (useful for debugging action triggers)
- `is-complete` — Check if a workflow is complete
- `export` — Export a workflow to a portable JSON file
- `import` — Import a workflow from an exported JSON file
- `sync-status` — Synchronize job statuses with Slurm (detect and fail orphaned jobs)

## `torc workflows create`

Create a workflow from a specification file (supports JSON, JSON5, YAML, and KDL formats)

**Usage:** `torc workflows create [OPTIONS] --user <USER> <FILE>`

###### **Arguments:**

- `<FILE>` — Path to specification file containing WorkflowSpec. Supported formats: JSON (.json),
  JSON5 (.json5), YAML (.yaml, .yml), KDL (.kdl). Format is auto-detected from file extension.

###### **Options:**

- `-u`, `--user <USER>` — User that owns the workflow (defaults to USER environment variable)
- `--no-resource-monitoring` — Disable resource monitoring (default: enabled with summary
  granularity and 5s sample rate). Default: `false`
- `--skip-checks` — Skip validation checks (e.g., scheduler node requirements). Use with caution.
  Default: `false`
- `--dry-run` — Validate the workflow specification without creating it (dry-run mode). Returns a
  summary of what would be created including job count after parameter expansion.

## `torc workflows create-slurm`

Create a workflow with auto-generated Slurm schedulers

Automatically generates Slurm schedulers based on job resource requirements and HPC profile. For
Slurm workflows without pre-configured schedulers.

**Usage:** `torc workflows create-slurm [OPTIONS] --account <ACCOUNT> --user <USER> <FILE>`

###### **Arguments:**

- `<FILE>` — Path to specification file containing WorkflowSpec

###### **Options:**

- `--account <ACCOUNT>` — Slurm account to use for allocations
- `--hpc-profile <HPC_PROFILE>` — HPC profile to use (auto-detected if not specified)
- `--single-allocation` — Bundle all nodes into a single Slurm allocation per scheduler. By default,
  creates one Slurm allocation per node (N×1 mode). With this flag, creates one large allocation
  with all nodes (1×N mode).
- `-u`, `--user <USER>` — User that owns the workflow (defaults to USER environment variable)
- `--no-resource-monitoring` — Disable resource monitoring (default: enabled with summary
  granularity and 5s sample rate). Default: `false`
- `--skip-checks` — Skip validation checks (e.g., scheduler node requirements). Use with caution.
  Default: `false`
- `--dry-run` — Validate the workflow specification without creating it (dry-run mode)

## `torc workflows new`

Create a new empty workflow

**Usage:** `torc workflows new [OPTIONS] --name <NAME> --user <USER>`

###### **Options:**

- `-n`, `--name <NAME>` — Name of the workflow
- `-d`, `--description <DESCRIPTION>` — Description of the workflow
- `-u`, `--user <USER>` — User that owns the workflow (defaults to USER environment variable)

## `torc workflows list`

List workflows

**Usage:** `torc workflows list [OPTIONS]`

###### **Options:**

- `-u`, `--user <USER>` — User to filter by (defaults to USER environment variable)
- `--all-users` — List workflows for all users (overrides --user)
- `-l`, `--limit <LIMIT>` — Maximum number of workflows to return. Default: `10000`
- `--offset <OFFSET>` — Offset for pagination (0-based). Default: `0`
- `--sort-by <SORT_BY>` — Field to sort by
- `--reverse-sort` — Reverse sort order
- `--archived-only` — Show only archived workflows. Default: `false`
- `--include-archived` — Include both archived and non-archived workflows. Default: `false`

## `torc workflows get`

Get a specific workflow by ID

**Usage:** `torc workflows get [OPTIONS] [ID]`

###### **Arguments:**

- `<ID>` — ID of the workflow to get (optional - will prompt if not provided)

###### **Options:**

- `-u`, `--user <USER>` — User to filter by (defaults to USER environment variable)

## `torc workflows update`

Update an existing workflow

**Usage:** `torc workflows update [OPTIONS] [ID]`

###### **Arguments:**

- `<ID>` — ID of the workflow to update (optional - will prompt if not provided)

###### **Options:**

- `-n`, `--name <NAME>` — Name of the workflow
- `-d`, `--description <DESCRIPTION>` — Description of the workflow
- `--owner-user <OWNER_USER>` — User that owns the workflow

## `torc workflows cancel`

Cancel a workflow and all associated Slurm jobs

**Usage:** `torc workflows cancel [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — ID of the workflow to cancel (optional - will prompt if not provided)

## `torc workflows delete`

Delete one or more workflows

**Usage:** `torc workflows delete [OPTIONS] [IDS]...`

###### **Arguments:**

- `<IDS>` — IDs of workflows to remove (optional - will prompt if not provided)

###### **Options:**

- `--no-prompts` — Skip confirmation prompt
- `--force` — Force deletion even if workflow belongs to a different user

## `torc workflows archive`

Archive or unarchive one or more workflows

**Usage:** `torc workflows archive <IS_ARCHIVED> [WORKFLOW_IDS]...`

###### **Arguments:**

- `<IS_ARCHIVED>` — Set to true to archive, false to unarchive
- `<WORKFLOW_IDS>` — IDs of workflows to archive/unarchive (if empty, will prompt for selection)

## `torc workflows submit`

Submit a workflow: initialize if needed and schedule nodes for on_workflow_start actions. This
command requires the workflow to have an on_workflow_start action with schedule_nodes.

**Usage:** `torc workflows submit [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — ID of the workflow to submit (optional - will prompt if not provided)

###### **Options:**

- `--force` — If false, fail the operation if missing data is present. Default: `false`

## `torc workflows run`

Run a workflow locally on the current node

**Usage:** `torc workflows run [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — ID of the workflow to run (optional - will prompt if not provided)

###### **Options:**

- `-p`, `--poll-interval <POLL_INTERVAL>` — Poll interval in seconds for checking job completion.
  Default: `5.0`
- `--max-parallel-jobs <MAX_PARALLEL_JOBS>` — Maximum number of parallel jobs to run (defaults to
  available CPUs)
- `--output-dir <OUTPUT_DIR>` — Output directory for job logs and results. Default: `output`

## `torc workflows initialize`

Initialize a workflow, including all job statuses

**Usage:** `torc workflows initialize [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — ID of the workflow to start (optional - will prompt if not provided)

###### **Options:**

- `--force` — If false, fail the operation if missing data is present. Default: `false`
- `--no-prompts` — Skip confirmation prompt
- `--dry-run` — Perform a dry run without making changes

## `torc workflows reinitialize`

Reinitialize a workflow. This will reinitialize all jobs with a status of canceled, submitting,
pending, or terminated. Jobs with a status of done will also be reinitialized if an input_file or
user_data record has changed.

**Usage:** `torc workflows reinitialize [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — ID of the workflow to reinitialize (optional - will prompt if not provided)

###### **Options:**

- `--force` — If false, fail the operation if missing data is present. Default: `false`
- `--dry-run` — Perform a dry run without making changes

## `torc workflows status`

Get workflow status

**Usage:** `torc workflows status [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — ID of the workflow to get status for (optional - will prompt if not provided)

###### **Options:**

- `-u`, `--user <USER>` — User to filter by (defaults to USER environment variable)

## `torc workflows reset-status`

Reset workflow and job status

**Usage:** `torc workflows reset-status [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — ID of the workflow to reset status for (optional - will prompt if not provided)

###### **Options:**

- `--failed-only` — Only reset failed jobs. Default: `false`
- `-r`, `--reinitialize` — Reinitialize the workflow after resetting status. Default: `false`
- `--force` — Force reset even if there are active jobs (ignores running/pending jobs check).
  Default: `false`
- `--no-prompts` — Skip confirmation prompt

## `torc workflows execution-plan`

Show the execution plan for a workflow specification or existing workflow

**Usage:** `torc workflows execution-plan <SPEC_OR_ID>`

###### **Arguments:**

- `<SPEC_OR_ID>` — Path to specification file OR workflow ID

## `torc workflows list-actions`

List workflow actions and their statuses (useful for debugging action triggers)

**Usage:** `torc workflows list-actions [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — ID of the workflow to show actions for (optional - will prompt if not provided)

###### **Options:**

- `-u`, `--user <USER>` — User to filter by when selecting workflow interactively (defaults to USER
  environment variable)

## `torc workflows is-complete`

Check if a workflow is complete

**Usage:** `torc workflows is-complete [ID]`

###### **Arguments:**

- `<ID>` — ID of the workflow to check (optional - will prompt if not provided)

## `torc workflows export`

Export a workflow to a portable JSON file

Creates a self-contained export that can be imported into the same or different torc-server
instance. All entity IDs are preserved in the export and remapped during import.

**Usage:** `torc workflows export [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — ID of the workflow to export (optional - will prompt if not provided)

###### **Options:**

- `-o`, `--output <OUTPUT>` — Output file path (default: stdout)
- `--include-results` — Include job results in export
- `--include-events` — Include events (workflow history) in export

###### **Examples:**

```bash
# Export workflow to stdout
torc workflows export 123

# Export to a file
torc workflows export 123 -o workflow.json

# Include job results in export
torc workflows export 123 --include-results -o backup.json

# Export with all optional data
torc workflows export 123 --include-results --include-events -o complete.json
```

## `torc workflows import`

Import a workflow from an exported JSON file

Imports a workflow that was previously exported. All entity IDs are remapped to new IDs assigned by
the server. By default, all job statuses are reset to uninitialized for a fresh start.

**Usage:** `torc workflows import [OPTIONS] <FILE>`

###### **Arguments:**

- `<FILE>` — Path to the exported workflow JSON file (use '-' for stdin)

###### **Options:**

- `--name <NAME>` — Override the workflow name
- `--skip-results` — Skip importing results even if present in export
- `--skip-events` — Skip importing events even if present in export

###### **Examples:**

```bash
# Import a workflow (resets job statuses by default)
torc workflows import workflow.json

# Import from stdin
cat workflow.json | torc workflows import -

# Import with a different name
torc workflows import workflow.json --name 'my-copy'

# Skip importing results even if present in file
torc workflows import workflow.json --skip-results
```

## `torc workflows sync-status`

Synchronize job statuses with Slurm (detect and fail orphaned jobs)

This command detects jobs that are stuck in "running" status because their Slurm allocation
terminated unexpectedly (e.g., due to timeout, node failure, or admin intervention). It marks these
orphaned jobs as failed so the workflow can be recovered or restarted.

Use this when:

- `torc recover` reports "there are active Slurm allocations" but `squeue` shows none
- Jobs appear stuck in "running" status after a Slurm allocation ended
- You want to clean up workflow state before running `torc recover`

**Usage:** `torc workflows sync-status [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — ID of the workflow to sync (optional - will prompt if not provided)

###### **Options:**

- `--dry-run` — Preview changes without applying them

###### **Examples:**

```bash
# Preview what would be cleaned up
torc workflows sync-status 123 --dry-run

# Clean up orphaned jobs
torc workflows sync-status 123

# Get JSON output for scripting
torc -f json workflows sync-status 123
```

## `torc compute-nodes`

Compute node management commands

**Usage:** `torc compute-nodes <COMMAND>`

###### **Subcommands:**

- `get` — Get a specific compute node by ID
- `list` — List compute nodes for a workflow

## `torc compute-nodes get`

Get a specific compute node by ID

**Usage:** `torc compute-nodes get <ID>`

###### **Arguments:**

- `<ID>` — ID of the compute node

## `torc compute-nodes list`

List compute nodes for a workflow

**Usage:** `torc compute-nodes list [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — List compute nodes for this workflow (optional - will prompt if not provided)

###### **Options:**

- `-l`, `--limit <LIMIT>` — Maximum number of compute nodes to return. Default: `10000`
- `-o`, `--offset <OFFSET>` — Offset for pagination (0-based). Default: `0`
- `-s`, `--sort-by <SORT_BY>` — Field to sort by
- `-r`, `--reverse-sort` — Reverse sort order. Default: `false`
- `--scheduled-compute-node <SCHEDULED_COMPUTE_NODE>` — Filter by scheduled compute node ID

## `torc files`

File management commands

**Usage:** `torc files <COMMAND>`

###### **Subcommands:**

- `create` — Create a new file
- `list` — List files
- `get` — Get a specific file by ID
- `update` — Update an existing file
- `delete` — Delete a file
- `list-required-existing` — List required existing files for a workflow

## `torc files create`

Create a new file

**Usage:** `torc files create --name <NAME> --path <PATH> [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Create the file in this workflow

###### **Options:**

- `-n`, `--name <NAME>` — Name of the job
- `-p`, `--path <PATH>` — Path of the file

## `torc files list`

List files

**Usage:** `torc files list [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — List files for this workflow (optional - will prompt if not provided)

###### **Options:**

- `--produced-by-job-id <PRODUCED_BY_JOB_ID>` — Filter by job ID that produced the files
- `-l`, `--limit <LIMIT>` — Maximum number of files to return. Default: `10000`
- `--offset <OFFSET>` — Offset for pagination (0-based). Default: `0`
- `--sort-by <SORT_BY>` — Field to sort by
- `--reverse-sort` — Reverse sort order

## `torc files get`

Get a specific file by ID

**Usage:** `torc files get <ID>`

###### **Arguments:**

- `<ID>` — ID of the file to get

## `torc files update`

Update an existing file

**Usage:** `torc files update [OPTIONS] <ID>`

###### **Arguments:**

- `<ID>` — ID of the file to update

###### **Options:**

- `-n`, `--name <NAME>` — Name of the file
- `-p`, `--path <PATH>` — Path of the file

## `torc files delete`

Delete a file

**Usage:** `torc files delete <ID>`

###### **Arguments:**

- `<ID>` — ID of the file to remove

## `torc files list-required-existing`

List required existing files for a workflow

**Usage:** `torc files list-required-existing [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — List required existing files for this workflow (optional - will prompt if not
  provided)

## `torc jobs`

Job management commands

**Usage:** `torc jobs <COMMAND>`

###### **Subcommands:**

- `create` — Create a new job
- `create-from-file` — Create multiple jobs from a text file containing one command per line
- `list` — List jobs
- `get` — Get a specific job by ID
- `update` — Update an existing job
- `delete` — Delete one or more jobs
- `delete-all` — Delete all jobs for a workflow
- `list-resource-requirements` — List jobs with their resource requirements

## `torc jobs create`

Create a new job

**Usage:** `torc jobs create [OPTIONS] --name <NAME> --command <COMMAND> [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Create the job in this workflow

###### **Options:**

- `-n`, `--name <NAME>` — Name of the job
- `-c`, `--command <COMMAND>` — Command to execute
- `-r`, `--resource-requirements-id <RESOURCE_REQUIREMENTS_ID>` — Resource requirements ID for this
  job
- `-b`, `--blocking-job-ids <BLOCKING_JOB_IDS>` — Job IDs that block this job
- `-i`, `--input-file-ids <INPUT_FILE_IDS>` — Input files needed by this job
- `-o`, `--output-file-ids <OUTPUT_FILE_IDS>` — Output files produced by this job

## `torc jobs create-from-file`

Create multiple jobs from a text file containing one command per line

This command reads a text file where each line contains a job command. Lines starting with '#' are
treated as comments and ignored. Empty lines are also ignored.

Jobs will be named sequentially as job1, job2, job3, etc., starting from the current job count + 1
to avoid naming conflicts.

All jobs created will share the same resource requirements, which are automatically created and
assigned.

Example: `torc jobs create-from-file 123 batch_jobs.txt --cpus-per-job 4 --memory-per-job 8g`

**Usage:** `torc jobs create-from-file [OPTIONS] <WORKFLOW_ID> <FILE>`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID to create jobs for
- `<FILE>` — Path to text file containing job commands (one per line). File format: One command per
  line, lines starting with # are comments (ignored), empty lines are ignored.

###### **Options:**

- `--cpus-per-job <CPUS_PER_JOB>` — Number of CPUs per job. Default: `1`
- `--memory-per-job <MEMORY_PER_JOB>` — Memory per job (e.g., "1m", "2g", "16g"). Default: `1m`
- `--runtime-per-job <RUNTIME_PER_JOB>` — Runtime per job (ISO 8601 duration format). Examples:
  P0DT1M = 1 minute, P0DT30M = 30 minutes, P0DT2H = 2 hours, P1DT0H = 1 day. Default: `P0DT1M`

## `torc jobs list`

List jobs

**Usage:** `torc jobs list [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — List jobs for this workflow (optional - will prompt if not provided)

###### **Options:**

- `-s`, `--status <STATUS>` — Filter by job status
- `--upstream-job-id <UPSTREAM_JOB_ID>` — Filter by upstream job ID (jobs that depend on this job)
- `-l`, `--limit <LIMIT>` — Maximum number of jobs to return. Default: `10000`
- `--offset <OFFSET>` — Offset for pagination (0-based). Default: `0`
- `--sort-by <SORT_BY>` — Field to sort by
- `--reverse-sort` — Reverse sort order
- `--include-relationships` — Include job relationships (depends_on_job_ids, input/output
  file/user_data IDs) - slower but more complete

## `torc jobs get`

Get a specific job by ID

**Usage:** `torc jobs get <ID>`

###### **Arguments:**

- `<ID>` — ID of the job to get

## `torc jobs update`

Update an existing job

**Usage:** `torc jobs update [OPTIONS] <ID>`

###### **Arguments:**

- `<ID>` — ID of the job to update

###### **Options:**

- `-n`, `--name <NAME>` — Name of the job
- `-c`, `--command <COMMAND>` — Command to execute

## `torc jobs delete`

Delete one or more jobs

**Usage:** `torc jobs delete [IDS]...`

###### **Arguments:**

- `<IDS>` — IDs of the jobs to remove

## `torc jobs delete-all`

Delete all jobs for a workflow

**Usage:** `torc jobs delete-all [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID to delete all jobs from (optional - will prompt if not provided)

## `torc jobs list-resource-requirements`

List jobs with their resource requirements

**Usage:** `torc jobs list-resource-requirements [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID to list jobs from (optional - will prompt if not provided)

###### **Options:**

- `-j`, `--job-id <JOB_ID>` — Filter by specific job ID

## `torc job-dependencies`

Job dependency and relationship queries

**Usage:** `torc job-dependencies <COMMAND>`

###### **Subcommands:**

- `job-job` — List job-to-job dependencies for a workflow
- `job-file` — List job-file relationships for a workflow
- `job-user-data` — List job-user_data relationships for a workflow

## `torc job-dependencies job-job`

List job-to-job dependencies for a workflow

**Usage:** `torc job-dependencies job-job [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — ID of the workflow (optional - will prompt if not provided)

###### **Options:**

- `-l`, `--limit <LIMIT>` — Maximum number of dependencies to return. Default: `10000`
- `--offset <OFFSET>` — Offset for pagination (0-based). Default: `0`

## `torc job-dependencies job-file`

List job-file relationships for a workflow

**Usage:** `torc job-dependencies job-file [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — ID of the workflow (optional - will prompt if not provided)

###### **Options:**

- `-l`, `--limit <LIMIT>` — Maximum number of relationships to return. Default: `10000`
- `--offset <OFFSET>` — Offset for pagination (0-based). Default: `0`

## `torc job-dependencies job-user-data`

List job-user_data relationships for a workflow

**Usage:** `torc job-dependencies job-user-data [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — ID of the workflow (optional - will prompt if not provided)

###### **Options:**

- `-l`, `--limit <LIMIT>` — Maximum number of relationships to return. Default: `10000`
- `--offset <OFFSET>` — Offset for pagination (0-based). Default: `0`

## `torc resource-requirements`

Resource requirements management commands

**Usage:** `torc resource-requirements <COMMAND>`

###### **Subcommands:**

- `create` — Create new resource requirements
- `list` — List resource requirements
- `get` — Get a specific resource requirement by ID
- `update` — Update existing resource requirements
- `delete` — Delete resource requirements

## `torc resource-requirements create`

Create new resource requirements

**Usage:** `torc resource-requirements create [OPTIONS] --name <NAME> [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Create resource requirements in this workflow

###### **Options:**

- `-n`, `--name <NAME>` — Name of the resource requirements
- `--num-cpus <NUM_CPUS>` — Number of CPUs required. Default: `1`
- `--num-gpus <NUM_GPUS>` — Number of GPUs required. Default: `0`
- `--num-nodes <NUM_NODES>` — Number of nodes required. Default: `1`
- `-m`, `--memory <MEMORY>` — Amount of memory required (e.g., "20g"). Default: `1m`
- `-r`, `--runtime <RUNTIME>` — Maximum runtime in ISO 8601 duration format (e.g., "P0DT1H").
  Default: `P0DT1M`

## `torc resource-requirements list`

List resource requirements

**Usage:** `torc resource-requirements list [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — List resource requirements for this workflow (optional - will prompt if not
  provided)

###### **Options:**

- `-l`, `--limit <LIMIT>` — Maximum number of resource requirements to return. Default: `10000`
- `--offset <OFFSET>` — Offset for pagination (0-based). Default: `0`
- `--sort-by <SORT_BY>` — Field to sort by
- `--reverse-sort` — Reverse sort order

## `torc resource-requirements get`

Get a specific resource requirement by ID

**Usage:** `torc resource-requirements get <ID>`

###### **Arguments:**

- `<ID>` — ID of the resource requirement to get

## `torc resource-requirements update`

Update existing resource requirements

**Usage:** `torc resource-requirements update [OPTIONS] <ID>`

###### **Arguments:**

- `<ID>` — ID of the resource requirement to update

###### **Options:**

- `-n`, `--name <NAME>` — Name of the resource requirements
- `--num-cpus <NUM_CPUS>` — Number of CPUs required
- `--num-gpus <NUM_GPUS>` — Number of GPUs required
- `--num-nodes <NUM_NODES>` — Number of nodes required
- `--memory <MEMORY>` — Amount of memory required (e.g., "20g")
- `--runtime <RUNTIME>` — Maximum runtime (e.g., "1h", "30m")

## `torc resource-requirements delete`

Delete resource requirements

**Usage:** `torc resource-requirements delete <ID>`

###### **Arguments:**

- `<ID>` — ID of the resource requirement to remove

## `torc events`

Event management commands

**Usage:** `torc events <COMMAND>`

###### **Subcommands:**

- `create` — Create a new event
- `list` — List events for a workflow
- `monitor` — Monitor events for a workflow in real-time
- `get-latest-event` — Get the latest event for a workflow
- `delete` — Delete an event

## `torc events create`

Create a new event

**Usage:** `torc events create --data <DATA> [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Create the event in this workflow

###### **Options:**

- `-d`, `--data <DATA>` — JSON data for the event

## `torc events list`

List events for a workflow

**Usage:** `torc events list [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — List events for this workflow (optional - will prompt if not provided)

###### **Options:**

- `-c`, `--category <CATEGORY>` — Filter events by category
- `-l`, `--limit <LIMIT>` — Maximum number of events to return. Default: `10000`
- `-o`, `--offset <OFFSET>` — Offset for pagination (0-based). Default: `0`
- `-s`, `--sort-by <SORT_BY>` — Field to sort by
- `-r`, `--reverse-sort` — Reverse sort order. Default: `false`

## `torc events monitor`

Monitor events for a workflow in real-time

**Usage:** `torc events monitor [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Monitor events for this workflow (optional - will prompt if not provided)

###### **Options:**

- `-d`, `--duration <DURATION>` — Duration to monitor in minutes (default: infinite)
- `-p`, `--poll-interval <POLL_INTERVAL>` — Poll interval in seconds. Default: `60`
- `-c`, `--category <CATEGORY>` — Filter events by category

## `torc events get-latest-event`

Get the latest event for a workflow

**Usage:** `torc events get-latest-event [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Get the latest event for this workflow (optional - will prompt if not provided)

## `torc events delete`

Delete an event

**Usage:** `torc events delete <ID>`

###### **Arguments:**

- `<ID>` — ID of the event to remove

## `torc results`

Result management commands

**Usage:** `torc results <COMMAND>`

###### **Subcommands:**

- `list` — List results
- `get` — Get a specific result by ID
- `delete` — Delete a result

## `torc results list`

List results

**Usage:** `torc results list [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — List results for this workflow (optional - will prompt if not provided). By
  default, only lists results for the latest run of the workflow.

###### **Options:**

- `-j`, `--job-id <JOB_ID>` — List results for this job
- `-r`, `--run-id <RUN_ID>` — List results for this run_id
- `--return-code <RETURN_CODE>` — Filter by return code
- `--failed` — Show only failed jobs (non-zero return code)
- `-s`, `--status <STATUS>` — Filter by job status (uninitialized, blocked, canceled, terminated,
  done, ready, scheduled, running, pending, disabled)
- `-l`, `--limit <LIMIT>` — Maximum number of results to return. Default: `10000`
- `--offset <OFFSET>` — Offset for pagination (0-based). Default: `0`
- `--sort-by <SORT_BY>` — Field to sort by
- `--reverse-sort` — Reverse sort order
- `--all-runs` — Show all historical results (default: false, only shows current results)
- `--compute-node <COMPUTE_NODE>` — Filter by compute node ID

## `torc results get`

Get a specific result by ID

**Usage:** `torc results get <ID>`

###### **Arguments:**

- `<ID>` — ID of the result to get

## `torc results delete`

Delete a result

**Usage:** `torc results delete <ID>`

###### **Arguments:**

- `<ID>` — ID of the result to remove

## `torc user-data`

User data management commands

**Usage:** `torc user-data <COMMAND>`

###### **Subcommands:**

- `create` — Create a new user data record
- `list` — List user data records
- `get` — Get a specific user data record
- `update` — Update a user data record
- `delete` — Delete a user data record
- `delete-all` — Delete all user data records for a workflow
- `list-missing` — List missing user data for a workflow

## `torc user-data create`

Create a new user data record

**Usage:** `torc user-data create [OPTIONS] --name <NAME> [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID

###### **Options:**

- `-n`, `--name <NAME>` — Name of the data object
- `-d`, `--data <DATA>` — JSON data content
- `--ephemeral` — Whether the data is ephemeral (cleared between runs)
- `--consumer-job-id <CONSUMER_JOB_ID>` — Consumer job ID (optional)
- `--producer-job-id <PRODUCER_JOB_ID>` — Producer job ID (optional)

## `torc user-data list`

List user data records

**Usage:** `torc user-data list [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID (if not provided, will be selected interactively)

###### **Options:**

- `-l`, `--limit <LIMIT>` — Maximum number of records to return. Default: `50`
- `-o`, `--offset <OFFSET>` — Number of records to skip. Default: `0`
- `--sort-by <SORT_BY>` — Field to sort by
- `--reverse-sort` — Reverse sort order
- `--name <NAME>` — Filter by name
- `--is-ephemeral <IS_EPHEMERAL>` — Filter by ephemeral status. Possible values: `true`, `false`
- `--consumer-job-id <CONSUMER_JOB_ID>` — Filter by consumer job ID
- `--producer-job-id <PRODUCER_JOB_ID>` — Filter by producer job ID

## `torc user-data get`

Get a specific user data record

**Usage:** `torc user-data get <ID>`

###### **Arguments:**

- `<ID>` — User data record ID

## `torc user-data update`

Update a user data record

**Usage:** `torc user-data update [OPTIONS] <ID>`

###### **Arguments:**

- `<ID>` — User data record ID

###### **Options:**

- `-n`, `--name <NAME>` — New name for the data object
- `-d`, `--data <DATA>` — New JSON data content
- `--ephemeral <EPHEMERAL>` — Update ephemeral status. Possible values: `true`, `false`

## `torc user-data delete`

Delete a user data record

**Usage:** `torc user-data delete <ID>`

###### **Arguments:**

- `<ID>` — User data record ID

## `torc user-data delete-all`

Delete all user data records for a workflow

**Usage:** `torc user-data delete-all <WORKFLOW_ID>`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID

## `torc user-data list-missing`

List missing user data for a workflow

**Usage:** `torc user-data list-missing <WORKFLOW_ID>`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID

## `torc slurm`

Slurm scheduler commands

**Usage:** `torc slurm <COMMAND>`

###### **Subcommands:**

- `create` — Add a Slurm config to the database
- `update` — Modify a Slurm config in the database
- `list` — Show the current Slurm configs in the database
- `get` — Get a specific Slurm config by ID
- `delete` — Delete a Slurm config by ID
- `schedule-nodes` — Schedule compute nodes using Slurm
- `parse-logs` — Parse Slurm log files for known error messages
- `sacct` — Call sacct for scheduled compute nodes and display summary
- `generate` — Generate Slurm schedulers for a workflow based on job resource requirements
- `regenerate` — Regenerate Slurm schedulers for an existing workflow based on pending jobs

## `torc slurm create`

Add a Slurm config to the database

**Usage:** `torc slurm create [OPTIONS] --name <NAME> --account <ACCOUNT> [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID

###### **Options:**

- `-n`, `--name <NAME>` — Name of config
- `-a`, `--account <ACCOUNT>` — HPC account
- `-g`, `--gres <GRES>` — Request nodes that have at least this number of GPUs. Ex: 'gpu:2'
- `-m`, `--mem <MEM>` — Request nodes that have at least this amount of memory. Ex: '180G'
- `-N`, `--nodes <NODES>` — Number of nodes to use for each job. Default: `1`
- `-p`, `--partition <PARTITION>` — HPC partition. Default is determined by the scheduler
- `-q`, `--qos <QOS>` — Controls priority of the jobs. Default: `normal`
- `-t`, `--tmp <TMP>` — Request nodes that have at least this amount of storage scratch space
- `-W`, `--walltime <WALLTIME>` — Slurm job walltime. Default: `04:00:00`
- `-e`, `--extra <EXTRA>` — Add extra Slurm parameters, for example
  --extra='--reservation=my-reservation'

## `torc slurm update`

Modify a Slurm config in the database

**Usage:** `torc slurm update [OPTIONS] <SCHEDULER_ID>`

###### **Arguments:**

- `<SCHEDULER_ID>`

###### **Options:**

- `-N`, `--name <NAME>` — Name of config
- `-a`, `--account <ACCOUNT>` — HPC account
- `-g`, `--gres <GRES>` — Request nodes that have at least this number of GPUs. Ex: 'gpu:2'
- `-m`, `--mem <MEM>` — Request nodes that have at least this amount of memory. Ex: '180G'
- `-n`, `--nodes <NODES>` — Number of nodes to use for each job
- `-p`, `--partition <PARTITION>` — HPC partition
- `-q`, `--qos <QOS>` — Controls priority of the jobs
- `-t`, `--tmp <TMP>` — Request nodes that have at least this amount of storage scratch space
- `--walltime <WALLTIME>` — Slurm job walltime
- `-e`, `--extra <EXTRA>` — Add extra Slurm parameters

## `torc slurm list`

Show the current Slurm configs in the database

**Usage:** `torc slurm list [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID

###### **Options:**

- `-l`, `--limit <LIMIT>` — Maximum number of configs to return. Default: `10000`
- `--offset <OFFSET>` — Offset for pagination (0-based). Default: `0`

## `torc slurm get`

Get a specific Slurm config by ID

**Usage:** `torc slurm get <ID>`

###### **Arguments:**

- `<ID>` — ID of the Slurm config to get

## `torc slurm delete`

Delete a Slurm config by ID

**Usage:** `torc slurm delete <ID>`

###### **Arguments:**

- `<ID>` — ID of the Slurm config to delete

## `torc slurm schedule-nodes`

Schedule compute nodes using Slurm

**Usage:** `torc slurm schedule-nodes [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID

###### **Options:**

- `-j`, `--job-prefix <JOB_PREFIX>` — Job prefix for the Slurm job names. Default: empty
- `--keep-submission-scripts` — Keep submission scripts after job submission. Default: `false`
- `-m`, `--max-parallel-jobs <MAX_PARALLEL_JOBS>` — Maximum number of parallel jobs
- `-n`, `--num-hpc-jobs <NUM_HPC_JOBS>` — Number of HPC jobs to submit. Default: `1`
- `-o`, `--output <OUTPUT>` — Output directory for job output files. Default: `output`
- `-p`, `--poll-interval <POLL_INTERVAL>` — Poll interval in seconds. Default: `60`
- `--scheduler-config-id <SCHEDULER_CONFIG_ID>` — Scheduler config ID
- `--start-one-worker-per-node` — Start one worker per node. Default: `false`

## `torc slurm parse-logs`

Parse Slurm log files for known error messages

**Usage:** `torc slurm parse-logs [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID

###### **Options:**

- `-o`, `--output-dir <OUTPUT_DIR>` — Output directory containing Slurm log files. Default: `output`
- `--errors-only` — Only show errors (skip warnings). Default: `false`

## `torc slurm sacct`

Call sacct for scheduled compute nodes and display summary

**Usage:** `torc slurm sacct [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID

###### **Options:**

- `-o`, `--output-dir <OUTPUT_DIR>` — Output directory for sacct JSON files (only used with
  --save-json). Default: `output`
- `--save-json` — Save full JSON output to files in addition to displaying summary. Default: `false`

## `torc slurm generate`

Generate Slurm schedulers for a workflow based on job resource requirements

**Usage:** `torc slurm generate [OPTIONS] --account <ACCOUNT> <WORKFLOW_FILE>`

###### **Arguments:**

- `<WORKFLOW_FILE>` — Path to workflow specification file (YAML, JSON, JSON5, or KDL)

###### **Options:**

- `--account <ACCOUNT>` — Slurm account to use
- `--profile <PROFILE>` — HPC profile to use (if not specified, tries to detect current system)
- `-o`, `--output <OUTPUT>` — Output file path (if not specified, prints to stdout)
- `--single-allocation` — Bundle all nodes into a single Slurm allocation per scheduler. By default,
  creates one Slurm allocation per node (N×1 mode). With this flag, creates one large allocation
  with all nodes (1×N mode).
- `--group-by <GROUP_BY>` — Strategy for grouping jobs into schedulers. Possible values:
  `resource-requirements` (default), `partition`
- `--walltime-strategy <STRATEGY>` — Strategy for determining Slurm job walltime. Possible values:
  `max-job-runtime` (default), `max-partition-time`. `max-job-runtime` uses the maximum job runtime
  multiplied by `--walltime-multiplier`. `max-partition-time` uses the partition's maximum allowed
  walltime.
- `--walltime-multiplier <MULTIPLIER>` — Multiplier for job runtime when using
  `--walltime-strategy=max-job-runtime`. Default: `1.5`
- `--no-actions` — Don't add workflow actions for scheduling nodes
- `--overwrite` — Overwrite existing schedulers in the workflow
- `--dry-run` — Show what would be generated without writing to output

## `torc slurm regenerate`

Regenerate Slurm schedulers for an existing workflow based on pending jobs

Analyzes jobs that are uninitialized, ready, or blocked and generates new Slurm schedulers to run
them. Uses existing scheduler configurations as defaults for account, partition, and other settings.

This is useful for recovery after job failures: update job resources, reset failed jobs, then
regenerate schedulers to submit new allocations.

**Usage:** `torc slurm regenerate [OPTIONS] <WORKFLOW_ID>`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID

###### **Options:**

- `--account <ACCOUNT>` — Slurm account to use (defaults to account from existing schedulers)
- `--profile <PROFILE>` — HPC profile to use (if not specified, tries to detect current system)
- `--single-allocation` — Bundle all nodes into a single Slurm allocation per scheduler
- `--submit` — Submit the generated allocations immediately
- `-o`, `--output-dir <OUTPUT_DIR>` — Output directory for job output files (used when submitting).
  Default: `output`
- `-p`, `--poll-interval <POLL_INTERVAL>` — Poll interval in seconds (used when submitting).
  Default: `60`
- `--group-by <GROUP_BY>` — Strategy for grouping jobs into schedulers. Possible values:
  `resource-requirements` (default), `partition`
- `--walltime-strategy <STRATEGY>` — Strategy for determining Slurm job walltime. Possible values:
  `max-job-runtime` (default), `max-partition-time`
- `--walltime-multiplier <MULTIPLIER>` — Multiplier for job runtime when using
  `--walltime-strategy=max-job-runtime`. Default: `1.5`
- `--dry-run` — Show what would be created without making changes
- `--include-job-ids <JOB_IDS>` — Include specific job IDs in planning regardless of their status
  (useful for recovery dry-run to include failed jobs)

## `torc remote`

Remote worker execution commands (SSH-based distributed execution)

**Usage:** `torc remote <COMMAND>`

###### **Subcommands:**

- `add-workers` — Add one or more remote workers to a workflow
- `add-workers-from-file` — Add remote workers to a workflow from a file
- `remove-worker` — Remove a remote worker from a workflow
- `list-workers` — List remote workers stored in the database for a workflow
- `run` — Run workers on remote machines via SSH
- `status` — Check status of remote workers
- `stop` — Stop workers on remote machines
- `collect-logs` — Collect logs from remote workers
- `delete-logs` — Delete logs from remote workers

## `torc remote add-workers`

Add one or more remote workers to a workflow

Workers are stored in the database and used by subsequent commands. Format: [user@]hostname[:port]

**Usage:** `torc remote add-workers <WORKFLOW_ID> <WORKERS>...`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID
- `<WORKERS>` — Worker addresses (format: [user@]hostname[:port])

## `torc remote add-workers-from-file`

Add remote workers to a workflow from a file

Each line in the file should be a worker address. Lines starting with # are comments.

**Usage:** `torc remote add-workers-from-file <WORKER_FILE> [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKER_FILE>` — Path to worker file listing remote machines
- `<WORKFLOW_ID>` — Workflow ID (optional - will prompt if not provided)

## `torc remote remove-worker`

Remove a remote worker from a workflow

**Usage:** `torc remote remove-worker <WORKER> [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKER>` — Worker address to remove
- `<WORKFLOW_ID>` — Workflow ID (optional - will prompt if not provided)

## `torc remote list-workers`

List remote workers stored in the database for a workflow

**Usage:** `torc remote list-workers [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID (optional - will prompt if not provided)

## `torc remote run`

Run workers on remote machines via SSH

SSH into each stored worker and start a torc worker process. Workers run detached (via nohup) and
survive SSH disconnection. Use add-workers first, or provide --workers to add and run in one step.

**Usage:** `torc remote run [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID to run (optional - will prompt if not provided)

###### **Options:**

- `-w`, `--workers <WORKERS>` — Path to worker file (optional - adds workers before running)
- `-o`, `--output-dir <OUTPUT_DIR>` — Output directory on remote machines (relative to home).
  Default: `torc_output`
- `--max-parallel-ssh <MAX_PARALLEL_SSH>` — Maximum parallel SSH connections. Default: `10`
- `-p`, `--poll-interval <POLL_INTERVAL>` — Poll interval in seconds for workers. Default: `5.0`
- `--max-parallel-jobs <MAX_PARALLEL_JOBS>` — Maximum number of parallel jobs per worker
- `--num-cpus <NUM_CPUS>` — Number of CPUs per worker (auto-detect if not specified)
- `--memory-gb <MEMORY_GB>` — Memory in GB per worker (auto-detect if not specified)
- `--num-gpus <NUM_GPUS>` — Number of GPUs per worker (auto-detect if not specified)
- `--skip-version-check` — Skip version check (not recommended). Default: `false`

## `torc remote status`

Check status of remote workers

**Usage:** `torc remote status [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID (optional - will prompt if not provided)

###### **Options:**

- `--output-dir <OUTPUT_DIR>` — Remote output directory (must match what was used in run). Default:
  `torc_output`
- `--max-parallel-ssh <MAX_PARALLEL_SSH>` — Maximum parallel SSH connections. Default: `10`

## `torc remote stop`

Stop workers on remote machines

**Usage:** `torc remote stop [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID (optional - will prompt if not provided)

###### **Options:**

- `--output-dir <OUTPUT_DIR>` — Remote output directory (must match what was used in run). Default:
  `torc_output`
- `--max-parallel-ssh <MAX_PARALLEL_SSH>` — Maximum parallel SSH connections. Default: `10`
- `--force` — Force kill (SIGKILL instead of SIGTERM). Default: `false`

## `torc remote collect-logs`

Collect logs from remote workers

**Usage:** `torc remote collect-logs [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID (optional - will prompt if not provided)

###### **Options:**

- `-l`, `--local-output-dir <LOCAL_OUTPUT_DIR>` — Local directory to save collected logs. Default:
  `remote_logs`
- `--remote-output-dir <REMOTE_OUTPUT_DIR>` — Remote output directory (must match what was used in
  run). Default: `torc_output`
- `--max-parallel-ssh <MAX_PARALLEL_SSH>` — Maximum parallel SSH connections. Default: `10`
- `--delete` — Delete remote logs after successful collection. Default: `false`

## `torc remote delete-logs`

Delete logs from remote workers

Removes the output directory from all remote workers. Use collect-logs --delete to safely collect
before deleting.

**Usage:** `torc remote delete-logs [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID (optional - will prompt if not provided)

###### **Options:**

- `--remote-output-dir <REMOTE_OUTPUT_DIR>` — Remote output directory to delete (must match what was
  used in run). Default: `torc_output`
- `--max-parallel-ssh <MAX_PARALLEL_SSH>` — Maximum parallel SSH connections. Default: `10`

## `torc scheduled-compute-nodes`

Scheduled compute node management commands

**Usage:** `torc scheduled-compute-nodes <COMMAND>`

###### **Subcommands:**

- `get` — Get a scheduled compute node by ID
- `list` — List scheduled compute nodes for a workflow
- `list-jobs` — List jobs that ran under a scheduled compute node

## `torc scheduled-compute-nodes get`

Get a scheduled compute node by ID

**Usage:** `torc scheduled-compute-nodes get <ID>`

###### **Arguments:**

- `<ID>` — ID of the scheduled compute node

## `torc scheduled-compute-nodes list`

List scheduled compute nodes for a workflow

**Usage:** `torc scheduled-compute-nodes list [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — List scheduled compute nodes for this workflow (optional - will prompt if not
  provided)

###### **Options:**

- `-l`, `--limit <LIMIT>` — Maximum number of scheduled compute nodes to return. Default: `10000`
- `-o`, `--offset <OFFSET>` — Offset for pagination (0-based). Default: `0`
- `-s`, `--sort-by <SORT_BY>` — Field to sort by
- `-r`, `--reverse-sort` — Reverse sort order. Default: `false`
- `--scheduler-id <SCHEDULER_ID>` — Filter by scheduler ID
- `--scheduler-config-id <SCHEDULER_CONFIG_ID>` — Filter by scheduler config ID
- `--status <STATUS>` — Filter by status

## `torc scheduled-compute-nodes list-jobs`

List jobs that ran under a scheduled compute node

**Usage:** `torc scheduled-compute-nodes list-jobs <ID>`

###### **Arguments:**

- `<ID>` — ID of the scheduled compute node

## `torc hpc`

HPC system profiles and partition information

**Usage:** `torc hpc <COMMAND>`

###### **Subcommands:**

- `list` — List known HPC system profiles
- `detect` — Detect the current HPC system
- `show` — Show details of an HPC profile
- `partitions` — Show partitions for an HPC profile
- `match` — Find partitions matching resource requirements

## `torc hpc list`

List known HPC system profiles

**Usage:** `torc hpc list`

## `torc hpc detect`

Detect the current HPC system

**Usage:** `torc hpc detect`

## `torc hpc show`

Show details of an HPC profile

**Usage:** `torc hpc show <NAME>`

###### **Arguments:**

- `<NAME>` — Profile name (e.g., "kestrel")

## `torc hpc partitions`

Show partitions for an HPC profile

**Usage:** `torc hpc partitions [OPTIONS] [NAME]`

###### **Arguments:**

- `<NAME>` — Profile name (e.g., "kestrel"). If not specified, tries to detect current system.

###### **Options:**

- `--gpu` — Filter to GPU partitions only
- `--cpu` — Filter to CPU-only partitions
- `--shared` — Filter to shared partitions

## `torc hpc match`

Find partitions matching resource requirements

**Usage:** `torc hpc match [OPTIONS]`

###### **Options:**

- `--cpus <CPUS>` — Number of CPUs required. Default: `1`
- `--memory <MEMORY>` — Memory required (e.g., "100g", "512m", or MB as number). Default: `1g`
- `--walltime <WALLTIME>` — Wall time required (e.g., "4:00:00", "2-00:00:00"). Default: `1:00:00`
- `--gpus <GPUS>` — Number of GPUs required
- `--profile <PROFILE>` — Profile name (if not specified, tries to detect current system)

## `torc reports`

Generate reports and analytics

**Usage:** `torc reports <COMMAND>`

###### **Subcommands:**

- `check-resource-utilization` — Check resource utilization and report jobs that exceeded their
  specified requirements
- `results` — Generate a comprehensive JSON report of job results including all log file paths
- `summary` — Generate a summary of workflow results (requires workflow to be complete)

## `torc reports check-resource-utilization`

Check resource utilization and report jobs that exceeded their specified requirements

**Usage:** `torc reports check-resource-utilization [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID to analyze (optional - will prompt if not provided)

###### **Options:**

- `-r`, `--run-id <RUN_ID>` — Run ID to analyze (optional - analyzes latest run if not provided)
- `-a`, `--all` — Show all jobs (default: only show jobs that exceeded requirements)
- `--include-failed` — Include failed and terminated jobs in the analysis (for recovery diagnostics)
- `--correct` — Automatically correct resource requirements for over-utilized jobs
- `--min-over-utilization <MIN_OVER_UTILIZATION>` — Minimum over-utilization percentage to flag as
  violation (default: 1.0%)

## `torc reports results`

Generate a comprehensive JSON report of job results including all log file paths

**Usage:** `torc reports results [OPTIONS] [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID to analyze (optional - will prompt if not provided)

###### **Options:**

- `-o`, `--output-dir <OUTPUT_DIR>` — Output directory (where job logs are stored, passed in
  `torc run` and `torc submit`). Default: `output`
- `--all-runs` — Include all runs for each job (default: only latest run)

## `torc reports summary`

Generate a summary of workflow results (requires workflow to be complete)

**Usage:** `torc reports summary [WORKFLOW_ID]`

###### **Arguments:**

- `<WORKFLOW_ID>` — Workflow ID to summarize (optional - will prompt if not provided)

## `torc config`

Manage configuration files and settings

**Usage:** `torc config <COMMAND>`

###### **Subcommands:**

- `show` — Show the effective configuration (merged from all sources)
- `paths` — Show configuration file paths
- `init` — Initialize a configuration file with defaults
- `validate` — Validate the current configuration

## `torc config show`

Show the effective configuration (merged from all sources)

**Usage:** `torc config show [OPTIONS]`

###### **Options:**

- `-f`, `--format <FORMAT>` — Output format (toml or json). Default: `toml`

## `torc config paths`

Show configuration file paths

**Usage:** `torc config paths`

## `torc config init`

Initialize a configuration file with defaults

**Usage:** `torc config init [OPTIONS]`

###### **Options:**

- `--system` — Create system-wide config (/etc/torc/config.toml)
- `--user` — Create user config (~/.config/torc/config.toml)
- `--local` — Create project-local config (./torc.toml)
- `-f`, `--force` — Force overwrite if file exists

## `torc config validate`

Validate the current configuration

**Usage:** `torc config validate`

## `torc tui`

Interactive terminal UI for managing workflows

**Usage:** `torc tui [OPTIONS]`

###### **Options:**

- `--standalone` — Start in standalone mode: automatically start a torc-server
- `--port <PORT>` — Port for the server in standalone mode. Default: `8080`
- `--database <DATABASE>` — Database path for standalone mode

## `torc plot-resources`

Generate interactive HTML plots from resource monitoring data

**Usage:** `torc plot-resources [OPTIONS] <DB_PATHS>...`

###### **Arguments:**

- `<DB_PATHS>` — Path to the resource metrics database file(s)

###### **Options:**

- `-o`, `--output-dir <OUTPUT_DIR>` — Output directory for generated plots (default: current
  directory). Default: `.`
- `-j`, `--job-ids <JOB_IDS>` — Only plot specific job IDs (comma-separated)
- `-p`, `--prefix <PREFIX>` — Prefix for output filenames. Default: `resource_plot`
- `-f`, `--format <FORMAT>` — Output format: html or json. Default: `html`

## `torc completions`

Generate shell completions

**Usage:** `torc completions <SHELL>`

###### **Arguments:**

- `<SHELL>` — The shell to generate completions for. Possible values: `bash`, `elvish`, `fish`,
  `powershell`, `zsh`

<hr/>

<small><i>This document was generated automatically by
<a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.</i></small>
