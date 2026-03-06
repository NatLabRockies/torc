-- Revert datasets migration

-- Drop indexes
DROP INDEX IF EXISTS idx_job_dataset_input_workflow_id;
DROP INDEX IF EXISTS idx_job_dataset_output_workflow_id;
DROP INDEX IF EXISTS idx_job_dataset_input_dataset_id;
DROP INDEX IF EXISTS idx_job_dataset_output_dataset_id;
DROP INDEX IF EXISTS idx_dataset_pending_finalization;
DROP INDEX IF EXISTS idx_dataset_status;
DROP INDEX IF EXISTS idx_dataset_workflow_id;

-- Drop junction tables
DROP TABLE IF EXISTS job_dataset_input;
DROP TABLE IF EXISTS job_dataset_output;

-- Drop dataset table
DROP TABLE IF EXISTS dataset;

-- Remove has_datasets column from workflow
-- SQLite doesn't support DROP COLUMN directly, so we need to recreate the table
-- For simplicity in development, we'll just leave the column (it defaults to 0)
-- In production, a proper table rebuild would be needed
