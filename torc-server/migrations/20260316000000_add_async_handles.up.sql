-- Add async_handles table for tracking long-running server operations.
--
-- This table is used for operations like workflow initialization that may take
-- a long time. Status is persisted so clients can poll and/or wait via SSE.

CREATE TABLE async_handles (
  id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
  workflow_id INTEGER NOT NULL,
  operation TEXT NOT NULL,
  status TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL,
  started_at_ms INTEGER NULL,
  finished_at_ms INTEGER NULL,
  requested_by TEXT NULL,
  request_json TEXT NULL,
  result_json TEXT NULL,
  error TEXT NULL,
  FOREIGN KEY (workflow_id) REFERENCES workflow(id) ON DELETE CASCADE
);

-- Enforce at most one active task per (workflow_id, operation).
-- SQLite supports partial indexes, which we use to scope uniqueness to "active" statuses.
CREATE UNIQUE INDEX idx_async_handles_unique_active_workflow_operation
  ON async_handles(workflow_id, operation)
  WHERE status IN ('queued', 'running');

CREATE INDEX idx_async_handles_workflow_status ON async_handles(workflow_id, status);
CREATE INDEX idx_async_handles_workflow_created_at ON async_handles(workflow_id, created_at_ms DESC);
