-- Add async_handle table for tracking long-running server operations.
--
-- This table is used for operations like workflow initialization that may take
-- a long time. Status is persisted so clients can poll and/or wait via SSE.

CREATE TABLE async_handle (
  id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
  workflow_id INTEGER NOT NULL,
  operation TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'succeeded', 'failed')),
  created_at_ms INTEGER NOT NULL,
  started_at_ms INTEGER NULL,
  finished_at_ms INTEGER NULL,
  requested_by TEXT NULL,
  request_json TEXT NULL,
  result_json TEXT NULL,
  error TEXT NULL,
  FOREIGN KEY (workflow_id) REFERENCES workflow(id) ON DELETE CASCADE
);

-- At most one active (queued or running) task per workflow. Different async operations on the
-- same workflow would conflict on overlapping state, so we serialize them at the workflow level
-- rather than per-operation. Partial indexes scope uniqueness to "active" statuses, so history
-- is unconstrained.
CREATE UNIQUE INDEX idx_async_handle_unique_active_workflow
  ON async_handle(workflow_id)
  WHERE status IN ('queued', 'running');

CREATE INDEX idx_async_handle_workflow_status ON async_handle(workflow_id, status);
CREATE INDEX idx_async_handle_workflow_created_at ON async_handle(workflow_id, created_at_ms DESC);
