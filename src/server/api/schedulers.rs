//! Scheduler-related API endpoints

#![allow(clippy::too_many_arguments)]

use crate::server::transport_types::context_types::{ApiError, Has, XSpanIdString};
use async_trait::async_trait;
use log::{debug, error, info};
use sqlx::Row;

use crate::server::api_responses::{
    ApplySchedulerPlanResponse, CreateLocalSchedulerResponse, CreateScheduledComputeNodeResponse,
    CreateSlurmSchedulerResponse, DeleteLocalSchedulerResponse, DeleteLocalSchedulersResponse,
    DeleteScheduledComputeNodeResponse, DeleteScheduledComputeNodesResponse,
    DeleteSlurmSchedulerResponse, DeleteSlurmSchedulersResponse, GetLocalSchedulerResponse,
    GetScheduledComputeNodeResponse, GetSlurmSchedulerResponse, ListLocalSchedulersResponse,
    ListScheduledComputeNodesResponse, ListSlurmSchedulersResponse, UpdateLocalSchedulerResponse,
    UpdateScheduledComputeNodeResponse, UpdateSlurmSchedulerResponse,
};

use crate::models;

use super::{
    ApiContext, SqlQueryBuilder, begin_immediate, database_error_with_msg,
    database_lock_aware_error, message_error_response, resource_not_found_response,
};

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
        context: &C,
    ) -> Result<DeleteLocalSchedulersResponse, ApiError>;

    /// Delete all scheduled compute nodes for one workflow.
    async fn delete_scheduled_compute_nodes(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodesResponse, ApiError>;

    /// Delete all Slurm schedulers for one workflow.
    async fn delete_slurm_schedulers(
        &self,
        workflow_id: i64,
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
        context: &C,
    ) -> Result<DeleteLocalSchedulerResponse, ApiError>;

    /// Delete a scheduled compute node.
    async fn delete_scheduled_compute_node(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteScheduledComputeNodeResponse, ApiError>;

    /// Delete a Slurm scheduler.
    async fn delete_slurm_scheduler(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteSlurmSchedulerResponse, ApiError>;

    /// Apply a client-computed scheduler plan in one transaction: validate, suppress existing
    /// schedule_nodes actions, create schedulers, assign jobs, and create armed recovery actions.
    async fn apply_scheduler_plan(
        &self,
        workflow_id: i64,
        body: models::ApplySchedulerPlanRequest,
        dry_run: bool,
        context: &C,
    ) -> Result<ApplySchedulerPlanResponse, ApiError>;
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
    "serialize_allocations",
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
            "create_local_scheduler - X-Span-ID: {:?}",
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
            RETURNING rowid AS "id!"
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
            "create_scheduled_compute_node - X-Span-ID: {:?}",
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
            RETURNING rowid AS "id!"
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
            "create_slurm_scheduler - X-Span-ID: {:?}",
            context.get().0.clone()
        );

        // Stored NOT NULL; an omitted value means "not serialized".
        let serialize_allocations = body.serialize_allocations.unwrap_or(false);

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
                ,serialize_allocations
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING rowid AS "id!"
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
            serialize_allocations,
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
        // Report what was stored, not what was sent: the column is NOT NULL, so an
        // omitted serialize_allocations persists as false and every other read path
        // (get/list/update) returns it explicitly.
        body.serialize_allocations = Some(serialize_allocations);
        Ok(CreateSlurmSchedulerResponse::SuccessfulResponse(body))
    }

    /// Delete all local schedulers for one workflow.
    async fn delete_local_schedulers(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteLocalSchedulersResponse, ApiError> {
        debug!(
            "delete_local_schedulers({}) - X-Span-ID: {:?}",
            workflow_id,
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
        context: &C,
    ) -> Result<DeleteScheduledComputeNodesResponse, ApiError> {
        debug!(
            "delete_scheduled_compute_nodes({}) - X-Span-ID: {:?}",
            workflow_id,
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
        context: &C,
    ) -> Result<DeleteSlurmSchedulersResponse, ApiError> {
        debug!(
            "delete_slurm_schedulers({}) - X-Span-ID: {:?}",
            workflow_id,
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
                return Ok(GetLocalSchedulerResponse::NotFoundErrorResponse(
                    resource_not_found_response("Local scheduler", id),
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
                return Ok(GetScheduledComputeNodeResponse::NotFoundErrorResponse(
                    resource_not_found_response("Scheduled compute node", id),
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
                   partition, qos, tmp, walltime, extra, serialize_allocations
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
                return Ok(GetSlurmSchedulerResponse::NotFoundErrorResponse(
                    resource_not_found_response("Slurm scheduler", id),
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
            serialize_allocations: Some(record.get::<i64, _>("serialize_allocations") != 0),
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
        let sort_by = if let Some(ref col) = sort_by {
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
            .with_pagination_and_sorting(
                offset,
                limit,
                sort_by,
                reverse_sort,
                "id",
                LOCAL_SCHEDULER_COLUMNS,
            )
            .build();

        debug!("Executing query: {}", query);

        // Execute the query
        let records = match sqlx::query(sqlx::AssertSqlSafe(&*query))
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

        let total_count = match sqlx::query(sqlx::AssertSqlSafe(&*count_query))
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

        let response = crate::paginated_list_response!(
            models::ListLocalSchedulersResponse,
            items,
            offset,
            total_count
        );

        debug!(
            "list_local_schedulers({}, {}/{}) - X-Span-ID: {:?}",
            workflow_id,
            response.count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListLocalSchedulersResponse::HTTP(response))
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
        let sort_by = if let Some(ref col) = sort_by {
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
            .with_pagination_and_sorting(
                offset,
                limit,
                sort_by,
                reverse_sort,
                "id",
                SCHEDULED_COMPUTE_NODE_COLUMNS,
            )
            .build();

        debug!("Executing query: {}", query);

        // Execute the query with dynamic parameter binding
        let mut sqlx_query = sqlx::query(sqlx::AssertSqlSafe(&*query)).bind(workflow_id);

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

        let mut count_sqlx_query =
            sqlx::query(sqlx::AssertSqlSafe(&*count_query)).bind(workflow_id);

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

        let response = crate::paginated_list_response!(
            models::ListScheduledComputeNodesResponse,
            items,
            offset,
            total_count
        );

        debug!(
            "list_scheduled_compute_nodes({}, {}/{}) - X-Span-ID: {:?}",
            workflow_id,
            response.count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListScheduledComputeNodesResponse::SuccessfulResponse(
            response,
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
        let base_query = "SELECT id, workflow_id, name, account, gres, mem, nodes, ntasks_per_node, partition, qos, tmp, walltime, extra, serialize_allocations FROM slurm_scheduler".to_string();

        // Build WHERE clause
        let where_clause = "workflow_id = ?".to_string();
        let sort_by = if let Some(ref col) = sort_by {
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
            .with_pagination_and_sorting(
                offset,
                limit,
                sort_by,
                reverse_sort,
                "id",
                SLURM_SCHEDULER_COLUMNS,
            )
            .build();

        debug!("Executing query: {}", query);

        // Execute the query
        let records = match sqlx::query(sqlx::AssertSqlSafe(&*query))
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
                serialize_allocations: Some(record.get::<i64, _>("serialize_allocations") != 0),
            });
        }

        // For proper pagination, we should get the total count without LIMIT/OFFSET
        let count_query =
            SqlQueryBuilder::new("SELECT COUNT(*) as total FROM slurm_scheduler".to_string())
                .with_where(where_clause)
                .build();

        let total_count = match sqlx::query(sqlx::AssertSqlSafe(&*count_query))
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

        let response = crate::paginated_list_response!(
            models::ListSlurmSchedulersResponse,
            items,
            offset,
            total_count
        );

        debug!(
            "list_slurm_schedulers({}, {}/{}) - X-Span-ID: {:?}",
            workflow_id,
            response.count,
            total_count,
            context.get().0.clone()
        );

        Ok(ListSlurmSchedulersResponse::SuccessfulResponse(response))
    }

    /// Update a local scheduler.
    async fn update_local_scheduler(
        &self,
        id: i64,
        body: models::LocalSchedulerModel,
        context: &C,
    ) -> Result<UpdateLocalSchedulerResponse, ApiError> {
        debug!(
            "update_local_scheduler({}) - X-Span-ID: {:?}",
            id,
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
                memory = COALESCE($1, memory)
                ,num_cpus = COALESCE($2, num_cpus)
            WHERE id = $3 AND workflow_id = $4
            "#,
        )
        .bind(body.memory)
        .bind(body.num_cpus)
        .bind(id)
        .bind(body.workflow_id)
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Database operation failed"));
            }
        };

        if result.rows_affected() == 0 {
            let pool = self.context.pool.as_ref();
            return Ok(
                if super::workflow_id_mismatch(pool, "local_scheduler", id).await? {
                    UpdateLocalSchedulerResponse::UnprocessableContentErrorResponse(
                        super::workflow_id_immutable_response(),
                    )
                } else {
                    UpdateLocalSchedulerResponse::NotFoundErrorResponse(
                        resource_not_found_response("Local scheduler", id),
                    )
                },
            );
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
            "update_scheduled_compute_node({}) - X-Span-ID: {:?}",
            id,
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
                scheduler_id = COALESCE($1, scheduler_id)
                ,scheduler_config_id = COALESCE($2, scheduler_config_id)
                ,scheduler_type = COALESCE($3, scheduler_type)
                ,status = COALESCE($4, status)
            WHERE id = $5 AND workflow_id = $6
            "#,
        )
        .bind(body.scheduler_id)
        .bind(body.scheduler_config_id)
        .bind(body.scheduler_type)
        .bind(body.status)
        .bind(id)
        .bind(body.workflow_id)
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Database operation failed"));
            }
        };

        if result.rows_affected() == 0 {
            let pool = self.context.pool.as_ref();
            return Ok(
                if super::workflow_id_mismatch(pool, "scheduled_compute_node", id).await? {
                    UpdateScheduledComputeNodeResponse::UnprocessableContentErrorResponse(
                        super::workflow_id_immutable_response(),
                    )
                } else {
                    UpdateScheduledComputeNodeResponse::NotFoundErrorResponse(
                        resource_not_found_response("Scheduled compute node", id),
                    )
                },
            );
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
            "update_slurm_scheduler({}) - X-Span-ID: {:?}",
            id,
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

        // Optional text fields: null keeps the current value, "" clears it.
        let result = match sqlx::query(
            r#"
            UPDATE slurm_scheduler
            SET
                name = COALESCE($1, name)
                ,account = COALESCE($2, account)
                ,gres = NULLIF(COALESCE($3, gres), '')
                ,mem = NULLIF(COALESCE($4, mem), '')
                ,nodes = COALESCE($5, nodes)
                ,ntasks_per_node = COALESCE($6, ntasks_per_node)
                ,partition = NULLIF(COALESCE($7, partition), '')
                ,qos = NULLIF(COALESCE($8, qos), '')
                ,tmp = NULLIF(COALESCE($9, tmp), '')
                ,walltime = COALESCE($10, walltime)
                ,extra = NULLIF(COALESCE($11, extra), '')
                ,serialize_allocations = COALESCE($12, serialize_allocations)
            WHERE id = $13 AND workflow_id = $14
            "#,
        )
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
        .bind(body.serialize_allocations)
        .bind(id)
        .bind(body.workflow_id)
        .execute(self.context.pool.as_ref())
        .await
        {
            Ok(result) => result,
            Err(e) => {
                return Err(database_error_with_msg(e, "Database operation failed"));
            }
        };

        if result.rows_affected() == 0 {
            let pool = self.context.pool.as_ref();
            return Ok(
                if super::workflow_id_mismatch(pool, "slurm_scheduler", id).await? {
                    UpdateSlurmSchedulerResponse::UnprocessableContentErrorResponse(
                        super::workflow_id_immutable_response(),
                    )
                } else {
                    UpdateSlurmSchedulerResponse::NotFoundErrorResponse(
                        resource_not_found_response("Slurm scheduler", id),
                    )
                },
            );
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
        context: &C,
    ) -> Result<DeleteLocalSchedulerResponse, ApiError> {
        debug!(
            "delete_local_scheduler({}) - X-Span-ID: {:?}",
            id,
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
        context: &C,
    ) -> Result<DeleteScheduledComputeNodeResponse, ApiError> {
        debug!(
            "delete_scheduled_compute_node({}) - X-Span-ID: {:?}",
            id,
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
        context: &C,
    ) -> Result<DeleteSlurmSchedulerResponse, ApiError> {
        debug!(
            "delete_slurm_scheduler({}) - X-Span-ID: {:?}",
            id,
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

    async fn apply_scheduler_plan(
        &self,
        workflow_id: i64,
        body: models::ApplySchedulerPlanRequest,
        dry_run: bool,
        context: &C,
    ) -> Result<ApplySchedulerPlanResponse, ApiError> {
        debug!(
            "apply_scheduler_plan(workflow_id={}, schedulers={}, actions={}, dry_run={}) - X-Span-ID: {:?}",
            workflow_id,
            body.schedulers.len(),
            body.recovery_actions.len(),
            dry_run,
            context.get().0.clone()
        );
        let unprocessable = |msg: String| {
            Ok(
                ApplySchedulerPlanResponse::UnprocessableContentErrorResponse(
                    message_error_response(msg),
                ),
            )
        };
        if body.schedulers.is_empty() {
            return unprocessable("Scheduler plan has no schedulers".to_string());
        }
        // job_id -> index of the scheduler it is assigned to.
        let mut job_scheduler = std::collections::HashMap::new();
        for (idx, entry) in body.schedulers.iter().enumerate() {
            for id in &entry.job_ids {
                if job_scheduler.insert(*id, idx).is_some() {
                    return unprocessable(format!(
                        "job_id={} is listed more than once in the plan",
                        id
                    ));
                }
            }
        }
        for (i, action) in body.recovery_actions.iter().enumerate() {
            let Some(idx) = usize::try_from(action.scheduler_index)
                .ok()
                .filter(|idx| *idx < body.schedulers.len())
            else {
                return unprocessable(format!(
                    "recovery_actions[{}]: scheduler_index {} is out of range",
                    i, action.scheduler_index
                ));
            };
            if action.job_ids.is_empty() || action.num_allocations < 1 {
                return unprocessable(format!(
                    "recovery_actions[{}]: needs job_ids and num_allocations >= 1",
                    i
                ));
            }
            if let Some(id) = action
                .job_ids
                .iter()
                .find(|id| job_scheduler.get(id) != Some(&idx))
            {
                return unprocessable(format!(
                    "recovery_actions[{}]: job_id={} is not assigned to scheduler_index {}",
                    i, id, action.scheduler_index
                ));
            }
        }
        // Id sets are bound as one JSON array and expanded with json_each.
        let to_json = |ids: &[i64]| serde_json::to_string(ids).expect("serialize ids");
        let all_job_ids: Vec<i64> = job_scheduler.keys().copied().collect();

        // The write lock is taken up front so validation, arming, and the writes see the same
        // job statuses. A dry run does everything and rolls back.
        let mut tx = begin_immediate(&self.context.pool)
            .await
            .map_err(|e| database_lock_aware_error(e, "Failed to begin transaction"))?;

        // Validate: every job belongs to the workflow. Jobs that are no longer pending (a worker
        // claimed one after the client planned) are left out of the plan and reported, so a
        // race with a running allocation does not void the whole plan.
        let statuses: std::collections::HashMap<i64, i64> = sqlx::query_as::<_, (i64, i64)>(
            "SELECT j.id, j.status FROM json_each(?1) p \
                 JOIN job j ON j.id = p.value AND j.workflow_id = ?2",
        )
        .bind(to_json(&all_job_ids))
        .bind(workflow_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| database_error_with_msg(e, "Failed to read job statuses"))?
        .into_iter()
        .collect();
        let pending = [
            models::JobStatus::Uninitialized.to_int() as i64,
            models::JobStatus::Blocked.to_int() as i64,
            models::JobStatus::Ready.to_int() as i64,
        ];
        let mut missing: Vec<i64> = all_job_ids
            .iter()
            .copied()
            .filter(|id| !statuses.contains_key(id))
            .collect();
        if !missing.is_empty() {
            missing.sort_unstable();
            let total = missing.len();
            missing.truncate(20);
            return unprocessable(format!(
                "{} job(s) in the plan are not in workflow_id={}: {:?}",
                total, workflow_id, missing
            ));
        }
        let is_pending = |id: &i64| pending.contains(&statuses[id]);
        let mut skipped_job_ids: Vec<i64> = all_job_ids
            .iter()
            .copied()
            .filter(|id| !is_pending(id))
            .collect();
        skipped_job_ids.sort_unstable();
        // A recovery action whose scheduler has no pending jobs left is dropped: its allocation
        // would have nothing to run. One that gates on a skipped job while its scheduler still
        // has pending jobs (a merged scheduler gates only on its root jobs) cannot be armed
        // correctly from this plan, so the client must replan.
        // ponytail: reject-and-replan until workflow action triggering is redesigned.
        let mut keep_action = Vec::with_capacity(body.recovery_actions.len());
        for (i, action) in body.recovery_actions.iter().enumerate() {
            let has_pending = body.schedulers[action.scheduler_index as usize]
                .job_ids
                .iter()
                .any(is_pending);
            if has_pending && let Some(id) = action.job_ids.iter().find(|id| !is_pending(id)) {
                return unprocessable(format!(
                    "recovery_actions[{}]: gate job_id={} is no longer pending but its scheduler \
                     still has pending jobs; regenerate the plan",
                    i, id
                ));
            }
            keep_action.push(has_pending);
        }
        let mut response = models::ApplySchedulerPlanResponse {
            skipped_job_ids,
            ..Default::default()
        };
        let now = chrono::Utc::now().to_rfc3339();

        // Suppress: the old schedule_nodes actions would otherwise start their own allocations.
        // Persistent ones too: they would keep firing the old schedulers from every new worker.
        // `executed = 0` skips actions already claimed or suppressed (a persistent action keeps
        // executed = 0 across claims, so it is suppressed even after one has run).
        if body.suppress_existing_schedule_actions {
            response.suppressed_action_ids = sqlx::query_scalar::<_, i64>(
                "UPDATE workflow_action SET executed = 1, executed_at = ?1, state = 'suppressed', \
                 error = NULL WHERE workflow_id = ?2 AND action_type = 'schedule_nodes' \
                 AND is_recovery = 0 AND executed = 0 RETURNING id",
            )
            .bind(&now)
            .bind(workflow_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to suppress schedule actions"))?;
            response.suppressed_action_ids.sort_unstable();
        }

        // Create schedulers and assign their jobs.
        for entry in &body.schedulers {
            let s = &entry.scheduler;
            let id = sqlx::query_scalar::<_, i64>(
                "INSERT INTO slurm_scheduler (workflow_id, name, account, gres, mem, nodes, \
                 ntasks_per_node, partition, qos, tmp, walltime, extra, serialize_allocations) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
            )
            .bind(workflow_id)
            .bind(&s.name)
            .bind(&s.account)
            .bind(&s.gres)
            .bind(&s.mem)
            .bind(s.nodes)
            .bind(s.ntasks_per_node)
            .bind(&s.partition)
            .bind(&s.qos)
            .bind(&s.tmp)
            .bind(&s.walltime)
            .bind(&s.extra)
            .bind(s.serialize_allocations.unwrap_or(false))
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to create slurm scheduler"))?;
            let job_ids: Vec<i64> = entry.job_ids.iter().copied().filter(is_pending).collect();
            if !job_ids.is_empty() {
                sqlx::query(
                    "UPDATE job SET scheduler_id = ?1 \
                     WHERE id IN (SELECT value FROM json_each(?2))",
                )
                .bind(id)
                .bind(to_json(&job_ids))
                .execute(&mut *tx)
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to assign jobs to scheduler"))?;
            }
            response.scheduler_ids.push(id);
        }

        // Recovery actions. Their trigger is evaluated from job state when read; `pending` in the
        // response reports it as of this transaction. Every gate job of a kept action is pending
        // (checked above), and of the pending statuses only ready satisfies on_jobs_ready.
        let ready = models::JobStatus::Ready.to_int() as i64;
        for (action, keep) in body.recovery_actions.iter().zip(keep_action) {
            if !keep {
                continue;
            }
            let job_ids = &action.job_ids;
            let scheduler_id = response.scheduler_ids[action.scheduler_index as usize];
            let required = job_ids.len() as i64;
            let ready_count = job_ids.iter().filter(|id| statuses[id] == ready).count() as i64;
            let config = serde_json::json!({
                "scheduler_type": "slurm",
                "scheduler_id": scheduler_id,
                "num_allocations": action.num_allocations,
            });
            let id = sqlx::query_scalar::<_, i64>(
                "INSERT INTO workflow_action (workflow_id, trigger_type, action_type, \
                 action_config, job_ids, trigger_count, required_triggers, executed, persistent, \
                 is_recovery) \
                 VALUES (?, 'on_jobs_ready', 'schedule_nodes', ?, ?, 0, ?, 0, 0, 1) RETURNING id",
            )
            .bind(workflow_id)
            .bind(config.to_string())
            .bind(to_json(job_ids))
            .bind(required)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| database_error_with_msg(e, "Failed to create recovery action"))?;
            response.actions.push(models::AppliedSchedulerAction {
                id,
                scheduler_id,
                num_allocations: action.num_allocations,
                pending: ready_count >= required,
            });
        }

        if dry_run {
            tx.rollback()
                .await
                .map_err(|e| database_error_with_msg(e, "Failed to roll back dry run"))?;
        } else {
            tx.commit()
                .await
                .map_err(|e| database_lock_aware_error(e, "Failed to commit transaction"))?;
            info!(
                "Applied scheduler plan workflow_id={} schedulers={} jobs={} skipped={} actions={} suppressed={}",
                workflow_id,
                response.scheduler_ids.len(),
                all_job_ids.len() - response.skipped_job_ids.len(),
                response.skipped_job_ids.len(),
                response.actions.len(),
                response.suppressed_action_ids.len()
            );
        }
        Ok(ApplySchedulerPlanResponse::SuccessfulResponse(response))
    }
}
