//! Compute nodes pagination functionality.
//!
//! This module provides lazy iteration and vector collection support for compute nodes
//! using the generic pagination framework.

use crate::client::apis;
use crate::client::commands::pagination::base::{
    Paginatable, PaginatedIterator, PaginatedResponse, PaginationParams,
};
use crate::models::ComputeNodeModel;

/// Parameters for listing compute nodes with default values and builder methods.
#[derive(Debug, Clone, Default)]
pub struct ComputeNodeListParams {
    /// Workflow ID to list compute nodes from
    pub workflow_id: i64,
    /// Pagination offset
    pub offset: i64,
    /// Maximum number of records to return
    pub limit: Option<i64>,
    /// Field to sort by
    pub sort_by: Option<String>,
    /// Reverse sort order
    pub reverse_sort: Option<bool>,
    /// Filter by hostname
    pub hostname: Option<String>,
    /// Filter by active status
    pub is_active: Option<bool>,
    /// Filter by scheduled compute node ID
    pub scheduled_compute_node_id: Option<i64>,
}

impl ComputeNodeListParams {
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

    pub fn with_sort_by(mut self, sort_by: String) -> Self {
        self.sort_by = Some(sort_by);
        self
    }

    pub fn with_reverse_sort(mut self, reverse: bool) -> Self {
        self.reverse_sort = Some(reverse);
        self
    }

    pub fn with_hostname(mut self, hostname: String) -> Self {
        self.hostname = Some(hostname);
        self
    }

    pub fn with_is_active(mut self, is_active: bool) -> Self {
        self.is_active = Some(is_active);
        self
    }

    pub fn with_scheduled_compute_node_id(mut self, id: i64) -> Self {
        self.scheduled_compute_node_id = Some(id);
        self
    }
}

impl PaginationParams for ComputeNodeListParams {
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
        self.sort_by.as_deref()
    }

    fn reverse_sort(&self) -> Option<bool> {
        self.reverse_sort
    }
}

impl Paginatable for ComputeNodeModel {
    type ListError = apis::compute_nodes_api::ListComputeNodesError;
    type Params = ComputeNodeListParams;

    fn fetch_page(
        config: &apis::configuration::Configuration,
        params: &Self::Params,
        limit: i64,
    ) -> Result<PaginatedResponse<Self>, apis::Error<Self::ListError>> {
        let response = apis::compute_nodes_api::list_compute_nodes(
            config,
            params.workflow_id,
            Some(params.offset),
            Some(limit),
            params.sort_by.as_deref(),
            params.reverse_sort,
            params.hostname.as_deref(),
            params.is_active,
            params.scheduled_compute_node_id,
        )?;

        Ok(PaginatedResponse {
            items: response.items,
            has_more: response.has_more,
        })
    }
}

/// Type alias for the compute nodes iterator
pub type ComputeNodesIterator = PaginatedIterator<ComputeNodeModel>;

/// Create a lazy iterator for compute nodes that fetches pages on-demand.
///
/// # Arguments
/// * `config` - API configuration
/// * `workflow_id` - ID of the workflow to list compute nodes from
/// * `params` - ComputeNodeListParams containing filter and pagination parameters
///
/// # Returns
/// An iterator that yields `Result<ComputeNodeModel, Error>` items
pub fn iter_compute_nodes(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: ComputeNodeListParams,
) -> ComputeNodesIterator {
    let mut params = params;
    params.workflow_id = workflow_id;
    PaginatedIterator::new(config.clone(), params, None)
}

/// Collect all compute nodes into a vector using lazy iteration internally.
///
/// # Arguments
/// * `config` - API configuration
/// * `workflow_id` - ID of the workflow to list compute nodes from
/// * `params` - ComputeNodeListParams containing filter and pagination parameters
///
/// # Returns
/// `Result<Vec<ComputeNodeModel>, Error>` containing all compute nodes or an error
#[allow(clippy::result_large_err)]
pub fn paginate_compute_nodes(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: ComputeNodeListParams,
) -> Result<Vec<ComputeNodeModel>, apis::Error<apis::compute_nodes_api::ListComputeNodesError>> {
    iter_compute_nodes(config, workflow_id, params).collect()
}
