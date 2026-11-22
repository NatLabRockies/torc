-- Reverse 20261121000000_add_workflow_action_state. Plain column drops; no recreate.

ALTER TABLE workflow_action DROP COLUMN error;
ALTER TABLE workflow_action DROP COLUMN state;
