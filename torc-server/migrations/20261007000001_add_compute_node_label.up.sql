-- Record the label a runner was started with (`torc run --label X`) on its
-- compute node, so `torc compute-nodes list` shows which runners serve which
-- labels. NULL means the runner claims only unlabeled jobs.
--
-- Plain ALTER TABLE ADD COLUMN (see CLAUDE.md migration warning).

ALTER TABLE compute_node ADD COLUMN label TEXT NULL DEFAULT NULL;
