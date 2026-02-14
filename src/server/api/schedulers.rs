//! Scheduler-related API endpoints

#![allow(clippy::too_many_arguments)]

use async_trait::async_trait;
use log::{debug, error, info};
use sqlx::Row;
use swagger::{ApiError, Has, XSpanIdString};

use crate::server::api_types::{
    CreateLocalSchedulerResponse, CreateScheduledComputeNodeResponse, CreateSlurmSchedulerResponse,
    DeleteLocalSchedulerResponse, DeleteLocalSchedulersResponse,
    DeleteScheduledComputeNodeResponse, DeleteScheduledComputeNodesResponse,
    DeleteSlurmSchedulerResponse, DeleteSlurmSchedulersResponse, GetLocalSchedulerResponse,
    GetScheduledComputeNodeResponse, GetSlurmSchedulerResponse, ListLocalSchedulersResponse,
    ListScheduledComputeNodesResponse, ListSlurmSchedulersResponse, UpdateLocalSchedulerResponse,
    UpdateScheduledComputeNodeResponse, UpdateSlurmSchedulerResponse,
};

use crate::models;

use super::{ApiContext, MAX_RECORD_TRANSFER_COUNT, SqlQueryBuilder, database_error_with_msg};

/// Trait defining scheduler-related API operations
#[async_trait]
pub trait SchedulersApi<C> {
    /// Store a local scheduler.
    async fn create_local_scheduler(
        &self,
        mut body: models::LocalSchedulerModel,
        context: &C,
    ) -> Result<CreateLocalSchedulerResponse, ApiError>;

    /// Store a scheduled compute node.
    async fn create_scheduled_compute_node(
        &self,
        mut body: models::ScheduledComputeNodesModel,
        context: &C,
    ) -> Result<CreateScheduledComputeNodeResponse, ApiError>;

    /// Store a Slurm scheduler.
    async fn create_slurm_scheduler(
        &self,
        mut body: models::SlurmSchedulerModel,
        context: &C,
    ) -> Result<CreateSlurmSchedulerResponse, ApiError>;

    /// Delete all local schedulers for one workflow.
    async fn delete_local_schedulers(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteLocalSchedulersResponse, ApiError>;

    /// Delete all scheduled compute nodes for one workflow.
    async fn delete_scheduled_compute_nodes(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodesResponse, ApiError>;

    /// Delete all Slurm schedulers for one workflow.
    async fn delete_slurm_schedulers(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteSlurmSchedulersResponse, ApiError>;

    /// Retrieve a local scheduler by ID.
    async fn get_local_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetLocalSchedulerResponse, ApiError>;

    /// Retrieve a scheduled compute node by ID.
    async fn get_scheduled_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetScheduledComputeNodeResponse, ApiError>;

    /// Retrieve a Slurm scheduler by ID.
    async fn get_slurm_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetSlurmSchedulerResponse, ApiError>;

    /// Retrieve local schedulers for one workflow.
    async fn list_local_schedulers(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        memory: Option<String>,
        num_cpus: Option<i64>,
        context: &C,
    ) -> Result<ListLocalSchedulersResponse, ApiError>;

    /// Retrieve scheduled compute node records for one workflow.
    async fn list_scheduled_compute_nodes(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        scheduler_id: Option<String>,
        scheduler_config_id: Option<String>,
        status: Option<String>,
        context: &C,
    ) -> Result<ListScheduledComputeNodesResponse, ApiError>;

    /// Retrieve all Slurm schedulers for one workflow.
    /// Retrieve all Slurm compute node configurations for one workflow.
    async fn list_slurm_schedulers(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListSlurmSchedulersResponse, ApiError>;

    /// Update a local scheduler.
    async fn update_local_scheduler(
        &self,
        id: i64,
        body: models::LocalSchedulerModel,
        context: &C,
    ) -> Result<UpdateLocalSchedulerResponse, ApiError>;

    /// Update a scheduled compute node.
    async fn update_scheduled_compute_node(
        &self,
        id: i64,
        body: models::ScheduledComputeNodesModel,
        context: &C,
    ) -> Result<UpdateScheduledComputeNodeResponse, ApiError>;

    /// Update a Slurm scheduler.
    async fn update_slurm_scheduler(
        &self,
        id: i64,
        body: models::SlurmSchedulerModel,
        context: &C,
    ) -> Result<UpdateSlurmSchedulerResponse, ApiError>;

    /// Delete a local scheduler.
    async fn delete_local_scheduler(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteLocalSchedulerResponse, ApiError>;

    /// Delete a scheduled compute node.
    async fn delete_scheduled_compute_node(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodeResponse, ApiError>;

    /// Delete a Slurm scheduler.
    async fn delete_slurm_scheduler(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteSlurmSchedulerResponse, ApiError>;
}

/// Implementation of schedulers API for the server
#[derive(Clone)]
pub struct SchedulersApiImpl {
    pub context: ApiContext,
}

const LOCAL_SCHEDULER_COLUMNS: &[&str] = &["id", "workflow_id", "memory", "num_cpus"];

const SCHEDULED_COMPUTE_NODE_COLUMNS: &[&str] = &[
    "id",
    "workflow_id",
    "scheduler_id",
    "scheduler_config_id",
    "scheduler_type",
    "status",
];

const SLURM_SCHEDULER_COLUMNS: &[&str] = &[
    "id",
    "workflow_id",
    "name",
    "account",
    "gres",
    "mem",
    "nodes",
    "ntasks_per_node",
    "partition",
    "qos",
    "tmp",
    "walltime",
    "extra",
];

impl SchedulersApiImpl {
    pub fn new(context: ApiContext) -> Self {
        Self { context }
    }
}

#[async_trait]
impl<C> SchedulersApi<C> for SchedulersApiImpl
where
    C: Has<XSpanIdString> + Send + Sync,
{
    /// Store a local scheduler.
    async fn create_local_scheduler(
        &self,
        mut body: models::LocalSchedulerModel,
        context: &C,
    ) -> Result<CreateLocalSchedulerResponse, ApiError> {
        debug!(
            "create_local_scheduler({:?}) - X-Span-ID: {:?}",
            body,
            context.get().0.clone()
        );

        let result = match sqlx::query!(
            r#"
            INSERT INTO local_scheduler
            (
                workflow_id
                ,memory
                ,num_cpus
            )
            VALUES ($1, $2, $3)
            RETURNING rowid
        "#,
            body.workflow_id,
            body.memory,
            body.num_cpus,
        )
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to create scheduler record",
                ));
            }
        };
        body.id = Some(result.id);
        Ok(CreateLocalSchedulerResponse::SuccessfulResponse(body))
    }

    /// Store a scheduled compute node.
    async fn create_scheduled_compute_node(
        &self,
        mut body: models::ScheduledComputeNodesModel,
        context: &C,
    ) -> Result<CreateScheduledComputeNodeResponse, ApiError> {
        debug!(
            "create_scheduled_compute_node({:?}) - X-Span-ID: {:?}",
            body,
            context.get().0.clone()
        );

        let result = match sqlx::query!(
            r#"
            INSERT INTO scheduled_compute_node
            (
                workflow_id
                ,scheduler_id
                ,scheduler_config_id
                ,scheduler_type
                ,status
            )
            VALUES ($1, $2, $3, $4, $5)
            RETURNING rowid
        "#,
            body.workflow_id,
            body.scheduler_id,
            body.scheduler_config_id,
            body.scheduler_type,
            body.status,
        )
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to create scheduler record",
                ));
            }
        };
        body.id = Some(result.id);
        Ok(CreateScheduledComputeNodeResponse::SuccessfulResponse(body))
    }

    /// Store a Slurm scheduler.
    async fn create_slurm_scheduler(
        &self,
        mut body: models::SlurmSchedulerModel,
        context: &C,
    ) -> Result<CreateSlurmSchedulerResponse, ApiError> {
        debug!(
            "create_slurm_scheduler({:?}) - X-Span-ID: {:?}",
            body,
            context.get().0.clone()
        );

        let result = match sqlx::query!(
            r#"
            INSERT INTO slurm_scheduler
            (
                workflow_id
                ,name
                ,account
                ,gres
                ,mem
                ,nodes
                ,ntasks_per_node
                ,partition
                ,qos
                ,tmp
                ,walltime
                ,extra
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING rowid
        "#,
            body.workflow_id,
            body.name,
            body.account,
            body.gres,
            body.mem,
            body.nodes,
            body.ntasks_per_node,
            body.partition,
            body.qos,
            body.tmp,
            body.walltime,
            body.extra,
        )
        .fetch_one(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("Database error: {}", e);
                return Err(ApiError("Database error".to_string()));
            }
        };
        body.id = Some(result.id);
        Ok(CreateSlurmSchedulerResponse::SuccessfulResponse(body))
    }

    /// Delete all local schedulers for one workflow.
    async fn delete_local_schedulers(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteLocalSchedulersResponse, ApiError> {
        debug!(
            "delete_local_schedulers({}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            body,
            context.get().0.clone()
        );

        let result = match sqlx::query!(
            "DELETE FROM local_scheduler WHERE workflow_id = $1",
            workflow_id
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to delete local schedulers",
                ));
            }
        };

        let deleted_count = result.rows_affected() as i64;

        info!(
            "Deleted {} local schedulers for workflow {}",
            deleted_count, workflow_id
        );

        Ok(DeleteLocalSchedulersResponse::SuccessfulResponse(
            serde_json::json!({
                "count": deleted_count
            }),
        ))
    }

    /// Delete all scheduled compute nodes for one workflow.
    async fn delete_scheduled_compute_nodes(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodesResponse, ApiError> {
        debug!(
            "delete_scheduled_compute_nodes({}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            body,
            context.get().0.clone()
        );

        let result = match sqlx::query!(
            "DELETE FROM scheduled_compute_node WHERE workflow_id = $1",
            workflow_id
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to delete scheduled compute nodes",
                ));
            }
        };

        let deleted_count = result.rows_affected() as i64;

        info!(
            "Deleted {} scheduled compute nodes for workflow {}",
            deleted_count, workflow_id
        );

        Ok(DeleteScheduledComputeNodesResponse::SuccessfulResponse(
            serde_json::json!({
                "count": deleted_count
            }),
        ))
    }

    /// Delete all Slurm schedulers for one workflow.
    async fn delete_slurm_schedulers(
        &self,
        workflow_id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteSlurmSchedulersResponse, ApiError> {
        debug!(
            "delete_slurm_schedulers({}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            body,
            context.get().0.clone()
        );

        let result = match sqlx::query!(
            "DELETE FROM slurm_scheduler WHERE workflow_id = $1",
            workflow_id
        )
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(
                    e,
                    "Failed to delete slurm schedulers",
                ));
            }
        };

        let deleted_count = result.rows_affected() as i64;

        info!(
            "Deleted {} slurm schedulers for workflow {}",
            deleted_count, workflow_id
        );

        Ok(DeleteSlurmSchedulersResponse::Message(serde_json::json!({
            "count": deleted_count
        })))
    }

    /// Retrieve a local scheduler by ID.
    async fn get_local_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetLocalSchedulerResponse, ApiError> {
        debug!(
            "get_local_scheduler({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        let record = match sqlx::query(
            r#"
            SELECT id, workflow_id, memory, num_cpus
            FROM local_scheduler
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(record)) => record,
            Ok(None) => {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Local scheduler not found with ID: {}", id)
                }));
                return Ok(GetLocalSchedulerResponse::NotFoundErrorResponse(
                    error_response,
                ));
            }
            Err(e) => {
                return Err(database_error_with_msg(e, "Database operation failed"));
            }
        };

        let local_scheduler_model = models::LocalSchedulerModel {
            id: Some(record.get("id")),
            workflow_id: record.get("workflow_id"),
            name: None,
            memory: record.get("memory"),
            num_cpus: record.get("num_cpus"),
        };

        Ok(GetLocalSchedulerResponse::SuccessfulResponse(
            local_scheduler_model,
        ))
    }

    /// Retrieve a scheduled compute node by ID.
    async fn get_scheduled_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetScheduledComputeNodeResponse, ApiError> {
        debug!(
            "get_scheduled_compute_node({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        let record = match sqlx::query(
            r#"
            SELECT id, workflow_id, scheduler_id, scheduler_config_id, scheduler_type, status
            FROM scheduled_compute_node
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(record)) => record,
            Ok(None) => {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Scheduled compute node not found with ID: {}", id)
                }));
                return Ok(GetScheduledComputeNodeResponse::NotFoundErrorResponse(
                    error_response,
                ));
            }
            Err(e) => {
                return Err(database_error_with_msg(e, "Database operation failed"));
            }
        };

        let scheduled_compute_node_model = models::ScheduledComputeNodesModel {
            id: Some(record.get("id")),
            workflow_id: record.get("workflow_id"),
            scheduler_id: record.get("scheduler_id"),
            scheduler_config_id: record.get("scheduler_config_id"),
            scheduler_type: record.get("scheduler_type"),
            status: record.get("status"),
        };

        Ok(GetScheduledComputeNodeResponse::HTTP(
            scheduled_compute_node_model,
        ))
    }

    /// Retrieve a Slurm scheduler by ID.
    async fn get_slurm_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetSlurmSchedulerResponse, ApiError> {
        debug!(
            "get_slurm_scheduler({}) - X-Span-ID: {:?}",
            id,
            context.get().0.clone()
        );

        let record = match sqlx::query(
            r#"
            SELECT id, workflow_id, name, account, gres, mem, nodes, ntasks_per_node, 
                   partition, qos, tmp, walltime, extra
            FROM slurm_scheduler
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(self.context.pool.as_ref())
        .await
        {
            Ok(Some(record)) => record,
            Ok(None) => {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Slurm scheduler not found with ID: {}", id)
                }));
                return Ok(GetSlurmSchedulerResponse::NotFoundErrorResponse(
                    error_response,
                ));
            }
            Err(e) => {
                return Err(database_error_with_msg(e, "Database operation failed"));
            }
        };

        let slurm_scheduler_model = models::SlurmSchedulerModel {
            id: Some(record.get("id")),
            workflow_id: record.get("workflow_id"),
            name: record.get("name"),
            account: record.get("account"),
            gres: record.get("gres"),
            mem: record.get("mem"),
            nodes: record.get("nodes"),
            ntasks_per_node: record.get("ntasks_per_node"),
            partition: record.get("partition"),
            qos: record.get("qos"),
            tmp: record.get("tmp"),
            walltime: record.get("walltime"),
            extra: record.get("extra"),
        };

        Ok(GetSlurmSchedulerResponse::SuccessfulResponse(
            slurm_scheduler_model,
        ))
    }

    /// Retrieve local schedulers for one workflow.
    async fn list_local_schedulers(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        memory: Option<String>,
        num_cpus: Option<i64>,
        context: &C,
    ) -> Result<ListLocalSchedulersResponse, ApiError> {
        debug!(
            "list_local_schedulers({}, {}, {}, {:?}, {:?}, {:?}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            memory,
            num_cpus,
            context.get().0.clone()
        );

        // Build base query
        let base_query =
            "SELECT id, workflow_id, memory, num_cpus FROM local_scheduler".to_string();

        // Build WHERE clause
        let where_clause = "workflow_id = ?".to_string();

        // Validate sort_by against whitelist
        let validated_sort_by = if let Some(ref col) = sort_by {
            if LOCAL_SCHEDULER_COLUMNS.contains(&col.as_str()) {
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
            .with_pagination_and_sorting(offset, limit, validated_sort_by, reverse_sort, "id")
            .build();

        debug!("Executing query: {}", query);

        // Execute the query
        let records = match sqlx::query(&query)
            .bind(workflow_id)
            .fetch_all(self.context.pool.as_ref())
            .await
        {
            Ok(recs) => recs,
            Err(e) => {
                return Err(database_error_with_msg(e, "Database operation failed"));
            }
        };

        let mut items: Vec<models::LocalSchedulerModel> = Vec::new();
        for record in records {
            items.push(models::LocalSchedulerModel {
                id: Some(record.get("id")),
                workflow_id: record.get("workflow_id"),
                name: None,
                memory: record.get("memory"),
                num_cpus: record.get("num_cpus"),
            });
        }

        // For proper pagination, we should get the total count without LIMIT/OFFSET
        let count_query =
            SqlQueryBuilder::new("SELECT COUNT(*) as total FROM local_scheduler".to_string())
                .with_where(where_clause)
                .build();

        let total_count = match sqlx::query(&count_query)
            .bind(workflow_id)
            .fetch_one(self.context.pool.as_ref())
            .await
        {
            Ok(row) => row.get::<i64, _>("total"),
            Err(e) => {
                error!("Database error getting count: {}", e);
                return Err(database_error_with_msg(e, "Database operation failed"));
            }
        };

        let current_count = items.len() as i64;
        let offset_val = offset;
        let has_more = offset_val + current_count < total_count;

        debug!(
            "list_local_schedulers({}, {}/{}) - X-Span-ID: {:?}",
            workflow_id,
            current_count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListLocalSchedulersResponse::HTTP(
            models::ListLocalSchedulersResponse {
                items: Some(items),
                offset: offset_val,
                max_limit: MAX_RECORD_TRANSFER_COUNT,
                count: current_count,
                total_count,
                has_more,
            },
        ))
    }

    /// Retrieve scheduled compute node records for one workflow.
    async fn list_scheduled_compute_nodes(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        scheduler_id: Option<String>,
        scheduler_config_id: Option<String>,
        status: Option<String>,
        context: &C,
    ) -> Result<ListScheduledComputeNodesResponse, ApiError> {
        debug!(
            "list_scheduled_compute_nodes({}, {}, {}, {:?}, {:?}, {:?}, {:?}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            scheduler_id,
            scheduler_config_id,
            status,
            context.get().0.clone()
        );

        // Build base query
        let base_query = "SELECT id, workflow_id, scheduler_id, scheduler_config_id, scheduler_type, status FROM scheduled_compute_node".to_string();

        let mut where_conditions = vec!["workflow_id = ?".to_string()];

        if scheduler_id.is_some() {
            where_conditions.push("scheduler_id = ?".to_string());
        }
        if scheduler_config_id.is_some() {
            where_conditions.push("scheduler_config_id = ?".to_string());
        }
        if status.is_some() {
            where_conditions.push("status = ?".to_string());
        }

        let where_clause = where_conditions.join(" AND ");

        // Validate sort_by against whitelist
        let validated_sort_by = if let Some(ref col) = sort_by {
            if SCHEDULED_COMPUTE_NODE_COLUMNS.contains(&col.as_str()) {
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
            .with_pagination_and_sorting(offset, limit, validated_sort_by, reverse_sort, "id")
            .build();

        debug!("Executing query: {}", query);

        // Execute the query with dynamic parameter binding
        let mut sqlx_query = sqlx::query(&query).bind(workflow_id);

        if let Some(ref sched_id) = scheduler_id {
            sqlx_query = sqlx_query.bind(sched_id.parse::<i64>().unwrap_or(0));
        }
        if let Some(ref sched_config_id) = scheduler_config_id {
            sqlx_query = sqlx_query.bind(sched_config_id.parse::<i64>().unwrap_or(0));
        }
        if let Some(ref stat) = status {
            sqlx_query = sqlx_query.bind(stat);
        }

        let records = match sqlx_query.fetch_all(self.context.pool.as_ref()).await {
            Ok(recs) => recs,
            Err(e) => {
                return Err(database_error_with_msg(e, "Database operation failed"));
            }
        };

        let mut items: Vec<models::ScheduledComputeNodesModel> = Vec::new();
        for record in records {
            items.push(models::ScheduledComputeNodesModel {
                id: Some(record.get("id")),
                workflow_id: record.get("workflow_id"),
                scheduler_id: record.get("scheduler_id"),
                scheduler_config_id: record.get("scheduler_config_id"),
                scheduler_type: record.get("scheduler_type"),
                status: record.get("status"),
            });
        }

        // For proper pagination, we should get the total count without LIMIT/OFFSET
        let count_query = SqlQueryBuilder::new(
            "SELECT COUNT(*) as total FROM scheduled_compute_node".to_string(),
        )
        .with_where(where_clause)
        .build();

        let mut count_sqlx_query = sqlx::query(&count_query).bind(workflow_id);

        if let Some(ref sched_id) = scheduler_id {
            count_sqlx_query = count_sqlx_query.bind(sched_id.parse::<i64>().unwrap_or(0));
        }
        if let Some(ref sched_config_id) = scheduler_config_id {
            count_sqlx_query = count_sqlx_query.bind(sched_config_id.parse::<i64>().unwrap_or(0));
        }
        if let Some(ref stat) = status {
            count_sqlx_query = count_sqlx_query.bind(stat);
        }

        let total_count = match count_sqlx_query.fetch_one(self.context.pool.as_ref()).await {
            Ok(row) => row.get::<i64, _>("total"),
            Err(e) => {
                error!("Database error getting count: {}", e);
                return Err(database_error_with_msg(e, "Database operation failed"));
            }
        };

        let current_count = items.len() as i64;
        let offset_val = offset;
        let has_more = offset_val + current_count < total_count;

        debug!(
            "list_scheduled_compute_nodes({}, {}/{}) - X-Span-ID: {:?}",
            workflow_id,
            current_count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListScheduledComputeNodesResponse::SuccessfulResponse(
            models::ListScheduledComputeNodesResponse {
                items: Some(items),
                offset: offset_val,
                max_limit: MAX_RECORD_TRANSFER_COUNT,
                count: current_count,
                total_count,
                has_more,
            },
        ))
    }

    /// Retrieve all Slurm schedulers for one workflow.
    /// Retrieve all Slurm compute node configurations for one workflow.
    async fn list_slurm_schedulers(
        &self,
        workflow_id: i64,
        offset: i64,
        limit: i64,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        context: &C,
    ) -> Result<ListSlurmSchedulersResponse, ApiError> {
        debug!(
            "list_slurm_schedulers({}, {}, {}, {:?}, {:?}) - X-Span-ID: {:?}",
            workflow_id,
            offset,
            limit,
            sort_by,
            reverse_sort,
            context.get().0.clone()
        );

        // Build base query
        let base_query = "SELECT id, workflow_id, name, account, gres, mem, nodes, ntasks_per_node, partition, qos, tmp, walltime, extra FROM slurm_scheduler".to_string();

        // Build WHERE clause
        let where_clause = "workflow_id = ?".to_string();

        // Validate sort_by against whitelist
        let validated_sort_by = if let Some(ref col) = sort_by {
            if SLURM_SCHEDULER_COLUMNS.contains(&col.as_str()) {
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
            .with_pagination_and_sorting(offset, limit, validated_sort_by, reverse_sort, "id")
            .build();

        debug!("Executing query: {}", query);

        // Execute the query
        let records = match sqlx::query(&query)
            .bind(workflow_id)
            .fetch_all(self.context.pool.as_ref())
            .await
        {
            Ok(recs) => recs,
            Err(e) => {
                return Err(database_error_with_msg(e, "Database operation failed"));
            }
        };

        let mut items: Vec<models::SlurmSchedulerModel> = Vec::new();
        for record in records {
            items.push(models::SlurmSchedulerModel {
                id: Some(record.get("id")),
                workflow_id: record.get("workflow_id"),
                name: record.get("name"),
                account: record.get("account"),
                gres: record.get("gres"),
                mem: record.get("mem"),
                nodes: record.get("nodes"),
                ntasks_per_node: record.get("ntasks_per_node"),
                partition: record.get("partition"),
                qos: record.get("qos"),
                tmp: record.get("tmp"),
                walltime: record.get("walltime"),
                extra: record.get("extra"),
            });
        }

        // For proper pagination, we should get the total count without LIMIT/OFFSET
        let count_query =
            SqlQueryBuilder::new("SELECT COUNT(*) as total FROM slurm_scheduler".to_string())
                .with_where(where_clause)
                .build();

        let total_count = match sqlx::query(&count_query)
            .bind(workflow_id)
            .fetch_one(self.context.pool.as_ref())
            .await
        {
            Ok(row) => row.get::<i64, _>("total"),
            Err(e) => {
                error!("Database error getting count: {}", e);
                return Err(database_error_with_msg(e, "Database operation failed"));
            }
        };

        let current_count = items.len() as i64;
        let offset_val = offset;
        let has_more = offset_val + current_count < total_count;

        debug!(
            "list_slurm_schedulers({}, {}/{}) - X-Span-ID: {:?}",
            workflow_id,
            current_count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListSlurmSchedulersResponse::SuccessfulResponse(
            models::ListSlurmSchedulersResponse {
                items: Some(items),
                offset: offset_val,
                max_limit: MAX_RECORD_TRANSFER_COUNT,
                count: current_count,
                total_count,
                has_more,
            },
        ))
    }

    /// Update a local scheduler.
    async fn update_local_scheduler(
        &self,
        id: i64,
        body: models::LocalSchedulerModel,
        context: &C,
    ) -> Result<UpdateLocalSchedulerResponse, ApiError> {
        debug!(
            "update_local_scheduler({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First get the existing local scheduler to ensure it exists
        match self.get_local_scheduler(id, context).await? {
            GetLocalSchedulerResponse::SuccessfulResponse(local_scheduler) => local_scheduler,
            GetLocalSchedulerResponse::ForbiddenErrorResponse(err) => {
                return Ok(UpdateLocalSchedulerResponse::ForbiddenErrorResponse(err));
            }
            GetLocalSchedulerResponse::NotFoundErrorResponse(err) => {
                return Ok(UpdateLocalSchedulerResponse::NotFoundErrorResponse(err));
            }
            GetLocalSchedulerResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get local scheduler".to_string()));
            }
        };

        let result = match sqlx::query(
            r#"
            UPDATE local_scheduler
            SET
                workflow_id = COALESCE($1, workflow_id)
                ,memory = COALESCE($2, memory)
                ,num_cpus = COALESCE($3, num_cpus)
            WHERE id = $4
            "#,
        )
        .bind(body.workflow_id)
        .bind(body.memory)
        .bind(body.num_cpus)
        .bind(id)
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Database operation failed"));
            }
        };

        if result.rows_affected() == 0 {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Local scheduler not found with ID: {}", id)
            }));
            return Ok(UpdateLocalSchedulerResponse::NotFoundErrorResponse(
                error_response,
            ));
        }

        // Return the updated local scheduler by fetching it again
        let updated_local_scheduler = match self.get_local_scheduler(id, context).await? {
            GetLocalSchedulerResponse::SuccessfulResponse(local_scheduler) => local_scheduler,
            _ => {
                return Err(ApiError(
                    "Failed to get updated local scheduler".to_string(),
                ));
            }
        };

        debug!("Modified local scheduler with id: {}", id);
        Ok(UpdateLocalSchedulerResponse::SuccessfulResponse(
            updated_local_scheduler,
        ))
    }

    /// Update a scheduled compute node.
    async fn update_scheduled_compute_node(
        &self,
        id: i64,
        body: models::ScheduledComputeNodesModel,
        context: &C,
    ) -> Result<UpdateScheduledComputeNodeResponse, ApiError> {
        debug!(
            "update_scheduled_compute_node({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First get the existing scheduled compute node to ensure it exists
        match self.get_scheduled_compute_node(id, context).await? {
            GetScheduledComputeNodeResponse::HTTP(scheduled_compute_node) => scheduled_compute_node,
            GetScheduledComputeNodeResponse::ForbiddenErrorResponse(err) => {
                return Ok(UpdateScheduledComputeNodeResponse::ForbiddenErrorResponse(
                    err,
                ));
            }
            GetScheduledComputeNodeResponse::NotFoundErrorResponse(err) => {
                return Ok(UpdateScheduledComputeNodeResponse::NotFoundErrorResponse(
                    err,
                ));
            }
            GetScheduledComputeNodeResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get scheduled compute node".to_string()));
            }
        };

        let result = match sqlx::query(
            r#"
            UPDATE scheduled_compute_node
            SET
                workflow_id = COALESCE($1, workflow_id)
                ,scheduler_id = COALESCE($2, scheduler_id)
                ,scheduler_config_id = COALESCE($3, scheduler_config_id)
                ,scheduler_type = COALESCE($4, scheduler_type)
                ,status = COALESCE($5, status)
            WHERE id = $6
            "#,
        )
        .bind(body.workflow_id)
        .bind(body.scheduler_id)
        .bind(body.scheduler_config_id)
        .bind(body.scheduler_type)
        .bind(body.status)
        .bind(id)
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Database operation failed"));
            }
        };

        if result.rows_affected() == 0 {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Scheduled compute node not found with ID: {}", id)
            }));
            return Ok(UpdateScheduledComputeNodeResponse::NotFoundErrorResponse(
                error_response,
            ));
        }

        // Return the updated scheduled compute node by fetching it again
        let updated_scheduled_compute_node = match self
            .get_scheduled_compute_node(id, context)
            .await?
        {
            GetScheduledComputeNodeResponse::HTTP(scheduled_compute_node) => scheduled_compute_node,
            _ => {
                return Err(ApiError(
                    "Failed to get updated scheduled compute node".to_string(),
                ));
            }
        };

        debug!("Modified scheduled compute node with id: {}", id);
        Ok(
            UpdateScheduledComputeNodeResponse::ScheduledComputeNodeUpdatedInTheTable(
                updated_scheduled_compute_node,
            ),
        )
    }

    /// Update a Slurm scheduler.
    async fn update_slurm_scheduler(
        &self,
        id: i64,
        body: models::SlurmSchedulerModel,
        context: &C,
    ) -> Result<UpdateSlurmSchedulerResponse, ApiError> {
        debug!(
            "update_slurm_scheduler({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First get the existing Slurm scheduler to ensure it exists
        match self.get_slurm_scheduler(id, context).await? {
            GetSlurmSchedulerResponse::SuccessfulResponse(slurm_scheduler) => slurm_scheduler,
            GetSlurmSchedulerResponse::ForbiddenErrorResponse(err) => {
                return Ok(UpdateSlurmSchedulerResponse::ForbiddenErrorResponse(err));
            }
            GetSlurmSchedulerResponse::NotFoundErrorResponse(err) => {
                return Ok(UpdateSlurmSchedulerResponse::NotFoundErrorResponse(err));
            }
            GetSlurmSchedulerResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get Slurm scheduler".to_string()));
            }
        };

        let result = match sqlx::query(
            r#"
            UPDATE slurm_scheduler
            SET
                workflow_id = COALESCE($1, workflow_id)
                ,name = COALESCE($2, name)
                ,account = COALESCE($3, account)
                ,gres = COALESCE($4, gres)
                ,mem = COALESCE($5, mem)
                ,nodes = COALESCE($6, nodes)
                ,ntasks_per_node = COALESCE($7, ntasks_per_node)
                ,partition = COALESCE($8, partition)
                ,qos = COALESCE($9, qos)
                ,tmp = COALESCE($10, tmp)
                ,walltime = COALESCE($11, walltime)
                ,extra = COALESCE($12, extra)
            WHERE id = $13
            "#,
        )
        .bind(body.workflow_id)
        .bind(body.name)
        .bind(body.account)
        .bind(body.gres)
        .bind(body.mem)
        .bind(body.nodes)
        .bind(body.ntasks_per_node)
        .bind(body.partition)
        .bind(body.qos)
        .bind(body.tmp)
        .bind(body.walltime)
        .bind(body.extra)
        .bind(id)
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Database operation failed"));
            }
        };

        if result.rows_affected() == 0 {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Slurm scheduler not found with ID: {}", id)
            }));
            return Ok(UpdateSlurmSchedulerResponse::NotFoundErrorResponse(
                error_response,
            ));
        }

        // Return the updated Slurm scheduler by fetching it again
        let updated_slurm_scheduler = match self.get_slurm_scheduler(id, context).await? {
            GetSlurmSchedulerResponse::SuccessfulResponse(slurm_scheduler) => slurm_scheduler,
            _ => {
                return Err(ApiError(
                    "Failed to get updated Slurm scheduler".to_string(),
                ));
            }
        };

        info!(
            "Updated Slurm scheduler with id: {} (name: {:?})",
            id, updated_slurm_scheduler.name
        );
        Ok(UpdateSlurmSchedulerResponse::SuccessfulResponse(
            updated_slurm_scheduler,
        ))
    }

    /// Delete a local scheduler.
    async fn delete_local_scheduler(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteLocalSchedulerResponse, ApiError> {
        debug!(
            "delete_local_scheduler({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First get the local scheduler to ensure it exists and extract the LocalSchedulerModel
        let local_scheduler = match self.get_local_scheduler(id, context).await? {
            GetLocalSchedulerResponse::SuccessfulResponse(local_scheduler) => local_scheduler,
            GetLocalSchedulerResponse::ForbiddenErrorResponse(err) => {
                return Ok(DeleteLocalSchedulerResponse::ForbiddenErrorResponse(err));
            }
            GetLocalSchedulerResponse::NotFoundErrorResponse(err) => {
                return Ok(DeleteLocalSchedulerResponse::NotFoundErrorResponse(err));
            }
            GetLocalSchedulerResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get local scheduler".to_string()));
            }
        };

        match sqlx::query("DELETE FROM local_scheduler WHERE id = $1")
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
                    info!("Deleted local scheduler with id: {}", id);
                    Ok(
                        DeleteLocalSchedulerResponse::LocalComputeNodeConfigurationStoredInTheTable(
                            local_scheduler,
                        ),
                    )
                }
            }
            Err(e) => Err(database_error_with_msg(e, "Failed to delete scheduler")),
        }
    }

    /// Delete a scheduled compute node.
    async fn delete_scheduled_compute_node(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodeResponse, ApiError> {
        debug!(
            "delete_scheduled_compute_node({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First get the scheduled compute node to ensure it exists and extract the ScheduledComputeNodesModel
        let scheduled_compute_node = match self.get_scheduled_compute_node(id, context).await? {
            GetScheduledComputeNodeResponse::HTTP(scheduled_compute_node) => scheduled_compute_node,
            GetScheduledComputeNodeResponse::ForbiddenErrorResponse(err) => {
                return Ok(DeleteScheduledComputeNodeResponse::ForbiddenErrorResponse(
                    err,
                ));
            }
            GetScheduledComputeNodeResponse::NotFoundErrorResponse(err) => {
                return Ok(DeleteScheduledComputeNodeResponse::NotFoundErrorResponse(
                    err,
                ));
            }
            GetScheduledComputeNodeResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get scheduled compute node".to_string()));
            }
        };

        match sqlx::query("DELETE FROM scheduled_compute_node WHERE id = $1")
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
                    info!("Deleted scheduled compute node with id: {}", id);
                    Ok(DeleteScheduledComputeNodeResponse::SuccessfulResponse(
                        scheduled_compute_node,
                    ))
                }
            }
            Err(e) => Err(database_error_with_msg(e, "Failed to delete scheduler")),
        }
    }

    /// Delete a Slurm scheduler.
    async fn delete_slurm_scheduler(
        &self,
        id: i64,
        body: Option<serde_json::Value>,
        context: &C,
    ) -> Result<DeleteSlurmSchedulerResponse, ApiError> {
        debug!(
            "delete_slurm_scheduler({}, {:?}) - X-Span-ID: {:?}",
            id,
            body,
            context.get().0.clone()
        );

        // First get the Slurm scheduler to ensure it exists and extract the SlurmSchedulerModel
        let slurm_scheduler = match self.get_slurm_scheduler(id, context).await? {
            GetSlurmSchedulerResponse::SuccessfulResponse(slurm_scheduler) => slurm_scheduler,
            GetSlurmSchedulerResponse::ForbiddenErrorResponse(err) => {
                return Ok(DeleteSlurmSchedulerResponse::ForbiddenErrorResponse(err));
            }
            GetSlurmSchedulerResponse::NotFoundErrorResponse(err) => {
                return Ok(DeleteSlurmSchedulerResponse::NotFoundErrorResponse(err));
            }
            GetSlurmSchedulerResponse::DefaultErrorResponse(_) => {
                return Err(ApiError("Failed to get Slurm scheduler".to_string()));
            }
        };

        match sqlx::query("DELETE FROM slurm_scheduler WHERE id = $1")
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
                    info!("Deleted Slurm scheduler with id: {}", id);
                    Ok(DeleteSlurmSchedulerResponse::SuccessfulResponse(
                        slurm_scheduler,
                    ))
                }
            }
            Err(e) => Err(database_error_with_msg(e, "Failed to delete scheduler")),
        }
    }
}
