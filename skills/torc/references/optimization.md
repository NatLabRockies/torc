# Optimizing throughput and cost

Slurm cost and time-to-solution are decided almost entirely by resource requirements, because Torc
derives packing and allocation counts from them arithmetically. Get the requirements right and the
rest follows.

## Contents

- [The three numbers](#the-three-numbers)
- [When runs can share a node](#when-runs-can-share-a-node)
- [Splitting runs by memory and time-to-solution](#splitting-runs-by-memory-and-time-to-solution)
- [Jobs per node versus more nodes](#jobs-per-node-versus-more-nodes)
- [Walltime: queue wait versus allocation reuse](#walltime-queue-wait-versus-allocation-reuse)
- [One large allocation or many small](#one-large-allocation-or-many-small)
- [Long chains that exceed one walltime](#long-chains-that-exceed-one-walltime)
- [Multi-node jobs](#multi-node-jobs)
- [When to abandon resource-aware packing](#when-to-abandon-resource-aware-packing)
- [Measure, then correct](#measure-then-correct)
- [Verification commands](#verification-commands)

## The three numbers

`torc slurm generate` computes, per scheduler group:

```text
concurrent_jobs_per_node = max(1, min(node_cpus / job_cpus,
                                      node_mem  / job_mem,
                                      node_gpus / job_gpus))   # integer division
time_slots               = max(1, allocation_walltime / job_runtime)
jobs_per_allocation      = concurrent_jobs_per_node * time_slots
allocations              = ceil(job_count / jobs_per_allocation) * nodes_per_job
```

Walltime itself comes from the strategy: `max-job-runtime` (default) uses
`ceil(max_job_runtime * multiplier)` with a default multiplier of 1.5, capped at the partition
maximum; `max-partition-time` uses the partition maximum.

Every dimension uses integer division, so the tightest one wins and remainders are wasted. A job
asking for 40% of a node's memory gets 2 per node, not 2.5, and the other 20% is idle.

The generator also merges resource requirements that map to the same partition and takes the
**maximum of each dimension** across the merged set. That single behavior causes most accidental
over-provisioning; see [splitting](#splitting-runs-by-memory-and-time-to-solution).

Worked example on a 104-CPU, 240 GB node with a 4-hour partition maximum:

| Job shape                                      | Count | Concurrent/node  | Slots | Allocations |
| ---------------------------------------------- | ----- | ---------------- | ----- | ----------- |
| 8 CPU, 16 GB, `PT1H`                           | 40    | 13 (CPU-bound)   | 1     | 4           |
| 8 CPU, 100 GB, `PT1H`                          | 40    | 2 (memory-bound) | 1     | 20          |
| 104 CPU, 200 GB, `PT1H`                        | 12    | 1                | 1     | 12          |
| Same, `--walltime-strategy max-partition-time` | 12    | 1                | 4     | 3           |

The second row is the memory trap: same CPU request, same runtime, 5x the allocations because a 100
GB footprint only fits twice in 240 GB.

## When runs can share a node

Node sharing is not a setting; it is what the arithmetic allows. Runs share a node when the sum of
their per-job CPU, memory, and GPU requests fits within one node, and Torc packs greedily up to that
limit.

Conditions that must all hold:

- **Resource requirements are declared and accurate.** A job with none is assigned the `default`
  requirement Torc creates with every workflow: 1 CPU, `1m` memory, 1 node, `P0DT1M` runtime.
  Packing then believes each job is free, so a node accepts as many as the claim limit allows and
  the real footprint oversubscribes it. The 1-minute runtime also makes such jobs claimable on
  almost any walltime remainder.
- **The sum fits.** The server tracks consumed CPU, memory, and GPU per claim and refuses a job that
  would exceed the remaining capacity.
- **`num_nodes` is 1.** A job with `num_nodes > 1` reserves whole nodes exclusively and never
  shares.
- **The remaining walltime covers the job's `runtime`.** Fit is checked against remaining allocation
  time plus a 120-second startup grace period.
- **The partition allows it.** With Slurm-mode execution each job runs as an `srun` step inside the
  allocation, and on a multi-node allocation the runner claims per node and pins each step with
  `srun --nodelist=<node>`. Check `shared` in `torc hpc partitions <profile>` when a partition is
  not exclusive.

Good candidates for sharing: many small independent runs, single-threaded solves, I/O-bound
pre-processing, and parameter sweeps where one case uses a fraction of a node.

Poor candidates: jobs whose real memory peak approaches node memory, jobs that internally spawn
threads across all cores regardless of the declared `num_cpus`, and anything whose measured peak you
have not verified.

Two failure modes to avoid:

- **Under-declaring to force more sharing.** The declared value is what packing believes.
  Under-declare memory and you get co-tenants that OOM each other. In direct mode with
  `execution_config.limit_resources` (default true) the resource monitor SIGKILLs a job that exceeds
  its declared memory, with no grace period, and reports it with the configured `oom_exit_code`, so
  under-declaring surfaces as killed jobs rather than as extra throughput.
- **Threads exceeding the declared CPUs.** A job declared at 8 CPUs that starts 104 OpenMP threads
  will be packed 13 to a node and thrash. Pin the thread count (`OMP_NUM_THREADS`, `--threads`) to
  match `num_cpus`, and set it in the job `env` or `invocation_script`.

Verify sharing actually happened: `torc jobs running <id>` shows the compute node per running job,
so several jobs on one node name confirms co-tenancy.

## Splitting runs by memory and time-to-solution

Split when a single scheduler group forces its worst-case job's footprint onto every job. Because
merged groups take the maximum of each dimension, one large or slow job drags the whole group.

Measured example, 104-CPU / 240 GB nodes, 100 light jobs (4 CPU, 8 GB, `PT20M`) plus 4 heavy jobs
(52 CPU, 120 GB, `PT3H`):

| Grouping                           | Schedulers | Allocations   |
| ---------------------------------- | ---------- | ------------- |
| `--group-by partition` (default)   | 1          | **52**        |
| `--group-by resource-requirements` | 2          | **6** (4 + 2) |

Both map to the same partition, so the default merges them and plans as if all 104 jobs need 52
CPUs, 120 GB, and 3 hours. Splitting by resource requirement plans the light jobs at 26 per node
with a 30-minute walltime and the heavy jobs separately: an 8.7x reduction in allocations for
identical work.

How to decide:

1. **Group by memory band.** Sort the distinct memory footprints. If the largest is more than ~2x
   the smallest and both land in the same partition, split them.
2. **Group by runtime band.** A group's walltime comes from its longest job. Mixing a 20-minute job
   with a 3-hour job forces the short jobs into a long walltime, which queues worse and wastes the
   tail of every allocation.
3. **Send genuinely big jobs to a big partition.** If a job needs more memory than the standard node
   provides, it belongs on a high-memory partition. Torc routes this automatically when the
   requirement exceeds standard node memory: distinct partitions never merge.
4. **Keep the number of groups small.** Each group is its own allocation stream with its own queue
   wait. Two or three bands usually capture the benefit; ten fragments your fair-share.

Mechanisms, in increasing strength:

```bash
# Separate scheduler per resource requirement
torc slurm generate --account <acct> --group-by resource-requirements workflow.yaml -o gen.yaml
```

```yaml
# Or pin specific jobs to specific schedulers in the spec
slurm_schedulers:
  - name: light_pool
    account: myproject
    walltime: "00:30:00"
    nodes: 1
  - name: heavy_pool
    account: myproject
    walltime: "04:00:00"
    nodes: 1

jobs:
  - name: light_{i}
    command: ./light.sh {i}
    resource_requirements: light
    scheduler: light_pool
    parameters:
      i: "1:100"
  - name: heavy_{i}
    command: ./heavy.sh {i}
    resource_requirements: heavy
    scheduler: heavy_pool
    parameters:
      i: "1:4"
```

A job's `scheduler` binding is advisory by default: a worker whose own queue is empty will claim
jobs from another scheduler rather than idle. Set `client.slurm.strict_scheduler_match = true` when
a heavy job must never land on a pool sized for light work.

Time-to-solution also depends on where the work sits in the graph. Splitting helps throughput only
for jobs that are ready at the same time; a deep dependency chain is limited by its critical path no
matter how many allocations you buy. Check the achievable width with
`torc workflows execution-plan <spec>` before adding nodes.

## Jobs per node versus more nodes

Both raise concurrency, and they fail differently.

**More jobs per node** (smaller per-job requests, more co-tenancy) gives better utilization, fewer
allocations, less queue wait, and no inter-node communication. It is bounded by real node memory and
cores, and it degrades sharply if the declared footprint is wrong: co-tenants contend for memory
bandwidth, cache, and local disk, so per-job runtime can rise even when nothing is oversubscribed on
paper.

**More nodes** (more allocations, or more nodes per allocation) scales past one node's limits and
isolates jobs from each other. It costs queue wait per allocation, drains fair-share faster, and
gives each job a cold page cache and no shared local scratch.

Decide with this order:

1. **Compute the ceiling.** `concurrent_jobs_per_node` from the formula above is the maximum sharing
   the declared requirements allow. If it is already 1, more jobs per node is not available; go
   wider.
2. **Check against the true peak.** Compare declared memory with observed peak from
   `torc results list` or `torc workflows check-resources`. Right-sizing a padded requirement is the
   cheapest throughput win available, and it needs no new allocations.
3. **Confirm the graph is wide enough.** If ready-job width is below the current node count, extra
   nodes sit idle. `torc workflows execution-plan` shows the width.
4. **Prefer packing until utilization stops improving.** Raise co-tenancy first, then add nodes.
5. **Validate empirically on one allocation.** Run one allocation at the intended packing and
   compare per-job execution time against a lightly-loaded run. If per-job time inflates more than
   the throughput gain, back the packing off one step.

Do not use `--max-parallel-jobs` to force packing on heterogeneous work. It disables resource
tracking entirely: the runner starts exactly that many jobs regardless of CPU, memory, or GPU, so
three 64 GB jobs will start on a 128 GB node. It is the right tool only when jobs are uniform,
lightweight relative to the node, or I/O-bound. With resource requirements declared, leave it unset
and let the server pack.

For direct-mode execution across a multi-node allocation, set `start_one_worker_per_node: true` on
the `schedule_nodes` action so each node runs its own worker. Without it, one worker manages the
whole allocation.

## Walltime: queue wait versus allocation reuse

`time_slots = allocation_walltime / job_runtime` means a longer walltime lets one allocation absorb
more sequential jobs. Measured, 12 jobs each needing a full node for 1 hour on a 4-hour partition:

| Strategy                          | Walltime | Slots | Allocations |
| --------------------------------- | -------- | ----- | ----------- |
| `max-job-runtime` (default, x1.5) | 1:30     | 1     | 12          |
| `max-partition-time`              | 4:00     | 4     | 3           |

Shorter walltime requests usually clear the queue sooner and are easier to backfill, but each
allocation retires fewer jobs. Longer walltime reuses each allocation, at the cost of lower queue
priority and a partially idle tail.

Guidance:

- Many short jobs, busy cluster: prefer `max-partition-time` (or a larger `--walltime-multiplier`)
  so each allocation drains a real batch.
- Few long jobs: keep the default; a walltime much longer than the work wastes the tail.
- Accurate `runtime` matters in both directions. Overstated runtime shrinks `time_slots` and
  inflates allocations; understated runtime gets jobs killed at 152 and blocks packing as
  allocations age.
- An allocation's tail is always partly unused. Under resource-aware claiming the runner asks only
  for jobs whose `runtime` fits the remaining walltime (plus a 120-second startup grace period), so
  the last sub-`runtime` slice goes idle. Under `--max-parallel-jobs` the runner instead stops
  requesting work once remaining walltime drops below `compute_node_min_time_for_new_jobs_seconds`
  (default 300).

Keep an aging allocation productive with `compute_node_wait_for_new_jobs_seconds` (default 90),
which holds a worker briefly through a lull instead of exiting, and
`compute_node_ignore_workflow_completion` when jobs are still being added.

## One large allocation or many small

Given N nodes of work, `1 x N` requests them together and `N x 1` (the Torc default) requests them
separately.

`1 x N` benefits from Slurm's backfill reservation for large jobs, consumes fair-share once, and
finishes within one walltime window; it must wait for N nodes simultaneously. `N x 1` starts as soon
as any node frees and tolerates node failure, but fair-share degrades progressively so the last
allocations can wait far longer than the first.

Ask Slurm instead of guessing:

```bash
torc slurm plan-allocations --account <acct> workflow.yaml
torc slurm plan-allocations --account <acct> --skip-test-only workflow.yaml   # heuristics only
torc slurm plan-allocations --account <acct> --offline workflow.yaml           # no cluster queries
```

This probes with `sbatch --test-only` for both shapes and reports estimated start and completion.
Read the raw estimates, not only the recommendation: the many-small start time is for the _first_
allocation, and the tool approximates later degradation as `first_wait * min(N, 10) + walltime`.
Also compare `max_parallelism` against `ideal_nodes` in the analysis; a narrow DAG cannot use the
nodes the arithmetic suggests.

Apply the answer:

```bash
torc slurm generate --account <acct> --single-allocation workflow.yaml   # 1 x N
torc slurm generate --account <acct> workflow.yaml                       # N x 1 (default)
```

## Long chains that exceed one walltime

When sequential work exceeds any single allocation, chain allocations instead of holding a
login-node process. Set `serialize_allocations: true` on the scheduler and Torc submits every
allocation under one Slurm job name with `--dependency=singleton`, so Slurm runs them strictly one
at a time.

```yaml
slurm_schedulers:
  - name: chain
    account: my_account
    walltime: "12:00:00"
    nodes: 1
    serialize_allocations: true
```

```bash
torc slurm schedule-nodes <workflow_id> -n 167
```

Size the chain from the same arithmetic: `ceil(total_jobs / floor(walltime / runtime))`, for example
`ceil(500 / floor(12 / 4)) = 167`. Round up. Over-submitting is cheap, because the finishing worker
cancels allocations still queued for the workflow once no runnable jobs remain. Note that
`--job-prefix` is rejected for a serialized scheduler, since chaining needs one fixed job name.

## Multi-node jobs

Set `num_nodes > 1` only when a single job genuinely spans nodes (MPI). Such a job reserves whole
nodes exclusively and never shares, so a 4-node job on a 4-node allocation leaves nothing for
anything else.

Do not use `num_nodes` to spread many single-node jobs across an allocation. That is a scheduler
setting: request `nodes: 4` on the Slurm scheduler and leave `num_nodes: 1` on the jobs, and Torc
places them across the allocation, sharing nodes where the resources fit.

## When to abandon resource-aware packing

Resource-aware claiming is the right default. Prefer `--max-parallel-jobs` only when jobs are
homogeneous, individually negligible against node capacity, or I/O-bound and mostly idle, or when
the payload self-limits through cgroups or its own thread pool.

Mixed strategies are legitimate: run a resource-aware runner for large jobs and a queue-depth runner
for a swarm of small ones against the same workflow, and the ready queue serves both.

Claim order is `priority DESC`, then GPUs, runtime, memory, CPUs descending, then job ID. Raising
`priority` on the largest jobs helps them claim space before small jobs fragment a node, which is
the cheapest fix for a workflow where big jobs keep starving.

## Measure, then correct

Optimization without measurement is guessing. Every number above depends on declared requirements
matching reality, so enable monitoring and check.

```yaml
resource_monitor:
  sample_interval_seconds: 10
  jobs:
    enabled: true
    granularity: summary
```

```bash
torc workflows check-resources <id> --all              # declared versus observed, all jobs
torc workflows check-resources <id> --include-failed   # include failed and terminated
torc workflows correct-resources <id> --dry-run        # proposed adjustments
torc workflows correct-resources <id>                  # apply (up and down, 1.2x default)
torc slurm usage <id>                                  # node-hours and CPU-hours consumed
torc slurm stats <id>                                  # per-job sacct stats from the database
```

`correct-resources` both raises violations and lowers over-allocations, which is exactly the lever
that increases `concurrent_jobs_per_node`. Pass `--no-downsize` when the workload will grow.

The iteration loop that works: run a representative subset with monitoring on, read
`check-resources --all`, apply `correct-resources`, regenerate schedulers, and compare the new
allocation count and `torc slurm usage` against the previous run.

If Torc MCP tools are connected, two of them do this better than the CLI can.
`analyze_resource_usage` reports the usage _distribution_ per requirement (min/max/mean/median of
peak memory, CPU, and exec time), which is what reveals one requirement hiding two workloads, and
`regroup_job_resources` creates the new requirement groups and reassigns jobs with a `dry_run`
preview. The CLI path above adjusts and separates existing requirements but cannot synthesize new
groups from observed clusters. See `mcp-tools.md`.

## Verification commands

```bash
# See the plan the arithmetic produces before spending anything
torc slurm generate --account <acct> workflow.yaml -o gen.yaml
grep -E 'name:|walltime|nodes:|mem:|num_allocations' gen.yaml

# Compare grouping strategies
torc slurm generate --account <acct> --group-by partition workflow.yaml --dry-run
torc slurm generate --account <acct> --group-by resource-requirements workflow.yaml --dry-run

# Ask Slurm which shape starts sooner
torc slurm plan-allocations --account <acct> workflow.yaml

# Confirm the graph is wide enough to use the nodes
torc workflows execution-plan workflow.yaml

# Confirm co-tenancy and placement during a run
torc jobs running <id>

# Explain ready jobs that will not start on live allocations
torc workflows diagnose <id>
```

`torc workflows diagnose` is the specific check for packing that degrades with allocation age: a
ready job whose `runtime` exceeds an allocation's remaining walltime will not be started, and this
reports it from persisted state with no Slurm access.
