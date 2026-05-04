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

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
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

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
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
fn test_claim_jobs_based_on_resources_prefers_gpu_jobs_with_equal_priority(
    start_server: &ServerProcess,
) {
    let config = &start_server.config;
    let workflow = models::WorkflowModel::new(
        "equal_priority_gpu_preference_test".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    let cpu_rr = create_test_resource_requirements(
        config,
        workflow_id,
        "cpu_rr_equal_priority",
        1,
        0,
        1,
        "1g",
        "PT5M",
    );
    let gpu_rr = create_test_resource_requirements(
        config,
        workflow_id,
        "gpu_rr_equal_priority",
        1,
        1,
        1,
        "1g",
        "PT5M",
    );

    for i in 0..4 {
        let mut job =
            models::JobModel::new(workflow_id, format!("cpu_job_{i}"), format!("echo cpu {i}"));
        job.resource_requirements_id = Some(cpu_rr.id.unwrap());
        apis::jobs_api::create_job(config, job).expect("Failed to create CPU job");
    }

    for i in 0..2 {
        let mut job =
            models::JobModel::new(workflow_id, format!("gpu_job_{i}"), format!("echo gpu {i}"));
        job.resource_requirements_id = Some(gpu_rr.id.unwrap());
        apis::jobs_api::create_job(config, job).expect("Failed to create GPU job");
    }

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    let resources = models::ComputeNodesResources::new(4, 8.0, 2, 1);
    let result =
        apis::workflows_api::claim_jobs_based_on_resources(config, workflow_id, 4, resources, None)
            .expect("claim_jobs_based_on_resources should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    let returned_names: Vec<&str> = returned_jobs.iter().map(|job| job.name.as_str()).collect();
    let gpu_job_count = returned_jobs
        .iter()
        .filter(|job| job.name.starts_with("gpu_job_"))
        .count();

    assert_eq!(returned_jobs.len(), 4);
    assert_eq!(
        gpu_job_count, 2,
        "Equal-priority GPU jobs should not be starved by earlier CPU-only jobs: {:?}",
        returned_names
    );
    assert!(
        returned_jobs[0].name.starts_with("gpu_job_")
            && returned_jobs[1].name.starts_with("gpu_job_"),
        "GPU jobs should be considered before CPU-only jobs when priority is equal: {:?}",
        returned_names
    );
}

#[rstest]
fn test_claim_jobs_based_on_resources_scans_past_limit_for_runnable_jobs(
    start_server: &ServerProcess,
) {
    let config = &start_server.config;
    let workflow =
        models::WorkflowModel::new("scan_past_limit_test".to_string(), "test_user".to_string());
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    let unfit_rr =
        create_test_resource_requirements(config, workflow_id, "unfit_rr", 4, 0, 1, "8g", "PT10M");
    let fit_rr =
        create_test_resource_requirements(config, workflow_id, "fit_rr", 1, 0, 1, "1g", "PT10M");

    for i in 0..3 {
        let mut job = models::JobModel::new(
            workflow_id,
            format!("unfit_job_{i}"),
            format!("echo unfit {i}"),
        );
        job.priority = Some(100);
        job.resource_requirements_id = Some(unfit_rr.id.unwrap());
        apis::jobs_api::create_job(config, job).expect("Failed to create unfit job");
    }

    let mut fit_job =
        models::JobModel::new(workflow_id, "fit_job".to_string(), "echo fit".to_string());
    fit_job.priority = Some(50);
    fit_job.resource_requirements_id = Some(fit_rr.id.unwrap());
    apis::jobs_api::create_job(config, fit_job).expect("Failed to create fit job");

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    let resources = models::ComputeNodesResources::new(1, 1.0, 0, 1);
    let result =
        apis::workflows_api::claim_jobs_based_on_resources(config, workflow_id, 2, resources, None)
            .expect("claim_jobs_based_on_resources should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(returned_jobs.len(), 1);
    assert_eq!(returned_jobs[0].name, "fit_job");
    assert_eq!(returned_jobs[0].priority, Some(50));
}

#[rstest]
fn test_claim_jobs_based_on_resources_backfills_after_gpu_saturates(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = models::WorkflowModel::new(
        "gpu_saturation_backfill_test".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    let gpu_rr =
        create_test_resource_requirements(config, workflow_id, "gpu_rr", 1, 1, 1, "1g", "PT10M");
    let cpu_rr =
        create_test_resource_requirements(config, workflow_id, "cpu_rr", 1, 0, 1, "1g", "PT10M");

    for i in 0..20 {
        let mut job = models::JobModel::new(
            workflow_id,
            format!("high_priority_gpu_{i}"),
            format!("echo gpu {i}"),
        );
        job.priority = Some(100);
        job.resource_requirements_id = Some(gpu_rr.id.unwrap());
        apis::jobs_api::create_job(config, job).expect("Failed to create GPU job");
    }

    for i in 0..3 {
        let mut job = models::JobModel::new(
            workflow_id,
            format!("lower_priority_cpu_{i}"),
            format!("echo cpu {i}"),
        );
        job.priority = Some(10);
        job.resource_requirements_id = Some(cpu_rr.id.unwrap());
        apis::jobs_api::create_job(config, job).expect("Failed to create CPU job");
    }

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    let resources = models::ComputeNodesResources::new(4, 4.0, 1, 1);
    let result =
        apis::workflows_api::claim_jobs_based_on_resources(config, workflow_id, 4, resources, None)
            .expect("claim_jobs_based_on_resources should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    let gpu_count = returned_jobs
        .iter()
        .filter(|job| job.name.starts_with("high_priority_gpu_"))
        .count();
    let cpu_count = returned_jobs
        .iter()
        .filter(|job| job.name.starts_with("lower_priority_cpu_"))
        .count();

    assert_eq!(returned_jobs.len(), 4);
    assert_eq!(gpu_count, 1);
    assert_eq!(cpu_count, 3);
}

#[rstest]
fn test_claim_jobs_based_on_resources_backfill_uses_relaxed_scheduler_fallback(
    start_server: &ServerProcess,
) {
    let config = &start_server.config;
    let workflow = models::WorkflowModel::new(
        "gpu_saturation_relaxed_scheduler_backfill_test".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    let gpu_rr = create_test_resource_requirements(
        config,
        workflow_id,
        "relaxed_gpu_rr",
        1,
        1,
        1,
        "1g",
        "PT10M",
    );
    let cpu_rr = create_test_resource_requirements(
        config,
        workflow_id,
        "relaxed_cpu_rr",
        1,
        0,
        1,
        "1g",
        "PT10M",
    );

    for i in 0..20 {
        let mut job = models::JobModel::new(
            workflow_id,
            format!("relaxed_high_priority_gpu_{i}"),
            format!("echo gpu {i}"),
        );
        job.priority = Some(100);
        job.scheduler_id = Some(7);
        job.resource_requirements_id = Some(gpu_rr.id.unwrap());
        apis::jobs_api::create_job(config, job).expect("Failed to create GPU job");
    }

    for i in 0..3 {
        let mut job = models::JobModel::new(
            workflow_id,
            format!("relaxed_lower_priority_cpu_{i}"),
            format!("echo cpu {i}"),
        );
        job.priority = Some(10);
        job.scheduler_id = Some(7);
        job.resource_requirements_id = Some(cpu_rr.id.unwrap());
        apis::jobs_api::create_job(config, job).expect("Failed to create CPU job");
    }

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    let mut resources = models::ComputeNodesResources::new(4, 4.0, 1, 1);
    resources.scheduler_config_id = Some(99);
    let result = apis::workflows_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        4,
        resources,
        Some(false),
    )
    .expect("claim_jobs_based_on_resources should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    let gpu_count = returned_jobs
        .iter()
        .filter(|job| job.name.starts_with("relaxed_high_priority_gpu_"))
        .count();
    let cpu_count = returned_jobs
        .iter()
        .filter(|job| job.name.starts_with("relaxed_lower_priority_cpu_"))
        .count();

    assert_eq!(returned_jobs.len(), 4);
    assert_eq!(gpu_count, 1);
    assert_eq!(cpu_count, 3);
}

#[rstest]
fn test_claim_jobs_based_on_resources_backfill_short_circuits_when_saturated(
    start_server: &ServerProcess,
) {
    let config = &start_server.config;
    let workflow = models::WorkflowModel::new(
        "backfill_saturated_resources_test".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    let gpu_rr = create_test_resource_requirements(
        config,
        workflow_id,
        "saturating_gpu_rr",
        1,
        1,
        1,
        "1g",
        "PT10M",
    );
    let cpu_rr = create_test_resource_requirements(
        config,
        workflow_id,
        "saturated_cpu_rr",
        1,
        0,
        1,
        "1g",
        "PT10M",
    );

    let mut gpu_job = models::JobModel::new(
        workflow_id,
        "saturating_gpu_job".to_string(),
        "echo gpu".to_string(),
    );
    gpu_job.priority = Some(100);
    gpu_job.resource_requirements_id = Some(gpu_rr.id.unwrap());
    apis::jobs_api::create_job(config, gpu_job).expect("Failed to create GPU job");

    for i in 0..3 {
        let mut job = models::JobModel::new(
            workflow_id,
            format!("cannot_backfill_cpu_{i}"),
            format!("echo cpu {i}"),
        );
        job.priority = Some(10);
        job.resource_requirements_id = Some(cpu_rr.id.unwrap());
        apis::jobs_api::create_job(config, job).expect("Failed to create CPU job");
    }

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    let resources = models::ComputeNodesResources::new(1, 1.0, 1, 1);
    let result =
        apis::workflows_api::claim_jobs_based_on_resources(config, workflow_id, 4, resources, None)
            .expect("claim_jobs_based_on_resources should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(returned_jobs.len(), 1);
    assert_eq!(returned_jobs[0].name, "saturating_gpu_job");
}

#[rstest]
fn test_claim_jobs_based_on_resources_backfill_enforces_per_node_limits(
    start_server: &ServerProcess,
) {
    let config = &start_server.config;
    let workflow = models::WorkflowModel::new(
        "backfill_per_node_limit_test".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    let gpu_rr = create_test_resource_requirements(
        config,
        workflow_id,
        "per_node_gpu_rr",
        1,
        1,
        1,
        "1g",
        "PT10M",
    );
    let too_large_rr = create_test_resource_requirements(
        config,
        workflow_id,
        "too_large_for_one_node_rr",
        5,
        0,
        1,
        "1g",
        "PT10M",
    );
    let small_rr = create_test_resource_requirements(
        config,
        workflow_id,
        "per_node_small_rr",
        1,
        0,
        1,
        "1g",
        "PT10M",
    );

    for i in 0..4 {
        let mut job = models::JobModel::new(
            workflow_id,
            format!("per_node_gpu_job_{i}"),
            format!("echo gpu {i}"),
        );
        job.priority = Some(100);
        job.resource_requirements_id = Some(gpu_rr.id.unwrap());
        apis::jobs_api::create_job(config, job).expect("Failed to create GPU job");
    }

    let mut too_large_job = models::JobModel::new(
        workflow_id,
        "too_large_for_one_node".to_string(),
        "echo too-large".to_string(),
    );
    too_large_job.priority = Some(90);
    too_large_job.resource_requirements_id = Some(too_large_rr.id.unwrap());
    apis::jobs_api::create_job(config, too_large_job).expect("Failed to create oversized job");

    let mut small_job = models::JobModel::new(
        workflow_id,
        "fits_one_node".to_string(),
        "echo small".to_string(),
    );
    small_job.priority = Some(10);
    small_job.resource_requirements_id = Some(small_rr.id.unwrap());
    apis::jobs_api::create_job(config, small_job).expect("Failed to create small job");

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    let resources = models::ComputeNodesResources::new(4, 4.0, 1, 2);
    let result =
        apis::workflows_api::claim_jobs_based_on_resources(config, workflow_id, 4, resources, None)
            .expect("claim_jobs_based_on_resources should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    let returned_names: Vec<&str> = returned_jobs.iter().map(|job| job.name.as_str()).collect();

    assert!(
        !returned_names.contains(&"too_large_for_one_node"),
        "Backfill must not claim a job that only fits aggregate capacity, not one node: {:?}",
        returned_names
    );
    assert!(
        returned_names.contains(&"fits_one_node"),
        "Backfill should still claim lower-priority work that fits one node: {:?}",
        returned_names
    );
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

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
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
