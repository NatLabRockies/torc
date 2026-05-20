-- Dynamic job spawning support.
--
-- `workflow.max_spawn_iterations_per_lineage` caps the number of `spawn_jobs`
-- calls per orchestrator lineage (a runaway guard that maps to "max
-- iterations per run"). NULL means use the server default.
--
-- Plain ALTER TABLE ADD COLUMN: no rename-recreate of the FK-cascade parent
-- `workflow` table (see CLAUDE.md migration warning).

ALTER TABLE workflow ADD COLUMN max_spawn_iterations_per_lineage INTEGER NULL;
