//! File pagination functionality.
//!
//! This module provides lazy iteration and vector collection support for files
//! using the generic pagination framework.

use crate::client::apis;
use crate::client::commands::pagination::base::{
    Paginatable, PaginatedIterator, PaginatedResponse, PaginationParams,
};
use crate::models::FileModel;

/// Parameters for listing files with default values and builder methods.
#[derive(Debug, Clone, Default)]
pub struct FileListParams {
    /// Workflow ID to list files from
    pub workflow_id: i64,
    /// Filter by job ID that produced the files
    pub produced_by_job_id: Option<i64>,
    /// Pagination offset
    pub offset: i64,
    /// Maximum number of files to return
    pub limit: Option<i64>,
    /// Field to sort by
    pub sort_by: Option<String>,
    /// Reverse sort order
    pub reverse_sort: Option<bool>,
    /// Filter by file name
    pub name: Option<String>,
    /// Filter by file path
    pub path: Option<String>,
    /// Filter by output status
    pub is_output: Option<bool>,
}

impl FileListParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_produced_by_job_id(mut self, job_id: i64) -> Self {
        self.produced_by_job_id = Some(job_id);
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

    pub fn with_is_output(mut self, is_output: bool) -> Self {
        self.is_output = Some(is_output);
        self
    }
}

impl PaginationParams for FileListParams {
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

impl Paginatable for FileModel {
    type ListError = apis::files_api::ListFilesError;
    type Params = FileListParams;

    fn fetch_page(
        config: &apis::configuration::Configuration,
        params: &Self::Params,
        limit: i64,
    ) -> Result<PaginatedResponse<Self>, apis::Error<Self::ListError>> {
        let response = apis::files_api::list_files(
            config,
            params.workflow_id,
            params.produced_by_job_id,
            Some(params.offset),
            Some(limit),
            params.sort_by.as_deref(),
            params.reverse_sort,
            params.name.as_deref(),
            params.path.as_deref(),
            params.is_output,
        )?;

        Ok(PaginatedResponse {
            items: response.items,
            has_more: response.has_more,
        })
    }
}

/// Type alias for the files iterator
pub type FilesIterator = PaginatedIterator<FileModel>;

/// Create a lazy iterator for files that fetches pages on-demand.
///
/// This is memory efficient as it only loads one page at a time.
///
/// # Arguments
/// * `config` - API configuration containing base URL and authentication
/// * `workflow_id` - ID of the workflow to list files from
/// * `params` - FileListParams containing filter and pagination parameters
///
/// # Returns
/// An iterator that yields `Result<FileModel, Error>` items
pub fn iter_files(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: FileListParams,
) -> FilesIterator {
    let mut params = params;
    params.workflow_id = workflow_id;
    PaginatedIterator::new(config.clone(), params, None)
}

/// Collect all files into a vector using lazy iteration internally.
///
/// # Arguments
/// * `config` - API configuration containing base URL and authentication
/// * `workflow_id` - ID of the workflow to list files from
/// * `params` - FileListParams containing filter and pagination parameters
///
/// # Returns
/// `Result<Vec<FileModel>, Error>` containing all files or an error
#[allow(clippy::result_large_err)]
pub fn paginate_files(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: FileListParams,
) -> Result<Vec<FileModel>, apis::Error<apis::files_api::ListFilesError>> {
    iter_files(config, workflow_id, params).collect()
}
