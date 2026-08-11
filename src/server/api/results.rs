//! Result-related API endpoints

#![allow(clippy::too_many_arguments)]

use crate::server::transport_types::context_types::{ApiError, Has, XSpanIdString};
use async_trait::async_trait;
use log::{debug, error, info};
use sqlx::Row;

use crate::server::api_responses::{
    CreateResultResponse, DeleteResultResponse, DeleteResultsResponse, GetResultResponse,
    ListResultsResponse, UpdateResultResponse,
};

use crate::models;

use super::{
    ApiContext, SqlQueryBuilder, database_error_with_msg, parse_job_status,
    resource_not_found_response,
};

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
    async fn delete_result(&self, id: i64, context: &C) -> Result<DeleteResultResponse, ApiError>;
}

/// Implementation of results API for the server
#[derive(Clone)]
pub struct ResultsApiImpl {
    context: ApiContext,
}

const RESULT_COLUMNS: &[&str] = &[
    "id",
    "job_id",
    "workflow_id",
    "run_id",
    "attempt_id",
    "compute_node_id",
    "return_code",
    "exec_time_minutes",
    "completion_time",
    "status",
    "peak_memory_bytes",
    "avg_memory_bytes",
    "peak_cpu_percent",
    "avg_cpu_percent",
];

impl ResultsApiImpl {
    pub(crate) fn new(context: ApiContext) -> Self {
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
        debug!("create_result - X-Span-ID: {:?}", context.get().0.clone());
        let status = body.status.to_int();
        let attempt_id = body.attempt_id.unwrap_or(1);

        // Use a transaction to atomically insert into both result and workflow_result.
        let mut tx = self
            .context
            .pool
            .begin()
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to begin transaction"))?;

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
        .fetch_one(&mut *tx)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to create result record"));
            }
        };
        body.id = Some(result.id);

        // Also populate workflow_result so this result is visible via default list queries.
        // workflow_result tracks the latest result per (workflow_id, job_id).
        let result_id = result.id;
        let workflow_id = body.workflow_id;
        let job_id = body.job_id;
        if let Err(e) = sqlx::query!(
            r#"
            INSERT OR REPLACE INTO workflow_result (workflow_id, job_id, result_id)
            VALUES (?, ?, ?)
            "#,
            workflow_id,
            job_id,
            result_id,
        )
        .execute(&mut *tx)
        .await
        {
            error!(
                "Failed to insert workflow_result for workflow_id={}, job_id={}: {}",
                workflow_id, job_id, e
            );
            return Err(database_error_with_msg(
                e,
                "Failed to create workflow_result record",
            ));
        }

        tx.commit()
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to commit transaction"))?;

        Ok(CreateResultResponse::SuccessfulResponse(body))
    }

    /// Delete all results for one workflow.
    async fn delete_results(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteResultsResponse, ApiError> {
        debug!(
            "delete_results({}) - X-Span-ID: {:?}",
            workflow_id,
            context.get().0.clone()
        );

        let result = match sqlx::query!("DELETE FROM result WHERE workflow_id = $1", workflow_id)
            .execute(self.context.pool.as_ref())
            .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to delete results"));
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

        // Runtime query (not the query! macro) so the LEFT JOIN to `job` for the
        // denormalized job_name doesn't trip compile-time nullability inference.
        let record = match sqlx::query(
            r#"
            SELECT r.id, r.job_id, r.workflow_id, r.run_id, r.attempt_id, r.compute_node_id,
                   r.return_code, r.exec_time_minutes, r.completion_time, r.status,
                   r.peak_memory_bytes, r.avg_memory_bytes, r.peak_cpu_percent, r.avg_cpu_percent,
                   j.name AS job_name
            FROM result r
            LEFT JOIN job j ON j.id = r.job_id
            WHERE r.id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(record)) => record,
            Ok(None) => {
                return Ok(GetResultResponse::NotFoundErrorResponse(
                    resource_not_found_response("Result", id),
                ));
            }
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to fetch result"));
            }
        };

        let status_int: i64 = record.get("status");
        let job_id: i64 = record.get("job_id");
        let status = parse_job_status(status_int as i32, job_id)?;

        let result_model = models::ResultModel {
            id: Some(record.get("id")),
            workflow_id: record.get("workflow_id"),
            job_id,
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
            job_name: record.get("job_name"),
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

        // Build base query. `result` is always aliased `r` so the LEFT JOIN to
        // `job` (for the denormalized job_name) and the column references stay
        // unambiguous in both branches. When all_runs is false, restrict to the
        // current results via workflow_result.
        let result_columns = "r.id, r.job_id, r.workflow_id, r.run_id, r.attempt_id, r.compute_node_id, r.return_code, r.exec_time_minutes, r.completion_time, r.status, r.peak_memory_bytes, r.avg_memory_bytes, r.peak_cpu_percent, r.avg_cpu_percent, j.name AS job_name";
        let base_query = if show_all_results {
            format!("SELECT {result_columns} FROM result r LEFT JOIN job j ON j.id = r.job_id")
        } else {
            format!(
                "SELECT {result_columns} FROM result r \
                 INNER JOIN workflow_result wr ON r.id = wr.result_id \
                 LEFT JOIN job j ON j.id = r.job_id"
            )
        };

        // All columns are referenced through the `r` alias now.
        let col_prefix = "r.";

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
        let sort_by = if let Some(ref col) = sort_by {
            if RESULT_COLUMNS.contains(&col.as_str()) {
                // `result` is always aliased `r`, so qualify the sort column to
                // avoid ambiguity with the joined `job` table.
                Some(format!("r.{}", col))
            } else {
                debug!("Invalid sort column requested: {}", col);
                None // Fall back to default
            }
        } else {
            None
        };

        // Build the complete query with pagination and sorting. The default sort
        // column is qualified (`r.id`) for the same reason.
        let query = SqlQueryBuilder::new(base_query)
            .with_where(where_clause.clone())
            .with_pagination_and_sorting(
                offset,
                limit,
                sort_by,
                reverse_sort,
                "r.id",
                RESULT_COLUMNS,
            )
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
                return Err(database_error_with_msg(e, "Failed to list results"));
            }
        };

        let mut items: Vec<models::ResultModel> = Vec::new();
        for record in records {
            let status_int: i64 = record.get("status");
            let job_id: i64 = record.get("job_id");
            let status = parse_job_status(status_int as i32, job_id)?;
            items.push(models::ResultModel {
                id: Some(record.get("id")),
                workflow_id: record.get("workflow_id"),
                job_id,
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
                job_name: record.get("job_name"),
            });
        }

        // For proper pagination, get the total count without LIMIT/OFFSET. The
        // WHERE clause uses the `r.` prefix, so `result` is aliased `r` here too
        // (the job LEFT JOIN is unnecessary for a count).
        let count_base = if show_all_results {
            "SELECT COUNT(*) as total FROM result r".to_string()
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
                return Err(database_error_with_msg(e, "Failed to list results"));
            }
        };

        let response = crate::paginated_list_response!(
            models::ListResultsResponse,
            items,
            offset,
            total_count
        );

        debug!(
            "list_results({}, {}/{}) - X-Span-ID: {:?}",
            workflow_id,
            response.count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListResultsResponse::SuccessfulResponse(response))
    }

    /// Update a result.
    async fn update_result(
        &self,
        id: i64,
        body: models::ResultModel,
        context: &C,
    ) -> Result<UpdateResultResponse, ApiError> {
        debug!(
            "update_result({}) - X-Span-ID: {:?}",
            id,
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
                return Err(database_error_with_msg(e, "Failed to update result"));
            }
        };

        if result.rows_affected() == 0 {
            return Ok(UpdateResultResponse::NotFoundErrorResponse(
                resource_not_found_response("Result", id),
            ));
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
    async fn delete_result(&self, id: i64, context: &C) -> Result<DeleteResultResponse, ApiError> {
        debug!(
            "delete_result({}) - X-Span-ID: {:?}",
            id,
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
            Err(e) => Err(database_error_with_msg(e, "Failed to delete result")),
        }
    }
}
