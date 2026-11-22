-- Reverse 20261122000000_add_workflow_is_draft. Plain column drop; no recreate.

ALTER TABLE workflow DROP COLUMN is_draft;
