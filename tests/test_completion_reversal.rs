mod common;

use common::{ServerProcess, create_test_compute_node, create_test_workflow, start_server};
use rstest::rstest;
use serial_test::serial;
use torc::client::{apis::default_api, workflow_manager::WorkflowManager};
use torc::config::TorcConfig;
use torc::models;
use torc::models::JobStatus;

/// Test that resetting a failed job with failed_only=true resets all downstream jobs to Uninitialized
/// This tests the update_jobs_from_completion_reversal functionality which should be triggered
/// when a completed job is reset to a non-complete status.
#[rstest]
#[serial]
fn test_completion_reversal_resets_downstream_jobs(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "test_completion_reversal");
    let workflow_id = workflow.id.unwrap();

    // Create jobs in a dependency chain: job1 -> job2 -> job3
    // job1 blocks job2, job2 blocks job3

    // Create job1 (first in chain, will fail)
    let job1 = models::JobModel::new(
        workflow_id,
        "job1".to_string(),
        "echo 'job1 failed' && exit 1".to_string(),
    );
    let created_job1 = default_api::create_job(config, job1).expect("Failed to create job1");
    let job1_id = created_job1.id.unwrap();

    // Create job2 (blocked by job1)
    let mut job2 = models::JobModel::new(
        workflow_id,
        "job2".to_string(),
        "echo 'job2 success'".to_string(),
    );
    job2.depends_on_job_ids = Some(vec![job1_id]);
    job2.cancel_on_blocking_job_failure = Some(false);
    let created_job2 = default_api::create_job(config, job2).expect("Failed to create job2");
    let job2_id = created_job2.id.unwrap();

    // Create job3 (blocked by job2)
    let mut job3 = models::JobModel::new(
        workflow_id,
        "job3".to_string(),
        "echo 'job3 success'".to_string(),
    );
    job3.depends_on_job_ids = Some(vec![job2_id]);
    job3.cancel_on_blocking_job_failure = Some(false);
    let created_job3 = default_api::create_job(config, job3).expect("Failed to create job3");
    let job3_id = created_job3.id.unwrap();

    // Initialize the workflow to set up job dependencies
    let torc_config = TorcConfig::load().unwrap_or_default();
    let manager = WorkflowManager::new(config.clone(), torc_config, workflow);
    manager.initialize(true).expect("Failed to start workflow");

    // Verify initial job states - job1 should be ready, others blocked
    let job1_initial = default_api::get_job(config, job1_id).expect("Failed to get job1");
    let job2_initial = default_api::get_job(config, job2_id).expect("Failed to get job2");
    let job3_initial = default_api::get_job(config, job3_id).expect("Failed to get job3");

    assert_eq!(job1_initial.status.unwrap(), JobStatus::Ready);
    assert_eq!(job2_initial.status.unwrap(), JobStatus::Blocked);
    assert_eq!(job3_initial.status.unwrap(), JobStatus::Blocked);

    // Create a compute node for the results
    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Complete job1 with failure (return_code = 1, status = Failed)
    // Note: The job status must match the return_code - a non-zero return_code
    // should have status = Failed, not Completed. The reset_failed_jobs_only
    // query filters by status, not return_code.
    let job1_result = models::ResultModel::new(
        job1_id,
        workflow_id,
        1, // run_id
        1, // attempt_id
        compute_node_id,
        1,   // return_code (failure)
        1.0, // exec_time_minutes
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Failed,
    );

    default_api::complete_job(
        config,
        job1_id,
        job1_result.status,
        1, // run_id
        job1_result,
    )
    .expect("Failed to complete job1");

    // Complete job2 with success (return_code = 0)
    // Note: In real scenarios, job2 might be canceled due to job1's failure,
    // but for this test we'll manually complete it to simulate a scenario where
    // downstream jobs completed successfully despite the upstream failure
    let job2_result = models::ResultModel::new(
        job2_id,
        workflow_id,
        1, // run_id
        1, // attempt_id
        compute_node_id,
        0,   // return_code (success)
        1.0, // exec_time_minutes
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Completed,
    );

    default_api::complete_job(
        config,
        job2_id,
        job2_result.status,
        1, // run_id
        job2_result,
    )
    .expect("Failed to complete job2");

    // Complete job3 with success (return_code = 0)
    let job3_result = models::ResultModel::new(
        job3_id,
        workflow_id,
        1, // run_id
        1, // attempt_id
        compute_node_id,
        0,   // return_code (success)
        1.0, // exec_time_minutes
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Completed,
    );

    default_api::complete_job(
        config,
        job3_id,
        job3_result.status,
        1, // run_id
        job3_result,
    )
    .expect("Failed to complete job3");

    // Verify all jobs are now in their expected final states
    let job1_completed = default_api::get_job(config, job1_id).expect("Failed to get job1");
    let job2_completed = default_api::get_job(config, job2_id).expect("Failed to get job2");
    let job3_completed = default_api::get_job(config, job3_id).expect("Failed to get job3");

    assert_eq!(job1_completed.status.unwrap(), JobStatus::Failed);
    assert_eq!(job2_completed.status.unwrap(), JobStatus::Completed);
    assert_eq!(job3_completed.status.unwrap(), JobStatus::Completed);

    // Now call reset_job_status with failed_only = true
    // This should reset job1 (which failed) and trigger the completion reversal
    // which should reset all downstream jobs (job2 and job3) to Uninitialized
    default_api::reset_job_status(
        config,
        workflow_id,
        Some(true), // failed_only = true
        None,       // body
    )
    .expect("Failed to reset job status");

    // Verify that ALL jobs are now Uninitialized
    // The completion reversal should have reset not just the failed job (job1)
    // but also all downstream jobs (job2 and job3)
    let job1_final = default_api::get_job(config, job1_id).expect("Failed to get job1");
    let job2_final = default_api::get_job(config, job2_id).expect("Failed to get job2");
    let job3_final = default_api::get_job(config, job3_id).expect("Failed to get job3");

    assert_eq!(
        job1_final.status.unwrap(),
        JobStatus::Uninitialized,
        "job1 should be Uninitialized after reset"
    );
    assert_eq!(
        job2_final.status.unwrap(),
        JobStatus::Uninitialized,
        "job2 should be Uninitialized due to completion reversal"
    );
    assert_eq!(
        job3_final.status.unwrap(),
        JobStatus::Uninitialized,
        "job3 should be Uninitialized due to completion reversal"
    );
}

/// Test that completion reversal works with more complex dependency chains
/// This creates a diamond-shaped dependency pattern to test recursive propagation
#[rstest]
#[serial]
fn test_completion_reversal_complex_dependencies(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "test_complex_reversal");
    let workflow_id = workflow.id.unwrap();

    // Create a diamond dependency pattern:
    //     job1 (fails)
    //    /    \
    //  job2   job3
    //    \    /
    //     job4

    // Create job1 (root job, will fail)
    let job1 = models::JobModel::new(
        workflow_id,
        "job1".to_string(),
        "echo 'job1 failed' && exit 1".to_string(),
    );
    let created_job1 = default_api::create_job(config, job1).expect("Failed to create job1");
    let job1_id = created_job1.id.unwrap();

    // Create job2 (blocked by job1)
    let mut job2 = models::JobModel::new(
        workflow_id,
        "job2".to_string(),
        "echo 'job2 success'".to_string(),
    );
    job2.depends_on_job_ids = Some(vec![job1_id]);
    job2.cancel_on_blocking_job_failure = Some(false);
    let created_job2 = default_api::create_job(config, job2).expect("Failed to create job2");
    let job2_id = created_job2.id.unwrap();

    // Create job3 (blocked by job1)
    let mut job3 = models::JobModel::new(
        workflow_id,
        "job3".to_string(),
        "echo 'job3 success'".to_string(),
    );
    job3.depends_on_job_ids = Some(vec![job1_id]);
    job3.cancel_on_blocking_job_failure = Some(false);
    let created_job3 = default_api::create_job(config, job3).expect("Failed to create job3");
    let job3_id = created_job3.id.unwrap();

    // Create job4 (blocked by both job2 and job3)
    let mut job4 = models::JobModel::new(
        workflow_id,
        "job4".to_string(),
        "echo 'job4 success'".to_string(),
    );
    job4.depends_on_job_ids = Some(vec![job2_id, job3_id]);
    job4.cancel_on_blocking_job_failure = Some(false);
    let created_job4 = default_api::create_job(config, job4).expect("Failed to create job4");
    let job4_id = created_job4.id.unwrap();

    // Initialize the workflow
    let torc_config = TorcConfig::load().unwrap_or_default();
    let manager = WorkflowManager::new(config.clone(), torc_config, workflow);
    manager.initialize(true).expect("Failed to start workflow");

    // Create a compute node for the results
    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Complete all jobs - job1 fails, others succeed
    // Note: status must match return_code - non-zero return_code requires Failed status
    let jobs_and_results: Vec<(i64, i64, JobStatus)> = vec![
        (job1_id, 1, JobStatus::Failed),    // job1 fails
        (job2_id, 0, JobStatus::Completed), // job2 succeeds
        (job3_id, 0, JobStatus::Completed), // job3 succeeds
        (job4_id, 0, JobStatus::Completed), // job4 succeeds
    ];

    for (job_id, return_code, status) in jobs_and_results {
        let result = models::ResultModel::new(
            job_id,
            workflow_id,
            1, // run_id
            1, // attempt_id
            compute_node_id,
            return_code,
            1.0,
            chrono::Utc::now().to_rfc3339(),
            status,
        );

        default_api::complete_job(config, job_id, result.status, 1, result)
            .unwrap_or_else(|_| panic!("Failed to complete job {}", job_id));
    }

    // Verify all jobs are in their expected states
    assert_eq!(
        default_api::get_job(config, job1_id)
            .expect("Failed to get job1")
            .status
            .unwrap(),
        JobStatus::Failed
    );
    for job_id in [job2_id, job3_id, job4_id] {
        let job = default_api::get_job(config, job_id).expect("Failed to get job");
        assert_eq!(job.status.unwrap(), JobStatus::Completed);
    }

    // Reset failed jobs only
    default_api::reset_job_status(config, workflow_id, Some(true), None)
        .expect("Failed to reset job status");

    // Verify that ALL jobs are now Uninitialized
    // Since job1 failed and all other jobs depend on it (directly or indirectly),
    // they should all be reset to Uninitialized
    for job_id in [job1_id, job2_id, job3_id, job4_id] {
        let job = default_api::get_job(config, job_id)
            .unwrap_or_else(|_| panic!("Failed to get job {}", job_id));
        assert_eq!(
            job.status.unwrap(),
            JobStatus::Uninitialized,
            "Job {} should be Uninitialized after completion reversal",
            job_id
        );
    }
}

/// Test that completion reversal only affects jobs downstream of the reset job
/// Jobs that are not downstream should remain in their current state
#[rstest]
#[serial]
fn test_completion_reversal_selective_reset(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "test_selective_reversal");
    let workflow_id = workflow.id.unwrap();

    // Create two independent chains:
    // Chain 1: job1 (fails) -> job2
    // Chain 2: job3 (succeeds) -> job4
    // Only Chain 1 should be affected by the reset

    // Chain 1: job1 -> job2
    let job1 = models::JobModel::new(
        workflow_id,
        "job1".to_string(),
        "echo 'job1 failed' && exit 1".to_string(),
    );
    let created_job1 = default_api::create_job(config, job1).expect("Failed to create job1");
    let job1_id = created_job1.id.unwrap();

    let mut job2 = models::JobModel::new(
        workflow_id,
        "job2".to_string(),
        "echo 'job2 success'".to_string(),
    );
    job2.depends_on_job_ids = Some(vec![job1_id]);
    let created_job2 = default_api::create_job(config, job2).expect("Failed to create job2");
    let job2_id = created_job2.id.unwrap();

    // Chain 2: job3 -> job4 (independent)
    let job3 = models::JobModel::new(
        workflow_id,
        "job3".to_string(),
        "echo 'job3 success'".to_string(),
    );
    let created_job3 = default_api::create_job(config, job3).expect("Failed to create job3");
    let job3_id = created_job3.id.unwrap();

    let mut job4 = models::JobModel::new(
        workflow_id,
        "job4".to_string(),
        "echo 'job4 success'".to_string(),
    );
    job4.depends_on_job_ids = Some(vec![job3_id]);
    let created_job4 = default_api::create_job(config, job4).expect("Failed to create job4");
    let job4_id = created_job4.id.unwrap();

    // Initialize the workflow
    let torc_config = TorcConfig::load().unwrap_or_default();
    let manager = WorkflowManager::new(config.clone(), torc_config, workflow);
    manager.initialize(true).expect("Failed to start workflow");

    // Create a compute node for the results
    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Complete all jobs - job1 fails, others succeed
    // Note: status must match return_code - non-zero return_code requires Failed status
    let jobs_and_results: Vec<(i64, i64, JobStatus)> = vec![
        (job1_id, 1, JobStatus::Failed),    // job1 fails
        (job2_id, 0, JobStatus::Completed), // job2 succeeds
        (job3_id, 0, JobStatus::Completed), // job3 succeeds
        (job4_id, 0, JobStatus::Completed), // job4 succeeds
    ];

    for (job_id, return_code, status) in jobs_and_results {
        let result = models::ResultModel::new(
            job_id,
            workflow_id,
            1, // run_id
            1, // attempt_id
            compute_node_id,
            return_code,
            1.0,
            chrono::Utc::now().to_rfc3339(),
            status,
        );

        default_api::complete_job(config, job_id, result.status, 1, result)
            .unwrap_or_else(|_| panic!("Failed to complete job {}", job_id));
    }

    // Reset failed jobs only
    default_api::reset_job_status(config, workflow_id, Some(true), None)
        .expect("Failed to reset job status");

    // Verify that only Chain 1 jobs are reset to Uninitialized
    let job1_final = default_api::get_job(config, job1_id).expect("Failed to get job1");
    let job2_final = default_api::get_job(config, job2_id).expect("Failed to get job2");
    assert_eq!(
        job1_final.status.unwrap(),
        JobStatus::Uninitialized,
        "job1 should be Uninitialized (it failed)"
    );
    assert_eq!(
        job2_final.status.unwrap(),
        JobStatus::Uninitialized,
        "job2 should be Uninitialized (downstream of failed job1)"
    );

    // Verify that Chain 2 jobs remain Completed (they were successful and independent)
    let job3_final = default_api::get_job(config, job3_id).expect("Failed to get job3");
    let job4_final = default_api::get_job(config, job4_id).expect("Failed to get job4");
    assert_eq!(
        job3_final.status.unwrap(),
        JobStatus::Completed,
        "job3 should remain Completed (independent successful job)"
    );
    assert_eq!(
        job4_final.status.unwrap(),
        JobStatus::Completed,
        "job4 should remain Completed (downstream of successful job3)"
    );
}
