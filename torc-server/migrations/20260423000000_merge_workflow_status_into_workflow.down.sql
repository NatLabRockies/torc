-- Revert merge of workflow_status into workflow (issue #300).
--
-- Recreates the satellite table, restores the status_id column with its FK,
-- then re-creates the back-reference column added by 20260222000001 so
-- subsequent down migrations replay cleanly.

-- Step 1: Recreate the workflow_status table with the same shape it had
-- after migration 20260222000001 ran.
CREATE TABLE workflow_status (
  id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
  run_id INTEGER NOT NULL DEFAULT 1,
  has_detected_need_to_run_completion_script INTEGER NOT NULL DEFAULT 0,
  is_canceled INTEGER NOT NULL DEFAULT 0,
  is_archived INTEGER NOT NULL DEFAULT 0,
  workflow_id INTEGER NULL
);

-- Step 2: One status row per workflow, with id = workflow.id (matches the
-- 1:1 invariant the original schema relied on, even though the FK only
-- enforced existence, not equality). has_detected_need_to_run_completion_script
-- was always 0 in practice (no code path ever set it), so a literal 0 is
-- faithful to the data.
INSERT INTO workflow_status (id, run_id, is_canceled, is_archived, workflow_id)
SELECT id, run_id, is_canceled, is_archived, id
FROM workflow;

-- Step 3: Add the status_id column back to workflow.
-- ALTER TABLE ADD COLUMN cannot include a FOREIGN KEY clause, so we use
-- writable_schema to inject it after populating the values.
ALTER TABLE workflow ADD COLUMN status_id INTEGER;
UPDATE workflow SET status_id = id;

-- Step 4: Re-attach the FOREIGN KEY (status_id) REFERENCES workflow_status(id)
-- constraint via writable_schema. This mirrors the original schema text
-- emitted by 20250101000000_initial_schema.up.sql.
PRAGMA writable_schema = 1;
UPDATE sqlite_master
SET sql = replace(sql,
    ' status_id INTEGER)',
    ' status_id INTEGER,
  FOREIGN KEY (status_id) REFERENCES workflow_status(id))')
WHERE type = 'table' AND name = 'workflow';
PRAGMA writable_schema = 0;
