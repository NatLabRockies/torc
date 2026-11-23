//! Compute node-related API endpoints

#![allow(clippy::too_many_arguments)]

use crate::server::transport_types::context_types::{ApiError, Has, XSpanIdString};
use async_trait::async_trait;
use log::{debug, info, warn};
use sqlx::Row;

use crate::server::api_responses::{
    CreateComputeNodeResponse, DeleteComputeNodeResponse, DeleteComputeNodesResponse,
    GetComputeNodeResponse, ListComputeNodesResponse, UpdateComputeNodeResponse,
};

use crate::models;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::{
    ApiContext, SqlQueryBuilder, database_error_with_msg, labels_column,
    resource_not_found_response,
};

/// Trait defining compute node-related API operations
#[async_trait]
pub trait ComputeNodesApi<C> {
    /// Store a compute node.
    async fn create_compute_node(
        &self,
        body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<CreateComputeNodeResponse, ApiError>;

    /// Delete all compute nodes for one workflow.
    async fn delete_compute_nodes(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteComputeNodesResponse, ApiError>;

    /// Retrieve a compute node by ID.
    async fn get_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetComputeNodeResponse, ApiError>;

    /// Retrieve all compute nodes for one workflow.
    async fn list_compute_nodes(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        hostname: Option<String>,
        is_active: Option<bool>,
        scheduled_compute_node_id: Option<i64>,
        stale: Option<bool>,
        context: &C,
    ) -> Result<ListComputeNodesResponse, ApiError>;

    /// Update a compute node.
    async fn update_compute_node(
        &self,
        id: i64,
        body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<UpdateComputeNodeResponse, ApiError>;

    /// Record a liveness signal for an active compute node.
    async fn heartbeat_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<UpdateComputeNodeResponse, ApiError>;

    /// Delete a compute node.
    async fn delete_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteComputeNodeResponse, ApiError>;
}

/// Implementation of compute nodes API for the server
#[derive(Clone)]
pub struct ComputeNodesApiImpl {
    pub context: ApiContext,
    stale_after: Option<std::time::Duration>,
    started: std::time::Instant,
    /// Last heartbeat of each node. The column is written only on the first heartbeat of a node in
    /// this process and when a read finds the node stale, so heartbeats do not take the write lock.
    // ponytail: entries of inactive nodes stay until restart, about 32 bytes per node.
    heartbeats: Arc<Mutex<HashMap<i64, DateTime<Utc>>>>,
}

/// The stored heartbeat is never newer than the in-memory heartbeat, so a node is stale when the
/// stored time is old and the node has no recent in-memory heartbeat. Binds the cutoff, then a
/// JSON array of the IDs with a recent in-memory heartbeat.
const STALE_CONDITION: &str = "(is_active IS NULL OR is_active = 1) AND last_heartbeat IS NOT NULL AND \
     julianday(last_heartbeat) < julianday(?) AND id NOT IN (SELECT value FROM json_each(?))";

const COMPUTE_NODE_COLUMNS: &[&str] = &[
    "id",
    "workflow_id",
    "hostname",
    "pid",
    "start_time",
    "duration_seconds",
    "is_active",
    "num_cpus",
    "memory_gb",
    "num_gpus",
    "num_nodes",
    "time_limit",
    "scheduler_config_id",
    "compute_node_type",
    "labels",
    "end_time",
    "last_heartbeat",
    "sample_count",
    "peak_cpu_percent",
    "avg_cpu_percent",
    "peak_memory_bytes",
    "avg_memory_bytes",
];

pub(crate) fn update_response_from_get(
    response: GetComputeNodeResponse,
) -> Result<UpdateComputeNodeResponse, ApiError> {
    match response {
        GetComputeNodeResponse::SuccessfulResponse(node) => {
            Ok(UpdateComputeNodeResponse::SuccessfulResponse(node))
        }
        GetComputeNodeResponse::ForbiddenErrorResponse(err) => {
            Ok(UpdateComputeNodeResponse::ForbiddenErrorResponse(err))
        }
        GetComputeNodeResponse::NotFoundErrorResponse(err) => {
            Ok(UpdateComputeNodeResponse::NotFoundErrorResponse(err))
        }
        GetComputeNodeResponse::DefaultErrorResponse(_) => {
            Err(ApiError("Failed to get compute node".to_string()))
        }
    }
}

impl ComputeNodesApiImpl {
    /// `stale_after_secs` of 0 turns stale marking off.
    pub fn new(context: ApiContext, stale_after_secs: u64) -> Self {
        Self {
            context,
            stale_after: (stale_after_secs > 0)
                .then(|| std::time::Duration::from_secs(stale_after_secs)),
            started: std::time::Instant::now(),
            heartbeats: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn heartbeats(&self) -> std::sync::MutexGuard<'_, HashMap<i64, DateTime<Utc>>> {
        self.heartbeats
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// JSON array of the node IDs whose in-memory heartbeat is at or after the cutoff.
    fn fresh_node_ids(&self, cutoff: DateTime<Utc>) -> String {
        let ids: Vec<i64> = self
            .heartbeats()
            .iter()
            .filter(|(_, seen)| **seen >= cutoff)
            .map(|(id, _)| *id)
            .collect();
        serde_json::to_string(&ids).unwrap_or_else(|_| "[]".to_string())
    }

    /// Write the in-memory heartbeat of each stale node whose stored heartbeat is older.
    async fn persist_stale_heartbeats(
        &self,
        records: &[sqlx::sqlite::SqliteRow],
        nodes: &[models::ComputeNodeModel],
    ) {
        for (record, node) in records.iter().zip(nodes) {
            let stored: Option<String> = record.get("last_heartbeat");
            if node.is_stale != Some(true) || node.last_heartbeat == stored {
                continue;
            }
            if let Err(e) = sqlx::query("UPDATE compute_node SET last_heartbeat = $1 WHERE id = $2")
                .bind(&node.last_heartbeat)
                .bind(node.id)
                .execute(self.context.pool.as_ref())
                .await
            {
                warn!(
                    "Failed to store the heartbeat of stale compute_node_id={:?}: {}",
                    node.id, e
                );
            }
        }
    }

    /// Heartbeats older than the cutoff are stale. There is no cutoff before the server has run
    /// for one full threshold: while it was down, no runner could send a heartbeat.
    fn stale_cutoff(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        let stale_after = self.stale_after?;
        if self.started.elapsed() < stale_after {
            return None;
        }
        Some(chrono::Utc::now() - chrono::Duration::from_std(stale_after).ok()?)
    }

    fn is_stale(
        cutoff: Option<chrono::DateTime<chrono::Utc>>,
        is_active: bool,
        last_heartbeat: Option<&str>,
    ) -> bool {
        let (Some(cutoff), Some(last_heartbeat)) = (cutoff, last_heartbeat) else {
            return false;
        };
        chrono::DateTime::parse_from_rfc3339(last_heartbeat)
            .is_ok_and(|seen| is_active && seen < cutoff)
    }

    fn row_to_model(
        &self,
        record: &sqlx::sqlite::SqliteRow,
        cutoff: Option<chrono::DateTime<chrono::Utc>>,
    ) -> models::ComputeNodeModel {
        let is_active_val: Option<i64> = record.get("is_active");
        let scheduler_str: Option<String> = record.get("scheduler");
        let scheduler = scheduler_str.and_then(|s| serde_json::from_str(&s).ok());
        let id: i64 = record.get("id");
        let last_heartbeat = self
            .heartbeats()
            .get(&id)
            .map(|seen| seen.to_rfc3339())
            .or_else(|| record.get("last_heartbeat"));
        let is_stale = Self::is_stale(cutoff, is_active_val != Some(0), last_heartbeat.as_deref());

        models::ComputeNodeModel {
            id: Some(id),
            workflow_id: record.get("workflow_id"),
            hostname: record.get("hostname"),
            pid: record.get("pid"),
            start_time: record.get("start_time"),
            end_time: record.get("end_time"),
            last_heartbeat,
            is_stale: Some(is_stale),
            duration_seconds: record.get("duration_seconds"),
            is_active: is_active_val.map(|v| v == 1),
            num_cpus: record.get("num_cpus"),
            memory_gb: record.get("memory_gb"),
            num_gpus: record.get("num_gpus"),
            num_nodes: record.get("num_nodes"),
            time_limit: record.get("time_limit"),
            scheduler_config_id: record.get("scheduler_config_id"),
            compute_node_type: record.get("compute_node_type"),
            labels: labels_column(record, "labels"),
            scheduler,
            sample_count: record.get("sample_count"),
            peak_cpu_percent: record.get("peak_cpu_percent"),
            avg_cpu_percent: record.get("avg_cpu_percent"),
            peak_memory_bytes: record.get("peak_memory_bytes"),
            avg_memory_bytes: record.get("avg_memory_bytes"),
        }
    }
}

#[async_trait]
impl<C> ComputeNodesApi<C> for ComputeNodesApiImpl
where
    C: Has<XSpanIdString> + Send + Sync,
{
    /// Store a compute node.
    async fn create_compute_node(
        &self,
        mut body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<CreateComputeNodeResponse, ApiError> {
        debug!(
            "create_compute_node - X-Span-ID: {:?}",
            context.get().0.clone()
        );

        // Serialize scheduler to JSON string if present
        let scheduler_json = body
            .scheduler
            .as_ref()
            .and_then(|s| serde_json::to_string(s).ok());

        match sqlx::query(
            r#"INSERT INTO compute_node (
                workflow_id
                ,hostname
                ,pid
                ,start_time
                ,duration_seconds
                ,is_active
                ,num_cpus
                ,memory_gb
                ,num_gpus
                ,num_nodes
                ,time_limit
                ,scheduler_config_id
                ,compute_node_type
                ,scheduler
                ,sample_count
                ,peak_cpu_percent
                ,avg_cpu_percent
                ,peak_memory_bytes
                ,avg_memory_bytes
                ,end_time
                ,labels
                ,last_heartbeat
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22)
            RETURNING id
            "#,
        )
        .bind(body.workflow_id)
        .bind(&body.hostname)
        .bind(body.pid)
        .bind(&body.start_time)
        .bind(body.duration_seconds)
        .bind(body.is_active)
        .bind(body.num_cpus)
        .bind(body.memory_gb)
        .bind(body.num_gpus)
        .bind(body.num_nodes)
        .bind(&body.time_limit)
        .bind(body.scheduler_config_id)
        .bind(&body.compute_node_type)
        .bind(scheduler_json)
        .bind(body.sample_count)
        .bind(body.peak_cpu_percent)
        .bind(body.avg_cpu_percent)
        .bind(body.peak_memory_bytes)
        .bind(body.avg_memory_bytes)
        .bind(&body.end_time)
        .bind(models::labels_to_json(body.labels.as_deref()))
        .bind(&body.last_heartbeat)
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(row) => {
                body.id = Some(row.get("id"));
                Ok(CreateComputeNodeResponse::SuccessfulResponse(body))
            }
            Err(e) => {
                debug!("Database error inserting compute node: {}", e);
                return Err(database_error_with_msg(e, "Failed to create compute node"));
            }
        }
    }

    /// Delete all compute nodes for one workflow.
    async fn delete_compute_nodes(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteComputeNodesResponse, ApiError> {
        debug!(
            "delete_compute_nodes({}) - X-Span-ID: {:?}",
            workflow_id,
            context.get().0.clone()
        );

        let result = match sqlx::query!(
            "DELETE FROM compute_node WHERE workflow_id = $1",
            workflow_id
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to delete compute nodes"));
            }
        };

        let deleted_count = result.rows_affected() as i64;

        info!(
            "Deleted {} compute nodes for workflow {}",
            deleted_count, workflow_id
        );

        Ok(DeleteComputeNodesResponse::SuccessfulResponse(
            serde_json::json!({
                "count": deleted_count
            }),
        ))
    }

    /// Retrieve a compute node by ID.
    async fn get_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetComputeNodeResponse, ApiError> {
        debug!(
            "get_compute_node({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        let record = match sqlx::query(
            r#"
            SELECT id, workflow_id, hostname, pid, start_time, duration_seconds, is_active,
                   num_cpus, memory_gb, num_gpus, num_nodes, time_limit, scheduler_config_id,
                   compute_node_type, scheduler, sample_count,
                   peak_cpu_percent, avg_cpu_percent,
                   peak_memory_bytes, avg_memory_bytes, end_time, labels, last_heartbeat
            FROM compute_node
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(record)) => record,
            Ok(None) => {
                return Ok(GetComputeNodeResponse::NotFoundErrorResponse(
                    resource_not_found_response("Compute node", id),
                ));
            }
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to fetch compute node"));
            }
        };

        let compute_node_model = self.row_to_model(&record, self.stale_cutoff());
        self.persist_stale_heartbeats(
            std::slice::from_ref(&record),
            std::slice::from_ref(&compute_node_model),
        )
        .await;

        Ok(GetComputeNodeResponse::SuccessfulResponse(
            compute_node_model,
        ))
    }

    /// Retrieve all compute nodes for one workflow.
    async fn list_compute_nodes(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        hostname: Option<String>,
        is_active: Option<bool>,
        scheduled_compute_node_id: Option<i64>,
        stale: Option<bool>,
        context: &C,
    ) -> Result<ListComputeNodesResponse, ApiError> {
        debug!(
            "list_compute_nodes({}, {}, {}, {:?}, {:?}, {:?}, {:?}, {:?}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            hostname,
            is_active,
            scheduled_compute_node_id,
            stale,
            context.get().0.clone()
        );

        let cutoff = self.stale_cutoff();

        // Build base query
        let base_query = "
            SELECT
                id
                ,workflow_id
                ,hostname
                ,pid
                ,start_time
                ,duration_seconds
                ,is_active
                ,num_cpus
                ,memory_gb
                ,num_gpus
                ,num_nodes
                ,time_limit
                ,scheduler_config_id
                ,compute_node_type
                ,scheduler
                ,sample_count
                ,peak_cpu_percent
                ,avg_cpu_percent
                ,peak_memory_bytes
                ,avg_memory_bytes
                ,end_time
                ,labels
                ,last_heartbeat
            FROM compute_node"
            .to_string();

        // Build WHERE clause conditions
        let mut where_conditions = vec!["workflow_id = ?".to_string()];
        if hostname.is_some() {
            // Substring match so callers can filter by a hostname fragment.
            where_conditions.push("hostname LIKE ? ESCAPE '\\'".to_string());
        }
        if is_active.is_some() {
            where_conditions.push("is_active = ?".to_string());
        }
        if scheduled_compute_node_id.is_some() {
            // Filter by scheduler.scheduler_id in the JSON field
            where_conditions.push("json_extract(scheduler, '$.scheduler_id') = ?".to_string());
        }
        let stale_cutoff_bind = match (stale, cutoff) {
            (Some(true), Some(cutoff)) => {
                where_conditions.push(format!("({STALE_CONDITION})"));
                Some((cutoff.to_rfc3339(), self.fresh_node_ids(cutoff)))
            }
            (Some(true), None) => {
                where_conditions.push("0 = 1".to_string());
                None
            }
            (Some(false), Some(cutoff)) => {
                where_conditions.push(format!("NOT COALESCE(({STALE_CONDITION}), 0)"));
                Some((cutoff.to_rfc3339(), self.fresh_node_ids(cutoff)))
            }
            (Some(false), None) | (None, _) => None,
        };
        let where_clause = where_conditions.join(" AND ");
        let sort_by = if let Some(ref col) = sort_by {
            if COMPUTE_NODE_COLUMNS.contains(&col.as_str()) {
                Some(col.clone())
            } else {
                debug!("Invalid sort column requested: {}", col);
                None // Fall back to default
            }
        } else {
            None
        };

        let query = SqlQueryBuilder::new(base_query)
            .with_where(where_clause.clone())
            .with_pagination_and_sorting(
                offset,
                limit,
                sort_by,
                reverse_sort,
                "id",
                COMPUTE_NODE_COLUMNS,
            )
            .build();

        debug!("Executing query: {}", query);

        // Execute the query
        let mut sqlx_query = sqlx::query(sqlx::AssertSqlSafe(&*query)).bind(workflow_id);
        if let Some(ref h) = hostname {
            sqlx_query = sqlx_query.bind(format!("%{}%", super::escape_like_pattern(h)));
        }
        if let Some(active) = is_active {
            // Convert boolean to integer for SQLite
            sqlx_query = sqlx_query.bind(if active { 1i64 } else { 0i64 });
        }
        if let Some(scn_id) = scheduled_compute_node_id {
            sqlx_query = sqlx_query.bind(scn_id);
        }
        if let Some((ref cutoff, ref fresh_ids)) = stale_cutoff_bind {
            sqlx_query = sqlx_query.bind(cutoff.clone()).bind(fresh_ids.clone());
        }

        let records = match sqlx_query.fetch_all(self.context.pool.as_ref()).await {
            Ok(recs) => recs,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to list compute nodes"));
            }
        };

        let items: Vec<models::ComputeNodeModel> = records
            .iter()
            .map(|record| self.row_to_model(record, cutoff))
            .collect();
        self.persist_stale_heartbeats(&records, &items).await;

        // For proper pagination, we should get the total count without LIMIT/OFFSET
        let count_query =
            SqlQueryBuilder::new("SELECT COUNT(*) as total FROM compute_node".to_string())
                .with_where(where_clause)
                .build();

        // Bind in the same order the WHERE clause was built (workflow_id,
        // hostname, is_active, scheduled_compute_node_id). The previous version
        // omitted hostname/is_active, which mis-bound the count query whenever
        // those filters were active.
        let mut count_sqlx_query =
            sqlx::query(sqlx::AssertSqlSafe(&*count_query)).bind(workflow_id);
        if let Some(ref h) = hostname {
            count_sqlx_query =
                count_sqlx_query.bind(format!("%{}%", super::escape_like_pattern(h)));
        }
        if let Some(active) = is_active {
            count_sqlx_query = count_sqlx_query.bind(if active { 1i64 } else { 0i64 });
        }
        if let Some(scn_id) = scheduled_compute_node_id {
            count_sqlx_query = count_sqlx_query.bind(scn_id);
        }
        if let Some((ref cutoff, ref fresh_ids)) = stale_cutoff_bind {
            count_sqlx_query = count_sqlx_query
                .bind(cutoff.clone())
                .bind(fresh_ids.clone());
        }

        let total_count = match count_sqlx_query.fetch_one(self.context.pool.as_ref()).await {
            Ok(row) => row.get::<i64, _>("total"),
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to list compute nodes"));
            }
        };

        let response = crate::paginated_list_response!(
            models::ListComputeNodesResponse,
            items,
            offset,
            total_count
        );

        debug!(
            "list_compute_nodes({}, {}/{}) - X-Span-ID: {:?}",
            workflow_id,
            response.count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListComputeNodesResponse::SuccessfulResponse(response))
    }

    /// Update a compute node.
    async fn update_compute_node(
        &self,
        id: i64,
        body: models::ComputeNodeModel,
        context: &C,
    ) -> Result<UpdateComputeNodeResponse, ApiError> {
        debug!(
            "update_compute_node({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        // First get the existing compute node to ensure it exists
        match self.get_compute_node(id, context).await? {
            GetComputeNodeResponse::SuccessfulResponse(compute_node) => compute_node,
            GetComputeNodeResponse::ForbiddenErrorResponse(err) => {
                return Ok(UpdateComputeNodeResponse::ForbiddenErrorResponse(err));
            }
            GetComputeNodeResponse::NotFoundErrorResponse(err) => {
                return Ok(UpdateComputeNodeResponse::NotFoundErrorResponse(err));
            }
            GetComputeNodeResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get compute node".to_string()));
            }
        };

        // Convert boolean to integer for database storage
        let is_active_val = body.is_active.map(|b| if b { 1i64 } else { 0i64 });

        // Serialize scheduler to JSON string if present
        let scheduler_json = body
            .scheduler
            .as_ref()
            .and_then(|s| serde_json::to_string(s).ok());

        let result = match sqlx::query(
            r#"
            UPDATE compute_node
            SET
                hostname = COALESCE($1, hostname)
                ,pid = COALESCE($2, pid)
                ,start_time = COALESCE($3, start_time)
                ,duration_seconds = COALESCE($4, duration_seconds)
                ,is_active = COALESCE($5, is_active)
                ,num_cpus = COALESCE($6, num_cpus)
                ,memory_gb = COALESCE($7, memory_gb)
                ,num_gpus = COALESCE($8, num_gpus)
                ,num_nodes = COALESCE($9, num_nodes)
                ,time_limit = COALESCE($10, time_limit)
                ,scheduler_config_id = COALESCE($11, scheduler_config_id)
                ,compute_node_type = COALESCE($12, compute_node_type)
                ,scheduler = COALESCE($13, scheduler)
                ,sample_count = COALESCE($14, sample_count)
                ,peak_cpu_percent = COALESCE($15, peak_cpu_percent)
                ,avg_cpu_percent = COALESCE($16, avg_cpu_percent)
                ,peak_memory_bytes = COALESCE($17, peak_memory_bytes)
                ,avg_memory_bytes = COALESCE($18, avg_memory_bytes)
                ,end_time = COALESCE($19, end_time)
            WHERE id = $20 AND workflow_id = $21
            "#,
        )
        .bind(body.hostname)
        .bind(body.pid)
        .bind(body.start_time)
        .bind(body.duration_seconds)
        .bind(is_active_val)
        .bind(body.num_cpus)
        .bind(body.memory_gb)
        .bind(body.num_gpus)
        .bind(body.num_nodes)
        .bind(body.time_limit)
        .bind(body.scheduler_config_id)
        .bind(body.compute_node_type)
        .bind(scheduler_json)
        .bind(body.sample_count)
        .bind(body.peak_cpu_percent)
        .bind(body.avg_cpu_percent)
        .bind(body.peak_memory_bytes)
        .bind(body.avg_memory_bytes)
        .bind(body.end_time)
        .bind(id)
        .bind(body.workflow_id)
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to update compute node"));
            }
        };

        if result.rows_affected() == 0 {
            let pool = self.context.pool.as_ref();
            return Ok(
                if super::workflow_id_mismatch(pool, "compute_node", id).await? {
                    UpdateComputeNodeResponse::UnprocessableContentErrorResponse(
                        super::workflow_id_immutable_response(),
                    )
                } else {
                    UpdateComputeNodeResponse::NotFoundErrorResponse(resource_not_found_response(
                        "Compute node",
                        id,
                    ))
                },
            );
        }

        // Return the updated compute node by fetching it again
        let updated_compute_node = match self.get_compute_node(id, context).await? {
            GetComputeNodeResponse::SuccessfulResponse(compute_node) => compute_node,
            _ => return Err(ApiError("Failed to get updated compute node".to_string())),
        };

        debug!("Modified compute node with id: {}", id);
        Ok(UpdateComputeNodeResponse::SuccessfulResponse(
            updated_compute_node,
        ))
    }

    /// Record a liveness signal for an active compute node.
    ///
    /// The server stamps its own clock, so runner clock skew does not matter. The heartbeat goes
    /// to memory. Only the first heartbeat of a node in this process is written to the database,
    /// so that the node can become stale after a server restart. Only an active node is
    /// touched: a node that was failed stays inactive, and the returned model shows that to
    /// the caller.
    async fn heartbeat_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<UpdateComputeNodeResponse, ApiError> {
        debug!(
            "heartbeat_compute_node({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        let mut node = match self.get_compute_node(id, context).await? {
            GetComputeNodeResponse::SuccessfulResponse(node) => node,
            other => return update_response_from_get(other),
        };
        if node.is_active == Some(false) {
            return Ok(UpdateComputeNodeResponse::SuccessfulResponse(node));
        }

        let now = Utc::now();
        let first = self.heartbeats().insert(id, now).is_none();
        if first
            && let Err(e) = sqlx::query("UPDATE compute_node SET last_heartbeat = $1 WHERE id = $2")
                .bind(now.to_rfc3339())
                .bind(id)
                .execute(self.context.pool.as_ref())
                .await
        {
            self.heartbeats().remove(&id);
            return Err(database_error_with_msg(
                e,
                "Failed to record compute node heartbeat",
            ));
        }

        node.last_heartbeat = Some(now.to_rfc3339());
        node.is_stale = Some(false);
        Ok(UpdateComputeNodeResponse::SuccessfulResponse(node))
    }

    /// Delete a compute node.
    async fn delete_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteComputeNodeResponse, ApiError> {
        debug!(
            "delete_compute_node({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        // First get the compute node to ensure it exists and extract the ComputeNodeModel
        let compute_node = match self.get_compute_node(id, context).await? {
            GetComputeNodeResponse::SuccessfulResponse(compute_node) => compute_node,
            GetComputeNodeResponse::ForbiddenErrorResponse(err) => {
                return Ok(DeleteComputeNodeResponse::ForbiddenErrorResponse(err));
            }
            GetComputeNodeResponse::NotFoundErrorResponse(err) => {
                return Ok(DeleteComputeNodeResponse::NotFoundErrorResponse(err));
            }
            GetComputeNodeResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get compute node".to_string()));
            }
        };

        match sqlx::query("DELETE FROM compute_node WHERE id = $1")
            .bind(id)
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
                    info!("Deleted compute node with id: {}", id);
                    Ok(DeleteComputeNodeResponse::SuccessfulResponse(compute_node))
                }
            }
            Err(e) => {
                return Err(database_error_with_msg(e, "Failed to delete compute node"));
            }
        }
    }
}
