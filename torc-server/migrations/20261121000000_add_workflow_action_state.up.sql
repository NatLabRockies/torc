-- Record what happened to a workflow action, separately from `executed`.
--
-- state: NULL (never claimed, or claimed by a pre-1.4 server), 'claimed', 'succeeded', 'failed',
-- or 'suppressed'. `executed` stays the "not available to claim" flag, so get_pending_actions and
-- older clients keep working. error holds the claimer's message for a failed action.
--
-- Plain ALTER TABLE ADD COLUMN (see CLAUDE.md migration warning).

ALTER TABLE workflow_action ADD COLUMN state TEXT NULL DEFAULT NULL;
ALTER TABLE workflow_action ADD COLUMN error TEXT NULL DEFAULT NULL;
