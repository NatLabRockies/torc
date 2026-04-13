use super::*;
use crate::server::api::{
    EventsApi, JobsApi, ResultsApi, WorkflowsApi, begin_immediate_transaction,
    rollback_immediate_transaction,
};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

const CLAIM_JOBS_PAGE_SIZE: i64 = 256;
const CLAIM_JOB_ID_CHUNK_SIZE: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClaimSchedulerMode {
    Scoped,
    Fallback,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct DownstreamFamilyKey {
    resource_requirements_id: i64,
    scheduler_id: Option<i64>,
}

#[derive(Clone, Debug)]
struct DownstreamBufferState {
    occupancy: usize,
    buffer_limit: usize,
}

#[derive(Clone, Debug)]
struct FamilyResourceShape {
    memory_bytes: i64,
    num_cpus: i64,
    num_gpus: i64,
    num_nodes: i64,
}

#[derive(Clone, Debug)]
struct ComputeNodeCapacitySnapshot {
    active_count: i64,
    total_cpus: i64,
    total_memory_bytes: i64,
    total_gpus: i64,
    total_nodes: i64,
}

#[derive(Clone, Debug)]
struct ClaimJobCandidate {
    workflow_id: i64,
    job_id: i64,
    name: String,
    command: String,
    invocation_script: Option<String>,
    status: i64,
    cancel_on_blocking_job_failure: bool,
    supports_termination: bool,
    failure_handler_id: Option<i64>,
    attempt_id: Option<i64>,
    priority: i64,
    resource_requirements_id: i64,
    scheduler_id: Option<i64>,
    memory_bytes: i64,
    num_cpus: i64,
    num_gpus: i64,
    num_nodes: i64,
    runtime_s: i64,
}

impl ClaimJobCandidate {
    fn from_row(row: sqlx::sqlite::SqliteRow) -> Self {
        Self {
            workflow_id: row.get("workflow_id"),
            job_id: row.get("job_id"),
            name: row.get("name"),
            command: row.get("command"),
            invocation_script: row.get("invocation_script"),
            status: row.get("status"),
            cancel_on_blocking_job_failure: row.get("cancel_on_blocking_job_failure"),
            supports_termination: row.get("supports_termination"),
            failure_handler_id: row.get("failure_handler_id"),
            attempt_id: row.get("attempt_id"),
            priority: row.get("priority"),
            resource_requirements_id: row.get("resource_requirements_id"),
            scheduler_id: row.get("scheduler_id"),
            memory_bytes: row.get("memory_bytes"),
            num_cpus: row.get("num_cpus"),
            num_gpus: row.get("num_gpus"),
            num_nodes: row.get("num_nodes"),
            runtime_s: row.get("runtime_s"),
        }
    }

    fn to_job_model(&self) -> Result<models::JobModel, ApiError> {
        let status =
            models::JobStatus::from_int(self.status as i32).map_err(|e| ApiError(e.to_string()))?;
        if status != models::JobStatus::Ready {
            return Err(ApiError("Invalid job status in ready queue".to_string()));
        }

        Ok(models::JobModel {
            id: Some(self.job_id),
            workflow_id: self.workflow_id,
            name: self.name.clone(),
            command: self.command.clone(),
            invocation_script: self.invocation_script.clone(),
            status: Some(models::JobStatus::Pending),
            schedule_compute_nodes: None,
            cancel_on_blocking_job_failure: Some(self.cancel_on_blocking_job_failure),
            supports_termination: Some(self.supports_termination),
            depends_on_job_ids: None,
            input_file_ids: None,
            output_file_ids: None,
            input_user_data_ids: None,
            output_user_data_ids: None,
            resource_requirements_id: Some(self.resource_requirements_id),
            scheduler_id: None,
            failure_handler_id: self.failure_handler_id,
            attempt_id: self.attempt_id,
            priority: Some(self.priority),
        })
    }
}

#[derive(Clone, Debug)]
struct ClaimSelectionSpec {
    workflow_id: i64,
    selection_limit: usize,
    scheduler_mode: ClaimSchedulerMode,
    scheduler_config_id: Option<i64>,
    requested_memory_bytes: i64,
    requested_num_cpus: i64,
    requested_num_gpus: i64,
    requested_num_nodes: i64,
    total_nodes_for_fit: i64,
    time_limit_seconds: i64,
    downstream_buffer_multiplier: Option<usize>,
}

impl ClaimSelectionSpec {
    fn scheduler_allows(&self, scheduler_id: Option<i64>) -> bool {
        match self.scheduler_mode {
            ClaimSchedulerMode::Scoped => {
                scheduler_id.is_none() || scheduler_id == self.scheduler_config_id
            }
            ClaimSchedulerMode::Fallback => true,
        }
    }

    fn candidate_matches_request(&self, candidate: &ClaimJobCandidate) -> bool {
        candidate.status == i64::from(models::JobStatus::Ready.to_int())
            && self.scheduler_allows(candidate.scheduler_id)
            && candidate.memory_bytes <= self.requested_memory_bytes
            && candidate.num_cpus <= self.requested_num_cpus
            && candidate.num_gpus <= self.requested_num_gpus
            && candidate.num_nodes <= self.requested_num_nodes
            && candidate.runtime_s <= self.time_limit_seconds
    }
}

#[derive(Default)]
struct ClaimSelectionState {
    consumed_memory_bytes: i64,
    consumed_cpus: i64,
    consumed_gpus: i64,
    exclusive_nodes: i64,
    selected_candidates: Vec<ClaimJobCandidate>,
    job_family_cache: HashMap<i64, Vec<DownstreamFamilyKey>>,
    family_resource_cache: HashMap<DownstreamFamilyKey, Option<FamilyResourceShape>>,
    family_state_cache: HashMap<DownstreamFamilyKey, Option<DownstreamBufferState>>,
    compute_node_capacity_loaded: bool,
    overall_compute_node_capacity: Option<ComputeNodeCapacitySnapshot>,
    compute_node_capacity_by_scheduler: HashMap<i64, ComputeNodeCapacitySnapshot>,
}

impl ClaimSelectionState {
    fn selected_job_ids(&self) -> Vec<i64> {
        self.selected_candidates
            .iter()
            .map(|candidate| candidate.job_id)
            .collect()
    }

    fn claim_candidate(
        &mut self,
        candidate: ClaimJobCandidate,
        downstream_families: &[DownstreamFamilyKey],
    ) {
        let reserved_nodes = candidate.num_nodes.max(1);
        if reserved_nodes > 1 {
            self.exclusive_nodes += reserved_nodes;
        } else {
            self.consumed_memory_bytes += candidate.memory_bytes;
            self.consumed_cpus += candidate.num_cpus;
            self.consumed_gpus += candidate.num_gpus;
        }

        for family in downstream_families {
            if let Some(Some(state)) = self.family_state_cache.get_mut(family) {
                state.occupancy += 1;
            }
        }

        self.selected_candidates.push(candidate);
    }
}

#[derive(Default)]
struct ClaimSelectionStats {
    rows_scanned: usize,
    skipped_for_fit: usize,
    skipped_for_downstream_buffer: usize,
    skipped_for_status: usize,
    skipped_for_scheduler: usize,
    skipped_for_request_limits: usize,
}

#[derive(Default)]
struct ClaimQueryStats {
    rows_scanned: usize,
    pages_scanned: usize,
}

#[allow(clippy::too_many_arguments)]
fn calculate_downstream_capacity(
    total_cpus: i64,
    total_memory_bytes: i64,
    total_gpus: i64,
    total_nodes: i64,
    rr_cpus: i64,
    rr_memory_bytes: i64,
    rr_gpus: i64,
    rr_nodes: i64,
) -> usize {
    let mut capacities = Vec::new();
    if rr_cpus > 0 {
        capacities.push(total_cpus / rr_cpus);
    }
    if rr_memory_bytes > 0 {
        capacities.push(total_memory_bytes / rr_memory_bytes);
    }
    if rr_gpus > 0 {
        capacities.push(total_gpus / rr_gpus);
    }
    if rr_nodes > 1 {
        capacities.push(total_nodes / rr_nodes);
    }
    capacities.into_iter().min().unwrap_or(0).max(0) as usize
}

fn claim_overfetch_multiplier(limit: usize) -> usize {
    match limit {
        0..=4 => 4,
        5..=32 => 3,
        _ => 2,
    }
}

fn candidate_fits_accumulated_resources(
    spec: &ClaimSelectionSpec,
    state: &ClaimSelectionState,
    candidate: &ClaimJobCandidate,
) -> bool {
    let reserved_nodes = candidate.num_nodes.max(1);

    if reserved_nodes > 1 {
        let shared_nodes_after = spec.total_nodes_for_fit - state.exclusive_nodes - reserved_nodes;
        state.exclusive_nodes + reserved_nodes <= spec.total_nodes_for_fit
            && state.consumed_cpus <= shared_nodes_after * spec.requested_num_cpus
            && state.consumed_memory_bytes <= shared_nodes_after * spec.requested_memory_bytes
            && state.consumed_gpus <= shared_nodes_after * spec.requested_num_gpus
    } else {
        let shared_capacity_cpus =
            (spec.total_nodes_for_fit - state.exclusive_nodes) * spec.requested_num_cpus;
        let shared_capacity_memory =
            (spec.total_nodes_for_fit - state.exclusive_nodes) * spec.requested_memory_bytes;
        let shared_capacity_gpus =
            (spec.total_nodes_for_fit - state.exclusive_nodes) * spec.requested_num_gpus;

        state.consumed_cpus + candidate.num_cpus <= shared_capacity_cpus
            && state.consumed_memory_bytes + candidate.memory_bytes <= shared_capacity_memory
            && state.consumed_gpus + candidate.num_gpus <= shared_capacity_gpus
    }
}

async fn preload_job_families_for_jobs(
    conn: &mut sqlx::SqliteConnection,
    workflow_id: i64,
    candidate_job_ids: &[i64],
) -> Result<HashMap<i64, Vec<DownstreamFamilyKey>>, ApiError> {
    let mut job_family_cache = candidate_job_ids
        .iter()
        .copied()
        .map(|job_id| (job_id, Vec::new()))
        .collect::<HashMap<_, _>>();

    if candidate_job_ids.is_empty() {
        return Ok(job_family_cache);
    }

    for chunk in candidate_job_ids.chunks(CLAIM_JOB_ID_CHUNK_SIZE) {
        let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            r#"
            SELECT DISTINCT
                dep.depends_on_job_id AS upstream_job_id,
                downstream.resource_requirements_id AS resource_requirements_id,
                downstream.scheduler_id AS scheduler_id
            FROM job_depends_on dep
            JOIN job downstream ON downstream.id = dep.job_id
            WHERE dep.workflow_id = "#,
        );
        builder
            .push_bind(workflow_id)
            .push(" AND dep.depends_on_job_id IN (");

        let mut separated = builder.separated(", ");
        for job_id in chunk {
            separated.push_bind(job_id);
        }
        separated.push_unseparated(") AND downstream.resource_requirements_id IS NOT NULL");

        let rows = builder.build().fetch_all(&mut *conn).await.map_err(|e| {
            error!(
                "Database error loading downstream families workflow_id={} candidate_count={}: {}",
                workflow_id,
                chunk.len(),
                e
            );
            ApiError("Database error".to_string())
        })?;

        for row in rows {
            let upstream_job_id: i64 = row.get("upstream_job_id");
            let family = DownstreamFamilyKey {
                resource_requirements_id: row.get("resource_requirements_id"),
                scheduler_id: row.get("scheduler_id"),
            };
            job_family_cache
                .entry(upstream_job_id)
                .or_default()
                .push(family);
        }
    }

    Ok(job_family_cache)
}

async fn ensure_compute_node_capacity_snapshot(
    conn: &mut sqlx::SqliteConnection,
    workflow_id: i64,
    state: &mut ClaimSelectionState,
) -> Result<(), ApiError> {
    if state.compute_node_capacity_loaded {
        return Ok(());
    }

    let rows = sqlx::query(
        r#"
        SELECT
            scheduler_config_id,
            COUNT(*) AS active_count,
            COALESCE(SUM(num_cpus), 0) AS total_cpus,
            COALESCE(SUM(memory_gb), 0.0) AS total_memory_gb,
            COALESCE(SUM(num_gpus), 0) AS total_gpus,
            COALESCE(SUM(num_nodes), 0) AS total_nodes
        FROM compute_node
        WHERE workflow_id = $1
        AND is_active = 1
        GROUP BY scheduler_config_id
        "#,
    )
    .bind(workflow_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| {
        error!(
            "Database error loading downstream compute node capacity workflow_id={}: {}",
            workflow_id, e
        );
        ApiError("Database error".to_string())
    })?;

    let mut overall = ComputeNodeCapacitySnapshot {
        active_count: 0,
        total_cpus: 0,
        total_memory_bytes: 0,
        total_gpus: 0,
        total_nodes: 0,
    };

    for row in rows {
        let snapshot = ComputeNodeCapacitySnapshot {
            active_count: row.get("active_count"),
            total_cpus: row.get("total_cpus"),
            total_memory_bytes: (row.get::<f64, _>("total_memory_gb") * 1024.0 * 1024.0 * 1024.0)
                as i64,
            total_gpus: row.get("total_gpus"),
            total_nodes: row.get("total_nodes"),
        };

        overall.active_count += snapshot.active_count;
        overall.total_cpus += snapshot.total_cpus;
        overall.total_memory_bytes += snapshot.total_memory_bytes;
        overall.total_gpus += snapshot.total_gpus;
        overall.total_nodes += snapshot.total_nodes;

        if let Some(scheduler_id) = row.get::<Option<i64>, _>("scheduler_config_id") {
            state
                .compute_node_capacity_by_scheduler
                .insert(scheduler_id, snapshot);
        }
    }

    state.overall_compute_node_capacity = if overall.active_count > 0 {
        Some(overall)
    } else {
        None
    };
    state.compute_node_capacity_loaded = true;
    Ok(())
}

async fn preload_family_resource_shapes(
    conn: &mut sqlx::SqliteConnection,
    missing_families: &[DownstreamFamilyKey],
    state: &mut ClaimSelectionState,
) -> Result<(), ApiError> {
    let mut missing_rr_ids = missing_families
        .iter()
        .filter_map(|family| {
            (!state.family_resource_cache.contains_key(family))
                .then_some(family.resource_requirements_id)
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    if missing_rr_ids.is_empty() {
        return Ok(());
    }

    missing_rr_ids.sort_unstable();
    let mut resource_shapes_by_id = HashMap::new();

    for chunk in missing_rr_ids.chunks(CLAIM_JOB_ID_CHUNK_SIZE) {
        let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            r#"
            SELECT id, memory_bytes, num_cpus, num_gpus, num_nodes
            FROM resource_requirements
            WHERE id IN (
            "#,
        );
        let mut separated = builder.separated(", ");
        for rr_id in chunk {
            separated.push_bind(rr_id);
        }
        separated.push_unseparated(")");

        let rows = builder.build().fetch_all(&mut *conn).await.map_err(|e| {
            error!(
                "Database error loading downstream resource requirements count={}: {}",
                chunk.len(),
                e
            );
            ApiError("Database error".to_string())
        })?;

        for row in rows {
            resource_shapes_by_id.insert(
                row.get::<i64, _>("id"),
                FamilyResourceShape {
                    memory_bytes: row.get("memory_bytes"),
                    num_cpus: row.get("num_cpus"),
                    num_gpus: row.get("num_gpus"),
                    num_nodes: row.get("num_nodes"),
                },
            );
        }
    }

    for family in missing_families {
        if state.family_resource_cache.contains_key(family) {
            continue;
        }
        state.family_resource_cache.insert(
            family.clone(),
            resource_shapes_by_id
                .get(&family.resource_requirements_id)
                .cloned(),
        );
    }

    Ok(())
}

async fn load_family_occupancy_counts(
    conn: &mut sqlx::SqliteConnection,
    workflow_id: i64,
    rr_ids: &[i64],
) -> Result<HashMap<DownstreamFamilyKey, usize>, ApiError> {
    let mut occupancy_by_family = HashMap::new();
    if rr_ids.is_empty() {
        return Ok(occupancy_by_family);
    }

    let pending_status = models::JobStatus::Pending.to_int();
    let running_status = models::JobStatus::Running.to_int();
    let ready_status = models::JobStatus::Ready.to_int();

    for chunk in rr_ids.chunks(CLAIM_JOB_ID_CHUNK_SIZE) {
        let mut upstream_builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            r#"
            SELECT
                downstream.resource_requirements_id AS resource_requirements_id,
                downstream.scheduler_id AS scheduler_id,
                COUNT(DISTINCT upstream.id) AS count
            FROM job upstream
            JOIN job_depends_on dep ON dep.depends_on_job_id = upstream.id
            JOIN job downstream ON downstream.id = dep.job_id
            WHERE upstream.workflow_id =
            "#,
        );
        upstream_builder
            .push_bind(workflow_id)
            .push(" AND dep.workflow_id = ")
            .push_bind(workflow_id)
            .push(" AND downstream.workflow_id = ")
            .push_bind(workflow_id)
            .push(" AND upstream.status IN (")
            .push_bind(pending_status)
            .push(", ")
            .push_bind(running_status)
            .push(") AND downstream.resource_requirements_id IN (");
        let mut separated = upstream_builder.separated(", ");
        for rr_id in chunk {
            separated.push_bind(rr_id);
        }
        separated.push_unseparated(
            ") GROUP BY downstream.resource_requirements_id, downstream.scheduler_id",
        );

        let upstream_rows = upstream_builder
            .build()
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| {
                error!(
                    "Database error loading upstream occupancy workflow_id={} rr_count={}: {}",
                    workflow_id,
                    chunk.len(),
                    e
                );
                ApiError("Database error".to_string())
            })?;

        for row in upstream_rows {
            let family = DownstreamFamilyKey {
                resource_requirements_id: row.get("resource_requirements_id"),
                scheduler_id: row.get("scheduler_id"),
            };
            *occupancy_by_family.entry(family).or_insert(0) +=
                row.get::<i64, _>("count").max(0) as usize;
        }

        let mut downstream_builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            r#"
            SELECT
                resource_requirements_id,
                scheduler_id,
                COUNT(*) AS count
            FROM job
            WHERE workflow_id =
            "#,
        );
        downstream_builder
            .push_bind(workflow_id)
            .push(" AND status IN (")
            .push_bind(ready_status)
            .push(", ")
            .push_bind(pending_status)
            .push(", ")
            .push_bind(running_status)
            .push(") AND resource_requirements_id IN (");
        let mut separated = downstream_builder.separated(", ");
        for rr_id in chunk {
            separated.push_bind(rr_id);
        }
        separated.push_unseparated(") GROUP BY resource_requirements_id, scheduler_id");

        let downstream_rows = downstream_builder
            .build()
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| {
                error!(
                    "Database error loading downstream occupancy workflow_id={} rr_count={}: {}",
                    workflow_id,
                    chunk.len(),
                    e
                );
                ApiError("Database error".to_string())
            })?;

        for row in downstream_rows {
            let family = DownstreamFamilyKey {
                resource_requirements_id: row.get("resource_requirements_id"),
                scheduler_id: row.get("scheduler_id"),
            };
            *occupancy_by_family.entry(family).or_insert(0) +=
                row.get::<i64, _>("count").max(0) as usize;
        }
    }

    Ok(occupancy_by_family)
}

async fn preload_downstream_buffer_state_for_candidates(
    conn: &mut sqlx::SqliteConnection,
    spec: &ClaimSelectionSpec,
    candidate_job_ids: &[i64],
    state: &mut ClaimSelectionState,
) -> Result<(), ApiError> {
    let Some(downstream_buffer_multiplier) = spec.downstream_buffer_multiplier else {
        return Ok(());
    };

    let missing_job_ids = candidate_job_ids
        .iter()
        .filter(|job_id| !state.job_family_cache.contains_key(job_id))
        .copied()
        .collect::<Vec<_>>();

    if missing_job_ids.is_empty() {
        return Ok(());
    }

    ensure_compute_node_capacity_snapshot(conn, spec.workflow_id, state).await?;

    let job_family_cache =
        preload_job_families_for_jobs(conn, spec.workflow_id, &missing_job_ids).await?;
    state.job_family_cache.extend(job_family_cache);

    let missing_families = missing_job_ids
        .iter()
        .flat_map(|job_id| state.job_family_cache.get(job_id).into_iter().flatten())
        .filter(|family| !state.family_state_cache.contains_key(*family))
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    if missing_families.is_empty() {
        return Ok(());
    }

    preload_family_resource_shapes(conn, &missing_families, state).await?;

    let mut rr_ids = missing_families
        .iter()
        .map(|family| family.resource_requirements_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    rr_ids.sort_unstable();
    let occupancy_by_family = load_family_occupancy_counts(conn, spec.workflow_id, &rr_ids).await?;

    for family in missing_families {
        let Some(resource_shape) = state.family_resource_cache.get(&family).cloned().flatten()
        else {
            state.family_state_cache.insert(family, None);
            continue;
        };

        let capacity_snapshot = if let Some(scheduler_id) = family.scheduler_id {
            state.compute_node_capacity_by_scheduler.get(&scheduler_id)
        } else {
            state.overall_compute_node_capacity.as_ref()
        };

        let Some(capacity_snapshot) = capacity_snapshot else {
            state.family_state_cache.insert(family, None);
            continue;
        };

        if capacity_snapshot.active_count <= 0 {
            state.family_state_cache.insert(family, None);
            continue;
        }

        let downstream_capacity = calculate_downstream_capacity(
            capacity_snapshot.total_cpus,
            capacity_snapshot.total_memory_bytes,
            capacity_snapshot.total_gpus,
            capacity_snapshot.total_nodes,
            resource_shape.num_cpus,
            resource_shape.memory_bytes,
            resource_shape.num_gpus,
            resource_shape.num_nodes,
        );

        let occupancy = occupancy_by_family.get(&family).copied().unwrap_or(0);
        state.family_state_cache.insert(
            family,
            Some(DownstreamBufferState {
                occupancy,
                buffer_limit: downstream_capacity.saturating_mul(downstream_buffer_multiplier),
            }),
        );
    }

    Ok(())
}

fn process_candidate_batch(
    spec: &ClaimSelectionSpec,
    candidates: Vec<ClaimJobCandidate>,
    target_count: usize,
    state: &mut ClaimSelectionState,
    stats: &mut ClaimSelectionStats,
) -> Result<(), ApiError> {
    for candidate in candidates {
        if state.selected_candidates.len() >= target_count {
            break;
        }

        stats.rows_scanned += 1;

        if candidate.status != i64::from(models::JobStatus::Ready.to_int()) {
            stats.skipped_for_status += 1;
            continue;
        }

        if !spec.scheduler_allows(candidate.scheduler_id) {
            stats.skipped_for_scheduler += 1;
            continue;
        }

        if !spec.candidate_matches_request(&candidate) {
            stats.skipped_for_request_limits += 1;
            continue;
        }

        if !candidate_fits_accumulated_resources(spec, state, &candidate) {
            stats.skipped_for_fit += 1;

            let reserved_nodes = candidate.num_nodes.max(1);
            let reason = if reserved_nodes > 1 {
                let available = spec.total_nodes_for_fit - state.exclusive_nodes;
                format!(
                    "multi-node job needs {} free nodes, {} available (exclusive_nodes={}, shared cpus={}/{})",
                    reserved_nodes,
                    available,
                    state.exclusive_nodes,
                    state.consumed_cpus,
                    (spec.total_nodes_for_fit - state.exclusive_nodes) * spec.requested_num_cpus
                )
            } else {
                let shared_nodes = spec.total_nodes_for_fit - state.exclusive_nodes;
                format!(
                    "cpus: {}/{}, memory: {}/{}, gpus: {}/{}",
                    state.consumed_cpus + candidate.num_cpus,
                    shared_nodes * spec.requested_num_cpus,
                    state.consumed_memory_bytes + candidate.memory_bytes,
                    shared_nodes * spec.requested_memory_bytes,
                    state.consumed_gpus + candidate.num_gpus,
                    shared_nodes * spec.requested_num_gpus
                )
            };

            debug!(
                "Skipping job {} - would exceed resource limits ({})",
                candidate.job_id, reason
            );
            continue;
        }

        let downstream_families = if spec.downstream_buffer_multiplier.is_some() {
            let families = state
                .job_family_cache
                .get(&candidate.job_id)
                .cloned()
                .unwrap_or_default();

            let mut blocked_by_downstream_buffer = None;
            for family in &families {
                if let Some(Some(state)) = state.family_state_cache.get(family)
                    && state.occupancy >= state.buffer_limit
                {
                    blocked_by_downstream_buffer =
                        Some((family.clone(), state.occupancy, state.buffer_limit));
                    break;
                }
            }

            if let Some((family, occupancy, buffer_limit)) = blocked_by_downstream_buffer {
                stats.skipped_for_downstream_buffer += 1;
                debug!(
                    "Skipping job {} - downstream buffer full for rr_id={} scheduler_id={:?} ({}/{})",
                    candidate.job_id,
                    family.resource_requirements_id,
                    family.scheduler_id,
                    occupancy,
                    buffer_limit
                );
                continue;
            }

            families
        } else {
            Vec::new()
        };

        state.claim_candidate(candidate, &downstream_families);
    }

    Ok(())
}

async fn fetch_candidate_page(
    conn: &mut sqlx::SqliteConnection,
    spec: &ClaimSelectionSpec,
    offset: i64,
) -> Result<Vec<ClaimJobCandidate>, ApiError> {
    let rows = match spec.scheduler_mode {
        ClaimSchedulerMode::Scoped => {
            sqlx::query(
                r#"
                SELECT
                    job.workflow_id,
                    job.id AS job_id,
                    job.name,
                    job.command,
                    job.invocation_script,
                    job.status,
                    job.cancel_on_blocking_job_failure,
                    job.supports_termination,
                    job.failure_handler_id,
                    job.attempt_id,
                    job.priority,
                    job.scheduler_id,
                    rr.id AS resource_requirements_id,
                    rr.memory_bytes,
                    rr.num_cpus,
                    rr.num_gpus,
                    rr.num_nodes,
                    rr.runtime_s
                FROM job
                JOIN resource_requirements rr ON job.resource_requirements_id = rr.id
                WHERE job.workflow_id = $1
                AND job.status = $2
                AND rr.memory_bytes <= $3
                AND rr.num_cpus <= $4
                AND rr.num_gpus <= $5
                AND rr.num_nodes <= $6
                AND rr.runtime_s <= $7
                AND (job.scheduler_id IS NULL OR job.scheduler_id = $8)
                ORDER BY job.priority DESC, job.id ASC
                LIMIT $9
                OFFSET $10
                "#,
            )
            .bind(spec.workflow_id)
            .bind(models::JobStatus::Ready.to_int())
            .bind(spec.requested_memory_bytes)
            .bind(spec.requested_num_cpus)
            .bind(spec.requested_num_gpus)
            .bind(spec.requested_num_nodes)
            .bind(spec.time_limit_seconds)
            .bind(spec.scheduler_config_id)
            .bind(CLAIM_JOBS_PAGE_SIZE)
            .bind(offset)
            .fetch_all(&mut *conn)
            .await
        }
        ClaimSchedulerMode::Fallback => {
            sqlx::query(
                r#"
                SELECT
                    job.workflow_id,
                    job.id AS job_id,
                    job.name,
                    job.command,
                    job.invocation_script,
                    job.status,
                    job.cancel_on_blocking_job_failure,
                    job.supports_termination,
                    job.failure_handler_id,
                    job.attempt_id,
                    job.priority,
                    job.scheduler_id,
                    rr.id AS resource_requirements_id,
                    rr.memory_bytes,
                    rr.num_cpus,
                    rr.num_gpus,
                    rr.num_nodes,
                    rr.runtime_s
                FROM job
                JOIN resource_requirements rr ON job.resource_requirements_id = rr.id
                WHERE job.workflow_id = $1
                AND job.status = $2
                AND rr.memory_bytes <= $3
                AND rr.num_cpus <= $4
                AND rr.num_gpus <= $5
                AND rr.num_nodes <= $6
                AND rr.runtime_s <= $7
                ORDER BY job.priority DESC, job.id ASC
                LIMIT $8
                OFFSET $9
                "#,
            )
            .bind(spec.workflow_id)
            .bind(models::JobStatus::Ready.to_int())
            .bind(spec.requested_memory_bytes)
            .bind(spec.requested_num_cpus)
            .bind(spec.requested_num_gpus)
            .bind(spec.requested_num_nodes)
            .bind(spec.time_limit_seconds)
            .bind(CLAIM_JOBS_PAGE_SIZE)
            .bind(offset)
            .fetch_all(&mut *conn)
            .await
        }
    }
    .map_err(|e| {
        error!(
            "Database error fetching claim candidate page workflow_id={} scheduler_mode={:?}: {}",
            spec.workflow_id, spec.scheduler_mode, e
        );
        ApiError("Database error".to_string())
    })?;

    Ok(rows.into_iter().map(ClaimJobCandidate::from_row).collect())
}

async fn collect_phase_candidates(
    conn: &mut sqlx::SqliteConnection,
    spec: &ClaimSelectionSpec,
    target_count: usize,
) -> Result<(ClaimSelectionState, ClaimSelectionStats, ClaimQueryStats), ApiError> {
    let mut state = ClaimSelectionState::default();
    let mut selection_stats = ClaimSelectionStats::default();
    let mut query_stats = ClaimQueryStats::default();
    let mut page = 0i64;

    while state.selected_candidates.len() < target_count {
        let offset = page * CLAIM_JOBS_PAGE_SIZE;
        let candidates = fetch_candidate_page(conn, spec, offset).await?;
        if candidates.is_empty() {
            break;
        }

        let page_len = candidates.len();
        query_stats.rows_scanned += page_len;
        query_stats.pages_scanned += 1;

        let candidate_job_ids = candidates
            .iter()
            .map(|candidate| candidate.job_id)
            .collect::<Vec<_>>();
        preload_downstream_buffer_state_for_candidates(conn, spec, &candidate_job_ids, &mut state)
            .await?;

        process_candidate_batch(
            spec,
            candidates,
            target_count,
            &mut state,
            &mut selection_stats,
        )?;

        if page_len < CLAIM_JOBS_PAGE_SIZE as usize {
            break;
        }

        page += 1;
    }

    Ok((state, selection_stats, query_stats))
}

async fn fetch_claim_candidates_by_ids(
    conn: &mut sqlx::SqliteConnection,
    workflow_id: i64,
    candidate_ids: &[i64],
) -> Result<Vec<ClaimJobCandidate>, ApiError> {
    if candidate_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut candidates = Vec::new();
    for chunk in candidate_ids.chunks(CLAIM_JOB_ID_CHUNK_SIZE) {
        let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            r#"
            SELECT
                job.workflow_id,
                job.id AS job_id,
                job.name,
                job.command,
                job.invocation_script,
                job.status,
                job.cancel_on_blocking_job_failure,
                job.supports_termination,
                job.failure_handler_id,
                job.attempt_id,
                job.priority,
                job.scheduler_id,
                rr.id AS resource_requirements_id,
                rr.memory_bytes,
                rr.num_cpus,
                rr.num_gpus,
                rr.num_nodes,
                rr.runtime_s
            FROM job
            JOIN resource_requirements rr ON job.resource_requirements_id = rr.id
            WHERE job.workflow_id = "#,
        );
        builder.push_bind(workflow_id).push(" AND job.id IN (");

        let mut separated = builder.separated(", ");
        for job_id in chunk {
            separated.push_bind(job_id);
        }
        separated.push_unseparated(")");

        let rows = builder.build().fetch_all(&mut *conn).await.map_err(|e| {
            error!(
                "Database error fetching buffered claim candidates workflow_id={}: {}",
                workflow_id, e
            );
            ApiError("Database error".to_string())
        })?;

        candidates.extend(rows.into_iter().map(ClaimJobCandidate::from_row));
    }

    candidates.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.job_id.cmp(&right.job_id))
    });

    Ok(candidates)
}

async fn update_claimed_jobs_to_pending(
    conn: &mut sqlx::SqliteConnection,
    claimed_job_ids: &[i64],
) -> Result<(), ApiError> {
    if claimed_job_ids.is_empty() {
        return Ok(());
    }

    for chunk in claimed_job_ids.chunks(CLAIM_JOB_ID_CHUNK_SIZE) {
        let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new("UPDATE job SET status = ");
        builder
            .push_bind(models::JobStatus::Pending.to_int())
            .push(" WHERE id IN (");

        let mut separated = builder.separated(", ");
        for job_id in chunk {
            separated.push_bind(job_id);
        }
        separated.push_unseparated(")");

        builder.build().execute(&mut *conn).await.map_err(|e| {
            error!("Failed to update claimed jobs to pending: {}", e);
            ApiError("Database update error".to_string())
        })?;
    }

    Ok(())
}

async fn hydrate_claimed_job_outputs(
    conn: &mut sqlx::SqliteConnection,
    workflow_id: i64,
    selected_jobs: &mut [models::JobModel],
) -> Result<(), ApiError> {
    if selected_jobs.is_empty() {
        return Ok(());
    }

    let selected_job_ids = selected_jobs
        .iter()
        .filter_map(|job| job.id)
        .collect::<Vec<_>>();

    let mut output_files_map: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut output_user_data_map: HashMap<i64, Vec<i64>> = HashMap::new();

    for chunk in selected_job_ids.chunks(CLAIM_JOB_ID_CHUNK_SIZE) {
        let mut file_builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "SELECT job_id, file_id FROM job_output_file WHERE workflow_id = ",
        );
        file_builder.push_bind(workflow_id).push(" AND job_id IN (");
        let mut file_ids = file_builder.separated(", ");
        for job_id in chunk {
            file_ids.push_bind(job_id);
        }
        file_ids.push_unseparated(")");

        let output_files = file_builder
            .build()
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| {
                error!("Failed to query output files for claimed jobs: {}", e);
                ApiError("Database query error".to_string())
            })?;

        for row in output_files {
            let job_id: i64 = row.get("job_id");
            let file_id: i64 = row.get("file_id");
            output_files_map.entry(job_id).or_default().push(file_id);
        }

        let mut user_data_builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "SELECT job_id, user_data_id FROM job_output_user_data WHERE job_id IN (",
        );
        let mut user_data_ids = user_data_builder.separated(", ");
        for job_id in chunk {
            user_data_ids.push_bind(job_id);
        }
        user_data_ids.push_unseparated(")");

        let output_user_data = user_data_builder
            .build()
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| {
                error!("Failed to query output user_data for claimed jobs: {}", e);
                ApiError("Database query error".to_string())
            })?;

        for row in output_user_data {
            let job_id: i64 = row.get("job_id");
            let user_data_id: i64 = row.get("user_data_id");
            output_user_data_map
                .entry(job_id)
                .or_default()
                .push(user_data_id);
        }
    }

    for job in selected_jobs {
        if let Some(job_id) = job.id {
            job.output_file_ids = output_files_map.get(&job_id).cloned();
            job.output_user_data_ids = output_user_data_map.get(&job_id).cloned();
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
impl<C> Server<C>
where
    C: Has<XSpanIdString> + Has<Option<Authorization>> + Send + Sync,
{
    pub(super) async fn transport_create_job(
        &self,
        mut job: models::JobModel,
        context: &C,
    ) -> Result<CreateJobResponse, ApiError> {
        authorize_workflow!(self, job.workflow_id, context, CreateJobResponse);

        if job.resource_requirements_id.is_none() {
            let default_id = self
                .get_default_resource_requirements_id(job.workflow_id, context)
                .await?;
            job.resource_requirements_id = Some(default_id);
        }

        self.jobs_api.create_job(job, context).await
    }

    pub(super) async fn transport_create_jobs(
        &self,
        mut body: models::JobsModel,
        context: &C,
    ) -> Result<CreateJobsResponse, ApiError> {
        if body.jobs.is_empty() {
            return self.jobs_api.create_jobs(body, context).await;
        }

        let first_workflow_id = body.jobs[0].workflow_id;
        for job in &body.jobs {
            if job.workflow_id != first_workflow_id {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!(
                        "All jobs in a batch must have the same workflow_id. Found workflow_ids: {} and {}",
                        first_workflow_id, job.workflow_id
                    )
                }));
                return Ok(CreateJobsResponse::UnprocessableContentErrorResponse(
                    error_response,
                ));
            }
        }

        authorize_workflow!(self, first_workflow_id, context, CreateJobsResponse);

        let default_resource_requirements_id = self
            .get_default_resource_requirements_id(first_workflow_id, context)
            .await?;

        for job in &mut body.jobs {
            if job.resource_requirements_id.is_none() {
                job.resource_requirements_id = Some(default_resource_requirements_id);
            }
        }

        self.jobs_api.create_jobs(body, context).await
    }

    pub(super) async fn transport_initialize_jobs(
        &self,
        id: i64,
        only_uninitialized: Option<bool>,
        clear_ephemeral_user_data: Option<bool>,
        context: &C,
    ) -> Result<InitializeJobsResponse, ApiError> {
        info!(
            "initialize_jobs({}, {:?}, {:?}) - X-Span-ID: {:?}",
            id,
            only_uninitialized,
            clear_ephemeral_user_data,
            Has::<XSpanIdString>::get(context).0.clone()
        );
        authorize_workflow!(self, id, context, InitializeJobsResponse);

        if let Ok(mut set) = self.workflows_with_failures.write() {
            set.remove(&id);
        }

        let mut tx = match self.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                error!("Failed to begin transaction for initialize_jobs: {}", e);
                return Err(ApiError("Database error".to_string()));
            }
        };

        if let Err(e) = self
            .add_depends_on_associations_from_files(&mut *tx, id)
            .await
        {
            error!("Failed to add depends-on associations from files: {}", e);
            let _ = tx.rollback().await;
            return Err(e);
        }

        if let Err(e) = self
            .add_depends_on_associations_from_user_data(&mut *tx, id)
            .await
        {
            error!(
                "Failed to add depends-on associations from user_data: {}",
                e
            );
            let _ = tx.rollback().await;
            return Err(e);
        }

        let only_uninit = only_uninitialized.unwrap_or(false);
        if only_uninit && let Err(e) = self.uninitialize_blocked_jobs(&mut *tx, id).await {
            error!("Failed to uninitialize blocked jobs: {}", e);
            let _ = tx.rollback().await;
            return Err(e);
        }

        if let Err(e) = self
            .initialize_blocked_jobs_to_blocked(&mut *tx, id, only_uninit)
            .await
        {
            error!("Failed to initialize blocked jobs to blocked: {}", e);
            let _ = tx.rollback().await;
            return Err(e);
        }

        if let Err(e) = self.initialize_unblocked_jobs(&mut *tx, id).await {
            error!("Failed to initialize unblocked jobs: {}", e);
            let _ = tx.rollback().await;
            return Err(e);
        }

        let completed_status = models::JobStatus::Completed.to_int();
        let failed_status = models::JobStatus::Failed.to_int();
        let canceled_status = models::JobStatus::Canceled.to_int();
        let terminated_status = models::JobStatus::Terminated.to_int();

        match sqlx::query!(
            r#"
            DELETE FROM workflow_result
            WHERE workflow_id = $1
              AND job_id IN (
                SELECT id FROM job
                WHERE workflow_id = $1
                  AND status NOT IN ($2, $3, $4, $5)
              )
            "#,
            id,
            completed_status,
            failed_status,
            canceled_status,
            terminated_status
        )
        .execute(&mut *tx)
        .await
        {
            Ok(result) => {
                debug!(
                    "Deleted {} workflow_result records for incomplete jobs in workflow {}",
                    result.rows_affected(),
                    id
                );
            }
            Err(e) => {
                error!(
                    "Failed to delete workflow_result records for incomplete jobs: {}",
                    e
                );
                let _ = tx.rollback().await;
                return Err(ApiError("Database error".to_string()));
            }
        }

        if let Err(e) = tx.commit().await {
            error!("Failed to commit transaction for initialize_jobs: {}", e);
            return Err(ApiError("Database error".to_string()));
        }

        self.jobs_api.compute_and_store_all_input_hashes(id).await?;

        match sqlx::query!("SELECT enable_ro_crate FROM workflow WHERE id = $1", id)
            .fetch_optional(self.pool.as_ref())
            .await
        {
            Ok(Some(row)) if row.enable_ro_crate == Some(1) => {
                debug!(
                    "enable_ro_crate is true for workflow {}, creating input file entities",
                    id
                );
                if let Err(e) = self.ro_crate_api.create_entities_for_input_files(id).await {
                    warn!("Failed to create RO-Crate entities for input files: {}", e);
                }
            }
            Ok(_) => {}
            Err(e) => warn!("Failed to check enable_ro_crate flag: {}", e),
        }

        if let Err(e) = self.ro_crate_api.create_server_software_entity(id).await {
            warn!("Failed to create torc-server software entity: {}", e);
        }

        if let Err(e) = self
            .workflow_actions_api
            .reset_actions_for_reinitialize(id)
            .await
        {
            error!(
                "Failed to reset workflow actions for workflow {}: {}",
                id, e
            );
        }

        if let Err(e) = self
            .workflow_actions_api
            .check_and_trigger_actions(id, "on_workflow_start", None)
            .await
        {
            error!(
                "Failed to check_and_trigger_actions for on_workflow_start: {}",
                e
            );
        }

        for trigger_type in &["on_worker_start", "on_worker_complete"] {
            match sqlx::query(
                "UPDATE workflow_action SET trigger_count = required_triggers WHERE workflow_id = ? AND trigger_type = ?"
            )
            .bind(id)
            .bind(trigger_type)
            .execute(self.pool.as_ref())
            .await
            {
                Ok(result) => {
                    let count = result.rows_affected();
                    if count > 0 {
                        debug!("Activated {} {} actions for workflow {}", count, trigger_type, id);
                    }
                }
                Err(e) => {
                    error!("Failed to activate {} actions for workflow {}: {}", trigger_type, id, e);
                }
            }
        }

        if let Err(e) = self
            .workflow_actions_api
            .check_and_trigger_actions(id, "on_jobs_ready", None)
            .await
        {
            error!(
                "Failed to check_and_trigger_actions for on_jobs_ready: {}",
                e
            );
        }

        let event_type = if only_uninitialized.unwrap_or(false) {
            "workflow_started"
        } else {
            "workflow_reinitialized"
        };

        let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
        let username = auth
            .map(|a| a.subject)
            .unwrap_or_else(|| "unknown".to_string());

        self.event_broadcaster.broadcast(BroadcastEvent {
            workflow_id: id,
            timestamp: chrono::Utc::now().timestamp_millis(),
            event_type: event_type.to_string(),
            severity: models::EventSeverity::Info,
            data: serde_json::json!({
                "category": "workflow",
                "type": event_type,
                "user": username,
                "message": format!("{} workflow {}", event_type.replace('_', " "), id),
            }),
        });

        Ok(InitializeJobsResponse::SuccessfulResponse(
            serde_json::json!({"message": "Initialized job status"}),
        ))
    }

    pub(super) async fn transport_delete_jobs(
        &self,
        workflow_id: i64,
        context: &C,
    ) -> Result<DeleteJobsResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, DeleteJobsResponse);
        self.jobs_api.delete_jobs(workflow_id, context).await
    }

    pub(super) async fn transport_list_jobs(
        &self,
        workflow_id: i64,
        status: Option<models::JobStatus>,
        needs_file_id: Option<i64>,
        upstream_job_id: Option<i64>,
        offset: Option<i64>,
        limit: Option<i64>,
        sort_by: Option<String>,
        reverse_sort: Option<bool>,
        include_relationships: Option<bool>,
        active_compute_node_id: Option<i64>,
        context: &C,
    ) -> Result<ListJobsResponse, ApiError> {
        authorize_workflow!(self, workflow_id, context, ListJobsResponse);
        let (processed_offset, processed_limit) = process_pagination_params(offset, limit)?;
        self.jobs_api
            .list_jobs(
                workflow_id,
                status,
                needs_file_id,
                upstream_job_id,
                processed_offset,
                processed_limit,
                sort_by,
                reverse_sort,
                include_relationships,
                active_compute_node_id,
                context,
            )
            .await
    }

    pub(super) async fn transport_get_job(
        &self,
        id: i64,
        context: &C,
    ) -> Result<GetJobResponse, ApiError> {
        authorize_job!(self, id, context, GetJobResponse);
        self.jobs_api.get_job(id, context).await
    }

    pub(super) async fn transport_list_job_ids(
        &self,
        id: i64,
        context: &C,
    ) -> Result<ListJobIdsResponse, ApiError> {
        authorize_workflow!(self, id, context, ListJobIdsResponse);
        self.jobs_api.list_job_ids(id, context).await
    }

    pub(super) async fn transport_update_job(
        &self,
        id: i64,
        body: models::JobModel,
        context: &C,
    ) -> Result<UpdateJobResponse, ApiError> {
        authorize_job!(self, id, context, UpdateJobResponse);
        self.jobs_api.update_job(id, body, context).await
    }

    pub(super) async fn transport_claim_next_jobs(
        &self,
        id: i64,
        limit: Option<i64>,
        context: &C,
    ) -> Result<ClaimNextJobsResponse, ApiError> {
        debug!(
            "claim_next_jobs({}, {:?}) - X-Span-ID: {:?}",
            id,
            limit,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        authorize_workflow!(self, id, context, ClaimNextJobsResponse);

        self.jobs_api
            .claim_next_jobs(id, limit.unwrap_or(10), context)
            .await
    }

    pub(super) async fn transport_process_changed_job_inputs(
        &self,
        id: i64,
        dry_run: Option<bool>,
        context: &C,
    ) -> Result<ProcessChangedJobInputsResponse, ApiError> {
        authorize_workflow!(self, id, context, ProcessChangedJobInputsResponse);
        let dry_run_value = dry_run.unwrap_or(false);
        self.jobs_api
            .process_changed_job_inputs(id, dry_run_value, context)
            .await
    }

    pub(super) async fn transport_retry_job(
        &self,
        id: i64,
        run_id: i64,
        max_retries: i32,
        context: &C,
    ) -> Result<RetryJobResponse, ApiError> {
        authorize_job!(self, id, context, RetryJobResponse);
        self.jobs_api
            .retry_job(id, run_id, max_retries, context)
            .await
    }

    pub(super) async fn transport_delete_job(
        &self,
        id: i64,
        context: &C,
    ) -> Result<DeleteJobResponse, ApiError> {
        authorize_job!(self, id, context, DeleteJobResponse);
        self.jobs_api.delete_job(id, context).await
    }

    pub(super) async fn transport_reset_job_status(
        &self,
        id: i64,
        failed_only: Option<bool>,
        context: &C,
    ) -> Result<ResetJobStatusResponse, ApiError> {
        info!(
            "reset_job_status(workflow_id={}, failed_only={:?}) - X-Span-ID: {:?}",
            id,
            failed_only,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        authorize_workflow!(self, id, context, ResetJobStatusResponse);

        let failed_only_value = failed_only.unwrap_or(false);
        let result = self
            .jobs_api
            .reset_job_status(id, failed_only_value, context)
            .await?;

        if let ResetJobStatusResponse::SuccessfulResponse(ref response) = result {
            let auth: Option<Authorization> = Has::<Option<Authorization>>::get(context).clone();
            let username = auth
                .map(|a| a.subject)
                .unwrap_or_else(|| "unknown".to_string());

            let event = models::EventModel::new(
                id,
                serde_json::json!({
                    "category": "user_action",
                    "action": "reset_job_status",
                    "user": username,
                    "workflow_id": id,
                    "failed_only": failed_only_value,
                    "updated_count": response.updated_count,
                }),
            );
            if let Err(e) = self.events_api.create_event(event, context).await {
                error!("Failed to create event for reset_job_status: {:?}", e);
            }
        }

        Ok(result)
    }

    pub(super) async fn transport_manage_status_change(
        &self,
        id: i64,
        status: models::JobStatus,
        run_id: i64,
        context: &C,
    ) -> Result<ManageStatusChangeResponse, ApiError> {
        debug!(
            "manage_status_change({}, {:?}, {}) - X-Span-ID: {:?}",
            id,
            status,
            run_id,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        if status.is_complete() {
            error!(
                "manage_status_change: cannot set completion status '{}' for job_id={}. Use complete_job instead.",
                status, id
            );
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "Cannot set completion status '{}' via manage_status_change. Use complete_job API instead.",
                    status
                )
            }));
            return Ok(
                ManageStatusChangeResponse::UnprocessableContentErrorResponse(error_response),
            );
        }

        authorize_job!(self, id, context, ManageStatusChangeResponse);

        let mut job = match self.jobs_api.get_job(id, context).await? {
            GetJobResponse::SuccessfulResponse(job) => job,
            GetJobResponse::ForbiddenErrorResponse(err) => {
                return Ok(ManageStatusChangeResponse::DefaultErrorResponse(err));
            }
            GetJobResponse::NotFoundErrorResponse(err) => {
                return Ok(ManageStatusChangeResponse::NotFoundErrorResponse(err));
            }
            GetJobResponse::DefaultErrorResponse(err) => {
                return Ok(ManageStatusChangeResponse::DefaultErrorResponse(err));
            }
        };

        let current_status = *job.status.as_ref().ok_or_else(|| {
            error!("Job status is missing for job_id={}", id);
            ApiError("Job status is required".to_string())
        })?;

        if current_status == status {
            debug!(
                "manage_status_change: job_id={} already has status '{}', no change needed",
                id, status
            );
            return Ok(ManageStatusChangeResponse::SuccessfulResponse(job));
        }

        if let Err(e) = self.validate_run_id(job.workflow_id, run_id).await {
            error!("manage_status_change: job_id={}, {}", id, e);
            let error_response = models::ErrorResponse::new(serde_json::json!({ "message": e }));
            return Ok(
                ManageStatusChangeResponse::UnprocessableContentErrorResponse(error_response),
            );
        }

        job.status = Some(status);

        let new_status_int = status.to_int();
        let current_status_int = current_status.to_int();
        let update_result = sqlx::query!(
            "UPDATE job SET status = ? WHERE id = ? AND status = ?",
            new_status_int,
            id,
            current_status_int,
        )
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| {
            error!("Failed to update job status: {}", e);
            ApiError("Database error".to_string())
        })?;

        if update_result.rows_affected() == 0 {
            let exists = sqlx::query_scalar!("SELECT id FROM job WHERE id = ?", id)
                .fetch_optional(self.pool.as_ref())
                .await
                .map_err(|e| {
                    error!("Failed to check job existence: {}", e);
                    ApiError("Database error".to_string())
                })?;

            if exists.is_none() {
                let error_response = models::ErrorResponse::new(serde_json::json!({
                    "message": format!("Job not found with ID: {}", id)
                }));
                return Ok(ManageStatusChangeResponse::NotFoundErrorResponse(
                    error_response,
                ));
            }

            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "Job {} status was concurrently modified (expected '{}'), please retry",
                    id, current_status
                )
            }));
            return Ok(
                ManageStatusChangeResponse::UnprocessableContentErrorResponse(error_response),
            );
        }

        let workflow_id = job.workflow_id;

        let updated_job = match self.get_job(id, context).await? {
            GetJobResponse::SuccessfulResponse(fresh_job) => fresh_job,
            _ => {
                job.status = Some(status);
                job
            }
        };

        if current_status.is_complete()
            && status == models::JobStatus::Uninitialized
            && let Err(e) = self.reinitialize_downstream_jobs(id, workflow_id).await
        {
            error!(
                "Failed to reinitialize downstream jobs for job {}: {}",
                id, e
            );
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": "Failed to reinitialize downstream jobs"
            }));
            return Ok(ManageStatusChangeResponse::DefaultErrorResponse(
                error_response,
            ));
        }

        debug!(
            "manage_status_change: successfully changed job_id={} status from '{}' to '{}'",
            id, current_status, status
        );

        Ok(ManageStatusChangeResponse::SuccessfulResponse(updated_job))
    }

    pub(super) async fn transport_start_job(
        &self,
        id: i64,
        run_id: i64,
        compute_node_id: i64,
        context: &C,
    ) -> Result<StartJobResponse, ApiError> {
        debug!(
            "start_job({}, {}, {}) - X-Span-ID: {:?}",
            id,
            run_id,
            compute_node_id,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        authorize_job!(self, id, context, StartJobResponse);

        let mut job = match self.jobs_api.get_job(id, context).await? {
            GetJobResponse::SuccessfulResponse(job) => job,
            GetJobResponse::ForbiddenErrorResponse(err) => {
                error!("Access denied for job {}: {:?}", id, err);
                return Ok(StartJobResponse::ForbiddenErrorResponse(err));
            }
            GetJobResponse::NotFoundErrorResponse(err) => {
                error!("Job not found {}: {:?}", id, err);
                return Ok(StartJobResponse::NotFoundErrorResponse(err));
            }
            GetJobResponse::DefaultErrorResponse(err) => {
                error!("Failed to get job {}: {:?}", id, err);
                return Ok(StartJobResponse::DefaultErrorResponse(err));
            }
        };
        match job.status {
            Some(models::JobStatus::Pending) => {
                job.status = Some(models::JobStatus::Running);
            }
            Some(status) => {
                error!(
                    "start_job: Invalid job status for job_id={}. Expected SubmittedPending, got {:?}",
                    id, status
                );
                return Err(ApiError(format!(
                    "Job {} has invalid status {:?}. Expected SubmittedPending for job start.",
                    id, status
                )));
            }
            None => {
                error!("start_job: Job status not set for job_id={}", id);
                return Err(ApiError(format!(
                    "Job {} has no status set. Expected SubmittedPending for job start.",
                    id
                )));
            }
        }

        if let Err(e) = self.validate_run_id(job.workflow_id, run_id).await {
            error!("start_job: job_id={}, {}", id, e);
            let error_response = models::ErrorResponse::new(serde_json::json!({ "message": e }));
            return Ok(StartJobResponse::UnprocessableContentErrorResponse(
                error_response,
            ));
        }

        let pending_int = models::JobStatus::Pending.to_int();
        let running_int = models::JobStatus::Running.to_int();
        let start_result = sqlx::query!(
            "UPDATE job SET status = ? WHERE id = ? AND status = ?",
            running_int,
            id,
            pending_int,
        )
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| {
            error!("Failed to update job status for start_job: {}", e);
            ApiError("Database error".to_string())
        })?;

        if start_result.rows_affected() == 0 {
            error!(
                "start_job: job_id={} status was concurrently changed from Pending, cannot start",
                id
            );
            return Err(ApiError(format!(
                "Job {} status was concurrently modified, cannot start",
                id
            )));
        }

        match sqlx::query!(
            "UPDATE job_internal SET active_compute_node_id = ? WHERE job_id = ?",
            compute_node_id,
            id
        )
        .execute(self.pool.as_ref())
        .await
        {
            Ok(_) => {
                debug!(
                    "Set active_compute_node_id={} for job_id={}",
                    compute_node_id, id
                );
            }
            Err(e) => {
                error!(
                    "Failed to set active_compute_node_id for job_id={}: {}",
                    id, e
                );
            }
        }

        self.event_broadcaster.broadcast(BroadcastEvent {
            workflow_id: job.workflow_id,
            timestamp: chrono::Utc::now().timestamp_millis(),
            event_type: "job_started".to_string(),
            severity: models::EventSeverity::Info,
            data: serde_json::json!({
                "job_id": id,
                "job_name": job.name,
                "compute_node_id": compute_node_id,
                "run_id": run_id,
            }),
        });
        debug!("Broadcast job_started event for job_id={}", id);

        Ok(StartJobResponse::SuccessfulResponse(job))
    }

    pub(super) async fn transport_complete_job(
        &self,
        id: i64,
        status: models::JobStatus,
        run_id: i64,
        result: models::ResultModel,
        context: &C,
    ) -> Result<CompleteJobResponse, ApiError> {
        debug!(
            "complete_job({}, {:?}, {}, {:?}) - X-Span-ID: {:?}",
            id,
            status,
            run_id,
            result,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        authorize_job!(self, id, context, CompleteJobResponse);

        if !status.is_terminal() {
            error!(
                "Attempted to complete job {} with non-terminal status '{}'",
                id, status
            );
            return Err(ApiError(format!(
                "Status '{}' is not a terminal status for job completion",
                status
            )));
        }

        let mut job = match self.jobs_api.get_job(id, context).await? {
            GetJobResponse::SuccessfulResponse(job) => job,
            GetJobResponse::ForbiddenErrorResponse(err) => {
                error!("Access denied for job {}: {:?}", id, err);
                return Ok(CompleteJobResponse::ForbiddenErrorResponse(err));
            }
            GetJobResponse::NotFoundErrorResponse(err) => {
                error!("Job not found {}: {:?}", id, err);
                return Ok(CompleteJobResponse::NotFoundErrorResponse(err));
            }
            GetJobResponse::DefaultErrorResponse(err) => {
                error!("Failed to get job {}: {:?}", id, err);
                return Ok(CompleteJobResponse::DefaultErrorResponse(err));
            }
        };

        if let Some(current_status) = &job.status
            && current_status.is_complete()
        {
            error!(
                "Job {} is already complete with status {:?}",
                id, current_status
            );
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Job {} is already complete with status {:?}", id, current_status)
            }));
            return Ok(CompleteJobResponse::UnprocessableContentErrorResponse(
                error_response,
            ));
        }

        if result.job_id != id {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "ResultModel job_id {} does not match target job_id {}",
                    result.job_id, id
                )
            }));
            return Ok(CompleteJobResponse::UnprocessableContentErrorResponse(
                error_response,
            ));
        }
        if result.workflow_id != job.workflow_id {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!(
                    "ResultModel workflow_id {} does not match job's workflow_id {}",
                    result.workflow_id, job.workflow_id
                )
            }));
            return Ok(CompleteJobResponse::UnprocessableContentErrorResponse(
                error_response,
            ));
        }

        job.status = Some(status);

        match sqlx::query!(
            "UPDATE job_internal SET active_compute_node_id = NULL WHERE job_id = ?",
            id
        )
        .execute(self.pool.as_ref())
        .await
        {
            Ok(_) => {
                debug!("Cleared active_compute_node_id for job_id={}", id);
            }
            Err(e) => {
                error!(
                    "Failed to clear active_compute_node_id for job_id={}: {}",
                    id, e
                );
            }
        }

        let result_return_code = result.return_code;
        let result_response = self.results_api.create_result(result, context).await?;

        let result_id = match result_response {
            CreateResultResponse::SuccessfulResponse(result) => {
                debug!(
                    "complete_job: added result with ID {:?} for job_id={}",
                    result.id, id
                );
                result.id
            }
            CreateResultResponse::ForbiddenErrorResponse(err) => {
                error!("Forbidden to add result for job {}: {:?}", id, err);
                return Ok(CompleteJobResponse::ForbiddenErrorResponse(err));
            }
            CreateResultResponse::NotFoundErrorResponse(err) => {
                error!("Failed to add result for job {}: {:?}", id, err);
                return Ok(CompleteJobResponse::NotFoundErrorResponse(err));
            }
            CreateResultResponse::DefaultErrorResponse(err) => {
                error!("Failed to add result for job {}: {:?}", id, err);
                return Ok(CompleteJobResponse::DefaultErrorResponse(err));
            }
        };

        let workflow_id = job.workflow_id;
        let result_id_value = result_id.ok_or_else(|| {
            error!("Result ID is missing after creating result");
            ApiError("Result ID is missing".to_string())
        })?;

        match sqlx::query!(
            r#"
            INSERT OR REPLACE INTO workflow_result (workflow_id, job_id, result_id)
            VALUES (?, ?, ?)
            "#,
            workflow_id,
            id,
            result_id_value
        )
        .execute(self.pool.as_ref())
        .await
        {
            Ok(_) => {
                debug!(
                    "complete_job: added workflow_result record for workflow_id={}, job_id={}, result_id={}",
                    workflow_id, id, result_id_value
                );
            }
            Err(e) => {
                error!(
                    "Failed to insert workflow_result for workflow_id={}, job_id={}, result_id={}: {}",
                    workflow_id, id, result_id_value, e
                );
                return Err(ApiError("Database error".to_string()));
            }
        }

        self.manage_job_status_change(&job, run_id).await?;

        let event_type = format!("job_{}", status.to_string().to_lowercase());
        let severity = match status {
            models::JobStatus::Completed => models::EventSeverity::Info,
            models::JobStatus::Failed => models::EventSeverity::Error,
            models::JobStatus::Terminated | models::JobStatus::Canceled => {
                models::EventSeverity::Warning
            }
            _ => models::EventSeverity::Info,
        };
        self.event_broadcaster.broadcast(BroadcastEvent {
            workflow_id: job.workflow_id,
            timestamp: chrono::Utc::now().timestamp_millis(),
            event_type,
            severity,
            data: serde_json::json!({
                "job_id": id,
                "job_name": job.name,
                "status": status.to_string(),
                "return_code": result_return_code,
            }),
        });
        debug!("Broadcast job completion event for job_id={}", id);

        debug!(
            "complete_job: successfully completed job_id={} with status={}, result_id={:?}",
            id, status, result_id
        );

        if let Err(e) = self
            .workflow_actions_api
            .check_and_trigger_actions(workflow_id, "on_jobs_complete", Some(vec![id]))
            .await
        {
            error!(
                "Failed to check_and_trigger_actions for on_jobs_complete: {}",
                e
            );
        }

        match self
            .workflows_api
            .is_workflow_complete(workflow_id, context)
            .await
        {
            Ok(response) => {
                if let IsWorkflowCompleteResponse::SuccessfulResponse(completion_status) = response
                    && completion_status.is_complete
                {
                    debug!(
                        "Workflow {} is complete, triggering on_workflow_complete actions",
                        workflow_id
                    );
                    if let Err(e) = self
                        .workflow_actions_api
                        .check_and_trigger_actions(workflow_id, "on_workflow_complete", None)
                        .await
                    {
                        error!(
                            "Failed to check_and_trigger_actions for on_workflow_complete: {}",
                            e
                        );
                    }
                }
            }
            Err(e) => {
                error!(
                    "Failed to check if workflow {} is complete: {}",
                    workflow_id, e
                );
            }
        }

        Ok(CompleteJobResponse::SuccessfulResponse(job))
    }

    pub(super) async fn transport_prepare_ready_jobs(
        &self,
        workflow_id: i64,
        resources: models::ComputeNodesResources,
        limit: i64,
        strict_scheduler_match: Option<bool>,
        context: &C,
    ) -> Result<ClaimJobsBasedOnResources, ApiError> {
        let strict_scheduler_match = strict_scheduler_match.unwrap_or(false);
        let claim_started = Instant::now();

        debug!(
            "get_ready_jobs: workflow_id={}, limit={}, resources={:?} - X-Span-ID: {:?}",
            workflow_id,
            limit,
            resources,
            Has::<XSpanIdString>::get(context).0.clone()
        );

        let mut conn = self.pool.acquire().await.map_err(|e| {
            error!("Failed to acquire database connection: {}", e);
            ApiError("Database connection error".to_string())
        })?;

        let workflow_record =
            sqlx::query("SELECT id, execution_config FROM workflow WHERE id = $1")
                .bind(workflow_id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|e| {
                    error!("Database error checking workflow existence: {}", e);
                    ApiError("Database error".to_string())
                })?;

        let Some(workflow_record) = workflow_record else {
            let error_response = models::ErrorResponse::new(serde_json::json!({
                "message": format!("Workflow not found with ID: {}", workflow_id)
            }));
            return Ok(ClaimJobsBasedOnResources::NotFoundErrorResponse(
                error_response,
            ));
        };

        let selection_limit =
            usize::try_from(limit).map_err(|_| ApiError("Invalid limit".to_string()))?;

        let downstream_buffer_multiplier = workflow_record
            .get::<Option<String>, _>("execution_config")
            .as_deref()
            .and_then(|json| {
                serde_json::from_str::<crate::client::workflow_spec::ExecutionConfig>(json).ok()
            })
            .and_then(|config| config.downstream_buffer_multiplier());

        let time_limit_seconds = if let Some(ref time_limit) = resources.time_limit {
            match duration_string_to_seconds(time_limit) {
                Ok(seconds) => seconds,
                Err(e) => {
                    let error_response = models::ErrorResponse::new(serde_json::json!({
                        "message": format!("Invalid time_limit format '{}': {}", time_limit, e),
                        "field": "time_limit",
                        "value": time_limit
                    }));
                    return Ok(
                        ClaimJobsBasedOnResources::UnprocessableContentErrorResponse(
                            error_response,
                        ),
                    );
                }
            }
        } else {
            i64::MAX
        };

        if selection_limit == 0 {
            debug!(
                "get_ready_jobs: workflow_id={} read_candidates=0 overfetch_multiplier=1 write_candidates=0 lost_to_race=0 lock_hold_ms=0 total_ms={}",
                workflow_id,
                claim_started.elapsed().as_millis()
            );
            return Ok(ClaimJobsBasedOnResources::SuccessfulResponse(
                models::ClaimJobsBasedOnResources {
                    jobs: Some(Vec::new()),
                    reason: None,
                },
            ));
        }

        let memory_bytes = (resources.memory_gb * 1024.0 * 1024.0 * 1024.0) as i64;
        let overfetch_multiplier = claim_overfetch_multiplier(selection_limit);
        let buffered_target = selection_limit.saturating_mul(overfetch_multiplier);

        let base_spec = ClaimSelectionSpec {
            workflow_id,
            selection_limit,
            scheduler_mode: ClaimSchedulerMode::Scoped,
            scheduler_config_id: resources.scheduler_config_id,
            requested_memory_bytes: memory_bytes,
            requested_num_cpus: resources.num_cpus,
            requested_num_gpus: resources.num_gpus,
            requested_num_nodes: resources.num_nodes,
            total_nodes_for_fit: resources.num_nodes.max(1),
            time_limit_seconds,
            downstream_buffer_multiplier,
        };

        let read_phase_started = Instant::now();
        let (mut read_state, read_selection_stats, scoped_query_stats) =
            collect_phase_candidates(&mut conn, &base_spec, buffered_target).await?;
        let mut read_scheduler_mode = ClaimSchedulerMode::Scoped;

        debug!(
            "get_ready_jobs read phase: workflow_id={} scheduler_mode={:?} pages={} rows_scanned={} read_candidates={} target_candidates={} skipped_for_fit={} skipped_for_downstream_buffer={}",
            workflow_id,
            read_scheduler_mode,
            scoped_query_stats.pages_scanned,
            scoped_query_stats.rows_scanned,
            read_state.selected_candidates.len(),
            buffered_target,
            read_selection_stats.skipped_for_fit,
            read_selection_stats.skipped_for_downstream_buffer
        );

        if scoped_query_stats.rows_scanned == 0 && !strict_scheduler_match {
            let fallback_spec = ClaimSelectionSpec {
                scheduler_mode: ClaimSchedulerMode::Fallback,
                ..base_spec.clone()
            };
            let (fallback_state, fallback_selection_stats, fallback_query_stats) =
                collect_phase_candidates(&mut conn, &fallback_spec, buffered_target).await?;

            if fallback_query_stats.rows_scanned > 0 {
                info!(
                    "Worker with scheduler_config_id={:?} found {} ready jobs after removing scheduler filter (strict_scheduler_match=false).",
                    resources.scheduler_config_id, fallback_query_stats.rows_scanned
                );
            }

            debug!(
                "get_ready_jobs read phase: workflow_id={} scheduler_mode={:?} pages={} rows_scanned={} read_candidates={} target_candidates={} skipped_for_fit={} skipped_for_downstream_buffer={}",
                workflow_id,
                ClaimSchedulerMode::Fallback,
                fallback_query_stats.pages_scanned,
                fallback_query_stats.rows_scanned,
                fallback_state.selected_candidates.len(),
                buffered_target,
                fallback_selection_stats.skipped_for_fit,
                fallback_selection_stats.skipped_for_downstream_buffer
            );

            read_state = fallback_state;
            read_scheduler_mode = ClaimSchedulerMode::Fallback;
        }

        let buffered_candidate_ids = read_state.selected_job_ids();
        let read_duration = read_phase_started.elapsed();

        begin_immediate_transaction(&mut conn).await.map_err(|e| {
            error!("Failed to begin immediate transaction: {}", e);
            ApiError("Database lock error".to_string())
        })?;

        let lock_started = Instant::now();
        let write_result: Result<(Vec<ClaimJobCandidate>, u128), ApiError> = async {
            let recheck_spec = ClaimSelectionSpec {
                scheduler_mode: read_scheduler_mode,
                downstream_buffer_multiplier: None,
                ..base_spec.clone()
            };

            let write_candidates =
                fetch_claim_candidates_by_ids(&mut conn, workflow_id, &buffered_candidate_ids)
                    .await?;
            let fetched_candidate_ids = write_candidates
                .iter()
                .map(|candidate| candidate.job_id)
                .collect::<HashSet<_>>();
            let missing_candidates = buffered_candidate_ids
                .iter()
                .filter(|job_id| !fetched_candidate_ids.contains(job_id))
                .count();

            let mut write_state = ClaimSelectionState::default();
            let mut write_stats = ClaimSelectionStats::default();
            process_candidate_batch(
                &recheck_spec,
                write_candidates,
                recheck_spec.selection_limit,
                &mut write_state,
                &mut write_stats,
            )?;

            let claimed_job_ids = write_state.selected_job_ids();
            update_claimed_jobs_to_pending(&mut conn, &claimed_job_ids).await?;

            sqlx::query("COMMIT")
                .execute(&mut *conn)
                .await
                .map_err(|e| {
                    error!("Failed to commit transaction: {}", e);
                    ApiError("Database commit error".to_string())
                })?;

            let jobs_lost_to_race = missing_candidates + write_stats.skipped_for_status;
            debug!(
                "get_ready_jobs write phase: workflow_id={} write_candidates={} claimed={} lost_to_race={} skipped_for_scheduler={} skipped_for_request_limits={} skipped_for_fit={} skipped_for_downstream_buffer={} lock_hold_ms={}",
                workflow_id,
                fetched_candidate_ids.len(),
                claimed_job_ids.len(),
                jobs_lost_to_race,
                write_stats.skipped_for_scheduler,
                write_stats.skipped_for_request_limits,
                write_stats.skipped_for_fit,
                write_stats.skipped_for_downstream_buffer,
                lock_started.elapsed().as_millis()
            );

            Ok((write_state.selected_candidates, lock_started.elapsed().as_millis()))
        }
        .await;

        if write_result.is_err() {
            rollback_immediate_transaction(&mut conn, "prepare_ready_jobs error exit").await;
        }

        let (claimed_candidates, lock_hold_ms) = write_result?;
        let mut selected_jobs = claimed_candidates
            .iter()
            .map(ClaimJobCandidate::to_job_model)
            .collect::<Result<Vec<_>, _>>()?;

        if let Err(e) =
            hydrate_claimed_job_outputs(&mut conn, workflow_id, &mut selected_jobs).await
        {
            warn!(
                "Failed to hydrate claimed job outputs after commit for workflow {}: {}",
                workflow_id, e
            );
        }

        debug!(
            "get_ready_jobs: workflow_id={} read_candidates={} overfetch_multiplier={} claimed={} read_ms={} lock_hold_ms={} total_ms={}",
            workflow_id,
            buffered_candidate_ids.len(),
            overfetch_multiplier,
            selected_jobs.len(),
            read_duration.as_millis(),
            lock_hold_ms,
            claim_started.elapsed().as_millis()
        );

        Ok(ClaimJobsBasedOnResources::SuccessfulResponse(
            models::ClaimJobsBasedOnResources {
                jobs: Some(selected_jobs),
                reason: None,
            },
        ))
    }
}
