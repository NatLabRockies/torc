-- Durable audit log for admin raw-SQL executions (`torc admin sql --write`).
--
-- Intentionally NOT referencing workflow(id): admin SQL is not necessarily
-- workflow-scoped, and this audit trail must survive workflow deletion, so it
-- has no foreign key and no ON DELETE CASCADE.

CREATE TABLE admin_audit_log (
  id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
  user_name TEXT NOT NULL,
  timestamp INTEGER NOT NULL, -- milliseconds since epoch
  sql_text TEXT NOT NULL,
  is_write INTEGER NOT NULL,
  allow_full_table INTEGER NOT NULL,
  rows_affected INTEGER,
  committed INTEGER NOT NULL,
  success INTEGER NOT NULL,
  error TEXT
);
