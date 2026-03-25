mod common;

use common::{
    ServerProcess, create_minimal_resources_workflow, create_test_resource_requirements,
    start_server,
};
use rstest::rstest;
use torc::client::apis;
use torc::models;

#[rstest]
fn test_claim_jobs_based_on_resources_honors_limit(start_server: &ServerProcess) {
    let config = &start_server.config;
    let jobs = create_minimal_resources_workflow(config, true);
    let workflow_id = jobs
        .values()
        .next()
        .expect("Should have at least one job")
        .workflow_id;

    let resources = models::ComputeNodesResources::new(2, 2.0, 0, 1);
    let result =
        apis::workflows_api::claim_jobs_based_on_resources(config, workflow_id, 2, resources, None)
            .expect("claim_jobs_based_on_resources should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(returned_jobs.len(), 2);
    for job in returned_jobs {
        assert_eq!(job.status, Some(models::JobStatus::Pending));
    }
}

#[rstest]
fn test_claim_jobs_based_on_resources_priority_ordering(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = models::WorkflowModel::new(
        "priority_resources_test".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    let resource_requirements = create_test_resource_requirements(
        config,
        workflow_id,
        "priority_resources_rr",
        1,
        0,
        1,
        "1g",
        "PT1M",
    );

    for priority in [0i64, 5, 10] {
        let mut job = models::JobModel::new(
            workflow_id,
            format!("priority_job_{priority}"),
            format!("echo priority {priority}"),
        );
        job.priority = Some(priority);
        job.resource_requirements_id = Some(resource_requirements.id.unwrap());
        apis::jobs_api::create_job(config, job).expect("Failed to create job");
    }

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to initialize jobs");

    let resources = models::ComputeNodesResources::new(1, 1.0, 0, 1);

    let first = apis::workflows_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        1,
        resources.clone(),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");
    let first_jobs = first.jobs.expect("Server must return jobs array");
    assert_eq!(first_jobs.len(), 1);
    assert_eq!(first_jobs[0].priority, Some(10));

    let second = apis::workflows_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        1,
        resources.clone(),
        None,
    )
    .expect("claim_jobs_based_on_resources should succeed");
    let second_jobs = second.jobs.expect("Server must return jobs array");
    assert_eq!(second_jobs.len(), 1);
    assert_eq!(second_jobs[0].priority, Some(5));

    let third =
        apis::workflows_api::claim_jobs_based_on_resources(config, workflow_id, 1, resources, None)
            .expect("claim_jobs_based_on_resources should succeed");
    let third_jobs = third.jobs.expect("Server must return jobs array");
    assert_eq!(third_jobs.len(), 1);
    assert_eq!(third_jobs[0].priority, Some(0));
}

#[rstest]
fn test_claim_jobs_based_on_resources_skips_high_priority_job_that_does_not_fit(
    start_server: &ServerProcess,
) {
    let config = &start_server.config;
    let workflow =
        models::WorkflowModel::new("priority_fit_test".to_string(), "test_user".to_string());
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    let gpu_rr =
        create_test_resource_requirements(config, workflow_id, "gpu_rr", 4, 1, 1, "8g", "PT10M");
    let cpu_rr =
        create_test_resource_requirements(config, workflow_id, "cpu_rr", 1, 0, 1, "1g", "PT10M");

    let mut gpu_job = models::JobModel::new(
        workflow_id,
        "high_priority_gpu".to_string(),
        "echo gpu".to_string(),
    );
    gpu_job.priority = Some(100);
    gpu_job.resource_requirements_id = Some(gpu_rr.id.unwrap());
    apis::jobs_api::create_job(config, gpu_job).expect("Failed to create GPU job");

    let mut cpu_job = models::JobModel::new(
        workflow_id,
        "lower_priority_cpu".to_string(),
        "echo cpu".to_string(),
    );
    cpu_job.priority = Some(10);
    cpu_job.resource_requirements_id = Some(cpu_rr.id.unwrap());
    apis::jobs_api::create_job(config, cpu_job).expect("Failed to create CPU job");

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to initialize jobs");

    let resources = models::ComputeNodesResources::new(1, 1.0, 0, 1);
    let result =
        apis::workflows_api::claim_jobs_based_on_resources(config, workflow_id, 2, resources, None)
            .expect("claim_jobs_based_on_resources should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(returned_jobs.len(), 1);
    assert_eq!(returned_jobs[0].name, "lower_priority_cpu");
    assert_eq!(returned_jobs[0].priority, Some(10));
}

#[rstest]
fn test_claim_jobs_based_on_resources_strict_scheduler_match_controls_fallback(
    start_server: &ServerProcess,
) {
    let config = &start_server.config;
    let workflow = models::WorkflowModel::new(
        "strict_scheduler_match_test".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    let rr = create_test_resource_requirements(
        config,
        workflow_id,
        "strict_scheduler_rr",
        1,
        0,
        1,
        "1g",
        "PT5M",
    );

    let mut job = models::JobModel::new(
        workflow_id,
        "scheduler_bound_job".to_string(),
        "echo scheduler".to_string(),
    );
    job.resource_requirements_id = Some(rr.id.unwrap());
    job.scheduler_id = Some(7);
    apis::jobs_api::create_job(config, job).expect("Failed to create job");

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to initialize jobs");

    let mut resources = models::ComputeNodesResources::new(1, 1.0, 0, 1);
    resources.scheduler_config_id = Some(99);

    let strict = apis::workflows_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        1,
        resources.clone(),
        Some(true),
    )
    .expect("strict claim should succeed");
    assert_eq!(
        strict.jobs.expect("Server must return jobs array").len(),
        0,
        "strict scheduler matching should not fall back to jobs with a different scheduler_id",
    );

    let relaxed = apis::workflows_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        1,
        resources,
        Some(false),
    )
    .expect("relaxed claim should succeed");
    let returned_jobs = relaxed.jobs.expect("Server must return jobs array");
    assert_eq!(returned_jobs.len(), 1);
    assert_eq!(returned_jobs[0].name, "scheduler_bound_job");
}
