use super::{Server, ServerFailJobError};
use crate::models;
use crate::server::api::ComputeNodesApi;
use crate::server::api::compute_nodes::update_response_from_get;
use crate::server::api_responses::{GetComputeNodeResponse, UpdateComputeNodeResponse};
use crate::server::transport_types::auth_types::Authorization;
use crate::server::transport_types::context_types::{ApiError, Has, XSpanIdString};
use chrono::Utc;
use log::{debug, info, warn};
use sqlx::Row;

impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync + 'static,
{
    /// Returns true when a job could not be failed because of a server error.
    async fn fail_running_jobs(&self, running: Vec<sqlx::sqlite::SqliteRow>, context: &C) -> bool {
        let mut transport_failed = false;
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
            match self
                .fail_job_as_server(job_id, run_id, result, context)
                .await
            {
                Ok(()) => info!(
                    "Failed job workflow_id={} compute_node_id={} job_id={} job_name={} \
                     return_code={}: its compute node was failed",
                    job_workflow_id,
                    node_id,
                    job_id,
                    job.get::<String, _>("name"),
                    models::ORPHANED_JOB_RETURN_CODE
                ),
                Err(ServerFailJobError::Rejected(reason)) => debug!(
                    "Did not fail workflow_id={} compute_node_id={} job_id={}: {}",
                    job_workflow_id, node_id, job_id, reason
                ),
                Err(ServerFailJobError::Transport(reason)) => {
                    transport_failed = true;
                    warn!(
                        "Did not fail workflow_id={} compute_node_id={} job_id={}: {}",
                        job_workflow_id, node_id, job_id, reason
                    );
                }
            }
        }
        transport_failed
    }

    /// Deactivate a compute node and fail its running jobs. Safe to call again: a second call
    /// finds no running job and sends no event.
    pub(super) async fn fail_compute_node_as_server(
        &self,
        id: i64,
        context: &C,
    ) -> Result<UpdateComputeNodeResponse, ApiError> {
        let db_err = |e: sqlx::Error| ApiError(format!("Database error: {}", e));
        let deactivated = sqlx::query(
            "UPDATE compute_node SET is_active = 0 \
                 WHERE id = $1 AND (is_active IS NULL OR is_active = 1)",
        )
        .bind(id)
        .execute(self.pool.as_ref())
        .await
        .map_err(db_err)?
        .rows_affected()
            > 0;

        let node = match self.compute_nodes_api.get_compute_node(id, context).await? {
            GetComputeNodeResponse::SuccessfulResponse(node) => node,
            other => return update_response_from_get(other),
        };

        let running = sqlx::query(
            "SELECT j.id, j.name, j.workflow_id, j.attempt_id, j.compute_node_id, w.run_id \
             FROM job j \
             JOIN workflow w ON w.id = j.workflow_id \
             WHERE j.compute_node_id = $1 AND j.status = $2",
        )
        .bind(id)
        .bind(models::JobStatus::Running.to_int())
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(db_err)?;
        let transport_failed = self.fail_running_jobs(running, context).await;

        if deactivated {
            warn!(
                "Failed compute node workflow_id={} compute_node_id={} hostname={} pid={}",
                node.workflow_id, id, node.hostname, node.pid
            );
            self.broadcast_compute_node_stopped(
                node.workflow_id,
                models::EventSeverity::Warning,
                serde_json::json!({
                    "compute_node_id": id,
                    "hostname": node.hostname,
                    "pid": node.pid,
                    "reason": "failed",
                }),
            );
        }
        if transport_failed {
            return Err(ApiError(format!(
                "Failed to fail every running job of compute_node_id={}",
                id
            )));
        }
        Ok(UpdateComputeNodeResponse::SuccessfulResponse(node))
    }
}
