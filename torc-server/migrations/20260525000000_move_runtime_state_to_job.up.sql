-- Move per-attempt runtime state from job_internal to the public `job` table so
-- it can be exposed via the API without joining. `job_internal` keeps only the
-- server-internal `input_hash` it was originally created for.
--
-- Both columns are set in start_job and cleared in complete_job / reset paths.
-- `job.status` (not these columns) remains the source of truth for "is running."

ALTER TABLE job ADD COLUMN start_time TEXT NULL;
ALTER TABLE job ADD COLUMN compute_node_id INTEGER NULL
  REFERENCES compute_node(id) ON DELETE SET NULL;

UPDATE job
SET compute_node_id = (
  SELECT active_compute_node_id
  FROM job_internal
  WHERE job_internal.job_id = job.id
);

DROP INDEX IF EXISTS idx_job_internal_active_compute_node_id;

ALTER TABLE job_internal DROP COLUMN active_compute_node_id;

CREATE INDEX idx_job_compute_node_id ON job(compute_node_id)
  WHERE compute_node_id IS NOT NULL;
