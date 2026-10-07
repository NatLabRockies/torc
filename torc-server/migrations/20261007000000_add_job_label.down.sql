-- Reverse 20261007000000_add_job_label. Plain column drop; no recreate.

ALTER TABLE job DROP COLUMN label;
