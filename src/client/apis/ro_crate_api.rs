use log::warn;

pub use super::ro_crate_entities_api::{
    CreateRoCrateEntityError, DeleteRoCrateEntitiesError, DeleteRoCrateEntityError,
    GetRoCrateEntityError, ListRoCrateEntitiesError, UpdateRoCrateEntityError,
    create_ro_crate_entity, delete_ro_crate_entity, get_ro_crate_entity, list_ro_crate_entities,
    update_ro_crate_entity,
};
use super::{Error, configuration, ro_crate_entities_api};
use crate::models;

#[allow(clippy::too_many_arguments)]
pub fn list_ro_crate_entities_with_filters(
    configuration: &configuration::Configuration,
    id: i64,
    offset: Option<i64>,
    limit: Option<i64>,
    file_id: Option<i64>,
    entity_id: Option<&str>,
    sort_by: Option<&str>,
    reverse_sort: Option<bool>,
) -> Result<models::ListRoCrateEntitiesResponse, Error<ListRoCrateEntitiesError>> {
    ro_crate_entities_api::list_ro_crate_entities(
        configuration,
        id,
        offset,
        limit,
        file_id,
        entity_id,
        sort_by,
        reverse_sort,
    )
}

fn first_entity_from_filtered_response(
    response: models::ListRoCrateEntitiesResponse,
    filter_name: &str,
    filter_value: impl std::fmt::Display,
) -> Option<models::RoCrateEntityModel> {
    if response.total_count > 1 {
        warn!(
            "Expected at most one RO-Crate entity for {}={}, found {}. Returning the first match.",
            filter_name, filter_value, response.total_count
        );
    }

    response.items.into_iter().next()
}

/// Find the first RO-Crate entity linked to a file.
///
/// If multiple entities match, logs a warning and returns the first item.
pub fn find_ro_crate_entity_by_file_id(
    configuration: &configuration::Configuration,
    workflow_id: i64,
    file_id: i64,
) -> Result<Option<models::RoCrateEntityModel>, Error<ListRoCrateEntitiesError>> {
    let response = list_ro_crate_entities_with_filters(
        configuration,
        workflow_id,
        Some(0),
        Some(2),
        Some(file_id),
        None,
        None,
        None,
    )?;

    Ok(first_entity_from_filtered_response(
        response, "file_id", file_id,
    ))
}

/// Find the first RO-Crate entity with a matching entity ID.
///
/// If multiple entities match, logs a warning and returns the first item.
pub fn find_ro_crate_entity_by_entity_id(
    configuration: &configuration::Configuration,
    workflow_id: i64,
    entity_id: &str,
) -> Result<Option<models::RoCrateEntityModel>, Error<ListRoCrateEntitiesError>> {
    let response = list_ro_crate_entities_with_filters(
        configuration,
        workflow_id,
        Some(0),
        Some(2),
        None,
        Some(entity_id),
        None,
        None,
    )?;

    Ok(first_entity_from_filtered_response(
        response,
        "entity_id",
        entity_id,
    ))
}

pub fn delete_ro_crate_entities(
    configuration: &configuration::Configuration,
    id: i64,
    _unused: Option<bool>,
) -> Result<models::DeleteRoCrateEntitiesResponse, Error<DeleteRoCrateEntitiesError>> {
    ro_crate_entities_api::delete_ro_crate_entities(configuration, id)
}
