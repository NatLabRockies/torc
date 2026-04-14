use super::Server;
use crate::models;
use crate::server::api::database_lock_aware_error;
use crate::server::transport_types::context_types::{ApiError, EmptyContext, Has, XSpanIdString};
use log::{debug, error, info};
use sqlx::Row;
use std::sync::atomic::Ordering;

#[derive(Debug)]
struct CompletedJobRecord {
    id: i64,
    return_code: i64,
}

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

    let workflows = match sqlx::query(
        r#"
        SELECT DISTINCT workflow_id
        FROM job
        WHERE status IN (?, ?, ?, ?)
          AND unblocking_processed = 0
        "#,
    )
    .bind(completed_status)
    .bind(failed_status)
    .bind(canceled_status)
    .bind(terminated_status)
    .fetch_all(server.pool.as_ref())
    .await
    {
        Ok(rows) => rows
            .into_iter()
            .map(|row| row.get("workflow_id"))
            .collect::<Vec<i64>>(),
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

    for workflow_id in workflows {
        if let Err(e) = process_workflow_unblocks(server, workflow_id).await {
            error!(
                "Error processing unblocks for workflow {}: {}",
                workflow_id, e
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

    let completed_jobs = match sqlx::query(
        r#"
        SELECT j.id, r.return_code
        FROM job j
        JOIN workflow_status ws ON j.workflow_id = ws.id
        JOIN result r
          ON j.id = r.job_id
         AND r.run_id = ws.run_id
         AND r.attempt_id = j.attempt_id
        WHERE j.workflow_id = ?
          AND j.status IN (?, ?, ?, ?)
          AND j.unblocking_processed = 0
        "#,
    )
    .bind(workflow_id)
    .bind(completed_status)
    .bind(failed_status)
    .bind(canceled_status)
    .bind(terminated_status)
    .fetch_all(&mut *tx)
    .await
    {
        Ok(rows) => rows
            .into_iter()
            .map(|row| CompletedJobRecord {
                id: row.get("id"),
                return_code: row.get("return_code"),
            })
            .collect::<Vec<_>>(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::auth::{SharedCredentialCache, SharedHtpasswd};
    use parking_lot::RwLock;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;
    use std::sync::Arc;

    async fn test_server_with_schema() -> Server<EmptyContext> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::from_str("sqlite::memory:")
                    .expect("sqlite memory connection")
                    .create_if_missing(true),
            )
            .await
            .expect("in-memory pool");
        sqlx::migrate!("./torc-server/migrations")
            .run(&pool)
            .await
            .expect("migrations");

        let htpasswd: SharedHtpasswd = Arc::new(RwLock::new(None));
        let credential_cache: SharedCredentialCache = Arc::new(RwLock::new(None));
        Server::new(pool, false, htpasswd, None, credential_cache)
    }

    async fn insert_workflow(server: &Server<EmptyContext>, run_id: i64) -> i64 {
        let result = sqlx::query(
            r#"
            INSERT INTO workflow_status (run_id, has_detected_need_to_run_completion_script, is_canceled, is_archived)
            VALUES (?, 0, 0, 0)
            "#,
        )
        .bind(run_id)
        .execute(server.pool.as_ref())
        .await
        .expect("insert workflow_status");
        let workflow_status_id = result.last_insert_rowid();

        let result = sqlx::query(
            r#"
            INSERT INTO workflow (name, description, user, timestamp, is_archived, status_id)
            VALUES (?, NULL, ?, ?, 0, ?)
            "#,
        )
        .bind(format!("wf-{workflow_status_id}"))
        .bind("test-user")
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(workflow_status_id)
        .execute(server.pool.as_ref())
        .await
        .expect("insert workflow");
        result.last_insert_rowid()
    }

    async fn insert_compute_node(server: &Server<EmptyContext>, workflow_id: i64) -> i64 {
        let result = sqlx::query(
            r#"
            INSERT INTO compute_node (
                workflow_id, hostname, pid, start_time, duration_seconds, is_active,
                num_cpus, memory_gb, num_gpus, num_nodes, time_limit,
                scheduler_config_id, compute_node_type, scheduler
            )
            VALUES (?, ?, ?, ?, NULL, 1, ?, ?, ?, ?, NULL, NULL, ?, NULL)
            "#,
        )
        .bind(workflow_id)
        .bind("test-node")
        .bind(1234_i64)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(8_i64)
        .bind(16.0_f64)
        .bind(0_i64)
        .bind(1_i64)
        .bind("local")
        .execute(server.pool.as_ref())
        .await
        .expect("insert compute_node");
        result.last_insert_rowid()
    }

    async fn insert_job(
        server: &Server<EmptyContext>,
        workflow_id: i64,
        name: &str,
        status: i32,
        unblocking_processed: i64,
        attempt_id: i64,
        cancel_on_blocking_job_failure: bool,
    ) -> i64 {
        let result = sqlx::query(
            r#"
            INSERT INTO job (
                workflow_id, name, command, cancel_on_blocking_job_failure,
                supports_termination, resource_requirements_id, invocation_script,
                status, scheduler_id, scheduler_type, schedule_compute_nodes,
                unblocking_processed, failure_handler_id, attempt_id, priority
            )
            VALUES (?, ?, ?, ?, 0, NULL, NULL, ?, NULL, NULL, NULL, ?, NULL, ?, 0)
            "#,
        )
        .bind(workflow_id)
        .bind(name)
        .bind(format!("echo {name}"))
        .bind(cancel_on_blocking_job_failure)
        .bind(status)
        .bind(unblocking_processed)
        .bind(attempt_id)
        .execute(server.pool.as_ref())
        .await
        .expect("insert job");
        result.last_insert_rowid()
    }

    async fn insert_dependency(
        server: &Server<EmptyContext>,
        workflow_id: i64,
        job_id: i64,
        depends_on_job_id: i64,
    ) {
        sqlx::query(
            "INSERT INTO job_depends_on (job_id, depends_on_job_id, workflow_id) VALUES (?, ?, ?)",
        )
        .bind(job_id)
        .bind(depends_on_job_id)
        .bind(workflow_id)
        .execute(server.pool.as_ref())
        .await
        .expect("insert dependency");
    }

    #[allow(clippy::too_many_arguments)]
    async fn insert_result(
        server: &Server<EmptyContext>,
        workflow_id: i64,
        job_id: i64,
        run_id: i64,
        attempt_id: i64,
        compute_node_id: i64,
        return_code: i64,
        status: i32,
    ) {
        sqlx::query(
            r#"
            INSERT INTO result (
                workflow_id, job_id, run_id, attempt_id, compute_node_id,
                return_code, exec_time_minutes, completion_time, status,
                peak_memory_bytes, avg_memory_bytes, peak_cpu_percent, avg_cpu_percent
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, NULL, NULL)
            "#,
        )
        .bind(workflow_id)
        .bind(job_id)
        .bind(run_id)
        .bind(attempt_id)
        .bind(compute_node_id)
        .bind(return_code)
        .bind(1.0_f64)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(status)
        .execute(server.pool.as_ref())
        .await
        .expect("insert result");
    }

    #[tokio::test]
    async fn process_pending_unblocks_handles_completed_jobs() {
        let server = test_server_with_schema().await;
        let workflow_id = insert_workflow(&server, 1).await;
        let compute_node_id = insert_compute_node(&server, workflow_id).await;
        let upstream_job_id = insert_job(
            &server,
            workflow_id,
            "upstream",
            models::JobStatus::Completed.to_int(),
            0,
            1,
            true,
        )
        .await;
        let downstream_job_id = insert_job(
            &server,
            workflow_id,
            "downstream",
            models::JobStatus::Blocked.to_int(),
            1,
            1,
            true,
        )
        .await;
        insert_dependency(&server, workflow_id, downstream_job_id, upstream_job_id).await;
        insert_result(
            &server,
            workflow_id,
            upstream_job_id,
            1,
            1,
            compute_node_id,
            0,
            models::JobStatus::Completed.to_int(),
        )
        .await;

        process_pending_unblocks(&server)
            .await
            .expect("process pending unblocks");

        let downstream_status: i64 = sqlx::query_scalar("SELECT status FROM job WHERE id = ?")
            .bind(downstream_job_id)
            .fetch_one(server.pool.as_ref())
            .await
            .expect("fetch downstream status");
        let unblocking_processed: i64 =
            sqlx::query_scalar("SELECT unblocking_processed FROM job WHERE id = ?")
                .bind(upstream_job_id)
                .fetch_one(server.pool.as_ref())
                .await
                .expect("fetch upstream unblocking_processed");

        assert_eq!(downstream_status, models::JobStatus::Ready.to_int() as i64);
        assert_eq!(unblocking_processed, 1);
    }

    #[tokio::test]
    async fn process_workflow_unblocks_uses_current_attempt_result_only() {
        let server = test_server_with_schema().await;
        let workflow_id = insert_workflow(&server, 7).await;
        let compute_node_id = insert_compute_node(&server, workflow_id).await;
        let upstream_job_id = insert_job(
            &server,
            workflow_id,
            "upstream",
            models::JobStatus::Completed.to_int(),
            0,
            2,
            true,
        )
        .await;
        let downstream_job_id = insert_job(
            &server,
            workflow_id,
            "downstream",
            models::JobStatus::Blocked.to_int(),
            1,
            1,
            true,
        )
        .await;
        insert_dependency(&server, workflow_id, downstream_job_id, upstream_job_id).await;
        insert_result(
            &server,
            workflow_id,
            upstream_job_id,
            7,
            1,
            compute_node_id,
            1,
            models::JobStatus::Failed.to_int(),
        )
        .await;
        insert_result(
            &server,
            workflow_id,
            upstream_job_id,
            7,
            2,
            compute_node_id,
            0,
            models::JobStatus::Completed.to_int(),
        )
        .await;

        process_workflow_unblocks_inner(&server, workflow_id)
            .await
            .expect("process workflow unblocks");

        let workflows_with_failures = server
            .workflows_with_failures
            .read()
            .expect("read workflows_with_failures")
            .clone();
        let downstream_status: i64 = sqlx::query_scalar("SELECT status FROM job WHERE id = ?")
            .bind(downstream_job_id)
            .fetch_one(server.pool.as_ref())
            .await
            .expect("fetch downstream status");

        assert!(
            !workflows_with_failures.contains(&workflow_id),
            "successful current attempt should not mark workflow as failed"
        );
        assert_eq!(downstream_status, models::JobStatus::Ready.to_int() as i64);
    }
}
