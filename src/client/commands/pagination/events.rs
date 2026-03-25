//! Event pagination functionality.
//!
//! This module provides lazy iteration and vector collection support for events
//! using the generic pagination framework.

use crate::client::apis;
use crate::client::commands::pagination::base::{
    Paginatable, PaginatedIterator, PaginatedResponse, PaginationParams,
};
use crate::models::EventModel;

/// Parameters for listing events with default values and builder methods.
#[derive(Debug, Clone, Default)]
pub struct EventListParams {
    /// Workflow ID to list events from
    pub workflow_id: i64,
    /// Pagination offset
    pub offset: i64,
    /// Maximum number of events to return
    pub limit: Option<i64>,
    /// Field to sort by
    pub sort_by: Option<String>,
    /// Reverse sort order
    pub reverse_sort: Option<bool>,
    /// Filter by category
    pub category: Option<String>,
}

impl EventListParams {
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

    pub fn with_category(mut self, category: String) -> Self {
        self.category = Some(category);
        self
    }
}

impl PaginationParams for EventListParams {
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

impl Paginatable for EventModel {
    type ListError = apis::events_api::ListEventsError;
    type Params = EventListParams;

    fn fetch_page(
        config: &apis::configuration::Configuration,
        params: &Self::Params,
        limit: i64,
    ) -> Result<PaginatedResponse<Self>, apis::Error<Self::ListError>> {
        let response = apis::events_api::list_events(
            config,
            params.workflow_id,
            Some(params.offset),
            Some(limit),
            params.sort_by.as_deref(),
            params.reverse_sort,
            params.category.as_deref(),
            None, // after_timestamp (not used in pagination)
        )?;

        Ok(PaginatedResponse {
            items: response.items,
            has_more: response.has_more,
        })
    }
}

/// Type alias for the events iterator
pub type EventsIterator = PaginatedIterator<EventModel>;

/// Create a lazy iterator for events that fetches pages on-demand.
///
/// # Arguments
/// * `config` - API configuration
/// * `workflow_id` - ID of the workflow to list events from
/// * `params` - EventListParams containing filter and pagination parameters
///
/// # Returns
/// An iterator that yields `Result<EventModel, Error>` items
pub fn iter_events(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: EventListParams,
) -> EventsIterator {
    let mut params = params;
    params.workflow_id = workflow_id;
    PaginatedIterator::new(config.clone(), params, None)
}

/// Collect all events into a vector using lazy iteration internally.
///
/// # Arguments
/// * `config` - API configuration
/// * `workflow_id` - ID of the workflow to list events from
/// * `params` - EventListParams containing filter and pagination parameters
///
/// # Returns
/// `Result<Vec<EventModel>, Error>` containing all events or an error
#[allow(clippy::result_large_err)]
pub fn paginate_events(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: EventListParams,
) -> Result<Vec<EventModel>, apis::Error<apis::events_api::ListEventsError>> {
    iter_events(config, workflow_id, params).collect()
}
