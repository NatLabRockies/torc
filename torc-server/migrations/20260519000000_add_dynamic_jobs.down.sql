-- Reverse 20260519000000_add_dynamic_jobs. Plain column drop; no recreate.

ALTER TABLE workflow DROP COLUMN max_spawn_iterations_per_lineage;
