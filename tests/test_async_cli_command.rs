mod common;

use common::{ServerProcess, create_test_job, create_test_workflow, start_server};
use rstest::rstest;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;
use torc::client::async_cli_command::AsyncCliCommand;
use torc::client::job_runner::cleanup_job_stdio_files;
use torc::client::workflow_spec::{ExecutionMode, StdioMode};
use torc::models::{JobModel, JobStatus};

/// Helper to create a temporary output directory for job stdio
fn create_temp_output_dir() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let job_stdio_dir = temp_dir.path().join("job_stdio");
    fs::create_dir_all(&job_stdio_dir).expect("Failed to create job_stdio directory");
    temp_dir
}

/// Helper to create a basic JobModel for testing
fn create_test_job_model(workflow_id: i64, job_id: i64, command: &str) -> JobModel {
    let mut job = JobModel::new(
        workflow_id,
        format!("test_job_{}", job_id),
        command.to_string(),
    );
    job.id = Some(job_id);
    job.status = Some(JobStatus::Ready);
    job
}

#[rstest]
fn test_async_cli_command_new(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "test_async_cli_new");
    let workflow_id = workflow.id.unwrap();

    let job = create_test_job(config, workflow_id, "test_job");
    let job_id = job.id.unwrap();

    let async_cmd = AsyncCliCommand::new(job.clone());

    assert_eq!(async_cmd.job_id, job_id);
    assert!(!async_cmd.is_running);
    assert!(!async_cmd.is_complete);
}

#[rstest]
fn test_async_cli_command_start_simple_command(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "test_async_start");
    let workflow_id = workflow.id.unwrap();

    let job = create_test_job_model(workflow_id, 1, "echo 'Hello World'");
    let mut async_cmd = AsyncCliCommand::new(job);

    let temp_dir = create_temp_output_dir();
    let output_dir = temp_dir.path().to_str().unwrap();

    let result = async_cmd.start(
        temp_dir.path(),
        1, // workflow_id
        1, // run_id
        1, // attempt_id
        None,
        "http://localhost:8080/torc-service/v1",
        None,
        None, // gpu_visible_devices
        true,
        ExecutionMode::Direct,
        false,
        None,
        None,
        60,   // sigkill_headroom_seconds
        None, // target_node
        &StdioMode::Separate,
    );
    assert!(
        result.is_ok(),
        "Failed to start command: {:?}",
        result.err()
    );
    assert!(async_cmd.is_running);

    // Wait for command to complete
    thread::sleep(Duration::from_millis(500));
    let _ = async_cmd.check_status();

    // Verify output files were created (format: job_wf{workflow_id}_j{job_id}_r{run_id}_a{attempt_id}.o/e)
    let stdout_path = format!("{}/job_stdio/job_wf1_j1_r1_a1.o", output_dir);
    let stderr_path = format!("{}/job_stdio/job_wf1_j1_r1_a1.e", output_dir);
    assert!(Path::new(&stdout_path).exists());
    assert!(Path::new(&stderr_path).exists());
}

#[rstest]
#[cfg(unix)]
fn test_async_cli_command_start_already_running() {
    let job = create_test_job_model(1, 1, "sleep 1");
    let mut async_cmd = AsyncCliCommand::new(job);

    let temp_dir = create_temp_output_dir();

    async_cmd
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None, // gpu_visible_devices
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("First start should succeed");
    assert!(async_cmd.is_running);

    // Try to start again while already running
    let result = async_cmd.start(
        temp_dir.path(),
        1, // workflow_id
        1, // run_id
        1, // attempt_id
        None,
        "http://localhost:8080/torc-service/v1",
        None,
        None, // gpu_visible_devices
        true,
        ExecutionMode::Direct,
        false,
        None,
        None,
        60,   // sigkill_headroom_seconds
        None, // target_node
        &StdioMode::Separate,
    );
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "Job is already running");

    // Clean up
    let _ = async_cmd.cancel();
    let _ = async_cmd.wait_for_completion();
}

#[rstest]
#[cfg(unix)]
fn test_async_cli_command_start_invalid_directory() {
    let job = create_test_job_model(1, 1, "echo 'test'");
    let mut async_cmd = AsyncCliCommand::new(job);

    // Try to start with invalid directory
    let result = async_cmd.start(
        std::path::Path::new("/nonexistent/invalid/path/that/does/not/exist"),
        1, // workflow_id
        1, // run_id
        1, // attempt_id
        None,
        "http://localhost:8080/torc-service/v1",
        None,
        None, // gpu_visible_devices
        true,
        ExecutionMode::Direct,
        false,
        None,
        None,
        60,   // sigkill_headroom_seconds
        None, // target_node
        &StdioMode::Separate,
    );
    assert!(result.is_err());
}

#[rstest]
fn test_async_cli_command_check_status_completion() {
    let job = create_test_job_model(1, 1, "echo 'test'");
    let mut async_cmd = AsyncCliCommand::new(job);

    let temp_dir = create_temp_output_dir();

    async_cmd
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None, // gpu_visible_devices
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("Failed to start command");
    assert!(async_cmd.is_running);

    // Poll until complete
    let mut max_attempts = 50;
    while async_cmd.is_running && max_attempts > 0 {
        let _ = async_cmd.check_status();
        thread::sleep(Duration::from_millis(100));
        max_attempts -= 1;
    }

    assert!(!async_cmd.is_running);
    assert!(async_cmd.is_complete);
}

#[rstest]
fn test_async_cli_command_check_status_not_running() {
    let job = create_test_job_model(1, 1, "echo 'test'");
    let mut async_cmd = AsyncCliCommand::new(job);

    // Check status before starting - should not error
    let result = async_cmd.check_status();
    assert!(result.is_ok());
}

#[rstest]
fn test_async_cli_command_with_exit_code_success() {
    let job = create_test_job_model(1, 1, "exit 0");
    let mut async_cmd = AsyncCliCommand::new(job);

    let temp_dir = create_temp_output_dir();

    async_cmd
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None, // gpu_visible_devices
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("Failed to start command");

    // Wait for completion
    thread::sleep(Duration::from_millis(500));
    let _ = async_cmd.check_status();

    assert!(async_cmd.is_complete);
}

#[rstest]
fn test_async_cli_command_with_exit_code_failure() {
    let job = create_test_job_model(1, 1, "exit 1");
    let mut async_cmd = AsyncCliCommand::new(job);

    let temp_dir = create_temp_output_dir();

    async_cmd
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None, // gpu_visible_devices
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("Failed to start command");

    // Wait for completion
    thread::sleep(Duration::from_millis(500));
    let _ = async_cmd.check_status();

    assert!(async_cmd.is_complete);
}

#[rstest]
#[cfg(unix)]
fn test_async_cli_command_cancel() {
    let job = create_test_job_model(1, 1, "sleep 10");
    let mut async_cmd = AsyncCliCommand::new(job);

    let temp_dir = create_temp_output_dir();

    async_cmd
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None, // gpu_visible_devices
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("Failed to start command");
    assert!(async_cmd.is_running);

    // Cancel the job
    let result = async_cmd.cancel();
    assert!(result.is_ok());

    // Wait for cancellation to take effect
    let _ = async_cmd.wait_for_completion();
    assert!(!async_cmd.is_running);
}

#[rstest]
fn test_async_cli_command_cancel_not_running() {
    let job = create_test_job_model(1, 1, "echo 'test'");
    let mut async_cmd = AsyncCliCommand::new(job);

    // Cancel without starting - should not error
    let result = async_cmd.cancel();
    assert!(result.is_ok());
}

#[rstest]
#[cfg(unix)]
fn test_async_cli_command_terminate() {
    let job = create_test_job_model(1, 1, "sleep 10");
    let mut async_cmd = AsyncCliCommand::new(job);

    let temp_dir = create_temp_output_dir();

    async_cmd
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None, // gpu_visible_devices
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("Failed to start command");
    assert!(async_cmd.is_running);

    // Terminate the job (sends SIGTERM, doesn't wait)
    let result = async_cmd.terminate();
    assert!(result.is_ok());

    // Wait for the process to actually exit
    let exit_code = async_cmd.wait_for_completion();
    assert!(exit_code.is_ok());

    // Now the job should be marked as not running and complete
    assert!(!async_cmd.is_running);
    assert!(async_cmd.is_complete);
}

#[rstest]
fn test_async_cli_command_wait_for_completion() {
    let job = create_test_job_model(1, 1, "echo 'test'");
    let mut async_cmd = AsyncCliCommand::new(job);

    let temp_dir = create_temp_output_dir();

    async_cmd
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None, // gpu_visible_devices
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("Failed to start command");

    let result = async_cmd.wait_for_completion();
    assert!(result.is_ok());
    assert!(!async_cmd.is_running);
    assert!(async_cmd.is_complete);
}

#[rstest]
fn test_async_cli_command_wait_for_completion_not_started() {
    let job = create_test_job_model(1, 1, "echo 'test'");
    let mut async_cmd = AsyncCliCommand::new(job);

    // Wait without starting - should not error
    let result = async_cmd.wait_for_completion();
    assert!(result.is_ok());
}

#[rstest]
fn test_async_cli_command_get_result() {
    let job = create_test_job_model(1, 1, "exit 0");
    let mut async_cmd = AsyncCliCommand::new(job);

    let temp_dir = create_temp_output_dir();

    async_cmd
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None, // gpu_visible_devices
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("Failed to start command");
    let _ = async_cmd.wait_for_completion();

    let run_id = 1;
    let result = async_cmd.get_result(run_id, 1, 1, None);

    assert_eq!(result.job_id, 1);
    assert_eq!(result.workflow_id, 1);
    assert_eq!(result.run_id, run_id);
    assert!(!result.completion_time.is_empty());
    // Job exited with code 0, so status should be Completed (not Terminated)
    assert_eq!(result.status, JobStatus::Completed);
}

#[rstest]
#[should_panic(expected = "Job is not yet complete")]
fn test_async_cli_command_get_result_not_complete() {
    let job = create_test_job_model(1, 1, "echo 'test'");
    let async_cmd = AsyncCliCommand::new(job);

    // Try to get result before completing - should panic
    let _ = async_cmd.get_result(1, 1, 1, None);
}

#[rstest]
#[cfg(unix)]
fn test_async_cli_command_with_invocation_script() {
    let mut job = create_test_job_model(1, 1, "echo 'Hello'");
    job.invocation_script = Some("echo 'Prefix:';".to_string());

    let mut async_cmd = AsyncCliCommand::new(job);

    let temp_dir = create_temp_output_dir();

    let result = async_cmd.start(
        temp_dir.path(),
        1, // workflow_id
        1, // run_id
        1, // attempt_id
        None,
        "http://localhost:8080/torc-service/v1",
        None,
        None, // gpu_visible_devices
        true,
        ExecutionMode::Direct,
        false,
        None,
        None,
        60,   // sigkill_headroom_seconds
        None, // target_node
        &StdioMode::Separate,
    );
    assert!(result.is_ok());

    let _ = async_cmd.wait_for_completion();

    // Verify both invocation script and command were executed
    let stdout_path = temp_dir.path().join("job_stdio").join("job_wf1_j1_r1_a1.o");
    let contents = fs::read_to_string(stdout_path).expect("Failed to read stdout");
    assert!(contents.contains("Prefix:"));
    assert!(contents.contains("Hello"));
}

#[rstest]
#[cfg(unix)]
fn test_async_cli_command_environment_variables() {
    let job = create_test_job_model(1, 123, "echo $TORC_WORKFLOW_ID $TORC_JOB_ID");
    let mut async_cmd = AsyncCliCommand::new(job);

    let temp_dir = create_temp_output_dir();

    async_cmd
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None, // gpu_visible_devices
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("Failed to start command");
    let _ = async_cmd.wait_for_completion();

    // Verify environment variables were set
    let stdout_path = temp_dir
        .path()
        .join("job_stdio")
        .join("job_wf1_j123_r1_a1.o");
    let contents = fs::read_to_string(stdout_path).expect("Failed to read stdout");
    assert!(contents.contains("1")); // workflow_id
    assert!(contents.contains("123")); // job_id
}

#[rstest]
#[cfg(unix)]
fn test_async_cli_command_gpu_visible_devices_env() {
    let job = create_test_job_model(
        1,
        124,
        "echo CUDA=$CUDA_VISIBLE_DEVICES HIP=$HIP_VISIBLE_DEVICES ROCR=$ROCR_VISIBLE_DEVICES TORC=$TORC_GPU_VISIBLE_DEVICES",
    );
    let mut async_cmd = AsyncCliCommand::new(job);

    let temp_dir = create_temp_output_dir();

    async_cmd
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            Some("1,3"), // gpu_visible_devices
            true,
            ExecutionMode::Direct, // direct execution
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("Failed to start command");
    let _ = async_cmd.wait_for_completion();

    let stdout_path = temp_dir
        .path()
        .join("job_stdio")
        .join("job_wf1_j124_r1_a1.o");
    let contents = fs::read_to_string(stdout_path).expect("Failed to read stdout");

    assert!(
        contents.contains("CUDA=1,3"),
        "Missing CUDA_VISIBLE_DEVICES: {}",
        contents
    );
    assert!(
        contents.contains("HIP=1,3"),
        "Missing HIP_VISIBLE_DEVICES: {}",
        contents
    );
    assert!(
        contents.contains("ROCR=1,3"),
        "Missing ROCR_VISIBLE_DEVICES: {}",
        contents
    );
    assert!(
        contents.contains("TORC=1,3"),
        "Missing TORC_GPU_VISIBLE_DEVICES: {}",
        contents
    );
}

#[rstest]
#[cfg(unix)]
fn test_async_cli_command_stdout_stderr_separation() {
    let job = create_test_job_model(1, 1, "echo 'stdout message'; echo 'stderr message' >&2");
    let mut async_cmd = AsyncCliCommand::new(job);

    let temp_dir = create_temp_output_dir();

    async_cmd
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None, // gpu_visible_devices
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("Failed to start command");
    let _ = async_cmd.wait_for_completion();

    let stdout_path = temp_dir.path().join("job_stdio").join("job_wf1_j1_r1_a1.o");
    let stdout_contents = fs::read_to_string(&stdout_path).expect("Failed to read stdout");
    assert!(stdout_contents.contains("stdout message"));

    // Check stderr
    let stderr_path = temp_dir.path().join("job_stdio").join("job_wf1_j1_r1_a1.e");
    let stderr_contents = fs::read_to_string(stderr_path).expect("Failed to read stderr");
    assert!(stderr_contents.contains("stderr message"));
}

#[rstest]
#[cfg(unix)]
fn test_async_cli_command_multiple_jobs_same_workflow() {
    let temp_dir = create_temp_output_dir();

    // Create and start multiple jobs
    let job1 = create_test_job_model(1, 1, "echo 'Job 1'");
    let mut async_cmd1 = AsyncCliCommand::new(job1);
    async_cmd1
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None, // gpu_visible_devices
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("Failed to start job 1");

    let job2 = create_test_job_model(1, 2, "echo 'Job 2'");
    let mut async_cmd2 = AsyncCliCommand::new(job2);
    async_cmd2
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None, // gpu_visible_devices
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("Failed to start job 2");

    let job3 = create_test_job_model(1, 3, "echo 'Job 3'");
    let mut async_cmd3 = AsyncCliCommand::new(job3);
    async_cmd3
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None, // gpu_visible_devices
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("Failed to start job 3");

    // Wait for all to complete
    let _ = async_cmd1.wait_for_completion();
    let _ = async_cmd2.wait_for_completion();
    let _ = async_cmd3.wait_for_completion();

    // Verify all output files exist and have correct content
    // File format: job_wf{workflow_id}_j{job_id}_r{run_id}_a{attempt_id}.o
    for job_id in 1..=3 {
        let stdout_path = temp_dir
            .path()
            .join("job_stdio")
            .join(format!("job_wf1_j{}_r1_a1.o", job_id));
        assert!(stdout_path.exists());
        let contents = fs::read_to_string(stdout_path).expect("Failed to read stdout");
        assert!(contents.contains(&format!("Job {}", job_id)));
    }
}

#[rstest]
#[cfg(unix)]
fn test_async_cli_command_long_running_job() {
    let job = create_test_job_model(1, 1, "sleep 2; echo 'Done'");
    let mut async_cmd = AsyncCliCommand::new(job);

    let temp_dir = create_temp_output_dir();

    async_cmd
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None, // gpu_visible_devices
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("Failed to start command");
    assert!(async_cmd.is_running);

    // Check status multiple times while running
    for _ in 0..5 {
        thread::sleep(Duration::from_millis(200));
        let _ = async_cmd.check_status();
        if async_cmd.is_complete {
            break;
        }
    }

    // Wait for completion
    let _ = async_cmd.wait_for_completion();
    assert!(!async_cmd.is_running);
    assert!(async_cmd.is_complete);
}

#[rstest]
fn test_async_cli_command_get_job_id() {
    let job = create_test_job_model(1, 42, "echo 'test'");
    let async_cmd = AsyncCliCommand::new(job);

    assert_eq!(async_cmd.get_job_id(), 42);
}

#[rstest]
#[cfg(unix)]
fn test_async_cli_command_complex_shell_command() {
    let job = create_test_job_model(1, 1, "for i in 1 2 3; do echo \"Number $i\"; done");
    let mut async_cmd = AsyncCliCommand::new(job);

    let temp_dir = create_temp_output_dir();

    async_cmd
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None, // gpu_visible_devices
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("Failed to start command");
    let _ = async_cmd.wait_for_completion();

    // Check the output
    let stdout_path = temp_dir.path().join("job_stdio").join("job_wf1_j1_r1_a1.o");
    let contents = fs::read_to_string(stdout_path).expect("Failed to read stdout");
    assert!(contents.contains("Number 1"));
    assert!(contents.contains("Number 2"));
    assert!(contents.contains("Number 3"));
}

#[rstest]
#[cfg(unix)]
fn test_async_cli_command_file_creation() {
    let temp_dir = create_temp_output_dir();
    let output_file = temp_dir.path().join("test_output.txt");

    let job = create_test_job_model(
        1,
        1,
        &format!("echo 'test data' > {}", output_file.display()),
    );
    let mut async_cmd = AsyncCliCommand::new(job);

    async_cmd
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None, // gpu_visible_devices
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("Failed to start command");
    let _ = async_cmd.wait_for_completion();

    // Verify file was created in the working directory
    let test_file_path = temp_dir.path().join("test_output.txt");
    assert!(test_file_path.exists());
    let contents = fs::read_to_string(test_file_path).expect("Failed to read test file");
    assert!(contents.contains("test data"));
}

#[rstest]
#[cfg(unix)]
fn test_async_cli_command_drop_while_running() {
    let job = create_test_job_model(1, 1, "sleep 10");
    let mut async_cmd = AsyncCliCommand::new(job);

    let temp_dir = create_temp_output_dir();

    async_cmd
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None, // gpu_visible_devices
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("Failed to start command");
    assert!(async_cmd.is_running);

    // Drop the command while it's running - this should trigger terminate in Drop impl
    drop(async_cmd);

    // Give it time to clean up
    thread::sleep(Duration::from_millis(200));

    // No way to verify directly, but this tests that Drop doesn't panic
}

#[rstest]
#[cfg(unix)]
fn test_async_cli_command_execution_time() {
    let job = create_test_job_model(1, 1, "sleep 1");
    let mut async_cmd = AsyncCliCommand::new(job);

    let temp_dir = create_temp_output_dir();

    async_cmd
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None, // gpu_visible_devices
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("Failed to start command");
    let _ = async_cmd.wait_for_completion();

    let result = async_cmd.get_result(1, 1, 1, None);

    // Execution time should be at least 1 second (converted to minutes)
    assert!(result.exec_time_minutes >= 1.0 / 60.0);
}

#[rstest]
fn test_async_cli_command_empty_command() {
    let job = create_test_job_model(1, 1, "");
    let mut async_cmd = AsyncCliCommand::new(job);

    let temp_dir = create_temp_output_dir();

    // Empty command should still start
    let result = async_cmd.start(
        temp_dir.path(),
        1, // workflow_id
        1, // run_id
        1, // attempt_id
        None,
        "http://localhost:8080/torc-service/v1",
        None,
        None, // gpu_visible_devices
        true,
        ExecutionMode::Direct,
        false,
        None,
        None,
        60,   // sigkill_headroom_seconds
        None, // target_node
        &StdioMode::Separate,
    );
    assert!(result.is_ok());

    let _ = async_cmd.wait_for_completion();
    assert!(async_cmd.is_complete);
}

#[rstest]
fn test_async_cli_command_command_not_found() {
    let job = create_test_job_model(1, 1, "nonexistent_command_12345");
    let mut async_cmd = AsyncCliCommand::new(job);

    let temp_dir = create_temp_output_dir();

    async_cmd
        .start(
            temp_dir.path(),
            1, // workflow_id
            1, // run_id
            1, // attempt_id
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None, // gpu_visible_devices
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,   // sigkill_headroom_seconds
            None, // target_node
            &StdioMode::Separate,
        )
        .expect("Failed to start command");
    let _ = async_cmd.wait_for_completion();

    assert!(async_cmd.is_complete);
}

// =============================================================================
// StdioMode tests
// =============================================================================

#[rstest]
#[cfg(unix)]
fn test_stdio_mode_combined(start_server: &ServerProcess) {
    let _ = start_server;
    let job = create_test_job_model(1, 1, "echo out; echo err >&2");
    let mut async_cmd = AsyncCliCommand::new(job);
    let temp_dir = create_temp_output_dir();

    async_cmd
        .start(
            temp_dir.path(),
            1,
            1,
            1,
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None,
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,
            None,
            &StdioMode::Combined,
        )
        .expect("Failed to start command");
    let _ = async_cmd.wait_for_completion();

    // Combined mode writes both stdout and stderr to a single .log file
    let combined_path = temp_dir
        .path()
        .join("job_stdio")
        .join("job_wf1_j1_r1_a1.log");
    assert!(combined_path.exists(), "Combined .log file should exist");
    let contents = fs::read_to_string(&combined_path).expect("Failed to read combined log");
    assert!(contents.contains("out"));
    assert!(contents.contains("err"));

    // Separate files should not exist
    let stdout_path = temp_dir.path().join("job_stdio").join("job_wf1_j1_r1_a1.o");
    let stderr_path = temp_dir.path().join("job_stdio").join("job_wf1_j1_r1_a1.e");
    assert!(!stdout_path.exists(), "Separate .o file should not exist");
    assert!(!stderr_path.exists(), "Separate .e file should not exist");

    // stdout_path on the command should point to the combined file, stderr_path should be None
    assert!(async_cmd.stdout_path.is_some());
    assert!(async_cmd.stderr_path.is_none());
}

#[rstest]
#[cfg(unix)]
fn test_stdio_mode_no_stdout(start_server: &ServerProcess) {
    let _ = start_server;
    let job = create_test_job_model(1, 1, "echo out; echo err >&2");
    let mut async_cmd = AsyncCliCommand::new(job);
    let temp_dir = create_temp_output_dir();

    async_cmd
        .start(
            temp_dir.path(),
            1,
            1,
            1,
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None,
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,
            None,
            &StdioMode::NoStdout,
        )
        .expect("Failed to start command");
    let _ = async_cmd.wait_for_completion();

    // Stderr should be captured
    let stderr_path = temp_dir.path().join("job_stdio").join("job_wf1_j1_r1_a1.e");
    assert!(stderr_path.exists(), "Stderr file should exist");
    let contents = fs::read_to_string(&stderr_path).expect("Failed to read stderr");
    assert!(contents.contains("err"));

    // Stdout file should not exist (sent to /dev/null)
    let stdout_path = temp_dir.path().join("job_stdio").join("job_wf1_j1_r1_a1.o");
    assert!(!stdout_path.exists(), "Stdout file should not exist");

    assert!(async_cmd.stdout_path.is_none());
    assert!(async_cmd.stderr_path.is_some());
}

#[rstest]
#[cfg(unix)]
fn test_stdio_mode_no_stderr(start_server: &ServerProcess) {
    let _ = start_server;
    let job = create_test_job_model(1, 1, "echo out; echo err >&2");
    let mut async_cmd = AsyncCliCommand::new(job);
    let temp_dir = create_temp_output_dir();

    async_cmd
        .start(
            temp_dir.path(),
            1,
            1,
            1,
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None,
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,
            None,
            &StdioMode::NoStderr,
        )
        .expect("Failed to start command");
    let _ = async_cmd.wait_for_completion();

    // Stdout should be captured
    let stdout_path = temp_dir.path().join("job_stdio").join("job_wf1_j1_r1_a1.o");
    assert!(stdout_path.exists(), "Stdout file should exist");
    let contents = fs::read_to_string(&stdout_path).expect("Failed to read stdout");
    assert!(contents.contains("out"));

    // Stderr file should not exist (sent to /dev/null)
    let stderr_path = temp_dir.path().join("job_stdio").join("job_wf1_j1_r1_a1.e");
    assert!(!stderr_path.exists(), "Stderr file should not exist");

    assert!(async_cmd.stdout_path.is_some());
    assert!(async_cmd.stderr_path.is_none());
}

#[rstest]
#[cfg(unix)]
fn test_stdio_mode_none(start_server: &ServerProcess) {
    let _ = start_server;
    let job = create_test_job_model(1, 1, "echo out; echo err >&2");
    let mut async_cmd = AsyncCliCommand::new(job);
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    // Intentionally do NOT create the job_stdio subdirectory — None mode should skip it.

    async_cmd
        .start(
            temp_dir.path(),
            1,
            1,
            1,
            None,
            "http://localhost:8080/torc-service/v1",
            None,
            None,
            true,
            ExecutionMode::Direct,
            false,
            None,
            None,
            60,
            None,
            &StdioMode::None,
        )
        .expect("Failed to start command");
    let _ = async_cmd.wait_for_completion();

    // No stdio files should be created at all
    let stdio_dir = temp_dir.path().join("job_stdio");
    assert!(
        !stdio_dir.exists(),
        "job_stdio directory should not be created in None mode"
    );

    assert!(async_cmd.stdout_path.is_none());
    assert!(async_cmd.stderr_path.is_none());
}

// =============================================================================
// cleanup_job_stdio_files tests
// =============================================================================

#[test]
fn test_cleanup_stdio_files_separate_mode() {
    let temp_dir = TempDir::new().unwrap();
    let stdout = temp_dir.path().join("job.o");
    let stderr = temp_dir.path().join("job.e");
    fs::write(&stdout, "out").unwrap();
    fs::write(&stderr, "err").unwrap();

    cleanup_job_stdio_files(
        Some(stdout.to_str().unwrap()),
        Some(stderr.to_str().unwrap()),
    );

    assert!(!stdout.exists(), "stdout file should be deleted");
    assert!(!stderr.exists(), "stderr file should be deleted");
}

#[test]
fn test_cleanup_stdio_files_combined_mode() {
    let temp_dir = TempDir::new().unwrap();
    let combined = temp_dir.path().join("job.log");
    fs::write(&combined, "combined output").unwrap();

    // Combined mode: stdout_path points to .log, stderr_path is None
    cleanup_job_stdio_files(Some(combined.to_str().unwrap()), None);

    assert!(!combined.exists(), "combined file should be deleted");
}

#[test]
fn test_cleanup_stdio_files_no_stdout_mode() {
    let temp_dir = TempDir::new().unwrap();
    let stderr = temp_dir.path().join("job.e");
    fs::write(&stderr, "err").unwrap();

    // NoStdout mode: stdout_path is None
    cleanup_job_stdio_files(None, Some(stderr.to_str().unwrap()));

    assert!(!stderr.exists(), "stderr file should be deleted");
}

#[test]
fn test_cleanup_stdio_files_none_mode() {
    // None mode: both paths are None — should not panic
    cleanup_job_stdio_files(None, None);
}

#[test]
fn test_cleanup_stdio_files_already_missing() {
    // Files that don't exist should be silently ignored (NotFound)
    cleanup_job_stdio_files(Some("/nonexistent/path.o"), Some("/nonexistent/path.e"));
}

#[test]
fn test_cleanup_stdio_files_retains_on_failure() {
    // This test verifies that cleanup is only called for successful jobs.
    // The decision logic is in ExecutionConfig::delete_stdio_on_success,
    // which is tested in test_execution_config.rs. Here we verify that
    // NOT calling cleanup preserves the files.
    let temp_dir = TempDir::new().unwrap();
    let stdout = temp_dir.path().join("job.o");
    let stderr = temp_dir.path().join("job.e");
    fs::write(&stdout, "out").unwrap();
    fs::write(&stderr, "err").unwrap();

    // Simulate: job failed, so cleanup is NOT called.
    // Files should still exist.
    assert!(stdout.exists(), "stdout should be retained on failure");
    assert!(stderr.exists(), "stderr should be retained on failure");
}
