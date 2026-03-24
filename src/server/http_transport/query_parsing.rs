#![allow(dead_code)]

use super::*;

#[derive(Debug, PartialEq)]
pub(super) struct DeleteComputeNodesQuery {
    pub(super) workflow_id: i64,
}

#[derive(Debug, PartialEq)]
pub(super) struct ComputeNodesQuery {
    pub(super) workflow_id: i64,
    pub(super) offset: Option<i64>,
    pub(super) limit: Option<i64>,
    pub(super) sort_by: Option<String>,
    pub(super) reverse_sort: Option<bool>,
    pub(super) hostname: Option<String>,
    pub(super) is_active: Option<bool>,
    pub(super) scheduled_compute_node_id: Option<i64>,
}

#[derive(Debug, PartialEq)]
pub(super) struct EventsQuery {
    pub(super) workflow_id: i64,
    pub(super) offset: Option<i64>,
    pub(super) limit: Option<i64>,
    pub(super) sort_by: Option<String>,
    pub(super) reverse_sort: Option<bool>,
    pub(super) category: Option<String>,
    pub(super) after_timestamp: Option<i64>,
}

#[derive(Debug, PartialEq)]
pub(super) struct FilesQuery {
    pub(super) workflow_id: i64,
    pub(super) produced_by_job_id: Option<i64>,
    pub(super) offset: Option<i64>,
    pub(super) limit: Option<i64>,
    pub(super) sort_by: Option<String>,
    pub(super) reverse_sort: Option<bool>,
    pub(super) name: Option<String>,
    pub(super) path: Option<String>,
    pub(super) is_output: Option<bool>,
}

#[derive(Debug, PartialEq)]
pub(super) struct LocalSchedulersQuery {
    pub(super) workflow_id: i64,
    pub(super) offset: Option<i64>,
    pub(super) limit: Option<i64>,
    pub(super) sort_by: Option<String>,
    pub(super) reverse_sort: Option<bool>,
    pub(super) memory: Option<String>,
    pub(super) num_cpus: Option<i64>,
}

#[derive(Debug, PartialEq)]
pub(super) struct ResultsQuery {
    pub(super) workflow_id: i64,
    pub(super) job_id: Option<i64>,
    pub(super) run_id: Option<i64>,
    pub(super) return_code: Option<i64>,
    pub(super) status: Option<models::JobStatus>,
    pub(super) compute_node_id: Option<i64>,
    pub(super) offset: Option<i64>,
    pub(super) limit: Option<i64>,
    pub(super) sort_by: Option<String>,
    pub(super) reverse_sort: Option<bool>,
    pub(super) all_runs: Option<bool>,
}

#[derive(Debug, PartialEq)]
pub(super) struct UserDataQuery {
    pub(super) workflow_id: i64,
    pub(super) consumer_job_id: Option<i64>,
    pub(super) producer_job_id: Option<i64>,
    pub(super) offset: Option<i64>,
    pub(super) limit: Option<i64>,
    pub(super) sort_by: Option<String>,
    pub(super) reverse_sort: Option<bool>,
    pub(super) name: Option<String>,
    pub(super) is_ephemeral: Option<bool>,
}

#[derive(Debug, PartialEq)]
pub(super) struct UserDataCreateQuery {
    pub(super) consumer_job_id: Option<i64>,
    pub(super) producer_job_id: Option<i64>,
}

#[derive(Debug, PartialEq)]
pub(super) struct ScheduledComputeNodesQuery {
    pub(super) workflow_id: i64,
    pub(super) offset: Option<i64>,
    pub(super) limit: Option<i64>,
    pub(super) sort_by: Option<String>,
    pub(super) reverse_sort: Option<bool>,
    pub(super) scheduler_id: Option<String>,
    pub(super) scheduler_config_id: Option<String>,
    pub(super) status: Option<String>,
}

#[derive(Debug, PartialEq)]
pub(super) struct SlurmSchedulersQuery {
    pub(super) workflow_id: i64,
    pub(super) offset: Option<i64>,
    pub(super) limit: Option<i64>,
    pub(super) sort_by: Option<String>,
    pub(super) reverse_sort: Option<bool>,
}

#[derive(Debug, PartialEq)]
pub(super) struct AccessPaginationQuery {
    pub(super) offset: Option<i64>,
    pub(super) limit: Option<i64>,
}

#[derive(Debug, PartialEq)]
pub(super) struct ResourceRequirementsQuery {
    pub(super) workflow_id: i64,
    pub(super) job_id: Option<i64>,
    pub(super) name: Option<String>,
    pub(super) memory: Option<String>,
    pub(super) num_cpus: Option<i64>,
    pub(super) num_gpus: Option<i64>,
    pub(super) num_nodes: Option<i64>,
    pub(super) runtime: Option<i64>,
    pub(super) offset: Option<i64>,
    pub(super) limit: Option<i64>,
    pub(super) sort_by: Option<String>,
    pub(super) reverse_sort: Option<bool>,
}

#[derive(Debug, PartialEq)]
pub(super) struct SlurmStatsQuery {
    pub(super) workflow_id: i64,
    pub(super) job_id: Option<i64>,
    pub(super) run_id: Option<i64>,
    pub(super) attempt_id: Option<i64>,
    pub(super) offset: Option<i64>,
    pub(super) limit: Option<i64>,
}

#[derive(Debug, PartialEq)]
pub(super) struct WorkflowsQuery {
    pub(super) offset: Option<i64>,
    pub(super) sort_by: Option<String>,
    pub(super) reverse_sort: Option<bool>,
    pub(super) limit: Option<i64>,
    pub(super) name: Option<String>,
    pub(super) user: Option<String>,
    pub(super) description: Option<String>,
    pub(super) is_archived: Option<bool>,
}

#[derive(Debug, PartialEq)]
pub(super) struct WorkflowRelationshipsQuery {
    pub(super) offset: Option<i64>,
    pub(super) limit: Option<i64>,
    pub(super) sort_by: Option<String>,
    pub(super) reverse_sort: Option<bool>,
}

#[derive(Debug, PartialEq)]
pub(super) struct PendingActionsQuery {
    pub(super) trigger_type: Option<Vec<String>>,
}

#[derive(Debug, PartialEq)]
pub(super) struct InitializeJobsQuery {
    pub(super) only_uninitialized: Option<bool>,
    pub(super) clear_ephemeral_user_data: Option<bool>,
}

#[derive(Debug, PartialEq)]
pub(super) struct ClaimJobsBasedOnResourcesQuery {
    pub(super) strict_scheduler_match: Option<bool>,
}

#[derive(Debug, PartialEq)]
pub(super) struct ClaimNextJobsQuery {
    pub(super) limit: Option<i64>,
}

#[derive(Debug, PartialEq)]
pub(super) struct ProcessChangedJobInputsQuery {
    pub(super) dry_run: Option<bool>,
}

#[derive(Debug, PartialEq)]
pub(super) struct GetReadyJobRequirementsQuery {
    pub(super) scheduler_config_id: Option<i64>,
}

#[derive(Debug, PartialEq)]
pub(super) struct ResetJobStatusQuery {
    pub(super) failed_only: Option<bool>,
}

#[derive(Debug, PartialEq)]
pub(super) struct ResetWorkflowStatusQuery {
    pub(super) force: Option<bool>,
}

pub(super) fn parse_delete_compute_nodes_query(
    query: Option<&str>,
) -> Result<DeleteComputeNodesQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(DeleteComputeNodesQuery {
        workflow_id: parse_required_i64(&params, "workflow_id")?,
    })
}

pub(super) fn parse_compute_nodes_query(query: Option<&str>) -> Result<ComputeNodesQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();

    Ok(ComputeNodesQuery {
        workflow_id: parse_required_i64(&params, "workflow_id")?,
        offset: parse_optional_i64(&params, "offset")?,
        limit: parse_optional_i64(&params, "limit")?,
        sort_by: params.get("sort_by").cloned(),
        reverse_sort: parse_optional_bool(&params, "reverse_sort")?,
        hostname: params.get("hostname").cloned(),
        is_active: parse_optional_bool(&params, "is_active")?,
        scheduled_compute_node_id: parse_optional_i64(&params, "scheduled_compute_node_id")?,
    })
}

pub(super) fn parse_events_query(query: Option<&str>) -> Result<EventsQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();

    let workflow_id = parse_required_i64(&params, "workflow_id")?;
    Ok(EventsQuery {
        workflow_id,
        offset: parse_optional_i64(&params, "offset")?,
        limit: parse_optional_i64(&params, "limit")?,
        sort_by: params.get("sort_by").cloned(),
        reverse_sort: parse_optional_bool(&params, "reverse_sort")?,
        category: params.get("category").cloned(),
        after_timestamp: parse_optional_i64(&params, "after_timestamp")?,
    })
}

pub(super) fn parse_files_query(query: Option<&str>) -> Result<FilesQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();

    let workflow_id = parse_required_i64(&params, "workflow_id")?;
    Ok(FilesQuery {
        workflow_id,
        produced_by_job_id: parse_optional_i64(&params, "produced_by_job_id")?,
        offset: parse_optional_i64(&params, "offset")?,
        limit: parse_optional_i64(&params, "limit")?,
        sort_by: params.get("sort_by").cloned(),
        reverse_sort: parse_optional_bool(&params, "reverse_sort")?,
        name: params.get("name").cloned(),
        path: params.get("path").cloned(),
        is_output: parse_optional_bool(&params, "is_output")?,
    })
}

pub(super) fn parse_local_schedulers_query(
    query: Option<&str>,
) -> Result<LocalSchedulersQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();

    let workflow_id = parse_required_i64(&params, "workflow_id")?;
    Ok(LocalSchedulersQuery {
        workflow_id,
        offset: parse_optional_i64(&params, "offset")?,
        limit: parse_optional_i64(&params, "limit")?,
        sort_by: params.get("sort_by").cloned(),
        reverse_sort: parse_optional_bool(&params, "reverse_sort")?,
        memory: params.get("memory").cloned(),
        num_cpus: parse_optional_i64(&params, "num_cpus")?,
    })
}

pub(super) fn parse_results_query(query: Option<&str>) -> Result<ResultsQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();

    let workflow_id = parse_required_i64(&params, "workflow_id")?;
    Ok(ResultsQuery {
        workflow_id,
        job_id: parse_optional_i64(&params, "job_id")?,
        run_id: parse_optional_i64(&params, "run_id")?,
        return_code: parse_optional_i64(&params, "return_code")?,
        status: parse_optional_job_status_name(&params, "status")?,
        compute_node_id: parse_optional_i64(&params, "compute_node_id")?,
        offset: parse_optional_i64(&params, "offset")?,
        limit: parse_optional_i64(&params, "limit")?,
        sort_by: params.get("sort_by").cloned(),
        reverse_sort: parse_optional_bool(&params, "reverse_sort")?,
        all_runs: parse_optional_bool(&params, "all_runs")?,
    })
}

pub(super) fn parse_user_data_query(query: Option<&str>) -> Result<UserDataQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();

    let workflow_id = parse_required_i64(&params, "workflow_id")?;
    Ok(UserDataQuery {
        workflow_id,
        consumer_job_id: parse_optional_i64(&params, "consumer_job_id")?,
        producer_job_id: parse_optional_i64(&params, "producer_job_id")?,
        offset: parse_optional_i64(&params, "offset")?,
        limit: parse_optional_i64(&params, "limit")?,
        sort_by: params.get("sort_by").cloned(),
        reverse_sort: parse_optional_bool(&params, "reverse_sort")?,
        name: params.get("name").cloned(),
        is_ephemeral: parse_optional_bool(&params, "is_ephemeral")?,
    })
}

pub(super) fn parse_user_data_create_query(
    query: Option<&str>,
) -> Result<UserDataCreateQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(UserDataCreateQuery {
        consumer_job_id: parse_optional_i64(&params, "consumer_job_id")?,
        producer_job_id: parse_optional_i64(&params, "producer_job_id")?,
    })
}

pub(super) fn parse_scheduled_compute_nodes_query(
    query: Option<&str>,
) -> Result<ScheduledComputeNodesQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(ScheduledComputeNodesQuery {
        workflow_id: parse_required_i64(&params, "workflow_id")?,
        offset: parse_optional_i64(&params, "offset")?,
        limit: parse_optional_i64(&params, "limit")?,
        sort_by: params.get("sort_by").cloned(),
        reverse_sort: parse_optional_bool(&params, "reverse_sort")?,
        scheduler_id: params.get("scheduler_id").cloned(),
        scheduler_config_id: params.get("scheduler_config_id").cloned(),
        status: params.get("status").cloned(),
    })
}

pub(super) fn parse_slurm_schedulers_query(
    query: Option<&str>,
) -> Result<SlurmSchedulersQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(SlurmSchedulersQuery {
        workflow_id: parse_required_i64(&params, "workflow_id")?,
        offset: parse_optional_i64(&params, "offset")?,
        limit: parse_optional_i64(&params, "limit")?,
        sort_by: params.get("sort_by").cloned(),
        reverse_sort: parse_optional_bool(&params, "reverse_sort")?,
    })
}

pub(super) fn parse_access_pagination_query(
    query: Option<&str>,
) -> Result<AccessPaginationQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(AccessPaginationQuery {
        offset: parse_optional_i64(&params, "offset")?,
        limit: parse_optional_i64(&params, "limit")?,
    })
}

pub(super) fn parse_resource_requirements_query(
    query: Option<&str>,
) -> Result<ResourceRequirementsQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(ResourceRequirementsQuery {
        workflow_id: parse_required_i64(&params, "workflow_id")?,
        job_id: parse_optional_i64(&params, "job_id")?,
        name: params.get("name").cloned(),
        memory: params.get("memory").cloned(),
        num_cpus: parse_optional_i64(&params, "num_cpus")?,
        num_gpus: parse_optional_i64(&params, "num_gpus")?,
        num_nodes: parse_optional_i64(&params, "num_nodes")?,
        runtime: parse_optional_i64(&params, "runtime")?,
        offset: parse_optional_i64(&params, "offset")?,
        limit: parse_optional_i64(&params, "limit")?,
        sort_by: params.get("sort_by").cloned(),
        reverse_sort: parse_optional_bool(&params, "reverse_sort")?,
    })
}

pub(super) fn parse_slurm_stats_query(query: Option<&str>) -> Result<SlurmStatsQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(SlurmStatsQuery {
        workflow_id: parse_required_i64(&params, "workflow_id")?,
        job_id: parse_optional_i64(&params, "job_id")?,
        run_id: parse_optional_i64(&params, "run_id")?,
        attempt_id: parse_optional_i64(&params, "attempt_id")?,
        offset: parse_optional_i64(&params, "offset")?,
        limit: parse_optional_i64(&params, "limit")?,
    })
}

pub(super) fn parse_workflows_query(query: Option<&str>) -> Result<WorkflowsQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(WorkflowsQuery {
        offset: parse_optional_i64(&params, "offset")?,
        sort_by: params.get("sort_by").cloned(),
        reverse_sort: parse_optional_bool(&params, "reverse_sort")?,
        limit: parse_optional_i64(&params, "limit")?,
        name: params.get("name").cloned(),
        user: params.get("user").cloned(),
        description: params.get("description").cloned(),
        is_archived: parse_optional_bool(&params, "is_archived")?,
    })
}

pub(super) fn parse_workflow_relationships_query(
    query: Option<&str>,
) -> Result<WorkflowRelationshipsQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(WorkflowRelationshipsQuery {
        offset: parse_optional_i64(&params, "offset")?,
        limit: parse_optional_i64(&params, "limit")?,
        sort_by: params.get("sort_by").cloned(),
        reverse_sort: parse_optional_bool(&params, "reverse_sort")?,
    })
}

pub(super) fn parse_pending_actions_query(
    query: Option<&str>,
) -> Result<PendingActionsQuery, String> {
    let pairs: Vec<(String, String)> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    let trigger_type: Vec<String> = pairs
        .into_iter()
        .filter_map(|(key, value)| (key == "trigger_type").then_some(value))
        .collect();
    Ok(PendingActionsQuery {
        trigger_type: (!trigger_type.is_empty()).then_some(trigger_type),
    })
}

pub(super) fn parse_initialize_jobs_query(
    query: Option<&str>,
) -> Result<InitializeJobsQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(InitializeJobsQuery {
        only_uninitialized: parse_optional_bool(&params, "only_uninitialized")?,
        clear_ephemeral_user_data: parse_optional_bool(&params, "clear_ephemeral_user_data")?,
    })
}

pub(super) fn parse_claim_jobs_based_on_resources_query(
    query: Option<&str>,
) -> Result<ClaimJobsBasedOnResourcesQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(ClaimJobsBasedOnResourcesQuery {
        strict_scheduler_match: parse_optional_bool(&params, "strict_scheduler_match")?,
    })
}

pub(super) fn parse_claim_next_jobs_query(
    query: Option<&str>,
) -> Result<ClaimNextJobsQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(ClaimNextJobsQuery {
        limit: parse_optional_i64(&params, "limit")?,
    })
}

pub(super) fn parse_process_changed_job_inputs_query(
    query: Option<&str>,
) -> Result<ProcessChangedJobInputsQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(ProcessChangedJobInputsQuery {
        dry_run: parse_optional_bool(&params, "dry_run")?,
    })
}

pub(super) fn parse_get_ready_job_requirements_query(
    query: Option<&str>,
) -> Result<GetReadyJobRequirementsQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(GetReadyJobRequirementsQuery {
        scheduler_config_id: parse_optional_i64(&params, "scheduler_config_id")?,
    })
}

pub(super) fn parse_reset_job_status_query(
    query: Option<&str>,
) -> Result<ResetJobStatusQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(ResetJobStatusQuery {
        failed_only: parse_optional_bool(&params, "failed_only")?,
    })
}

pub(super) fn parse_reset_workflow_status_query(
    query: Option<&str>,
) -> Result<ResetWorkflowStatusQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(ResetWorkflowStatusQuery {
        force: parse_optional_bool(&params, "force")?,
    })
}

pub(super) fn parse_required_i64(
    params: &HashMap<String, String>,
    key: &str,
) -> Result<i64, String> {
    let raw = params
        .get(key)
        .ok_or_else(|| format!("Missing required query parameter: {key}"))?;
    raw.parse::<i64>()
        .map_err(|_| format!("Invalid integer for query parameter: {key}"))
}

pub(super) fn parse_optional_i64(
    params: &HashMap<String, String>,
    key: &str,
) -> Result<Option<i64>, String> {
    params
        .get(key)
        .map(|raw| {
            raw.parse::<i64>()
                .map_err(|_| format!("Invalid integer for query parameter: {key}"))
        })
        .transpose()
}

pub(super) fn parse_optional_bool(
    params: &HashMap<String, String>,
    key: &str,
) -> Result<Option<bool>, String> {
    params
        .get(key)
        .map(|raw| {
            raw.parse::<bool>()
                .map_err(|_| format!("Invalid boolean for query parameter: {key}"))
        })
        .transpose()
}

pub(super) fn parse_optional_job_status_name(
    params: &HashMap<String, String>,
    key: &str,
) -> Result<Option<models::JobStatus>, String> {
    params
        .get(key)
        .map(|raw| {
            raw.parse::<models::JobStatus>()
                .map_err(|_| format!("Invalid job status for query parameter: {key}"))
        })
        .transpose()
}

pub(super) fn parse_event_stream_level(query: Option<&str>) -> models::EventSeverity {
    query
        .and_then(|query| {
            form_urlencoded::parse(query.as_bytes())
                .find(|(key, _)| key == "level")
                .map(|(_, value)| value.into_owned())
        })
        .and_then(|value| value.parse::<models::EventSeverity>().ok())
        .unwrap_or(models::EventSeverity::Info)
}
