-- Reverse 20261119000000_add_compute_node_heartbeat. Plain column drop; no recreate.

DROP INDEX IF EXISTS idx_compute_node_active_heartbeat;
ALTER TABLE compute_node DROP COLUMN last_heartbeat;
