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
        output_files: None,
        slurm_stats: None,
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

/// The runner skips output-file st_mtime updates while offline, so reconcile records them
/// after replaying the completions.
#[test]
#[serial(offline_reconcile)]
fn test_reconcile_refreshes_output_file_mtimes() {
    let server = start_server();
    let config = &server.config;

    let workflow = create_test_workflow(config, "offline_reconcile_mtimes");
    let workflow_id = workflow.id.unwrap();

    let base = tempfile::tempdir().expect("tempdir");
    let output_path = base.path().join("out.txt");
    let output = apis::files_api::create_file(
        config,
        models::FileModel::new(
            workflow_id,
            "out".to_string(),
            output_path.to_string_lossy().to_string(),
        ),
    )
    .expect("create file");
    let output_id = output.id.unwrap();

    let mut job = models::JobModel::new(workflow_id, "producer".into(), "true".into());
    job.output_file_ids = Some(vec![output_id]);
    let job_id = apis::jobs_api::create_job(config, job)
        .expect("create job")
        .id
        .unwrap();

    let torc_config = TorcConfig::load().unwrap_or_default();
    WorkflowManager::new(config.clone(), torc_config, workflow)
        .initialize(true)
        .expect("Failed to initialize");
    let run_id = 1;
    let compute_node_id = create_test_compute_node(config, workflow_id).id.unwrap();

    // The job "ran" offline and wrote its output.
    std::fs::write(&output_path, "done").expect("write output");
    let journal = OfflineJournal::open_or_create(
        &base.path().join("torc_output"),
        workflow_id,
        run_id,
        "node",
    )
    .expect("open journal");
    journal
        .append(&completion_entry(
            workflow_id,
            run_id,
            compute_node_id,
            job_id,
        ))
        .expect("append");

    let summary = reconcile(config, workflow_id, run_id, base.path(), "table")
        .expect("reconcile should succeed");
    assert_eq!(summary.accepted, 1);

    let file = apis::files_api::get_file(config, output_id).expect("get file");
    assert!(
        file.st_mtime.is_some(),
        "reconcile should record the output file's mtime"
    );
}

/// For RO-Crate workflows, reconcile restores the output provenance (File and CreateAction
/// entities) that the runner skips while offline.
#[test]
#[serial(offline_reconcile)]
fn test_reconcile_restores_ro_crate_output_provenance() {
    let server = start_server();
    let config = &server.config;

    let mut workflow = create_test_workflow(config, "offline_reconcile_ro_crate");
    let workflow_id = workflow.id.unwrap();
    workflow.enable_ro_crate = Some(true);
    let workflow = apis::workflows_api::update_workflow(config, workflow_id, workflow)
        .expect("enable RO-Crate");

    let base = tempfile::tempdir().expect("tempdir");
    let output_path = base.path().join("out.txt");
    let output_id = apis::files_api::create_file(
        config,
        models::FileModel::new(
            workflow_id,
            "out".to_string(),
            output_path.to_string_lossy().to_string(),
        ),
    )
    .expect("create file")
    .id
    .unwrap();
    let mut job = models::JobModel::new(workflow_id, "producer".into(), "true".into());
    job.output_file_ids = Some(vec![output_id]);
    let job_id = apis::jobs_api::create_job(config, job)
        .expect("create job")
        .id
        .unwrap();

    WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    )
    .initialize(true)
    .expect("Failed to initialize");
    let run_id = apis::workflows_api::get_workflow(config, workflow_id)
        .unwrap()
        .run_id
        .unwrap();
    let compute_node_id = create_test_compute_node(config, workflow_id).id.unwrap();

    std::fs::write(&output_path, "done").expect("write output");
    OfflineJournal::open_or_create(
        &base.path().join("torc_output"),
        workflow_id,
        run_id,
        "node",
    )
    .expect("open journal")
    .append(&completion_entry(
        workflow_id,
        run_id,
        compute_node_id,
        job_id,
    ))
    .expect("append");

    reconcile(config, workflow_id, run_id, base.path(), "table").expect("reconcile");

    let entities = apis::ro_crate_api::list_ro_crate_entities(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("list entities")
    .items;
    assert!(
        entities.iter().any(|e| e.file_id == Some(output_id)),
        "output file should get a provenance entity"
    );
    assert!(
        entities.iter().any(|e| e.entity_type == "CreateAction"),
        "the job should get a CreateAction entity"
    );
}

/// Relative output paths resolve against the workflow's submission directory, and a failed
/// metadata restore (output not visible yet) is retried by re-running reconcile even though
/// the completion itself was already accepted.
#[test]
#[serial(offline_reconcile)]
fn test_reconcile_resolves_relative_paths_and_retries_metadata() {
    let server = start_server();
    let config = &server.config;
    let base = tempfile::tempdir().expect("tempdir");

    let mut workflow = models::WorkflowModel::new(
        "offline_reconcile_relative".to_string(),
        "test_user".to_string(),
    );
    workflow.submission_directory = Some(base.path().to_string_lossy().to_string());
    let workflow = apis::workflows_api::create_workflow(config, workflow).expect("create workflow");
    let workflow_id = workflow.id.unwrap();

    let output_id = apis::files_api::create_file(
        config,
        models::FileModel::new(workflow_id, "out".to_string(), "rel_out.txt".to_string()),
    )
    .expect("create file")
    .id
    .unwrap();
    let mut job = models::JobModel::new(workflow_id, "producer".into(), "true".into());
    job.output_file_ids = Some(vec![output_id]);
    let job_id = apis::jobs_api::create_job(config, job)
        .expect("create job")
        .id
        .unwrap();

    WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    )
    .initialize(true)
    .expect("Failed to initialize");
    let run_id = apis::workflows_api::get_workflow(config, workflow_id)
        .unwrap()
        .run_id
        .unwrap();
    let compute_node_id = create_test_compute_node(config, workflow_id).id.unwrap();
    OfflineJournal::open_or_create(
        &base.path().join("torc_output"),
        workflow_id,
        run_id,
        "node",
    )
    .expect("open journal")
    .append(&completion_entry(
        workflow_id,
        run_id,
        compute_node_id,
        job_id,
    ))
    .expect("append");

    // First reconcile: the completion is accepted, but the output is not visible yet.
    let summary = reconcile(config, workflow_id, run_id, base.path(), "table").expect("reconcile");
    assert_eq!(summary.accepted, 1);
    assert!(
        apis::files_api::get_file(config, output_id)
            .unwrap()
            .st_mtime
            .is_none(),
        "no mtime while the output is missing"
    );

    // The output appears (relative to the submission directory); re-running retries it.
    std::fs::write(base.path().join("rel_out.txt"), "done").expect("write output");
    let summary = reconcile(config, workflow_id, run_id, base.path(), "table").expect("reconcile");
    assert_eq!(summary.accepted, 0, "the completion was already accepted");
    assert!(
        apis::files_api::get_file(config, output_id)
            .unwrap()
            .st_mtime
            .is_some(),
        "re-running reconcile should restore the mtime"
    );
}

/// A journal written by a current runner carries the completion-time output mtimes and Slurm
/// stats. Replay applies them as journaled: the output is not restatted (here it no longer
/// exists on disk), and the stats are stored.
#[test]
#[serial(offline_reconcile)]
fn test_reconcile_applies_journaled_mtimes_and_slurm_stats() {
    let server = start_server();
    let config = &server.config;

    let workflow = create_test_workflow(config, "offline_reconcile_payload");
    let workflow_id = workflow.id.unwrap();
    let base = tempfile::tempdir().expect("tempdir");
    let output_id = apis::files_api::create_file(
        config,
        models::FileModel::new(
            workflow_id,
            "out".to_string(),
            base.path().join("gone.txt").to_string_lossy().to_string(),
        ),
    )
    .expect("create file")
    .id
    .unwrap();
    let mut job = models::JobModel::new(workflow_id, "producer".into(), "true".into());
    job.output_file_ids = Some(vec![output_id]);
    let job_id = apis::jobs_api::create_job(config, job)
        .expect("create job")
        .id
        .unwrap();

    WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    )
    .initialize(true)
    .expect("Failed to initialize");
    let run_id = apis::workflows_api::get_workflow(config, workflow_id)
        .unwrap()
        .run_id
        .unwrap();
    let compute_node_id = create_test_compute_node(config, workflow_id).id.unwrap();

    let mut entry = completion_entry(workflow_id, run_id, compute_node_id, job_id);
    entry.output_files = Some(vec![models::OutputFileStat {
        file_id: output_id,
        st_mtime: 1_700_000_000.25,
    }]);
    let mut stats = models::SlurmStatsModel::new(workflow_id, job_id, run_id, 1);
    stats.slurm_job_id = Some("12345".to_string());
    stats.max_rss_bytes = Some(1 << 20);
    entry.slurm_stats = Some(stats);
    OfflineJournal::open_or_create(
        &base.path().join("torc_output"),
        workflow_id,
        run_id,
        "node",
    )
    .expect("open journal")
    .append(&entry)
    .expect("append");

    let summary = reconcile(config, workflow_id, run_id, base.path(), "table").expect("reconcile");
    assert_eq!(summary.accepted, 1);
    assert_eq!(
        apis::files_api::get_file(config, output_id)
            .unwrap()
            .st_mtime,
        Some(1_700_000_000.25),
        "the journaled completion-time mtime should be recorded"
    );
    let stats = apis::slurm_stats_api::list_slurm_stats(
        config,
        workflow_id,
        Some(job_id),
        None,
        None,
        None,
        None,
    )
    .unwrap()
    .items;
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].slurm_job_id.as_deref(), Some("12345"));
    assert_eq!(stats[0].max_rss_bytes, Some(1 << 20));
}
