DROP INDEX IF EXISTS idx_ro_crate_entity_workflow_file;
CREATE INDEX idx_ro_crate_entity_workflow_file
ON ro_crate_entity(workflow_id, file_id);
