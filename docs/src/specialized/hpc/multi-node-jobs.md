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

**How it works**: Torc requests a multi-node Slurm allocation (e.g., 4 nodes). One worker manages
the allocation and places each single-node job onto one node via `srun --nodes=1`. Single-node jobs
may share a node as long as CPU, memory, and GPU limits allow. With N nodes, Torc can spread work
across the allocation for throughput.

**Example**: 100 independent analysis jobs, each needing 8 CPUs and 32 GB, across a 4-node
allocation:

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

Each node has 8 CPUs and 32 GB available per job. If a node has 64 CPUs total, it can run up to 8
jobs concurrently (64 / 8 = 8). Across 4 nodes, that means up to 32 jobs running at once.

### Pattern 2: True Multi-Node Jobs (MPI, Distributed Training)

**Use when**: A single job needs to span multiple nodes — for example, MPI applications, distributed
deep learning, or Julia `Distributed.jl`.

**How it works**: You set `num_nodes` to the number of nodes the job needs. Torc treats that job as
an exclusive whole-node reservation: if a job needs 4 nodes, those 4 nodes are reserved for that
step and are not shared with other jobs until the step completes. Torc passes
`srun --nodes=<num_nodes>` when launching the job, so the process spans multiple nodes within the
allocation. The job receives the standard Slurm step environment (`SLURM_JOB_NODELIST`,
`SLURM_NTASKS`, etc.), so MPI launchers and distributed frameworks work automatically.

**Example**: An MPI training job that spans all 4 nodes in the allocation:

```yaml
name: distributed_training
description: MPI training across 4 nodes

resource_requirements:
  - name: mpi_training
    num_cpus: 32      # per node (128 total across 4 nodes)
    memory: 128g      # per node (512 GB total across 4 nodes)
    num_nodes: 4      # allocates and spans 4 nodes
    runtime: PT8H

jobs:
  - name: train
    command: srun --mpi=pmix python -m torch.distributed.run train.py
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
to the job are 128 CPUs and 512 GB.

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

Request all nodes in a single `sbatch` job (e.g., `nodes: 4`). Torc runs one worker per node and
distributes jobs across them.

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

For example, single-node preprocessing jobs followed by a multi-node MPI training step:

```yaml
name: preprocess_then_train

resource_requirements:
  - name: preprocess
    num_cpus: 4
    memory: 16g
    runtime: PT30M

  - name: mpi_training
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
    command: srun --mpi=pmix python train.py
    resource_requirements: mpi_training
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
requested for the MPI training job (Pattern 2).

## `num_nodes`

The `num_nodes` field controls both the Slurm allocation size (`sbatch --nodes`) and the number of
nodes each job step spans (`srun --nodes`). It defaults to `1`.

- **Pattern 1**: `num_nodes=1` (default) -- each job runs on a single node
- **Pattern 2**: `num_nodes=N` -- each job spans all N nodes (MPI, distributed training)

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

### Using `num_nodes > 1` for independent jobs

If your jobs don't need inter-node communication, keep `num_nodes=1` (the default) and let Torc
schedule them independently across nodes for maximum throughput.

## See Also

- [Slurm Overview](./slurm-workflows.md) — Auto-generated Slurm configuration with `submit-slurm`
- [Advanced Slurm Configuration](./slurm.md) — Manual scheduler and srun wrapping details
- [Resource Requirements Reference](../../core/reference/resources.md) — Complete field reference
