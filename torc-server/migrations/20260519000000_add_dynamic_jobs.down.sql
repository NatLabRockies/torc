-- Reverse 20260519000000_add_dynamic_jobs. Plain column drops; no recreate.

ALTER TABLE job DROP COLUMN origin;
ALTER TABLE workflow DROP COLUMN dynamic_jobs;
