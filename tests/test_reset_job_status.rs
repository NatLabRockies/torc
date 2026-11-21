//! Selective `reset_job_status` (request body) and server-enforced `retry_job` limits.

mod common;

use common::{ServerProcess, create_test_job, create_test_workflow, start_server};
use rstest::rstest;
use torc::client::Configuration;
use torc::client::apis;
use torc::models::{self, JobStatus};

fn run_id(config: &Configuration, workflow_id: i64) -> i64 {
    apis::workflows_api::get_workflow(config, workflow_id)
        .unwrap()
        .run_id
        .unwrap()
}

fn status(config: &Configuration, job_id: i64) -> JobStatus {
    apis::jobs_api::get_job(config, job_id)
        .unwrap()
        .status
        .unwrap()
}

fn initialize(config: &Configuration, workflow_id: i64) {
    apis::workflows_api::initialize_jobs(config, workflow_id, Some(false), Some(false), None)
        .unwrap();
}

/// Move a ready job to failed through the normal start/complete path.
fn fail_job(config: &Configuration, workflow_id: i64, job_id: i64) {
    let run_id = run_id(config, workflow_id);
    apis::jobs_api::manage_status_change(config, job_id, JobStatus::Running, run_id).unwrap();
    let attempt_id = apis::jobs_api::get_job(config, job_id)
        .unwrap()
        .attempt_id
        .unwrap_or(1);
    let result = models::ResultModel::new(
        job_id,
        workflow_id,
        run_id,
        attempt_id,
        1, // default compute node from create_test_workflow
        1,
        0.1,
        "2026-01-01T00:00:00Z".to_string(),
        JobStatus::Failed,
    );
    apis::jobs_api::complete_job(config, job_id, JobStatus::Failed, run_id, result).unwrap();
}

/// Selective reset; on an error response, returns its HTTP status code.
fn reset(
    config: &Configuration,
    workflow_id: i64,
    job_ids: Option<Vec<i64>>,
    dry_run: bool,
    force: bool,
) -> Result<models::ResetJobStatusResponse, u16> {
    let body = models::ResetJobStatusRequest {
        job_ids,
        statuses: None,
        dry_run: Some(dry_run),
    };
    apis::workflows_api::reset_job_status(config, workflow_id, None, Some(force), Some(body))
        .map_err(|e| status_code(&e))
}

fn status_code<T>(err: &apis::Error<T>) -> u16 {
    match err {
        apis::Error::ResponseError(r) => r.status.as_u16(),
        other => panic!("unexpected error: {}", other),
    }
}

#[rstest]
fn test_reset_skips_other_workflow_and_ineligible_jobs(start_server: &ServerProcess) {
    let config = &start_server.config;
    let wf = create_test_workflow(config, "reset_cross_wf_a").id.unwrap();
    let failed = create_test_job(config, wf, "failed").id.unwrap();
    let ready = create_test_job(config, wf, "ready").id.unwrap();
    initialize(config, wf);
    fail_job(config, wf, failed);

    let other_wf = create_test_workflow(config, "reset_cross_wf_b").id.unwrap();
    let foreign = create_test_job(config, other_wf, "foreign").id.unwrap();
    initialize(config, other_wf);
    fail_job(config, other_wf, foreign);

    let run_id_before = run_id(config, wf);
    let response = reset(config, wf, Some(vec![foreign, ready, failed]), false, false).unwrap();

    assert_eq!(response.reset, Some(vec![failed]));
    assert_eq!(response.updated_count, 1);
    let skipped = response.skipped.unwrap();
    let skipped_ids: Vec<i64> = skipped.iter().map(|s| s.job_id).collect();
    assert_eq!(skipped_ids, vec![ready, foreign]);
    assert_eq!(status(config, failed), JobStatus::Uninitialized);
    assert_eq!(status(config, ready), JobStatus::Ready);
    assert_eq!(status(config, foreign), JobStatus::Failed);
    // The reset leaves run_id to the following reinitialize.
    assert_eq!(run_id(config, wf), run_id_before);
}

#[rstest]
fn test_reset_dry_run_and_default_selection(start_server: &ServerProcess) {
    let config = &start_server.config;
    let wf = create_test_workflow(config, "reset_dry_run").id.unwrap();
    let failed = create_test_job(config, wf, "failed").id.unwrap();
    create_test_job(config, wf, "ready");
    initialize(config, wf);
    fail_job(config, wf, failed);

    let preview = reset(config, wf, None, true, false).unwrap();
    assert_eq!(preview.reset, Some(vec![failed]));
    assert_eq!(preview.updated_count, 0);
    // Without job_ids, jobs in other statuses are not reported as skipped.
    assert_eq!(preview.skipped, Some(vec![]));
    assert_eq!(status(config, failed), JobStatus::Failed);
}

#[rstest]
fn test_reset_refuses_busy_workflow_unless_forced(start_server: &ServerProcess) {
    let config = &start_server.config;
    let wf = create_test_workflow(config, "reset_busy").id.unwrap();
    let failed = create_test_job(config, wf, "failed").id.unwrap();
    let running = create_test_job(config, wf, "running").id.unwrap();
    initialize(config, wf);
    fail_job(config, wf, failed);

    // An active compute node makes the workflow busy.
    let mut node = common::create_test_compute_node(config, wf);
    node.is_active = Some(true);
    let node_id = node.id.unwrap();
    apis::compute_nodes_api::update_compute_node(config, node_id, node.clone()).unwrap();
    assert_eq!(
        reset(config, wf, Some(vec![failed]), false, false),
        Err(422)
    );
    assert_eq!(status(config, failed), JobStatus::Failed);

    // So does a running job.
    node.is_active = Some(false);
    apis::compute_nodes_api::update_compute_node(config, node_id, node).unwrap();
    apis::jobs_api::manage_status_change(config, running, JobStatus::Running, run_id(config, wf))
        .unwrap();
    assert_eq!(reset(config, wf, Some(vec![failed]), true, false), Err(422));

    let response = reset(config, wf, Some(vec![failed]), false, true).unwrap();
    assert_eq!(response.reset, Some(vec![failed]));
    assert_eq!(status(config, failed), JobStatus::Uninitialized);
    assert_eq!(status(config, running), JobStatus::Running);
}

/// A claim racing a reset must serialize with it: either the claim lands first and the reset
/// is refused (workflow busy), or the reset lands first and succeeds. Never a server error
/// (e.g. SQLITE_BUSY from a lock upgrade). Which order won is not observable here, so this
/// checks for consistent outcomes, not atomicity itself; that comes from the reset running
/// its check and writes in one BEGIN IMMEDIATE transaction.
#[rstest]
fn test_reset_concurrent_with_claim(start_server: &ServerProcess) {
    let config = &start_server.config;
    for i in 0..10 {
        let wf = create_test_workflow(config, &format!("reset_race_{}", i))
            .id
            .unwrap();
        let failed = create_test_job(config, wf, "failed").id.unwrap();
        let ready = create_test_job(config, wf, "ready").id.unwrap();
        initialize(config, wf);
        fail_job(config, wf, failed);

        let (reset_result, claimed) = std::thread::scope(|s| {
            let claim = s.spawn(|| {
                apis::workflows_api::claim_next_jobs(config, wf, Some(1), None, None, None)
                    .unwrap()
                    .jobs
                    .unwrap_or_default()
            });
            let reset_result = reset(config, wf, Some(vec![failed]), false, false);
            (reset_result, claim.join().unwrap())
        });
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].id, Some(ready));
        match reset_result {
            Ok(response) => {
                assert_eq!(response.reset, Some(vec![failed]));
                assert_eq!(status(config, failed), JobStatus::Uninitialized);
            }
            Err(code) => {
                assert_eq!(code, 422);
                assert_eq!(status(config, failed), JobStatus::Failed);
            }
        }
    }
}

#[rstest]
fn test_retry_enforces_failure_handler_limit(start_server: &ServerProcess) {
    let config = &start_server.config;
    let wf = create_test_workflow(config, "retry_limit").id.unwrap();
    let handler = apis::failure_handlers_api::create_failure_handler(
        config,
        models::FailureHandlerModel::new(
            wf,
            "fh".to_string(),
            r#"[{"exit_codes":[1],"max_retries":2}]"#.to_string(),
        ),
    )
    .unwrap();
    let mut job = models::JobModel::new(wf, "job".to_string(), "false".to_string());
    job.failure_handler_id = handler.id;
    let job_id = apis::jobs_api::create_job(config, job).unwrap().id.unwrap();
    initialize(config, wf);
    let run_id = run_id(config, wf);

    fail_job(config, wf, job_id);
    let retried = apis::jobs_api::retry_job(config, job_id, run_id, None).unwrap();
    assert_eq!(retried.attempt_id, Some(2));

    // A client-supplied max_retries is ignored. attempt_id must stay below the handler's
    // max_retries (the same comparison the job runner makes), so max_retries=2 allows one retry.
    fail_job(config, wf, job_id);
    let err = apis::jobs_api::retry_job(config, job_id, run_id, Some(100)).unwrap_err();
    assert_eq!(status_code(&err), 422);
    assert_eq!(status(config, job_id), JobStatus::Failed);
}

#[rstest]
fn test_retry_rule_without_max_retries_defaults_to_three(start_server: &ServerProcess) {
    let config = &start_server.config;
    let wf = create_test_workflow(config, "retry_default_limit")
        .id
        .unwrap();
    let handler = apis::failure_handlers_api::create_failure_handler(
        config,
        models::FailureHandlerModel::new(
            wf,
            "fh".to_string(),
            r#"[{"exit_codes":[1]}]"#.to_string(),
        ),
    )
    .unwrap();
    let mut job = models::JobModel::new(wf, "job".to_string(), "false".to_string());
    job.failure_handler_id = handler.id;
    let job_id = apis::jobs_api::create_job(config, job).unwrap().id.unwrap();
    initialize(config, wf);
    let run_id = run_id(config, wf);

    // Same as the job runner's default: attempts 1 and 2 may retry, attempt 3 may not.
    for expected_attempt in [2, 3] {
        fail_job(config, wf, job_id);
        let retried = apis::jobs_api::retry_job(config, job_id, run_id, None).unwrap();
        assert_eq!(retried.attempt_id, Some(expected_attempt));
    }
    fail_job(config, wf, job_id);
    let err = apis::jobs_api::retry_job(config, job_id, run_id, None).unwrap_err();
    assert_eq!(status_code(&err), 422);
}

/// Requests without a usable body keep the legacy whole-workflow behavior, however the client
/// spells "no body": no body at all, an empty body with a JSON content type (the generated
/// Python client), or `null` (the generated Rust client).
#[rstest]
fn test_reset_without_body_keeps_legacy_behavior(start_server: &ServerProcess) {
    let config = &start_server.config;
    let requests: [(Option<&str>, &str); 3] = [
        (None, ""),
        (Some("application/json"), ""),
        (Some("application/json"), "null"),
    ];
    for (i, (content_type, body)) in requests.into_iter().enumerate() {
        let wf = create_test_workflow(config, &format!("reset_no_body_{}", i))
            .id
            .unwrap();
        let failed = create_test_job(config, wf, "failed").id.unwrap();
        initialize(config, wf);
        fail_job(config, wf, failed);
        let url = format!(
            "{}/workflows/{}/reset_job_status?failed_only=true",
            config.base_path, wf
        );
        let mut req = config.client.post(&url).body(body.to_string());
        if let Some(ct) = content_type {
            req = req.header("Content-Type", ct);
        }
        let resp = config.apply_auth(req).send().unwrap();
        assert_eq!(
            resp.status().as_u16(),
            200,
            "content_type={content_type:?} body={body:?}"
        );
        let response: models::ResetJobStatusResponse = resp.json().unwrap();
        assert_eq!(response.updated_count, 1);
        assert_eq!(response.reset, None);
        assert_eq!(status(config, failed), JobStatus::Uninitialized);
    }
}
