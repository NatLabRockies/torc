mod common;

use common::{
    ServerProcess, create_test_compute_node, create_test_workflow, get_exe_path, run_cli_with_json,
    start_server,
};
use rstest::rstest;
use torc::client::apis;
use torc::client::workflow_manager::WorkflowManager;
use torc::config::TorcConfig;
use torc::models;
use torc::models::JobStatus;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Initialize a freshly-created workflow and return its run_id.
fn initialize_workflow(
    config: &torc::client::Configuration,
    workflow: &models::WorkflowModel,
) -> i64 {
    let workflow_id = workflow.id.unwrap();
    let torc_config = TorcConfig::load().unwrap_or_default();
    let manager = WorkflowManager::new(config.clone(), torc_config, workflow.clone());
    manager
        .initialize(false)
        .expect("Failed to initialize workflow");
    apis::workflows_api::get_workflow(config, workflow_id)
        .expect("Failed to get workflow after init")
        .run_id
        .unwrap_or(1)
}

/// Complete a job via complete_job (never via manage_status_change per CLAUDE.md).
fn complete_job(
    config: &torc::client::Configuration,
    job_id: i64,
    workflow_id: i64,
    compute_node_id: i64,
    run_id: i64,
    return_code: i64,
    status: JobStatus,
) {
    let result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        return_code,
        1.0, // exec_time_minutes
        chrono::Utc::now().to_rfc3339(),
        status,
    );
    apis::jobs_api::complete_job(config, job_id, status, run_id, result)
        .unwrap_or_else(|e| panic!("Failed to complete job {}: {}", job_id, e));
}

/// Poll until `job_id` reaches `expected`, tolerating the asynchronous unblocking
/// task that flips a dependent from blocked -> ready after its predecessor
/// completes. Without this, driving the dependent straight to `running` can race
/// the unblocker and fail the server's optimistic-concurrency check with a 422.
fn wait_for_status(config: &torc::client::Configuration, job_id: i64, expected: JobStatus) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let status = apis::jobs_api::get_job(config, job_id)
            .expect("Failed to get job while waiting for status")
            .status
            .unwrap();
        if status == expected {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "job_id={job_id} did not reach {expected:?} within timeout (last status {status:?})"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

// ---------------------------------------------------------------------------
// Test 1: Selective reset — only targeted jobs are reset, others unchanged
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_selective(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_selective");
    let workflow_id = workflow.id.unwrap();

    // Create 3 independent jobs
    let job1 = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "job1".to_string(), "echo job1".to_string()),
    )
    .expect("Failed to create job1");
    let job1_id = job1.id.unwrap();

    let job2 = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "job2".to_string(), "echo job2".to_string()),
    )
    .expect("Failed to create job2");
    let job2_id = job2.id.unwrap();

    let job3 = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "job3".to_string(), "echo job3".to_string()),
    )
    .expect("Failed to create job3");
    let job3_id = job3.id.unwrap();

    let run_id = initialize_workflow(config, &workflow);
    let compute_node = create_test_compute_node(config, workflow_id);
    let cn_id = compute_node.id.unwrap();

    // Advance all to running then fail them all
    for &id in &[job1_id, job2_id, job3_id] {
        apis::jobs_api::manage_status_change(config, id, JobStatus::Running, run_id)
            .unwrap_or_else(|e| panic!("Failed to set job {} running: {}", id, e));
    }
    complete_job(
        config,
        job1_id,
        workflow_id,
        cn_id,
        run_id,
        1,
        JobStatus::Failed,
    );
    complete_job(
        config,
        job2_id,
        workflow_id,
        cn_id,
        run_id,
        1,
        JobStatus::Failed,
    );
    complete_job(
        config,
        job3_id,
        workflow_id,
        cn_id,
        run_id,
        1,
        JobStatus::Failed,
    );

    // Reset only job1 and job2
    let json = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            &job1_id.to_string(),
            &job2_id.to_string(),
        ],
        start_server,
        None,
    )
    .expect("reset-status should succeed");

    assert_eq!(json["status"], "success");
    assert_eq!(json["workflow_id"], workflow_id);

    // job1 and job2 must be Uninitialized; job3 must remain Failed
    assert_eq!(
        apis::jobs_api::get_job(config, job1_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Uninitialized,
        "job1 should be Uninitialized"
    );
    assert_eq!(
        apis::jobs_api::get_job(config, job2_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Uninitialized,
        "job2 should be Uninitialized"
    );
    assert_eq!(
        apis::jobs_api::get_job(config, job3_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Failed,
        "job3 should remain Failed"
    );

    // Workflow run_id must be unchanged (only reinit bumps it)
    let wf_after = apis::workflows_api::get_workflow(config, workflow_id).unwrap();
    assert_eq!(
        wf_after.run_id.unwrap_or(0),
        run_id,
        "run_id must not change on reset-status"
    );

    // reinit object must be present with requested=false when flag is absent
    assert_eq!(
        json["reinit"]["requested"], false,
        "reinit.requested should be false"
    );
    assert_eq!(
        json["reinit"]["applied"], false,
        "reinit.applied should be false"
    );
    assert!(
        json["reinit"]["error"].is_null(),
        "reinit.error should be null"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Downstream closure via reinitialize — A→B→C all completed; reset A.
// The command resets ONLY A. Immediately after: A is uninitialized, B is
// uninitialized (the server's incidental one-level reset of direct
// Completed/Failed dependents on complete→uninitialized), C is STILL
// completed. The dry-run output lists B and C as downstream-affected. Then
// 'workflows reinit' resets C too via the server's recursive
// uninitialize_blocked_jobs CTE (observable afterwards as Blocked, since
// reinit recomputes blocked/ready in the same transaction).
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_recursive_downstream(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_downstream");
    let workflow_id = workflow.id.unwrap();

    // Create a linear chain: A → B → C
    let job_a = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "job_a".to_string(), "echo a".to_string()),
    )
    .expect("Failed to create job_a");
    let a_id = job_a.id.unwrap();

    let mut job_b_model =
        models::JobModel::new(workflow_id, "job_b".to_string(), "echo b".to_string());
    job_b_model.depends_on_job_ids = Some(vec![a_id]);
    job_b_model.cancel_on_blocking_job_failure = Some(false);
    let job_b = apis::jobs_api::create_job(config, job_b_model).expect("Failed to create job_b");
    let b_id = job_b.id.unwrap();

    let mut job_c_model =
        models::JobModel::new(workflow_id, "job_c".to_string(), "echo c".to_string());
    job_c_model.depends_on_job_ids = Some(vec![b_id]);
    job_c_model.cancel_on_blocking_job_failure = Some(false);
    let job_c = apis::jobs_api::create_job(config, job_c_model).expect("Failed to create job_c");
    let c_id = job_c.id.unwrap();

    let run_id = initialize_workflow(config, &workflow);
    let compute_node = create_test_compute_node(config, workflow_id);
    let cn_id = compute_node.id.unwrap();

    // Complete all three successfully (chain A→B→C all done)
    apis::jobs_api::manage_status_change(config, a_id, JobStatus::Running, run_id)
        .expect("set a running");
    complete_job(
        config,
        a_id,
        workflow_id,
        cn_id,
        run_id,
        0,
        JobStatus::Completed,
    );

    // After A completes, unblocking fires and B becomes ready
    // Advance B to running and complete it
    wait_for_status(config, b_id, JobStatus::Ready);
    apis::jobs_api::manage_status_change(config, b_id, JobStatus::Running, run_id)
        .expect("set b running");
    complete_job(
        config,
        b_id,
        workflow_id,
        cn_id,
        run_id,
        0,
        JobStatus::Completed,
    );

    // After B completes, C becomes ready
    wait_for_status(config, c_id, JobStatus::Ready);
    apis::jobs_api::manage_status_change(config, c_id, JobStatus::Running, run_id)
        .expect("set c running");
    complete_job(
        config,
        c_id,
        workflow_id,
        cn_id,
        run_id,
        0,
        JobStatus::Completed,
    );

    // Sanity: all completed
    assert_eq!(
        apis::jobs_api::get_job(config, a_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Completed
    );
    assert_eq!(
        apis::jobs_api::get_job(config, b_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Completed
    );
    assert_eq!(
        apis::jobs_api::get_job(config, c_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Completed
    );

    // Dry-run first: the display must list B and C as downstream-affected
    let dry = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--force",
            "--dry-run",
            &a_id.to_string(),
        ],
        start_server,
        None,
    )
    .expect("dry-run should succeed");
    assert_eq!(dry["dry_run"], true);
    let dry_requested: Vec<i64> = dry["jobs"]
        .as_array()
        .expect("jobs should be an array")
        .iter()
        .map(|j| j["job_id"].as_i64().unwrap())
        .collect();
    assert_eq!(dry_requested, vec![a_id], "only A should be requested");
    let dry_downstream: Vec<i64> = dry["downstream_jobs"]
        .as_array()
        .expect("downstream_jobs should be an array")
        .iter()
        .map(|j| j["job_id"].as_i64().unwrap())
        .collect();
    assert_eq!(
        dry_downstream,
        vec![b_id, c_id],
        "B and C should be listed as downstream-affected"
    );

    // Reset only A — the command must issue status changes for A only
    let json = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--force",
            &a_id.to_string(),
        ],
        start_server,
        None,
    )
    .expect("reset-status should succeed");

    assert_eq!(json["status"], "success");

    // Only A was explicitly reset; B and C appear as informational downstream
    let reset_ids: Vec<i64> = json["reset_job_ids"]
        .as_array()
        .expect("reset_job_ids should be an array")
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    assert_eq!(reset_ids, vec![a_id], "only A should have been reset");
    let downstream_ids: Vec<i64> = json["downstream_jobs"]
        .as_array()
        .expect("downstream_jobs should be an array")
        .iter()
        .map(|j| j["job_id"].as_i64().unwrap())
        .collect();
    assert_eq!(
        downstream_ids,
        vec![b_id, c_id],
        "B and C should be listed as downstream-affected"
    );

    // Intermediate state: A uninitialized; B uninitialized too (the server's
    // incidental one-level reset of direct Completed/Failed dependents on
    // complete→uninitialized); C STILL completed.
    assert_eq!(
        apis::jobs_api::get_job(config, a_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Uninitialized,
        "A should be Uninitialized"
    );
    assert_eq!(
        apis::jobs_api::get_job(config, b_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Uninitialized,
        "B should be Uninitialized (server's one-level reset of direct dependents)"
    );
    assert_eq!(
        apis::jobs_api::get_job(config, c_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Completed,
        "C should STILL be Completed until reinit"
    );

    // Reinitialize: the server's recursive uninitialize_blocked_jobs CTE resets
    // C within the same transaction, then blocked/ready are recomputed —
    // A (no deps) becomes Ready, B and C become Blocked. C is no longer Completed.
    let torc_config = TorcConfig::load().unwrap_or_default();
    let updated_wf = apis::workflows_api::get_workflow(config, workflow_id).unwrap();
    let manager = WorkflowManager::new(config.clone(), torc_config, updated_wf);
    manager
        .reinitialize(false, false)
        .expect("reinitialize failed");

    assert_eq!(
        apis::jobs_api::get_job(config, a_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Ready,
        "A should be Ready after reinit"
    );
    assert_eq!(
        apis::jobs_api::get_job(config, b_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Blocked,
        "B should be Blocked after reinit (depends on A)"
    );
    assert_eq!(
        apis::jobs_api::get_job(config, c_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Blocked,
        "C should be Blocked after reinit (recursive CTE reset it from Completed)"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Cross-workflow rejection — IDs from two workflows → hard error,
//         nothing reset in either workflow
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_cross_workflow_rejected(start_server: &ServerProcess) {
    let config = &start_server.config;

    let wf1 = create_test_workflow(config, "reset_xwf1");
    let wf2 = create_test_workflow(config, "reset_xwf2");
    let wf1_id = wf1.id.unwrap();
    let wf2_id = wf2.id.unwrap();

    let job_in_wf1 = apis::jobs_api::create_job(
        config,
        models::JobModel::new(wf1_id, "j1".to_string(), "echo 1".to_string()),
    )
    .expect("create j1");
    let j1_id = job_in_wf1.id.unwrap();

    let job_in_wf2 = apis::jobs_api::create_job(
        config,
        models::JobModel::new(wf2_id, "j2".to_string(), "echo 2".to_string()),
    )
    .expect("create j2");
    let j2_id = job_in_wf2.id.unwrap();

    initialize_workflow(config, &wf1);
    initialize_workflow(config, &wf2);

    // Attempt to reset jobs from two different workflows — must fail
    let result = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--force",
            &j1_id.to_string(),
            &j2_id.to_string(),
        ],
        start_server,
        None,
    );
    assert!(result.is_err(), "Should fail for cross-workflow IDs");

    // Neither job should have been touched
    assert_eq!(
        apis::jobs_api::get_job(config, j1_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Ready,
        "j1 should still be Ready (untouched)"
    );
    assert_eq!(
        apis::jobs_api::get_job(config, j2_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Ready,
        "j2 should still be Ready (untouched)"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Unknown ID → clear error, nothing reset
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_unknown_id(start_server: &ServerProcess) {
    let result = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--force",
            "999999999",
        ],
        start_server,
        None,
    );
    assert!(result.is_err(), "Should fail for unknown job ID");
}

// ---------------------------------------------------------------------------
// Test 5: Active-job guard — Running job rejected without --force,
//         proceeds with --force
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_active_job_guard(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_active_guard");
    let workflow_id = workflow.id.unwrap();

    let job = apis::jobs_api::create_job(
        config,
        models::JobModel::new(
            workflow_id,
            "active_job".to_string(),
            "sleep 60".to_string(),
        ),
    )
    .expect("create job");
    let job_id = job.id.unwrap();

    let run_id = initialize_workflow(config, &workflow);

    // Set job to Running (using manage_status_change — legitimate for advancing to Running)
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id)
        .expect("set running");

    // Without --force: should be rejected because of Running status
    let result_no_force = run_cli_with_json(
        &["jobs", "reset-status", "--no-prompts", &job_id.to_string()],
        start_server,
        None,
    );
    assert!(
        result_no_force.is_err(),
        "Should fail for Running job without --force"
    );
    // Job must still be Running
    assert_eq!(
        apis::jobs_api::get_job(config, job_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Running,
        "Job should still be Running after rejected reset"
    );

    // With --force: should succeed
    let json = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--force",
            &job_id.to_string(),
        ],
        start_server,
        None,
    )
    .expect("reset-status with --force should succeed");

    assert_eq!(json["status"], "success");
    assert_eq!(
        apis::jobs_api::get_job(config, job_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Uninitialized,
        "Job should be Uninitialized after forced reset"
    );
}

// ---------------------------------------------------------------------------
// Test 6: Dry-run — no status changes occur, output lists jobs
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_dry_run(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_dry_run");
    let workflow_id = workflow.id.unwrap();

    let job = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "dry_job".to_string(), "echo dry".to_string()),
    )
    .expect("create job");
    let job_id = job.id.unwrap();

    let run_id = initialize_workflow(config, &workflow);
    let compute_node = create_test_compute_node(config, workflow_id);
    let cn_id = compute_node.id.unwrap();

    // Complete the job
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id)
        .expect("set running");
    complete_job(
        config,
        job_id,
        workflow_id,
        cn_id,
        run_id,
        1,
        JobStatus::Failed,
    );

    // Dry-run
    let json = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--force",
            "--dry-run",
            &job_id.to_string(),
        ],
        start_server,
        None,
    )
    .expect("dry-run should succeed");

    assert_eq!(json["dry_run"], true);
    assert_eq!(json["workflow_id"], workflow_id);
    let jobs_arr = json["jobs"].as_array().expect("should have jobs array");
    assert!(!jobs_arr.is_empty(), "dry-run should list jobs to reset");

    // Status must be unchanged (still Failed)
    assert_eq!(
        apis::jobs_api::get_job(config, job_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Failed,
        "Job should still be Failed after dry-run"
    );
}

// ---------------------------------------------------------------------------
// Test 7: End-to-end flow — reset subset → reinit → reset jobs become
//         ready/blocked, run_id bumped exactly once
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_end_to_end_reinit(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_e2e_reinit");
    let workflow_id = workflow.id.unwrap();

    // A → B (B depends on A)
    let job_a = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "e2e_a".to_string(), "echo a".to_string()),
    )
    .expect("create a");
    let a_id = job_a.id.unwrap();

    let mut job_b_model =
        models::JobModel::new(workflow_id, "e2e_b".to_string(), "echo b".to_string());
    job_b_model.depends_on_job_ids = Some(vec![a_id]);
    let job_b = apis::jobs_api::create_job(config, job_b_model).expect("create b");
    let b_id = job_b.id.unwrap();

    let run_id_before = initialize_workflow(config, &workflow);
    let compute_node = create_test_compute_node(config, workflow_id);
    let cn_id = compute_node.id.unwrap();

    // Complete A, then complete B
    apis::jobs_api::manage_status_change(config, a_id, JobStatus::Running, run_id_before)
        .expect("set a running");
    complete_job(
        config,
        a_id,
        workflow_id,
        cn_id,
        run_id_before,
        0,
        JobStatus::Completed,
    );

    wait_for_status(config, b_id, JobStatus::Ready);
    apis::jobs_api::manage_status_change(config, b_id, JobStatus::Running, run_id_before)
        .expect("set b running");
    complete_job(
        config,
        b_id,
        workflow_id,
        cn_id,
        run_id_before,
        1,
        JobStatus::Failed,
    );

    // Reset only B (failed) — A is also completed and is upstream, but NOT in the closure
    // because we only requested B, and A is not downstream of B
    let json = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--force",
            &b_id.to_string(),
        ],
        start_server,
        None,
    )
    .expect("reset-status should succeed");

    assert_eq!(json["status"], "success");

    // B should be Uninitialized; A remains Completed
    assert_eq!(
        apis::jobs_api::get_job(config, b_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Uninitialized,
        "B should be Uninitialized after reset"
    );
    assert_eq!(
        apis::jobs_api::get_job(config, a_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Completed,
        "A should remain Completed"
    );

    // Now reinitialize — this bumps run_id exactly once
    let torc_config = TorcConfig::load().unwrap_or_default();
    let updated_wf = apis::workflows_api::get_workflow(config, workflow_id).unwrap();
    let manager = WorkflowManager::new(config.clone(), torc_config, updated_wf);
    manager
        .reinitialize(false, false)
        .expect("reinitialize failed");

    let run_id_after = apis::workflows_api::get_workflow(config, workflow_id)
        .unwrap()
        .run_id
        .unwrap_or(0);
    assert_eq!(
        run_id_after,
        run_id_before + 1,
        "run_id should be bumped exactly once by reinitialize"
    );

    // After reinit, B (which depends on A=Completed) should be Ready
    let b_after = apis::jobs_api::get_job(config, b_id).unwrap();
    assert_eq!(
        b_after.status.unwrap(),
        JobStatus::Ready,
        "B should be Ready after reinitialize (A is still Completed)"
    );
}

// ---------------------------------------------------------------------------
// Test 8: Idempotency — resetting an already-uninitialized job succeeds as
//         a no-op (no error, no status change)
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_idempotent(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_idempotent");
    let workflow_id = workflow.id.unwrap();

    // Job is in Uninitialized state (before initialize_jobs)
    let job = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "uninit_job".to_string(), "echo x".to_string()),
    )
    .expect("create job");
    let job_id = job.id.unwrap();

    // Do NOT initialize — job stays Uninitialized
    assert_eq!(
        apis::jobs_api::get_job(config, job_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Uninitialized,
        "Job should start Uninitialized"
    );

    // Reset an already-uninitialized job — should succeed (warns but no error)
    let json = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--force",
            &job_id.to_string(),
        ],
        start_server,
        None,
    )
    .expect("resetting uninitialized job should succeed as no-op");

    assert_eq!(json["status"], "success");
    assert_eq!(
        apis::jobs_api::get_job(config, job_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Uninitialized,
        "Job should remain Uninitialized"
    );
}

// ---------------------------------------------------------------------------
// Test 9: --reinit flag end-to-end — reset + reinit in one invocation.
//         A → B (B depends on A). Both completed. Reset B with --reinit.
//         Assert: B uninitialized then ready after reinit, A unchanged,
//         run_id bumped by exactly 1.
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_reinit_flag_end_to_end(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_reinit_e2e");
    let workflow_id = workflow.id.unwrap();

    let job_a = apis::jobs_api::create_job(
        config,
        models::JobModel::new(
            workflow_id,
            "reinit_e2e_a".to_string(),
            "echo a".to_string(),
        ),
    )
    .expect("create a");
    let a_id = job_a.id.unwrap();

    let mut job_b_model = models::JobModel::new(
        workflow_id,
        "reinit_e2e_b".to_string(),
        "echo b".to_string(),
    );
    job_b_model.depends_on_job_ids = Some(vec![a_id]);
    let job_b = apis::jobs_api::create_job(config, job_b_model).expect("create b");
    let b_id = job_b.id.unwrap();

    let run_id_before = initialize_workflow(config, &workflow);
    let compute_node = create_test_compute_node(config, workflow_id);
    let cn_id = compute_node.id.unwrap();

    // Complete A successfully, complete B with failure
    apis::jobs_api::manage_status_change(config, a_id, JobStatus::Running, run_id_before)
        .expect("set a running");
    complete_job(
        config,
        a_id,
        workflow_id,
        cn_id,
        run_id_before,
        0,
        JobStatus::Completed,
    );
    wait_for_status(config, b_id, JobStatus::Ready);
    apis::jobs_api::manage_status_change(config, b_id, JobStatus::Running, run_id_before)
        .expect("set b running");
    complete_job(
        config,
        b_id,
        workflow_id,
        cn_id,
        run_id_before,
        1,
        JobStatus::Failed,
    );

    // Reset B with --reinit in one step (no separate reinit needed)
    let json = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--force",
            "--reinit",
            &b_id.to_string(),
        ],
        start_server,
        None,
    )
    .expect("reset-status --reinit should succeed");

    assert_eq!(json["status"], "success", "status should be success");
    assert_eq!(json["reinit"]["requested"], true);
    assert_eq!(json["reinit"]["applied"], true);
    assert!(json["reinit"]["error"].is_null());

    // run_id bumped exactly once
    let run_id_after = apis::workflows_api::get_workflow(config, workflow_id)
        .unwrap()
        .run_id
        .unwrap_or(0);
    assert_eq!(
        run_id_after,
        run_id_before + 1,
        "run_id should be bumped exactly once by --reinit"
    );

    // B should be Ready (A is Completed, so B's dependency is satisfied)
    let b_after = apis::jobs_api::get_job(config, b_id).unwrap();
    assert_eq!(
        b_after.status.unwrap(),
        JobStatus::Ready,
        "B should be Ready after --reinit"
    );

    // A (upstream, completed) must NOT be reset
    let a_after = apis::jobs_api::get_job(config, a_id).unwrap();
    assert_eq!(
        a_after.status.unwrap(),
        JobStatus::Completed,
        "A should remain Completed"
    );

    // next_steps should not mention 'workflows reinit'
    let next_steps = json["next_steps"].as_str().unwrap_or("");
    assert!(
        !next_steps.contains("workflows reinit"),
        "next_steps should not mention 'workflows reinit' when reinit was applied: {}",
        next_steps
    );
}

// ---------------------------------------------------------------------------
// Test 10: --reinit JSON output shape
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_reinit_json_output(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_reinit_json");
    let workflow_id = workflow.id.unwrap();

    let job = apis::jobs_api::create_job(
        config,
        models::JobModel::new(
            workflow_id,
            "reinit_json_job".to_string(),
            "echo hi".to_string(),
        ),
    )
    .expect("create job");
    let job_id = job.id.unwrap();

    let run_id_before = initialize_workflow(config, &workflow);
    let compute_node = create_test_compute_node(config, workflow_id);
    let cn_id = compute_node.id.unwrap();

    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id_before)
        .expect("set running");
    complete_job(
        config,
        job_id,
        workflow_id,
        cn_id,
        run_id_before,
        1,
        JobStatus::Failed,
    );

    let json = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--force",
            "--reinit",
            &job_id.to_string(),
        ],
        start_server,
        None,
    )
    .expect("reset-status --reinit --no-prompts should succeed");

    assert_eq!(json["status"], "success");
    assert_eq!(json["reinit"]["requested"], true);
    assert_eq!(json["reinit"]["applied"], true);
    assert!(json["reinit"]["error"].is_null());

    // next_steps should not mention 'workflows reinit' since it was applied
    let next_steps = json["next_steps"].as_str().unwrap_or("");
    assert!(
        !next_steps.contains("workflows reinit"),
        "next_steps should not reference 'workflows reinit' when reinit was applied: {}",
        next_steps
    );
}

// ---------------------------------------------------------------------------
// Test 11: --dry-run --reinit — no changes applied, reinit_requested flag set
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_reinit_dry_run(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_reinit_dry");
    let workflow_id = workflow.id.unwrap();

    let job = apis::jobs_api::create_job(
        config,
        models::JobModel::new(
            workflow_id,
            "reinit_dry_job".to_string(),
            "echo dry".to_string(),
        ),
    )
    .expect("create job");
    let job_id = job.id.unwrap();

    let run_id_before = initialize_workflow(config, &workflow);
    let compute_node = create_test_compute_node(config, workflow_id);
    let cn_id = compute_node.id.unwrap();

    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id_before)
        .expect("set running");
    complete_job(
        config,
        job_id,
        workflow_id,
        cn_id,
        run_id_before,
        1,
        JobStatus::Failed,
    );

    let json = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--force",
            "--dry-run",
            "--reinit",
            &job_id.to_string(),
        ],
        start_server,
        None,
    )
    .expect("dry-run --reinit should succeed");

    assert_eq!(json["dry_run"], true);
    assert_eq!(json["reinit_requested"], true);

    // Job status must be unchanged (dry-run made no changes)
    assert_eq!(
        apis::jobs_api::get_job(config, job_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Failed,
        "Job should still be Failed after dry-run"
    );

    // run_id must be unchanged (no reinit happened)
    let run_id_after = apis::workflows_api::get_workflow(config, workflow_id)
        .unwrap()
        .run_id
        .unwrap_or(0);
    assert_eq!(
        run_id_after, run_id_before,
        "run_id must not change on dry-run"
    );
}

// ---------------------------------------------------------------------------
// Test 12: Re-runnable without --force — a first reset leaves the workflow
//          incomplete (uninitialized jobs), but a second invocation must still
//          succeed because the command only requires no active workers, not
//          workflow completeness.
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_rerunnable_without_force(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_rerunnable");
    let workflow_id = workflow.id.unwrap();

    let job1 = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "job1".to_string(), "echo job1".to_string()),
    )
    .expect("create job1");
    let job1_id = job1.id.unwrap();
    let job2 = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "job2".to_string(), "echo job2".to_string()),
    )
    .expect("create job2");
    let job2_id = job2.id.unwrap();

    let run_id = initialize_workflow(config, &workflow);
    let compute_node = create_test_compute_node(config, workflow_id);
    let cn_id = compute_node.id.unwrap();

    for &id in &[job1_id, job2_id] {
        apis::jobs_api::manage_status_change(config, id, JobStatus::Running, run_id)
            .unwrap_or_else(|e| panic!("Failed to set job {} running: {}", id, e));
        complete_job(config, id, workflow_id, cn_id, run_id, 1, JobStatus::Failed);
    }

    // First reset (workflow is complete) — no --force needed
    let json = run_cli_with_json(
        &["jobs", "reset-status", "--no-prompts", &job1_id.to_string()],
        start_server,
        None,
    )
    .expect("first reset-status should succeed");
    assert_eq!(json["status"], "success");

    // job1 is now uninitialized, so the workflow is no longer complete.
    // A second invocation must STILL succeed without --force.
    let json = run_cli_with_json(
        &["jobs", "reset-status", "--no-prompts", &job2_id.to_string()],
        start_server,
        None,
    )
    .expect("second reset-status should succeed even though workflow is incomplete");
    assert_eq!(json["status"], "success");

    for &id in &[job1_id, job2_id] {
        assert_eq!(
            apis::jobs_api::get_job(config, id).unwrap().status.unwrap(),
            JobStatus::Uninitialized,
            "job {} should be Uninitialized",
            id
        );
    }
}

// ---------------------------------------------------------------------------
// Test 13: Warning for successfully completed jobs — resetting a Completed job
//          succeeds but prints a stderr warning; a Failed job in the same
//          invocation is not listed in the warning.
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_completed_job_warning(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_completed_warning");
    let workflow_id = workflow.id.unwrap();

    let completed_job = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "ok_job".to_string(), "echo ok".to_string()),
    )
    .expect("create completed job");
    let completed_id = completed_job.id.unwrap();
    let failed_job = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "bad_job".to_string(), "false".to_string()),
    )
    .expect("create failed job");
    let failed_id = failed_job.id.unwrap();

    let run_id = initialize_workflow(config, &workflow);
    let compute_node = create_test_compute_node(config, workflow_id);
    let cn_id = compute_node.id.unwrap();

    for &id in &[completed_id, failed_id] {
        apis::jobs_api::manage_status_change(config, id, JobStatus::Running, run_id)
            .unwrap_or_else(|e| panic!("Failed to set job {} running: {}", id, e));
    }
    complete_job(
        config,
        completed_id,
        workflow_id,
        cn_id,
        run_id,
        0,
        JobStatus::Completed,
    );
    complete_job(
        config,
        failed_id,
        workflow_id,
        cn_id,
        run_id,
        1,
        JobStatus::Failed,
    );

    // Run the CLI directly so we can capture stderr alongside stdout
    let output = std::process::Command::new(get_exe_path("./target/debug/torc"))
        .args([
            "--format",
            "json",
            "jobs",
            "reset-status",
            "--no-prompts",
            &completed_id.to_string(),
            &failed_id.to_string(),
        ])
        .env("TORC_API_URL", &start_server.config.base_path)
        .output()
        .expect("run torc CLI");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "reset-status should succeed; stderr: {}",
        stderr
    );
    assert!(
        stderr.contains("completed successfully"),
        "stderr should warn about completed jobs; stderr: {}",
        stderr
    );
    assert!(
        stderr.contains(&format!("job {} (ok_job)", completed_id)),
        "warning should list the completed job; stderr: {}",
        stderr
    );
    assert!(
        !stderr.contains(&format!("job {} (bad_job)", failed_id)),
        "warning should not list the failed job; stderr: {}",
        stderr
    );

    // Both jobs were still reset
    for &id in &[completed_id, failed_id] {
        assert_eq!(
            apis::jobs_api::get_job(config, id).unwrap().status.unwrap(),
            JobStatus::Uninitialized,
            "job {} should be Uninitialized",
            id
        );
    }
}

// ---------------------------------------------------------------------------
// Test 14: Select by --status — reset every job in a given status, leaving
//          jobs in other statuses untouched.
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_by_status_filter(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_by_status");
    let workflow_id = workflow.id.unwrap();

    let failed1 = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "failed1".to_string(), "false".to_string()),
    )
    .expect("create failed1");
    let failed1_id = failed1.id.unwrap();
    let failed2 = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "failed2".to_string(), "false".to_string()),
    )
    .expect("create failed2");
    let failed2_id = failed2.id.unwrap();
    let completed = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "ok".to_string(), "echo ok".to_string()),
    )
    .expect("create completed");
    let completed_id = completed.id.unwrap();

    let run_id = initialize_workflow(config, &workflow);
    let cn_id = create_test_compute_node(config, workflow_id).id.unwrap();

    for &id in &[failed1_id, failed2_id, completed_id] {
        apis::jobs_api::manage_status_change(config, id, JobStatus::Running, run_id)
            .unwrap_or_else(|e| panic!("set {} running: {}", id, e));
    }
    complete_job(
        config,
        failed1_id,
        workflow_id,
        cn_id,
        run_id,
        1,
        JobStatus::Failed,
    );
    complete_job(
        config,
        failed2_id,
        workflow_id,
        cn_id,
        run_id,
        1,
        JobStatus::Failed,
    );
    complete_job(
        config,
        completed_id,
        workflow_id,
        cn_id,
        run_id,
        0,
        JobStatus::Completed,
    );

    let json = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--status",
            "failed",
            "--workflow-id",
            &workflow_id.to_string(),
        ],
        start_server,
        None,
    )
    .expect("reset-status --status should succeed");

    assert_eq!(json["status"], "success");
    assert_eq!(json["workflow_id"], workflow_id);
    let reset_ids: Vec<i64> = json["reset_job_ids"]
        .as_array()
        .expect("reset_job_ids array")
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    assert_eq!(reset_ids.len(), 2, "exactly the two failed jobs reset");
    assert!(reset_ids.contains(&failed1_id) && reset_ids.contains(&failed2_id));

    for &id in &[failed1_id, failed2_id] {
        assert_eq!(
            apis::jobs_api::get_job(config, id).unwrap().status.unwrap(),
            JobStatus::Uninitialized,
            "failed job {} should be Uninitialized",
            id
        );
    }
    assert_eq!(
        apis::jobs_api::get_job(config, completed_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Completed,
        "completed job should be untouched"
    );
}

// ---------------------------------------------------------------------------
// Test 15: Select by multiple --status values (comma-separated) — union of
//          all matching statuses is reset.
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_by_multiple_statuses(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_by_multi_status");
    let workflow_id = workflow.id.unwrap();

    let failed = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "f".to_string(), "false".to_string()),
    )
    .expect("create failed");
    let failed_id = failed.id.unwrap();
    let terminated = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "t".to_string(), "sleep 1".to_string()),
    )
    .expect("create terminated");
    let terminated_id = terminated.id.unwrap();
    let completed = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "c".to_string(), "echo c".to_string()),
    )
    .expect("create completed");
    let completed_id = completed.id.unwrap();

    let run_id = initialize_workflow(config, &workflow);
    let cn_id = create_test_compute_node(config, workflow_id).id.unwrap();

    for &id in &[failed_id, terminated_id, completed_id] {
        apis::jobs_api::manage_status_change(config, id, JobStatus::Running, run_id)
            .unwrap_or_else(|e| panic!("set {} running: {}", id, e));
    }
    complete_job(
        config,
        failed_id,
        workflow_id,
        cn_id,
        run_id,
        1,
        JobStatus::Failed,
    );
    complete_job(
        config,
        terminated_id,
        workflow_id,
        cn_id,
        run_id,
        1,
        JobStatus::Terminated,
    );
    complete_job(
        config,
        completed_id,
        workflow_id,
        cn_id,
        run_id,
        0,
        JobStatus::Completed,
    );

    let json = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--status",
            "failed,terminated",
            "--workflow-id",
            &workflow_id.to_string(),
        ],
        start_server,
        None,
    )
    .expect("reset-status --status failed,terminated should succeed");

    assert_eq!(json["status"], "success");
    let reset_ids: Vec<i64> = json["reset_job_ids"]
        .as_array()
        .expect("reset_job_ids array")
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    assert_eq!(reset_ids.len(), 2);
    assert!(reset_ids.contains(&failed_id) && reset_ids.contains(&terminated_id));
    assert_eq!(
        apis::jobs_api::get_job(config, completed_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Completed,
        "completed job should be untouched"
    );
}

// ---------------------------------------------------------------------------
// Test 16: Select by --return-code — reset jobs whose latest result has the
//          given return code.
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_by_return_code(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_by_return_code");
    let workflow_id = workflow.id.unwrap();

    let rc42_a = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "rc42a".to_string(), "false".to_string()),
    )
    .expect("create rc42a");
    let rc42_a_id = rc42_a.id.unwrap();
    let rc42_b = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "rc42b".to_string(), "false".to_string()),
    )
    .expect("create rc42b");
    let rc42_b_id = rc42_b.id.unwrap();
    let rc0 = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "rc0".to_string(), "echo ok".to_string()),
    )
    .expect("create rc0");
    let rc0_id = rc0.id.unwrap();

    let run_id = initialize_workflow(config, &workflow);
    let cn_id = create_test_compute_node(config, workflow_id).id.unwrap();

    for &id in &[rc42_a_id, rc42_b_id, rc0_id] {
        apis::jobs_api::manage_status_change(config, id, JobStatus::Running, run_id)
            .unwrap_or_else(|e| panic!("set {} running: {}", id, e));
    }
    complete_job(
        config,
        rc42_a_id,
        workflow_id,
        cn_id,
        run_id,
        42,
        JobStatus::Failed,
    );
    complete_job(
        config,
        rc42_b_id,
        workflow_id,
        cn_id,
        run_id,
        42,
        JobStatus::Failed,
    );
    complete_job(
        config,
        rc0_id,
        workflow_id,
        cn_id,
        run_id,
        0,
        JobStatus::Completed,
    );

    let json = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--return-code",
            "42",
            "--workflow-id",
            &workflow_id.to_string(),
        ],
        start_server,
        None,
    )
    .expect("reset-status --return-code should succeed");

    assert_eq!(json["status"], "success");
    let reset_ids: Vec<i64> = json["reset_job_ids"]
        .as_array()
        .expect("reset_job_ids array")
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    assert_eq!(reset_ids.len(), 2, "both return-code-42 jobs reset");
    assert!(reset_ids.contains(&rc42_a_id) && reset_ids.contains(&rc42_b_id));
    for &id in &[rc42_a_id, rc42_b_id] {
        assert_eq!(
            apis::jobs_api::get_job(config, id).unwrap().status.unwrap(),
            JobStatus::Uninitialized,
            "rc42 job {} should be Uninitialized",
            id
        );
    }
    assert_eq!(
        apis::jobs_api::get_job(config, rc0_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Completed,
        "return-code-0 job should be untouched"
    );
}

// ---------------------------------------------------------------------------
// Test 17: Filter modes that match nothing fail with a non-zero exit (the
//          caller expected matching jobs and none were found), and reset nothing.
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_filter_no_match(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_no_match");
    let workflow_id = workflow.id.unwrap();

    let job = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "j".to_string(), "echo j".to_string()),
    )
    .expect("create job");
    let job_id = job.id.unwrap();
    initialize_workflow(config, &workflow);

    let result = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--status",
            "terminated",
            "--workflow-id",
            &workflow_id.to_string(),
        ],
        start_server,
        None,
    );
    assert!(
        result.is_err(),
        "reset-status with no matches should fail with a non-zero exit"
    );

    // The non-matching job must be untouched.
    assert_eq!(
        apis::jobs_api::get_job(config, job_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Ready,
        "job should remain Ready (nothing reset)"
    );
}

// ---------------------------------------------------------------------------
// Test 18: Selection modes are mutually exclusive — combining job IDs with
//          --status is a usage error (enforced by clap).
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_modes_mutually_exclusive(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_mutex");
    let workflow_id = workflow.id.unwrap();

    let job = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "j".to_string(), "echo j".to_string()),
    )
    .expect("create job");
    let job_id = job.id.unwrap();
    initialize_workflow(config, &workflow);

    let result = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            &job_id.to_string(),
            "--status",
            "failed",
        ],
        start_server,
        None,
    );
    assert!(
        result.is_err(),
        "combining job IDs with --status should be rejected"
    );
}

// ---------------------------------------------------------------------------
// Test 19: Invalid --status value → clear error, nothing reset.
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_invalid_status_value(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_bad_status");
    let workflow_id = workflow.id.unwrap();
    initialize_workflow(config, &workflow);

    let result = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--status",
            "bogus",
            "--workflow-id",
            &workflow_id.to_string(),
        ],
        start_server,
        None,
    );
    assert!(result.is_err(), "invalid status should be rejected");
}

// ---------------------------------------------------------------------------
// Test 20: --return-code matches only the LATEST result per job. A job that
//          failed with rc=42 and was then rerun to success (rc=0) must NOT be
//          matched by `--return-code 42`, but MUST be matched by
//          `--return-code 0`.
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_return_code_uses_latest_result(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_rc_latest");
    let workflow_id = workflow.id.unwrap();

    let job = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "reran".to_string(), "echo x".to_string()),
    )
    .expect("create job");
    let job_id = job.id.unwrap();

    // Run 1: fail with return code 42.
    let run_id1 = initialize_workflow(config, &workflow);
    let cn_id = create_test_compute_node(config, workflow_id).id.unwrap();
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id1)
        .expect("set running (run 1)");
    complete_job(
        config,
        job_id,
        workflow_id,
        cn_id,
        run_id1,
        42,
        JobStatus::Failed,
    );

    // Reset the job and reinitialize to bump run_id, mimicking a real rerun.
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Uninitialized, run_id1)
        .expect("reset to uninitialized");
    let torc_config = TorcConfig::load().unwrap_or_default();
    let updated_wf = apis::workflows_api::get_workflow(config, workflow_id).unwrap();
    let manager = WorkflowManager::new(config.clone(), torc_config, updated_wf);
    manager.reinitialize(false, false).expect("reinitialize");
    let run_id2 = apis::workflows_api::get_workflow(config, workflow_id)
        .unwrap()
        .run_id
        .unwrap();
    assert_eq!(run_id2, run_id1 + 1, "run_id should be bumped once");

    // Run 2: succeed with return code 0 (now the latest result).
    wait_for_status(config, job_id, JobStatus::Ready);
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id2)
        .expect("set running (run 2)");
    complete_job(
        config,
        job_id,
        workflow_id,
        cn_id,
        run_id2,
        0,
        JobStatus::Completed,
    );

    // --return-code 42 must NOT match: the stale failure is not the latest
    // result. A filter that matches nothing exits non-zero.
    let result_42 = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--force",
            "--return-code",
            "42",
            "--workflow-id",
            &workflow_id.to_string(),
        ],
        start_server,
        None,
    );
    assert!(
        result_42.is_err(),
        "stale rc=42 from a previous run must not match (no-match should fail)"
    );
    assert_eq!(
        apis::jobs_api::get_job(config, job_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Completed,
        "job should be untouched by the non-matching return code"
    );

    // --return-code 0 must match the latest (successful) result.
    let json_0 = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--force",
            "--return-code",
            "0",
            "--workflow-id",
            &workflow_id.to_string(),
        ],
        start_server,
        None,
    )
    .expect("reset-status --return-code 0 should succeed");
    let reset_ids: Vec<i64> = json_0["reset_job_ids"]
        .as_array()
        .expect("reset_job_ids array")
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    assert_eq!(reset_ids, vec![job_id], "latest rc=0 should match");
    assert_eq!(
        apis::jobs_api::get_job(config, job_id)
            .unwrap()
            .status
            .unwrap(),
        JobStatus::Uninitialized,
        "job should be reset when its latest result matches"
    );
}

// ---------------------------------------------------------------------------
// Test 21: --status may be passed as repeated flags (not just comma-separated);
//          the union of matching jobs is reset.
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_repeated_status_flag(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_repeated_flag");
    let workflow_id = workflow.id.unwrap();

    let failed = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "f".to_string(), "false".to_string()),
    )
    .expect("create failed");
    let failed_id = failed.id.unwrap();
    let terminated = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "t".to_string(), "sleep 1".to_string()),
    )
    .expect("create terminated");
    let terminated_id = terminated.id.unwrap();

    let run_id = initialize_workflow(config, &workflow);
    let cn_id = create_test_compute_node(config, workflow_id).id.unwrap();
    for &id in &[failed_id, terminated_id] {
        apis::jobs_api::manage_status_change(config, id, JobStatus::Running, run_id)
            .unwrap_or_else(|e| panic!("set {} running: {}", id, e));
    }
    complete_job(
        config,
        failed_id,
        workflow_id,
        cn_id,
        run_id,
        1,
        JobStatus::Failed,
    );
    complete_job(
        config,
        terminated_id,
        workflow_id,
        cn_id,
        run_id,
        1,
        JobStatus::Terminated,
    );

    let json = run_cli_with_json(
        &[
            "jobs",
            "reset-status",
            "--no-prompts",
            "--status",
            "failed",
            "--status",
            "terminated",
            "--workflow-id",
            &workflow_id.to_string(),
        ],
        start_server,
        None,
    )
    .expect("repeated --status flags should succeed");

    assert_eq!(json["status"], "success");
    let reset_ids: Vec<i64> = json["reset_job_ids"]
        .as_array()
        .expect("reset_job_ids array")
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    assert_eq!(reset_ids.len(), 2);
    assert!(reset_ids.contains(&failed_id) && reset_ids.contains(&terminated_id));
}

// ---------------------------------------------------------------------------
// Test 22: Remaining mutually-exclusive selection-mode combinations are all
//          rejected by clap: --status + --return-code, job IDs + --return-code,
//          and job IDs + --workflow-id.
// ---------------------------------------------------------------------------
#[rstest]
fn test_reset_status_other_mutex_combinations(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "reset_other_mutex");
    let workflow_id = workflow.id.unwrap();
    let job = apis::jobs_api::create_job(
        config,
        models::JobModel::new(workflow_id, "j".to_string(), "echo j".to_string()),
    )
    .expect("create job");
    let job_id = job.id.unwrap();
    initialize_workflow(config, &workflow);
    let wf = workflow_id.to_string();
    let jid = job_id.to_string();

    let cases: &[Vec<&str>] = &[
        // --status + --return-code
        vec![
            "jobs",
            "reset-status",
            "--no-prompts",
            "--status",
            "failed",
            "--return-code",
            "1",
            "--workflow-id",
            &wf,
        ],
        // job IDs + --return-code
        vec![
            "jobs",
            "reset-status",
            "--no-prompts",
            &jid,
            "--return-code",
            "1",
        ],
        // job IDs + --workflow-id
        vec![
            "jobs",
            "reset-status",
            "--no-prompts",
            &jid,
            "--workflow-id",
            &wf,
        ],
    ];

    for args in cases {
        let result = run_cli_with_json(args, start_server, None);
        assert!(
            result.is_err(),
            "mutually exclusive combination should be rejected: {:?}",
            args
        );
    }
}
