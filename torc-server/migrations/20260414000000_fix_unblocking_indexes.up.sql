DROP INDEX IF EXISTS idx_job_unblocking_pending;
DROP INDEX IF EXISTS idx_job_workflow_unblocking;

CREATE INDEX idx_job_unblocking_pending
ON job(workflow_id, status, unblocking_processed)
WHERE status IN (5, 6, 7, 8) AND unblocking_processed = 0;

CREATE INDEX idx_job_workflow_unblocking
ON job(workflow_id)
WHERE status IN (5, 6, 7, 8) AND unblocking_processed = 0;
