# Design: Orchestrator Continuation via `spawn_jobs`

- **Status:** Implemented.
- **Related artifacts:** user docs at `docs/src/core/how-to/dynamic-jobs.md`; example workflow at
  `examples/yaml/dynamic_orchestrator_slurm.yaml` and orchestrator at
  `examples/scripts/dynamic_orchestrator.py`; integration tests in `tests/test_dynamic_jobs.rs`.

## 1. Problem

Some workflows have a **data-dependent iteration count** unknown at workflow-creation time. The
motivating case is a ReEDS/PRAS feedback loop:

```
reeds -> pras -> reeds -> pras -> ... (3-10 cycles) -> converged, stop
```

ReEDS decides convergence only after inspecting the previous PRAS results. The iteration count
cannot be expressed in a static DAG.

Today torc only allows job creation while a workflow is `Uninitialized` (`src/server/api/jobs.rs`).
After `initialize_jobs`, the job set is fixed and there is no incremental dependency resolution.
Full "dynamic jobs" — re-resolving the file/user_data/ `depends_on` graph against a live workflow —
is expensive. This proposal scopes the minimum capability that solves the problem.

## 2. Rejected alternatives

The design was reached by eliminating weaker options:

- **Pre-allocate + cancel-on-convergence.** Create the maximum chain up front (e.g. 10 cases × 5
  iterations × 2 jobs = 100 jobs) and have a converging job cancel the rest of its chain. Wasteful,
  fragile, pushes discovery and cancellation logic into user scripts. Worked as a temporary
  workaround but not the right primitive.
- **Long-lived orchestrator that polls or subscribes to SSE.** Conceptually simple but the
  orchestrator process holds a compute slot for the whole run. Decisive problem: one run is 5–10
  iterations × 12–24h ReEDS, i.e. 60–240h, against a 48h Slurm walltime. The orchestrator process is
  killed mid-loop on _every_ run and must re-attach to in-flight jobs. Resume complexity does not
  disappear — it relocates into the user's process and gets harder.
- **Orchestrator that completes itself atomically when adding new jobs.** Right structure (each
  generation is a fresh, short-lived orchestrator job), wrong primitive: the torc runner already
  owns the completion path. When the orchestrator subprocess exits, the runner calls `complete_job`
  on it — so a self-completing orchestrator produces a double-completion error and fights the
  runner.

**Chosen:** the orchestrator adds child jobs blocked on itself via `POST /jobs/{id}/spawn_jobs` and
exits. The runner completes the orchestrator on exit through the normal path, and the unblock
cascade promotes the spawned jobs. No new completion mechanism, no double-completion risk.

## 3. Goals / Non-goals

**Goals**

- A single transactional primitive (`spawn_jobs`) that, in one DB transaction: inserts a batch of
  new jobs into an initialized workflow (each blocked on the calling job), wires their dependency
  edges, and persists an opaque per-lineage state payload.
- Carry per-iteration state across orchestrator generations with no user-side store, keying, or
  replay-race handling.
- Keep dynamically added jobs first-class for resource packing, restart, targeted reset/revert, and
  completion detection.

**Non-goals**

- No incremental re-resolution of file/user_data dependency graphs.
- No mid-workflow re-run of `initialize_jobs`.
- No general post-hoc graph mutation by arbitrary clients — only the constrained "add jobs blocked
  on the caller" operation described here.
- Torc does not decide convergence or define "what state matters" — that is irreducible domain logic
  and stays with the user.

## 4. The primitive: `spawn_jobs`

```jsonc
POST /jobs/{id}/spawn_jobs
{
  "lineage": "case3",
  "jobs": [
    {
      "name": "reeds_case3_i04",
      "command": "bash scripts/reeds.sh case3 4",
      "resource_requirements": "reeds_rr",     // must already exist
      "priority": 1
    },
    {
      "name": "pras_case3_i04",
      "command": "bash scripts/pras.sh case3 4",
      "resource_requirements": "pras_rr",
      "priority": 10,
      "depends_on": ["reeds_case3_i04"]         // intra-batch reference by name
    },
    {
      "name": "orch_case3_g05",
      "command": "python3 scripts/orchestrator.py",
      "resource_requirements": "orch_rr",
      "depends_on": ["reeds_case3_i04", "pras_case3_i04"]   // fan-in continuation
    }
  ],
  "state": { "gen": 5, "tol": 1e-3 }
}
```

Semantics:

- One transaction (`BEGIN IMMEDIATE`): every spawned job row inserted `blocked`; every
  `job_depends_on` edge inserted; the `state` payload persisted (Section 6). All-or-nothing.
- **Every spawned job is auto-blocked on the calling job.** The server inserts an implicit
  `job_depends_on` edge from each spawned job to the caller, in addition to any explicit
  `depends_on`. Users don't list the orchestrator in `depends_on`.
- `depends_on` may reference **existing** jobs _or_ **siblings created in the same batch**, resolved
  by name within the transaction. The batch (over siblings) must form a DAG; cycles are
  rejected 422.
- `resource_requirements` must name an RR record that already exists in the workflow. No inline RR
  in v1.
- Rejected 422 if the call would exceed the per-lineage iteration cap (Section 7); nothing is
  persisted.

The orchestrator script then exits 0. The runner observes the exit and completes the orchestrator
through the standard completion path. The normal unblock cascade (`batch_unblock_jobs_tx`,
`src/server/http_server/lifecycle_support.rs`) then promotes the spawned jobs as their dependencies
— including the just-completed caller — become terminal.

## 5. Continuation pattern

```
orchestrator_g(k) runs (cheap; inspects prior reeds/pras outputs on shared FS)
  converged?  -> POST /spawn_jobs  with jobs=[] (and optional final state) -> exit 0
  else        -> POST /spawn_jobs  with { reeds_i(k),
                                          pras_i(k)   depends_on=[reeds_i(k)],
                                          orchestrator_g(k+1) depends_on=[reeds_i(k), pras_i(k)],
                                          state: {...} }
                 -> exit 0
runner completes orchestrator_g(k) on exit
reeds_i(k) unblocks -> runs (8 CPU / 10 GB) -> pras_i(k) unblocks -> runs (120 GB)
both terminal -> orchestrator_g(k+1) unblocks and runs -> repeat
```

Convergence is "spawn nothing." When the converging generation completes (via the runner), no
further jobs are pending for that lineage and the workflow finishes naturally for it. The
orchestrator declares a tiny `orchestrator_rr` (it only inspects files and issues one API call), so
N concurrent runs cost N small, short-lived slots — never a long-held one.

## 6. Continuation-state channel (user_data-backed, append-only)

The annoying, error-prone part of this model is carrying per-iteration state across a process that
exits. Torc automates the **mechanical** part; the user supplies only the dict.

- **Same-transaction persistence.** The `state` payload is written in the same transaction as the
  job inserts. A re-run of the orchestrator (after restart) finds its spawned jobs already exist via
  the name-existence check (Section 8), is detected as a replay, and appends nothing —
  deterministic.
- **Backing store: `user_data`.** Reuse the existing table (workflow-scoped JSON, `is_ephemeral`
  flag, FK-cascade cleanup). No new table or migration. State records are written **ephemeral**.
- **Append-only, one record per generation**, keyed by name as
  `__torc_lineage__<lineage>__g<NNNNNN>`, holding `{generation, spawn_count, state}`. History is
  kept deliberately: convergence logic often needs the trend across iterations, and immutable
  snapshots are robust against any future replay ambiguity.
- **Convergence record.** A no-spawn call carrying `state` writes (or upserts) a single
  `__torc_lineage__<lineage>__final` record with `{generation, spawn_count, final: true, state}`.
- **Spawn counter is derived**, not stored separately: it is the highest generation present.
- **Delivery.** Torc injects `TORC_ORCHESTRATOR_LINEAGE_ID` into every spawned job's environment
  (via the spawned job's `env` column, which the runner already injects into the subprocess — no
  runner change needed). On a continuation, the orchestrator reads the lineage from this env var; on
  the seed, it reads it from `argv`.

**Contract / constraints:**

- Incoming state is **read-only**; the next state is produced only by the next `spawn_jobs` call.
- **Small JSON only**: counters, tolerances, metric history, file paths. Large artifacts stay as
  files on the shared FS; the payload carries a path, not the bytes.
- What stays with the user: deciding convergence and choosing the dict contents. Irreducible domain
  logic.

## 7. Iteration cap (`dynamic_jobs` spec section)

There is no separate enable flag — calling `spawn_jobs` is itself the explicit opt-in. A job that
never calls it behaves exactly as today.

The runaway guard is a cap on the **number of `spawn_jobs` calls per orchestrator lineage**:

```yaml
dynamic_jobs:
  max_iterations: 6        # per orchestrator lineage
```

This is chosen over a cap on total dynamically created jobs because it maps directly to the quantity
the user reasons about: "no ReEDS run iterates more than N times."

### Config vs. enforcement counter

|             | `dynamic_jobs.max_iterations` | spawn-iteration counter                     |
| ----------- | ----------------------------- | ------------------------------------------- |
| Author      | user, once                    | server, every spawn                         |
| Mutability  | read-only at runtime          | derived from `user_data` generation records |
| Cardinality | one per workflow              | **one per lineage**                         |

The counter is per lineage, not per workflow, so a 10-run workflow with `max_iterations: 6` means
each run independently iterates ≤6 times. A `WorkflowModel` scalar could not represent this — it
would smear all lineages into one number. The counter therefore lives implicitly in the per-lineage
state record namespace (Section 6) and is computed as the max generation present in the same
transaction as the spawn.

Enforcement: in the spawn transaction, derive the lineage's current generation; if advancing it
would exceed `max_iterations`, reject 422 and persist nothing. (Server default when unset: 1000.)

## 8. Idempotency & failure policy

- **Idempotent on job name within a workflow.** Before inserting, the server checks whether all
  requested spawn-job names already exist; if so, it's a replay — skip all writes and return the
  existing job IDs. Partial overlap (some names exist, some don't) is ambiguous and rejected 422.
  This matches the existing in-batch name validation `bulk_create` uses and avoids a destructive
  migration to add a DB unique index on `(workflow_id, name)`.
- **Sub-job failure.** If a spawned worker fails (nonzero return code → result row), the
  `cancel_on_blocking_job_failure` cascade can cancel the blocked continuation (fail-stop). That is
  a sensible default for some pipelines. If the orchestrator must instead see partial failures and
  adapt, set `cancel_on_blocking_job_failure: false` on the continuation so it runs and inspects
  results. Per-continuation choice.

## 9. Restart & recovery

Because each `spawn_jobs` call is one transaction and spawned successors carry real `job_depends_on`
edges, restart and recovery follow torc's existing row-based semantics with no dynamic-job-specific
code:

- **Job died mid-execution:** the row-based reset returns it to `ready`/`blocked`; replays normally.
- **Completed orchestrator after restart:** stays `completed`, skipped — its spawned rows and state
  record were committed atomically, so the loop continues.
- **Orchestrator that ran but exited before commit:** the transaction did not commit, so nothing was
  inserted. On restart it re-runs and spawns cleanly.
- **`run_id` bump:** completed dynamic jobs stay terminal and are skipped exactly like static jobs.
  The append-only state records remain across restarts and the spawn counter is naturally preserved.
- **Targeted reset/revert of a mid-chain job:** traverses the explicit edges (including the implicit
  one back to the original orchestrator) and resets dynamic successors consistently.

## 10. Validation & observability

In-transaction validation:

- Workflow runnable; calling job exists.
- Each spawned job has a non-empty command and an existing `resource_requirements`.
- `depends_on` names resolve within the batch or to existing workflow jobs.
- The batch (siblings) is acyclic.
- This lineage's spawn-iteration counter is below `dynamic_jobs.max_iterations`.

Any failure aborts the whole transaction so nothing is persisted — the runner observes a recoverable
failed call without side effects.

Log lines use the parsing-friendly `workflow_id=<> job_id=<> spawned job_id=<>` format.

Spawned rows carry `origin = 'spawn'` on the `job` table. This is **not** for TUI/reports provenance
display — it's the marker that the Slurm auto-scheduling path (Section 11) uses to recognize jobs
that need unplanned allocations. Statically-declared jobs (the originally-planned workload,
anticipated by `on_jobs_ready` / `schedule_nodes` deferred actions) have `origin = NULL`;
failure-handler retries are tagged `origin = 'retry'` by the existing `retry_job` path. The watch
detector keys on `origin IS NOT NULL`.

## 11. Scheduling on Slurm — shared with failure-handler retries

A spawned job sits `ready` until a compute node with matching resources claims it. If the workflow
was submitted with allocations sized only for the seed orchestrators, a spawned `reeds`/`pras` job
may have no fitting node — the same situation as a failure-handler retry that needs a larger node
than the original plan provided for.

The solution is the same operator command for both cases: `torc watch --auto-schedule
<workflow_id>`.
The watch loop (`src/client/commands/watch.rs`) counts ready jobs that need unplanned allocations
and calls `regenerate_and_submit`, which mints a Slurm scheduler sized for the current pending RR
shapes and submits a new allocation.

The detector keys on **`job.origin IS NOT NULL`**:

| `origin` value | Meaning                                    | Anticipated by deferred actions?                          |
| -------------- | ------------------------------------------ | --------------------------------------------------------- |
| `NULL`         | declared at workflow creation              | yes — `on_jobs_ready` / `schedule_nodes` actions cover it |
| `'retry'`      | resurrected by failure-handler `retry_job` | no — auto-schedule must provision                         |
| `'spawn'`      | added at runtime by `spawn_jobs`           | no — auto-schedule must provision                         |

This is more correct than a fit-based detector (e.g. "ready jobs that no active compute node
satisfies"): a fit-based check would race the legitimate deferred actions that are about to fire for
declared jobs, causing over-provisioning. Provenance is the right marker because it directly
expresses "the original workflow plan does not account for this row."

## 12. Impact on the ReEDS workflow

Per case (lineage), the workflow seeds a single orchestrator job. Each orchestrator generation
inspects the prior PRAS metric on the shared filesystem and either spawns the next reeds + pras +
continuation (each automatically blocked on the orchestrator and chained via `depends_on` among
themselves) or, on convergence, spawns nothing. The pre-allocation workaround's 100 jobs collapse to
~2 jobs per actual iteration; resource packing is unchanged (each job still declares its real RR —
ReEDS 8 CPU / 10 GB, PRAS 120 GB), so torc packs ReEDS against PRAS across the Slurm nodes exactly
as before. PRAS priority 10 vs. ReEDS priority 1 keeps PRAS preferentially scheduled to unblock the
next ReEDS sooner.

## 13. Open questions

- Should `max_iterations` be mandatory (vs. a generous default applied when omitted)? Leaning toward
  a default so the safety cap always exists without forcing boilerplate.
- Lineage identity: derive from a stable job-name prefix, or accept an explicit `lineage` field on
  the request (current behavior)? The explicit field is less magical and is what v1 ships.
- Retain final-generation state as a non-ephemeral "result" `user_data` record for provenance?
  Currently `__final` is ephemeral like the generation records — a flag could opt into non-ephemeral
  retention. Nice-to-have, low cost.
