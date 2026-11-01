# Multi-Node Jobs

This guide explains how to run jobs across multiple nodes in a Slurm allocation. There are two
distinct patterns, and choosing the right one depends on whether your individual jobs need more than
one node.

## Key Concept: Per-Node Resource Values

**All resource values in Torc are per-node.** When you write `num_cpus: 32` and `memory: 128g`, you
are describing what each node provides — not the total across all nodes. This keeps the mental model
simple: resource requirements always describe what a single node looks like, regardless of how many
nodes are in the allocation.

## Two Patterns

### Pattern 1: Many Single-Node Jobs in a Multi-Node Allocation

**Use when**: You have many independent jobs that each fit on one node, and you want them to run in
parallel across multiple nodes for throughput.

**How it works**: Torc requests a multi-node Slurm allocation (e.g., 4 nodes). The behavior depends
on the execution mode:

- **Slurm mode** (default): A single worker manages the allocation and places each single-node job
  onto a node via `srun --nodes=1`. Slurm handles resource isolation and node placement.
- **Direct mode**: Jobs are executed directly without `srun` wrapping. To distribute work across
  nodes, set `start_one_worker_per_node: true` on the `schedule_nodes` action. This launches one
  worker per node via `srun --ntasks-per-node=1`, and each worker executes jobs directly on its
  node.

Single-node jobs may share a node as long as CPU, memory, and GPU limits allow. With N nodes, Torc
can spread work across the allocation for throughput.

**Example (Slurm mode)**: 100 independent analysis jobs, each needing 8 CPUs and 32 GB, across a
4-node allocation:

```yaml
name: parallel_analysis
description: Run 100 analysis tasks across 4 nodes

resource_requirements:
  - name: analysis
    num_cpus: 8       # per node
    memory: 32g       # per node
    runtime: PT1H

jobs:
  - name: analyze_{i}
    command: python analyze.py --chunk {i}
    resource_requirements: analysis
    scheduler: multi_node
    parameters:
      i: "1:100"

slurm_schedulers:
  - name: multi_node
    account: myproject
    nodes: 4
    walltime: "08:00:00"

actions:
  - trigger_type: on_workflow_start
    action_type: schedule_nodes
    scheduler: multi_node
    scheduler_type: slurm
    num_allocations: 1
```

**Example (Direct mode)**: The same workload using direct execution with one worker per node:

```yaml
name: parallel_analysis_direct
description: Run 20 analysis tasks across 2 nodes via direct execution

execution_config:
  mode: direct
  srun_mpi: "none"

resource_requirements:
  - name: analysis
    num_cpus: 5
    num_nodes: 1
    memory: 2g
    runtime: PT3M

jobs:
  - name: analyze_{i}
    command: python analyze.py --chunk {i}
    resource_requirements: analysis
    scheduler: multi_node
    parameters:
      i: "1:20"

slurm_schedulers:
  - name: multi_node
    account: myproject
    nodes: 2
    walltime: "00:10:00"

actions:
  - trigger_type: on_workflow_start
    action_type: schedule_nodes
    scheduler: multi_node
    scheduler_type: slurm
    start_one_worker_per_node: true
    num_allocations: 1
```

Each node has 8 CPUs and 32 GB available per job. If a node has 64 CPUs total, it can run up to 8
jobs concurrently (64 / 8 = 8). Across 4 nodes, that means up to 32 jobs running at once.

> **Note:** `start_one_worker_per_node` is only supported with `execution_config.mode: direct`. It
> is not compatible with slurm execution mode, where Torc uses a single worker with `srun`-based
> node placement.
>
> If your cluster requires an explicit MPI mode for the outer worker launch, set
> `execution_config.srun_mpi`. For example, `srun_mpi: "none"` produces:
>
> ```console
> srun --ntasks-per-node=1 --mpi=none torc-slurm-job-runner ...
> ```

### Pattern 2: True Multi-Node Jobs (MPI, Distributed Training)

**Use when**: A single job needs to span multiple nodes — for example, MPI applications, distributed
deep learning, or Julia `Distributed.jl`.

**How it works**: You set `num_nodes` to the number of nodes the job needs. Torc treats that job as
an exclusive whole-node reservation: if a job needs 4 nodes, those 4 nodes are reserved for that
step and are not shared with other jobs until the step completes. Torc passes
`srun --nodes=<num_nodes>` when launching the job, so the process spans multiple nodes within the
allocation. The job receives the standard Slurm step environment (`SLURM_JOB_NODELIST`,
`SLURM_NTASKS`, etc.), so MPI launchers and distributed frameworks work automatically.

**Important**: In slurm execution mode (the default), Torc wraps each job with `srun`. Do not use
`srun` or `mpirun` in your command — this would create nested process managers that conflict over
node and task placement. Just write the application command directly and let Torc handle the launch.
If you need explicit control over the MPI launcher (e.g., `mpirun`), use direct execution mode
instead (see example below).

**Example (Slurm mode)**: A distributed training job that spans all 4 nodes in the allocation:

```yaml
name: distributed_training
description: Distributed training across 4 nodes

resource_requirements:
  - name: mpi_training
    num_cpus: 32      # per node (128 total across 4 nodes)
    memory: 128g      # per node (512 GB total across 4 nodes)
    num_nodes: 4      # allocates and spans 4 nodes
    runtime: PT8H

jobs:
  - name: train
    command: python -m torch.distributed.run train.py
    resource_requirements: mpi_training
    scheduler: training_nodes

slurm_schedulers:
  - name: training_nodes
    account: myproject
    nodes: 4
    walltime: "12:00:00"

actions:
  - trigger_type: on_workflow_start
    action_type: schedule_nodes
    scheduler: training_nodes
    scheduler_type: slurm
    num_allocations: 1
```

Here `num_cpus: 32` and `memory: 128g` describe each of the 4 nodes. The total resources available
to the job are 128 CPUs and 512 GB. Torc launches this as
`srun --nodes=4 python -m torch.distributed.run train.py`.

**Example (Direct mode)**: If you need to use `mpirun` or another MPI launcher explicitly, use
direct execution mode so Torc does not wrap the command with `srun`:

```yaml
name: distributed_training_mpi
description: MPI training across 4 nodes with explicit mpirun

execution_config:
  mode: direct

resource_requirements:
  - name: mpi_training
    num_cpus: 32
    memory: 128g
    num_nodes: 4
    runtime: PT8H

jobs:
  - name: train
    command: mpirun python train.py
    resource_requirements: mpi_training
    scheduler: training_nodes

slurm_schedulers:
  - name: training_nodes
    account: myproject
    nodes: 4
    walltime: "12:00:00"

actions:
  - trigger_type: on_workflow_start
    action_type: schedule_nodes
    scheduler: training_nodes
    scheduler_type: slurm
    num_allocations: 1
```

## Choosing Between the Patterns

| Question                                      | Pattern 1 (single-node jobs) | Pattern 2 (multi-node jobs) |
| --------------------------------------------- | ---------------------------- | --------------------------- |
| Does each job fit on one node?                | Yes                          | No                          |
| Does the job use MPI or distributed training? | No                           | Yes                         |
| Goal is throughput (many jobs in parallel)?   | Yes                          | No                          |
| Goal is scaling one job across nodes?         | No                           | Yes                         |
| `num_nodes` setting                           | `1` (default)                | Number of nodes needed      |

## Whole-Node Reservation Rule

Torc uses this scheduling rule inside a multi-node Slurm allocation:

- Single-node jobs (`num_nodes=1`) may share a node.
- Multi-node jobs (`num_nodes>1`) reserve whole nodes exclusively.

This is intentionally conservative. Torc does not try to pack other work onto nodes that are part of
an active multi-node step.

## Allocation Strategy: One Large vs. Many Small

When running many single-node jobs (Pattern 1), you also need to decide how to request the
underlying Slurm allocations. There are two approaches, each with trade-offs.

### One multi-node allocation

Request all nodes in a single `sbatch` job (e.g., `nodes: 4`). In slurm mode, a single worker
distributes jobs across nodes via `srun`. In direct mode with `start_one_worker_per_node`, Torc runs
one worker per node and each worker executes jobs locally.

**Advantages:**

- Slurm schedulers typically give **priority to larger allocations**. On busy clusters, a 4-node job
  may start sooner than four separate 1-node jobs.
- Works well when all jobs take roughly the **same amount of time**, because you pay for all nodes
  until the last job finishes.

**Disadvantages:**

- **Wasted node-hours when runtimes vary.** If one job takes 4 hours and the rest take 30 minutes,
  three nodes sit idle for 3.5 hours waiting for the slow job to finish. You are billed for all four
  nodes for the full 4 hours.
- On heavily loaded clusters, large allocations can take **much longer to schedule** because Slurm
  must find all nodes free at the same time.

### Many single-node allocations

Request separate 1-node allocations (e.g., `num_allocations: 4` with `nodes: 1`). Each allocation
runs its own worker and pulls jobs independently.

**Advantages:**

- **Tolerant of variable runtimes.** Each allocation finishes as soon as its last job completes.
  Fast jobs release their nodes immediately instead of waiting for the slowest one.
- Single-node jobs are **easier for Slurm to schedule** because they can fill gaps in the queue. You
  may start running sooner and finish sooner overall.
- If one allocation fails or is preempted, the others keep running. The failed allocation's
  incomplete jobs return to the ready queue and are picked up by another worker.

**Disadvantages:**

- On clusters that prioritize large jobs, many small allocations may sit in the queue longer.
- More Slurm jobs to manage (though Torc handles this automatically).

### Which to choose

| Scenario                                       | Recommended strategy      |
| ---------------------------------------------- | ------------------------- |
| Jobs have similar runtimes, cluster is busy    | One multi-node allocation |
| Jobs have variable runtimes                    | Many single-node allocs   |
| Cluster has long queue wait for large jobs     | Many single-node allocs   |
| Cluster prioritizes large jobs, queue is short | One multi-node allocation |

You can also mix strategies: use a multi-node allocation for a batch of similar jobs, then switch to
single-node allocations for a stage with variable runtimes. Torc's scheduler actions make this easy
to express per workflow stage.

## Mixing Both Patterns

A workflow can combine both patterns, but the cleanest approach is to use separate stages or
separate allocations. Once a true multi-node step starts, Torc reserves whole nodes for it
exclusively.

For example, single-node preprocessing jobs followed by a multi-node training step:

```yaml
name: preprocess_then_train

resource_requirements:
  - name: preprocess
    num_cpus: 4
    memory: 16g
    runtime: PT30M

  - name: distributed_training
    num_cpus: 32
    memory: 128g
    num_nodes: 4
    runtime: PT4H

jobs:
  - name: prep_{i}
    command: python preprocess.py --shard {i}
    resource_requirements: preprocess
    scheduler: prep_nodes
    parameters:
      i: "1:8"

  - name: train
    command: python -m torch.distributed.run train.py
    resource_requirements: distributed_training
    scheduler: training_nodes
    depends_on: [prep_1, prep_2, prep_3, prep_4, prep_5, prep_6, prep_7, prep_8]

slurm_schedulers:
  - name: prep_nodes
    account: myproject
    nodes: 2
    walltime: "01:00:00"

  - name: training_nodes
    account: myproject
    nodes: 4
    walltime: "06:00:00"

actions:
  - trigger_type: on_workflow_start
    action_type: schedule_nodes
    scheduler: prep_nodes
    scheduler_type: slurm
    num_allocations: 1

  - trigger_type: on_jobs_ready
    action_type: schedule_nodes
    jobs: [train]
    scheduler: training_nodes
    scheduler_type: slurm
    num_allocations: 1
```

The preprocessing jobs run across 2 nodes (Pattern 1). When they complete, a 4-node allocation is
requested for the training job (Pattern 2). Torc wraps the training command with `srun --nodes=4`
automatically.

## MPI and Single-Node Stages in One Allocation

**Use when**: A workflow alternates between MPI jobs that span every node and many single-node jobs,
and you want to wait in the Slurm queue only once.

For example, in a 100-node allocation:

1. Run an MPI job across all nodes.
2. Run 200 single-node jobs that use its output, with two jobs fitting on each node.
3. Run a second MPI job across all nodes that uses their output.
4. Run 100 single-node jobs that use its output.

**How it works**: Use one job runner, started on the first node by the batch script, and do not set
`start_one_worker_per_node`. Set `execution_config.mode: direct` so that Torc runs each command as
written, and call `srun` in each command. The single-node jobs still run on the compute nodes
because `srun` places them there. File dependencies keep the stages in order, so you do not need job
labels or workflow actions to switch between them.

```yaml
execution_config:
  mode: direct
  limit_resources: false

slurm_schedulers:
  - name: all_nodes
    account: my_project
    nodes: 100
    mem: 240G
    walltime: "16:00:00"

resource_requirements:
  - name: mpi_all_nodes
    num_cpus: 104
    memory: 230g
    num_nodes: 100
    runtime: PT4H

  - name: half_node
    num_cpus: 52
    memory: 110g
    runtime: PT2H

jobs:
  - name: solve_initial
    command: >-
      srun --mpi=pmix --nodes=100 --ntasks=10400 --cpus-per-task=1
      ./solver --output output/solve_initial.h5
    scheduler: all_nodes
    resource_requirements: mpi_all_nodes
    output_files: [solve_initial_output]

  - name: postprocess_{i:03d}
    command: >-
      srun --exact --nodes=1 --ntasks=1 --cpus-per-task=52 --mem=110G
      ./postprocess --input output/solve_initial.h5 --index {i}
      --output output/postprocess_{i:03d}.h5
    scheduler: all_nodes
    resource_requirements: half_node
    input_files: [solve_initial_output]
    output_files: ["postprocess_{i:03d}_output"]
    parameters:
      i: "1:200"
```

The complete workflow, including the `files` section and the `schedule_nodes` action, is in
[`single_allocation_mpi_stages.yaml`](https://github.com/NatLabRockies/torc/blob/main/examples/yaml/single_allocation_mpi_stages.yaml).

Keep these points in mind:

- **Keep requirements and `srun` flags in step.** Torc uses `resource_requirements` to decide when
  to start a job, and Slurm uses the `srun` flags to run it. Change them together.
- **Use `--exact` on single-node jobs.** It lets jobs share a node. Leave it off the MPI command so
  that the step gets the resources of every node.
- **Leave memory free on the first node.** The runner and one `srun` process per running job share
  that node with its jobs. Measure this on your cluster at close to your full job count.
- **Expect no useful resource data.** The runner sees only the local `srun` process, so it cannot
  measure or limit what the job uses on other nodes. Set `limit_resources: false`.
- **Set the walltime for all stages combined.** A job is claimed only if its `runtime` fits in the
  time remaining in the allocation.

> **Note:** Any multi-node job in a workflow makes the runner track the allocation's resources as a
> total rather than node by node. A job then fits only if the free total divided by the number of
> nodes is at least the job's size. The single-node stages can therefore run below capacity: in this
> example, about 100 to 150 of the possible 200 jobs at once. Allow for this in the walltime.

Before running this under Torc, test the same `srun` commands in a plain `sbatch` script on a few
nodes. If that script fails, the problem is in the cluster setup or the application.

## `num_nodes`

The `num_nodes` field controls how many nodes each job step spans (`srun --nodes`). It defaults to
`1`. The Slurm allocation size (`sbatch --nodes`) is set separately via the Slurm scheduler
configuration.

- **Pattern 1**: `num_nodes=1` (default) -- each job runs on a single node; allocation size is set
  on the scheduler
- **Pattern 2**: `num_nodes=N` -- each job spans N nodes (MPI, distributed training)

## Common Mistakes

### Specifying total resources instead of per-node

```yaml
# WRONG: 512g is the total across 4 nodes
resource_requirements:
  - name: mpi_job
    num_cpus: 128     # total across nodes
    memory: 512g      # total across nodes
    num_nodes: 4

# CORRECT: 128g is what each of the 4 nodes provides
resource_requirements:
  - name: mpi_job
    num_cpus: 32      # per node (128 total)
    memory: 128g      # per node (512g total)
    num_nodes: 4
```

### Using `srun` or `mpirun` in job commands with slurm execution mode

In slurm mode (the default), Torc wraps each job with `srun`. Adding `srun` or `mpirun` to your
command creates nested process managers that conflict over node and task placement.

```yaml
# WRONG: nested srun
command: srun --mpi=pmix python train.py

# WRONG: mpirun under Torc's srun
command: mpirun python train.py

# CORRECT: let Torc handle srun wrapping
command: python train.py
```

If you need explicit control over `mpirun`, use `execution_config.mode: direct` so Torc does not
wrap the command with `srun`.

### Using `num_nodes > 1` for independent jobs

If your jobs don't need inter-node communication, keep `num_nodes=1` (the default) and let Torc
schedule them independently across nodes for maximum throughput.

## See Also

- [Slurm Overview](./slurm-workflows.md) — Auto-generated Slurm configuration with
  `slurm generate` + `submit`
- [Advanced Slurm Configuration](./slurm.md) — Manual scheduler and srun wrapping details
- [Resource Requirements Reference](../../core/reference/resources.md) — Complete field reference
