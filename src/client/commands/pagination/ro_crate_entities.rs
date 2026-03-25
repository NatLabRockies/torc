//! RO-Crate entity pagination functionality.
//!
//! This module provides lazy iteration and vector collection support for RO-Crate entities
//! using the generic pagination framework.

use crate::client::apis;
use crate::client::commands::pagination::base::{
    Paginatable, PaginatedIterator, PaginatedResponse, PaginationParams,
};
use crate::models::RoCrateEntityModel;

/// Parameters for listing RO-Crate entities with default values and builder methods.
#[derive(Debug, Clone, Default)]
pub struct RoCrateEntityListParams {
    /// Workflow ID to list RO-Crate entities from
    pub workflow_id: i64,
    /// Pagination offset
    pub offset: i64,
    /// Maximum number of entities to return
    pub limit: Option<i64>,
}

impl RoCrateEntityListParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_offset(mut self, offset: i64) -> Self {
        self.offset = offset;
        self
    }

    pub fn with_limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }
}

impl PaginationParams for RoCrateEntityListParams {
    fn offset(&self) -> i64 {
        self.offset
    }

    fn set_offset(&mut self, offset: i64) {
        self.offset = offset;
    }

    fn limit(&self) -> Option<i64> {
        self.limit
    }

    fn sort_by(&self) -> Option<&str> {
        None
    }

    fn reverse_sort(&self) -> Option<bool> {
        None
    }
}

impl Paginatable for RoCrateEntityModel {
    type ListError = apis::ro_crate_entities_api::ListRoCrateEntitiesError;
    type Params = RoCrateEntityListParams;

    fn fetch_page(
        config: &apis::configuration::Configuration,
        params: &Self::Params,
        limit: i64,
    ) -> Result<PaginatedResponse<Self>, apis::Error<Self::ListError>> {
        let response = apis::ro_crate_entities_api::list_ro_crate_entities(
            config,
            params.workflow_id,
            Some(params.offset),
            Some(limit),
            params.sort_by(),
            params.reverse_sort(),
        )?;

        Ok(PaginatedResponse {
            items: response.items,
            has_more: response.has_more,
        })
    }
}

/// Type alias for the RO-Crate entities iterator
pub type RoCrateEntitiesIterator = PaginatedIterator<RoCrateEntityModel>;

/// Create a lazy iterator for RO-Crate entities that fetches pages on-demand.
///
/// This is memory efficient as it only loads one page at a time.
///
/// # Arguments
/// * `config` - API configuration containing base URL and authentication
/// * `workflow_id` - ID of the workflow to list RO-Crate entities from
/// * `params` - RoCrateEntityListParams containing filter and pagination parameters
///
/// # Returns
/// An iterator that yields `Result<RoCrateEntityModel, Error>` items
pub fn iter_ro_crate_entities(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: RoCrateEntityListParams,
) -> RoCrateEntitiesIterator {
    let mut params = params;
    params.workflow_id = workflow_id;
    PaginatedIterator::new(config.clone(), params, None)
}

/// Collect all RO-Crate entities into a vector using lazy iteration internally.
///
/// # Arguments
/// * `config` - API configuration containing base URL and authentication
/// * `workflow_id` - ID of the workflow to list RO-Crate entities from
/// * `params` - RoCrateEntityListParams containing filter and pagination parameters
///
/// # Returns
/// `Result<Vec<RoCrateEntityModel>, Error>` containing all entities or an error
#[allow(clippy::result_large_err)]
pub fn paginate_ro_crate_entities(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: RoCrateEntityListParams,
) -> Result<
    Vec<RoCrateEntityModel>,
    apis::Error<apis::ro_crate_entities_api::ListRoCrateEntitiesError>,
> {
    iter_ro_crate_entities(config, workflow_id, params).collect()
}
