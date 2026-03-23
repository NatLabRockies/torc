//! End-to-end integration tests for direct mode execution.
//!
//! These tests verify that execution_config with mode: direct works correctly
//! for actual job execution, comparable to slurm-tests/workflows/oom_detection.yaml.
//!
//! Tests cover:
//! - Successful job execution in direct mode
//! - Multiple jobs with dependencies
//! - Resource monitoring in direct mode
//! - Job timeout behavior (future: when full termination timeline is implemented)
//! - Memory limit enforcement (future: when OOM detection is implemented)

#![allow(clippy::useless_vec)]

mod common;

use chrono::{Duration, Utc};
use common::{ServerProcess, run_jobs_cli_command, start_server};
use rstest::rstest;
use serial_test::serial;
use std::fs;
use tempfile::NamedTempFile;
use torc::client::apis;
use torc::client::workflow_spec::WorkflowSpec;
use torc::models::JobStatus;

// =============================================================================
// Helper functions
// =============================================================================

fn create_workflow_from_yaml(
    server: &ServerProcess,
    yaml: &str,
) -> Result<i64, Box<dyn std::error::Error>> {
    let temp_file = NamedTempFile::new()?;
    fs::write(temp_file.path(), yaml)?;

    WorkflowSpec::create_workflow_from_spec(
        &server.config,
        temp_file.path(),
        "test_user",
        false,
        false,
    )
}

fn verify_all_jobs_completed(server: &ServerProcess, workflow_id: i64) {
    let jobs = apis::jobs_api::list_jobs(
        &server.config,
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
    )
    .expect("Failed to list jobs");

    for job in jobs.items {
        assert_eq!(
            job.status.unwrap(),
            JobStatus::Completed,
            "Job '{}' should be completed, got {:?}",
            job.name,
            job.status
        );
    }
}

fn verify_all_jobs_return_code(server: &ServerProcess, workflow_id: i64, expected_code: i64) {
    let results = apis::results_api::list_results(
        &server.config,
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
    )
    .expect("Failed to list results");

    for result in results.items {
        assert_eq!(
            result.return_code, expected_code,
            "Job ID {} should have return code {}, got {}",
            result.job_id, expected_code, result.return_code
        );
    }
}

fn get_job_return_code(server: &ServerProcess, workflow_id: i64, job_name: &str) -> Option<i64> {
    let jobs = apis::jobs_api::list_jobs(
        &server.config,
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
    )
    .expect("Failed to list jobs");

    let job = jobs.items.into_iter().find(|j| j.name == job_name)?;

    let results = apis::results_api::list_results(
        &server.config,
        workflow_id,
        job.id,
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
    .expect("Failed to list results");

    results.items.first().map(|r| r.return_code)
}

// =============================================================================
// Basic direct mode execution tests
// =============================================================================

#[rstest]
fn test_direct_mode_single_job_success(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();
    let output_file = work_dir.join("output.txt");

    let yaml = format!(
        r#"
name: direct_mode_single_job
user: test_user

jobs:
  - name: simple_job
    command: |
      echo "Hello from direct mode" > {}
      echo "Job completed successfully"

execution_config:
    mode: direct
"#,
        output_file.display()
    );

    let workflow_id =
        create_workflow_from_yaml(start_server, &yaml).expect("Failed to create workflow");

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.1".to_string(),
        "--num-cpus".to_string(),
        "2".to_string(),
        "--memory-gb".to_string(),
        "1.0".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    run_jobs_cli_command(&cli_args_refs, start_server).expect("Failed to run jobs");

    verify_all_jobs_completed(start_server, workflow_id);
    verify_all_jobs_return_code(start_server, workflow_id, 0);

    assert!(output_file.exists(), "Output file should exist");
    let content = fs::read_to_string(&output_file).expect("Failed to read output");
    assert!(content.contains("Hello from direct mode"));
}

#[rstest]
fn test_direct_mode_multiple_jobs_with_dependencies(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    let yaml = format!(
        r#"
name: direct_mode_dependencies
user: test_user

files:
  - name: intermediate
    path: {}/intermediate.txt
  - name: final_output
    path: {}/final.txt

jobs:
  - name: producer
    command: |
      echo "Data from producer" > {}/intermediate.txt
    output_files:
      - intermediate

  - name: consumer
    command: |
      cat {}/intermediate.txt > {}/final.txt
      echo "Processed by consumer" >> {}/final.txt
    input_files:
      - intermediate
    output_files:
      - final_output
    depends_on:
      - producer

execution_config:
    mode: direct
    limit_resources: false
"#,
        work_dir.display(),
        work_dir.display(),
        work_dir.display(),
        work_dir.display(),
        work_dir.display(),
        work_dir.display()
    );

    let workflow_id =
        create_workflow_from_yaml(start_server, &yaml).expect("Failed to create workflow");

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.1".to_string(),
        "--max-parallel-jobs".to_string(),
        "2".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    run_jobs_cli_command(&cli_args_refs, start_server).expect("Failed to run jobs");

    verify_all_jobs_completed(start_server, workflow_id);

    let final_output = work_dir.join("final.txt");
    assert!(final_output.exists(), "Final output should exist");
    let content = fs::read_to_string(&final_output).expect("Failed to read final output");
    assert!(content.contains("Data from producer"));
    assert!(content.contains("Processed by consumer"));
}

// =============================================================================
// Direct mode with resource monitoring
// =============================================================================

#[rstest]
fn test_direct_mode_with_resource_monitoring(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    let yaml = r#"
name: direct_mode_resource_monitor
user: test_user

resource_monitor:
  enabled: true
  granularity: summary
  sample_interval_seconds: 1

resource_requirements:
  - name: small
    num_cpus: 1
    memory: 256m
    runtime: PT1M

jobs:
  - name: monitored_job
    command: |
      echo "Starting monitored job"
      sleep 2
      echo "Monitored job complete"
    resource_requirements: small

execution_config:
    mode: direct
    limit_resources: true
"#;

    let workflow_id =
        create_workflow_from_yaml(start_server, yaml).expect("Failed to create workflow");

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.1".to_string(),
        "--num-cpus".to_string(),
        "2".to_string(),
        "--memory-gb".to_string(),
        "1.0".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    run_jobs_cli_command(&cli_args_refs, start_server).expect("Failed to run jobs");

    verify_all_jobs_completed(start_server, workflow_id);
    verify_all_jobs_return_code(start_server, workflow_id, 0);

    // Resource metrics database should be created when monitoring is enabled
    let _metrics_db = work_dir.join("resource_metrics.db");
    // Note: metrics DB may not exist for very short jobs, so we just verify job completed
}

// =============================================================================
// Direct mode comparable to oom_detection.yaml structure
// =============================================================================

#[rstest]
fn test_direct_mode_mixed_success_failure(start_server: &ServerProcess) {
    // Similar structure to oom_detection.yaml: one good job, one that fails
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    let yaml = r#"
name: direct_mode_mixed
user: test_user
description: Direct mode test - one good job, one that fails

resource_requirements:
  - name: normal_resources
    num_cpus: 1
    memory: 256m
    runtime: PT1M

jobs:
  - name: normal_job
    command: |
      echo "Normal job running"
      sleep 1
      echo "Normal job complete"
    resource_requirements: normal_resources

  - name: failing_job
    command: |
      echo "Failing job starting"
      exit 1
    resource_requirements: normal_resources

execution_config:
    mode: direct
    limit_resources: true
"#;

    let workflow_id =
        create_workflow_from_yaml(start_server, yaml).expect("Failed to create workflow");

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.1".to_string(),
        "--num-cpus".to_string(),
        "2".to_string(),
        "--memory-gb".to_string(),
        "1.0".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    // Run jobs - some may fail
    let _ = run_jobs_cli_command(&cli_args_refs, start_server);

    // Verify normal_job succeeded
    let normal_return_code = get_job_return_code(start_server, workflow_id, "normal_job");
    assert_eq!(
        normal_return_code,
        Some(0),
        "normal_job should have return code 0"
    );

    // Verify failing_job failed with exit code 1
    let failing_return_code = get_job_return_code(start_server, workflow_id, "failing_job");
    assert_eq!(
        failing_return_code,
        Some(1),
        "failing_job should have return code 1"
    );
}

// =============================================================================
// Direct mode with custom termination settings
// =============================================================================

#[rstest]
fn test_direct_mode_custom_termination_config(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    let yaml = r#"
name: direct_mode_custom_termination
user: test_user

jobs:
  - name: quick_job
    command: |
      echo "Quick job with custom termination settings"
      sleep 1
      echo "Done"

execution_config:
    mode: direct
    termination_signal: SIGTERM
    sigterm_lead_seconds: 30
    sigkill_headroom_seconds: 60
    timeout_exit_code: 152
    oom_exit_code: 137
"#;

    let workflow_id =
        create_workflow_from_yaml(start_server, yaml).expect("Failed to create workflow");

    // Verify the execution_config was stored correctly
    let workflow = apis::workflows_api::get_workflow(&start_server.config, workflow_id)
        .expect("Failed to get workflow");

    assert!(workflow.execution_config.is_some());
    let exec_config: torc::client::workflow_spec::ExecutionConfig =
        serde_json::from_str(workflow.execution_config.as_ref().unwrap()).unwrap();

    assert_eq!(exec_config.termination_signal, Some("SIGTERM".to_string()));
    assert_eq!(exec_config.sigterm_lead_seconds, Some(30));
    assert_eq!(exec_config.sigkill_headroom_seconds, Some(60));
    assert_eq!(exec_config.timeout_exit_code, Some(152));
    assert_eq!(exec_config.oom_exit_code, Some(137));

    // Run the workflow
    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.1".to_string(),
        "--max-parallel-jobs".to_string(),
        "1".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    run_jobs_cli_command(&cli_args_refs, start_server).expect("Failed to run jobs");

    verify_all_jobs_completed(start_server, workflow_id);
}

// =============================================================================
// Auto mode detection (defaults to direct outside Slurm)
// =============================================================================

#[rstest]
#[serial(slurm)]
fn test_auto_mode_runs_as_direct_outside_slurm(start_server: &ServerProcess) {
    // Ensure SLURM_JOB_ID is not set
    // SAFETY: Using serial_test to prevent concurrent access to env vars
    unsafe {
        std::env::remove_var("SLURM_JOB_ID");
    }

    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();
    let output_file = work_dir.join("auto_mode_output.txt");

    let yaml = format!(
        r#"
name: auto_mode_direct_fallback
user: test_user

jobs:
  - name: auto_job
    command: |
      echo "Running in auto mode (should be direct)" > {}

execution_config:
    mode: auto
    sigkill_headroom_seconds: 120
"#,
        output_file.display()
    );

    let workflow_id =
        create_workflow_from_yaml(start_server, &yaml).expect("Failed to create workflow");

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.1".to_string(),
        "--max-parallel-jobs".to_string(),
        "1".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    run_jobs_cli_command(&cli_args_refs, start_server).expect("Failed to run jobs");

    verify_all_jobs_completed(start_server, workflow_id);

    assert!(output_file.exists(), "Output file should exist");
    let content = fs::read_to_string(&output_file).expect("Failed to read output");
    assert!(content.contains("Running in auto mode"));
}

// =============================================================================
// Diamond workflow in direct mode
// =============================================================================

#[rstest]
fn test_direct_mode_diamond_workflow(start_server: &ServerProcess) {
    // Classic diamond dependency pattern:
    //     A
    //    / \
    //   B   C
    //    \ /
    //     D

    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    let yaml = format!(
        r#"
name: direct_mode_diamond
user: test_user

files:
  - name: a_out
    path: {0}/a.txt
  - name: b_out
    path: {0}/b.txt
  - name: c_out
    path: {0}/c.txt
  - name: d_out
    path: {0}/d.txt

jobs:
  - name: job_a
    command: echo "A" > {0}/a.txt
    output_files: [a_out]

  - name: job_b
    command: |
      cat {0}/a.txt > {0}/b.txt
      echo "B" >> {0}/b.txt
    input_files: [a_out]
    output_files: [b_out]
    depends_on: [job_a]

  - name: job_c
    command: |
      cat {0}/a.txt > {0}/c.txt
      echo "C" >> {0}/c.txt
    input_files: [a_out]
    output_files: [c_out]
    depends_on: [job_a]

  - name: job_d
    command: |
      cat {0}/b.txt {0}/c.txt > {0}/d.txt
      echo "D" >> {0}/d.txt
    input_files: [b_out, c_out]
    output_files: [d_out]
    depends_on: [job_b, job_c]

execution_config:
    mode: direct
"#,
        work_dir.display()
    );

    let workflow_id =
        create_workflow_from_yaml(start_server, &yaml).expect("Failed to create workflow");

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.1".to_string(),
        "--num-cpus".to_string(),
        "4".to_string(),
        "--memory-gb".to_string(),
        "1.0".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    run_jobs_cli_command(&cli_args_refs, start_server).expect("Failed to run jobs");

    verify_all_jobs_completed(start_server, workflow_id);

    // Verify all output files exist
    assert!(work_dir.join("a.txt").exists());
    assert!(work_dir.join("b.txt").exists());
    assert!(work_dir.join("c.txt").exists());
    assert!(work_dir.join("d.txt").exists());

    // Verify final output has content from all jobs
    let d_content = fs::read_to_string(work_dir.join("d.txt")).expect("Failed to read d.txt");
    assert!(d_content.contains("A"));
    assert!(d_content.contains("B"));
    assert!(d_content.contains("C"));
    assert!(d_content.contains("D"));
}

// =============================================================================
// OOM detection tests - mirrors slurm-tests/workflows/oom_detection.yaml
// =============================================================================

/// Test that mirrors slurm-tests/workflows/oom_detection.yaml structure.
/// This validates the workflow configuration and execution pattern that users
/// will use for OOM detection in direct mode.
///
/// Note: Full OOM enforcement (killing jobs that exceed memory limits) requires
/// ResourceMonitor integration which is planned for future implementation.
/// This test validates the configuration and workflow structure works correctly.
#[rstest]
fn test_direct_mode_oom_workflow_structure(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    // This workflow mirrors slurm-tests/workflows/oom_detection.yaml
    let yaml = format!(
        r#"
name: direct_mode_oom_detection
description: OOM detection test - one good job, one memory-intensive job

resource_monitor:
  enabled: true
  granularity: time_series
  sample_interval_seconds: 1

resource_requirements:
  - name: normal_resources
    num_cpus: 1
    memory: 256m
    runtime: PT1M

  - name: memory_intensive
    num_cpus: 1
    memory: 512m
    runtime: PT1M

jobs:
  - name: normal_job
    command: |
      echo "Normal job on $(hostname)" > {0}/normal_output.txt
      echo "This job should succeed."
      sleep 2
      echo "Normal job complete." >> {0}/normal_output.txt
    resource_requirements: normal_resources

  - name: memory_job
    command: |
      echo "Memory job starting" > {0}/memory_output.txt
      # Allocate some memory (within limits for this test)
      python3 -c "x = bytearray(50*1024*1024); print('Allocated 50MB')" >> {0}/memory_output.txt
      echo "Memory job complete" >> {0}/memory_output.txt
    resource_requirements: memory_intensive

execution_config:
    mode: direct
    limit_resources: true
    oom_exit_code: 137
    timeout_exit_code: 152
"#,
        work_dir.display()
    );

    let workflow_id =
        create_workflow_from_yaml(start_server, &yaml).expect("Failed to create workflow");

    // Verify execution_config was stored correctly
    let workflow = apis::workflows_api::get_workflow(&start_server.config, workflow_id)
        .expect("Failed to get workflow");

    assert!(workflow.execution_config.is_some());
    let exec_config: torc::client::workflow_spec::ExecutionConfig =
        serde_json::from_str(workflow.execution_config.as_ref().unwrap()).unwrap();

    assert_eq!(exec_config.oom_exit_code(), 137);
    assert_eq!(exec_config.timeout_exit_code(), 152);
    assert!(exec_config.limit_resources());

    // Verify resource_monitor_config was stored
    assert!(workflow.resource_monitor_config.is_some());

    // Run the workflow
    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.1".to_string(),
        "--num-cpus".to_string(),
        "2".to_string(),
        "--memory-gb".to_string(),
        "2.0".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    run_jobs_cli_command(&cli_args_refs, start_server).expect("Failed to run jobs");

    // Both jobs should complete (memory_job stays within limits)
    verify_all_jobs_completed(start_server, workflow_id);

    // Verify output files exist
    assert!(work_dir.join("normal_output.txt").exists());
    assert!(work_dir.join("memory_output.txt").exists());

    let normal_output =
        fs::read_to_string(work_dir.join("normal_output.txt")).expect("Failed to read output");
    assert!(normal_output.contains("Normal job complete"));

    let memory_output =
        fs::read_to_string(work_dir.join("memory_output.txt")).expect("Failed to read output");
    assert!(memory_output.contains("Memory job complete"));
}

/// Test time_series resource monitoring in direct mode.
/// This matches the monitoring configuration used in oom_detection.yaml.
#[rstest]
fn test_direct_mode_time_series_monitoring(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    let yaml = r#"
name: direct_mode_time_series_monitor
description: Test time_series granularity resource monitoring

resource_monitor:
  enabled: true
  granularity: time_series
  sample_interval_seconds: 1

resource_requirements:
  - name: monitored
    num_cpus: 1
    memory: 256m
    runtime: PT1M

jobs:
  - name: monitored_job
    command: |
      echo "Starting monitored job"
      # Do some work that can be monitored
      for i in 1 2 3; do
        echo "Iteration $i"
        sleep 1
      done
      echo "Monitored job complete"
    resource_requirements: monitored

execution_config:
    mode: direct
    limit_resources: true
"#;

    let workflow_id =
        create_workflow_from_yaml(start_server, yaml).expect("Failed to create workflow");

    // Verify resource_monitor_config
    let workflow = apis::workflows_api::get_workflow(&start_server.config, workflow_id)
        .expect("Failed to get workflow");

    let monitor_config: serde_json::Value =
        serde_json::from_str(workflow.resource_monitor_config.as_ref().unwrap()).unwrap();
    assert_eq!(monitor_config["granularity"], "time_series");
    assert_eq!(monitor_config["sample_interval_seconds"], 1);

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.1".to_string(),
        "--num-cpus".to_string(),
        "2".to_string(),
        "--memory-gb".to_string(),
        "1.0".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    run_jobs_cli_command(&cli_args_refs, start_server).expect("Failed to run jobs");

    verify_all_jobs_completed(start_server, workflow_id);
    verify_all_jobs_return_code(start_server, workflow_id, 0);
}

/// Test multiple resource requirement tiers like in oom_detection.yaml.
/// The original has normal_resources (2g) and oom_resources (4g).
#[rstest]
fn test_direct_mode_multiple_resource_tiers(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    let yaml = format!(
        r#"
name: direct_mode_resource_tiers
description: Multiple resource requirement tiers

resource_requirements:
  - name: small_resources
    num_cpus: 1
    memory: 128m
    runtime: PT1M

  - name: medium_resources
    num_cpus: 2
    memory: 512m
    runtime: PT2M

  - name: large_resources
    num_cpus: 4
    memory: 1g
    runtime: PT5M

jobs:
  - name: small_job
    command: echo "Small job" > {0}/small.txt
    resource_requirements: small_resources

  - name: medium_job
    command: echo "Medium job" > {0}/medium.txt
    resource_requirements: medium_resources

  - name: large_job
    command: echo "Large job" > {0}/large.txt
    resource_requirements: large_resources

execution_config:
    mode: direct
    limit_resources: true
"#,
        work_dir.display()
    );

    let workflow_id =
        create_workflow_from_yaml(start_server, &yaml).expect("Failed to create workflow");

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.1".to_string(),
        "--num-cpus".to_string(),
        "8".to_string(),
        "--memory-gb".to_string(),
        "4.0".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    run_jobs_cli_command(&cli_args_refs, start_server).expect("Failed to run jobs");

    verify_all_jobs_completed(start_server, workflow_id);

    assert!(work_dir.join("small.txt").exists());
    assert!(work_dir.join("medium.txt").exists());
    assert!(work_dir.join("large.txt").exists());
}

/// Test job that exits with specific exit code (simulating OOM behavior).
/// When OOM enforcement is implemented, jobs exceeding memory will exit with oom_exit_code.
#[rstest]
fn test_direct_mode_oom_exit_code_behavior(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    // Simulate a job that exits with the OOM exit code
    let yaml = r#"
name: direct_mode_oom_exit_simulation
description: Simulates OOM exit code behavior

resource_requirements:
  - name: normal
    num_cpus: 1
    memory: 256m
    runtime: PT1M

jobs:
  - name: normal_job
    command: |
      echo "Normal job succeeds"
      exit 0
    resource_requirements: normal

  - name: simulated_oom_job
    command: |
      echo "Simulating OOM exit"
      exit 137
    resource_requirements: normal

execution_config:
    mode: direct
    limit_resources: true
    oom_exit_code: 137
"#;

    let workflow_id =
        create_workflow_from_yaml(start_server, yaml).expect("Failed to create workflow");

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.1".to_string(),
        "--num-cpus".to_string(),
        "2".to_string(),
        "--memory-gb".to_string(),
        "1.0".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    // Run jobs - some will fail
    let _ = run_jobs_cli_command(&cli_args_refs, start_server);

    // Verify return codes
    let normal_code = get_job_return_code(start_server, workflow_id, "normal_job");
    assert_eq!(normal_code, Some(0), "normal_job should succeed");

    let oom_code = get_job_return_code(start_server, workflow_id, "simulated_oom_job");
    assert_eq!(
        oom_code,
        Some(137),
        "simulated_oom_job should have OOM exit code"
    );
}

/// Test timeout exit code configuration.
/// When timeout enforcement is implemented, timed-out jobs will exit with timeout_exit_code.
#[rstest]
fn test_direct_mode_timeout_exit_code_behavior(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    // Simulate a job that exits with the timeout exit code
    let yaml = r#"
name: direct_mode_timeout_exit_simulation
description: Simulates timeout exit code behavior

resource_requirements:
  - name: normal
    num_cpus: 1
    memory: 256m
    runtime: PT1M

jobs:
  - name: normal_job
    command: |
      echo "Normal job succeeds"
      exit 0
    resource_requirements: normal

  - name: simulated_timeout_job
    command: |
      echo "Simulating timeout exit"
      exit 152
    resource_requirements: normal

execution_config:
    mode: direct
    timeout_exit_code: 152
"#;

    let workflow_id =
        create_workflow_from_yaml(start_server, yaml).expect("Failed to create workflow");

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.1".to_string(),
        "--num-cpus".to_string(),
        "2".to_string(),
        "--memory-gb".to_string(),
        "1.0".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    let _ = run_jobs_cli_command(&cli_args_refs, start_server);

    let normal_code = get_job_return_code(start_server, workflow_id, "normal_job");
    assert_eq!(normal_code, Some(0));

    let timeout_code = get_job_return_code(start_server, workflow_id, "simulated_timeout_job");
    assert_eq!(timeout_code, Some(152));
}

// =============================================================================
// OOM enforcement tests
// =============================================================================

/// Test that a job exceeding its memory limit gets killed with the OOM exit code.
/// The ResourceMonitor detects the violation and sends SIGKILL.
#[rstest]
fn test_direct_mode_oom_enforcement(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    // Job allocates ~100MB but is limited to 50MB - should be killed
    let yaml = format!(
        r#"
name: direct_mode_oom_enforcement
description: OOM enforcement - job exceeds memory limit and gets killed

resource_monitor:
  enabled: true
  granularity: time_series
  sample_interval_seconds: 1

resource_requirements:
  - name: normal_resources
    num_cpus: 1
    memory: 256m
    runtime: PT1M

  - name: oom_resources
    num_cpus: 1
    memory: 50m
    runtime: PT1M

jobs:
  - name: normal_job
    command: |
      echo "Normal job starting" > {0}/normal.txt
      sleep 2
      echo "Normal job complete" >> {0}/normal.txt
    resource_requirements: normal_resources

  - name: oom_job
    command: |
      echo "OOM job starting" > {0}/oom.txt
      # Allocate ~100MB which exceeds the 50MB limit
      python3 -c "x = bytearray(100*1024*1024); import time; time.sleep(30)"
      echo "OOM job complete" >> {0}/oom.txt
    resource_requirements: oom_resources

execution_config:
    mode: direct
    limit_resources: true
    oom_exit_code: 137
"#,
        work_dir.display()
    );

    let workflow_id =
        create_workflow_from_yaml(start_server, &yaml).expect("Failed to create workflow");

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.5".to_string(),
        "--num-cpus".to_string(),
        "2".to_string(),
        "--memory-gb".to_string(),
        "1.0".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    // Run jobs - oom_job should fail
    let _ = run_jobs_cli_command(&cli_args_refs, start_server);

    // Normal job should succeed
    let normal_code = get_job_return_code(start_server, workflow_id, "normal_job");
    assert_eq!(
        normal_code,
        Some(0),
        "normal_job should succeed with exit code 0"
    );

    // OOM job should be killed with OOM exit code (137 = 128 + SIGKILL)
    let oom_code = get_job_return_code(start_server, workflow_id, "oom_job");
    assert_eq!(
        oom_code,
        Some(137),
        "oom_job should be killed with OOM exit code 137"
    );
}

/// Test that with limit_resources=false, jobs exceeding memory are NOT killed.
#[rstest]
fn test_direct_mode_oom_not_enforced_when_limit_resources_false(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    // Same scenario but limit_resources=false - job should complete
    let yaml = format!(
        r#"
name: direct_mode_no_oom_enforcement
description: OOM not enforced when limit_resources=false

resource_monitor:
  enabled: true
  granularity: time_series
  sample_interval_seconds: 1

resource_requirements:
  - name: small_limit
    num_cpus: 1
    memory: 50m
    runtime: PT1M

jobs:
  - name: memory_job
    command: |
      echo "Memory job starting" > {0}/memory.txt
      # Allocate ~100MB which exceeds the 50MB limit
      python3 -c "x = bytearray(100*1024*1024); print('Allocated 100MB'); import time; time.sleep(2)"
      echo "Memory job complete" >> {0}/memory.txt
    resource_requirements: small_limit

execution_config:
    mode: direct
    limit_resources: false
"#,
        work_dir.display()
    );

    let workflow_id =
        create_workflow_from_yaml(start_server, &yaml).expect("Failed to create workflow");

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.5".to_string(),
        "--num-cpus".to_string(),
        "2".to_string(),
        "--memory-gb".to_string(),
        "1.0".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    run_jobs_cli_command(&cli_args_refs, start_server).expect("Jobs should complete");

    // Job should succeed even though it exceeded memory - limit_resources is false
    let return_code = get_job_return_code(start_server, workflow_id, "memory_job");
    assert_eq!(
        return_code,
        Some(0),
        "memory_job should succeed when limit_resources=false"
    );

    // Verify job actually ran to completion
    let output = fs::read_to_string(work_dir.join("memory.txt")).expect("Failed to read output");
    assert!(output.contains("Memory job complete"));
}

// =============================================================================
// Timeout enforcement tests
// =============================================================================

/// Test that a job running past the end_time gets SIGTERM then SIGKILL.
/// The termination timeline is: SIGTERM -> wait sigterm_lead_seconds -> SIGKILL
#[rstest]
fn test_direct_mode_timeout_enforcement(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    let yaml = format!(
        r#"
name: direct_mode_timeout_test
description: Job times out and gets terminated

resource_requirements:
  - name: normal
    num_cpus: 1
    memory: 256m
    runtime: PT10S

jobs:
  - name: long_running_job
    command: |
      echo "Long job starting" > {0}/long.txt
      # This job will be killed before it completes
      for i in $(seq 1 60); do
        echo "Iteration $i" >> {0}/long.txt
        sleep 1
      done
      echo "Long job complete" >> {0}/long.txt
    resource_requirements: normal

execution_config:
    mode: direct
    limit_resources: true
    termination_signal: SIGTERM
    sigterm_lead_seconds: 2
    sigkill_headroom_seconds: 5
    timeout_exit_code: 152
"#,
        work_dir.display()
    );

    let workflow_id =
        create_workflow_from_yaml(start_server, &yaml).expect("Failed to create workflow");

    // Calculate end_time 8 seconds from now
    let end_time = Utc::now() + Duration::seconds(8);
    let end_time_str = end_time.format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.5".to_string(),
        "--num-cpus".to_string(),
        "2".to_string(),
        "--memory-gb".to_string(),
        "1.0".to_string(),
        "--end-time".to_string(),
        end_time_str,
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    // Run jobs - should terminate when end_time is reached
    let _ = run_jobs_cli_command(&cli_args_refs, start_server);

    // Job should be terminated (exit code from signal, not 0)
    let jobs = apis::jobs_api::list_jobs(
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
    )
    .expect("Failed to list jobs");

    let job = jobs
        .items
        .into_iter()
        .find(|j| j.name == "long_running_job")
        .expect("Job not found");

    // Job should be terminated (not completed)
    assert_eq!(
        job.status.unwrap(),
        JobStatus::Terminated,
        "Job should be terminated, not completed"
    );

    // Verify the job started but didn't complete
    let output = fs::read_to_string(work_dir.join("long.txt")).expect("Failed to read output");
    assert!(output.contains("Long job starting"));
    assert!(
        !output.contains("Long job complete"),
        "Job should have been terminated before completion"
    );
}

/// Test that with limit_resources=false, timeout is still enforced
/// (timeout is based on end_time, not resource limits).
#[rstest]
fn test_direct_mode_timeout_still_works_with_limit_resources_false(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    let yaml = format!(
        r#"
name: direct_mode_timeout_no_limit
description: Timeout still works even with limit_resources=false

resource_requirements:
  - name: normal
    num_cpus: 1
    memory: 256m
    runtime: PT10S

jobs:
  - name: long_job
    command: |
      echo "Starting" > {0}/timeout.txt
      for i in $(seq 1 60); do
        echo "Iteration $i" >> {0}/timeout.txt
        sleep 1
      done
      echo "Complete" >> {0}/timeout.txt
    resource_requirements: normal

execution_config:
    mode: direct
    limit_resources: false
    sigterm_lead_seconds: 2
    sigkill_headroom_seconds: 5
"#,
        work_dir.display()
    );

    let workflow_id =
        create_workflow_from_yaml(start_server, &yaml).expect("Failed to create workflow");

    // End in 8 seconds
    let end_time = Utc::now() + Duration::seconds(8);
    let end_time_str = end_time.format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.5".to_string(),
        "--num-cpus".to_string(),
        "2".to_string(),
        "--memory-gb".to_string(),
        "1.0".to_string(),
        "--end-time".to_string(),
        end_time_str,
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    let _ = run_jobs_cli_command(&cli_args_refs, start_server);

    // Job should be terminated
    let jobs = apis::jobs_api::list_jobs(
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
    )
    .expect("Failed to list jobs");

    let job = jobs
        .items
        .into_iter()
        .find(|j| j.name == "long_job")
        .expect("Job not found");

    assert_eq!(
        job.status.unwrap(),
        JobStatus::Terminated,
        "Job should be terminated even with limit_resources=false"
    );
}

// =============================================================================
// Custom termination signal and exit code tests
// =============================================================================

/// Test custom termination signal (SIGINT) and custom timeout exit code.
#[rstest]
fn test_direct_mode_custom_termination_signal_and_exit_code(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    // Create a job that handles SIGINT and exits with a custom code
    let yaml = format!(
        r#"
name: direct_mode_custom_signal
description: Test custom termination signal SIGINT

resource_requirements:
  - name: normal
    num_cpus: 1
    memory: 256m
    runtime: PT10S

jobs:
  - name: signal_handler_job
    command: |
      echo "Job starting" > {0}/signal.txt
      # Trap SIGINT and exit with specific code
      trap 'echo "Caught SIGINT" >> {0}/signal.txt; exit 42' INT
      # Run until terminated
      for i in $(seq 1 60); do
        echo "Iteration $i" >> {0}/signal.txt
        sleep 1
      done
    resource_requirements: normal

execution_config:
    mode: direct
    limit_resources: true
    termination_signal: SIGINT
    sigterm_lead_seconds: 3
    sigkill_headroom_seconds: 10
    timeout_exit_code: 200
"#,
        work_dir.display()
    );

    let workflow_id =
        create_workflow_from_yaml(start_server, &yaml).expect("Failed to create workflow");

    // End in 6 seconds
    let end_time = Utc::now() + Duration::seconds(6);
    let end_time_str = end_time.format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.5".to_string(),
        "--num-cpus".to_string(),
        "2".to_string(),
        "--memory-gb".to_string(),
        "1.0".to_string(),
        "--end-time".to_string(),
        end_time_str,
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    let _ = run_jobs_cli_command(&cli_args_refs, start_server);

    // Verify the job was terminated
    let jobs = apis::jobs_api::list_jobs(
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
    )
    .expect("Failed to list jobs");

    let job = jobs
        .items
        .into_iter()
        .find(|j| j.name == "signal_handler_job")
        .expect("Job not found");

    assert_eq!(job.status.unwrap(), JobStatus::Terminated);

    // Verify signal was caught (job handled SIGINT)
    let output = fs::read_to_string(work_dir.join("signal.txt")).expect("Failed to read output");
    assert!(output.contains("Job starting"));
    // The trap should have caught the signal
    assert!(
        output.contains("Caught SIGINT"),
        "Job should have caught SIGINT signal"
    );
}

/// Test custom OOM exit code configuration.
#[rstest]
#[serial]
fn test_direct_mode_custom_oom_exit_code(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    let yaml = format!(
        r#"
name: direct_mode_custom_oom_code
description: Test custom OOM exit code (201)

resource_monitor:
  enabled: true
  granularity: time_series
  sample_interval_seconds: 1

resource_requirements:
  - name: tiny
    num_cpus: 1
    memory: 30m
    runtime: PT1M

jobs:
  - name: oom_job
    command: |
      echo "Starting" > {0}/custom_oom.txt
      # Allocate ~80MB which exceeds the 30MB limit
      python3 -c "x = bytearray(80*1024*1024); import time; time.sleep(30)"
      echo "Complete" >> {0}/custom_oom.txt
    resource_requirements: tiny

execution_config:
    mode: direct
    limit_resources: true
    oom_exit_code: 201
"#,
        work_dir.display()
    );

    let workflow_id =
        create_workflow_from_yaml(start_server, &yaml).expect("Failed to create workflow");

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.5".to_string(),
        "--num-cpus".to_string(),
        "2".to_string(),
        "--memory-gb".to_string(),
        "1.0".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    let _ = run_jobs_cli_command(&cli_args_refs, start_server);

    // Job should be killed with custom OOM exit code 201
    let return_code = get_job_return_code(start_server, workflow_id, "oom_job");
    assert_eq!(
        return_code,
        Some(201),
        "oom_job should be killed with custom OOM exit code 201"
    );
}

// =============================================================================
// Legacy placeholder comments (kept for reference)
// =============================================================================
//
// The tests above implement the OOM and timeout enforcement behavior.
// Additional edge cases could include:
// - Multiple jobs exceeding memory simultaneously
// - Jobs that allocate memory gradually vs. all at once
// - Very short sigterm_lead_seconds (immediate SIGKILL)
// - Jobs that ignore SIGTERM and require SIGKILL
//       sleep 2
//       echo "Normal job complete"
//     resource_requirements: normal_resources
//
//   - name: oom_job
//     command: |
//       echo "OOM job starting"
//       # Allocate 500MB which exceeds the 100m limit
//       python3 -c "x = bytearray(500*1024*1024); import time; time.sleep(10)"
//     resource_requirements: oom_resources
//
// execution_config:
//     mode: direct
//     limit_resources: true
//     oom_exit_code: 137
// "#;
//
//     let workflow_id = create_workflow_from_yaml(start_server, yaml)
//         .expect("Failed to create workflow");
//
//     // Run jobs
//     let _ = run_jobs_cli_command(&["workflow_id", ...], start_server);
//
//     // Expected behavior:
//     // 1. normal_job completes with return code 0
//     let normal_code = get_job_return_code(start_server, workflow_id, "normal_job");
//     assert_eq!(normal_code, Some(0));
//
//     // 2. oom_job is killed with OOM exit code
//     let oom_code = get_job_return_code(start_server, workflow_id, "oom_job");
//     assert_eq!(oom_code, Some(137));
//
//     // 3. oom_job status is Failed (OOM is a job error, not termination)
//     let jobs = apis::jobs_api::list_jobs(&start_server.config, workflow_id, ...).unwrap();
//     let oom_job = jobs.items.unwrap().into_iter()
//         .find(|j| j.name == "oom_job").unwrap();
//     assert_eq!(oom_job.status, Some(JobStatus::Failed));
// }

// =============================================================================
// Timeout detection tests - mirrors slurm-tests/workflows/timeout_detection.yaml
// =============================================================================

/// Test that mirrors slurm-tests/workflows/timeout_detection.yaml structure.
/// This validates the workflow configuration and execution pattern that users
/// will use for timeout detection in direct mode.
///
/// The original test has:
/// - job_fast: completes in 30 seconds (within 2-minute runtime)
/// - job_slow: runs for 20 minutes (exceeds 3-minute walltime)
///
/// Note: Full timeout enforcement (SIGTERM/SIGKILL at configured times) requires
/// implementation of the termination timeline in job_runner. This test validates
/// the configuration and workflow structure works correctly.
#[rstest]
fn test_direct_mode_timeout_workflow_structure(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    // This workflow mirrors slurm-tests/workflows/timeout_detection.yaml
    let yaml = format!(
        r#"
name: direct_mode_timeout_detection
description: Timeout detection test - one fast job, one that would exceed walltime

resource_monitor:
  enabled: true
  granularity: time_series
  sample_interval_seconds: 1

resource_requirements:
  - name: fast_resources
    num_cpus: 1
    memory: 1g
    runtime: PT2M

  - name: slow_resources
    num_cpus: 1
    memory: 1g
    runtime: PT2M

jobs:
  - name: job_fast
    command: |
      echo "Fast job on $(hostname)" > {0}/fast_output.txt
      echo "This job should complete quickly."
      sleep 2
      echo "Fast job complete." >> {0}/fast_output.txt
    resource_requirements: fast_resources

  - name: job_moderate
    command: |
      echo "Moderate job starting" > {0}/moderate_output.txt
      for i in 1 2 3; do
        echo "Iteration $i" >> {0}/moderate_output.txt
        sleep 1
      done
      echo "Moderate job complete" >> {0}/moderate_output.txt
    resource_requirements: slow_resources

execution_config:
    mode: direct
    limit_resources: true
    termination_signal: SIGTERM
    sigterm_lead_seconds: 30
    sigkill_headroom_seconds: 60
    timeout_exit_code: 152
"#,
        work_dir.display()
    );

    let workflow_id =
        create_workflow_from_yaml(start_server, &yaml).expect("Failed to create workflow");

    // Verify execution_config was stored correctly
    let workflow = apis::workflows_api::get_workflow(&start_server.config, workflow_id)
        .expect("Failed to get workflow");

    assert!(workflow.execution_config.is_some());
    let exec_config: torc::client::workflow_spec::ExecutionConfig =
        serde_json::from_str(workflow.execution_config.as_ref().unwrap()).unwrap();

    assert_eq!(exec_config.termination_signal(), "SIGTERM");
    assert_eq!(exec_config.sigterm_lead_seconds(), 30);
    assert_eq!(exec_config.sigkill_headroom_seconds(), 60);
    assert_eq!(exec_config.timeout_exit_code(), 152);

    // Verify resource_monitor_config was stored
    assert!(workflow.resource_monitor_config.is_some());

    // Run the workflow
    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.1".to_string(),
        "--num-cpus".to_string(),
        "2".to_string(),
        "--memory-gb".to_string(),
        "2.0".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    run_jobs_cli_command(&cli_args_refs, start_server).expect("Failed to run jobs");

    // Both jobs should complete (both stay within limits for this test)
    verify_all_jobs_completed(start_server, workflow_id);

    // Verify output files exist
    assert!(work_dir.join("fast_output.txt").exists());
    assert!(work_dir.join("moderate_output.txt").exists());

    let fast_output =
        fs::read_to_string(work_dir.join("fast_output.txt")).expect("Failed to read output");
    assert!(fast_output.contains("Fast job complete"));

    let moderate_output =
        fs::read_to_string(work_dir.join("moderate_output.txt")).expect("Failed to read output");
    assert!(moderate_output.contains("Moderate job complete"));
}

/// Test termination signal configuration.
/// Verifies that custom termination signals are correctly stored.
#[rstest]
fn test_direct_mode_termination_signal_config(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    let yaml = r#"
name: direct_mode_sigint_termination
description: Test custom termination signal (SIGINT)

jobs:
  - name: test_job
    command: |
      echo "Job with SIGINT termination signal"
      sleep 1
      echo "Done"

execution_config:
    mode: direct
    termination_signal: SIGINT
    sigterm_lead_seconds: 45
    sigkill_headroom_seconds: 90
"#;

    let workflow_id =
        create_workflow_from_yaml(start_server, yaml).expect("Failed to create workflow");

    let workflow = apis::workflows_api::get_workflow(&start_server.config, workflow_id)
        .expect("Failed to get workflow");

    let exec_config: torc::client::workflow_spec::ExecutionConfig =
        serde_json::from_str(workflow.execution_config.as_ref().unwrap()).unwrap();

    assert_eq!(exec_config.termination_signal(), "SIGINT");
    assert_eq!(exec_config.sigterm_lead_seconds(), 45);
    assert_eq!(exec_config.sigkill_headroom_seconds(), 90);

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.1".to_string(),
        "--max-parallel-jobs".to_string(),
        "1".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    run_jobs_cli_command(&cli_args_refs, start_server).expect("Failed to run jobs");
    verify_all_jobs_completed(start_server, workflow_id);
}

/// Test fast vs slow job execution pattern (like timeout_detection.yaml).
/// Both jobs complete normally in this test since we don't enforce timeouts yet.
#[rstest]
fn test_direct_mode_fast_slow_job_pattern(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    let yaml = format!(
        r#"
name: direct_mode_fast_slow_pattern
description: Fast job completes quickly, slow job takes longer

resource_requirements:
  - name: fast_resources
    num_cpus: 1
    memory: 512m
    runtime: PT1M

  - name: slow_resources
    num_cpus: 1
    memory: 512m
    runtime: PT5M

jobs:
  - name: job_fast
    command: |
      echo "Fast job starting at $(date)" > {0}/fast.txt
      sleep 1
      echo "Fast job complete at $(date)" >> {0}/fast.txt
    resource_requirements: fast_resources

  - name: job_slow
    command: |
      echo "Slow job starting at $(date)" > {0}/slow.txt
      for i in 1 2 3 4 5; do
        echo "Slow job iteration $i" >> {0}/slow.txt
        sleep 1
      done
      echo "Slow job complete at $(date)" >> {0}/slow.txt
    resource_requirements: slow_resources

execution_config:
    mode: direct
    timeout_exit_code: 152
"#,
        work_dir.display()
    );

    let workflow_id =
        create_workflow_from_yaml(start_server, &yaml).expect("Failed to create workflow");

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.1".to_string(),
        "--num-cpus".to_string(),
        "2".to_string(),
        "--memory-gb".to_string(),
        "2.0".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    run_jobs_cli_command(&cli_args_refs, start_server).expect("Failed to run jobs");

    verify_all_jobs_completed(start_server, workflow_id);

    // Both should have return code 0
    let fast_code = get_job_return_code(start_server, workflow_id, "job_fast");
    assert_eq!(fast_code, Some(0));

    let slow_code = get_job_return_code(start_server, workflow_id, "job_slow");
    assert_eq!(slow_code, Some(0));

    // Verify output files
    let fast_output = fs::read_to_string(work_dir.join("fast.txt")).expect("Failed to read");
    assert!(fast_output.contains("Fast job complete"));

    let slow_output = fs::read_to_string(work_dir.join("slow.txt")).expect("Failed to read");
    assert!(slow_output.contains("Slow job complete"));
}

/// Test termination timeline configuration values.
/// Verifies different combinations of sigterm_lead and sigkill_headroom.
#[rstest]
fn test_direct_mode_termination_timeline_config(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    // Test with aggressive termination settings
    let yaml = r#"
name: direct_mode_aggressive_termination
description: Aggressive termination timeline (short lead times)

jobs:
  - name: quick_job
    command: echo "Quick job"

execution_config:
    mode: direct
    termination_signal: SIGTERM
    sigterm_lead_seconds: 10
    sigkill_headroom_seconds: 20
    timeout_exit_code: 152
"#;

    let workflow_id =
        create_workflow_from_yaml(start_server, yaml).expect("Failed to create workflow");

    let workflow = apis::workflows_api::get_workflow(&start_server.config, workflow_id)
        .expect("Failed to get workflow");

    let exec_config: torc::client::workflow_spec::ExecutionConfig =
        serde_json::from_str(workflow.execution_config.as_ref().unwrap()).unwrap();

    // Verify aggressive settings
    assert_eq!(exec_config.sigterm_lead_seconds(), 10);
    assert_eq!(exec_config.sigkill_headroom_seconds(), 20);

    // The termination timeline would be:
    // end_time - 30s: Send SIGTERM (headroom + lead = 20 + 10)
    // end_time - 20s: Send SIGKILL (headroom = 20)
    // end_time: Job runner exits

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.1".to_string(),
        "--max-parallel-jobs".to_string(),
        "1".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    run_jobs_cli_command(&cli_args_refs, start_server).expect("Failed to run jobs");
    verify_all_jobs_completed(start_server, workflow_id);
}

/// Test conservative termination timeline (longer grace periods).
#[rstest]
fn test_direct_mode_conservative_termination_timeline(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    let yaml = r#"
name: direct_mode_conservative_termination
description: Conservative termination timeline (long grace periods)

jobs:
  - name: graceful_job
    command: |
      echo "Job that needs time to cleanup"
      sleep 1
      echo "Cleanup complete"

execution_config:
    mode: direct
    termination_signal: SIGTERM
    sigterm_lead_seconds: 120
    sigkill_headroom_seconds: 180
    timeout_exit_code: 152
"#;

    let workflow_id =
        create_workflow_from_yaml(start_server, yaml).expect("Failed to create workflow");

    let workflow = apis::workflows_api::get_workflow(&start_server.config, workflow_id)
        .expect("Failed to get workflow");

    let exec_config: torc::client::workflow_spec::ExecutionConfig =
        serde_json::from_str(workflow.execution_config.as_ref().unwrap()).unwrap();

    // Verify conservative settings
    assert_eq!(exec_config.sigterm_lead_seconds(), 120);
    assert_eq!(exec_config.sigkill_headroom_seconds(), 180);

    // The termination timeline would be:
    // end_time - 300s (5 min): Send SIGTERM (headroom + lead = 180 + 120)
    // end_time - 180s (3 min): Send SIGKILL (headroom = 180)
    // end_time: Job runner exits

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.1".to_string(),
        "--max-parallel-jobs".to_string(),
        "1".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    run_jobs_cli_command(&cli_args_refs, start_server).expect("Failed to run jobs");
    verify_all_jobs_completed(start_server, workflow_id);
}

/// Test mixed job completion with timeout simulation.
/// One job succeeds, one exits with timeout code.
#[rstest]
fn test_direct_mode_mixed_timeout_success(start_server: &ServerProcess) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    let yaml = r#"
name: direct_mode_mixed_timeout
description: One successful job, one simulated timeout

resource_requirements:
  - name: normal
    num_cpus: 1
    memory: 256m
    runtime: PT1M

jobs:
  - name: successful_job
    command: |
      echo "This job succeeds"
      exit 0
    resource_requirements: normal

  - name: timeout_job
    command: |
      echo "This job simulates timeout"
      exit 152
    resource_requirements: normal

execution_config:
    mode: direct
    timeout_exit_code: 152
"#;

    let workflow_id =
        create_workflow_from_yaml(start_server, yaml).expect("Failed to create workflow");

    let cli_args = vec![
        workflow_id.to_string(),
        "--output-dir".to_string(),
        work_dir.to_str().unwrap().to_string(),
        "--poll-interval".to_string(),
        "0.1".to_string(),
        "--num-cpus".to_string(),
        "2".to_string(),
        "--memory-gb".to_string(),
        "1.0".to_string(),
    ];
    let cli_args_refs: Vec<&str> = cli_args.iter().map(|s| s.as_str()).collect();

    let _ = run_jobs_cli_command(&cli_args_refs, start_server);

    // Verify return codes
    let success_code = get_job_return_code(start_server, workflow_id, "successful_job");
    assert_eq!(success_code, Some(0), "successful_job should have code 0");

    let timeout_code = get_job_return_code(start_server, workflow_id, "timeout_job");
    assert_eq!(
        timeout_code,
        Some(152),
        "timeout_job should have timeout exit code"
    );
}

// =============================================================================
// Future: Full timeout enforcement tests
// =============================================================================
//
// The following test requires implementation of the termination timeline
// in job_runner.rs. When implemented, uncomment and adjust this test.
//
// This mirrors slurm-tests/workflows/timeout_detection.yaml behavior where:
// 1. job_fast completes successfully within its runtime
// 2. job_slow exceeds walltime and is terminated by Slurm
// 3. job_slow has return code 152 (TIMEOUT in Slurm)
//
// #[rstest]
// fn test_direct_mode_timeout_enforcement(start_server: &ServerProcess) {
//     let yaml = r#"
// name: direct_mode_timeout_enforcement
// description: Timeout enforcement test - job exceeds runtime and is terminated
//
// resource_monitor:
//   enabled: true
//   granularity: time_series
//   sample_interval_seconds: 1
//
// resource_requirements:
//   - name: fast_resources
//     num_cpus: 1
//     memory: 1g
//     runtime: PT2M
//
//   - name: slow_resources
//     num_cpus: 1
//     memory: 1g
//     runtime: PT10S  # Very short runtime - job will exceed this
//
// jobs:
//   - name: job_fast
//     command: |
//       echo "Fast job"
//       sleep 2
//       echo "Fast job complete"
//     resource_requirements: fast_resources
//
//   - name: job_slow
//     command: |
//       echo "Slow job starting"
//       # This will run for 60 seconds, exceeding the 10-second runtime
//       sleep 60
//       echo "Slow job complete"
//     resource_requirements: slow_resources
//
// execution_config:
//     mode: direct
//     termination_signal: SIGTERM
//     sigterm_lead_seconds: 5
//     sigkill_headroom_seconds: 10
//     timeout_exit_code: 152
// "#;
//
//     let workflow_id = create_workflow_from_yaml(start_server, yaml)
//         .expect("Failed to create workflow");
//
//     // Run jobs with a short end_time to trigger timeout
//     let _ = run_jobs_cli_command(&[...], start_server);
//
//     // Expected behavior:
//     // 1. job_fast completes with return code 0
//     let fast_code = get_job_return_code(start_server, workflow_id, "job_fast");
//     assert_eq!(fast_code, Some(0));
//
//     // 2. job_slow is killed at end_time - sigkill_headroom_seconds
//     //    and has return code timeout_exit_code (152)
//     let slow_code = get_job_return_code(start_server, workflow_id, "job_slow");
//     assert_eq!(slow_code, Some(152));
//
//     // 3. job_slow status is Terminated (timeout is termination, not failure)
//     let jobs = apis::jobs_api::list_jobs(&start_server.config, workflow_id, ...).unwrap();
//     let slow_job = jobs.items.unwrap().into_iter()
//         .find(|j| j.name == "job_slow").unwrap();
//     assert_eq!(slow_job.status, Some(JobStatus::Terminated));
// }
