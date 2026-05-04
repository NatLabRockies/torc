# Torc Workflow Database

This database is the persistent store for **Torc**, a distributed workflow orchestration system.
Each workflow contains a graph of computational jobs with dependencies, resource requirements, and
per-execution results. Users analyze this data to understand failure patterns, find slow jobs,
right-size resource allocations, and audit which compute nodes ran what.

This is a **SQLite** database — use SQLite syntax (`json_extract`, `strftime`, `datetime`, etc.). Do
not use PostgreSQL- or DuckDB-specific functions.

## Schema Overview

| Table                   | What it represents                                                 |
| ----------------------- | ------------------------------------------------------------------ |
| `workflow`              | Top-level workflow definition + per-workflow run state             |
| `job`                   | Individual computational tasks within a workflow                   |
| `resource_requirements` | Named resource specs (CPU/memory/runtime) shared by groups of jobs |
| `result`                | One row per job execution attempt — the primary fact table         |
| `workflow_result`       | Pointer to the latest `result` row for each (workflow, job)        |
| `compute_node`          | A worker process (local or Slurm allocation) that executed jobs    |
| `file`                  | File artifacts that establish implicit job dependencies            |
| `user_data`             | User-defined JSON artifacts that establish implicit dependencies   |

The **`result`** table is the most important for analysis: it has actual measured CPU/memory/exec
time per execution, plus return codes. Most "what failed" or "what was slow" questions start there.

## Job Status (CRITICAL — stored as INTEGER, not text)

The `job.status` column is an integer. Always decode it with a `CASE` expression:

```sql
CASE job.status
  WHEN 0  THEN 'uninitialized'
  WHEN 1  THEN 'blocked'
  WHEN 2  THEN 'ready'
  WHEN 3  THEN 'pending'
  WHEN 4  THEN 'running'
  WHEN 5  THEN 'completed'
  WHEN 6  THEN 'failed'
  WHEN 7  THEN 'canceled'
  WHEN 8  THEN 'terminated'
  WHEN 9  THEN 'disabled'
  WHEN 10 THEN 'pending_failed'
END AS status_name
```

Same integer scheme is used in `result.status` (the recorded status of a single execution attempt).

- `failed` (6) — the job failed and Torc gave up on it.
- `terminated` (8) — the job was killed (e.g. by Slurm time limit or signal).
- `pending_failed` (10) — failed and awaiting AI failure classification before being retried.

**Do not assume `status` is a string.** Joining `result` to `job` returns integers; decode them.

## Workflow Status

Workflow status is **not** a named status like `job.status`; it lives as columns directly on the
`workflow` row:

- `is_canceled` — user (or scheduler) canceled the workflow.
- `is_archived` — workflow has been archived.
- `run_id` — the current run number (incremented on each restart/recovery).

To answer "is workflow X currently running?" you typically check whether any of its jobs are in
status 3 (`pending`) or 4 (`running`), not these flags directly.

## Return Code Conventions

`result.return_code` is the process exit code. Special values worth knowing:

- `0` — success
- `137` — killed by SIGKILL, almost always **out-of-memory (OOM)**
- `139` — segmentation fault
- `143` — terminated by SIGTERM
- `152` — Slurm SIGUSR1 / time-limit warning, almost always **timeout**

For "did this job hit OOM?" filter `result.return_code = 137`. For timeouts use `152` (or compare
`exec_time_minutes` against the configured runtime in `resource_requirements`).

## Paired Columns: Strings vs. Numerics

Resource fields exist in **two forms**. Always use the numeric form for math and comparisons:

| String form (display)                               | Numeric form (use for queries)               |
| --------------------------------------------------- | -------------------------------------------- |
| `resource_requirements.memory` (`"2g"`)             | `resource_requirements.memory_bytes` (bytes) |
| `resource_requirements.runtime` (ISO8601, `"PT2H"`) | `resource_requirements.runtime_s` (seconds)  |

The `result` table records **actual** usage, which you compare against the **configured** limit:

| Actual (in `result`)                        | Configured (in `resource_requirements`) |
| ------------------------------------------- | --------------------------------------- |
| `peak_memory_bytes`                         | `memory_bytes`                          |
| `peak_cpu_percent` (e.g. 350.0 = 3.5 cores) | `num_cpus * 100`                        |
| `exec_time_minutes`                         | `runtime_s / 60.0`                      |

A job exceeded its memory allocation when `result.peak_memory_bytes > rr.memory_bytes`. A job
exceeded its CPU allocation when `result.peak_cpu_percent > rr.num_cpus * 100`.

## Key Joins

```sql
-- Latest result per job (use this when the user says "the result" of a job)
FROM workflow_result wr
JOIN result r ON r.id = wr.result_id

-- All historical execution attempts for a job
FROM job j
JOIN result r ON r.job_id = j.id
ORDER BY r.run_id

-- Resource allocation context for a result
FROM result r
JOIN job j ON j.id = r.job_id
JOIN resource_requirements rr ON rr.id = j.resource_requirements_id

-- Which compute node ran a job
FROM result r
JOIN compute_node cn ON cn.id = r.compute_node_id
```

A workflow can have multiple `compute_node` rows (one per worker process or Slurm allocation).
`compute_node.compute_node_type` distinguishes local vs. Slurm allocations.

## User-Specific Context: `workflow.metadata` and `user_data.data`

`workflow.metadata` is a nullable **JSON TEXT** column for user-defined workflow context — things
like project name, dataset version, experiment tag, ticket ID. Look here first when the user asks
project-specific questions ("show workflows tagged for project X"). Extract with SQLite JSON:

```sql
SELECT id, name, json_extract(metadata, '$.project') AS project
FROM workflow
WHERE json_extract(metadata, '$.project') = 'climate-2026'
```

Common keys vary by user/org — there is no enforced schema. If the user asks about an unfamiliar
field, run `SELECT DISTINCT json_extract(metadata, '$.<key>') FROM workflow` to see what's there, or
`SELECT metadata FROM workflow WHERE metadata IS NOT NULL LIMIT 5` to scan the shape.

`user_data.data` is also JSON TEXT and holds workflow-defined intermediate values that establish job
dependencies. Inspect it the same way.

## Things to Prefer When Querying

- For "the result" of a job, prefer `workflow_result → result` (latest attempt) over scanning
  `result` directly — much faster and matches what the UI shows.
- When grouping by resource allocation, group by `resource_requirements_id` (multiple jobs typically
  share one entry) rather than per-job.
- Memory comparisons: always use `_bytes`. However, display memory values in GiB.
- Time comparisons: always use `runtime_s` and `exec_time_minutes` (note the unit mismatch —
  convert).
- Timestamps in `result.completion_time` and `workflow.timestamp` are ISO8601 TEXT. Use SQLite's
  `datetime()` / `strftime()` to compare or bucket them.
- The database is shared across many users; always include `WHERE workflow.user = '<name>'` or
  `WHERE workflow.id = <id>` unless the question is explicitly cross-user.

## Things to Avoid

- Do not `SELECT *` from `result` for whole-DB scans — it can be very large. Always join through
  `workflow_result` or filter by `workflow_id` first.
- Do not assume `job.status = 'failed'` works — `status` is an integer; use `= 6`.
- Do not reason about "is this workflow done?" from `workflow.is_canceled` / `workflow.is_archived`
  alone — check the latest job statuses.
- Do not modify the database (no INSERT/UPDATE/DELETE). datasight is read-only by design; if a query
  implies a mutation, refuse and suggest the equivalent `torc` CLI command.
