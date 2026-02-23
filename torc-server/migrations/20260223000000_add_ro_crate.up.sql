-- ============================================================================
-- ADD RO-CRATE ENTITIES
-- ============================================================================
-- This migration adds support for storing RO-Crate (Research Object Crate)
-- metadata entries. Each row represents one JSON-LD entity description that
-- can be assembled into a valid ro-crate-metadata.json document.
--
-- Schema Version: 2026-02-23
-- ============================================================================

-- ----------------------------------------------------------------------------
-- ro_crate_entity: JSON-LD entity descriptions for RO-Crate metadata
-- ----------------------------------------------------------------------------
-- Each entity has:
-- - entity_id: The JSON-LD @id (e.g., "data/output.parquet", "https://...")
-- - entity_type: Schema.org type (e.g., "File", "Dataset", "SoftwareApplication")
-- - metadata: Full JSON-LD object as text
-- - file_id: Optional link to a file record (NULL for external entities)
-- ----------------------------------------------------------------------------
CREATE TABLE ro_crate_entity (
  id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
  workflow_id INTEGER NOT NULL,
  file_id INTEGER,
  entity_id TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  metadata TEXT NOT NULL,
  FOREIGN KEY (workflow_id) REFERENCES workflow(id) ON DELETE CASCADE,
  FOREIGN KEY (file_id) REFERENCES file(id) ON DELETE SET NULL
);

CREATE INDEX idx_ro_crate_entity_workflow_id ON ro_crate_entity(workflow_id);
CREATE INDEX idx_ro_crate_entity_file_id ON ro_crate_entity(file_id);
