//! Tests for srun argument construction in `AsyncCliCommand::start()`.
//!
//! These tests verify that the correct srun arguments are generated for all
//! combinations of Slurm configuration options:
//! - `use_srun` (true/false)
//! - `limit_resources` (true/false)
//! - `enable_cpu_bind` (true/false)
//! - `srun_termination_signal` (set/unset)
//! - `num_nodes` (1/N)
//! - `end_time` (set/unset)
//!
//! Each test sets `SLURM_JOB_ID` and `TORC_FAKE_SRUN` to a script that logs
//! all arguments, then asserts the logged arguments contain/omit expected flags.

use serial_test::serial;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;
use torc::client::async_cli_command::AsyncCliCommand;
use torc::client::workflow_spec::{ExecutionMode, StdioMode};
use torc::models::{JobModel, JobStatus, ResourceRequirementsModel};

/// Create a minimal JobModel for testing.
fn make_job(job_id: i64, command: &str) -> JobModel {
    let mut job = JobModel::new(1, format!("test_job_{}", job_id), command.to_string());
    job.id = Some(job_id);
    job.status = Some(JobStatus::Ready);
    job
}

/// Create a ResourceRequirementsModel with the given parameters.
fn make_rr(name: &str, num_cpus: i64, memory: &str, num_nodes: i64) -> ResourceRequirementsModel {
    make_rr_with_gpus(name, num_cpus, memory, num_nodes, 0)
}

/// Create a ResourceRequirementsModel with the given parameters including GPUs.
fn make_rr_with_gpus(
    name: &str,
    num_cpus: i64,
    memory: &str,
    num_nodes: i64,
    num_gpus: i64,
) -> ResourceRequirementsModel {
    ResourceRequirementsModel {
        id: Some(1),
        workflow_id: 1,
        name: name.to_string(),
        num_cpus,
        num_gpus,
        num_nodes,
        memory: memory.to_string(),
        runtime: "PT30M".to_string(),
    }
}

/// Path to the fake srun that logs arguments.
fn fake_srun_path() -> PathBuf {
    let current_dir = env::current_dir().expect("Failed to get current directory");
    current_dir.join("tests/scripts/fake_srun_log_args.sh")
}

/// Set up environment for srun tests. Returns the temp dir and args log path.
/// The temp dir must be kept alive for the duration of the test.
fn setup_srun_env(temp_dir: &TempDir) -> PathBuf {
    let args_log = temp_dir.path().join("srun_args.log");

    unsafe {
        env::set_var("SLURM_JOB_ID", "99999");
        env::set_var(
            "TORC_FAKE_SRUN",
            fake_srun_path().to_string_lossy().to_string(),
        );
        env::set_var("TORC_SRUN_ARGS_LOG", args_log.to_string_lossy().to_string());
    }

    args_log
}

/// Clean up srun-related environment variables.
fn cleanup_srun_env() {
    unsafe {
        env::remove_var("SLURM_JOB_ID");
        env::remove_var("TORC_FAKE_SRUN");
        env::remove_var("TORC_SRUN_ARGS_LOG");
    }
}

/// Run a job with the given srun configuration and return the logged srun arguments.
#[allow(clippy::too_many_arguments)]
fn run_and_capture_srun_args(
    temp_dir: &TempDir,
    args_log: &PathBuf,
    rr: Option<&ResourceRequirementsModel>,
    limit_resources: bool,
    execution_mode: ExecutionMode,
    enable_cpu_bind: bool,
    end_time: Option<chrono::DateTime<chrono::Utc>>,
    srun_termination_signal: Option<&str>,
) -> Option<String> {
    run_and_capture_srun_args_with_headroom(
        temp_dir,
        args_log,
        rr,
        limit_resources,
        execution_mode,
        enable_cpu_bind,
        end_time,
        srun_termination_signal,
        60, // default sigkill_headroom_seconds
    )
}

/// Run a job with the given srun configuration and custom headroom, returning logged srun args.
#[allow(clippy::too_many_arguments)]
fn run_and_capture_srun_args_with_headroom(
    temp_dir: &TempDir,
    args_log: &PathBuf,
    rr: Option<&ResourceRequirementsModel>,
    limit_resources: bool,
    execution_mode: ExecutionMode,
    enable_cpu_bind: bool,
    end_time: Option<chrono::DateTime<chrono::Utc>>,
    srun_termination_signal: Option<&str>,
    sigkill_headroom_seconds: i64,
) -> Option<String> {
    let job = make_job(1, "echo hello");
    let mut cmd = AsyncCliCommand::new(job);

    let result = cmd.start(
        temp_dir.path(),
        1, // workflow_id
        1, // run_id
        1, // attempt_id
        None,
        "http://localhost:8080/torc-service/v1",
        rr,
        None, // gpu_visible_devices
        limit_resources,
        execution_mode,
        enable_cpu_bind,
        end_time,
        srun_termination_signal,
        sigkill_headroom_seconds,
        None, // target_node
        &StdioMode::Separate,
    );
    assert!(
        result.is_ok(),
        "Failed to start command: {:?}",
        result.err()
    );

    // Poll until the process finishes (up to 10s).
    for _ in 0..100 {
        let _ = cmd.check_status();
        if !cmd.is_running {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    if args_log.exists() {
        Some(fs::read_to_string(args_log).expect("Failed to read srun args log"))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn test_srun_default_single_node() {
    let temp_dir = TempDir::new().unwrap();
    let args_log = setup_srun_env(&temp_dir);
    let rr = make_rr("compute", 4, "8g", 1);

    let args = run_and_capture_srun_args(
        &temp_dir,
        &args_log,
        Some(&rr),
        true,                 // limit_resources
        ExecutionMode::Slurm, // execution_mode
        false,                // enable_cpu_bind (default)
        None,                 // end_time
        None,                 // srun_termination_signal
    )
    .expect("srun should have been invoked");

    cleanup_srun_env();

    // Should have --jobid
    assert!(args.contains("--jobid=99999"), "Missing --jobid: {}", args);
    // Should have --ntasks=1
    assert!(args.contains("--ntasks=1"), "Missing --ntasks=1: {}", args);
    // Should have --cpu-bind=none (enable_cpu_bind=false)
    assert!(
        args.contains("--cpu-bind=none"),
        "Missing --cpu-bind=none: {}",
        args
    );
    // Should have --exact
    assert!(args.contains("--exact"), "Missing --exact: {}", args);
    // Should have --nodes=1 (num_nodes=1)
    assert!(args.contains("--nodes=1"), "Missing --nodes=1: {}", args);
    // Should have --cpus-per-task=4 (limit_resources=true, name != "default")
    assert!(
        args.contains("--cpus-per-task=4"),
        "Missing --cpus-per-task=4: {}",
        args
    );
    // Should have --mem=8192M (8g = 8192 MB)
    assert!(
        args.contains("--mem=8192M"),
        "Missing --mem=8192M: {}",
        args
    );
    // Should NOT have --signal (not set)
    assert!(!args.contains("--signal"), "Unexpected --signal: {}", args);
    // Should NOT have --time (no end_time)
    assert!(!args.contains("--time="), "Unexpected --time: {}", args);
    // Should end with bash -c <command>
    assert!(args.contains("bash -c"), "Missing 'bash -c': {}", args);
}

// NOTE: test_srun_no_resource_limits was removed because limit_resources=false
// is now only valid in direct mode (which doesn't use srun).

#[test]
#[serial]
fn test_srun_cpu_bind_enabled() {
    let temp_dir = TempDir::new().unwrap();
    let args_log = setup_srun_env(&temp_dir);
    let rr = make_rr("compute", 2, "4g", 1);

    let args = run_and_capture_srun_args(
        &temp_dir,
        &args_log,
        Some(&rr),
        true,                 // limit_resources
        ExecutionMode::Slurm, // execution_mode
        true,                 // enable_cpu_bind = true
        None,
        None,
    )
    .expect("srun should have been invoked");

    cleanup_srun_env();

    // --cpu-bind=none should NOT be present when cpu_bind is enabled
    assert!(
        !args.contains("--cpu-bind"),
        "Unexpected --cpu-bind with enable_cpu_bind=true: {}",
        args
    );
    // Other flags should still be present
    assert!(args.contains("--exact"), "Missing --exact: {}", args);
    assert!(
        args.contains("--cpus-per-task=2"),
        "Missing --cpus-per-task=2: {}",
        args
    );
}

#[test]
#[serial]
fn test_srun_termination_signal() {
    let temp_dir = TempDir::new().unwrap();
    let args_log = setup_srun_env(&temp_dir);
    let rr = make_rr("compute", 2, "4g", 1);

    let args = run_and_capture_srun_args(
        &temp_dir,
        &args_log,
        Some(&rr),
        true,                 // limit_resources
        ExecutionMode::Slurm, // execution_mode
        false,                // enable_cpu_bind
        None,
        Some("TERM@120"), // srun_termination_signal
    )
    .expect("srun should have been invoked");

    cleanup_srun_env();

    assert!(
        args.contains("--signal=TERM@120"),
        "Missing --signal=TERM@120: {}",
        args
    );
}

#[test]
#[serial]
fn test_srun_termination_signal_usr1() {
    let temp_dir = TempDir::new().unwrap();
    let args_log = setup_srun_env(&temp_dir);
    let rr = make_rr("compute", 2, "4g", 1);

    let args = run_and_capture_srun_args(
        &temp_dir,
        &args_log,
        Some(&rr),
        true,
        ExecutionMode::Slurm,
        false,
        None,
        Some("USR1@60"),
    )
    .expect("srun should have been invoked");

    cleanup_srun_env();

    assert!(
        args.contains("--signal=USR1@60"),
        "Missing --signal=USR1@60: {}",
        args
    );
}

#[test]
#[serial]
fn test_srun_multi_node_step() {
    let temp_dir = TempDir::new().unwrap();
    let args_log = setup_srun_env(&temp_dir);
    let rr = make_rr("mpi_compute", 8, "16g", 4);

    let args = run_and_capture_srun_args(
        &temp_dir,
        &args_log,
        Some(&rr),
        true,                 // limit_resources
        ExecutionMode::Slurm, // execution_mode
        false,                // enable_cpu_bind
        None,
        None,
    )
    .expect("srun should have been invoked");

    cleanup_srun_env();

    assert!(
        args.contains("--nodes=4"),
        "Missing --nodes=4 for num_nodes=4: {}",
        args
    );
    assert!(
        args.contains("--cpus-per-task=8"),
        "Missing --cpus-per-task=8: {}",
        args
    );
    assert!(
        args.contains("--mem=16384M"),
        "Missing --mem=16384M: {}",
        args
    );
}

#[test]
#[serial]
fn test_srun_with_end_time() {
    let temp_dir = TempDir::new().unwrap();
    let args_log = setup_srun_env(&temp_dir);
    let rr = make_rr("compute", 2, "4g", 1);

    // Set end_time 30 minutes from now
    let end_time = chrono::Utc::now() + chrono::Duration::minutes(30);

    let args = run_and_capture_srun_args(
        &temp_dir,
        &args_log,
        Some(&rr),
        true,
        ExecutionMode::Slurm,
        false,
        Some(end_time),
        None,
    )
    .expect("srun should have been invoked");

    cleanup_srun_env();

    // Should have --time= with approximately 28-29 minutes:
    // - Start with 30 minutes
    // - Subtract a few seconds for test setup → ~29 minutes
    // - Subtract 1 minute headroom (srun ends 1 min before allocation) → ~28 minutes
    assert!(
        args.contains("--time=30") || args.contains("--time=29") || args.contains("--time=28"),
        "Missing --time=28-30 for 30-minute end_time: {}",
        args
    );
}

#[test]
#[serial]
fn test_srun_with_end_time_insufficient_time_rejected() {
    let temp_dir = TempDir::new().unwrap();
    let _args_log = setup_srun_env(&temp_dir);
    let rr = make_rr("compute", 2, "4g", 1);

    // Set end_time 10 seconds from now — with 60s default headroom, usable time
    // is negative, so the launch should be refused.
    let end_time = chrono::Utc::now() + chrono::Duration::seconds(10);

    let job = make_job(1, "echo hello");
    let mut cmd = AsyncCliCommand::new(job);

    let result = cmd.start(
        temp_dir.path(),
        1,
        1,
        1,
        None,
        "http://localhost:8080/torc-service/v1",
        Some(&rr),
        None, // gpu_visible_devices
        true,
        ExecutionMode::Slurm,
        false,
        Some(end_time),
        None,
        60,
        None,
        &StdioMode::Separate,
    );

    cleanup_srun_env();

    assert!(
        result.is_err(),
        "Should refuse to launch srun step when insufficient time remains"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Refusing to launch"),
        "Error should mention refusing to launch: {}",
        err_msg
    );
}

#[test]
#[serial]
fn test_srun_use_srun_false() {
    let temp_dir = TempDir::new().unwrap();
    let args_log = setup_srun_env(&temp_dir);
    let rr = make_rr("compute", 4, "8g", 1);

    let args = run_and_capture_srun_args(
        &temp_dir,
        &args_log,
        Some(&rr),
        true,                  // limit_resources
        ExecutionMode::Direct, // execution_mode = direct
        false,
        None,
        None,
    );

    cleanup_srun_env();

    // When use_srun=false, srun should NOT be invoked at all
    assert!(
        args.is_none(),
        "srun should not have been invoked when use_srun=false, but got: {:?}",
        args
    );
}

#[test]
#[serial]
fn test_srun_default_resource_requirements_name() {
    let temp_dir = TempDir::new().unwrap();
    let args_log = setup_srun_env(&temp_dir);
    // Use name "default" — the code skips --cpus-per-task and --mem for this name
    let rr = make_rr("default", 4, "8g", 1);

    let args = run_and_capture_srun_args(
        &temp_dir,
        &args_log,
        Some(&rr),
        true,                 // limit_resources = true
        ExecutionMode::Slurm, // execution_mode
        false,                // enable_cpu_bind
        None,
        None,
    )
    .expect("srun should have been invoked");

    cleanup_srun_env();

    // Even with limit_resources=true, "default" RR name should skip resource args
    assert!(
        !args.contains("--cpus-per-task"),
        "Unexpected --cpus-per-task for 'default' RR name: {}",
        args
    );
    assert!(
        !args.contains("--mem="),
        "Unexpected --mem for 'default' RR name: {}",
        args
    );
    // Other flags should still be present
    assert!(args.contains("--nodes=1"), "Missing --nodes=1: {}", args);
    assert!(args.contains("--exact"), "Missing --exact: {}", args);
}

#[test]
#[serial]
fn test_srun_no_resource_requirements() {
    let temp_dir = TempDir::new().unwrap();
    let args_log = setup_srun_env(&temp_dir);

    let args = run_and_capture_srun_args(
        &temp_dir,
        &args_log,
        None,                 // no resource requirements
        true,                 // limit_resources
        ExecutionMode::Slurm, // execution_mode
        false,                // enable_cpu_bind
        None,
        None,
    )
    .expect("srun should have been invoked");

    cleanup_srun_env();

    // Without RR, should not have --nodes, --cpus-per-task, or --mem
    assert!(
        !args.contains("--nodes="),
        "Unexpected --nodes without RR: {}",
        args
    );
    assert!(
        !args.contains("--cpus-per-task"),
        "Unexpected --cpus-per-task without RR: {}",
        args
    );
    assert!(
        !args.contains("--mem="),
        "Unexpected --mem without RR: {}",
        args
    );
    // Core flags should still be present
    assert!(args.contains("--jobid=99999"), "Missing --jobid: {}", args);
    assert!(args.contains("--ntasks=1"), "Missing --ntasks=1: {}", args);
    assert!(args.contains("--exact"), "Missing --exact: {}", args);
}

#[test]
#[serial]
fn test_srun_all_options_set() {
    let temp_dir = TempDir::new().unwrap();
    let args_log = setup_srun_env(&temp_dir);
    let rr = make_rr("gpu_compute", 16, "32g", 2);

    let end_time = chrono::Utc::now() + chrono::Duration::hours(2);

    let args = run_and_capture_srun_args(
        &temp_dir,
        &args_log,
        Some(&rr),
        true,                 // limit_resources
        ExecutionMode::Slurm, // execution_mode
        true,                 // enable_cpu_bind
        Some(end_time),       // end_time
        Some("USR1@60"),      // srun_termination_signal
    )
    .expect("srun should have been invoked");

    cleanup_srun_env();

    assert!(args.contains("--jobid=99999"), "Missing --jobid: {}", args);
    assert!(args.contains("--ntasks=1"), "Missing --ntasks=1: {}", args);
    assert!(args.contains("--exact"), "Missing --exact: {}", args);
    assert!(args.contains("--nodes=2"), "Missing --nodes=2: {}", args);
    assert!(
        args.contains("--cpus-per-task=16"),
        "Missing --cpus-per-task=16: {}",
        args
    );
    assert!(
        args.contains("--mem=32768M"),
        "Missing --mem=32768M: {}",
        args
    );
    assert!(
        args.contains("--signal=USR1@60"),
        "Missing --signal=USR1@60: {}",
        args
    );
    assert!(
        args.contains("--time="),
        "Missing --time for end_time: {}",
        args
    );
    // cpu_bind=true → no --cpu-bind=none
    assert!(
        !args.contains("--cpu-bind"),
        "Unexpected --cpu-bind with enable_cpu_bind=true: {}",
        args
    );
}

#[test]
#[serial]
fn test_srun_step_name_format() {
    let temp_dir = TempDir::new().unwrap();
    let args_log = setup_srun_env(&temp_dir);
    let rr = make_rr("compute", 2, "4g", 1);

    let job = make_job(42, "echo hello");
    let mut cmd = AsyncCliCommand::new(job);

    let result = cmd.start(
        temp_dir.path(),
        100, // workflow_id
        3,   // run_id
        5,   // attempt_id
        None,
        "http://localhost:8080/torc-service/v1",
        Some(&rr),
        None, // gpu_visible_devices
        true,
        ExecutionMode::Slurm,
        false,
        None,
        None,
        60,   // sigkill_headroom_seconds
        None, // target_node
        &StdioMode::Separate,
    );
    assert!(result.is_ok());

    thread::sleep(Duration::from_millis(500));
    let _ = cmd.check_status();

    let args = fs::read_to_string(&args_log).expect("Failed to read args log");
    cleanup_srun_env();

    // Step name format: wf{workflow_id}_j{job_id}_r{run_id}_a{attempt_id}
    assert!(
        args.contains("--job-name=wf100_j42_r3_a5"),
        "Missing expected step name --job-name=wf100_j42_r3_a5: {}",
        args
    );
}

// NOTE: test_srun_limit_resources_false_with_cpu_bind_and_signal was removed because
// limit_resources=false is now only valid in direct mode (which doesn't use srun).

#[test]
#[serial]
fn test_srun_small_memory_rounds_up_to_1mb() {
    let temp_dir = TempDir::new().unwrap();
    let args_log = setup_srun_env(&temp_dir);
    // Sub-MB memory value; memory_string_to_mb clamps non-zero values to at least 1 MB
    let rr = make_rr("tiny", 1, "100k", 1);

    let args = run_and_capture_srun_args(
        &temp_dir,
        &args_log,
        Some(&rr),
        true,                 // limit_resources
        ExecutionMode::Slurm, // execution_mode
        false,                // enable_cpu_bind
        None,
        None,
    )
    .expect("srun should have been invoked");

    cleanup_srun_env();

    // 100k < 1MB but memory_string_to_mb clamps to 1 MB minimum for non-zero values
    assert!(
        args.contains("--mem=1M"),
        "Sub-MB memory should round up to --mem=1M: {}",
        args
    );
    assert!(
        args.contains("--cpus-per-task=1"),
        "Missing --cpus-per-task=1: {}",
        args
    );
}

#[test]
#[serial]
fn test_srun_zero_memory_omits_mem() {
    let temp_dir = TempDir::new().unwrap();
    let args_log = setup_srun_env(&temp_dir);
    // Zero memory — should omit --mem to avoid --mem=0 which in Slurm means "all memory"
    let rr = make_rr("zero_mem", 1, "0m", 1);

    let args = run_and_capture_srun_args(
        &temp_dir,
        &args_log,
        Some(&rr),
        true,                 // limit_resources
        ExecutionMode::Slurm, // execution_mode
        false,                // enable_cpu_bind
        None,
        None,
    )
    .expect("srun should have been invoked");

    cleanup_srun_env();

    // 0m = 0 MB, should omit --mem
    assert!(
        !args.contains("--mem="),
        "Should omit --mem for zero memory: {}",
        args
    );
    assert!(
        args.contains("--cpus-per-task=1"),
        "Missing --cpus-per-task=1: {}",
        args
    );
}

#[test]
#[serial]
fn test_srun_with_gpus() {
    let temp_dir = TempDir::new().unwrap();
    let args_log = setup_srun_env(&temp_dir);
    // Request 2 GPUs
    let rr = make_rr_with_gpus("gpu_compute", 8, "32g", 1, 2);

    let args = run_and_capture_srun_args(
        &temp_dir,
        &args_log,
        Some(&rr),
        true,                 // limit_resources
        ExecutionMode::Slurm, // execution_mode
        false,                // enable_cpu_bind
        None,
        None,
    )
    .expect("srun should have been invoked");

    cleanup_srun_env();

    // Should have --gpus=2 for the 2 GPUs requested
    assert!(
        args.contains("--gpus=2"),
        "Missing --gpus=2 for job with 2 GPUs: {}",
        args
    );
    // Other resource flags should still be present
    assert!(
        args.contains("--cpus-per-task=8"),
        "Missing --cpus-per-task=8: {}",
        args
    );
    assert!(
        args.contains("--mem=32768M"),
        "Missing --mem=32768M: {}",
        args
    );
}

#[test]
#[serial]
fn test_srun_zero_gpus_omits_flag() {
    let temp_dir = TempDir::new().unwrap();
    let args_log = setup_srun_env(&temp_dir);
    // Request 0 GPUs (default)
    let rr = make_rr_with_gpus("cpu_only", 4, "8g", 1, 0);

    let args = run_and_capture_srun_args(
        &temp_dir,
        &args_log,
        Some(&rr),
        true,                 // limit_resources
        ExecutionMode::Slurm, // execution_mode
        false,                // enable_cpu_bind
        None,
        None,
    )
    .expect("srun should have been invoked");

    cleanup_srun_env();

    // Should NOT have --gpus when num_gpus=0
    assert!(
        !args.contains("--gpus"),
        "Unexpected --gpus flag for job with 0 GPUs: {}",
        args
    );
    // Other resource flags should still be present
    assert!(
        args.contains("--cpus-per-task=4"),
        "Missing --cpus-per-task=4: {}",
        args
    );
}

// NOTE: test_srun_gpus_with_limit_resources_false was removed because
// limit_resources=false is now only valid in direct mode (which doesn't use srun).
