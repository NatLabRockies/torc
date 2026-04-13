use super::*;
use crate::server::api::{database_error_with_msg, database_lock_aware_error};

impl<C> Server<C> {
    pub(super) async fn manage_job_status_change(
        &self,
        job: &models::JobModel,
        run_id: i64,
    ) -> Result<(), ApiError> {
        let job_id = job
            .id
            .ok_or_else(|| ApiError("Job ID is required".to_string()))?;
        let new_status = job
            .status
            .as_ref()
            .ok_or_else(|| ApiError("Job status is required".to_string()))?;

        debug!(
            "manage_job_status_change: job_id={}, new_status={}, run_id={}",
            job_id, new_status, run_id
        );

        let current_job =
            match sqlx::query!("SELECT status, workflow_id FROM job WHERE id = ?", job_id)
                .fetch_optional(self.pool.as_ref())
                .await
            {
                Ok(Some(row)) => row,
                Ok(None) => {
                    error!("Job not found with ID: {}", job_id);
                    return Err(ApiError("Job not found".to_string()));
                }
                Err(e) => {
                    error!("Database error looking up job: {}", e);
                    return Err(ApiError("Database error".to_string()));
                }
            };

        let current_status = match models::JobStatus::from_int(current_job.status as i32) {
            Ok(status) => status,
            Err(e) => {
                error!(
                    "Failed to parse current job status '{}': {}",
                    current_job.status, e
                );
                return Err(ApiError("Invalid current job status".to_string()));
            }
        };

        if current_status == *new_status {
            debug!(
                "manage_job_status_change: job_id={} already has status '{}', no change needed",
                job_id, current_status
            );
            return Ok(());
        }

        debug!(
            "manage_job_status_change: job_id={} status change from '{}' to '{}'",
            job_id, current_status, new_status
        );

        if let Err(e) = self.validate_run_id(current_job.workflow_id, run_id).await {
            error!("manage_job_status_change: {}", e);
            return Err(ApiError(e));
        }

        if new_status.is_complete() {
            let result_record = match sqlx::query!(
                "SELECT return_code FROM result WHERE job_id = ? AND run_id = ?",
                job_id,
                run_id
            )
            .fetch_optional(self.pool.as_ref())
            .await
            {
                Ok(result) => result,
                Err(e) => {
                    error!("Database error checking for result: {}", e);
                    return Err(ApiError("Database error".to_string()));
                }
            };

            if result_record.is_none() {
                error!(
                    "No result found for job ID {} and run_id {}",
                    job_id, run_id
                );
                return Err(ApiError(
                    "No result found when transitioning to terminal status".to_string(),
                ));
            }
        }

        let new_status_int = new_status.to_int();

        if new_status.is_complete() {
            let completed_int = models::JobStatus::Completed.to_int();
            let failed_int = models::JobStatus::Failed.to_int();
            let canceled_int = models::JobStatus::Canceled.to_int();
            let terminated_int = models::JobStatus::Terminated.to_int();
            let disabled_int = models::JobStatus::Disabled.to_int();
            let pending_failed_int = models::JobStatus::PendingFailed.to_int();
            match sqlx::query!(
                "UPDATE job SET status = ?, unblocking_processed = 0 WHERE id = ? AND status NOT IN (?, ?, ?, ?, ?, ?)",
                new_status_int,
                job_id,
                completed_int,
                failed_int,
                canceled_int,
                terminated_int,
                disabled_int,
                pending_failed_int,
            )
            .execute(self.pool.as_ref())
            .await
            {
                Ok(result) => {
                    if result.rows_affected() == 0 {
                        let current = sqlx::query_scalar!(
                            "SELECT status FROM job WHERE id = ?",
                            job_id
                        )
                        .fetch_optional(self.pool.as_ref())
                        .await
                        .map_err(|e| {
                            database_error_with_msg(e, "Failed to re-check job status")
                        })?;

                        match current {
                            Some(status_int) => {
                                let status = models::JobStatus::from_int(status_int as i32)
                                    .unwrap_or(models::JobStatus::Failed);
                                if status.is_complete() {
                                    debug!(
                                        "Job {} already in terminal status {:?}, treating as idempotent success",
                                        job_id, status
                                    );
                                    return Ok(());
                                }
                                error!(
                                    "Job {} has unexpected status {:?} after conditional update matched 0 rows",
                                    job_id, status
                                );
                                return Err(ApiError(format!(
                                    "Job {} is in unexpected status {:?}",
                                    job_id, status
                                )));
                            }
                            None => {
                                error!("Job {} was deleted during status transition", job_id);
                                return Err(ApiError(format!("Job {} not found", job_id)));
                            }
                        }
                    }
                }
                Err(e) => {
                    return Err(database_error_with_msg(e, "Failed to update job status"));
                }
            }
            self.signal_job_completion();
            debug!(
                "Marked job {} as complete, unblocking will be processed by background task",
                job_id
            );
        } else {
            match sqlx::query!(
                "UPDATE job SET status = ? WHERE id = ?",
                new_status_int,
                job_id
            )
            .execute(self.pool.as_ref())
            .await
            {
                Ok(result) => {
                    if result.rows_affected() == 0 {
                        error!(
                            "No rows affected for job ID {} when updating status",
                            job_id
                        );
                        return Err(ApiError(
                            "Failed to update job status: no rows affected".to_string(),
                        ));
                    }
                }
                Err(e) => {
                    return Err(database_error_with_msg(e, "Failed to update job status"));
                }
            }
        }

        if current_status.is_complete() && !new_status.is_complete() {
            debug!(
                "manage_job_status_change: reverting completed job_id={}, resetting downstream jobs",
                job_id
            );
            self.update_jobs_from_completion_reversal(job_id).await?;
        }

        Ok(())
    }

    pub(super) async fn batch_unblock_jobs_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        workflow_id: i64,
        workflow_has_failures: bool,
    ) -> Result<Vec<i64>, ApiError> {
        let completed_status = models::JobStatus::Completed.to_int();
        let failed_status = models::JobStatus::Failed.to_int();
        let canceled_status = models::JobStatus::Canceled.to_int();
        let terminated_status = models::JobStatus::Terminated.to_int();
        let ready_status = models::JobStatus::Ready.to_int();
        let blocked_status = models::JobStatus::Blocked.to_int();

        if workflow_has_failures {
            let mut iterations = 0;
            loop {
                let canceled = match sqlx::query(
                    r#"
                    UPDATE job
                    SET status = ?
                    WHERE workflow_id = ?
                      AND status = ?
                      AND cancel_on_blocking_job_failure = 1
                      AND NOT EXISTS (
                          SELECT 1
                          FROM job_depends_on jbb
                          JOIN job j ON jbb.depends_on_job_id = j.id
                          WHERE jbb.job_id = job.id
                            AND j.status NOT IN (?, ?, ?, ?)
                      )
                      AND EXISTS (
                          SELECT 1
                          FROM job_depends_on jbb
                          JOIN job j ON jbb.depends_on_job_id = j.id
                          JOIN result r ON j.id = r.job_id
                          JOIN workflow_status ws ON j.workflow_id = ws.id
                            AND r.run_id = ws.run_id
                          WHERE jbb.job_id = job.id
                            AND j.status IN (?, ?, ?)
                            AND r.return_code != 0
                      )
                    "#,
                )
                .bind(canceled_status)
                .bind(workflow_id)
                .bind(blocked_status)
                .bind(completed_status)
                .bind(failed_status)
                .bind(canceled_status)
                .bind(terminated_status)
                .bind(failed_status)
                .bind(canceled_status)
                .bind(terminated_status)
                .execute(&mut **tx)
                .await
                {
                    Ok(result) => result.rows_affected(),
                    Err(e) => {
                        debug!("batch_unblock_jobs_tx: cancellation query failed: {}", e);
                        return Err(database_lock_aware_error(e, "Failed to update job status"));
                    }
                };

                if canceled == 0 {
                    break;
                }

                debug!(
                    "batch_unblock_jobs_tx: canceled {} jobs in iteration {} for workflow_id={}",
                    canceled, iterations, workflow_id
                );

                iterations += 1;
                if iterations >= 100 {
                    debug!(
                        "batch_unblock_jobs_tx: hit 100-iteration cap for cascading cancellations in workflow_id={}",
                        workflow_id
                    );
                    break;
                }
            }
        }

        let updated_jobs = match sqlx::query(
            r#"
            UPDATE job
            SET status = ?
            WHERE workflow_id = ?
              AND status = ?
              AND NOT EXISTS (
                  SELECT 1
                  FROM job_depends_on jbb
                  JOIN job j ON jbb.depends_on_job_id = j.id
                  WHERE jbb.job_id = job.id
                    AND j.status NOT IN (?, ?, ?, ?)
              )
            RETURNING id
            "#,
        )
        .bind(ready_status)
        .bind(workflow_id)
        .bind(blocked_status)
        .bind(completed_status)
        .bind(failed_status)
        .bind(canceled_status)
        .bind(terminated_status)
        .fetch_all(&mut **tx)
        .await
        {
            Ok(rows) => rows,
            Err(e) => {
                debug!("batch_unblock_jobs_tx: ready query failed: {}", e);
                return Err(database_lock_aware_error(e, "Failed to update job status"));
            }
        };

        let ready_job_ids: Vec<i64> = updated_jobs.iter().map(|r| r.get("id")).collect();
        debug!(
            "batch_unblock_jobs_tx: {} jobs became ready for workflow_id={}",
            ready_job_ids.len(),
            workflow_id
        );
        Ok(ready_job_ids)
    }

    pub(super) async fn reinitialize_downstream_jobs(
        &self,
        job_id: i64,
        workflow_id: i64,
    ) -> Result<(), ApiError> {
        debug!(
            "reinitialize_downstream_jobs: resetting downstream jobs for job_id={} in workflow={}",
            job_id, workflow_id
        );

        let completed_status = models::JobStatus::Completed.to_int();
        let failed_status = models::JobStatus::Failed.to_int();
        let uninitialized_status = models::JobStatus::Uninitialized.to_int();

        let result = match sqlx::query!(
            r#"
            UPDATE job
            SET status = ?
            WHERE workflow_id = ?
            AND id IN (
                SELECT DISTINCT jbb.job_id
                FROM job_depends_on jbb
                JOIN job j ON jbb.job_id = j.id
                WHERE jbb.depends_on_job_id = ?
                AND jbb.workflow_id = ?
                AND j.status IN (?, ?)
            )
            "#,
            uninitialized_status,
            workflow_id,
            job_id,
            workflow_id,
            completed_status,
            failed_status
        )
        .execute(self.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("Database error reinitializing downstream jobs: {}", e);
                return Err(ApiError("Database error".to_string()));
            }
        };

        let affected_count = result.rows_affected();
        if affected_count == 0 {
            debug!(
                "reinitialize_downstream_jobs: no downstream jobs to reinitialize for job_id={}",
                job_id
            );
        } else {
            info!(
                "reinitialize_downstream_jobs: successfully reinitialized {} downstream jobs for job_id={}",
                affected_count, job_id
            );
        }

        Ok(())
    }

    pub(super) async fn update_jobs_from_completion_reversal(
        &self,
        job_id: i64,
    ) -> Result<(), ApiError> {
        debug!(
            "update_jobs_from_completion_reversal: resetting downstream jobs for job_id={}",
            job_id
        );

        let uninitialized_status = models::JobStatus::Uninitialized.to_int();

        let mut tx = match self.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                error!("Failed to begin transaction for completion reversal: {}", e);
                return Err(ApiError("Database error".to_string()));
            }
        };

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

        let result = sqlx::query!(
            r#"
            WITH RECURSIVE downstream_jobs(job_id, level) AS (
                SELECT
                    jbb.job_id,
                    0 as level
                FROM job_depends_on jbb
                WHERE jbb.depends_on_job_id = ?
                  AND jbb.workflow_id = ?

                UNION ALL

                SELECT
                    jbb.job_id,
                    dj.level + 1 as level
                FROM downstream_jobs dj
                JOIN job_depends_on jbb ON jbb.depends_on_job_id = dj.job_id
                WHERE jbb.workflow_id = ?
                  AND dj.level < 100
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

                if let Err(e) = tx.commit().await {
                    error!(
                        "Failed to commit transaction for completion reversal: {}",
                        e
                    );
                    return Err(ApiError("Database error".to_string()));
                }

                info!(
                    "Successfully reset {} downstream jobs for job_id={} in workflow={}",
                    affected_rows, job_id, workflow_id
                );

                Ok(())
            }
            Err(e) => {
                error!(
                    "Database error during completion reversal for job {}: {}",
                    job_id, e
                );
                Err(ApiError("Database error".to_string()))
            }
        }
    }
}
