# Workflow Actions

Workflow actions enable automatic execution of commands and resource allocation in response to
workflow lifecycle events. Actions provide hooks for setup, teardown, monitoring, and dynamic
resource management throughout workflow execution.

## Overview

Actions consist of three components:

1. **Trigger** - The condition that activates the action
2. **Action Type** - The operation to perform
3. **Configuration** - Parameters specific to the action

```yaml
actions:
  - trigger_type: "on_workflow_start"
    action_type: "run_commands"
    commands:
      - "mkdir -p output logs"
      - "echo 'Workflow started' > logs/status.txt"
```

## Trigger Types

### Workflow Lifecycle Triggers

#### `on_workflow_start`

Executes once when the workflow is initialized.

**When it fires**: During `initialize_jobs` after jobs are transitioned from uninitialized to
ready/blocked states.

**Typical use cases**:

- Scheduling Slurm allocations
- Creating directory structures
- Copying initial data

```yaml
- trigger_type: "on_workflow_start"
  action_type: "run_commands"
  commands:
    - "mkdir -p output checkpoints temp"
    - "echo 'Workflow started at $(date)' > workflow.log"
```

#### `on_workflow_complete`

Executes once when all jobs reach terminal states (completed, failed, or canceled).

**When it fires**: After the last job completes, as detected by the job runner.

**Typical use cases**:

- Archiving final results
- Uploading to remote storage
- Cleanup of temporary files
- Generating summary reports

```yaml
- trigger_type: "on_workflow_complete"
  action_type: "run_commands"
  commands:
    - "tar -czf results.tar.gz output/"
    - "aws s3 cp results.tar.gz s3://bucket/results/"
    - "rm -rf temp/"
```

### Job-Based Triggers

#### `on_jobs_ready`

Executes when **all** specified jobs transition to the "ready" state.

**When it fires**: When the last specified job becomes ready to execute (all dependencies
satisfied).

**Typical use cases**:

- Scheduling Slurm allocations
- Starting phase-specific monitoring
- Pre-computation setup
- Notifications before expensive operations

```yaml
- trigger_type: "on_jobs_ready"
  action_type: "schedule_nodes"
  jobs: ["train_model_001", "train_model_002", "train_model_003"]
  scheduler: "gpu_cluster"
  scheduler_type: "slurm"
  num_allocations: 2
```

**Important**: The action triggers only when **all** matching jobs are ready, not individually as
each job becomes ready.

#### `on_jobs_complete`

Executes when **all** specified jobs reach terminal states (completed, failed, or canceled).

**When it fires**: When the last specified job finishes execution.

**Typical use cases**:

- Scheduling Slurm allocations
- Cleaning up intermediate files
- Archiving phase results
- Freeing storage space
- Phase-specific reporting

```yaml
- trigger_type: "on_jobs_complete"
  action_type: "run_commands"
  jobs: ["preprocess_1", "preprocess_2", "preprocess_3"]
  commands:
    - "echo 'Preprocessing phase complete' >> workflow.log"
    - "rm -rf raw_data/"
```

### Worker Lifecycle Triggers

Worker lifecycle triggers are **persistent by default**, meaning they execute once per worker (job
runner), not once per workflow.

#### `on_worker_start`

Executes when each worker (job runner) starts.

**When it fires**: After a job runner starts and checks for workflow actions, before claiming any
jobs.

**Typical use cases**:

- Worker-specific initialization
- Setting up worker-local logging
- Copying data to compute node local storage
- Initializing worker-specific resources
- Recording worker startup metrics

```yaml
- trigger_type: "on_worker_start"
  action_type: "run_commands"
  persistent: true  # Each worker executes this
  commands:
    - "echo 'Worker started on $(hostname) at $(date)' >> worker.log"
    - "mkdir -p worker_temp"
```

#### `on_worker_complete`

Executes when each worker completes (exits the main loop).

**When it fires**: After a worker finishes processing jobs and before it shuts down.

**Typical use cases**:

- Worker-specific cleanup
- Uploading worker-specific logs
- Recording worker completion metrics
- Cleaning up worker-local resources

```yaml
- trigger_type: "on_worker_complete"
  action_type: "run_commands"
  persistent: true  # Each worker executes this
  commands:
    - "echo 'Worker completed on $(hostname) at $(date)' >> worker.log"
    - "rm -rf worker_temp"
```

## Job Selection

For `on_jobs_ready` and `on_jobs_complete` triggers, specify which jobs to monitor.

### Exact Job Names

```yaml
- trigger_type: "on_jobs_complete"
  action_type: "run_commands"
  jobs: ["job1", "job2", "job3"]
  commands:
    - "echo 'Specific jobs complete'"
```

### Regular Expressions

```yaml
- trigger_type: "on_jobs_ready"
  action_type: "schedule_nodes"
  job_name_regexes: ["train_model_[0-9]+", "eval_.*"]
  scheduler: "gpu_cluster"
  scheduler_type: "slurm"
  num_allocations: 2
```

**Common regex patterns**:

- `"train_.*"` - All jobs starting with "train_"
- `"model_[0-9]+"` - Jobs like "model_1", "model_2"
- `".*_stage1"` - All jobs ending with "_stage1"
- `"job_(a|b|c)"` - Jobs "job_a", "job_b", or "job_c"

### Combining Selection Methods

You can use both together - the action triggers when **all** matching jobs meet the condition:

```yaml
jobs: ["critical_job"]
job_name_regexes: ["batch_.*"]
# Triggers when "critical_job" AND all "batch_*" jobs are ready/complete
```

## Action Types

### `run_commands`

Execute shell commands sequentially on a compute node.

**Configuration**:

```yaml
- trigger_type: "on_workflow_complete"
  action_type: "run_commands"
  commands:
    - "tar -czf results.tar.gz output/"
    - "aws s3 cp results.tar.gz s3://bucket/"
```

**Execution details**:

- Commands run in the workflow's output directory
- Commands execute sequentially (one after another)
- If a command fails, the action fails (but workflow continues)
- Commands run on compute nodes, not the submission node
- Uses the shell environment of the job runner process

### `schedule_nodes`

Dynamically allocate compute resources from a Slurm scheduler.

**Configuration**:

```yaml
- trigger_type: "on_jobs_ready"
  action_type: "schedule_nodes"
  jobs: ["train_model_1", "train_model_2"]
  scheduler: "gpu_cluster"
  scheduler_type: "slurm"
  num_allocations: 2
```

**Parameters**:

- `scheduler` (required) - Name of Slurm scheduler configuration (must exist in `slurm_schedulers`)
- `scheduler_type` (required) - Must be "slurm"
- `num_allocations` (required) - Number of Slurm allocation requests to submit
- `start_one_worker_per_node` (optional, default: false) - Launch one worker per allocated node via
  `srun --ntasks-per-node=1`. Use this for direct-mode workflows with single-node jobs sharing a
  multi-node allocation. Not compatible with `execution_config.mode: slurm`.

**Use cases**:

- Just-in-time resource allocation
- Cost optimization (allocate only when needed)
- Separating workflow phases with different resource requirements

## Complete Examples

Refer to this
[example](https://github.com/NatLabRockies/torc/blob/main/examples/yaml/workflow_actions_simple_slurm.yaml)

## Execution Model

### Action Claiming and Execution

1. **Atomic Claiming**: Actions are claimed atomically by workers to prevent duplicate execution
2. **Non-Persistent Actions**: Execute once per workflow (first worker to claim executes)
3. **Persistent Actions**: Can be claimed and executed by multiple workers
4. **Trigger Counting**: Job-based triggers increment a counter as jobs transition; action becomes
   pending when count reaches threshold
5. **Immediate Availability**: Worker lifecycle actions are immediately available after workflow
   initialization

### Action Lifecycle

```
[Workflow Created]
    ↓
[initialize_jobs called]
    ↓
├─→ on_workflow_start actions become pending
├─→ on_worker_start actions become pending (persistent)
├─→ on_worker_complete actions become pending (persistent)
└─→ on_jobs_ready actions wait for job transitions
    ↓
[Worker Claims and Executes Actions]
    ↓
[Jobs Execute]
    ↓
[Jobs Complete]
    ↓
├─→ on_jobs_complete actions become pending when all specified jobs complete
└─→ on_workflow_complete actions become pending when all jobs complete
    ↓
[Workers Exit]
    ↓
[on_worker_complete actions execute per worker]
```

### Important Characteristics

1. **No Rollback**: Failed actions don't affect workflow execution
2. **Compute Node Execution**: Actions run on compute nodes via job runners
3. **One-Time Triggers**: Non-persistent actions trigger once when conditions are first met
4. **No Inter-Action Dependencies**: Actions don't depend on other actions
5. **Concurrent Workers**: Multiple workers can execute different actions simultaneously

### Workflow Reinitialization

When a workflow is reinitialized (e.g., after resetting failed jobs), actions are reset to allow
them to trigger again:

1. **Executed flags are cleared**: All actions can be claimed and executed again
2. **Trigger counts are recalculated**: For `on_jobs_ready` and `on_jobs_complete` actions, the
   trigger count is set based on current job states

**Example scenario**:

- job1 and job2 are independent jobs
- postprocess_job depends on both job1 and job2
- An `on_jobs_ready` action triggers when postprocess_job becomes ready

After first run completes:

1. job1 fails, job2 succeeds
2. User resets failed jobs and reinitializes
3. job2 is already Completed, so it counts toward the trigger count
4. When job1 completes in the second run, postprocess_job becomes ready
5. The action triggers again because the trigger count reaches the required threshold

This ensures actions properly re-trigger after workflow reinitialization, even when some jobs remain
in their completed state.

## Limitations

1. **No Action Dependencies**: Actions cannot depend on other actions completing
2. **No Conditional Execution**: Actions cannot have conditional logic (use multiple actions with
   different job selections instead)
3. **No Action Retries**: Failed actions are not automatically retried
4. **Single Action Type**: Each action has one action_type (cannot combine run_commands and
   schedule_nodes)
5. **No Dynamic Job Selection**: Job names/patterns are fixed at action creation time

For complex workflows requiring these features, consider:

- Using job dependencies to order operations
- Creating separate jobs for conditional logic
- Implementing retry logic within command scripts
- Creating multiple actions for different scenarios
