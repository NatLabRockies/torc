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

-- Backfill `origin='retry'` for rows that were already retries when this
-- column was added. Before this migration, retries were detected with the
-- heuristic `attempt_id > 1`; `torc watch --auto-schedule` now keys on
-- `origin IS NOT NULL`, so without this backfill operators upgrading from
-- a pre-spawn_jobs version would silently lose detection of already-
-- enqueued retries. Safe at migration time because `spawn_jobs` did not
-- exist before this column, so every existing `attempt_id > 1` row is a
-- failure-handler retry, not a spawned job.
UPDATE job SET origin = 'retry' WHERE attempt_id > 1 AND origin IS NULL;
