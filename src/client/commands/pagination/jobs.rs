//! Job pagination functionality.
//!
//! This module provides lazy iteration and vector collection support for jobs
//! using the generic pagination framework.

use crate::client::apis;
use crate::client::commands::pagination::base::{
    Paginatable, PaginatedIterator, PaginatedResponse, PaginationParams,
};
use crate::models::{JobModel, JobStatus};

/// Parameters for listing jobs with default values and builder methods.
#[derive(Debug, Clone, Default)]
pub struct JobListParams {
    /// Workflow ID to list jobs from
    pub workflow_id: i64,
    /// Filter by job status
    pub status: Option<JobStatus>,
    /// Filter by file ID that the job needs
    pub needs_file_id: Option<i64>,
    /// Filter by upstream job ID
    pub upstream_job_id: Option<i64>,
    /// Pagination offset
    pub offset: i64,
    /// Maximum number of jobs to return
    pub limit: Option<i64>,
    /// Field to sort by
    pub sort_by: Option<String>,
    /// Reverse sort order
    pub reverse_sort: Option<bool>,
    /// Include job relationships in response
    pub include_relationships: Option<bool>,
    /// Filter by active compute node ID
    pub active_compute_node_id: Option<i64>,
}

impl JobListParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_status(mut self, status: JobStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub fn with_needs_file_id(mut self, file_id: i64) -> Self {
        self.needs_file_id = Some(file_id);
        self
    }

    pub fn with_upstream_job_id(mut self, job_id: i64) -> Self {
        self.upstream_job_id = Some(job_id);
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

    pub fn with_include_relationships(mut self, include: bool) -> Self {
        self.include_relationships = Some(include);
        self
    }

    pub fn with_active_compute_node_id(mut self, id: i64) -> Self {
        self.active_compute_node_id = Some(id);
        self
    }
}

impl PaginationParams for JobListParams {
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

impl Paginatable for JobModel {
    type ListError = apis::jobs_api::ListJobsError;
    type Params = JobListParams;

    fn fetch_page(
        config: &apis::configuration::Configuration,
        params: &Self::Params,
        limit: i64,
    ) -> Result<PaginatedResponse<Self>, apis::Error<Self::ListError>> {
        let response = apis::jobs_api::list_jobs(
            config,
            params.workflow_id,
            params.status,
            params.needs_file_id,
            params.upstream_job_id,
            Some(params.offset),
            Some(limit),
            params.sort_by.as_deref(),
            params.reverse_sort,
            params.include_relationships,
            params.active_compute_node_id,
        )?;

        Ok(PaginatedResponse {
            items: response.items,
            has_more: response.has_more,
        })
    }
}

/// Type alias for the jobs iterator
pub type JobsIterator = PaginatedIterator<JobModel>;

/// Create a lazy iterator for jobs that fetches pages on-demand.
///
/// This is memory efficient as it only loads one page at a time.
///
/// # Arguments
/// * `config` - API configuration containing base URL and authentication
/// * `workflow_id` - ID of the workflow to list jobs from
/// * `params` - JobListParams containing filter and pagination parameters
///
/// # Returns
/// An iterator that yields `Result<JobModel, Error>` items
pub fn iter_jobs(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: JobListParams,
) -> JobsIterator {
    let mut params = params;
    params.workflow_id = workflow_id;
    PaginatedIterator::new(config.clone(), params, None)
}

/// Collect all jobs into a vector using lazy iteration internally.
///
/// # Arguments
/// * `config` - API configuration containing base URL and authentication
/// * `workflow_id` - ID of the workflow to list jobs from
/// * `params` - JobListParams containing filter and pagination parameters
///
/// # Returns
/// `Result<Vec<JobModel>, Error>` containing all jobs or an error
#[allow(clippy::result_large_err)]
pub fn paginate_jobs(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: JobListParams,
) -> Result<Vec<JobModel>, apis::Error<apis::jobs_api::ListJobsError>> {
    iter_jobs(config, workflow_id, params).collect()
}
