//! User data pagination functionality.
//!
//! This module provides lazy iteration and vector collection support for user data
//! using the generic pagination framework.

use crate::client::apis;
use crate::client::commands::pagination::base::{
    Paginatable, PaginatedIterator, PaginatedResponse, PaginationParams,
};
use crate::models::UserDataModel;

/// Parameters for listing user data with default values and builder methods.
#[derive(Debug, Clone, Default)]
pub struct UserDataListParams {
    /// Workflow ID to list user data from
    pub workflow_id: i64,
    /// Filter by consumer job ID
    pub consumer_job_id: Option<i64>,
    /// Filter by producer job ID
    pub producer_job_id: Option<i64>,
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
    /// Filter by ephemeral status
    pub is_ephemeral: Option<bool>,
}

impl UserDataListParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_consumer_job_id(mut self, job_id: i64) -> Self {
        self.consumer_job_id = Some(job_id);
        self
    }

    pub fn with_producer_job_id(mut self, job_id: i64) -> Self {
        self.producer_job_id = Some(job_id);
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

    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn with_is_ephemeral(mut self, is_ephemeral: bool) -> Self {
        self.is_ephemeral = Some(is_ephemeral);
        self
    }
}

impl PaginationParams for UserDataListParams {
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

impl Paginatable for UserDataModel {
    type ListError = apis::user_data_api::ListUserDataError;
    type Params = UserDataListParams;

    fn fetch_page(
        config: &apis::configuration::Configuration,
        params: &Self::Params,
        limit: i64,
    ) -> Result<PaginatedResponse<Self>, apis::Error<Self::ListError>> {
        let response = apis::user_data_api::list_user_data(
            config,
            params.workflow_id,
            params.consumer_job_id,
            params.producer_job_id,
            Some(params.offset),
            Some(limit),
            params.sort_by.as_deref(),
            params.reverse_sort,
            params.name.as_deref(),
            params.is_ephemeral,
        )?;

        Ok(PaginatedResponse {
            items: response.items,
            has_more: response.has_more,
        })
    }
}

/// Type alias for the user data iterator
pub type UserDataIterator = PaginatedIterator<UserDataModel>;

/// Create a lazy iterator for user data that fetches pages on-demand.
///
/// # Arguments
/// * `config` - API configuration
/// * `workflow_id` - ID of the workflow to list user data from
/// * `params` - UserDataListParams containing filter and pagination parameters
///
/// # Returns
/// An iterator that yields `Result<UserDataModel, Error>` items
pub fn iter_user_data(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: UserDataListParams,
) -> UserDataIterator {
    let mut params = params;
    params.workflow_id = workflow_id;
    PaginatedIterator::new(config.clone(), params, None)
}

/// Collect all user data into a vector using lazy iteration internally.
///
/// # Arguments
/// * `config` - API configuration
/// * `workflow_id` - ID of the workflow to list user data from
/// * `params` - UserDataListParams containing filter and pagination parameters
///
/// # Returns
/// `Result<Vec<UserDataModel>, Error>` containing all user data or an error
#[allow(clippy::result_large_err)]
pub fn paginate_user_data(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: UserDataListParams,
) -> Result<Vec<UserDataModel>, apis::Error<apis::user_data_api::ListUserDataError>> {
    iter_user_data(config, workflow_id, params).collect()
}
