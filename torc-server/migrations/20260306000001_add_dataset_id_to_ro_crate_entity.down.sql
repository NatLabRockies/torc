-- ============================================================================
-- REMOVE DATASET_ID FROM RO_CRATE_ENTITY
-- ============================================================================
-- Reverses the addition of dataset_id column.
-- ============================================================================

DROP INDEX IF EXISTS idx_ro_crate_entity_dataset_id;

-- SQLite doesn't support DROP COLUMN directly, so we need to recreate the table
CREATE TABLE ro_crate_entity_new (
  id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
  workflow_id INTEGER NOT NULL,
  file_id INTEGER,
  entity_id TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  metadata TEXT NOT NULL,
  FOREIGN KEY (workflow_id) REFERENCES workflow(id) ON DELETE CASCADE,
  FOREIGN KEY (file_id) REFERENCES file(id) ON DELETE SET NULL
);

INSERT INTO ro_crate_entity_new (id, workflow_id, file_id, entity_id, entity_type, metadata)
SELECT id, workflow_id, file_id, entity_id, entity_type, metadata FROM ro_crate_entity;

DROP TABLE ro_crate_entity;
ALTER TABLE ro_crate_entity_new RENAME TO ro_crate_entity;

CREATE INDEX idx_ro_crate_entity_workflow_id ON ro_crate_entity(workflow_id);
CREATE INDEX idx_ro_crate_entity_file_id ON ro_crate_entity(file_id);
