CREATE INDEX IF NOT EXISTS idx_job_workflow_status_priority
    ON job(workflow_id, status, priority DESC);

DROP INDEX IF EXISTS idx_job_workflow_status_label_priority;
