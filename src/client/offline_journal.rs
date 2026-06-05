//! Local journal of job completions recorded while the server is unreachable.
//!
//! When a job runner enters offline-drain mode (see [`crate::client::job_runner`]),
//! it can no longer report job completions to the server. Instead it appends each
//! completion to a per-node SQLite journal on local (typically shared) storage.
//! Two readers consume these journals:
//!
//! 1. The runner itself, when the server comes back while jobs are still running:
//!    it flushes the journal via `batch_complete_jobs` and resumes normal operation.
//! 2. The `torc workflows reconcile` command, which discovers every journal for a given
//!    `(workflow_id, run_id)` under a base directory and replays them in bulk so a
//!    user does not have to run one command per compute node.
//!
//! The journal file name encodes both `workflow_id` and `run_id` so reconcile can
//! locate the right files by glob, plus the runner's `unique_label` for per-node
//! uniqueness. Completions are keyed by `job_id` (a job completes at most once per
//! run), so re-appending is an idempotent upsert.

use crate::models::JobCompletionEntry;
use log::{debug, info};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

/// Subdirectory under the runner's `output_dir` where journals are written.
const JOURNAL_SUBDIR: &str = "offline_journal";
const FILENAME_PREFIX: &str = "offline_results";

/// Maximum completions to send in a single `batch_complete_jobs` request when
/// replaying a journal. Bounds request body size and lets replay make partial
/// progress. Shared by the runner's resume flush and `torc workflows reconcile`.
pub const FLUSH_BATCH_SIZE: usize = 500;

/// Build the journal file name for a runner. The `workflow_id` / `run_id` prefix
/// is what [`OfflineJournal::discover`] globs on; `unique_label` makes the name
/// unique across the compute nodes participating in a run.
fn journal_filename(workflow_id: i64, run_id: i64, unique_label: &str) -> String {
    format!("{FILENAME_PREFIX}_wf{workflow_id}_r{run_id}_{unique_label}.db")
}

/// Filename prefix shared by every journal belonging to a `(workflow_id, run_id)`.
fn journal_filename_prefix(workflow_id: i64, run_id: i64) -> String {
    format!("{FILENAME_PREFIX}_wf{workflow_id}_r{run_id}_")
}

/// Returns the path of the journal file that would be created for the given
/// runner identity, mirroring the layout used by [`OfflineJournal::open_or_create`].
pub fn journal_path(
    output_dir: &Path,
    workflow_id: i64,
    run_id: i64,
    unique_label: &str,
) -> PathBuf {
    output_dir
        .join(JOURNAL_SUBDIR)
        .join(journal_filename(workflow_id, run_id, unique_label))
}

/// A handle to a single compute node's offline completion journal.
pub struct OfflineJournal {
    conn: Connection,
    path: PathBuf,
}

impl OfflineJournal {
    /// Open (creating if necessary) the journal for this runner. Uses the same
    /// WAL + `synchronous=NORMAL` durability settings as the resource monitor so
    /// finished results survive a process kill or node crash during the outage.
    pub fn open_or_create(
        output_dir: &Path,
        workflow_id: i64,
        run_id: i64,
        unique_label: &str,
    ) -> rusqlite::Result<Self> {
        let dir = output_dir.join(JOURNAL_SUBDIR);
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::error!("Failed to create offline journal directory: {}", e);
            return Err(rusqlite::Error::InvalidPath(dir));
        }

        let path = journal_path(output_dir, workflow_id, run_id, unique_label);
        info!("Opening offline completion journal at: {}", path.display());

        let conn = Connection::open(&path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS journaled_completions (
                job_id INTEGER PRIMARY KEY,
                run_id INTEGER NOT NULL,
                status INTEGER NOT NULL,
                payload TEXT NOT NULL,
                journaled_at INTEGER NOT NULL
            )",
            [],
        )?;

        Ok(Self { conn, path })
    }

    /// Path to the underlying SQLite file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append (or replace) a completion. Keyed by `job_id`, so re-journaling the
    /// same job is idempotent.
    pub fn append(&self, entry: &JobCompletionEntry) -> Result<(), String> {
        let payload = serde_json::to_string(entry)
            .map_err(|e| format!("Failed to serialize completion: {e}"))?;
        let now = chrono::Utc::now().timestamp();
        self.conn
            .execute(
                "INSERT OR REPLACE INTO journaled_completions
                 (job_id, run_id, status, payload, journaled_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    entry.job_id,
                    entry.run_id,
                    entry.status as i64,
                    payload,
                    now,
                ],
            )
            .map_err(|e| format!("Failed to journal completion: {e}"))?;
        debug!(
            "Journaled completion job_id={} run_id={} status={:?}",
            entry.job_id, entry.run_id, entry.status
        );
        Ok(())
    }

    /// Read every journaled completion from this file.
    pub fn read_all(&self) -> Result<Vec<JobCompletionEntry>, String> {
        Self::read_all_from_conn(&self.conn)
    }

    /// Delete all journaled completions. Called after a successful flush to the
    /// server so a subsequent outage does not re-submit already-recorded jobs.
    pub fn clear(&self) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM journaled_completions", [])
            .map_err(|e| format!("Failed to clear journal: {e}"))?;
        Ok(())
    }

    fn read_all_from_conn(conn: &Connection) -> Result<Vec<JobCompletionEntry>, String> {
        let mut stmt = conn
            .prepare("SELECT payload FROM journaled_completions ORDER BY job_id")
            .map_err(|e| format!("Failed to prepare journal query: {e}"))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| format!("Failed to query journal: {e}"))?;

        let mut entries = Vec::new();
        for row in rows {
            let payload = row.map_err(|e| format!("Failed to read journal row: {e}"))?;
            let entry: JobCompletionEntry = serde_json::from_str(&payload)
                .map_err(|e| format!("Failed to deserialize journaled completion: {e}"))?;
            entries.push(entry);
        }
        Ok(entries)
    }

    /// Read all completions from a journal file at `path` without holding a
    /// long-lived handle. Used by `torc workflows reconcile`.
    pub fn read_file(path: &Path) -> Result<Vec<JobCompletionEntry>, String> {
        let conn = Connection::open(path)
            .map_err(|e| format!("Failed to open journal {}: {e}", path.display()))?;
        Self::read_all_from_conn(&conn)
    }

    /// Count the journaled completions in the file at `path` without
    /// deserializing each payload. Cheaper than [`read_file`] when only the
    /// number of pending completions is needed (e.g. an advisory check).
    pub fn count_file(path: &Path) -> Result<usize, String> {
        let conn = Connection::open(path)
            .map_err(|e| format!("Failed to open journal {}: {e}", path.display()))?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM journaled_completions", [], |row| {
                row.get(0)
            })
            .map_err(|e| format!("Failed to count journal {}: {e}", path.display()))?;
        Ok(count.max(0) as usize)
    }

    /// Find every journal file for a `(workflow_id, run_id)` by recursively
    /// walking `base_dir`. Matching is by file name prefix, so journals written
    /// to per-node `output_dir`s nested anywhere under `base_dir` are all found.
    pub fn discover(base_dir: &Path, workflow_id: i64, run_id: i64) -> Vec<PathBuf> {
        let prefix = journal_filename_prefix(workflow_id, run_id);
        let mut found = Vec::new();
        walk(base_dir, &prefix, &mut found);
        found.sort();
        found
    }
}

/// Recursively collect files whose name starts with `prefix` and ends with `.db`.
/// Symlinks are not followed to avoid cycles. Unreadable directories are skipped.
fn walk(dir: &Path, prefix: &str, found: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            walk(&path, prefix, found);
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && name.starts_with(prefix)
            && name.ends_with(".db")
        {
            found.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{JobStatus, ResultModel};

    fn sample_entry(job_id: i64, run_id: i64) -> JobCompletionEntry {
        JobCompletionEntry {
            job_id,
            status: JobStatus::Completed,
            run_id,
            result: ResultModel {
                id: None,
                job_id,
                workflow_id: 1,
                run_id,
                attempt_id: Some(1),
                compute_node_id: 7,
                return_code: 0,
                exec_time_minutes: 1.0,
                completion_time: "2026-05-23T00:00:00Z".to_string(),
                peak_memory_bytes: None,
                avg_memory_bytes: None,
                peak_cpu_percent: None,
                avg_cpu_percent: None,
                status: JobStatus::Completed,
                job_name: None,
            },
        }
    }

    #[test]
    fn test_append_read_and_idempotent_upsert() {
        let dir = tempfile::tempdir().unwrap();
        let journal = OfflineJournal::open_or_create(dir.path(), 1, 42, "wf1_h_node_r42").unwrap();
        journal.append(&sample_entry(10, 42)).unwrap();
        journal.append(&sample_entry(11, 42)).unwrap();
        // Re-appending the same job_id replaces rather than duplicates.
        journal.append(&sample_entry(10, 42)).unwrap();

        let entries = journal.read_all().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].job_id, 10);
        assert_eq!(entries[1].job_id, 11);
    }

    #[test]
    fn test_discover_finds_nested_journals_for_wf_run() {
        let base = tempfile::tempdir().unwrap();
        // Two nodes write into separate nested output dirs.
        let node_a = base.path().join("node_a").join("torc_output");
        let node_b = base.path().join("node_b").join("torc_output");
        OfflineJournal::open_or_create(&node_a, 5, 3, "node_a")
            .unwrap()
            .append(&sample_entry(100, 3))
            .unwrap();
        OfflineJournal::open_or_create(&node_b, 5, 3, "node_b")
            .unwrap()
            .append(&sample_entry(200, 3))
            .unwrap();
        // A journal for a different run must not match.
        OfflineJournal::open_or_create(&node_b, 5, 4, "node_b")
            .unwrap()
            .append(&sample_entry(300, 4))
            .unwrap();

        let found = OfflineJournal::discover(base.path(), 5, 3);
        assert_eq!(found.len(), 2, "expected exactly the two run_id=3 journals");

        let total: usize = found
            .iter()
            .map(|p| OfflineJournal::read_file(p).unwrap().len())
            .sum();
        assert_eq!(total, 2);
    }

    #[test]
    fn test_clear_empties_journal() {
        let dir = tempfile::tempdir().unwrap();
        let journal = OfflineJournal::open_or_create(dir.path(), 1, 1, "label").unwrap();
        journal.append(&sample_entry(1, 1)).unwrap();
        journal.clear().unwrap();
        assert!(journal.read_all().unwrap().is_empty());
    }
}
