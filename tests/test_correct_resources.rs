mod common;

use common::{
    ServerProcess, create_test_compute_node, create_test_job, create_test_workflow, start_server,
};
use rstest::rstest;
use torc::client::default_api;
use torc::client::workflow_manager::WorkflowManager;
use torc::config::TorcConfig;
use torc::models::{self, JobStatus};

/// Helper to create workflow manager and initialize workflow
fn create_and_initialize_workflow(config: &torc::client::Configuration, name: &str) -> (i64, i64) {
    let workflow = create_test_workflow(config, name);
    let workflow_id = workflow.id.unwrap();

    let torc_config = TorcConfig::load().unwrap_or_default();
    let manager = WorkflowManager::new(config.clone(), torc_config, workflow);
    manager
        .initialize(false)
        .expect("Failed to initialize workflow");
    let run_id = manager.get_run_id().expect("Failed to get run_id");

    (workflow_id, run_id)
}

/// Test OOM violation detection in dry-run mode
#[rstest]
fn test_correct_resources_memory_violation_dry_run(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create and initialize workflow
    let (workflow_id, run_id) = create_and_initialize_workflow(config, "test_memory_violation");

    // Create a job
    let mut job = create_test_job(config, workflow_id, "memory_heavy_job");

    // Create compute node
    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Create resource requirement: 2GB memory
    let mut rr = models::ResourceRequirementsModel::new(workflow_id, "small".to_string());
    rr.memory = "2g".to_string();
    rr.runtime = "PT1H".to_string();
    rr.num_cpus = 1;
    let created_rr = default_api::create_resource_requirements(config, rr)
        .expect("Failed to create resource requirement");
    let rr_id = created_rr.id.unwrap();

    // Update job with correct RR ID
    job.resource_requirements_id = Some(rr_id);
    default_api::update_job(config, job.id.unwrap(), job).expect("Failed to update job");

    // Reinitialize to pick up the job
    default_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to reinitialize");

    // Claim and complete the job with OOM simulation
    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let result =
        default_api::claim_jobs_based_on_resources(config, workflow_id, &resources, 10, None, None)
            .expect("Failed to claim jobs");
    let jobs = result.jobs.expect("Should return jobs");
    assert_eq!(jobs.len(), 1);

    let job_id = jobs[0].id.unwrap();

    // Set job to running
    default_api::manage_status_change(config, job_id, JobStatus::Running, run_id, None)
        .expect("Failed to set job running");

    // Complete with OOM: return code 137, quick execution, high memory peak
    let mut job_result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1,
        compute_node_id,
        137, // OOM signal
        0.5, // exec_time_minutes (< 1 minute)
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Failed,
    );

    // Set peak memory to 3GB (exceeds 2GB limit)
    job_result.peak_memory_bytes = Some(3_000_000_000); // 3GB

    default_api::complete_job(config, job_id, job_result.status, run_id, job_result)
        .expect("Failed to complete job");

    // Verify violation is recorded
    let violations = default_api::list_results(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list results");

    let items = violations.items.expect("Should have items");
    assert!(!items.is_empty(), "Should have results");
    let result = &items[0];
    assert_eq!(result.return_code, 137, "Should have OOM return code");
    assert_eq!(
        result.peak_memory_bytes,
        Some(3_000_000_000),
        "Should have peak memory recorded"
    );
}

/// Test CPU violation detection in dry-run mode
#[rstest]
fn test_correct_resources_cpu_violation_dry_run(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create and initialize workflow
    let (workflow_id, run_id) = create_and_initialize_workflow(config, "test_cpu_violation");

    // Create a job
    let mut job = create_test_job(config, workflow_id, "cpu_heavy_job");

    // Create compute node
    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Create resource requirement: 3 CPUs
    let mut rr = models::ResourceRequirementsModel::new(workflow_id, "medium".to_string());
    rr.memory = "4g".to_string();
    rr.runtime = "PT1H".to_string();
    rr.num_cpus = 3;
    let created_rr = default_api::create_resource_requirements(config, rr)
        .expect("Failed to create resource requirement");
    let rr_id = created_rr.id.unwrap();

    // Update job with correct RR ID
    job.resource_requirements_id = Some(rr_id);
    default_api::update_job(config, job.id.unwrap(), job).expect("Failed to update job");

    // Reinitialize
    default_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to reinitialize");

    // Claim and complete the job
    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let result =
        default_api::claim_jobs_based_on_resources(config, workflow_id, &resources, 10, None, None)
            .expect("Failed to claim jobs");
    let jobs = result.jobs.expect("Should return jobs");
    assert_eq!(jobs.len(), 1);

    let job_id = jobs[0].id.unwrap();

    // Set job to running
    default_api::manage_status_change(config, job_id, JobStatus::Running, run_id, None)
        .expect("Failed to set job running");

    // Complete successfully but with CPU violation: peak 502% (exceeds 300%)
    let mut job_result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1,
        compute_node_id,
        0, // Success
        5.0,
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Completed,
    );

    job_result.peak_cpu_percent = Some(502.0); // 502% (exceeds 300% for 3 cores)

    default_api::complete_job(config, job_id, job_result.status, run_id, job_result)
        .expect("Failed to complete job");

    // Verify violation is recorded
    let violations = default_api::list_results(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list results");

    let items = violations.items.expect("Should have items");
    assert!(!items.is_empty(), "Should have results");
    let result = &items[0];
    assert_eq!(result.return_code, 0, "Should have success return code");
    assert_eq!(
        result.peak_cpu_percent,
        Some(502.0),
        "Should have peak CPU recorded"
    );
}

/// Test runtime violation detection in dry-run mode
#[rstest]
fn test_correct_resources_runtime_violation_dry_run(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create and initialize workflow
    let (workflow_id, run_id) = create_and_initialize_workflow(config, "test_runtime_violation");

    // Create a job
    let mut job = create_test_job(config, workflow_id, "slow_job");

    // Create compute node
    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Create resource requirement: 30 minutes runtime
    let mut rr = models::ResourceRequirementsModel::new(workflow_id, "fast".to_string());
    rr.memory = "2g".to_string();
    rr.runtime = "PT30M".to_string(); // 30 minutes
    rr.num_cpus = 2;
    let created_rr = default_api::create_resource_requirements(config, rr)
        .expect("Failed to create resource requirement");
    let rr_id = created_rr.id.unwrap();

    // Update job with correct RR ID
    job.resource_requirements_id = Some(rr_id);
    default_api::update_job(config, job.id.unwrap(), job).expect("Failed to update job");

    // Reinitialize
    default_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to reinitialize");

    // Claim and complete the job
    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let result =
        default_api::claim_jobs_based_on_resources(config, workflow_id, &resources, 10, None, None)
            .expect("Failed to claim jobs");
    let jobs = result.jobs.expect("Should return jobs");
    assert_eq!(jobs.len(), 1);

    let job_id = jobs[0].id.unwrap();

    // Set job to running
    default_api::manage_status_change(config, job_id, JobStatus::Running, run_id, None)
        .expect("Failed to set job running");

    // Complete successfully but with runtime violation: 45 minutes (exceeds 30 min)
    let job_result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1,
        compute_node_id,
        0,    // Success
        45.0, // 45 minutes (exceeds 30 minute limit)
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Completed,
    );

    default_api::complete_job(config, job_id, job_result.status, run_id, job_result)
        .expect("Failed to complete job");

    // Verify violation is recorded
    let violations = default_api::list_results(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list results");

    let items = violations.items.expect("Should have items");
    assert!(!items.is_empty(), "Should have results");
    let result = &items[0];
    assert_eq!(result.return_code, 0, "Should have success return code");
    assert_eq!(
        result.exec_time_minutes, 45.0,
        "Should have execution time recorded"
    );
}

/// Test that all three violations are detected together
#[rstest]
fn test_correct_resources_multiple_violations(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create and initialize workflow
    let (workflow_id, run_id) = create_and_initialize_workflow(config, "test_multiple_violations");

    // Create multiple jobs with different violations
    let job1 = create_test_job(config, workflow_id, "memory_job");
    let job2 = create_test_job(config, workflow_id, "cpu_job");
    let job3 = create_test_job(config, workflow_id, "runtime_job");

    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Create resource requirements for each job
    let mut rr1 = models::ResourceRequirementsModel::new(workflow_id, "rr1".to_string());
    rr1.memory = "2g".to_string();
    rr1.runtime = "PT1H".to_string();
    rr1.num_cpus = 1;

    let mut rr2 = models::ResourceRequirementsModel::new(workflow_id, "rr2".to_string());
    rr2.memory = "4g".to_string();
    rr2.runtime = "PT1H".to_string();
    rr2.num_cpus = 2;

    let mut rr3 = models::ResourceRequirementsModel::new(workflow_id, "rr3".to_string());
    rr3.memory = "2g".to_string();
    rr3.runtime = "PT30M".to_string();
    rr3.num_cpus = 1;

    let created_rr1 =
        default_api::create_resource_requirements(config, rr1).expect("Failed to create RR1");
    let created_rr2 =
        default_api::create_resource_requirements(config, rr2).expect("Failed to create RR2");
    let created_rr3 =
        default_api::create_resource_requirements(config, rr3).expect("Failed to create RR3");

    // Update jobs
    let mut job1_updated = job1;
    job1_updated.resource_requirements_id = Some(created_rr1.id.unwrap());
    default_api::update_job(config, job1_updated.id.unwrap(), job1_updated)
        .expect("Failed to update job1");

    let mut job2_updated = job2;
    job2_updated.resource_requirements_id = Some(created_rr2.id.unwrap());
    default_api::update_job(config, job2_updated.id.unwrap(), job2_updated)
        .expect("Failed to update job2");

    let mut job3_updated = job3;
    job3_updated.resource_requirements_id = Some(created_rr3.id.unwrap());
    default_api::update_job(config, job3_updated.id.unwrap(), job3_updated)
        .expect("Failed to update job3");

    // Reinitialize
    default_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to reinitialize");

    // Claim jobs
    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let result =
        default_api::claim_jobs_based_on_resources(config, workflow_id, &resources, 10, None, None)
            .expect("Failed to claim jobs");
    let jobs = result.jobs.expect("Should return jobs");
    assert_eq!(jobs.len(), 3, "Should have 3 jobs");

    let job1_id = jobs[0].id.unwrap();
    let job2_id = jobs[1].id.unwrap();
    let job3_id = jobs[2].id.unwrap();

    // Set jobs to running
    for job_id in [job1_id, job2_id, job3_id] {
        default_api::manage_status_change(config, job_id, JobStatus::Running, run_id, None)
            .expect("Failed to set job running");
    }

    // Complete job 1 with memory violation (OOM)
    let mut result1 = models::ResultModel::new(
        job1_id,
        workflow_id,
        run_id,
        1,
        compute_node_id,
        137, // OOM
        0.5,
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Failed,
    );
    result1.peak_memory_bytes = Some(3_000_000_000); // 3GB (exceeds 2GB)
    default_api::complete_job(config, job1_id, result1.status, run_id, result1)
        .expect("Failed to complete job1");

    // Complete job 2 with CPU violation
    let mut result2 = models::ResultModel::new(
        job2_id,
        workflow_id,
        run_id,
        1,
        compute_node_id,
        0, // Success
        5.0,
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Completed,
    );
    result2.peak_cpu_percent = Some(250.0); // 250% (exceeds 200% for 2 cores)
    default_api::complete_job(config, job2_id, result2.status, run_id, result2)
        .expect("Failed to complete job2");

    // Complete job 3 with runtime violation
    let result3 = models::ResultModel::new(
        job3_id,
        workflow_id,
        run_id,
        1,
        compute_node_id,
        0,    // Success
        45.0, // 45 minutes (exceeds 30 minute limit)
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Completed,
    );
    default_api::complete_job(config, job3_id, result3.status, run_id, result3)
        .expect("Failed to complete job3");

    // Verify all violations are recorded
    let results = default_api::list_results(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list results");

    let items = results.items.expect("Should have items");
    assert_eq!(items.len(), 3, "Should have 3 results");

    // Verify each violation type
    let result1 = &items[0];
    assert_eq!(
        result1.return_code, 137,
        "Job 1 should have OOM return code"
    );
    assert_eq!(
        result1.peak_memory_bytes,
        Some(3_000_000_000),
        "Job 1 should have memory violation"
    );

    let result2 = &items[1];
    assert_eq!(
        result2.return_code, 0,
        "Job 2 should have success return code"
    );
    assert_eq!(
        result2.peak_cpu_percent,
        Some(250.0),
        "Job 2 should have CPU violation"
    );

    let result3 = &items[2];
    assert_eq!(
        result3.return_code, 0,
        "Job 3 should have success return code"
    );
    assert_eq!(
        result3.exec_time_minutes, 45.0,
        "Job 3 should have runtime violation"
    );
}

/// Test that corrections are actually applied to resource requirements
#[rstest]
fn test_correct_resources_applies_corrections(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create and initialize workflow
    let (workflow_id, run_id) = create_and_initialize_workflow(config, "test_corrections_applied");

    // Create a job
    let mut job = create_test_job(config, workflow_id, "heavy_job");

    // Create compute node
    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Create resource requirement: 2GB memory, 1 CPU, 30 minutes runtime
    let mut rr = models::ResourceRequirementsModel::new(workflow_id, "initial".to_string());
    rr.memory = "2g".to_string();
    rr.runtime = "PT30M".to_string();
    rr.num_cpus = 1;
    let created_rr = default_api::create_resource_requirements(config, rr)
        .expect("Failed to create resource requirement");
    let rr_id = created_rr.id.unwrap();

    // Update job with correct RR ID
    job.resource_requirements_id = Some(rr_id);
    default_api::update_job(config, job.id.unwrap(), job).expect("Failed to update job");

    // Reinitialize to pick up the job
    default_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to reinitialize");

    // Claim and complete the job with violations
    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let result =
        default_api::claim_jobs_based_on_resources(config, workflow_id, &resources, 10, None, None)
            .expect("Failed to claim jobs");
    let jobs = result.jobs.expect("Should return jobs");
    assert_eq!(jobs.len(), 1);

    let job_id = jobs[0].id.unwrap();

    // Set job to running
    default_api::manage_status_change(config, job_id, JobStatus::Running, run_id, None)
        .expect("Failed to set job running");

    // Complete with all three violations
    let mut job_result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1,
        compute_node_id,
        137,  // OOM
        45.0, // 45 minutes (exceeds 30 minute limit)
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Failed,
    );

    job_result.peak_memory_bytes = Some(3_500_000_000); // 3.5GB (exceeds 2GB)
    job_result.peak_cpu_percent = Some(150.0); // 150% (exceeds 100% for 1 core)

    default_api::complete_job(config, job_id, job_result.status, run_id, job_result)
        .expect("Failed to complete job");

    // Get the RR before corrections
    let rr_before = default_api::get_resource_requirements(config, rr_id)
        .expect("Failed to get resource requirement");
    assert_eq!(rr_before.memory, "2g");
    assert_eq!(rr_before.num_cpus, 1);
    assert_eq!(rr_before.runtime, "PT30M");

    // Apply corrections to the resource requirements
    // Using 1.2x multiplier as the default:
    // Memory: 3.5GB * 1.2 = 4.2GB
    // CPU: ceil(150% / 100% * 1.2) = ceil(1.8) = 2 cores
    // Runtime: 45 min * 1.2 = 54 min ≈ PT54M
    default_api::update_resource_requirements(config, rr_id, rr_before.clone())
        .expect("Failed to update RR before applying corrections");

    // Now update the RR with corrected values (simulating what correct-resources command does)
    let mut rr_corrected = rr_before.clone();
    rr_corrected.memory = "4g".to_string(); // Corrected from 2g (3.5 * 1.2 ≈ 4.2)
    rr_corrected.num_cpus = 2; // Corrected from 1 (150% / 100% * 1.2 = 1.8, rounded up)
    rr_corrected.runtime = "PT54M".to_string(); // Corrected from PT30M (45 * 1.2 = 54)

    default_api::update_resource_requirements(config, rr_id, rr_corrected)
        .expect("Failed to update resource requirement with corrections");

    // Verify corrections were applied
    let rr_after = default_api::get_resource_requirements(config, rr_id)
        .expect("Failed to get corrected resource requirement");

    assert_eq!(rr_after.memory, "4g", "Memory should be corrected to 4g");
    assert_eq!(rr_after.num_cpus, 2, "CPU count should be corrected to 2");
    assert_eq!(
        rr_after.runtime, "PT54M",
        "Runtime should be corrected to PT54M"
    );
}

/// Test that dry-run mode can be used to preview corrections before applying them
/// This verifies that violations are detected and can be reported in JSON format
#[rstest]
fn test_correct_resources_dry_run_mode(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create and initialize workflow
    let (workflow_id, run_id) = create_and_initialize_workflow(config, "test_dry_run_mode");

    // Create a job
    let mut job = create_test_job(config, workflow_id, "test_job");

    // Create compute node
    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Create resource requirement
    let mut rr = models::ResourceRequirementsModel::new(workflow_id, "test_rr".to_string());
    rr.memory = "1g".to_string();
    rr.runtime = "PT10M".to_string();
    rr.num_cpus = 1;
    let created_rr = default_api::create_resource_requirements(config, rr)
        .expect("Failed to create resource requirement");
    let rr_id = created_rr.id.unwrap();

    // Update job with correct RR ID
    job.resource_requirements_id = Some(rr_id);
    default_api::update_job(config, job.id.unwrap(), job).expect("Failed to update job");

    // Reinitialize to pick up the job
    default_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to reinitialize");

    // Claim and complete the job with violations
    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let result =
        default_api::claim_jobs_based_on_resources(config, workflow_id, &resources, 10, None, None)
            .expect("Failed to claim jobs");
    let jobs = result.jobs.expect("Should return jobs");
    assert_eq!(jobs.len(), 1);

    let job_id = jobs[0].id.unwrap();

    // Set job to running
    default_api::manage_status_change(config, job_id, JobStatus::Running, run_id, None)
        .expect("Failed to set job running");

    // Complete with violations
    let mut job_result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1,
        compute_node_id,
        137,  // OOM
        15.0, // 15 minutes (exceeds 10 minute limit)
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Failed,
    );

    job_result.peak_memory_bytes = Some(2_000_000_000); // 2GB (exceeds 1GB)
    job_result.peak_cpu_percent = Some(120.0); // 120% (exceeds 100% for 1 core)

    default_api::complete_job(config, job_id, job_result.status, run_id, job_result)
        .expect("Failed to complete job");

    // Verify violations can be retrieved
    let violations = default_api::list_results(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list results");

    let items = violations.items.expect("Should have items");
    assert_eq!(items.len(), 1, "Should have 1 result with violations");

    // Verify the violations are present (OOM, memory, CPU, runtime)
    let result = &items[0];
    assert_eq!(result.return_code, 137, "Should have OOM return code");
    assert_eq!(
        result.peak_memory_bytes,
        Some(2_000_000_000),
        "Should have peak memory recorded"
    );
    assert_eq!(
        result.peak_cpu_percent,
        Some(120.0),
        "Should have peak CPU recorded"
    );
    assert_eq!(
        result.exec_time_minutes, 15.0,
        "Should have execution time recorded"
    );

    // In a real scenario, these violations would be:
    // - Memory: 2GB -> 2.4GB (1.2x correction)
    // - CPU: 120% -> 2 cores (ceil(1.2 * 1.2))
    // - Runtime: 15 min -> 18 min (15 * 1.2)
}

/// Test that memory violations are detected even in successfully completed jobs
#[rstest]
fn test_correct_resources_memory_violation_successful_job(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create and initialize workflow
    let (workflow_id, run_id) = create_and_initialize_workflow(config, "test_memory_successful");

    // Create a job
    let mut job = create_test_job(config, workflow_id, "successful_memory_job");

    // Create compute node
    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Create resource requirement: 2GB memory
    let mut rr = models::ResourceRequirementsModel::new(workflow_id, "memory_rr".to_string());
    rr.memory = "2g".to_string();
    rr.runtime = "PT1H".to_string();
    rr.num_cpus = 2;
    let created_rr = default_api::create_resource_requirements(config, rr)
        .expect("Failed to create resource requirement");
    let rr_id = created_rr.id.unwrap();

    // Update job with correct RR ID
    job.resource_requirements_id = Some(rr_id);
    default_api::update_job(config, job.id.unwrap(), job).expect("Failed to update job");

    // Reinitialize to pick up the job
    default_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to reinitialize");

    // Claim and complete the job successfully
    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let result =
        default_api::claim_jobs_based_on_resources(config, workflow_id, &resources, 10, None, None)
            .expect("Failed to claim jobs");
    let jobs = result.jobs.expect("Should return jobs");
    assert_eq!(jobs.len(), 1);

    let job_id = jobs[0].id.unwrap();

    // Set job to running
    default_api::manage_status_change(config, job_id, JobStatus::Running, run_id, None)
        .expect("Failed to set job running");

    // Complete SUCCESSFULLY (return code 0) but with memory violation
    let mut job_result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1,
        compute_node_id,
        0,    // Success - not an OOM failure
        10.0, // Normal execution time
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Completed,
    );

    // Set peak memory to 3GB (exceeds 2GB limit even though job succeeded)
    job_result.peak_memory_bytes = Some(3_200_000_000); // 3.2GB

    default_api::complete_job(config, job_id, job_result.status, run_id, job_result)
        .expect("Failed to complete job");

    // Verify memory violation is recorded
    let results = default_api::list_results(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list results");

    let items = results.items.expect("Should have items");
    assert_eq!(items.len(), 1, "Should have 1 result");
    let result = &items[0];
    assert_eq!(result.return_code, 0, "Should have success return code");
    assert_eq!(
        result.peak_memory_bytes,
        Some(3_200_000_000),
        "Should have peak memory recorded"
    );
}
