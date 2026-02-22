-- Revert: SQLite does not support DROP COLUMN, so this is a table recreation.
-- The workflow_id column is removed by recreating the table without it.

CREATE TABLE workflow_status_old (
  id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
  run_id INTEGER NOT NULL DEFAULT 1,
  has_detected_need_to_run_completion_script INTEGER NOT NULL DEFAULT 0,
  is_canceled INTEGER NOT NULL DEFAULT 0,
  is_archived INTEGER NOT NULL DEFAULT 0
);

INSERT INTO workflow_status_old (id, run_id, has_detected_need_to_run_completion_script, is_canceled, is_archived)
SELECT id, run_id, has_detected_need_to_run_completion_script, is_canceled, is_archived
FROM workflow_status;

DROP TABLE workflow_status;
ALTER TABLE workflow_status_old RENAME TO workflow_status;
