//! Tests for auto-schedule functionality in the watch command.
//!
//! These tests verify that:
//! 1. CLI arguments for auto-schedule are properly parsed
//! 2. The scenario detection logic (ready jobs, no schedulers) works correctly
//!
//! Note: Full integration testing of the auto-schedule watch loop requires Slurm
//! and is not feasible in unit tests. These tests verify the building blocks.

mod common;

use common::{ServerProcess, start_server};
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
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None)
        .expect("Failed to initialize jobs");

    (workflow_id, job_ids)
}

/// Test that auto-schedule CLI arguments are parsed correctly
#[rstest]
fn test_auto_schedule_cli_help(_start_server: &ServerProcess) {
    use std::process::Command;

    // Run torc watch --help and verify auto-schedule options are present
    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "torc",
            "--features",
            "client,tui,plot_resources",
            "--",
            "watch",
            "--help",
        ])
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

/// Test scenario: No schedulers and ready jobs - the condition for immediate auto-schedule
#[rstest]
fn test_no_schedulers_with_ready_jobs_scenario(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow with ready jobs but no scheduled compute nodes
    let (workflow_id, job_ids) =
        create_workflow_with_ready_jobs(config, "test_no_schedulers_scenario", 5);

    // Verify no scheduled compute nodes exist
    let scn_response = apis::scheduled_compute_nodes_api::list_scheduled_compute_nodes(
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
    .expect("Failed to list scheduled compute nodes");

    let scheduled_nodes = scn_response.items;
    assert!(
        scheduled_nodes.is_empty(),
        "Should have no scheduled compute nodes"
    );

    // Verify we have ready jobs
    let jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        Some(models::JobStatus::Ready),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list jobs");

    let ready_jobs = jobs.items;
    assert_eq!(
        ready_jobs.len(),
        job_ids.len(),
        "All jobs should be in Ready state"
    );

    // This scenario (ready jobs + no schedulers) is exactly when auto-schedule
    // should trigger immediately without waiting for threshold.
}

/// Test that jobs have default attempt_id = 1
#[rstest]
fn test_jobs_have_default_attempt_id(start_server: &ServerProcess) {
    let config = &start_server.config;

    let (_workflow_id, job_ids) =
        create_workflow_with_ready_jobs(config, "test_default_attempt_id", 3);

    // Verify all jobs have attempt_id = 1
    for job_id in job_ids {
        let job = apis::jobs_api::get_job(config, job_id).expect("Failed to get job");
        assert_eq!(
            job.attempt_id,
            Some(1),
            "New jobs should have attempt_id = 1"
        );
    }
}

/// Test that counting jobs by attempt_id works (simulating the count_ready_retry_jobs function)
#[rstest]
fn test_count_jobs_by_attempt_id(start_server: &ServerProcess) {
    let config = &start_server.config;

    let (workflow_id, _job_ids) =
        create_workflow_with_ready_jobs(config, "test_count_by_attempt", 10);

    // Get all ready jobs
    let jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        Some(models::JobStatus::Ready),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list jobs");

    let ready_jobs = jobs.items;
    let total_ready = ready_jobs.len();
    let retry_count = ready_jobs
        .iter()
        .filter(|job| job.attempt_id.unwrap_or(1) > 1)
        .count();

    assert_eq!(total_ready, 10, "Should have 10 total ready jobs");
    assert_eq!(
        retry_count, 0,
        "New jobs should have no retries (attempt_id = 1)"
    );
}

/// Test that slurm schedulers can be created for a workflow
#[rstest]
fn test_create_slurm_scheduler(start_server: &ServerProcess) {
    let config = &start_server.config;

    let (workflow_id, _job_ids) =
        create_workflow_with_ready_jobs(config, "test_create_scheduler", 3);

    // Create a slurm scheduler
    let scheduler = models::SlurmSchedulerModel {
        id: None,
        workflow_id,
        name: Some("test_scheduler".to_string()),
        account: "test_account".to_string(),
        partition: Some("standard".to_string()),
        mem: Some("8g".to_string()),
        walltime: "01:00:00".to_string(),
        nodes: 1,
        gres: None,
        ntasks_per_node: None,
        qos: None,
        tmp: None,
        extra: None,
    };

    let created = apis::slurm_schedulers_api::create_slurm_scheduler(config, scheduler)
        .expect("Failed to create scheduler");

    assert!(created.id.is_some(), "Scheduler should have an ID");
    assert_eq!(created.account, "test_account");

    // Verify we can list the scheduler
    let response = apis::slurm_schedulers_api::list_slurm_schedulers(
        config,
        workflow_id,
        Some(0),
        Some(100),
        None,
        None,
    )
    .expect("Failed to list schedulers");

    let schedulers = response.items;
    assert_eq!(schedulers.len(), 1, "Should have 1 scheduler");
}

/// Test that scheduled_compute_nodes can be created (simulating what regenerate does)
#[rstest]
fn test_create_scheduled_compute_node(start_server: &ServerProcess) {
    let config = &start_server.config;

    let (workflow_id, _job_ids) = create_workflow_with_ready_jobs(config, "test_create_scn", 2);

    // Create a slurm scheduler first
    let scheduler = models::SlurmSchedulerModel {
        id: None,
        workflow_id,
        name: Some("test_scheduler".to_string()),
        account: "test_account".to_string(),
        partition: None,
        mem: Some("8g".to_string()),
        walltime: "01:00:00".to_string(),
        nodes: 1,
        gres: None,
        ntasks_per_node: None,
        qos: None,
        tmp: None,
        extra: None,
    };

    let created_scheduler = apis::slurm_schedulers_api::create_slurm_scheduler(config, scheduler)
        .expect("Failed to create scheduler");
    let scheduler_config_id = created_scheduler.id.unwrap();

    // Create a scheduled compute node (simulating what happens when sbatch submits)
    let scn = models::ScheduledComputeNodesModel {
        id: None,
        workflow_id,
        scheduler_id: 12345, // Mock Slurm job ID
        scheduler_config_id,
        scheduler_type: "slurm".to_string(),
        status: "pending".to_string(),
    };

    let created_scn = apis::scheduled_compute_nodes_api::create_scheduled_compute_node(config, scn)
        .expect("Failed to create scheduled compute node");

    assert!(created_scn.id.is_some(), "SCN should have an ID");
    assert_eq!(created_scn.status, "pending");

    // Verify it shows up in list
    let response = apis::scheduled_compute_nodes_api::list_scheduled_compute_nodes(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        Some("pending"),
    )
    .expect("Failed to list SCNs");

    let scns = response.items;
    assert_eq!(scns.len(), 1, "Should have 1 pending SCN");
}

/// Test the regenerate command with dry-run (the command auto-schedule calls)
#[rstest]
fn test_regenerate_command_dry_run(start_server: &ServerProcess) {
    use common::run_cli_with_json;

    let config = &start_server.config;

    let (workflow_id, _job_ids) =
        create_workflow_with_ready_jobs(config, "test_regenerate_dry_run", 5);

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
}
