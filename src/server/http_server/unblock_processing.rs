use super::Server;
use crate::models;
use crate::server::api::database_lock_aware_error;
use crate::server::transport_types::context_types::{ApiError, EmptyContext, Has, XSpanIdString};
use log::{debug, error, info};
use std::sync::atomic::Ordering;

pub(super) async fn background_unblock_task<C>(server: Server<C>, interval_seconds: f64)
where
    C: Has<XSpanIdString> + Send + Sync,
{
    info!(
        "Starting background job completion checker with interval = {} seconds",
        interval_seconds
    );

    let mut interval = tokio::time::interval(std::time::Duration::from_secs_f64(interval_seconds));
    let mut last_checked_time: u64 = 0;

    loop {
        interval.tick().await;

        let completion_time = server.last_completion_time.load(Ordering::Acquire);
        if completion_time <= last_checked_time {
            debug!("No new job completions since last check, skipping unblock processing");
            continue;
        }

        last_checked_time = completion_time;

        if let Err(e) = process_pending_unblocks(&server).await {
            error!("Error processing pending unblocks: {}", e);
        }
    }
}

async fn process_pending_unblocks<C>(server: &Server<C>) -> Result<(), ApiError>
where
    C: Has<XSpanIdString> + Send + Sync,
{
    let completed_status = models::JobStatus::Completed.to_int();
    let failed_status = models::JobStatus::Failed.to_int();
    let canceled_status = models::JobStatus::Canceled.to_int();
    let terminated_status = models::JobStatus::Terminated.to_int();

    let workflows = match sqlx::query!(
        r#"
        SELECT DISTINCT workflow_id
        FROM job
        WHERE status IN (?, ?, ?, ?)
          AND unblocking_processed = 0
        "#,
        completed_status,
        failed_status,
        canceled_status,
        terminated_status
    )
    .fetch_all(server.pool.as_ref())
    .await
    {
        Ok(workflows) => workflows,
        Err(e) => {
            error!(
                "Database error finding workflows with pending unblocks: {}",
                e
            );
            return Err(ApiError("Database error".to_string()));
        }
    };

    if workflows.is_empty() {
        return Ok(());
    }

    debug!(
        "Processing pending unblocks for {} workflows",
        workflows.len()
    );

    for workflow in workflows {
        if let Err(e) = process_workflow_unblocks(server, workflow.workflow_id).await {
            error!(
                "Error processing unblocks for workflow {}: {}",
                workflow.workflow_id, e
            );
        }
    }

    Ok(())
}

fn is_database_lock_error(error: &ApiError) -> bool {
    let error_str = error.0.to_lowercase();
    error_str.contains("database is locked")
        || error_str.contains("database is busy")
        || error_str.contains("sqlite_busy")
}

async fn process_workflow_unblocks<C>(server: &Server<C>, workflow_id: i64) -> Result<(), ApiError>
where
    C: Has<XSpanIdString> + Send + Sync,
{
    const MAX_RETRIES: u32 = 20;
    const INITIAL_DELAY_MS: u64 = 10;
    const MAX_DELAY_MS: u64 = 2000;

    let mut last_error: Option<ApiError> = None;
    let mut delay_ms = INITIAL_DELAY_MS;

    for attempt in 0..MAX_RETRIES {
        match process_workflow_unblocks_inner(server, workflow_id).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                if is_database_lock_error(&e) && attempt < MAX_RETRIES - 1 {
                    debug!(
                        "Database locked for workflow {}, retrying in {}ms (attempt {}/{})",
                        workflow_id,
                        delay_ms,
                        attempt + 1,
                        MAX_RETRIES
                    );
                    last_error = Some(e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                    delay_ms = (delay_ms * 2).min(MAX_DELAY_MS);
                    continue;
                }
                return Err(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| ApiError("Unknown error in retry loop".to_string())))
}

async fn process_workflow_unblocks_inner<C>(
    server: &Server<C>,
    workflow_id: i64,
) -> Result<(), ApiError>
where
    C: Has<XSpanIdString> + Send + Sync,
{
    let completed_status = models::JobStatus::Completed.to_int();
    let failed_status = models::JobStatus::Failed.to_int();
    let canceled_status = models::JobStatus::Canceled.to_int();
    let terminated_status = models::JobStatus::Terminated.to_int();

    let mut tx = match server.pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            debug!(
                "Failed to begin transaction for workflow {}: {}",
                workflow_id, e
            );
            return Err(database_lock_aware_error(e, "Failed to begin transaction"));
        }
    };

    let completed_jobs = match sqlx::query!(
        r#"
        SELECT j.id, r.return_code
        FROM job j
        JOIN result r ON j.id = r.job_id
        JOIN workflow_status ws ON j.workflow_id = ws.id AND r.run_id = ws.run_id
        WHERE j.workflow_id = ?
          AND j.status IN (?, ?, ?, ?)
          AND j.unblocking_processed = 0
        "#,
        workflow_id,
        completed_status,
        failed_status,
        canceled_status,
        terminated_status
    )
    .fetch_all(&mut *tx)
    .await
    {
        Ok(jobs) => jobs,
        Err(e) => {
            debug!(
                "Database error fetching completed jobs for workflow {}: {}",
                workflow_id, e
            );
            return Err(database_lock_aware_error(
                e,
                "Failed to fetch completed jobs",
            ));
        }
    };

    if completed_jobs.is_empty() {
        return Ok(());
    }

    debug!(
        "Processing {} completed jobs for workflow {}",
        completed_jobs.len(),
        workflow_id
    );

    let batch_has_failures = completed_jobs.iter().any(|j| j.return_code != 0);
    let workflow_has_prior_failures = server
        .workflows_with_failures
        .read()
        .map(|set| set.contains(&workflow_id))
        .unwrap_or(true);

    if batch_has_failures && let Ok(mut set) = server.workflows_with_failures.write() {
        set.insert(workflow_id);
    }

    let workflow_has_failures = batch_has_failures || workflow_has_prior_failures;

    let all_ready_job_ids = match Server::<EmptyContext>::batch_unblock_jobs_tx(
        &mut tx,
        workflow_id,
        workflow_has_failures,
    )
    .await
    {
        Ok(ready_job_ids) => ready_job_ids,
        Err(e) => {
            debug!(
                "Error batch-unblocking jobs for workflow {}: {}",
                workflow_id, e
            );
            return Err(e);
        }
    };

    let job_ids: Vec<i64> = completed_jobs.iter().map(|j| j.id).collect();
    let job_ids_str = job_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let sql = format!(
        "UPDATE job SET unblocking_processed = 1 WHERE id IN ({})",
        job_ids_str
    );

    if let Err(e) = sqlx::query(&sql).execute(&mut *tx).await {
        debug!(
            "Database error marking jobs as processed for workflow {}: {}",
            workflow_id, e
        );
        return Err(database_lock_aware_error(
            e,
            "Failed to mark jobs processed",
        ));
    }

    if let Err(e) = tx.commit().await {
        debug!(
            "Failed to commit transaction for workflow {}: {}",
            workflow_id, e
        );
        return Err(database_lock_aware_error(e, "Failed to commit transaction"));
    }

    info!(
        "Jobs unblocked workflow_id={} completed_count={} ready_count={}",
        workflow_id,
        completed_jobs.len(),
        all_ready_job_ids.len()
    );

    if !all_ready_job_ids.is_empty() {
        debug!(
            "process_workflow_unblocks: checking on_jobs_ready actions for {} jobs that became ready",
            all_ready_job_ids.len()
        );

        if let Err(e) = server
            .workflow_actions_api
            .check_and_trigger_actions(
                workflow_id,
                "on_jobs_ready",
                Some(all_ready_job_ids.clone()),
            )
            .await
        {
            error!(
                "Failed to check_and_trigger_actions for on_jobs_ready: {}",
                e
            );
        }
    }

    Ok(())
}
