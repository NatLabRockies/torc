//! Compute node heartbeats, stale marking, and the fail call.

mod common;

use common::{
    ServerProcess, create_test_workflow, ensure_test_binaries_built, restart_server_with_env,
    start_server, start_server_with_env, torc_binary_path,
};
use rstest::rstest;
use std::time::{Duration, Instant};
use torc::client::Configuration;
use torc::client::apis;
use torc::models;

const STALE_ENV: &str = "TORC_COMPUTE_NODE_STALE_AFTER_SECS";

/// A server that marks nodes stale after `secs` without a heartbeat.
fn start_stale_server(secs: &str) -> ServerProcess {
    start_server_with_env(&[(STALE_ENV, secs)])
}

fn heartbeat_age(seconds: i64) -> String {
    (chrono::Utc::now() - chrono::Duration::seconds(seconds)).to_rfc3339()
}

fn new_node(workflow_id: i64, host: &str) -> models::ComputeNodeModel {
    models::ComputeNodeModel::new(
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
    )
}

fn create_node(
    config: &Configuration,
    workflow_id: i64,
    host: &str,
    last_heartbeat: Option<String>,
) -> i64 {
    let mut node = new_node(workflow_id, host);
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

fn fail_node(config: &Configuration, id: i64) -> models::ComputeNodeModel {
    apis::compute_nodes_api::fail_compute_node(config, id).expect("fail_compute_node")
}

fn is_stale(config: &Configuration, id: i64) -> bool {
    apis::compute_nodes_api::get_compute_node(config, id)
        .expect("Failed to get compute node")
        .is_stale
        == Some(true)
}

fn list_stale(
    config: &Configuration,
    workflow_id: i64,
    stale: bool,
) -> models::ListComputeNodesResponse {
    apis::compute_nodes_api::list_compute_nodes(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(stale),
    )
    .expect("Failed to list compute nodes")
}

fn ids(nodes: Vec<models::ComputeNodeModel>) -> Vec<i64> {
    nodes.iter().map(|n| n.id.unwrap()).collect()
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

/// A stale node stays active, its job keeps running, and its completion is accepted.
#[test]
fn test_stale_node_stays_active_and_its_job_keeps_running() {
    let server = start_stale_server("2");
    let config = &server.config;
    let workflow_id = create_test_workflow(config, "test_stale_node_stays_active")
        .id
        .unwrap();

    let node = create_node(config, workflow_id, "stale-host", Some(heartbeat_age(0)));
    let heartbeater = Heartbeater::start(config, node);
    let (job_id, run_id) = start_running_job(config, workflow_id, "stale_job", node);
    drop(heartbeater);

    assert!(
        wait_until(Duration::from_secs(20), || is_stale(config, node)),
        "node did not become stale"
    );
    assert!(is_node_active(config, node));
    assert_eq!(job_status(config, job_id), models::JobStatus::Running);

    let done = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1,
        node,
        0,
        0.1,
        chrono::Utc::now().to_rfc3339(),
        models::JobStatus::Completed,
    );
    apis::jobs_api::complete_job(config, job_id, models::JobStatus::Completed, run_id, done)
        .expect("a stale node must still complete its job");
    assert_eq!(job_status(config, job_id), models::JobStatus::Completed);
}

#[test]
fn test_restart_guard_hides_stale_until_uptime_reaches_threshold() {
    let server = start_stale_server("5");
    let config = &server.config;
    let workflow_id = create_test_workflow(config, "test_restart_guard")
        .id
        .unwrap();
    let node = create_node(config, workflow_id, "old-host", Some(heartbeat_age(1000)));
    assert!(!is_stale(config, node));
    assert!(list_stale(config, workflow_id, true).items.is_empty());
    assert!(wait_until(Duration::from_secs(20), || is_stale(
        config, node
    )));
}

#[test]
fn test_list_filters_stale_nodes() {
    let server = start_stale_server("2");
    let config = &server.config;
    let workflow_id = create_test_workflow(config, "test_list_filters_stale")
        .id
        .unwrap();
    let old = create_node(config, workflow_id, "old-host", Some(heartbeat_age(1000)));
    assert!(
        wait_until(Duration::from_secs(20), || is_stale(config, old)),
        "node did not become stale"
    );
    let fresh = create_node(config, workflow_id, "fresh-host", Some(heartbeat_age(0)));
    let _heartbeater = Heartbeater::start(config, fresh);

    let stale = list_stale(config, workflow_id, true);
    assert_eq!(stale.total_count, 1);
    assert_eq!(ids(stale.items), vec![old]);
    // The workflow helper also registers a node without a heartbeat, which is never stale.
    let not_stale = ids(list_stale(config, workflow_id, false).items);
    assert!(not_stale.contains(&fresh));
    assert!(!not_stale.contains(&old));
}

/// The `last_heartbeat` column as stored in the database, read past the server.
fn stored_heartbeat(server: &ServerProcess, id: i64) -> Option<String> {
    let url = format!("sqlite:{}", server.db_file.path().display());
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let pool = sqlx::SqlitePool::connect(&url).await.unwrap();
        let stored = sqlx::query_scalar("SELECT last_heartbeat FROM compute_node WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        pool.close().await;
        stored
    })
}

/// Only the first heartbeat is stored. The stale filter uses the in-memory heartbeat, and the
/// server stores the last heartbeat when a read finds the node stale.
#[test]
fn test_heartbeats_stay_in_memory_until_the_node_is_stale() {
    let server = start_stale_server("2");
    let config = &server.config;
    let workflow_id = create_test_workflow(config, "test_heartbeats_in_memory")
        .id
        .unwrap();
    let node = create_node(config, workflow_id, "live-host", None);
    let first = apis::compute_nodes_api::heartbeat_compute_node(config, node)
        .unwrap()
        .last_heartbeat;
    assert!(first.is_some());
    assert_eq!(stored_heartbeat(&server, node), first);

    // Beat for longer than the threshold. The stored time becomes older than the cutoff.
    let heartbeater = Heartbeater::start(config, node);
    std::thread::sleep(Duration::from_secs(4));
    assert_eq!(stored_heartbeat(&server, node), first);
    assert!(!is_stale(config, node));
    assert!(!ids(list_stale(config, workflow_id, true).items).contains(&node));
    assert!(ids(list_stale(config, workflow_id, false).items).contains(&node));

    drop(heartbeater);
    let last = apis::compute_nodes_api::get_compute_node(config, node)
        .unwrap()
        .last_heartbeat;
    assert!(last > first);
    assert!(
        wait_until(Duration::from_secs(20), || is_stale(config, node)),
        "node did not become stale"
    );
    assert_eq!(ids(list_stale(config, workflow_id, true).items), vec![node]);
    assert_eq!(stored_heartbeat(&server, node), last);
}

/// After a restart the in-memory heartbeats are gone. A node that does not come back becomes
/// stale from its stored heartbeat. A node that sends a heartbeat again does not.
#[test]
fn test_node_that_dies_during_a_server_outage_becomes_stale() {
    let mut server = start_stale_server("5");
    let workflow_id = create_test_workflow(&server.config, "test_outage")
        .id
        .unwrap();
    let dead = create_node(&server.config, workflow_id, "dead-host", None);
    let live = create_node(&server.config, workflow_id, "live-host", None);
    for node in [dead, live] {
        apis::compute_nodes_api::heartbeat_compute_node(&server.config, node).unwrap();
    }

    restart_server_with_env(&mut server, &[(STALE_ENV, "5")]);
    let config = &server.config;
    let _heartbeater = Heartbeater::start(config, live);
    assert!(!is_stale(config, dead));
    assert!(
        wait_until(Duration::from_secs(20), || is_stale(config, dead)),
        "node did not become stale after the restart"
    );
    assert!(!is_stale(config, live));
    assert_eq!(ids(list_stale(config, workflow_id, true).items), vec![dead]);
}

#[test]
fn test_malformed_heartbeat_is_never_stale() {
    let server = start_stale_server("2");
    let config = &server.config;
    let workflow_id = create_test_workflow(config, "test_malformed_heartbeat")
        .id
        .unwrap();
    let node = create_node(
        config,
        workflow_id,
        "bad-host",
        Some("not-a-time".to_string()),
    );
    std::thread::sleep(Duration::from_secs(3));
    assert!(!is_stale(config, node));
    assert!(list_stale(config, workflow_id, true).items.is_empty());
}

/// A running job on a dead runner blocks `reset-status`; after the node is failed, the reset
/// succeeds without `--force`.
#[rstest]
fn test_failed_node_lets_reset_status_run_without_force(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "test_fail_unblocks_reset")
        .id
        .unwrap();
    let node = create_node(config, workflow_id, "dead-host", Some(heartbeat_age(0)));
    let (job_id, _) = start_running_job(config, workflow_id, "dead_job", node);
    fail_node(config, node);

    assert_eq!(job_status(config, job_id), models::JobStatus::Failed);
    apis::workflows_api::reset_workflow_status(config, workflow_id, None)
        .expect("reset-status should not need --force after the node is failed");
}

#[test]
fn test_fresh_heartbeat_is_not_stale() {
    let server = start_stale_server("3");
    let config = &server.config;
    let workflow_id = create_test_workflow(config, "test_fresh_heartbeat_not_stale")
        .id
        .unwrap();
    let node = create_node(config, workflow_id, "live-host", Some(heartbeat_age(0)));
    let (job_id, _) = start_running_job(config, workflow_id, "live_job", node);

    // Run for twice the threshold, heartbeating every half second.
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(6) {
        let model = apis::compute_nodes_api::heartbeat_compute_node(config, node)
            .expect("heartbeat failed");
        assert_eq!(model.is_active, Some(true));
        assert_eq!(model.is_stale, Some(false));
        std::thread::sleep(Duration::from_millis(500));
    }
    assert!(is_node_active(config, node));
    assert_eq!(job_status(config, job_id), models::JobStatus::Running);

    assert!(
        wait_until(Duration::from_secs(20), || is_stale(config, node)),
        "node did not become stale after heartbeats stopped"
    );
}

#[test]
fn test_node_without_heartbeat_is_never_stale() {
    let server = start_stale_server("2");
    let config = &server.config;
    let workflow_id = create_test_workflow(config, "test_null_heartbeat_not_stale")
        .id
        .unwrap();
    let legacy_node = create_node(config, workflow_id, "legacy-host", None);
    let (job_id, _) = start_running_job(config, workflow_id, "legacy_job", legacy_node);
    // A stale node on the same server proves the threshold passed during the wait.
    let stale_node = create_node(config, workflow_id, "stale-host", Some(heartbeat_age(3600)));

    assert!(
        wait_until(Duration::from_secs(20), || is_stale(config, stale_node)),
        "node did not become stale"
    );
    assert!(!is_stale(config, legacy_node));
    assert!(is_node_active(config, legacy_node));
    assert_eq!(job_status(config, job_id), models::JobStatus::Running);
    let node = apis::compute_nodes_api::get_compute_node(config, legacy_node).unwrap();
    assert_eq!(node.last_heartbeat, None);
}

#[test]
fn test_stale_after_zero_disables_stale_marking() {
    let server = start_stale_server("0");
    let config = &server.config;
    let workflow_id = create_test_workflow(config, "test_stale_disabled")
        .id
        .unwrap();
    let node = create_node(config, workflow_id, "old-host", Some(heartbeat_age(1000)));

    std::thread::sleep(Duration::from_secs(2));

    assert!(!is_stale(config, node));
    assert!(list_stale(config, workflow_id, true).items.is_empty());
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
    let mut update = new_node(workflow_id, "inactive-host");
    update.is_active = Some(false);
    apis::compute_nodes_api::update_compute_node(config, inactive, update).unwrap();
    let model = apis::compute_nodes_api::heartbeat_compute_node(config, inactive).unwrap();
    assert_eq!(model.is_active, Some(false));
    assert_eq!(model.last_heartbeat.as_deref(), Some(old.as_str()));

    assert!(apis::compute_nodes_api::heartbeat_compute_node(config, i64::MAX - 1).is_err());
}

/// Runs `torc run` on a one-job workflow and waits until the job started. The job writes its
/// pid to `job.pid` in `dir`, then runs `job_tail`. Returns the guard, ids, and the log path.
#[cfg(unix)]
fn spawn_runner(
    config: &Configuration,
    dir: &std::path::Path,
    name: &str,
    job_tail: &str,
) -> (RunnerGuard, i64, i64, i64, std::path::PathBuf) {
    let pid_file = dir.join("job.pid");
    let spec_path = dir.join("spec.yaml");
    let log_file = dir.join("runner.log");
    std::fs::write(
        &spec_path,
        format!(
            "name: {name}\njobs:\n  - name: long_job\n    command: \"echo $$ > {} && {job_tail}\"\n",
            pid_file.display()
        ),
    )
    .unwrap();

    let runner = std::process::Command::new(torc_binary_path())
        .args(["--url", &config.base_path, "run"])
        .arg(&spec_path)
        .current_dir(dir)
        .env("TORC_CLIENT__RUN__HEARTBEAT_INTERVAL_SECS", "1")
        .env("RUST_LOG", "warn")
        .stdout(std::process::Stdio::null())
        .stderr(std::fs::File::create(&log_file).unwrap())
        .spawn()
        .expect("failed to spawn torc run");
    let guard = RunnerGuard {
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
            Some(name),
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
    let node_id = apis::compute_nodes_api::list_compute_nodes(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        Some(true),
        None,
        None,
    )
    .unwrap()
    .items
    .pop()
    .expect("no active compute node")
    .id
    .unwrap();
    let job_id = apis::jobs_api::list_jobs(
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
    .items[0]
        .id
        .unwrap();
    (guard, workflow_id, node_id, job_id, log_file)
}

/// A runner killed with SIGKILL cannot deactivate its node. The node becomes stale, stays
/// active, and its job keeps running until the node is failed.
#[cfg(unix)]
#[test]
fn test_killed_runner_node_becomes_stale() {
    ensure_test_binaries_built();
    let server = start_stale_server("4");
    let config = &server.config;
    let dir = tempfile::tempdir().unwrap();
    let (mut guard, workflow_id, node_id, job_id, _) =
        spawn_runner(config, dir.path(), "killed_runner", "exec sleep 120");
    let node = apis::compute_nodes_api::get_compute_node(config, node_id).unwrap();
    assert!(node.last_heartbeat.is_some(), "runner set no heartbeat");

    guard.runner.kill().expect("failed to SIGKILL the runner");
    let _ = guard.runner.wait();

    assert!(
        wait_until(Duration::from_secs(30), || is_stale(config, node_id)),
        "node of the killed runner did not become stale"
    );
    assert!(is_node_active(config, node_id));
    assert_eq!(job_status(config, job_id), models::JobStatus::Running);

    fail_node(config, node_id);
    assert_eq!(job_status(config, job_id), models::JobStatus::Failed);
    assert_eq!(
        result_return_codes(config, workflow_id, job_id),
        vec![models::ORPHANED_JOB_RETURN_CODE]
    );
}

/// Failing a node through the server stops its runner and kills its job.
#[cfg(unix)]
#[test]
fn test_runner_stops_when_its_node_is_failed() {
    ensure_test_binaries_built();
    let server = start_server_with_env(&[]);
    let config = &server.config;
    let dir = tempfile::tempdir().unwrap();
    let (mut guard, workflow_id, node_id, job_id, log_file) =
        spawn_runner(config, dir.path(), "fenced_runner", "exec sleep 120");
    let pid_file = dir.path().join("job.pid");

    fail_node(config, node_id);
    assert!(
        wait_until(Duration::from_secs(30), || guard
            .runner
            .try_wait()
            .unwrap()
            .is_some()),
        "runner did not stop after its node was failed"
    );
    let log = std::fs::read_to_string(&log_file).unwrap();
    assert!(
        log.contains("was failed by the server"),
        "runner stopped for another reason: {log}"
    );
    let pid = std::fs::read_to_string(&pid_file).unwrap();
    assert!(
        wait_until(Duration::from_secs(10), || !std::process::Command::new(
            "kill"
        )
        .args(["-0", pid.trim()])
        .status()
        .unwrap()
        .success()),
        "job process still alive"
    );
    assert_eq!(job_status(config, job_id), models::JobStatus::Failed);
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

/// After a node is failed and the job retried, the old runner's late completion must not
/// complete the new attempt.
#[rstest]
fn test_late_completion_after_retry_is_rejected(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "test_late_completion_after_retry")
        .id
        .unwrap();
    let old_node = create_node(config, workflow_id, "old-host", Some(heartbeat_age(0)));
    let (job_id, run_id) = start_running_job(config, workflow_id, "retried_job", old_node);
    fail_node(config, old_node);
    assert_eq!(job_status(config, job_id), models::JobStatus::Failed);

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
        "a late completion must be rejected"
    );
    assert_eq!(job_status(config, job_id), models::JobStatus::Running);
}

/// After a node is failed and the job is reset in the same run, the old runner's late
/// completion must not complete the job that is ready to run again.
#[rstest]
fn test_late_completion_after_reset_is_rejected(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "test_late_completion_after_reset")
        .id
        .unwrap();
    let old_node = create_node(config, workflow_id, "old-host", Some(heartbeat_age(0)));
    let (job_id, run_id) = start_running_job(config, workflow_id, "reset_job", old_node);
    fail_node(config, old_node);
    assert_eq!(job_status(config, job_id), models::JobStatus::Failed);

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
        "a late completion must be rejected"
    );
    assert_eq!(job_status(config, job_id), models::JobStatus::Ready);
}

/// A pending job with no node, claimed without a node, rejects a completion from an inactive node.
#[rstest]
fn test_late_completion_on_pending_job_from_inactive_node_is_rejected(
    start_server: &ServerProcess,
) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "test_late_completion_pending")
        .id
        .unwrap();
    let old_node = create_node(config, workflow_id, "old-host", Some(heartbeat_age(0)));
    fail_node(config, old_node);

    let job = models::JobModel::new(workflow_id, "pending_job".to_string(), "echo".to_string());
    let job_id = apis::jobs_api::create_job(config, job).unwrap().id.unwrap();
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None).unwrap();
    let run_id = apis::workflows_api::get_workflow(config, workflow_id)
        .unwrap()
        .run_id
        .unwrap_or(0);
    apis::workflows_api::claim_next_jobs(config, workflow_id, Some(1), None, None, None)
        .expect("claim failed");
    assert_eq!(job_status(config, job_id), models::JobStatus::Pending);

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
        "a completion from an inactive node must be rejected"
    );
    assert_eq!(job_status(config, job_id), models::JobStatus::Pending);
}

/// A job cannot start on a node that was failed.
#[rstest]
fn test_start_on_failed_node_is_rejected(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "test_start_on_failed_node")
        .id
        .unwrap();
    let failed = create_node(config, workflow_id, "failed-host", Some(heartbeat_age(0)));
    fail_node(config, failed);

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
            Some(failed),
        )
        .is_err(),
        "a claim for a failed node must be rejected"
    );
    apis::workflows_api::claim_next_jobs(config, workflow_id, Some(1), None, None, None)
        .expect("claim failed");
    assert!(
        apis::jobs_api::start_job(config, job_id, run_id, failed).is_err(),
        "start on a failed node must be rejected"
    );
    assert_eq!(job_status(config, job_id), models::JobStatus::Pending);
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

#[rstest]
fn test_fail_compute_node_fails_only_its_running_jobs(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "test_fail_node_scope")
        .id
        .unwrap();
    let a = create_node(config, workflow_id, "host-a", Some(heartbeat_age(0)));
    let other_workflow_id = create_test_workflow(config, "test_fail_node_scope_other")
        .id
        .unwrap();
    let b = create_node(config, other_workflow_id, "host-b", Some(heartbeat_age(0)));
    let (job_a, _) = start_running_job(config, workflow_id, "job_a", a);
    let (job_b, _) = start_running_job(config, other_workflow_id, "job_b", b);

    let failed = fail_node(config, a);
    assert_eq!(failed.is_active, Some(false));
    assert_eq!(job_status(config, job_a), models::JobStatus::Failed);
    assert_eq!(
        result_return_codes(config, workflow_id, job_a),
        vec![models::ORPHANED_JOB_RETURN_CODE]
    );
    assert_eq!(job_status(config, job_b), models::JobStatus::Running);
    assert!(is_node_active(config, b));
}

#[rstest]
fn test_fail_compute_node_is_idempotent(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "test_fail_node_idempotent")
        .id
        .unwrap();
    let a = create_node(config, workflow_id, "host-a", Some(heartbeat_age(0)));
    let (job_a, _) = start_running_job(config, workflow_id, "job_a", a);

    fail_node(config, a);
    fail_node(config, a);
    assert_eq!(result_return_codes(config, workflow_id, job_a).len(), 1);
}

#[rstest]
fn test_fail_unknown_compute_node_is_not_found(start_server: &ServerProcess) {
    let config = &start_server.config;
    let err = apis::compute_nodes_api::fail_compute_node(config, 9_999_999).unwrap_err();
    assert!(
        matches!(&err, torc::client::apis::Error::ResponseError(r)
            if r.status == reqwest::StatusCode::NOT_FOUND),
        "expected 404, got {err:?}"
    );
}

#[rstest]
fn test_fail_compute_node_without_heartbeat(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "test_fail_node_no_heartbeat")
        .id
        .unwrap();
    let a = create_node(config, workflow_id, "legacy-host", None);
    let (job_a, _) = start_running_job(config, workflow_id, "job_a", a);

    let failed = fail_node(config, a);
    assert_eq!(failed.is_active, Some(false));
    assert_eq!(job_status(config, job_a), models::JobStatus::Failed);
}

#[rstest]
fn test_fail_skips_a_job_that_was_already_completed(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "test_fail_node_after_complete")
        .id
        .unwrap();
    let a = create_node(config, workflow_id, "host-a", Some(heartbeat_age(0)));
    let (job_a, run_id) = start_running_job(config, workflow_id, "job_a", a);
    let done = models::ResultModel::new(
        job_a,
        workflow_id,
        run_id,
        1,
        a,
        0,
        0.1,
        chrono::Utc::now().to_rfc3339(),
        models::JobStatus::Completed,
    );
    apis::jobs_api::complete_job(config, job_a, models::JobStatus::Completed, run_id, done)
        .expect("complete_job failed");

    fail_node(config, a);
    assert_eq!(job_status(config, job_a), models::JobStatus::Completed);
    assert_eq!(result_return_codes(config, workflow_id, job_a), vec![0]);
}

#[rstest]
fn test_heartbeat_after_fail_reports_inactive(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "test_heartbeat_after_fail")
        .id
        .unwrap();
    let a = create_node(config, workflow_id, "host-a", Some(heartbeat_age(0)));
    fail_node(config, a);
    let node = apis::compute_nodes_api::heartbeat_compute_node(config, a).unwrap();
    assert_eq!(node.is_active, Some(false));
}

fn deactivate_node(config: &Configuration, workflow_id: i64, id: i64) {
    let mut update = new_node(workflow_id, "host");
    update.is_active = Some(false);
    apis::compute_nodes_api::update_compute_node(config, id, update).unwrap();
}

/// The orphan fallback reports each job on its own node, not on the first node of the workflow.
#[rstest]
fn test_orphan_fallback_completes_each_job_with_its_own_node(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "test_orphan_fallback_own_node")
        .id
        .unwrap();
    let a = create_node(config, workflow_id, "host-a", Some(heartbeat_age(0)));
    let b = create_node(config, workflow_id, "host-b", Some(heartbeat_age(0)));
    let mut job_ids = Vec::new();
    for name in ["job_a", "job_b"] {
        let job = models::JobModel::new(workflow_id, name.to_string(), "echo test".to_string());
        job_ids.push(
            apis::jobs_api::create_job(config, job)
                .expect("Failed to create job")
                .id
                .unwrap(),
        );
    }
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");
    let run_id = apis::workflows_api::get_workflow(config, workflow_id)
        .expect("Failed to get workflow")
        .run_id
        .unwrap_or(0);
    apis::workflows_api::claim_next_jobs(config, workflow_id, Some(2), None, None, None)
        .expect("Failed to claim jobs");
    apis::jobs_api::start_job(config, job_ids[0], run_id, a).expect("Failed to start job_a");
    apis::jobs_api::start_job(config, job_ids[1], run_id, b).expect("Failed to start job_b");
    deactivate_node(config, workflow_id, a);
    deactivate_node(config, workflow_id, b);

    torc::client::commands::orphan_detection::cleanup_orphaned_jobs(config, workflow_id, false)
        .unwrap();

    assert_eq!(job_status(config, job_ids[0]), models::JobStatus::Failed);
    assert_eq!(job_status(config, job_ids[1]), models::JobStatus::Failed);
}

#[rstest]
fn test_node_without_is_active_counts_as_active(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "test_null_is_active")
        .id
        .unwrap();
    let node = new_node(workflow_id, "legacy-host");
    let id = apis::compute_nodes_api::create_compute_node(config, node)
        .unwrap()
        .id
        .unwrap();
    let fetched = apis::compute_nodes_api::get_compute_node(config, id).unwrap();
    assert_ne!(fetched.is_active, Some(false));

    let beat = apis::compute_nodes_api::heartbeat_compute_node(config, id).unwrap();
    assert_ne!(beat.is_active, Some(false));
    assert!(beat.last_heartbeat.is_some(), "heartbeat not recorded");
}

#[rstest]
fn test_concurrent_fail_calls_record_one_result(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "test_concurrent_fail")
        .id
        .unwrap();
    let a = create_node(config, workflow_id, "host-a", Some(heartbeat_age(0)));
    let (job_a, _) = start_running_job(config, workflow_id, "job_a", a);

    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        for _ in 0..2 {
            scope.spawn(|| {
                barrier.wait();
                apis::compute_nodes_api::fail_compute_node(config, a).expect("fail call failed");
            });
        }
    });
    assert_eq!(job_status(config, job_a), models::JobStatus::Failed);
    assert_eq!(result_return_codes(config, workflow_id, job_a).len(), 1);
}

#[rstest]
fn test_fail_deactivates_node_created_without_is_active(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "test_fail_null_is_active")
        .id
        .unwrap();
    let node = new_node(workflow_id, "legacy-host");
    let id = apis::compute_nodes_api::create_compute_node(config, node)
        .unwrap()
        .id
        .unwrap();
    let (job, _) = start_running_job(config, workflow_id, "job_a", id);

    let failed = fail_node(config, id);
    assert_eq!(failed.is_active, Some(false));
    assert_eq!(job_status(config, job), models::JobStatus::Failed);
    let beat = apis::compute_nodes_api::heartbeat_compute_node(config, id).unwrap();
    assert_eq!(beat.is_active, Some(false));
}
