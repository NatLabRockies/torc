//! Integration tests for the `torc recover` recovery pipeline.
//!
//! These exercise the server-interacting recovery helpers directly against a live test
//! server: `reset_failed_jobs` (status/ownership guards, partial success) and
//! `apply_recovery_heuristics` (unknown-failure classification + retry-unknown inclusion,
//! the mechanism behind the `--recovery-hook` implies `--retry-unknown` fix).
//!
//! Pure decision logic (prompt parsing, violation categorization, `effective_retry_unknown`)
//! is unit-tested in `src/client/commands/recover.rs` where no server is required.

mod common;

use common::{
    ServerProcess, create_test_compute_node, create_test_job, create_test_workflow, start_server,
};
use rstest::rstest;
use torc::client::apis;
use torc::client::apis::configuration::Configuration;
use torc::client::commands::recover::{
    apply_recovery_heuristics, diagnose_failures, reset_failed_jobs,
};
use torc::client::workflow_manager::WorkflowManager;
use torc::config::TorcConfig;
use torc::models::{self, JobStatus};

/// Create and initialize an (empty) workflow, returning `(workflow_id, run_id)`.
fn init_workflow(config: &Configuration, name: &str) -> (i64, i64) {
    let workflow = create_test_workflow(config, name);
    let workflow_id = workflow.id.unwrap();
    let torc_config = TorcConfig::load().unwrap_or_default();
    let manager = WorkflowManager::new(config.clone(), torc_config, workflow);
    manager
        .initialize(false)
        .expect("Failed to initialize workflow");
    let run_id = manager.get_run_id().expect("Failed to get run_id");
    (workflow_id, run_id)
}

/// Add a job with its own resource requirement, returning the job id. The job is created in
/// the uninitialized state; call `initialize_jobs` afterward to make it claimable.
fn add_job_with_rr(
    config: &Configuration,
    workflow_id: i64,
    name: &str,
    memory: &str,
    runtime: &str,
    cpus: i64,
) -> i64 {
    let mut job = create_test_job(config, workflow_id, name);
    let mut rr = models::ResourceRequirementsModel::new(workflow_id, format!("{}_rr", name));
    rr.memory = memory.to_string();
    rr.runtime = runtime.to_string();
    rr.num_cpus = cpus;
    let created_rr = apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("Failed to create resource requirement");
    let job_id = job.id.unwrap();
    job.resource_requirements_id = Some(created_rr.id.unwrap());
    apis::jobs_api::update_job(config, job_id, job).expect("Failed to update job");
    job_id
}

/// Initialize jobs (ready them) and claim everything that is ready onto `compute_node_id`.
fn claim_all_ready(config: &Configuration, workflow_id: i64) {
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");
    let resources = models::ComputeNodesResources::new(36, 100.0, 0, 1);
    apis::workflows_api::claim_jobs_based_on_resources(config, workflow_id, 100, resources, None)
        .expect("Failed to claim jobs");
}

/// Drive an already-claimed job to a terminal state by recording a result.
#[allow(clippy::too_many_arguments)]
fn finish_job(
    config: &Configuration,
    workflow_id: i64,
    run_id: i64,
    compute_node_id: i64,
    job_id: i64,
    return_code: i64,
    status: JobStatus,
    exec_minutes: f64,
    peak_memory_bytes: Option<i64>,
) {
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id)
        .expect("Failed to set job running");
    let mut result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1,
        compute_node_id,
        return_code,
        exec_minutes,
        chrono::Utc::now().to_rfc3339(),
        status,
    );
    result.peak_memory_bytes = peak_memory_bytes;
    apis::jobs_api::complete_job(config, job_id, result.status, run_id, result)
        .expect("Failed to complete job");
}

fn job_status(config: &Configuration, job_id: i64) -> JobStatus {
    apis::jobs_api::get_job(config, job_id)
        .expect("Failed to fetch job")
        .status
        .expect("Job missing status")
}

fn output_dir_for(workflow_id: i64) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("torc_recover_test_{}", workflow_id))
}

// ---------------------------------------------------------------------------------------
// reset_failed_jobs
// ---------------------------------------------------------------------------------------

/// A failed job is reset to uninitialized; a completed job in the same call is left alone.
#[rstest]
fn reset_failed_jobs_resets_failed_skips_completed(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, run_id) = init_workflow(config, "reset_failed_skips_completed");
    let compute_node_id = create_test_compute_node(config, workflow_id).id.unwrap();

    let failed_id = add_job_with_rr(config, workflow_id, "will_fail", "2g", "PT1H", 1);
    let done_id = add_job_with_rr(config, workflow_id, "will_succeed", "2g", "PT1H", 1);
    claim_all_ready(config, workflow_id);

    finish_job(
        config,
        workflow_id,
        run_id,
        compute_node_id,
        failed_id,
        1,
        JobStatus::Failed,
        5.0,
        None,
    );
    finish_job(
        config,
        workflow_id,
        run_id,
        compute_node_id,
        done_id,
        0,
        JobStatus::Completed,
        5.0,
        None,
    );

    let reset = reset_failed_jobs(config, workflow_id, &[failed_id, done_id])
        .expect("reset should partially succeed");
    assert_eq!(reset, 1, "only the failed job is recoverable");
    assert_eq!(job_status(config, failed_id), JobStatus::Uninitialized);
    assert_eq!(
        job_status(config, done_id),
        JobStatus::Completed,
        "completed job must not be clobbered"
    );
}

/// All targets are non-recoverable (completed) -> hard error, nothing reset.
#[rstest]
fn reset_failed_jobs_errors_when_nothing_recoverable(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, run_id) = init_workflow(config, "reset_nothing_recoverable");
    let compute_node_id = create_test_compute_node(config, workflow_id).id.unwrap();

    let done_id = add_job_with_rr(config, workflow_id, "succeeds", "2g", "PT1H", 1);
    claim_all_ready(config, workflow_id);
    finish_job(
        config,
        workflow_id,
        run_id,
        compute_node_id,
        done_id,
        0,
        JobStatus::Completed,
        5.0,
        None,
    );

    let err =
        reset_failed_jobs(config, workflow_id, &[done_id]).expect_err("nothing recoverable -> Err");
    assert!(
        err.contains("0 of 1"),
        "message should report nothing reset: {err}"
    );
}

/// A job id from another workflow is refused (manage_status_change is not workflow-scoped,
/// so reset_failed_jobs must guard ownership itself).
#[rstest]
fn reset_failed_jobs_refuses_foreign_workflow(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (wf1, _run1) = init_workflow(config, "reset_owner_wf1");
    let (wf2, run2) = init_workflow(config, "reset_owner_wf2");
    let compute_node_id = create_test_compute_node(config, wf2).id.unwrap();

    let foreign_id = add_job_with_rr(config, wf2, "foreign", "2g", "PT1H", 1);
    claim_all_ready(config, wf2);
    finish_job(
        config,
        wf2,
        run2,
        compute_node_id,
        foreign_id,
        1,
        JobStatus::Failed,
        5.0,
        None,
    );

    // Ask wf1 to reset a job that actually belongs to wf2.
    let err =
        reset_failed_jobs(config, wf1, &[foreign_id]).expect_err("foreign job must be refused");
    assert!(
        err.contains("belongs to workflow"),
        "unexpected message: {err}"
    );
    // The foreign job is untouched.
    assert_eq!(job_status(config, foreign_id), JobStatus::Failed);
}

/// An empty id list is a no-op success.
#[rstest]
fn reset_failed_jobs_empty_list_is_ok_zero(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, _run_id) = init_workflow(config, "reset_empty_list");
    assert_eq!(reset_failed_jobs(config, workflow_id, &[]).unwrap(), 0);
}

/// Duplicate ids are deduplicated: the job is reset exactly once (count reflects unique ids).
#[rstest]
fn reset_failed_jobs_dedups_repeated_ids(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, run_id) = init_workflow(config, "reset_dedup");
    let compute_node_id = create_test_compute_node(config, workflow_id).id.unwrap();

    let failed_id = add_job_with_rr(config, workflow_id, "fails_once", "2g", "PT1H", 1);
    claim_all_ready(config, workflow_id);
    finish_job(
        config,
        workflow_id,
        run_id,
        compute_node_id,
        failed_id,
        1,
        JobStatus::Failed,
        5.0,
        None,
    );

    let reset = reset_failed_jobs(config, workflow_id, &[failed_id, failed_id, failed_id])
        .expect("dedup reset should succeed");
    assert_eq!(reset, 1, "duplicate ids must not double-count");
    assert_eq!(job_status(config, failed_id), JobStatus::Uninitialized);
}

// ---------------------------------------------------------------------------------------
// apply_recovery_heuristics: unknown-failure classification + retry-unknown
// ---------------------------------------------------------------------------------------

/// A job that failed with no resource violation (exit 1, well under memory/runtime/cpu) is an
/// "unknown" failure: it is counted in other_failures but only joins jobs_to_retry when
/// retry-unknown is in effect. This is the mechanism the `--recovery-hook` => retry-unknown
/// fix relies on.
#[rstest]
fn apply_heuristics_unknown_failure_respects_retry_unknown(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, run_id) = init_workflow(config, "heuristics_unknown_failure");
    let compute_node_id = create_test_compute_node(config, workflow_id).id.unwrap();

    // 2g / PT1H / 1 cpu, failed with exit 1, 5 min runtime, 1GB peak: no violation flags set.
    let job_id = add_job_with_rr(config, workflow_id, "mystery_fail", "2g", "PT1H", 1);
    claim_all_ready(config, workflow_id);
    finish_job(
        config,
        workflow_id,
        run_id,
        compute_node_id,
        job_id,
        1,
        JobStatus::Failed,
        5.0,
        Some(1_000_000_000),
    );

    let diagnosis = diagnose_failures(config, workflow_id).expect("diagnose");
    let output_dir = output_dir_for(workflow_id);

    // retry_unknown = false: counted as an other failure but NOT scheduled for retry.
    let without = apply_recovery_heuristics(
        config,
        workflow_id,
        &diagnosis,
        1.5,
        1.4,
        false,
        &output_dir,
        true,
    )
    .expect("heuristics (no retry-unknown)");
    assert_eq!(without.other_failures, 1, "unknown failure counted");
    assert!(
        without.jobs_to_retry.is_empty(),
        "unknown failure must not be retried without retry-unknown"
    );
    assert_eq!(without.oom_fixed, 0);
    assert_eq!(without.timeout_fixed, 0);

    // retry_unknown = true (what --recovery-hook implies): the unknown job IS scheduled.
    let with = apply_recovery_heuristics(
        config,
        workflow_id,
        &diagnosis,
        1.5,
        1.4,
        true,
        &output_dir,
        true,
    )
    .expect("heuristics (retry-unknown)");
    assert_eq!(with.other_failures, 1);
    assert_eq!(with.unknown_retried, 1);
    assert_eq!(with.jobs_to_retry, vec![job_id]);
}

/// With no failed jobs at all, heuristics find nothing to do.
#[rstest]
fn apply_heuristics_no_failures_is_empty(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, _run_id) = init_workflow(config, "heuristics_no_failures");
    let diagnosis = diagnose_failures(config, workflow_id).expect("diagnose");
    let result = apply_recovery_heuristics(
        config,
        workflow_id,
        &diagnosis,
        1.5,
        1.4,
        true,
        &output_dir_for(workflow_id),
        true,
    )
    .expect("heuristics");
    assert_eq!(result.other_failures, 0);
    assert!(result.jobs_to_retry.is_empty());
}
