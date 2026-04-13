//! Job-related API endpoints

#![allow(clippy::too_many_arguments)]

use std::collections::HashMap;

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

use crate::models::{self as models, JobStatus};

use super::{ApiContext, MAX_RECORD_TRANSFER_COUNT, SqlQueryBuilder, database_error_with_msg};

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
    async fn list_jobs(
        &self,
        workflow_id: i64,
        status: Option<JobStatus>,
        needs_file_id: Option<i64>,
        upstream_job_id: Option<i64>,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        include_relationships: Option<bool>,
        active_compute_node_id: Option<i64>,
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

    /// Return user-requested number of jobs that are ready for submission. Sets status to pending.
    async fn claim_next_jobs(
        &self,
        id: i64,
        requested_job_count: i64,
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
    /// - attempt_id must be less than max_retries
    async fn retry_job(
        &self,
        id: i64,
        run_id: i64,
        max_retries: i32,
        context: &C,
    ) -> Result<RetryJobResponse, ApiError>;
}

/// Implementation of jobs API for the server
#[derive(Clone)]
pub struct JobsApiImpl {
    pub context: ApiContext,
}

const JOB_COLUMNS: &[&str] = &[
    "id",
    "workflow_id",
    "name",
    "command",
    "cancel_on_blocking_job_failure",
    "supports_termination",
    "resource_requirements_id",
    "invocation_script",
    "status",
    "scheduler_id",
    "failure_handler_id",
    "attempt_id",
];

impl JobsApiImpl {
    pub fn new(context: ApiContext) -> Self {
        Self { context }
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

        match sqlx::query(&sql)
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

        match sqlx::query(&sql)
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
                       status, cancel_on_blocking_job_failure, supports_termination, scheduler_id,
                       failure_handler_id, attempt_id, priority
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
        let status = match JobStatus::from_int(status_int) {
            Ok(s) => s,
            Err(e) => {
                error!(
                    "Failed to parse job status '{}' for job {}: {}",
                    status_int, id, e
                );
                return Err(ApiError(format!("Failed to parse job status: {}", e)));
            }
        };

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
            scheduler_id: record.try_get("scheduler_id").ok(),
            schedule_compute_nodes: None, // This field is not stored in the database
            failure_handler_id: record.try_get("failure_handler_id").ok(),
            attempt_id: record.try_get("attempt_id").ok(),
            priority: record.try_get("priority").ok(),
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

            // Parse the current status
            let current_status = match JobStatus::from_int(current_status_int) {
                Ok(status) => status,
                Err(e) => {
                    error!(
                        "Failed to parse current job status '{}': {}",
                        current_status_int, e
                    );
                    continue;
                }
            };

            // Reset the job status
            match sqlx::query!(
                "UPDATE job SET status = $1 WHERE id = $2",
                uninitialized_status,
                job_id
            )
            .execute(self.context.pool.as_ref())
            .await
            {
                Ok(result) => {
                    if result.rows_affected() > 0 {
                        total_reset_count += 1;

                        // Clear active_compute_node_id for the reset job
                        if let Err(e) = sqlx::query!(
                            "UPDATE job_internal SET active_compute_node_id = NULL WHERE job_id = ?",
                            job_id
                        )
                        .execute(self.context.pool.as_ref())
                        .await
                        {
                            error!(
                                "Failed to clear active_compute_node_id for job {}: {}",
                                job_id, e
                            );
                            // Continue anyway
                        }

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

    /// Reset all jobs downstream of the given job to Uninitialized status
    /// This is called when a completed job needs to be reset, requiring all
    /// downstream jobs to also be reset recursively
    async fn update_jobs_from_completion_reversal(&self, job_id: i64) -> Result<(), ApiError> {
        debug!(
            "update_jobs_from_completion_reversal: resetting downstream jobs for job_id={}",
            job_id
        );

        let uninitialized_status = JobStatus::Uninitialized.to_int();

        // Begin a transaction with immediate lock to ensure atomicity
        // SQLx automatically uses BEGIN IMMEDIATE for SQLite when the first write occurs
        let mut tx = match self.context.pool.begin().await {
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
            SET status = ?
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
                // Transaction will be automatically rolled back when tx is dropped
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

        // Build JSON object with all input fields in deterministic order
        let hash_input = serde_json::json!({
            "command": job.command,
            "invocation_script": invocation_script,
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
        let hash_hex = format!("{:x}", hash_bytes);

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
            SELECT id, command, invocation_script
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
                let mut query = sqlx::query(&sql);
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
            let hash_hex = format!("{:x}", hasher.finalize());

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
            let mut query = sqlx::query(&sql);
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
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("priority must be >= 0, got {} for job '{}'", priority, job.name)
            }));
            return Ok(CreateJobResponse::UnprocessableContentErrorResponse(
                error_response,
            ));
        }
        let status = JobStatus::Uninitialized;
        let status_int = status.to_int();
        job.status = Some(status);

        // Begin a transaction to ensure job and all relationships are created atomically
        let mut tx = match self.context.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to begin transaction"));
            }
        };

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
                status,
                scheduler_id,
                failure_handler_id,
                priority
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
        .bind(status_int)
        .bind(job.scheduler_id)
        .bind(job.failure_handler_id)
        .bind(priority)
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
                if let Err(e) = self
                    .add_depends_on_association(&mut *tx, job_id, *blocking_id, job.workflow_id)
                    .await
                {
                    let _ = tx.rollback().await;
                    return Err(e);
                }
            }
        }

        // Handle input files
        if let Some(input_file_ids) = &job.input_file_ids {
            for file_id in input_file_ids {
                if let Err(e) = self
                    .add_job_file_association(
                        &mut *tx,
                        job_id,
                        *file_id,
                        job.workflow_id,
                        "job_input_file",
                    )
                    .await
                {
                    let _ = tx.rollback().await;
                    return Err(e);
                }
            }
        }

        // Handle output files
        if let Some(output_file_ids) = &job.output_file_ids {
            for file_id in output_file_ids {
                if let Err(e) = self
                    .add_job_file_association(
                        &mut *tx,
                        job_id,
                        *file_id,
                        job.workflow_id,
                        "job_output_file",
                    )
                    .await
                {
                    let _ = tx.rollback().await;
                    return Err(e);
                }
            }
        }

        // Handle input user_data
        if let Some(input_user_data_ids) = &job.input_user_data_ids {
            for user_data_id in input_user_data_ids {
                if let Err(e) = self
                    .add_job_user_data_association(
                        &mut *tx,
                        job_id,
                        *user_data_id,
                        "job_input_user_data",
                    )
                    .await
                {
                    let _ = tx.rollback().await;
                    return Err(e);
                }
            }
        }

        // Handle output user_data
        if let Some(output_user_data_ids) = &job.output_user_data_ids {
            for user_data_id in output_user_data_ids {
                if let Err(e) = self
                    .add_job_user_data_association(
                        &mut *tx,
                        job_id,
                        *user_data_id,
                        "job_output_user_data",
                    )
                    .await
                {
                    let _ = tx.rollback().await;
                    return Err(e);
                }
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

        if body.jobs.is_empty() {
            return Ok(CreateJobsResponse::SuccessfulResponse(
                models::CreateJobsResponse { jobs: Some(vec![]) },
            ));
        }

        // Check if we're within the recommended limit
        if body.jobs.len() > MAX_RECORD_TRANSFER_COUNT as usize {
            error!(
                "Too many jobs in batch: {}. Maximum is {}",
                body.jobs.len(),
                MAX_RECORD_TRANSFER_COUNT
            );
            return Err(ApiError(format!(
                "Too many jobs in batch: {}. Maximum is {}",
                body.jobs.len(),
                MAX_RECORD_TRANSFER_COUNT
            )));
        }

        let mut added_jobs = Vec::new();

        // Use a transaction for all operations to ensure consistency
        let mut transaction = match self.context.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => return Err(database_error_with_msg(e, "Failed to begin transaction")),
        };

        // Process each job
        for mut job in body.jobs {
            let invocation_script = job.invocation_script.clone();
            let cancel_on_blocking_job_failure = job.cancel_on_blocking_job_failure.unwrap_or(true);
            let supports_termination = job.supports_termination.unwrap_or(false);
            let priority = job.priority.unwrap_or(0);
            if priority < 0 {
                let _ = transaction.rollback().await;
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("priority must be >= 0, got {} for job '{}'", priority, job.name)
                }));
                return Ok(CreateJobsResponse::UnprocessableContentErrorResponse(
                    error_response,
                ));
            }
            let status = JobStatus::Uninitialized;
            let status_int = status.to_int();
            job.status = Some(status);

            // Insert the job
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
                    status,
                    scheduler_id,
                    failure_handler_id,
                    priority
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
            .bind(status_int)
            .bind(job.scheduler_id)
            .bind(job.failure_handler_id)
            .bind(priority)
            .fetch_one(&mut *transaction)
            .await
            {
                Ok(result) => result,
                Err(e) => {
                    let _ = transaction.rollback().await;
                    return Err(database_error_with_msg(
                        e,
                        "Failed to create job record in bulk",
                    ));
                }
            };

            let job_id: i64 = job_result.get("id");
            job.id = Some(job_id);

            // Handle job dependencies
            if let Some(depends_on_job_ids) = &job.depends_on_job_ids {
                for blocking_id in depends_on_job_ids {
                    if let Err(e) = sqlx::query!(
                        r#"
                        INSERT INTO job_depends_on (job_id, depends_on_job_id, workflow_id)
                        VALUES ($1, $2, $3)
                        "#,
                        job_id,
                        *blocking_id,
                        job.workflow_id
                    )
                    .execute(&mut *transaction)
                    .await
                    {
                        let _ = transaction.rollback().await;
                        return Err(database_error_with_msg(
                            e,
                            "Failed to create job association in bulk",
                        ));
                    }
                }
            }

            // Handle input files
            if let Some(input_file_ids) = &job.input_file_ids {
                for file_id in input_file_ids {
                    if let Err(e) = sqlx::query!(
                        r#"
                        INSERT INTO job_input_file (job_id, file_id, workflow_id)
                        VALUES ($1, $2, $3)
                        "#,
                        job_id,
                        *file_id,
                        job.workflow_id
                    )
                    .execute(&mut *transaction)
                    .await
                    {
                        let _ = transaction.rollback().await;
                        return Err(database_error_with_msg(
                            e,
                            "Failed to create job association in bulk",
                        ));
                    }
                }
            }

            // Handle output files
            if let Some(output_file_ids) = &job.output_file_ids {
                for file_id in output_file_ids {
                    if let Err(e) = sqlx::query!(
                        r#"
                        INSERT INTO job_output_file (job_id, file_id, workflow_id)
                        VALUES ($1, $2, $3)
                        "#,
                        job_id,
                        *file_id,
                        job.workflow_id
                    )
                    .execute(&mut *transaction)
                    .await
                    {
                        let _ = transaction.rollback().await;
                        return Err(database_error_with_msg(
                            e,
                            "Failed to create job association in bulk",
                        ));
                    }
                }
            }

            // Handle input user_data
            if let Some(input_user_data_ids) = &job.input_user_data_ids {
                for user_data_id in input_user_data_ids {
                    if let Err(e) = sqlx::query!(
                        r#"
                        INSERT INTO job_input_user_data (job_id, user_data_id)
                        VALUES ($1, $2)
                        "#,
                        job_id,
                        *user_data_id
                    )
                    .execute(&mut *transaction)
                    .await
                    {
                        let _ = transaction.rollback().await;
                        return Err(database_error_with_msg(
                            e,
                            "Failed to create job association in bulk",
                        ));
                    }
                }
            }

            // Handle output user_data
            if let Some(output_user_data_ids) = &job.output_user_data_ids {
                for user_data_id in output_user_data_ids {
                    if let Err(e) = sqlx::query!(
                        r#"
                        INSERT INTO job_output_user_data (job_id, user_data_id)
                        VALUES ($1, $2)
                        "#,
                        job_id,
                        *user_data_id
                    )
                    .execute(&mut *transaction)
                    .await
                    {
                        let _ = transaction.rollback().await;
                        return Err(database_error_with_msg(
                            e,
                            "Failed to create job association in bulk",
                        ));
                    }
                }
            }

            added_jobs.push(job);
        }

        // Commit the transaction
        if let Err(e) = transaction.commit().await {
            return Err(database_error_with_msg(e, "Failed to commit transaction"));
        }

        let workflow_id = added_jobs.first().map(|j| j.workflow_id).unwrap_or(0);
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
            Err(ApiError(msg)) if msg.contains("not found") => {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Job not found with ID: {}", id)
                }));
                Ok(GetJobResponse::NotFoundErrorResponse(error_response))
            }
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
    #[instrument(skip(self, context), fields(workflow_id, offset, limit))]
    async fn list_jobs(
        &self,
        workflow_id: i64,
        status: Option<JobStatus>,
        needs_file_id: Option<i64>,
        upstream_job_id: Option<i64>,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        include_relationships: Option<bool>,
        active_compute_node_id: Option<i64>,
        context: &C,
    ) -> Result<ListJobsResponse, ApiError> {
        debug!(
            "list_jobs({}, {:?}, {:?}, {:?}, {}, {}, {:?}, {:?}, {:?}, {:?}) - X-Span-ID: {:?}",
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
            context.get().0.clone()
        );

        // Build base query
        let base_query = "SELECT id, workflow_id, name, command, resource_requirements_id, invocation_script, status, cancel_on_blocking_job_failure, supports_termination, scheduler_id, failure_handler_id, attempt_id, priority FROM job".to_string();

        // Build WHERE clause conditions
        let mut where_conditions = vec!["workflow_id = ?".to_string()];
        let mut bind_values: Vec<Box<dyn sqlx::Encode<'_, sqlx::Sqlite> + Send>> =
            vec![Box::new(workflow_id)];

        if let Some(job_status) = &status {
            where_conditions.push("status = ?".to_string());
            bind_values.push(Box::new(job_status.to_int()));
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
            where_conditions.push(
                "id IN (SELECT job_id FROM job_internal WHERE active_compute_node_id = ?)"
                    .to_string(),
            );
        }

        let where_clause = where_conditions.join(" AND ");

        // Validate sort_by against whitelist
        let validated_sort_by = if let Some(ref col) = sort_by {
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
            .with_pagination_and_sorting(
                offset,
                limit,
                validated_sort_by,
                reverse_sort,
                "id",
                JOB_COLUMNS,
            )
            .build();

        debug!("Executing query: {}", query);

        // Execute the query
        let mut sqlx_query = sqlx::query(&query);

        // Bind workflow_id
        sqlx_query = sqlx_query.bind(workflow_id);

        // Bind optional parameters in order
        if let Some(ref s) = status {
            sqlx_query = sqlx_query.bind(s.to_int());
        }
        if let Some(file_id) = needs_file_id {
            sqlx_query = sqlx_query.bind(file_id);
        }
        if let Some(upstream_id) = upstream_job_id {
            sqlx_query = sqlx_query.bind(upstream_id);
        }
        if let Some(cn_id) = active_compute_node_id {
            sqlx_query = sqlx_query.bind(cn_id);
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
                let status = match JobStatus::from_int(status_int) {
                    Ok(s) => s,
                    Err(e) => {
                        error!(
                            "Failed to parse job status '{}' for job {}: {}",
                            status_int, job_id, e
                        );
                        return Err(ApiError(format!("Failed to parse job status: {}", e)));
                    }
                };

                items.push(models::JobModel {
                    id: Some(record.get("id")),
                    workflow_id: record.get("workflow_id"),
                    name: record.get("name"),
                    command: record.get("command"),
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
                    scheduler_id: record.try_get("scheduler_id").ok(),
                    schedule_compute_nodes: None,
                    failure_handler_id: record.try_get("failure_handler_id").ok(),
                    attempt_id: record.try_get("attempt_id").ok(),
                    priority: record.try_get("priority").ok(),
                });
            }
        }

        // For proper pagination, we should get the total count without LIMIT/OFFSET
        let count_query = SqlQueryBuilder::new("SELECT COUNT(*) as total FROM job".to_string())
            .with_where(where_clause)
            .build();

        let mut count_sqlx_query = sqlx::query(&count_query);
        count_sqlx_query = count_sqlx_query.bind(workflow_id);
        if let Some(ref s) = status {
            count_sqlx_query = count_sqlx_query.bind(s.to_int());
        }
        if let Some(file_id) = needs_file_id {
            count_sqlx_query = count_sqlx_query.bind(file_id);
        }
        if let Some(upstream_id) = upstream_job_id {
            count_sqlx_query = count_sqlx_query.bind(upstream_id);
        }
        if let Some(cn_id) = active_compute_node_id {
            count_sqlx_query = count_sqlx_query.bind(cn_id);
        }

        let total_count = match count_sqlx_query.fetch_one(self.context.pool.as_ref()).await {
            Ok(row) => row.get::<i64, _>("total"),
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to list jobs"));
            }
        };

        let current_count = items.len() as i64;
        let offset_val = offset;
        let has_more = offset_val + current_count < total_count;

        debug!(
            "list_jobs({}, {}/{}) - X-Span-ID: {:?}",
            workflow_id,
            current_count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListJobsResponse::SuccessfulResponse(
            models::ListJobsResponse {
                items,
                offset: offset_val,
                max_limit: MAX_RECORD_TRANSFER_COUNT,
                count: current_count,
                total_count,
                has_more,
            },
        ))
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
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Job not found with ID: {}", id)
                }));
                return Ok(UpdateJobResponse::NotFoundErrorResponse(error_response));
            }
            Err(e) => return Err(e),
        };

        // Check if job has a status
        let existing_status = match existing_job.status {
            Some(status) => status,
            None => {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": "Cannot update job - job has no status set"
                }));
                return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                    error_response,
                ));
            }
        };

        // Determine if we're only updating fields that are allowed at any time
        // (scheduler_id and resource_requirements_id can be updated regardless of status)
        // All fields are checked by comparing to existing values, since the client may
        // send the full job object with only scheduler_id/resource_requirements_id changed
        // Note: If body field is None, we treat it as "not changing" that field
        let name_changed = body.name != existing_job.name;
        let command_changed = body.command != existing_job.command;
        // Treat None as "not changing" - only compare if body.status is Some
        let status_changed = body.status.is_some() && body.status != existing_job.status;
        let input_file_ids_changed =
            body.input_file_ids.is_some() && body.input_file_ids != existing_job.input_file_ids;
        let output_file_ids_changed =
            body.output_file_ids.is_some() && body.output_file_ids != existing_job.output_file_ids;
        let input_user_data_ids_changed = body.input_user_data_ids.is_some()
            && body.input_user_data_ids != existing_job.input_user_data_ids;
        let output_user_data_ids_changed = body.output_user_data_ids.is_some()
            && body.output_user_data_ids != existing_job.output_user_data_ids;
        let depends_on_job_ids_changed = body.depends_on_job_ids.is_some()
            && body.depends_on_job_ids != existing_job.depends_on_job_ids;

        let has_restricted_updates = name_changed
            || command_changed
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

            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "Cannot update job {} when status is '{}' - most updates are only allowed when status is 'uninitialized'. \
                     Only scheduler_id and resource_requirements_id can be updated at any time. \
                     Changed fields: [{}]",
                    id,
                    existing_status,
                    changed_fields.join(", ")
                )
            }));
            return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                error_response,
            ));
        }

        // Restriction 2: Updating job status is not allowed, except to Disabled
        if let Some(new_status) = &body.status
            && let Some(ref existing_status) = existing_job.status
            && *new_status != *existing_status
            && *new_status != models::JobStatus::Disabled
        {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": "Cannot update job status - this field is immutable after job creation (except to Disabled)"
            }));
            return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                error_response,
            ));
        }

        // Check if depends_on_job_ids is being modified
        let mut depends_on_job_ids_modified = false;
        if let Some(depends_on_ids) = &body.depends_on_job_ids {
            let empty_vec = vec![];
            let existing_depends_on = existing_job
                .depends_on_job_ids
                .as_ref()
                .unwrap_or(&empty_vec);
            let mut body_sorted = depends_on_ids.clone();
            let mut existing_sorted = existing_depends_on.clone();
            body_sorted.sort();
            existing_sorted.sort();
            if body_sorted != existing_sorted {
                depends_on_job_ids_modified = true;
            }
        }

        // Validate other immutable fields - return error if they are set in body but don't match current job

        if let Some(input_file_ids) = &body.input_file_ids {
            let empty_vec = vec![];
            let existing_input_files = existing_job.input_file_ids.as_ref().unwrap_or(&empty_vec);
            let mut body_sorted = input_file_ids.clone();
            let mut existing_sorted = existing_input_files.clone();
            body_sorted.sort();
            existing_sorted.sort();
            if body_sorted != existing_sorted {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": "Cannot modify input_file_ids - this field is immutable after job creation"
                }));
                return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                    error_response,
                ));
            }
        }

        if let Some(output_file_ids) = &body.output_file_ids {
            let empty_vec = vec![];
            let existing_output_files = existing_job.output_file_ids.as_ref().unwrap_or(&empty_vec);
            let mut body_sorted = output_file_ids.clone();
            let mut existing_sorted = existing_output_files.clone();
            body_sorted.sort();
            existing_sorted.sort();
            if body_sorted != existing_sorted {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": "Cannot modify output_file_ids - this field is immutable after job creation"
                }));
                return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                    error_response,
                ));
            }
        }

        if let Some(input_user_data_ids) = &body.input_user_data_ids {
            let empty_vec = vec![];
            let existing_input_user_data = existing_job
                .input_user_data_ids
                .as_ref()
                .unwrap_or(&empty_vec);
            let mut body_sorted = input_user_data_ids.clone();
            let mut existing_sorted = existing_input_user_data.clone();
            body_sorted.sort();
            existing_sorted.sort();
            if body_sorted != existing_sorted {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": "Cannot modify input_user_data_ids - this field is immutable after job creation"
                }));
                return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                    error_response,
                ));
            }
        }

        if let Some(output_user_data_ids) = &body.output_user_data_ids {
            let empty_vec = vec![];
            let existing_output_user_data = existing_job
                .output_user_data_ids
                .as_ref()
                .unwrap_or(&empty_vec);
            let mut body_sorted = output_user_data_ids.clone();
            let mut existing_sorted = existing_output_user_data.clone();
            body_sorted.sort();
            existing_sorted.sort();
            if body_sorted != existing_sorted {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": "Cannot modify output_user_data_ids - this field is immutable after job creation"
                }));
                return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                    error_response,
                ));
            }
        }

        // Update the job (only non-relationship fields)
        let status_int = body.status.map(|s| s.to_int());

        if let Some(p) = body.priority
            && p < 0
        {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("priority must be >= 0, got {}", p)
            }));
            return Ok(UpdateJobResponse::UnprocessableContentErrorResponse(
                error_response,
            ));
        }

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
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Job not found with ID: {}", id)
            }));
            return Ok(UpdateJobResponse::NotFoundErrorResponse(error_response));
        }

        // If depends_on_job_ids was modified, update the relationships
        if depends_on_job_ids_modified {
            // Start a transaction for relationship updates
            let mut tx = match self.context.pool.begin().await {
                Ok(tx) => tx,
                Err(e) => return Err(database_error_with_msg(e, "Failed to begin transaction")),
            };

            // Delete existing depends_on relationships for this job
            if let Err(e) = sqlx::query!("DELETE FROM job_depends_on WHERE job_id = $1", id)
                .execute(&mut *tx)
                .await
            {
                let _ = tx.rollback().await;
                return Err(database_error_with_msg(
                    e,
                    "Failed to delete old job dependencies",
                ));
            }

            // Add new depends_on relationships if provided
            if let Some(depends_on_ids) = &body.depends_on_job_ids {
                for blocking_id in depends_on_ids {
                    if let Err(e) = sqlx::query!(
                        "INSERT INTO job_depends_on (job_id, depends_on_job_id, workflow_id) VALUES ($1, $2, $3)",
                        id,
                        *blocking_id,
                        existing_job.workflow_id
                    )
                    .execute(&mut *tx)
                    .await
                    {
                        let _ = tx.rollback().await;
                        return Err(database_error_with_msg(e, "Failed to update job dependencies"));
                    }
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
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Job not found with ID: {}", id)
            }));
            return Ok(UpdateJobResponse::NotFoundErrorResponse(error_response));
        }

        // Return the updated job by fetching it again with relationships
        let updated_job = self.get_job_with_relationships(id).await?;

        debug!("Updated job status for job id: {}", id);
        Ok(UpdateJobResponse::SuccessfulResponse(updated_job))
    }

    /// Return user-requested number of jobs that are ready for submission. Sets status to pending.
    #[instrument(skip(self, context), fields(workflow_id = id, requested_job_count))]
    async fn claim_next_jobs(
        &self,
        id: i64,
        requested_job_count: i64,
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

        let workflow_is_canceled =
            match sqlx::query("SELECT is_canceled FROM workflow_status WHERE workflow_id = $1")
                .bind(id)
                .fetch_optional(&mut *conn)
                .await
            {
                Ok(Some(row)) => row.get::<bool, _>("is_canceled"),
                Ok(None) => false,
                Err(e) => {
                    error!("Failed to query workflow cancellation status: {}", e);
                    return Err(ApiError("Database error".to_string()));
                }
            };

        if workflow_is_canceled {
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
        let query = r#"
            SELECT
                id as job_id,
                workflow_id,
                name,
                command,
                invocation_script,
                status,
                cancel_on_blocking_job_failure,
                supports_termination,
                resource_requirements_id,
                failure_handler_id,
                attempt_id,
                priority
            FROM job
            WHERE workflow_id = $1 AND status = $2
            ORDER BY priority DESC, id ASC
            LIMIT $3
            "#;

        let rows = match sqlx::query(query)
            .bind(id)
            .bind(ready_status)
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
        let mut job_ids_to_update = Vec::new();

        for row in rows {
            let job_id: i64 = row.get("job_id");
            job_ids_to_update.push(job_id);

            let job = models::JobModel {
                id: Some(job_id),
                workflow_id: row.get("workflow_id"),
                name: row.get("name"),
                command: row.get("command"),
                invocation_script: row.get("invocation_script"),
                status: Some(models::JobStatus::Pending),
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
            };

            selected_jobs.push(job);
        }

        let mut output_files_map: HashMap<i64, Vec<i64>> = HashMap::new();
        let mut output_user_data_map: HashMap<i64, Vec<i64>> = HashMap::new();

        if !job_ids_to_update.is_empty() {
            let output_files = match sqlx::query(
                "SELECT job_id, file_id FROM job_output_file WHERE workflow_id = $1",
            )
            .bind(id)
            .fetch_all(&mut *conn)
            .await
            {
                Ok(rows) => rows,
                Err(e) => {
                    error!("Failed to query output files: {}", e);
                    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                    return Err(ApiError("Database query error".to_string()));
                }
            };

            for row in output_files {
                let job_id: i64 = row.get("job_id");
                let file_id: i64 = row.get("file_id");
                if job_ids_to_update.contains(&job_id) {
                    output_files_map.entry(job_id).or_default().push(file_id);
                }
            }

            let output_user_data = match sqlx::query(
                "SELECT job_id, user_data_id FROM job_output_user_data WHERE job_id IN (SELECT id FROM job WHERE workflow_id = $1)",
            )
            .bind(id)
            .fetch_all(&mut *conn)
            .await
            {
                Ok(rows) => rows,
                Err(e) => {
                    error!("Failed to query output user_data: {}", e);
                    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                    return Err(ApiError("Database query error".to_string()));
                }
            };

            for row in output_user_data {
                let job_id: i64 = row.get("job_id");
                let user_data_id: i64 = row.get("user_data_id");
                if job_ids_to_update.contains(&job_id) {
                    output_user_data_map
                        .entry(job_id)
                        .or_default()
                        .push(user_data_id);
                }
            }
        }

        for job in &mut selected_jobs {
            if let Some(job_id) = job.id {
                job.output_file_ids = output_files_map.get(&job_id).cloned();
                job.output_user_data_ids = output_user_data_map.get(&job_id).cloned();
            }
        }

        if !job_ids_to_update.is_empty() {
            let pending = models::JobStatus::Pending.to_int();
            let job_ids_str = job_ids_to_update
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "UPDATE job SET status = {} WHERE id IN ({})",
                pending, job_ids_str
            );
            if let Err(e) = sqlx::query(&sql).execute(&mut *conn).await {
                error!("Failed to update job status: {}", e);
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                return Err(ApiError("Database update error".to_string()));
            }

            debug!(
                "Updated {} jobs to pending status for workflow {}",
                job_ids_to_update.len(),
                id
            );
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
                    "Job {} ({}) input hash changed: stored={}, current={}",
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
            SET status = $1
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

        // Clear active_compute_node_id for all jobs in the workflow
        if let Err(e) = sqlx::query!(
            "UPDATE job_internal SET active_compute_node_id = NULL WHERE job_id IN (SELECT id FROM job WHERE workflow_id = ?)",
            id
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            error!(
                "Failed to clear active_compute_node_id for workflow {}: {}",
                id, e
            );
            // Continue anyway - the job status reset succeeded
        }

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
        max_retries: i32,
        context: &C,
    ) -> Result<RetryJobResponse, ApiError> {
        debug!(
            "retry_job({}, {}, {}) - X-Span-ID: {:?}",
            id,
            run_id,
            max_retries,
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
                   j.invocation_script, j.cancel_on_blocking_job_failure, j.supports_termination,
                   j.resource_requirements_id, j.scheduler_id, j.priority,
                   ws.run_id as workflow_run_id
            FROM job j
            JOIN workflow w ON j.workflow_id = w.id
            JOIN workflow_status ws ON w.status_id = ws.id
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
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Job not found with ID: {}", id)
                }));
                return Ok(RetryJobResponse::NotFoundErrorResponse(error_response));
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
        let cancel_on_blocking_job_failure: Option<bool> =
            job_record.get("cancel_on_blocking_job_failure");
        let supports_termination: Option<bool> = job_record.get("supports_termination");
        let resource_requirements_id: Option<i64> = job_record.get("resource_requirements_id");
        let scheduler_id: Option<i64> = job_record.get("scheduler_id");
        let workflow_run_id: i64 = job_record.get("workflow_run_id");

        // Verify run_id matches
        if workflow_run_id != run_id {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "Run ID mismatch: provided {} but workflow is at run {}",
                    run_id, workflow_run_id
                )
            }));
            return Ok(RetryJobResponse::UnprocessableContentErrorResponse(
                error_response,
            ));
        }

        // Verify job is in a retryable state (Running, Failed, or Terminated)
        // Note: Running is allowed because the job runner may call retry_job before complete_job
        // when handling failure recovery (the job has finished locally but the server hasn't been
        // notified yet).
        let current_status = match JobStatus::from_int(status_int) {
            Ok(s) => s,
            Err(e) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                error!("Failed to parse job status: {}", e);
                return Err(ApiError(format!("Failed to parse job status: {}", e)));
            }
        };

        if current_status != JobStatus::Running
            && current_status != JobStatus::Failed
            && current_status != JobStatus::Terminated
        {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "Job cannot be retried: status is {:?}, must be Running, Failed, or Terminated",
                    current_status
                )
            }));
            return Ok(RetryJobResponse::UnprocessableContentErrorResponse(
                error_response,
            ));
        }

        // Validate max_retries (server-side enforcement)
        if attempt_id >= max_retries as i64 {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "Job cannot be retried: attempt_id {} >= max_retries {}",
                    attempt_id, max_retries
                )
            }));
            return Ok(RetryJobResponse::UnprocessableContentErrorResponse(
                error_response,
            ));
        }

        // Get current attempt_id and increment
        let new_attempt = attempt_id + 1;

        // Update job status to Ready and increment attempt_id
        let ready_status = JobStatus::Ready.to_int();
        if let Err(e) = sqlx::query(
            r#"
            UPDATE job
            SET status = ?, attempt_id = ?
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
            status: Some(status),
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
        };

        Ok(RetryJobResponse::SuccessfulResponse(job_model))
    }
}
