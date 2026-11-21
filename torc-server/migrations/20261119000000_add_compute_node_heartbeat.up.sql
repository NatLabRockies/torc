-- Time of the last liveness signal (RFC3339, UTC) from the runner that owns the node.
-- NULL for nodes created by runners that do not send heartbeats. The server never
-- marks a node with a NULL heartbeat as stale.
ALTER TABLE compute_node ADD COLUMN last_heartbeat TEXT NULL;
