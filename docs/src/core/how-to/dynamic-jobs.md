# Dynamic Jobs (Orchestrator Continuation)

Most workflows are a static DAG declared up front. Some workflows instead have a **data-dependent
iteration count** that is only known at runtime — for example an iterative solver that keeps
refining until it converges. Torc supports this with a single operation: **`spawn_jobs`**.

A lightweight _orchestrator_ job inspects the previous iteration's results and then calls
`spawn_jobs` to add the next iteration's jobs (including a continuation of itself) — all blocked on
this orchestrator job. The orchestrator then exits 0; the torc runner completes it on exit, and the
normal unblock cascade promotes the spawned jobs. When the orchestrator decides it is done, it
simply exits (optionally writing a final state) and the workflow finishes naturally.

This is the recommended pattern for feedback loops such as ReEDS ↔ PRAS.

## How it works

Each `spawn_jobs` call runs as one database transaction that:

1. Inserts the requested jobs as **blocked**, with an implicit dependency edge to the calling
   orchestrator. (You do not need to list the orchestrator in `depends_on` — torc adds the edge for
   you.) Explicit `depends_on` references to existing jobs or to siblings in the same batch are
   still honored.
2. Persists an opaque JSON `state` payload for the lineage.

The orchestrator then exits 0. The torc runner observes the exit and marks the orchestrator
`completed` through the normal path — which fires the unblock cascade, promoting the spawned jobs to
`ready`. There is no double completion: the orchestrator never completes itself.

Because the inserts and state write are one transaction, retries are safe: a re-run with the same
spawn-job names is detected and is an idempotent no-op (no duplicate jobs, no double-counted
iterations).

## Lineage

A **lineage** is one independent run-sequence. Many lineages can run concurrently in the same
workflow — e.g. 10 ReEDS cases iterating independently — each with its own state and its own
iteration counter. Pass a stable, unique `lineage` string on every call for a given sequence. Torc
injects `TORC_ORCHESTRATOR_LINEAGE_ID` into every spawned job's environment so a continuation
automatically keeps using the same lineage.

## Limiting iterations

Set a per-lineage cap in the workflow spec so a non-converging loop cannot run forever:

```yaml
name: reeds_pras
dynamic_jobs:
  max_iterations: 10        # max spawn_jobs calls per lineage
jobs:
  - name: orchestrator_case1
    command: python3 scripts/orchestrator.py case1
  # ... one seed orchestrator job per concurrent case ...
```

The cap is **per lineage**, so `max_iterations: 10` means _each_ case iterates at most 10 times — no
arithmetic across cases. A call that would exceed the cap is rejected (HTTP 422); nothing is
persisted, the orchestrator stays Running, and you can re-run it after fixing the issue. When the
field is omitted, a generous server default applies.

## The orchestrator script

The orchestrator is cheap (it only inspects prior results and issues one API call), so it never
holds a compute slot while the long jobs run. A full working example is at
`examples/scripts/dynamic_orchestrator.py` (Python, using the torc OpenAPI client); a complete Slurm
workflow that uses it is at `examples/yaml/dynamic_orchestrator_slurm.yaml`.

The request body sent to `POST /jobs/{id}/spawn_jobs` is:

```jsonc
{
  "lineage": "case1",
  "jobs": [
    { "name": "reeds_case1_i01", "command": "...", "resource_requirements": "reeds_rr" },
    { "name": "pras_case1_i01",  "command": "...", "resource_requirements": "pras_rr",
      "depends_on": ["reeds_case1_i01"] },
    { "name": "orch_case1_g1",   "command": "...", "resource_requirements": "orch_rr",
      "depends_on": ["reeds_case1_i01", "pras_case1_i01"] }
  ],
  "state": { "gen": 1 }
}
```

`resource_requirements` must name a record already declared in the workflow. `depends_on` may
reference existing jobs or sibling jobs created in the same call (resolved by name); the batch must
be acyclic. Each spawned job is also automatically blocked on the calling orchestrator — you do not
need to list it in `depends_on`.

## Convergence and completion

Convergence is just "spawn nothing." On the converging generation the orchestrator calls
`spawn_jobs` with an empty `jobs` array (optionally with a final `state` payload — torc stores it as
the lineage's `__final` record), then exits. The runner completes the orchestrator on exit, and
because no more jobs were spawned the workflow has no incomplete jobs and completes naturally.

## Scheduling on Slurm

A spawned job has the same lifecycle as a normal job: it sits `ready` until a compute node with
matching resources claims it. If the workflow was submitted with allocations sized only for the seed
orchestrator, a spawned `reeds`/`pras` job may have no fitting node. This is the same situation as a
failure-handler retry that needs a larger node, and the solution is the same: run

```bash
torc watch --auto-schedule <workflow_id>
```

alongside the workflow. `torc watch` periodically calls `regenerate_and_submit`, which mints a Slurm
scheduler sized for the currently-pending RR shapes and submits a new allocation. No
dynamic-jobs-specific operator step is required.

## Notes and limits

- A failed dependency cancels a blocked continuation by default (fail-stop). Set
  `cancel_on_blocking_job_failure: false` on the continuation if the orchestrator must instead
  inspect partial failures and decide.
- Keep `state` small (counters, tolerances, file paths). Large artifacts belong on the shared
  filesystem; pass a path, not the bytes.

See `docs/plans/dynamic-jobs-design.md` for the full design rationale.
