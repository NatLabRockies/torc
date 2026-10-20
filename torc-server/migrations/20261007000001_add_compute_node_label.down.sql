-- Reverse 20261007000001_add_compute_node_label. Plain column drop; no recreate.

ALTER TABLE compute_node DROP COLUMN label;
