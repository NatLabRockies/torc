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
        None,
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
        None,
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
        Some(vec![JobStatus::PendingFailed]),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None, // origin_is_set
        None, // name
        None, // command
        None, // labels
        None, // ids
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
        None,
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
    apis::workflows_api::reset_job_status(config, workflow_id, Some(true), None, None)
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

/// MCP classify_and_resolve_failures must apply both decisions through the API:
/// "retry" -> Ready with attempt_id bumped, "fail" -> Failed.
#[cfg(feature = "mcp-server")]
#[rstest]
fn test_classify_and_resolve_failures_applies(start_server: &ServerProcess) {
    use torc::mcp_server::tools::{FailureClassification, classify_and_resolve_failures};

    let config = &start_server.config;
    let (workflow_id, run_id) = create_and_initialize_workflow(config, "test_classify_apply");

    let mut workflow = apis::workflows_api::get_workflow(config, workflow_id).unwrap();
    workflow.use_pending_failed = Some(true);
    apis::workflows_api::update_workflow(config, workflow_id, workflow)
        .expect("Failed to enable use_pending_failed");

    let retry_id = create_test_job(config, workflow_id, "retry_job")
        .id
        .unwrap();
    let fail_id = create_test_job(config, workflow_id, "fail_job").id.unwrap();
    let compute_node_id = create_test_compute_node(config, workflow_id).id.unwrap();
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to reinitialize");

    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    let claimed = apis::workflows_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        10,
        resources,
        None,
        None,
        None,
    )
    .expect("Failed to claim jobs");
    assert_eq!(claimed.jobs.expect("Should return jobs").len(), 2);

    for job_id in [retry_id, fail_id] {
        apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id)
            .expect("Failed to set job running");
        let result = models::ResultModel::new(
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
        apis::jobs_api::complete_job(config, job_id, result.status, run_id, result)
            .expect("Failed to complete job");
    }

    let classification = |job_id, action: &str| FailureClassification {
        job_id,
        action: action.to_string(),
        memory: None,
        runtime: None,
        reason: None,
    };
    classify_and_resolve_failures(
        config,
        workflow_id,
        vec![
            classification(retry_id, "retry"),
            classification(fail_id, "fail"),
        ],
        false,
    )
    .expect("classify_and_resolve_failures failed");

    let retried = apis::jobs_api::get_job(config, retry_id).unwrap();
    assert_eq!(retried.status, Some(JobStatus::Ready));
    assert_eq!(retried.attempt_id, Some(2));

    let failed = apis::jobs_api::get_job(config, fail_id).unwrap();
    assert_eq!(failed.status, Some(JobStatus::Failed));
}

/// regroup_job_resources must reject a group that omits num_gpus when its jobs currently
/// use different values, instead of silently applying the first job's value to all.
#[cfg(feature = "mcp-server")]
#[rstest]
fn test_regroup_rejects_ambiguous_omitted_gpus(start_server: &ServerProcess) {
    use torc::mcp_server::tools::{ResourceGroup, regroup_job_resources};

    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "regroup_ambiguous")
        .id
        .unwrap();

    let mut job_ids = Vec::new();
    for (name, gpus) in [("cpu_rr", 0), ("gpu_rr", 1)] {
        let rr = apis::resource_requirements_api::create_resource_requirements(
            config,
            models::ResourceRequirementsModel {
                id: None,
                workflow_id,
                name: name.to_string(),
                num_cpus: 1,
                num_gpus: gpus,
                num_nodes: 1,
                memory: "1g".to_string(),
                runtime: "PT10M".to_string(),
            },
        )
        .unwrap();
        let mut job = models::JobModel::new(workflow_id, format!("job_{}", name), "true".into());
        job.resource_requirements_id = rr.id;
        job_ids.push(apis::jobs_api::create_job(config, job).unwrap().id.unwrap());
    }
    let rr_count = |config| {
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
        .unwrap()
        .items
        .len()
    };
    let before = rr_count(config);

    regroup_job_resources(
        config,
        workflow_id,
        vec![ResourceGroup {
            memory: "2g".to_string(),
            num_cpus: 2,
            runtime: "PT20M".to_string(),
            num_gpus: None,
            num_nodes: None,
            name: Some("merged".to_string()),
            job_ids: job_ids.clone(),
        }],
        false,
    )
    .expect("regroup_job_resources returned an error");

    assert_eq!(
        rr_count(config),
        before,
        "no resource requirements should be created"
    );
}

/// Resolving the last pending_failed job to Failed completes the workflow, so it must
/// trigger on_workflow_complete actions.
#[rstest]
fn test_resolving_last_pending_failed_triggers_workflow_complete(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, run_id) =
        create_and_initialize_workflow(config, "test_pending_failed_workflow_complete");

    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        models::WorkflowActionModel {
            id: None,
            workflow_id,
            trigger_type: "on_workflow_complete".to_string(),
            action_type: "run_commands".to_string(),
            action_config: serde_json::json!({"commands": ["true"]}),
            job_ids: None,
            trigger_count: 0,
            required_triggers: 1,
            executed: false,
            executed_at: None,
            executed_by: None,
            persistent: false,
            is_recovery: false,
        },
    )
    .expect("Failed to create action");
    let action_id = action.id.unwrap();

    let job_id = create_test_job(config, workflow_id, "only_job").id.unwrap();
    let compute_node_id = create_test_compute_node(config, workflow_id).id.unwrap();
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to reinitialize");
    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    apis::workflows_api::claim_jobs_based_on_resources(
        config,
        workflow_id,
        10,
        resources,
        None,
        None,
        None,
    )
    .expect("Failed to claim jobs");
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id)
        .expect("Failed to set job running");
    let result = models::ResultModel::new(
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
    apis::jobs_api::complete_job(config, job_id, result.status, run_id, result)
        .expect("Failed to complete job");

    let triggered = |config| {
        let actions = apis::workflow_actions_api::get_workflow_actions(config, workflow_id)
            .expect("Failed to get actions");
        let a = actions
            .iter()
            .find(|a| a.id == Some(action_id))
            .expect("action not found");
        a.trigger_count >= a.required_triggers
    };
    assert!(
        !triggered(config),
        "workflow is not complete while pending_failed"
    );

    // A pending_failed job records a failed attempt; it cannot be resolved to Completed.
    match apis::jobs_api::manage_status_change(config, job_id, JobStatus::Completed, run_id) {
        Err(apis::Error::ResponseError(r)) => assert_eq!(r.status, 422),
        other => panic!(
            "expected 422 resolving pending_failed to completed, got {:?}",
            other
        ),
    }

    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Failed, run_id)
        .expect("Failed to resolve pending_failed job");
    assert!(
        triggered(config),
        "on_workflow_complete should be triggered"
    );

    // The result row follows the job to Failed, so status filters agree.
    let failed_results = apis::results_api::list_results(
        config,
        workflow_id,
        Some(job_id),
        None,
        None,
        Some(vec![JobStatus::Failed]),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list results");
    assert_eq!(
        failed_results.items.len(),
        1,
        "result status should be failed"
    );
}

/// Resolving attempt 2 of a pending_failed job to Failed must leave attempt 1's result row
/// (classified as retry earlier) unchanged.
#[rstest]
fn test_resolving_pending_failed_only_updates_current_attempt_result(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, run_id) =
        create_and_initialize_workflow(config, "test_pending_failed_attempts");
    let job_id = create_test_job(config, workflow_id, "flaky").id.unwrap();
    let compute_node_id = create_test_compute_node(config, workflow_id).id.unwrap();
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to reinitialize");

    let run_attempt_to_pending_failed = |attempt_id: i64| {
        let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
        let claimed = apis::workflows_api::claim_jobs_based_on_resources(
            config,
            workflow_id,
            10,
            resources,
            None,
            None,
            None,
        )
        .expect("Failed to claim jobs");
        assert_eq!(claimed.jobs.expect("Should return jobs").len(), 1);
        apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id)
            .expect("Failed to set job running");
        let result = models::ResultModel::new(
            job_id,
            workflow_id,
            run_id,
            attempt_id,
            compute_node_id,
            1,
            0.1,
            chrono::Utc::now().to_rfc3339(),
            JobStatus::PendingFailed,
        );
        apis::jobs_api::complete_job(config, job_id, result.status, run_id, result)
            .expect("Failed to complete job");
    };

    run_attempt_to_pending_failed(1);
    // Classified as retry: Ready with attempt_id 2.
    apis::jobs_api::retry_job(config, job_id, run_id, None).expect("Failed to retry");
    run_attempt_to_pending_failed(2);
    // Classified as fail.
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Failed, run_id)
        .expect("Failed to resolve pending_failed job");

    let results = apis::results_api::list_results(
        config,
        workflow_id,
        Some(job_id),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(true),
        None,
    )
    .expect("Failed to list results")
    .items;
    let status_of = |attempt: i64| {
        results
            .iter()
            .find(|r| r.attempt_id == Some(attempt))
            .map(|r| r.status)
    };
    assert_eq!(status_of(1), Some(JobStatus::PendingFailed));
    assert_eq!(status_of(2), Some(JobStatus::Failed));
}
