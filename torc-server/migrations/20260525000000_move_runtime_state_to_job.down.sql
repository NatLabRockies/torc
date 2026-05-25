-- Reverse the move: put active_compute_node_id back on job_internal and drop
-- start_time + compute_node_id from job.

ALTER TABLE job_internal ADD COLUMN active_compute_node_id INTEGER
  REFERENCES compute_node(id) ON DELETE SET NULL;

UPDATE job_internal
SET active_compute_node_id = (
  SELECT compute_node_id
  FROM job
  WHERE job.id = job_internal.job_id
);

DROP INDEX IF EXISTS idx_job_compute_node_id;

ALTER TABLE job DROP COLUMN compute_node_id;
ALTER TABLE job DROP COLUMN start_time;

CREATE INDEX idx_job_internal_active_compute_node_id
  ON job_internal(active_compute_node_id)
  WHERE active_compute_node_id IS NOT NULL;
