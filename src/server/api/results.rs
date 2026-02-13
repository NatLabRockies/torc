//! Result-related API endpoints

#![allow(clippy::too_many_arguments)]

use async_trait::async_trait;
use log::{debug, error, info};
use sqlx::Row;
use swagger::{ApiError, Has, XSpanIdString};

use crate::server::api_types::{
    CreateResultResponse, DeleteResultResponse, DeleteResultsResponse, GetResultResponse,
    ListResultsResponse, UpdateResultResponse,
};

use crate::models;

use super::{ApiContext, MAX_RECORD_TRANSFER_COUNT, SqlQueryBuilder, database_error};

/// Trait defining result-related API operations
#[async_trait]
pub trait ResultsApi<C> {
    /// Store a job result.
    async fn create_result(
        &self,
        mut body: models::ResultModel,
        context: &C,
    ) -> Result<CreateResultResponse, ApiError>;

    /// Delete all results for one workflow.
    async fn delete_results(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteResultsResponse, ApiError>;

    /// Retrieve a result by ID.
    async fn get_result(&self, id: i64, context: &C) -> Result<GetResultResponse, ApiError>;

    /// Retrieve all job results for one workflow.
    async fn list_results(
        &self,
        workflow_id: i64,
        job_id: Option<i64>,
        run_id: Option<i64>,
        return_code: Option<i64>,
        status: Option<models::JobStatus>,
        compute_node_id: Option<i64>,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        all_runs: Option<bool>,
        context: &C,
    ) -> Result<ListResultsResponse, ApiError>;

    /// Update a result.
    async fn update_result(
        &self,
        id: i64,
        body: models::ResultModel,
        context: &C,
    ) -> Result<UpdateResultResponse, ApiError>;

    /// Delete a result.
    async fn delete_result(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteResultResponse, ApiError>;
}

/// Implementation of results API for the server
#[derive(Clone)]
pub struct ResultsApiImpl {
    pub context: ApiContext,
}

impl ResultsApiImpl {
    pub fn new(context: ApiContext) -> Self {
        Self { context }
    }
}

#[async_trait]
impl<C> ResultsApi<C> for ResultsApiImpl
where
    C: Has<XSpanIdString> + Send + Sync,
{
    /// Store a job result.
    async fn create_result(
        &self,
        mut body: models::ResultModel,
        context: &C,
    ) -> Result<CreateResultResponse, ApiError> {
        debug!(
            "create_result({:?}) - X-Span-ID: {:?}",
            body,
            context.get().0.clone()
        );
        let status = body.status.to_int();

        let attempt_id = body.attempt_id.unwrap_or(1);
        let result = match sqlx::query!(
            r#"
            INSERT INTO result
            (
                job_id
                ,workflow_id
                ,run_id
                ,attempt_id
                ,compute_node_id
                ,return_code
                ,exec_time_minutes
                ,completion_time
                ,status
                ,peak_memory_bytes
                ,avg_memory_bytes
                ,peak_cpu_percent
                ,avg_cpu_percent
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING rowid
        "#,
            body.job_id,
            body.workflow_id,
            body.run_id,
            attempt_id,
            body.compute_node_id,
            body.return_code,
            body.exec_time_minutes,
            body.completion_time,
            status,
            body.peak_memory_bytes,
            body.avg_memory_bytes,
            body.peak_cpu_percent,
            body.avg_cpu_percent,
        )
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("Database error: {}", e);
                return Err(ApiError("Database error".to_string()));
            }
        };
        body.id = Some(result.id);
        Ok(CreateResultResponse::SuccessfulResponse(body))
    }

    /// Delete all results for one workflow.
    async fn delete_results(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteResultsResponse, ApiError> {
        debug!(
            "delete_results({}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            body,
            context.get().0.clone()
        );

        let result = match sqlx::query!("DELETE FROM result WHERE workflow_id = $1", workflow_id)
            .execute(self.context.pool.as_ref())
            .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("Database error: {}", e);
                return Err(database_error(e));
            }
        };

        let deleted_count = result.rows_affected() as i64;

        info!(
            "Deleted {} results for workflow {}",
            deleted_count, workflow_id
        );

        Ok(DeleteResultsResponse::SuccessfulResponse(
            serde_json::json!({
                "count": deleted_count
            }),
        ))
    }

    /// Retrieve a result by ID.
    async fn get_result(&self, id: i64, context: &C) -> Result<GetResultResponse, ApiError> {
        debug!(
            "get_result({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        let record = match sqlx::query!(
            r#"
            SELECT id, job_id, workflow_id, run_id, attempt_id, compute_node_id, return_code, exec_time_minutes, completion_time, status,
                   peak_memory_bytes, avg_memory_bytes, peak_cpu_percent, avg_cpu_percent
            FROM result
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(record)) => record,
            Ok(None) => {
                let error_response = models::ErrorResponse::new(
                    serde_json::json!({
                        "message": format!("Result not found with ID: {}", id)
                    })
                );
                return Ok(GetResultResponse::NotFoundErrorResponse(error_response));
            }
            Err(e) => {
                error!("Database error: {}", e);
                return Err(database_error(e));
            }
        };

        let status_int = record.status;
        let status = match models::JobStatus::from_int(status_int as i32) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to parse job status '{}': {}", status_int, e);
                return Err(ApiError(format!("Failed to parse job status: {}", e)));
            }
        };

        let result_model = models::ResultModel {
            id: Some(record.id),
            workflow_id: record.workflow_id,
            job_id: record.job_id,
            run_id: record.run_id,
            attempt_id: Some(record.attempt_id),
            compute_node_id: record.compute_node_id,
            return_code: record.return_code,
            exec_time_minutes: record.exec_time_minutes,
            completion_time: record.completion_time,
            peak_memory_bytes: record.peak_memory_bytes,
            avg_memory_bytes: record.avg_memory_bytes,
            peak_cpu_percent: record.peak_cpu_percent,
            avg_cpu_percent: record.avg_cpu_percent,
            status,
        };

        Ok(GetResultResponse::SuccessfulResponse(result_model))
    }

    /// Retrieve all job results for one workflow.
    async fn list_results(
        &self,
        workflow_id: i64,
        job_id: Option<i64>,
        run_id: Option<i64>,
        return_code: Option<i64>,
        status: Option<models::JobStatus>,
        compute_node_id: Option<i64>,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        all_runs: Option<bool>,
        context: &C,
    ) -> Result<ListResultsResponse, ApiError> {
        // all_runs defaults to false - only show current results in workflow_result table
        let show_all_results = all_runs.unwrap_or(false);

        debug!(
            "list_results({}, {:?}, {:?}, {:?}, {:?}, {:?}, {}, {}, {:?}, {:?}, all_runs={}) - X-Span-ID: {:?}",
            workflow_id,
            job_id,
            run_id,
            return_code,
            status,
            compute_node_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            show_all_results,
            context.get().0.clone()
        );

        // Build base query
        // If all_runs is false, only return results that are in workflow_result table (current results)
        let base_query = if show_all_results {
            "SELECT id, job_id, workflow_id, run_id, attempt_id, compute_node_id, return_code, exec_time_minutes, completion_time, status, peak_memory_bytes, avg_memory_bytes, peak_cpu_percent, avg_cpu_percent FROM result".to_string()
        } else {
            "SELECT r.id, r.job_id, r.workflow_id, r.run_id, r.attempt_id, r.compute_node_id, r.return_code, r.exec_time_minutes, r.completion_time, r.status, r.peak_memory_bytes, r.avg_memory_bytes, r.peak_cpu_percent, r.avg_cpu_percent FROM result r INNER JOIN workflow_result wr ON r.id = wr.result_id".to_string()
        };

        // Build WHERE clause conditions
        // Use table alias prefix when joining with workflow_result
        let col_prefix = if show_all_results { "" } else { "r." };

        let mut where_conditions = vec![format!("{}workflow_id = ?", col_prefix)];
        let mut bind_values: Vec<Box<dyn sqlx::Encode<'_, sqlx::Sqlite> + Send>> =
            vec![Box::new(workflow_id)];

        if let Some(j_id) = job_id {
            where_conditions.push(format!("{}job_id = ?", col_prefix));
            bind_values.push(Box::new(j_id));
        }

        if let Some(r_id) = run_id {
            where_conditions.push(format!("{}run_id = ?", col_prefix));
            bind_values.push(Box::new(r_id));
        }

        if let Some(ret_code) = return_code {
            where_conditions.push(format!("{}return_code = ?", col_prefix));
            bind_values.push(Box::new(ret_code));
        }

        if let Some(result_status) = &status {
            where_conditions.push(format!("{}status = ?", col_prefix));
            bind_values.push(Box::new(result_status.to_int()));
        }

        if let Some(cn_id) = compute_node_id {
            where_conditions.push(format!("{}compute_node_id = ?", col_prefix));
            bind_values.push(Box::new(cn_id));
        }

        let where_clause = where_conditions.join(" AND ");

        // Build the complete query with pagination and sorting
        let query = SqlQueryBuilder::new(base_query)
            .with_where(where_clause.clone())
            .with_pagination_and_sorting(offset, limit, sort_by, reverse_sort, "id")
            .build();

        debug!("Executing query: {}", query);

        // Execute the query
        let mut sqlx_query = sqlx::query(&query);

        // Bind workflow_id
        sqlx_query = sqlx_query.bind(workflow_id);

        // Bind optional parameters in order
        if let Some(j_id) = job_id {
            sqlx_query = sqlx_query.bind(j_id);
        }
        if let Some(r_id) = run_id {
            sqlx_query = sqlx_query.bind(r_id);
        }
        if let Some(ret_code) = return_code {
            sqlx_query = sqlx_query.bind(ret_code);
        }
        if let Some(ref s) = status {
            sqlx_query = sqlx_query.bind(s.to_int());
        }
        if let Some(cn_id) = compute_node_id {
            sqlx_query = sqlx_query.bind(cn_id);
        }

        let records = match sqlx_query.fetch_all(self.context.pool.as_ref()).await {
            Ok(recs) => recs,
            Err(e) => {
                error!("Database error: {}", e);
                return Err(database_error(e));
            }
        };

        let mut items: Vec<models::ResultModel> = Vec::new();
        for record in records {
            let status_int: i64 = record.get("status");
            let status = match models::JobStatus::from_int(status_int as i32) {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to parse job status '{}': {}", status_int, e);
                    return Err(ApiError(format!("Failed to parse job status: {}", e)));
                }
            };
            items.push(models::ResultModel {
                id: Some(record.get("id")),
                workflow_id: record.get("workflow_id"),
                job_id: record.get("job_id"),
                run_id: record.get("run_id"),
                attempt_id: Some(record.get("attempt_id")),
                compute_node_id: record.get("compute_node_id"),
                return_code: record.get("return_code"),
                exec_time_minutes: record.get("exec_time_minutes"),
                completion_time: record.get("completion_time"),
                peak_memory_bytes: record.get("peak_memory_bytes"),
                avg_memory_bytes: record.get("avg_memory_bytes"),
                peak_cpu_percent: record.get("peak_cpu_percent"),
                avg_cpu_percent: record.get("avg_cpu_percent"),
                status,
            });
        }

        // For proper pagination, we should get the total count without LIMIT/OFFSET
        let count_base = if show_all_results {
            "SELECT COUNT(*) as total FROM result".to_string()
        } else {
            "SELECT COUNT(*) as total FROM result r INNER JOIN workflow_result wr ON r.id = wr.result_id".to_string()
        };
        let count_query = SqlQueryBuilder::new(count_base)
            .with_where(where_clause)
            .build();

        let mut count_sqlx_query = sqlx::query(&count_query);
        count_sqlx_query = count_sqlx_query.bind(workflow_id);
        if let Some(j_id) = job_id {
            count_sqlx_query = count_sqlx_query.bind(j_id);
        }
        if let Some(r_id) = run_id {
            count_sqlx_query = count_sqlx_query.bind(r_id);
        }
        if let Some(ret_code) = return_code {
            count_sqlx_query = count_sqlx_query.bind(ret_code);
        }
        if let Some(ref s) = status {
            count_sqlx_query = count_sqlx_query.bind(s.to_int());
        }
        if let Some(cn_id) = compute_node_id {
            count_sqlx_query = count_sqlx_query.bind(cn_id);
        }

        let total_count = match count_sqlx_query.fetch_one(self.context.pool.as_ref()).await {
            Ok(row) => row.get::<i64, _>("total"),
            Err(e) => {
                error!("Database error getting count: {}", e);
                return Err(database_error(e));
            }
        };

        let current_count = items.len() as i64;
        let offset_val = offset;
        let has_more = offset_val + current_count < total_count;

        debug!(
            "list_results({}, {}/{}) - X-Span-ID: {:?}",
            workflow_id,
            current_count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListResultsResponse::SuccessfulResponse(
            models::ListResultsResponse {
                items: Some(items),
                offset: offset_val,
                max_limit: MAX_RECORD_TRANSFER_COUNT,
                count: current_count,
                total_count,
                has_more,
            },
        ))
    }

    /// Update a result.
    async fn update_result(
        &self,
        id: i64,
        body: models::ResultModel,
        context: &C,
    ) -> Result<UpdateResultResponse, ApiError> {
        debug!(
            "update_result({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First get the existing result to ensure it exists
        match self.get_result(id, context).await? {
            GetResultResponse::SuccessfulResponse(result) => result,
            GetResultResponse::ForbiddenErrorResponse(err) => {
                return Ok(UpdateResultResponse::ForbiddenErrorResponse(err));
            }
            GetResultResponse::NotFoundErrorResponse(err) => {
                return Ok(UpdateResultResponse::NotFoundErrorResponse(err));
            }
            GetResultResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get result".to_string()));
            }
        };

        let status_int = body.status.to_int();

        let result = match sqlx::query!(
            r#"
            UPDATE result
            SET
                job_id = COALESCE($1, job_id)
                ,workflow_id = COALESCE($2, workflow_id)
                ,run_id = COALESCE($3, run_id)
                ,return_code = COALESCE($4, return_code)
                ,exec_time_minutes = COALESCE($5, exec_time_minutes)
                ,completion_time = COALESCE($6, completion_time)
                ,status = COALESCE($7, status)
            WHERE id = $8
            "#,
            body.job_id,
            body.workflow_id,
            body.run_id,
            body.return_code,
            body.exec_time_minutes,
            body.completion_time,
            status_int,
            id,
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("Database error: {}", e);
                return Err(database_error(e));
            }
        };

        if result.rows_affected() == 0 {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Result not found with ID: {}", id)
            }));
            return Ok(UpdateResultResponse::NotFoundErrorResponse(error_response));
        }

        // Return the updated result by fetching it again
        let updated_result = match self.get_result(id, context).await? {
            GetResultResponse::SuccessfulResponse(result) => result,
            _ => return Err(ApiError("Failed to get updated result".to_string())),
        };

        debug!("Modified result with id: {}", id);
        Ok(UpdateResultResponse::SuccessfulResponse(updated_result))
    }

    /// Delete a result.
    async fn delete_result(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteResultResponse, ApiError> {
        debug!(
            "delete_result({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First get the result to ensure it exists and extract the ResultModel
        let result = match self.get_result(id, context).await? {
            GetResultResponse::SuccessfulResponse(result) => result,
            GetResultResponse::ForbiddenErrorResponse(err) => {
                return Ok(DeleteResultResponse::ForbiddenErrorResponse(err));
            }
            GetResultResponse::NotFoundErrorResponse(err) => {
                return Ok(DeleteResultResponse::NotFoundErrorResponse(err));
            }
            GetResultResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get result".to_string()));
            }
        };

        match sqlx::query!(r#"DELETE FROM result WHERE id = $1"#, id)
            .execute(self.context.pool.as_ref())
            .await
        {
            Ok(res) => {
                if res.rows_affected() > 1 {
                    Err(ApiError(format!(
                        "Database error: Unexpected number of rows affected: {}",
                        res.rows_affected()
                    )))
                } else if res.rows_affected() == 0 {
                    Err(ApiError("Database error: No rows affected".to_string()))
                } else {
                    info!("Deleted result with id: {}", id);
                    Ok(DeleteResultResponse::SuccessfulResponse(result))
                }
            }
            Err(e) => {
                error!("Database error: {}", e);
                Err(database_error(e))
            }
        }
    }
}
