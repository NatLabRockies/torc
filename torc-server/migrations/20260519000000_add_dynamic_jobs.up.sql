-- Dynamic job spawning support.
--
-- `workflow.max_spawn_iterations_per_lineage` caps the number of `spawn_jobs`
-- calls per orchestrator lineage (a runaway guard that maps to "max
-- iterations per run"). NULL means use the server default.
--
-- `job.origin` records why a job exists — NULL for jobs declared at workflow
-- creation (anticipated by `on_jobs_ready` / `schedule_nodes` deferred
-- actions), `'retry'` for jobs resurrected by failure-handler retries, and
-- `'spawn'` for jobs added at runtime by `spawn_jobs`. `torc watch
-- --auto-schedule` uses `origin IS NOT NULL` as its "needs unplanned
-- allocation" signal, replacing the older `attempt_id > 1` heuristic.
--
-- Plain ALTER TABLE ADD COLUMN: no rename-recreate of the FK-cascade parent
-- `workflow` table (see CLAUDE.md migration warning).

ALTER TABLE workflow ADD COLUMN max_spawn_iterations_per_lineage INTEGER NULL;

ALTER TABLE job ADD COLUMN origin TEXT NULL DEFAULT NULL;
