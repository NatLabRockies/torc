pub use super::ro_crate_entities_api::{
    CreateRoCrateEntityError, DeleteRoCrateEntitiesError, DeleteRoCrateEntityError,
    GetRoCrateEntityError, ListRoCrateEntitiesError, UpdateRoCrateEntityError,
    create_ro_crate_entity, delete_ro_crate_entity, get_ro_crate_entity, list_ro_crate_entities,
    update_ro_crate_entity,
};
use super::{Error, configuration, ro_crate_entities_api};
use crate::models;

pub fn delete_ro_crate_entities(
    configuration: &configuration::Configuration,
    id: i64,
    _unused: Option<bool>,
) -> Result<models::DeleteRoCrateEntitiesResponse, Error<DeleteRoCrateEntitiesError>> {
    ro_crate_entities_api::delete_ro_crate_entities(configuration, id)
}
