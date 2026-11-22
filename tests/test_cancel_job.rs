mod common;

use common::{
    AccessControlServerProcess, ServerProcess, create_test_compute_node, create_test_workflow,
    run_cli_command, start_server, start_server_with_access_control,
};
use rstest::rstest;
use std::time::{Duration, Instant};
use torc::client::{Configuration, apis};
use torc::models::{self, JobStatus};

fn create_job(
    config: &Configuration,
    workflow_id: i64,
    name: &str,
    depends_on: Vec<i64>,
    cancel_on_blocking_job_failure: bool,
) -> i64 {
    let mut job = models::JobModel::new(workflow_id, name.to_string(), "true".to_string());
    job.depends_on_job_ids = Some(depends_on);
    job.cancel_on_blocking_job_failure = Some(cancel_on_blocking_job_failure);
    apis::jobs_api::create_job(config, job)
        .expect("create job")
        .id
        .unwrap()
}

fn initialize(config: &Configuration, workflow_id: i64) -> i64 {
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("initialize");
    apis::workflows_api::get_workflow(config, workflow_id)
        .expect("get workflow")
        .run_id
        .unwrap_or(1)
}

fn status(config: &Configuration, job_id: i64) -> JobStatus {
    apis::jobs_api::get_job(config, job_id)
        .expect("get job")
        .status
        .unwrap()
}

fn is_complete(config: &Configuration, workflow_id: i64) -> bool {
    apis::workflows_api::is_workflow_complete(config, workflow_id)
        .expect("is complete")
        .is_complete
}

fn response_status(
    result: Result<models::CancelJobResponse, apis::Error<apis::jobs_api::CancelJobError>>,
) -> u16 {
    match result {
        Err(apis::Error::ResponseError(r)) => r.status.as_u16(),
        other => panic!("expected an error response, got {:?}", other.map(|_| ())),
    }
}

/// Run a ready job to successful completion through the normal runner calls.
fn run_to_completion(config: &Configuration, workflow_id: i64, job_id: i64, run_id: i64) {
    let compute_node_id = create_test_compute_node(config, workflow_id).id.unwrap();
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id)
        .expect("set running");
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
    apis::jobs_api::complete_job(config, job_id, JobStatus::Completed, run_id, result)
        .expect("complete job");
}

#[rstest]
fn test_cancel_ready_job_cancels_downstream_chain(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "cancel_ready_chain")
        .id
        .unwrap();
    let a = create_job(config, workflow_id, "a", vec![], true);
    let b = create_job(config, workflow_id, "b", vec![a], true);
    let c = create_job(config, workflow_id, "c", vec![b], true);
    initialize(config, workflow_id);
    assert_eq!(status(config, a), JobStatus::Ready);
    assert!(!is_complete(config, workflow_id));

    let response = apis::jobs_api::cancel_job(config, a).expect("cancel job");
    assert_eq!(response.canceled_job_ids, vec![a, b, c]);
    for id in [a, b, c] {
        assert_eq!(status(config, id), JobStatus::Canceled);
    }
    assert!(is_complete(config, workflow_id));

    // Already terminal.
    assert_eq!(response_status(apis::jobs_api::cancel_job(config, a)), 409);
}

#[rstest]
fn test_cancel_respects_cancel_on_blocking_job_failure(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "cancel_keep_going")
        .id
        .unwrap();
    let a = create_job(config, workflow_id, "a", vec![], true);
    let b = create_job(config, workflow_id, "b", vec![a], false);
    initialize(config, workflow_id);

    let out = run_cli_command(
        &["-f", "json", "jobs", "cancel", &a.to_string()],
        start_server,
        None,
    )
    .expect("CLI cancel");
    let response: serde_json::Value = serde_json::from_str(&out).expect("JSON output");
    assert_eq!(response["canceled_job_ids"], serde_json::json!([a]));
    // Same as for a failed dependency: the job opted out of cancellation, so it is released.
    assert_eq!(status(config, b), JobStatus::Ready);
    assert!(!is_complete(config, workflow_id));
}

#[rstest]
fn test_cancel_blocked_job(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "cancel_blocked").id.unwrap();
    let a = create_job(config, workflow_id, "a", vec![], true);
    let b = create_job(config, workflow_id, "b", vec![a], true);
    let c = create_job(config, workflow_id, "c", vec![b], true);
    // d also waits on a, which is still pending, so it is canceled only once a finishes.
    let d = create_job(config, workflow_id, "d", vec![a, b], true);
    let run_id = initialize(config, workflow_id);
    assert_eq!(status(config, b), JobStatus::Blocked);

    let response = apis::jobs_api::cancel_job(config, b).expect("cancel job");
    assert_eq!(response.canceled_job_ids, vec![b, c]);
    assert_eq!(status(config, a), JobStatus::Ready);
    assert_eq!(status(config, d), JobStatus::Blocked);

    run_to_completion(config, workflow_id, a, run_id);
    // The background unblock task cancels d once a's completion is processed.
    let deadline = Instant::now() + Duration::from_secs(20);
    while status(config, d) != JobStatus::Canceled {
        assert!(Instant::now() < deadline, "d was not canceled");
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(is_complete(config, workflow_id));
}

#[rstest]
fn test_cancel_running_or_pending_job_conflicts(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "cancel_running").id.unwrap();
    let a = create_job(config, workflow_id, "a", vec![], true);
    let b = create_job(config, workflow_id, "b", vec![a], true);
    let run_id = initialize(config, workflow_id);
    apis::jobs_api::manage_status_change(config, a, JobStatus::Running, run_id)
        .expect("set running");

    match apis::jobs_api::cancel_job(config, a) {
        Err(apis::Error::ResponseError(r)) => {
            assert_eq!(r.status.as_u16(), 409);
            assert!(r.content.contains("cannot be canceled individually"));
        }
        other => panic!("expected 409, got {:?}", other.map(|_| ())),
    }
    assert_eq!(status(config, a), JobStatus::Running);
    assert_eq!(status(config, b), JobStatus::Blocked);

    let err = run_cli_command(&["jobs", "cancel", &a.to_string()], start_server, None)
        .expect_err("CLI should fail for a running job")
        .to_string();
    assert!(
        err.contains("cannot be canceled individually") && err.contains("409"),
        "{err}"
    );

    apis::jobs_api::manage_status_change(config, a, JobStatus::Pending, run_id)
        .expect("set pending");
    assert_eq!(response_status(apis::jobs_api::cancel_job(config, a)), 409);
    assert_eq!(response_status(apis::jobs_api::cancel_job(config, -1)), 404);
}

#[rstest]
fn test_cancel_job_unauthorized_user_rejected(
    start_server_with_access_control: &AccessControlServerProcess,
) {
    let server = start_server_with_access_control;
    let owner = server.config_for_user("job_owner");
    let workflow = apis::workflows_api::create_workflow(
        &owner,
        models::WorkflowModel::new("cancel_job_auth".to_string(), "job_owner".to_string()),
    )
    .expect("create workflow");
    let workflow_id = workflow.id.unwrap();
    let a = create_job(&owner, workflow_id, "a", vec![], true);
    initialize(&owner, workflow_id);

    let other = server.config_for_user("unauthorized_job_user");
    assert_eq!(response_status(apis::jobs_api::cancel_job(&other, a)), 403);
    assert_eq!(status(&owner, a), JobStatus::Ready);

    let response = apis::jobs_api::cancel_job(&owner, a).expect("owner cancels");
    assert_eq!(response.canceled_job_ids, vec![a]);
}

#[rstest]
fn test_cancel_uninitialized_job_conflicts(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "cancel_uninitialized")
        .id
        .unwrap();
    let a = create_job(config, workflow_id, "a", vec![], true);
    let b = create_job(config, workflow_id, "b", vec![a], true);

    // Canceling a would leave b uninitialized, and initialize_jobs would then make it ready.
    match apis::jobs_api::cancel_job(config, a) {
        Err(apis::Error::ResponseError(r)) => {
            assert_eq!(r.status.as_u16(), 409);
            assert!(
                r.content.contains("initialize the workflow first"),
                "{}",
                r.content
            );
        }
        other => panic!("expected 409, got {:?}", other.map(|_| ())),
    }
    assert_eq!(status(config, a), JobStatus::Uninitialized);

    initialize(config, workflow_id);
    let response = apis::jobs_api::cancel_job(config, a).expect("cancel job");
    assert_eq!(response.canceled_job_ids, vec![a, b]);
}

#[rstest]
fn test_cancel_response_excludes_unrelated_cascade(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_test_workflow(config, "cancel_unrelated").id.unwrap();
    let x = create_job(config, workflow_id, "x", vec![], true);
    let y = create_job(config, workflow_id, "y", vec![x], true);
    let b = create_job(config, workflow_id, "b", vec![], true);
    initialize(config, workflow_id);
    // Leave y's cancellation pending: x is terminated but marked processed, so the background
    // task never cascades it. No API reaches this state deterministically (the background task
    // races any completion), so write it directly. The cascade run by canceling b picks it up.
    let db = rusqlite::Connection::open(start_server.db_file.path()).expect("open db");
    db.busy_timeout(Duration::from_secs(10)).unwrap();
    db.execute(
        "UPDATE job SET status = ?1, unblocking_processed = 1 WHERE id = ?2",
        (JobStatus::Terminated.to_int(), x),
    )
    .expect("terminate x");
    assert_eq!(status(config, y), JobStatus::Blocked);

    let response = apis::jobs_api::cancel_job(config, b).expect("cancel job");
    assert_eq!(response.canceled_job_ids, vec![b]);
    assert_eq!(status(config, y), JobStatus::Canceled);
}
