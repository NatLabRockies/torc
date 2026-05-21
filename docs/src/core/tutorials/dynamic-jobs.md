# Dynamic Jobs: Iterative Workflows with `spawn_jobs`

This tutorial teaches you how to build a torc workflow whose **iteration count is decided at
runtime** — for example a numerical refinement that keeps going until it converges. The shape can't
be expressed as a static DAG, so torc gives you one primitive to extend the workflow as it runs:
**`spawn_jobs`**.

## Learning Objectives

By the end of this tutorial, you will:

- Recognize when a static DAG can't express your workflow
- Write a lightweight orchestrator job that adds the next iteration at runtime
- Build a converging loop and watch it stop on its own
- Run multiple independent iterations concurrently in one workflow
- Know how to cap a runaway loop

## Prerequisites

- Completed [Tutorial 2: Diamond Workflow](./diamond.md)
- Torc server running (see [Quick Start (Local)](../../getting-started/quick-start-local.md))
- The Python client installed: `pip install torc_client`
- `jq` and `awk` available on `PATH` (used by the demo worker)
- Roughly 15 minutes

## The Problem a Static DAG Can't Express

Most torc workflows are a fixed DAG declared up front: "run these N jobs in this order." That works
when the work is known.

Suppose you have a simulation that:

- Refines its estimate each iteration
- Converges when the change between iterations drops below a tolerance
- Might take 3 iterations or 30 — it depends on the data

Three iterations of (worker → check → worker) you could write by hand. Thirty you can't, because you
don't know it's thirty until you've already started.

### A Naive Workaround That Doesn't Work

**Pre-allocate the maximum and cancel the rest.** Declare 30 worker jobs in a chain and have the
converging iteration cancel the remaining 27.

This is wasteful (the unused jobs still occupy slots in the dependency graph), fragile (cancellation
has to live in your scripts), and pushes discovery into user code. For 10 concurrent runs of this
pattern you'd be staring at 300 jobs that mostly won't run, and a debugging story that gets worse
with every retry.

There's a better primitive.

## The Spawn Pattern

A lightweight **orchestrator** job does three things:

1. Inspects the previous iteration's output (cheap — just reads a file or a small `user_data`
   record)
2. Calls `spawn_jobs` to add the next worker plus a **continuation of itself**, both blocked on this
   orchestrator
3. Exits 0

The torc runner completes the orchestrator on exit (the normal path). The spawned jobs were inserted
`blocked` with an implicit dependency edge on the calling orchestrator, so the unblock cascade
promotes them as soon as the orchestrator becomes terminal. The next orchestrator generation then
does the same thing. **Convergence is "spawn nothing."** When the orchestrator decides it's done, it
calls `spawn_jobs` with `jobs=[]` (optionally with a final state payload) and exits. With no further
jobs in the queue, the workflow finishes naturally.

### What `spawn_jobs` Does in One Call

In one database transaction, the server:

1. Inserts each requested job as `Blocked`
2. Adds an implicit `job_depends_on` edge to the calling orchestrator (you don't list it yourself)
3. Resolves explicit `depends_on` against existing jobs and against siblings in the same batch
4. Persists an opaque JSON `state` payload as an immutable per-generation record

If anything in the batch is invalid — unknown `resource_requirements`, a dependency cycle, the cap
exceeded — the whole transaction aborts and nothing is persisted.

## Step 1: Write the Worker

The "real work" of each iteration. For this tutorial we'll simulate convergence: each iteration's
metric is half the previous one, so it crosses our tolerance after 7 generations.

Save as `worker.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail
lineage="$1"
gen="$2"
work_dir="./work/$lineage"
mkdir -p "$work_dir"

prev_file="$work_dir/worker_i$(printf '%02d' $((10#$gen - 1))).json"
prior_metric=$(jq -r '.metric' "$prev_file" 2>/dev/null || echo "1.0")
new_metric=$(awk "BEGIN { print $prior_metric / 2 }")
out_file="$work_dir/worker_i$(printf '%02d' "$gen").json"
printf '{"metric": %s}\n' "$new_metric" > "$out_file"
echo "iter $gen: $prior_metric -> $new_metric" >&2
```

## Step 2: Write the Orchestrator

The orchestrator reads the prior worker's output, decides whether to continue, and either spawns the
next iteration or stops.

Save as `orchestrator.py`:

```python
#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
from torc import make_api
from torc.openapi_client import SpawnJobModel, SpawnJobsRequest

CONVERGENCE_TOLERANCE = 0.01

# Lineage: from env on a spawned continuation, from argv on the seed.
lineage = os.environ.get("TORC_ORCHESTRATOR_LINEAGE_ID") or sys.argv[1]

api = make_api(os.environ["TORC_API_URL"])
workflow_id = int(os.environ["TORC_WORKFLOW_ID"])
job_id = int(os.environ["TORC_JOB_ID"])

# Discover current generation from torc's append-only lineage records.
ud = api.list_user_data(workflow_id, limit=10000)
prefix = f"__torc_lineage__{lineage}__g"
current_gen = max(
    (int(u.name[len(prefix):]) for u in (ud.items or []) if u.name.startswith(prefix)),
    default=0,
)
next_gen = current_gen + 1

# Read prior worker's metric (None on the seed generation).
prior_file = Path(f"./work/{lineage}/worker_i{current_gen:02d}.json")
prior_metric = json.loads(prior_file.read_text())["metric"] if prior_file.exists() else None
print(f"[orch {lineage}] current_gen={current_gen} prior_metric={prior_metric}", file=sys.stderr)

# Convergence: spawn nothing, write a final state record, exit.
if prior_metric is not None and prior_metric < CONVERGENCE_TOLERANCE:
    api.spawn_jobs(job_id, SpawnJobsRequest(
        lineage=lineage, jobs=[],
        state={"converged": True, "final_metric": prior_metric, "iterations": current_gen},
    ))
    print(f"[orch {lineage}] converged at gen={current_gen}", file=sys.stderr)
    sys.exit(0)

# Otherwise spawn the next worker + the next orchestrator continuation.
worker = f"worker_{lineage}_i{next_gen:02d}"
cont = f"orch_{lineage}_g{next_gen:02d}"
api.spawn_jobs(job_id, SpawnJobsRequest(
    lineage=lineage,
    jobs=[
        SpawnJobModel(
            name=worker,
            command=f"bash {os.path.abspath('worker.sh')} {lineage} {next_gen}",
            resource_requirements="worker_rr",
        ),
        SpawnJobModel(
            name=cont,
            command=f"python3 {os.path.abspath('orchestrator.py')}",
            resource_requirements="orch_rr",
            depends_on=[worker],
            cancel_on_blocking_job_failure=False,
        ),
    ],
    state={"generation": next_gen, "prior_metric": prior_metric},
))
print(f"[orch {lineage}] spawned gen={next_gen}: {worker} -> {cont}", file=sys.stderr)
```

Notice what the orchestrator does **not** do:

- It does **not** call `complete_job` on itself. The runner does that when the script exits.
- It does **not** list itself in any spawned job's `depends_on`. The server adds that edge.

## Step 3: Write the Workflow Spec

Save as `iterative.yaml`:

```yaml
name: iterative_refinement
description: Iterative loop that converges via spawn_jobs

dynamic_jobs:
  max_iterations: 20   # safety cap; 7 are expected

resource_requirements:
  - name: worker_rr
    num_cpus: 1
    memory: 256m
  - name: orch_rr
    num_cpus: 1
    memory: 128m

jobs:
  - name: orch_caseA_g00
    command: python3 orchestrator.py caseA
    resource_requirements: orch_rr
```

The workflow declares **one** job — the seed orchestrator. Every worker and every continuation is
added at runtime by `spawn_jobs`.

## Step 4: Run It

```bash
torc run iterative.yaml
```

The seed orchestrator runs, sees no prior worker, and spawns `worker_caseA_i01` plus
`orch_caseA_g01`. The runner completes the seed; the unblock cascade promotes the worker; the worker
writes its metric; the next orchestrator unblocks, reads the metric, and spawns the next pair. This
repeats until the metric drops below `0.01` (around generation 7).

## Step 5: Watch It Happen

While the workflow runs, you can list jobs and see new ones appear:

```bash
torc jobs list <workflow_id>
```

```
NAME                STATUS      ORIGIN
orch_caseA_g00      completed   (NULL)        # declared at workflow creation
worker_caseA_i01    completed   spawn         # added at runtime
orch_caseA_g01      completed   spawn
worker_caseA_i02    completed   spawn
orch_caseA_g02      completed   spawn
...
worker_caseA_i07    completed   spawn         # metric < 0.01 here
orch_caseA_g07      completed   spawn         # this generation spawned nothing
```

Things to notice:

- The seed orchestrator has `origin = NULL` (declared at workflow creation). Every other job has
  `origin = 'spawn'`.
- The orchestrators in the middle never produced output of their own — they just inspected the prior
  metric and re-spawned.
- The last orchestrator generation called `spawn_jobs` with `jobs=[]` and the workflow finished
  naturally with no leftover Ready or Blocked jobs.

To inspect the final state record:

```bash
torc user-data list <workflow_id> | grep __torc_lineage__caseA__final
```

It holds `{"converged": true, "final_metric": ..., "iterations": 7}` — the payload your orchestrator
sent on the converging call.

## Step 6: Run Multiple Concurrent Lineages

What if you want 10 independent runs of this same loop? Add 10 seed orchestrators:

```yaml
jobs:
  - name: orch_caseA_g00
    command: python3 orchestrator.py caseA
    resource_requirements: orch_rr
  - name: orch_caseB_g00
    command: python3 orchestrator.py caseB
    resource_requirements: orch_rr
  # ... add caseC ... caseJ ...
```

Each runs its own independent lineage. Their per-lineage state records (`__torc_lineage__caseA__*`,
`__torc_lineage__caseB__*`, …) and iteration counters do not interact. Torc's resource packer
interleaves all of their workers across compute nodes; the orchestrators are cheap and don't crowd
anything.

The `max_iterations: 20` cap is **per lineage**, not per workflow. Case A iterating 20 times has no
effect on Case B's allowance.

## Safety: the Iteration Cap

`dynamic_jobs.max_iterations` is a runaway guard. The first attempted iteration that would push a
lineage past the cap is rejected with HTTP 422:

```
Per-lineage iteration cap reached for lineage 'caseA':
attempted iteration 21 exceeds dynamic_jobs.max_iterations=20 (current spawn_count=20)
```

When the cap rejects a call, nothing is persisted, the orchestrator stays `Running`, and you can
investigate or raise the cap on the next run. Omit the field to use the server default (1000).

The server also validates the cap at workflow-creation time: `max_iterations: 0` or negative values
are rejected up front with 422 — they would silently disable spawning otherwise.

## What About Slurm?

A spawned job is a normal job from Slurm's perspective: it sits `Ready` until a compute node with
matching resources claims it. If the workflow was submitted with an allocation sized only for the
seed orchestrators, a spawned worker may have no fitting node.

The fix is the same as for failure-handler retries — run `torc watch` alongside the workflow with
`--auto-schedule`:

```bash
torc watch --auto-schedule <workflow_id>
```

The watch loop counts ready jobs with `origin IS NOT NULL` (spawned + retried) and asks
`regenerate_and_submit` to mint a Slurm allocation sized for the currently-pending resource shapes
when needed.

## Reference

### Request Body

```jsonc
{
  "lineage": "caseA",
  "jobs": [
    {
      "name": "worker_caseA_i01",
      "command": "bash worker.sh caseA 1",
      "resource_requirements": "worker_rr"
    },
    {
      "name": "orch_caseA_g01",
      "command": "python3 orchestrator.py",
      "resource_requirements": "orch_rr",
      "depends_on": ["worker_caseA_i01"],
      "cancel_on_blocking_job_failure": false
    }
  ],
  "state": { "generation": 1 }
}
```

### Rules

| Rule                                                                                   | Why                                                                                                                                          |
| -------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Every spawned job is auto-blocked on the calling orchestrator                          | Without this, children would race the runner's completion of the caller; you'd have to coordinate it yourself                                |
| `depends_on` may reference existing jobs or siblings in the same batch                 | Lets one spawn call describe a small DAG (worker → next-orchestrator)                                                                        |
| The sibling subgraph must be acyclic                                                   | Rejected 422; the same dependency rule as static jobs                                                                                        |
| `resource_requirements` must name a record declared at workflow creation               | Falls back to the workflow's `default` RR if omitted, same as `create_job`                                                                   |
| Replays (same names already exist) are idempotent no-ops                               | Lets you safely re-run an orchestrator that crashed after committing its spawn — no duplicate jobs, no double-counted iteration              |
| Convergence is `jobs=[]`, optionally with a final `state` payload                      | No separate "stop" endpoint; the workflow finishes naturally once nothing else is pending                                                    |
| `job.origin` is `'spawn'` for spawned jobs, `'retry'` for retries, `NULL` for declared | Lets `torc watch --auto-schedule` recognize jobs that need an unplanned Slurm allocation; lets reports separate static plan from dynamic add |

### What Stays the Same as Static Jobs

- Status lifecycle (`Blocked → Ready → Running → Completed/Failed`)
- Resource claiming and packing
- Restart and reset semantics (a row created by `spawn_jobs` resets the same way a declared job
  does)
- File and `user_data` dependencies
- Failure handlers and `cancel_on_blocking_job_failure`

## Next Steps

- The energy-modeling example at `examples/yaml/dynamic_orchestrator_slurm.yaml` plus
  `examples/scripts/dynamic_orchestrator.py` is the same pattern applied to a ReEDS↔PRAS feedback
  loop with realistic resource shapes (ReEDS at 8 CPU / 10 GB, PRAS at 1 CPU / 120 GB).
