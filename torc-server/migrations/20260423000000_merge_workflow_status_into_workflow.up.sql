-- Merge workflow_status into workflow (issue #300).
--
-- workflow_status was a 1:1 satellite table of workflow whose only purpose was
-- to hold mutable status fields (run_id, is_canceled, is_archived). The
-- indirection added no analytical value, leaked workflow counts in filtered
-- exports (PR #299), and produced a class of orphans that no FK could enforce
-- away.
--
-- This migration copies the satellite columns onto workflow, drops the
-- now-redundant status_id FK, drops the column, and drops the satellite
-- table. It also drops the dead has_detected_need_to_run_completion_script
-- field — no code path ever set it to 1; the completion-script concept is
-- now expressed via on_workflow_complete workflow actions.
-- workflow.is_archived already existed but was unused in code; the backfill
-- overwrites it with the authoritative value from workflow_status.

-- Step 1: Add the missing status columns to workflow.
-- workflow already has is_archived; we add the rest.
ALTER TABLE workflow ADD COLUMN run_id INTEGER NOT NULL DEFAULT 1;
ALTER TABLE workflow ADD COLUMN is_canceled INTEGER NOT NULL DEFAULT 0;

-- Step 2: Backfill from workflow_status. Use COALESCE so workflows with a
-- missing status row (which shouldn't exist after migration
-- 20260222000001, but defend in depth) keep the column defaults instead
-- of getting NULL into NOT NULL columns.
UPDATE workflow
SET run_id = COALESCE(
        (SELECT ws.run_id FROM workflow_status ws WHERE ws.id = workflow.status_id),
        run_id
    ),
    is_canceled = COALESCE(
        (SELECT ws.is_canceled FROM workflow_status ws WHERE ws.id = workflow.status_id),
        is_canceled
    ),
    is_archived = COALESCE(
        (SELECT ws.is_archived FROM workflow_status ws WHERE ws.id = workflow.status_id),
        is_archived
    );

-- Step 3: Strip the FOREIGN KEY (status_id) constraint from workflow's schema.
--
-- We cannot recreate workflow via the rename-recreate pattern: workflow is
-- the parent of 15+ child tables with ON DELETE CASCADE, and DROP TABLE
-- inside a sqlx migration transaction (where PRAGMA foreign_keys=OFF is a
-- no-op) would cascade-delete the entire database. See CLAUDE.md.
--
-- Editing sqlite_master directly via PRAGMA writable_schema is the
-- documented workaround for altering a parent table's constraints in place.
-- The replace() target matches the literal substring emitted by the original
-- CREATE TABLE in 20250101000000_initial_schema.up.sql; ALTER TABLE
-- ADD COLUMN appends new columns after the FK clause without disturbing it.
PRAGMA writable_schema = 1;
UPDATE sqlite_master
SET sql = replace(sql,
    ',
  FOREIGN KEY (status_id) REFERENCES workflow_status(id)',
    '')
WHERE type = 'table' AND name = 'workflow';
PRAGMA writable_schema = 0;

-- Step 4: Drop the now-vestigial status_id column.
-- DROP COLUMN requires the column not to be referenced by any constraint;
-- step 3 cleared the FK that would otherwise block this.
ALTER TABLE workflow DROP COLUMN status_id;

-- Step 5: Drop the workflow_status table. No other tables reference it now.
DROP TABLE workflow_status;
