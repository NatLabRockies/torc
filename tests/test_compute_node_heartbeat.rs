//! Compute node heartbeats and server-side expiry of nodes whose runner went silent.

mod common;

use common::{
    ServerProcess, create_test_workflow, ensure_test_binaries_built, start_server,
    start_server_with_env, torc_binary_path,
};
use rstest::rstest;
use std::time::{Duration, Instant};
use torc::client::Configuration;
use torc::client::apis;
use torc::models;

const TIMEOUT_ENV: &str = "TORC_COMPUTE_NODE_HEARTBEAT_TIMEOUT_SECS";
const SWEEP_ENV: &str = "TORC_COMPUTE_NODE_SWEEP_INTERVAL_SECS";

/// A server that expires nodes after `timeout_secs` without a heartbeat, sweeping every second.
fn start_expiring_server(timeout_secs: &str) -> ServerProcess {
    start_server_with_env(&[(TIMEOUT_ENV, timeout_secs), (SWEEP_ENV, "1")])
}

fn heartbeat_age(seconds: i64) -> String {
    (chrono::Utc::now() - chrono::Duration::seconds(seconds)).to_rfc3339()
}

fn create_node(
    config: &Configuration,
    workflow_id: i64,
    host: &str,
    last_heartbeat: Option<String>,
) -> i64 {
    let mut node = models::ComputeNodeModel::new(
        workflow_id,
        host.to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        8,
        16.0,
        0,
        1,
        "local".to_string(),
        None,
    );
    node.is_active = Some(true);
    node.last_heartbeat = last_heartbeat;
    apis::compute_nodes_api::create_compute_node(config, node)
        .expect("Failed to create compute node")
        .id
        .unwrap()
}

/// Create a job and run it on `compute_node_id`. Returns `(job_id, run_id)`.
fn start_running_job(
    config: &Configuration,
    workflow_id: i64,
    name: &str,
    compute_node_id: i64,
) -> (i64, i64) {
    let job = models::JobModel::new(workflow_id, name.to_string(), "echo test".to_string());
    let job_id = apis::jobs_api::create_job(config, job)
        .expect("Failed to create job")
        .id
        .unwrap();
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");
    let run_id = apis::workflows_api::get_workflow(config, workflow_id)
        .expect("Failed to get workflow")
        .run_id
        .unwrap_or(0);
    apis::workflows_api::claim_next_jobs(config, workflow_id, Some(1), None, None, None)
        .expect("Failed to claim job");
    apis::jobs_api::start_job(config, job_id, run_id, compute_node_id)
        .expect("Failed to start job");
    (job_id, run_id)
}

/// Keeps a node alive from a thread while a test sets up, so a loaded machine cannot expire
/// the node before the scenario starts. Dropping it stops the heartbeats.
struct Heartbeater {
    stop: Option<std::sync::mpsc::Sender<()>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Heartbeater {
    fn start(config: &Configuration, node_id: i64) -> Self {
        let config = config.clone();
        let (stop, stopped) = std::sync::mpsc::channel::<()>();
        let handle = std::thread::spawn(move || {
            loop {
                let _ = apis::compute_nodes_api::heartbeat_compute_node(&config, node_id);
                if !matches!(
                    stopped.recv_timeout(Duration::from_millis(250)),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                ) {
                    break;
                }
            }
        });
        Self {
            stop: Some(stop),
            handle: Some(handle),
        }
    }
}

impl Drop for Heartbeater {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Kills and reaps the spawned runner, and the job it started, on success and on panic.
struct RunnerGuard {
    runner: std::process::Child,
    job_pid_file: std::path::PathBuf,
}

impl Drop for RunnerGuard {
    fn drop(&mut self) {
        let _ = self.runner.kill();
        let _ = self.runner.wait();
        if let Ok(pid) = std::fs::read_to_string(&self.job_pid_file) {
            let _ = std::process::Command::new("kill").arg(pid.trim()).status();
        }
    }
}

fn is_node_active(config: &Configuration, compute_node_id: i64) -> bool {
    apis::compute_nodes_api::get_compute_node(config, compute_node_id)
        .expect("Failed to get compute node")
        .is_active
        .unwrap()
}

fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    condition()
}

fn job_status(config: &Configuration, job_id: i64) -> models::JobStatus {
    apis::jobs_api::get_job(config, job_id)
        .expect("Failed to get job")
        .status
        .unwrap()
}

fn result_return_codes(config: &Configuration, workflow_id: i64, job_id: i64) -> Vec<i64> {
    apis::results_api::list_results(
        config,
        workflow_id,
        Some(job_id),
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
    .expect("Failed to list results")
    .items
    .iter()
    .map(|r| r.return_code)
    .collect()
}

#[test]
fn test_stale_node_expires_and_its_running_job_fails() {
    let server = start_expiring_server("2");
    let config = &server.config;
    let workflow_id = create_test_workflow(config, "test_stale_node_expires")
        .id
        .unwrap();

    let stale_node = create_node(config, workflow_id, "stale-host", Some(heartbeat_age(0)));
    let heartbeater = Heartbeater::start(config, stale_node);
    let (job_id, run_id) = start_running_job(config, workflow_id, "stale_job", stale_node);
    assert_eq!(job_status(config, job_id), models::JobStatus::Running);
    drop(heartbeater);

    assert!(
        wait_until(Duration::from_secs(20), || !is_node_active(
            config, stale_node
        )),
        "stale node was not expired"
    );
    assert!(
        wait_until(Duration::from_secs(10), || job_status(config, job_id)
            == models::JobStatus::Failed),
        "running job of the expired node did not fail"
    );
    assert_eq!(
        result_return_codes(config, workflow_id, job_id),
        vec![models::ORPHANED_JOB_RETURN_CODE]
    );

    // A late completion from the silent runner is rejected: the job is already complete.
    let late = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1,
        stale_node,
        0,
        0.1,
        chrono::Utc::now().to_rfc3339(),
        models::JobStatus::Completed,
    );
    assert!(
        apis::jobs_api::complete_job(config, job_id, models::JobStatus::Completed, run_id, late)
            .is_err(),
        "a late completion must not overwrite the failure"
    );
    assert_eq!(job_status(config, job_id), models::JobStatus::Failed);
}

/// Without expiry a dead runner's running job blocks `reset-status`; with it, the reset
/// succeeds without `--force`.
#[test]
fn test_expired_node_lets_reset_status_run_without_force() {
    let server = start_expiring_server("2");
    let config = &server.config;
    let workflow_id = create_test_workflow(config, "test_expiry_unblocks_reset")
        .id
        .unwrap();
    let stale_node = create_node(config, workflow_id, "stale-host", Some(heartbeat_age(0)));
    let heartbeater = Heartbeater::start(config, stale_node);
    let (job_id, _) = start_running_job(config, workflow_id, "stale_job", stale_node);
    drop(heartbeater);

    assert!(
        wait_until(Duration::from_secs(20), || job_status(config, job_id)
            == models::JobStatus::Failed),
        "running job of the expired node did not fail"
    );
    apis::workflows_api::reset_workflow_status(config, workflow_id, None)
        .expect("reset-status should not need --force after expiry");
}

#[test]
fn test_node_with_fresh_heartbeat_is_not_expired_while_heartbeats_continue() {
    let server = start_expiring_server("3");
    let config = &server.config;
    let workflow_id = create_test_workflow(config, "test_fresh_heartbeat_kept")
        .id
        .unwrap();
    let node = create_node(config, workflow_id, "live-host", Some(heartbeat_age(0)));
    let (job_id, _) = start_running_job(config, workflow_id, "live_job", node);

    // Run for twice the timeout, heartbeating every half second.
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(6) {
        let model = apis::compute_nodes_api::heartbeat_compute_node(config, node)
            .expect("heartbeat failed");
        assert_eq!(model.is_active, Some(true));
        std::thread::sleep(Duration::from_millis(500));
    }
    assert!(is_node_active(config, node));
    assert_eq!(job_status(config, job_id), models::JobStatus::Running);

    // Once heartbeats stop, the node expires.
    assert!(
        wait_until(Duration::from_secs(20), || !is_node_active(config, node)),
        "node was not expired after heartbeats stopped"
    );
}

#[test]
fn test_node_without_heartbeat_is_never_expired() {
    let server = start_expiring_server("2");
    let config = &server.config;
    let workflow_id = create_test_workflow(config, "test_null_heartbeat_kept")
        .id
        .unwrap();
    let legacy_node = create_node(config, workflow_id, "legacy-host", None);
    let (job_id, _) = start_running_job(config, workflow_id, "legacy_job", legacy_node);
    // A stale node on the same server proves the sweep ran during the wait.
    let stale_node = create_node(config, workflow_id, "stale-host", Some(heartbeat_age(3600)));

    assert!(
        wait_until(Duration::from_secs(20), || !is_node_active(
            config, stale_node
        )),
        "sweep did not run"
    );
    assert!(is_node_active(config, legacy_node));
    assert_eq!(job_status(config, job_id), models::JobStatus::Running);
    let node = apis::compute_nodes_api::get_compute_node(config, legacy_node).unwrap();
    assert_eq!(node.last_heartbeat, None);
}

#[test]
fn test_timeout_zero_disables_expiry() {
    let server = start_expiring_server("0");
    let config = &server.config;
    let workflow_id = create_test_workflow(config, "test_expiry_disabled")
        .id
        .unwrap();
    let stale_node = create_node(config, workflow_id, "stale-host", Some(heartbeat_age(3600)));
    let (job_id, _) = start_running_job(config, workflow_id, "stale_job", stale_node);

    std::thread::sleep(Duration::from_secs(5));

    assert!(is_node_active(config, stale_node));
    assert_eq!(job_status(config, job_id), models::JobStatus::Running);
}

#[rstest]
fn test_heartbeat_endpoint_updates_only_active_nodes(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "test_heartbeat_endpoint")
        .id
        .unwrap();

    let old = heartbeat_age(3600);
    let active = create_node(config, workflow_id, "active-host", Some(old.clone()));
    let model = apis::compute_nodes_api::heartbeat_compute_node(config, active).unwrap();
    let refreshed = model.last_heartbeat.expect("heartbeat not recorded");
    assert!(refreshed > old, "{refreshed} should be newer than {old}");
    assert_eq!(model.is_active, Some(true));

    // An inactive node keeps its heartbeat and stays inactive.
    let inactive = create_node(config, workflow_id, "inactive-host", Some(old.clone()));
    let mut update = models::ComputeNodeModel::new(
        workflow_id,
        "inactive-host".to_string(),
        1,
        chrono::Utc::now().to_rfc3339(),
        8,
        16.0,
        0,
        1,
        "local".to_string(),
        None,
    );
    update.is_active = Some(false);
    apis::compute_nodes_api::update_compute_node(config, inactive, update).unwrap();
    let model = apis::compute_nodes_api::heartbeat_compute_node(config, inactive).unwrap();
    assert_eq!(model.is_active, Some(false));
    assert_eq!(model.last_heartbeat.as_deref(), Some(old.as_str()));

    assert!(apis::compute_nodes_api::heartbeat_compute_node(config, i64::MAX - 1).is_err());
}

/// A runner killed with SIGKILL cannot deactivate its node. With heartbeats on, the server
/// expires the node and fails the job that was running on it.
#[cfg(unix)]
#[test]
fn test_killed_runner_node_expires() {
    ensure_test_binaries_built();
    let server = start_expiring_server("4");
    let config = &server.config;
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("job.pid");
    let spec_path = dir.path().join("spec.yaml");
    std::fs::write(
        &spec_path,
        format!(
            "name: killed_runner\njobs:\n  - name: long_job\n    command: \"sh -c 'echo $$ > {} && exec sleep 120'\"\n",
            pid_file.display()
        ),
    )
    .unwrap();

    let runner = std::process::Command::new(torc_binary_path())
        .args(["--url", &config.base_path, "run"])
        .arg(&spec_path)
        .current_dir(dir.path())
        .env("TORC_CLIENT__RUN__HEARTBEAT_INTERVAL_SECS", "1")
        .env("RUST_LOG", "warn")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to spawn torc run");
    let mut guard = RunnerGuard {
        runner,
        job_pid_file: pid_file.clone(),
    };

    let find_workflow = || {
        apis::workflows_api::list_workflows(
            config,
            None,
            None,
            None,
            None,
            Some("killed_runner"),
            None,
            None,
            None,
            None,
        )
        .ok()
        .and_then(|r| r.items.first().and_then(|w| w.id))
    };
    assert!(
        wait_until(Duration::from_secs(30), || find_workflow().is_some()
            && pid_file.exists()),
        "job did not start"
    );
    let workflow_id = find_workflow().unwrap();
    let active_nodes = || {
        apis::compute_nodes_api::list_compute_nodes(
            config,
            workflow_id,
            None,
            None,
            None,
            None,
            None,
            Some(true),
            None,
        )
        .unwrap()
        .items
    };
    let node = active_nodes().pop().expect("no active compute node");
    let node_id = node.id.unwrap();
    assert!(node.last_heartbeat.is_some(), "runner set no heartbeat");

    guard.runner.kill().expect("failed to SIGKILL the runner");
    let _ = guard.runner.wait();

    assert!(
        wait_until(Duration::from_secs(30), || !is_node_active(config, node_id)),
        "node of the killed runner was not expired"
    );
    let jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        Some(100),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap()
    .items;
    let job_id = jobs[0].id.unwrap();
    assert!(
        wait_until(Duration::from_secs(10), || job_status(config, job_id)
            == models::JobStatus::Failed),
        "job of the killed runner did not fail"
    );
    assert_eq!(
        result_return_codes(config, workflow_id, job_id),
        vec![models::ORPHANED_JOB_RETURN_CODE]
    );
}

/// A node with fresh heartbeats keeps ownership. A completion that names another node, or a
/// stale attempt, is rejected. The owner's completion is accepted.
#[rstest]
fn test_completion_requires_the_owning_node_and_attempt(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "test_completion_ownership")
        .id
        .unwrap();
    let owner = create_node(config, workflow_id, "owner-host", None);
    let other = create_node(config, workflow_id, "other-host", None);
    let (job_id, run_id) = start_running_job(config, workflow_id, "owned_job", owner);

    let complete = |node: i64, attempt: i64| -> bool {
        let result = models::ResultModel::new(
            job_id,
            workflow_id,
            run_id,
            attempt,
            node,
            0,
            0.1,
            chrono::Utc::now().to_rfc3339(),
            models::JobStatus::Completed,
        );
        apis::jobs_api::complete_job(config, job_id, models::JobStatus::Completed, run_id, result)
            .is_ok()
    };

    assert!(
        !complete(other, 1),
        "another node must not complete the job"
    );
    assert!(
        !complete(owner, 2),
        "a stale attempt must not complete the job"
    );
    assert_eq!(job_status(config, job_id), models::JobStatus::Running);
    assert!(complete(owner, 1), "the owner completes its job");
    assert_eq!(job_status(config, job_id), models::JobStatus::Completed);
}

/// After expiry and a retry, the old runner's late completion must not complete the new
/// attempt.
#[test]
fn test_late_completion_after_retry_is_rejected() {
    let server = start_expiring_server("2");
    let config = &server.config;
    let workflow_id = create_test_workflow(config, "test_late_completion_after_retry")
        .id
        .unwrap();
    let old_node = create_node(config, workflow_id, "old-host", Some(heartbeat_age(0)));
    let heartbeater = Heartbeater::start(config, old_node);
    let (job_id, run_id) = start_running_job(config, workflow_id, "retried_job", old_node);
    drop(heartbeater);
    assert!(
        wait_until(Duration::from_secs(20), || job_status(config, job_id)
            == models::JobStatus::Failed),
        "job of the expired node did not fail"
    );

    apis::jobs_api::retry_job(config, job_id, run_id, 3).expect("retry_job failed");
    let new_node = create_node(config, workflow_id, "new-host", None);
    apis::workflows_api::claim_next_jobs(config, workflow_id, Some(1), None, None, None)
        .expect("claim failed");
    apis::jobs_api::start_job(config, job_id, run_id, new_node).expect("start failed");

    let late = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1,
        old_node,
        0,
        0.1,
        chrono::Utc::now().to_rfc3339(),
        models::JobStatus::Completed,
    );
    assert!(
        apis::jobs_api::complete_job(config, job_id, models::JobStatus::Completed, run_id, late)
            .is_err(),
        "a late completion must not complete the retried attempt"
    );
    assert_eq!(job_status(config, job_id), models::JobStatus::Running);
}

/// After expiry and a same-run reset, the old runner's late completion must not complete the
/// job that is ready to run again.
#[test]
fn test_late_completion_after_reset_is_rejected() {
    let server = start_expiring_server("2");
    let config = &server.config;
    let workflow_id = create_test_workflow(config, "test_late_completion_after_reset")
        .id
        .unwrap();
    let old_node = create_node(config, workflow_id, "old-host", Some(heartbeat_age(0)));
    let heartbeater = Heartbeater::start(config, old_node);
    let (job_id, run_id) = start_running_job(config, workflow_id, "reset_job", old_node);
    drop(heartbeater);
    assert!(
        wait_until(Duration::from_secs(20), || job_status(config, job_id)
            == models::JobStatus::Failed),
        "job of the expired node did not fail"
    );

    apis::workflows_api::reset_job_status(config, workflow_id, Some(true)).expect("reset failed");
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("initialize failed");
    assert_eq!(job_status(config, job_id), models::JobStatus::Ready);

    let late = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1,
        old_node,
        0,
        0.1,
        chrono::Utc::now().to_rfc3339(),
        models::JobStatus::Completed,
    );
    assert!(
        apis::jobs_api::complete_job(config, job_id, models::JobStatus::Completed, run_id, late)
            .is_err(),
        "a late completion from an expired node must be rejected"
    );
    assert_eq!(job_status(config, job_id), models::JobStatus::Ready);
}

/// A job cannot start on a node the server has deactivated.
#[test]
fn test_start_on_expired_node_is_rejected() {
    let server = start_expiring_server("2");
    let config = &server.config;
    let workflow_id = create_test_workflow(config, "test_start_on_expired_node")
        .id
        .unwrap();
    let expired = create_node(
        config,
        workflow_id,
        "expired-host",
        Some(heartbeat_age(3600)),
    );
    assert!(
        wait_until(Duration::from_secs(20), || !is_node_active(config, expired)),
        "node was not expired"
    );

    let job = models::JobModel::new(workflow_id, "late_start".to_string(), "echo".to_string());
    let job_id = apis::jobs_api::create_job(config, job).unwrap().id.unwrap();
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None).unwrap();
    let run_id = apis::workflows_api::get_workflow(config, workflow_id)
        .unwrap()
        .run_id
        .unwrap_or(0);
    assert!(
        apis::workflows_api::claim_next_jobs(
            config,
            workflow_id,
            Some(1),
            None,
            Some(run_id),
            Some(expired),
        )
        .is_err(),
        "a claim for an expired node must be rejected"
    );
    apis::workflows_api::claim_next_jobs(config, workflow_id, Some(1), None, None, None)
        .expect("claim failed");
    assert!(
        apis::jobs_api::start_job(config, job_id, run_id, expired).is_err(),
        "start on an expired node must be rejected"
    );
    assert_eq!(job_status(config, job_id), models::JobStatus::Pending);
}

/// Failing the jobs of an expired node can be retried: a running job on an inactive node with a
/// heartbeat is failed by a later sweep, whatever deactivated the node.
#[test]
fn test_running_job_on_inactive_node_is_failed_by_a_later_sweep() {
    let server = start_expiring_server("2");
    let config = &server.config;
    let workflow_id = create_test_workflow(config, "test_sweep_retries_jobs")
        .id
        .unwrap();
    let node = create_node(
        config,
        workflow_id,
        "half-expired-host",
        Some(heartbeat_age(0)),
    );
    let heartbeater = Heartbeater::start(config, node);
    let (job_id, _) = start_running_job(config, workflow_id, "stuck_job", node);
    drop(heartbeater);

    let mut update = models::ComputeNodeModel::new(
        workflow_id,
        "half-expired-host".to_string(),
        1,
        chrono::Utc::now().to_rfc3339(),
        8,
        16.0,
        0,
        1,
        "local".to_string(),
        None,
    );
    update.is_active = Some(false);
    apis::compute_nodes_api::update_compute_node(config, node, update).unwrap();

    assert!(
        wait_until(Duration::from_secs(20), || job_status(config, job_id)
            == models::JobStatus::Failed),
        "job on an inactive node was not failed"
    );
}

/// A client cannot roll the stored heartbeat back through a general update.
#[rstest]
fn test_update_ignores_last_heartbeat(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "test_update_ignores_heartbeat")
        .id
        .unwrap();
    let node = create_node(config, workflow_id, "host", Some(heartbeat_age(0)));
    let stored = apis::compute_nodes_api::get_compute_node(config, node)
        .unwrap()
        .last_heartbeat
        .unwrap();

    let mut update = apis::compute_nodes_api::get_compute_node(config, node).unwrap();
    update.last_heartbeat = Some(heartbeat_age(3600));
    let updated = apis::compute_nodes_api::update_compute_node(config, node, update).unwrap();
    assert_eq!(updated.last_heartbeat.as_deref(), Some(stored.as_str()));
}
