# Admin SQL Access (Database Escape Hatch)

`torc admin sql` lets an admin run a raw SQL statement against the server database. It is a
controlled, audit-logged escape hatch for inspecting state and surgically repairing the database
when a bug or edge case leaves a workflow stuck and no dedicated command can fix it. It is
admin-only (requires `--admin-user` on the server) and is safer than opening `sqlite3` on the server
host directly, because it is authenticated, guard-railed, and audited.

> **Warning:** Raw writes bypass application invariants (the job status state machine, the
> background unblocking contract). A direct `UPDATE`/`DELETE` can leave the database in a state Torc
> never produces. Prefer a dedicated `torc` command when one exists, and treat raw SQL as a last
> resort.

## Reading (default)

Without `--write`, the statement runs on a **read-only** connection, so any write is rejected by
SQLite itself. Results honor the global `-f/--format` option (`table`, `csv`, or `json`):

```bash
torc admin sql "SELECT id, status FROM job LIMIT 5"
torc -f csv  admin sql "SELECT id, status FROM job"
torc -f json admin sql "SELECT * FROM admin_audit_log"
```

`--limit` caps the number of returned rows (default and maximum 100,000, the same as other list
endpoints).

JSON output is an array of objects under `items`, with a parallel `columns` array giving the display
order. Column names the query repeats (e.g. an unaliased join on two `id` columns) are suffixed
(`id`, `id_2`, ...) so each object's keys stay unique:

```json
{
  "columns": ["id", "workflow_id", "max_mem"],
  "items": [{ "id": 2, "workflow_id": 1, "max_mem": 447610880 }],
  "committed": false
}
```

## Writing

Writes require `--write`. The CLI first previews how many rows the statement would affect (a dry-run
that rolls back), prompts for confirmation, then commits:

```bash
torc admin sql --write "UPDATE result SET return_code=0 WHERE id=42"
```

- `--yes` skips the confirmation prompt.
- An unqualified `UPDATE`/`DELETE` (no `WHERE` clause) is refused unless you pass
  `--allow-full-table`. The leading verb is detected past an optional `WITH` (CTE), so
  `WITH ... DELETE FROM t` is guarded the same as a bare `DELETE`.
- DDL (`DROP`/`ALTER`/`TRUNCATE`), `ATTACH`/`DETACH`, and multi-statement input are always rejected.
  Leading comments cannot be used to hide a disallowed keyword.

## Audit log

Every committing write is recorded in the durable `admin_audit_log` table (user, timestamp,
statement, rows affected, success). This table has no foreign key to `workflow`, so the audit trail
survives workflow deletion. Review it with a read query:

```bash
torc admin sql "SELECT user_name, timestamp, sql_text, rows_affected FROM admin_audit_log ORDER BY id DESC LIMIT 20"
```

## Disabling the feature

The raw-SQL endpoint is enabled by default. Cautious operators can turn it off at the server with
two `torc-server run` flags (also settable in the server config file):

- `--disable-admin-sql` disables the endpoint entirely; both reads and writes return `403`.
- `--disable-admin-sql-writes` keeps read-only queries working but rejects every `--write`
  (including the dry-run preview) with `403`.

The audit-log listing (`GET /admin/audit-log`) is **not** gated by these flags, so a past trail of
admin writes stays reviewable even after the feature is disabled.
