use super::{Error, configuration};
use crate::models;

pub use super::failure_handlers_api::{
    CreateFailureHandlerError, GetFailureHandlerError, ListFailureHandlersError,
};
pub use super::jobs_api::CreateJobsError;
pub use super::resource_requirements_api::{
    CreateResourceRequirementsError, DeleteResourceRequirementError, GetResourceRequirementsError,
    ListResourceRequirementsError, UpdateResourceRequirementsError,
};
pub use super::slurm_stats_api::{CreateSlurmStatsError, ListSlurmStatsError};

pub fn create_resource_requirements(
    configuration: &configuration::Configuration,
    resource_requirements_model: models::ResourceRequirementsModel,
) -> Result<models::ResourceRequirementsModel, Error<CreateResourceRequirementsError>> {
    super::resource_requirements_api::create_resource_requirements(
        configuration,
        resource_requirements_model,
    )
}

pub fn get_resource_requirements(
    configuration: &configuration::Configuration,
    id: i64,
) -> Result<models::ResourceRequirementsModel, Error<GetResourceRequirementsError>> {
    super::resource_requirements_api::get_resource_requirements(configuration, id)
}

pub fn update_resource_requirements(
    configuration: &configuration::Configuration,
    id: i64,
    resource_requirements_model: models::ResourceRequirementsModel,
) -> Result<models::ResourceRequirementsModel, Error<UpdateResourceRequirementsError>> {
    super::resource_requirements_api::update_resource_requirements(
        configuration,
        id,
        resource_requirements_model,
    )
}

pub fn delete_resource_requirement(
    configuration: &configuration::Configuration,
    id: i64,
    _body: Option<serde_json::Value>,
) -> Result<models::ResourceRequirementsModel, Error<DeleteResourceRequirementError>> {
    super::resource_requirements_api::delete_resource_requirement(configuration, id)
}

#[allow(clippy::too_many_arguments)]
pub fn list_resource_requirements(
    configuration: &configuration::Configuration,
    workflow_id: i64,
    job_id: Option<i64>,
    name: Option<&str>,
    memory: Option<&str>,
    num_cpus: Option<i64>,
    num_gpus: Option<i64>,
    num_nodes: Option<i64>,
    runtime: Option<i64>,
    offset: Option<i64>,
    limit: Option<i64>,
    sort_by: Option<&str>,
    reverse_sort: Option<bool>,
) -> Result<models::ListResourceRequirementsResponse, Error<ListResourceRequirementsError>> {
    super::resource_requirements_api::list_resource_requirements(
        configuration,
        workflow_id,
        job_id,
        name,
        memory,
        num_cpus,
        num_gpus,
        num_nodes,
        runtime,
        offset,
        limit,
        sort_by,
        reverse_sort,
    )
}

pub fn create_failure_handler(
    configuration: &configuration::Configuration,
    failure_handler_model: models::FailureHandlerModel,
) -> Result<models::FailureHandlerModel, Error<CreateFailureHandlerError>> {
    super::failure_handlers_api::create_failure_handler(configuration, failure_handler_model)
}

pub fn get_failure_handler(
    configuration: &configuration::Configuration,
    id: i64,
) -> Result<models::FailureHandlerModel, Error<GetFailureHandlerError>> {
    super::failure_handlers_api::get_failure_handler(configuration, id)
}

pub fn list_failure_handlers(
    configuration: &configuration::Configuration,
    id: i64,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<models::ListFailureHandlersResponse, Error<ListFailureHandlersError>> {
    super::failure_handlers_api::list_failure_handlers(configuration, id, offset, limit)
}

pub fn create_jobs(
    configuration: &configuration::Configuration,
    jobs_model: models::JobsModel,
) -> Result<models::CreateJobsResponse, Error<CreateJobsError>> {
    super::jobs_api::create_jobs(configuration, jobs_model)
}

pub fn create_slurm_stats(
    configuration: &configuration::Configuration,
    slurm_stats_model: models::SlurmStatsModel,
) -> Result<models::SlurmStatsModel, Error<CreateSlurmStatsError>> {
    super::slurm_stats_api::create_slurm_stats(configuration, slurm_stats_model)
}

pub fn list_slurm_stats(
    configuration: &configuration::Configuration,
    workflow_id: i64,
    job_id: Option<i64>,
    run_id: Option<i64>,
    attempt_id: Option<i64>,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<models::ListSlurmStatsResponse, Error<ListSlurmStatsError>> {
    super::slurm_stats_api::list_slurm_stats(
        configuration,
        workflow_id,
        job_id,
        run_id,
        attempt_id,
        offset,
        limit,
    )
}
