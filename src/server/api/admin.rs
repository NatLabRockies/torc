//! Admin raw-SQL endpoint business logic.
//!
//! Backs `POST /admin/sql` (`torc admin sql`). This is a controlled, audit-logged
//! escape hatch for admins to inspect and surgically repair database state when a
//! bug leaves a workflow stuck. It is intentionally narrow:
//!
//! - Reads run on a dedicated [read-only connection](execute_read_only), so any
//!   write fails at the SQLite engine layer regardless of statement parsing.
//! - Writes run inside a transaction with an optional dry-run (rollback) preview
//!   and a no-WHERE guard, and every committing write is recorded in
//!   `admin_audit_log`.
//!
//! Raw writes bypass application invariants (the job status state machine, the
//! background unblocking contract); callers are warned to treat this as a last
//! resort.

use chrono::Utc;
use serde_json::Value;
use sqlx::sqlite::SqlitePool;
use sqlx::{Column, ConnectOptions, Row, TypeInfo, ValueRef};

use crate::models;
use crate::server::api::begin_immediate;

/// Default and hard maximum number of SELECT rows returned, matching the
/// standard list pagination cap.
pub const MAX_RESULT_ROWS: i64 = 10_000;

/// Clamp a caller-supplied row limit into `1..=MAX_RESULT_ROWS`.
pub fn clamp_limit(limit: Option<i64>) -> usize {
    match limit {
        Some(n) if n > 0 => n.min(MAX_RESULT_ROWS) as usize,
        _ => MAX_RESULT_ROWS as usize,
    }
}

/// Validate an admin SQL statement before execution.
///
/// Enforces the statement-level guardrails that apply regardless of the
/// read/write path:
/// - non-empty, single statement only (no `;`-separated batches)
/// - `ATTACH`/`DETACH` are rejected (they can read/write arbitrary files even on
///   a read-only connection)
///
/// For the write path, also rejects an unqualified `UPDATE`/`DELETE` (one with no
/// `WHERE` clause) unless `allow_full_table` is set. This is a deliberately simple
/// guard, not a full SQL parser: a `WHERE` inside a subquery can satisfy it.
///
/// Returns `Err(message)` describing a 422-class rejection.
pub fn validate_statement(sql: &str, is_write: bool, allow_full_table: bool) -> Result<(), String> {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return Err("SQL statement is empty".to_string());
    }

    // Single statement only. Allow one optional trailing ';'.
    let body = trimmed.strip_suffix(';').unwrap_or(trimmed);
    if body.contains(';') {
        return Err("Only a single SQL statement is allowed".to_string());
    }

    let upper = body.to_uppercase();
    let first_word = upper.split_whitespace().next().unwrap_or("");

    if first_word == "ATTACH" || first_word == "DETACH" {
        return Err(format!("{first_word} statements are not allowed"));
    }

    if is_write && !allow_full_table && (first_word == "UPDATE" || first_word == "DELETE") {
        // Tokenize on non-identifier characters so "WHERE(" still counts.
        let has_where = upper
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .any(|w| w == "WHERE");
        if !has_where {
            return Err(format!(
                "Refusing to run an unqualified {first_word} with no WHERE clause. \
                 Pass allow_full_table=true to override."
            ));
        }
    }

    Ok(())
}

/// Execute a read-only statement on a dedicated `SQLITE_OPEN_READONLY` connection.
///
/// Opening a separate read-only connection (derived from the pool's connect
/// options) means writes are rejected by the SQLite engine itself, and the
/// read-only state can never leak back into a pooled writer connection. Returns
/// the result column names and up to `limit` rows of JSON-encoded cell values.
pub async fn execute_read_only(
    pool: &SqlitePool,
    sql: &str,
    limit: usize,
) -> Result<(Vec<String>, Vec<Vec<Value>>), String> {
    use futures::TryStreamExt;

    let opts = pool.connect_options();
    let mut conn = opts
        .as_ref()
        .clone()
        .read_only(true)
        .connect()
        .await
        .map_err(|e| format!("Failed to open read-only connection: {e}"))?;

    let mut stream = sqlx::query(sql).fetch(&mut conn);
    let mut columns: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<Value>> = Vec::new();

    while let Some(row) = stream
        .try_next()
        .await
        .map_err(|e| format!("Query failed: {e}"))?
    {
        if columns.is_empty() {
            columns = row.columns().iter().map(|c| c.name().to_string()).collect();
        }
        if rows.len() >= limit {
            break;
        }
        let values = (0..row.columns().len())
            .map(|i| cell_to_json(&row, i))
            .collect();
        rows.push(values);
    }

    Ok((columns, rows))
}

/// Execute a write statement inside a transaction.
///
/// Uses `BEGIN IMMEDIATE` (via [`begin_immediate`]) to match the server's write
/// convention. When `dry_run` is true the transaction is rolled back after
/// capturing the affected-row count (preview); otherwise it is committed. Returns
/// the number of rows affected.
pub async fn execute_write(pool: &SqlitePool, sql: &str, dry_run: bool) -> Result<i64, String> {
    let mut tx = begin_immediate(pool)
        .await
        .map_err(|e| format!("Failed to begin transaction: {e}"))?;

    match sqlx::query(sql).execute(&mut *tx).await {
        Ok(done) => {
            let affected = done.rows_affected() as i64;
            if dry_run {
                tx.rollback()
                    .await
                    .map_err(|e| format!("Rollback failed: {e}"))?;
            } else {
                tx.commit()
                    .await
                    .map_err(|e| format!("Commit failed: {e}"))?;
            }
            Ok(affected)
        }
        Err(e) => {
            let _ = tx.rollback().await;
            Err(format!("Statement failed: {e}"))
        }
    }
}

/// Record one committing-write attempt in `admin_audit_log` (best effort).
///
/// Only the write path calls this, on both success and failure. Read-only queries
/// and dry-run previews are not audited. A failure to write the audit row is
/// logged but does not fail the request.
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
    let now = Utc::now().timestamp_millis();
    let result = sqlx::query(
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
    .execute(pool)
    .await;

    if let Err(e) = result {
        log::error!("Failed to write admin_audit_log entry: {e}");
    }
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
    fn clamp_limit_defaults_and_caps() {
        assert_eq!(clamp_limit(None), MAX_RESULT_ROWS as usize);
        assert_eq!(clamp_limit(Some(0)), MAX_RESULT_ROWS as usize);
        assert_eq!(clamp_limit(Some(-5)), MAX_RESULT_ROWS as usize);
        assert_eq!(clamp_limit(Some(50)), 50);
        assert_eq!(
            clamp_limit(Some(MAX_RESULT_ROWS + 100)),
            MAX_RESULT_ROWS as usize
        );
    }

    #[test]
    fn hex_encode_basic() {
        assert_eq!(hex_encode(&[0x00, 0x0f, 0xff]), "000fff");
    }
}
