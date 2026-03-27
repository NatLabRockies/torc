# Advanced Slurm Configuration

This guide covers advanced Slurm configuration for users who need fine-grained control over their
HPC workflows.

> **For most users**: See [Slurm Overview](./slurm-workflows.md) for the recommended approach using
> `torc slurm generate` + `torc submit`. You don't need to manually configure schedulers or
> actions—Torc handles this automatically.

## When to Use Manual Configuration

Manual Slurm configuration is useful when you need:

- Custom Slurm directives (e.g., `--constraint`, `--exclusive`)
- Multi-node jobs with specific topology requirements
- Shared allocations across multiple jobs for efficiency
- Non-standard partition configurations
- Fine-tuned control over allocation timing

## Torc Server Requirements

The Torc server must be accessible from compute nodes:

- **External server** (Recommended): A team member allocates a shared server in the HPC environment.
  This is recommended if your operations team provides this capability.
- **Login node**: Suitable for small workflows. The server runs single-threaded by default. If you
  have many thousands of short jobs, check with your operations team about resource limits.

## Manual Scheduler Configuration

### Defining Slurm Schedulers

Define schedulers in your workflow specification:

```yaml
slurm_schedulers:
  - name: standard
    account: my_project
    nodes: 1
    walltime: "12:00:00"
    partition: compute
    mem: 64G

  - name: gpu_nodes
    account: my_project
    nodes: 1
    walltime: "08:00:00"
    partition: gpu
    gres: "gpu:4"
    mem: 256G
```

### Scheduler Fields

| Field             | Description                         | Required |
| ----------------- | ----------------------------------- | -------- |
| `name`            | Scheduler identifier                | Yes      |
| `account`         | Slurm account/allocation            | Yes      |
| `nodes`           | Number of nodes                     | Yes      |
| `walltime`        | Time limit (HH:MM:SS or D-HH:MM:SS) | Yes      |
| `partition`       | Slurm partition                     | No       |
| `mem`             | Memory per node                     | No       |
| `gres`            | Generic resources (e.g., GPUs)      | No       |
| `qos`             | Quality of Service                  | No       |
| `ntasks_per_node` | Tasks per node                      | No       |
| `tmp`             | Temporary disk space                | No       |
| `extra`           | Additional sbatch arguments         | No       |

### Defining Workflow Actions

Actions trigger scheduler allocations:

```yaml
actions:
  - trigger_type: on_workflow_start
    action_type: schedule_nodes
    scheduler: standard
    scheduler_type: slurm
    num_allocations: 1

  - trigger_type: on_jobs_ready
    action_type: schedule_nodes
    jobs: [train_model]
    scheduler: gpu_nodes
    scheduler_type: slurm
    num_allocations: 2
```

### Action Trigger Types

| Trigger                | Description                            |
| ---------------------- | -------------------------------------- |
| `on_workflow_start`    | Fires when workflow is submitted       |
| `on_jobs_ready`        | Fires when specified jobs become ready |
| `on_jobs_complete`     | Fires when specified jobs complete     |
| `on_workflow_complete` | Fires when all jobs complete           |

### Assigning Jobs to Schedulers

Reference schedulers in job definitions:

```yaml
jobs:
  - name: preprocess
    command: ./preprocess.sh
    scheduler: standard

  - name: train
    command: python train.py
    scheduler: gpu_nodes
    depends_on: [preprocess]
```

## Scheduling Strategies

### Strategy 1: Many Single-Node Allocations

Submit multiple Slurm jobs, each with its own Torc worker:

```yaml
slurm_schedulers:
  - name: work_scheduler
    account: my_account
    nodes: 1
    walltime: "04:00:00"

actions:
  - trigger_type: on_workflow_start
    action_type: schedule_nodes
    scheduler: work_scheduler
    scheduler_type: slurm
    num_allocations: 10
```

**When to use:**

- Jobs have diverse resource requirements
- Want independent time limits per job
- Cluster has low queue wait times

**Benefits:**

- Maximum scheduling flexibility
- Independent time limits per allocation
- Fault isolation

**Drawbacks:**

- More Slurm queue overhead
- Multiple jobs to schedule

### Strategy 2: Multi-Node Allocation

A single Torc worker manages all nodes in the allocation. The worker reports the total resources
across all nodes (CPUs × nodes, memory × nodes, etc.) and launches each job via `srun --exact`,
which lets Slurm place it on whichever node has capacity:

```yaml
slurm_schedulers:
  - name: work_scheduler
    account: my_account
    nodes: 10
    walltime: "04:00:00"

actions:
  - trigger_type: on_workflow_start
    action_type: schedule_nodes
    scheduler: work_scheduler
    scheduler_type: slurm
    num_allocations: 1
```

**When to use:**

- Many single-node jobs with similar requirements
- Want faster queue scheduling (larger jobs often prioritized)
- MPI or multi-node jobs that span multiple nodes

**Benefits:**

- Single queue wait
- Full per-step `sacct` accounting and cgroup enforcement
- Slurm handles node placement automatically via `srun --exact`

**Drawbacks:**

- Shared time limit for all jobs in the allocation

## Staged Allocations

For pipelines with distinct phases, stage allocations to avoid wasted resources:

```yaml
slurm_schedulers:
  - name: preprocess_sched
    account: my_project
    nodes: 2
    walltime: "01:00:00"

  - name: compute_sched
    account: my_project
    nodes: 20
    walltime: "08:00:00"

  - name: postprocess_sched
    account: my_project
    nodes: 1
    walltime: "00:30:00"

actions:
  # Preprocessing starts immediately
  - trigger_type: on_workflow_start
    action_type: schedule_nodes
    scheduler: preprocess_sched
    scheduler_type: slurm
    num_allocations: 1

  # Compute nodes allocated when compute jobs are ready
  - trigger_type: on_jobs_ready
    action_type: schedule_nodes
    jobs: [compute_step]
    scheduler: compute_sched
    scheduler_type: slurm
    num_allocations: 1

  # Postprocessing allocated when those jobs are ready
  - trigger_type: on_jobs_ready
    action_type: schedule_nodes
    jobs: [postprocess]
    scheduler: postprocess_sched
    scheduler_type: slurm
    num_allocations: 1
```

> **Note**: The `torc slurm generate` + `torc submit` command handles this automatically by
> analyzing job dependencies.

## Custom Slurm Directives

Use the `extra` field for additional sbatch arguments:

```yaml
slurm_schedulers:
  - name: exclusive_nodes
    account: my_project
    nodes: 4
    walltime: "04:00:00"
    extra: "--exclusive --constraint=skylake"
```

## Submitting Workflows

### With Manual Configuration

```bash
# Submit workflow with pre-defined schedulers and actions
torc submit workflow.yaml
```

### Scheduling Additional Nodes

Add more allocations to a running workflow:

```bash
torc slurm schedule-nodes -n 5 $WORKFLOW_ID
```

## Debugging

### Check Slurm Job Status

```bash
squeue --me
```

### View Torc Worker Logs

Workers log to the Slurm output file. Check:

```bash
cat slurm-<jobid>.out
```

### Verify Server Connectivity

From a compute node:

```bash
curl $TORC_API_URL/health
```

## srun Job Step Wrapping

When Torc detects that it is running inside a Slurm allocation (`SLURM_JOB_ID` is set in the
environment), it automatically wraps each individual job with `srun`. This creates a dedicated Slurm
job step for every Torc job, which provides:

- **Cgroup enforcement** — Slurm enforces CPU and memory limits from the job's resource
  requirements. Jobs that exceed their stated requirements are immediately killed.
- **`sstat` visibility** — HPC administrators and users can inspect per-step metrics (CPU, memory,
  wall-time) with `sstat -j <SLURM_JOB_ID>`.
- **Scheduler awareness** — Every running Torc job appears as a named step in `squeue`, giving the
  HPC team and users full visibility into what is actually executing.
- **Accounting data** — After each step exits, Torc calls `sacct` to collect Slurm accounting
  statistics and stores them with the job result (see
  [Slurm Accounting Stats](#slurm-accounting-stats) below).

### Step Naming

Each `srun` step is named `wf<workflow_id>_j<job_id>_r<run_id>_a<attempt_id>`, for example
`wf10_j42_r1_a1`. This name appears in `squeue --me` and `sacct` output, and the same component
string is embedded in the log file prefix `job_wf<workflow_id>_j<job_id>_r<run_id>_a<attempt_id>`
(for example, `job_wf10_j42_r1_a1.o`), so all Slurm and Torc records for a job can be easily
correlated.

### Multi-Node Jobs

> For a comprehensive guide to multi-node patterns, see [Multi-Node Jobs](./multi-node-jobs.md).

The `num_nodes` resource requirement field controls how many nodes each job step spans
(`srun --nodes`). It defaults to `1`. The Slurm allocation size (`sbatch --nodes`) is set separately
via the Slurm scheduler configuration.

**Single-node jobs (default)** — no extra configuration needed:

```yaml
resource_requirements:
  - name: standard
    num_cpus: 4
    memory: 16g
    runtime: PT2H
    # num_nodes defaults to 1
```

**True multi-node jobs** (MPI, Julia `Distributed.jl`, etc.) — the job spans multiple nodes in the
allocation:

```yaml
resource_requirements:
  - name: mpi_job
    num_cpus: 32
    memory: 128g
    runtime: PT8H
    num_nodes: 4      # srun spans all 4 nodes; allocation size set via scheduler
```

In this pattern, the step spans 4 nodes exclusively, and Torc passes `srun --nodes=4` when launching
the job. The job command receives `SLURM_JOB_NODELIST`, `SLURM_NTASKS`, and the rest of the standard
Slurm step environment, so MPI launchers (`mpirun`, `mpiexec`) and Julia `Distributed.jl` will
automatically use all allocated nodes.

### Multi-Node Allocation Rule

Inside a multi-node Slurm allocation, Torc uses two scheduling modes:

- Single-node jobs (`num_nodes=1`) may share nodes based on CPU, memory, and GPU availability.
- Multi-node jobs (`num_nodes>1`) reserve whole nodes exclusively.

This keeps job claiming and local resource accounting aligned with Slurm allocations.

### Resource Limit Enforcement

In Slurm mode, Torc always passes `--cpus-per-task` and `--mem` to `srun` so Slurm enforces the
cgroup limits defined in each job's resource requirements. These flags work together with `--exact`
to allow multiple job steps to run concurrently on shared nodes.

> **Note**: `limit_resources: false` is not supported in Slurm mode. If you need to run jobs without
> resource enforcement inside a Slurm allocation, use `mode: direct` instead:
>
> ```yaml
> execution_config:
>   mode: direct
>   limit_resources: false
> ```
>
> In direct mode, jobs run as plain processes without `srun` wrapping. This means you lose per-step
> `sacct` accounting and cgroup isolation, but jobs can use any available resources without
> restriction.

### Disabling srun Wrapping

To disable srun wrapping entirely and run jobs via direct shell execution inside a Slurm allocation,
set `mode: direct` in your execution config:

```yaml
execution_config:
  mode: direct
```

In direct mode, Slurm accounting (`sacct`) and live monitoring (`sstat`) are unavailable since jobs
do not run as Slurm steps. However, Torc's own resource monitor can still track memory and CPU usage
if enabled.

> **Note**: Direct mode inside a Slurm allocation is useful when `srun` has compatibility issues, or
> when you want to run jobs without resource limits (`limit_resources: false`). For most workflows,
> the default auto mode (which selects Slurm mode inside allocations) is recommended.

### Slurm Accounting Stats

After each job step exits, Torc calls `sacct` once to collect the following Slurm-native accounting
fields and stores them in the `slurm_stats` table:

| Field                  | sacct source   | Description                           |
| ---------------------- | -------------- | ------------------------------------- |
| `max_rss_bytes`        | `MaxRSS`       | Peak resident-set size (from cgroups) |
| `max_vm_size_bytes`    | `MaxVMSize`    | Peak virtual memory size              |
| `max_disk_read_bytes`  | `MaxDiskRead`  | Peak disk read bytes                  |
| `max_disk_write_bytes` | `MaxDiskWrite` | Peak disk write bytes                 |
| `ave_cpu_seconds`      | `AveCPU`       | Average CPU time in seconds           |
| `node_list`            | `NodeList`     | Nodes used by the job step            |

These fields complement the existing sysinfo-based metrics (`peak_memory_bytes`, `peak_cpu_percent`,
etc.) and are available via `torc slurm stats <workflow_id>`.

`sacct` data is collected on a best-effort basis. Fields are `null` when:

- The job ran locally (no `SLURM_JOB_ID`)
- `sacct` is not available on the node
- The step was not found in the Slurm accounting database at collection time

### Local Execution

When running locally (no `SLURM_JOB_ID` environment variable), Torc uses its standard shell wrapper
and the `srun` behavior is never triggered. No configuration is needed for local runs.

## See Also

- [Slurm Overview](./slurm-workflows.md) — Simplified workflow approach
- [HPC Profiles](./hpc-profiles.md) — Automatic partition matching
- [Workflow Actions](../design/workflow-actions.md) — Action system details
- [Debugging Slurm Workflows](./debugging-slurm.md) — Troubleshooting guide
