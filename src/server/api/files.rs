//! File-related API endpoints

#![allow(clippy::too_many_arguments)]

use crate::server::transport_types::context_types::{ApiError, Has, XSpanIdString};
use async_trait::async_trait;
use log::{debug, info};
use sqlx::Row;

use crate::server::api_responses::{
    CreateFileResponse, CreateFilesResponse, DeleteFileResponse, DeleteFilesResponse,
    GetFileResponse, ListFilesResponse, ListRequiredExistingFilesResponse, UpdateFileResponse,
    UpdateFileStatsResponse,
};

use crate::models;

use super::{
    ApiContext, MAX_RECORD_TRANSFER_COUNT, SqlQueryBuilder, begin_immediate,
    database_error_with_msg, database_lock_aware_error, message_error_response,
    resource_not_found_response,
};

/// Trait defining file-related API operations
#[async_trait]
pub trait FilesApi<C> {
    /// Store a file.
    async fn create_file(
        &self,
        mut file: models::FileModel,
        context: &C,
    ) -> Result<CreateFileResponse, ApiError>;

    /// Store many files in a single transaction.
    async fn create_files(
        &self,
        body: models::FilesModel,
        context: &C,
    ) -> Result<CreateFilesResponse, ApiError>;

    /// Delete all files for one workflow.
    async fn delete_files(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteFilesResponse, ApiError>;

    /// Retrieve a file by ID.
    async fn get_file(&self, id: i64, context: &C) -> Result<GetFileResponse, ApiError>;

    /// Retrieve all files for one workflow.
    async fn list_files(
        &self,
        workflow_id: i64,
        produced_by_job_id: Option<i64>,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        name: Option<String>,
        path: Option<String>,
        is_output: Option<bool>,
        context: &C,
    ) -> Result<ListFilesResponse, ApiError>;

    /// Return files that are marked as required to exist but don't exist.
    async fn list_required_existing_files(
        &self,
        id: i64,
        context: &C,
    ) -> Result<ListRequiredExistingFilesResponse, ApiError>;

    /// Update a file.
    async fn update_file(
        &self,
        id: i64,
        body: models::FileModel,
        context: &C,
    ) -> Result<UpdateFileResponse, ApiError>;

    /// Delete a file.
    async fn delete_file(&self, id: i64, context: &C) -> Result<DeleteFileResponse, ApiError>;

    /// Compare a client's file stat snapshot with stored mtimes, store the changes, and reset
    /// affected jobs and their downstream jobs.
    async fn update_file_stats(
        &self,
        workflow_id: i64,
        body: models::FileStatsRequest,
        dry_run: bool,
        context: &C,
    ) -> Result<UpdateFileStatsResponse, ApiError>;
}

/// Implementation of files API for the server
#[derive(Clone)]
pub struct FilesApiImpl {
    pub context: ApiContext,
}

const FILE_COLUMNS: &[&str] = &["id", "workflow_id", "name", "path", "st_mtime"];

impl FilesApiImpl {
    pub fn new(context: ApiContext) -> Self {
        Self { context }
    }
}

#[async_trait]
impl<C> FilesApi<C> for FilesApiImpl
where
    C: Has<XSpanIdString> + Send + Sync,
{
    /// Store a file.
    async fn create_file(
        &self,
        mut file: models::FileModel,
        context: &C,
    ) -> Result<CreateFileResponse, ApiError> {
        debug!(
            "create_file({:?}) - X-Span-ID: {:?}",
            file,
            context.get().0.clone()
        );

        let result = match sqlx::query!(
            r#"
            INSERT INTO file
            (
                workflow_id,
                name,
                path,
                st_mtime
            )
            VALUES ($1, $2, $3, $4)
            RETURNING rowid AS "id!"
            "#,
            file.workflow_id,
            file.name,
            file.path,
            file.st_mtime,
        )
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to create file record"));
            }
        };

        file.id = Some(result.id);
        Ok(CreateFileResponse::SuccessfulResponse(file))
    }

    /// Store many files in a single transaction.
    ///
    /// Uses multi-row `INSERT … VALUES (…), (…), … RETURNING id, name` so a
    /// 20k-file batch becomes a few dozen statements rather than 20k prepared
    /// INSERTs. SQLite's `RETURNING` order is undefined, so we look up each
    /// generated `id` by `name` (validated unique below) and rebuild the
    /// response in the caller's input order before committing.
    async fn create_files(
        &self,
        body: models::FilesModel,
        context: &C,
    ) -> Result<CreateFilesResponse, ApiError> {
        debug!(
            "create_files({} files) - X-Span-ID: {:?}",
            body.files.len(),
            context.get().0.clone()
        );

        // The transport layer rejects empty batches with 422 so an unauthenticated
        // caller can't reach this method without a workflow_id to authorize against.
        // Mirror that contract here for in-process callers that bypass the transport.
        if body.files.is_empty() {
            return Ok(CreateFilesResponse::UnprocessableContentErrorResponse(
                message_error_response(
                    "Bulk file creation requires a non-empty `files` array".to_string(),
                ),
            ));
        }

        // Oversized batches are a client input error → 422, not 500.
        if body.files.len() > MAX_RECORD_TRANSFER_COUNT as usize {
            return Ok(CreateFilesResponse::UnprocessableContentErrorResponse(
                message_error_response(format!(
                    "Too many files in batch: {}. Maximum is {}",
                    body.files.len(),
                    MAX_RECORD_TRANSFER_COUNT
                )),
            ));
        }

        // Preflight: a `file` table has no UNIQUE (workflow_id, name) constraint, so
        // duplicate names in one request would silently commit two rows and then
        // confuse the post-insert response rebuild. Reject up front with 422.
        {
            let mut seen: std::collections::HashSet<&str> =
                std::collections::HashSet::with_capacity(body.files.len());
            for file in &body.files {
                if !seen.insert(file.name.as_str()) {
                    return Ok(CreateFilesResponse::UnprocessableContentErrorResponse(
                        message_error_response(format!(
                            "Duplicate file name '{}' in bulk create request",
                            file.name
                        )),
                    ));
                }
            }
        }

        // Acquire the SQLite write lock up front to avoid SQLITE_BUSY_SNAPSHOT under
        // BEGIN DEFERRED if another writer commits between our INSERTs.
        let mut transaction = match begin_immediate(&self.context.pool).await {
            Ok(tx) => tx,
            Err(e) => return Err(database_error_with_msg(e, "Failed to begin transaction")),
        };

        // 4 bound parameters per row × 500 rows = 2000 params, well under SQLite's
        // SQLITE_MAX_VARIABLE_NUMBER limit (32766 since 3.32).
        const ROWS_PER_INSERT: usize = 500;
        let mut id_by_name: std::collections::HashMap<String, i64> =
            std::collections::HashMap::with_capacity(body.files.len());

        for chunk in body.files.chunks(ROWS_PER_INSERT) {
            let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
                "INSERT INTO file (workflow_id, name, path, st_mtime) ",
            );
            qb.push_values(chunk, |mut row, file| {
                row.push_bind(file.workflow_id)
                    .push_bind(&file.name)
                    .push_bind(&file.path)
                    .push_bind(file.st_mtime);
            });
            qb.push(" RETURNING id, name");

            let rows = match qb.build().fetch_all(&mut *transaction).await {
                Ok(rows) => rows,
                Err(e) => {
                    let _ = transaction.rollback().await;
                    return Err(database_error_with_msg(
                        e,
                        "Failed to create file records in bulk",
                    ));
                }
            };
            for row in rows {
                let id: i64 = row.get("id");
                let name: String = row.get("name");
                id_by_name.insert(name, id);
            }
        }

        // Rebuild the response in the caller's input order BEFORE committing so any
        // unexpected name mismatch can still roll back the transaction. The preflight
        // uniqueness check above means each name should resolve exactly once; if it
        // doesn't, something is wrong with the RETURNING projection itself.
        let total_input = body.files.len();
        let mut created_files = Vec::with_capacity(total_input);
        for mut file in body.files {
            match id_by_name.remove(&file.name) {
                Some(id) => file.id = Some(id),
                None => {
                    let _ = transaction.rollback().await;
                    return Err(ApiError(format!(
                        "Bulk file insert returned no id for name '{}'",
                        file.name
                    )));
                }
            }
            created_files.push(file);
        }

        if let Err(e) = transaction.commit().await {
            return Err(database_error_with_msg(e, "Failed to commit transaction"));
        }

        // `created_files` is non-empty here — the empty-input branch returns above.
        let workflow_id = created_files[0].workflow_id;
        info!(
            "Files created workflow_id={} count={}",
            workflow_id,
            created_files.len()
        );

        Ok(CreateFilesResponse::SuccessfulResponse(
            models::CreateFilesResponse {
                files: Some(created_files),
            },
        ))
    }

    /// Delete all files for one workflow.
    async fn delete_files(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteFilesResponse, ApiError> {
        debug!(
            "delete_files(\"{}\") - X-Span-ID: {:?}",
            workflow_id,
            context.get().0.clone()
        );

        // Delete all files for the workflow
        let result = match sqlx::query!("DELETE FROM file WHERE workflow_id = $1", workflow_id)
            .execute(self.context.pool.as_ref())
            .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to delete files"));
            }
        };

        let deleted_count = result.rows_affected();
        info!(
            "Deleted {} files for workflow {}",
            deleted_count, workflow_id
        );

        Ok(DeleteFilesResponse::SuccessfulResponse(serde_json::json!({
            "deleted_count": deleted_count
        })))
    }

    /// Retrieve a file by ID.
    async fn get_file(&self, id: i64, context: &C) -> Result<GetFileResponse, ApiError> {
        debug!(
            "get_file({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        let record = match sqlx::query!(
            "SELECT id, workflow_id, name, path, st_mtime FROM file WHERE id = $1",
            id
        )
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(rec)) => rec,
            Ok(None) => {
                return Ok(GetFileResponse::NotFoundErrorResponse(
                    resource_not_found_response("File", id),
                ));
            }
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to fetch file"));
            }
        };

        let file_model = models::FileModel {
            id: Some(record.id),
            workflow_id: record.workflow_id,
            name: record.name,
            path: record.path,
            st_mtime: record.st_mtime,
        };

        debug!(
            "get_file({}) - Found file '{}' - X-Span-ID: {:?}",
            id,
            file_model.name,
            context.get().0.clone()
        );

        Ok(GetFileResponse::SuccessfulResponse(file_model))
    }

    /// Retrieve all files for one workflow.
    async fn list_files(
        &self,
        workflow_id: i64,
        produced_by_job_id: Option<i64>,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        name: Option<String>,
        path: Option<String>,
        is_output: Option<bool>,
        context: &C,
    ) -> Result<ListFilesResponse, ApiError> {
        debug!(
            "list_files({}, {:?}, {}, {}, {:?}, {:?}, {:?}, {:?}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            produced_by_job_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            name,
            path,
            is_output,
            context.get().0.clone()
        );

        // Build base query - include JOIN if we need to filter by produced_by_job_id or is_output=true
        let needs_join = produced_by_job_id.is_some() || is_output == Some(true);
        let base_query = if needs_join {
            "
                SELECT
                    DISTINCT f.id
                    ,f.workflow_id
                    ,f.name
                    ,f.path
                    ,f.st_mtime
                FROM file f
                JOIN job_output_file jof ON f.id = jof.file_id
            "
            .to_string()
        } else {
            "SELECT id, workflow_id, name, path, st_mtime FROM file".to_string()
        };

        // Build WHERE clause conditions
        let mut where_conditions = vec![];

        if needs_join {
            where_conditions.push("f.workflow_id = ?".to_string());
            if produced_by_job_id.is_some() {
                where_conditions.push("jof.job_id = ?".to_string());
            }
        } else {
            where_conditions.push("workflow_id = ?".to_string());
        }

        // Add name filter if provided
        if name.is_some() {
            let name_column = if needs_join { "f.name" } else { "name" };
            where_conditions.push(format!("{} = ?", name_column));
        }

        // Add path filter if provided (substring match so callers can filter
        // by a path fragment).
        if path.is_some() {
            let path_column = if needs_join { "f.path" } else { "path" };
            where_conditions.push(format!("{} LIKE ? ESCAPE '\\'", path_column));
        }

        let where_clause = where_conditions.join(" AND ");
        let sort_by = if let Some(ref col) = sort_by {
            if FILE_COLUMNS.contains(&col.as_str()) {
                // If we have a join (needs_join is true), prefix with "f." if it's a file column
                if needs_join {
                    Some(format!("f.{}", col))
                } else {
                    Some(col.clone())
                }
            } else {
                debug!("Invalid sort column requested: {}", col);
                None // Fall back to default
            }
        } else {
            None
        };

        // Build the complete query with pagination and sorting
        // Use f.id for sorting when we have JOIN, otherwise just id
        let sort_column = if needs_join { "f.id" } else { "id" };
        let query = SqlQueryBuilder::new(base_query)
            .with_where(where_clause.clone())
            .with_pagination_and_sorting(
                offset,
                limit,
                sort_by,
                reverse_sort,
                sort_column,
                FILE_COLUMNS,
            )
            .build();

        debug!("Executing query: {}", query);

        // Execute the query
        let mut sqlx_query = sqlx::query(sqlx::AssertSqlSafe(&*query));
        sqlx_query = sqlx_query.bind(workflow_id);
        if let Some(job_id) = produced_by_job_id {
            sqlx_query = sqlx_query.bind(job_id);
        }
        if let Some(ref name_filter) = name {
            sqlx_query = sqlx_query.bind(name_filter);
        }
        if let Some(ref path_filter) = path {
            sqlx_query = sqlx_query.bind(format!("%{}%", super::escape_like_pattern(path_filter)));
        }

        let records = match sqlx_query.fetch_all(self.context.pool.as_ref()).await {
            Ok(recs) => recs,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to list files"));
            }
        };

        let mut items: Vec<models::FileModel> = Vec::new();
        for record in records {
            items.push(models::FileModel {
                id: Some(record.get("id")),
                workflow_id: record.get("workflow_id"),
                name: record.get("name"),
                path: record.get("path"),
                st_mtime: record.get("st_mtime"),
            });
        }

        // For proper pagination, we should get the total count without LIMIT/OFFSET
        let count_base_query = if needs_join {
            "
                SELECT
                    COUNT(DISTINCT f.id) as total
                FROM file f
                JOIN job_output_file jof ON f.id = jof.file_id
            "
            .to_string()
        } else {
            "SELECT COUNT(*) as total FROM file".to_string()
        };

        let count_query = SqlQueryBuilder::new(count_base_query)
            .with_where(where_clause)
            .build();

        let mut count_sqlx_query = sqlx::query(sqlx::AssertSqlSafe(&*count_query));
        count_sqlx_query = count_sqlx_query.bind(workflow_id);
        if let Some(job_id) = produced_by_job_id {
            count_sqlx_query = count_sqlx_query.bind(job_id);
        }
        if let Some(ref name_filter) = name {
            count_sqlx_query = count_sqlx_query.bind(name_filter);
        }
        if let Some(ref path_filter) = path {
            count_sqlx_query =
                count_sqlx_query.bind(format!("%{}%", super::escape_like_pattern(path_filter)));
        }

        let total_count = match count_sqlx_query.fetch_one(self.context.pool.as_ref()).await {
            Ok(row) => row.get::<i64, _>("total"),
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to list files"));
            }
        };

        let response =
            crate::paginated_list_response!(models::ListFilesResponse, items, offset, total_count);

        debug!(
            "list_files({}, {}/{}) - X-Span-ID: {:?}",
            workflow_id,
            response.count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListFilesResponse::SuccessfulResponse(response))
    }

    /// Return files that are marked as required to exist but don't exist.
    async fn list_required_existing_files(
        &self,
        id: i64,
        context: &C,
    ) -> Result<ListRequiredExistingFilesResponse, ApiError> {
        debug!(
            "list_required_existing_files({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        // Get file IDs needed by jobs but not produced by jobs
        let user_required_files = match self.find_user_required_files(id).await {
            Ok(ids) => ids,
            Err(e) => return Err(e),
        };

        // Get file IDs that should have been produced by completed jobs
        let job_produced_files = match self.find_job_produced_files(id).await {
            Ok(ids) => ids,
            Err(e) => return Err(e),
        };

        // Combine both sets of file IDs
        let mut all_required_files = user_required_files;
        all_required_files.extend(job_produced_files);
        all_required_files.sort();
        all_required_files.dedup();

        let response = models::ListRequiredExistingFilesResponse {
            files: all_required_files,
        };

        Ok(ListRequiredExistingFilesResponse::SuccessfulResponse(
            response,
        ))
    }

    async fn update_file_stats(
        &self,
        workflow_id: i64,
        body: models::FileStatsRequest,
        dry_run: bool,
        context: &C,
    ) -> Result<UpdateFileStatsResponse, ApiError> {
        debug!(
            "update_file_stats(workflow_id={}, count={}, dry_run={}) - X-Span-ID: {:?}",
            workflow_id,
            body.files.len(),
            dry_run,
            context.get().0.clone()
        );
        let unprocessable = |msg: String| {
            Ok(UpdateFileStatsResponse::UnprocessableContentErrorResponse(
                message_error_response(msg),
            ))
        };
        if body.files.len() > MAX_RECORD_TRANSFER_COUNT as usize {
            return unprocessable(format!(
                "Too many files in batch: {}. Maximum is {}",
                body.files.len(),
                MAX_RECORD_TRANSFER_COUNT
            ));
        }
        let mut ids = std::collections::BTreeSet::new();
        for f in &body.files {
            if !ids.insert(f.id) {
                return unprocessable(format!("Duplicate file id {} in request", f.id));
            }
            if f.exists && !f.st_mtime.is_some_and(f64::is_finite) {
                return unprocessable(format!("file_id={} exists but has no valid st_mtime", f.id));
            }
        }
        let mut response = models::FileStatsResponse::default();
        if ids.is_empty() {
            return Ok(UpdateFileStatsResponse::SuccessfulResponse(response));
        }
        // Id sets are bound as one JSON array and expanded with json_each, so a 100k-entry
        // batch needs one query per step instead of chunked IN lists. CROSS JOIN keeps
        // json_each outermost (SQLite never reorders it), so each id is an index lookup rather
        // than a rescan of the batch per row.
        let to_json = |ids: &[i64]| serde_json::to_string(ids).expect("serialize ids");
        let ids: Vec<i64> = ids.into_iter().collect();

        // A dry run only reads; a real run takes the write lock up front so the comparison and
        // the writes see the same state.
        let mut tx = if dry_run {
            self.context.pool.begin().await
        } else {
            begin_immediate(&self.context.pool).await
        }
        .map_err(|e| database_lock_aware_error(e, "Failed to begin transaction"))?;

        let stored: std::collections::HashMap<i64, Option<f64>> =
            sqlx::query_as::<_, (i64, Option<f64>)>(
                "SELECT f.id, f.st_mtime FROM json_each(?1) j \
                 CROSS JOIN file f ON f.id = j.value AND f.workflow_id = ?2",
            )
            .bind(to_json(&ids))
            .bind(workflow_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to read file mtimes"))?
            .into_iter()
            .collect();
        if stored.len() != ids.len() {
            let bad: Vec<String> = ids
                .iter()
                .filter(|id| !stored.contains_key(id))
                .take(20)
                .map(i64::to_string)
                .collect();
            return unprocessable(format!(
                "Files do not exist in workflow_id={}: [{}]",
                workflow_id,
                bad.join(", ")
            ));
        }

        // Same rules and 10 ms tolerance (filesystem timestamp precision) the client used.
        let mut updates: Vec<(i64, Option<f64>)> = Vec::new();
        let mut missing_ids: Vec<i64> = Vec::new();
        for f in &body.files {
            let reason = match (f.exists, stored[&f.id], f.st_mtime) {
                (true, Some(old), Some(new)) if (old - new).abs() > 0.01 => Some("modified"),
                (true, None, _) => Some("appeared"),
                (false, Some(_), _) => Some("disappeared"),
                _ => None,
            };
            if let Some(reason) = reason {
                updates.push((f.id, if f.exists { f.st_mtime } else { None }));
                response.changed_files.push(models::FileStatChange {
                    file_id: f.id,
                    reason: reason.to_string(),
                });
            }
            if !f.exists {
                missing_ids.push(f.id);
            }
        }
        let changed_ids: Vec<i64> = updates.iter().map(|(id, _)| *id).collect();

        // Jobs to reset, in the order of precedence of their reason.
        let mut seeds: std::collections::BTreeMap<i64, (String, i64, &'static str)> =
            std::collections::BTreeMap::new();
        let completed = models::JobStatus::Completed.to_int();
        if !changed_ids.is_empty() {
            let rows = sqlx::query_as::<_, (i64, String, i64)>(
                "SELECT DISTINCT j.id, j.name, j.status FROM json_each(?1) c \
                 CROSS JOIN job_input_file jif ON jif.file_id = c.value \
                 CROSS JOIN job j ON j.id = jif.job_id \
                 WHERE j.workflow_id = ?2 AND j.status IN (?3, ?4, ?5)",
            )
            .bind(to_json(&changed_ids))
            .bind(workflow_id)
            .bind(completed)
            .bind(models::JobStatus::Failed.to_int())
            .bind(models::JobStatus::Canceled.to_int())
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to find jobs with changed inputs"))?;
            for (id, name, status) in rows {
                seeds.insert(id, (name, status, "input_changed"));
            }
        }
        if !missing_ids.is_empty() {
            let missing_json = to_json(&missing_ids);
            let rows = sqlx::query_as::<_, (i64, String, i64)>(
                "SELECT DISTINCT j.id, j.name, j.status FROM json_each(?1) m \
                 CROSS JOIN job_output_file jof ON jof.file_id = m.value \
                 CROSS JOIN job j ON j.id = jof.job_id \
                 WHERE j.workflow_id = ?2 AND j.status = ?3",
            )
            .bind(&missing_json)
            .bind(workflow_id)
            .bind(completed)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to find jobs with missing outputs"))?;
            for (id, name, status) in rows {
                seeds.entry(id).or_insert((name, status, "output_missing"));
            }
            response.missing_required_file_ids = sqlx::query_scalar::<_, i64>(
                "SELECT m.value FROM json_each(?1) m \
                 WHERE EXISTS (SELECT 1 FROM job_input_file jif WHERE jif.file_id = m.value) \
                 AND NOT EXISTS (SELECT 1 FROM job_output_file jof WHERE jof.file_id = m.value) \
                 ORDER BY m.value",
            )
            .bind(&missing_json)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to find missing required files"))?;
        }

        let mut downstream = Vec::new();
        if !seeds.is_empty() {
            // Every job that depends on a reset job, directly or transitively, whatever its
            // status. This is the set the following initialize_jobs(only_uninitialized) would
            // reset (uninitialize_blocked_jobs); resetting it here makes the response complete
            // and leaves the same state if the client stops before initialize_jobs.
            let seed_ids: Vec<i64> = seeds.keys().copied().collect();
            downstream = sqlx::query_as::<_, (i64, String, i64)>(
                "WITH RECURSIVE downstream(job_id) AS ( \
                     SELECT value FROM json_each(?1) \
                     UNION \
                     SELECT d.job_id FROM job_depends_on d \
                     JOIN downstream ds ON d.depends_on_job_id = ds.job_id \
                     WHERE d.workflow_id = ?2 \
                 ) \
                 SELECT j.id, j.name, j.status FROM job j \
                 JOIN downstream ds ON ds.job_id = j.id \
                 WHERE j.workflow_id = ?2 AND j.status != ?3 ORDER BY j.id",
            )
            .bind(to_json(&seed_ids))
            .bind(workflow_id)
            .bind(models::JobStatus::Uninitialized.to_int())
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to find downstream jobs"))?;
            downstream.retain(|(id, _, _)| !seeds.contains_key(id));
        }
        let reset = seeds
            .into_iter()
            .map(|(id, (name, status, reason))| (id, name, status, reason))
            .chain(
                downstream
                    .into_iter()
                    .map(|(id, name, status)| (id, name, status, "downstream")),
            );
        for (job_id, name, status, reason) in reset {
            response.reset_jobs.push(models::FileStatsResetJob {
                job_id,
                name,
                status: super::parse_job_status(status as i32, job_id)?,
                reason: reason.to_string(),
            });
        }

        if dry_run {
            return Ok(UpdateFileStatsResponse::SuccessfulResponse(response));
        }
        for (id, st_mtime) in &updates {
            sqlx::query("UPDATE file SET st_mtime = ? WHERE id = ?")
                .bind(st_mtime)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to update file mtime"))?;
        }
        for job in &response.reset_jobs {
            sqlx::query(
                "UPDATE job SET status = ?, start_time = NULL, compute_node_id = NULL \
                 WHERE id = ?",
            )
            .bind(models::JobStatus::Uninitialized.to_int())
            .bind(job.job_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to reset job status"))?;
        }
        tx.commit()
            .await
            .map_err(|e| database_lock_aware_error(e, "Failed to commit transaction"))?;
        if !updates.is_empty() || !response.reset_jobs.is_empty() {
            info!(
                "File stats applied workflow_id={} changed_files={} reset_jobs={}",
                workflow_id,
                updates.len(),
                response.reset_jobs.len()
            );
        }
        Ok(UpdateFileStatsResponse::SuccessfulResponse(response))
    }

    /// Update a file.
    async fn update_file(
        &self,
        id: i64,
        body: models::FileModel,
        context: &C,
    ) -> Result<UpdateFileResponse, ApiError> {
        debug!(
            "update_file({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        // First check if the file exists
        match self.get_file(id, context).await? {
            GetFileResponse::SuccessfulResponse(_) => {}
            GetFileResponse::ForbiddenErrorResponse(err) => {
                return Ok(UpdateFileResponse::ForbiddenErrorResponse(err));
            }
            GetFileResponse::NotFoundErrorResponse(err) => {
                return Ok(UpdateFileResponse::NotFoundErrorResponse(err));
            }
            GetFileResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get file".to_string()));
            }
        };

        // Update the file record using COALESCE to only update non-null fields
        // Exception: st_mtime should always be updated, even if null
        let result = match sqlx::query!(
            r#"
            UPDATE file
            SET
                name = COALESCE($1, name),
                path = COALESCE($2, path),
                st_mtime = $3
            WHERE id = $4 AND workflow_id = $5
            "#,
            body.name,
            body.path,
            body.st_mtime,
            id,
            body.workflow_id,
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to update file"));
            }
        };

        if result.rows_affected() == 0 {
            let pool = self.context.pool.as_ref();
            return Ok(if super::workflow_id_mismatch(pool, "file", id).await? {
                UpdateFileResponse::UnprocessableContentErrorResponse(
                    super::workflow_id_immutable_response(),
                )
            } else {
                UpdateFileResponse::NotFoundErrorResponse(resource_not_found_response("File", id))
            });
        }

        // Return the updated file by fetching it again
        let updated_file = match self.get_file(id, context).await? {
            GetFileResponse::SuccessfulResponse(file) => file,
            _ => return Err(ApiError("Failed to get updated file".to_string())),
        };

        debug!("Updated file with id: {}", id);
        Ok(UpdateFileResponse::SuccessfulResponse(updated_file))
    }

    /// Delete a file.
    async fn delete_file(&self, id: i64, context: &C) -> Result<DeleteFileResponse, ApiError> {
        debug!(
            "delete_file({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        // First get the file to ensure it exists and extract the FileModel
        let file = match self.get_file(id, context).await? {
            GetFileResponse::SuccessfulResponse(file) => file,
            GetFileResponse::ForbiddenErrorResponse(err) => {
                return Ok(DeleteFileResponse::ForbiddenErrorResponse(err));
            }
            GetFileResponse::NotFoundErrorResponse(err) => {
                return Ok(DeleteFileResponse::NotFoundErrorResponse(err));
            }
            GetFileResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get file".to_string()));
            }
        };

        match sqlx::query!(r#"DELETE FROM file WHERE id = $1"#, id)
            .execute(self.context.pool.as_ref())
            .await
        {
            Ok(res) => {
                if res.rows_affected() > 1 {
                    return Err(database_error_with_msg(
                        "Unexpected number of rows affected",
                        "Failed to delete file",
                    ));
                } else if res.rows_affected() == 0 {
                    return Err(database_error_with_msg(
                        "No rows affected",
                        "Failed to delete file",
                    ));
                } else {
                    info!(
                        "Deleted file {} (path: {:?}) from workflow {}",
                        id, file.path, file.workflow_id
                    );
                    Ok(DeleteFileResponse::SuccessfulResponse(file))
                }
            }
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to delete file"));
            }
        }
    }
}

impl FilesApiImpl {
    /// Find all file IDs needed by a job, as shown in the table job_input_file,
    /// that are not produced by a job, as shown in the table job_output_file.
    async fn find_user_required_files(&self, workflow_id: i64) -> Result<Vec<i64>, ApiError> {
        let rows = match sqlx::query!(
            r#"
            SELECT DISTINCT jif.file_id
            FROM job_input_file jif
            INNER JOIN job j ON jif.job_id = j.id
            WHERE j.workflow_id = $1
            AND jif.file_id NOT IN (
                SELECT jof.file_id
                FROM job_output_file jof
            )
            "#,
            workflow_id
        )
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(rows) => rows,
            Err(e) => return Err(database_error_with_msg(e, "Failed to list required files")),
        };

        Ok(rows.into_iter().map(|row| row.file_id).collect())
    }

    /// Find all file IDs produced by a job, as shown in the table job_output_file,
    /// where the job status is JobStatus::Completed.
    async fn find_job_produced_files(&self, workflow_id: i64) -> Result<Vec<i64>, ApiError> {
        let completed_status = models::JobStatus::Completed.to_int();

        let rows = match sqlx::query!(
            r#"
            SELECT DISTINCT jof.file_id
            FROM job_output_file jof
            INNER JOIN job j ON jof.job_id = j.id
            WHERE j.workflow_id = $1
            AND j.status = $2
            "#,
            workflow_id,
            completed_status
        )
        .fetch_all(self.context.pool.as_ref())
        .await
        {
            Ok(rows) => rows,
            Err(e) => return Err(database_error_with_msg(e, "Failed to list required files")),
        };

        Ok(rows.into_iter().map(|row| row.file_id).collect())
    }
}
