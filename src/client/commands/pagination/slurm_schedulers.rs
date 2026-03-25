//! Slurm schedulers pagination functionality.
//!
//! This module provides lazy iteration and vector collection support for slurm schedulers
//! using the generic pagination framework.

use crate::client::apis;
use crate::client::commands::pagination::base::{
    Paginatable, PaginatedIterator, PaginatedResponse, PaginationParams,
};
use crate::models::SlurmSchedulerModel;

/// Parameters for listing slurm schedulers with default values and builder methods.
#[derive(Debug, Clone, Default)]
pub struct SlurmSchedulersListParams {
    /// Workflow ID to list slurm schedulers from
    pub workflow_id: i64,
    /// Pagination offset
    pub offset: i64,
    /// Maximum number of records to return
    pub limit: Option<i64>,
    /// Field to sort by
    pub sort_by: Option<String>,
    /// Reverse sort order
    pub reverse_sort: Option<bool>,
    /// Filter by name
    pub name: Option<String>,
    /// Filter by account
    pub account: Option<String>,
    /// Filter by gres
    pub gres: Option<String>,
    /// Filter by mem
    pub mem: Option<String>,
    /// Filter by nodes
    pub nodes: Option<i64>,
    /// Filter by partition
    pub partition: Option<String>,
    /// Filter by qos
    pub qos: Option<String>,
    /// Filter by tmp
    pub tmp: Option<String>,
    /// Filter by walltime
    pub walltime: Option<String>,
}

impl SlurmSchedulersListParams {
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

    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn with_account(mut self, account: String) -> Self {
        self.account = Some(account);
        self
    }

    pub fn with_partition(mut self, partition: String) -> Self {
        self.partition = Some(partition);
        self
    }
}

impl PaginationParams for SlurmSchedulersListParams {
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

impl Paginatable for SlurmSchedulerModel {
    type ListError = apis::slurm_schedulers_api::ListSlurmSchedulersError;
    type Params = SlurmSchedulersListParams;

    fn fetch_page(
        config: &apis::configuration::Configuration,
        params: &Self::Params,
        limit: i64,
    ) -> Result<PaginatedResponse<Self>, apis::Error<Self::ListError>> {
        let response = apis::slurm_schedulers_api::list_slurm_schedulers(
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

/// Type alias for the slurm schedulers iterator
pub type SlurmSchedulersIterator = PaginatedIterator<SlurmSchedulerModel>;

/// Create a lazy iterator for slurm schedulers that fetches pages on-demand.
///
/// # Arguments
/// * `config` - API configuration
/// * `workflow_id` - ID of the workflow to list slurm schedulers from
/// * `params` - SlurmSchedulersListParams containing filter and pagination parameters
///
/// # Returns
/// An iterator that yields `Result<SlurmSchedulerModel, Error>` items
pub fn iter_slurm_schedulers(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: SlurmSchedulersListParams,
) -> SlurmSchedulersIterator {
    let mut params = params;
    params.workflow_id = workflow_id;
    PaginatedIterator::new(config.clone(), params, None)
}

/// Collect all slurm schedulers into a vector using lazy iteration internally.
///
/// # Arguments
/// * `config` - API configuration
/// * `workflow_id` - ID of the workflow to list slurm schedulers from
/// * `params` - SlurmSchedulersListParams containing filter and pagination parameters
///
/// # Returns
/// `Result<Vec<SlurmSchedulerModel>, Error>` containing all slurm schedulers or an error
#[allow(clippy::result_large_err)]
pub fn paginate_slurm_schedulers(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: SlurmSchedulersListParams,
) -> Result<
    Vec<SlurmSchedulerModel>,
    apis::Error<apis::slurm_schedulers_api::ListSlurmSchedulersError>,
> {
    iter_slurm_schedulers(config, workflow_id, params).collect()
}
