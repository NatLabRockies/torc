# Chained Allocations

Some workflows need more wall time than any single Slurm allocation provides. A chain of 500
sequential jobs at 2-4 hours each is weeks of serial work, but partitions typically cap allocations
at hours. You need one allocation to run as many jobs as fit, exit, and the next to pick up where it
left off.

Set `serialize_allocations` on a Slurm scheduler and torc submits every allocation for that
scheduler under one shared Slurm job name with `--dependency=singleton`. Slurm then runs them
strictly one at a time. Submit them all up front and they chain themselves, with no long-running
process on the login node.

## Configuring a Scheduler

```yaml
slurm_schedulers:
  - name: chain
    account: my_account
    walltime: "12:00:00"
    nodes: 1
    serialize_allocations: true

resource_requirements:
  - name: serial
    num_cpus: 104
    memory: "200g"
    runtime: "PT4H"
```

Or on an existing scheduler:

```bash
torc slurm create <workflow_id> -n chain -a my_account -W 12:00:00 --serialize-allocations
torc slurm update <scheduler_id> --serialize-allocations true
```

Then submit the whole chain at once:

```bash
torc slurm schedule-nodes <workflow_id> -n 167
```

All 167 allocations enter the queue immediately. Slurm starts one, holds the rest, and releases the
next each time the current one ends.

## How Many Allocations to Submit

Divide the total work by what one allocation can absorb. A worker claims jobs until its remaining
wall time can no longer fit the next one, so with a 12-hour walltime and a declared `runtime` of
`PT4H`, each allocation completes at least three jobs:

```
allocations = ceil(total_jobs / floor(walltime / runtime))
            = ceil(500 / floor(12 / 4)) = 167
```

Round up. Over-submitting is cheap: once the workflow has no runnable jobs left, the finishing
worker cancels every allocation still queued for the workflow, so the surplus never starts.

## Why the Chain Beats Scheduling on Shutdown

A worker could submit its own replacement as it exits, but each replacement would enter the queue
with no accrued age and pay full queue wait — 167 times over. A chained allocation is queued from
the start and accrues priority while its predecessor runs, so it is typically ready to start the
moment the slot frees.

## Wall Time and Job Runtime

The server only hands a worker jobs whose declared `runtime` fits in the allocation's remaining wall
time. This is what makes the handoff clean: an allocation stops claiming once the next job no longer
fits, idles briefly, and exits, releasing the slot early rather than sitting idle until walltime.

Declare `runtime` at or above the worst case you expect. If a job overruns its declared runtime it
is terminated when the walltime expires, and jobs downstream of it are left blocked with nothing to
unblock them — which ends the chain, since the remaining queued allocations have no runnable work.

## Scope of the Chain

The shared job name is derived from the workflow ID and the scheduler ID, so:

- Two schedulers in one workflow chain independently.
- The same workflow submitted twice chains independently per scheduler.
- Allocations added later — by a second `schedule-nodes` call, or by a `schedule_nodes` action
  firing from a compute node — join the existing chain rather than running alongside it.

Slurm scopes `singleton` to a job name **per user**, so the name is prefixed with `torc-` to keep
the chain from serializing against your unrelated Slurm jobs.

## Interaction with `extra`

`extra` is emitted after torc's own `#SBATCH` directives, so a `--dependency` set there overrides
the generated `--dependency=singleton` and breaks the chain. Use `extra` for unrelated flags
(`--reservation`, `--constraint`) when serializing allocations.
