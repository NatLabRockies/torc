mod common;

use std::thread;
use std::time::Duration;

use common::{ServerProcess, create_test_workflow, start_server};
use rstest::rstest;
use serde_json::json;
use torc::client::apis;
use torc::client::workflow_manager::WorkflowManager;
use torc::config::TorcConfig;
use torc::models::{ClaimActionRequest, JobModel, WorkflowActionModel};

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
