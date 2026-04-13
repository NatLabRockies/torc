DROP INDEX IF EXISTS idx_job_workflow_status_priority;

ALTER TABLE job DROP COLUMN priority;
