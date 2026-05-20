//! Integration tests for the orchestrator-continuation feature (`spawn_jobs`).
//!
//! The orchestrator adds child jobs blocked on itself and exits normally; the
//! runner completes the orchestrator on exit, and the unblock cascade then
//! promotes the spawned jobs. These tests model that runner flow by leaving the
//! caller `Running` across the `spawn_jobs` call and then calling `complete_job`
//! on it to stand in for the runner's post-exit completion.
//!
//! See `docs/plans/dynamic-jobs-design.md`.

mod common;

use common::{ServerProcess, start_server};
use rstest::rstest;
use std::thread::sleep;
use std::time::{Duration, Instant};
use torc::client::apis;
use torc::client::workflow_manager::WorkflowManager;
use torc::config::TorcConfig;
use torc::models::{self, JobStatus};

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn setup(config: &torc::client::Configuration, name: &str, max_iters: Option<i64>) -> (i64, i64) {
    let mut wf = models::WorkflowModel::new(name.to_string(), "test_user".to_string());
    wf.max_spawn_iterations_per_lineage = max_iters;
    let created = apis::workflows_api::create_workflow(config, wf).expect("create_workflow failed");
    let workflow_id = created.id.unwrap();

    let cn = models::ComputeNodeModel::new(
        workflow_id,
        "test-host".to_string(),
        std::process::id() as i64,
        now(),
        64,
        256.0,
        0,
        1,
        "local".to_string(),
        None,
    );
    let compute_node_id = apis::compute_nodes_api::create_compute_node(config, cn)
        .expect("create_compute_node failed")
        .id
        .unwrap();

    let rr = models::ResourceRequirementsModel::new(workflow_id, "rr".to_string());
    apis::resource_requirements_api::create_resource_requirements(config, rr)
        .expect("create_resource_requirements failed");

    (workflow_id, compute_node_id)
}

fn seed_and_init(config: &torc::client::Configuration, workflow_id: i64, seed_name: &str) -> i64 {
    let job = models::JobModel::new(
        workflow_id,
        seed_name.to_string(),
        format!("echo {}", seed_name),
    );
    let job_id = apis::jobs_api::create_job(config, job)
        .expect("create_job failed")
        .id
        .unwrap();

    let workflow = apis::workflows_api::get_workflow(config, workflow_id).expect("get_workflow");
    let torc_config = TorcConfig::load().unwrap_or_default();
    let manager = WorkflowManager::new(config.clone(), torc_config, workflow);
    manager.initialize(false).expect("initialize failed");
    job_id
}

fn run_id_of(config: &torc::client::Configuration, workflow_id: i64) -> i64 {
    apis::workflows_api::get_workflow(config, workflow_id)
        .expect("get_workflow")
        .run_id
        .unwrap_or(1)
}

fn wait_for_status(config: &torc::client::Configuration, job_id: i64, expected: JobStatus) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let job = apis::jobs_api::get_job(config, job_id).expect("get_job");
        if job.status == Some(expected) {
            return;
        }
        if Instant::now() > deadline {
            panic!(
                "job {} did not reach {:?} (last: {:?})",
                job_id, expected, job.status
            );
        }
        sleep(Duration::from_millis(100));
    }
}

/// Stand in for the runner: claim, mark Running, run "as the orchestrator",
/// then close the loop. Splits the lifecycle so the test can call `spawn_jobs`
/// in between.
fn claim_and_mark_running(
    config: &torc::client::Configuration,
    workflow_id: i64,
    run_id: i64,
    job_id: i64,
) {
    let resources = models::ComputeNodesResources::new(64, 100.0, 0, 1);
    apis::workflows_api::claim_jobs_based_on_resources(config, workflow_id, 10, resources, None)
        .expect("claim failed");
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id)
        .expect("set running failed");
}

/// What the real runner does after the subprocess exits 0.
fn runner_completes(
    config: &torc::client::Configuration,
    workflow_id: i64,
    run_id: i64,
    compute_node_id: i64,
    job_id: i64,
) {
    let result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        1,
        compute_node_id,
        0,
        0.1,
        now(),
        JobStatus::Completed,
    );
    apis::jobs_api::complete_job(config, job_id, JobStatus::Completed, run_id, result)
        .expect("complete_job failed");
}

/// Run a worker job (no spawn): claim, mark Running, then complete it.
fn finish_job(
    config: &torc::client::Configuration,
    workflow_id: i64,
    run_id: i64,
    compute_node_id: i64,
    job_id: i64,
) {
    claim_and_mark_running(config, workflow_id, run_id, job_id);
    runner_completes(config, workflow_id, run_id, compute_node_id, job_id);
}

fn spawn_job(name: &str, deps: &[&str], priority: i64) -> models::SpawnJobModel {
    models::SpawnJobModel {
        name: name.to_string(),
        command: format!("echo {}", name),
        resource_requirements: Some("rr".to_string()),
        priority: Some(priority),
        cancel_on_blocking_job_failure: Some(false),
        depends_on: if deps.is_empty() {
            None
        } else {
            Some(deps.iter().map(|s| s.to_string()).collect())
        },
    }
}

fn all_user_data(
    config: &torc::client::Configuration,
    workflow_id: i64,
) -> Vec<models::UserDataModel> {
    apis::user_data_api::list_user_data(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("list_user_data failed")
    .items
}

fn gen_record_names(
    config: &torc::client::Configuration,
    workflow_id: i64,
    lineage: &str,
) -> Vec<String> {
    let prefix = format!("__torc_lineage__{}__g", lineage);
    let mut names: Vec<String> = all_user_data(config, workflow_id)
        .into_iter()
        .map(|u| u.name)
        .filter(|n| n.starts_with(&prefix))
        .collect();
    names.sort();
    names
}

fn latest_gen_state(
    config: &torc::client::Configuration,
    workflow_id: i64,
    lineage: &str,
) -> Option<serde_json::Value> {
    let names = gen_record_names(config, workflow_id, lineage);
    let last = names.last()?.clone();
    all_user_data(config, workflow_id)
        .into_iter()
        .find(|u| u.name == last)
        .and_then(|u| u.data)
}

fn gen_state(
    config: &torc::client::Configuration,
    workflow_id: i64,
    lineage: &str,
    generation: i64,
) -> Option<serde_json::Value> {
    let name = format!("__torc_lineage__{}__g{:06}", lineage, generation);
    all_user_data(config, workflow_id)
        .into_iter()
        .find(|u| u.name == name)
        .and_then(|u| u.data)
}

fn final_state(
    config: &torc::client::Configuration,
    workflow_id: i64,
    lineage: &str,
) -> Option<serde_json::Value> {
    let name = format!("__torc_lineage__{}__final", lineage);
    all_user_data(config, workflow_id)
        .into_iter()
        .find(|u| u.name == name)
        .and_then(|u| u.data)
}

/// End-to-end runner flow: orchestrator runs, spawns children blocked on
/// itself, exits; the runner completes it; the unblock cascade promotes the
/// children; the iteration's worker finishes; the next orchestrator runs and
/// converges by calling spawn_jobs with no jobs.
#[rstest]
fn test_runner_flow_continuation_and_convergence(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, cn) = setup(config, "dyn_runner_flow", Some(5));
    let orch = seed_and_init(config, workflow_id, "orch_g0");
    let run_id = run_id_of(config, workflow_id);

    // --- Generation 0: orchestrator runs and spawns one worker + continuation
    claim_and_mark_running(config, workflow_id, run_id, orch);
    let resp = apis::jobs_api::spawn_jobs(
        config,
        orch,
        models::SpawnJobsRequest {
            lineage: Some("A".to_string()),
            jobs: vec![
                spawn_job("work_A_i01", &[], 1),
                spawn_job("orch_g1", &["work_A_i01"], 0),
            ],
            state: Some(serde_json::json!({ "gen": 1 })),
        },
    )
    .expect("spawn_jobs failed");
    assert_eq!(resp.iteration, 1);
    assert_eq!(resp.spawned_job_ids.len(), 2);

    let by_name = |n: &str| {
        apis::jobs_api::list_jobs(
            config,
            workflow_id,
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
        .expect("list_jobs")
        .items
        .into_iter()
        .find(|j| j.name == n)
        .unwrap_or_else(|| panic!("job {} not found", n))
    };

    // Both spawned jobs must be Blocked — they all carry an implicit edge to
    // the still-Running orchestrator.
    let work = by_name("work_A_i01");
    let cont = by_name("orch_g1");
    assert_eq!(
        work.status,
        Some(JobStatus::Blocked),
        "work must be Blocked on orchestrator"
    );
    assert_eq!(
        cont.status,
        Some(JobStatus::Blocked),
        "continuation Blocked on orchestrator + work"
    );
    // Orchestrator is still Running after spawn_jobs — no double completion.
    assert_eq!(
        apis::jobs_api::get_job(config, orch).unwrap().status,
        Some(JobStatus::Running)
    );

    // --- Runner completes the orchestrator (script exited 0) ---------------
    runner_completes(config, workflow_id, run_id, cn, orch);
    // The worker job (no other deps) unblocks first.
    wait_for_status(config, work.id.unwrap(), JobStatus::Ready);

    // --- Worker runs and completes -> continuation unblocks --------------
    finish_job(config, workflow_id, run_id, cn, work.id.unwrap());
    wait_for_status(config, cont.id.unwrap(), JobStatus::Ready);

    // --- Generation 1: orchestrator converges (spawns nothing) ----------
    claim_and_mark_running(config, workflow_id, run_id, cont.id.unwrap());
    let resp = apis::jobs_api::spawn_jobs(
        config,
        cont.id.unwrap(),
        models::SpawnJobsRequest {
            lineage: Some("A".to_string()),
            jobs: vec![],
            state: Some(serde_json::json!({ "converged": true })),
        },
    )
    .expect("convergence call failed");
    assert!(resp.spawned_job_ids.is_empty());
    assert_eq!(resp.iteration, 1, "counter unchanged on convergence");
    let fin = final_state(config, workflow_id, "A").expect("final state");
    assert_eq!(fin["final"], serde_json::json!(true));
    assert_eq!(fin["state"]["converged"], serde_json::json!(true));

    // Runner completes the converging orchestrator -> workflow finishes.
    runner_completes(config, workflow_id, run_id, cn, cont.id.unwrap());
}

/// Two concurrent lineages in the same workflow each maintain their own
/// counter and state record.
#[rstest]
fn test_multi_lineage_independence(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, cn) = setup(config, "dyn_multi_lineage", Some(5));
    let a = seed_and_init(config, workflow_id, "orch_a_g0");
    let run_id = run_id_of(config, workflow_id);
    // Seed lineage B alongside A.
    let b = {
        let job = models::JobModel::new(workflow_id, "orch_b_g0".to_string(), "echo b".to_string());
        let id = apis::jobs_api::create_job(config, job).unwrap().id.unwrap();
        apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
            .expect("reinit failed");
        id
    };

    claim_and_mark_running(config, workflow_id, run_id, a);
    let ra = apis::jobs_api::spawn_jobs(
        config,
        a,
        models::SpawnJobsRequest {
            lineage: Some("A".to_string()),
            jobs: vec![spawn_job("work_A_i01", &[], 1)],
            state: Some(serde_json::json!({ "gen": 1 })),
        },
    )
    .unwrap();
    assert_eq!(ra.iteration, 1);

    claim_and_mark_running(config, workflow_id, run_id, b);
    let rb = apis::jobs_api::spawn_jobs(
        config,
        b,
        models::SpawnJobsRequest {
            lineage: Some("B".to_string()),
            jobs: vec![spawn_job("work_B_i01", &[], 1)],
            state: Some(serde_json::json!({ "gen": 1 })),
        },
    )
    .unwrap();
    assert_eq!(rb.iteration, 1, "B counts independently of A");

    // Each lineage has its own state record; A's is untouched by B.
    assert!(latest_gen_state(config, workflow_id, "A").is_some());
    assert!(latest_gen_state(config, workflow_id, "B").is_some());
    assert_eq!(
        latest_gen_state(config, workflow_id, "A").unwrap()["spawn_count"],
        serde_json::json!(1)
    );

    // Tidy: complete the orchestrators so we don't leak Running jobs.
    runner_completes(config, workflow_id, run_id, cn, a);
    runner_completes(config, workflow_id, run_id, cn, b);
}

/// Append-only history: two real generations leave two immutable records with
/// their own distinct state, neither overwriting the other.
#[rstest]
fn test_append_only_history(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, cn) = setup(config, "dyn_history", Some(10));
    let orch = seed_and_init(config, workflow_id, "h_g0");
    let run_id = run_id_of(config, workflow_id);

    // Generation 1.
    claim_and_mark_running(config, workflow_id, run_id, orch);
    apis::jobs_api::spawn_jobs(
        config,
        orch,
        models::SpawnJobsRequest {
            lineage: Some("H".to_string()),
            jobs: vec![
                spawn_job("h_work_i01", &[], 1),
                spawn_job("h_g1", &["h_work_i01"], 0),
            ],
            state: Some(serde_json::json!({ "gen": 1, "metric": 0.9 })),
        },
    )
    .unwrap();
    runner_completes(config, workflow_id, run_id, cn, orch);

    let work1 = apis::jobs_api::list_jobs(
        config,
        workflow_id,
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
    .unwrap()
    .items
    .into_iter()
    .find(|j| j.name == "h_work_i01")
    .unwrap();
    let cont1 = apis::jobs_api::list_jobs(
        config,
        workflow_id,
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
    .unwrap()
    .items
    .into_iter()
    .find(|j| j.name == "h_g1")
    .unwrap();
    wait_for_status(config, work1.id.unwrap(), JobStatus::Ready);
    finish_job(config, workflow_id, run_id, cn, work1.id.unwrap());
    wait_for_status(config, cont1.id.unwrap(), JobStatus::Ready);

    // Generation 2.
    claim_and_mark_running(config, workflow_id, run_id, cont1.id.unwrap());
    let r2 = apis::jobs_api::spawn_jobs(
        config,
        cont1.id.unwrap(),
        models::SpawnJobsRequest {
            lineage: Some("H".to_string()),
            jobs: vec![spawn_job("h_work_i02", &[], 1)],
            state: Some(serde_json::json!({ "gen": 2, "metric": 0.2 })),
        },
    )
    .unwrap();
    assert_eq!(r2.iteration, 2);
    runner_completes(config, workflow_id, run_id, cn, cont1.id.unwrap());

    // Both generations retained with distinct state.
    let names = gen_record_names(config, workflow_id, "H");
    assert_eq!(names.len(), 2, "two generations retained: {:?}", names);
    let s1 = gen_state(config, workflow_id, "H", 1).unwrap();
    let s2 = gen_state(config, workflow_id, "H", 2).unwrap();
    assert_eq!(s1["state"]["metric"], serde_json::json!(0.9));
    assert_eq!(s2["state"]["metric"], serde_json::json!(0.2));
}

/// A replayed spawn (same caller still Running, identical request) is an
/// idempotent no-op: no duplicate jobs, no double-counted iteration.
#[rstest]
fn test_idempotent_replay(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, _cn) = setup(config, "dyn_replay", Some(10));
    let orch = seed_and_init(config, workflow_id, "orch_g0");
    let run_id = run_id_of(config, workflow_id);

    claim_and_mark_running(config, workflow_id, run_id, orch);

    let make_req = || models::SpawnJobsRequest {
        lineage: Some("R".to_string()),
        jobs: vec![
            spawn_job("work_R_i01", &[], 1),
            spawn_job("orch_R_g1", &["work_R_i01"], 0),
        ],
        state: Some(serde_json::json!({ "gen": 1 })),
    };

    let first = apis::jobs_api::spawn_jobs(config, orch, make_req()).unwrap();
    assert_eq!(first.spawned_job_ids.len(), 2);
    assert_eq!(first.iteration, 1);

    let count_jobs = || {
        apis::jobs_api::list_jobs(
            config,
            workflow_id,
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
        .unwrap()
        .items
        .len()
    };
    let after_first = count_jobs();

    let replay = apis::jobs_api::spawn_jobs(config, orch, make_req())
        .expect("replay should succeed idempotently");
    assert_eq!(
        count_jobs(),
        after_first,
        "replay must not create duplicate jobs"
    );
    assert_eq!(replay.spawned_job_ids, first.spawned_job_ids);
    assert_eq!(replay.iteration, 1, "counter not advanced on replay");
    assert_eq!(
        gen_record_names(config, workflow_id, "R").len(),
        1,
        "replay must not append a duplicate generation record"
    );
}

/// Per-lineage cap rejects a spawn that would exceed it; the caller stays
/// Running (nothing was persisted).
#[rstest]
fn test_max_iterations_cap(start_server: &ServerProcess) {
    let config = &start_server.config;
    let (workflow_id, cn) = setup(config, "dyn_cap", Some(1));
    let orch = seed_and_init(config, workflow_id, "orch_cap_g0");
    let run_id = run_id_of(config, workflow_id);

    claim_and_mark_running(config, workflow_id, run_id, orch);
    // First spawn allowed (counter 0 -> 1; cap is 1).
    let r0 = apis::jobs_api::spawn_jobs(
        config,
        orch,
        models::SpawnJobsRequest {
            lineage: Some("C".to_string()),
            jobs: vec![
                spawn_job("c_work_i01", &[], 1),
                spawn_job("orch_cap_g1", &["c_work_i01"], 0),
            ],
            state: None,
        },
    )
    .unwrap();
    assert_eq!(r0.iteration, 1);

    runner_completes(config, workflow_id, run_id, cn, orch);

    let cont = apis::jobs_api::list_jobs(
        config,
        workflow_id,
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
    .unwrap()
    .items
    .into_iter()
    .find(|j| j.name == "orch_cap_g1")
    .unwrap();
    let work_id = apis::jobs_api::list_jobs(
        config,
        workflow_id,
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
    .unwrap()
    .items
    .into_iter()
    .find(|j| j.name == "c_work_i01")
    .unwrap()
    .id
    .unwrap();
    wait_for_status(config, work_id, JobStatus::Ready);
    finish_job(config, workflow_id, run_id, cn, work_id);
    wait_for_status(config, cont.id.unwrap(), JobStatus::Ready);
    claim_and_mark_running(config, workflow_id, run_id, cont.id.unwrap());

    let err = apis::jobs_api::spawn_jobs(
        config,
        cont.id.unwrap(),
        models::SpawnJobsRequest {
            lineage: Some("C".to_string()),
            jobs: vec![spawn_job("c_work_i02", &[], 1)],
            state: None,
        },
    )
    .expect_err("cap must reject the second spawn");
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("422") || msg.to_lowercase().contains("cap"),
        "expected a 422 cap rejection, got: {}",
        msg
    );

    // Caller untouched by the rejected call.
    assert_eq!(
        apis::jobs_api::get_job(config, cont.id.unwrap())
            .unwrap()
            .status,
        Some(JobStatus::Running),
    );
}
