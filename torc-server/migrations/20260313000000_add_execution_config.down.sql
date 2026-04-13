-- Remove execution_config column from workflow table.
-- Note: SQLite doesn't support DROP COLUMN directly before version 3.35.0.
-- The approach below creates a new table without the column and copies data.
-- However, since this is a simple TEXT column with no constraints or foreign keys,
-- we can use the ALTER TABLE DROP COLUMN syntax (requires SQLite 3.35.0+).

-- For SQLite < 3.35.0, you would need to use the rename-recreate pattern,
-- but per CLAUDE.md this is dangerous for parent tables with CASCADE deletes.
-- Since execution_config has no references, a simple approach is safest.

-- This requires SQLite 3.35.0+
ALTER TABLE workflow DROP COLUMN execution_config;
