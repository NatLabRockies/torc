-- Claim queries filter ready jobs by (workflow_id, status, label) and order by
-- priority. Without label in the index, a runner for a rare label scans every
-- ready job in the workflow on each poll, inside the claim write lock.

CREATE INDEX idx_job_workflow_status_label_priority
    ON job(workflow_id, status, label, priority DESC);
