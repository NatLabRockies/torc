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
                output_files: None,
                slurm_stats: None,
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

fn entry_with_payload(
    workflow_id: i64,
    run_id: i64,
    compute_node_id: i64,
    job_id: i64,
    output_files: Vec<(i64, f64)>,
) -> models::JobCompletionEntry {
    let result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1,
        compute_node_id,
        0,
        0.1,
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Completed,
    );
    let mut stats = models::SlurmStatsModel::new(workflow_id, job_id, run_id, 1);
    stats.max_rss_bytes = Some(4096);
    models::JobCompletionEntry {
        job_id,
        status: JobStatus::Completed,
        run_id,
        result,
        output_files: Some(
            output_files
                .into_iter()
                .map(|(file_id, st_mtime)| models::OutputFileStat {
                    file_id,
                    st_mtime,
                    size: None,
                    sha256: None,
                })
                .collect(),
        ),
        slurm_stats: Some(stats),
    }
}

/// Output mtimes and Slurm stats in a completion entry are written only when the completion
/// is accepted. A stale run_id rejects the entry and writes nothing. Invalid metadata (a
/// `file_id` that is not an output of the job, or stats for another attempt) is skipped
/// while the completion itself is still applied.
#[test]
#[serial(batch_complete_inmem)]
fn test_batch_complete_jobs_applies_output_mtimes_and_slurm_stats() {
    let server = start_in_memory_server();
    let config = &server.config;

    let workflow = create_test_workflow(config, "batch_complete_payload");
    let workflow_id = workflow.id.unwrap();
    let other_workflow_id = create_test_workflow(config, "batch_complete_payload_other")
        .id
        .unwrap();
    let new_file = |wf: i64, name: &str| {
        apis::files_api::create_file(
            config,
            models::FileModel::new(wf, name.to_string(), format!("/tmp/{name}")),
        )
        .expect("create file")
        .id
        .unwrap()
    };
    let out1 = new_file(workflow_id, "out1");
    let out2 = new_file(workflow_id, "out2");
    let foreign = new_file(other_workflow_id, "foreign");
    let new_job = |name: &str, output: i64| {
        let mut job = models::JobModel::new(workflow_id, name.into(), "true".into());
        job.output_file_ids = Some(vec![output]);
        apis::jobs_api::create_job(config, job)
            .expect("create job")
            .id
            .unwrap()
    };
    let job1 = new_job("job1", out1);
    let job2 = new_job("job2", out2);

    WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    )
    .initialize(true)
    .expect("Failed to initialize workflow");
    let run_id = apis::workflows_api::get_workflow(config, workflow_id)
        .unwrap()
        .run_id
        .unwrap();
    let compute_node_id = create_test_compute_node(config, workflow_id).id.unwrap();

    let complete = |entry: models::JobCompletionEntry| {
        apis::workflows_api::batch_complete_jobs(
            config,
            workflow_id,
            models::BatchCompleteJobsRequest {
                completions: vec![entry],
            },
        )
        .expect("batch_complete_jobs failed")
    };
    let mtime = |file_id| apis::files_api::get_file(config, file_id).unwrap().st_mtime;
    let stats_for = |job_id| {
        apis::slurm_stats_api::list_slurm_stats(
            config,
            workflow_id,
            Some(job_id),
            None,
            None,
            None,
            None,
        )
        .unwrap()
        .items
    };
    let status = |job_id| apis::jobs_api::get_job(config, job_id).unwrap().status;

    // Stale run: the completion is rejected and nothing is written.
    let response = complete(entry_with_payload(
        workflow_id,
        run_id + 1,
        compute_node_id,
        job1,
        vec![(out1, 111.0)],
    ));
    assert!(response.completed.is_empty(), "{:?}", response.completed);
    assert_eq!(response.errors.len(), 1, "{:?}", response.errors);
    assert_eq!(mtime(out1), None);
    assert!(stats_for(job1).is_empty());
    assert_ne!(status(job1), Some(JobStatus::Completed));

    // Another job's output, a file from another workflow, and stats for another attempt are
    // skipped; the completion and the valid mtime are applied.
    let mut entry = entry_with_payload(
        workflow_id,
        run_id,
        compute_node_id,
        job1,
        vec![(out1, 222.5), (out2, 111.0), (foreign, 111.0)],
    );
    entry.slurm_stats.as_mut().unwrap().attempt_id = 2;
    let response = complete(entry);
    assert_eq!(response.completed, vec![job1], "{:?}", response.errors);
    assert_eq!(status(job1), Some(JobStatus::Completed));
    assert_eq!(mtime(out1), Some(222.5));
    assert_eq!(mtime(out2), None);
    assert_eq!(mtime(foreign), None);
    assert!(stats_for(job1).is_empty());

    // Valid stats are stored with the completion.
    let response = complete(entry_with_payload(
        workflow_id,
        run_id,
        compute_node_id,
        job2,
        vec![(out2, 333.0)],
    ));
    assert_eq!(response.completed, vec![job2], "{:?}", response.errors);
    assert_eq!(mtime(out2), Some(333.0));
    let stats = stats_for(job2);
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].max_rss_bytes, Some(4096));

    // A duplicate completion is rejected and leaves the recorded values alone.
    let response = complete(entry_with_payload(
        workflow_id,
        run_id,
        compute_node_id,
        job2,
        vec![(out2, 444.0)],
    ));
    assert!(response.completed.is_empty());
    assert_eq!(mtime(out2), Some(333.0));
    assert_eq!(stats_for(job2).len(), 1);
}

/// For an RO-Crate workflow, an accepted completion stores each output's size and SHA-256
/// and creates the output File and CreateAction entities in the same transaction. A rejected
/// completion writes none of them. `update_file` and `update_file_stats` clear a stored size
/// and hash when `st_mtime` (or the path) changes without a new pair, and reject a size
/// without a hash, as do `create_file` and `create_files`.
#[test]
#[serial(batch_complete_inmem)]
fn test_batch_complete_jobs_stores_hashes_and_ro_crate_entities() {
    let server = start_in_memory_server();
    let config = &server.config;

    let mut workflow = models::WorkflowModel::new(
        "batch_complete_ro_crate".to_string(),
        "test_user".to_string(),
    );
    workflow.enable_ro_crate = Some(true);
    let workflow = apis::workflows_api::create_workflow(config, workflow).unwrap();
    let workflow_id = workflow.id.unwrap();
    let new_file = |name: &str| {
        apis::files_api::create_file(
            config,
            models::FileModel::new(workflow_id, name.to_string(), format!("/tmp/{name}.txt")),
        )
        .unwrap()
        .id
        .unwrap()
    };
    let input = new_file("in");
    let out1 = new_file("out1");
    let out2 = new_file("out2");
    let new_job = |name: &str, output: i64| {
        let mut job = models::JobModel::new(workflow_id, name.into(), "true".into());
        job.input_file_ids = Some(vec![input]);
        job.output_file_ids = Some(vec![output]);
        apis::jobs_api::create_job(config, job).unwrap().id.unwrap()
    };
    let job1 = new_job("job1", out1);
    let job2 = new_job("job2", out2);
    WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    )
    .initialize(true)
    .expect("Failed to initialize workflow");
    let run_id = apis::workflows_api::get_workflow(config, workflow_id)
        .unwrap()
        .run_id
        .unwrap();
    let compute_node_id = create_test_compute_node(config, workflow_id).id.unwrap();

    let hash = "ab".repeat(32);
    let entry = |run_id: i64, job_id: i64, file_id: i64, size: Option<i64>, sha: Option<&str>| {
        let mut entry = entry_with_payload(workflow_id, run_id, compute_node_id, job_id, vec![]);
        entry.output_files = Some(vec![models::OutputFileStat {
            file_id,
            st_mtime: 500.0,
            size,
            sha256: sha.map(str::to_string),
        }]);
        entry
    };
    let complete = |entry: models::JobCompletionEntry| {
        apis::workflows_api::batch_complete_jobs(
            config,
            workflow_id,
            models::BatchCompleteJobsRequest {
                completions: vec![entry],
            },
        )
        .unwrap()
    };
    let file = |file_id| apis::files_api::get_file(config, file_id).unwrap();
    let entities = || {
        apis::ro_crate_api::list_ro_crate_entities(
            config,
            workflow_id,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap()
        .items
    };
    let action_id = format!("#job-{}-attempt-1", job1);

    // Rejected (stale run): no stats, no entities.
    let response = complete(entry(run_id + 1, job1, out1, Some(7), Some(&hash)));
    assert!(response.completed.is_empty());
    let f = file(out1);
    assert_eq!((f.st_mtime, f.size, f.sha256), (None, None, None));
    assert!(
        !entities()
            .iter()
            .any(|e| e.file_id == Some(out1) || e.entity_id == action_id)
    );

    // Accepted: stats and entities written together.
    let response = complete(entry(run_id, job1, out1, Some(7), Some(&hash)));
    assert_eq!(response.completed, vec![job1], "{:?}", response.errors);
    let f = file(out1);
    assert_eq!(
        (f.st_mtime, f.size, f.sha256.as_deref()),
        (Some(500.0), Some(7), Some(hash.as_str()))
    );
    let all = entities();
    let file_entity = all.iter().find(|e| e.file_id == Some(out1)).unwrap();
    assert_eq!(file_entity.entity_id, "/tmp/out1.txt");
    assert_eq!(file_entity.metadata["contentSize"], 7);
    assert_eq!(file_entity.metadata["sha256"], hash.as_str());
    assert_eq!(
        file_entity.metadata["prov:wasGeneratedBy"]["@id"],
        action_id
    );
    assert_eq!(
        file_entity.metadata["prov:wasDerivedFrom"]["@id"],
        "/tmp/in.txt"
    );
    let action = all.iter().find(|e| e.entity_id == action_id).unwrap();
    assert_eq!(action.metadata["name"], "job1");
    assert_eq!(action.metadata["object"]["@id"], "/tmp/in.txt");
    assert_eq!(action.metadata["result"][0]["@id"], "/tmp/out1.txt");

    // A size without a hash is ignored, but the mtime, the completion, and the entities still
    // apply.
    let response = complete(entry(run_id, job2, out2, Some(7), None));
    assert_eq!(response.completed, vec![job2], "{:?}", response.errors);
    let f = file(out2);
    assert_eq!((f.st_mtime, f.size, f.sha256), (Some(500.0), None, None));
    let all = entities();
    let out2_entity = all.iter().find(|e| e.file_id == Some(out2)).unwrap();
    assert!(!out2_entity.metadata.contains_key("sha256"));
    let action2 = format!("#job-{}-attempt-1", job2);
    let action2 = all.iter().find(|e| e.entity_id == action2).unwrap();
    assert_eq!(action2.metadata["result"][0]["@id"], "/tmp/out2.txt");

    // create_file and create_files reject a size without a hash, or a pair without an mtime.
    let mut bad = models::FileModel::new(workflow_id, "bad".into(), "/tmp/bad".into());
    bad.size = Some(1);
    assert!(apis::files_api::create_file(config, bad.clone()).is_err());
    bad.sha256 = Some(hash.clone());
    assert!(apis::files_api::create_file(config, bad.clone()).is_err());
    assert!(
        apis::files_api::create_files(
            config,
            models::FilesModel {
                files: vec![bad.clone()]
            }
        )
        .is_err()
    );
    bad.st_mtime = Some(1.0);
    assert!(apis::files_api::create_file(config, bad).is_ok());

    // update_file: the same mtime keeps the stored hash; a new mtime alone clears it; a new
    // mtime with a new pair stores it; a new path clears it; a size alone is rejected.
    let mut update = file(out1);
    update.size = None;
    update.sha256 = None;
    let f = apis::files_api::update_file(config, out1, update.clone()).unwrap();
    assert_eq!(f.sha256.as_deref(), Some(hash.as_str()));
    update.st_mtime = Some(600.0);
    let f = apis::files_api::update_file(config, out1, update.clone()).unwrap();
    assert_eq!((f.st_mtime, f.size, f.sha256), (Some(600.0), None, None));
    update.size = Some(8);
    update.sha256 = Some("cd".repeat(32));
    let f = apis::files_api::update_file(config, out1, update.clone()).unwrap();
    assert_eq!((f.st_mtime, f.size), (Some(600.0), Some(8)));
    update.size = None;
    update.sha256 = None;
    update.path = "/tmp/out1_moved.txt".into();
    let f = apis::files_api::update_file(config, out1, update.clone()).unwrap();
    assert_eq!((f.st_mtime, f.size, f.sha256), (Some(600.0), None, None));
    update.size = Some(9);
    assert!(
        apis::files_api::update_file(config, out1, update.clone()).is_err(),
        "size without sha256"
    );
    update.size = Some(8);
    update.sha256 = Some("cd".repeat(32));
    apis::files_api::update_file(config, out1, update).unwrap();

    // update_file_stats follows the same rule: a new mtime alone clears the stored pair.
    let stat = |exists: bool, st_mtime: Option<f64>, sha: Option<String>| {
        apis::files_api::update_file_stats(
            config,
            workflow_id,
            models::FileStatsRequest {
                files: vec![models::FileStatEntry {
                    id: out1,
                    exists,
                    st_mtime,
                    size: sha.as_ref().map(|_| 10),
                    sha256: sha,
                }],
            },
            Some(false),
        )
        .is_ok()
    };
    assert!(stat(true, Some(700.0), None));
    let f = file(out1);
    assert_eq!((f.st_mtime, f.size, f.sha256), (Some(700.0), None, None));
    assert!(stat(true, Some(800.0), Some("ef".repeat(32))));
    let f = file(out1);
    assert_eq!((f.st_mtime, f.size), (Some(800.0), Some(10)));
    assert!(stat(false, None, None));
    let f = file(out1);
    assert_eq!((f.st_mtime, f.size, f.sha256), (None, None, None));
}
