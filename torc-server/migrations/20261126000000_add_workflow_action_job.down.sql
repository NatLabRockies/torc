-- Reverse 20261126000000_add_workflow_action_job. Plain drops; no recreate.

DROP TRIGGER IF EXISTS job_status_updates_action_gates;
DROP TABLE IF EXISTS workflow_action_gate;
DROP INDEX IF EXISTS idx_workflow_action_job_job_id;
DROP TABLE IF EXISTS workflow_action_job;
