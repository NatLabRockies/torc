//! `torc-server export` implementation.
//!
//! Produces a standalone SQLite copy of the live torc database, optionally
//! filtered to a subset of workflows. Workflow IDs and all per-workflow rows
//! are preserved verbatim, so log files referring to the original IDs remain
//! interpretable in the exported database — the primary use case is handing a
//! debugging copy to a user analyzing their workflows with external tools
//! (datasight, sqlite3, etc.).
//!
//! Strategy:
//! 1. `VACUUM INTO '<output>'` — produces a transactionally consistent,
//!    defragmented snapshot without quiescing the source server. A separate
//!    SQLite connection participates in the source's WAL coherency (it
//!    opens the `-wal` and `-shm` files), so the snapshot reflects every
//!    committed transaction the running server can see.
//! 2. Open the snapshot, `PRAGMA foreign_keys=ON`, and (if a filter was
//!    requested) `DELETE FROM workflow WHERE id NOT IN (<filter>)`. Every
//!    per-workflow table has `ON DELETE CASCADE` on `workflow_id`, so the
//!    cascade chain cleans up jobs, files, results, events, etc. for the
//!    rows we just deleted.
//! 3. Sweep pre-existing orphans, regardless of whether a filter was
//!    applied. The source DB can carry rows whose `workflow_id` already
//!    pointed at a missing workflow before the export ran — typically from
//!    a `delete_workflow` code path that bypasses cascade by toggling
//!    `PRAGMA foreign_keys = OFF`, or a bare `sqlite3` CLI session (the
//!    CLI defaults to FKs off). Cascade only fires for parent rows we
//!    actually delete, so orphans survive verbatim. Iteratively run
//!    `PRAGMA foreign_key_check` and delete every reported violation until
//!    none remain. Then explicitly prune `workflow_status` (whose
//!    back-reference column has no FK declared and so is invisible to
//!    `foreign_key_check`). Both steps run in unfiltered mode too — FK
//!    violations are data corruption, not "fidelity to the source."
//! 4. If a filter was applied and `preserve_access_groups` is false, wipe
//!    `user_group_membership` (which has no per-workflow scoping and would
//!    leak unrelated users' group affiliations) and `access_group` (which
//!    cascades the `workflow_access_group` join table).
//! 5. Optional final `VACUUM` to reclaim space freed by the deletes.
//!
//! If anything in steps 2–5 fails after `VACUUM INTO` has written the file,
//! the partial output is removed before the error propagates so callers
//! can't mistake it for a valid export.

use anyhow::{Context, Result, anyhow, bail};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;
use tracing::info;

#[derive(Debug, Clone)]
pub enum Filter {
    None,
    Users(Vec<String>),
    AccessGroups(Vec<i64>),
    WorkflowIds(Vec<i64>),
}

#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// Source database URL or bare path. `sqlite:` prefix is added if missing.
    pub source_db_url: String,
    pub output_path: PathBuf,
    pub filter: Filter,
    pub overwrite: bool,
    pub preserve_access_groups: bool,
    pub run_final_vacuum: bool,
}

pub async fn run_export(opts: ExportOptions) -> Result<()> {
    let source_url = normalize_db_url(&opts.source_db_url);

    refuse_self_overwrite(&source_url, &opts.output_path)?;
    prepare_output_path(&opts.output_path, opts.overwrite)?;

    let mut src = SqliteConnection::connect_with(
        &SqliteConnectOptions::from_str(&source_url)?
            .create_if_missing(false)
            .busy_timeout(Duration::from_secs(60)),
    )
    .await
    .with_context(|| format!("opening source database at {source_url}"))?;

    let target = opts
        .output_path
        .to_str()
        .ok_or_else(|| anyhow!("output path is not valid UTF-8"))?;
    info!("Snapshotting database to {}", target);
    // VACUUM INTO requires a string literal, not a bound parameter. Escape
    // single quotes per the SQL standard (' → '').
    let escaped = target.replace('\'', "''");
    sqlx::query(&format!("VACUUM INTO '{escaped}'"))
        .execute(&mut src)
        .await
        .context("VACUUM INTO")?;
    let _ = src.close().await;

    // Once VACUUM INTO writes the snapshot, any subsequent failure must clean
    // up the partial output file so a caller can't mistake it for a valid
    // export.
    match finalize_snapshot(&opts).await {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = std::fs::remove_file(&opts.output_path);
            Err(err)
        }
    }
}

async fn finalize_snapshot(opts: &ExportOptions) -> Result<()> {
    let target = opts
        .output_path
        .to_str()
        .ok_or_else(|| anyhow!("output path is not valid UTF-8"))?;
    let dest_url = format!("sqlite:{target}");
    let mut dst = SqliteConnection::connect_with(
        &SqliteConnectOptions::from_str(&dest_url)?
            .foreign_keys(true)
            .create_if_missing(false)
            .busy_timeout(Duration::from_secs(60)),
    )
    .await
    .with_context(|| format!("opening output database at {dest_url}"))?;

    let filter_applied = !matches!(opts.filter, Filter::None);
    if filter_applied {
        apply_workflow_filter(&mut dst, &opts.filter).await?;
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workflow")
            .fetch_one(&mut dst)
            .await?;
        if remaining == 0 {
            bail!("filter matched no workflows; refusing to write empty export");
        }
        info!("Retained {remaining} workflow(s) after filter");
    }

    // Always sweep FK orphans and orphan workflow_status rows, regardless of
    // filter mode. Cascade only fires for parent rows the filter deleted;
    // pre-existing orphans (from any code path that ran with foreign_keys=OFF
    // — including the bare sqlite3 CLI, which defaults to OFF) survive
    // VACUUM INTO and are data corruption, not fidelity to the source.
    prune_orphans(&mut dst).await?;
    // workflow_status has no FK declared back to workflow (the back-reference
    // column was added via ALTER TABLE ADD COLUMN, which SQLite cannot extend
    // with FK constraints), so foreign_key_check doesn't see its orphans.
    sqlx::query("DELETE FROM workflow_status WHERE id NOT IN (SELECT status_id FROM workflow)")
        .execute(&mut dst)
        .await
        .context("DELETE FROM workflow_status (orphan prune)")?;

    if filter_applied && !opts.preserve_access_groups {
        info!(
            "Stripping access_group / user_group_membership (use --preserve-access-groups to keep)"
        );
        sqlx::query("DELETE FROM user_group_membership")
            .execute(&mut dst)
            .await
            .context("DELETE FROM user_group_membership")?;
        // access_group cascades workflow_access_group via ON DELETE CASCADE.
        sqlx::query("DELETE FROM access_group")
            .execute(&mut dst)
            .await
            .context("DELETE FROM access_group")?;
    }

    if opts.run_final_vacuum {
        info!("Running final VACUUM");
        sqlx::query("VACUUM")
            .execute(&mut dst)
            .await
            .context("VACUUM")?;
    }

    let workflow_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workflow")
        .fetch_one(&mut dst)
        .await?;
    let job_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM job")
        .fetch_one(&mut dst)
        .await?;
    let _ = dst.close().await;

    let bytes = std::fs::metadata(&opts.output_path)
        .map(|m| m.len())
        .unwrap_or(0);
    info!(
        "Wrote {} ({} workflow(s), {} job(s), {:.2} MiB)",
        opts.output_path.display(),
        workflow_count,
        job_count,
        bytes as f64 / (1024.0 * 1024.0)
    );
    Ok(())
}

/// Reject `--output` paths that resolve to the same filesystem location as the
/// source database. Without this check, a slip like
/// `torc-server export --database prod.db --output prod.db --overwrite` would
/// trigger `prepare_output_path` to `remove_file(prod.db)` before the export
/// even opens the source — turning a typo into permanent data loss.
///
/// Comparison is by canonical filesystem path so symlinks, relative paths, and
/// the various `sqlite:` URL prefixes all collapse to the same target. If the
/// source URL is in-memory or refers to a path that doesn't exist on disk
/// (which would fail later anyway), this check abstains.
fn refuse_self_overwrite(source_url: &str, output: &Path) -> Result<()> {
    let raw_source = strip_sqlite_url_prefix(source_url);
    if raw_source == ":memory:" || raw_source.contains("file::memory:") {
        return Ok(());
    }
    let source_canonical = match Path::new(raw_source).canonicalize() {
        Ok(p) => p,
        // Source path doesn't resolve — opening it for VACUUM INTO will fail
        // with a clearer error than anything we'd produce here.
        Err(_) => return Ok(()),
    };
    let output_canonical = canonicalize_with_missing_file(output)?;
    if source_canonical == output_canonical {
        bail!(
            "refusing to overwrite the source database with itself ({})",
            source_canonical.display()
        );
    }
    Ok(())
}

/// Canonicalize a path that may not exist yet by canonicalizing its parent
/// directory and rejoining the file name. The parent must exist — that's a
/// reasonable precondition for a writeable output location.
fn canonicalize_with_missing_file(path: &Path) -> Result<PathBuf> {
    if let Ok(p) = path.canonicalize() {
        return Ok(p);
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let parent_canonical = parent
        .canonicalize()
        .with_context(|| format!("canonicalizing output directory {}", parent.display()))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow!("output path has no file name"))?;
    Ok(parent_canonical.join(file_name))
}

/// Strip whichever `sqlite:` URL form is in front of the path. Handles the
/// bare `sqlite:` form (e.g. `sqlite:relative.db`) and the URL-style
/// `sqlite://` (e.g. `sqlite:///abs/path.db`).
fn strip_sqlite_url_prefix(s: &str) -> &str {
    s.strip_prefix("sqlite://")
        .or_else(|| s.strip_prefix("sqlite:"))
        .unwrap_or(s)
}

fn prepare_output_path(path: &PathBuf, overwrite: bool) -> Result<()> {
    if path.exists() {
        if !overwrite {
            bail!(
                "output file {} already exists (use --overwrite to replace)",
                path.display()
            );
        }
        std::fs::remove_file(path)
            .with_context(|| format!("removing existing {}", path.display()))?;
    }
    // SQLite refuses to write VACUUM INTO if a stale -journal/-wal/-shm sidecar
    // is present alongside the target.
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut p = path.clone().into_os_string();
        p.push(suffix);
        let p: PathBuf = p.into();
        if p.exists() {
            std::fs::remove_file(&p).with_context(|| format!("removing {}", p.display()))?;
        }
    }
    Ok(())
}

async fn apply_workflow_filter(dst: &mut SqliteConnection, filter: &Filter) -> Result<()> {
    match filter {
        Filter::None => Ok(()),
        Filter::Users(users) => {
            let placeholders = repeat_placeholders(users.len());
            let sql = format!("DELETE FROM workflow WHERE user NOT IN ({placeholders})");
            let mut q = sqlx::query(&sql);
            for u in users {
                q = q.bind(u);
            }
            q.execute(dst)
                .await
                .context("DELETE FROM workflow (user filter)")?;
            Ok(())
        }
        Filter::AccessGroups(ids) => {
            // i64 values — safe to interpolate directly.
            let id_list = id_list(ids);
            let sql = format!(
                "DELETE FROM workflow WHERE id NOT IN \
                 (SELECT workflow_id FROM workflow_access_group WHERE group_id IN ({id_list}))"
            );
            sqlx::query(&sql)
                .execute(dst)
                .await
                .context("DELETE FROM workflow (access-group filter)")?;
            Ok(())
        }
        Filter::WorkflowIds(ids) => {
            let id_list = id_list(ids);
            let sql = format!("DELETE FROM workflow WHERE id NOT IN ({id_list})");
            sqlx::query(&sql)
                .execute(dst)
                .await
                .context("DELETE FROM workflow (id filter)")?;
            Ok(())
        }
    }
}

/// Iteratively delete rows that violate any foreign-key constraint.
///
/// `PRAGMA foreign_key_check` enumerates every FK violation currently present
/// in the database — in our case, this catches pre-existing orphans the
/// source DB carried in (where the FK parent was already missing before our
/// filter ran). Deletes are performed per-table in batches keyed off the
/// virtual-table's `rowid` column (the violating row's rowid), and the loop
/// repeats because a single deletion can cascade and surface further
/// violations on the next pass. Bounded by `MAX_ITERATIONS` so a pathological
/// schema can't spin forever.
async fn prune_orphans(dst: &mut SqliteConnection) -> Result<()> {
    use std::collections::BTreeMap;
    const MAX_ITERATIONS: usize = 16;

    for pass in 1..=MAX_ITERATIONS {
        let violations: Vec<(String, i64)> =
            sqlx::query_as(r#"SELECT "table", rowid FROM pragma_foreign_key_check"#)
                .fetch_all(&mut *dst)
                .await
                .context("PRAGMA foreign_key_check")?;
        if violations.is_empty() {
            return Ok(());
        }
        let mut by_table: BTreeMap<String, Vec<i64>> = BTreeMap::new();
        for (t, r) in violations {
            by_table.entry(t).or_default().push(r);
        }
        let total: usize = by_table.values().map(|v| v.len()).sum();
        info!(
            "Pruning {total} orphan row(s) across {} table(s) (pass {pass})",
            by_table.len()
        );
        for (table, rowids) in by_table {
            let placeholders = repeat_placeholders(rowids.len());
            // Quote the identifier in case a future migration introduces a
            // table name that needs it; double any embedded quote per the SQL
            // standard.
            let sql = format!(
                r#"DELETE FROM "{}" WHERE rowid IN ({})"#,
                table.replace('"', "\"\""),
                placeholders,
            );
            let mut q = sqlx::query(&sql);
            for r in &rowids {
                q = q.bind(*r);
            }
            q.execute(&mut *dst)
                .await
                .with_context(|| format!("DELETE orphans from {table}"))?;
        }
    }
    bail!(
        "orphan pruning did not converge after {MAX_ITERATIONS} iterations \
         — likely a schema issue, refusing to loop forever"
    );
}

fn repeat_placeholders(n: usize) -> String {
    if n == 0 {
        return String::new();
    }
    let mut s = String::with_capacity(2 * n - 1);
    s.push('?');
    for _ in 1..n {
        s.push_str(",?");
    }
    s
}

fn id_list(ids: &[i64]) -> String {
    ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
}

fn normalize_db_url(input: &str) -> String {
    if input.starts_with("sqlite:") {
        input.to_string()
    } else {
        format!("sqlite:{input}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_handles_zero_one_many() {
        assert_eq!(repeat_placeholders(0), "");
        assert_eq!(repeat_placeholders(1), "?");
        assert_eq!(repeat_placeholders(3), "?,?,?");
    }

    #[test]
    fn id_list_formats_correctly() {
        assert_eq!(id_list(&[]), "");
        assert_eq!(id_list(&[42]), "42");
        assert_eq!(id_list(&[1, 2, 3]), "1,2,3");
    }

    #[test]
    fn normalize_adds_prefix() {
        assert_eq!(normalize_db_url("/tmp/x.db"), "sqlite:/tmp/x.db");
        assert_eq!(normalize_db_url("sqlite:foo"), "sqlite:foo");
    }
}
