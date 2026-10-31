-- Job labels route jobs to specific runners.
--
-- `job.label` is an optional routing tag. A runner started with `--label X`
-- claims only jobs whose label is X; a runner with no label claims only jobs
-- with no label (NULL). The match is enforced by the server in the claim
-- queries.
--
-- Plain ALTER TABLE ADD COLUMN (see CLAUDE.md migration warning).

ALTER TABLE job ADD COLUMN label TEXT NULL DEFAULT NULL;
