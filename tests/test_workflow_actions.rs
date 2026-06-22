mod common;

use std::thread;
use std::time::Duration;

use common::{ServerProcess, create_test_workflow, run_cli_with_json, start_server};
use rstest::rstest;
use serde_json::json;
use torc::client::apis;
use torc::client::workflow_manager::WorkflowManager;
use torc::config::TorcConfig;
use torc::models::{ClaimActionRequest, JobModel, JobStatus, ResultModel, WorkflowActionModel};

/// Helper function to create a test job
fn create_test_job(
    config: &torc::client::Configuration,
    workflow_id: i64,
    name: &str,
) -> Result<JobModel, Box<dyn std::error::Error>> {
    let job = JobModel::new(
        workflow_id,
        name.to_string(),
        format!("echo 'Running {}'", name),
    );

    let created_job = apis::jobs_api::create_job(config, job)?;
    Ok(created_job)
}

/// Helper function to create a compute node
fn create_test_compute_node(
    config: &torc::client::Configuration,
    workflow_id: i64,
) -> Result<i64, Box<dyn std::error::Error>> {
    let compute_node = torc::models::ComputeNodeModel::new(
        workflow_id,
        "test-host".to_string(),
        12345,
        chrono::Utc::now().to_rfc3339(),
        4,
        8.0,
        0,
        1,
        "local".to_string(),
        None,
    );

    let created = apis::compute_nodes_api::create_compute_node(config, compute_node)?;
    Ok(created.id.expect("Compute node should have ID"))
}

fn workflow_action(
    workflow_id: i64,
    trigger_type: &str,
    action_type: &str,
    action_config: serde_json::Value,
    job_ids: Option<Vec<i64>>,
) -> WorkflowActionModel {
    WorkflowActionModel {
        id: None,
        workflow_id,
        trigger_type: trigger_type.to_string(),
        action_type: action_type.to_string(),
        action_config,
        job_ids,
        trigger_count: 0,
        required_triggers: 1,
        executed: false,
        executed_at: None,
        executed_by: None,
        persistent: false,
        is_recovery: false,
    }
}

#[rstest]
fn test_create_workflow_action_run_commands(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "action_test_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create a run_commands action
    let action_config = json!({
        "commands": ["echo 'Starting workflow'", "mkdir -p output"]
    });

    let action_body = workflow_action(
        workflow_id,
        "on_workflow_start",
        "run_commands",
        action_config,
        None,
    );

    let result =
        apis::workflow_actions_api::create_workflow_action(config, workflow_id, action_body)
            .expect("Failed to create workflow action");

    assert!(result.id.is_some());
    assert_eq!(result.workflow_id, workflow_id);
    assert_eq!(result.trigger_type.as_str(), "on_workflow_start");
    assert_eq!(result.action_type.as_str(), "run_commands");
}

#[rstest]
fn test_create_workflow_action_schedule_nodes(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "action_schedule_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create a schedule_nodes action
    let action_config = json!({
        "scheduler_type": "slurm",
        "scheduler_id": 1,
        "num_allocations": 2,
        "max_parallel_jobs": 4
    });

    let action_body = workflow_action(
        workflow_id,
        "on_jobs_ready",
        "schedule_nodes",
        action_config,
        None,
    );

    let result =
        apis::workflow_actions_api::create_workflow_action(config, workflow_id, action_body)
            .expect("Failed to create schedule_nodes action");

    assert!(result.id.is_some());
    assert_eq!(result.action_type.as_str(), "schedule_nodes");
}

#[rstest]
fn test_get_workflow_actions(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "action_get_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create multiple actions
    for i in 0..3 {
        let action_config = json!({
            "commands": [format!("echo 'Command {}'", i)]
        });

        let action_body = workflow_action(
            workflow_id,
            "on_workflow_start",
            "run_commands",
            action_config,
            None,
        );

        apis::workflow_actions_api::create_workflow_action(config, workflow_id, action_body)
            .expect("Failed to create action");
    }

    // Get all actions
    let actions = apis::workflow_actions_api::get_workflow_actions(config, workflow_id)
        .expect("Failed to get workflow actions");

    assert_eq!(actions.len(), 3);
    for action in &actions {
        assert_eq!(action.workflow_id, workflow_id);
        assert_eq!(action.trigger_type.as_str(), "on_workflow_start");
    }
}

#[rstest]
fn test_get_pending_actions(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "action_pending_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create an action
    let action_config = json!({
        "commands": ["echo 'Pending action'"]
    });

    let action_body = workflow_action(
        workflow_id,
        "on_workflow_start",
        "run_commands",
        action_config,
        None,
    );

    apis::workflow_actions_api::create_workflow_action(config, workflow_id, action_body)
        .expect("Failed to create action");

    // Initialize the workflow to trigger on_workflow_start actions
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize workflow");

    // Get pending actions (should include the newly created action)
    let pending_actions =
        apis::workflow_actions_api::get_pending_actions(config, workflow_id, None)
            .expect("Failed to get pending actions");

    assert_eq!(pending_actions.len(), 1);
    assert!(!pending_actions[0].executed);
}

#[rstest]
fn test_claim_action_success(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "action_claim_workflow");
    let workflow_id = workflow.id.unwrap();
    let compute_node_id =
        create_test_compute_node(config, workflow_id).expect("Failed to create compute node");

    // Create an action
    let action_config = json!({
        "commands": ["echo 'Claimable action'"]
    });

    let action_body = workflow_action(
        workflow_id,
        "on_workflow_start",
        "run_commands",
        action_config,
        None,
    );

    let created_action =
        apis::workflow_actions_api::create_workflow_action(config, workflow_id, action_body)
            .expect("Failed to create action");
    let action_id = created_action.id.unwrap();

    // Initialize the workflow to trigger on_workflow_start actions
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize workflow");

    // Claim the action
    let claim_body = ClaimActionRequest {
        compute_node_id: Some(compute_node_id),
    };

    let claim_result =
        apis::workflow_actions_api::claim_action(config, workflow_id, action_id, claim_body)
            .expect("Failed to claim action");

    assert!(claim_result.success);
    assert_eq!(claim_result.action_id, action_id);

    // Verify the action is no longer pending
    let pending_actions =
        apis::workflow_actions_api::get_pending_actions(config, workflow_id, None)
            .expect("Failed to get pending actions");
    assert_eq!(pending_actions.len(), 0);
}

#[rstest]
fn test_claim_action_already_claimed(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "action_double_claim_workflow");
    let workflow_id = workflow.id.unwrap();
    let compute_node_id1 =
        create_test_compute_node(config, workflow_id).expect("Failed to create compute node 1");
    let compute_node_id2 =
        create_test_compute_node(config, workflow_id).expect("Failed to create compute node 2");

    // Create an action
    let action_config = json!({
        "commands": ["echo 'Double claim test'"]
    });

    let action_body = workflow_action(
        workflow_id,
        "on_workflow_start",
        "run_commands",
        action_config,
        None,
    );

    let created_action =
        apis::workflow_actions_api::create_workflow_action(config, workflow_id, action_body)
            .expect("Failed to create action");
    let action_id = created_action.id.unwrap();

    // Initialize the workflow to trigger on_workflow_start actions
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize workflow");

    // First claim should succeed
    let claim_body1 = ClaimActionRequest {
        compute_node_id: Some(compute_node_id1),
    };

    let claim_result1 =
        apis::workflow_actions_api::claim_action(config, workflow_id, action_id, claim_body1)
            .expect("Failed to claim action first time");
    assert!(claim_result1.success);

    // Second claim should return CONFLICT
    let claim_body2 = ClaimActionRequest {
        compute_node_id: Some(compute_node_id2),
    };

    let claim_result2 =
        apis::workflow_actions_api::claim_action(config, workflow_id, action_id, claim_body2);

    match claim_result2 {
        Err(torc::client::apis::Error::ResponseError(ref response_content)) => {
            assert_eq!(response_content.status, reqwest::StatusCode::CONFLICT);
        }
        _ => panic!("Expected CONFLICT error for already claimed action"),
    }
}

#[rstest]
fn test_action_with_job_names(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "action_patterns_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create test jobs
    let job1 =
        create_test_job(config, workflow_id, "train_model_1").expect("Failed to create job 1");
    let job2 =
        create_test_job(config, workflow_id, "train_model_2").expect("Failed to create job 2");
    let _job3 =
        create_test_job(config, workflow_id, "evaluate_model").expect("Failed to create job 3");

    // Create action with job_ids
    let action_config = json!({
        "scheduler_type": "slurm",
        "scheduler_id": 1,
        "num_allocations": 1
    });

    let job_ids_array = vec![job1.id.unwrap(), job2.id.unwrap()];
    let action_body = workflow_action(
        workflow_id,
        "on_jobs_ready",
        "schedule_nodes",
        action_config,
        Some(job_ids_array),
    );

    let created_action =
        apis::workflow_actions_api::create_workflow_action(config, workflow_id, action_body)
            .expect("Failed to create action");

    // Verify job_ids were set correctly
    assert!(created_action.job_ids.is_some());
    let stored_ids = created_action.job_ids.unwrap();
    assert!(stored_ids.contains(&job1.id.unwrap()));
    assert!(stored_ids.contains(&job2.id.unwrap()));
}

#[rstest]
fn test_action_with_job_name_regexes(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "action_regex_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create test jobs
    let job1 =
        create_test_job(config, workflow_id, "train_model_001").expect("Failed to create job 1");
    let job2 =
        create_test_job(config, workflow_id, "train_model_002").expect("Failed to create job 2");
    let _job3 =
        create_test_job(config, workflow_id, "evaluate_model").expect("Failed to create job 3");

    // Create action with job_ids
    let action_config = json!({
        "scheduler_type": "slurm",
        "scheduler_id": 1,
        "num_allocations": 1
    });

    let job_ids_array = vec![job1.id.unwrap(), job2.id.unwrap()];
    let action_body = workflow_action(
        workflow_id,
        "on_jobs_ready",
        "schedule_nodes",
        action_config,
        Some(job_ids_array),
    );

    let created_action =
        apis::workflow_actions_api::create_workflow_action(config, workflow_id, action_body)
            .expect("Failed to create action");

    // Verify job_ids were set correctly
    assert!(created_action.job_ids.is_some());
    let stored_ids = created_action.job_ids.unwrap();
    assert!(stored_ids.contains(&job1.id.unwrap()));
    assert!(stored_ids.contains(&job2.id.unwrap()));
}

#[rstest]
fn test_action_with_combined_patterns_and_regexes(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "action_combined_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create test jobs
    let job1 = create_test_job(config, workflow_id, "preprocess").expect("Failed to create job 1");
    let job2 =
        create_test_job(config, workflow_id, "train_model_001").expect("Failed to create job 2");
    let job3 =
        create_test_job(config, workflow_id, "train_model_002").expect("Failed to create job 3");
    let _job4 = create_test_job(config, workflow_id, "evaluate").expect("Failed to create job 4");

    // Create action with job_ids
    let action_config = json!({
        "commands": ["echo 'All training ready'"]
    });

    let action_body = workflow_action(
        workflow_id,
        "on_jobs_ready",
        "run_commands",
        action_config,
        Some(vec![job1.id.unwrap(), job2.id.unwrap(), job3.id.unwrap()]),
    );

    let created_action =
        apis::workflow_actions_api::create_workflow_action(config, workflow_id, action_body)
            .expect("Failed to create action");

    // Verify job_ids were set correctly
    assert!(created_action.job_ids.is_some());
    let stored_ids = created_action.job_ids.unwrap();
    assert!(stored_ids.contains(&job1.id.unwrap()));
    assert!(stored_ids.contains(&job2.id.unwrap()));
    assert!(stored_ids.contains(&job3.id.unwrap()));
}

#[rstest]
fn test_multiple_actions_different_triggers(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "action_multi_trigger_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create actions with different trigger types
    let triggers = vec![
        "on_workflow_start",
        "on_workflow_complete",
        "on_jobs_ready",
        "on_jobs_complete",
    ];

    for trigger in &triggers {
        let action_config = json!({
            "commands": [format!("echo 'Trigger: {}'", trigger)]
        });

        let action_body =
            workflow_action(workflow_id, trigger, "run_commands", action_config, None);

        apis::workflow_actions_api::create_workflow_action(config, workflow_id, action_body)
            .unwrap_or_else(|_| panic!("Failed to create action for trigger: {}", trigger));
    }

    // Verify all actions were created
    let actions = apis::workflow_actions_api::get_workflow_actions(config, workflow_id)
        .expect("Failed to get workflow actions");

    assert_eq!(actions.len(), 4);

    // Verify each trigger type is present
    let trigger_types: Vec<String> = actions.iter().map(|a| a.trigger_type.clone()).collect();

    for trigger in &triggers {
        assert!(trigger_types.contains(&trigger.to_string()));
    }
}

#[rstest]
fn test_action_status_lifecycle(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "action_lifecycle_workflow");
    let workflow_id = workflow.id.unwrap();
    let compute_node_id =
        create_test_compute_node(config, workflow_id).expect("Failed to create compute node");

    // Create an action
    let action_config = json!({
        "commands": ["echo 'Status lifecycle test'"]
    });

    let action_body = workflow_action(
        workflow_id,
        "on_workflow_start",
        "run_commands",
        action_config,
        None,
    );

    let created_action =
        apis::workflow_actions_api::create_workflow_action(config, workflow_id, action_body)
            .expect("Failed to create action");
    let action_id = created_action.id.unwrap();

    // Initial status should be "not executed"
    assert!(!created_action.executed);
    assert!(created_action.executed_by.is_none());

    // Initialize the workflow to trigger on_workflow_start actions
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize workflow");

    // Claim the action
    let claim_body = ClaimActionRequest {
        compute_node_id: Some(compute_node_id),
    };

    apis::workflow_actions_api::claim_action(config, workflow_id, action_id, claim_body)
        .expect("Failed to claim action");

    // Get all actions and verify status changed
    let actions = apis::workflow_actions_api::get_workflow_actions(config, workflow_id)
        .expect("Failed to get workflow actions");

    let claimed_action = actions
        .iter()
        .find(|a| a.id.unwrap() == action_id)
        .expect("Action not found");

    assert!(claimed_action.executed);
    assert_eq!(claimed_action.executed_by.unwrap(), compute_node_id);

    // Verify it's no longer in pending actions
    let pending_actions =
        apis::workflow_actions_api::get_pending_actions(config, workflow_id, None)
            .expect("Failed to get pending actions");
    assert_eq!(pending_actions.len(), 0);
}

/// Test that workflow actions are properly reset when a workflow is reinitialized.
///
/// This test matches the user's scenario:
/// - job1 produces output, job2 produces output independently
/// - postprocess_job depends on both job1 and job2 outputs
/// - There is a workflow action set to trigger on on_jobs_ready with jobs = ["postprocess_job"]
/// - First run: all jobs complete, postprocess_job becomes ready, action triggers and is claimed
/// - job1's input changes, requiring job1 to be reset and rerun (but job2 stays completed)
/// - We reset job1 and reinitialize the workflow
/// - After reinitialize: job2 remains completed, postprocess_job is blocked (waiting for job1)
/// - The action's trigger_count should account for completed jobs when checking on_jobs_ready
/// - Second run: job1 completes again, postprocess_job becomes ready
/// - Expected: The workflow action should trigger again when postprocess_job becomes ready
#[rstest]
fn test_action_executed_flag_reset_on_reinitialize(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "action_reinit_test_workflow");
    let workflow_id = workflow.id.unwrap();
    let torc_config = TorcConfig::load().unwrap_or_default();
    let manager = WorkflowManager::new(config.clone(), torc_config, workflow);

    // Create job1 (independent, will fail in first run and be reset)
    let job1 =
        torc::models::JobModel::new(workflow_id, "job1".to_string(), "echo 'job1'".to_string());
    let job1 = apis::jobs_api::create_job(config, job1).expect("Failed to create job1");
    let job1_id = job1.id.unwrap();

    // Create job2 (independent, will succeed and stay completed)
    let job2 =
        torc::models::JobModel::new(workflow_id, "job2".to_string(), "echo 'job2'".to_string());
    let job2 = apis::jobs_api::create_job(config, job2).expect("Failed to create job2");
    let job2_id = job2.id.unwrap();

    // Create postprocess_job that depends on BOTH job1 and job2
    let mut postprocess_job = torc::models::JobModel::new(
        workflow_id,
        "postprocess_job".to_string(),
        "echo 'postprocess'".to_string(),
    );
    postprocess_job.depends_on_job_ids = Some(vec![job1_id, job2_id]);
    postprocess_job.cancel_on_blocking_job_failure = Some(false);
    let postprocess_job = apis::jobs_api::create_job(config, postprocess_job)
        .expect("Failed to create postprocess_job");
    let postprocess_job_id = postprocess_job.id.unwrap();

    // Create workflow action: trigger on_jobs_ready for postprocess_job
    let action_config = json!({
        "commands": ["echo 'postprocess_job is ready'"]
    });
    let action_body = workflow_action(
        workflow_id,
        "on_jobs_ready",
        "run_commands",
        action_config,
        Some(vec![postprocess_job_id]),
    );
    let created_action =
        apis::workflow_actions_api::create_workflow_action(config, workflow_id, action_body)
            .expect("Failed to create workflow action");
    let action_id = created_action.id.unwrap();

    // Initialize workflow using WorkflowManager
    manager
        .initialize(true)
        .expect("Failed to initialize workflow");
    let run_id = manager.get_run_id().expect("Failed to get run_id");

    // Create compute node for completing jobs
    let compute_node_id =
        create_test_compute_node(config, workflow_id).expect("Failed to create compute node");

    // === First run: Complete job1 with FAILURE ===
    // Note: status must match return_code - non-zero return_code requires Failed status
    apis::jobs_api::manage_status_change(config, job1_id, torc::models::JobStatus::Running, run_id)
        .expect("Failed to set job1 to running");
    let result1 = torc::models::ResultModel::new(
        job1_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        1, // non-zero return_code = failure
        1.0,
        chrono::Utc::now().to_rfc3339(),
        torc::models::JobStatus::Failed,
    );
    apis::jobs_api::complete_job(config, job1_id, result1.status, run_id, result1)
        .expect("Failed to complete job1 with failure");

    // === First run: Complete job2 with SUCCESS ===
    apis::jobs_api::manage_status_change(config, job2_id, torc::models::JobStatus::Running, run_id)
        .expect("Failed to set job2 to running");
    let result2 = torc::models::ResultModel::new(
        job2_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        0,
        1.0,
        chrono::Utc::now().to_rfc3339(),
        torc::models::JobStatus::Completed,
    );
    apis::jobs_api::complete_job(config, job2_id, result2.status, run_id, result2)
        .expect("Failed to complete job2 with success");

    // Wait for unblock processing — poll until the action becomes pending
    let start = std::time::Instant::now();
    let mut pending_actions;
    loop {
        pending_actions =
            apis::workflow_actions_api::get_pending_actions(config, workflow_id, None)
                .expect("Failed to get pending actions");
        if !pending_actions.is_empty() {
            break;
        }
        assert!(
            start.elapsed().as_secs() < 10,
            "Timed out waiting for action to become pending after postprocess_job becomes ready"
        );
        thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        pending_actions.len(),
        1,
        "Action should be pending after postprocess_job becomes ready"
    );

    // Claim the action
    let claim_body = ClaimActionRequest {
        compute_node_id: Some(compute_node_id),
    };
    apis::workflow_actions_api::claim_action(config, workflow_id, action_id, claim_body)
        .expect("Failed to claim action");

    // Verify action is executed
    let actions = apis::workflow_actions_api::get_workflow_actions(config, workflow_id)
        .expect("Failed to get workflow actions");
    let action = actions.iter().find(|a| a.id.unwrap() == action_id).unwrap();
    assert!(action.executed, "Action should be executed after claiming");
    assert_eq!(action.trigger_count, 1);

    // === Reset failed job and reinitialize using WorkflowManager ===
    apis::workflows_api::reset_job_status(config, workflow_id, Some(true))
        .expect("Failed to reset failed jobs");

    // Reinitialize workflow using WorkflowManager (this gets a new run_id)
    manager
        .reinitialize(true, false)
        .expect("Failed to reinitialize workflow");
    let run_id2 = manager
        .get_run_id()
        .expect("Failed to get run_id after reinit");

    // Verify job statuses after reinitialize
    let job1_after = apis::jobs_api::get_job(config, job1_id).expect("Failed to get job1");
    let job2_after = apis::jobs_api::get_job(config, job2_id).expect("Failed to get job2");
    let postprocess_after =
        apis::jobs_api::get_job(config, postprocess_job_id).expect("Failed to get postprocess_job");

    assert_eq!(
        job1_after.status.unwrap(),
        torc::models::JobStatus::Ready,
        "job1 should be Ready"
    );
    assert_eq!(
        job2_after.status.unwrap(),
        torc::models::JobStatus::Completed,
        "job2 should still be Completed"
    );
    assert_eq!(
        postprocess_after.status.unwrap(),
        torc::models::JobStatus::Blocked,
        "postprocess_job should be Blocked"
    );

    // Check action state after reinitialize - should be reset
    let actions_after = apis::workflow_actions_api::get_workflow_actions(config, workflow_id)
        .expect("Failed to get workflow actions");
    let action_after = actions_after
        .iter()
        .find(|a| a.id.unwrap() == action_id)
        .unwrap();
    assert_eq!(
        action_after.trigger_count, 0,
        "trigger_count should be 0 after reinitialize"
    );
    assert!(
        !action_after.executed,
        "executed should be false after reinitialize"
    );
    assert!(
        action_after.executed_by.is_none(),
        "executed_by should be None after reinitialize"
    );

    // Action should not be pending yet (postprocess_job is blocked)
    let pending_after = apis::workflow_actions_api::get_pending_actions(config, workflow_id, None)
        .expect("Failed to get pending actions");
    assert_eq!(
        pending_after.len(),
        0,
        "No actions should be pending while postprocess_job is blocked"
    );

    // === Second run: Complete job1 with SUCCESS ===
    apis::jobs_api::manage_status_change(
        config,
        job1_id,
        torc::models::JobStatus::Running,
        run_id2,
    )
    .expect("Failed to set job1 to running");
    let result1_second = torc::models::ResultModel::new(
        job1_id,
        workflow_id,
        run_id2,
        1, // attempt_id
        compute_node_id,
        0,
        1.0,
        chrono::Utc::now().to_rfc3339(),
        torc::models::JobStatus::Completed,
    );
    apis::jobs_api::complete_job(
        config,
        job1_id,
        result1_second.status,
        run_id2,
        result1_second,
    )
    .expect("Failed to complete job1");

    // Wait for unblock processing — poll until action becomes pending again
    let start = std::time::Instant::now();
    let mut pending_final;
    loop {
        pending_final = apis::workflow_actions_api::get_pending_actions(config, workflow_id, None)
            .expect("Failed to get pending actions");
        if !pending_final.is_empty() {
            break;
        }
        assert!(
            start.elapsed().as_secs() < 10,
            "Timed out waiting for action to become pending again after job1 completes"
        );
        thread::sleep(Duration::from_millis(50));
    }

    // postprocess_job should now be Ready
    let postprocess_final =
        apis::jobs_api::get_job(config, postprocess_job_id).expect("Failed to get postprocess_job");
    assert_eq!(
        postprocess_final.status.unwrap(),
        torc::models::JobStatus::Ready,
        "postprocess_job should be Ready"
    );

    assert_eq!(
        pending_final.len(),
        1,
        "Action should be pending again after postprocess_job becomes ready"
    );

    // Verify action state
    let actions_final = apis::workflow_actions_api::get_workflow_actions(config, workflow_id)
        .expect("Failed to get workflow actions");
    let action_final = actions_final
        .iter()
        .find(|a| a.id.unwrap() == action_id)
        .unwrap();
    assert_eq!(action_final.trigger_count, 1, "trigger_count should be 1");
    assert!(
        !action_final.executed,
        "executed should be false (pending, not claimed)"
    );
}

/// Regression test for the recover wizard re-submitting the action-defined allocation count.
///
/// After reinitialization re-arms the workflow's `schedule_nodes` actions, the recover wizard's
/// "reuse existing scheduler" path calls `mark_satisfied_schedule_actions_executed` before
/// submitting the user's chosen number of allocations. This prevents any *already-satisfied*
/// re-armed action — the `on_workflow_start` action, plus job-gated actions (e.g.
/// `on_jobs_complete`) whose gating jobs already completed in a prior run — from firing again on
/// the first compute node and submitting its own (original) `num_allocations`. The helper must mark
/// exactly the satisfied `schedule_nodes` actions executed and leave everything else (including
/// job-gated actions still waiting on their jobs) untouched.
#[rstest]
fn test_mark_satisfied_schedule_actions_executed(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "mark_satisfied_schedule_workflow");
    let workflow_id = workflow.id.unwrap();

    let schedule_config = json!({
        "scheduler_type": "slurm",
        "scheduler_id": 1,
        "num_allocations": 5,
    });

    // Target 1: on_workflow_start + schedule_nodes (non-recovery) — always satisfied, marked.
    let target = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_workflow_start",
            "schedule_nodes",
            schedule_config.clone(),
            None,
        ),
    )
    .expect("Failed to create target action");
    let target_id = target.id.unwrap();

    // Target 2: on_jobs_complete + schedule_nodes whose gating jobs already completed
    // (trigger_count >= required_triggers) — this is the reported bug; must be marked executed.
    let mut satisfied_body = workflow_action(
        workflow_id,
        "on_jobs_complete",
        "schedule_nodes",
        schedule_config.clone(),
        Some(vec![1]), // required_triggers becomes 1 (one gating job)
    );
    satisfied_body.trigger_count = 1; // already satisfied: trigger_count >= required_triggers
    let satisfied =
        apis::workflow_actions_api::create_workflow_action(config, workflow_id, satisfied_body)
            .expect("Failed to create satisfied on_jobs_complete action");
    let satisfied_id = satisfied.id.unwrap();

    // Target 3: on_worker_start + schedule_nodes — fires immediately at worker startup, so it is
    // always satisfied and must be marked executed.
    let worker_start = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_worker_start",
            "schedule_nodes",
            schedule_config.clone(),
            None,
        ),
    )
    .expect("Failed to create on_worker_start action");
    let worker_start_id = worker_start.id.unwrap();

    // Decoy 1: on_workflow_start + run_commands — wrong action_type, must be left alone.
    let run_cmd = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_workflow_start",
            "run_commands",
            json!({ "commands": ["echo hi"] }),
            None,
        ),
    )
    .expect("Failed to create run_commands action");
    let run_cmd_id = run_cmd.id.unwrap();

    // Decoy 2: on_jobs_complete + schedule_nodes still waiting on its job
    // (trigger_count < required_triggers) — left armed so it can fire legitimately in recovery.
    let unsatisfied = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_jobs_complete",
            "schedule_nodes",
            schedule_config.clone(),
            Some(vec![2]), // required_triggers=1, trigger_count defaults to 0 → unsatisfied
        ),
    )
    .expect("Failed to create unsatisfied on_jobs_complete action");
    let unsatisfied_id = unsatisfied.id.unwrap();

    // Decoy 3: on_workflow_start + schedule_nodes but is_recovery=true — must be left alone.
    let mut recovery_body = workflow_action(
        workflow_id,
        "on_workflow_start",
        "schedule_nodes",
        schedule_config.clone(),
        None,
    );
    recovery_body.is_recovery = true;
    let recovery =
        apis::workflow_actions_api::create_workflow_action(config, workflow_id, recovery_body)
            .expect("Failed to create recovery action");
    let recovery_id = recovery.id.unwrap();

    // Decoy 4: persistent on_workflow_start + schedule_nodes — satisfied, but claiming a persistent
    // action does not clear its armed state (the server keeps executed=0 for persistent actions),
    // so the helper must leave it untouched (and warn) rather than falsely mark it executed.
    let mut persistent_body = workflow_action(
        workflow_id,
        "on_workflow_start",
        "schedule_nodes",
        schedule_config,
        None,
    );
    persistent_body.persistent = true;
    let persistent =
        apis::workflow_actions_api::create_workflow_action(config, workflow_id, persistent_body)
            .expect("Failed to create persistent action");
    let persistent_id = persistent.id.unwrap();

    // Run the fix under test.
    torc::client::commands::recover::mark_satisfied_schedule_actions_executed(config, workflow_id)
        .expect("mark_satisfied_schedule_actions_executed failed");

    let actions = apis::workflow_actions_api::get_workflow_actions(config, workflow_id)
        .expect("Failed to get workflow actions");
    let find = |id: i64| actions.iter().find(|a| a.id == Some(id)).unwrap();

    assert!(
        find(target_id).executed,
        "on_workflow_start schedule_nodes action should be marked executed"
    );
    assert!(
        find(satisfied_id).executed,
        "satisfied on_jobs_complete schedule_nodes action should be marked executed"
    );
    assert!(
        find(worker_start_id).executed,
        "on_worker_start schedule_nodes action should be marked executed"
    );
    assert!(
        !find(persistent_id).executed,
        "persistent schedule_nodes action should be left untouched (claim cannot suppress it)"
    );
    assert!(
        !find(run_cmd_id).executed,
        "run_commands action should be left untouched"
    );
    assert!(
        !find(unsatisfied_id).executed,
        "unsatisfied on_jobs_complete schedule_nodes action should be left untouched"
    );
    assert!(
        !find(recovery_id).executed,
        "recovery schedule_nodes action should be left untouched"
    );
}

// ---------------------------------------------------------------------------
// Regression tests for `reset_actions_for_reinitialize` not re-arming an
// already-satisfied job-gated `schedule_nodes` action.
//
// Bug: `torc workflows reinit` (and every other reinitialize entry point) cleared
// `executed` on all actions. An `on_jobs_complete` schedule_nodes action whose
// gating job stays complete therefore became pending again, and `torc slurm
// schedule-nodes` — which has no suppression of its own — let the next worker fire
// it, re-scheduling the original node count. These tests exercise the server-side
// reinitialize path (via `WorkflowManager::reinitialize`) directly.
// ---------------------------------------------------------------------------

/// `action_config` for a Slurm schedule_nodes action. `scheduler_id` is a placeholder; these tests
/// never execute the action (no worker), they only assert its post-reinitialize armed state.
fn schedule_nodes_config() -> serde_json::Value {
    json!({ "scheduler_type": "slurm", "scheduler_id": 1, "num_allocations": 3 })
}

/// Drive a job Running -> terminal `status` with `return_code`, recording a result on
/// `compute_node_id` — the same sequence a worker performs when it finishes a job.
fn run_job_to_status(
    config: &torc::client::Configuration,
    workflow_id: i64,
    job_id: i64,
    run_id: i64,
    compute_node_id: i64,
    return_code: i64,
    status: JobStatus,
) {
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id)
        .expect("Failed to set job to running");
    let result = ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        return_code,
        1.0,
        chrono::Utc::now().to_rfc3339(),
        status,
    );
    apis::jobs_api::complete_job(config, job_id, status, run_id, result)
        .expect("Failed to complete job");
}

/// Poll `get_pending_actions` until at least one action is pending; panic after 10s.
fn wait_for_pending_action(
    config: &torc::client::Configuration,
    workflow_id: i64,
) -> Vec<WorkflowActionModel> {
    let start = std::time::Instant::now();
    loop {
        let pending = apis::workflow_actions_api::get_pending_actions(config, workflow_id, None)
            .expect("Failed to get pending actions");
        if !pending.is_empty() {
            return pending;
        }
        assert!(
            start.elapsed().as_secs() < 10,
            "Timed out waiting for an action to become pending"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

/// Fetch a single action by id.
fn fetch_action(
    config: &torc::client::Configuration,
    workflow_id: i64,
    action_id: i64,
) -> WorkflowActionModel {
    apis::workflow_actions_api::get_workflow_actions(config, workflow_id)
        .expect("Failed to get workflow actions")
        .into_iter()
        .find(|a| a.id == Some(action_id))
        .expect("action not found")
}

/// True if `action_id` is in the workflow's current pending set.
fn action_is_pending(
    config: &torc::client::Configuration,
    workflow_id: i64,
    action_id: i64,
) -> bool {
    apis::workflow_actions_api::get_pending_actions(config, workflow_id, None)
        .expect("Failed to get pending actions")
        .iter()
        .any(|a| a.id == Some(action_id))
}

/// PRIMARY regression test (the reported bug): an `on_jobs_complete` schedule_nodes action whose
/// gating job stays complete across a *subset* re-run must NOT be re-armed — otherwise it re-fires
/// and submits a duplicate allocation when a worker starts.
#[rstest]
fn test_satisfied_schedule_action_not_rearmed_on_reinitialize(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reinit_satisfied_schedule_not_rearmed");
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    // gate "copy-lock-file" (root) -> "work1" depends on it.
    let gate = create_test_job(config, workflow_id, "copy-lock-file").expect("create gate");
    let gate_id = gate.id.unwrap();
    let mut work1 = JobModel::new(workflow_id, "work1".to_string(), "echo work1".to_string());
    work1.depends_on_job_ids = Some(vec![gate_id]);
    work1.cancel_on_blocking_job_failure = Some(false);
    let work1_id = apis::jobs_api::create_job(config, work1)
        .expect("create work1")
        .id
        .unwrap();

    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_jobs_complete",
            "schedule_nodes",
            schedule_nodes_config(),
            Some(vec![gate_id]),
        ),
    )
    .expect("create action");
    let action_id = action.id.unwrap();

    manager.initialize(true).expect("initialize");
    let run_id = manager.get_run_id().expect("run_id");
    let compute_node_id = create_test_compute_node(config, workflow_id).expect("compute node");

    // Run 1: gate completes -> action becomes pending -> claim it (simulates it firing 3 nodes).
    run_job_to_status(
        config,
        workflow_id,
        gate_id,
        run_id,
        compute_node_id,
        0,
        JobStatus::Completed,
    );
    wait_for_pending_action(config, workflow_id);
    apis::workflow_actions_api::claim_action(
        config,
        workflow_id,
        action_id,
        ClaimActionRequest {
            compute_node_id: Some(compute_node_id),
        },
    )
    .expect("claim action");
    assert!(
        fetch_action(config, workflow_id, action_id).executed,
        "action should be executed after claiming"
    );

    // work1 fails, then we re-run only the failed job: reset failed jobs + reinitialize.
    run_job_to_status(
        config,
        workflow_id,
        work1_id,
        run_id,
        compute_node_id,
        1,
        JobStatus::Failed,
    );
    apis::workflows_api::reset_job_status(config, workflow_id, Some(true))
        .expect("reset failed jobs");
    manager.reinitialize(true, false).expect("reinitialize");

    // The gate was never reset.
    assert_eq!(
        apis::jobs_api::get_job(config, gate_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Completed,
        "gate should still be Completed"
    );

    // THE FIX: the already-satisfied schedule_nodes action keeps executed=1 and is not pending.
    assert!(
        fetch_action(config, workflow_id, action_id).executed,
        "satisfied schedule_nodes action must stay executed after reinitialize (it must not re-fire)"
    );
    assert!(
        !action_is_pending(config, workflow_id, action_id),
        "suppressed schedule_nodes action must not be pending after reinitialize"
    );
}

/// Complement to the primary test: when the gating job IS reset (a *full* re-run), the schedule_nodes
/// action must be re-armed and fire again once the gate completes. Guards against over-suppression.
#[rstest]
fn test_schedule_action_rearmed_when_gate_job_reset(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reinit_schedule_rearmed_on_gate_reset");
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    let gate_id = create_test_job(config, workflow_id, "copy-lock-file")
        .expect("create gate")
        .id
        .unwrap();
    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_jobs_complete",
            "schedule_nodes",
            schedule_nodes_config(),
            Some(vec![gate_id]),
        ),
    )
    .expect("create action");
    let action_id = action.id.unwrap();

    manager.initialize(true).expect("initialize");
    let run_id = manager.get_run_id().expect("run_id");
    let compute_node_id = create_test_compute_node(config, workflow_id).expect("compute node");

    // Run 1: gate completes, action fires (claimed).
    run_job_to_status(
        config,
        workflow_id,
        gate_id,
        run_id,
        compute_node_id,
        0,
        JobStatus::Completed,
    );
    wait_for_pending_action(config, workflow_id);
    apis::workflow_actions_api::claim_action(
        config,
        workflow_id,
        action_id,
        ClaimActionRequest {
            compute_node_id: Some(compute_node_id),
        },
    )
    .expect("claim action");
    assert!(fetch_action(config, workflow_id, action_id).executed);

    // Full re-run: reset the GATE job itself (like `torc jobs reset-status <gate>`), then reinitialize.
    apis::jobs_api::manage_status_change(config, gate_id, JobStatus::Uninitialized, run_id)
        .expect("reset gate");
    manager.reinitialize(true, false).expect("reinitialize");

    assert_ne!(
        apis::jobs_api::get_job(config, gate_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Completed,
        "gate should have been reset"
    );
    assert!(
        !fetch_action(config, workflow_id, action_id).executed,
        "action must be re-armed when its gate job was reset (full re-run)"
    );

    // It fires again once the gate completes in the new run.
    let run_id2 = manager.get_run_id().expect("run_id2");
    run_job_to_status(
        config,
        workflow_id,
        gate_id,
        run_id2,
        compute_node_id,
        0,
        JobStatus::Completed,
    );
    let pending = wait_for_pending_action(config, workflow_id);
    assert!(
        pending.iter().any(|a| a.id == Some(action_id)),
        "re-armed action should become pending again after the gate completes"
    );
}

/// Guard (on_jobs_ready variant of `test_schedule_action_rearmed_when_gate_job_reset`).
///
/// `on_jobs_ready` is the dominant trigger for `schedule_nodes` in the deferred-scheduling examples
/// (see `examples/subgraphs/`), so its re-arm behavior on a full re-run matters in practice.
///
/// This guards against keying the keep-executed decision off the action's own `trigger_count`. That
/// would be wrong for `on_jobs_ready`: `count_jobs_in_satisfied_state` counts `Ready` as satisfied,
/// and `reset_actions_for_reinitialize` runs AFTER `initialize_jobs` has already returned the reset
/// gate to `Ready`. A *full* re-run of the gate would then still yield
/// `trigger_count >= required_triggers`, the action that already fired would never be re-armed, no
/// allocation would be requested, and the gate would sit `Ready` forever.
///
/// `reset_actions_for_reinitialize` instead measures keep-executed with the terminal-only
/// `on_jobs_complete` notion, so a reset (now `Ready`, not terminal) gate correctly re-arms. This
/// asserts that correct behavior (re-arm on a full re-run, exactly like the on_jobs_complete sibling
/// test). It would fail against a `trigger_count`-based heuristic.
#[rstest]
fn test_on_jobs_ready_schedule_action_rearmed_when_gate_job_reset(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reinit_on_jobs_ready_rearmed_on_gate_reset");
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    // Root gate: after (re)initialize it lands directly in `Ready`, which is exactly the state that
    // makes an on_jobs_ready action satisfied.
    let gate_id = create_test_job(config, workflow_id, "copy-lock-file")
        .expect("create gate")
        .id
        .unwrap();
    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_jobs_ready",
            "schedule_nodes",
            schedule_nodes_config(),
            Some(vec![gate_id]),
        ),
    )
    .expect("create action");
    let action_id = action.id.unwrap();

    manager.initialize(true).expect("initialize");
    let run_id = manager.get_run_id().expect("run_id");
    let compute_node_id = create_test_compute_node(config, workflow_id).expect("compute node");

    // Run 1: initialize sets the root gate Ready, so the on_jobs_ready action becomes pending.
    // Claim it (simulates it firing its allocation).
    wait_for_pending_action(config, workflow_id);
    apis::workflow_actions_api::claim_action(
        config,
        workflow_id,
        action_id,
        ClaimActionRequest {
            compute_node_id: Some(compute_node_id),
        },
    )
    .expect("claim action");
    assert!(fetch_action(config, workflow_id, action_id).executed);

    // Full re-run: reset the GATE job itself, then reinitialize. The gate returns to Ready.
    apis::jobs_api::manage_status_change(config, gate_id, JobStatus::Uninitialized, run_id)
        .expect("reset gate");
    manager.reinitialize(true, false).expect("reinitialize");

    // The gate was reset and is runnable again...
    assert_ne!(
        apis::jobs_api::get_job(config, gate_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Completed,
        "gate should have been reset"
    );

    // ...so the schedule_nodes action MUST be re-armed to allocate nodes for the re-run. Keeping
    // executed off the on_jobs_complete (terminal-only) notion is what makes this work: the reset
    // gate is now Ready, not terminal, so keep-executed is false and the action re-arms. (A
    // trigger_count-based heuristic would wrongly keep executed=1, since Ready satisfies
    // on_jobs_ready, and this assert would fail.)
    assert!(
        !fetch_action(config, workflow_id, action_id).executed,
        "on_jobs_ready schedule_nodes action must be re-armed when its gate job was reset (full re-run); \
         otherwise no allocation is requested and the re-run stalls"
    );
    assert!(
        action_is_pending(config, workflow_id, action_id),
        "re-armed on_jobs_ready action should be pending again once its reset gate is Ready"
    );
}

/// Complement to the `on_jobs_ready` re-arm test: a *subset* re-run (gate left Completed, only a
/// downstream job reset) must still suppress the on_jobs_ready schedule_nodes action — its gate
/// already ran to terminal, so re-firing would submit a duplicate allocation. Guards the fix against
/// over-correcting into "always re-arm on_jobs_ready".
#[rstest]
fn test_on_jobs_ready_satisfied_schedule_action_not_rearmed_on_subset_reinitialize(
    start_server: &ServerProcess,
) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reinit_on_jobs_ready_subset_suppressed");
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    // gate "copy-lock-file" (root) -> "work1" depends on it. Only work1 is reset below.
    let gate = create_test_job(config, workflow_id, "copy-lock-file").expect("create gate");
    let gate_id = gate.id.unwrap();
    let mut work1 = JobModel::new(workflow_id, "work1".to_string(), "echo work1".to_string());
    work1.depends_on_job_ids = Some(vec![gate_id]);
    work1.cancel_on_blocking_job_failure = Some(false);
    let work1_id = apis::jobs_api::create_job(config, work1)
        .expect("create work1")
        .id
        .unwrap();

    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_jobs_ready",
            "schedule_nodes",
            schedule_nodes_config(),
            Some(vec![gate_id]),
        ),
    )
    .expect("create action");
    let action_id = action.id.unwrap();

    manager.initialize(true).expect("initialize");
    let run_id = manager.get_run_id().expect("run_id");
    let compute_node_id = create_test_compute_node(config, workflow_id).expect("compute node");

    // Run 1: gate is Ready at init -> on_jobs_ready action pending -> claim it (fires allocation).
    wait_for_pending_action(config, workflow_id);
    apis::workflow_actions_api::claim_action(
        config,
        workflow_id,
        action_id,
        ClaimActionRequest {
            compute_node_id: Some(compute_node_id),
        },
    )
    .expect("claim action");
    assert!(fetch_action(config, workflow_id, action_id).executed);

    // Gate completes; work1 then runs and fails.
    run_job_to_status(
        config,
        workflow_id,
        gate_id,
        run_id,
        compute_node_id,
        0,
        JobStatus::Completed,
    );
    wait_for_job_status(config, work1_id, JobStatus::Ready);
    run_job_to_status(
        config,
        workflow_id,
        work1_id,
        run_id,
        compute_node_id,
        1,
        JobStatus::Failed,
    );

    // Subset re-run: reset only the failed job, then reinitialize. The gate stays Completed.
    apis::workflows_api::reset_job_status(config, workflow_id, Some(true))
        .expect("reset failed jobs");
    manager.reinitialize(true, false).expect("reinitialize");

    assert_eq!(
        apis::jobs_api::get_job(config, gate_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Completed,
        "gate should still be Completed (only the downstream job was reset)"
    );
    assert!(
        fetch_action(config, workflow_id, action_id).executed,
        "on_jobs_ready schedule_nodes action whose gate stayed terminal must stay executed (no duplicate allocation)"
    );
    assert!(
        !action_is_pending(config, workflow_id, action_id),
        "suppressed on_jobs_ready action must not be pending after a subset reinitialize"
    );
}

/// The keep-vs-re-arm decision is action-type-agnostic: a job-gated `run_commands` action whose
/// gating job stays terminal across a *subset* re-run must NOT be re-armed, exactly like
/// `schedule_nodes`. Re-running it would repeat its side effect (an archive, an upload, a
/// notification) for a phase that did not re-run.
#[rstest]
fn test_run_commands_action_suppressed_on_subset_reinitialize(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reinit_run_commands_suppressed");
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    // gate "copy-lock-file" (root) -> "work1" depends on it. Only work1 is reset below.
    let gate = create_test_job(config, workflow_id, "copy-lock-file").expect("create gate");
    let gate_id = gate.id.unwrap();
    let mut work1 = JobModel::new(workflow_id, "work1".to_string(), "echo work1".to_string());
    work1.depends_on_job_ids = Some(vec![gate_id]);
    work1.cancel_on_blocking_job_failure = Some(false);
    let work1_id = apis::jobs_api::create_job(config, work1)
        .expect("create work1")
        .id
        .unwrap();

    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_jobs_complete",
            "run_commands",
            json!({ "commands": ["echo hi"] }),
            Some(vec![gate_id]),
        ),
    )
    .expect("create action");
    let action_id = action.id.unwrap();

    manager.initialize(true).expect("initialize");
    let run_id = manager.get_run_id().expect("run_id");
    let compute_node_id = create_test_compute_node(config, workflow_id).expect("compute node");

    // Run 1: gate completes -> action fires (claimed); work1 then fails.
    run_job_to_status(
        config,
        workflow_id,
        gate_id,
        run_id,
        compute_node_id,
        0,
        JobStatus::Completed,
    );
    wait_for_pending_action(config, workflow_id);
    apis::workflow_actions_api::claim_action(
        config,
        workflow_id,
        action_id,
        ClaimActionRequest {
            compute_node_id: Some(compute_node_id),
        },
    )
    .expect("claim action");
    assert!(fetch_action(config, workflow_id, action_id).executed);

    wait_for_job_status(config, work1_id, JobStatus::Ready);
    run_job_to_status(
        config,
        workflow_id,
        work1_id,
        run_id,
        compute_node_id,
        1,
        JobStatus::Failed,
    );

    // Subset re-run: reset only the failed job, then reinitialize. The gate stays Completed.
    apis::workflows_api::reset_job_status(config, workflow_id, Some(true))
        .expect("reset failed jobs");
    manager.reinitialize(true, false).expect("reinitialize");

    assert_eq!(
        apis::jobs_api::get_job(config, gate_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Completed,
        "gate should still be Completed (only the downstream job was reset)"
    );
    assert!(
        fetch_action(config, workflow_id, action_id).executed,
        "run_commands action whose gate stayed terminal must stay executed (no duplicate side effect)"
    );
    assert!(
        !action_is_pending(config, workflow_id, action_id),
        "suppressed run_commands action must not be pending after a subset reinitialize"
    );
}

/// Complement to the suppression test: when the gating job IS reset (a *full* re-run), the
/// `run_commands` action must be re-armed and run again once the gate completes — the phase is
/// genuinely re-running, so its post-action should re-run too.
#[rstest]
fn test_run_commands_action_rearmed_when_gate_job_reset(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reinit_run_commands_rearmed");
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    let gate_id = create_test_job(config, workflow_id, "copy-lock-file")
        .expect("create gate")
        .id
        .unwrap();
    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_jobs_complete",
            "run_commands",
            json!({ "commands": ["echo hi"] }),
            Some(vec![gate_id]),
        ),
    )
    .expect("create action");
    let action_id = action.id.unwrap();

    manager.initialize(true).expect("initialize");
    let run_id = manager.get_run_id().expect("run_id");
    let compute_node_id = create_test_compute_node(config, workflow_id).expect("compute node");

    run_job_to_status(
        config,
        workflow_id,
        gate_id,
        run_id,
        compute_node_id,
        0,
        JobStatus::Completed,
    );
    wait_for_pending_action(config, workflow_id);
    apis::workflow_actions_api::claim_action(
        config,
        workflow_id,
        action_id,
        ClaimActionRequest {
            compute_node_id: Some(compute_node_id),
        },
    )
    .expect("claim action");
    assert!(fetch_action(config, workflow_id, action_id).executed);

    // Full re-run: reset the GATE job itself, then reinitialize.
    apis::jobs_api::manage_status_change(config, gate_id, JobStatus::Uninitialized, run_id)
        .expect("reset gate");
    manager.reinitialize(true, false).expect("reinitialize");

    assert!(
        !fetch_action(config, workflow_id, action_id).executed,
        "run_commands action must be re-armed when its gate job was reset (full re-run)"
    );
}

/// `on_workflow_start` actions fire exactly once in a workflow's lifetime — a reinitialize is not a
/// new start — so they are KEPT executed (suppressed) on reinitialize, regardless of action_type.
/// This is what stops plain `reinit` from re-submitting the original node count. (`schedule_nodes`
/// variant.)
#[rstest]
fn test_workflow_start_schedule_action_kept_on_reinitialize(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reinit_workflow_start_kept");
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    // At least one job so the workflow can be (re)initialized.
    create_test_job(config, workflow_id, "j1").expect("create job");
    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_workflow_start",
            "schedule_nodes",
            schedule_nodes_config(),
            None,
        ),
    )
    .expect("create action");
    let action_id = action.id.unwrap();

    manager.initialize(true).expect("initialize");
    let compute_node_id = create_test_compute_node(config, workflow_id).expect("compute node");

    // on_workflow_start actions are activated at initialize, so the action is immediately pending.
    wait_for_pending_action(config, workflow_id);
    apis::workflow_actions_api::claim_action(
        config,
        workflow_id,
        action_id,
        ClaimActionRequest {
            compute_node_id: Some(compute_node_id),
        },
    )
    .expect("claim action");
    assert!(fetch_action(config, workflow_id, action_id).executed);

    manager.reinitialize(true, false).expect("reinitialize");

    assert!(
        fetch_action(config, workflow_id, action_id).executed,
        "on_workflow_start schedule_nodes action must stay executed on reinitialize (a reinit is not a new start)"
    );
    assert!(
        !action_is_pending(config, workflow_id, action_id),
        "suppressed on_workflow_start action must not be pending after reinitialize"
    );
}

/// `on_workflow_start` `run_commands` is kept on reinitialize too — start-time setup (mkdir, copy
/// data, notifications) must not re-run just because the workflow was reinitialized.
#[rstest]
fn test_workflow_start_run_commands_kept_on_reinitialize(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reinit_workflow_start_run_commands_kept");
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    create_test_job(config, workflow_id, "j1").expect("create job");
    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_workflow_start",
            "run_commands",
            json!({ "commands": ["echo hi"] }),
            None,
        ),
    )
    .expect("create action");
    let action_id = action.id.unwrap();

    manager.initialize(true).expect("initialize");
    let compute_node_id = create_test_compute_node(config, workflow_id).expect("compute node");

    wait_for_pending_action(config, workflow_id);
    apis::workflow_actions_api::claim_action(
        config,
        workflow_id,
        action_id,
        ClaimActionRequest {
            compute_node_id: Some(compute_node_id),
        },
    )
    .expect("claim action");
    assert!(fetch_action(config, workflow_id, action_id).executed);

    manager.reinitialize(true, false).expect("reinitialize");

    assert!(
        fetch_action(config, workflow_id, action_id).executed,
        "on_workflow_start run_commands action must stay executed on reinitialize"
    );
    assert!(
        !action_is_pending(config, workflow_id, action_id),
        "suppressed on_workflow_start run_commands action must not be pending after reinitialize"
    );
}

/// "Keep" preserves the flag, it does not force `executed = 1`: an `on_workflow_start` action that
/// never fired (e.g. created on a workflow that has not been initialized yet) stays fireable across a
/// reinitialize, so a first real initialize still runs it.
#[rstest]
fn test_never_fired_workflow_start_action_stays_fireable_after_reinitialize(
    start_server: &ServerProcess,
) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reinit_workflow_start_never_fired");
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    create_test_job(config, workflow_id, "j1").expect("create job");
    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_workflow_start",
            "schedule_nodes",
            schedule_nodes_config(),
            None,
        ),
    )
    .expect("create action");
    let action_id = action.id.unwrap();

    // Reinitialize WITHOUT a prior initialize/claim: the action has never fired (executed = 0).
    manager.reinitialize(true, false).expect("reinitialize");

    let after = fetch_action(config, workflow_id, action_id);
    assert!(
        !after.executed,
        "never-fired on_workflow_start action must remain executed=false (still able to fire)"
    );
    assert!(
        action_is_pending(config, workflow_id, action_id),
        "never-fired on_workflow_start action should be pending after (re)initialize"
    );
}

/// Threshold check: a multi-job-gated schedule_nodes action is only suppressed when ALL gating jobs
/// remain satisfied. Resetting one of two gate jobs drops trigger_count below required_triggers, so
/// the action must be re-armed.
#[rstest]
fn test_schedule_action_rearmed_when_one_of_multiple_gate_jobs_reset(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reinit_partial_gate_rearmed");
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    let gate_a = create_test_job(config, workflow_id, "gate_a")
        .expect("create gate_a")
        .id
        .unwrap();
    let gate_b = create_test_job(config, workflow_id, "gate_b")
        .expect("create gate_b")
        .id
        .unwrap();
    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_jobs_complete",
            "schedule_nodes",
            schedule_nodes_config(),
            Some(vec![gate_a, gate_b]),
        ),
    )
    .expect("create action");
    let action_id = action.id.unwrap();
    // The create response echoes the request body; required_triggers is computed server-side, so
    // read it back from the persisted row.
    assert_eq!(
        fetch_action(config, workflow_id, action_id).required_triggers,
        2,
        "two gating jobs => required_triggers should be 2"
    );

    manager.initialize(true).expect("initialize");
    let run_id = manager.get_run_id().expect("run_id");
    let compute_node_id = create_test_compute_node(config, workflow_id).expect("compute node");

    // Both gates complete -> action fires (claimed).
    run_job_to_status(
        config,
        workflow_id,
        gate_a,
        run_id,
        compute_node_id,
        0,
        JobStatus::Completed,
    );
    run_job_to_status(
        config,
        workflow_id,
        gate_b,
        run_id,
        compute_node_id,
        0,
        JobStatus::Completed,
    );
    wait_for_pending_action(config, workflow_id);
    apis::workflow_actions_api::claim_action(
        config,
        workflow_id,
        action_id,
        ClaimActionRequest {
            compute_node_id: Some(compute_node_id),
        },
    )
    .expect("claim action");
    assert!(fetch_action(config, workflow_id, action_id).executed);

    // Reset only ONE gate job -> partial satisfaction.
    apis::jobs_api::manage_status_change(config, gate_b, JobStatus::Uninitialized, run_id)
        .expect("reset gate_b");
    manager.reinitialize(true, false).expect("reinitialize");

    let after = fetch_action(config, workflow_id, action_id);
    assert!(
        !after.executed,
        "action must be re-armed when only some gating jobs remain satisfied (trigger_count < required_triggers)"
    );
    assert!(
        after.trigger_count < after.required_triggers,
        "trigger_count ({}) should be below required_triggers ({}) after one gate was reset",
        after.trigger_count,
        after.required_triggers
    );
}

/// on_jobs_ready variant of `test_schedule_action_rearmed_when_one_of_multiple_gate_jobs_reset`, and
/// the sharpest guard against keying keep-executed off `trigger_count`.
///
/// Two root gate jobs, `required_triggers = 2`. Both run to Completed and the action fires. Reset
/// ONE gate (it returns to `Ready`) and reinitialize. The wrinkle unique to on_jobs_ready: after the
/// reset, the action's `trigger_count` is recomputed with the on_jobs_ready notion, under which BOTH
/// `Completed` and `Ready` count — so `trigger_count == required_triggers == 2`. A heuristic that
/// suppressed when `trigger_count >= required_triggers` would therefore wrongly keep this action
/// executed and stall the re-run.
///
/// The fix instead measures keep-executed with the terminal-only on_jobs_complete notion: only ONE
/// gate is terminal (`already_ran_count = 1 < 2`), so the action is re-armed. This test asserts the
/// action re-arms EVEN THOUGH `trigger_count >= required_triggers`, which only holds against the
/// terminal-count implementation.
#[rstest]
fn test_on_jobs_ready_schedule_action_rearmed_when_one_of_multiple_gate_jobs_reset(
    start_server: &ServerProcess,
) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reinit_on_jobs_ready_partial_gate_rearmed");
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    // Two root gates: after (re)initialize both land in Ready, which satisfies on_jobs_ready.
    let gate_a = create_test_job(config, workflow_id, "gate_a")
        .expect("create gate_a")
        .id
        .unwrap();
    let gate_b = create_test_job(config, workflow_id, "gate_b")
        .expect("create gate_b")
        .id
        .unwrap();
    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_jobs_ready",
            "schedule_nodes",
            schedule_nodes_config(),
            Some(vec![gate_a, gate_b]),
        ),
    )
    .expect("create action");
    let action_id = action.id.unwrap();
    assert_eq!(
        fetch_action(config, workflow_id, action_id).required_triggers,
        2,
        "two gating jobs => required_triggers should be 2"
    );

    manager.initialize(true).expect("initialize");
    let run_id = manager.get_run_id().expect("run_id");
    let compute_node_id = create_test_compute_node(config, workflow_id).expect("compute node");

    // Run 1: both gates Ready at init -> action pending -> claim it (fires allocation).
    wait_for_pending_action(config, workflow_id);
    apis::workflow_actions_api::claim_action(
        config,
        workflow_id,
        action_id,
        ClaimActionRequest {
            compute_node_id: Some(compute_node_id),
        },
    )
    .expect("claim action");
    assert!(fetch_action(config, workflow_id, action_id).executed);

    // Both gates run to Completed (terminal).
    run_job_to_status(
        config,
        workflow_id,
        gate_a,
        run_id,
        compute_node_id,
        0,
        JobStatus::Completed,
    );
    run_job_to_status(
        config,
        workflow_id,
        gate_b,
        run_id,
        compute_node_id,
        1,
        JobStatus::Completed,
    );

    // Reset only ONE gate -> it returns to Ready, the other stays Completed.
    apis::jobs_api::manage_status_change(config, gate_b, JobStatus::Uninitialized, run_id)
        .expect("reset gate_b");
    manager.reinitialize(true, false).expect("reinitialize");

    let after = fetch_action(config, workflow_id, action_id);
    // The on_jobs_ready trigger_count counts both the Completed and the reset-now-Ready gate, so it
    // is back at the threshold -- a trigger_count heuristic would suppress here.
    assert_eq!(
        after.trigger_count, after.required_triggers,
        "on_jobs_ready trigger_count should be back at the threshold (Ready + Completed both count)"
    );
    // But only one gate actually ran to terminal, so the action must re-arm.
    assert!(
        !after.executed,
        "on_jobs_ready schedule_nodes action must re-arm when a gate was reset, even though \
         trigger_count >= required_triggers (only the terminal-job count must gate suppression)"
    );
    assert!(
        action_is_pending(config, workflow_id, action_id),
        "re-armed on_jobs_ready action should be pending again"
    );
}

/// A satisfied schedule_nodes action that NEVER fired (executed=false) must remain fireable after
/// reinitialize — "keep" preserves the flag, it does not force executed=1.
#[rstest]
fn test_never_fired_satisfied_schedule_action_stays_fireable_after_reinitialize(
    start_server: &ServerProcess,
) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reinit_never_fired_stays_fireable");
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    let gate_id = create_test_job(config, workflow_id, "copy-lock-file")
        .expect("create gate")
        .id
        .unwrap();
    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_jobs_complete",
            "schedule_nodes",
            schedule_nodes_config(),
            Some(vec![gate_id]),
        ),
    )
    .expect("create action");
    let action_id = action.id.unwrap();

    manager.initialize(true).expect("initialize");
    let run_id = manager.get_run_id().expect("run_id");
    let compute_node_id = create_test_compute_node(config, workflow_id).expect("compute node");

    // Gate completes so the action becomes pending, but it is deliberately NEVER claimed.
    run_job_to_status(
        config,
        workflow_id,
        gate_id,
        run_id,
        compute_node_id,
        0,
        JobStatus::Completed,
    );
    wait_for_pending_action(config, workflow_id);
    assert!(
        !fetch_action(config, workflow_id, action_id).executed,
        "action was never claimed"
    );

    manager.reinitialize(true, false).expect("reinitialize");

    let after = fetch_action(config, workflow_id, action_id);
    assert!(
        !after.executed,
        "never-fired satisfied action must remain executed=false (still able to fire) after reinitialize"
    );
    assert!(
        action_is_pending(config, workflow_id, action_id),
        "never-fired satisfied action should remain pending after reinitialize"
    );
}

// ---------------------------------------------------------------------------
// Multi-stage matrix: a 4-stage pipeline (stage1 -> stage2 -> stage3 -> stage4)
// where each stage has its own `schedule_nodes` action gated on that stage's
// completion (`on_jobs_complete[stage_k]`, num_allocations = k). Resetting stage
// K and reinitializing must leave the actions for stages 1..K-1 executed (their
// jobs stayed terminal) and re-arm the actions for stages K..4 (stage K is reset
// and K+1..4 cascade-reset). Verified across all three reinitialize entry points.
//
// "Firing" an action in a test = claim it AND insert its `num_allocations`
// scheduled_compute_node rows (a faithful stand-in for a worker running
// schedule_slurm_nodes; there is no Slurm in tests). The rows are created
// `complete` so they don't block a later reinitialize. The *count* of rows is
// driven entirely by which actions torc leaves pending, so it still verifies the
// re-arm decision under test.
// ---------------------------------------------------------------------------

/// Build a 4-stage linear pipeline: `stage1 -> stage2 -> stage3 -> stage4`, with one
/// `schedule_nodes` action per stage gated on that stage's completion
/// (`on_jobs_complete[stage_k]`, `num_allocations = k`). `cancel_on_blocking_job_failure=false`
/// so a failed stage leaves its downstream Blocked rather than Canceled. Returns
/// `(stage_job_ids, action_ids)` (both indexed 0..3 for stages 1..4). Does not initialize.
fn build_four_stage_chain(
    config: &torc::client::Configuration,
    workflow_id: i64,
) -> ([i64; 4], [i64; 4]) {
    let mut stage_ids = [0i64; 4];
    let mut prev: Option<i64> = None;
    for (k, slot) in stage_ids.iter_mut().enumerate() {
        let mut job = JobModel::new(
            workflow_id,
            format!("stage{}", k + 1),
            format!("echo stage{}", k + 1),
        );
        if let Some(p) = prev {
            job.depends_on_job_ids = Some(vec![p]);
        }
        job.cancel_on_blocking_job_failure = Some(false);
        let id = apis::jobs_api::create_job(config, job)
            .expect("create stage job")
            .id
            .unwrap();
        *slot = id;
        prev = Some(id);
    }

    let mut action_ids = [0i64; 4];
    for (k, slot) in action_ids.iter_mut().enumerate() {
        let cfg = json!({
            "scheduler_type": "slurm",
            "scheduler_id": 1,
            "num_allocations": (k as i64) + 1,
        });
        let action = apis::workflow_actions_api::create_workflow_action(
            config,
            workflow_id,
            workflow_action(
                workflow_id,
                "on_jobs_complete",
                "schedule_nodes",
                cfg,
                Some(vec![stage_ids[k]]),
            ),
        )
        .expect("create stage action");
        *slot = action.id.unwrap();
    }
    (stage_ids, action_ids)
}

/// Poll until `job_id` reaches `expected` (tolerating the async unblock task), or panic after 10s.
fn wait_for_job_status(config: &torc::client::Configuration, job_id: i64, expected: JobStatus) {
    let start = std::time::Instant::now();
    loop {
        let status = apis::jobs_api::get_job(config, job_id)
            .expect("get job")
            .status
            .unwrap();
        if status == expected {
            return;
        }
        assert!(
            start.elapsed().as_secs() < 10,
            "job {job_id} did not reach {expected:?} in time (last {status:?})"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

/// Poll until `action_id` specifically is pending, or panic after 10s.
fn wait_for_specific_action_pending(
    config: &torc::client::Configuration,
    workflow_id: i64,
    action_id: i64,
) {
    let start = std::time::Instant::now();
    loop {
        if action_is_pending(config, workflow_id, action_id) {
            return;
        }
        assert!(
            start.elapsed().as_secs() < 10,
            "action {action_id} did not become pending in time"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

/// Drive a stage Ready -> Running -> terminal `status`.
fn drive_stage(
    config: &torc::client::Configuration,
    workflow_id: i64,
    stage_id: i64,
    run_id: i64,
    compute_node_id: i64,
    status: JobStatus,
) {
    wait_for_job_status(config, stage_id, JobStatus::Ready);
    let return_code = if status == JobStatus::Completed { 0 } else { 1 };
    run_job_to_status(
        config,
        workflow_id,
        stage_id,
        run_id,
        compute_node_id,
        return_code,
        status,
    );
}

/// "Fire" a schedule_nodes action: claim it and insert its `num_allocations` scheduled_compute_node
/// rows (status `complete`, so they don't block a later reinitialize).
fn fire_schedule_action(
    config: &torc::client::Configuration,
    workflow_id: i64,
    action_id: i64,
    num_allocations: i64,
    compute_node_id: i64,
) {
    apis::workflow_actions_api::claim_action(
        config,
        workflow_id,
        action_id,
        ClaimActionRequest {
            compute_node_id: Some(compute_node_id),
        },
    )
    .expect("claim action");
    for i in 0..num_allocations {
        let scn = torc::models::ScheduledComputeNodesModel::new(
            workflow_id,
            action_id * 1000 + i, // synthetic, unique-ish Slurm job id
            1,                    // scheduler_config_id (no FK; arbitrary)
            "slurm".to_string(),
            "complete".to_string(),
        );
        apis::scheduled_compute_nodes_api::create_scheduled_compute_node(config, scn)
            .expect("create scheduled_compute_node");
    }
}

/// Count scheduled_compute_node rows for a workflow.
fn count_scheduled_compute_nodes(config: &torc::client::Configuration, workflow_id: i64) -> usize {
    apis::scheduled_compute_nodes_api::list_scheduled_compute_nodes(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("list scheduled_compute_nodes")
    .items
    .len()
}

/// Run the nominal pass on an initialized 4-stage workflow: complete every stage and fire its
/// action. Asserts all four actions are executed and exactly 10 (1+2+3+4) nodes were scheduled.
fn run_nominal_four_stages(
    config: &torc::client::Configuration,
    workflow_id: i64,
    stage_ids: &[i64; 4],
    action_ids: &[i64; 4],
    run_id: i64,
    compute_node_id: i64,
) {
    for k in 0..4 {
        drive_stage(
            config,
            workflow_id,
            stage_ids[k],
            run_id,
            compute_node_id,
            JobStatus::Completed,
        );
        wait_for_specific_action_pending(config, workflow_id, action_ids[k]);
        fire_schedule_action(
            config,
            workflow_id,
            action_ids[k],
            (k as i64) + 1,
            compute_node_id,
        );
    }
    for (k, &action_id) in action_ids.iter().enumerate() {
        assert!(
            fetch_action(config, workflow_id, action_id).executed,
            "A{} should be executed after the nominal run",
            k + 1
        );
    }
    assert_eq!(
        count_scheduled_compute_nodes(config, workflow_id),
        10,
        "nominal run should schedule 10 nodes (1+2+3+4)"
    );
}

/// Assert the per-action state after resetting stage `reset_stage` (1-based): actions for stages
/// 1..reset_stage-1 stay executed (kept) and are not pending; actions for stages reset_stage..4 are
/// re-armed (executed=false).
fn assert_stage_action_states(
    config: &torc::client::Configuration,
    workflow_id: i64,
    action_ids: &[i64; 4],
    reset_stage: usize,
) {
    for (k, &action_id) in action_ids.iter().enumerate() {
        let action = fetch_action(config, workflow_id, action_id);
        if k + 1 < reset_stage {
            assert!(
                action.executed,
                "A{} (before reset stage {}) must stay executed (kept)",
                k + 1,
                reset_stage
            );
            assert!(
                !action_is_pending(config, workflow_id, action_id),
                "A{} (kept) must not be pending after reinitialize",
                k + 1
            );
        } else {
            assert!(
                !action.executed,
                "A{} (reset stage {} or later) must be re-armed (executed=false)",
                k + 1,
                reset_stage
            );
        }
    }
}

/// PRIMARY 4-stage matrix via the `torc workflows reinit` path (`WorkflowManager::reinitialize`,
/// which is exactly what the CLI invokes). reset_stage=0 is the nominal-only case. Verifies both the
/// per-action executed/pending state and the total scheduled-compute-node count after re-running.
#[rstest]
#[case::nominal(0)]
#[case::reset_stage1(1)]
#[case::reset_stage2(2)]
#[case::reset_stage3(3)]
#[case::reset_stage4(4)]
fn test_four_stage_reinit_keeps_prior_schedule_actions(
    start_server: &ServerProcess,
    #[case] reset_stage: usize,
) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, &format!("four_stage_reinit_{reset_stage}"));
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    let (stage_ids, action_ids) = build_four_stage_chain(config, workflow_id);
    manager.initialize(true).expect("initialize");
    let run_id = manager.get_run_id().expect("run_id");
    let compute_node_id = create_test_compute_node(config, workflow_id).expect("compute node");

    run_nominal_four_stages(
        config,
        workflow_id,
        &stage_ids,
        &action_ids,
        run_id,
        compute_node_id,
    );

    if reset_stage == 0 {
        return; // nominal-only assertions already made
    }

    // Reset the chosen stage, then reinitialize (the cascade re-arms it and all later stages).
    apis::jobs_api::manage_status_change(
        config,
        stage_ids[reset_stage - 1],
        JobStatus::Uninitialized,
        run_id,
    )
    .expect("reset stage");
    manager.reinitialize(true, false).expect("reinitialize");
    let run_id2 = manager.get_run_id().expect("run_id2");

    assert_stage_action_states(config, workflow_id, &action_ids, reset_stage);
    // Reinitialize itself must not schedule anything: still the original 10 nodes.
    assert_eq!(
        count_scheduled_compute_nodes(config, workflow_id),
        10,
        "reinitialize must not schedule new nodes on its own"
    );

    // Re-run stages reset_stage..4; only their (re-armed) actions fire and schedule nodes.
    for k in (reset_stage - 1)..4 {
        drive_stage(
            config,
            workflow_id,
            stage_ids[k],
            run_id2,
            compute_node_id,
            JobStatus::Completed,
        );
        wait_for_specific_action_pending(config, workflow_id, action_ids[k]);
        fire_schedule_action(
            config,
            workflow_id,
            action_ids[k],
            (k as i64) + 1,
            compute_node_id,
        );
    }

    let new_nodes: usize = (reset_stage..=4).sum();
    assert_eq!(
        count_scheduled_compute_nodes(config, workflow_id),
        10 + new_nodes,
        "after resetting stage {reset_stage}, only stages {reset_stage}..4 should reschedule ({new_nodes} new nodes)"
    );
}

/// Same matrix via the real `torc jobs reset-status <stageK> --reinit` CLI (spawns the binary).
/// Verifies the bundled reset+reinit command produces the same action states, and that it schedules
/// nothing on its own (node count unchanged from the nominal run).
#[rstest]
#[case::reset_stage1(1)]
#[case::reset_stage2(2)]
#[case::reset_stage3(3)]
#[case::reset_stage4(4)]
fn test_four_stage_reset_status_reinit_cli_keeps_prior_schedule_actions(
    start_server: &ServerProcess,
    #[case] reset_stage: usize,
) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, &format!("four_stage_resetstatus_{reset_stage}"));
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    let (stage_ids, action_ids) = build_four_stage_chain(config, workflow_id);
    manager.initialize(true).expect("initialize");
    let run_id = manager.get_run_id().expect("run_id");
    let compute_node_id = create_test_compute_node(config, workflow_id).expect("compute node");

    run_nominal_four_stages(
        config,
        workflow_id,
        &stage_ids,
        &action_ids,
        run_id,
        compute_node_id,
    );

    // Reset + reinit via the actual CLI command.
    run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--force",
            "--reinit",
            &stage_ids[reset_stage - 1].to_string(),
        ],
        start_server,
        None,
    )
    .expect("jobs reset-status --reinit should succeed");

    assert_stage_action_states(config, workflow_id, &action_ids, reset_stage);
    assert_eq!(
        count_scheduled_compute_nodes(config, workflow_id),
        10,
        "reset-status --reinit must not schedule new nodes on its own"
    );
}

/// Same matrix via `torc recover`'s action-handling steps: `reset_failed_jobs` + `reinitialize_workflow`
/// + `mark_satisfied_schedule_actions_executed`. Recover only resets FAILED jobs, so here stages
/// 1..K-1 complete (and fire), stage K fails (Failed is terminal, so its action also fires), and
/// stages K+1..4 never run. After recover, stages 1..K-1 stay executed and stage K..4 are
/// executed=false — and crucially `mark_satisfied` must NOT re-suppress the re-armed stage-K action.
#[rstest]
#[case::reset_stage1(1)]
#[case::reset_stage2(2)]
#[case::reset_stage3(3)]
#[case::reset_stage4(4)]
fn test_four_stage_recover_keeps_prior_schedule_actions(
    start_server: &ServerProcess,
    #[case] reset_stage: usize,
) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, &format!("four_stage_recover_{reset_stage}"));
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    let (stage_ids, action_ids) = build_four_stage_chain(config, workflow_id);
    manager.initialize(true).expect("initialize");
    let run_id = manager.get_run_id().expect("run_id");
    let compute_node_id = create_test_compute_node(config, workflow_id).expect("compute node");
    let k0 = reset_stage - 1;

    // Stages before the failing one complete and fire their actions.
    for k in 0..k0 {
        drive_stage(
            config,
            workflow_id,
            stage_ids[k],
            run_id,
            compute_node_id,
            JobStatus::Completed,
        );
        wait_for_specific_action_pending(config, workflow_id, action_ids[k]);
        fire_schedule_action(
            config,
            workflow_id,
            action_ids[k],
            (k as i64) + 1,
            compute_node_id,
        );
    }
    // Stage K fails; its on_jobs_complete action still fires (Failed is terminal).
    drive_stage(
        config,
        workflow_id,
        stage_ids[k0],
        run_id,
        compute_node_id,
        JobStatus::Failed,
    );
    wait_for_specific_action_pending(config, workflow_id, action_ids[k0]);
    fire_schedule_action(
        config,
        workflow_id,
        action_ids[k0],
        (k0 as i64) + 1,
        compute_node_id,
    );

    // Pre-recover sanity: stages 1..K fired (executed); K+1..4 never fired.
    for (k, &action_id) in action_ids.iter().enumerate() {
        assert_eq!(
            fetch_action(config, workflow_id, action_id).executed,
            k <= k0,
            "pre-recover: A{} executed state",
            k + 1
        );
    }
    let scn_before = count_scheduled_compute_nodes(config, workflow_id);

    // Recover's action-relevant steps (the Slurm submit is environment-specific and out of scope).
    torc::client::commands::recover::reset_failed_jobs(config, workflow_id, &[stage_ids[k0]])
        .expect("reset_failed_jobs");
    torc::client::commands::recover::reinitialize_workflow(config, workflow_id)
        .expect("reinitialize_workflow");
    torc::client::commands::recover::mark_satisfied_schedule_actions_executed(config, workflow_id)
        .expect("mark_satisfied_schedule_actions_executed");

    assert_stage_action_states(config, workflow_id, &action_ids, reset_stage);
    assert_eq!(
        count_scheduled_compute_nodes(config, workflow_id),
        scn_before,
        "recover (reinit + mark_satisfied) must not schedule new nodes on its own"
    );
}

// ===========================================================================
// Consolidated decision-table enforcement.
//
// `reset_actions_for_reinitialize` decides keep-vs-re-arm with one rule,
// independent of action_type. These parameterized tests encode that table
// directly so a future change that narrows or inverts any arm fails loudly,
// and they exercise EVERY combination through all three reinit entry points
// (`workflows reinit`, the real `jobs reset-status --reinit` CLI, and recover's
// reset+reinit+mark_satisfied steps) rather than only `schedule_nodes`.
// ===========================================================================

/// Which reinit entry point to drive a reset+reinit through.
#[derive(Clone, Copy, Debug)]
enum ReinitEntry {
    /// `WorkflowManager::reinitialize` (what `torc workflows reinit` invokes).
    Reinit,
    /// The real `torc jobs reset-status --reinit <job>` CLI (spawns the binary).
    Cli,
    /// `torc recover`'s action steps: reset_failed_jobs + reinitialize_workflow + mark_satisfied.
    Recover,
}

/// Re-run shape for a job-gated action.
#[derive(Clone, Copy, Debug)]
enum ResetScenario {
    /// A downstream job is reset; the gate stays terminal (the action's event is not recurring).
    Subset,
    /// The gate job itself is reset (it will run again, so the action should fire again).
    Full,
}

/// `action_config` for the action_type under test.
fn action_config_for(action_type: &str) -> serde_json::Value {
    match action_type {
        "schedule_nodes" => schedule_nodes_config(),
        "run_commands" => json!({ "commands": ["echo hi"] }),
        other => panic!("unsupported action_type {other}"),
    }
}

/// Perform "reset the (only) Failed job + reinitialize" through the given entry point. Every caller
/// arranges that `reset_job_id` is the sole Failed job, so the Reinit path's reset-failed semantics
/// and the explicit-id CLI/recover paths target the same job.
fn reset_and_reinit_via(
    entry: ReinitEntry,
    config: &torc::client::Configuration,
    server: &ServerProcess,
    manager: &WorkflowManager,
    workflow_id: i64,
    reset_job_id: i64,
) {
    match entry {
        ReinitEntry::Reinit => {
            apis::workflows_api::reset_job_status(config, workflow_id, Some(true))
                .expect("reset failed jobs");
            manager.reinitialize(true, false).expect("reinitialize");
        }
        ReinitEntry::Cli => {
            run_cli_with_json(
                &[
                    "jobs",
                    "reset-status",
                    "--no-prompts",
                    "--force",
                    "--reinit",
                    &reset_job_id.to_string(),
                ],
                server,
                None,
            )
            .expect("jobs reset-status --reinit should succeed");
        }
        ReinitEntry::Recover => {
            torc::client::commands::recover::reset_failed_jobs(
                config,
                workflow_id,
                &[reset_job_id],
            )
            .expect("reset_failed_jobs");
            torc::client::commands::recover::reinitialize_workflow(config, workflow_id)
                .expect("reinitialize_workflow");
            torc::client::commands::recover::mark_satisfied_schedule_actions_executed(
                config,
                workflow_id,
            )
            .expect("mark_satisfied_schedule_actions_executed");
        }
    }
}

/// DECISION TABLE (job-gated, `on_jobs_complete`): a *subset* re-run keeps the action executed; a
/// *full* re-run (gate reset) re-arms it AND it fires again when the gate re-completes. Verified for
/// both action types across all three reinit entry points. `expect_keep` is spelled out per row so
/// the cases read as the intended table; a regressed arm flips a row.
#[rstest]
#[case::sched_subset_reinit("schedule_nodes", ResetScenario::Subset, ReinitEntry::Reinit, true)]
#[case::sched_subset_cli("schedule_nodes", ResetScenario::Subset, ReinitEntry::Cli, true)]
#[case::sched_subset_recover("schedule_nodes", ResetScenario::Subset, ReinitEntry::Recover, true)]
#[case::sched_full_reinit("schedule_nodes", ResetScenario::Full, ReinitEntry::Reinit, false)]
#[case::sched_full_cli("schedule_nodes", ResetScenario::Full, ReinitEntry::Cli, false)]
#[case::sched_full_recover("schedule_nodes", ResetScenario::Full, ReinitEntry::Recover, false)]
#[case::cmd_subset_reinit("run_commands", ResetScenario::Subset, ReinitEntry::Reinit, true)]
#[case::cmd_subset_cli("run_commands", ResetScenario::Subset, ReinitEntry::Cli, true)]
#[case::cmd_subset_recover("run_commands", ResetScenario::Subset, ReinitEntry::Recover, true)]
#[case::cmd_full_reinit("run_commands", ResetScenario::Full, ReinitEntry::Reinit, false)]
#[case::cmd_full_cli("run_commands", ResetScenario::Full, ReinitEntry::Cli, false)]
#[case::cmd_full_recover("run_commands", ResetScenario::Full, ReinitEntry::Recover, false)]
fn test_reinit_job_gated_keep_rearm_matrix(
    start_server: &ServerProcess,
    #[case] action_type: &str,
    #[case] scenario: ResetScenario,
    #[case] entry: ReinitEntry,
    #[case] expect_keep: bool,
) {
    let config = &start_server.config;
    let name = format!("matrix_jg_{action_type}_{scenario:?}_{entry:?}").to_lowercase();
    let workflow = create_test_workflow(config, &name);
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    // gate -> work (work depends on gate; left Blocked rather than Canceled if the gate fails).
    let gate_id = create_test_job(config, workflow_id, "gate")
        .expect("create gate")
        .id
        .unwrap();
    let mut work = JobModel::new(workflow_id, "work".to_string(), "echo work".to_string());
    work.depends_on_job_ids = Some(vec![gate_id]);
    work.cancel_on_blocking_job_failure = Some(false);
    let work_id = apis::jobs_api::create_job(config, work)
        .expect("create work")
        .id
        .unwrap();

    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_jobs_complete",
            action_type,
            action_config_for(action_type),
            Some(vec![gate_id]),
        ),
    )
    .expect("create action");
    let action_id = action.id.unwrap();

    manager.initialize(true).expect("initialize");
    let run_id = manager.get_run_id().expect("run_id");
    let compute_node_id = create_test_compute_node(config, workflow_id).expect("compute node");

    // Drive the gate to a terminal state so the on_jobs_complete action fires, then claim it. For
    // Full the gate itself fails (so it is the sole Failed job to reset); for Subset the gate
    // completes and the downstream work job fails instead.
    let (gate_status, gate_rc) = match scenario {
        ResetScenario::Subset => (JobStatus::Completed, 0),
        ResetScenario::Full => (JobStatus::Failed, 1),
    };
    run_job_to_status(
        config,
        workflow_id,
        gate_id,
        run_id,
        compute_node_id,
        gate_rc,
        gate_status,
    );
    wait_for_pending_action(config, workflow_id);
    apis::workflow_actions_api::claim_action(
        config,
        workflow_id,
        action_id,
        ClaimActionRequest {
            compute_node_id: Some(compute_node_id),
        },
    )
    .expect("claim action");
    assert!(
        fetch_action(config, workflow_id, action_id).executed,
        "action should be executed after claiming"
    );

    let reset_job_id = match scenario {
        ResetScenario::Subset => {
            wait_for_job_status(config, work_id, JobStatus::Ready);
            run_job_to_status(
                config,
                workflow_id,
                work_id,
                run_id,
                compute_node_id,
                1,
                JobStatus::Failed,
            );
            work_id
        }
        ResetScenario::Full => gate_id,
    };

    reset_and_reinit_via(
        entry,
        config,
        start_server,
        &manager,
        workflow_id,
        reset_job_id,
    );

    let after = fetch_action(config, workflow_id, action_id);
    assert_eq!(
        after.executed, expect_keep,
        "{action_type}/{scenario:?}/{entry:?}: executed should be {expect_keep} (keep) after reinit"
    );

    if expect_keep {
        assert!(
            !action_is_pending(config, workflow_id, action_id),
            "{action_type}/{scenario:?}/{entry:?}: kept action must not be pending"
        );
    } else {
        // Re-armed: prove it actually fires again when the gate completes in the new run.
        let run_id2 = manager.get_run_id().expect("run_id2");
        wait_for_job_status(config, gate_id, JobStatus::Ready);
        run_job_to_status(
            config,
            workflow_id,
            gate_id,
            run_id2,
            compute_node_id,
            0,
            JobStatus::Completed,
        );
        wait_for_specific_action_pending(config, workflow_id, action_id);
    }
}

/// DECISION TABLE (`on_workflow_start`): always kept on reinitialize (a reinit is not a new start),
/// for both action types, across all three reinit entry points. A failed downstream job is reset so
/// each entry point performs a real reset+reinit; the on_workflow_start action must be unaffected.
#[rstest]
#[case::sched_reinit("schedule_nodes", ReinitEntry::Reinit)]
#[case::sched_cli("schedule_nodes", ReinitEntry::Cli)]
#[case::sched_recover("schedule_nodes", ReinitEntry::Recover)]
#[case::cmd_reinit("run_commands", ReinitEntry::Reinit)]
#[case::cmd_cli("run_commands", ReinitEntry::Cli)]
#[case::cmd_recover("run_commands", ReinitEntry::Recover)]
fn test_reinit_workflow_start_kept_matrix(
    start_server: &ServerProcess,
    #[case] action_type: &str,
    #[case] entry: ReinitEntry,
) {
    let config = &start_server.config;
    let name = format!("matrix_ws_{action_type}_{entry:?}").to_lowercase();
    let workflow = create_test_workflow(config, &name);
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    // A job that will fail, giving each entry point a real job to reset.
    let work_id = create_test_job(config, workflow_id, "work")
        .expect("create work")
        .id
        .unwrap();
    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_workflow_start",
            action_type,
            action_config_for(action_type),
            None,
        ),
    )
    .expect("create action");
    let action_id = action.id.unwrap();

    manager.initialize(true).expect("initialize");
    let run_id = manager.get_run_id().expect("run_id");
    let compute_node_id = create_test_compute_node(config, workflow_id).expect("compute node");

    // on_workflow_start is pending at init; claim it (it fired).
    wait_for_pending_action(config, workflow_id);
    apis::workflow_actions_api::claim_action(
        config,
        workflow_id,
        action_id,
        ClaimActionRequest {
            compute_node_id: Some(compute_node_id),
        },
    )
    .expect("claim action");
    assert!(fetch_action(config, workflow_id, action_id).executed);

    // Fail the work job so there is a Failed job to reset.
    run_job_to_status(
        config,
        workflow_id,
        work_id,
        run_id,
        compute_node_id,
        1,
        JobStatus::Failed,
    );

    reset_and_reinit_via(entry, config, start_server, &manager, workflow_id, work_id);

    assert!(
        fetch_action(config, workflow_id, action_id).executed,
        "{action_type}/{entry:?}: on_workflow_start action must stay executed (a reinit is not a new start)"
    );
    assert!(
        !action_is_pending(config, workflow_id, action_id),
        "{action_type}/{entry:?}: suppressed on_workflow_start action must not be pending after reinit"
    );
}

/// DECISION TABLE (`on_workflow_complete`): always RE-ARMED on reinitialize (the workflow will
/// complete again at the end of the re-run), for both action types, across all three reinit entry
/// points — and the re-armed action actually fires again when the workflow next completes. This
/// guards the catch-all `_ => false` arm. The single job is driven to Failed (a terminal state, so
/// the workflow counts as complete and the action fires) which also leaves it as the sole Failed job
/// for the uniform reset+reinit helper.
#[rstest]
#[case::sched_reinit("schedule_nodes", ReinitEntry::Reinit)]
#[case::sched_cli("schedule_nodes", ReinitEntry::Cli)]
#[case::sched_recover("schedule_nodes", ReinitEntry::Recover)]
#[case::cmd_reinit("run_commands", ReinitEntry::Reinit)]
#[case::cmd_cli("run_commands", ReinitEntry::Cli)]
#[case::cmd_recover("run_commands", ReinitEntry::Recover)]
fn test_reinit_workflow_complete_rearmed_matrix(
    start_server: &ServerProcess,
    #[case] action_type: &str,
    #[case] entry: ReinitEntry,
) {
    let config = &start_server.config;
    let name = format!("matrix_wc_{action_type}_{entry:?}").to_lowercase();
    let workflow = create_test_workflow(config, &name);
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    // Single job; failing it makes the workflow complete (Failed is terminal) and gives the helper a
    // sole Failed job to reset.
    let job_id = create_test_job(config, workflow_id, "j1")
        .expect("create job")
        .id
        .unwrap();
    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_workflow_complete",
            action_type,
            action_config_for(action_type),
            None,
        ),
    )
    .expect("create action");
    let action_id = action.id.unwrap();

    manager.initialize(true).expect("initialize");
    let run_id = manager.get_run_id().expect("run_id");
    let compute_node_id = create_test_compute_node(config, workflow_id).expect("compute node");

    // Run the job to Failed -> workflow is complete -> on_workflow_complete fires; claim it.
    run_job_to_status(
        config,
        workflow_id,
        job_id,
        run_id,
        compute_node_id,
        1,
        JobStatus::Failed,
    );
    wait_for_pending_action(config, workflow_id);
    apis::workflow_actions_api::claim_action(
        config,
        workflow_id,
        action_id,
        ClaimActionRequest {
            compute_node_id: Some(compute_node_id),
        },
    )
    .expect("claim action");
    assert!(fetch_action(config, workflow_id, action_id).executed);

    reset_and_reinit_via(entry, config, start_server, &manager, workflow_id, job_id);

    assert!(
        !fetch_action(config, workflow_id, action_id).executed,
        "{action_type}/{entry:?}: on_workflow_complete action must be re-armed on reinit"
    );

    // Prove the re-arm is effective: completing the job again completes the workflow and re-fires it.
    let run_id2 = manager.get_run_id().expect("run_id2");
    wait_for_job_status(config, job_id, JobStatus::Ready);
    run_job_to_status(
        config,
        workflow_id,
        job_id,
        run_id2,
        compute_node_id,
        0,
        JobStatus::Completed,
    );
    wait_for_specific_action_pending(config, workflow_id, action_id);
}

// ===========================================================================
// `torc submit` supports on_jobs_ready scheduling: after a subset re-run, the set of pending
// schedule_nodes actions that submit fires (across on_workflow_start / on_jobs_ready /
// on_jobs_complete) must be exactly the reset classes. This is the selection that lets
// `reset-status <ids> --reinit` + `submit` re-schedule only the jobs being re-run.
// ===========================================================================

/// True if `action_id` is in the set of pending schedule_nodes actions that `WorkflowManager::start`
/// (i.e. `torc submit`) would fire: pending actions across the three schedule-capable trigger types.
fn action_in_submit_pending_set(
    config: &torc::client::Configuration,
    workflow_id: i64,
    action_id: i64,
) -> bool {
    apis::workflow_actions_api::get_pending_actions(
        config,
        workflow_id,
        Some(vec![
            "on_workflow_start".to_string(),
            "on_jobs_ready".to_string(),
            "on_jobs_complete".to_string(),
        ]),
    )
    .expect("get_pending_actions")
    .iter()
    .any(|a| a.id == Some(action_id))
}

/// The reported scenario: per-class `on_jobs_ready` schedule_nodes actions (regular / bigmem / gpu),
/// plus a postprocess job depending on all three with its own action. Run once, then reset only the
/// gpu + bigmem jobs and reinitialize. The submit pending-set must then be EXACTLY {gpu, bigmem}:
/// regular stays suppressed (its jobs are still terminal), and postprocess is not pending (its job is
/// Blocked). So `torc submit` re-schedules only the gpu + bigmem allocations.
#[rstest]
fn test_submit_pending_set_after_subset_reinit_is_only_reset_classes(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "submit_on_jobs_ready_subset");
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    // Three root job classes + a postprocess job gated on all of them.
    let regular = create_test_job(config, workflow_id, "regular1")
        .expect("create regular")
        .id
        .unwrap();
    let bigmem = create_test_job(config, workflow_id, "bigmem1")
        .expect("create bigmem")
        .id
        .unwrap();
    let gpu = create_test_job(config, workflow_id, "gpu1")
        .expect("create gpu")
        .id
        .unwrap();
    let mut post = JobModel::new(
        workflow_id,
        "postprocess".to_string(),
        "echo post".to_string(),
    );
    post.depends_on_job_ids = Some(vec![regular, bigmem, gpu]);
    post.cancel_on_blocking_job_failure = Some(false);
    let post_id = apis::jobs_api::create_job(config, post)
        .expect("create postprocess")
        .id
        .unwrap();

    // One on_jobs_ready schedule_nodes action per class + one for postprocess.
    let mk_action = |trigger_jobs: Vec<i64>| {
        apis::workflow_actions_api::create_workflow_action(
            config,
            workflow_id,
            workflow_action(
                workflow_id,
                "on_jobs_ready",
                "schedule_nodes",
                schedule_nodes_config(),
                Some(trigger_jobs),
            ),
        )
        .expect("create action")
        .id
        .unwrap()
    };
    let reg_action = mk_action(vec![regular]);
    let big_action = mk_action(vec![bigmem]);
    let gpu_action = mk_action(vec![gpu]);
    let post_action = mk_action(vec![post_id]);

    manager.initialize(true).expect("initialize");
    let run_id = manager.get_run_id().expect("run_id");
    let compute_node_id = create_test_compute_node(config, workflow_id).expect("compute node");

    // Run 1: the three roots are Ready at init, so their actions are pending. Claim each (fired),
    // then complete the jobs so postprocess becomes ready; claim its action and run postprocess.
    for (job, action) in [
        (regular, reg_action),
        (bigmem, big_action),
        (gpu, gpu_action),
    ] {
        wait_for_specific_action_pending(config, workflow_id, action);
        apis::workflow_actions_api::claim_action(
            config,
            workflow_id,
            action,
            ClaimActionRequest {
                compute_node_id: Some(compute_node_id),
            },
        )
        .expect("claim class action");
        run_job_to_status(
            config,
            workflow_id,
            job,
            run_id,
            compute_node_id,
            0,
            JobStatus::Completed,
        );
    }
    wait_for_job_status(config, post_id, JobStatus::Ready);
    wait_for_specific_action_pending(config, workflow_id, post_action);
    apis::workflow_actions_api::claim_action(
        config,
        workflow_id,
        post_action,
        ClaimActionRequest {
            compute_node_id: Some(compute_node_id),
        },
    )
    .expect("claim postprocess action");
    run_job_to_status(
        config,
        workflow_id,
        post_id,
        run_id,
        compute_node_id,
        1,
        JobStatus::Failed,
    );

    // Subset re-run: reset only gpu + bigmem (postprocess cascade-resets since it depends on them),
    // then reinitialize. regular is left untouched.
    apis::jobs_api::manage_status_change(config, gpu, JobStatus::Uninitialized, run_id)
        .expect("reset gpu");
    apis::jobs_api::manage_status_change(config, bigmem, JobStatus::Uninitialized, run_id)
        .expect("reset bigmem");
    manager.reinitialize(true, false).expect("reinitialize");

    // The submit pending-set must be exactly {gpu, bigmem}.
    assert!(
        action_in_submit_pending_set(config, workflow_id, gpu_action),
        "gpu action must be pending (gpu was reset and is Ready) so submit re-schedules it"
    );
    assert!(
        action_in_submit_pending_set(config, workflow_id, big_action),
        "bigmem action must be pending (bigmem was reset and is Ready) so submit re-schedules it"
    );
    assert!(
        !action_in_submit_pending_set(config, workflow_id, reg_action),
        "regular action must NOT be pending (its jobs stayed terminal) so submit does not re-schedule it"
    );
    assert!(
        !action_in_submit_pending_set(config, workflow_id, post_action),
        "postprocess action must NOT be pending yet (postprocess is Blocked); a running worker fires it later"
    );
    // regular's action stays executed (kept), confirming it was suppressed rather than just unready.
    assert!(
        fetch_action(config, workflow_id, reg_action).executed,
        "regular action should remain executed (kept) after the subset reinit"
    );
}

// ===========================================================================
// Full init (`torc workflows init`, only_uninitialized = false) is a clean slate: it resets every
// job and re-arms EVERY action, including on_workflow_start. This contrasts with a partial
// reinitialize (only_uninitialized = true), which keeps a satisfied on_workflow_start action
// (test_workflow_start_schedule_action_kept_on_reinitialize).
// ===========================================================================

/// A full initialize re-arms an already-fired on_workflow_start action (clean slate), so re-running
/// `torc workflows init` on a workflow that already ran does not leave its start actions stranded.
#[rstest]
fn test_full_init_rearms_workflow_start_action(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "full_init_rearms_workflow_start");
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    create_test_job(config, workflow_id, "j1").expect("create job");
    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_workflow_start",
            "schedule_nodes",
            schedule_nodes_config(),
            None,
        ),
    )
    .expect("create action");
    let action_id = action.id.unwrap();

    // First init: on_workflow_start is pending; claim it (fired).
    manager.initialize(true).expect("initialize");
    let compute_node_id = create_test_compute_node(config, workflow_id).expect("compute node");
    wait_for_pending_action(config, workflow_id);
    apis::workflow_actions_api::claim_action(
        config,
        workflow_id,
        action_id,
        ClaimActionRequest {
            compute_node_id: Some(compute_node_id),
        },
    )
    .expect("claim action");
    assert!(fetch_action(config, workflow_id, action_id).executed);

    // Full init again (what `torc workflows init` does: reset all jobs + initialize). This is a
    // clean slate, so the on_workflow_start action is re-armed (unlike a partial reinitialize, which
    // keeps it).
    manager.initialize(true).expect("re-initialize (full)");

    assert!(
        !fetch_action(config, workflow_id, action_id).executed,
        "full init must re-arm on_workflow_start (clean slate)"
    );
    assert!(
        action_is_pending(config, workflow_id, action_id),
        "re-armed on_workflow_start should be pending again after a full init"
    );
}

// ===========================================================================
// `torc slurm schedule-nodes` pending-action handling
// (handle_pending_schedule_actions): a worker started by schedule-nodes would claim the workflow's
// own pending schedule_nodes actions and submit their allocations on top of the manual request.
// --suppress-actions marks them executed; --no-prompts (non-interactive) proceeds and lets them
// fire. These call the helper directly (no Slurm submission / sbatch needed).
// ===========================================================================

/// Create a workflow with one job and a pending `on_workflow_start` `schedule_nodes` action
/// (pending right after initialize). Returns (workflow_id, action_id).
fn workflow_with_pending_schedule_action(
    config: &torc::client::Configuration,
    name: &str,
) -> (i64, i64) {
    let workflow = create_test_workflow(config, name);
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );
    create_test_job(config, workflow_id, "j1").expect("create job");
    let action_id = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_workflow_start",
            "schedule_nodes",
            schedule_nodes_config(),
            None,
        ),
    )
    .expect("create action")
    .id
    .unwrap();
    manager.initialize(true).expect("initialize");
    wait_for_pending_action(config, workflow_id);
    (workflow_id, action_id)
}

/// `--suppress-actions`: a pending `schedule_nodes` action is marked executed so the worker started
/// by `schedule-nodes` will not re-fire it.
#[rstest]
fn test_schedule_nodes_suppress_actions_marks_pending_executed(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, action_id) =
        workflow_with_pending_schedule_action(config, "schedule_nodes_suppress");
    assert!(
        !fetch_action(config, workflow_id, action_id).executed,
        "precondition: action is pending (not executed)"
    );

    torc::client::commands::slurm::handle_pending_schedule_actions(
        config,
        workflow_id,
        1,
        /* suppress_actions */ true,
        /* no_prompts */ false,
    )
    .expect("handle_pending_schedule_actions");

    assert!(
        fetch_action(config, workflow_id, action_id).executed,
        "--suppress-actions must mark the pending schedule_nodes action executed"
    );
    assert!(
        !action_is_pending(config, workflow_id, action_id),
        "suppressed action must no longer be pending"
    );
}

/// `--no-prompts` (non-interactive, no `--suppress-actions`): proceed and leave the action armed so
/// it still fires (historical behavior).
#[rstest]
fn test_schedule_nodes_no_prompts_proceeds_without_suppressing(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, action_id) =
        workflow_with_pending_schedule_action(config, "schedule_nodes_no_prompts");

    torc::client::commands::slurm::handle_pending_schedule_actions(
        config,
        workflow_id,
        1,
        /* suppress_actions */ false,
        /* no_prompts */ true,
    )
    .expect("handle_pending_schedule_actions");

    assert!(
        !fetch_action(config, workflow_id, action_id).executed,
        "--no-prompts without --suppress-actions must leave the action armed (proceed)"
    );
    assert!(
        action_is_pending(config, workflow_id, action_id),
        "un-suppressed action must remain pending so the worker still fires it"
    );
}

/// The helper only targets `schedule_nodes` actions: a pending `run_commands` action is left armed
/// even with `--suppress-actions` (it does not submit Slurm allocations, so it is not the hazard).
#[rstest]
fn test_schedule_nodes_suppress_ignores_run_commands(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "schedule_nodes_ignores_run_commands");
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );
    create_test_job(config, workflow_id, "j1").expect("create job");
    let action_id = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_workflow_start",
            "run_commands",
            json!({ "commands": ["echo hi"] }),
            None,
        ),
    )
    .expect("create action")
    .id
    .unwrap();
    manager.initialize(true).expect("initialize");
    wait_for_pending_action(config, workflow_id);

    torc::client::commands::slurm::handle_pending_schedule_actions(
        config,
        workflow_id,
        1,
        /* suppress_actions */ true,
        /* no_prompts */ false,
    )
    .expect("handle_pending_schedule_actions");

    assert!(
        !fetch_action(config, workflow_id, action_id).executed,
        "run_commands action must not be suppressed (only schedule_nodes is the hazard)"
    );
}

/// On the first run (`run_id <= 1`) the submit review is a no-op: there is nothing to reconcile, so
/// it returns no overrides without inspecting or mutating any action.
#[rstest]
fn test_review_submit_pending_actions_first_run_is_noop(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "review_submit_first_run");
    let workflow_id = workflow.id.unwrap();

    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_workflow_start",
            "schedule_nodes",
            json!({"scheduler_type": "slurm", "scheduler_id": 1, "num_allocations": 2}),
            None,
        ),
    )
    .expect("create action");
    let action_id = action.id.unwrap();

    let overrides = torc::client::commands::slurm::review_submit_pending_actions(
        config,
        workflow_id,
        1, // first run
        false,
    )
    .expect("review");

    assert!(
        overrides.is_empty(),
        "first-run review must not produce allocation overrides"
    );
    assert!(
        !fetch_action(config, workflow_id, action_id).executed,
        "first-run review must not suppress any action"
    );
}

/// On a re-submission (`run_id > 1`) the review must NEVER suppress or change anything when run
/// non-interactively (the test harness has a non-TTY stdin, which the helper treats like
/// `--no-prompts`): it prints what will happen and proceeds with the configured counts. This guards
/// the safe default — the interactive disable/override path is opt-in only.
#[rstest]
fn test_review_submit_pending_actions_noninteractive_does_not_suppress(
    start_server: &ServerProcess,
) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "review_submit_noninteractive");
    let workflow_id = workflow.id.unwrap();
    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );

    // Root job gated by an on_jobs_ready schedule_nodes action; after initialize the root job is
    // Ready, so the action becomes pending (the "submit now" group).
    let job_id = create_test_job(config, workflow_id, "root")
        .expect("create root")
        .id
        .unwrap();
    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        workflow_action(
            workflow_id,
            "on_jobs_ready",
            "schedule_nodes",
            json!({"scheduler_type": "slurm", "scheduler_id": 1, "num_allocations": 3}),
            Some(vec![job_id]),
        ),
    )
    .expect("create action");
    let action_id = action.id.unwrap();

    manager.initialize(true).expect("initialize");
    wait_for_pending_action(config, workflow_id);
    assert!(
        action_is_pending(config, workflow_id, action_id),
        "action should be pending after initialize"
    );

    let overrides = torc::client::commands::slurm::review_submit_pending_actions(
        config,
        workflow_id,
        2, // re-submission
        false,
    )
    .expect("review");

    assert!(
        overrides.is_empty(),
        "non-interactive review must not override allocation counts"
    );
    assert!(
        action_is_pending(config, workflow_id, action_id),
        "non-interactive review must leave the pending action pending (not suppressed)"
    );
    assert!(
        !fetch_action(config, workflow_id, action_id).executed,
        "non-interactive review must not mark the action executed"
    );
}
