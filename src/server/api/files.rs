//! File-related API endpoints

#![allow(clippy::too_many_arguments)]

use async_trait::async_trait;
use log::{debug, info};
use sqlx::Row;
use swagger::{ApiError, Has, XSpanIdString};

use crate::server::api_types::{
    CreateFileResponse, DeleteFileResponse, DeleteFilesResponse, GetFileResponse,
    ListFilesResponse, ListRequiredExistingFilesResponse, UpdateFileResponse,
};

use crate::models;

use super::{ApiContext, MAX_RECORD_TRANSFER_COUNT, SqlQueryBuilder, database_error_with_msg};

/// Trait defining file-related API operations
#[async_trait]
pub trait FilesApi<C> {
    /// Store a file.
    async fn create_file(
        &self,
        mut file: models::FileModel,
        context: &C,
    ) -> Result<CreateFileResponse, ApiError>;

    /// Delete all files for one workflow.
    async fn delete_files(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
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
    async fn delete_file(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteFileResponse, ApiError>;
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
            RETURNING rowid
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

    /// Delete all files for one workflow.
    async fn delete_files(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteFilesResponse, ApiError> {
        debug!(
            "delete_files(\"{}\", {:?}) - X-Span-ID: {:?}",
            workflow_id,
            body,
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
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("File not found with ID: {}", id)
                }));
                return Ok(GetFileResponse::NotFoundErrorResponse(error_response));
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

        // Add path filter if provided
        if path.is_some() {
            let path_column = if needs_join { "f.path" } else { "path" };
            where_conditions.push(format!("{} = ?", path_column));
        }

        let where_clause = where_conditions.join(" AND ");

        // Validate sort_by against whitelist
        let validated_sort_by = if let Some(ref col) = sort_by {
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
                validated_sort_by,
                reverse_sort,
                sort_column,
            )
            .build();

        debug!("Executing query: {}", query);

        // Execute the query
        let mut sqlx_query = sqlx::query(&query);
        sqlx_query = sqlx_query.bind(workflow_id);
        if let Some(job_id) = produced_by_job_id {
            sqlx_query = sqlx_query.bind(job_id);
        }
        if let Some(ref name_filter) = name {
            sqlx_query = sqlx_query.bind(name_filter);
        }
        if let Some(ref path_filter) = path {
            sqlx_query = sqlx_query.bind(path_filter);
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

        let mut count_sqlx_query = sqlx::query(&count_query);
        count_sqlx_query = count_sqlx_query.bind(workflow_id);
        if let Some(job_id) = produced_by_job_id {
            count_sqlx_query = count_sqlx_query.bind(job_id);
        }
        if let Some(ref name_filter) = name {
            count_sqlx_query = count_sqlx_query.bind(name_filter);
        }
        if let Some(ref path_filter) = path {
            count_sqlx_query = count_sqlx_query.bind(path_filter);
        }

        let total_count = match count_sqlx_query.fetch_one(self.context.pool.as_ref()).await {
            Ok(row) => row.get::<i64, _>("total"),
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to list files"));
            }
        };

        let current_count = items.len() as i64;
        let offset_val = offset;
        let has_more = offset_val + current_count < total_count;

        debug!(
            "list_files({}, {}/{}) - X-Span-ID: {:?}",
            workflow_id,
            current_count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListFilesResponse::SuccessfulResponse(
            models::ListFilesResponse {
                items: Some(items),
                offset: offset_val,
                max_limit: MAX_RECORD_TRANSFER_COUNT,
                count: current_count,
                total_count,
                has_more,
            },
        ))
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

    /// Update a file.
    async fn update_file(
        &self,
        id: i64,
        body: models::FileModel,
        context: &C,
    ) -> Result<UpdateFileResponse, ApiError> {
        debug!(
            "update_file({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
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
                workflow_id = COALESCE($1, workflow_id),
                name = COALESCE($2, name),
                path = COALESCE($3, path),
                st_mtime = $4
            WHERE id = $5
            "#,
            body.workflow_id,
            body.name,
            body.path,
            body.st_mtime,
            id,
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
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("File not found with ID: {}", id)
            }));
            return Ok(UpdateFileResponse::NotFoundErrorResponse(error_response));
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
    async fn delete_file(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteFileResponse, ApiError> {
        debug!(
            "delete_file({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
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
