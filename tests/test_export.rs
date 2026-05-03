#![cfg(feature = "server-bin")]
//! Integration tests for `torc-server export`.
//!
//! Each test seeds a freshly-migrated SQLite database with two users (alice,
//! bob), an access group linking both users' workflows, and a couple of jobs,
//! then drives `torc::server::export::run_export` directly and asserts on the
//! resulting file.

use sqlx::Connection;
use sqlx::SqliteConnection;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tempfile::TempDir;
use torc::server::export::{ExportOptions, Filter, run_export};

struct SeedIds {
    alice_workflow: i64,
    bob_workflow: i64,
    group: i64,
}

async fn setup_source_db(path: &Path) -> SqlitePool {
    let url = format!("sqlite:{}", path.display());
    let opts = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new().connect_with(opts).await.unwrap();
    sqlx::migrate!("./torc-server/migrations")
        .run(&pool)
        .await
        .expect("migrations");
    pool
}

async fn seed(pool: &SqlitePool) -> SeedIds {
    // Each workflow needs its own workflow_status row (FK).
    let s1: i64 = sqlx::query_scalar("INSERT INTO workflow_status DEFAULT VALUES RETURNING id")
        .fetch_one(pool)
        .await
        .unwrap();
    let s2: i64 = sqlx::query_scalar("INSERT INTO workflow_status DEFAULT VALUES RETURNING id")
        .fetch_one(pool)
        .await
        .unwrap();

    let alice: i64 = sqlx::query_scalar(
        "INSERT INTO workflow (name, user, timestamp, status_id) \
         VALUES ('alice-wf', 'alice', '2026-01-01', ?) RETURNING id",
    )
    .bind(s1)
    .fetch_one(pool)
    .await
    .unwrap();
    let bob: i64 = sqlx::query_scalar(
        "INSERT INTO workflow (name, user, timestamp, status_id) \
         VALUES ('bob-wf', 'bob', '2026-01-01', ?) RETURNING id",
    )
    .bind(s2)
    .fetch_one(pool)
    .await
    .unwrap();

    // Two jobs for alice, one for bob — exercises the cascade chain.
    sqlx::query("INSERT INTO job (workflow_id, name, command, status) VALUES (?, 'j1', 'echo', 0)")
        .bind(alice)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO job (workflow_id, name, command, status) VALUES (?, 'j2', 'echo', 0)")
        .bind(alice)
        .execute(pool)
        .await
        .unwrap();
    let bob_job: i64 = sqlx::query_scalar(
        "INSERT INTO job (workflow_id, name, command, status) \
         VALUES (?, 'j1', 'echo', 0) RETURNING id",
    )
    .bind(bob)
    .fetch_one(pool)
    .await
    .unwrap();

    // Seed rows in workflow-scoped tables added across multiple migration eras
    // so the cascade chain is verified end-to-end. If any of these tables ever
    // loses its ON DELETE CASCADE on workflow_id, the filtered export tests
    // will fail because bob's rows will survive the workflow DELETE.
    sqlx::query("INSERT INTO event (workflow_id, timestamp, data) VALUES (?, 0, '{}')")
        .bind(bob)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO failure_handler (workflow_id, name, rules) VALUES (?, 'h', '[]')")
        .bind(bob)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO ro_crate_entity (workflow_id, entity_id, entity_type, metadata) \
         VALUES (?, '#bob', 'Workflow', '{}')",
    )
    .bind(bob)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO slurm_stats (workflow_id, job_id, run_id) VALUES (?, ?, 1)")
        .bind(bob)
        .bind(bob_job)
        .execute(pool)
        .await
        .unwrap();

    let group: i64 =
        sqlx::query_scalar("INSERT INTO access_group (name) VALUES ('proj-x') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();
    for &wf in &[alice, bob] {
        sqlx::query("INSERT INTO workflow_access_group (workflow_id, group_id) VALUES (?, ?)")
            .bind(wf)
            .bind(group)
            .execute(pool)
            .await
            .unwrap();
    }
    for user in ["alice", "bob"] {
        sqlx::query("INSERT INTO user_group_membership (user_name, group_id) VALUES (?, ?)")
            .bind(user)
            .bind(group)
            .execute(pool)
            .await
            .unwrap();
    }

    SeedIds {
        alice_workflow: alice,
        bob_workflow: bob,
        group,
    }
}

async fn count(path: &Path, sql: &str) -> i64 {
    let url = format!("sqlite:{}", path.display());
    let mut c = SqliteConnection::connect_with(
        &SqliteConnectOptions::from_str(&url)
            .unwrap()
            .foreign_keys(true),
    )
    .await
    .unwrap();
    let n: i64 = sqlx::query_scalar(sql).fetch_one(&mut c).await.unwrap();
    let _ = c.close().await;
    n
}

async fn workflow_ids(path: &Path) -> Vec<i64> {
    let url = format!("sqlite:{}", path.display());
    let mut c = SqliteConnection::connect_with(
        &SqliteConnectOptions::from_str(&url)
            .unwrap()
            .foreign_keys(true),
    )
    .await
    .unwrap();
    let rows: Vec<(i64,)> = sqlx::query_as("SELECT id FROM workflow ORDER BY id")
        .fetch_all(&mut c)
        .await
        .unwrap();
    let _ = c.close().await;
    rows.into_iter().map(|(id,)| id).collect()
}

fn opts(src: &Path, out: PathBuf, filter: Filter) -> ExportOptions {
    ExportOptions {
        source_db_url: src.display().to_string(),
        output_path: out,
        filter,
        overwrite: false,
        preserve_access_groups: false,
        run_final_vacuum: true,
    }
}

async fn fresh_source(tmp: &TempDir) -> (PathBuf, SeedIds) {
    let src = tmp.path().join("src.db");
    let pool = setup_source_db(&src).await;
    let ids = seed(&pool).await;
    pool.close().await;
    (src, ids)
}

struct ExtendedIds {
    base: SeedIds,
    carol_workflow: i64,
    group_y: i64,
}

/// Like `fresh_source` but also seeds a third workflow owned by `carol` linked
/// to a separate access group `proj-y`. This gives the multi-value filter
/// tests three workflows split across two groups so unions and exclusions are
/// visible.
async fn fresh_source_extended(tmp: &TempDir) -> (PathBuf, ExtendedIds) {
    let src = tmp.path().join("src.db");
    let pool = setup_source_db(&src).await;
    let base = seed(&pool).await;

    let s3: i64 = sqlx::query_scalar("INSERT INTO workflow_status DEFAULT VALUES RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    let carol: i64 = sqlx::query_scalar(
        "INSERT INTO workflow (name, user, timestamp, status_id) \
         VALUES ('carol-wf', 'carol', '2026-01-01', ?) RETURNING id",
    )
    .bind(s3)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO job (workflow_id, name, command, status) VALUES (?, 'j1', 'echo', 0)")
        .bind(carol)
        .execute(&pool)
        .await
        .unwrap();
    let group_y: i64 =
        sqlx::query_scalar("INSERT INTO access_group (name) VALUES ('proj-y') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO workflow_access_group (workflow_id, group_id) VALUES (?, ?)")
        .bind(carol)
        .bind(group_y)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO user_group_membership (user_name, group_id) VALUES ('carol', ?)")
        .bind(group_y)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    (
        src,
        ExtendedIds {
            base,
            carol_workflow: carol,
            group_y,
        },
    )
}

fn sorted(mut v: Vec<i64>) -> Vec<i64> {
    v.sort();
    v
}

#[tokio::test]
async fn user_filter_preserves_ids_and_strips_acl_tables() {
    let tmp = TempDir::new().unwrap();
    let (src, ids) = fresh_source(&tmp).await;
    let out = tmp.path().join("alice.db");

    run_export(opts(&src, out.clone(), Filter::Users(vec!["alice".into()])))
        .await
        .expect("export");

    // Same workflow IDs as in the source — that's the headline guarantee.
    assert_eq!(workflow_ids(&out).await, vec![ids.alice_workflow]);

    // Cascade removed bob's job; alice's two jobs remain.
    assert_eq!(count(&out, "SELECT COUNT(*) FROM job").await, 2);

    // Cascade also removed bob's rows in every other workflow-scoped table.
    // Alice has no rows in these tables (the seed only populates them for
    // bob), so all four counts must be zero.
    for table in ["event", "failure_handler", "ro_crate_entity", "slurm_stats"] {
        assert_eq!(
            count(&out, &format!("SELECT COUNT(*) FROM {table}")).await,
            0,
            "{table} should be empty after bob's workflow was filtered out — \
             missing ON DELETE CASCADE on workflow_id?",
        );
    }

    // workflow_status has no cascade FK; the export prunes orphans explicitly
    // so bob's status row is not left behind to leak workflow counts.
    assert_eq!(count(&out, "SELECT COUNT(*) FROM workflow_status").await, 1);

    // ACL tables stripped by default in filtered mode.
    assert_eq!(count(&out, "SELECT COUNT(*) FROM access_group").await, 0);
    assert_eq!(
        count(&out, "SELECT COUNT(*) FROM user_group_membership").await,
        0
    );
    assert_eq!(
        count(&out, "SELECT COUNT(*) FROM workflow_access_group").await,
        0
    );
}

#[tokio::test]
async fn workflow_id_filter_keeps_only_listed_ids() {
    let tmp = TempDir::new().unwrap();
    let (src, ids) = fresh_source(&tmp).await;
    let out = tmp.path().join("just_bob.db");

    run_export(opts(
        &src,
        out.clone(),
        Filter::WorkflowIds(vec![ids.bob_workflow]),
    ))
    .await
    .expect("export");

    assert_eq!(workflow_ids(&out).await, vec![ids.bob_workflow]);
    assert_eq!(count(&out, "SELECT COUNT(*) FROM job").await, 1);
}

#[tokio::test]
async fn access_group_filter_returns_all_linked_workflows() {
    let tmp = TempDir::new().unwrap();
    let (src, ids) = fresh_source(&tmp).await;
    let out = tmp.path().join("group.db");

    run_export(opts(
        &src,
        out.clone(),
        Filter::AccessGroups(vec![ids.group]),
    ))
    .await
    .expect("export");

    let mut found = workflow_ids(&out).await;
    found.sort();
    let mut expected = vec![ids.alice_workflow, ids.bob_workflow];
    expected.sort();
    assert_eq!(found, expected);
}

#[tokio::test]
async fn full_copy_keeps_acl_tables() {
    let tmp = TempDir::new().unwrap();
    let (src, _) = fresh_source(&tmp).await;
    let out = tmp.path().join("full.db");

    run_export(opts(&src, out.clone(), Filter::None))
        .await
        .expect("export");

    assert_eq!(count(&out, "SELECT COUNT(*) FROM workflow").await, 2);
    // No filter applied, so ACL tables and workflow_status are preserved
    // verbatim — no orphan pruning runs in unfiltered mode.
    assert_eq!(count(&out, "SELECT COUNT(*) FROM access_group").await, 1);
    assert_eq!(
        count(&out, "SELECT COUNT(*) FROM user_group_membership").await,
        2
    );
    assert_eq!(
        count(&out, "SELECT COUNT(*) FROM workflow_access_group").await,
        2
    );
    assert_eq!(count(&out, "SELECT COUNT(*) FROM workflow_status").await, 2);
}

#[tokio::test]
async fn preserve_access_groups_flag_keeps_acl_tables_in_filtered_export() {
    let tmp = TempDir::new().unwrap();
    let (src, ids) = fresh_source(&tmp).await;
    let out = tmp.path().join("alice_preserved.db");

    let mut o = opts(&src, out.clone(), Filter::Users(vec!["alice".into()]));
    o.preserve_access_groups = true;
    run_export(o).await.expect("export");

    assert_eq!(workflow_ids(&out).await, vec![ids.alice_workflow]);
    assert_eq!(count(&out, "SELECT COUNT(*) FROM access_group").await, 1);
    assert_eq!(
        count(&out, "SELECT COUNT(*) FROM user_group_membership").await,
        2
    );
}

#[tokio::test]
async fn empty_filter_result_errors_and_removes_partial_output() {
    let tmp = TempDir::new().unwrap();
    let (src, _) = fresh_source(&tmp).await;
    let out = tmp.path().join("nobody.db");

    let err = run_export(opts(
        &src,
        out.clone(),
        Filter::Users(vec!["nobody-here".into()]),
    ))
    .await
    .expect_err("expected error for empty filter result");
    assert!(
        err.to_string().contains("no workflows"),
        "unexpected error: {err}"
    );
    assert!(
        !out.exists(),
        "partial export file should be removed on empty result"
    );
}

#[tokio::test]
async fn refuses_to_overwrite_existing_file_without_flag() {
    let tmp = TempDir::new().unwrap();
    let (src, _) = fresh_source(&tmp).await;
    let out = tmp.path().join("first.db");

    run_export(opts(&src, out.clone(), Filter::None))
        .await
        .expect("first export");

    // Second run without --overwrite should fail.
    let err = run_export(opts(&src, out.clone(), Filter::None))
        .await
        .expect_err("expected refusal to overwrite");
    assert!(err.to_string().contains("--overwrite"), "got: {err}");

    // With overwrite=true it succeeds.
    let mut o = opts(&src, out.clone(), Filter::None);
    o.overwrite = true;
    run_export(o).await.expect("overwrite");
    assert_eq!(count(&out, "SELECT COUNT(*) FROM workflow").await, 2);
}

// --- Multi-value filter variants -------------------------------------------
//
// The single-value tests above exercise `IN (?)` with one bound parameter;
// these tests exercise the multi-bind / comma-joined-id paths and confirm the
// filters behave as a union (OR) rather than an intersection.

#[tokio::test]
async fn multi_user_filter_keeps_all_listed_users() {
    let tmp = TempDir::new().unwrap();
    let (src, ext) = fresh_source_extended(&tmp).await;
    let out = tmp.path().join("alice_carol.db");

    run_export(opts(
        &src,
        out.clone(),
        Filter::Users(vec!["alice".into(), "carol".into()]),
    ))
    .await
    .expect("export");

    assert_eq!(
        sorted(workflow_ids(&out).await),
        sorted(vec![ext.base.alice_workflow, ext.carol_workflow]),
    );
    // bob's job (1) dropped; alice's two + carol's one remain.
    assert_eq!(count(&out, "SELECT COUNT(*) FROM job").await, 3);
}

#[tokio::test]
async fn multi_workflow_id_filter_keeps_all_listed_ids() {
    let tmp = TempDir::new().unwrap();
    let (src, ext) = fresh_source_extended(&tmp).await;
    let out = tmp.path().join("alice_and_carol_ids.db");

    run_export(opts(
        &src,
        out.clone(),
        Filter::WorkflowIds(vec![ext.base.alice_workflow, ext.carol_workflow]),
    ))
    .await
    .expect("export");

    assert_eq!(
        sorted(workflow_ids(&out).await),
        sorted(vec![ext.base.alice_workflow, ext.carol_workflow]),
    );
}

#[tokio::test]
async fn multi_workflow_id_filter_ignores_nonexistent_ids() {
    let tmp = TempDir::new().unwrap();
    let (src, ext) = fresh_source_extended(&tmp).await;
    let out = tmp.path().join("alice_plus_bogus.db");

    // alice + a non-existent ID — should keep alice and silently ignore the bogus one.
    run_export(opts(
        &src,
        out.clone(),
        Filter::WorkflowIds(vec![ext.base.alice_workflow, 999_999]),
    ))
    .await
    .expect("export");

    assert_eq!(workflow_ids(&out).await, vec![ext.base.alice_workflow]);
}

#[tokio::test]
async fn multi_access_group_filter_returns_union() {
    let tmp = TempDir::new().unwrap();
    let (src, ext) = fresh_source_extended(&tmp).await;
    let out = tmp.path().join("both_groups.db");

    // proj-x links alice + bob; proj-y links carol. Filtering by both groups
    // must return all three workflows.
    run_export(opts(
        &src,
        out.clone(),
        Filter::AccessGroups(vec![ext.base.group, ext.group_y]),
    ))
    .await
    .expect("export");

    assert_eq!(
        sorted(workflow_ids(&out).await),
        sorted(vec![
            ext.base.alice_workflow,
            ext.base.bob_workflow,
            ext.carol_workflow,
        ]),
    );
}

#[tokio::test]
async fn single_access_group_excludes_workflows_not_in_that_group() {
    let tmp = TempDir::new().unwrap();
    let (src, ext) = fresh_source_extended(&tmp).await;
    let out = tmp.path().join("just_proj_y.db");

    // proj-y has only carol. alice and bob (proj-x only) must be dropped.
    run_export(opts(
        &src,
        out.clone(),
        Filter::AccessGroups(vec![ext.group_y]),
    ))
    .await
    .expect("export");

    assert_eq!(workflow_ids(&out).await, vec![ext.carol_workflow]);
}
