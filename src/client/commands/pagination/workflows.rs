//! Workflow pagination functionality.
//!
//! This module provides lazy iteration and vector collection support for workflows
//! using the generic pagination framework.

use crate::client::apis;
use crate::client::commands::pagination::base::{
    Paginatable, PaginatedIterator, PaginatedResponse, PaginationParams,
};
use crate::models::WorkflowModel;

/// Parameters for listing workflows with default values and builder methods.
#[derive(Debug, Clone, Default)]
pub struct WorkflowListParams {
    /// Pagination offset
    offset: i64,
    /// Maximum number of records to return
    limit: Option<i64>,
    /// Field to sort by
    sort_by: Option<String>,
    /// Reverse sort order
    reverse_sort: Option<bool>,
    /// Filter by name
    name: Option<String>,
    /// Filter by user
    user: Option<String>,
    /// Filter by description
    description: Option<String>,
    /// Filter by archived status
    is_archived: Option<bool>,
    /// Filter to workflows shared with this access group (by group name)
    access_group: Option<String>,
}

impl WorkflowListParams {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_offset(mut self, offset: i64) -> Self {
        self.offset = offset;
        self
    }

    pub(crate) fn with_limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }

    pub(crate) fn with_sort_by(mut self, sort_by: String) -> Self {
        self.sort_by = Some(sort_by);
        self
    }

    pub(crate) fn with_reverse_sort(mut self, reverse: bool) -> Self {
        self.reverse_sort = Some(reverse);
        self
    }

    fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub(crate) fn with_user(mut self, user: String) -> Self {
        self.user = Some(user);
        self
    }

    fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    pub(crate) fn with_is_archived(mut self, is_archived: bool) -> Self {
        self.is_archived = Some(is_archived);
        self
    }

    pub(crate) fn with_access_group(mut self, access_group: String) -> Self {
        self.access_group = Some(access_group);
        self
    }
}

impl PaginationParams for WorkflowListParams {
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

impl Paginatable for WorkflowModel {
    type ListError = apis::workflows_api::ListWorkflowsError;
    type Params = WorkflowListParams;

    fn fetch_page(
        config: &apis::configuration::Configuration,
        params: &Self::Params,
        limit: i64,
    ) -> Result<PaginatedResponse<Self>, apis::Error<Self::ListError>> {
        let response = apis::workflows_api::list_workflows(
            config,
            Some(params.offset),
            Some(limit),
            params.sort_by.as_deref(),
            params.reverse_sort,
            params.name.as_deref(),
            params.user.as_deref(),
            params.description.as_deref(),
            params.is_archived,
            params.access_group.as_deref(),
        )?;

        Ok(PaginatedResponse {
            items: response.items,
            has_more: response.has_more,
        })
    }
}

/// Type alias for the workflows iterator
type WorkflowsIterator = PaginatedIterator<WorkflowModel>;

/// Create a lazy iterator for workflows that fetches pages on-demand.
///
/// # Arguments
/// * `config` - API configuration
/// * `params` - WorkflowListParams containing filter and pagination parameters
///
/// # Returns
/// An iterator that yields `Result<WorkflowModel, Error>` items
fn iter_workflows(
    config: &apis::configuration::Configuration,
    params: WorkflowListParams,
) -> WorkflowsIterator {
    PaginatedIterator::new(config.clone(), params, None)
}

/// Collect all workflows into a vector using lazy iteration internally.
///
/// # Arguments
/// * `config` - API configuration
/// * `params` - WorkflowListParams containing filter and pagination parameters
///
/// # Returns
/// `Result<Vec<WorkflowModel>, Error>` containing all workflows or an error
pub(crate) fn paginate_workflows(
    config: &apis::configuration::Configuration,
    params: WorkflowListParams,
) -> Result<Vec<WorkflowModel>, apis::Error<apis::workflows_api::ListWorkflowsError>> {
    iter_workflows(config, params).collect()
}
