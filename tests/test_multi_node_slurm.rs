/// Tests for multi-node Slurm scheduling patterns:
///   1. Two-node allocation, single worker, true multi-node step (num_nodes = 2)
///   2. Two-node allocation, multiple parallel single-node jobs
mod common;

use std::fs;

use common::{ServerProcess, start_server};
use rstest::rstest;
use tempfile::NamedTempFile;
use torc::client::default_api;
use torc::client::workflow_spec::WorkflowSpec;
use torc::models::JobStatus;

// =============================================================================
// Pattern 1: 2-node allocation, job requires both nodes (num_nodes=2)
// =============================================================================

/// Verify that a workflow with a 2-node Slurm allocation and a single job that
/// spans both nodes (num_nodes=2) is accepted and stores the resource
/// requirements correctly.
#[rstest]
fn test_two_node_allocation_single_worker_multi_node_step(start_server: &ServerProcess) {
    let workflow_data = serde_json::json!({
        "name": "multi_node_step_workflow",
        "description": "2-node allocation, single worker, job spans both nodes",
        "jobs": [
            {
                "name": "mpi_job",
                "command": "srun --mpi=pmix python mpi_train.py",
                "resource_requirements": "two_node_req",
                "scheduler": "two_node_scheduler"
            }
        ],
        "resource_requirements": [
            {
                "name": "two_node_req",
                "num_cpus": 32,
                "num_nodes": 2,
                "memory": "128g",
                "runtime": "PT4H"
            }
        ],
        "slurm_schedulers": [
            {
                "name": "two_node_scheduler",
                "account": "test_account",
                "nodes": 2,
                "walltime": "04:00:00"
            }
        ],
        "actions": [
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes",
                "scheduler": "two_node_scheduler",
                "scheduler_type": "slurm",
                "num_allocations": 1
            }
        ]
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "test_user",
        false,
        false, // skip_checks = false — validation must pass
    );

    assert!(
        result.is_ok(),
        "Workflow with num_nodes=2 should be valid, got: {:?}",
        result.err()
    );

    let workflow_id = result.unwrap();

    // --- Verify resource requirements were persisted correctly ---
    let rr_list = default_api::list_resource_requirements(
        &start_server.config,
        workflow_id,
        None, // job_id
        None, // offset
        None, // limit
        None, // sort_by
        None, // reverse_sort
        None, // name
        None, // memory
        None, // num_cpus
        None, // num_gpus
        None, // num_nodes
        None, // runtime
    )
    .expect("Failed to list resource requirements")
    .items
    .unwrap_or_default();

    // Filter out the "default" RR that is auto-created for every workflow
    let rr_list: Vec<_> = rr_list
        .into_iter()
        .filter(|r| r.name != "default")
        .collect();
    assert_eq!(
        rr_list.len(),
        1,
        "Expected exactly 1 user-defined resource requirements record"
    );
    let rr = &rr_list[0];
    assert_eq!(rr.name, "two_node_req");
    assert_eq!(rr.num_nodes, 2, "num_nodes should be 2");
    assert_eq!(rr.num_cpus, 32, "num_cpus should be 32");

    // --- Verify scheduler has 2 nodes ---
    let schedulers = default_api::list_slurm_schedulers(
        &start_server.config,
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
        None,
        None,
    )
    .expect("Failed to list slurm schedulers")
    .items
    .unwrap_or_default();

    assert_eq!(schedulers.len(), 1, "Expected 1 scheduler");
    assert_eq!(schedulers[0].nodes, 2, "Scheduler should have 2 nodes");

    // --- Verify schedule_nodes action was created ---
    let actions = default_api::get_workflow_actions(&start_server.config, workflow_id)
        .expect("Failed to get workflow actions");

    let schedule_actions: Vec<_> = actions
        .into_iter()
        .filter(|a| a.action_type == "schedule_nodes")
        .collect();

    assert_eq!(
        schedule_actions.len(),
        1,
        "Expected 1 schedule_nodes action"
    );

    let _ = default_api::delete_workflow(&start_server.config, workflow_id, None);
}

// =============================================================================
// Pattern 2: 2-node allocation, multiple parallel single-node jobs
// =============================================================================

/// Verify that a workflow with a 2-node Slurm allocation and multiple single-node jobs
/// is accepted and all jobs become ready after initialization (i.e., they can be dispatched
/// in parallel across the workers).
#[rstest]
fn test_two_node_allocation_one_worker_per_node_parallel_jobs(start_server: &ServerProcess) {
    let workflow_data = serde_json::json!({
        "name": "parallel_single_node_jobs_workflow",
        "description": "2-node allocation with one-worker-per-node, 4 parallel single-node jobs",
        "jobs": [
            {
                "name": "work_a",
                "command": "python work.py --id a",
                "resource_requirements": "single_node_req",
                "scheduler": "two_node_scheduler"
            },
            {
                "name": "work_b",
                "command": "python work.py --id b",
                "resource_requirements": "single_node_req",
                "scheduler": "two_node_scheduler"
            },
            {
                "name": "work_c",
                "command": "python work.py --id c",
                "resource_requirements": "single_node_req",
                "scheduler": "two_node_scheduler"
            },
            {
                "name": "work_d",
                "command": "python work.py --id d",
                "resource_requirements": "single_node_req",
                "scheduler": "two_node_scheduler"
            }
        ],
        "resource_requirements": [
            {
                "name": "single_node_req",
                "num_cpus": 8,
                "num_nodes": 1,
                "memory": "64g",
                "runtime": "PT2H"
            }
        ],
        "slurm_schedulers": [
            {
                "name": "two_node_scheduler",
                "account": "test_account",
                "nodes": 2,
                "walltime": "02:00:00"
            }
        ],
        "actions": [
            {
                "trigger_type": "on_workflow_start",
                "action_type": "schedule_nodes",
                "scheduler": "two_node_scheduler",
                "scheduler_type": "slurm",
                "num_allocations": 1
            }
        ]
    });

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(
        temp_file.path(),
        serde_json::to_string_pretty(&workflow_data).unwrap(),
    )
    .expect("Failed to write temp file");

    let result = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "test_user",
        false,
        false, // skip_checks = false — validation must pass
    );

    assert!(
        result.is_ok(),
        "Workflow with 2-node allocation and parallel jobs should be valid, got: {:?}",
        result.err()
    );

    let workflow_id = result.unwrap();

    // --- Verify schedule_nodes action was created ---
    let actions = default_api::get_workflow_actions(&start_server.config, workflow_id)
        .expect("Failed to get workflow actions");

    let schedule_actions: Vec<_> = actions
        .into_iter()
        .filter(|a| a.action_type == "schedule_nodes")
        .collect();

    assert_eq!(
        schedule_actions.len(),
        1,
        "Expected 1 schedule_nodes action"
    );

    // --- Verify resource requirements use num_nodes=1 ---
    let rr_list = default_api::list_resource_requirements(
        &start_server.config,
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
    .unwrap_or_default();

    // Filter out the "default" RR that is auto-created for every workflow
    let rr_list: Vec<_> = rr_list
        .into_iter()
        .filter(|r| r.name != "default")
        .collect();
    assert_eq!(
        rr_list.len(),
        1,
        "Expected exactly 1 user-defined resource requirements record"
    );
    let rr = &rr_list[0];
    assert_eq!(
        rr.num_nodes, 1,
        "num_nodes should be 1 for single-node jobs"
    );

    // --- Initialize the workflow so jobs transition to 'ready' ---
    default_api::initialize_jobs(&start_server.config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    // --- Verify all 4 jobs are now ready ---
    let jobs = default_api::list_jobs(
        &start_server.config,
        workflow_id,
        None, // status
        None, // offset
        None, // limit
        None, // sort_by
        Some(10000),
        None,
        None,
        None, // include_relationships
        None, // active_compute_node_id
    )
    .expect("Failed to list jobs")
    .items
    .unwrap_or_default();

    assert_eq!(jobs.len(), 4, "Expected 4 jobs");

    let ready_count = jobs
        .iter()
        .filter(|j| j.status == Some(JobStatus::Ready))
        .count();

    assert_eq!(
        ready_count, 4,
        "All 4 single-node jobs should be ready after initialization (got {} ready out of 4)",
        ready_count
    );

    let _ = default_api::delete_workflow(&start_server.config, workflow_id, None);
}
