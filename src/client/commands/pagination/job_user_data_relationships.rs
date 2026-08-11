//! Job-user_data relationship pagination functionality.
//!
//! This module provides lazy iteration and vector collection support for job-user_data
//! relationships using the generic pagination framework.

use crate::client::apis;
use crate::client::commands::pagination::base::{
    Paginatable, PaginatedIterator, PaginatedResponse, PaginationParams,
};
use crate::models::JobUserDataRelationshipModel;

/// Parameters for listing job-user_data relationships with default values and builder methods.
#[derive(Debug, Clone, Default)]
pub struct JobUserDataRelationshipListParams {
    /// Workflow ID to list relationships from
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

impl JobUserDataRelationshipListParams {
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

impl PaginationParams for JobUserDataRelationshipListParams {
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

impl Paginatable for JobUserDataRelationshipModel {
    type ListError = apis::workflows_api::ListJobUserDataRelationshipsError;
    type Params = JobUserDataRelationshipListParams;

    fn fetch_page(
        config: &apis::configuration::Configuration,
        params: &Self::Params,
        limit: i64,
    ) -> Result<PaginatedResponse<Self>, apis::Error<Self::ListError>> {
        let response = apis::workflows_api::list_job_user_data_relationships(
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

/// Type alias for the job-user_data relationships iterator
type JobUserDataRelationshipsIterator = PaginatedIterator<JobUserDataRelationshipModel>;

/// Create a lazy iterator for job-user_data relationships that fetches pages on-demand.
fn iter_job_user_data_relationships(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: JobUserDataRelationshipListParams,
) -> JobUserDataRelationshipsIterator {
    let mut params = params;
    params.workflow_id = workflow_id;
    PaginatedIterator::new(config.clone(), params, None)
}

/// Collect all job-user_data relationships into a vector using lazy iteration internally.
#[allow(clippy::result_large_err)]
pub(crate) fn paginate_job_user_data_relationships(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: JobUserDataRelationshipListParams,
) -> Result<
    Vec<JobUserDataRelationshipModel>,
    apis::Error<apis::workflows_api::ListJobUserDataRelationshipsError>,
> {
    iter_job_user_data_relationships(config, workflow_id, params).collect()
}
