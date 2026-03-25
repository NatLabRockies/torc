//! Resource requirements pagination functionality.
//!
//! This module provides lazy iteration and vector collection support for resource requirements
//! using the generic pagination framework.

use crate::client::apis;
use crate::client::commands::pagination::base::{
    Paginatable, PaginatedIterator, PaginatedResponse, PaginationParams,
};
use crate::models::ResourceRequirementsModel;
use crate::time_utils::duration_string_to_seconds;

/// Parameters for listing resource requirements with default values and builder methods.
#[derive(Debug, Clone, Default)]
pub struct ResourceRequirementsListParams {
    /// Workflow ID to list resource requirements from
    pub workflow_id: i64,
    /// Filter by job ID
    pub job_id: Option<i64>,
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
    /// Filter by memory
    pub memory: Option<String>,
    /// Filter by number of CPUs
    pub num_cpus: Option<i64>,
    /// Filter by number of GPUs
    pub num_gpus: Option<i64>,
    /// Filter by number of nodes
    pub num_nodes: Option<i64>,
    /// Filter by runtime
    pub runtime: Option<String>,
}

impl ResourceRequirementsListParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_job_id(mut self, job_id: i64) -> Self {
        self.job_id = Some(job_id);
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

    pub fn with_memory(mut self, memory: String) -> Self {
        self.memory = Some(memory);
        self
    }

    pub fn with_num_cpus(mut self, num_cpus: i64) -> Self {
        self.num_cpus = Some(num_cpus);
        self
    }

    pub fn with_num_gpus(mut self, num_gpus: i64) -> Self {
        self.num_gpus = Some(num_gpus);
        self
    }

    pub fn with_num_nodes(mut self, num_nodes: i64) -> Self {
        self.num_nodes = Some(num_nodes);
        self
    }

    pub fn with_runtime(mut self, runtime: String) -> Self {
        self.runtime = Some(runtime);
        self
    }
}

impl PaginationParams for ResourceRequirementsListParams {
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

impl Paginatable for ResourceRequirementsModel {
    type ListError = apis::resource_requirements_api::ListResourceRequirementsError;
    type Params = ResourceRequirementsListParams;

    fn fetch_page(
        config: &apis::configuration::Configuration,
        params: &Self::Params,
        limit: i64,
    ) -> Result<PaginatedResponse<Self>, apis::Error<Self::ListError>> {
        let response = apis::resource_requirements_api::list_resource_requirements(
            config,
            params.workflow_id,
            params.job_id,
            params.name.as_deref(),
            params.memory.as_deref(),
            params.num_cpus,
            params.num_gpus,
            params.num_nodes,
            match params.runtime.as_deref() {
                Some(runtime) => Some(
                    duration_string_to_seconds(runtime)
                        .or_else(|_| runtime.parse::<i64>().map_err(|e| e.to_string()))
                        .map_err(|e| apis::Error::Io(std::io::Error::other(e)))?,
                ),
                None => None,
            },
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

/// Type alias for the resource requirements iterator
pub type ResourceRequirementsIterator = PaginatedIterator<ResourceRequirementsModel>;

/// Create a lazy iterator for resource requirements that fetches pages on-demand.
///
/// # Arguments
/// * `config` - API configuration
/// * `workflow_id` - ID of the workflow to list resource requirements from
/// * `params` - ResourceRequirementsListParams containing filter and pagination parameters
///
/// # Returns
/// An iterator that yields `Result<ResourceRequirementsModel, Error>` items
pub fn iter_resource_requirements(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: ResourceRequirementsListParams,
) -> ResourceRequirementsIterator {
    let mut params = params;
    params.workflow_id = workflow_id;
    PaginatedIterator::new(config.clone(), params, None)
}

/// Collect all resource requirements into a vector using lazy iteration internally.
///
/// # Arguments
/// * `config` - API configuration
/// * `workflow_id` - ID of the workflow to list resource requirements from
/// * `params` - ResourceRequirementsListParams containing filter and pagination parameters
///
/// # Returns
/// `Result<Vec<ResourceRequirementsModel>, Error>` containing all resource requirements or an error
#[allow(clippy::result_large_err)]
pub fn paginate_resource_requirements(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    params: ResourceRequirementsListParams,
) -> Result<
    Vec<ResourceRequirementsModel>,
    apis::Error<apis::resource_requirements_api::ListResourceRequirementsError>,
> {
    iter_resource_requirements(config, workflow_id, params).collect()
}
