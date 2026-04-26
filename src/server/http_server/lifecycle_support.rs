use super::*;
use crate::server::api::database_lock_aware_error;

impl<C> Server<C> {
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
}
