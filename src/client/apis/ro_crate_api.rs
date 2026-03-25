use super::{Error, configuration};
use crate::models;

pub use super::ro_crate_entities_api::{
    CreateRoCrateEntityError, DeleteRoCrateEntitiesError, DeleteRoCrateEntityError,
    GetRoCrateEntityError, ListRoCrateEntitiesError, UpdateRoCrateEntityError,
};

pub fn create_ro_crate_entity(
    configuration: &configuration::Configuration,
    ro_crate_entity_model: models::RoCrateEntityModel,
) -> Result<models::RoCrateEntityModel, Error<CreateRoCrateEntityError>> {
    super::ro_crate_entities_api::create_ro_crate_entity(configuration, ro_crate_entity_model)
}

pub fn get_ro_crate_entity(
    configuration: &configuration::Configuration,
    id: i64,
) -> Result<models::RoCrateEntityModel, Error<GetRoCrateEntityError>> {
    super::ro_crate_entities_api::get_ro_crate_entity(configuration, id)
}

pub fn update_ro_crate_entity(
    configuration: &configuration::Configuration,
    id: i64,
    ro_crate_entity_model: models::RoCrateEntityModel,
) -> Result<models::RoCrateEntityModel, Error<UpdateRoCrateEntityError>> {
    super::ro_crate_entities_api::update_ro_crate_entity(configuration, id, ro_crate_entity_model)
}

pub fn delete_ro_crate_entity(
    configuration: &configuration::Configuration,
    id: i64,
) -> Result<models::MessageResponse, Error<DeleteRoCrateEntityError>> {
    super::ro_crate_entities_api::delete_ro_crate_entity(configuration, id)
}

#[allow(clippy::too_many_arguments)]
pub fn list_ro_crate_entities(
    configuration: &configuration::Configuration,
    workflow_id: i64,
    offset: Option<i64>,
    limit: Option<i64>,
    sort_by: Option<&str>,
    reverse_sort: Option<bool>,
) -> Result<models::ListRoCrateEntitiesResponse, Error<ListRoCrateEntitiesError>> {
    super::ro_crate_entities_api::list_ro_crate_entities(
        configuration,
        workflow_id,
        offset,
        limit,
        sort_by,
        reverse_sort,
    )
}

pub fn delete_ro_crate_entities(
    configuration: &configuration::Configuration,
    id: i64,
    _body: Option<serde_json::Value>,
) -> Result<models::DeleteRoCrateEntitiesResponse, Error<DeleteRoCrateEntitiesError>> {
    super::ro_crate_entities_api::delete_ro_crate_entities(configuration, id)
}
