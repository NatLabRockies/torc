mod common;

use common::{
    ServerProcess, create_diamond_workflow, run_cli_command, run_jobs_cli_command, start_server,
};
use rstest::rstest;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use torc::client::default_api;
use torc::models;

#[rstest]
#[case(None)] // Test with resource-based allocation
#[case(Some(2))] // Test with simple queue-based allocation (max 2 jobs at a time)
fn test_diamond_workflow(start_server: &ServerProcess, #[case] max_parallel_jobs: Option<i64>) {
    assert!(start_server.child.id() > 0);
    let config = &start_server.config;

    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path().to_path_buf();

    let jobs = create_diamond_workflow(config, false, &work_dir);
    let preprocess = jobs.get("preprocess").expect("preprocess job not found");
    let workflow_id = preprocess.workflow_id;
    create_input_file(&work_dir);

    // Build CLI arguments based on max_parallel_jobs parameter
    let mut cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.1".to_string(),
    ];

    if let Some(max_jobs) = max_parallel_jobs {
        // Use simple queue-based allocation with claim_next_jobs
        cli_args.push("--max-parallel-jobs".to_string());
        cli_args.push(max_jobs.to_string());
    } else {
        // Use resource-based allocation with claim_jobs_based_on_resources
        cli_args.push("--num-cpus".to_string());
        cli_args.push("4".to_string());
        cli_args.push("--memory-gb".to_string());
        cli_args.push("8.0".to_string());
    }

    // Convert Vec<String> to Vec<&str> for run_jobs_cli_command
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    run_jobs_cli_command(&cli_args_refs, start_server).expect("Failed to run jobs");

    verify_diamond_workflow_completion(config, workflow_id, &work_dir);

    let temp_dir2 = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir2 = temp_dir2.path();
    let jobs2 = create_diamond_workflow(config, true, work_dir2);
    check_diamond_workflow_init_job_statuses(config, &jobs2);

    default_api::delete_workflow(config, workflow_id, None).expect("Failed to delete workflow");
    for (name, job) in &jobs {
        let result = default_api::get_job(config, job.id.unwrap());
        assert!(
            result.is_err(),
            "Expected job {} to be deleted with workflow",
            name
        );
    }
    check_diamond_workflow_init_job_statuses(config, &jobs2);
}

fn create_input_file(work_dir: &Path) {
    let input_data = r#"{"data": "initial input", "value": 42}"#;
    fs::write(work_dir.join("f1.json"), input_data).expect("Failed to write f1.json");
}

fn verify_diamond_workflow_completion(
    config: &torc::client::Configuration,
    workflow_id: i64,
    work_dir: &Path,
) {
    let jobs = default_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None, // include_relationships
        None, // active_compute_node_id
    )
    .expect("Failed to list jobs");

    for job in jobs.items.unwrap() {
        assert_eq!(
            job.status.unwrap(),
            models::JobStatus::Completed,
            "Job {} should be completed. actual status: {:?}",
            job.name,
            job.status
        );
    }

    // Get results for all jobs in the workflow and verify return codes
    let results = default_api::list_results(
        config,
        workflow_id,
        None, // job_id - get results for all jobs
        None, // run_id
        None, // offset
        None, // limit
        None, // sort_by
        None, // reverse_sort
        None, // return_code filter
        None, // status filter
        None, // all_runs
        None, // compute_node_id
    )
    .expect("Failed to list results");

    let result_items = results.items.unwrap();

    for result in result_items {
        assert_eq!(
            result.return_code, 0,
            "Job ID {} should have return code 0, but got {}",
            result.job_id, result.return_code
        );
    }

    assert!(work_dir.join("f2.json").exists(), "f2.json should exist");
    assert!(work_dir.join("f3.json").exists(), "f3.json should exist");
    assert!(work_dir.join("f4.json").exists(), "f4.json should exist");
    assert!(work_dir.join("f5.json").exists(), "f5.json should exist");
    assert!(work_dir.join("f6.json").exists(), "f6.json should exist");

    let f6_content = fs::read_to_string(work_dir.join("f6.json")).expect("Failed to read f6.json");
    println!("Final output (f6.json): {}", f6_content);
}

#[rstest]
fn test_uninitialize_blocked_jobs(start_server: &ServerProcess) {
    assert!(start_server.child.id() > 0);
    let config = &start_server.config;
    let name = "test_workflow".to_string();
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new(name.clone(), user.clone());
    let created_workflow =
        default_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    let job1 = default_api::create_job(
        config,
        models::JobModel::new(
            workflow_id as i64,
            "job1".to_string(),
            "command".to_string(),
        ),
    )
    .expect("Failed to create job1");
    let mut job2_pre = models::JobModel::new(
        workflow_id as i64,
        "job2".to_string(),
        "command".to_string(),
    );
    job2_pre.depends_on_job_ids = Some(vec![job1.id.unwrap()]);
    let mut job2 = default_api::create_job(config, job2_pre).expect("Failed to create job2");
    // let job2_id = job2.id.unwrap();
    let mut bystander = default_api::create_job(
        config,
        models::JobModel::new(
            workflow_id as i64,
            "bystander".to_string(),
            "command".to_string(),
        ),
    )
    .expect("Failed to create bystander");
    // let bystander_id = bystander.id.unwrap();

    assert_eq!(job1.status, Some(models::JobStatus::Uninitialized));
    job2.status = Some(models::JobStatus::Completed);
    bystander.status = Some(models::JobStatus::Completed);
    // TODO: Is this providing value? Updating status like this is no longer allowed.
    // let job2b = default_api::update_job(config, job2_id, job2).expect("Failed to update job2");
    // assert_eq!(job2b.status, Some(models::JobStatus::Completed));
    // let bystander_b = default_api::update_job(config, bystander_id, bystander)
    //     .expect("Failed to update bystander");
    // assert_eq!(bystander_b.status, Some(models::JobStatus::Completed));

    // default_api::initialize_jobs(config, workflow_id as i64, Some(false), None, None)
    //     .expect("Failed to initialize jobs");
    // let job1_post = default_api::get_job(config, job1.id.unwrap()).expect("Failed to get job1");
    // let job2_post = default_api::get_job(config, job2_id).expect("Failed to get job2");
    // let bystander_post =
    //     default_api::get_job(config, bystander_id).expect("Failed to get bystander");
    // assert_eq!(job1_post.status, Some(models::JobStatus::Ready));
    // assert_eq!(job2_post.status, Some(models::JobStatus::Blocked));
    // assert_eq!(bystander_post.status, Some(models::JobStatus::Completed));
}

#[rstest]
fn test_remove_job(start_server: &ServerProcess) {
    assert!(start_server.child.id() > 0);
    let config = &start_server.config;
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();
    let jobs = create_diamond_workflow(config, true, work_dir);
    for (name, job) in &jobs {
        let removed =
            default_api::delete_job(config, job.id.unwrap(), None).expect("Failed to delete job");
        let result = default_api::get_job(config, removed.id.unwrap());
        assert!(result.is_err(), "Expected job {} to be deleted", name);
    }
}

#[rstest]
fn test_events(start_server: &ServerProcess) {
    assert!(start_server.child.id() > 0);
    let config = &start_server.config;
    let name = "test_event_workflow".to_string();
    let user = "test_user".to_string();
    let workflow = models::WorkflowModel::new(name.clone(), user.clone());
    let created_workflow =
        default_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();
    let event1 = default_api::create_event(
        config,
        models::EventModel::new(
            workflow_id as i64,
            serde_json::json!({"key1": 1, "key2": 2}),
        ),
    )
    .expect("Failed to create event");
    let event2 = default_api::create_event(
        config,
        models::EventModel::new(
            workflow_id as i64,
            serde_json::json!({"key3": 3, "key4": 4}),
        ),
    )
    .expect("Failed to create event");

    let event_id1 = event1.id.unwrap();
    let event_id2 = event2.id.unwrap();

    let events = default_api::list_events(
        config,
        workflow_id as i64,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list events");
    assert_eq!(events.items.as_ref().unwrap().len(), 2);
    assert_eq!(
        events.items.as_ref().unwrap()[1].data,
        serde_json::json!({"key3": 3, "key4": 4})
    );
    default_api::delete_event(config, event_id1, None).expect("Failed to delete event");
    default_api::delete_event(config, event_id2, None).expect("Failed to delete event");
    let events = default_api::list_events(
        config,
        workflow_id as i64,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list events");
    assert!(events.items.as_ref().unwrap().is_empty());
}

fn check_diamond_workflow_init_job_statuses(
    config: &torc::client::Configuration,
    jobs: &HashMap<String, models::JobModel>,
) {
    let preprocess = jobs.get("preprocess").expect("preprocess job not found");
    let work1 = jobs.get("work1").expect("work1 job not found");
    let work2 = jobs.get("work2").expect("work2 job not found");
    let postprocess = jobs.get("postprocess").expect("postprocess job not found");

    let preprocess_post =
        default_api::get_job(config, preprocess.id.unwrap()).expect("Failed to get preprocess");
    assert_eq!(preprocess_post.status.unwrap(), models::JobStatus::Ready);
    let work1_post = default_api::get_job(config, work1.id.unwrap()).expect("Failed to get work1");
    assert_eq!(work1_post.status.unwrap(), models::JobStatus::Blocked);
    let work2_post = default_api::get_job(config, work2.id.unwrap()).expect("Failed to get work2");
    assert_eq!(work2_post.status.unwrap(), models::JobStatus::Blocked);
    let postprocess_post =
        default_api::get_job(config, postprocess.id.unwrap()).expect("Failed to get postprocess");
    assert_eq!(postprocess_post.status.unwrap(), models::JobStatus::Blocked);
}

#[rstest]
#[case(None)] // Test with resource-based allocation
#[case(Some(10))] // Test with simple queue-based allocation (max 10 jobs at a time)
fn test_many_jobs_parameterized(
    start_server: &ServerProcess,
    #[case] max_parallel_jobs: Option<i64>,
) {
    assert!(start_server.child.id() > 0);
    let config = &start_server.config;

    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path().to_path_buf();

    // Generate unique workflow name to avoid conflicts when running in parallel
    let workflow_name = format!(
        "many_jobs_test_{}",
        max_parallel_jobs
            .map(|n| n.to_string())
            .unwrap_or("resources".to_string())
    );

    // Create YAML workflow specification with 30 parameterized jobs
    // Note: {{i:03d}} and {{i}} are escaped for format! macro (becomes {i:03d} and {i} in output)
    let yaml_content = format!(
        r#"name: {}
user: test_user
description: Test workflow with 30 parameterized jobs

jobs:
  - name: job_{{i:03d}}
    command: echo {{i}}
    resource_requirements: minimal
    parameters:
      i: "1:30"

resource_requirements:
  - name: minimal
    num_cpus: 1
    num_gpus: 0
    num_nodes: 1
    memory: 1m
    runtime: P0DT1M
"#,
        workflow_name
    );

    // Write YAML to temp file
    let yaml_path = work_dir.join("hundred_jobs_test.yaml");
    fs::write(&yaml_path, yaml_content).expect("Failed to write YAML file");

    // Build CLI arguments based on max_parallel_jobs parameter
    let mut cli_args = vec![yaml_path.to_str().unwrap(), "--poll-interval", "0.1"];

    let max_jobs_str;
    if let Some(max_jobs) = max_parallel_jobs {
        // Use simple queue-based allocation with claim_next_jobs
        max_jobs_str = max_jobs.to_string();
        cli_args.push("--max-parallel-jobs");
        cli_args.push(&max_jobs_str);
    } else {
        // Use resource-based allocation with claim_jobs_based_on_resources
        cli_args.push("--num-cpus");
        cli_args.push("12");
        cli_args.push("--memory-gb");
        cli_args.push("64.0");
    }

    run_jobs_cli_command(&cli_args, start_server).expect("Failed to run jobs");

    // Get the workflow that was created by 'torc run'
    let workflows = default_api::list_workflows(
        config,
        None,
        None,
        None,
        None,
        Some(&workflow_name),
        None,
        None,
        None,
    )
    .expect("Failed to list workflows");

    let workflow = workflows
        .items
        .as_ref()
        .and_then(|items| items.first())
        .expect("Workflow not found");
    let workflow_id = workflow.id.unwrap();

    // Verify all 100 jobs completed successfully
    verify_many_jobs_completion(config, workflow_id, 30);

    // Cleanup
    default_api::delete_workflow(config, workflow_id, None).expect("Failed to delete workflow");
}

fn verify_many_jobs_completion(
    config: &torc::client::Configuration,
    workflow_id: i64,
    num_jobs: usize,
) {
    let jobs = default_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None, // include_relationships
        None, // active_compute_node_id
    )
    .expect("Failed to list jobs");

    let job_items = jobs.items.unwrap();
    assert_eq!(
        job_items.len(),
        num_jobs,
        "Expected {} jobs, but got {}",
        num_jobs,
        job_items.len()
    );

    for job in &job_items {
        assert_eq!(
            job.status.unwrap(),
            models::JobStatus::Completed,
            "Job {} should be completed. actual status: {:?}",
            job.name,
            job.status
        );
    }

    // Get results for all jobs in the workflow and verify return codes
    let results = default_api::list_results(
        config,
        workflow_id,
        None, // job_id - get results for all jobs
        None, // run_id
        None, // offset
        None, // limit
        None, // sort_by
        None, // reverse_sort
        None, // return_code filter
        None, // status filter
        None, // all_runs
        None, // compute_node_id
    )
    .expect("Failed to list results");

    let result_items = results.items.unwrap();
    assert_eq!(
        result_items.len(),
        num_jobs,
        "Expected {} results, but got {}",
        num_jobs,
        result_items.len()
    );

    for result in &result_items {
        assert_eq!(
            result.return_code, 0,
            "Job ID {} should have return code 0, but got {}",
            result.job_id, result.return_code
        );
    }
}

/// Test workflow reinitialization after a job failure.
///
/// This test creates a three-stage workflow:
/// - Stage 1: One job (setup)
/// - Stage 2: Three jobs (work_a, work_b, work_fail) - work_fail fails based on a flag file
/// - Stage 3: One job (finalize) that depends on all Stage 2 jobs
///
/// The test verifies:
/// 1. First run: setup completes, work_a/work_b complete, work_fail fails, finalize is canceled
/// 2. After reset-status --failed-only: work_fail becomes ready, finalize becomes blocked
/// 3. Second run (with flag file removed): all jobs complete with return code 0
#[rstest]
fn test_workflow_reinitialization_after_failure(start_server: &ServerProcess) {
    assert!(start_server.child.id() > 0);
    let config = &start_server.config;

    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path().to_path_buf();

    // Create a flag file that will cause work_fail to fail
    // The job checks for the presence of this file to determine pass/fail
    let fail_flag_path = work_dir.join("should_fail.flag");
    fs::write(&fail_flag_path, "fail").expect("Failed to write fail flag file");

    // Create workflow with three stages using YAML spec
    // Stage 2's work_fail job checks for the presence of a flag file to determine pass/fail
    let yaml_content = format!(
        r#"name: restart_test_workflow
user: test_user
description: Test workflow restart after failure

jobs:
  # Stage 1: Setup job (no dependencies)
  - name: setup
    command: echo "Setup complete"
    resource_requirements: minimal

  # Stage 2: Three parallel jobs that depend on setup
  - name: work_a
    command: echo "Work A complete"
    depends_on:
      - setup
    resource_requirements: minimal

  - name: work_b
    command: echo "Work B complete"
    depends_on:
      - setup
    resource_requirements: minimal

  - name: work_fail
    command: 'if [ -f "{}" ]; then echo "Intentional failure"; exit 1; else echo "Work fail job succeeds"; exit 0; fi'
    depends_on:
      - setup
    resource_requirements: minimal

  # Stage 3: Finalize job that depends on all Stage 2 jobs
  # Note: cancel_on_blocking_job_failure defaults to true, so finalize will be
  # automatically canceled if any of its dependencies fail
  - name: finalize
    command: echo "Finalize complete"
    depends_on:
      - work_a
      - work_b
      - work_fail
    resource_requirements: minimal

resource_requirements:
  - name: minimal
    num_cpus: 1
    num_gpus: 0
    num_nodes: 1
    memory: 1m
    runtime: P0DT1M
"#,
        fail_flag_path.display()
    );

    // Write YAML to temp file
    let yaml_path = work_dir.join("restart_test.yaml");
    fs::write(&yaml_path, &yaml_content).expect("Failed to write YAML file");

    // === First run: work_fail should fail (flag file exists) ===

    // Run with default parallelism - the server correctly handles the case where
    // multiple jobs complete together and one fails, ensuring dependent jobs are canceled
    run_jobs_cli_command(
        &[
            yaml_path.to_str().unwrap(),
            "--poll-interval",
            "0.1",
            "--max-parallel-jobs",
            "4",
        ],
        start_server,
    )
    .expect("First run command should succeed (workflow completes, checking job statuses)");

    // Find the workflow that was created
    let workflows = default_api::list_workflows(
        config,
        None,
        None,
        None,
        None,
        Some("restart_test_workflow"),
        None,
        None,
        None,
    )
    .expect("Failed to list workflows");

    let workflow = workflows
        .items
        .as_ref()
        .and_then(|items| items.first())
        .expect("Workflow not found");
    let workflow_id = workflow.id.unwrap();

    // Verify job statuses after first run
    let jobs = default_api::list_jobs(
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
        None, // active_compute_node_id
    )
    .expect("Failed to list jobs");

    let job_items = jobs.items.unwrap();
    let job_statuses: HashMap<String, models::JobStatus> = job_items
        .iter()
        .map(|j| (j.name.clone(), j.status.unwrap()))
        .collect();

    // Stage 1 should be complete
    assert_eq!(
        job_statuses.get("setup").unwrap(),
        &models::JobStatus::Completed,
        "setup should be completed"
    );

    // Stage 2: work_a and work_b should be complete, work_fail should be failed
    assert_eq!(
        job_statuses.get("work_a").unwrap(),
        &models::JobStatus::Completed,
        "work_a should be completed"
    );
    assert_eq!(
        job_statuses.get("work_b").unwrap(),
        &models::JobStatus::Completed,
        "work_b should be completed"
    );
    assert_eq!(
        job_statuses.get("work_fail").unwrap(),
        &models::JobStatus::Failed,
        "work_fail should be failed"
    );

    // Stage 3: finalize should be canceled (because a dependency failed)
    assert_eq!(
        job_statuses.get("finalize").unwrap(),
        &models::JobStatus::Canceled,
        "finalize should be canceled due to failed dependency"
    );

    // Verify return codes from results
    let results = default_api::list_results(
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
        None, // compute_node_id
    )
    .expect("Failed to list results");

    let result_items = results.items.unwrap();

    // We should have 4 results (setup, work_a, work_b, work_fail)
    // finalize was canceled so it shouldn't have a result
    assert_eq!(
        result_items.len(),
        4,
        "Expected 4 results (finalize was canceled)"
    );

    // Find work_fail result and verify it has non-zero return code
    let work_fail_result = result_items
        .iter()
        .find(|r| {
            let job = job_items
                .iter()
                .find(|j| j.id.unwrap() == r.job_id)
                .unwrap();
            job.name == "work_fail"
        })
        .expect("work_fail result not found");
    assert_eq!(
        work_fail_result.return_code, 1,
        "work_fail should have return code 1"
    );

    // === Reset status with --failed-only and --reinitialize ===

    run_cli_command(
        &[
            "workflows",
            "reset-status",
            &workflow_id.to_string(),
            "--failed-only",
            "--reinitialize",
            "--no-prompts",
        ],
        start_server,
        None,
    )
    .expect("Failed to reset workflow status");

    // Verify job statuses after reset
    let jobs = default_api::list_jobs(
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
        None, // active_compute_node_id
    )
    .expect("Failed to list jobs after reset");

    let job_items = jobs.items.unwrap();
    let job_statuses: HashMap<String, models::JobStatus> = job_items
        .iter()
        .map(|j| (j.name.clone(), j.status.unwrap()))
        .collect();

    // Stage 1 should still be complete (wasn't failed)
    assert_eq!(
        job_statuses.get("setup").unwrap(),
        &models::JobStatus::Completed,
        "setup should still be completed after reset"
    );

    // Stage 2: work_a and work_b should still be complete
    assert_eq!(
        job_statuses.get("work_a").unwrap(),
        &models::JobStatus::Completed,
        "work_a should still be completed after reset"
    );
    assert_eq!(
        job_statuses.get("work_b").unwrap(),
        &models::JobStatus::Completed,
        "work_b should still be completed after reset"
    );

    // work_fail should now be ready (reset from failed)
    assert_eq!(
        job_statuses.get("work_fail").unwrap(),
        &models::JobStatus::Ready,
        "work_fail should be ready after reset"
    );

    // finalize should be blocked (waiting on work_fail to complete)
    assert_eq!(
        job_statuses.get("finalize").unwrap(),
        &models::JobStatus::Blocked,
        "finalize should be blocked after reset"
    );

    // === Second run: work_fail should succeed now ===

    // Remove the flag file so work_fail will succeed
    fs::remove_file(&fail_flag_path).expect("Failed to remove flag file");

    // Run the workflow again
    run_jobs_cli_command(
        &[
            &workflow_id.to_string(),
            "--poll-interval",
            "0.1",
            "--max-parallel-jobs",
            "4",
        ],
        start_server,
    )
    .expect("Second run should succeed");

    // Verify all jobs are now completed
    let jobs = default_api::list_jobs(
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
        None, // active_compute_node_id
    )
    .expect("Failed to list jobs after second run");

    let job_items = jobs.items.unwrap();
    for job in &job_items {
        assert_eq!(
            job.status.unwrap(),
            models::JobStatus::Completed,
            "Job {} should be completed after second run, got {:?}",
            job.name,
            job.status
        );
    }

    // Verify all results have return code 0
    // Get results for all runs (not just run_id 1)
    let results = default_api::list_results(
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
        Some(true), // all_runs=true
        None,       // compute_node_id
    )
    .expect("Failed to list all results");

    let result_items = results.items.unwrap();

    // Find the latest run_id for work_fail job
    let work_fail_job = job_items.iter().find(|j| j.name == "work_fail").unwrap();
    let work_fail_latest = result_items
        .iter()
        .filter(|r| r.job_id == work_fail_job.id.unwrap())
        .max_by_key(|r| (r.run_id, r.attempt_id.unwrap_or(1)))
        .expect("work_fail should have results");
    assert_eq!(
        work_fail_latest.return_code, 0,
        "work_fail latest run should have return code 0"
    );

    // Find finalize result
    let finalize_job = job_items.iter().find(|j| j.name == "finalize").unwrap();
    let finalize_result = result_items
        .iter()
        .find(|r| r.job_id == finalize_job.id.unwrap())
        .expect("finalize should have a result after second run");
    assert_eq!(
        finalize_result.return_code, 0,
        "finalize should have return code 0"
    );

    // Cleanup
    default_api::delete_workflow(config, workflow_id, None)
        .expect("Failed to delete restart_test workflow");
}

/// Test workflow restart after fixing a bad input file using reinitialize.
///
/// This test creates a three-stage workflow similar to test_workflow_restart_after_failure,
/// but instead of using a flag file, it uses an input file with bad data that causes the job
/// to fail. The reinitialize command detects the file has changed and resets the job.
///
/// The test verifies:
/// 1. First run: setup completes, work_a/work_b complete, work_fail fails (bad input), finalize is canceled
/// 2. After fixing input file and running reinitialize: work_fail becomes ready, finalize becomes blocked
/// 3. Second run: all jobs complete with return code 0
#[rstest]
fn test_workflow_reinitialize_after_fixing_input(start_server: &ServerProcess) {
    assert!(start_server.child.id() > 0);
    let config = &start_server.config;

    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path().to_path_buf();

    // Create an input file with bad data that will cause the job to fail
    let input_file_path = work_dir.join("config.json");
    let bad_input = r#"{"valid": false, "message": "This input should cause failure"}"#;
    fs::write(&input_file_path, bad_input).expect("Failed to write config.json");

    // Create workflow with three stages using YAML spec
    // The work_fail job reads the config.json file and fails if valid=false
    let yaml_content = format!(
        r#"name: reinitialize_test_workflow
user: test_user
description: Test workflow reinitialize after fixing input file

files:
  - name: config_file
    path: {config_path}

jobs:
  # Stage 1: Setup job (no dependencies)
  - name: setup
    command: echo "Setup complete"
    resource_requirements: minimal

  # Stage 2: Three parallel jobs that depend on setup
  - name: work_a
    command: echo "Work A complete"
    depends_on:
      - setup
    resource_requirements: minimal

  - name: work_b
    command: echo "Work B complete"
    depends_on:
      - setup
    resource_requirements: minimal

  # This job reads the config file and fails if valid=false
  - name: work_fail
    command: 'if grep -q "\"valid\": true" {config_path}; then echo "Input valid, job succeeds"; exit 0; else echo "Input invalid, job fails"; exit 1; fi'
    depends_on:
      - setup
    input_files:
      - config_file
    resource_requirements: minimal

  # Stage 3: Finalize job that depends on all Stage 2 jobs
  - name: finalize
    command: echo "Finalize complete"
    depends_on:
      - work_a
      - work_b
      - work_fail
    resource_requirements: minimal

resource_requirements:
  - name: minimal
    num_cpus: 1
    num_gpus: 0
    num_nodes: 1
    memory: 1m
    runtime: P0DT1M
"#,
        config_path = input_file_path.display()
    );

    // Write YAML to temp file
    let yaml_path = work_dir.join("reinitialize_test.yaml");
    fs::write(&yaml_path, &yaml_content).expect("Failed to write YAML file");

    // === First run: work_fail should fail (bad input) ===

    run_jobs_cli_command(
        &[
            yaml_path.to_str().unwrap(),
            "--poll-interval",
            "0.1",
            "--max-parallel-jobs",
            "4",
        ],
        start_server,
    )
    .expect("First run command should succeed (workflow completes, checking job statuses)");

    // Find the workflow that was created
    let workflows = default_api::list_workflows(
        config,
        None,
        None,
        None,
        None,
        Some("reinitialize_test_workflow"),
        None,
        None,
        None,
    )
    .expect("Failed to list workflows");

    let workflow = workflows
        .items
        .as_ref()
        .and_then(|items| items.first())
        .expect("Workflow not found");
    let workflow_id = workflow.id.unwrap();

    // Verify job statuses after first run
    let jobs = default_api::list_jobs(
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
        None, // active_compute_node_id
    )
    .expect("Failed to list jobs");

    let job_items = jobs.items.unwrap();
    let job_statuses: HashMap<String, models::JobStatus> = job_items
        .iter()
        .map(|j| (j.name.clone(), j.status.unwrap()))
        .collect();

    // Stage 1 should be complete
    assert_eq!(
        job_statuses.get("setup").unwrap(),
        &models::JobStatus::Completed,
        "setup should be completed"
    );

    // Stage 2: work_a and work_b should be complete, work_fail should be failed
    assert_eq!(
        job_statuses.get("work_a").unwrap(),
        &models::JobStatus::Completed,
        "work_a should be completed"
    );
    assert_eq!(
        job_statuses.get("work_b").unwrap(),
        &models::JobStatus::Completed,
        "work_b should be completed"
    );
    assert_eq!(
        job_statuses.get("work_fail").unwrap(),
        &models::JobStatus::Failed,
        "work_fail should be failed"
    );

    // Stage 3: finalize should be canceled (because a dependency failed)
    assert_eq!(
        job_statuses.get("finalize").unwrap(),
        &models::JobStatus::Canceled,
        "finalize should be canceled due to failed dependency"
    );

    // === Fix the input file and run reinitialize ===

    // Wait a moment to ensure file mtime changes
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Fix the input file
    let good_input = r#"{"valid": true, "message": "This input should succeed"}"#;
    fs::write(&input_file_path, good_input).expect("Failed to write fixed config.json");

    // Run reinitialize command
    run_cli_command(
        &["workflows", "reinitialize", &workflow_id.to_string()],
        start_server,
        None,
    )
    .expect("Failed to reinitialize workflow");

    // Verify job statuses after reinitialize
    let jobs = default_api::list_jobs(
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
        None, // active_compute_node_id
    )
    .expect("Failed to list jobs after reinitialize");

    let job_items = jobs.items.unwrap();
    let job_statuses: HashMap<String, models::JobStatus> = job_items
        .iter()
        .map(|j| (j.name.clone(), j.status.unwrap()))
        .collect();

    // Stage 1 should still be complete (wasn't affected)
    assert_eq!(
        job_statuses.get("setup").unwrap(),
        &models::JobStatus::Completed,
        "setup should still be completed after reinitialize"
    );

    // Stage 2: work_a and work_b should still be complete
    assert_eq!(
        job_statuses.get("work_a").unwrap(),
        &models::JobStatus::Completed,
        "work_a should still be completed after reinitialize"
    );
    assert_eq!(
        job_statuses.get("work_b").unwrap(),
        &models::JobStatus::Completed,
        "work_b should still be completed after reinitialize"
    );

    // work_fail should now be ready (reinitialize detected changed input file)
    assert_eq!(
        job_statuses.get("work_fail").unwrap(),
        &models::JobStatus::Ready,
        "work_fail should be ready after reinitialize (input file changed)"
    );

    // finalize should be blocked (waiting on work_fail to complete)
    assert_eq!(
        job_statuses.get("finalize").unwrap(),
        &models::JobStatus::Blocked,
        "finalize should be blocked after reinitialize"
    );

    // === Second run: work_fail should succeed now ===

    run_jobs_cli_command(
        &[
            &workflow_id.to_string(),
            "--poll-interval",
            "0.1",
            "--max-parallel-jobs",
            "4",
        ],
        start_server,
    )
    .expect("Second run should succeed");

    // Verify all jobs are now completed
    let jobs = default_api::list_jobs(
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
        None, // active_compute_node_id
    )
    .expect("Failed to list jobs after second run");

    let job_items = jobs.items.unwrap();
    for job in &job_items {
        assert_eq!(
            job.status.unwrap(),
            models::JobStatus::Completed,
            "Job {} should be completed after second run, got {:?}",
            job.name,
            job.status
        );
    }

    // Verify all results have return code 0
    // Get results for all runs (not just run_id 1)
    let results = default_api::list_results(
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
        Some(true), // all_runs=true
        None,       // compute_node_id
    )
    .expect("Failed to list all results");

    let result_items = results.items.unwrap();

    // Find the latest run_id for work_fail job
    let work_fail_job = job_items.iter().find(|j| j.name == "work_fail").unwrap();
    let work_fail_latest = result_items
        .iter()
        .filter(|r| r.job_id == work_fail_job.id.unwrap())
        .max_by_key(|r| (r.run_id, r.attempt_id.unwrap_or(1)))
        .expect("work_fail should have results");
    assert_eq!(
        work_fail_latest.return_code, 0,
        "work_fail latest run should have return code 0"
    );

    // Find finalize result
    let finalize_job = job_items.iter().find(|j| j.name == "finalize").unwrap();
    let finalize_result = result_items
        .iter()
        .find(|r| r.job_id == finalize_job.id.unwrap())
        .expect("finalize should have a result after second run");
    assert_eq!(
        finalize_result.return_code, 0,
        "finalize should have return code 0"
    );

    // Cleanup
    default_api::delete_workflow(config, workflow_id, None)
        .expect("Failed to delete reinitialize_test workflow");
}
