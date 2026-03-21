mod common;

use common::{
    ServerProcess, create_minimal_resources_workflow, create_test_workflow, run_cli_with_json,
    start_server,
};
use rstest::rstest;
use serde_json::json;
use serial_test::serial;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use torc::client::config::TorcConfig;
use torc::client::default_api;
use torc::client::hpc::common::HpcJobStatus;
use torc::client::hpc::hpc_interface::HpcInterface;
use torc::client::hpc::slurm_interface::SlurmInterface;
use torc::client::workflow_manager::WorkflowManager;
use torc::models;

#[rstest]
fn test_slurm_interface_new() {
    let interface = SlurmInterface::new();
    assert!(interface.is_ok(), "Failed to create SlurmInterface");
}

#[rstest]
#[serial(slurm)]
fn test_slurm_interface_ignores_torc_username_override() {
    let original_torc_username = env::var("TORC_USERNAME").ok();
    let original_user = env::var("USER").ok();
    let original_username = env::var("USERNAME").ok();

    unsafe {
        env::set_var("TORC_USERNAME", "torc-api-user");
        env::set_var("USER", "scheduler-user");
        env::remove_var("USERNAME");
    }

    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");
    let scheduler_user = interface.get_user().expect("Failed to get scheduler user");
    assert_eq!(scheduler_user, "scheduler-user");

    match original_torc_username {
        Some(value) => unsafe { env::set_var("TORC_USERNAME", value) },
        None => unsafe { env::remove_var("TORC_USERNAME") },
    }
    match original_user {
        Some(value) => unsafe { env::set_var("USER", value) },
        None => unsafe { env::remove_var("USER") },
    }
    match original_username {
        Some(value) => unsafe { env::set_var("USERNAME", value) },
        None => unsafe { env::remove_var("USERNAME") },
    }
}

#[rstest]
#[serial(slurm)]
fn test_submit_job_success() {
    cleanup_fake_slurm_state();
    setup_fake_slurm_commands();

    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    // Create a temporary submission script
    let temp_dir = env::temp_dir();
    let script_path = temp_dir.join("test_submit.sh");
    fs::write(&script_path, "#!/bin/bash\necho 'test job'\n").expect("Failed to write test script");

    let result = interface.submit(&script_path);
    assert!(result.is_ok(), "Submit failed: {:?}", result.err());

    let (return_code, job_id, stderr) = result.unwrap();
    assert_eq!(
        return_code, 0,
        "Expected return code 0, got {}",
        return_code
    );
    assert!(!job_id.is_empty(), "Job ID should not be empty");
    assert_eq!(stderr, "", "Stderr should be empty on success");

    // Verify job ID is numeric
    assert!(
        job_id.parse::<i32>().is_ok(),
        "Job ID should be numeric: {}",
        job_id
    );

    cleanup_fake_slurm_state();
}

#[rstest]
#[serial(slurm)]
#[ignore] // This is a slow test and we don't need to continue running it.
fn test_submit_job_failure() {
    cleanup_fake_slurm_state();
    setup_fake_slurm_commands();

    // Set failure mode
    unsafe {
        env::set_var("TORC_FAKE_SBATCH_FAIL", "1");
    }

    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    let temp_dir = env::temp_dir();
    let script_path = temp_dir.join("test_submit_fail.sh");
    fs::write(&script_path, "#!/bin/bash\necho 'test job'\n").expect("Failed to write test script");

    let result = interface.submit(&script_path);
    assert!(result.is_ok(), "Submit should return Ok even on failure");

    let (return_code, job_id, _stderr) = result.unwrap();
    // Note: Due to retry logic (6 retries), this may still succeed or fail
    // We just verify the interface handles the error gracefully
    if return_code != 0 {
        assert_eq!(job_id, "", "Job ID should be empty on failure");
    }

    cleanup_fake_slurm_state();
}

#[rstest]
#[serial(slurm)]
fn test_get_status_pending_job() {
    cleanup_fake_slurm_state();
    setup_fake_slurm_commands();

    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    // Submit a job first
    let temp_dir = env::temp_dir();
    let script_path = temp_dir.join("test_status_pending.sh");
    fs::write(&script_path, "#!/bin/bash\necho 'test'\n").expect("Failed to write test script");

    let (_, job_id, _) = interface
        .submit(&script_path)
        .expect("Failed to submit job");

    // Get status
    let status = interface.get_status(&job_id);
    assert!(status.is_ok(), "Failed to get status: {:?}", status.err());

    let job_info = status.unwrap();
    assert_eq!(job_info.job_id, job_id);
    assert_eq!(job_info.status, HpcJobStatus::Queued); // PENDING maps to Queued

    cleanup_fake_slurm_state();
}

#[rstest]
#[serial(slurm)]
fn test_get_status_running_job() {
    cleanup_fake_slurm_state();
    setup_fake_slurm_commands();

    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    // Submit a job first
    let temp_dir = env::temp_dir();
    let script_path = temp_dir.join("test_status_running.sh");
    fs::write(&script_path, "#!/bin/bash\necho 'test'\n").expect("Failed to write test script");

    let (_, job_id, _) = interface
        .submit(&script_path)
        .expect("Failed to submit job");

    // Change state to RUNNING
    unsafe {
        env::set_var("TORC_FAKE_SQUEUE_STATE", "RUNNING");
    }

    let status = interface.get_status(&job_id);
    assert!(status.is_ok(), "Failed to get status: {:?}", status.err());

    let job_info = status.unwrap();
    assert_eq!(job_info.job_id, job_id);
    assert_eq!(job_info.status, HpcJobStatus::Running);

    cleanup_fake_slurm_state();
}

#[rstest]
#[serial(slurm)]
fn test_get_status_completed_job() {
    cleanup_fake_slurm_state();
    setup_fake_slurm_commands();

    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    // Submit a job first
    let temp_dir = env::temp_dir();
    let script_path = temp_dir.join("test_status_completed.sh");
    fs::write(&script_path, "#!/bin/bash\necho 'test'\n").expect("Failed to write test script");

    let (_, job_id, _) = interface
        .submit(&script_path)
        .expect("Failed to submit job");

    // Change state to COMPLETED
    unsafe {
        env::set_var("TORC_FAKE_SQUEUE_STATE", "COMPLETED");
    }

    let status = interface.get_status(&job_id);
    assert!(status.is_ok(), "Failed to get status: {:?}", status.err());

    let job_info = status.unwrap();
    assert_eq!(job_info.job_id, job_id);
    assert_eq!(job_info.status, HpcJobStatus::Complete);

    cleanup_fake_slurm_state();
}

#[rstest]
#[serial(slurm)]
fn test_get_status_invalid_job_id() {
    cleanup_fake_slurm_state();
    setup_fake_slurm_commands();

    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    // Try to get status of non-existent job
    let status = interface.get_status("999999");
    assert!(status.is_ok(), "Should return Ok for invalid job ID");

    let job_info = status.unwrap();
    assert_eq!(job_info.job_id, "");
    assert_eq!(job_info.name, "");
    assert_eq!(job_info.status, HpcJobStatus::None);

    cleanup_fake_slurm_state();
}

#[rstest]
#[serial(slurm)]
fn test_get_statuses_multiple_jobs() {
    cleanup_fake_slurm_state();
    setup_fake_slurm_commands();

    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    // Submit multiple jobs
    let temp_dir = env::temp_dir();
    let mut job_ids = Vec::new();

    for i in 0..3 {
        let script_path = temp_dir.join(format!("test_multi_job_{}.sh", i));
        fs::write(&script_path, "#!/bin/bash\necho 'test'\n").expect("Failed to write test script");

        let (_, job_id, _) = interface
            .submit(&script_path)
            .expect("Failed to submit job");
        job_ids.push(job_id);
    }

    // Get all statuses
    let statuses = interface.get_statuses();
    assert!(
        statuses.is_ok(),
        "Failed to get statuses: {:?}",
        statuses.err()
    );

    let statuses_map = statuses.unwrap();
    assert!(statuses_map.len() >= 3, "Should have at least 3 jobs");

    // Verify all our jobs are in the map
    for job_id in &job_ids {
        assert!(
            statuses_map.contains_key(job_id),
            "Job {} not found in statuses",
            job_id
        );
        assert_eq!(statuses_map[job_id], HpcJobStatus::Queued);
    }

    cleanup_fake_slurm_state();
}

#[rstest]
#[serial(slurm)]
fn test_cancel_job_success() {
    cleanup_fake_slurm_state();
    let (_, _, _, scancel, _) = setup_fake_slurm_commands();

    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    // Submit a job first
    let temp_dir = env::temp_dir();
    let script_path = temp_dir.join("test_cancel.sh");
    fs::write(&script_path, "#!/bin/bash\necho 'test'\n").expect("Failed to write test script");

    let (_, job_id, _) = interface
        .submit(&script_path)
        .expect("Failed to submit job");

    // Cancel the job - need to use scancel directly since cancel_job doesn't use env var
    let result = std::process::Command::new(scancel).arg(&job_id).output();

    assert!(result.is_ok(), "Failed to run scancel");
    let output = result.unwrap();
    assert_eq!(
        output.status.code().unwrap_or(-1),
        0,
        "scancel should succeed"
    );

    cleanup_fake_slurm_state();
}

#[rstest]
#[serial(slurm)]
fn test_get_job_stats() {
    cleanup_fake_slurm_state();
    let (_, _, sacct, _, _) = setup_fake_slurm_commands();

    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    // Submit a job first
    let temp_dir = env::temp_dir();
    let script_path = temp_dir.join("test_stats.sh");
    fs::write(&script_path, "#!/bin/bash\necho 'test'\n").expect("Failed to write test script");

    let (_, job_id, _) = interface
        .submit(&script_path)
        .expect("Failed to submit job");

    // Get job stats using sacct directly
    let output = std::process::Command::new(sacct)
        .args([
            "-j",
            &job_id,
            "--format=JobID,JobName%20,state,start,end,Account,Partition%15,QOS",
        ])
        .output();

    assert!(output.is_ok(), "Failed to run sacct");
    let output = output.unwrap();
    assert_eq!(
        output.status.code().unwrap_or(-1),
        0,
        "sacct should succeed"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&job_id), "Output should contain job ID");
    assert!(
        stdout.contains("test_job"),
        "Output should contain job name"
    );

    cleanup_fake_slurm_state();
}

#[test]
fn test_create_submission_script() {
    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    let temp_dir = env::temp_dir();
    let script_path = temp_dir.join("test_submission_script.sh");

    let mut config = std::collections::HashMap::new();
    config.insert("account".to_string(), "test_account".to_string());
    config.insert("walltime".to_string(), "01:00:00".to_string());
    config.insert("nodes".to_string(), "2".to_string());
    config.insert("ntasks_per_node".to_string(), "4".to_string());

    let result = interface.create_submission_script(
        "test_job",
        "http://localhost:8080/torc-service/v1",
        12345,
        "/tmp/output",
        5,
        None,
        &script_path,
        &config,
        false,
        None,
        false,
        0,
    );

    assert!(
        result.is_ok(),
        "Failed to create submission script: {:?}",
        result.err()
    );

    // Read and verify the script
    let script_content =
        fs::read_to_string(&script_path).expect("Failed to read submission script");

    assert!(
        script_content.contains("#!/bin/bash"),
        "Should have shebang"
    );
    assert!(
        script_content.contains("#SBATCH --account=test_account"),
        "Should have account"
    );
    assert!(
        script_content.contains("#SBATCH --job-name=test_job"),
        "Should have job name"
    );
    assert!(
        script_content.contains("#SBATCH --time=01:00:00"),
        "Should have walltime"
    );
    assert!(
        script_content.contains("#SBATCH --nodes=2"),
        "Should have nodes"
    );
    assert!(
        script_content.contains("#SBATCH --ntasks-per-node=4"),
        "Should have ntasks-per-node"
    );
    assert!(
        script_content.contains("torc-slurm-job-runner"),
        "Should have torc-slurm-job-runner command"
    );
    assert!(
        script_content.contains("http://localhost:8080/torc-service/v1"),
        "Should have server URL"
    );
    assert!(script_content.contains("12345"), "Should have workflow_id");
    assert!(
        script_content.contains("--poll-interval 5"),
        "Should have poll interval"
    );

    // Clean up
    let _ = fs::remove_file(&script_path);
}

#[test]
fn test_create_submission_script_with_extra() {
    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    let temp_dir = env::temp_dir();
    let script_path = temp_dir.join("test_submission_script_extra.sh");

    let mut config = std::collections::HashMap::new();
    config.insert("account".to_string(), "test_account".to_string());
    config.insert("walltime".to_string(), "02:00:00".to_string());
    config.insert("extra".to_string(), "--constraint=haswell".to_string());

    let result = interface.create_submission_script(
        "test_job_extra",
        "http://localhost:8080/torc-service/v1",
        67890,
        "/tmp/output",
        10,
        Some(4),
        &script_path,
        &config,
        false,
        None,
        false,
        0,
    );

    assert!(
        result.is_ok(),
        "Failed to create submission script: {:?}",
        result.err()
    );

    let script_content =
        fs::read_to_string(&script_path).expect("Failed to read submission script");

    assert!(
        script_content.contains("#SBATCH --constraint=haswell"),
        "Should have extra parameter"
    );
    assert!(
        script_content.contains("--max-parallel-jobs 4"),
        "Should have max-parallel-jobs parameter"
    );

    let _ = fs::remove_file(&script_path);
}

#[test]
fn test_create_submission_script_without_srun() {
    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    let temp_dir = env::temp_dir();
    let script_path = temp_dir.join("test_submission_script_without_srun.sh");

    let mut config = std::collections::HashMap::new();
    config.insert("account".to_string(), "test_account".to_string());
    config.insert("walltime".to_string(), "01:00:00".to_string());

    let result = interface.create_submission_script(
        "test_srun_job",
        "http://localhost:8080/torc-service/v1",
        11111,
        "/tmp/output",
        5,
        None,
        &script_path,
        &config,
        false,
        None,
        false,
        0,
    );

    assert!(
        result.is_ok(),
        "Failed to create submission script: {:?}",
        result.err()
    );

    let script_content =
        fs::read_to_string(&script_path).expect("Failed to read submission script");

    // Multi-node allocations use a single worker that manages all nodes
    // and uses srun --exact for each job.
    assert!(
        !script_content.contains("srun --ntasks-per-node=1"),
        "Should NOT have outer srun wrapper (single worker manages all nodes)"
    );

    assert!(
        script_content.contains("unset SLURM_MEM_PER_CPU SLURM_MEM_PER_GPU"),
        "Should unset conflicting Slurm memory variables"
    );

    assert!(
        script_content.contains("torc-slurm-job-runner"),
        "Should run torc-slurm-job-runner directly"
    );

    let _ = fs::remove_file(&script_path);
}

#[test]
fn test_create_submission_script_with_srun() {
    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    let temp_dir = env::temp_dir();
    let script_path = temp_dir.join("test_submission_script_with_srun.sh");

    let mut config = std::collections::HashMap::new();
    config.insert("account".to_string(), "test_account".to_string());
    config.insert("walltime".to_string(), "01:00:00".to_string());

    let result = interface.create_submission_script(
        "test_srun_job",
        "http://localhost:8080/torc-service/v1",
        11111,
        "/tmp/output",
        5,
        None,
        &script_path,
        &config,
        true,
        None,
        false,
        0,
    );

    assert!(
        result.is_ok(),
        "Failed to create submission script: {:?}",
        result.err()
    );

    let script_content =
        fs::read_to_string(&script_path).expect("Failed to read submission script");

    assert!(
        script_content.contains("srun --ntasks-per-node=1 "),
        "Should have outer srun wrapper when start_one_worker_per_node is true"
    );
    assert!(
        script_content.contains("torc-slurm-job-runner"),
        "Should run torc-slurm-job-runner via srun"
    );

    let _ = fs::remove_file(&script_path);
}

#[test]
fn test_compute_startup_delay() {
    use torc::client::commands::slurm::compute_startup_delay;

    // Single runner: no delay
    assert_eq!(compute_startup_delay(0), 0);
    assert_eq!(compute_startup_delay(1), 0);

    // 2-10 runners: delay equals runner count
    assert_eq!(compute_startup_delay(2), 2);
    assert_eq!(compute_startup_delay(5), 5);
    assert_eq!(compute_startup_delay(10), 10);

    // 11-100 runners: linear scale from 10 to 60
    assert_eq!(compute_startup_delay(11), 10); // 10 + (1*50/90) = 10
    assert_eq!(compute_startup_delay(55), 35); // 10 + (45*50/90) = 35
    assert_eq!(compute_startup_delay(100), 60); // 10 + (90*50/90) = 60

    // 100+ runners: capped at 60
    assert_eq!(compute_startup_delay(101), 60);
    assert_eq!(compute_startup_delay(500), 60);
    assert_eq!(compute_startup_delay(1000), 60);
}

#[test]
fn test_create_submission_script_with_startup_delay() {
    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    let temp_dir = env::temp_dir();
    let script_path = temp_dir.join("test_submission_script_startup_delay.sh");

    let mut config = std::collections::HashMap::new();
    config.insert("account".to_string(), "test_account".to_string());
    config.insert("walltime".to_string(), "01:00:00".to_string());

    let result = interface.create_submission_script(
        "test_job",
        "http://localhost:8080/torc-service/v1",
        12345,
        "/tmp/output",
        5,
        None,
        &script_path,
        &config,
        false,
        None,
        false,
        30,
    );

    assert!(
        result.is_ok(),
        "Failed to create submission script: {:?}",
        result.err()
    );

    let script_content =
        fs::read_to_string(&script_path).expect("Failed to read submission script");

    assert!(
        script_content.contains("--startup-delay-seconds 30"),
        "Should have --startup-delay-seconds flag when delay > 0"
    );

    let _ = fs::remove_file(&script_path);
}

#[test]
fn test_create_submission_script_without_startup_delay() {
    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    let temp_dir = env::temp_dir();
    let script_path = temp_dir.join("test_submission_script_no_startup_delay.sh");

    let mut config = std::collections::HashMap::new();
    config.insert("account".to_string(), "test_account".to_string());
    config.insert("walltime".to_string(), "01:00:00".to_string());

    let result = interface.create_submission_script(
        "test_job",
        "http://localhost:8080/torc-service/v1",
        12345,
        "/tmp/output",
        5,
        None,
        &script_path,
        &config,
        false,
        None,
        false,
        0,
    );

    assert!(
        result.is_ok(),
        "Failed to create submission script: {:?}",
        result.err()
    );

    let script_content =
        fs::read_to_string(&script_path).expect("Failed to read submission script");

    assert!(
        !script_content.contains("--startup-delay-seconds"),
        "Should NOT have --startup-delay-seconds flag when delay is 0"
    );

    let _ = fs::remove_file(&script_path);
}

#[rstest]
#[serial(slurm)]
fn test_status_mapping() {
    cleanup_fake_slurm_state();
    setup_fake_slurm_commands();

    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    // Test different status mappings
    let test_cases = vec![
        ("PENDING", HpcJobStatus::Queued),
        ("CONFIGURING", HpcJobStatus::Queued),
        ("RUNNING", HpcJobStatus::Running),
        ("COMPLETED", HpcJobStatus::Complete),
        ("COMPLETING", HpcJobStatus::Complete),
        ("FAILED", HpcJobStatus::Unknown),
        ("CANCELLED", HpcJobStatus::Unknown),
        ("TIMEOUT", HpcJobStatus::Unknown),
    ];

    for (slurm_state, expected_status) in test_cases {
        // Submit a job
        let temp_dir = env::temp_dir();
        let script_path = temp_dir.join(format!("test_status_{}.sh", slurm_state));
        fs::write(&script_path, "#!/bin/bash\necho 'test'\n").expect("Failed to write test script");

        let (_, job_id, _) = interface
            .submit(&script_path)
            .expect("Failed to submit job");

        // Set the desired state
        unsafe {
            env::set_var("TORC_FAKE_SQUEUE_STATE", slurm_state);
        }

        let status = interface.get_status(&job_id);
        assert!(
            status.is_ok(),
            "Failed to get status for {}: {:?}",
            slurm_state,
            status.err()
        );

        let job_info = status.unwrap();
        assert_eq!(
            job_info.status, expected_status,
            "Status mismatch for {}",
            slurm_state
        );

        // Clean up for next iteration
        unsafe {
            env::remove_var("TORC_FAKE_SQUEUE_STATE");
        }
    }

    cleanup_fake_slurm_state();
}

#[rstest]
#[serial(slurm)]
fn test_squeue_output_parsing() {
    cleanup_fake_slurm_state();
    setup_fake_slurm_commands();

    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    // Submit a job
    let temp_dir = env::temp_dir();
    let script_path = temp_dir.join("test_parsing.sh");
    fs::write(&script_path, "#!/bin/bash\necho 'test'\n").expect("Failed to write test script");

    let (_, job_id, _) = interface
        .submit(&script_path)
        .expect("Failed to submit job");

    // Get status and verify parsing
    let status = interface.get_status(&job_id);
    assert!(status.is_ok(), "Failed to get status");

    let job_info = status.unwrap();

    // Verify all fields are populated
    assert!(!job_info.job_id.is_empty(), "Job ID should not be empty");
    assert!(!job_info.name.is_empty(), "Job name should not be empty");
    assert_ne!(
        job_info.status,
        HpcJobStatus::Unknown,
        "Status should not be Unknown for valid job"
    );

    cleanup_fake_slurm_state();
}

#[rstest]
#[serial(slurm)]
fn test_sbatch_regex_parsing() {
    cleanup_fake_slurm_state();
    setup_fake_slurm_commands();

    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    let temp_dir = env::temp_dir();
    let script_path = temp_dir.join("test_regex.sh");
    fs::write(&script_path, "#!/bin/bash\necho 'test'\n").expect("Failed to write test script");

    let result = interface.submit(&script_path);
    assert!(result.is_ok(), "Submit failed");

    let (return_code, job_id, _) = result.unwrap();
    assert_eq!(return_code, 0, "Should succeed");

    // Job ID should be a valid integer
    let parsed_id = job_id.parse::<i32>();
    assert!(parsed_id.is_ok(), "Job ID should be numeric: {}", job_id);
    assert!(parsed_id.unwrap() >= 1000, "Job ID should be >= 1000");

    cleanup_fake_slurm_state();
}

#[rstest]
#[serial(slurm)]
fn test_get_statuses_empty() {
    cleanup_fake_slurm_state();
    setup_fake_slurm_commands();

    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    // Get statuses when no jobs exist
    let statuses = interface.get_statuses();
    assert!(statuses.is_ok(), "Failed to get statuses");

    let _statuses_map = statuses.unwrap();
    // Should return empty map or contain no jobs for this user
    // (Implementation detail depends on fake squeue behavior)

    cleanup_fake_slurm_state();
}

#[rstest]
#[serial(slurm)]
fn test_incremental_job_ids() {
    cleanup_fake_slurm_state();
    setup_fake_slurm_commands();

    let interface = SlurmInterface::new().expect("Failed to create SlurmInterface");

    let temp_dir = env::temp_dir();
    let mut job_ids = Vec::new();

    // Submit 5 jobs and verify IDs increment
    for i in 0..5 {
        let script_path = temp_dir.join(format!("test_increment_{}.sh", i));
        fs::write(&script_path, "#!/bin/bash\necho 'test'\n").expect("Failed to write test script");

        let (_, job_id, _) = interface
            .submit(&script_path)
            .expect("Failed to submit job");
        job_ids.push(job_id.parse::<i32>().expect("Job ID should be numeric"));
    }

    // Verify job IDs are incrementing
    for i in 1..job_ids.len() {
        assert_eq!(
            job_ids[i],
            job_ids[i - 1] + 1,
            "Job IDs should increment by 1"
        );
    }

    cleanup_fake_slurm_state();
}
#[rstest]
fn test_slurm_create_config(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_slurm_create_workflow");
    let workflow_id = workflow.id.unwrap();

    // Test the CLI create command with JSON output
    let args = [
        "slurm",
        "create",
        &workflow_id.to_string(),
        "--name",
        "test_slurm_config",
        "--account",
        "my_account",
        "--nodes",
        "2",
        "--walltime",
        "02:00:00",
        "--qos",
        "normal",
    ];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run slurm create command");

    assert!(json_output.get("id").is_some());
    assert_eq!(json_output.get("workflow_id").unwrap(), &json!(workflow_id));
    assert_eq!(
        json_output.get("name").unwrap(),
        &json!("test_slurm_config")
    );
    assert_eq!(json_output.get("account").unwrap(), &json!("my_account"));
    assert_eq!(json_output.get("nodes").unwrap(), &json!(2));
    assert_eq!(json_output.get("walltime").unwrap(), &json!("02:00:00"));
    assert_eq!(json_output.get("qos").unwrap(), &json!("normal"));
}

#[rstest]
fn test_slurm_create_with_optional_params(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_slurm_optional_params_workflow");
    let workflow_id = workflow.id.unwrap();

    // Test creating with optional parameters
    let args = [
        "slurm",
        "create",
        &workflow_id.to_string(),
        "--name",
        "gpu_config",
        "--account",
        "gpu_account",
        "--gres",
        "gpu:2",
        "--mem",
        "180G",
        "--partition",
        "gpu_partition",
        "--tmp",
        "100G",
        "--extra",
        "'--reservation=my-reservation'",
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run slurm create with optional params");

    assert_eq!(json_output.get("name").unwrap(), &json!("gpu_config"));
    assert_eq!(json_output.get("gres").unwrap(), &json!("gpu:2"));
    assert_eq!(json_output.get("mem").unwrap(), &json!("180G"));
    assert_eq!(
        json_output.get("partition").unwrap(),
        &json!("gpu_partition")
    );
    assert_eq!(json_output.get("tmp").unwrap(), &json!("100G"));
    assert_eq!(
        json_output.get("extra").unwrap(),
        &json!("'--reservation=my-reservation'")
    );
}

#[rstest]
fn test_slurm_get_config_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_slurm_get_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create a Slurm config
    let create_args = [
        "slurm",
        "create",
        &workflow_id.to_string(),
        "--name",
        "test_get_config",
        "--account",
        "test_account",
        "--nodes",
        "1",
    ];

    let created_config =
        run_cli_with_json(&create_args, start_server, None).expect("Failed to create Slurm config");
    let config_id = created_config.get("id").unwrap().as_i64().unwrap();

    // Test the CLI get command
    let args = ["slurm", "get", &config_id.to_string()];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run slurm get command");

    // Verify JSON structure
    assert_eq!(json_output.get("id").unwrap(), &json!(config_id));
    assert_eq!(json_output.get("workflow_id").unwrap(), &json!(workflow_id));
    assert_eq!(json_output.get("name").unwrap(), &json!("test_get_config"));
    assert_eq!(json_output.get("account").unwrap(), &json!("test_account"));
    assert_eq!(json_output.get("nodes").unwrap(), &json!(1));
}

#[rstest]
fn test_slurm_update_config_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_slurm_update_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create a Slurm config
    let create_args = [
        "slurm",
        "create",
        &workflow_id.to_string(),
        "--name",
        "original_config",
        "--account",
        "original_account",
        "--nodes",
        "1",
    ];

    let created_config =
        run_cli_with_json(&create_args, start_server, None).expect("Failed to create Slurm config");
    let config_id = created_config.get("id").unwrap().as_i64().unwrap();

    // Test the CLI update command
    let args = [
        "slurm",
        "update",
        &config_id.to_string(),
        "-N",
        "updated_config",
        "--account",
        "updated_account",
        "--nodes",
        "4",
    ];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run slurm update command");

    // Verify the updated values
    assert_eq!(json_output.get("id").unwrap(), &json!(config_id));
    assert_eq!(json_output.get("name").unwrap(), &json!("updated_config"));
    assert_eq!(
        json_output.get("account").unwrap(),
        &json!("updated_account")
    );
    assert_eq!(json_output.get("nodes").unwrap(), &json!(4));

    // Verify unchanged values
    assert_eq!(json_output.get("workflow_id").unwrap(), &json!(workflow_id));
}

#[rstest]
fn test_slurm_update_partial_fields(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_slurm_partial_update_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create a Slurm config
    let create_args = [
        "slurm",
        "create",
        &workflow_id.to_string(),
        "--name",
        "partial_update_config",
        "--account",
        "partial_account",
        "--nodes",
        "2",
        "--walltime",
        "01:00:00",
    ];

    let created_config =
        run_cli_with_json(&create_args, start_server, None).expect("Failed to create Slurm config");
    let config_id = created_config.get("id").unwrap().as_i64().unwrap();

    // Test updating only the account
    let args = [
        "slurm",
        "update",
        &config_id.to_string(),
        "--account",
        "only_account_updated",
    ];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run partial slurm update");

    // Only account should be updated
    assert_eq!(
        json_output.get("account").unwrap(),
        &json!("only_account_updated")
    );
    // Other fields should remain unchanged
    assert_eq!(
        json_output.get("name").unwrap(),
        &json!("partial_update_config")
    );
    assert_eq!(json_output.get("nodes").unwrap(), &json!(2));
    assert_eq!(json_output.get("walltime").unwrap(), &json!("01:00:00"));
}

#[rstest]
fn test_slurm_update_multiple_optional_fields(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_slurm_multi_update_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create a Slurm config
    let create_args = [
        "slurm",
        "create",
        &workflow_id.to_string(),
        "--name",
        "multi_update_config",
        "--account",
        "multi_account",
    ];

    let created_config =
        run_cli_with_json(&create_args, start_server, None).expect("Failed to create Slurm config");
    let config_id = created_config.get("id").unwrap().as_i64().unwrap();

    // Test updating multiple optional fields
    let args = [
        "slurm",
        "update",
        &config_id.to_string(),
        "--gres",
        "gpu:4",
        "--mem",
        "256G",
        "--partition",
        "high_mem",
        "--qos",
        "high",
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run multi-field slurm update");

    assert_eq!(json_output.get("gres").unwrap(), &json!("gpu:4"));
    assert_eq!(json_output.get("mem").unwrap(), &json!("256G"));
    assert_eq!(json_output.get("partition").unwrap(), &json!("high_mem"));
    assert_eq!(json_output.get("qos").unwrap(), &json!("high"));
}

#[rstest]
fn test_slurm_list_configs_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_slurm_list_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create multiple Slurm configs
    for i in 0..3 {
        let create_args = [
            "slurm",
            "create",
            &workflow_id.to_string(),
            "--name",
            &format!("config_{}", i),
            "--account",
            "list_account",
            "--nodes",
            "1",
        ];
        run_cli_with_json(&create_args, start_server, None)
            .expect("Failed to create Slurm config for list test");
    }

    // Test the CLI list command
    let args = ["slurm", "list", &workflow_id.to_string()];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run slurm list command");

    // Verify JSON structure is an object with slurm_schedulers field
    assert!(
        json_output.is_object(),
        "Slurm configs list should return an object"
    );
    assert!(
        json_output.get("slurm_schedulers").is_some(),
        "Response should have slurm_schedulers field"
    );

    let configs_array = json_output
        .get("slurm_schedulers")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(
        configs_array.len() >= 3,
        "Should have at least 3 Slurm configs"
    );

    // Verify each config has the expected structure
    for slurm_config in configs_array {
        assert!(slurm_config.get("id").is_some());
        assert!(slurm_config.get("workflow_id").is_some());
        assert!(slurm_config.get("name").is_some());
        assert!(slurm_config.get("account").is_some());
        assert!(slurm_config.get("nodes").is_some());
    }
}

#[rstest]
fn test_slurm_list_pagination(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_slurm_pagination_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create multiple Slurm configs
    for i in 0..5 {
        let create_args = [
            "slurm",
            "create",
            &workflow_id.to_string(),
            "--name",
            &format!("pagination_config_{}", i),
            "--account",
            "pagination_account",
        ];
        run_cli_with_json(&create_args, start_server, None).expect("Failed to create Slurm config");
    }

    // Test with limit
    let args = ["slurm", "list", &workflow_id.to_string(), "--limit", "3"];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run paginated slurm list");

    let configs_array = json_output
        .get("slurm_schedulers")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(configs_array.len() <= 3, "Should respect limit parameter");
    assert!(!configs_array.is_empty(), "Should have at least one config");

    // Test with offset
    let args_with_offset = [
        "slurm",
        "list",
        &workflow_id.to_string(),
        "--limit",
        "2",
        "--offset",
        "2",
    ];

    let json_output_offset = run_cli_with_json(&args_with_offset, start_server, None)
        .expect("Failed to run slurm list with offset");

    let configs_with_offset = json_output_offset
        .get("slurm_schedulers")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(
        !configs_with_offset.is_empty(),
        "Should have configs with offset"
    );
}

#[rstest]
fn test_slurm_error_handling(start_server: &ServerProcess) {
    // Test getting a non-existent config
    let args = ["slurm", "get", "999999"];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(
        result.is_err(),
        "Should fail when getting non-existent Slurm config"
    );

    // Test updating a non-existent config
    let args = ["slurm", "update", "999999", "--account", "should_fail"];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(
        result.is_err(),
        "Should fail when updating non-existent Slurm config"
    );
}

#[rstest]
fn test_slurm_create_with_all_parameters(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_slurm_all_params_workflow");
    let workflow_id = workflow.id.unwrap();

    // Test creating with all possible parameters
    let args = [
        "slurm",
        "create",
        &workflow_id.to_string(),
        "--name",
        "full_config",
        "--account",
        "full_account",
        "--gres",
        "gpu:8",
        "--mem",
        "512G",
        "--nodes",
        "8",
        "--partition",
        "full_partition",
        "--qos",
        "high",
        "--tmp",
        "500G",
        "--walltime",
        "24:00:00",
        "--extra",
        "'--constraint=feature1'",
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run slurm create with all parameters");

    // Verify all fields are set correctly
    assert_eq!(json_output.get("name").unwrap(), &json!("full_config"));
    assert_eq!(json_output.get("account").unwrap(), &json!("full_account"));
    assert_eq!(json_output.get("gres").unwrap(), &json!("gpu:8"));
    assert_eq!(json_output.get("mem").unwrap(), &json!("512G"));
    assert_eq!(json_output.get("nodes").unwrap(), &json!(8));
    assert_eq!(
        json_output.get("partition").unwrap(),
        &json!("full_partition")
    );
    assert_eq!(json_output.get("qos").unwrap(), &json!("high"));
    assert_eq!(json_output.get("tmp").unwrap(), &json!("500G"));
    assert_eq!(json_output.get("walltime").unwrap(), &json!("24:00:00"));
    assert_eq!(
        json_output.get("extra").unwrap(),
        &json!("'--constraint=feature1'")
    );
}

#[rstest]
fn test_slurm_multiple_workflows(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create two workflows
    let workflow1 = create_test_workflow(config, "test_slurm_workflow_1");
    let workflow1_id = workflow1.id.unwrap();

    let workflow2 = create_test_workflow(config, "test_slurm_workflow_2");
    let workflow2_id = workflow2.id.unwrap();

    // Create configs for each workflow
    let create_args1 = [
        "slurm",
        "create",
        &workflow1_id.to_string(),
        "--name",
        "workflow1_config",
        "--account",
        "account1",
    ];

    let create_args2 = [
        "slurm",
        "create",
        &workflow2_id.to_string(),
        "--name",
        "workflow2_config",
        "--account",
        "account2",
    ];

    run_cli_with_json(&create_args1, start_server, None)
        .expect("Failed to create config for workflow 1");
    run_cli_with_json(&create_args2, start_server, None)
        .expect("Failed to create config for workflow 2");

    // List configs for workflow 1
    let list_args1 = ["slurm", "list", &workflow1_id.to_string()];
    let json_output1 = run_cli_with_json(&list_args1, start_server, None)
        .expect("Failed to list configs for workflow 1");

    let configs1 = json_output1
        .get("slurm_schedulers")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(
        !configs1.is_empty(),
        "Should have at least one config for workflow 1"
    );

    // Verify the configs belong to the correct workflow
    for config_item in configs1 {
        assert_eq!(
            config_item.get("workflow_id").unwrap(),
            &json!(workflow1_id)
        );
    }

    // List configs for workflow 2
    let list_args2 = ["slurm", "list", &workflow2_id.to_string()];
    let json_output2 = run_cli_with_json(&list_args2, start_server, None)
        .expect("Failed to list configs for workflow 2");

    let configs2 = json_output2
        .get("slurm_schedulers")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(
        !configs2.is_empty(),
        "Should have at least one config for workflow 2"
    );

    // Verify the configs belong to the correct workflow
    for config_item in configs2 {
        assert_eq!(
            config_item.get("workflow_id").unwrap(),
            &json!(workflow2_id)
        );
    }
}

#[rstest]
fn test_slurm_get_after_update(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_slurm_get_after_update_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create a config
    let create_args = [
        "slurm",
        "create",
        &workflow_id.to_string(),
        "--name",
        "before_update",
        "--account",
        "before_account",
        "--nodes",
        "1",
    ];

    let created_config =
        run_cli_with_json(&create_args, start_server, None).expect("Failed to create Slurm config");
    let config_id = created_config.get("id").unwrap().as_i64().unwrap();

    // Update the config
    let update_args = [
        "slurm",
        "update",
        &config_id.to_string(),
        "-N",
        "after_update",
        "--account",
        "after_account",
        "--nodes",
        "3",
    ];

    run_cli_with_json(&update_args, start_server, None).expect("Failed to update Slurm config");

    // Get the config and verify updates were persisted
    let get_args = ["slurm", "get", &config_id.to_string()];
    let json_output =
        run_cli_with_json(&get_args, start_server, None).expect("Failed to get updated config");

    assert_eq!(json_output.get("name").unwrap(), &json!("after_update"));
    assert_eq!(json_output.get("account").unwrap(), &json!("after_account"));
    assert_eq!(json_output.get("nodes").unwrap(), &json!(3));
}

#[rstest]
fn test_slurm_delete_config(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "test_slurm_delete_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create a Slurm config to delete
    let create_args = [
        "slurm",
        "create",
        &workflow_id.to_string(),
        "--name",
        "config_to_delete",
        "--account",
        "delete_account",
        "--nodes",
        "1",
    ];

    let created_config =
        run_cli_with_json(&create_args, start_server, None).expect("Failed to create Slurm config");
    let config_id = created_config.get("id").unwrap().as_i64().unwrap();

    // Test the CLI delete command
    let args = ["slurm", "delete", &config_id.to_string()];

    let json_output =
        run_cli_with_json(&args, start_server, None).expect("Failed to run slurm delete command");

    // Verify the deleted config is returned
    assert_eq!(json_output.get("id").unwrap(), &json!(config_id));
    assert_eq!(json_output.get("name").unwrap(), &json!("config_to_delete"));
    assert_eq!(
        json_output.get("account").unwrap(),
        &json!("delete_account")
    );

    // Verify the config no longer exists by trying to get it
    let get_args = ["slurm", "get", &config_id.to_string()];
    let result = run_cli_with_json(&get_args, start_server, None);
    assert!(
        result.is_err(),
        "Should fail when getting deleted Slurm config"
    );
}

#[rstest]
#[serial(slurm)]
fn test_slurm_run_jobs(start_server: &ServerProcess) {
    let config = &start_server.config;
    // Create a temporary directory for job output
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path();

    // Create a simple workflow with independent jobs (no blocked jobs)
    let jobs = create_minimal_resources_workflow(config, false);

    // Get workflow ID from one of the jobs
    let first_job = jobs.values().next().expect("Should have at least one job");
    let workflow_id = first_job.workflow_id;

    let workflow = torc::client::default_api::get_workflow(config, workflow_id)
        .expect("Failed to get workflow");
    let torc_config = TorcConfig::default();
    let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow);
    workflow_manager
        .initialize(true)
        .expect("Failed to start workflow");

    run_slurm_job_runner_cli_command(start_server, workflow_id, work_dir.to_str().unwrap())
        .expect("Failed to run slurm job runner command");

    // Verify all jobs completed successfully
    let job_list = torc::client::default_api::list_jobs(
        config,
        workflow_id,
        None, // status
        None, // needs_file_id
        None, // upstream_job_id
        None, // offset
        None, // limit
        None, // sort_by
        None, // reverse_sort
        None, // include_relationships
        None, // active_compute_node_id
    )
    .expect("Failed to list jobs");

    let job_items = job_list.items.unwrap();
    assert_eq!(job_items.len(), 4, "Should have 4 jobs in the workflow");

    for job in job_items {
        assert_eq!(
            job.status.unwrap(),
            models::JobStatus::Completed,
            "Job {} should be Completed",
            job.name
        );
    }

    let results = torc::client::default_api::list_results(
        config,
        workflow_id,
        None, // job_id
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
        4,
        "Should have 4 results for the 4 jobs"
    );

    for result in result_items {
        assert_eq!(
            result.return_code, 0,
            "Job ID {} should have return code 0",
            result.job_id
        );
    }

    // Verify that log files were created
    let log_files: Vec<_> = fs::read_dir(work_dir)
        .expect("Failed to read work directory")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("job_runner_slurm_")
        })
        .collect();

    assert!(
        !log_files.is_empty(),
        "Should have created job runner log files"
    );
}

/// Helper function to create a test Slurm scheduler
fn create_test_slurm_scheduler(
    config: &torc::client::Configuration,
    workflow_id: i64,
) -> models::SlurmSchedulerModel {
    let scheduler = models::SlurmSchedulerModel {
        id: None,
        workflow_id,
        name: Some("test_slurm_scheduler".to_string()),
        account: "test_account".to_string(),
        gres: Some("gpu:1".to_string()),
        mem: Some("8G".to_string()),
        nodes: 1,
        ntasks_per_node: Some(4),
        partition: Some("test_partition".to_string()),
        qos: Some("normal".to_string()),
        tmp: Some("50G".to_string()),
        walltime: "01:00:00".to_string(),
        extra: None,
    };
    default_api::create_slurm_scheduler(config, scheduler)
        .expect("Failed to create Slurm scheduler")
}

/// Helper function to create simple test jobs
fn create_test_jobs(
    config: &torc::client::Configuration,
    workflow_id: i64,
    num_jobs: usize,
) -> Vec<models::JobModel> {
    let mut jobs = Vec::new();
    for i in 0..num_jobs {
        let job = models::JobModel::new(
            workflow_id,
            format!("test_job_{}", i),
            format!("echo 'Job {}'", i),
        );
        let created_job = default_api::create_job(config, job)
            .unwrap_or_else(|_| panic!("Failed to create job {}", i));
        jobs.push(created_job);
    }
    jobs
}

// TODO: Fix this test
#[rstest]
#[ignore]
fn test_cancel_workflow_with_slurm_scheduler(start_server: &ServerProcess) {
    let config = &start_server.config;
    let base_url = &config.base_path;

    // Create workflow
    let workflow = create_test_workflow(config, "test_cancel_slurm_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create Slurm scheduler
    let scheduler = create_test_slurm_scheduler(config, workflow_id);
    let scheduler_config_id = scheduler.id.unwrap();

    // Create 5 simple jobs
    let jobs = create_test_jobs(config, workflow_id, 5);

    // Start the workflow using WorkflowManager
    let torc_config = TorcConfig::default();
    let workflow_manager = WorkflowManager::new(config.clone(), torc_config, workflow.clone());
    workflow_manager
        .initialize(true)
        .expect("Failed to start workflow");

    setup_fake_slurm_commands();
    // Schedule nodes using CLI command
    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "torc",
            "--",
            "--url",
            base_url,
            "slurm",
            "schedule-nodes",
            &workflow_id.to_string(),
            "--scheduler-config-id",
            &scheduler_config_id.to_string(),
            "--num-hpc-jobs",
            "1",
            "--output",
            "/tmp/test_cancel_output",
        ])
        .env("SLURM_JOB_ID", "12345")
        .env("SLURM_NODEID", "0")
        .env("SLURM_PROCID", "0")
        .env("SLURM_MEM_PER_NODE", "8000")
        .env("SLURM_CPUS_ON_NODE", "4")
        .env("SLURM_CPUS_PER_TASK", "1")
        .env("SLURM_JOB_GPUS", "0")
        .env("SLURM_JOB_NUM_NODES", "1")
        .env("SLURM_TASK_PID", "1001")
        .output()
        .expect("Failed to execute schedule-nodes command");

    if !output.status.success() {
        eprintln!(
            "schedule-nodes stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        eprintln!(
            "schedule-nodes stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        panic!(
            "schedule-nodes command failed with status: {}",
            output.status
        );
    }

    // Verify scheduled compute node was created
    let scheduled_nodes = default_api::list_scheduled_compute_nodes(
        config,
        workflow_id,
        Some(0),
        Some(10),
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list scheduled compute nodes");

    let nodes = scheduled_nodes.items.unwrap();
    assert_eq!(nodes.len(), 1, "Expected 1 scheduled compute node");
    let node = &nodes[0];
    // assert_eq!(node.scheduler_id, 12345);  // TODO: this isn't synced with the fake script
    assert_eq!(node.scheduler_type, "slurm");
    assert_eq!(node.status, "pending");

    // Fake completion of some jobs (not all)
    for (i, job) in jobs.iter().enumerate() {
        if i < 3 {
            // Complete only first 3 jobs
            let mut updated_job = job.clone();
            updated_job.status = Some(models::JobStatus::Completed);

            let job_id = job.id.unwrap();
            default_api::update_job(config, job_id, updated_job)
                .unwrap_or_else(|_| panic!("Failed to update job {}", i));
        }
    }

    setup_fake_slurm_commands();
    // Cancel the workflow using CLI command
    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "torc",
            "--",
            "--url",
            base_url,
            "workflows",
            "cancel",
            &workflow_id.to_string(),
        ])
        .env("SLURM_JOB_ID", "12345")
        .env("SLURM_NODEID", "0")
        .env("SLURM_PROCID", "0")
        .env("SLURM_JOB_ID", "12345")
        .env("SLURM_MEM_PER_NODE", "8000")
        .env("SLURM_NODEID", "0")
        .env("SLURM_CPUS_ON_NODE", "4")
        .env("SLURM_CPUS_PER_TASK", "1")
        .env("SLURM_JOB_GPUS", "0")
        .env("SLURM_JOB_NUM_NODES", "1")
        .env("SLURM_TASK_PID", "1000")
        .output()
        .expect("Failed to execute cancel command");

    if !output.status.success() {
        eprintln!("cancel stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("cancel stderr: {}", String::from_utf8_lossy(&output.stderr));
        panic!("cancel command failed with status: {}", output.status);
    }

    // Verify workflow was canceled
    let workflow_status = default_api::get_workflow_status(config, workflow_id)
        .expect("Failed to get workflow status");
    assert!(workflow_status.is_canceled, "Workflow should be canceled");

    // Get the scheduled compute node
    let scheduled_nodes = default_api::list_scheduled_compute_nodes(
        config,
        workflow_id,
        Some(0),
        Some(10),
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list scheduled compute nodes after cancel");

    let nodes = scheduled_nodes.items.unwrap();
    assert_eq!(
        nodes.len(),
        1,
        "Expected 1 scheduled compute node after cancel"
    );
    let node = &nodes[0];
    let node_id = node.id.unwrap();

    // Simulate the job runner updating the node status to 'complete' after detecting cancellation
    let mut updated_node = node.clone();
    updated_node.status = "complete".to_string();
    default_api::update_scheduled_compute_node(config, node_id, updated_node)
        .expect("Failed to update scheduled compute node to complete");

    // Verify the node status is now 'complete'
    let final_node = default_api::get_scheduled_compute_node(config, node_id)
        .expect("Failed to get scheduled compute node");
    assert_eq!(
        final_node.status, "complete",
        "Scheduled compute node should be marked as complete"
    );

    // Verify that all jobs still exist
    let all_jobs = default_api::list_jobs(
        config,
        workflow_id,
        None,      // status
        None,      // needs_file_id
        None,      // upstream_job_id
        Some(0),   // offset
        Some(100), // limit
        None,      // sort_by
        None,      // reverse_sort
        None,      // include_relationships
        None,      // active_compute_node_id
    )
    .expect("Failed to list jobs");

    assert_eq!(
        all_jobs.items.unwrap().len(),
        5,
        "All 5 jobs should still exist"
    );
}

fn setup_fake_slurm_commands() -> (PathBuf, PathBuf, PathBuf, PathBuf, PathBuf) {
    let current_dir = env::current_dir().expect("Failed to get current directory");

    let sbatch = current_dir.join("tests/scripts/fake_sbatch.sh");
    let squeue = current_dir.join("tests/scripts/fake_squeue.sh");
    let sacct = current_dir.join("tests/scripts/fake_sacct.sh");
    let scancel = current_dir.join("tests/scripts/fake_scancel.sh");
    let srun = current_dir.join("tests/scripts/fake_srun.sh");

    // Verify scripts exist
    assert!(sbatch.exists(), "fake_sbatch.sh not found at {:?}", sbatch);
    assert!(squeue.exists(), "fake_squeue.sh not found at {:?}", squeue);
    assert!(sacct.exists(), "fake_sacct.sh not found at {:?}", sacct);
    assert!(
        scancel.exists(),
        "fake_scancel.sh not found at {:?}",
        scancel
    );
    assert!(srun.exists(), "fake_srun.sh not found at {:?}", srun);

    // Set environment variables to use fake commands
    unsafe {
        env::set_var("TORC_FAKE_SBATCH", sbatch.to_string_lossy().to_string());
        env::set_var("TORC_FAKE_SQUEUE", squeue.to_string_lossy().to_string());
        env::set_var("TORC_FAKE_SACCT", sacct.to_string_lossy().to_string());
        env::set_var("TORC_FAKE_SCANCEL", scancel.to_string_lossy().to_string());
        env::set_var("TORC_FAKE_SRUN", srun.to_string_lossy().to_string());
    }

    (sbatch, squeue, sacct, scancel, srun)
}

fn cleanup_fake_slurm_state() {
    let tmpdir = env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_string());
    let counter_file = format!("{}/fake_sbatch_counter.txt", tmpdir);
    let jobs_file = format!("{}/fake_slurm_jobs.txt", tmpdir);

    let _ = fs::remove_file(&counter_file);
    let _ = fs::remove_file(&jobs_file);

    // Clear failure simulation env vars
    unsafe {
        env::remove_var("TORC_FAKE_SACCT");
        env::remove_var("TORC_FAKE_SBATCH");
        env::remove_var("TORC_FAKE_SCANCEL");
        env::remove_var("TORC_FAKE_SQUEUE");
        env::remove_var("TORC_FAKE_SRUN");
        env::remove_var("TORC_FAKE_SBATCH_FAIL");
        env::remove_var("TORC_FAKE_SQUEUE_FAIL");
        env::remove_var("TORC_FAKE_SACCT_FAIL");
        env::remove_var("TORC_FAKE_SCANCEL_FAIL");
        env::remove_var("TORC_FAKE_SQUEUE_STATE");
        env::remove_var("TORC_FAKE_SACCT_STATE");
    }
}

/// Helper function to run CLI commands without JSON output
pub fn run_slurm_job_runner_cli_command(
    server: &ServerProcess,
    workflow_id: i64,
    output_dir: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    cleanup_fake_slurm_state();
    setup_fake_slurm_commands();
    let mut cmd = Command::new(common::get_exe_path("./target/debug/torc-slurm-job-runner"));
    cmd.args(&[
        server.config.base_path.clone(),
        workflow_id.to_string(),
        output_dir.to_string(),
        "--poll-interval=1".to_string(),
    ]);
    cmd.env("SLURM_JOB_ID", "12345");
    cmd.env("SLURM_MEM_PER_NODE", "8000");
    cmd.env("SLURM_NODEID", "0");
    cmd.env("SLURM_CPUS_ON_NODE", "4");
    cmd.env("SLURM_CPUS_PER_TASK", "1");
    cmd.env("SLURM_JOB_GPUS", "0");
    cmd.env("SLURM_JOB_NUM_NODES", "1");
    cmd.env("SLURM_TASK_PID", "1000");
    let output = cmd.output()?;

    let result = if output.status.success() {
        let stdout = String::from_utf8(output.stdout)?;
        Ok(stdout)
    } else {
        let error_str = String::from_utf8(output.stderr)?;
        Err(format!("Command failed: {}", error_str).into())
    };

    cleanup_fake_slurm_state();
    result
}

/// Test that `slurm generate --group-by` correctly groups jobs by the specified strategy.
///
/// The resource_monitoring_demo.kdl workflow has 3 resource requirements (cpu, memory, mixed)
/// that all map to the same partition.
/// - With `--group-by partition`: 1 scheduler (all jobs map to same partition)
/// - With `--group-by resource-requirements` (default): 3 schedulers (one per resource_requirements)
#[rstest]
fn test_slurm_generate_group_by_strategy() {
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let workflow_file = current_dir.join("examples/kdl/resource_monitoring_demo.kdl");

    // Test with --group-by partition: should produce 1 scheduler
    let output_grouped = Command::new(common::get_exe_path("./target/debug/torc"))
        .args([
            "slurm",
            "generate",
            workflow_file.to_str().unwrap(),
            "--account",
            "dsgrid",
            "--group-by",
            "partition",
            "--dry-run",
            "--profile",
            "kestrel",
        ])
        .output()
        .expect("Failed to execute slurm generate with --group-by partition");

    assert!(
        output_grouped.status.success(),
        "slurm generate --group-by partition failed: {}",
        String::from_utf8_lossy(&output_grouped.stderr)
    );

    let stdout_grouped = String::from_utf8_lossy(&output_grouped.stdout);
    // Count lines containing "Scheduler:" to determine number of schedulers generated
    let scheduler_count_grouped = stdout_grouped
        .lines()
        .filter(|line| line.contains("Scheduler:"))
        .count();

    assert_eq!(
        scheduler_count_grouped, 1,
        "With --group-by partition, should generate 1 scheduler (all jobs map to same partition), \
         but got {}. Output:\n{}",
        scheduler_count_grouped, stdout_grouped
    );

    // Test with --group-by resource-requirements (explicit): should produce 3 schedulers
    let output_explicit = Command::new(common::get_exe_path("./target/debug/torc"))
        .args([
            "slurm",
            "generate",
            workflow_file.to_str().unwrap(),
            "--account",
            "dsgrid",
            "--group-by",
            "resource-requirements",
            "--dry-run",
            "--profile",
            "kestrel",
        ])
        .output()
        .expect("Failed to execute slurm generate with --group-by resource-requirements");

    assert!(
        output_explicit.status.success(),
        "slurm generate --group-by resource-requirements failed: {}",
        String::from_utf8_lossy(&output_explicit.stderr)
    );

    let stdout_explicit = String::from_utf8_lossy(&output_explicit.stdout);
    let scheduler_count_explicit = stdout_explicit
        .lines()
        .filter(|line| line.contains("Scheduler:"))
        .count();

    assert_eq!(
        scheduler_count_explicit, 3,
        "With --group-by resource-requirements, should generate 3 schedulers, \
         but got {}. Output:\n{}",
        scheduler_count_explicit, stdout_explicit
    );

    // Test without --group-by (default): should also produce 3 schedulers
    let output_default = Command::new(common::get_exe_path("./target/debug/torc"))
        .args([
            "slurm",
            "generate",
            workflow_file.to_str().unwrap(),
            "--account",
            "dsgrid",
            "--dry-run",
            "--profile",
            "kestrel",
        ])
        .output()
        .expect("Failed to execute slurm generate without --group-by");

    assert!(
        output_default.status.success(),
        "slurm generate (default) failed: {}",
        String::from_utf8_lossy(&output_default.stderr)
    );

    let stdout_default = String::from_utf8_lossy(&output_default.stdout);
    let scheduler_count_default = stdout_default
        .lines()
        .filter(|line| line.contains("Scheduler:"))
        .count();

    assert_eq!(
        scheduler_count_default, 3,
        "Default (no --group-by) should generate 3 schedulers (one per resource_requirements), \
         but got {}. Output:\n{}",
        scheduler_count_default, stdout_default
    );
}

/// Test that auto-merge combines deferred and non-deferred groups when total allocations are small.
///
/// When using `--group-by partition`, if a partition has both:
/// - Non-deferred jobs (no dependencies, can start at workflow start)
/// - Deferred jobs (have dependencies, need on_jobs_ready trigger)
///
/// And the total number of allocations is ≤2, they should be merged into a single scheduler
/// with a single `on_workflow_start` action. This reduces Slurm submissions.
///
/// This test specifically verifies that runtime is factored into allocation calculations:
/// - With num_cpus=52, only 2 jobs can run concurrently (104 CPUs / 52)
/// - Without runtime factor: 12 jobs / 2 concurrent = 6 allocations (would NOT merge)
/// - With runtime factor: 4h walltime / 10min = 24 time slots
///   - Jobs per allocation = 2 concurrent × 24 slots = 48
///   - 12 jobs / 48 = 1 allocation (WILL merge)
#[rstest]
fn test_slurm_generate_auto_merge_small_allocations() {
    // Create a temporary workflow file that tests runtime-aware allocation calculation
    // - 1 job without dependencies (build)
    // - Multiple jobs with dependencies (job_1..job_10)
    // - 1 job depending on all job_* (join)
    //
    // With num_cpus=52, concurrent capacity is only 2 jobs per node.
    // Using max-partition-time strategy, the allocation walltime equals the partition max (4h).
    // With 10-minute jobs and 4-hour walltime, 24 time slots are available,
    // so jobs_per_allocation = 2 * 24 = 48, which can handle all 12 jobs in 1 allocation.
    let workflow_json = r#"{
  "name": "test_auto_merge",
  "resource_requirements": [
    {
      "name": "small",
      "runtime": "PT10M",
      "memory": "2g",
      "num_cpus": 1
    },
    {
      "name": "medium",
      "runtime": "PT10M",
      "memory": "1g",
      "num_cpus": 52
    }
  ],
  "parameters": {
    "i": "1:10"
  },
  "jobs": [
    {
      "name": "build",
      "resource_requirements": "small",
      "command": "echo build"
    },
    {
      "name": "job_{i}",
      "resource_requirements": "medium",
      "depends_on": ["build"],
      "command": "echo job {i}",
      "use_parameters": ["i"]
    },
    {
      "name": "join",
      "resource_requirements": "small",
      "depends_on_regexes": ["job_\\d+"],
      "command": "echo join"
    }
  ]
}"#;

    // Write to a unique temp file using tempfile crate
    let mut workflow_file =
        tempfile::NamedTempFile::new().expect("Failed to create temp workflow file");
    std::io::Write::write_all(&mut workflow_file, workflow_json.as_bytes())
        .expect("Failed to write temp workflow file");

    // Run slurm generate with --group-by partition and max-partition-time strategy.
    // max-partition-time is needed so the allocation walltime equals the partition max (4h),
    // allowing time_slots = 4h / 10min = 24 sequential batches per allocation.
    let output = Command::new(common::get_exe_path("./target/debug/torc"))
        .args([
            "slurm",
            "generate",
            workflow_file.path().to_str().unwrap(),
            "--account",
            "test_account",
            "--group-by",
            "partition",
            "--profile",
            "kestrel",
            "--walltime-strategy",
            "max-partition-time",
        ])
        .output()
        .expect("Failed to execute slurm generate");

    assert!(
        output.status.success(),
        "slurm generate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse the JSON output
    let generated: serde_json::Value =
        serde_json::from_str(&stdout).expect("Failed to parse JSON output");

    // Verify only 1 scheduler was generated (auto-merge combined deferred + non-deferred)
    let schedulers = generated
        .get("slurm_schedulers")
        .and_then(|s| s.as_array())
        .expect("Expected slurm_schedulers array");

    assert_eq!(
        schedulers.len(),
        1,
        "Auto-merge should combine deferred and non-deferred groups into 1 scheduler when \
         total allocations are small. Got {} schedulers:\n{}",
        schedulers.len(),
        serde_json::to_string_pretty(&schedulers).unwrap()
    );

    // Verify only 1 action was generated with on_workflow_start trigger
    let actions = generated
        .get("actions")
        .and_then(|a| a.as_array())
        .expect("Expected actions array");

    assert_eq!(
        actions.len(),
        1,
        "Auto-merge should result in 1 action. Got {} actions:\n{}",
        actions.len(),
        serde_json::to_string_pretty(&actions).unwrap()
    );

    // Verify the action uses on_workflow_start (not on_jobs_ready)
    let action = &actions[0];
    let trigger_type = action
        .get("trigger_type")
        .and_then(|t| t.as_str())
        .expect("Expected trigger_type");

    assert_eq!(
        trigger_type, "on_workflow_start",
        "Merged scheduler should use on_workflow_start trigger (not on_jobs_ready)"
    );

    // Verify all jobs are assigned to the same scheduler
    // Note: slurm generate returns the spec before parameter expansion,
    // so we have 3 job specs (build, job_{i}, join), not 12 expanded jobs
    let jobs = generated
        .get("jobs")
        .and_then(|j| j.as_array())
        .expect("Expected jobs array");

    assert_eq!(
        jobs.len(),
        3,
        "Expected 3 job specs before parameter expansion"
    );

    let scheduler_name = schedulers[0]
        .get("name")
        .and_then(|n| n.as_str())
        .expect("Expected scheduler name");

    // Verify ALL jobs have scheduler assignments (not just some)
    let mut jobs_with_scheduler = 0;
    for job in jobs {
        let job_name = job
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("unknown");
        let job_scheduler = job
            .get("scheduler")
            .and_then(|s| s.as_str())
            .unwrap_or_else(|| panic!("Job '{}' should have a scheduler assignment", job_name));
        assert_eq!(
            job_scheduler, scheduler_name,
            "Job '{}' should be assigned to the merged scheduler",
            job_name
        );
        jobs_with_scheduler += 1;
    }

    assert_eq!(
        jobs_with_scheduler,
        jobs.len(),
        "All jobs should have scheduler assignments"
    );

    // Temp file is automatically cleaned up when workflow_file goes out of scope
}

/// Test that auto-merge does NOT combine groups when total allocations exceed the threshold.
///
/// When jobs require more than 2 allocations total, they should remain separate
/// (deferred and non-deferred schedulers).
#[rstest]
fn test_slurm_generate_no_merge_large_allocations() {
    // Create a workflow where jobs need many nodes (exceeding merge threshold)
    // Each job needs 1 full node, so 100 jobs = 100 allocations
    let workflow_json = r#"{
  "name": "test_no_merge",
  "resource_requirements": [
    {
      "name": "full_node",
      "runtime": "PT10M",
      "memory": "200g",
      "num_cpus": 100
    }
  ],
  "parameters": {
    "i": "1:50"
  },
  "jobs": [
    {
      "name": "setup",
      "resource_requirements": "full_node",
      "command": "echo setup"
    },
    {
      "name": "worker_{i}",
      "resource_requirements": "full_node",
      "depends_on": ["setup"],
      "command": "echo worker {i}",
      "use_parameters": ["i"]
    }
  ]
}"#;

    // Write to a unique temp file using tempfile crate
    let mut workflow_file =
        tempfile::NamedTempFile::new().expect("Failed to create temp workflow file");
    std::io::Write::write_all(&mut workflow_file, workflow_json.as_bytes())
        .expect("Failed to write temp workflow file");

    // Run slurm generate with --group-by partition
    let output = Command::new(common::get_exe_path("./target/debug/torc"))
        .args([
            "slurm",
            "generate",
            workflow_file.path().to_str().unwrap(),
            "--account",
            "test_account",
            "--group-by",
            "partition",
            "--profile",
            "kestrel",
        ])
        .output()
        .expect("Failed to execute slurm generate");

    assert!(
        output.status.success(),
        "slurm generate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse the JSON output
    let generated: serde_json::Value =
        serde_json::from_str(&stdout).expect("Failed to parse JSON output");

    // Verify 2 schedulers were generated (not merged due to large allocation count)
    let schedulers = generated
        .get("slurm_schedulers")
        .and_then(|s| s.as_array())
        .expect("Expected slurm_schedulers array");

    assert_eq!(
        schedulers.len(),
        2,
        "Should NOT merge when total allocations exceed threshold. Expected 2 schedulers \
         (one for setup, one deferred for workers). Got {} schedulers:\n{}",
        schedulers.len(),
        serde_json::to_string_pretty(&schedulers).unwrap()
    );

    // Verify 2 actions were generated
    let actions = generated
        .get("actions")
        .and_then(|a| a.as_array())
        .expect("Expected actions array");

    assert_eq!(
        actions.len(),
        2,
        "Should have 2 actions when not merged. Got {} actions:\n{}",
        actions.len(),
        serde_json::to_string_pretty(&actions).unwrap()
    );

    // Verify we have both on_workflow_start and on_jobs_ready triggers
    let trigger_types: Vec<&str> = actions
        .iter()
        .filter_map(|a| a.get("trigger_type").and_then(|t| t.as_str()))
        .collect();

    assert!(
        trigger_types.contains(&"on_workflow_start"),
        "Should have on_workflow_start action"
    );
    assert!(
        trigger_types.contains(&"on_jobs_ready"),
        "Should have on_jobs_ready action for deferred jobs"
    );

    // Temp file is automatically cleaned up when workflow_file goes out of scope
}
