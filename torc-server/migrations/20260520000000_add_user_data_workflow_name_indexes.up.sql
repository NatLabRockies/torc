-- Add indexes on user_data(workflow_id, name).
--
-- `idx_user_data_workflow_name` accelerates every existing read path that
-- filters by `(workflow_id, name)` — `read_user_data`, `delete_user_data`,
-- and the new `spawn_jobs` lineage helpers
-- (`derive_lineage_spawn_count`, `upsert_final_state`). Without it those
-- queries fall back to `idx_user_data_workflow_id` and post-filter by
-- name, which becomes O(rows-per-workflow) once a workflow accumulates
-- many user_data records.
--
-- `idx_user_data_lineage_unique` is a **partial** unique index covering
-- only the spawn_jobs lineage records (names prefixed
-- `__torc_lineage__`). It makes `upsert_final_state` race-safe by
-- construction: the lookup-then-insert pattern collapses to
-- `INSERT … ON CONFLICT(workflow_id, name) WHERE … DO UPDATE`. The
-- partial scope is deliberate — a full `UNIQUE(workflow_id, name)`
-- could fail on existing deployments that may already have duplicate
-- general-purpose user_data rows, since no uniqueness was enforced
-- historically. Scoping to the `__torc_lineage__` prefix limits the
-- constraint to records this code path owns end-to-end.
--
-- Plain CREATE INDEX statements — no table recreate, FK-cascade safe
-- (see CLAUDE.md migration warning).

CREATE INDEX idx_user_data_workflow_name ON user_data(workflow_id, name);

CREATE UNIQUE INDEX idx_user_data_lineage_unique
ON user_data(workflow_id, name)
WHERE name LIKE '__torc_lineage__%';
