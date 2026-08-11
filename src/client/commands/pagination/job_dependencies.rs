//! Job-to-job dependency pagination functionality.
//!
//! This module provides lazy iteration and vector collection support for job-to-job
//! dependencies using the generic pagination framework.

use crate::client::apis;
use crate::client::commands::pagination::base::{
    Paginatable, PaginatedIterator, PaginatedResponse, PaginationParams,
};
use crate::models::JobDependencyModel;

/// Parameters for listing job-to-job dependencies with default values and builder methods.
#[derive(Debug, Clone, Default)]
pub struct JobDependencyListParams {
    /// Workflow ID to list dependencies from
    workflow_id: i64,
    /// Pagination offset
    offset: i64,
    /// Maximum number of records to return
    limit: Option<i64>,
    /// Field to sort by
    sort_by: Option<String>,
    /// Reverse sort order
    reverse_sort: Option<bool>,
}

impl JobDependencyListParams {
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
}

impl PaginationParams for JobDependencyListParams {
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

impl Paginatable for JobDependencyModel {
    type ListError = apis::workflows_api::ListJobDependenciesError;
    type Params = JobDependencyListParams;

    fn fetch_page(
        config: &apis::configuration::Configuration,
        params: &Self::Params,
        limit: i64,
    ) -> Result<PaginatedResponse<Self>, apis::Error<Self::ListError>> {
        let response = apis::workflows_api::list_job_dependencies(
            config,
            params.workflow_id,
            Some(params.offset),
            Some(limit),
            params.sort_by.as_deref(),
            params.reverse_sort,
        )?;

        Ok(PaginatedResponse {
            items: response.items,
            has_more: response.has_more,
        })
    }
}

/// Type alias for the job dependencies iterator
type JobDependenciesIterator = PaginatedIterator<JobDependencyModel>;

/// Create a lazy iterator for job-to-job dependencies that fetches pages on-demand.
fn iter_job_dependencies(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: JobDependencyListParams,
) -> JobDependenciesIterator {
    let mut params = params;
    params.workflow_id = workflow_id;
    PaginatedIterator::new(config.clone(), params, None)
}

/// Collect all job-to-job dependencies into a vector using lazy iteration internally.
#[allow(clippy::result_large_err)]
pub(crate) fn paginate_job_dependencies(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: JobDependencyListParams,
) -> Result<Vec<JobDependencyModel>, apis::Error<apis::workflows_api::ListJobDependenciesError>> {
    iter_job_dependencies(config, workflow_id, params).collect()
}
