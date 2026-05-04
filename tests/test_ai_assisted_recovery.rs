mod common;

use common::{
    ServerProcess, create_test_compute_node, create_test_job, create_test_workflow, start_server,
};
use rstest::rstest;
use std::fs;
use std::path::PathBuf;
use torc::client::apis;
use torc::client::commands::recover::invoke_ai_agent;
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

/// Test that jobs can be set to pending_failed status via complete_job
#[rstest]
fn test_pending_failed_status(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create and initialize workflow
    let (workflow_id, run_id) = create_and_initialize_workflow(config, "test_pending_failed");

    // Create job after initialization
    let job = create_test_job(config, workflow_id, "test_job");
    let job_id = job.id.unwrap();

    // Create compute node
    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Reinitialize to pick up the new job
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to reinitialize");

    // Claim the job using resources
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

    // Set job to running
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id)
        .expect("Failed to set job running");

    // Complete the job with PendingFailed status (simulating no failure handler match)
    let job_result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        1,   // return_code (non-zero)
        0.1, // exec_time_minutes
        chrono::Utc::now().to_rfc3339(),
        JobStatus::PendingFailed,
    );

    let completed_job =
        apis::jobs_api::complete_job(config, job_id, job_result.status, run_id, job_result)
            .expect("Failed to complete job");

    // Verify job is in pending_failed status
    assert_eq!(completed_job.status, Some(JobStatus::PendingFailed));

    // Fetch the job again to confirm status
    let fetched_job = apis::jobs_api::get_job(config, job_id).expect("Failed to get job");
    assert_eq!(fetched_job.status, Some(JobStatus::PendingFailed));
}

/// Test listing pending_failed jobs via status filter
#[rstest]
fn test_list_pending_failed_jobs(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create and initialize workflow
    let (workflow_id, run_id) = create_and_initialize_workflow(config, "test_list_pending_failed");

    // Create jobs after initialization
    let job1 = create_test_job(config, workflow_id, "job1");
    let job1_id = job1.id.unwrap();
    let job2 = create_test_job(config, workflow_id, "job2");
    let job2_id = job2.id.unwrap();

    // Create compute node
    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Reinitialize to pick up the new jobs
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to reinitialize");

    // Claim both jobs
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
    assert_eq!(jobs.len(), 2);

    // Set both jobs to running
    for job_id in [job1_id, job2_id] {
        apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id)
            .expect("Failed to set job running");
    }

    // Complete job1 as pending_failed
    let job1_result = models::ResultModel::new(
        job1_id,
        workflow_id,
        run_id,
        1,
        compute_node_id,
        1,
        0.1,
        chrono::Utc::now().to_rfc3339(),
        JobStatus::PendingFailed,
    );
    apis::jobs_api::complete_job(config, job1_id, job1_result.status, run_id, job1_result)
        .expect("Failed to complete job1");

    // Complete job2 as completed (success)
    let job2_result = models::ResultModel::new(
        job2_id,
        workflow_id,
        run_id,
        1,
        compute_node_id,
        0,
        0.1,
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Completed,
    );
    apis::jobs_api::complete_job(config, job2_id, job2_result.status, run_id, job2_result)
        .expect("Failed to complete job2");

    // List jobs with pending_failed status
    let pending_failed_jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        Some(JobStatus::PendingFailed),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list pending_failed jobs");

    assert_eq!(pending_failed_jobs.total_count, 1);
    assert_eq!(pending_failed_jobs.items[0].id, Some(job1_id));
}

/// Test that reset_workflow_status includes pending_failed jobs
#[rstest]
fn test_reset_includes_pending_failed(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create and initialize workflow
    let (workflow_id, run_id) = create_and_initialize_workflow(config, "test_reset_pending_failed");

    // Create job after initialization
    let job = create_test_job(config, workflow_id, "test_job");
    let job_id = job.id.unwrap();

    // Create compute node
    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Reinitialize to pick up the new job
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to reinitialize");

    // Claim the job
    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let result = apis::workflows_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        10,
        resources,
        None,
    )
    .expect("Failed to claim jobs");
    assert_eq!(result.jobs.expect("Should return jobs").len(), 1);

    // Set job to running
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id)
        .expect("Failed to set job running");

    // Complete as pending_failed
    let job_result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1,
        compute_node_id,
        1,
        0.1,
        chrono::Utc::now().to_rfc3339(),
        JobStatus::PendingFailed,
    );
    apis::jobs_api::complete_job(config, job_id, job_result.status, run_id, job_result)
        .expect("Failed to complete job");

    // Verify job is pending_failed
    let job_before = apis::jobs_api::get_job(config, job_id).expect("Failed to get job");
    assert_eq!(job_before.status, Some(JobStatus::PendingFailed));

    // Reset failed jobs only (should include pending_failed)
    apis::workflows_api::reset_job_status(config, workflow_id, Some(true))
        .expect("Failed to reset job status");

    // Verify job is now uninitialized
    let job_after = apis::jobs_api::get_job(config, job_id).expect("Failed to get job");
    assert_eq!(job_after.status, Some(JobStatus::Uninitialized));
}

/// Test invoke_ai_agent with unsupported agent
#[rstest]
fn test_invoke_ai_agent_unsupported() {
    let output_dir = PathBuf::from("/tmp");
    let result = invoke_ai_agent(123, "unsupported_agent", &output_dir);

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unsupported AI agent"));
}

/// Test invoke_ai_agent with mock claude script
/// This test creates a temporary mock "claude" command and tests the invocation
#[rstest]
#[cfg(unix)]
#[serial_test::serial] // Serialize tests that modify PATH
fn test_invoke_ai_agent_mock() {
    use tempfile::TempDir;

    // Create a temp directory with a mock "claude" script
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let mock_claude_path = temp_dir.path().join("claude");

    // Create a simple mock script that just echoes and exits successfully
    let mock_script = r#"#!/bin/bash
# Accept --version check
if [ "$1" = "--version" ]; then
    echo "Mock Claude 1.0.0"
    exit 0
fi
# Accept --print with prompt
echo "Mock Claude: Received prompt: $2"
echo "Mock Claude: Classification complete"
exit 0
"#;

    fs::write(&mock_claude_path, mock_script).expect("Failed to write mock script");

    // Make executable
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(&mock_claude_path)
        .expect("Failed to get metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&mock_claude_path, perms).expect("Failed to set permissions");

    // Add temp dir to PATH using a wrapper approach
    // Note: This is still technically unsafe in concurrent test execution,
    // but serial_test::serial ensures tests run sequentially
    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", temp_dir.path().display(), original_path);

    // Use unsafe block for set_var (required in Rust 2024 edition)
    unsafe {
        std::env::set_var("PATH", &new_path);
    }

    // Run the test
    let output_dir = PathBuf::from("/tmp");
    let result = invoke_ai_agent(123, "claude", &output_dir);

    // Restore PATH
    unsafe {
        std::env::set_var("PATH", original_path);
    }

    // The test should succeed with our mock
    assert!(result.is_ok(), "invoke_ai_agent failed: {:?}", result);
}
