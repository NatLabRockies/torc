use super::Server;
use crate::models;
use crate::server::event_broadcast::BroadcastEvent;
use crate::server::transport_types::context_types::EmptyContext;
use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use sqlx::Row;
use std::time::Duration;

/// Expires compute nodes whose runner stopped sending heartbeats.
///
/// Every `interval`, each active node with a heartbeat older than `timeout` is marked inactive.
/// Then every running job whose node is inactive and has a heartbeat is failed with
/// `ORPHANED_JOB_RETURN_CODE`. The second step does not depend on the first, so a pass that
/// fails part way is finished by the next one. A node without a heartbeat (created by a runner
/// that does not send them) is never expired and its jobs are never failed here.
///
/// The first sweep waits one `timeout` after start-up. While the server was down, no runner
/// could deliver a heartbeat, so every node would look stale right after a restart.
pub(super) async fn background_compute_node_sweep_task(
    server: Server<EmptyContext>,
    timeout: Duration,
    interval: Duration,
) {
    info!(
        "Starting compute node heartbeat sweep (timeout = {:?}, interval = {:?})",
        timeout, interval
    );
    let mut ticker = tokio::time::interval_at(tokio::time::Instant::now() + timeout, interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let timeout = chrono::Duration::from_std(timeout).unwrap_or(chrono::Duration::MAX);
    loop {
        ticker.tick().await;
        if let Err(e) = sweep_stale_compute_nodes(&server, timeout).await {
            error!("Compute node heartbeat sweep failed: {}", e);
        }
        if let Err(e) = fail_jobs_of_inactive_nodes(&server).await {
            error!("Failing jobs of expired compute nodes failed: {}", e);
        }
    }
}

async fn sweep_stale_compute_nodes(
    server: &Server<EmptyContext>,
    timeout: chrono::Duration,
) -> Result<(), sqlx::Error> {
    let candidates = sqlx::query(
        "SELECT id, workflow_id, hostname, pid, last_heartbeat FROM compute_node \
         WHERE is_active = 1 AND last_heartbeat IS NOT NULL",
    )
    .fetch_all(server.pool.as_ref())
    .await?;

    let cutoff = Utc::now() - timeout;
    for row in candidates {
        let id: i64 = row.get("id");
        let last_heartbeat: String = row.get("last_heartbeat");
        match DateTime::parse_from_rfc3339(&last_heartbeat) {
            Ok(seen) if seen >= cutoff => continue,
            Ok(_) => {}
            Err(e) => {
                warn!(
                    "compute_node_id={} has an unparseable last_heartbeat '{}': {}",
                    id, last_heartbeat, e
                );
                continue;
            }
        }
        if let Err(e) = expire_compute_node(
            server,
            id,
            row.get("workflow_id"),
            row.get("hostname"),
            row.get("pid"),
            &last_heartbeat,
        )
        .await
        {
            error!("Failed to expire compute_node_id={}: {}", id, e);
        }
    }
    Ok(())
}

async fn expire_compute_node(
    server: &Server<EmptyContext>,
    id: i64,
    workflow_id: i64,
    hostname: String,
    pid: i64,
    last_heartbeat: &str,
) -> Result<(), sqlx::Error> {
    // Matching on the heartbeat value we read makes this a no-op when the runner sent a
    // heartbeat after the candidate query.
    let updated = sqlx::query(
        "UPDATE compute_node SET is_active = 0 \
         WHERE id = $1 AND is_active = 1 AND last_heartbeat = $2",
    )
    .bind(id)
    .bind(last_heartbeat)
    .execute(server.pool.as_ref())
    .await?;
    if updated.rows_affected() == 0 {
        debug!("compute_node_id={} sent a heartbeat during the sweep", id);
        return Ok(());
    }

    warn!(
        "Expired compute node workflow_id={} compute_node_id={} hostname={} pid={} \
         last_heartbeat={}: no heartbeat within the timeout",
        workflow_id, id, hostname, pid, last_heartbeat
    );
    server.event_broadcaster.broadcast(BroadcastEvent {
        workflow_id,
        timestamp: Utc::now().timestamp_millis(),
        event_type: "compute_node_stopped".to_string(),
        severity: models::EventSeverity::Warning,
        data: serde_json::json!({
            "compute_node_id": id,
            "hostname": hostname,
            "pid": pid,
            "reason": "heartbeat_timeout",
        }),
    });

    Ok(())
}

/// Fail each running job that belongs to an inactive node with a heartbeat. The completion
/// path rejects a job that was reset, retried or restarted on another node since this query
/// ran, so such a job is left alone.
async fn fail_jobs_of_inactive_nodes(server: &Server<EmptyContext>) -> Result<(), sqlx::Error> {
    let running = sqlx::query(
        "SELECT j.id, j.name, j.workflow_id, j.attempt_id, j.compute_node_id, w.run_id \
         FROM job j \
         JOIN compute_node n ON n.id = j.compute_node_id \
         JOIN workflow w ON w.id = j.workflow_id \
         WHERE j.compute_node_id IS NOT NULL AND j.status = $1 \
           AND n.is_active = 0 AND n.last_heartbeat IS NOT NULL",
    )
    .bind(models::JobStatus::Running.to_int())
    .fetch_all(server.pool.as_ref())
    .await?;

    let context = EmptyContext::default();
    for job in running {
        let job_id: i64 = job.get("id");
        let node_id: i64 = job.get("compute_node_id");
        let job_workflow_id: i64 = job.get("workflow_id");
        let run_id: i64 = job.get("run_id");
        let attempt_id: i64 = job.get("attempt_id");
        let result = models::ResultModel::new(
            job_id,
            job_workflow_id,
            run_id,
            attempt_id,
            node_id,
            models::ORPHANED_JOB_RETURN_CODE,
            0.0,
            Utc::now().to_rfc3339(),
            models::JobStatus::Failed,
        );
        match server
            .fail_job_as_server(job_id, run_id, result, &context)
            .await
        {
            Ok(()) => info!(
                "Failed job workflow_id={} job_id={} job_name={} return_code={}: its compute \
                 node compute_node_id={} stopped reporting",
                job_workflow_id,
                job_id,
                job.get::<String, _>("name"),
                models::ORPHANED_JOB_RETURN_CODE,
                node_id
            ),
            Err(e) => warn!(
                "Did not fail job_id={} of inactive compute_node_id={} (retried on the next \
                 sweep if still running there): {}",
                job_id, node_id, e
            ),
        }
    }
    Ok(())
}
