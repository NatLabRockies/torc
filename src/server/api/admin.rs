//! Admin raw-SQL endpoint business logic.
//!
//! Backs `POST /admin/sql` (`torc admin sql`). This is a controlled, audit-logged
//! escape hatch for admins to inspect and surgically repair database state when a
//! bug leaves a workflow stuck. It is intentionally narrow:
//!
//! - Reads are restricted to `SELECT`/`WITH`/`VALUES` and run with
//!   [`PRAGMA query_only`](execute_read_only) enabled on a pooled connection, so
//!   any write fails at the SQLite engine layer regardless of statement parsing,
//!   and no connection-scoped state (pragmas, transactions) can leak back to the
//!   pool.
//! - Writes run inside a transaction with an optional dry-run (rollback) preview
//!   and a no-WHERE guard, and every committing write is recorded in
//!   `admin_audit_log`. `PRAGMA` and transaction-control verbs
//!   (`BEGIN`/`COMMIT`/`ROLLBACK`/...) are rejected so they can't disturb that
//!   transaction.
//! - DDL (`DROP`/`ALTER`/`TRUNCATE`) and `ATTACH`/`DETACH` are rejected outright
//!   on both paths.
//!
//! Raw writes bypass application invariants (the job status state machine, the
//! background unblocking contract); callers are warned to treat this as a last
//! resort.

use chrono::Utc;
use serde_json::Value;
use sqlx::sqlite::{SqliteConnection, SqlitePool};
use sqlx::{Column, Row, TypeInfo, ValueRef};

use crate::MAX_RECORD_TRANSFER_COUNT;
use crate::models;
use crate::server::api::begin_immediate;

/// A failure from executing an admin SQL statement, tagged with who can act on it.
///
/// Separates a caller-fault SQL problem (parse error, constraint violation, a
/// statement that fails when run) from a server-side/infrastructure failure
/// (pool exhaustion, transaction begin/commit, audit-row insert, restoring
/// connection state). The transport maps [`User`](AdminSqlError::User) to a 422
/// and [`Internal`](AdminSqlError::Internal) to a 500 so the HTTP status reflects
/// whether the caller or the operator needs to act.
#[derive(Debug)]
pub enum AdminSqlError {
    /// The caller's SQL is at fault; surfaced as HTTP 422.
    User(String),
    /// A server-side failure unrelated to the statement's validity; HTTP 500.
    Internal(String),
}

impl AdminSqlError {
    /// The human-readable message, regardless of kind.
    pub fn message(&self) -> &str {
        match self {
            AdminSqlError::User(m) | AdminSqlError::Internal(m) => m,
        }
    }

    /// Whether this is a server-side failure (HTTP 500) rather than a caller
    /// SQL error (HTTP 422).
    pub fn is_internal(&self) -> bool {
        matches!(self, AdminSqlError::Internal(_))
    }
}

/// Clamp a caller-supplied row limit into `1..=MAX_RECORD_TRANSFER_COUNT`. The
/// admin endpoints (`admin sql` SELECTs and the audit-log listing) use the same
/// row cap (100,000) as the standard list endpoints.
pub fn clamp_limit(limit: Option<i64>) -> usize {
    match limit {
        Some(n) if n > 0 => n.min(MAX_RECORD_TRANSFER_COUNT) as usize,
        _ => MAX_RECORD_TRANSFER_COUNT as usize,
    }
}

/// Validate an admin SQL statement before execution.
///
/// Enforces the statement-level guardrails that apply regardless of the
/// read/write path. All checks run against a copy of the statement with string
/// literals and comments blanked out (see [`strip_strings_and_comments`]), so
/// their contents can't fool a check (a `;`, `WHERE`, or `(` inside a string) and
/// a leading comment can't hide the real keyword:
/// - non-empty, single statement only (no `;`-separated batches)
/// - `ATTACH`/`DETACH` are rejected (they can read/write arbitrary files even on
///   a read-only connection)
/// - DDL (`DROP`/`ALTER`/`TRUNCATE`) is rejected: this is a data-repair escape
///   hatch, and schema changes belong in migrations. `DROP TABLE` in particular
///   would cascade-delete child rows via `ON DELETE CASCADE`.
///
/// For the read path (`is_write == false`), an allow list permits only
/// `SELECT`/`WITH`/`VALUES`. Reads run on a pooled connection guarded by `PRAGMA
/// query_only`, which stops data writes but not connection-scoped state changes;
/// rejecting everything else keeps a `PRAGMA foreign_keys = OFF` or a stray
/// `BEGIN` from leaking onto the connection and corrupting later borrowers from
/// the pool. `EXPLAIN` is excluded so an inner `ATTACH`/`DETACH`/DDL statement
/// can't slip past the leading-keyword guards.
///
/// For the write path, a deny list rejects `PRAGMA` and transaction-control verbs
/// (`BEGIN`/`COMMIT`/`END`/`ROLLBACK`/`SAVEPOINT`/`RELEASE`): the write runs
/// inside a transaction, so a connection-state pragma or nested transaction
/// control would corrupt it. The write path also rejects an unqualified
/// `UPDATE`/`DELETE` (one with no `WHERE` clause) unless `allow_full_table` is
/// set. The leading verb is detected
/// at parenthesis depth 0, so a CTE-prefixed write (`WITH c AS (...) DELETE FROM
/// t`) is guarded the same as a bare `DELETE`. This is still a heuristic, not a
/// full SQL parser: a `WHERE` inside a subquery can satisfy the guard.
///
/// Returns `Err(message)` describing a 422-class rejection.
pub fn validate_statement(sql: &str, is_write: bool, allow_full_table: bool) -> Result<(), String> {
    if sql.trim().is_empty() {
        return Err("SQL statement is empty".to_string());
    }

    // Blank string literals and comments so their contents can't influence the
    // token-based checks below, then uppercase for keyword matching.
    let upper = strip_strings_and_comments(sql).to_uppercase();

    // Single statement only. Allow one optional trailing ';'.
    let body = upper.trim();
    let body = body.strip_suffix(';').unwrap_or(body).trim_end();
    if body.contains(';') {
        return Err("Only a single SQL statement is allowed".to_string());
    }
    if body.is_empty() {
        // Nothing left after removing comments/whitespace.
        return Err("SQL statement is empty".to_string());
    }

    let first_word = body.split_whitespace().next().unwrap_or("");

    if first_word == "ATTACH" || first_word == "DETACH" {
        return Err(format!("{first_word} statements are not allowed"));
    }

    // Block DDL outright. A repair escape hatch has no business changing the
    // schema, and `DROP`/`ALTER` are especially dangerous (e.g. `DROP TABLE`
    // cascades through `ON DELETE CASCADE`). SQLite has no `TRUNCATE`, but reject
    // it too so the error is explicit rather than a confusing parse failure.
    const BLOCKED_DDL: &[&str] = &["DROP", "ALTER", "TRUNCATE"];
    if BLOCKED_DDL.contains(&first_word) {
        return Err(format!("{first_word} (DDL) statements are not allowed"));
    }

    if !is_write {
        // The read path runs on a pooled connection guarded only by `PRAGMA
        // query_only`, which blocks data writes but not connection-scoped state
        // changes. A leaked `PRAGMA foreign_keys = OFF` or an open `BEGIN` would
        // ride the connection back into the pool and corrupt later borrowers, so
        // restrict reads (an allow list) to statement forms that produce rows and
        // touch no connection state. EXPLAIN is intentionally excluded: it would
        // let a disallowed inner statement (`EXPLAIN ATTACH ...`, `EXPLAIN DROP
        // ...`) slip past the leading-keyword guards above.
        const READ_VERBS: &[&str] = &["SELECT", "WITH", "VALUES"];
        if !READ_VERBS.contains(&first_word) {
            return Err(format!(
                "Only SELECT, WITH, or VALUES statements are allowed on the read path; \
                 got {first_word}. Set write=true for statements that modify data."
            ));
        }
    } else {
        // The write path runs inside a transaction. Connection-state pragmas and
        // transaction-control verbs have no business here (a `PRAGMA` would change
        // connection-scoped state, and `BEGIN`/`COMMIT`/etc. would corrupt the
        // surrounding transaction), so reject them with a deny list. Data writes
        // (INSERT/UPDATE/DELETE/REPLACE, CTE-prefixed or not) are unaffected.
        const WRITE_DENIED_VERBS: &[&str] = &[
            "PRAGMA",
            "BEGIN",
            "COMMIT",
            "END",
            "ROLLBACK",
            "SAVEPOINT",
            "RELEASE",
        ];
        if WRITE_DENIED_VERBS.contains(&first_word) {
            return Err(format!(
                "{first_word} statements are not allowed: PRAGMA and transaction-control \
                 statements cannot be run through admin SQL."
            ));
        }
    }

    if is_write && !allow_full_table {
        // Classify the statement's top-level verb at paren depth 0. A leading
        // `WITH` (CTE) puts its definitions in parentheses, so the real verb
        // still appears at depth 0 afterward.
        let depth0 = depth0_keywords(body);
        let is_update_or_delete = if first_word == "WITH" {
            depth0.iter().any(|w| *w == "UPDATE" || *w == "DELETE")
        } else {
            first_word == "UPDATE" || first_word == "DELETE"
        };
        // A WHERE anywhere satisfies the guard (intentionally permissive). String
        // literals were already blanked, so `SET note='WHERE'` does not count.
        let has_where = body
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .any(|w| w == "WHERE");
        if is_update_or_delete && !has_where {
            return Err("Refusing to run an UPDATE/DELETE with no WHERE clause. \
                 Pass allow_full_table=true to override."
                .to_string());
        }
    }

    Ok(())
}

/// Return a copy of `sql` with SQL string literals (`'...'`, `"..."`, including
/// doubled-quote escapes) and comments (`-- line`, `/* block */`) replaced by
/// spaces. Token boundaries are preserved, so the guard checks in
/// [`validate_statement`] see structure (`;`, `WHERE`, parentheses, the leading
/// keyword) without being fooled by characters inside strings or comments. This
/// is a lexer-level pass, not a full SQL parser; exotic quoting (backtick/bracket
/// identifiers) is left as-is.
fn strip_strings_and_comments(sql: &str) -> String {
    let chars: Vec<char> = sql.chars().collect();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // Line comment: blank to end of line (keep the newline).
        if c == '-' && chars.get(i + 1) == Some(&'-') {
            while i < chars.len() && chars[i] != '\n' {
                out.push(' ');
                i += 1;
            }
            continue;
        }
        // Block comment: blank through the closing `*/`.
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            out.push(' ');
            out.push(' ');
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                out.push(' ');
                i += 1;
            }
            if i < chars.len() {
                out.push(' ');
                out.push(' ');
                i += 2;
            }
            continue;
        }
        // String literal / quoted identifier: blank contents, honoring doubled
        // quotes (`''`, `""`) as escapes.
        if c == '\'' || c == '"' {
            let quote = c;
            out.push(' ');
            i += 1;
            while i < chars.len() {
                if chars[i] == quote {
                    if chars.get(i + 1) == Some(&quote) {
                        out.push(' ');
                        out.push(' ');
                        i += 2;
                        continue;
                    }
                    out.push(' ');
                    i += 1;
                    break;
                }
                out.push(' ');
                i += 1;
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Collect identifier-like keywords that occur at parenthesis depth 0 in an
/// already-uppercased statement. Tokens inside parentheses (subqueries, CTE
/// bodies, function args) are skipped, so the caller can find the statement's
/// top-level verb. This is a lexical heuristic and does not account for string
/// literals containing parentheses.
fn depth0_keywords(upper: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth: i32 = 0;
    let mut start: Option<usize> = None;
    for (i, c) in upper.char_indices() {
        let is_word = c.is_alphanumeric() || c == '_';
        if is_word && depth == 0 {
            start.get_or_insert(i);
            continue;
        }
        if let Some(s) = start.take() {
            out.push(&upper[s..i]);
        }
        match c {
            '(' => depth += 1,
            ')' => depth = (depth - 1).max(0),
            _ => {}
        }
    }
    if let Some(s) = start.take() {
        out.push(&upper[s..]);
    }
    out
}

/// Execute a read-only statement on a pooled connection with `PRAGMA query_only`.
///
/// `query_only = ON` makes the SQLite engine itself reject any write for the
/// duration of the query, independent of statement parsing. We deliberately use a
/// connection from the pool rather than opening a separate one: a fresh
/// connection would target a *different* database for `:memory:` setups (a bare
/// `:memory:` connection is private, and even a shared-cache in-memory database
/// is destroyed once the pool's last connection closes). The pragma is always
/// restored before the connection returns to the pool. Returns the result column
/// names (with duplicates suffixed, see [`dedupe_columns`]) and up to `limit`
/// rows as JSON objects keyed by those column names.
///
/// Errors are tagged ([`AdminSqlError`]): acquiring the connection and toggling
/// `query_only` are server-side failures ([`Internal`](AdminSqlError::Internal),
/// 500); a statement that fails to run is the caller's fault
/// ([`User`](AdminSqlError::User), 422).
pub async fn execute_read_only(
    pool: &SqlitePool,
    sql: &str,
    limit: usize,
) -> Result<(Vec<String>, Vec<serde_json::Map<String, Value>>), AdminSqlError> {
    let mut conn = pool
        .acquire()
        .await
        .map_err(|e| AdminSqlError::Internal(format!("Failed to acquire connection: {e}")))?;

    sqlx::query("PRAGMA query_only = ON")
        .execute(&mut *conn)
        .await
        .map_err(|e| AdminSqlError::Internal(format!("Failed to enter read-only mode: {e}")))?;

    // A failed query here is the caller's SQL at fault, not the server's.
    let result = read_rows(&mut conn, sql, limit)
        .await
        .map_err(AdminSqlError::User);

    // Always restore read-write before the connection goes back to the pool;
    // otherwise a later writer borrowing it would silently fail.
    if let Err(e) = sqlx::query("PRAGMA query_only = OFF")
        .execute(&mut *conn)
        .await
    {
        // Could not restore: discard the connection (detach so it is closed on
        // drop instead of returned to the pool stuck in read-only mode).
        drop(conn.detach());
        return Err(AdminSqlError::Internal(format!(
            "Failed to restore read-write mode: {e}"
        )));
    }

    result
}

/// Stream a query's rows into JSON objects, capped at `limit`. Column names are
/// taken from the first row and de-duplicated (see [`dedupe_columns`]); each row
/// becomes an object keyed by those names. Borrows the connection mutably so the
/// caller can restore connection state once the stream is dropped.
async fn read_rows(
    conn: &mut SqliteConnection,
    sql: &str,
    limit: usize,
) -> Result<(Vec<String>, Vec<serde_json::Map<String, Value>>), String> {
    use futures::TryStreamExt;

    let mut stream = sqlx::query(sql).fetch(&mut *conn);
    let mut columns: Vec<String> = Vec::new();
    let mut items: Vec<serde_json::Map<String, Value>> = Vec::new();

    while let Some(row) = stream
        .try_next()
        .await
        .map_err(|e| format!("Query failed: {e}"))?
    {
        if columns.is_empty() {
            let raw = row.columns().iter().map(|c| c.name().to_string()).collect();
            columns = dedupe_columns(raw);
        }
        if items.len() >= limit {
            break;
        }
        let mut obj = serde_json::Map::with_capacity(columns.len());
        for (i, name) in columns.iter().enumerate() {
            obj.insert(name.clone(), cell_to_json(&row, i));
        }
        items.push(obj);
    }

    Ok((columns, items))
}

/// Make column names unique by suffixing collisions (`id`, `id_2`, `id_3`, ...).
/// Arbitrary SQL (joins, `SELECT *`, `SELECT id, id`) can repeat a name, but each
/// result row is serialized as an object keyed by these names, so duplicates would
/// otherwise clobber each other. Result order is preserved. If a generated suffix
/// itself collides with another column, it keeps probing for a free name.
fn dedupe_columns(raw: Vec<String>) -> Vec<String> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(raw.len());
    for name in raw {
        if seen.insert(name.clone()) {
            out.push(name);
            continue;
        }
        let mut n = 2;
        let mut candidate = format!("{name}_{n}");
        while !seen.insert(candidate.clone()) {
            n += 1;
            candidate = format!("{name}_{n}");
        }
        out.push(candidate);
    }
    out
}

/// Attribution recorded alongside a committing write in `admin_audit_log`.
pub struct AuditContext<'a> {
    pub user_name: &'a str,
    pub allow_full_table: bool,
}

/// Execute a write statement inside a transaction.
///
/// Uses `BEGIN IMMEDIATE` (via [`begin_immediate`]) to match the server's write
/// convention.
///
/// - `dry_run`: roll back after capturing the affected-row count (preview). No
///   audit row is written.
/// - commit: the `admin_audit_log` row is inserted **in the same transaction** as
///   the change and they commit atomically, so a committed write always has a
///   durable audit entry. If the audit insert fails, the whole write is rolled
///   back and an error is returned.
///
/// Returns the number of rows affected. Failed statements (and failed
/// commit-audit attempts) are not audited here; the caller records those
/// best-effort via [`record_audit`], since no change persisted.
///
/// Errors are tagged ([`AdminSqlError`]): a statement that fails to run is the
/// caller's fault ([`User`](AdminSqlError::User), 422); transaction begin/commit,
/// rollback, and the atomic audit insert are server-side failures
/// ([`Internal`](AdminSqlError::Internal), 500).
pub async fn execute_write(
    pool: &SqlitePool,
    sql: &str,
    dry_run: bool,
    audit: &AuditContext<'_>,
) -> Result<i64, AdminSqlError> {
    let mut tx = begin_immediate(pool)
        .await
        .map_err(|e| AdminSqlError::Internal(format!("Failed to begin transaction: {e}")))?;

    let affected = match sqlx::query(sql).execute(&mut *tx).await {
        Ok(done) => done.rows_affected() as i64,
        Err(e) => {
            let _ = tx.rollback().await;
            return Err(AdminSqlError::User(format!("Statement failed: {e}")));
        }
    };

    if dry_run {
        tx.rollback()
            .await
            .map_err(|e| AdminSqlError::Internal(format!("Rollback failed: {e}")))?;
        return Ok(affected);
    }

    // Commit path: record the audit row atomically with the change so a committed
    // write can never lack a durable audit entry.
    if let Err(e) = insert_audit_row(
        &mut *tx,
        audit.user_name,
        sql,
        audit.allow_full_table,
        Some(affected),
        true,
        true,
        None,
    )
    .await
    {
        let _ = tx.rollback().await;
        return Err(AdminSqlError::Internal(format!(
            "Failed to record audit entry; write rolled back: {e}"
        )));
    }

    tx.commit()
        .await
        .map_err(|e| AdminSqlError::Internal(format!("Commit failed: {e}")))?;
    Ok(affected)
}

/// Record a non-committing write attempt in `admin_audit_log` (best effort).
///
/// Used only for writes that failed before committing (the statement errored, or
/// the atomic commit-audit was rolled back). No change persisted, so a missing
/// audit row here cannot hide a committed change; a failure to insert is logged
/// but does not fail the request. Committing writes are audited atomically inside
/// [`execute_write`] instead. Read-only queries and dry-run previews are not
/// audited.
#[allow(clippy::too_many_arguments)]
pub async fn record_audit(
    pool: &SqlitePool,
    user_name: &str,
    sql: &str,
    allow_full_table: bool,
    rows_affected: Option<i64>,
    committed: bool,
    success: bool,
    error: Option<&str>,
) {
    if let Err(e) = insert_audit_row(
        pool,
        user_name,
        sql,
        allow_full_table,
        rows_affected,
        committed,
        success,
        error,
    )
    .await
    {
        log::error!("Failed to write admin_audit_log entry: {e}");
    }
}

/// Insert one `admin_audit_log` row using the given executor (the shared pool for
/// best-effort failure records, or an open transaction for the atomic commit
/// record). All audited rows are writes (`is_write = 1`).
#[allow(clippy::too_many_arguments)]
async fn insert_audit_row<'e, E>(
    executor: E,
    user_name: &str,
    sql: &str,
    allow_full_table: bool,
    rows_affected: Option<i64>,
    committed: bool,
    success: bool,
    error: Option<&str>,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let now = Utc::now().timestamp_millis();
    sqlx::query(
        "INSERT INTO admin_audit_log \
         (user_name, timestamp, sql_text, is_write, allow_full_table, rows_affected, committed, success, error) \
         VALUES (?, ?, ?, 1, ?, ?, ?, ?, ?)",
    )
    .bind(user_name)
    .bind(now)
    .bind(sql)
    .bind(allow_full_table as i64)
    .bind(rows_affected)
    .bind(committed as i64)
    .bind(success as i64)
    .bind(error)
    .execute(executor)
    .await
    .map(|_| ())
}

/// Fetch a page of `admin_audit_log` rows (newest first) plus the total count.
///
/// Reads run on the shared pool: the audit log is append-only and admin-only, so
/// no read-only connection is needed here. Returns the page of typed entries and
/// the unpaginated total for pagination metadata.
pub async fn list_audit_log(
    pool: &SqlitePool,
    offset: i64,
    limit: i64,
) -> Result<(Vec<models::AdminAuditLogEntry>, i64), String> {
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_audit_log")
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to count audit log: {e}"))?;

    let rows = sqlx::query(
        "SELECT id, user_name, timestamp, sql_text, is_write, allow_full_table, \
         rows_affected, committed, success, error \
         FROM admin_audit_log ORDER BY timestamp DESC, id DESC LIMIT ? OFFSET ?",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to query audit log: {e}"))?;

    let items = rows
        .iter()
        .map(|row| models::AdminAuditLogEntry {
            id: row.get("id"),
            user_name: row.get("user_name"),
            timestamp: row.get("timestamp"),
            sql_text: row.get("sql_text"),
            is_write: row.get::<i64, _>("is_write") != 0,
            allow_full_table: row.get::<i64, _>("allow_full_table") != 0,
            rows_affected: row.get("rows_affected"),
            committed: row.get::<i64, _>("committed") != 0,
            success: row.get::<i64, _>("success") != 0,
            error: row.get("error"),
        })
        .collect();

    Ok((items, total))
}

/// Convert one SQLite cell to a JSON value based on its dynamic storage type.
fn cell_to_json(row: &sqlx::sqlite::SqliteRow, idx: usize) -> Value {
    let raw = match row.try_get_raw(idx) {
        Ok(raw) => raw,
        Err(_) => return Value::Null,
    };
    if raw.is_null() {
        return Value::Null;
    }

    match raw.type_info().name() {
        "INTEGER" | "INT" | "BIGINT" => row
            .try_get::<i64, _>(idx)
            .map(Value::from)
            .unwrap_or(Value::Null),
        "REAL" | "FLOAT" | "DOUBLE" => row
            .try_get::<f64, _>(idx)
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        "TEXT" => row
            .try_get::<String, _>(idx)
            .map(Value::from)
            .unwrap_or(Value::Null),
        "BLOB" => row
            .try_get::<Vec<u8>, _>(idx)
            .map(|bytes| Value::from(hex_encode(&bytes)))
            .unwrap_or(Value::Null),
        // Unknown/declared types: probe common Rust types in order.
        _ => {
            if let Ok(s) = row.try_get::<String, _>(idx) {
                Value::from(s)
            } else if let Ok(i) = row.try_get::<i64, _>(idx) {
                Value::from(i)
            } else if let Ok(f) = row.try_get::<f64, _>(idx) {
                serde_json::Number::from_f64(f)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            } else {
                Value::Null
            }
        }
    }
}

/// Lowercase hex encoding for BLOB cells.
fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_sql_error_tags_message_and_kind() {
        let user = AdminSqlError::User("bad sql".to_string());
        assert_eq!(user.message(), "bad sql");
        assert!(!user.is_internal());

        let internal = AdminSqlError::Internal("pool gone".to_string());
        assert_eq!(internal.message(), "pool gone");
        assert!(internal.is_internal());
    }

    #[test]
    fn rejects_empty_statement() {
        assert!(validate_statement("   ", false, false).is_err());
    }

    #[test]
    fn rejects_multiple_statements() {
        let err = validate_statement("SELECT 1; SELECT 2", false, false).unwrap_err();
        assert!(err.contains("single SQL statement"));
    }

    #[test]
    fn allows_single_trailing_semicolon() {
        assert!(validate_statement("SELECT 1;", false, false).is_ok());
    }

    #[test]
    fn rejects_attach_detach() {
        assert!(validate_statement("ATTACH DATABASE 'x' AS y", false, false).is_err());
        assert!(validate_statement("detach database y", false, false).is_err());
    }

    #[test]
    fn rejects_ddl_on_both_paths() {
        for stmt in [
            "DROP TABLE job",
            "drop index idx_foo",
            "ALTER TABLE job ADD COLUMN x INTEGER",
            "TRUNCATE TABLE job",
        ] {
            // Rejected regardless of the write flag or full-table override.
            assert!(validate_statement(stmt, false, false).is_err(), "{stmt}");
            assert!(validate_statement(stmt, true, true).is_err(), "{stmt}");
        }
    }

    #[test]
    fn leading_comments_do_not_hide_keyword() {
        // Line and block comments must not mask a disallowed leading keyword.
        assert!(validate_statement("-- harmless\nATTACH DATABASE 'x' AS y", false, false).is_err());
        assert!(validate_statement("/* note */ DROP TABLE job", false, false).is_err());
        // A statement that is only comments is treated as empty.
        assert!(validate_statement("-- just a comment", false, false).is_err());
        // Leading comments on an allowed statement are fine.
        assert!(validate_statement("-- pick one\nSELECT 1", false, false).is_ok());
    }

    #[test]
    fn no_where_guard_catches_cte_prefixed_writes() {
        // CTE-prefixed UPDATE/DELETE must be guarded like a bare one.
        assert!(
            validate_statement(
                "WITH c AS (SELECT id FROM job) DELETE FROM result",
                true,
                false
            )
            .is_err()
        );
        // A WHERE clause (even via the CTE join) satisfies the guard.
        assert!(
            validate_statement(
                "WITH c AS (SELECT id FROM job) DELETE FROM result WHERE id IN (SELECT id FROM c)",
                true,
                false
            )
            .is_ok()
        );
        // A DELETE that only appears inside a subquery does not trip the guard
        // for an otherwise-qualified statement.
        assert!(
            validate_statement(
                "WITH c AS (SELECT id FROM job) UPDATE result SET return_code=0 WHERE id=1",
                true,
                false
            )
            .is_ok()
        );
    }

    #[test]
    fn depth0_keywords_skips_parenthesized_tokens() {
        let kw = depth0_keywords("WITH C AS ( SELECT ID FROM JOB ) DELETE FROM RESULT");
        assert!(kw.contains(&"WITH"));
        assert!(kw.contains(&"DELETE"));
        // SELECT/ID/JOB live inside the CTE parentheses and must be skipped.
        assert!(!kw.contains(&"SELECT"));
        assert!(!kw.contains(&"JOB"));
    }

    #[test]
    fn no_where_guard_blocks_unqualified_write() {
        assert!(validate_statement("DELETE FROM result", true, false).is_err());
        assert!(validate_statement("UPDATE result SET return_code=0", true, false).is_err());
    }

    #[test]
    fn no_where_guard_allows_qualified_write() {
        assert!(validate_statement("DELETE FROM result WHERE id=1", true, false).is_ok());
        assert!(
            validate_statement("UPDATE result SET return_code=0 WHERE id=1", true, false).is_ok()
        );
    }

    #[test]
    fn no_where_guard_overridable() {
        assert!(validate_statement("DELETE FROM result", true, true).is_ok());
    }

    #[test]
    fn no_where_guard_does_not_apply_to_reads() {
        assert!(validate_statement("SELECT * FROM result", false, false).is_ok());
    }

    #[test]
    fn read_path_allows_row_producing_verbs() {
        for stmt in [
            "SELECT 1",
            "select * from job",
            "WITH c AS (SELECT id FROM job) SELECT * FROM c",
            "VALUES (1, 2)",
            "-- pick one\nSELECT 1",
        ] {
            assert!(validate_statement(stmt, false, false).is_ok(), "{stmt}");
        }
    }

    #[test]
    fn read_path_rejects_connection_state_statements() {
        // PRAGMA / transaction control could leak connection-scoped state (e.g.
        // foreign_keys=OFF, an open transaction) back into the pool. query_only
        // does not stop these, so validation must.
        for stmt in [
            "PRAGMA foreign_keys = OFF",
            "pragma table_info(job)",
            "BEGIN",
            "BEGIN IMMEDIATE",
            "COMMIT",
            "ROLLBACK",
            "SAVEPOINT s",
            "-- sneaky\nPRAGMA foreign_keys = OFF",
            // EXPLAIN is not on the read allow list: it must not be usable to
            // wrap an otherwise-blocked inner statement.
            "EXPLAIN SELECT 1",
            "EXPLAIN QUERY PLAN SELECT * FROM job",
            "EXPLAIN ATTACH DATABASE 'x' AS y",
            "EXPLAIN DROP TABLE job",
        ] {
            let err = validate_statement(stmt, false, false).unwrap_err();
            assert!(err.contains("read path"), "{stmt}: {err}");
        }
    }

    #[test]
    fn write_path_rejects_pragma_and_transaction_control() {
        // The write path runs inside a transaction; PRAGMA and transaction-control
        // verbs are rejected (a deny list) so they can't disturb it. allow_full_table
        // does not change this.
        for stmt in [
            "PRAGMA foreign_keys = OFF",
            "BEGIN",
            "BEGIN IMMEDIATE",
            "COMMIT",
            "END",
            "ROLLBACK",
            "SAVEPOINT s",
            "RELEASE s",
            "-- sneaky\nPRAGMA foreign_keys = OFF",
        ] {
            assert!(validate_statement(stmt, true, false).is_err(), "{stmt}");
            assert!(validate_statement(stmt, true, true).is_err(), "{stmt}");
        }
    }

    #[test]
    fn write_path_still_allows_data_writes() {
        // The read allow list is read-path-only; data writes (INSERT/UPDATE/DELETE)
        // and their guards are unaffected by the write-path deny list.
        assert!(validate_statement("INSERT INTO job (id) VALUES (1)", true, false).is_ok());
        assert!(validate_statement("DELETE FROM result WHERE id=1", true, false).is_ok());
    }

    #[test]
    fn clamp_limit_defaults_and_caps() {
        assert_eq!(clamp_limit(None), MAX_RECORD_TRANSFER_COUNT as usize);
        assert_eq!(clamp_limit(Some(0)), MAX_RECORD_TRANSFER_COUNT as usize);
        assert_eq!(clamp_limit(Some(-5)), MAX_RECORD_TRANSFER_COUNT as usize);
        assert_eq!(clamp_limit(Some(50)), 50);
        assert_eq!(
            clamp_limit(Some(MAX_RECORD_TRANSFER_COUNT + 100)),
            MAX_RECORD_TRANSFER_COUNT as usize
        );
    }

    #[test]
    fn where_in_string_literal_does_not_satisfy_guard() {
        // A `WHERE` token inside a string literal is not a real WHERE clause, so an
        // otherwise-unqualified UPDATE/DELETE is still blocked.
        assert!(validate_statement("UPDATE t SET note = 'WHERE'", true, false).is_err());
        assert!(
            validate_statement("DELETE FROM t WHERE note = 'no WHERE here'", true, false).is_ok()
        );
        // A real WHERE alongside a string containing the word is fine.
        assert!(
            validate_statement("UPDATE t SET note = 'WHERE' WHERE id = 1", true, false).is_ok()
        );
    }

    #[test]
    fn semicolon_inside_string_or_comment_is_not_multi_statement() {
        assert!(validate_statement("SELECT 'a;b' AS x", false, false).is_ok());
        assert!(validate_statement("SELECT 1 /* a;b */", false, false).is_ok());
    }

    #[test]
    fn strip_strings_and_comments_blanks_contents() {
        let out = strip_strings_and_comments("SELECT 'x;y' -- z;\n, \"w;v\"");
        assert!(
            !out.contains(';'),
            "semicolons inside string/comment survived: {out:?}"
        );
        assert!(out.to_uppercase().contains("SELECT"));
        // Doubled-quote escapes don't end the literal early.
        let out = strip_strings_and_comments("'a''b;c'");
        assert!(
            out.trim().is_empty(),
            "escaped-quote string not fully blanked: {out:?}"
        );
    }

    #[test]
    fn dedupe_columns_suffixes_collisions() {
        assert_eq!(
            dedupe_columns(vec!["id".into(), "id".into(), "name".into(), "id".into()]),
            vec!["id", "id_2", "name", "id_3"]
        );
        // A generated suffix that collides with a real column keeps probing.
        assert_eq!(
            dedupe_columns(vec!["id".into(), "id".into(), "id_2".into()]),
            vec!["id", "id_2", "id_2_2"]
        );
    }

    #[test]
    fn hex_encode_basic() {
        assert_eq!(hex_encode(&[0x00, 0x0f, 0xff]), "000fff");
    }
}
