//! Generic pagination framework for API resources.
//!
//! This module provides a trait-based pagination system that eliminates
//! code duplication across different resource types (jobs, files, events, etc.).
//!
//! # Usage
//!
//! To add pagination for a new resource type:
//! 1. Define a `*ListParams` struct with the resource-specific filters
//! 2. Implement `PaginationParams` for the params struct
//! 3. Implement `Paginatable` for the model type
//! 4. Create `iter_*` and `paginate_*` convenience functions

use crate::client::apis;
use std::fmt::Debug;

/// Common pagination parameters that all resources share.
///
/// This trait provides access to the standard pagination fields
/// (offset, limit, sort_by, reverse_sort) that are common across all resources.
pub trait PaginationParams {
    /// Get the current offset
    fn offset(&self) -> i64;

    /// Set the offset (mutably)
    fn set_offset(&mut self, offset: i64);

    /// Get the limit (if set)
    fn limit(&self) -> Option<i64>;

    /// Get the sort field (if set)
    fn sort_by(&self) -> Option<&str>;

    /// Get the reverse sort flag (if set)
    fn reverse_sort(&self) -> Option<bool>;
}

/// Standard paginated response structure.
///
/// This wraps the API response with the essential pagination metadata.
pub struct PaginatedResponse<T> {
    /// The items in this page (None if empty)
    pub items: Option<Vec<T>>,
    /// Whether there are more pages available
    pub has_more: bool,
}

/// Trait for types that can be paginated.
///
/// Implement this trait for each model type (JobModel, FileModel, etc.)
/// to define how to fetch pages of that resource from the API.
pub trait Paginatable: Clone + Sized {
    /// The API error type for list operations
    type ListError: Debug;

    /// Parameters specific to this resource type
    type Params: PaginationParams + Clone;

    /// Fetch a page of items from the API.
    ///
    /// # Arguments
    /// * `config` - API configuration
    /// * `params` - Resource-specific parameters (including offset from PaginationParams)
    /// * `limit` - Maximum items to fetch for this page
    ///
    /// # Returns
    /// A PaginatedResponse containing the items and has_more flag
    fn fetch_page(
        config: &apis::configuration::Configuration,
        params: &Self::Params,
        limit: i64,
    ) -> Result<PaginatedResponse<Self>, apis::Error<Self::ListError>>;
}

/// Generic paginated iterator.
///
/// This iterator fetches pages lazily on-demand, providing memory-efficient
/// iteration over potentially large result sets.
pub struct PaginatedIterator<T: Paginatable> {
    config: apis::configuration::Configuration,
    params: T::Params,
    remaining_limit: i64,
    initial_limit: i64,
    current_page: std::vec::IntoIter<T>,
    finished: bool,
}

impl<T: Paginatable> PaginatedIterator<T> {
    /// Create a new paginated iterator.
    ///
    /// # Arguments
    /// * `config` - API configuration
    /// * `params` - Resource-specific parameters
    /// * `initial_limit` - Page size for each API call (default: MAX_RECORD_TRANSFER_COUNT)
    pub fn new(
        config: apis::configuration::Configuration,
        params: T::Params,
        initial_limit: Option<i64>,
    ) -> Self {
        let remaining_limit = params.limit().unwrap_or(i64::MAX);
        Self {
            config,
            params,
            remaining_limit,
            initial_limit: initial_limit.unwrap_or(crate::MAX_RECORD_TRANSFER_COUNT),
            current_page: Vec::new().into_iter(),
            finished: false,
        }
    }

    fn fetch_next_page(&mut self) -> Result<bool, apis::Error<T::ListError>> {
        if self.finished || (self.remaining_limit != i64::MAX && self.remaining_limit <= 0) {
            return Ok(false);
        }

        let page_limit = std::cmp::min(self.remaining_limit, self.initial_limit);
        let response = T::fetch_page(&self.config, &self.params, page_limit)?;

        if let Some(items) = response.items {
            let items_to_take = if self.remaining_limit == i64::MAX {
                items.len()
            } else {
                std::cmp::min(items.len() as i64, self.remaining_limit) as usize
            };
            let taken_items: Vec<T> = items.into_iter().take(items_to_take).collect();

            if self.remaining_limit != i64::MAX {
                self.remaining_limit -= taken_items.len() as i64;
            }

            let new_offset = self.params.offset() + taken_items.len() as i64;
            self.params.set_offset(new_offset);
            self.current_page = taken_items.into_iter();

            if !response.has_more || (self.remaining_limit != i64::MAX && self.remaining_limit <= 0)
            {
                self.finished = true;
            }
            Ok(true)
        } else {
            self.finished = true;
            Ok(false)
        }
    }
}

impl<T: Paginatable> Iterator for PaginatedIterator<T> {
    type Item = Result<T, apis::Error<T::ListError>>;

    fn next(&mut self) -> Option<Self::Item> {
        // Try to get next item from current page
        if let Some(item) = self.current_page.next() {
            return Some(Ok(item));
        }

        // If current page is exhausted, try to fetch next page
        if !self.finished {
            match self.fetch_next_page() {
                Ok(true) => self.current_page.next().map(Ok),
                Ok(false) => None,
                Err(e) => Some(Err(e)),
            }
        } else {
            None
        }
    }
}

/// Helper function to collect all paginated results into a Vec.
///
/// This is a convenience function for when you need all results at once.
pub fn paginate<T: Paginatable>(
    config: &apis::configuration::Configuration,
    params: T::Params,
) -> Result<Vec<T>, apis::Error<T::ListError>> {
    let initial_limit = params.limit().unwrap_or(crate::MAX_RECORD_TRANSFER_COUNT);
    let iter = PaginatedIterator::<T>::new(config.clone(), params, Some(initial_limit));
    iter.collect()
}
