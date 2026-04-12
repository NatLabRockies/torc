mod common;

use common::{
    ServerProcess, create_minimal_resources_workflow, create_test_resource_requirements,
    start_server,
};
use rstest::rstest;
use torc::client::apis;
use torc::client::workflow_spec::ExecutionConfig;
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
fn test_claim_jobs_based_on_resources_invalid_limit_does_not_poison_connection(
    start_server: &ServerProcess,
) {
    let config = &start_server.config;
    let jobs = create_minimal_resources_workflow(config, true);
    let workflow_id = jobs
        .values()
        .next()
        .expect("Should have at least one job")
        .workflow_id;

    let resources = models::ComputeNodesResources::new(2, 2.0, 0, 1);
    let invalid_result = apis::workflows_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        -1,
        resources.clone(),
        None,
    );
    assert!(
        invalid_result.is_err(),
        "negative limits should be rejected before selecting jobs"
    );

    let valid_result =
        apis::workflows_api::claim_jobs_based_on_resources(config, workflow_id, 1, resources, None)
            .expect("connection should remain usable after invalid limit");

    let returned_jobs = valid_result.jobs.expect("Server must return jobs array");
    assert_eq!(returned_jobs.len(), 1);
    assert_eq!(returned_jobs[0].status, Some(models::JobStatus::Pending));
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
fn test_claim_jobs_based_on_resources_backfills_beyond_first_candidate_page(
    start_server: &ServerProcess,
) {
    let config = &start_server.config;
    let workflow = models::WorkflowModel::new(
        "priority_backfill_paging_test".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    let high_rr = create_test_resource_requirements(
        config,
        workflow_id,
        "high_priority_rr",
        3,
        1,
        1,
        "3g",
        "PT10M",
    );
    let low_rr = create_test_resource_requirements(
        config,
        workflow_id,
        "low_priority_rr",
        1,
        0,
        1,
        "1g",
        "PT10M",
    );

    for i in 0..257 {
        let mut job = models::JobModel::new(
            workflow_id,
            format!("high_priority_gpu_{i}"),
            format!("echo high {i}"),
        );
        job.priority = Some(100);
        job.resource_requirements_id = Some(high_rr.id.unwrap());
        apis::jobs_api::create_job(config, job).expect("Failed to create high-priority job");
    }

    let mut low_job = models::JobModel::new(
        workflow_id,
        "lower_priority_cpu_backfill".to_string(),
        "echo low".to_string(),
    );
    low_job.priority = Some(10);
    low_job.resource_requirements_id = Some(low_rr.id.unwrap());
    apis::jobs_api::create_job(config, low_job).expect("Failed to create low-priority job");

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to initialize jobs");

    let resources = models::ComputeNodesResources::new(4, 4.0, 1, 1);
    let result =
        apis::workflows_api::claim_jobs_based_on_resources(config, workflow_id, 2, resources, None)
            .expect("claim_jobs_based_on_resources should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(returned_jobs.len(), 2);
    assert_eq!(returned_jobs[0].priority, Some(100));
    assert_eq!(returned_jobs[1].name, "lower_priority_cpu_backfill");
    assert_eq!(returned_jobs[1].priority, Some(10));
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

fn create_downstream_buffer_test_workflow(
    config: &apis::configuration::Configuration,
    workflow_name: &str,
    downstream_buffer_multiplier: u32,
    setup_job_count: usize,
) -> i64 {
    let mut workflow =
        models::WorkflowModel::new(workflow_name.to_string(), "test_user".to_string());
    workflow.execution_config = Some(
        serde_json::to_string(&ExecutionConfig {
            downstream_buffer_multiplier: Some(downstream_buffer_multiplier),
            ..Default::default()
        })
        .expect("execution config should serialize"),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    let setup_rr =
        create_test_resource_requirements(config, workflow_id, "setup_rr", 1, 0, 1, "1g", "PT5M");
    let gpu_rr =
        create_test_resource_requirements(config, workflow_id, "gpu_rr", 1, 1, 1, "1g", "PT5M");

    for i in 0..setup_job_count {
        let mut setup =
            models::JobModel::new(workflow_id, format!("setup_{i}"), format!("echo setup {i}"));
        setup.resource_requirements_id = Some(setup_rr.id.unwrap());
        let setup = apis::jobs_api::create_job(config, setup).expect("Failed to create setup job");
        let setup_id = setup.id.unwrap();

        let mut gpu =
            models::JobModel::new(workflow_id, format!("gpu_{i}"), format!("echo gpu {i}"));
        gpu.resource_requirements_id = Some(gpu_rr.id.unwrap());
        gpu.depends_on_job_ids = Some(vec![setup_id]);
        apis::jobs_api::create_job(config, gpu).expect("Failed to create gpu job");
    }

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to initialize jobs");

    workflow_id
}

fn create_active_compute_node(
    config: &apis::configuration::Configuration,
    workflow_id: i64,
    num_cpus: i64,
    memory_gb: f64,
    num_gpus: i64,
) {
    let mut compute_node = models::ComputeNodeModel::new(
        workflow_id,
        "gpu-node".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        num_cpus,
        memory_gb,
        num_gpus,
        1,
        "slurm".to_string(),
        None,
    );
    compute_node.is_active = Some(true);
    apis::compute_nodes_api::create_compute_node(config, compute_node)
        .expect("Failed to create active compute node");
}

#[rstest]
fn test_claim_jobs_based_on_resources_downstream_buffer_inactive_without_active_compute_nodes(
    start_server: &ServerProcess,
) {
    let config = &start_server.config;
    let workflow_id =
        create_downstream_buffer_test_workflow(config, "downstream_buffer_inactive_test", 2, 6);

    let resources = models::ComputeNodesResources::new(6, 6.0, 0, 1);
    let result =
        apis::workflows_api::claim_jobs_based_on_resources(config, workflow_id, 6, resources, None)
            .expect("claim_jobs_based_on_resources should succeed");

    let returned_jobs = result.jobs.expect("Server must return jobs array");
    assert_eq!(returned_jobs.len(), 6);
    assert!(
        returned_jobs
            .iter()
            .all(|job| job.name.starts_with("setup_"))
    );
}

#[rstest]
fn test_claim_jobs_based_on_resources_downstream_buffer_uses_active_compute_node_capacity(
    start_server: &ServerProcess,
) {
    let config = &start_server.config;
    let workflow_id =
        create_downstream_buffer_test_workflow(config, "downstream_buffer_active_test", 2, 6);
    create_active_compute_node(config, workflow_id, 8, 8.0, 2);

    let resources = models::ComputeNodesResources::new(6, 6.0, 0, 1);
    let first = apis::workflows_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        6,
        resources.clone(),
        None,
    )
    .expect("first claim should succeed");
    let first_jobs = first.jobs.expect("Server must return jobs array");
    assert_eq!(first_jobs.len(), 4);
    assert!(first_jobs.iter().all(|job| job.name.starts_with("setup_")));

    let second =
        apis::workflows_api::claim_jobs_based_on_resources(config, workflow_id, 6, resources, None)
            .expect("second claim should succeed");
    let second_jobs = second.jobs.expect("Server must return jobs array");
    assert_eq!(second_jobs.len(), 0);
}
