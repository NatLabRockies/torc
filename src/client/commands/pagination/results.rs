//! Result pagination functionality.
//!
//! This module provides lazy iteration and vector collection support for results
//! using the generic pagination framework.

use crate::client::apis;
use crate::client::commands::pagination::base::{
    Paginatable, PaginatedIterator, PaginatedResponse, PaginationParams,
};
use crate::models::{JobStatus, ResultModel};

/// Parameters for listing results with default values and builder methods.
#[derive(Debug, Clone, Default)]
pub struct ResultListParams {
    /// Workflow ID to list results from
    pub workflow_id: i64,
    /// Filter by job ID
    pub job_id: Option<i64>,
    /// Filter by run ID
    pub run_id: Option<i64>,
    /// Pagination offset
    pub offset: i64,
    /// Maximum number of records to return
    pub limit: Option<i64>,
    /// Field to sort by
    pub sort_by: Option<String>,
    /// Reverse sort order
    pub reverse_sort: Option<bool>,
    /// Filter by return code
    pub return_code: Option<i64>,
    /// Filter by status
    pub status: Option<JobStatus>,
    /// Include all runs
    pub all_runs: Option<bool>,
    /// Filter by compute node ID
    pub compute_node_id: Option<i64>,
}

impl ResultListParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_job_id(mut self, job_id: i64) -> Self {
        self.job_id = Some(job_id);
        self
    }

    pub fn with_run_id(mut self, run_id: i64) -> Self {
        self.run_id = Some(run_id);
        self
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

    pub fn with_return_code(mut self, return_code: i64) -> Self {
        self.return_code = Some(return_code);
        self
    }

    pub fn with_status(mut self, status: JobStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub fn with_all_runs(mut self, all_runs: bool) -> Self {
        self.all_runs = Some(all_runs);
        self
    }

    pub fn with_compute_node_id(mut self, compute_node_id: i64) -> Self {
        self.compute_node_id = Some(compute_node_id);
        self
    }
}

impl PaginationParams for ResultListParams {
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

impl Paginatable for ResultModel {
    type ListError = apis::results_api::ListResultsError;
    type Params = ResultListParams;

    fn fetch_page(
        config: &apis::configuration::Configuration,
        params: &Self::Params,
        limit: i64,
    ) -> Result<PaginatedResponse<Self>, apis::Error<Self::ListError>> {
        let response = apis::results_api::list_results(
            config,
            params.workflow_id,
            params.job_id,
            params.run_id,
            params.return_code,
            params.status,
            params.compute_node_id,
            Some(params.offset),
            Some(limit),
            params.sort_by.as_deref(),
            params.reverse_sort,
            params.all_runs,
        )?;

        Ok(PaginatedResponse {
            items: response.items,
            has_more: response.has_more,
        })
    }
}

/// Type alias for the results iterator
pub type ResultsIterator = PaginatedIterator<ResultModel>;

/// Create a lazy iterator for results that fetches pages on-demand.
///
/// # Arguments
/// * `config` - API configuration
/// * `workflow_id` - ID of the workflow to list results from
/// * `params` - ResultListParams containing filter and pagination parameters
///
/// # Returns
/// An iterator that yields `Result<ResultModel, Error>` items
pub fn iter_results(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: ResultListParams,
) -> ResultsIterator {
    let mut params = params;
    params.workflow_id = workflow_id;
    PaginatedIterator::new(config.clone(), params, None)
}

/// Collect all results into a vector using lazy iteration internally.
///
/// # Arguments
/// * `config` - API configuration
/// * `workflow_id` - ID of the workflow to list results from
/// * `params` - ResultListParams containing filter and pagination parameters
///
/// # Returns
/// `Result<Vec<ResultModel>, Error>` containing all results or an error
#[allow(clippy::result_large_err)]
pub fn paginate_results(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: ResultListParams,
) -> Result<Vec<ResultModel>, apis::Error<apis::results_api::ListResultsError>> {
    iter_results(config, workflow_id, params).collect()
}
