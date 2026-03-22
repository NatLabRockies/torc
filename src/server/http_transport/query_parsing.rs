#[derive(Debug, PartialEq)]
struct ComputeNodesQuery {
    workflow_id: i64,
    offset: Option<i64>,
    limit: Option<i64>,
    sort_by: Option<String>,
    reverse_sort: Option<bool>,
    hostname: Option<String>,
    is_active: Option<bool>,
    scheduled_compute_node_id: Option<i64>,
}

#[derive(Debug, PartialEq)]
struct DeleteComputeNodesQuery {
    workflow_id: i64,
}

#[derive(Debug, PartialEq)]
struct EventsQuery {
    workflow_id: i64,
    offset: Option<i64>,
    limit: Option<i64>,
    sort_by: Option<String>,
    reverse_sort: Option<bool>,
    category: Option<String>,
    after_timestamp: Option<i64>,
}

#[derive(Debug, PartialEq)]
struct FilesQuery {
    workflow_id: i64,
    produced_by_job_id: Option<i64>,
    offset: Option<i64>,
    limit: Option<i64>,
    sort_by: Option<String>,
    reverse_sort: Option<bool>,
    name: Option<String>,
    path: Option<String>,
    is_output: Option<bool>,
}

#[derive(Debug, PartialEq)]
struct LocalSchedulersQuery {
    workflow_id: i64,
    offset: Option<i64>,
    limit: Option<i64>,
    sort_by: Option<String>,
    reverse_sort: Option<bool>,
    memory: Option<String>,
    num_cpus: Option<i64>,
}

#[derive(Debug, PartialEq)]
struct ResultsQuery {
    workflow_id: i64,
    job_id: Option<i64>,
    run_id: Option<i64>,
    return_code: Option<i64>,
    status: Option<models::JobStatus>,
    compute_node_id: Option<i64>,
    offset: Option<i64>,
    limit: Option<i64>,
    sort_by: Option<String>,
    reverse_sort: Option<bool>,
    all_runs: Option<bool>,
}

#[derive(Debug, PartialEq)]
struct UserDataQuery {
    workflow_id: i64,
    consumer_job_id: Option<i64>,
    producer_job_id: Option<i64>,
    offset: Option<i64>,
    limit: Option<i64>,
    sort_by: Option<String>,
    reverse_sort: Option<bool>,
    name: Option<String>,
    is_ephemeral: Option<bool>,
}

#[derive(Debug, PartialEq)]
struct UserDataCreateQuery {
    consumer_job_id: Option<i64>,
    producer_job_id: Option<i64>,
}

#[derive(Debug, PartialEq)]
struct ScheduledComputeNodesQuery {
    workflow_id: i64,
    offset: Option<i64>,
    limit: Option<i64>,
    sort_by: Option<String>,
    reverse_sort: Option<bool>,
    scheduler_id: Option<String>,
    scheduler_config_id: Option<String>,
    status: Option<String>,
}

#[derive(Debug, PartialEq)]
struct SlurmSchedulersQuery {
    workflow_id: i64,
    offset: Option<i64>,
    limit: Option<i64>,
    sort_by: Option<String>,
    reverse_sort: Option<bool>,
}

#[derive(Debug, PartialEq)]
struct AccessPaginationQuery {
    offset: Option<i64>,
    limit: Option<i64>,
}

#[derive(Debug, PartialEq)]
struct ResourceRequirementsQuery {
    workflow_id: i64,
    job_id: Option<i64>,
    name: Option<String>,
    memory: Option<String>,
    num_cpus: Option<i64>,
    num_gpus: Option<i64>,
    num_nodes: Option<i64>,
    runtime: Option<i64>,
    offset: Option<i64>,
    limit: Option<i64>,
    sort_by: Option<String>,
    reverse_sort: Option<bool>,
}

#[derive(Debug, PartialEq)]
struct SlurmStatsQuery {
    workflow_id: i64,
    job_id: Option<i64>,
    run_id: Option<i64>,
    attempt_id: Option<i64>,
    offset: Option<i64>,
    limit: Option<i64>,
}

#[derive(Debug, PartialEq)]
struct JobsQuery {
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
}

#[derive(Debug, PartialEq)]
struct WorkflowsQuery {
    offset: Option<i64>,
    sort_by: Option<String>,
    reverse_sort: Option<bool>,
    limit: Option<i64>,
    name: Option<String>,
    user: Option<String>,
    description: Option<String>,
    is_archived: Option<bool>,
}

#[derive(Debug, PartialEq)]
struct WorkflowRelationshipsQuery {
    offset: Option<i64>,
    limit: Option<i64>,
}

#[derive(Debug, PartialEq)]
struct PendingActionsQuery {
    trigger_type: Option<Vec<String>>,
}

#[derive(Debug, PartialEq)]
struct InitializeJobsQuery {
    only_uninitialized: Option<bool>,
    clear_ephemeral_user_data: Option<bool>,
}

#[derive(Debug, PartialEq)]
struct ClaimJobsBasedOnResourcesQuery {
    sort_method: Option<models::ClaimJobsSortMethod>,
    strict_scheduler_match: Option<bool>,
}

#[derive(Debug, PartialEq)]
struct ClaimNextJobsQuery {
    limit: Option<i64>,
}

#[derive(Debug, PartialEq)]
struct ProcessChangedJobInputsQuery {
    dry_run: Option<bool>,
}

#[derive(Debug, PartialEq)]
struct GetReadyJobRequirementsQuery {
    scheduler_config_id: Option<i64>,
}

#[derive(Debug, PartialEq)]
struct ResetJobStatusQuery {
    failed_only: Option<bool>,
}

#[derive(Debug, PartialEq)]
struct ResetWorkflowStatusQuery {
    force: Option<bool>,
}

#[derive(Debug, PartialEq)]
struct RetryJobQuery {
    max_retries: i32,
}

fn parse_compute_nodes_query(query: Option<&str>) -> Result<ComputeNodesQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();

    let workflow_id = parse_required_i64(&params, "workflow_id")?;
    Ok(ComputeNodesQuery {
        workflow_id,
        offset: parse_optional_i64(&params, "offset")?,
        limit: parse_optional_i64(&params, "limit")?,
        sort_by: params.get("sort_by").cloned(),
        reverse_sort: parse_optional_bool(&params, "reverse_sort")?,
        hostname: params.get("hostname").cloned(),
        is_active: parse_optional_bool(&params, "is_active")?,
        scheduled_compute_node_id: parse_optional_i64(&params, "scheduled_compute_node_id")?,
    })
}

fn parse_delete_compute_nodes_query(
    query: Option<&str>,
) -> Result<DeleteComputeNodesQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(DeleteComputeNodesQuery {
        workflow_id: parse_required_i64(&params, "workflow_id")?,
    })
}

fn parse_events_query(query: Option<&str>) -> Result<EventsQuery, String> {
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

fn parse_files_query(query: Option<&str>) -> Result<FilesQuery, String> {
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

fn parse_local_schedulers_query(query: Option<&str>) -> Result<LocalSchedulersQuery, String> {
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

fn parse_results_query(query: Option<&str>) -> Result<ResultsQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();

    let workflow_id = parse_required_i64(&params, "workflow_id")?;
    Ok(ResultsQuery {
        workflow_id,
        job_id: parse_optional_i64(&params, "job_id")?,
        run_id: parse_optional_i64(&params, "run_id")?,
        return_code: parse_optional_i64(&params, "return_code")?,
        status: parse_optional_job_status(&params, "status")?,
        compute_node_id: parse_optional_i64(&params, "compute_node_id")?,
        offset: parse_optional_i64(&params, "offset")?,
        limit: parse_optional_i64(&params, "limit")?,
        sort_by: params.get("sort_by").cloned(),
        reverse_sort: parse_optional_bool(&params, "reverse_sort")?,
        all_runs: parse_optional_bool(&params, "all_runs")?,
    })
}

fn parse_user_data_query(query: Option<&str>) -> Result<UserDataQuery, String> {
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

fn parse_user_data_create_query(query: Option<&str>) -> Result<UserDataCreateQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(UserDataCreateQuery {
        consumer_job_id: parse_optional_i64(&params, "consumer_job_id")?,
        producer_job_id: parse_optional_i64(&params, "producer_job_id")?,
    })
}

fn parse_scheduled_compute_nodes_query(
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

fn parse_slurm_schedulers_query(query: Option<&str>) -> Result<SlurmSchedulersQuery, String> {
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

fn parse_access_pagination_query(query: Option<&str>) -> Result<AccessPaginationQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(AccessPaginationQuery {
        offset: parse_optional_i64(&params, "offset")?,
        limit: parse_optional_i64(&params, "limit")?,
    })
}

fn parse_resource_requirements_query(
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

fn parse_slurm_stats_query(query: Option<&str>) -> Result<SlurmStatsQuery, String> {
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

fn parse_jobs_query(query: Option<&str>) -> Result<JobsQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(JobsQuery {
        workflow_id: parse_required_i64(&params, "workflow_id")?,
        status: parse_optional_job_status_name(&params, "status")?,
        needs_file_id: parse_optional_i64(&params, "needs_file_id")?,
        upstream_job_id: parse_optional_i64(&params, "upstream_job_id")?,
        offset: parse_optional_i64(&params, "offset")?,
        limit: parse_optional_i64(&params, "limit")?,
        sort_by: params.get("sort_by").cloned(),
        reverse_sort: parse_optional_bool(&params, "reverse_sort")?,
        include_relationships: parse_optional_bool(&params, "include_relationships")?,
        active_compute_node_id: parse_optional_i64(&params, "active_compute_node_id")?,
    })
}

fn parse_workflows_query(query: Option<&str>) -> Result<WorkflowsQuery, String> {
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

fn parse_workflow_relationships_query(
    query: Option<&str>,
) -> Result<WorkflowRelationshipsQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(WorkflowRelationshipsQuery {
        offset: parse_optional_i64(&params, "offset")?,
        limit: parse_optional_i64(&params, "limit")?,
    })
}

fn parse_pending_actions_query(query: Option<&str>) -> Result<PendingActionsQuery, String> {
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

fn parse_initialize_jobs_query(query: Option<&str>) -> Result<InitializeJobsQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(InitializeJobsQuery {
        only_uninitialized: parse_optional_bool(&params, "only_uninitialized")?,
        clear_ephemeral_user_data: parse_optional_bool(&params, "clear_ephemeral_user_data")?,
    })
}

fn parse_claim_jobs_based_on_resources_query(
    query: Option<&str>,
) -> Result<ClaimJobsBasedOnResourcesQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(ClaimJobsBasedOnResourcesQuery {
        sort_method: parse_optional_claim_jobs_sort_method(&params, "sort_method")?,
        strict_scheduler_match: parse_optional_bool(&params, "strict_scheduler_match")?,
    })
}

fn parse_claim_next_jobs_query(query: Option<&str>) -> Result<ClaimNextJobsQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(ClaimNextJobsQuery {
        limit: parse_optional_i64(&params, "limit")?,
    })
}

fn parse_process_changed_job_inputs_query(
    query: Option<&str>,
) -> Result<ProcessChangedJobInputsQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(ProcessChangedJobInputsQuery {
        dry_run: parse_optional_bool(&params, "dry_run")?,
    })
}

fn parse_get_ready_job_requirements_query(
    query: Option<&str>,
) -> Result<GetReadyJobRequirementsQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(GetReadyJobRequirementsQuery {
        scheduler_config_id: parse_optional_i64(&params, "scheduler_config_id")?,
    })
}

fn parse_reset_job_status_query(query: Option<&str>) -> Result<ResetJobStatusQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(ResetJobStatusQuery {
        failed_only: parse_optional_bool(&params, "failed_only")?,
    })
}

fn parse_reset_workflow_status_query(
    query: Option<&str>,
) -> Result<ResetWorkflowStatusQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(ResetWorkflowStatusQuery {
        force: parse_optional_bool(&params, "force")?,
    })
}

fn parse_retry_job_query(query: Option<&str>) -> Result<RetryJobQuery, String> {
    let params: HashMap<String, String> = form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .into_owned()
        .collect();
    Ok(RetryJobQuery {
        max_retries: parse_required_i32(&params, "max_retries")?,
    })
}

fn parse_required_i64(params: &HashMap<String, String>, key: &str) -> Result<i64, String> {
    let raw = params
        .get(key)
        .ok_or_else(|| format!("Missing required query parameter: {key}"))?;
    raw.parse::<i64>()
        .map_err(|_| format!("Invalid integer for query parameter: {key}"))
}

fn parse_optional_i64(params: &HashMap<String, String>, key: &str) -> Result<Option<i64>, String> {
    params
        .get(key)
        .map(|raw| {
            raw.parse::<i64>()
                .map_err(|_| format!("Invalid integer for query parameter: {key}"))
        })
        .transpose()
}

fn parse_optional_bool(
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

fn parse_optional_job_status(
    params: &HashMap<String, String>,
    key: &str,
) -> Result<Option<models::JobStatus>, String> {
    params
        .get(key)
        .map(|raw| {
            raw.parse::<i32>()
                .map_err(|_| format!("Invalid integer for query parameter: {key}"))
                .and_then(models::JobStatus::from_int)
        })
        .transpose()
}

fn parse_optional_job_status_name(
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

fn parse_optional_claim_jobs_sort_method(
    params: &HashMap<String, String>,
    key: &str,
) -> Result<Option<models::ClaimJobsSortMethod>, String> {
    params
        .get(key)
        .map(|raw| {
            raw.parse::<models::ClaimJobsSortMethod>()
                .map_err(|_| format!("Invalid claim jobs sort method for query parameter: {key}"))
        })
        .transpose()
}

fn parse_event_stream_level(query: Option<&str>) -> models::EventSeverity {
    query
        .and_then(|query| {
            form_urlencoded::parse(query.as_bytes())
                .find(|(key, _)| key == "level")
                .map(|(_, value)| value.into_owned())
        })
        .and_then(|value| value.parse::<models::EventSeverity>().ok())
        .unwrap_or(models::EventSeverity::Info)
}

fn is_known_api_path(path: &str) -> bool {
    matches!(
        path,
        "/torc-service/v1/ping"
            | "/torc-service/v1/version"
            | "/torc-service/v1/bulk_jobs"
            | "/torc-service/v1/compute_nodes"
            | "/torc-service/v1/events"
            | "/torc-service/v1/files"
            | "/torc-service/v1/jobs"
            | "/torc-service/v1/local_schedulers"
            | "/torc-service/v1/resource_requirements"
            | "/torc-service/v1/results"
            | "/torc-service/v1/scheduled_compute_nodes"
            | "/torc-service/v1/slurm_schedulers"
            | "/torc-service/v1/user_data"
            | "/torc-service/v1/workflows"
            | "/torc-service/v1/access_groups"
            | "/torc-service/v1/failure_handlers"
            | "/torc-service/v1/ro_crate_entities"
            | "/torc-service/v1/slurm_stats"
            | "/torc-service/v1/admin/reload-auth"
    ) || parse_resource_id(path, "/torc-service/v1/compute_nodes/").is_some()
        || parse_resource_id(path, "/torc-service/v1/events/").is_some()
        || parse_resource_id(path, "/torc-service/v1/files/").is_some()
        || parse_resource_id(path, "/torc-service/v1/jobs/").is_some()
        || parse_resource_id(path, "/torc-service/v1/local_schedulers/").is_some()
        || parse_resource_id(path, "/torc-service/v1/resource_requirements/").is_some()
        || parse_resource_id(path, "/torc-service/v1/results/").is_some()
        || parse_resource_id(path, "/torc-service/v1/scheduled_compute_nodes/").is_some()
        || parse_resource_id(path, "/torc-service/v1/slurm_schedulers/").is_some()
        || parse_resource_id(path, "/torc-service/v1/user_data/").is_some()
        || parse_resource_id(path, "/torc-service/v1/workflows/").is_some()
        || parse_resource_id(path, "/torc-service/v1/access_groups/").is_some()
        || parse_resource_id(path, "/torc-service/v1/failure_handlers/").is_some()
        || parse_resource_id(path, "/torc-service/v1/ro_crate_entities/").is_some()
        || parse_access_group_members_collection_path(path).is_some()
        || parse_group_member_path(path).is_some()
        || parse_user_groups_path(path).is_some()
        || parse_workflow_access_groups_collection_path(path).is_some()
        || parse_workflow_access_group_item_path(path).is_some()
        || parse_access_check_path(path).is_some()
        || parse_workflow_failure_handlers_path(path).is_some()
        || parse_workflow_ro_crate_entities_path(path).is_some()
        || parse_workflow_remote_workers_collection_path(path).is_some()
        || parse_workflow_remote_worker_item_path(path).is_some()
        || parse_workflow_actions_collection_path(path).is_some()
        || parse_workflow_pending_actions_path(path).is_some()
        || parse_workflow_action_claim_path(path).is_some()
        || parse_workflow_claim_jobs_resources_path(path).is_some()
        || parse_workflow_events_stream_path(path).is_some()
        || parse_workflow_dot_graph_path(path).is_some()
        || parse_workflow_suffix_path(path, "/cancel").is_some()
        || parse_workflow_suffix_path(path, "/claim_next_jobs").is_some()
        || parse_workflow_suffix_path(path, "/initialize_jobs").is_some()
        || parse_workflow_suffix_path(path, "/is_complete").is_some()
        || parse_workflow_suffix_path(path, "/is_uninitialized").is_some()
        || parse_workflow_suffix_path(path, "/job_dependencies").is_some()
        || parse_workflow_suffix_path(path, "/job_file_relationships").is_some()
        || parse_workflow_suffix_path(path, "/job_ids").is_some()
        || parse_workflow_suffix_path(path, "/job_user_data_relationships").is_some()
        || parse_workflow_suffix_path(path, "/missing_user_data").is_some()
        || parse_workflow_suffix_path(path, "/process_changed_job_inputs").is_some()
        || parse_workflow_suffix_path(path, "/ready_job_requirements").is_some()
        || parse_workflow_suffix_path(path, "/required_existing_files").is_some()
        || parse_workflow_suffix_path(path, "/reset_job_status").is_some()
        || parse_workflow_suffix_path(path, "/reset_status").is_some()
        || parse_workflow_suffix_path(path, "/status").is_some()
        || parse_job_status_run_path(path, "/torc-service/v1/jobs/", "/complete_job/").is_some()
        || parse_job_status_run_path(path, "/torc-service/v1/jobs/", "/manage_status_change/")
            .is_some()
        || parse_job_start_path(path).is_some()
        || parse_job_retry_path(path).is_some()
}
