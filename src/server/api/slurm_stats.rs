//! Slurm accounting stats API endpoints

use crate::server::transport_types::context_types::{ApiError, Has, XSpanIdString};
use async_trait::async_trait;
use log::debug;

use crate::models;
use crate::server::api_responses::{CreateSlurmStatsResponse, ListSlurmStatsResponse};

use super::{ApiContext, MAX_RECORD_TRANSFER_COUNT, database_error_with_msg};

/// Trait defining slurm_stats API operations
#[async_trait]
#[allow(clippy::too_many_arguments)]
pub trait SlurmStatsApi<C> {
    /// Store Slurm accounting stats for a job step.
    async fn create_slurm_stats(
        &self,
        body: models::SlurmStatsModel,
        context: &C,
    ) -> Result<CreateSlurmStatsResponse, ApiError>;

    /// List Slurm accounting stats for a workflow, optionally filtered by job/run/attempt.
    async fn list_slurm_stats(
        &self,
        workflow_id: i64,
        job_id: Option<i64>,
        run_id: Option<i64>,
        attempt_id: Option<i64>,
        offset: i64,
        limit: i64,
        context: &C,
    ) -> Result<ListSlurmStatsResponse, ApiError>;
}

#[derive(Clone)]
pub struct SlurmStatsApiImpl {
    context: ApiContext,
}

impl SlurmStatsApiImpl {
    pub fn new(context: ApiContext) -> Self {
        SlurmStatsApiImpl { context }
    }
}

#[async_trait]
impl<C: Send + Sync + Has<XSpanIdString>> SlurmStatsApi<C> for SlurmStatsApiImpl {
    async fn create_slurm_stats(
        &self,
        body: models::SlurmStatsModel,
        context: &C,
    ) -> Result<CreateSlurmStatsResponse, ApiError> {
        let span_id: &XSpanIdString = context.get();
        debug!(
            "create_slurm_stats(workflow_id={} job_id={} run_id={} attempt_id={}) - X-Span-ID: {:?}",
            body.workflow_id, body.job_id, body.run_id, body.attempt_id, span_id
        );

        let pool = self.context.pool.clone();
        let result = sqlx::query!(
            r#"
            INSERT INTO slurm_stats
            (workflow_id, job_id, run_id, attempt_id,
             slurm_job_id,
             max_rss_bytes, max_vm_size_bytes,
             max_disk_read_bytes, max_disk_write_bytes,
             ave_cpu_seconds, node_list)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING rowid
            "#,
            body.workflow_id,
            body.job_id,
            body.run_id,
            body.attempt_id,
            body.slurm_job_id,
            body.max_rss_bytes,
            body.max_vm_size_bytes,
            body.max_disk_read_bytes,
            body.max_disk_write_bytes,
            body.ave_cpu_seconds,
            body.node_list,
        )
        .fetch_one(&*pool)
        .await;

        match result {
            Ok(row) => {
                let mut created = body;
                created.id = Some(row.id);
                debug!(
                    "Created slurm_stats id={} workflow_id={} job_id={}",
                    row.id, created.workflow_id, created.job_id
                );
                Ok(CreateSlurmStatsResponse::SuccessfulResponse(created))
            }
            Err(e) => Err(database_error_with_msg(
                e,
                "Failed to create slurm_stats record",
            )),
        }
    }

    async fn list_slurm_stats(
        &self,
        workflow_id: i64,
        job_id: Option<i64>,
        run_id: Option<i64>,
        attempt_id: Option<i64>,
        offset: i64,
        limit: i64,
        context: &C,
    ) -> Result<ListSlurmStatsResponse, ApiError> {
        let span_id: &XSpanIdString = context.get();
        debug!(
            "list_slurm_stats(workflow_id={} job_id={:?} run_id={:?} attempt_id={:?}) - X-Span-ID: {:?}",
            workflow_id, job_id, run_id, attempt_id, span_id
        );

        let limit = limit.min(MAX_RECORD_TRANSFER_COUNT);
        let pool = self.context.pool.clone();

        let count_row = sqlx::query!(
            r#"
            SELECT COUNT(*) as total
            FROM slurm_stats
            WHERE workflow_id = $1
              AND ($2 IS NULL OR job_id = $2)
              AND ($3 IS NULL OR run_id = $3)
              AND ($4 IS NULL OR attempt_id = $4)
            "#,
            workflow_id,
            job_id,
            run_id,
            attempt_id,
        )
        .fetch_one(&*pool)
        .await;

        let total_count = match count_row {
            Ok(row) => row.total,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to count slurm_stats"));
            }
        };

        let rows = sqlx::query!(
            r#"
            SELECT id, workflow_id, job_id, run_id, attempt_id,
                   slurm_job_id,
                   max_rss_bytes, max_vm_size_bytes,
                   max_disk_read_bytes, max_disk_write_bytes,
                   ave_cpu_seconds, node_list
            FROM slurm_stats
            WHERE workflow_id = $1
              AND ($2 IS NULL OR job_id = $2)
              AND ($3 IS NULL OR run_id = $3)
              AND ($4 IS NULL OR attempt_id = $4)
            ORDER BY id
            LIMIT $5 OFFSET $6
            "#,
            workflow_id,
            job_id,
            run_id,
            attempt_id,
            limit,
            offset,
        )
        .fetch_all(&*pool)
        .await;

        match rows {
            Ok(records) => {
                let items: Vec<models::SlurmStatsModel> = records
                    .into_iter()
                    .map(|r| models::SlurmStatsModel {
                        id: Some(r.id),
                        workflow_id: r.workflow_id,
                        job_id: r.job_id,
                        run_id: r.run_id,
                        attempt_id: r.attempt_id,
                        slurm_job_id: r.slurm_job_id,
                        max_rss_bytes: r.max_rss_bytes,
                        max_vm_size_bytes: r.max_vm_size_bytes,
                        max_disk_read_bytes: r.max_disk_read_bytes,
                        max_disk_write_bytes: r.max_disk_write_bytes,
                        ave_cpu_seconds: r.ave_cpu_seconds,
                        node_list: r.node_list,
                    })
                    .collect();
                let count = items.len() as i64;
                let has_more = offset + count < total_count;
                let mut response = models::ListSlurmStatsResponse::new(
                    offset,
                    limit,
                    count,
                    total_count,
                    has_more,
                );
                response.items = items;
                Ok(ListSlurmStatsResponse::SuccessfulResponse(response))
            }
            Err(e) => Err(database_error_with_msg(e, "Failed to list slurm_stats")),
        }
    }
}
