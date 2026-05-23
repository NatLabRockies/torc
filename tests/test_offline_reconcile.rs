//! Integration test for the offline-drain → local journal → `torc workflows reconcile` flow.
//!
//! Simulates what happens when the server is unreachable: job runners drain (run
//! their jobs to completion) and journal the results to per-node SQLite files
//! instead of killing the jobs. After the server recovers, `torc reconcile`
//! discovers every journal for a `(workflow_id, run_id)` under a base directory
//! and replays the completions in bulk.
//!
//! Rather than killing a live server mid-run (flaky), this test writes journals
//! exactly as a drained runner would, then drives the real `reconcile` function
//! against a real server and asserts the server's state converges.

mod common;

use common::{
    create_test_compute_node, create_test_workflow, ensure_test_binaries_built, get_exe_path,
    get_server_url,
};
use serial_test::serial;
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use torc::client::apis;
use torc::client::apis::configuration::Configuration;
use torc::client::commands::reconcile::reconcile;
use torc::client::offline_journal::OfflineJournal;
use torc::client::workflow_manager::WorkflowManager;
use torc::config::TorcConfig;
use torc::models;
use torc::models::JobStatus;

struct TestServer {
    child: Child,
    config: Configuration,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn find_available_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind to random port")
        .local_addr()
        .expect("Failed to get local address")
        .port()
}

fn wait_for_ready(child: &mut Child, port: u16, timeout: Duration) -> Result<(), String> {
    let url = get_server_url(port);
    let client = reqwest::blocking::Client::new();
    let start = Instant::now();
    while start.elapsed() < timeout {
        if client.get(&url).send().is_ok() {
            return Ok(());
        }
        if let Some(status) = child.try_wait().map_err(|e| format!("poll failed: {e}"))? {
            return Err(format!("server exited before ready: {status}"));
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(format!("server not ready within {:?}", timeout))
}

fn start_server() -> TestServer {
    ensure_test_binaries_built();
    let port = find_available_port();
    let mut child = Command::new(get_exe_path("./target/debug/torc-server"))
        .arg("run")
        .arg("--port")
        .arg(port.to_string())
        .arg("--database")
        .arg(":memory:")
        .arg("--completion-check-interval-secs")
        .arg("0.1")
        .env("RUST_LOG", "warn")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("failed to spawn torc-server");

    if let Err(e) = wait_for_ready(&mut child, port, Duration::from_secs(15)) {
        let _ = child.kill();
        let _ = child.wait();
        panic!("Test server failed to start: {e}");
    }

    let mut config = Configuration::new();
    config.base_path = get_server_url(port);
    config.client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .expect("Failed to build blocking reqwest client");
    TestServer { child, config }
}

/// Build a successful-completion entry the way a drained runner would.
fn completion_entry(
    workflow_id: i64,
    run_id: i64,
    compute_node_id: i64,
    job_id: i64,
) -> models::JobCompletionEntry {
    let result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        0,   // return_code (success)
        0.1, // exec_time_minutes
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Completed,
    );
    models::JobCompletionEntry {
        job_id,
        status: JobStatus::Completed,
        run_id,
        result,
    }
}

fn is_complete(config: &Configuration, workflow_id: i64) -> bool {
    apis::workflows_api::is_workflow_complete(config, workflow_id)
        .expect("is_workflow_complete failed")
        .is_complete
}

/// Poll `is_workflow_complete` for up to `timeout`, since completions unblock
/// dependents via a background task that runs on an interval.
fn wait_until_complete(config: &Configuration, workflow_id: i64, timeout: Duration) -> bool {
    let start = Instant::now();
    loop {
        if is_complete(config, workflow_id) {
            return true;
        }
        if start.elapsed() >= timeout {
            return false;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

#[test]
#[serial(offline_reconcile)]
fn test_offline_drain_journal_reconcile_round_trip() {
    let server = start_server();
    let config = &server.config;

    // Create a workflow with four independent jobs and initialize it.
    let workflow = create_test_workflow(config, "offline_reconcile_round_trip");
    let workflow_id = workflow.id.unwrap();

    let mut job_ids = Vec::new();
    for i in 0..4 {
        let job = models::JobModel::new(workflow_id, format!("job{}", i), format!("echo job{}", i));
        let created = apis::jobs_api::create_job(config, job).expect("Failed to create job");
        job_ids.push(created.id.unwrap());
    }

    let torc_config = TorcConfig::load().unwrap_or_default();
    let manager = WorkflowManager::new(config.clone(), torc_config, workflow);
    manager.initialize(true).expect("Failed to initialize");
    let run_id = 1; // initialize() sets the first generation to run_id=1
    let compute_node_id = create_test_compute_node(config, workflow_id).id.unwrap();

    // Simulate two drained compute nodes writing journals into separate, nested
    // output directories under a shared base dir (the HPC shared-filesystem case).
    let base = tempfile::tempdir().expect("tempdir");
    let node_a_out = base.path().join("nodeA").join("torc_output");
    let node_b_out = base.path().join("nodeB").join("torc_output");

    let journal_a = OfflineJournal::open_or_create(&node_a_out, workflow_id, run_id, "nodeA")
        .expect("open journal A");
    let journal_b = OfflineJournal::open_or_create(&node_b_out, workflow_id, run_id, "nodeB")
        .expect("open journal B");

    // Split the four completions across the two node journals.
    for &job_id in &job_ids[..2] {
        journal_a
            .append(&completion_entry(
                workflow_id,
                run_id,
                compute_node_id,
                job_id,
            ))
            .expect("append A");
    }
    for &job_id in &job_ids[2..] {
        journal_b
            .append(&completion_entry(
                workflow_id,
                run_id,
                compute_node_id,
                job_id,
            ))
            .expect("append B");
    }

    // Before reconciling, the server has seen no completions.
    assert!(
        !is_complete(config, workflow_id),
        "workflow should not be complete before reconcile"
    );

    // Reconcile: discover both journals and replay them against the server.
    let summary = reconcile(config, workflow_id, run_id, base.path(), "table")
        .expect("reconcile should succeed");
    assert_eq!(summary.files, 2, "should discover both node journals");
    assert_eq!(summary.total_completions, 4);
    assert_eq!(
        summary.accepted, 4,
        "all four completions should be accepted"
    );
    assert_eq!(summary.rejected, 0);

    // The server now reflects every completion, and the workflow is complete.
    for &job_id in &job_ids {
        let job = apis::jobs_api::get_job(config, job_id).expect("get_job");
        assert_eq!(
            job.status.unwrap(),
            JobStatus::Completed,
            "job_id={} should be Completed after reconcile",
            job_id
        );
    }
    assert!(
        wait_until_complete(config, workflow_id, Duration::from_secs(5)),
        "workflow should be complete after all jobs reconciled"
    );

    // Re-running reconcile is safe: the jobs are already terminal, so the server
    // rejects the duplicates rather than erroring, and reconcile still succeeds.
    let summary_again = reconcile(config, workflow_id, run_id, base.path(), "table")
        .expect("re-running reconcile must not error");
    assert_eq!(summary_again.accepted, 0, "no new completions on re-run");
    for &job_id in &job_ids {
        let job = apis::jobs_api::get_job(config, job_id).expect("get_job");
        assert_eq!(job.status.unwrap(), JobStatus::Completed);
    }
}

#[test]
#[serial(offline_reconcile)]
fn test_reconcile_rejects_stale_run_id() {
    let server = start_server();
    let config = &server.config;

    let workflow = create_test_workflow(config, "offline_reconcile_stale_run");
    let workflow_id = workflow.id.unwrap();

    let job = models::JobModel::new(workflow_id, "only_job".to_string(), "echo hi".to_string());
    let job_id = apis::jobs_api::create_job(config, job)
        .expect("create_job")
        .id
        .unwrap();

    let torc_config = TorcConfig::load().unwrap_or_default();
    let manager = WorkflowManager::new(config.clone(), torc_config, workflow);
    manager.initialize(true).expect("initialize");
    let compute_node_id = create_test_compute_node(config, workflow_id).id.unwrap();

    // Journal a completion for a run that is NOT the workflow's current generation
    // (current run_id is 1). This mirrors a journal from a superseded run.
    let stale_run_id = 999;
    let base = tempfile::tempdir().expect("tempdir");
    let out = base.path().join("torc_output");
    let journal = OfflineJournal::open_or_create(&out, workflow_id, stale_run_id, "stale")
        .expect("open journal");
    journal
        .append(&completion_entry(
            workflow_id,
            stale_run_id,
            compute_node_id,
            job_id,
        ))
        .expect("append");

    let summary = reconcile(config, workflow_id, stale_run_id, base.path(), "table")
        .expect("reconcile should not error on stale completions");
    assert_eq!(summary.files, 1);
    assert_eq!(summary.accepted, 0, "stale completion must not be accepted");
    assert_eq!(summary.rejected, 1, "stale completion must be rejected");

    // The job is untouched by the rejected stale completion.
    let job = apis::jobs_api::get_job(config, job_id).expect("get_job");
    assert_ne!(
        job.status.unwrap(),
        JobStatus::Completed,
        "job must not be completed by a stale-run reconcile"
    );
}
