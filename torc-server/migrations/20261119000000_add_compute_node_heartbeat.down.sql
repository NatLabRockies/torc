-- Reverse 20261119000000_add_compute_node_heartbeat. Plain column drop; no recreate.
ALTER TABLE compute_node DROP COLUMN last_heartbeat;
