-- Add workflow_id column to workflow_status.
-- Previously workflow_status had no back-reference to workflow, causing
-- orphaned status records when workflows were deleted.
--
-- Note: SQLite ALTER TABLE ADD COLUMN cannot include FK constraints, so
-- there is no CASCADE here. The delete_workflow function handles cleanup
-- explicitly (and runs with PRAGMA foreign_keys = OFF anyway).

ALTER TABLE workflow_status ADD COLUMN workflow_id INTEGER NULL;

-- Populate workflow_id from the workflow table's status_id reference
UPDATE workflow_status
SET workflow_id = (SELECT w.id FROM workflow w WHERE w.status_id = workflow_status.id);

-- Delete orphaned status records (no matching workflow)
DELETE FROM workflow_status WHERE workflow_id IS NULL;
