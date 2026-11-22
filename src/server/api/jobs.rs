//! Job-related API endpoints

#![allow(clippy::too_many_arguments)]

use std::collections::{BTreeMap, HashMap};

use crate::server::transport_types::context_types::{ApiError, Has, XSpanIdString};
use async_trait::async_trait;
use chrono::Utc;
use log::{debug, error, info};
use sha2::{Digest, Sha256};
use sqlx::Row;
use tracing::instrument;

use crate::server::api_responses::{
    ClaimNextJobsResponse, CreateJobResponse, CreateJobsResponse, DeleteJobResponse,
    DeleteJobsResponse, GetJobResponse, GetReadyJobRequirementsResponse, ListJobIdsResponse,
    ListJobsResponse, ProcessChangedJobInputsResponse, ResetJobStatusResponse, RetryJobResponse,
    UpdateJobResponse,
};

use crate::CLAIM_START_MISMATCH;
use crate::models::{self as models, JobStatus};

use super::{
    ApiContext, MAX_RECORD_TRANSFER_COUNT, SqlQueryBuilder, begin_immediate, claimable_labels_sql,
    database_error_with_msg, database_lock_aware_error, deserialize_env_map, escape_like_pattern,
    int_in_clause, labels_column, message_error_response, normalize_env_map, parse_job_status,
    resource_not_found_response, serialize_env_map, validate_env_map,
};

/// Run and compute node a job runner passes with a claim. When present, the claim marks
/// the jobs running in its own transaction, replacing one `start_job` call per job.
#[derive(Debug, Clone, Copy)]
pub struct ClaimStart {
    pub run_id: i64,
    pub compute_node_id: i64,
}

impl ClaimStart {
    /// Build from a claim's optional query parameters, which must be passed together.
    pub fn from_query(
        run_id: Option<i64>,
        compute_node_id: Option<i64>,
    ) -> Result<Option<Self>, String> {
        match (run_id, compute_node_id) {
            (None, None) => Ok(None),
            (Some(run_id), Some(compute_node_id)) => Ok(Some(Self {
                run_id,
                compute_node_id,
            })),
            _ => Err("run_id and compute_node_id must be passed together".to_string()),
        }
    }
}

/// Check `run_id` against the workflow's current run on `conn`, which may be inside an open
/// transaction. Returns `Some(message)` when the workflow is missing or the run is stale.
pub(crate) async fn run_id_mismatch(
    conn: &mut sqlx::SqliteConnection,
    workflow_id: i64,
    run_id: i64,
) -> Result<Option<String>, sqlx::Error> {
    let row = sqlx::query!("SELECT run_id FROM workflow WHERE id = ?", workflow_id)
        .fetch_optional(&mut *conn)
        .await?;
    Ok(match row {
        None => Some(format!(
            "Workflow status not found for workflow ID: {}",
            workflow_id
        )),
        Some(row) if row.run_id != run_id => Some(format!(
            "{}: provided {} but workflow is at run {}",
            crate::STALE_RUN,
            run_id,
            row.run_id
        )),
        Some(_) => None,
    })
}

/// True when `start` is absent or names the workflow's current run and one of its compute nodes.
pub(crate) async fn claim_start_is_valid(
    conn: &mut sqlx::SqliteConnection,
    workflow_id: i64,
    start: Option<ClaimStart>,
) -> Result<bool, sqlx::Error> {
    let Some(s) = start else {
        return Ok(true);
    };
    let row = sqlx::query(
        "SELECT 1 FROM workflow w JOIN compute_node c ON c.workflow_id = w.id \
         WHERE w.id = ? AND w.run_id = ? AND c.id = ?",
    )
    .bind(workflow_id)
    .bind(s.run_id)
    .bind(s.compute_node_id)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(row.is_some())
}

/// Finish a claim inside its write transaction: attach output file and user_data IDs
/// to the claimed jobs and mark them pending, or running when `start` is set.
///
/// Returns false, changing nothing, when `start` names a run other than the workflow's
/// current one or a compute node outside the workflow. The check runs under the claim's
/// write lock so a concurrent reset cannot hand the new run's jobs to a stale runner.
pub(crate) async fn finish_claim(
    conn: &mut sqlx::SqliteConnection,
    workflow_id: i64,
    jobs: &mut [models::JobModel],
    start: Option<ClaimStart>,
) -> Result<bool, sqlx::Error> {
    if !claim_start_is_valid(conn, workflow_id, start).await? {
        return Ok(false);
    }
    if jobs.is_empty() {
        return Ok(true);
    }
    let ids = jobs
        .iter()
        .filter_map(|j| j.id)
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let mut files: HashMap<i64, Vec<i64>> = HashMap::new();
    let sql = format!("SELECT job_id, file_id FROM job_output_file WHERE job_id IN ({ids})");
    for row in sqlx::query(sqlx::AssertSqlSafe(&*sql))
        .fetch_all(&mut *conn)
        .await?
    {
        files
            .entry(row.get("job_id"))
            .or_default()
            .push(row.get("file_id"));
    }
    let mut user_data: HashMap<i64, Vec<i64>> = HashMap::new();
    let sql =
        format!("SELECT job_id, user_data_id FROM job_output_user_data WHERE job_id IN ({ids})");
    for row in sqlx::query(sqlx::AssertSqlSafe(&*sql))
        .fetch_all(&mut *conn)
        .await?
    {
        user_data
            .entry(row.get("job_id"))
            .or_default()
            .push(row.get("user_data_id"));
    }

    for job in jobs.iter_mut() {
        let id = job.id.unwrap_or_default();
        job.output_file_ids = files.remove(&id);
        job.output_user_data_ids = user_data.remove(&id);
    }

    if let Some(s) = start {
        let start_time = Utc::now().to_rfc3339();
        let sql = format!(
            "UPDATE job SET status = ?, start_time = ?, compute_node_id = ? WHERE id IN ({ids})"
        );
        sqlx::query(sqlx::AssertSqlSafe(&*sql))
            .bind(JobStatus::Running.to_int())
            .bind(&start_time)
            .bind(s.compute_node_id)
            .execute(&mut *conn)
            .await?;
        for job in jobs {
            job.status = Some(JobStatus::Running);
            job.start_time = Some(start_time.clone());
            job.compute_node_id = Some(s.compute_node_id);
        }
    } else {
        let sql = format!("UPDATE job SET status = ? WHERE id IN ({ids})");
        sqlx::query(sqlx::AssertSqlSafe(&*sql))
            .bind(JobStatus::Pending.to_int())
            .execute(&mut *conn)
            .await?;
        for job in jobs {
            job.status = Some(JobStatus::Pending);
        }
    }
    Ok(true)
}

/// Per-job values prepared during the first pass of bulk `create_jobs` so the
/// chunked INSERT in the second pass can stay infallible. `env_serialized` and the
/// per-job booleans/priority that get NULL/default coalescing in the single-row
/// path are materialized here once.
struct PreparedJobRow {
    job: models::JobModel,
    env_serialized: Option<String>,
    cancel_on_blocking_job_failure: bool,
    supports_termination: bool,
    priority: i64,
    status_int: i32,
}

/// Bulk INSERT a relationship table whose row shape is `(i64, i64, i64)`
/// (e.g. `job_depends_on`, `job_input_file`, `job_output_file`).
///
/// The caller passes the table name and the parenthesized column list as
/// trusted static strings — they never come from request data. Rows are split
/// into chunks sized so each statement stays below `SQLITE_MAX_VARIABLE_NUMBER`.
async fn bulk_insert_job_3col(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    table: &str,
    columns_paren: &str,
    rows: &[(i64, i64, i64)],
) -> Result<(), ApiError> {
    if rows.is_empty() {
        return Ok(());
    }
    // 3 cols × 1000 rows = 3000 bound parameters per statement.
    const ROWS_PER_INSERT: usize = 1000;
    for chunk in rows.chunks(ROWS_PER_INSERT) {
        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(format!(
            "INSERT INTO {} {} ",
            table, columns_paren
        ));
        qb.push_values(chunk, |mut row, (a, b, c)| {
            row.push_bind(*a).push_bind(*b).push_bind(*c);
        });
        qb.build().execute(&mut **tx).await.map_err(|e| {
            database_error_with_msg(e, format!("Failed to bulk insert into {}", table))
        })?;
    }
    Ok(())
}

/// Bulk INSERT a relationship table whose row shape is `(i64, i64)`
/// (e.g. `job_input_user_data`, `job_output_user_data`).
async fn bulk_insert_job_2col(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    table: &str,
    columns_paren: &str,
    rows: &[(i64, i64)],
) -> Result<(), ApiError> {
    if rows.is_empty() {
        return Ok(());
    }
    // 2 cols × 1500 rows = 3000 bound parameters per statement.
    const ROWS_PER_INSERT: usize = 1500;
    for chunk in rows.chunks(ROWS_PER_INSERT) {
        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(format!(
            "INSERT INTO {} {} ",
            table, columns_paren
        ));
        qb.push_values(chunk, |mut row, (a, b)| {
            row.push_bind(*a).push_bind(*b);
        });
        qb.build().execute(&mut **tx).await.map_err(|e| {
            database_error_with_msg(e, format!("Failed to bulk insert into {}", table))
        })?;
    }
    Ok(())
}

/// Each id-valued reference on a job: field name, target tables (the id must be in one of
/// them), and an extractor.
type JobRefField = (
    &'static str,
    &'static [&'static str],
    fn(&models::JobModel) -> &[i64],
);
const JOB_REF_FIELDS: [JobRefField; 8] = [
    ("depends_on_job_ids", &["job"], |j| {
        j.depends_on_job_ids.as_deref().unwrap_or_default()
    }),
    ("input_file_ids", &["file"], |j| {
        j.input_file_ids.as_deref().unwrap_or_default()
    }),
    ("output_file_ids", &["file"], |j| {
        j.output_file_ids.as_deref().unwrap_or_default()
    }),
    ("input_user_data_ids", &["user_data"], |j| {
        j.input_user_data_ids.as_deref().unwrap_or_default()
    }),
    ("output_user_data_ids", &["user_data"], |j| {
        j.output_user_data_ids.as_deref().unwrap_or_default()
    }),
    (
        "resource_requirements_id",
        &["resource_requirements"],
        |j| j.resource_requirements_id.as_slice(),
    ),
    // Specs bind jobs to Slurm schedulers, but `torc workflows import` remaps a job's
    // scheduler_id through local schedulers too (`IdMappings::remap_scheduler_id`).
    (
        "scheduler_id",
        &["slurm_scheduler", "local_scheduler"],
        |j| j.scheduler_id.as_slice(),
    ),
    ("failure_handler_id", &["failure_handler"], |j| {
        j.failure_handler_id.as_slice()
    }),
];

/// Check that every id a job references (dependencies, files, user data, resource
/// requirements, scheduler, failure handler) is a record in `workflow_id`. Foreign keys only
/// check that the target exists somewhere. Ids are collected across `jobs` and checked once
/// per field. Returns `Some(message)` naming the offending ids (missing or in another
/// workflow), for a 422.
pub(crate) async fn job_refs_outside_workflow(
    conn: &mut sqlx::SqliteConnection,
    workflow_id: i64,
    jobs: &[&models::JobModel],
) -> Result<Option<String>, ApiError> {
    const MAX_IDS_IN_MESSAGE: usize = 20;
    let mut problems = Vec::new();
    for (field, tables, extract) in JOB_REF_FIELDS {
        let ids: std::collections::BTreeSet<i64> =
            jobs.iter().flat_map(|j| extract(j)).copied().collect();
        if ids.is_empty() {
            continue;
        }
        // Pass the ids as one JSON array so a 100k-job batch needs one query per field
        // instead of chunked IN lists.
        let ids_json = serde_json::to_string(&ids)
            .map_err(|e| ApiError(format!("Failed to serialize {}: {}", field, e)))?;
        let not_in_tables = tables
            .iter()
            .map(|t| {
                format!(
                    "NOT EXISTS (SELECT 1 FROM {t} t WHERE t.id = j.value AND t.workflow_id = ?2)"
                )
            })
            .collect::<Vec<_>>()
            .join(" AND ");
        let sql =
            format!("SELECT j.value FROM json_each(?1) j WHERE {not_in_tables} ORDER BY j.value");
        let bad: Vec<i64> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .bind(ids_json)
            .bind(workflow_id)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| database_error_with_msg(e, format!("Failed to validate {}", field)))?;
        if !bad.is_empty() {
            let mut shown: Vec<String> = bad
                .iter()
                .take(MAX_IDS_IN_MESSAGE)
                .map(i64::to_string)
                .collect();
            if bad.len() > MAX_IDS_IN_MESSAGE {
                shown.push(format!("... ({} more)", bad.len() - MAX_IDS_IN_MESSAGE));
            }
            problems.push(format!("{} [{}]", field, shown.join(", ")));
        }
    }
    if problems.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!(
        "Job references records that do not exist in workflow_id={}: {}",
        workflow_id,
        problems.join("; ")
    )))
}

/// Return the nodes of one cycle in a directed graph (node -> deps), if any.
/// Iterative DFS so long dependency chains cannot overflow the stack.
pub(crate) fn find_cycle<T>(adjacency: &std::collections::HashMap<T, Vec<T>>) -> Option<Vec<T>>
where
    T: Copy + Eq + std::hash::Hash + Ord,
{
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Visiting,
        Done,
    }
    let mut marks: std::collections::HashMap<T, Mark> = std::collections::HashMap::new();
    let mut starts: Vec<T> = adjacency.keys().copied().collect();
    starts.sort();
    for start in starts {
        if marks.contains_key(&start) {
            continue;
        }
        marks.insert(start, Mark::Visiting);
        // (node, index of the next dependency to visit); the stack is the current path.
        let mut stack: Vec<(T, usize)> = vec![(start, 0)];
        while let Some(&(node, idx)) = stack.last() {
            let deps = adjacency.get(&node).map(|v| v.as_slice()).unwrap_or(&[]);
            if idx == deps.len() {
                marks.insert(node, Mark::Done);
                stack.pop();
                continue;
            }
            if let Some(top) = stack.last_mut() {
                top.1 += 1;
            }
            let next = deps[idx];
            match marks.get(&next) {
                Some(Mark::Visiting) => {
                    let pos = stack.iter().position(|(n, _)| *n == next).unwrap_or(0);
                    return Some(stack[pos..].iter().map(|(n, _)| *n).collect());
                }
                Some(Mark::Done) => {}
                None => {
                    marks.insert(next, Mark::Visiting);
                    stack.push((next, 0));
                }
            }
        }
    }
    None
}

/// A `name_refs` problem in a `create_jobs` request (422), or an internal error (500).
enum NameRefError {
    Invalid(String),
    Api(ApiError),
}

impl From<ApiError> for NameRefError {
    fn from(e: ApiError) -> Self {
        NameRefError::Api(e)
    }
}

/// `name -> ids` for the records of `table` in the workflow. With `names`, only those names are
/// loaded; without, every named record is (regexes need them all). `table` is a trusted static.
async fn record_ids_by_name(
    conn: &mut sqlx::SqliteConnection,
    table: &str,
    workflow_id: i64,
    names: Option<&std::collections::BTreeSet<&str>>,
) -> Result<HashMap<String, Vec<i64>>, ApiError> {
    let mut sql =
        format!("SELECT id, name FROM {table} WHERE workflow_id = ?1 AND name IS NOT NULL");
    let names_json = match names {
        Some(names) => {
            sql.push_str(" AND name IN (SELECT value FROM json_each(?2))");
            Some(
                serde_json::to_string(names)
                    .map_err(|e| ApiError(format!("Failed to serialize names: {}", e)))?,
            )
        }
        None => None,
    };
    let rows: Vec<(i64, String)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(workflow_id)
        .bind(names_json)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| database_error_with_msg(e, format!("Failed to resolve {} names", table)))?;
    let mut out: HashMap<String, Vec<i64>> = HashMap::new();
    for (id, name) in rows {
        out.entry(name).or_default().push(id);
    }
    Ok(out)
}

/// Resolve one job's names and regexes for one reference type against `ids_by_name` and
/// return the matching ids (deduplicated, in first-match order).
fn resolve_job_names(
    label: &str,
    job_name: &str,
    names: Option<&Vec<String>>,
    regexes: Option<&Vec<String>>,
    ids_by_name: &HashMap<String, Vec<i64>>,
) -> Result<Vec<i64>, NameRefError> {
    let mut ids = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |id: i64, ids: &mut Vec<i64>| {
        if seen.insert(id) {
            ids.push(id);
        }
    };
    for name in names.into_iter().flatten() {
        match ids_by_name.get(name).map(Vec::as_slice) {
            Some([id]) => push(*id, &mut ids),
            Some(matches) => {
                return Err(NameRefError::Invalid(format!(
                    "{} '{}' for job '{}' is ambiguous: {} records have that name",
                    label,
                    name,
                    job_name,
                    matches.len()
                )));
            }
            None => {
                return Err(NameRefError::Invalid(format!(
                    "{} '{}' not found for job '{}'",
                    label, name, job_name
                )));
            }
        }
    }
    for pattern in regexes.into_iter().flatten() {
        let re = regex::Regex::new(pattern).map_err(|e| {
            NameRefError::Invalid(format!(
                "Invalid regex '{}' for {} in job '{}': {}",
                pattern,
                label.to_lowercase(),
                job_name,
                e
            ))
        })?;
        let mut matched = false;
        for (name, name_ids) in ids_by_name {
            if re.is_match(name) {
                matched = true;
                for id in name_ids {
                    push(*id, &mut ids);
                }
            }
        }
        if !matched {
            return Err(NameRefError::Invalid(format!(
                "{} regex '{}' did not match any names for job '{}'",
                label, pattern, job_name
            )));
        }
    }
    Ok(ids)
}

/// A list-valued name reference: label, table, (names, regexes) accessor, id field accessor.
type ListNameRef = (
    &'static str,
    &'static str,
    fn(&models::JobNameRefs) -> (Option<&Vec<String>>, Option<&Vec<String>>),
    fn(&mut models::JobModel) -> &mut Option<Vec<i64>>,
);
const LIST_NAME_REFS: [ListNameRef; 4] = [
    (
        "Input file",
        "file",
        |r| (r.input_files.as_ref(), r.input_file_regexes.as_ref()),
        |j| &mut j.input_file_ids,
    ),
    (
        "Output file",
        "file",
        |r| (r.output_files.as_ref(), r.output_file_regexes.as_ref()),
        |j| &mut j.output_file_ids,
    ),
    (
        "Input user data",
        "user_data",
        |r| {
            (
                r.input_user_data.as_ref(),
                r.input_user_data_regexes.as_ref(),
            )
        },
        |j| &mut j.input_user_data_ids,
    ),
    (
        "Output user data",
        "user_data",
        |r| {
            (
                r.output_user_data.as_ref(),
                r.output_user_data_regexes.as_ref(),
            )
        },
        |j| &mut j.output_user_data_ids,
    ),
];

/// A single-valued name reference: label, table, name accessor, id field accessor.
type ScalarNameRef = (
    &'static str,
    &'static str,
    fn(&models::JobNameRefs) -> Option<&String>,
    fn(&mut models::JobModel) -> &mut Option<i64>,
);
const SCALAR_NAME_REFS: [ScalarNameRef; 3] = [
    (
        "Resource requirements",
        "resource_requirements",
        |r| r.resource_requirements.as_ref(),
        |j| &mut j.resource_requirements_id,
    ),
    (
        "Scheduler",
        "slurm_scheduler",
        |r| r.scheduler.as_ref(),
        |j| &mut j.scheduler_id,
    ),
    (
        "Failure handler",
        "failure_handler",
        |r| r.failure_handler.as_ref(),
        |j| &mut j.failure_handler_id,
    ),
];

/// Resolve the `name_refs` of a `create_jobs` batch inside its transaction.
///
/// Ids of existing records are added to each job's id fields. Dependencies on other jobs of the
/// batch have no ids yet, so they are returned per job as batch indexes for the caller to insert
/// after the job rows; the batch's own dependency graph is checked for cycles here. Clears
/// `name_refs` on every job.
async fn resolve_batch_name_refs(
    conn: &mut sqlx::SqliteConnection,
    workflow_id: i64,
    jobs: &mut [&mut models::JobModel],
) -> Result<Vec<Vec<usize>>, NameRefError> {
    let refs: Vec<Option<models::JobNameRefs>> =
        jobs.iter_mut().map(|j| j.name_refs.take()).collect();
    let mut batch_deps: Vec<Vec<usize>> = vec![Vec::new(); jobs.len()];
    if refs.iter().all(Option::is_none) {
        return Ok(batch_deps);
    }

    // Per table: whether any job uses a regex (then every name is loaded) and the exact names.
    let mut wanted: BTreeMap<&'static str, (bool, std::collections::BTreeSet<&str>)> =
        BTreeMap::new();
    fn want<'a>(
        wanted: &mut BTreeMap<&'static str, (bool, std::collections::BTreeSet<&'a str>)>,
        table: &'static str,
        names: Option<&'a Vec<String>>,
        regex: bool,
    ) {
        let entry = wanted.entry(table).or_default();
        entry.0 |= regex;
        entry
            .1
            .extend(names.into_iter().flatten().map(String::as_str));
    }
    for r in refs.iter().flatten() {
        want(
            &mut wanted,
            "job",
            r.depends_on.as_ref(),
            r.depends_on_regexes.is_some(),
        );
        for (_, table, names, _) in LIST_NAME_REFS {
            let (names, regexes) = names(r);
            want(&mut wanted, table, names, regexes.is_some());
        }
        for (_, table, name, _) in SCALAR_NAME_REFS {
            if let Some(name) = name(r) {
                wanted.entry(table).or_default().1.insert(name.as_str());
            }
        }
    }
    let mut ids_by_table: HashMap<&str, HashMap<String, Vec<i64>>> = HashMap::new();
    for (table, (all, names)) in &wanted {
        if *all || !names.is_empty() {
            let filter = if *all { None } else { Some(names) };
            let ids = record_ids_by_name(conn, table, workflow_id, filter).await?;
            ids_by_table.insert(table, ids);
        }
    }
    let mut job_ids = ids_by_table.remove("job").unwrap_or_default();
    // The batch's own jobs get placeholder ids -1, -2, ... (batch index i -> -(i + 1)). A name
    // that is both in the batch and in the workflow then resolves as ambiguous.
    for (i, job) in jobs.iter().enumerate() {
        job_ids
            .entry(job.name.clone())
            .or_default()
            .push(-(i as i64) - 1);
    }
    let empty = HashMap::new();

    for (i, (job, r)) in jobs.iter_mut().zip(&refs).enumerate() {
        let Some(r) = r else { continue };
        for (label, table, names, field) in LIST_NAME_REFS {
            let (names, regexes) = names(r);
            let table_ids = ids_by_table.get(table).unwrap_or(&empty);
            let ids = resolve_job_names(label, &job.name, names, regexes, table_ids)?;
            extend_unique(field(job), ids);
        }
        for (label, table, name, field) in SCALAR_NAME_REFS {
            let Some(name) = name(r) else { continue };
            let table_ids = ids_by_table.get(table).unwrap_or(&empty);
            let names = vec![name.clone()];
            let id = resolve_job_names(label, &job.name, Some(&names), None, table_ids)?[0];
            let slot = field(job);
            if slot.is_some_and(|existing| existing != id) {
                return Err(NameRefError::Invalid(format!(
                    "Job '{}' sets both an id and the name '{}' for {}",
                    job.name,
                    name,
                    label.to_lowercase()
                )));
            }
            *slot = Some(id);
        }
        let dep_ids = resolve_job_names(
            "Job dependency",
            &job.name,
            r.depends_on.as_ref(),
            r.depends_on_regexes.as_ref(),
            &job_ids,
        )
        .map_err(|e| match e {
            NameRefError::Invalid(msg) if msg.contains("not found") => {
                NameRefError::Invalid(format!(
                    "{} (a name must be a job already in workflow_id={} or in the same request)",
                    msg, workflow_id
                ))
            }
            other => other,
        })?;
        let (batch, existing): (Vec<i64>, Vec<i64>) = dep_ids.into_iter().partition(|id| *id < 0);
        extend_unique(&mut job.depends_on_job_ids, existing);
        batch_deps[i] = batch.into_iter().map(|id| (-id - 1) as usize).collect();
    }

    // Edges to existing jobs can't close a cycle (existing jobs can't depend on new ones), so
    // only the batch's own graph needs checking.
    let adjacency: HashMap<usize, Vec<usize>> = batch_deps
        .iter()
        .enumerate()
        .filter(|(_, deps)| !deps.is_empty())
        .map(|(i, deps)| (i, deps.clone()))
        .collect();
    if let Some(cycle) = find_cycle(&adjacency) {
        let path = cycle
            .iter()
            .chain(cycle.first())
            .map(|i| format!("'{}'", jobs[*i].name))
            .collect::<Vec<_>>()
            .join(" -> ");
        return Err(NameRefError::Invalid(format!(
            "Job dependencies in this request contain a cycle (job depends on job): {}",
            path
        )));
    }
    Ok(batch_deps)
}

/// Append `ids` to an optional id list, skipping ids already present.
fn extend_unique(target: &mut Option<Vec<i64>>, ids: Vec<i64>) {
    if ids.is_empty() {
        return;
    }
    let target = target.get_or_insert_with(Vec::new);
    let mut seen: std::collections::HashSet<i64> = target.iter().copied().collect();
    target.extend(ids.into_iter().filter(|id| seen.insert(*id)));
}

/// Trait defining job-related API operations
#[async_trait]
pub trait JobsApi<C> {
    /// Store a job.
    async fn create_job(
        &self,
        mut job: models::JobModel,
        context: &C,
    ) -> Result<CreateJobResponse, ApiError>;

    /// Create jobs in bulk.
    async fn create_jobs(
        &self,
        body: models::JobsModel,
        context: &C,
    ) -> Result<CreateJobsResponse, ApiError>;

    /// Delete all jobs for one workflow.
    async fn delete_jobs(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteJobsResponse, ApiError>;

    /// Retrieve a job.
    async fn get_job(&self, id: i64, context: &C) -> Result<GetJobResponse, ApiError>;

    /// Return the resource requirements for jobs with a status of ready.
    async fn get_ready_job_requirements(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetReadyJobRequirementsResponse, ApiError>;

    /// Retrieve all job IDs for one workflow.
    async fn list_job_ids(&self, id: i64, context: &C) -> Result<ListJobIdsResponse, ApiError>;

    /// Retrieve all jobs for one workflow.
    #[allow(clippy::too_many_arguments)]
    async fn list_jobs(
        &self,
        workflow_id: i64,
        status: Option<Vec<JobStatus>>,
        needs_file_id: Option<i64>,
        upstream_job_id: Option<i64>,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        include_relationships: Option<bool>,
        active_compute_node_id: Option<i64>,
        origin_is_set: Option<bool>,
        name: Option<String>,
        command: Option<String>,
        labels: Option<String>,
        ids: Option<Vec<i64>>,
        context: &C,
    ) -> Result<ListJobsResponse, ApiError>;

    /// Update a job.
    ///
    /// Restrictions:
    /// - Jobs can only be updated when their status is `Uninitialized`
    /// - The job status field itself cannot be modified
    /// - Relationship fields (input_file_ids, output_file_ids, input_user_data_ids, output_user_data_ids) are immutable after job creation
    /// - The `depends_on_job_ids` field can be modified only when the job status is `Uninitialized`
    async fn update_job(
        &self,
        id: i64,
        body: models::JobModel,
        context: &C,
    ) -> Result<UpdateJobResponse, ApiError>;

    /// Update a job's status only.
    ///
    /// This function updates only the status field with no restrictions.
    /// All other job fields remain unchanged.
    async fn update_job_status(
        &self,
        id: i64,
        status: JobStatus,
        context: &C,
    ) -> Result<UpdateJobResponse, ApiError>;

    /// Return user-requested number of jobs that are ready for submission. Sets status to pending, or running when `start` is set.
    async fn claim_next_jobs(
        &self,
        id: i64,
        requested_job_count: i64,
        labels: Option<String>,
        start: Option<ClaimStart>,
        context: &C,
    ) -> Result<ClaimNextJobsResponse, ApiError>;

    /// Check for changed job inputs and update status accordingly.
    async fn process_changed_job_inputs(
        &self,
        id: i64,
        dry_run: bool,
        context: &C,
    ) -> Result<ProcessChangedJobInputsResponse, ApiError>;

    /// Delete a job.
    async fn delete_job(&self, id: i64, context: &C) -> Result<DeleteJobResponse, ApiError>;

    /// Reset status for jobs to uninitialized.
    async fn reset_job_status(
        &self,
        id: i64,
        failed_only: bool,
        context: &C,
    ) -> Result<ResetJobStatusResponse, ApiError>;

    /// Retry a failed job by resetting its status to Ready and incrementing attempt_id.
    ///
    /// Prerequisites:
    /// - Job must be in Failed or Terminated status
    /// - run_id must match the workflow's current run_id
    /// - if the job has a failure handler, attempt_id must be less than the largest
    ///   max_retries among its rules
    async fn retry_job(
        &self,
        id: i64,
        run_id: i64,
        context: &C,
    ) -> Result<RetryJobResponse, ApiError>;
}

/// Implementation of jobs API for the server
#[derive(Clone)]
pub struct JobsApiImpl {
    pub context: ApiContext,
}

/// Largest `max_retries` among a failure handler's rules (JSON array), or None if it has no
/// rules. A rule without `max_retries` defaults to 3, as in the workflow spec.
fn max_retries_from_rules(rules: &str) -> Result<Option<i64>, serde_json::Error> {
    #[derive(serde::Deserialize)]
    struct Rule {
        #[serde(default = "default_max_retries")]
        max_retries: i64,
    }
    fn default_max_retries() -> i64 {
        3
    }
    let rules: Vec<Rule> = serde_json::from_str(rules)?;
    Ok(rules.iter().map(|r| r.max_retries).max())
}

const JOB_COLUMNS: &[&str] = &[
    "id",
    "workflow_id",
    "name",
    "command",
    "env",
    "cancel_on_blocking_job_failure",
    "supports_termination",
    "resource_requirements_id",
    "invocation_script",
    "status",
    "scheduler_id",
    "failure_handler_id",
    "attempt_id",
];

/// Returns true if `body_field` is `Some` and its contents differ from `existing_field`
/// when both are compared as **sets** (order-independent).
///
/// `update_job` treats relationship-id vectors (`input_file_ids`, `depends_on_job_ids`,
/// etc.) as sets: a request that sends the same IDs in a different order is not a
/// modification. This helper centralises the sort-then-compare so the call sites can
/// stay one-liners.
fn vec_id_set_changed<T: Ord + Clone>(
    body_field: &Option<Vec<T>>,
    existing_field: &Option<Vec<T>>,
) -> bool {
    let Some(body_vec) = body_field else {
        return false;
    };
    let mut body_sorted = body_vec.clone();
    body_sorted.sort();
    let empty: Vec<T> = Vec::new();
    let mut existing_sorted = existing_field.as_ref().unwrap_or(&empty).clone();
    existing_sorted.sort();
    body_sorted != existing_sorted
}

impl JobsApiImpl {
    pub fn new(context: ApiContext) -> Self {
        Self { context }
    }

    async fn fetch_workflow_env<'e, E>(
        executor: E,
        workflow_id: i64,
    ) -> Result<Option<HashMap<String, String>>, ApiError>
    where
        E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
    {
        let env_json: Option<String> = sqlx::query_scalar("SELECT env FROM workflow WHERE id = ?")
            .bind(workflow_id)
            .fetch_optional(executor)
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to fetch workflow env"))?
            .flatten();
        deserialize_env_map(env_json, "workflow env")
    }

    fn merge_env(
        workflow_env: Option<&HashMap<String, String>>,
        job_env: Option<&HashMap<String, String>>,
    ) -> Option<HashMap<String, String>> {
        let mut effective_env = workflow_env.cloned().unwrap_or_default();
        if let Some(job_env) = job_env {
            effective_env.extend(job_env.clone());
        }
        normalize_env_map(Some(effective_env))
    }

    fn env_for_hash(env: Option<HashMap<String, String>>) -> Option<BTreeMap<String, String>> {
        env.map(|env_map| env_map.into_iter().collect())
    }

    /// Create an association between a job and a file.
    async fn add_job_file_association<'e, E>(
        &self,
        executor: E,
        job_id: i64,
        file_id: i64,
        workflow_id: i64,
        table_name: &str,
    ) -> Result<(), ApiError>
    where
        E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
    {
        let sql = format!(
            "INSERT INTO {} (job_id, file_id, workflow_id) VALUES ($1, $2, $3)",
            table_name
        );

        match sqlx::query(sqlx::AssertSqlSafe(&*sql))
            .bind(job_id)
            .bind(file_id)
            .bind(workflow_id)
            .execute(executor)
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(database_error_with_msg(
                e,
                "Failed to create job association",
            )),
        }
    }

    /// Create user_data association between job and user_data.
    async fn add_job_user_data_association<'e, E>(
        &self,
        executor: E,
        job_id: i64,
        user_data_id: i64,
        table_name: &str,
    ) -> Result<(), ApiError>
    where
        E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
    {
        let sql = format!(
            "INSERT INTO {} (job_id, user_data_id) VALUES ($1, $2)",
            table_name
        );

        match sqlx::query(sqlx::AssertSqlSafe(&*sql))
            .bind(job_id)
            .bind(user_data_id)
            .execute(executor)
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(database_error_with_msg(
                e,
                "Failed to create job association",
            )),
        }
    }

    /// Create depends-on association between two jobs.
    async fn add_depends_on_association<'e, E>(
        &self,
        executor: E,
        job_id: i64,
        depends_on_job_id: i64,
        workflow_id: i64,
    ) -> Result<(), ApiError>
    where
        E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
    {
        match sqlx::query!(
            r#"
            INSERT INTO job_depends_on (job_id, depends_on_job_id, workflow_id)
            VALUES ($1, $2, $3)
            "#,
            job_id,
            depends_on_job_id,
            workflow_id
        )
        .execute(executor)
        .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(database_error_with_msg(
                e,
                "Failed to create job association",
            )),
        }
    }

    /// Get complete job with all relationships
    async fn get_job_with_relationships(&self, id: i64) -> Result<models::JobModel, ApiError> {
        // Get basic job info
        let record = match sqlx::query(
            r#"
                SELECT id, workflow_id, name, command, resource_requirements_id, invocation_script,
                       env,
                       status, cancel_on_blocking_job_failure, supports_termination, scheduler_id,
                       failure_handler_id, attempt_id, priority, origin, labels,
                       start_time, compute_node_id
                FROM job
                WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(rec)) => rec,
            Ok(None) => {
                error!("Job not found with ID: {}", id);
                return Err(ApiError(format!("Job not found with ID: {}", id)));
            }
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to fetch job record"));
            }
        };

        let status_int: i32 = record.get("status");
        let status = parse_job_status(status_int, id)?;

        // Get depends_on relationships
        let depends_on_records = match sqlx::query!(
            "SELECT depends_on_job_id FROM job_depends_on WHERE job_id = $1 ORDER BY depends_on_job_id",
            id
        )
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(records) => records,
            Err(e) => return Err(database_error_with_msg(e, "Failed to fetch job relationships")),
        };
        let depends_on_job_ids = if depends_on_records.is_empty() {
            None
        } else {
            Some(
                depends_on_records
                    .into_iter()
                    .map(|r| r.depends_on_job_id)
                    .collect(),
            )
        };

        // Get input file relationships
        let input_file_records = match sqlx::query!(
            "SELECT file_id FROM job_input_file WHERE job_id = $1 ORDER BY file_id",
            id
        )
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(records) => records,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to fetch job relationships",
                ));
            }
        };
        let input_file_ids = if input_file_records.is_empty() {
            None
        } else {
            Some(input_file_records.into_iter().map(|r| r.file_id).collect())
        };

        // Get output file relationships
        let output_file_records = match sqlx::query!(
            "SELECT file_id FROM job_output_file WHERE job_id = $1 ORDER BY file_id",
            id
        )
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(records) => records,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to fetch job relationships",
                ));
            }
        };
        let output_file_ids = if output_file_records.is_empty() {
            None
        } else {
            Some(output_file_records.into_iter().map(|r| r.file_id).collect())
        };

        // Get input user_data relationships
        let input_user_data_records = match sqlx::query!(
            "SELECT user_data_id FROM job_input_user_data WHERE job_id = $1 ORDER BY user_data_id",
            id
        )
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(records) => records,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to fetch job relationships",
                ));
            }
        };
        let input_user_data_ids = if input_user_data_records.is_empty() {
            None
        } else {
            Some(
                input_user_data_records
                    .into_iter()
                    .map(|r| r.user_data_id)
                    .collect(),
            )
        };

        // Get output user_data relationships
        let output_user_data_records = match sqlx::query!(
            "SELECT user_data_id FROM job_output_user_data WHERE job_id = $1 ORDER BY user_data_id",
            id
        )
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(records) => records,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to fetch job relationships",
                ));
            }
        };
        let output_user_data_ids = if output_user_data_records.is_empty() {
            None
        } else {
            Some(
                output_user_data_records
                    .into_iter()
                    .map(|r| r.user_data_id)
                    .collect(),
            )
        };

        Ok(models::JobModel {
            id: Some(record.get("id")),
            workflow_id: record.get("workflow_id"),
            name: record.get("name"),
            command: record.get("command"),
            env: deserialize_env_map(
                record
                    .try_get::<Option<String>, _>("env")
                    .map_err(|e| database_error_with_msg(e, "Failed to decode job env"))?,
                "job env",
            )?,
            cancel_on_blocking_job_failure: record.try_get("cancel_on_blocking_job_failure").ok(),
            supports_termination: record.try_get("supports_termination").ok(),
            depends_on_job_ids,
            input_file_ids,
            output_file_ids,
            input_user_data_ids,
            output_user_data_ids,
            resource_requirements_id: record.try_get("resource_requirements_id").ok(),
            invocation_script: record.try_get("invocation_script").ok(),
            status: Some(status),
            start_time: record
                .try_get::<Option<String>, _>("start_time")
                .ok()
                .flatten(),
            compute_node_id: record
                .try_get::<Option<i64>, _>("compute_node_id")
                .ok()
                .flatten(),
            scheduler_id: record.try_get("scheduler_id").ok(),
            schedule_compute_nodes: None, // This field is not stored in the database
            failure_handler_id: record.try_get("failure_handler_id").ok(),
            attempt_id: record.try_get("attempt_id").ok(),
            priority: record.try_get("priority").ok(),
            origin: record.try_get::<Option<String>, _>("origin").ok().flatten(),
            labels: labels_column(&record, "labels"),
            name_refs: None,
        })
    }

    /// Reset only failed/canceled/terminated/pending_failed jobs to uninitialized status.
    async fn reset_failed_jobs_only(
        &self,
        workflow_id: i64,
    ) -> Result<ResetJobStatusResponse, ApiError> {
        let uninitialized_status = JobStatus::Uninitialized.to_int();
        let failed_status = JobStatus::Failed.to_int();
        let canceled_status = JobStatus::Canceled.to_int();
        let terminated_status = JobStatus::Terminated.to_int();
        let pending_failed_status = JobStatus::PendingFailed.to_int();

        // Get jobs with failed/canceled/terminated/pending_failed status. The status field is
        // the source of truth - we don't check return_code since status should always be
        // consistent with the actual outcome. This also handles jobs that were canceled/terminated
        // before running (which have no result record).
        let failed_jobs = match sqlx::query!(
            r#"
            SELECT id, status
            FROM job
            WHERE workflow_id = $1
              AND status IN ($2, $3, $4, $5)
            "#,
            workflow_id,
            failed_status,
            canceled_status,
            terminated_status,
            pending_failed_status
        )
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(jobs) => jobs,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to fetch failed jobs"));
            }
        };

        let mut total_reset_count = 0;

        // For each failed job, reset it and trigger completion reversal for downstream jobs
        for job_row in failed_jobs {
            let job_id = job_row.id;
            let current_status_int = job_row.status as i32;

            // Parse the current status; skip jobs whose status integer cannot be decoded.
            let Ok(current_status) = parse_job_status(current_status_int, job_id) else {
                continue;
            };

            // Reset the job status and clear runtime state from any prior attempt.
            match sqlx::query!(
                "UPDATE job SET status = $1, start_time = NULL, compute_node_id = NULL WHERE id = $2",
                uninitialized_status,
                job_id
            )
            .execute(self.context.pool.as_ref())
            .await
            {
                Ok(result) => {
                    if result.rows_affected() > 0 {
                        total_reset_count += 1;

                        // If the job was previously complete, trigger completion reversal for downstream jobs
                        if current_status.is_complete() {
                            debug!(
                                "reset_failed_jobs_only: reverting completed job_id={}, resetting downstream jobs",
                                job_id
                            );

                            if let Err(e) = self.update_jobs_from_completion_reversal(job_id).await
                            {
                                error!("Failed to reset downstream jobs for job {}: {}", job_id, e);
                                // Continue with other jobs even if one fails
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to reset status for job {}: {}", job_id, e);
                    // Continue with other jobs
                }
            }
        }

        info!(
            "Jobs status reset workflow_id={} count={} new_status=uninitialized",
            workflow_id, total_reset_count
        );

        Ok(ResetJobStatusResponse::SuccessfulResponse(
            models::ResetJobStatusResponse::new(
                workflow_id,
                total_reset_count,
                JobStatus::Uninitialized.to_string(),
            )
            .with_reset_type("failed_only".to_string()),
        ))
    }

    /// Selective reset (`reset_job_status` with a body). Everything runs in one
    /// `BEGIN IMMEDIATE` transaction so no runner can claim jobs or register a compute node
    /// between the quiet check and the reset. Does not bump the workflow's run_id: callers follow
    /// up with a reinitialize, which bumps it exactly once.
    pub async fn reset_selected_jobs(
        &self,
        workflow_id: i64,
        body: models::ResetJobStatusRequest,
        force: bool,
    ) -> Result<ResetJobStatusResponse, ApiError> {
        let statuses = body.statuses.unwrap_or_else(|| {
            vec![
                JobStatus::Failed,
                JobStatus::Terminated,
                JobStatus::Canceled,
                JobStatus::PendingFailed,
            ]
        });
        let dry_run = body.dry_run.unwrap_or(false);

        let mut tx = begin_immediate(&self.context.pool)
            .await
            .map_err(|e| database_lock_aware_error(e, "Failed to begin transaction"))?;

        if !force && let Some(reason) = Self::workflow_busy_reason(&mut tx, workflow_id).await? {
            return Ok(ResetJobStatusResponse::UnprocessableContentErrorResponse(
                message_error_response(format!(
                    "Cannot reset jobs in workflow {}: {}",
                    workflow_id, reason
                )),
            ));
        }

        // Current status of every job in the workflow that the request could touch.
        let rows: Vec<(i64, i64)> =
            sqlx::query_as("SELECT id, status FROM job WHERE workflow_id = ?")
                .bind(workflow_id)
                .fetch_all(&mut *tx)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to fetch jobs for reset"))?;
        let current: BTreeMap<i64, i64> = rows.into_iter().collect();

        let mut reset = Vec::new();
        let mut skipped = Vec::new();
        let mut reset_complete = Vec::new();
        let explicit = body.job_ids.is_some();
        let candidates: Vec<i64> = match body.job_ids {
            Some(mut ids) => {
                ids.sort_unstable();
                ids.dedup();
                ids
            }
            None => current.keys().copied().collect(),
        };
        for job_id in candidates {
            let Some(&status_int) = current.get(&job_id) else {
                skipped.push(models::ResetJobStatusSkipped {
                    job_id,
                    reason: format!("job is not in workflow {}", workflow_id),
                });
                continue;
            };
            let status = parse_job_status(status_int as i32, job_id)?;
            if !statuses.contains(&status) {
                // Listing every non-matching job of a whole-workflow selection would be noise.
                if explicit {
                    skipped.push(models::ResetJobStatusSkipped {
                        job_id,
                        reason: format!("status {} is not eligible for reset", status),
                    });
                }
                continue;
            }
            if status.is_complete() {
                reset_complete.push(job_id);
            }
            reset.push(job_id);
        }

        if !dry_run && !reset.is_empty() {
            let uninitialized_status = JobStatus::Uninitialized.to_int();
            // One statement per job (not an IN list) so a large selection cannot exceed
            // SQLite's bound-parameter limit.
            for &job_id in &reset {
                sqlx::query(
                    "UPDATE job SET status = ?, start_time = NULL, compute_node_id = NULL \
                     WHERE id = ?",
                )
                .bind(uninitialized_status)
                .bind(job_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to reset job status"))?;
            }
            // Same side effect as manage_status_change (reinitialize_downstream_jobs): resetting
            // a terminal job also resets its direct completed or failed dependents. Deeper
            // dependents are reset by the following reinitialize.
            for &job_id in &reset_complete {
                sqlx::query(
                    "UPDATE job SET status = ? WHERE workflow_id = ? AND id IN ( \
                     SELECT jbb.job_id FROM job_depends_on jbb JOIN job j ON jbb.job_id = j.id \
                     WHERE jbb.depends_on_job_id = ? AND j.status IN (?, ?))",
                )
                .bind(uninitialized_status)
                .bind(workflow_id)
                .bind(job_id)
                .bind(JobStatus::Completed.to_int())
                .bind(JobStatus::Failed.to_int())
                .execute(&mut *tx)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to reset downstream jobs"))?;
            }
            tx.commit()
                .await
                .map_err(|e| database_lock_aware_error(e, "Failed to commit transaction"))?;
            info!(
                "Jobs status reset workflow_id={} count={} skipped={} new_status=uninitialized",
                workflow_id,
                reset.len(),
                skipped.len()
            );
        }

        let updated_count = if dry_run { 0 } else { reset.len() as i64 };
        let mut response = models::ResetJobStatusResponse::new(
            workflow_id,
            updated_count,
            JobStatus::Uninitialized.to_string(),
        )
        .with_reset_type("selected".to_string());
        response.reset = Some(reset);
        response.skipped = Some(skipped);
        Ok(ResetJobStatusResponse::SuccessfulResponse(response))
    }

    /// Why the workflow is not quiet enough for a selective reset, or None if it is. Same rules
    /// as the client's `check_no_active_workers`, plus no running or pending jobs.
    async fn workflow_busy_reason(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        workflow_id: i64,
    ) -> Result<Option<&'static str>, ApiError> {
        let (jobs_active, nodes_active, allocations_active): (bool, bool, bool) = sqlx::query_as(
            "SELECT \
                 EXISTS(SELECT 1 FROM job WHERE workflow_id = ? AND status IN (?, ?)), \
                 EXISTS(SELECT 1 FROM compute_node WHERE workflow_id = ? AND is_active = 1), \
                 EXISTS(SELECT 1 FROM scheduled_compute_node WHERE workflow_id = ? \
                        AND status IN ('pending', 'active'))",
        )
        .bind(workflow_id)
        .bind(JobStatus::Pending.to_int())
        .bind(JobStatus::Running.to_int())
        .bind(workflow_id)
        .bind(workflow_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| database_error_with_msg(e, "Failed to check workflow activity"))?;
        Ok(if jobs_active {
            Some("jobs are running or pending")
        } else if nodes_active {
            Some("there are active compute nodes")
        } else if allocations_active {
            Some("there are pending or active scheduled compute nodes")
        } else {
            None
        })
    }

    /// Reset all jobs downstream of the given job to Uninitialized status
    /// This is called when a completed job needs to be reset, requiring all
    /// downstream jobs to also be reset recursively
    async fn update_jobs_from_completion_reversal(&self, job_id: i64) -> Result<(), ApiError> {
        debug!(
            "update_jobs_from_completion_reversal: resetting downstream jobs for job_id={}",
            job_id
        );

        let uninitialized_status = JobStatus::Uninitialized.to_int();

        // Acquire the SQLite write lock up front. We read `workflow_id` before
        // the recursive UPDATE, which would risk SQLITE_BUSY_SNAPSHOT under
        // BEGIN DEFERRED if another writer commits in between.
        let mut tx = match begin_immediate(&self.context.pool).await {
            Ok(tx) => tx,
            Err(e) => {
                error!("Failed to begin transaction for completion reversal: {}", e);
                return Err(ApiError("Database error".to_string()));
            }
        };

        // Get the workflow_id for the given job_id first
        let workflow_id = match sqlx::query!("SELECT workflow_id FROM job WHERE id = ?", job_id)
            .fetch_optional(&mut *tx)
            .await
        {
            Ok(Some(row)) => row.workflow_id,
            Ok(None) => {
                error!("Job with id {} not found", job_id);
                return Err(ApiError("Job not found".to_string()));
            }
            Err(e) => {
                error!("Database error finding job {}: {}", job_id, e);
                return Err(ApiError("Database error".to_string()));
            }
        };

        // Use a recursive CTE to find all jobs downstream of the given job
        // and reset them to uninitialized status
        let result = sqlx::query!(
            r#"
            WITH RECURSIVE downstream_jobs(job_id, level) AS (
                -- Base case: find jobs directly blocked by the given job
                SELECT
                    jbb.job_id,
                    0 as level
                FROM job_depends_on jbb
                WHERE jbb.depends_on_job_id = ?
                  AND jbb.workflow_id = ?

                UNION ALL

                -- Recursive case: find jobs blocked by any downstream job
                SELECT
                    jbb.job_id,
                    dj.level + 1 as level
                FROM downstream_jobs dj
                JOIN job_depends_on jbb ON jbb.depends_on_job_id = dj.job_id
                WHERE jbb.workflow_id = ?
                  AND dj.level < 100  -- Prevent infinite loops
            )
            UPDATE job
            SET status = ?, start_time = NULL, compute_node_id = NULL
            WHERE workflow_id = ?
              AND id IN (SELECT DISTINCT job_id FROM downstream_jobs)
            "#,
            job_id,
            workflow_id,
            workflow_id,
            uninitialized_status,
            workflow_id
        )
        .execute(&mut *tx)
        .await;

        match result {
            Ok(result) => {
                let affected_rows = result.rows_affected();
                debug!(
                    "update_jobs_from_completion_reversal: reset {} downstream jobs for job_id={}",
                    affected_rows, job_id
                );

                // Commit the transaction
                if let Err(e) = tx.commit().await {
                    error!(
                        "Failed to commit transaction for completion reversal: {}",
                        e
                    );
                    return Err(ApiError("Database error".to_string()));
                }

                info!(
                    "Jobs downstream reset workflow_id={} job_id={} count={}",
                    workflow_id, job_id, affected_rows
                );

                Ok(())
            }
            Err(e) => {
                error!(
                    "Database error during completion reversal for job {}: {}",
                    job_id, e
                );
                // Transaction is rolled back automatically when `tx` is dropped.
                Err(ApiError("Database error".to_string()))
            }
        }
    }

    /// Compute SHA256 hash of job inputs
    ///
    /// This hash includes:
    /// - command
    /// - input_user_data_ids and their data contents
    /// - output_user_data_ids
    /// - input_file_ids
    /// - output_file_ids
    /// - invocation_script
    /// - depends_on_job_ids
    ///
    /// The hash is used to detect if job inputs have changed, requiring re-execution.
    pub async fn compute_job_input_hash(&self, job_id: i64) -> Result<String, ApiError> {
        // Get the job with all relationships
        let job = self.get_job_with_relationships(job_id).await?;

        // Query for input user_data content
        let mut input_user_data_contents = Vec::new();
        if let Some(input_user_data_ids) = &job.input_user_data_ids {
            for ud_id in input_user_data_ids {
                match sqlx::query!(
                    r#"
                    SELECT id, data as "data: Option<String>"
                    FROM user_data
                    WHERE id = $1
                    "#,
                    ud_id
                )
                .fetch_optional(self.context.pool.as_ref())
                .await
                {
                    Ok(Some(row)) => {
                        input_user_data_contents.push(serde_json::json!({
                            "id": row.id,
                            "data": row.data
                        }));
                    }
                    Ok(None) => {
                        debug!("User data {} not found for job {}", ud_id, job_id);
                    }
                    Err(e) => {
                        return Err(database_error_with_msg(
                            e,
                            "Failed to fetch user data for hash",
                        ));
                    }
                }
            }
        }

        // Normalize invocation_script: treat Some("") the same as None to ensure
        // consistent hashing between per-job and bulk hash computation methods.
        let invocation_script = job.invocation_script.filter(|s| !s.is_empty());
        let effective_env = Self::env_for_hash(normalize_env_map(job.env.clone()));

        // Build JSON object with all input fields in deterministic order
        let hash_input = serde_json::json!({
            "command": job.command,
            "invocation_script": invocation_script,
            "env": effective_env,
            "depends_on_job_ids": job.depends_on_job_ids,
            "input_file_ids": job.input_file_ids,
            "output_file_ids": job.output_file_ids,
            "input_user_data_ids": job.input_user_data_ids,
            "output_user_data_ids": job.output_user_data_ids,
            "input_user_data_contents": input_user_data_contents,
        });

        // Serialize to JSON string (canonical representation)
        let json_string = match serde_json::to_string(&hash_input) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to serialize hash input for job {}: {}", job_id, e);
                return Err(ApiError(format!("JSON serialization error: {}", e)));
            }
        };

        // Compute SHA256 hash
        let mut hasher = Sha256::new();
        hasher.update(json_string.as_bytes());
        let hash_bytes = hasher.finalize();
        let hash_hex = hex::encode(hash_bytes);

        debug!("Computed input hash for job {}: {}", job_id, hash_hex);
        Ok(hash_hex)
    }

    /// Store job input hash in job_internal table
    ///
    /// Uses INSERT ON CONFLICT to upsert - will insert new record or update existing one.
    pub async fn store_job_input_hash(&self, job_id: i64, hash: &str) -> Result<(), ApiError> {
        match sqlx::query!(
            r#"
            INSERT INTO job_internal (job_id, input_hash)
            VALUES ($1, $2)
            ON CONFLICT(job_id) DO UPDATE SET input_hash = excluded.input_hash
            "#,
            job_id,
            hash
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(_) => {
                debug!(
                    "Stored input hash {} for job {} in job_internal",
                    hash, job_id
                );
                Ok(())
            }
            Err(e) => Err(database_error_with_msg(e, "Failed to store job input hash")),
        }
    }

    /// Compute and store input hashes for all jobs in a workflow using bulk queries.
    ///
    /// This is much more efficient than calling `compute_job_input_hash` per job because it
    /// fetches all relationship data in a small number of bulk queries instead of 7+ queries
    /// per job. For a workflow with 100K jobs, this reduces ~700K sequential queries to ~7.
    pub async fn compute_and_store_all_input_hashes(
        &self,
        workflow_id: i64,
    ) -> Result<(), ApiError> {
        let pool = self.context.pool.as_ref();

        // Query 1: All jobs in the workflow
        let job_rows = sqlx::query(
            r#"
            SELECT id, command, invocation_script, env
            FROM job
            WHERE workflow_id = ?
            ORDER BY id
            "#,
        )
        .bind(workflow_id)
        .fetch_all(pool)
        .await
        .map_err(|e| database_error_with_msg(e, "Failed to fetch jobs for bulk hash"))?;

        if job_rows.is_empty() {
            return Ok(());
        }

        let job_count = job_rows.len();
        debug!(
            "Computing bulk input hashes for {} jobs in workflow {}",
            job_count, workflow_id
        );
        // Query 2: All depends_on relationships
        let depends_on_rows = sqlx::query!(
            r#"
            SELECT job_id, depends_on_job_id
            FROM job_depends_on
            WHERE workflow_id = $1
            ORDER BY job_id, depends_on_job_id
            "#,
            workflow_id
        )
        .fetch_all(pool)
        .await
        .map_err(|e| database_error_with_msg(e, "Failed to fetch depends_on for bulk hash"))?;

        // Query 3: All input file relationships
        let input_file_rows = sqlx::query!(
            r#"
            SELECT jif.job_id, jif.file_id
            FROM job_input_file jif
            JOIN job j ON j.id = jif.job_id
            WHERE j.workflow_id = $1
            ORDER BY jif.job_id, jif.file_id
            "#,
            workflow_id
        )
        .fetch_all(pool)
        .await
        .map_err(|e| database_error_with_msg(e, "Failed to fetch input files for bulk hash"))?;

        // Query 4: All output file relationships
        let output_file_rows = sqlx::query!(
            r#"
            SELECT jof.job_id, jof.file_id
            FROM job_output_file jof
            JOIN job j ON j.id = jof.job_id
            WHERE j.workflow_id = $1
            ORDER BY jof.job_id, jof.file_id
            "#,
            workflow_id
        )
        .fetch_all(pool)
        .await
        .map_err(|e| database_error_with_msg(e, "Failed to fetch output files for bulk hash"))?;

        // Query 5: All input user_data relationships
        let input_ud_rows = sqlx::query!(
            r#"
            SELECT jiud.job_id, jiud.user_data_id
            FROM job_input_user_data jiud
            JOIN job j ON j.id = jiud.job_id
            WHERE j.workflow_id = $1
            ORDER BY jiud.job_id, jiud.user_data_id
            "#,
            workflow_id
        )
        .fetch_all(pool)
        .await
        .map_err(|e| database_error_with_msg(e, "Failed to fetch input user_data for bulk hash"))?;

        // Query 6: All output user_data relationships
        let output_ud_rows = sqlx::query!(
            r#"
            SELECT joud.job_id, joud.user_data_id
            FROM job_output_user_data joud
            JOIN job j ON j.id = joud.job_id
            WHERE j.workflow_id = $1
            ORDER BY joud.job_id, joud.user_data_id
            "#,
            workflow_id
        )
        .fetch_all(pool)
        .await
        .map_err(|e| {
            database_error_with_msg(e, "Failed to fetch output user_data for bulk hash")
        })?;

        // Build lookup maps: job_id -> Vec<related_id> (already sorted by ORDER BY)
        let mut depends_on_map: HashMap<i64, Vec<i64>> = HashMap::new();
        for row in &depends_on_rows {
            depends_on_map
                .entry(row.job_id)
                .or_default()
                .push(row.depends_on_job_id);
        }

        let mut input_file_map: HashMap<i64, Vec<i64>> = HashMap::new();
        for row in &input_file_rows {
            input_file_map
                .entry(row.job_id)
                .or_default()
                .push(row.file_id);
        }

        let mut output_file_map: HashMap<i64, Vec<i64>> = HashMap::new();
        for row in &output_file_rows {
            output_file_map
                .entry(row.job_id)
                .or_default()
                .push(row.file_id);
        }

        let mut input_ud_map: HashMap<i64, Vec<i64>> = HashMap::new();
        for row in &input_ud_rows {
            input_ud_map
                .entry(row.job_id)
                .or_default()
                .push(row.user_data_id);
        }

        let mut output_ud_map: HashMap<i64, Vec<i64>> = HashMap::new();
        for row in &output_ud_rows {
            output_ud_map
                .entry(row.job_id)
                .or_default()
                .push(row.user_data_id);
        }

        // Query 7: User data contents for all distinct input user_data IDs
        let mut user_data_contents: HashMap<i64, Option<String>> = HashMap::new();
        let all_input_ud_ids: Vec<i64> = input_ud_map
            .values()
            .flatten()
            .copied()
            .collect::<std::collections::HashSet<i64>>()
            .into_iter()
            .collect();

        if !all_input_ud_ids.is_empty() {
            // Fetch in batches to avoid SQLite variable limit
            for chunk in all_input_ud_ids.chunks(500) {
                let placeholders: Vec<&str> = chunk.iter().map(|_| "?").collect();
                let sql = format!(
                    "SELECT id, data FROM user_data WHERE id IN ({})",
                    placeholders.join(",")
                );
                let mut query = sqlx::query(sqlx::AssertSqlSafe(&*sql));
                for id in chunk {
                    query = query.bind(id);
                }
                let rows = query.fetch_all(pool).await.map_err(|e| {
                    database_error_with_msg(e, "Failed to fetch user_data contents for bulk hash")
                })?;
                for row in rows {
                    let id: i64 = row.get("id");
                    let data: Option<String> = row.get("data");
                    user_data_contents.insert(id, data);
                }
            }
        }

        // Compute all hashes in memory first
        let mut hash_pairs: Vec<(i64, String)> = Vec::with_capacity(job_rows.len());

        for job_row in &job_rows {
            let job_id: i64 = job_row.get("id");
            let command: Option<String> = job_row.get("command");
            // Normalize invocation_script: treat Some("") the same as None to ensure
            // consistent hashing between per-job and bulk hash computation methods.
            let invocation_script: Option<String> = job_row
                .get::<Option<String>, _>("invocation_script")
                .filter(|s| !s.is_empty());
            let effective_env =
                Self::env_for_hash(deserialize_env_map(job_row.get("env"), "job env")?);

            // Build the same JSON structure as compute_job_input_hash
            let depends_on: Option<&Vec<i64>> = depends_on_map.get(&job_id);
            let input_files: Option<&Vec<i64>> = input_file_map.get(&job_id);
            let output_files: Option<&Vec<i64>> = output_file_map.get(&job_id);
            let input_uds: Option<&Vec<i64>> = input_ud_map.get(&job_id);
            let output_uds: Option<&Vec<i64>> = output_ud_map.get(&job_id);

            // Build input_user_data_contents matching the per-job method
            let input_user_data_contents: Vec<serde_json::Value> = input_uds
                .map(|ids| {
                    ids.iter()
                        .filter_map(|ud_id| {
                            user_data_contents.get(ud_id).map(|data| {
                                serde_json::json!({
                                    "id": ud_id,
                                    "data": data
                                })
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();

            let hash_input = serde_json::json!({
                "command": command,
                "invocation_script": invocation_script,
                "env": effective_env,
                "depends_on_job_ids": depends_on,
                "input_file_ids": input_files,
                "output_file_ids": output_files,
                "input_user_data_ids": input_uds,
                "output_user_data_ids": output_uds,
                "input_user_data_contents": input_user_data_contents,
            });

            let json_string = serde_json::to_string(&hash_input).map_err(|e| {
                ApiError(format!(
                    "JSON serialization error for job {}: {}",
                    job_id, e
                ))
            })?;

            let mut hasher = Sha256::new();
            hasher.update(json_string.as_bytes());
            let hash_hex = hex::encode(hasher.finalize());

            hash_pairs.push((job_id, hash_hex));
        }

        // Batch store hashes using multi-row INSERT for efficiency
        let mut tx = pool
            .begin()
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to begin hash storage transaction"))?;

        // SQLite has a variable limit (default 999), and each row needs 2 variables,
        // so we batch in chunks of 499 rows.
        for chunk in hash_pairs.chunks(499) {
            let mut placeholders = Vec::with_capacity(chunk.len());
            for i in 0..chunk.len() {
                placeholders.push(format!("(${}, ${})", i * 2 + 1, i * 2 + 2));
            }
            let sql = format!(
                "INSERT INTO job_internal (job_id, input_hash) VALUES {} \
                 ON CONFLICT(job_id) DO UPDATE SET input_hash = excluded.input_hash",
                placeholders.join(", ")
            );
            let mut query = sqlx::query(sqlx::AssertSqlSafe(&*sql));
            for (job_id, hash_hex) in chunk {
                query = query.bind(job_id).bind(hash_hex);
            }
            query
                .execute(&mut *tx)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to batch store input hashes"))?;
        }

        tx.commit()
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to commit hash storage transaction"))?;

        debug!(
            "Completed bulk hash computation and storage for {} jobs in workflow {}",
            job_count, workflow_id
        );

        Ok(())
    }

    /// Get stored job input hash from job_internal table
    ///
    /// Returns None if no hash has been stored for this job yet.
    pub async fn get_stored_job_input_hash(&self, job_id: i64) -> Result<Option<String>, ApiError> {
        match sqlx::query!(
            r#"
            SELECT input_hash
            FROM job_internal
            WHERE job_id = $1
            "#,
            job_id
        )
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(row)) => {
                debug!(
                    "Retrieved stored hash for job {}: {}",
                    job_id, row.input_hash
                );
                Ok(Some(row.input_hash))
            }
            Ok(None) => {
                debug!("No stored hash found for job {}", job_id);
                Ok(None)
            }
            Err(e) => Err(database_error_with_msg(e, "Failed to retrieve stored hash")),
        }
    }
}

#[async_trait]
impl<C> JobsApi<C> for JobsApiImpl
where
    C: Has<XSpanIdString> + Send + Sync,
{
    /// Store a job.
    ///
    /// This operation wraps job creation and relationship creation in a transaction
    /// to ensure atomicity. If any insert fails, all will be rolled back.
    #[instrument(skip(self, job, context), fields(workflow_id = job.workflow_id))]
    async fn create_job(
        &self,
        mut job: models::JobModel,
        context: &C,
    ) -> Result<CreateJobResponse, ApiError> {
        debug!(
            "create_job({:?}) - X-Span-ID: {:?}",
            job,
            context.get().0.clone()
        );

        let invocation_script = job.invocation_script.clone();
        let cancel_on_blocking_job_failure = job.cancel_on_blocking_job_failure.unwrap_or(true);
        let supports_termination = job.supports_termination.unwrap_or(false);
        let priority = job.priority.unwrap_or(0);
        if priority < 0 {
            return Ok(CreateJobResponse::UnprocessableContentErrorResponse(
                message_error_response(format!(
                    "priority must be >= 0, got {} for job '{}'",
                    priority, job.name
                )),
            ));
        }
        if let Err(err) = models::validate_labels(job.labels.as_deref()) {
            return Ok(CreateJobResponse::UnprocessableContentErrorResponse(
                message_error_response(format!("{} for job '{}'", err, job.name)),
            ));
        }
        if let Err(err) = validate_env_map(job.env.as_ref(), "job env") {
            return Ok(CreateJobResponse::UnprocessableContentErrorResponse(
                message_error_response(err.0),
            ));
        }
        let status = JobStatus::Uninitialized;
        let status_int = status.to_int();
        job.status = Some(status);
        // Echo the stored (sorted, deduplicated) form back to the caller.
        job.labels =
            models::labels_from_json(models::labels_to_json(job.labels.as_deref()).as_deref());

        // Acquire the SQLite write lock up front. `fetch_workflow_env` reads
        // before the INSERT, which would risk SQLITE_BUSY_SNAPSHOT under
        // BEGIN DEFERRED if another writer commits in between.
        let mut tx = match begin_immediate(&self.context.pool).await {
            Ok(tx) => tx,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to begin transaction"));
            }
        };

        let workflow_env = crate::tx_try!(
            tx,
            Self::fetch_workflow_env(&mut *tx, job.workflow_id).await
        );
        job.env = Self::merge_env(workflow_env.as_ref(), job.env.as_ref());
        let job_env = crate::tx_try!(tx, serialize_env_map(job.env.clone(), "job env"));

        if let Some(msg) = crate::tx_try!(
            tx,
            job_refs_outside_workflow(&mut tx, job.workflow_id, &[&job]).await
        ) {
            let _ = tx.rollback().await;
            return Ok(CreateJobResponse::UnprocessableContentErrorResponse(
                message_error_response(msg),
            ));
        }

        let job_result = match sqlx::query(
            r#"
            INSERT INTO job
            (
                workflow_id,
                name,
                command,
                cancel_on_blocking_job_failure,
                supports_termination,
                resource_requirements_id,
                invocation_script,
                env,
                status,
                scheduler_id,
                failure_handler_id,
                priority,
                labels
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING id
            "#,
        )
        .bind(job.workflow_id)
        .bind(&job.name)
        .bind(&job.command)
        .bind(cancel_on_blocking_job_failure)
        .bind(supports_termination)
        .bind(job.resource_requirements_id)
        .bind(&invocation_script)
        .bind(job_env)
        .bind(status_int)
        .bind(job.scheduler_id)
        .bind(job.failure_handler_id)
        .bind(priority)
        .bind(models::labels_to_json(job.labels.as_deref()))
        .fetch_one(&mut *tx)
        .await
        {
            Ok(job_result) => job_result,
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(database_error_with_msg(e, "Failed to create job record"));
            }
        };

        let job_id: i64 = job_result.get("id");
        job.id = Some(job_id);

        // Handle job dependencies
        if let Some(depends_on_job_ids) = &job.depends_on_job_ids {
            for blocking_id in depends_on_job_ids {
                crate::tx_try!(
                    tx,
                    self.add_depends_on_association(
                        &mut *tx,
                        job_id,
                        *blocking_id,
                        job.workflow_id
                    )
                    .await
                );
            }
        }

        // Handle input files
        if let Some(input_file_ids) = &job.input_file_ids {
            for file_id in input_file_ids {
                crate::tx_try!(
                    tx,
                    self.add_job_file_association(
                        &mut *tx,
                        job_id,
                        *file_id,
                        job.workflow_id,
                        "job_input_file",
                    )
                    .await
                );
            }
        }

        // Handle output files
        if let Some(output_file_ids) = &job.output_file_ids {
            for file_id in output_file_ids {
                crate::tx_try!(
                    tx,
                    self.add_job_file_association(
                        &mut *tx,
                        job_id,
                        *file_id,
                        job.workflow_id,
                        "job_output_file",
                    )
                    .await
                );
            }
        }

        // Handle input user_data
        if let Some(input_user_data_ids) = &job.input_user_data_ids {
            for user_data_id in input_user_data_ids {
                crate::tx_try!(
                    tx,
                    self.add_job_user_data_association(
                        &mut *tx,
                        job_id,
                        *user_data_id,
                        "job_input_user_data",
                    )
                    .await
                );
            }
        }

        // Handle output user_data
        if let Some(output_user_data_ids) = &job.output_user_data_ids {
            for user_data_id in output_user_data_ids {
                crate::tx_try!(
                    tx,
                    self.add_job_user_data_association(
                        &mut *tx,
                        job_id,
                        *user_data_id,
                        "job_output_user_data",
                    )
                    .await
                );
            }
        }

        // Commit the transaction
        if let Err(e) = tx.commit().await {
            return Err(database_error_with_msg(e, "Failed to commit transaction"));
        }

        debug!("Created job with id: {:?}", job_id);
        let response = CreateJobResponse::SuccessfulResponse(job);
        Ok(response)
    }

    /// Create jobs in bulk.
    #[instrument(skip(self, body, context), fields(job_count = body.jobs.len()))]
    async fn create_jobs(
        &self,
        body: models::JobsModel,
        context: &C,
    ) -> Result<CreateJobsResponse, ApiError> {
        debug!(
            "create_jobs({} jobs) - X-Span-ID: {:?}",
            body.jobs.len(),
            context.get().0.clone()
        );

        // The transport layer rejects empty batches with 422 so an unauthenticated
        // caller can't reach this method without a workflow_id to authorize against.
        // Mirror that contract here for in-process callers that bypass the transport.
        if body.jobs.is_empty() {
            return Ok(CreateJobsResponse::UnprocessableContentErrorResponse(
                message_error_response(
                    "Bulk job creation requires a non-empty `jobs` array".to_string(),
                ),
            ));
        }

        // Check if we're within the recommended limit
        if body.jobs.len() > MAX_RECORD_TRANSFER_COUNT as usize {
            error!(
                "Too many jobs in batch: {}. Maximum is {}",
                body.jobs.len(),
                MAX_RECORD_TRANSFER_COUNT
            );
            return Ok(CreateJobsResponse::UnprocessableContentErrorResponse(
                message_error_response(format!(
                    "Too many jobs in batch: {}. Maximum is {}",
                    body.jobs.len(),
                    MAX_RECORD_TRANSFER_COUNT
                )),
            ));
        }

        // Preflight: the `job` table has no UNIQUE (workflow_id, name) constraint;
        // duplicate names in one request would silently commit two rows and then
        // make the post-insert name→id lookup ambiguous. The client's
        // `WorkflowSpec::validate_unique_names_after_expansion` already rejects
        // this, but defend in depth against direct API callers. 422, not 500.
        {
            let mut seen: HashMap<&str, ()> = HashMap::with_capacity(body.jobs.len());
            for job in &body.jobs {
                if seen.insert(job.name.as_str(), ()).is_some() {
                    return Ok(CreateJobsResponse::UnprocessableContentErrorResponse(
                        message_error_response(format!(
                            "Duplicate job name '{}' in bulk create request",
                            job.name
                        )),
                    ));
                }
            }
        }

        // Acquire the SQLite write lock up front. `fetch_workflow_env` reads
        // before the per-job INSERTs, which would risk SQLITE_BUSY_SNAPSHOT
        // under BEGIN DEFERRED if another writer commits in between.
        let mut transaction = match begin_immediate(&self.context.pool).await {
            Ok(tx) => tx,
            Err(e) => return Err(database_error_with_msg(e, "Failed to begin transaction")),
        };

        // Phase A: validate every job and prepare the values we'll INSERT later.
        // This loop touches the DB only via `fetch_workflow_env` (per workflow_id, cached).
        // Each job's env is merged with the workflow env and serialized here so the bulk
        // INSERT closure stays infallible.
        let mut workflow_env_cache: HashMap<i64, Option<HashMap<String, String>>> = HashMap::new();
        let mut prepared: Vec<PreparedJobRow> = Vec::with_capacity(body.jobs.len());
        for mut job in body.jobs {
            if let Err(err) = validate_env_map(job.env.as_ref(), "job env") {
                let _ = transaction.rollback().await;
                return Ok(CreateJobsResponse::UnprocessableContentErrorResponse(
                    message_error_response(err.0),
                ));
            }
            let workflow_env = match workflow_env_cache.get(&job.workflow_id) {
                Some(cached) => cached.clone(),
                None => {
                    let fetched =
                        match Self::fetch_workflow_env(&mut *transaction, job.workflow_id).await {
                            Ok(env) => env,
                            Err(e) => {
                                let _ = transaction.rollback().await;
                                return Err(e);
                            }
                        };
                    workflow_env_cache.insert(job.workflow_id, fetched.clone());
                    fetched
                }
            };
            job.env = Self::merge_env(workflow_env.as_ref(), job.env.as_ref());
            let env_serialized = match serialize_env_map(job.env.clone(), "job env") {
                Ok(env) => env,
                Err(e) => {
                    let _ = transaction.rollback().await;
                    return Err(e);
                }
            };
            let cancel_on_blocking_job_failure = job.cancel_on_blocking_job_failure.unwrap_or(true);
            let supports_termination = job.supports_termination.unwrap_or(false);
            let priority = job.priority.unwrap_or(0);
            if priority < 0 {
                let _ = transaction.rollback().await;
                return Ok(CreateJobsResponse::UnprocessableContentErrorResponse(
                    message_error_response(format!(
                        "priority must be >= 0, got {} for job '{}'",
                        priority, job.name
                    )),
                ));
            }
            if let Err(err) = models::validate_labels(job.labels.as_deref()) {
                let _ = transaction.rollback().await;
                return Ok(CreateJobsResponse::UnprocessableContentErrorResponse(
                    message_error_response(format!("{} for job '{}'", err, job.name)),
                ));
            }
            let status = JobStatus::Uninitialized;
            let status_int = status.to_int();
            job.status = Some(status);
            job.labels =
                models::labels_from_json(models::labels_to_json(job.labels.as_deref()).as_deref());

            prepared.push(PreparedJobRow {
                job,
                env_serialized,
                cancel_on_blocking_job_failure,
                supports_termination,
                priority,
                status_int,
            });
        }

        // Validate references once per field across the whole batch. The transport layer
        // already requires a single workflow_id per batch; re-check for in-process callers.
        let workflow_id = prepared[0].job.workflow_id;
        if let Some(other) = prepared.iter().find(|p| p.job.workflow_id != workflow_id) {
            let _ = transaction.rollback().await;
            return Ok(CreateJobsResponse::UnprocessableContentErrorResponse(
                message_error_response(format!(
                    "All jobs in a batch must have the same workflow_id. Found workflow_ids: {} and {}",
                    workflow_id, other.job.workflow_id
                )),
            ));
        }
        let batch_deps = {
            let mut jobs: Vec<&mut models::JobModel> =
                prepared.iter_mut().map(|p| &mut p.job).collect();
            match resolve_batch_name_refs(&mut transaction, workflow_id, &mut jobs).await {
                Ok(deps) => deps,
                Err(NameRefError::Invalid(msg)) => {
                    let _ = transaction.rollback().await;
                    return Ok(CreateJobsResponse::UnprocessableContentErrorResponse(
                        message_error_response(msg),
                    ));
                }
                Err(NameRefError::Api(e)) => {
                    let _ = transaction.rollback().await;
                    return Err(e);
                }
            }
        };
        let job_refs: Vec<&models::JobModel> = prepared.iter().map(|p| &p.job).collect();
        let refs_check = job_refs_outside_workflow(&mut transaction, workflow_id, &job_refs).await;
        match refs_check {
            Ok(None) => {}
            Ok(Some(msg)) => {
                let _ = transaction.rollback().await;
                return Ok(CreateJobsResponse::UnprocessableContentErrorResponse(
                    message_error_response(msg),
                ));
            }
            Err(e) => {
                let _ = transaction.rollback().await;
                return Err(e);
            }
        }

        // Phase B: bulk INSERT job rows in chunks, capturing each row's generated id
        // by name. Job names are unique per workflow — duplicates after parameter
        // expansion are rejected client-side by
        // `WorkflowSpec::validate_unique_names_after_expansion`. SQLite's RETURNING
        // order is undefined, but a name→id map is order-independent.
        //
        // 12 columns × 200 rows = 2400 bound parameters per statement, safely under
        // SQLITE_MAX_VARIABLE_NUMBER (32766 since 3.32).
        const JOB_ROWS_PER_INSERT: usize = 200;
        let mut id_by_name: HashMap<String, i64> = HashMap::with_capacity(prepared.len());
        for chunk in prepared.chunks(JOB_ROWS_PER_INSERT) {
            let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
                "INSERT INTO job (\
                    workflow_id, name, command, cancel_on_blocking_job_failure, \
                    supports_termination, resource_requirements_id, invocation_script, \
                    env, status, scheduler_id, failure_handler_id, priority, labels\
                ) ",
            );
            qb.push_values(chunk, |mut row, prep| {
                row.push_bind(prep.job.workflow_id)
                    .push_bind(&prep.job.name)
                    .push_bind(&prep.job.command)
                    .push_bind(prep.cancel_on_blocking_job_failure)
                    .push_bind(prep.supports_termination)
                    .push_bind(prep.job.resource_requirements_id)
                    .push_bind(prep.job.invocation_script.as_deref())
                    .push_bind(prep.env_serialized.as_deref())
                    .push_bind(prep.status_int)
                    .push_bind(prep.job.scheduler_id)
                    .push_bind(prep.job.failure_handler_id)
                    .push_bind(prep.priority)
                    .push_bind(models::labels_to_json(prep.job.labels.as_deref()));
            });
            qb.push(" RETURNING id, name");
            let rows = match qb.build().fetch_all(&mut *transaction).await {
                Ok(rows) => rows,
                Err(e) => {
                    let _ = transaction.rollback().await;
                    return Err(database_error_with_msg(
                        e,
                        "Failed to create job records in bulk",
                    ));
                }
            };
            for row in rows {
                let id: i64 = row.get("id");
                let name: String = row.get("name");
                id_by_name.insert(name, id);
            }
        }

        // Phase C: attach ids to each JobModel in input order and gather relationship rows
        // for the five join tables. Each `Vec<(...)>` will be fed into a single chunked
        // INSERT below; for a 22k-job workflow this typically packs 100k+ rows into a few
        // dozen statements instead of ~150k individual prepared INSERTs.
        let mut added_jobs: Vec<models::JobModel> = Vec::with_capacity(prepared.len());
        let mut depends_on_rows: Vec<(i64, i64, i64)> = Vec::new();
        let mut input_file_rows: Vec<(i64, i64, i64)> = Vec::new();
        let mut output_file_rows: Vec<(i64, i64, i64)> = Vec::new();
        let mut input_user_data_rows: Vec<(i64, i64)> = Vec::new();
        let mut output_user_data_rows: Vec<(i64, i64)> = Vec::new();

        let mut new_ids: Vec<i64> = Vec::with_capacity(prepared.len());
        for prep in &prepared {
            match id_by_name.get(&prep.job.name) {
                Some(id) => new_ids.push(*id),
                None => {
                    let _ = transaction.rollback().await;
                    return Err(ApiError(format!(
                        "Bulk job insert returned no id for name '{}'",
                        prep.job.name
                    )));
                }
            }
        }
        drop(id_by_name);

        for ((prep, job_id), deps) in prepared.into_iter().zip(&new_ids).zip(batch_deps) {
            let mut job = prep.job;
            let job_id = *job_id;
            job.id = Some(job_id);
            // Dependencies on jobs of this batch, resolved from name_refs.
            extend_unique(
                &mut job.depends_on_job_ids,
                deps.iter().map(|d| new_ids[*d]).collect(),
            );

            if let Some(ids) = &job.depends_on_job_ids {
                for &blocking_id in ids {
                    depends_on_rows.push((job_id, blocking_id, workflow_id));
                }
            }
            if let Some(ids) = &job.input_file_ids {
                for &file_id in ids {
                    input_file_rows.push((job_id, file_id, workflow_id));
                }
            }
            if let Some(ids) = &job.output_file_ids {
                for &file_id in ids {
                    output_file_rows.push((job_id, file_id, workflow_id));
                }
            }
            if let Some(ids) = &job.input_user_data_ids {
                for &user_data_id in ids {
                    input_user_data_rows.push((job_id, user_data_id));
                }
            }
            if let Some(ids) = &job.output_user_data_ids {
                for &user_data_id in ids {
                    output_user_data_rows.push((job_id, user_data_id));
                }
            }

            added_jobs.push(job);
        }

        // Phase D: bulk INSERT each relationship table. The helpers chunk on a row budget
        // sized for SQLite's bound-parameter limit (3 cols × 1000 = 3000 params).
        if let Err(e) = bulk_insert_job_3col(
            &mut transaction,
            "job_depends_on",
            "(job_id, depends_on_job_id, workflow_id)",
            &depends_on_rows,
        )
        .await
        {
            let _ = transaction.rollback().await;
            return Err(e);
        }
        if let Err(e) = bulk_insert_job_3col(
            &mut transaction,
            "job_input_file",
            "(job_id, file_id, workflow_id)",
            &input_file_rows,
        )
        .await
        {
            let _ = transaction.rollback().await;
            return Err(e);
        }
        if let Err(e) = bulk_insert_job_3col(
            &mut transaction,
            "job_output_file",
            "(job_id, file_id, workflow_id)",
            &output_file_rows,
        )
        .await
        {
            let _ = transaction.rollback().await;
            return Err(e);
        }
        if let Err(e) = bulk_insert_job_2col(
            &mut transaction,
            "job_input_user_data",
            "(job_id, user_data_id)",
            &input_user_data_rows,
        )
        .await
        {
            let _ = transaction.rollback().await;
            return Err(e);
        }
        if let Err(e) = bulk_insert_job_2col(
            &mut transaction,
            "job_output_user_data",
            "(job_id, user_data_id)",
            &output_user_data_rows,
        )
        .await
        {
            let _ = transaction.rollback().await;
            return Err(e);
        }

        // Commit the transaction
        if let Err(e) = transaction.commit().await {
            return Err(database_error_with_msg(e, "Failed to commit transaction"));
        }

        info!(
            "Jobs created workflow_id={} count={}",
            workflow_id,
            added_jobs.len()
        );
        Ok(CreateJobsResponse::SuccessfulResponse(
            models::CreateJobsResponse {
                jobs: Some(added_jobs),
            },
        ))
    }

    /// Delete all jobs for one workflow.
    async fn delete_jobs(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteJobsResponse, ApiError> {
        debug!(
            "delete_jobs({}) - X-Span-ID: {:?}",
            workflow_id,
            context.get().0.clone()
        );

        let result = match sqlx::query!("DELETE FROM job WHERE workflow_id = $1", workflow_id)
            .execute(self.context.pool.as_ref())
            .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to delete jobs"));
            }
        };

        let deleted_count = result.rows_affected() as i64;

        info!(
            "Jobs deleted workflow_id={} count={}",
            workflow_id, deleted_count
        );

        Ok(DeleteJobsResponse::SuccessfulResponse(serde_json::json!({
            "count": deleted_count
        })))
    }

    /// Retrieve a job.
    #[instrument(skip(self, context), fields(job_id = id))]
    async fn get_job(&self, id: i64, context: &C) -> Result<GetJobResponse, ApiError> {
        debug!("get_job({}) - X-Span-ID: {:?}", id, context.get().0.clone());
        match self.get_job_with_relationships(id).await {
            Ok(job) => Ok(GetJobResponse::SuccessfulResponse(job)),
            Err(ApiError(msg)) if msg.contains("not found") => Ok(
                GetJobResponse::NotFoundErrorResponse(resource_not_found_response("Job", id)),
            ),
            Err(e) => Err(e),
        }
    }

    /// Return the resource requirements for jobs with a status of ready.
    #[instrument(skip(self, context), fields(workflow_id = id))]
    async fn get_ready_job_requirements(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetReadyJobRequirementsResponse, ApiError> {
        debug!(
            "get_ready_job_requirements({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );
        error!("get_ready_job_requirements operation is not implemented");
        Err(ApiError("Api-Error: Operation is NOT implemented".into()))
    }

    /// Retrieve all job IDs for one workflow.
    async fn list_job_ids(&self, id: i64, context: &C) -> Result<ListJobIdsResponse, ApiError> {
        debug!(
            "list_job_ids({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        // Query for all job IDs for the given workflow
        let records = match sqlx::query!(
            r#"
            SELECT id
            FROM job
            WHERE workflow_id = $1
            ORDER BY id
            "#,
            id
        )
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(recs) => recs,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to list job IDs"));
            }
        };

        // Extract job IDs into a vector
        let job_ids: Vec<i64> = records.iter().map(|record| record.id).collect();

        debug!("Retrieved {} job IDs for workflow {}", job_ids.len(), id);

        Ok(ListJobIdsResponse::SuccessfulResponse(
            models::ListJobIdsResponse::new(job_ids),
        ))
    }

    /// Retrieve all jobs for one workflow.
    #[allow(clippy::too_many_arguments)]
    #[instrument(skip(self, context), fields(workflow_id, offset, limit))]
    async fn list_jobs(
        &self,
        workflow_id: i64,
        status: Option<Vec<JobStatus>>,
        needs_file_id: Option<i64>,
        upstream_job_id: Option<i64>,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        include_relationships: Option<bool>,
        active_compute_node_id: Option<i64>,
        origin_is_set: Option<bool>,
        name: Option<String>,
        command: Option<String>,
        labels: Option<String>,
        ids: Option<Vec<i64>>,
        context: &C,
    ) -> Result<ListJobsResponse, ApiError> {
        debug!(
            "list_jobs({}, {:?}, {:?}, {:?}, {}, {}, {:?}, {:?}, {:?}, {:?}, {:?}, {:?}, {:?}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            status,
            needs_file_id,
            upstream_job_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            include_relationships,
            active_compute_node_id,
            origin_is_set,
            name,
            command,
            labels,
            context.get().0.clone()
        );

        // Build base query
        let base_query = "SELECT id, workflow_id, name, command, resource_requirements_id, invocation_script, env, status, cancel_on_blocking_job_failure, supports_termination, scheduler_id, failure_handler_id, attempt_id, priority, origin, labels, start_time, compute_node_id FROM job".to_string();

        // Build WHERE clause conditions
        let mut where_conditions = vec!["workflow_id = ?".to_string()];
        let mut bind_values: Vec<Box<dyn sqlx::Encode<'_, sqlx::Sqlite> + Send>> =
            vec![Box::new(workflow_id)];

        if let Some(statuses) = &status {
            where_conditions.push(int_in_clause(
                "status",
                statuses.iter().map(|s| s.to_int() as i64),
            ));
        }

        if let Some(ids) = &ids {
            if ids.len() as i64 > MAX_RECORD_TRANSFER_COUNT {
                return Ok(ListJobsResponse::UnprocessableContentErrorResponse(
                    message_error_response(format!(
                        "ids has {} values; at most {} are allowed",
                        ids.len(),
                        MAX_RECORD_TRANSFER_COUNT
                    )),
                ));
            }
            where_conditions.push(int_in_clause("id", ids.iter().copied()));
        }

        if let Some(file_id) = needs_file_id {
            where_conditions
                .push("id IN (SELECT job_id FROM job_input_file WHERE file_id = ?)".to_string());
            bind_values.push(Box::new(file_id));
        }

        if let Some(upstream_id) = upstream_job_id {
            where_conditions.push(
                "id IN (SELECT job_id FROM job_depends_on WHERE depends_on_job_id = ?)".to_string(),
            );
            bind_values.push(Box::new(upstream_id));
        }

        if active_compute_node_id.is_some() {
            where_conditions.push("compute_node_id = ?".to_string());
        }

        // Filter by provenance: `true` matches `'retry'`/`'spawn'`; `false`
        // matches statically-declared jobs (origin IS NULL). `torc watch
        // --auto-schedule` uses `origin_is_set=true` with `limit=1` to count
        // jobs needing an unplanned allocation without downloading rows.
        if let Some(true) = origin_is_set {
            where_conditions.push("origin IS NOT NULL".to_string());
        } else if let Some(false) = origin_is_set {
            where_conditions.push("origin IS NULL".to_string());
        }

        // Substring filters on name/command (LIKE '%value%'), matching the
        // pattern used by the workflows `description` filter. Bound last so the
        // `?` placeholders line up with the manual binds below.
        if name.is_some() {
            where_conditions.push("name LIKE ? ESCAPE '\\'".to_string());
        }
        if command.is_some() {
            where_conditions.push("command LIKE ? ESCAPE '\\'".to_string());
        }
        // Jobs a runner with these (comma-separated) labels would claim; an empty
        // filter selects unlabeled jobs.
        let label_filter = labels
            .as_deref()
            .map(|l| claimable_labels_sql(&models::labels_from_query(l), "labels", "?"));
        if let Some((predicate, _)) = &label_filter {
            where_conditions.push(predicate.clone());
        }

        let where_clause = where_conditions.join(" AND ");
        let sort_by = if let Some(ref col) = sort_by {
            if JOB_COLUMNS.contains(&col.as_str()) {
                Some(col.clone())
            } else {
                debug!("Invalid sort column requested: {}", col);
                None // Fall back to default
            }
        } else {
            None
        };

        // Build the complete query with pagination and sorting
        let query = SqlQueryBuilder::new(base_query)
            .with_where(where_clause.clone())
            .with_pagination_and_sorting(offset, limit, sort_by, reverse_sort, "id", JOB_COLUMNS)
            .build();

        debug!("Executing query: {}", query);

        // Execute the query
        let mut sqlx_query = sqlx::query(sqlx::AssertSqlSafe(&*query));

        // Bind workflow_id
        sqlx_query = sqlx_query.bind(workflow_id);

        // Bind optional parameters in order
        if let Some(file_id) = needs_file_id {
            sqlx_query = sqlx_query.bind(file_id);
        }
        if let Some(upstream_id) = upstream_job_id {
            sqlx_query = sqlx_query.bind(upstream_id);
        }
        if let Some(cn_id) = active_compute_node_id {
            sqlx_query = sqlx_query.bind(cn_id);
        }
        if let Some(ref n) = name {
            sqlx_query = sqlx_query.bind(format!("%{}%", escape_like_pattern(n)));
        }
        if let Some(ref c) = command {
            sqlx_query = sqlx_query.bind(format!("%{}%", escape_like_pattern(c)));
        }
        if let Some((_, l)) = &label_filter {
            sqlx_query = sqlx_query.bind(l.clone());
        }

        let records = match sqlx_query.fetch_all(self.context.pool.as_ref()).await {
            Ok(recs) => recs,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to list jobs"));
            }
        };

        let mut items: Vec<models::JobModel> = Vec::new();
        let should_include_relationships = include_relationships.unwrap_or(false);

        for record in records {
            let job_id: i64 = record.get("id");

            if should_include_relationships {
                // Fetch the complete job with all relationships
                match self.get_job_with_relationships(job_id).await {
                    Ok(job) => items.push(job),
                    Err(e) => {
                        error!("Failed to get job {} with relationships: {}", job_id, e);
                        return Err(e);
                    }
                }
            } else {
                // Create job model without relationships for better performance
                let status_int: i32 = record.get("status");
                let status = parse_job_status(status_int, job_id)?;

                items.push(models::JobModel {
                    id: Some(record.get("id")),
                    workflow_id: record.get("workflow_id"),
                    name: record.get("name"),
                    command: record.get("command"),
                    env: deserialize_env_map(
                        record
                            .try_get::<Option<String>, _>("env")
                            .map_err(|e| database_error_with_msg(e, "Failed to decode job env"))?,
                        "job env",
                    )?,
                    cancel_on_blocking_job_failure: record
                        .try_get("cancel_on_blocking_job_failure")
                        .ok(),
                    supports_termination: record.try_get("supports_termination").ok(),
                    depends_on_job_ids: None,
                    input_file_ids: None,
                    output_file_ids: None,
                    input_user_data_ids: None,
                    output_user_data_ids: None,
                    resource_requirements_id: record.try_get("resource_requirements_id").ok(),
                    invocation_script: record.try_get("invocation_script").ok(),
                    status: Some(status),
                    start_time: record
                        .try_get::<Option<String>, _>("start_time")
                        .ok()
                        .flatten(),
                    compute_node_id: record
                        .try_get::<Option<i64>, _>("compute_node_id")
                        .ok()
                        .flatten(),
                    scheduler_id: record.try_get("scheduler_id").ok(),
                    schedule_compute_nodes: None,
                    failure_handler_id: record.try_get("failure_handler_id").ok(),
                    attempt_id: record.try_get("attempt_id").ok(),
                    priority: record.try_get("priority").ok(),
                    origin: record.try_get::<Option<String>, _>("origin").ok().flatten(),
                    labels: labels_column(&record, "labels"),
                    name_refs: None,
                });
            }
        }

        // For proper pagination, we should get the total count without LIMIT/OFFSET
        let count_query = SqlQueryBuilder::new("SELECT COUNT(*) as total FROM job".to_string())
            .with_where(where_clause)
            .build();

        let mut count_sqlx_query = sqlx::query(sqlx::AssertSqlSafe(&*count_query));
        count_sqlx_query = count_sqlx_query.bind(workflow_id);
        if let Some(file_id) = needs_file_id {
            count_sqlx_query = count_sqlx_query.bind(file_id);
        }
        if let Some(upstream_id) = upstream_job_id {
            count_sqlx_query = count_sqlx_query.bind(upstream_id);
        }
        if let Some(cn_id) = active_compute_node_id {
            count_sqlx_query = count_sqlx_query.bind(cn_id);
        }
        if let Some(ref n) = name {
            count_sqlx_query = count_sqlx_query.bind(format!("%{}%", escape_like_pattern(n)));
        }
        if let Some(ref c) = command {
            count_sqlx_query = count_sqlx_query.bind(format!("%{}%", escape_like_pattern(c)));
        }
        if let Some((_, l)) = &label_filter {
            count_sqlx_query = count_sqlx_query.bind(l.clone());
        }

        let total_count = match count_sqlx_query.fetch_one(self.context.pool.as_ref()).await {
            Ok(row) => row.get::<i64, _>("total"),
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to list jobs"));
            }
        };

        let response =
            crate::paginated_list_response!(models::ListJobsResponse, items, offset, total_count);

        debug!(
            "list_jobs({}, {}/{}) - X-Span-ID: {:?}",
            workflow_id,
            response.count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListJobsResponse::SuccessfulResponse(response))
    }

    /// Update a job.
    ///
    /// Restrictions:
    /// - Jobs can only be updated when their status is `Uninitialized`
    /// - The job status field itself cannot be modified
    /// - Relationship fields (input_file_ids, output_file_ids, input_user_data_ids, output_user_data_ids) are immutable after job creation
    /// - The `depends_on_job_ids` field can be modified only when the job status is `Uninitialized`
    ///
    /// When `depends_on_job_ids` is modified, the function will:
    /// 1. Delete all existing depends_on relationships for the job
    /// 2. Create new relationships based on the provided IDs
    /// 3. Use a database transaction to ensure consistency
    ///
    async fn update_job(
        &self,
        id: i64,
        body: models::JobModel,
        context: &C,
    ) -> Result<UpdateJobResponse, ApiError> {
        debug!(
            "update_job({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        // Get the existing job with all relationships to validate immutable fields
        let existing_job = match self.get_job_with_relationships(id).await {
            Ok(job) => job,
            Err(ApiError(msg)) if msg.contains("not found") => {
                return Ok(UpdateJobResponse::NotFoundErrorResponse(
                    resource_not_found_response("Job", id),
                ));
            }
            Err(e) => return Err(e),
        };

        // No endpoint writes job.workflow_id, so comparing to the stored value is race-free.
        if body.workflow_id != existing_job.workflow_id {
            return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                super::workflow_id_immutable_response(),
            ));
        }

        // Check if job has a status
        let existing_status = match existing_job.status {
            Some(status) => status,
            None => {
                return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                    message_error_response("Cannot update job - job has no status set"),
                ));
            }
        };

        // Determine if we're only updating fields that are allowed at any time
        // (scheduler_id and resource_requirements_id can be updated regardless of status)
        // All fields are checked by comparing to existing values, since the client may
        // send the full job object with only scheduler_id/resource_requirements_id changed
        // Note: If body field is None, we treat it as "not changing" that field.
        // Vector relationship fields are compared as sets (order-independent) so a
        // re-send of the same IDs in a different order is not flagged as a change.
        let name_changed = body.name != existing_job.name;
        let command_changed = body.command != existing_job.command;
        let body_env = normalize_env_map(body.env.clone());
        let env_changed = body.env.is_some() && body_env != existing_job.env;
        let status_changed = body.status.is_some() && body.status != existing_job.status;
        let input_file_ids_changed =
            vec_id_set_changed(&body.input_file_ids, &existing_job.input_file_ids);
        let output_file_ids_changed =
            vec_id_set_changed(&body.output_file_ids, &existing_job.output_file_ids);
        let input_user_data_ids_changed =
            vec_id_set_changed(&body.input_user_data_ids, &existing_job.input_user_data_ids);
        let output_user_data_ids_changed = vec_id_set_changed(
            &body.output_user_data_ids,
            &existing_job.output_user_data_ids,
        );
        let depends_on_job_ids_changed =
            vec_id_set_changed(&body.depends_on_job_ids, &existing_job.depends_on_job_ids);

        let has_restricted_updates = name_changed
            || command_changed
            || env_changed
            || status_changed
            || input_file_ids_changed
            || output_file_ids_changed
            || input_user_data_ids_changed
            || output_user_data_ids_changed
            || depends_on_job_ids_changed;
        let only_updating_always_allowed_fields = !has_restricted_updates;

        // Restriction 1: Most updates are only allowed if the job status is Uninitialized
        // Exception: scheduler_id and resource_requirements_id can be updated at any time
        if existing_status != JobStatus::Uninitialized && !only_updating_always_allowed_fields {
            // Build detailed error message showing which fields changed
            let mut changed_fields = Vec::new();
            if name_changed {
                changed_fields.push(format!("name: '{}' -> '{}'", existing_job.name, body.name));
            }
            if command_changed {
                changed_fields.push("command".to_string());
            }
            if env_changed {
                changed_fields.push("env".to_string());
            }
            if status_changed {
                changed_fields.push(format!(
                    "status: {:?} -> {:?}",
                    existing_job.status, body.status
                ));
            }
            if input_file_ids_changed {
                changed_fields.push("input_file_ids".to_string());
            }
            if output_file_ids_changed {
                changed_fields.push("output_file_ids".to_string());
            }
            if input_user_data_ids_changed {
                changed_fields.push("input_user_data_ids".to_string());
            }
            if output_user_data_ids_changed {
                changed_fields.push("output_user_data_ids".to_string());
            }
            if depends_on_job_ids_changed {
                changed_fields.push("depends_on_job_ids".to_string());
            }

            return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                message_error_response(format!(
                    "Cannot update job {} when status is '{}' - most updates are only allowed when status is 'uninitialized'. \
                     Only scheduler_id and resource_requirements_id can be updated at any time. \
                     Changed fields: [{}]",
                    id,
                    existing_status,
                    changed_fields.join(", ")
                )),
            ));
        }

        // Restriction 2: Updating job status is not allowed, except to Disabled
        if let Some(new_status) = &body.status
            && let Some(ref existing_status) = existing_job.status
            && *new_status != *existing_status
            && *new_status != models::JobStatus::Disabled
        {
            return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                message_error_response(
                    "Cannot update job status - this field is immutable after job creation (except to Disabled)",
                ),
            ));
        }

        // Validate immutable fields. `input_file_ids`, `output_file_ids`,
        // `input_user_data_ids`, `output_user_data_ids`, and `env` are immutable after
        // job creation (`depends_on_job_ids` is mutable and handled separately below).
        // The *_changed flags above already use set-comparison semantics, so a single
        // check per field is enough.
        if input_file_ids_changed {
            return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                message_error_response(
                    "Cannot modify input_file_ids - this field is immutable after job creation",
                ),
            ));
        }
        if output_file_ids_changed {
            return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                message_error_response(
                    "Cannot modify output_file_ids - this field is immutable after job creation",
                ),
            ));
        }
        if input_user_data_ids_changed {
            return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                message_error_response(
                    "Cannot modify input_user_data_ids - this field is immutable after job creation",
                ),
            ));
        }
        if output_user_data_ids_changed {
            return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                message_error_response(
                    "Cannot modify output_user_data_ids - this field is immutable after job creation",
                ),
            ));
        }
        if env_changed {
            return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                message_error_response(
                    "Cannot modify env - this field is immutable after job creation",
                ),
            ));
        }

        // Update the job (only non-relationship fields)
        let status_int = body.status.map(|s| s.to_int());

        if let Some(p) = body.priority
            && p < 0
        {
            return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                message_error_response(format!("priority must be >= 0, got {}", p)),
            ));
        }

        // An empty label set clears the labels (see the NULLIF below); a missing
        // one keeps them.
        if let Err(err) = models::validate_labels(body.labels.as_deref()) {
            return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                message_error_response(err),
            ));
        }

        // Only check references this update writes and changes, so a client re-sending the
        // full job object isn't blocked by values already stored.
        let mut refs =
            models::JobModel::new(existing_job.workflow_id, String::new(), String::new());
        if depends_on_job_ids_changed {
            refs.depends_on_job_ids = body.depends_on_job_ids.clone();
        }
        refs.resource_requirements_id = body
            .resource_requirements_id
            .filter(|id| Some(*id) != existing_job.resource_requirements_id);
        refs.scheduler_id = body
            .scheduler_id
            .filter(|id| Some(*id) != existing_job.scheduler_id);
        let mut conn = self
            .context
            .pool
            .acquire()
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to acquire connection"))?;
        if let Some(msg) =
            job_refs_outside_workflow(&mut conn, existing_job.workflow_id, &[&refs]).await?
        {
            return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                message_error_response(msg),
            ));
        }
        drop(conn);

        // `Some("[]")` clears, `None` keeps; labels_to_json folds an empty set to None.
        let labels_update = body
            .labels
            .as_deref()
            .map(|l| models::labels_to_json(Some(l)).unwrap_or_else(|| "[]".to_string()));
        let result = match sqlx::query(
            r#"
            UPDATE job
            SET
                name = COALESCE(?, name)
                ,status = COALESCE(?, status)
                ,command = COALESCE(?, command)
                ,invocation_script = COALESCE(?, invocation_script)
                ,cancel_on_blocking_job_failure = COALESCE(?, cancel_on_blocking_job_failure)
                ,supports_termination = COALESCE(?, supports_termination)
                ,resource_requirements_id = COALESCE(?, resource_requirements_id)
                ,scheduler_id = COALESCE(?, scheduler_id)
                ,priority = COALESCE(?, priority)
                ,labels = CASE WHEN ? IS NULL THEN labels ELSE NULLIF(?, '[]') END
            WHERE id = ?
        "#,
        )
        .bind(body.name)
        .bind(status_int)
        .bind(body.command)
        .bind(body.invocation_script)
        .bind(body.cancel_on_blocking_job_failure)
        .bind(body.supports_termination)
        .bind(body.resource_requirements_id)
        .bind(body.scheduler_id)
        .bind(body.priority)
        .bind(labels_update.clone())
        .bind(labels_update)
        .bind(id)
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to update job"));
            }
        };

        if result.rows_affected() == 0 {
            return Ok(UpdateJobResponse::NotFoundErrorResponse(
                resource_not_found_response("Job", id),
            ));
        }

        // If depends_on_job_ids was modified, update the relationships
        if depends_on_job_ids_changed {
            // Start a transaction for relationship updates
            let mut tx = match self.context.pool.begin().await {
                Ok(tx) => tx,
                Err(e) => return Err(database_error_with_msg(e, "Failed to begin transaction")),
            };

            // Delete existing depends_on relationships for this job
            crate::tx_try!(
                tx,
                sqlx::query!("DELETE FROM job_depends_on WHERE job_id = $1", id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| database_error_with_msg(
                        e,
                        "Failed to delete old job dependencies"
                    ))
            );

            // Add new depends_on relationships if provided
            if let Some(depends_on_ids) = &body.depends_on_job_ids {
                for blocking_id in depends_on_ids {
                    crate::tx_try!(
                        tx,
                        sqlx::query!(
                            "INSERT INTO job_depends_on (job_id, depends_on_job_id, workflow_id) VALUES ($1, $2, $3)",
                            id,
                            *blocking_id,
                            existing_job.workflow_id
                        )
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| database_error_with_msg(e, "Failed to update job dependencies"))
                    );
                }
            }

            // Commit the transaction
            if let Err(e) = tx.commit().await {
                return Err(database_error_with_msg(e, "Failed to commit transaction"));
            }
        }

        // Return the updated job by fetching it again with relationships
        let updated_job = self.get_job_with_relationships(id).await?;

        debug!("Updated job with id: {}", id);
        Ok(UpdateJobResponse::SuccessfulResponse(updated_job))
    }

    /// Update a job's status only.
    ///
    /// This function updates only the status field with no restrictions.
    /// All other job fields remain unchanged.
    #[instrument(skip(self, context), fields(job_id = id, status = ?status))]
    async fn update_job_status(
        &self,
        id: i64,
        status: JobStatus,
        context: &C,
    ) -> Result<UpdateJobResponse, ApiError> {
        debug!(
            "update_job_status({}, {:?}) - X-Span-ID: {:?}",
            id,
            status,
            context.get().0.clone()
        );

        let status_int = status.to_int();

        let result = match sqlx::query!(
            r#"
            UPDATE job
            SET status = $1
            WHERE id = $2
            "#,
            status_int,
            id,
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to update job status"));
            }
        };

        if result.rows_affected() == 0 {
            return Ok(UpdateJobResponse::NotFoundErrorResponse(
                resource_not_found_response("Job", id),
            ));
        }

        // Return the updated job by fetching it again with relationships
        let updated_job = self.get_job_with_relationships(id).await?;

        debug!("Updated job status for job id: {}", id);
        Ok(UpdateJobResponse::SuccessfulResponse(updated_job))
    }

    /// Return user-requested number of jobs that are ready for submission. Sets status to pending, or running when `start` is set.
    #[instrument(skip(self, context), fields(workflow_id = id, requested_job_count))]
    async fn claim_next_jobs(
        &self,
        id: i64,
        requested_job_count: i64,
        labels: Option<String>,
        start: Option<ClaimStart>,
        context: &C,
    ) -> Result<ClaimNextJobsResponse, ApiError> {
        debug!(
            "claim_next_jobs({}, {}) - X-Span-ID: {:?}",
            id,
            requested_job_count,
            context.get().0.clone()
        );

        let mut conn = self.context.pool.acquire().await.map_err(|e| {
            error!("Failed to acquire database connection: {}", e);
            ApiError("Database connection error".to_string())
        })?;

        // A draft is never runnable, so nothing is claimed from it, as for a canceled workflow.
        let workflow_is_canceled =
            match sqlx::query("SELECT is_canceled, is_draft FROM workflow WHERE id = $1")
                .bind(id)
                .fetch_optional(&mut *conn)
                .await
            {
                Ok(Some(row)) => {
                    row.get::<bool, _>("is_canceled") || row.get::<bool, _>("is_draft")
                }
                Ok(None) => {
                    return Ok(ClaimNextJobsResponse::NotFoundErrorResponse(
                        resource_not_found_response("Workflow", id),
                    ));
                }
                Err(e) => {
                    error!("Failed to query workflow cancellation status: {}", e);
                    return Err(ApiError("Database error".to_string()));
                }
            };

        if workflow_is_canceled {
            // Nothing is claimed, but still reject a stale run so its runner stops.
            let valid = claim_start_is_valid(&mut conn, id, start)
                .await
                .map_err(|e| {
                    error!("Database error validating claim start: {}", e);
                    ApiError("Database error".to_string())
                })?;
            if !valid {
                return Ok(ClaimNextJobsResponse::UnprocessableContentErrorResponse(
                    message_error_response(CLAIM_START_MISMATCH),
                ));
            }
            return Ok(ClaimNextJobsResponse::SuccessfulResponse(
                models::ClaimNextJobsResponse { jobs: Some(vec![]) },
            ));
        }

        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(|e| {
                error!("Failed to begin immediate transaction: {}", e);
                ApiError("Database lock error".to_string())
            })?;

        let ready_status = models::JobStatus::Ready.to_int();
        let (labels_predicate, labels_json) = claimable_labels_sql(
            &models::labels_from_query(labels.as_deref().unwrap_or("")),
            "labels",
            "$3",
        );
        let query = format!(
            r#"
            SELECT
                id as job_id,
                workflow_id,
                name,
                command,
                invocation_script,
                env,
                status,
                cancel_on_blocking_job_failure,
                supports_termination,
                resource_requirements_id,
                failure_handler_id,
                attempt_id,
                priority,
                origin,
                labels
            FROM job
            WHERE workflow_id = $1 AND status = $2 AND {labels_predicate}
            ORDER BY priority DESC, id ASC
            LIMIT $4
            "#
        );

        let rows = match sqlx::query(sqlx::AssertSqlSafe(&*query))
            .bind(id)
            .bind(ready_status)
            .bind(labels_json)
            .bind(requested_job_count)
            .fetch_all(&mut *conn)
            .await
        {
            Ok(rows) => rows,
            Err(e) => {
                error!("Database error in claim_next_jobs: {}", e);
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                return Err(ApiError("Database error".to_string()));
            }
        };

        debug!(
            "claim_next_jobs: Found {} jobs for workflow {}",
            rows.len(),
            id
        );

        let mut selected_jobs = Vec::new();

        for row in rows {
            let job_id: i64 = row.get("job_id");

            let job = models::JobModel {
                id: Some(job_id),
                workflow_id: row.get("workflow_id"),
                name: row.get("name"),
                command: row.get("command"),
                env: deserialize_env_map(row.get("env"), "job env")?,
                invocation_script: row.get("invocation_script"),
                status: Some(models::JobStatus::Pending),
                start_time: None,
                compute_node_id: None,
                schedule_compute_nodes: None,
                cancel_on_blocking_job_failure: Some(row.get("cancel_on_blocking_job_failure")),
                supports_termination: Some(row.get("supports_termination")),
                depends_on_job_ids: None,
                input_file_ids: None,
                output_file_ids: None,
                input_user_data_ids: None,
                output_user_data_ids: None,
                resource_requirements_id: Some(row.get("resource_requirements_id")),
                scheduler_id: None,
                failure_handler_id: row.get("failure_handler_id"),
                attempt_id: row.get("attempt_id"),
                priority: row.try_get("priority").ok(),
                origin: row.try_get::<Option<String>, _>("origin").ok().flatten(),
                labels: labels_column(&row, "labels"),
                name_refs: None,
            };

            selected_jobs.push(job);
        }

        match finish_claim(&mut conn, id, &mut selected_jobs, start).await {
            Ok(true) => {}
            Ok(false) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                return Ok(ClaimNextJobsResponse::UnprocessableContentErrorResponse(
                    message_error_response(CLAIM_START_MISMATCH),
                ));
            }
            Err(e) => {
                error!("Failed to finish claim for workflow_id={}: {}", id, e);
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                return Err(ApiError("Database update error".to_string()));
            }
        }

        if let Err(e) = sqlx::query("COMMIT").execute(&mut *conn).await {
            error!("Failed to commit transaction: {}", e);
            if let Err(rollback_err) = sqlx::query("ROLLBACK").execute(&mut *conn).await {
                error!("Failed to rollback after commit failure: {}", rollback_err);
            }
            return Err(ApiError("Database commit error".to_string()));
        }

        Ok(ClaimNextJobsResponse::SuccessfulResponse(
            models::ClaimNextJobsResponse {
                jobs: Some(selected_jobs),
            },
        ))
    }

    /// Check for changed job inputs and update status accordingly.
    /// IMPORTANT: All status updates are performed within a transaction (all or none).
    #[instrument(skip(self, context), fields(workflow_id = id, dry_run))]
    async fn process_changed_job_inputs(
        &self,
        id: i64,
        dry_run: bool,
        context: &C,
    ) -> Result<ProcessChangedJobInputsResponse, ApiError> {
        debug!(
            "process_changed_job_inputs(workflow_id={}, dry_run={}) - X-Span-ID: {:?}",
            id,
            dry_run,
            context.get().0.clone()
        );

        // Get all jobs for this workflow
        let jobs = match sqlx::query!(
            r#"
            SELECT id, name
            FROM job
            WHERE workflow_id = $1
            ORDER BY id
            "#,
            id
        )
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(jobs) => jobs,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to list jobs for hash check",
                ));
            }
        };

        let mut jobs_to_reinitialize = Vec::new();
        let uninitialized_status = models::JobStatus::Uninitialized.to_int();

        // First pass: identify jobs with changed inputs
        for job_row in &jobs {
            let job_id = job_row.id;
            let job_name = &job_row.name;

            // Compute current input hash
            let current_hash = match self.compute_job_input_hash(job_id).await {
                Ok(hash) => hash,
                Err(e) => {
                    error!("Failed to compute hash for job {}: {}", job_id, e);
                    return Err(ApiError(format!(
                        "Failed to compute hash for job {}: {}",
                        job_id, e
                    )));
                }
            };

            // Get stored hash
            let stored_hash = match self.get_stored_job_input_hash(job_id).await {
                Ok(Some(hash)) => hash,
                Ok(None) => {
                    debug!("No stored hash for job {}, skipping", job_id);
                    continue; // No stored hash, skip this job
                }
                Err(e) => {
                    error!("Failed to retrieve stored hash for job {}: {}", job_id, e);
                    return Err(ApiError(format!(
                        "Failed to retrieve stored hash for job {}: {}",
                        job_id, e
                    )));
                }
            };

            // Compare hashes
            if current_hash != stored_hash {
                debug!(
                    "job_id={} name='{}' input hash changed: stored={}, current={}",
                    job_id, job_name, stored_hash, current_hash
                );
                jobs_to_reinitialize.push((job_id, job_name.clone()));
            }
        }

        // Second pass: update job statuses within a transaction (all or none)
        if !dry_run && !jobs_to_reinitialize.is_empty() {
            let mut tx = match self.context.pool.begin().await {
                Ok(tx) => tx,
                Err(e) => {
                    return Err(database_error_with_msg(e, "Failed to begin transaction"));
                }
            };

            for (job_id, job_name) in &jobs_to_reinitialize {
                match sqlx::query!(
                    r#"
                    UPDATE job
                    SET status = $1
                    WHERE id = $2
                    "#,
                    uninitialized_status,
                    job_id
                )
                .execute(&mut *tx)
                .await
                {
                    Ok(_) => {
                        debug!(
                            "Set job {} ({}) to Uninitialized due to input change",
                            job_id, job_name
                        );
                    }
                    Err(e) => {
                        let _ = tx.rollback().await;
                        return Err(database_error_with_msg(
                            e,
                            "Failed to update job status during reinitialization",
                        ));
                    }
                }
            }

            // Commit the transaction
            if let Err(e) = tx.commit().await {
                return Err(database_error_with_msg(e, "Failed to commit transaction"));
            }

            debug!(
                "Successfully reinitialized {} jobs for workflow {} in transaction",
                jobs_to_reinitialize.len(),
                id
            );
        }

        let reinitialized_jobs: Vec<String> = jobs_to_reinitialize
            .into_iter()
            .map(|(_, name)| name)
            .collect();

        debug!(
            "Processed changed job inputs for workflow {}: {} jobs {}",
            id,
            reinitialized_jobs.len(),
            if dry_run { "would be" } else { "were" }
        );

        let response = models::ProcessChangedJobInputsResponse { reinitialized_jobs };

        Ok(ProcessChangedJobInputsResponse::SuccessfulResponse(
            response,
        ))
    }

    /// Delete a job.
    async fn delete_job(&self, id: i64, context: &C) -> Result<DeleteJobResponse, ApiError> {
        debug!(
            "delete_job({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        // First get the job to ensure it exists and extract the JobModel
        let job = match self.get_job(id, context).await? {
            GetJobResponse::SuccessfulResponse(job) => job,
            GetJobResponse::ForbiddenErrorResponse(err) => {
                return Ok(DeleteJobResponse::ForbiddenErrorResponse(err));
            }
            GetJobResponse::NotFoundErrorResponse(err) => {
                return Ok(DeleteJobResponse::NotFoundErrorResponse(err));
            }
            GetJobResponse::DefaultErrorResponse(_) => {
                error!("Failed to get job {} before deletion", id);
                return Err(ApiError("Failed to get job".to_string()));
            }
        };

        match sqlx::query!(r#"DELETE FROM job WHERE id = $1"#, id)
            .execute(self.context.pool.as_ref())
            .await
        {
            Ok(res) => {
                if res.rows_affected() > 1 {
                    return Err(database_error_with_msg(
                        "Unexpected number of rows affected",
                        "Failed to delete job",
                    ));
                } else if res.rows_affected() == 0 {
                    return Err(database_error_with_msg(
                        "No rows affected",
                        "Failed to delete job",
                    ));
                } else {
                    info!(
                        "Job deleted workflow_id={} job_id={} job_name={}",
                        job.workflow_id, id, job.name
                    );
                    Ok(DeleteJobResponse::SuccessfulResponse(job))
                }
            }
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to delete job"));
            }
        }
    }

    /// Reset status for jobs to uninitialized.
    async fn reset_job_status(
        &self,
        id: i64,
        failed_only: bool,
        context: &C,
    ) -> Result<ResetJobStatusResponse, ApiError> {
        debug!(
            "reset_job_status({}, {}) - X-Span-ID: {:?}",
            id,
            failed_only,
            context.get().0.clone()
        );

        if failed_only {
            return self.reset_failed_jobs_only(id).await;
        }

        // Update all jobs with the given workflow_id that are not already uninitialized
        let uninitialized_status = JobStatus::Uninitialized.to_int();

        let result = match sqlx::query!(
            r#"
            UPDATE job
            SET status = $1, start_time = NULL, compute_node_id = NULL
            WHERE workflow_id = $2 AND status != $1
            "#,
            uninitialized_status,
            id
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to reset job status"));
            }
        };

        let updated_count = result.rows_affected();

        info!(
            "Jobs status reset workflow_id={} count={} new_status=uninitialized",
            id, updated_count
        );

        Ok(ResetJobStatusResponse::SuccessfulResponse(
            models::ResetJobStatusResponse::new(
                id,
                updated_count as i64,
                JobStatus::Uninitialized.to_string(),
            ),
        ))
    }

    /// Retry a failed job by resetting its status to Ready and incrementing attempt_id.
    async fn retry_job(
        &self,
        id: i64,
        run_id: i64,
        context: &C,
    ) -> Result<RetryJobResponse, ApiError> {
        debug!(
            "retry_job({}, {}) - X-Span-ID: {:?}",
            id,
            run_id,
            context.get().0.clone()
        );

        // Use BEGIN IMMEDIATE to acquire a write lock immediately,
        // preventing race conditions where multiple processes might try to
        // retry the same job simultaneously.
        // Note: We use pool.acquire() + raw BEGIN IMMEDIATE instead of pool.begin()
        // because pool.begin() starts a DEFERRED transaction, and issuing
        // BEGIN IMMEDIATE inside an existing transaction always fails in SQLite.
        let mut conn = match self.context.pool.acquire().await {
            Ok(conn) => conn,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to acquire database connection",
                ));
            }
        };

        if let Err(e) = sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await {
            return Err(database_error_with_msg(
                e,
                "Failed to begin immediate transaction for retry",
            ));
        }

        // Get the job and verify it's in a retryable state
        // Using sqlx::query instead of sqlx::query! to handle nullable columns properly
        let job_record = match sqlx::query(
            r#"
            SELECT j.id, j.workflow_id, j.name, j.command, j.status, j.failure_handler_id, j.attempt_id,
                   j.invocation_script, j.env, j.cancel_on_blocking_job_failure, j.supports_termination,
                   j.resource_requirements_id, j.scheduler_id, j.priority, j.origin, j.labels,
                   w.run_id as workflow_run_id, fh.rules as failure_handler_rules
            FROM job j
            JOIN workflow w ON j.workflow_id = w.id
            LEFT JOIN failure_handler fh ON j.failure_handler_id = fh.id
            WHERE j.id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *conn)
        .await
        {
            Ok(Some(record)) => record,
            Ok(None) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                return Ok(RetryJobResponse::NotFoundErrorResponse(
                    resource_not_found_response("Job", id),
                ));
            }
            Err(e) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                return Err(database_error_with_msg(e, "Failed to get job record for retry"));
            }
        };

        // Extract fields from the row
        let job_id: i64 = job_record.get("id");
        let workflow_id: i64 = job_record.get("workflow_id");
        let name: String = job_record.get("name");
        let command: String = job_record.get("command");
        let status_int: i32 = job_record.get("status");
        let failure_handler_id: Option<i64> = job_record.get("failure_handler_id");
        let attempt_id: i64 = job_record.get("attempt_id");
        let invocation_script: Option<String> = job_record.get("invocation_script");
        let env: Option<HashMap<String, String>> =
            deserialize_env_map(job_record.get("env"), "job env")?;
        let cancel_on_blocking_job_failure: Option<bool> =
            job_record.get("cancel_on_blocking_job_failure");
        let supports_termination: Option<bool> = job_record.get("supports_termination");
        let resource_requirements_id: Option<i64> = job_record.get("resource_requirements_id");
        let scheduler_id: Option<i64> = job_record.get("scheduler_id");
        let workflow_run_id: i64 = job_record.get("workflow_run_id");
        let existing_origin: Option<String> = job_record.try_get("origin").ok().flatten();

        // Verify run_id matches
        if workflow_run_id != run_id {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            return Ok(RetryJobResponse::UnprocessableContentErrorResponse(
                message_error_response(format!(
                    "{}: provided {} but workflow is at run {}",
                    crate::STALE_RUN,
                    run_id,
                    workflow_run_id
                )),
            ));
        }

        // Verify job is in a retryable state (Running, Failed, or Terminated)
        // Note: Running is allowed because the job runner may call retry_job before complete_job
        // when handling failure recovery (the job has finished locally but the server hasn't been
        // notified yet).
        let current_status = match parse_job_status(status_int, job_id) {
            Ok(s) => s,
            Err(e) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                return Err(e);
            }
        };

        // PendingFailed is retried when AI classification decides the failure was transient.
        if current_status != JobStatus::Running
            && current_status != JobStatus::Failed
            && current_status != JobStatus::Terminated
            && current_status != JobStatus::PendingFailed
        {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            return Ok(RetryJobResponse::UnprocessableContentErrorResponse(
                message_error_response(format!(
                    "Job cannot be retried: status is {:?}, must be Running, Failed, Terminated, or PendingFailed",
                    current_status
                )),
            ));
        }

        // Enforce the failure handler's retry limit. The job runner matches a rule by exit code
        // before calling this endpoint, but the server does not know the exit code yet (the
        // runner retries before complete_job), so it enforces the largest max_retries among the
        // handler's rules. attempt_id counts every attempt, so retries decided by the AI
        // classifier or requested from the TUI consume the same budget. A job without a failure
        // handler has no configured limit.
        let rules: Option<String> = job_record.get("failure_handler_rules");
        let max_retries = match rules.as_deref().map(max_retries_from_rules).transpose() {
            Ok(limit) => limit.flatten(),
            Err(e) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                return Err(ApiError(format!(
                    "Failed to parse failure handler rules for job_id={}: {}",
                    job_id, e
                )));
            }
        };
        if let Some(max_retries) = max_retries
            && attempt_id >= max_retries
        {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            return Ok(RetryJobResponse::UnprocessableContentErrorResponse(
                message_error_response(format!(
                    "Job cannot be retried: attempt_id={} >= max_retries={} (failure handler limit)",
                    attempt_id, max_retries
                )),
            ));
        }

        // Get current attempt_id and increment
        let new_attempt = attempt_id + 1;

        // Update job status to Ready, increment attempt_id, and tag origin
        // as 'retry' *only if it's not already set* so `torc watch
        // --auto-schedule` recognizes this row as needing an unplanned
        // allocation. COALESCE preserves the original provenance: a job
        // created by `spawn_jobs` (origin='spawn') that later gets retried
        // keeps `origin='spawn'`, because "why does this job exist" is still
        // "spawn_jobs added it at runtime" — it just happens to be on its
        // 2nd attempt. The watch detector keys on `origin IS NOT NULL` and
        // doesn't care which non-null value it sees.
        let ready_status = JobStatus::Ready.to_int();
        if let Err(e) = sqlx::query(
            r#"
            UPDATE job
            SET status = ?, attempt_id = ?, origin = COALESCE(origin, 'retry'),
                start_time = NULL, compute_node_id = NULL
            WHERE id = ?
            "#,
        )
        .bind(ready_status)
        .bind(new_attempt)
        .bind(id)
        .execute(&mut *conn)
        .await
        {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            return Err(database_error_with_msg(
                e,
                "Failed to update job status for retry",
            ));
        }

        // Create an event for the retry (within the transaction)
        let event_data = serde_json::json!({
            "event_type": "job_retried",
            "job_id": id,
            "job_name": name,
            "previous_attempt": attempt_id,
            "new_attempt": new_attempt,
            "run_id": run_id,
            "message": format!("Job with name = {} retried: attempt {} -> {}", name, attempt_id, new_attempt),
        });
        let timestamp = Utc::now().timestamp_millis();

        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO event (workflow_id, timestamp, data)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(workflow_id)
        .bind(timestamp)
        .bind(event_data.to_string())
        .execute(&mut *conn)
        .await
        {
            // Log the error but don't fail the retry operation
            error!("Failed to create retry event for job {}: {}", id, e);
        }

        // Commit the transaction. If COMMIT fails (e.g. SQLITE_BUSY in WAL mode),
        // the transaction may remain active. Best-effort ROLLBACK to avoid returning
        // a pooled connection with an open transaction/write lock.
        if let Err(e) = sqlx::query("COMMIT").execute(&mut *conn).await {
            if let Err(rollback_err) = sqlx::query("ROLLBACK").execute(&mut *conn).await {
                error!("Failed to rollback after commit failure: {}", rollback_err);
            }
            return Err(database_error_with_msg(e, "Failed to commit transaction"));
        }

        info!(
            "Job retried workflow_id={} job_id={} job_name={} run_id={} attempt_id={} new_attempt_id={} status=ready",
            workflow_id, id, name, run_id, attempt_id, new_attempt
        );

        // Return updated job model
        let status = JobStatus::Ready;
        let priority: Option<i64> = job_record.get("priority");
        let job_model = models::JobModel {
            id: Some(job_id),
            workflow_id,
            name,
            command,
            invocation_script,
            env,
            status: Some(status),
            start_time: None,
            compute_node_id: None,
            schedule_compute_nodes: None,
            cancel_on_blocking_job_failure,
            supports_termination,
            depends_on_job_ids: None,
            input_file_ids: None,
            output_file_ids: None,
            input_user_data_ids: None,
            output_user_data_ids: None,
            resource_requirements_id,
            scheduler_id,
            failure_handler_id,
            attempt_id: Some(new_attempt),
            priority,
            // Mirror the UPDATE above: COALESCE(existing, 'retry'). A
            // previously-spawned row keeps origin='spawn'; a fresh retry of
            // a statically-declared job becomes 'retry'.
            origin: Some(existing_origin.unwrap_or_else(|| "retry".to_string())),
            labels: labels_column(&job_record, "labels"),
            name_refs: None,
        };

        Ok(RetryJobResponse::SuccessfulResponse(job_model))
    }
}
