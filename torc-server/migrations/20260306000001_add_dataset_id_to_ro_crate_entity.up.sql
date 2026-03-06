-- ============================================================================
-- ADD DATASET_ID TO RO_CRATE_ENTITY
-- ============================================================================
-- This migration adds support for linking RO-Crate entities to datasets,
-- enabling Dataset entities to be included in RO-Crate metadata exports.
--
-- Schema Version: 2026-03-06
-- ============================================================================

-- Add dataset_id column to ro_crate_entity table
ALTER TABLE ro_crate_entity ADD COLUMN dataset_id INTEGER REFERENCES dataset(id) ON DELETE SET NULL;

-- Create index for efficient lookups by dataset_id
CREATE INDEX idx_ro_crate_entity_dataset_id ON ro_crate_entity(dataset_id);
