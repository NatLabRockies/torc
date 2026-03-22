fn decode_path_segment(segment: &str) -> Option<String> {
    percent_encoding::percent_decode_str(segment)
        .decode_utf8()
        .ok()
        .map(|value| value.into_owned())
}

fn parse_required_i32(params: &HashMap<String, String>, key: &str) -> Result<i32, String> {
    let raw = params
        .get(key)
        .ok_or_else(|| format!("Missing required query parameter: {key}"))?;
    raw.parse::<i32>()
        .map_err(|_| format!("Invalid integer for query parameter: {key}"))
}

fn parse_resource_id(path: &str, prefix: &str) -> Option<i64> {
    let id = path.strip_prefix(prefix)?;
    if id.contains('/') {
        return None;
    }
    id.parse::<i64>().ok()
}

fn parse_group_member_path(path: &str) -> Option<(i64, String)> {
    let suffix = path.strip_prefix("/torc-service/v1/access_groups/")?;
    let (group_id, tail) = suffix.split_once("/members/")?;
    if tail.contains('/') {
        return None;
    }
    Some((group_id.parse::<i64>().ok()?, decode_path_segment(tail)?))
}

fn parse_access_group_members_collection_path(path: &str) -> Option<i64> {
    let group_id = path.strip_prefix("/torc-service/v1/access_groups/")?;
    let group_id = group_id.strip_suffix("/members")?;
    if group_id.contains('/') {
        return None;
    }
    group_id.parse::<i64>().ok()
}

fn parse_user_groups_path(path: &str) -> Option<String> {
    let user_name = path.strip_prefix("/torc-service/v1/users/")?;
    let user_name = user_name.strip_suffix("/groups")?;
    if user_name.contains('/') {
        return None;
    }
    decode_path_segment(user_name)
}

fn parse_workflow_access_groups_collection_path(path: &str) -> Option<i64> {
    let workflow_id = path.strip_prefix("/torc-service/v1/workflows/")?;
    let workflow_id = workflow_id.strip_suffix("/access_groups")?;
    if workflow_id.contains('/') {
        return None;
    }
    workflow_id.parse::<i64>().ok()
}

fn parse_workflow_access_group_item_path(path: &str) -> Option<(i64, i64)> {
    let suffix = path.strip_prefix("/torc-service/v1/workflows/")?;
    let (workflow_id, tail) = suffix.split_once("/access_groups/")?;
    if tail.contains('/') {
        return None;
    }
    Some((workflow_id.parse::<i64>().ok()?, tail.parse::<i64>().ok()?))
}

fn parse_access_check_path(path: &str) -> Option<(i64, String)> {
    let suffix = path.strip_prefix("/torc-service/v1/access_check/")?;
    let (workflow_id, user_name) = suffix.split_once('/')?;
    if user_name.contains('/') {
        return None;
    }
    Some((workflow_id.parse::<i64>().ok()?, decode_path_segment(user_name)?))
}

fn parse_workflow_failure_handlers_path(path: &str) -> Option<i64> {
    let workflow_id = path.strip_prefix("/torc-service/v1/workflows/")?;
    let workflow_id = workflow_id.strip_suffix("/failure_handlers")?;
    if workflow_id.contains('/') {
        return None;
    }
    workflow_id.parse::<i64>().ok()
}

fn parse_workflow_ro_crate_entities_path(path: &str) -> Option<i64> {
    let workflow_id = path.strip_prefix("/torc-service/v1/workflows/")?;
    let workflow_id = workflow_id.strip_suffix("/ro_crate_entities")?;
    if workflow_id.contains('/') {
        return None;
    }
    workflow_id.parse::<i64>().ok()
}

fn parse_workflow_remote_workers_collection_path(path: &str) -> Option<i64> {
    let workflow_id = path.strip_prefix("/torc-service/v1/workflows/")?;
    let workflow_id = workflow_id.strip_suffix("/remote_workers")?;
    if workflow_id.contains('/') {
        return None;
    }
    workflow_id.parse::<i64>().ok()
}

fn parse_workflow_remote_worker_item_path(path: &str) -> Option<(i64, String)> {
    let suffix = path.strip_prefix("/torc-service/v1/workflows/")?;
    let (workflow_id, worker) = suffix.split_once("/remote_workers/")?;
    if worker.contains('/') {
        return None;
    }
    Some((workflow_id.parse::<i64>().ok()?, worker.to_string()))
}

fn parse_workflow_suffix_path(path: &str, suffix: &str) -> Option<i64> {
    let workflow_id = path.strip_prefix("/torc-service/v1/workflows/")?;
    let workflow_id = workflow_id.strip_suffix(suffix)?;
    if workflow_id.contains('/') {
        return None;
    }
    workflow_id.parse::<i64>().ok()
}

fn parse_workflow_events_stream_path(path: &str) -> Option<i64> {
    parse_workflow_suffix_path(path, "/events/stream")
}

fn parse_workflow_dot_graph_path(path: &str) -> Option<(i64, String)> {
    let suffix = path.strip_prefix("/torc-service/v1/workflows/")?;
    let (workflow_id, name) = suffix.split_once("/dot_graph/")?;
    if workflow_id.contains('/') || name.contains('/') {
        return None;
    }
    Some((workflow_id.parse::<i64>().ok()?, name.to_string()))
}

fn parse_workflow_actions_collection_path(path: &str) -> Option<i64> {
    parse_workflow_suffix_path(path, "/actions")
}

fn parse_workflow_pending_actions_path(path: &str) -> Option<i64> {
    parse_workflow_suffix_path(path, "/actions/pending")
}

fn parse_workflow_action_claim_path(path: &str) -> Option<(i64, i64)> {
    let suffix = path.strip_prefix("/torc-service/v1/workflows/")?;
    let (workflow_id, rest) = suffix.split_once("/actions/")?;
    let action_id = rest.strip_suffix("/claim")?;
    if workflow_id.contains('/') || action_id.contains('/') {
        return None;
    }
    Some((
        workflow_id.parse::<i64>().ok()?,
        action_id.parse::<i64>().ok()?,
    ))
}

fn parse_workflow_claim_jobs_resources_path(path: &str) -> Option<(i64, i64)> {
    let suffix = path.strip_prefix("/torc-service/v1/workflows/")?;
    let (workflow_id, limit) = suffix.split_once("/claim_jobs_based_on_resources/")?;
    if workflow_id.contains('/') || limit.contains('/') {
        return None;
    }
    Some((workflow_id.parse::<i64>().ok()?, limit.parse::<i64>().ok()?))
}

fn parse_job_status_run_path(
    path: &str,
    prefix: &str,
    middle: &str,
) -> Option<(i64, models::JobStatus, i64)> {
    let suffix = path.strip_prefix(prefix)?;
    let (id, rest) = suffix.split_once(middle)?;
    let (status, run_id) = rest.split_once('/')?;
    if run_id.contains('/') {
        return None;
    }
    Some((
        id.parse::<i64>().ok()?,
        status.parse::<models::JobStatus>().ok()?,
        run_id.parse::<i64>().ok()?,
    ))
}

fn parse_job_start_path(path: &str) -> Option<(i64, i64, i64)> {
    let suffix = path.strip_prefix("/torc-service/v1/jobs/")?;
    let (id, rest) = suffix.split_once("/start_job/")?;
    let (run_id, compute_node_id) = rest.split_once('/')?;
    if compute_node_id.contains('/') {
        return None;
    }
    Some((
        id.parse::<i64>().ok()?,
        run_id.parse::<i64>().ok()?,
        compute_node_id.parse::<i64>().ok()?,
    ))
}

fn parse_job_retry_path(path: &str) -> Option<(i64, i64)> {
    let suffix = path.strip_prefix("/torc-service/v1/jobs/")?;
    let (id, run_id) = suffix.split_once("/retry/")?;
    if run_id.contains('/') {
        return None;
    }
    Some((id.parse::<i64>().ok()?, run_id.parse::<i64>().ok()?))
}
