-- Add priority column to job table.
-- Higher values are scheduled first. Minimum 0, default 0.
ALTER TABLE job ADD COLUMN priority INTEGER NOT NULL DEFAULT 0;

CREATE INDEX idx_job_workflow_status_priority
    ON job(workflow_id, status, priority DESC);
