-- A job and a runner each carry a set of routing labels instead of one label.
-- The sets are stored as sorted JSON arrays (e.g. '["gpu","windows"]'); NULL
-- still means "no labels". A runner claims a job only when it has every label
-- the job has; unlabeled jobs still go only to unlabeled runners.
--
-- RENAME COLUMN keeps the existing index on job(workflow_id, status, labels,
-- priority DESC) intact and avoids the rename-recreate pattern (see CLAUDE.md).
--
-- The single-label schema accepted commas, and its `labels=a,b` query already
-- treated them as separators, so a legacy 'a,b' becomes '["a","b"]': json_quote
-- escapes the value, then each comma becomes a JSON element boundary.

ALTER TABLE job RENAME COLUMN label TO labels;
UPDATE job SET labels = '[' || replace(json_quote(labels), ',', '","') || ']'
WHERE labels IS NOT NULL;

ALTER TABLE compute_node RENAME COLUMN label TO labels;
UPDATE compute_node SET labels = '[' || replace(json_quote(labels), ',', '","') || ']'
WHERE labels IS NOT NULL;
