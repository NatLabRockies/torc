//! Tests for auto-schedule functionality in the watch command.
//!
//! These tests verify that:
//! 1. CLI arguments for auto-schedule are properly parsed
//! 2. The scenario detection logic (ready jobs, no schedulers) works correctly
//!
//! Note: Full integration testing of the auto-schedule watch loop requires Slurm
//! and is not feasible in unit tests. These tests verify the building blocks.

mod common;

use common::{ServerProcess, ensure_test_binaries_built, start_server, torc_binary_path};
use rstest::rstest;
use torc::client::{Configuration, apis};
use torc::models;

/// Create a simple workflow with ready jobs for testing scenarios.
fn create_workflow_with_ready_jobs(
    config: &Configuration,
    name: &str,
    num_jobs: usize,
) -> (i64, Vec<i64>) {
    // Create workflow
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new(name.to_string(), user);
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create resource requirements
    let mut rr = models::ResourceRequirementsModel::new(workflow_id, "test_rr".to_string());
    rr.num_cpus = 4;
    rr.num_gpus = 0;
    rr.num_nodes = 1;
    rr.memory = "8g".to_string();
    rr.runtime = "PT1H".to_string();
    let rr = apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("Failed to create resource requirements");
    let rr_id = rr.id.unwrap();

    // Create jobs
    let mut job_ids = Vec::new();
    for i in 0..num_jobs {
        let mut job =
            models::JobModel::new(workflow_id, format!("job_{}", i), format!("echo job_{}", i));
        job.resource_requirements_id = Some(rr_id);
        let created = apis::jobs_api::create_job(config, job).expect("Failed to create job");
        job_ids.push(created.id.unwrap());
    }

    // Initialize jobs (sets them to Ready state)
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    (workflow_id, job_ids)
}

/// Test that auto-schedule CLI arguments are parsed correctly
#[test]
fn test_auto_schedule_cli_help() {
    ensure_test_binaries_built();

    // Run torc watch --help and verify auto-schedule options are present
    let output = std::process::Command::new(torc_binary_path())
        .args(["watch", "--help"])
        .output()
        .expect("Failed to run command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        combined.contains("--auto-schedule"),
        "Help should mention --auto-schedule flag. Output: {}",
        combined
    );
    assert!(
        combined.contains("--auto-schedule-threshold"),
        "Help should mention --auto-schedule-threshold option. Output: {}",
        combined
    );
    assert!(
        combined.contains("--auto-schedule-cooldown"),
        "Help should mention --auto-schedule-cooldown option. Output: {}",
        combined
    );
}

/// Test the regenerate command with dry-run (the command auto-schedule calls)
#[rstest]
fn test_regenerate_command_dry_run(start_server: &ServerProcess) {
    use common::run_cli_with_json;

    let config = &start_server.config;

    let (workflow_id, _job_ids) =
        create_workflow_with_ready_jobs(config, "test_regenerate_dry_run", 5);

    // An existing schedule_nodes action. A real regenerate claims it (suppressing it);
    // a dry run must leave it untouched.
    let action = apis::workflow_actions_api::create_workflow_action(
        config,
        workflow_id,
        models::WorkflowActionModel {
            id: None,
            workflow_id,
            trigger_type: "on_workflow_start".to_string(),
            action_type: "schedule_nodes".to_string(),
            action_config: serde_json::json!({
                "scheduler_type": "slurm",
                "scheduler_id": 1,
                "num_allocations": 1
            }),
            job_ids: None,
            trigger_count: 0,
            required_triggers: 1,
            executed: false,
            executed_at: None,
            executed_by: None,
            persistent: false,
            is_recovery: false,
            state: None,
            error: None,
        },
    )
    .expect("Failed to create schedule_nodes action");
    let action_id = action.id.unwrap();

    // Run regenerate with dry-run to verify it works without actually submitting
    let args = [
        "slurm",
        "regenerate",
        &workflow_id.to_string(),
        "--account",
        "test_account",
        "--profile",
        "kestrel",
        "--dry-run",
    ];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(result.is_ok(), "Regenerate command failed: {:?}", result);

    let json = result.unwrap();

    // Verify it found the pending jobs
    assert_eq!(
        json.get("pending_jobs").and_then(|v| v.as_i64()),
        Some(5),
        "Should report 5 pending jobs. Output: {:?}",
        json
    );

    // Verify it's a dry run
    assert_eq!(
        json.get("dry_run").and_then(|v| v.as_bool()),
        Some(true),
        "Should indicate dry run"
    );

    let actions = apis::workflow_actions_api::get_workflow_actions(config, workflow_id)
        .expect("Failed to get workflow actions");
    let action = actions
        .iter()
        .find(|a| a.id == Some(action_id))
        .expect("action not found");
    assert!(!action.executed, "dry run must not claim existing actions");
}
