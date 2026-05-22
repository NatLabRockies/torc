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
-- only the spawn_jobs lineage records (names that literally start with
-- `__torc_lineage__`). Covers both the per-generation `__g######` rows
-- and the per-lineage `__final` row; enforces uniqueness on
-- `(workflow_id, name)`, which in practice means at most one `__final`
-- per lineage (the per-generation names are unique by construction).
-- Makes `upsert_final_state` race-safe by construction: the lookup-
-- then-insert pattern collapses to
-- `INSERT … ON CONFLICT(workflow_id, name) WHERE … DO UPDATE`. The
-- partial scope is deliberate — a full `UNIQUE(workflow_id, name)`
-- could fail on existing deployments that may already have duplicate
-- general-purpose user_data rows, since no uniqueness was enforced
-- historically. Scoping to the `__torc_lineage__` prefix limits the
-- constraint to records this code path owns end-to-end.
--
-- The predicate uses `GLOB`, not `LIKE`, because in SQL `LIKE` the
-- underscore is a single-character wildcard. With `LIKE
-- '__torc_lineage__%'` the partial index would match unrelated names
-- like `XYtorc_lineageZ…`, broadening the constraint past the literal
-- prefix this feature actually owns. `GLOB '__torc_lineage__*'` treats
-- underscores literally and uses `*` for the only wildcard. The
-- matching `ON CONFLICT … WHERE` clause in `upsert_final_state` must
-- stay identical to this predicate so SQLite binds the upsert to this
-- partial index.
--
-- Plain CREATE INDEX statements — no table recreate, FK-cascade safe
-- (see CLAUDE.md migration warning).

CREATE INDEX idx_user_data_workflow_name ON user_data(workflow_id, name);

CREATE UNIQUE INDEX idx_user_data_lineage_unique
ON user_data(workflow_id, name)
WHERE name GLOB '__torc_lineage__*';
