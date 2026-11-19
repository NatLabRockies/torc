-- Time of the last liveness signal (RFC3339, UTC) from the runner that owns the node.
-- NULL for nodes created by runners that do not send heartbeats. The server never
-- expires a node with a NULL heartbeat.
ALTER TABLE compute_node ADD COLUMN last_heartbeat TEXT NULL;

-- Lets the expiry sweep find stale active nodes without scanning the whole table.
CREATE INDEX idx_compute_node_active_heartbeat ON compute_node(last_heartbeat)
  WHERE is_active = 1 AND last_heartbeat IS NOT NULL;
