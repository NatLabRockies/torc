-- A job and a runner each carry a set of routing labels instead of one label.
-- The sets are stored as sorted JSON arrays (e.g. '["gpu","windows"]'); NULL
-- still means "no labels". A runner claims a job only when it has every label
-- the job has; unlabeled jobs still go only to unlabeled runners.
--
-- RENAME COLUMN keeps the existing index on job(workflow_id, status, labels,
-- priority DESC) intact and avoids the rename-recreate pattern (see CLAUDE.md).

ALTER TABLE job RENAME COLUMN label TO labels;
UPDATE job SET labels = json_array(labels) WHERE labels IS NOT NULL;

ALTER TABLE compute_node RENAME COLUMN label TO labels;
UPDATE compute_node SET labels = json_array(labels) WHERE labels IS NOT NULL;
