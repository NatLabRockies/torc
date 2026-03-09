-- Re-add step_nodes to resource_requirements for rollback.
ALTER TABLE resource_requirements ADD COLUMN step_nodes INTEGER NOT NULL DEFAULT 1;
