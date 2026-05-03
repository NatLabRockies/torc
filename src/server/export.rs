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
//! 2. Open the snapshot, `PRAGMA foreign_keys=ON`, and
//!    `DELETE FROM workflow WHERE id NOT IN (<filter>)`. Every per-workflow
//!    table has `ON DELETE CASCADE` on `workflow_id`, so the cascade chain
//!    cleans up jobs, files, results, events, etc. automatically.
//! 3. Unless `preserve_access_groups` is set, wipe `user_group_membership`
//!    (which has no per-workflow scoping and would leak unrelated users'
//!    group affiliations) and `access_group` (which cascades the
//!    `workflow_access_group` join table).
//! 4. Optional final `VACUUM` to reclaim space freed by the deletes.

use anyhow::{Context, Result, anyhow, bail};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use std::path::PathBuf;
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
            // Drop the connection so we can remove the file cleanly.
            let _ = dst.close().await;
            let _ = std::fs::remove_file(&opts.output_path);
            bail!("filter matched no workflows; refusing to write empty export");
        }
        info!("Retained {remaining} workflow(s) after filter");

        // workflow_status has no ON DELETE CASCADE from workflow (the FK column
        // was added via ALTER TABLE ADD COLUMN, which SQLite cannot extend with
        // FK constraints). The DELETE above leaves orphan status rows whose
        // count alone would leak how many workflows existed in the source DB.
        // Prune them here.
        sqlx::query("DELETE FROM workflow_status WHERE id NOT IN (SELECT status_id FROM workflow)")
            .execute(&mut dst)
            .await
            .context("DELETE FROM workflow_status (orphan prune)")?;
    }

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
