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
use torc::client::workflow_manager::WorkflowManager;
use torc::config::TorcConfig;
use torc::models;
use torc::models::JobStatus;

/// In-memory torc-server with `--threads 1` — matches the production conditions where the
/// `apply_job_completion_state_tx` deadlock manifests. Shared-cache in-memory SQLite uses
/// table-level locks (unlike WAL on disk), so once an iteration's UPDATE on `tx` takes a
/// write lock, any cross-connection SELECT against the same table blocks until commit.
struct InMemoryServer {
    child: Child,
    config: Configuration,
}

impl Drop for InMemoryServer {
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

fn start_in_memory_server() -> InMemoryServer {
    ensure_test_binaries_built();
    let port = find_available_port();
    let mut child = Command::new(get_exe_path("./target/debug/torc-server"))
        .arg("run")
        .arg("--port")
        .arg(port.to_string())
        .arg("--threads")
        .arg("1")
        .arg("--database")
        .arg(":memory:")
        .arg("--completion-check-interval-secs")
        .arg("0.1")
        .env("RUST_LOG", "info")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("failed to spawn torc-server");

    if let Err(e) = wait_for_ready(&mut child, port, Duration::from_secs(15)) {
        let _ = child.kill();
        let _ = child.wait();
        panic!("In-memory server failed to start: {e}");
    }

    let mut config = Configuration::new();
    config.base_path = get_server_url(port);
    // The default `reqwest::blocking::Client` has no request timeout. If the deadlock
    // regresses, an unbounded HTTP wait would let the assertion below never fire and
    // the test could hang for the full CI timeout. Cap each request at 15s so a
    // regression surfaces as a fast failure rather than a stalled run.
    config.client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .expect("Failed to build blocking reqwest client");
    InMemoryServer { child, config }
}

/// Regression test for the deadlock in `apply_job_completion_state_tx`.
///
/// The bug: the batch handler opened a transaction on connection A from the pool, then for
/// each completion called `jobs_api.get_job(...)` and `validate_run_id(...)` which grabbed
/// fresh pool connections. After the first iteration's UPDATE took a write lock on `tx`,
/// every subsequent iteration's read on a separate connection blocked on that lock. With
/// `--threads 1`, the single tokio worker awaited the blocked read while the transaction
/// holding the lock could only release once the worker continued — self-deadlock. In
/// production the runner's 30s HTTP timeout fired, retries failed for 20 minutes, and
/// in-flight jobs were killed.
///
/// The deadlock only manifests under shared-cache in-memory SQLite (table-level locking);
/// WAL mode on disk allows concurrent reads alongside open writers, so a file-based test
/// server does not reproduce the bug. This test therefore spawns its own in-memory server
/// with `--threads 1` to match the production conditions.
#[test]
#[serial(batch_complete_inmem)]
fn test_batch_complete_jobs_does_not_deadlock_in_memory() {
    let server = start_in_memory_server();
    let config = &server.config;

    let workflow = create_test_workflow(config, "test_batch_complete_jobs_does_not_deadlock");
    let workflow_id = workflow.id.unwrap();

    let mut job_ids = Vec::new();
    for i in 0..4 {
        let job = models::JobModel::new(workflow_id, format!("job{}", i), format!("echo job{}", i));
        let created = apis::jobs_api::create_job(config, job).expect("Failed to create job");
        job_ids.push(created.id.unwrap());
    }

    let torc_config = TorcConfig::load().unwrap_or_default();
    let manager = WorkflowManager::new(config.clone(), torc_config, workflow);
    manager
        .initialize(true)
        .expect("Failed to initialize workflow");

    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    let run_id = 1;
    let completions: Vec<models::JobCompletionEntry> = job_ids
        .iter()
        .map(|&job_id| {
            let result = models::ResultModel::new(
                job_id,
                workflow_id,
                run_id,
                1, // attempt_id
                compute_node_id,
                0,   // return_code
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
        })
        .collect();

    let request = models::BatchCompleteJobsRequest { completions };

    // The deadlock manifests as the request hanging until the client times out (~30s in
    // production). 10s is generous on CI but well below the timeout window — if this test
    // takes longer than that, the deadlock has regressed.
    let start = Instant::now();
    let response = apis::workflows_api::batch_complete_jobs(config, workflow_id, request)
        .expect("batch_complete_jobs failed");
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(10),
        "batch_complete_jobs took {:?} — expected <10s. Likely deadlock regression.",
        elapsed
    );

    assert!(
        response.errors.is_empty(),
        "Expected no errors, got: {:?}",
        response.errors
    );
    let mut completed_sorted = response.completed.clone();
    completed_sorted.sort();
    let mut expected_sorted = job_ids.clone();
    expected_sorted.sort();
    assert_eq!(
        completed_sorted, expected_sorted,
        "All submitted jobs should be reported completed"
    );

    for &job_id in &job_ids {
        let job = apis::jobs_api::get_job(config, job_id).expect("Failed to get job");
        assert_eq!(
            job.status.unwrap(),
            JobStatus::Completed,
            "job_id={} should be Completed",
            job_id
        );
    }
}
