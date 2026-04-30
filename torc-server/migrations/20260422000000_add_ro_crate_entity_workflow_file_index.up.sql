DELETE FROM ro_crate_entity
WHERE file_id IS NOT NULL
  AND id NOT IN (
    SELECT MAX(id)
    FROM ro_crate_entity
    WHERE file_id IS NOT NULL
    GROUP BY workflow_id, file_id
  );

DROP INDEX IF EXISTS idx_ro_crate_entity_workflow_file;
CREATE UNIQUE INDEX idx_ro_crate_entity_workflow_file
ON ro_crate_entity(workflow_id, file_id)
WHERE file_id IS NOT NULL;
