mod common;

use serde_json::{Value, json};
use serial_test::serial;

// Integration tests for the admin raw-SQL endpoint (`POST /admin/sql`,
// `torc admin sql`). `owner` is an admin user and `dave` is a non-admin in
// `start_server_with_required_auth`; both share the test password.

const PASSWORD: &str = "correct horse battery staple";

fn http_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("blocking client")
}

/// POST a body to `/admin/sql` as `user` and return (status, parsed JSON body).
fn post_sql(base_path: &str, user: &str, body: Value) -> (u16, Value) {
    let url = format!("{base_path}/admin/sql");
    let resp = http_client()
        .post(&url)
        .basic_auth(user, Some(PASSWORD))
        .json(&body)
        .send()
        .expect("request sent");
    let status = resp.status().as_u16();
    let json: Value = resp.json().unwrap_or(Value::Null);
    (status, json)
}

/// GET `/admin/audit-log{query}` as `user` and return (status, parsed JSON body).
fn get_audit_log(base_path: &str, user: &str, query: &str) -> (u16, Value) {
    let url = format!("{base_path}/admin/audit-log{query}");
    let resp = http_client()
        .get(&url)
        .basic_auth(user, Some(PASSWORD))
        .send()
        .expect("request sent");
    let status = resp.status().as_u16();
    let json: Value = resp.json().unwrap_or(Value::Null);
    (status, json)
}

#[test]
#[serial(auth)]
fn non_admin_is_forbidden() {
    let server = common::start_server_with_required_auth();
    let (status, _) = post_sql(
        &server.config.base_path,
        "dave",
        json!({ "sql": "SELECT 1" }),
    );
    assert_eq!(status, 403, "non-admin should be forbidden from /admin/sql");
}

#[test]
#[serial(auth)]
fn read_only_select_returns_columns_and_rows() {
    let server = common::start_server_with_required_auth();
    let (status, body) = post_sql(
        &server.config.base_path,
        "owner",
        json!({ "sql": "SELECT 7 AS seven, 'hi' AS greeting" }),
    );
    assert_eq!(status, 200, "admin SELECT should succeed: {body}");
    assert_eq!(body["columns"], json!(["seven", "greeting"]));
    assert_eq!(body["rows"], json!([[7, "hi"]]));
    assert_eq!(body["committed"], json!(false));
}

#[test]
#[serial(auth)]
fn read_path_rejects_writes() {
    let server = common::start_server_with_required_auth();
    // Without write=true the statement runs on a read-only connection, so SQLite
    // rejects the write at the engine layer.
    let (status, _) = post_sql(
        &server.config.base_path,
        "owner",
        json!({ "sql": "CREATE TABLE t_should_not_exist (id INTEGER)" }),
    );
    assert_eq!(status, 422, "write on the read-only path must be rejected");
}

#[test]
#[serial(auth)]
fn attach_and_multi_statement_rejected() {
    let server = common::start_server_with_required_auth();
    let base = &server.config.base_path;

    let (status, _) = post_sql(base, "owner", json!({ "sql": "ATTACH DATABASE 'x' AS y" }));
    assert_eq!(status, 422, "ATTACH must be rejected");

    let (status, _) = post_sql(base, "owner", json!({ "sql": "SELECT 1; SELECT 2" }));
    assert_eq!(status, 422, "multi-statement input must be rejected");
}

#[test]
#[serial(auth)]
fn write_with_where_commits_and_is_audited() {
    let server = common::start_server_with_required_auth();
    let base = &server.config.base_path;

    // Scratch table (CREATE has no WHERE guard).
    let (status, _) = post_sql(
        base,
        "owner",
        json!({ "sql": "CREATE TABLE t_scratch (id INTEGER, v INTEGER)", "write": true }),
    );
    assert_eq!(status, 200);

    let (status, body) = post_sql(
        base,
        "owner",
        json!({ "sql": "INSERT INTO t_scratch (id, v) VALUES (1, 10), (2, 20)", "write": true }),
    );
    assert_eq!(status, 200);
    assert_eq!(body["rows_affected"], json!(2));
    assert_eq!(body["committed"], json!(true));

    // Qualified UPDATE commits.
    let (status, body) = post_sql(
        base,
        "owner",
        json!({ "sql": "UPDATE t_scratch SET v = 0 WHERE id = 1", "write": true }),
    );
    assert_eq!(status, 200);
    assert_eq!(body["rows_affected"], json!(1));
    assert_eq!(body["committed"], json!(true));

    // Confirm the change persisted.
    let (status, body) = post_sql(
        base,
        "owner",
        json!({ "sql": "SELECT v FROM t_scratch WHERE id = 1" }),
    );
    assert_eq!(status, 200);
    assert_eq!(body["rows"], json!([[0]]));

    // The committing UPDATE was recorded in the durable audit log.
    let (status, body) = post_sql(
        base,
        "owner",
        json!({ "sql": "SELECT COUNT(*) AS n FROM admin_audit_log WHERE sql_text LIKE 'UPDATE t_scratch%'" }),
    );
    assert_eq!(status, 200);
    assert_eq!(body["rows"][0][0], json!(1), "audit row expected: {body}");
}

#[test]
#[serial(auth)]
fn unqualified_write_blocked_unless_allowed() {
    let server = common::start_server_with_required_auth();
    let base = &server.config.base_path;

    post_sql(
        base,
        "owner",
        json!({ "sql": "CREATE TABLE t_full (id INTEGER)", "write": true }),
    );
    post_sql(
        base,
        "owner",
        json!({ "sql": "INSERT INTO t_full (id) VALUES (1), (2)", "write": true }),
    );

    // No WHERE clause: rejected by default.
    let (status, _) = post_sql(
        base,
        "owner",
        json!({ "sql": "DELETE FROM t_full", "write": true }),
    );
    assert_eq!(status, 422, "unqualified DELETE must be blocked");

    // Still present.
    let (_, body) = post_sql(
        base,
        "owner",
        json!({ "sql": "SELECT COUNT(*) FROM t_full" }),
    );
    assert_eq!(body["rows"][0][0], json!(2));

    // Explicit override succeeds.
    let (status, body) = post_sql(
        base,
        "owner",
        json!({ "sql": "DELETE FROM t_full", "write": true, "allow_full_table": true }),
    );
    assert_eq!(status, 200);
    assert_eq!(body["rows_affected"], json!(2));
}

#[test]
#[serial(auth)]
fn dry_run_does_not_persist() {
    let server = common::start_server_with_required_auth();
    let base = &server.config.base_path;

    post_sql(
        base,
        "owner",
        json!({ "sql": "CREATE TABLE t_dry (id INTEGER)", "write": true }),
    );
    post_sql(
        base,
        "owner",
        json!({ "sql": "INSERT INTO t_dry (id) VALUES (1)", "write": true }),
    );

    // Dry-run reports the affected count but rolls back.
    let (status, body) = post_sql(
        base,
        "owner",
        json!({ "sql": "INSERT INTO t_dry (id) VALUES (2)", "write": true, "dry_run": true }),
    );
    assert_eq!(status, 200);
    assert_eq!(body["rows_affected"], json!(1));
    assert_eq!(body["committed"], json!(false));

    // Row count unchanged.
    let (_, body) = post_sql(
        base,
        "owner",
        json!({ "sql": "SELECT COUNT(*) FROM t_dry" }),
    );
    assert_eq!(body["rows"][0][0], json!(1), "dry-run must not persist");

    // And the dry-run statement itself was not written to the audit log (the
    // earlier committed inserts are audited, so match the dry-run text exactly).
    let (_, body) = post_sql(
        base,
        "owner",
        json!({ "sql": "SELECT COUNT(*) AS n FROM admin_audit_log WHERE sql_text = 'INSERT INTO t_dry (id) VALUES (2)'" }),
    );
    assert_eq!(body["rows"][0][0], json!(0));
}

#[test]
#[serial(auth)]
fn audit_log_list_requires_admin() {
    let server = common::start_server_with_required_auth();
    let (status, _) = get_audit_log(&server.config.base_path, "dave", "");
    assert_eq!(
        status, 403,
        "non-admin should be forbidden from /admin/audit-log"
    );
}

#[test]
#[serial(auth)]
fn audit_log_list_returns_committed_writes_newest_first() {
    let server = common::start_server_with_required_auth();
    let base = &server.config.base_path;

    // Two committing writes -> two audit rows (CREATE then INSERT).
    post_sql(
        base,
        "owner",
        json!({ "sql": "CREATE TABLE t_audit (id INTEGER)", "write": true }),
    );
    post_sql(
        base,
        "owner",
        json!({ "sql": "INSERT INTO t_audit (id) VALUES (1)", "write": true }),
    );

    let (status, body) = get_audit_log(base, "owner", "");
    assert_eq!(status, 200, "admin list should succeed: {body}");

    let items = body["items"].as_array().expect("items array");
    assert_eq!(items.len(), 2, "expected exactly two audit rows: {body}");
    assert_eq!(body["count"], json!(2));
    assert_eq!(body["total_count"], json!(2));
    assert_eq!(body["offset"], json!(0));
    assert_eq!(body["has_more"], json!(false));

    // Newest first: the INSERT was the most recent committing write.
    let newest = &items[0];
    assert_eq!(
        newest["sql_text"],
        json!("INSERT INTO t_audit (id) VALUES (1)")
    );
    assert_eq!(newest["committed"], json!(true));
    assert_eq!(newest["success"], json!(true));
    assert_eq!(newest["is_write"], json!(true));
    assert_eq!(newest["user_name"], json!("owner"));
    assert!(
        newest["timestamp"].as_i64().unwrap_or(0) > 0,
        "timestamp should be a positive epoch-ms value: {body}"
    );
}

#[test]
#[serial(auth)]
fn audit_log_list_paginates() {
    let server = common::start_server_with_required_auth();
    let base = &server.config.base_path;

    // Three committing writes -> three audit rows.
    post_sql(
        base,
        "owner",
        json!({ "sql": "CREATE TABLE t_page (id INTEGER)", "write": true }),
    );
    post_sql(
        base,
        "owner",
        json!({ "sql": "INSERT INTO t_page (id) VALUES (1)", "write": true }),
    );
    post_sql(
        base,
        "owner",
        json!({ "sql": "INSERT INTO t_page (id) VALUES (2)", "write": true }),
    );

    // First page of one: more remain.
    let (status, body) = get_audit_log(base, "owner", "?limit=1");
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["count"], json!(1));
    assert_eq!(body["total_count"], json!(3));
    assert_eq!(body["has_more"], json!(true));

    // Final page: offset past the last row clears has_more.
    let (status, body) = get_audit_log(base, "owner", "?limit=1&offset=2");
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["count"], json!(1));
    assert_eq!(body["offset"], json!(2));
    assert_eq!(body["has_more"], json!(false));
}

#[test]
#[serial(auth)]
fn disable_admin_sql_blocks_reads_and_writes_but_keeps_audit_log() {
    // Whole feature off: reads and writes are 403, but the audit-log listing
    // stays available so past activity can still be reviewed.
    let server = common::start_server_with_required_auth_and_args(&["--disable-admin-sql"]);
    let base = &server.server.config.base_path;

    let (status, _) = post_sql(base, "owner", json!({ "sql": "SELECT 1" }));
    assert_eq!(status, 403, "reads should be disabled");

    let (status, _) = post_sql(
        base,
        "owner",
        json!({ "sql": "UPDATE result SET return_code=0 WHERE id=1", "write": true }),
    );
    assert_eq!(status, 403, "writes should be disabled");

    let (status, body) = get_audit_log(base, "owner", "");
    assert_eq!(
        status, 200,
        "audit-log listing must remain available: {body}"
    );
}

#[test]
#[serial(auth)]
fn disable_admin_sql_writes_allows_reads_blocks_writes() {
    // Writes off, reads on.
    let server = common::start_server_with_required_auth_and_args(&["--disable-admin-sql-writes"]);
    let base = &server.server.config.base_path;

    let (status, body) = post_sql(base, "owner", json!({ "sql": "SELECT 1 AS one" }));
    assert_eq!(status, 200, "reads should still work: {body}");

    // Even a dry-run write is rejected (the CLI previews via dry-run first).
    let (status, _) = post_sql(
        base,
        "owner",
        json!({
            "sql": "UPDATE result SET return_code=0 WHERE id=1",
            "write": true,
            "dry_run": true
        }),
    );
    assert_eq!(status, 403, "writes should be disabled");
}
