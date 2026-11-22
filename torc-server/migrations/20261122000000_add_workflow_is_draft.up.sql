-- Draft workflows (#524). A client builds a workflow with is_draft = 1, fills it with bulk
-- requests, and calls finalize_workflow, which validates it and sets is_draft = 0. Drafts are
-- hidden from list_workflows and can't be initialized. The server deletes drafts that are never
-- finalized.
--
-- Plain ALTER TABLE ADD COLUMN (see CLAUDE.md migration warning).

ALTER TABLE workflow ADD COLUMN is_draft INTEGER NOT NULL DEFAULT 0;
