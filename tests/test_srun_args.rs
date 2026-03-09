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
    ResourceRequirementsModel {
        id: Some(1),
        workflow_id: 1,
        name: name.to_string(),
        num_cpus,
        num_gpus: 0,
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
    use_srun: bool,
    enable_cpu_bind: bool,
    end_time: Option<chrono::DateTime<chrono::Utc>>,
    srun_termination_signal: Option<&str>,
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
        limit_resources,
        use_srun,
        enable_cpu_bind,
        end_time,
        srun_termination_signal,
        None, // target_node
    );
    assert!(
        result.is_ok(),
        "Failed to start command: {:?}",
        result.err()
    );

    // Wait for the job to finish
    thread::sleep(Duration::from_millis(500));
    let _ = cmd.check_status();

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
        true,  // limit_resources
        true,  // use_srun
        false, // enable_cpu_bind (default)
        None,  // end_time
        None,  // srun_termination_signal
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

#[test]
#[serial]
fn test_srun_no_resource_limits() {
    let temp_dir = TempDir::new().unwrap();
    let args_log = setup_srun_env(&temp_dir);
    let rr = make_rr("compute", 4, "8g", 1);

    let args = run_and_capture_srun_args(
        &temp_dir,
        &args_log,
        Some(&rr),
        false, // limit_resources = false
        true,  // use_srun
        false, // enable_cpu_bind
        None,
        None,
    )
    .expect("srun should have been invoked");

    cleanup_srun_env();

    // Core srun flags should still be present
    assert!(args.contains("--jobid=99999"), "Missing --jobid: {}", args);
    assert!(args.contains("--ntasks=1"), "Missing --ntasks=1: {}", args);
    assert!(args.contains("--exact"), "Missing --exact: {}", args);
    assert!(args.contains("--nodes=1"), "Missing --nodes=1: {}", args);

    // Resource-limiting flags should be ABSENT
    assert!(
        !args.contains("--cpus-per-task"),
        "Unexpected --cpus-per-task with limit_resources=false: {}",
        args
    );
    assert!(
        !args.contains("--mem="),
        "Unexpected --mem with limit_resources=false: {}",
        args
    );
}

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
        true, // limit_resources
        true, // use_srun
        true, // enable_cpu_bind = true
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
        true,  // limit_resources
        true,  // use_srun
        false, // enable_cpu_bind
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
        true,
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
        true,  // limit_resources
        true,  // use_srun
        false, // enable_cpu_bind
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
        true,
        false,
        Some(end_time),
        None,
    )
    .expect("srun should have been invoked");

    cleanup_srun_env();

    // Should have --time= with approximately 30 (could be 29 due to rounding down)
    assert!(
        args.contains("--time=30") || args.contains("--time=29"),
        "Missing --time=~30 for 30-minute end_time: {}",
        args
    );
}

#[test]
#[serial]
fn test_srun_with_end_time_minimum_one_minute() {
    let temp_dir = TempDir::new().unwrap();
    let args_log = setup_srun_env(&temp_dir);
    let rr = make_rr("compute", 2, "4g", 1);

    // Set end_time 10 seconds from now (should clamp to minimum 1 minute)
    let end_time = chrono::Utc::now() + chrono::Duration::seconds(10);

    let args = run_and_capture_srun_args(
        &temp_dir,
        &args_log,
        Some(&rr),
        true,
        true,
        false,
        Some(end_time),
        None,
    )
    .expect("srun should have been invoked");

    cleanup_srun_env();

    assert!(
        args.contains("--time=1"),
        "Should clamp to --time=1 for near-expired end_time: {}",
        args
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
        true,  // limit_resources
        false, // use_srun = false
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
        true,  // limit_resources = true
        true,  // use_srun
        false, // enable_cpu_bind
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
        &temp_dir, &args_log, None,  // no resource requirements
        true,  // limit_resources
        true,  // use_srun
        false, // enable_cpu_bind
        None, None,
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
        true,            // limit_resources
        true,            // use_srun
        true,            // enable_cpu_bind
        Some(end_time),  // end_time
        Some("USR1@60"), // srun_termination_signal
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
        true,
        true,
        false,
        None,
        None,
        None, // target_node
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

#[test]
#[serial]
fn test_srun_no_slurm_job_id_falls_back_to_shell() {
    let temp_dir = TempDir::new().unwrap();
    let args_log = temp_dir.path().join("srun_args.log");

    // Set TORC_FAKE_SRUN but NOT SLURM_JOB_ID
    unsafe {
        env::remove_var("SLURM_JOB_ID");
        env::set_var(
            "TORC_FAKE_SRUN",
            fake_srun_path().to_string_lossy().to_string(),
        );
        env::set_var("TORC_SRUN_ARGS_LOG", args_log.to_string_lossy().to_string());
    }

    let rr = make_rr("compute", 4, "8g", 1);
    let args = run_and_capture_srun_args(
        &temp_dir,
        &args_log,
        Some(&rr),
        true,  // limit_resources
        true,  // use_srun
        false, // enable_cpu_bind
        None,
        None,
    );

    cleanup_srun_env();

    // Without SLURM_JOB_ID, even with use_srun=true, it should fall back to shell
    assert!(
        args.is_none(),
        "srun should not have been invoked without SLURM_JOB_ID, but got: {:?}",
        args
    );
}

#[test]
#[serial]
fn test_srun_limit_resources_false_with_cpu_bind_and_signal() {
    let temp_dir = TempDir::new().unwrap();
    let args_log = setup_srun_env(&temp_dir);
    let rr = make_rr("compute", 8, "16g", 2);

    let args = run_and_capture_srun_args(
        &temp_dir,
        &args_log,
        Some(&rr),
        false,            // limit_resources = false
        true,             // use_srun
        true,             // enable_cpu_bind = true
        None,             // end_time
        Some("TERM@300"), // srun_termination_signal
    )
    .expect("srun should have been invoked");

    cleanup_srun_env();

    // Resource limits should be absent (limit_resources=false)
    assert!(
        !args.contains("--cpus-per-task"),
        "Unexpected --cpus-per-task: {}",
        args
    );
    assert!(!args.contains("--mem="), "Unexpected --mem: {}", args);
    // CPU bind should be absent (enable_cpu_bind=true)
    assert!(
        !args.contains("--cpu-bind"),
        "Unexpected --cpu-bind: {}",
        args
    );
    // Signal should be present
    assert!(
        args.contains("--signal=TERM@300"),
        "Missing --signal=TERM@300: {}",
        args
    );
    // num_nodes=2 should still be present
    assert!(args.contains("--nodes=2"), "Missing --nodes=2: {}", args);
}

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
        true,  // limit_resources
        true,  // use_srun
        false, // enable_cpu_bind
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
        true,  // limit_resources
        true,  // use_srun
        false, // enable_cpu_bind
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
