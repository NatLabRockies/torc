//! Scheduled compute nodes pagination functionality.
//!
//! This module provides lazy iteration and vector collection support for scheduled compute nodes
//! using the generic pagination framework.

use crate::client::apis;
use crate::client::commands::pagination::base::{
    Paginatable, PaginatedIterator, PaginatedResponse, PaginationParams,
};
use crate::models::ScheduledComputeNodesModel;

/// Parameters for listing scheduled compute nodes with default values and builder methods.
#[derive(Debug, Clone, Default)]
pub struct ScheduledComputeNodeListParams {
    /// Workflow ID to list scheduled compute nodes from
    pub workflow_id: i64,
    /// Pagination offset
    pub offset: i64,
    /// Maximum number of records to return
    pub limit: Option<i64>,
    /// Field to sort by
    pub sort_by: Option<String>,
    /// Reverse sort order
    pub reverse_sort: Option<bool>,
    /// Filter by scheduler ID
    pub scheduler_id: Option<String>,
    /// Filter by scheduler config ID
    pub scheduler_config_id: Option<String>,
    /// Filter by status
    pub status: Option<String>,
}

impl ScheduledComputeNodeListParams {
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

    pub fn with_scheduler_id(mut self, scheduler_id: String) -> Self {
        self.scheduler_id = Some(scheduler_id);
        self
    }

    pub fn with_scheduler_config_id(mut self, scheduler_config_id: String) -> Self {
        self.scheduler_config_id = Some(scheduler_config_id);
        self
    }

    pub fn with_status(mut self, status: String) -> Self {
        self.status = Some(status);
        self
    }
}

impl PaginationParams for ScheduledComputeNodeListParams {
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

impl Paginatable for ScheduledComputeNodesModel {
    type ListError = apis::scheduled_compute_nodes_api::ListScheduledComputeNodesError;
    type Params = ScheduledComputeNodeListParams;

    fn fetch_page(
        config: &apis::configuration::Configuration,
        params: &Self::Params,
        limit: i64,
    ) -> Result<PaginatedResponse<Self>, apis::Error<Self::ListError>> {
        let response = apis::scheduled_compute_nodes_api::list_scheduled_compute_nodes(
            config,
            params.workflow_id,
            Some(params.offset),
            Some(limit),
            params.sort_by.as_deref(),
            params.reverse_sort,
            params.scheduler_id.as_deref(),
            params.scheduler_config_id.as_deref(),
            params.status.as_deref(),
        )?;

        Ok(PaginatedResponse {
            items: response.items,
            has_more: response.has_more,
        })
    }
}

/// Type alias for the scheduled compute nodes iterator
pub type ScheduledComputeNodesIterator = PaginatedIterator<ScheduledComputeNodesModel>;

/// Create a lazy iterator for scheduled compute nodes that fetches pages on-demand.
///
/// # Arguments
/// * `config` - API configuration
/// * `workflow_id` - ID of the workflow to list scheduled compute nodes from
/// * `params` - ScheduledComputeNodeListParams containing filter and pagination parameters
///
/// # Returns
/// An iterator that yields `Result<ScheduledComputeNodesModel, Error>` items
pub fn iter_scheduled_compute_nodes(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: ScheduledComputeNodeListParams,
) -> ScheduledComputeNodesIterator {
    let mut params = params;
    params.workflow_id = workflow_id;
    PaginatedIterator::new(config.clone(), params, None)
}

/// Collect all scheduled compute nodes into a vector using lazy iteration internally.
///
/// # Arguments
/// * `config` - API configuration
/// * `workflow_id` - ID of the workflow to list scheduled compute nodes from
/// * `params` - ScheduledComputeNodeListParams containing filter and pagination parameters
///
/// # Returns
/// `Result<Vec<ScheduledComputeNodesModel>, Error>` containing all scheduled compute nodes or an error
#[allow(clippy::result_large_err)]
pub fn paginate_scheduled_compute_nodes(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: ScheduledComputeNodeListParams,
) -> Result<
    Vec<ScheduledComputeNodesModel>,
    apis::Error<apis::scheduled_compute_nodes_api::ListScheduledComputeNodesError>,
> {
    iter_scheduled_compute_nodes(config, workflow_id, params).collect()
}
