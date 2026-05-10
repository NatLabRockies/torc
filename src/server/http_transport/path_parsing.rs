#[cfg(test)]
pub(super) fn decode_path_segment(segment: &str) -> Option<String> {
    percent_encoding::percent_decode_str(segment)
        .decode_utf8()
        .ok()
        .map(|value| value.into_owned())
}

/// Strip `prefix` from the start of `path` and `suffix` from the end, returning the segment in
/// between only when it is a single path segment (i.e. contains no `/`). This is the shared
/// shape behind the `/torc-service/v1/<resource>/{id}/<sub>` helpers below.
#[cfg(test)]
fn strip_prefix_and_suffix<'a>(path: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    let middle = path.strip_prefix(prefix)?.strip_suffix(suffix)?;
    if middle.contains('/') {
        return None;
    }
    Some(middle)
}

#[cfg(test)]
pub(super) fn parse_group_member_path(path: &str) -> Option<(i64, String)> {
    let suffix = path.strip_prefix("/torc-service/v1/access_groups/")?;
    let (group_id, tail) = suffix.split_once("/members/")?;
    if tail.contains('/') {
        return None;
    }
    Some((group_id.parse::<i64>().ok()?, decode_path_segment(tail)?))
}

#[cfg(test)]
pub(super) fn parse_access_group_members_collection_path(path: &str) -> Option<i64> {
    strip_prefix_and_suffix(path, "/torc-service/v1/access_groups/", "/members")?
        .parse::<i64>()
        .ok()
}

#[cfg(test)]
pub(super) fn parse_user_groups_path(path: &str) -> Option<String> {
    let user_name = strip_prefix_and_suffix(path, "/torc-service/v1/users/", "/groups")?;
    decode_path_segment(user_name)
}

#[cfg(test)]
pub(super) fn parse_workflow_access_groups_collection_path(path: &str) -> Option<i64> {
    parse_workflow_suffix_path(path, "/access_groups")
}

#[cfg(test)]
pub(super) fn parse_workflow_access_group_item_path(path: &str) -> Option<(i64, i64)> {
    let suffix = path.strip_prefix("/torc-service/v1/workflows/")?;
    let (workflow_id, tail) = suffix.split_once("/access_groups/")?;
    if tail.contains('/') {
        return None;
    }
    Some((workflow_id.parse::<i64>().ok()?, tail.parse::<i64>().ok()?))
}

#[cfg(test)]
pub(super) fn parse_access_check_path(path: &str) -> Option<(i64, String)> {
    let suffix = path.strip_prefix("/torc-service/v1/access_check/")?;
    let (workflow_id, user_name) = suffix.split_once('/')?;
    if user_name.contains('/') {
        return None;
    }
    Some((
        workflow_id.parse::<i64>().ok()?,
        decode_path_segment(user_name)?,
    ))
}

#[cfg(test)]
pub(super) fn parse_workflow_failure_handlers_path(path: &str) -> Option<i64> {
    parse_workflow_suffix_path(path, "/failure_handlers")
}

#[cfg(test)]
pub(super) fn parse_workflow_suffix_path(path: &str, suffix: &str) -> Option<i64> {
    strip_prefix_and_suffix(path, "/torc-service/v1/workflows/", suffix)?
        .parse::<i64>()
        .ok()
}

#[cfg(test)]
pub(super) fn parse_workflow_events_stream_path(path: &str) -> Option<i64> {
    parse_workflow_suffix_path(path, "/events/stream")
}
