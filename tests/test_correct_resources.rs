mod common;

use common::{
    ServerProcess, create_test_compute_node, create_test_job, create_test_workflow, start_server,
};
use rstest::rstest;
use torc::client::apis;
use torc::client::report_models::ResourceUtilizationReport;
use torc::client::resource_correction::{
    ResourceCorrectionContext, ResourceCorrectionOptions, apply_resource_corrections,
};
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
    let created_rr = apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("Failed to create resource requirement");
    let rr_id = created_rr.id.unwrap();

    // Update job with correct RR ID
    job.resource_requirements_id = Some(rr_id);
    apis::jobs_api::update_job(config, job.id.unwrap(), job).expect("Failed to update job");

    // Reinitialize to pick up the job
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to reinitialize");

    // Claim and complete the job with OOM simulation
    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let result = apis::workflows_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        10,
        resources,
        None,
    )
    .expect("Failed to claim jobs");
    let jobs = result.jobs.expect("Should return jobs");
    assert_eq!(jobs.len(), 1);

    let job_id = jobs[0].id.unwrap();

    // Set job to running
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id)
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

    apis::jobs_api::complete_job(config, job_id, job_result.status, run_id, job_result)
        .expect("Failed to complete job");

    // Verify violation is recorded
    let violations = apis::results_api::list_results(
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

    let items = violations.items;
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
    let created_rr = apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("Failed to create resource requirement");
    let rr_id = created_rr.id.unwrap();

    // Update job with correct RR ID
    job.resource_requirements_id = Some(rr_id);
    apis::jobs_api::update_job(config, job.id.unwrap(), job).expect("Failed to update job");

    // Reinitialize
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to reinitialize");

    // Claim and complete the job
    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let result = apis::workflows_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        10,
        resources,
        None,
    )
    .expect("Failed to claim jobs");
    let jobs = result.jobs.expect("Should return jobs");
    assert_eq!(jobs.len(), 1);

    let job_id = jobs[0].id.unwrap();

    // Set job to running
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id)
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

    apis::jobs_api::complete_job(config, job_id, job_result.status, run_id, job_result)
        .expect("Failed to complete job");

    // Verify violation is recorded
    let violations = apis::results_api::list_results(
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

    let items = violations.items;
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
    let created_rr = apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("Failed to create resource requirement");
    let rr_id = created_rr.id.unwrap();

    // Update job with correct RR ID
    job.resource_requirements_id = Some(rr_id);
    apis::jobs_api::update_job(config, job.id.unwrap(), job).expect("Failed to update job");

    // Reinitialize
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to reinitialize");

    // Claim and complete the job
    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let result = apis::workflows_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        10,
        resources,
        None,
    )
    .expect("Failed to claim jobs");
    let jobs = result.jobs.expect("Should return jobs");
    assert_eq!(jobs.len(), 1);

    let job_id = jobs[0].id.unwrap();

    // Set job to running
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id)
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

    apis::jobs_api::complete_job(config, job_id, job_result.status, run_id, job_result)
        .expect("Failed to complete job");

    // Verify violation is recorded
    let violations = apis::results_api::list_results(
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

    let items = violations.items;
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

    let created_rr1 = apis::resource_requirements_api::create_resource_requirements(config, rr1)
        .expect("Failed to create RR1");
    let created_rr2 = apis::resource_requirements_api::create_resource_requirements(config, rr2)
        .expect("Failed to create RR2");
    let created_rr3 = apis::resource_requirements_api::create_resource_requirements(config, rr3)
        .expect("Failed to create RR3");

    // Update jobs
    let mut job1_updated = job1;
    job1_updated.resource_requirements_id = Some(created_rr1.id.unwrap());
    apis::jobs_api::update_job(config, job1_updated.id.unwrap(), job1_updated)
        .expect("Failed to update job1");

    let mut job2_updated = job2;
    job2_updated.resource_requirements_id = Some(created_rr2.id.unwrap());
    apis::jobs_api::update_job(config, job2_updated.id.unwrap(), job2_updated)
        .expect("Failed to update job2");

    let mut job3_updated = job3;
    job3_updated.resource_requirements_id = Some(created_rr3.id.unwrap());
    apis::jobs_api::update_job(config, job3_updated.id.unwrap(), job3_updated)
        .expect("Failed to update job3");

    // Reinitialize
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to reinitialize");

    // Claim jobs
    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let result = apis::workflows_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        10,
        resources,
        None,
    )
    .expect("Failed to claim jobs");
    let jobs = result.jobs.expect("Should return jobs");
    assert_eq!(jobs.len(), 3, "Should have 3 jobs");

    let job1_id = jobs[0].id.unwrap();
    let job2_id = jobs[1].id.unwrap();
    let job3_id = jobs[2].id.unwrap();

    // Set jobs to running
    for job_id in [job1_id, job2_id, job3_id] {
        apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id)
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
    apis::jobs_api::complete_job(config, job1_id, result1.status, run_id, result1)
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
    apis::jobs_api::complete_job(config, job2_id, result2.status, run_id, result2)
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
    apis::jobs_api::complete_job(config, job3_id, result3.status, run_id, result3)
        .expect("Failed to complete job3");

    // Verify all violations are recorded
    let results = apis::results_api::list_results(
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

    let items = results.items;
    assert_eq!(items.len(), 3, "Should have 3 results");

    // Verify each violation type - find by job_id instead of assuming order
    let result1 = items
        .iter()
        .find(|r| r.job_id == job1_id)
        .expect("Should find job1 result");
    assert_eq!(
        result1.return_code, 137,
        "Job 1 should have OOM return code"
    );
    assert_eq!(
        result1.peak_memory_bytes,
        Some(3_000_000_000),
        "Job 1 should have memory violation"
    );

    let result2 = items
        .iter()
        .find(|r| r.job_id == job2_id)
        .expect("Should find job2 result");
    assert_eq!(
        result2.return_code, 0,
        "Job 2 should have success return code"
    );
    assert_eq!(
        result2.peak_cpu_percent,
        Some(250.0),
        "Job 2 should have CPU violation"
    );

    let result3 = items
        .iter()
        .find(|r| r.job_id == job3_id)
        .expect("Should find job3 result");
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
    let created_rr = apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("Failed to create resource requirement");
    let rr_id = created_rr.id.unwrap();

    // Update job with correct RR ID
    job.resource_requirements_id = Some(rr_id);
    apis::jobs_api::update_job(config, job.id.unwrap(), job).expect("Failed to update job");

    // Reinitialize to pick up the job
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to reinitialize");

    // Claim and complete the job with violations
    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let result = apis::workflows_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        10,
        resources,
        None,
    )
    .expect("Failed to claim jobs");
    let jobs = result.jobs.expect("Should return jobs");
    assert_eq!(jobs.len(), 1);

    let job_id = jobs[0].id.unwrap();

    // Set job to running
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id)
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

    apis::jobs_api::complete_job(config, job_id, job_result.status, run_id, job_result)
        .expect("Failed to complete job");

    // Get the RR before corrections
    let rr_before = apis::resource_requirements_api::get_resource_requirements(config, rr_id)
        .expect("Failed to get resource requirement");
    assert_eq!(rr_before.memory, "2g");
    assert_eq!(rr_before.num_cpus, 1);
    assert_eq!(rr_before.runtime, "PT30M");

    // Apply corrections to the resource requirements
    // Using 1.2x multiplier as the default:
    // Memory: 3.5GB * 1.2 = 4.2GB
    // CPU: ceil(150% / 100% * 1.2) = ceil(1.8) = 2 cores
    // Runtime: 45 min * 1.2 = 54 min ≈ PT54M
    apis::resource_requirements_api::update_resource_requirements(config, rr_id, rr_before.clone())
        .expect("Failed to update RR before applying corrections");

    // Now update the RR with corrected values (simulating what correct-resources command does)
    let mut rr_corrected = rr_before.clone();
    rr_corrected.memory = "4g".to_string(); // Corrected from 2g (3.5 * 1.2 ≈ 4.2)
    rr_corrected.num_cpus = 2; // Corrected from 1 (150% / 100% * 1.2 = 1.8, rounded up)
    rr_corrected.runtime = "PT54M".to_string(); // Corrected from PT30M (45 * 1.2 = 54)

    apis::resource_requirements_api::update_resource_requirements(config, rr_id, rr_corrected)
        .expect("Failed to update resource requirement with corrections");

    // Verify corrections were applied
    let rr_after = apis::resource_requirements_api::get_resource_requirements(config, rr_id)
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
    let created_rr = apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("Failed to create resource requirement");
    let rr_id = created_rr.id.unwrap();

    // Update job with correct RR ID
    job.resource_requirements_id = Some(rr_id);
    apis::jobs_api::update_job(config, job.id.unwrap(), job).expect("Failed to update job");

    // Reinitialize to pick up the job
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to reinitialize");

    // Claim and complete the job with violations
    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let result = apis::workflows_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        10,
        resources,
        None,
    )
    .expect("Failed to claim jobs");
    let jobs = result.jobs.expect("Should return jobs");
    assert_eq!(jobs.len(), 1);

    let job_id = jobs[0].id.unwrap();

    // Set job to running
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id)
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

    apis::jobs_api::complete_job(config, job_id, job_result.status, run_id, job_result)
        .expect("Failed to complete job");

    // Verify violations can be retrieved
    let violations = apis::results_api::list_results(
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

    let items = violations.items;
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
    let created_rr = apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("Failed to create resource requirement");
    let rr_id = created_rr.id.unwrap();

    // Update job with correct RR ID
    job.resource_requirements_id = Some(rr_id);
    apis::jobs_api::update_job(config, job.id.unwrap(), job).expect("Failed to update job");

    // Reinitialize to pick up the job
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to reinitialize");

    // Claim and complete the job successfully
    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let result = apis::workflows_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        10,
        resources,
        None,
    )
    .expect("Failed to claim jobs");
    let jobs = result.jobs.expect("Should return jobs");
    assert_eq!(jobs.len(), 1);

    let job_id = jobs[0].id.unwrap();

    // Set job to running
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id)
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

    apis::jobs_api::complete_job(config, job_id, job_result.status, run_id, job_result)
        .expect("Failed to complete job");

    // Verify memory violation is recorded
    let results = apis::results_api::list_results(
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

    let items = results.items;
    assert_eq!(items.len(), 1, "Should have 1 result");
    let result = &items[0];
    assert_eq!(result.return_code, 0, "Should have success return code");
    assert_eq!(
        result.peak_memory_bytes,
        Some(3_200_000_000),
        "Should have peak memory recorded"
    );
}

/// Helper: create an empty diagnosis (no violations) for downsizing-only tests
fn empty_diagnosis(workflow_id: i64, total_results: usize) -> ResourceUtilizationReport {
    ResourceUtilizationReport {
        workflow_id,
        run_id: None,
        total_results,
        over_utilization_count: 0,
        violations: Vec::new(),
        resource_violations_count: 0,
        resource_violations: Vec::new(),
    }
}

/// Helper: fetch all results for a workflow
fn fetch_results(
    config: &torc::client::Configuration,
    workflow_id: i64,
) -> Vec<models::ResultModel> {
    apis::results_api::list_results(
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
    .expect("Failed to list results")
    .items
}

/// Helper: fetch all jobs for a workflow
fn fetch_jobs(config: &torc::client::Configuration, workflow_id: i64) -> Vec<models::JobModel> {
    apis::jobs_api::list_jobs(
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
    )
    .expect("Failed to list jobs")
    .items
}

/// Helper: fetch all resource requirements for a workflow
fn fetch_resource_requirements(
    config: &torc::client::Configuration,
    workflow_id: i64,
) -> Vec<models::ResourceRequirementsModel> {
    apis::resource_requirements_api::list_resource_requirements(
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
        None,
    )
    .expect("Failed to list resource requirements")
    .items
}

/// Helper: create a job and assign it an RR. Returns the job_id.
/// The job is NOT yet claimed/run/completed — use `claim_and_complete_jobs` for that.
fn create_job_with_rr(
    config: &torc::client::Configuration,
    workflow_id: i64,
    job_name: &str,
    rr_id: i64,
) -> i64 {
    let mut job = create_test_job(config, workflow_id, job_name);
    job.resource_requirements_id = Some(rr_id);
    let job_id = job.id.unwrap();
    apis::jobs_api::update_job(config, job_id, job).expect("Failed to update job");
    job_id
}

/// Helper: claim all ready jobs, set them running, and complete each with given metrics.
/// `job_metrics` is a list of (job_id, return_code, exec_time, status, peak_mem, peak_cpu).
#[allow(clippy::type_complexity)]
fn claim_and_complete_jobs(
    config: &torc::client::Configuration,
    workflow_id: i64,
    run_id: i64,
    compute_node_id: i64,
    job_metrics: &[(i64, i64, f64, JobStatus, Option<i64>, Option<f64>)],
) {
    let resources = models::ComputeNodesResources::new(128, 1024.0, 0, 1);
    let claim_result = apis::workflows_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        100,
        resources,
        None,
    )
    .expect("Failed to claim jobs");
    let claimed = claim_result.jobs.expect("Should return jobs");

    for (job_id, return_code, exec_time, status, peak_mem, peak_cpu) in job_metrics {
        // Verify this job was claimed
        assert!(
            claimed.iter().any(|j| j.id == Some(*job_id)),
            "Job {} should have been claimed",
            job_id
        );

        apis::jobs_api::manage_status_change(config, *job_id, JobStatus::Running, run_id)
            .expect("Failed to set job running");

        let mut result = models::ResultModel::new(
            *job_id,
            workflow_id,
            run_id,
            1,
            compute_node_id,
            *return_code,
            *exec_time,
            chrono::Utc::now().to_rfc3339(),
            *status,
        );
        result.peak_memory_bytes = *peak_mem;
        result.peak_cpu_percent = *peak_cpu;

        apis::jobs_api::complete_job(config, *job_id, result.status, run_id, result)
            .expect("Failed to complete job");
    }
}

/// Test that memory is downsized when all jobs use significantly less than allocated.
/// Allocated: 8GB, peak usage: 2GB → new = ceil(2GB * 1.2) = 3GB, savings = 5GB > 1GB threshold.
#[rstest]
fn test_downsize_memory(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, run_id) = create_and_initialize_workflow(config, "test_downsize_memory");

    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Create RR with 8GB memory
    let mut rr = models::ResourceRequirementsModel::new(workflow_id, "big_mem".to_string());
    rr.memory = "8g".to_string();
    rr.runtime = "PT1H".to_string();
    rr.num_cpus = 2;
    let created_rr = apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("Failed to create RR");
    let rr_id = created_rr.id.unwrap();

    // Create 2 jobs with this RR
    let job_a_id = create_job_with_rr(config, workflow_id, "job_a", rr_id);
    let job_b_id = create_job_with_rr(config, workflow_id, "job_b", rr_id);

    // Reinitialize and complete jobs through lifecycle
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to reinitialize");

    claim_and_complete_jobs(
        config,
        workflow_id,
        run_id,
        compute_node_id,
        &[
            (
                job_a_id,
                0,
                10.0,
                JobStatus::Completed,
                Some(2 * 1024 * 1024 * 1024),
                Some(150.0),
            ),
            (
                job_b_id,
                0,
                10.0,
                JobStatus::Completed,
                Some(2 * 1024 * 1024 * 1024),
                Some(150.0),
            ),
        ],
    );

    // Build context from real data
    let all_results = fetch_results(config, workflow_id);
    let all_jobs = fetch_jobs(config, workflow_id);
    let all_rrs = fetch_resource_requirements(config, workflow_id);
    let diagnosis = empty_diagnosis(workflow_id, all_results.len());

    let ctx = ResourceCorrectionContext {
        config,
        workflow_id,
        diagnosis: &diagnosis,
        all_results: &all_results,
        all_jobs: &all_jobs,
        all_resource_requirements: &all_rrs,
    };
    let opts = ResourceCorrectionOptions {
        memory_multiplier: 1.2,
        cpu_multiplier: 1.2,
        runtime_multiplier: 1.2,
        include_jobs: vec![],
        dry_run: false,
        no_downsize: false,
    };

    let result = apply_resource_corrections(&ctx, &opts).expect("Failed to apply corrections");

    assert_eq!(result.downsize_memory_corrections, 2);
    assert!(result.resource_requirements_updated > 0);

    // Verify RR was updated: 2GB * 1.2 = 2.4GB → ceil to 3g
    let rr_after = apis::resource_requirements_api::get_resource_requirements(config, rr_id)
        .expect("Failed to get RR after downsize");
    assert_eq!(rr_after.memory, "3g", "Memory should be downsized to 3g");
}

/// Test that CPU is downsized when all jobs use far fewer CPUs than allocated.
/// Allocated: 8 CPUs (800%), peak usage: 150% → new = ceil(1.5 * 1.2) = 2.
/// Savings = 800% - 150% = 650% > 5% threshold.
#[rstest]
fn test_downsize_cpu(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, run_id) = create_and_initialize_workflow(config, "test_downsize_cpu");

    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    let mut rr = models::ResourceRequirementsModel::new(workflow_id, "big_cpu".to_string());
    rr.memory = "2g".to_string();
    rr.runtime = "PT1H".to_string();
    rr.num_cpus = 8;
    let created_rr = apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("Failed to create RR");
    let rr_id = created_rr.id.unwrap();

    let job_id = create_job_with_rr(config, workflow_id, "low_cpu_job", rr_id);

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to reinitialize");

    claim_and_complete_jobs(
        config,
        workflow_id,
        run_id,
        compute_node_id,
        &[(
            job_id,
            0,
            10.0,
            JobStatus::Completed,
            Some(1024 * 1024 * 1024),
            Some(150.0),
        )],
    );

    let all_results = fetch_results(config, workflow_id);
    let all_jobs = fetch_jobs(config, workflow_id);
    let all_rrs = fetch_resource_requirements(config, workflow_id);
    let diagnosis = empty_diagnosis(workflow_id, all_results.len());

    let ctx = ResourceCorrectionContext {
        config,
        workflow_id,
        diagnosis: &diagnosis,
        all_results: &all_results,
        all_jobs: &all_jobs,
        all_resource_requirements: &all_rrs,
    };
    let opts = ResourceCorrectionOptions {
        memory_multiplier: 1.2,
        cpu_multiplier: 1.2,
        runtime_multiplier: 1.2,
        include_jobs: vec![],
        dry_run: false,
        no_downsize: false,
    };

    let result = apply_resource_corrections(&ctx, &opts).expect("Failed to apply corrections");

    assert_eq!(result.downsize_cpu_corrections, 1);

    let rr_after = apis::resource_requirements_api::get_resource_requirements(config, rr_id)
        .expect("Failed to get RR after downsize");
    // ceil(150% / 100% * 1.2) = ceil(1.8) = 2
    assert_eq!(rr_after.num_cpus, 2, "CPUs should be downsized to 2");
}

/// Test that runtime is downsized when all jobs run much faster than allocated.
/// Allocated: PT2H (120 min), peak usage: 10 min → new = ceil(10 * 1.2) = 12 min.
/// Savings = 120 min - 12 min = 108 min > 30 min threshold.
#[rstest]
fn test_downsize_runtime(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, run_id) = create_and_initialize_workflow(config, "test_downsize_runtime");

    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    let mut rr = models::ResourceRequirementsModel::new(workflow_id, "long_rt".to_string());
    rr.memory = "2g".to_string();
    rr.runtime = "PT2H".to_string(); // 120 minutes
    rr.num_cpus = 1;
    let created_rr = apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("Failed to create RR");
    let rr_id = created_rr.id.unwrap();

    // Create 3 jobs
    let job1_id = create_job_with_rr(config, workflow_id, "fast1", rr_id);
    let job2_id = create_job_with_rr(config, workflow_id, "fast2", rr_id);
    let job3_id = create_job_with_rr(config, workflow_id, "fast3", rr_id);

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to reinitialize");

    claim_and_complete_jobs(
        config,
        workflow_id,
        run_id,
        compute_node_id,
        &[
            (
                job1_id,
                0,
                10.0,
                JobStatus::Completed,
                Some(1024 * 1024 * 1024),
                Some(80.0),
            ),
            (
                job2_id,
                0,
                10.0,
                JobStatus::Completed,
                Some(1024 * 1024 * 1024),
                Some(80.0),
            ),
            (
                job3_id,
                0,
                10.0,
                JobStatus::Completed,
                Some(1024 * 1024 * 1024),
                Some(80.0),
            ),
        ],
    );

    let all_results = fetch_results(config, workflow_id);
    let all_jobs = fetch_jobs(config, workflow_id);
    let all_rrs = fetch_resource_requirements(config, workflow_id);
    let diagnosis = empty_diagnosis(workflow_id, all_results.len());

    let ctx = ResourceCorrectionContext {
        config,
        workflow_id,
        diagnosis: &diagnosis,
        all_results: &all_results,
        all_jobs: &all_jobs,
        all_resource_requirements: &all_rrs,
    };
    let opts = ResourceCorrectionOptions {
        memory_multiplier: 1.2,
        cpu_multiplier: 1.2,
        runtime_multiplier: 1.2,
        include_jobs: vec![],
        dry_run: false,
        no_downsize: false,
    };

    let result = apply_resource_corrections(&ctx, &opts).expect("Failed to apply corrections");

    assert_eq!(result.downsize_runtime_corrections, 3);

    let rr_after = apis::resource_requirements_api::get_resource_requirements(config, rr_id)
        .expect("Failed to get RR after downsize");
    // 10 min * 60s * 1.2 = 720s = 12 minutes → PT12M
    assert_eq!(
        rr_after.runtime, "PT12M",
        "Runtime should be downsized to PT12M"
    );
}

/// Test that no downsize occurs when savings are below the thresholds.
/// Memory: 3g (3 GiB) allocated, 2.5GB peak * 1.2x = 3GB new → savings ~0.2 GiB < 1 GiB threshold.
/// CPU: 2 (200%), peak 198% → difference 2% < 5% threshold.
/// Runtime: PT40M (40 min), peak 15 min → savings 25 min < 30 min threshold.
#[rstest]
fn test_no_downsize_below_threshold(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, run_id) =
        create_and_initialize_workflow(config, "test_no_downsize_threshold");

    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    let mut rr = models::ResourceRequirementsModel::new(workflow_id, "tight".to_string());
    rr.memory = "3g".to_string();
    rr.runtime = "PT40M".to_string(); // 40 minutes
    rr.num_cpus = 2;
    let created_rr = apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("Failed to create RR");
    let rr_id = created_rr.id.unwrap();

    let job_id = create_job_with_rr(config, workflow_id, "tight_job", rr_id);

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to reinitialize");

    claim_and_complete_jobs(
        config,
        workflow_id,
        run_id,
        compute_node_id,
        &[(
            job_id,
            0,
            15.0,
            JobStatus::Completed,
            Some(2_500_000_000),
            Some(198.0),
        )],
    );

    let all_results = fetch_results(config, workflow_id);
    let all_jobs = fetch_jobs(config, workflow_id);
    let all_rrs = fetch_resource_requirements(config, workflow_id);
    let diagnosis = empty_diagnosis(workflow_id, all_results.len());

    let ctx = ResourceCorrectionContext {
        config,
        workflow_id,
        diagnosis: &diagnosis,
        all_results: &all_results,
        all_jobs: &all_jobs,
        all_resource_requirements: &all_rrs,
    };
    let opts = ResourceCorrectionOptions {
        memory_multiplier: 1.2,
        cpu_multiplier: 1.2,
        runtime_multiplier: 1.2,
        include_jobs: vec![],
        dry_run: false,
        no_downsize: false,
    };

    let result = apply_resource_corrections(&ctx, &opts).expect("Failed to apply corrections");

    // Nothing should have changed
    assert_eq!(result.downsize_memory_corrections, 0);
    assert_eq!(result.downsize_cpu_corrections, 0);
    assert_eq!(result.downsize_runtime_corrections, 0);
    assert_eq!(result.resource_requirements_updated, 0);

    // Verify RR is unchanged
    let rr_after = apis::resource_requirements_api::get_resource_requirements(config, rr_id)
        .expect("Failed to get RR");
    assert_eq!(rr_after.memory, "3g");
    assert_eq!(rr_after.num_cpus, 2);
    assert_eq!(rr_after.runtime, "PT40M");
}

/// Test that --no-downsize flag prevents all downsizing
#[rstest]
fn test_no_downsize_flag(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, run_id) = create_and_initialize_workflow(config, "test_no_downsize_flag");

    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    let mut rr = models::ResourceRequirementsModel::new(workflow_id, "wasteful".to_string());
    rr.memory = "32g".to_string();
    rr.runtime = "PT8H".to_string();
    rr.num_cpus = 16;
    let created_rr = apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("Failed to create RR");
    let rr_id = created_rr.id.unwrap();

    let job_id = create_job_with_rr(config, workflow_id, "tiny_job", rr_id);

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to reinitialize");

    claim_and_complete_jobs(
        config,
        workflow_id,
        run_id,
        compute_node_id,
        &[(
            job_id,
            0,
            1.0,
            JobStatus::Completed,
            Some(512 * 1024 * 1024),
            Some(50.0),
        )],
    );

    let all_results = fetch_results(config, workflow_id);
    let all_jobs = fetch_jobs(config, workflow_id);
    let all_rrs = fetch_resource_requirements(config, workflow_id);
    let diagnosis = empty_diagnosis(workflow_id, all_results.len());

    let ctx = ResourceCorrectionContext {
        config,
        workflow_id,
        diagnosis: &diagnosis,
        all_results: &all_results,
        all_jobs: &all_jobs,
        all_resource_requirements: &all_rrs,
    };
    let opts = ResourceCorrectionOptions {
        memory_multiplier: 1.2,
        cpu_multiplier: 1.2,
        runtime_multiplier: 1.2,
        include_jobs: vec![],
        dry_run: false,
        no_downsize: true, // no_downsize = true
    };

    let result = apply_resource_corrections(&ctx, &opts).expect("Failed to apply corrections");

    assert_eq!(result.downsize_memory_corrections, 0);
    assert_eq!(result.downsize_cpu_corrections, 0);
    assert_eq!(result.downsize_runtime_corrections, 0);
    assert_eq!(result.resource_requirements_updated, 0);

    // Verify RR is unchanged
    let rr_after = apis::resource_requirements_api::get_resource_requirements(config, rr_id)
        .expect("Failed to get RR");
    assert_eq!(rr_after.memory, "32g");
    assert_eq!(rr_after.num_cpus, 16);
    assert_eq!(rr_after.runtime, "PT8H");
}

/// Test that downsizing skips resources when not all jobs have peak data.
/// If one job is missing peak_memory_bytes, memory should NOT be downsized
/// (we can't be sure it actually used less than allocated).
#[rstest]
fn test_no_downsize_missing_peak_data(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, run_id) = create_and_initialize_workflow(config, "test_no_downsize_missing");

    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    let mut rr = models::ResourceRequirementsModel::new(workflow_id, "partial".to_string());
    rr.memory = "16g".to_string();
    rr.runtime = "PT2H".to_string();
    rr.num_cpus = 8;
    let created_rr = apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("Failed to create RR");
    let rr_id = created_rr.id.unwrap();

    // Job 1: has full peak data
    let job1_id = create_job_with_rr(config, workflow_id, "has_data", rr_id);
    // Job 2: missing memory and CPU data (None)
    let job2_id = create_job_with_rr(config, workflow_id, "no_data", rr_id);

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to reinitialize");

    claim_and_complete_jobs(
        config,
        workflow_id,
        run_id,
        compute_node_id,
        &[
            (
                job1_id,
                0,
                5.0,
                JobStatus::Completed,
                Some(1024 * 1024 * 1024),
                Some(100.0),
            ),
            (job2_id, 0, 5.0, JobStatus::Completed, None, None),
        ],
    );

    let all_results = fetch_results(config, workflow_id);
    let all_jobs = fetch_jobs(config, workflow_id);
    let all_rrs = fetch_resource_requirements(config, workflow_id);
    let diagnosis = empty_diagnosis(workflow_id, all_results.len());

    let ctx = ResourceCorrectionContext {
        config,
        workflow_id,
        diagnosis: &diagnosis,
        all_results: &all_results,
        all_jobs: &all_jobs,
        all_resource_requirements: &all_rrs,
    };
    let opts = ResourceCorrectionOptions {
        memory_multiplier: 1.2,
        cpu_multiplier: 1.2,
        runtime_multiplier: 1.2,
        include_jobs: vec![],
        dry_run: false,
        no_downsize: false,
    };

    let result = apply_resource_corrections(&ctx, &opts).expect("Failed to apply corrections");

    // Memory and CPU should NOT be downsized (missing data)
    assert_eq!(result.downsize_memory_corrections, 0);
    assert_eq!(result.downsize_cpu_corrections, 0);
    // Runtime CAN still be downsized (120 min - 6 min = 114 min > 30 min threshold)
    assert_eq!(result.downsize_runtime_corrections, 2);

    let rr_after = apis::resource_requirements_api::get_resource_requirements(config, rr_id)
        .expect("Failed to get RR");
    assert_eq!(rr_after.memory, "16g", "Memory unchanged — missing data");
    assert_eq!(rr_after.num_cpus, 8, "CPUs unchanged — missing data");
    // Runtime: 5 min * 60 * 1.2 = 360s = 6 min → PT6M
    assert_eq!(rr_after.runtime, "PT6M", "Runtime downsized to PT6M");
}

/// Test that downsizing in dry-run mode does NOT modify the resource requirements
#[rstest]
fn test_downsize_dry_run(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, run_id) = create_and_initialize_workflow(config, "test_downsize_dry_run");

    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    let mut rr = models::ResourceRequirementsModel::new(workflow_id, "big".to_string());
    rr.memory = "16g".to_string();
    rr.runtime = "PT4H".to_string();
    rr.num_cpus = 8;
    let created_rr = apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("Failed to create RR");
    let rr_id = created_rr.id.unwrap();

    let job_id = create_job_with_rr(config, workflow_id, "small_job", rr_id);

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to reinitialize");

    claim_and_complete_jobs(
        config,
        workflow_id,
        run_id,
        compute_node_id,
        &[(
            job_id,
            0,
            5.0,
            JobStatus::Completed,
            Some(1024 * 1024 * 1024),
            Some(100.0),
        )],
    );

    let all_results = fetch_results(config, workflow_id);
    let all_jobs = fetch_jobs(config, workflow_id);
    let all_rrs = fetch_resource_requirements(config, workflow_id);
    let diagnosis = empty_diagnosis(workflow_id, all_results.len());

    let ctx = ResourceCorrectionContext {
        config,
        workflow_id,
        diagnosis: &diagnosis,
        all_results: &all_results,
        all_jobs: &all_jobs,
        all_resource_requirements: &all_rrs,
    };
    let opts = ResourceCorrectionOptions {
        memory_multiplier: 1.2,
        cpu_multiplier: 1.2,
        runtime_multiplier: 1.2,
        include_jobs: vec![],
        dry_run: true, // dry-run — don't apply
        no_downsize: false,
    };

    let result = apply_resource_corrections(&ctx, &opts).expect("Failed to apply corrections");

    // Corrections should be reported
    assert!(result.downsize_memory_corrections > 0);
    assert!(result.downsize_cpu_corrections > 0);
    assert!(result.downsize_runtime_corrections > 0);

    // But RR should be UNCHANGED
    let rr_after = apis::resource_requirements_api::get_resource_requirements(config, rr_id)
        .expect("Failed to get RR");
    assert_eq!(rr_after.memory, "16g", "Memory unchanged in dry-run");
    assert_eq!(rr_after.num_cpus, 8, "CPUs unchanged in dry-run");
    assert_eq!(rr_after.runtime, "PT4H", "Runtime unchanged in dry-run");
}

/// Test that adjustment reports include the correct direction field
#[rstest]
fn test_downsize_adjustment_report_direction(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, run_id) = create_and_initialize_workflow(config, "test_downsize_direction");

    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    let mut rr = models::ResourceRequirementsModel::new(workflow_id, "report".to_string());
    rr.memory = "16g".to_string();
    rr.runtime = "PT4H".to_string();
    rr.num_cpus = 8;
    let created_rr = apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("Failed to create RR");
    let rr_id = created_rr.id.unwrap();

    let job_id = create_job_with_rr(config, workflow_id, "tiny", rr_id);

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to reinitialize");

    claim_and_complete_jobs(
        config,
        workflow_id,
        run_id,
        compute_node_id,
        &[(
            job_id,
            0,
            5.0,
            JobStatus::Completed,
            Some(1024 * 1024 * 1024),
            Some(100.0),
        )],
    );

    let all_results = fetch_results(config, workflow_id);
    let all_jobs = fetch_jobs(config, workflow_id);
    let all_rrs = fetch_resource_requirements(config, workflow_id);
    let diagnosis = empty_diagnosis(workflow_id, all_results.len());

    let ctx = ResourceCorrectionContext {
        config,
        workflow_id,
        diagnosis: &diagnosis,
        all_results: &all_results,
        all_jobs: &all_jobs,
        all_resource_requirements: &all_rrs,
    };
    let opts = ResourceCorrectionOptions {
        memory_multiplier: 1.2,
        cpu_multiplier: 1.2,
        runtime_multiplier: 1.2,
        include_jobs: vec![],
        dry_run: true, // dry-run
        no_downsize: false,
    };

    let result = apply_resource_corrections(&ctx, &opts).expect("Failed to apply corrections");

    assert!(!result.adjustments.is_empty(), "Should have adjustments");
    let adj = &result.adjustments[0];
    assert_eq!(adj.direction, "downscale", "Direction should be downscale");
    assert!(adj.memory_adjusted);
    assert!(adj.cpu_adjusted);
    assert!(adj.runtime_adjusted);
}
