-- Dynamic job spawning support.
--
-- `workflow.dynamic_jobs` is a JSON blob mirroring the workflow-spec
-- `dynamic_jobs` section. Currently holds `{"max_iterations": N}` (per-
-- orchestrator-lineage spawn cap, a runaway guard); NULL means use the
-- server default. Stored as JSON for forward compatibility — additional
-- `dynamic_jobs` fields can be added without further migrations. Matches
-- the JSON-blob pattern used by `workflow.slurm_defaults`,
-- `workflow.resource_monitor_config`, and `workflow.execution_config`.
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

ALTER TABLE workflow ADD COLUMN dynamic_jobs TEXT NULL;

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
