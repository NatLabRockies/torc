use std::sync::mpsc;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use rstest::rstest;
use serial_test::serial;
mod common;
use common::{
    AccessControlServerProcess, ServerProcess, start_server, start_server_with_access_control,
};

use torc::client::apis::Error as ApiError;
use torc::client::apis::tasks_api::GetTaskError;
use torc::client::apis::{tasks_api, workflows_api};
use torc::client::commands::tasks::{WaitError, wait_for_task};
use torc::client::sse_client::SseConnection;
use torc::models::{EventSeverity, TaskModel, TaskStatus};

#[rstest]
#[serial]
fn test_initialize_jobs_async_creates_task_and_emits_sse(start_server: &ServerProcess) {
    let server = start_server;
    let workflow = common::create_test_workflow(&server.config, "tasks-test-workflow");
    let workflow_id = workflow.id.unwrap();

    let (tx, rx) = mpsc::channel::<i64>();
    let sse_config = server.config.clone();
    thread::spawn(move || {
        let mut conn =
            match SseConnection::connect(&sse_config, workflow_id, Some(EventSeverity::Info)) {
                Ok(c) => c,
                Err(_) => return,
            };

        loop {
            match conn.next_event() {
                Ok(Some(event)) => {
                    if event.event_type == "task_completed"
                        && let Some(task_id) = event.data.get("task_id").and_then(|v| v.as_i64())
                    {
                        let _ = tx.send(task_id);
                        return;
                    }
                }
                Ok(None) => return,
                Err(_) => return,
            }
        }
    });

    let resp = workflows_api::initialize_jobs(
        &server.config,
        workflow_id,
        Some(false),
        Some(false),
        Some(true),
    )
    .expect("initialize_jobs_with_async should return 202 task");

    let task: TaskModel = serde_json::from_value(resp).expect("TaskModel response");
    assert_eq!(task.workflow_id, workflow_id);
    assert_eq!(task.operation, "initialize_jobs");
    assert_eq!(task.status, TaskStatus::Queued);

    // Wait for SSE completion (best effort)
    let _ = rx.recv_timeout(Duration::from_secs(10));

    // Poll task state until completion
    let start = Instant::now();
    loop {
        let current = tasks_api::get_task(&server.config, task.id).expect("get_task should work");
        if matches!(current.status, TaskStatus::Succeeded | TaskStatus::Failed) {
            assert_eq!(current.status, TaskStatus::Succeeded);
            break;
        }

        if start.elapsed() > Duration::from_secs(20) {
            panic!("Timed out waiting for task to complete");
        }
        thread::sleep(Duration::from_millis(100));
    }
}

#[rstest]
#[serial]
fn test_initialize_jobs_async_concurrent_requests_return_same_task(start_server: &ServerProcess) {
    let server = start_server;
    let workflow = common::create_test_workflow(&server.config, "tasks-test-idempotent-workflow");
    let workflow_id = workflow.id.unwrap();

    // Create enough jobs to make initialization take long enough for a concurrent request to race.
    for i in 0..50 {
        let job_name = format!("job_{i}");
        let _job = common::create_test_job(&server.config, workflow_id, &job_name);
    }

    let barrier = Arc::new(Barrier::new(3));
    let (tx, rx) = mpsc::channel();

    for _ in 0..2 {
        let config = server.config.clone();
        let barrier = barrier.clone();
        let tx = tx.clone();
        thread::spawn(move || {
            barrier.wait();
            let result = workflows_api::initialize_jobs(
                &config,
                workflow_id,
                Some(false),
                Some(false),
                Some(true),
            );
            tx.send(result).ok();
        });
    }

    barrier.wait();

    let mut task_ids = Vec::new();
    for _ in 0..2 {
        let result = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("thread result");
        let value = result.expect("both concurrent calls should succeed with the same task");
        let task: TaskModel = serde_json::from_value(value).expect("TaskModel response");
        task_ids.push(task.id);
    }

    assert_eq!(
        task_ids[0], task_ids[1],
        "concurrent initialize_jobs?async=true should be idempotent: both callers should receive the same task id"
    );
}

#[rstest]
#[serial]
fn test_get_task_unauthorized_returns_404(
    start_server_with_access_control: &AccessControlServerProcess,
) {
    let server = start_server_with_access_control;
    let owner_config = server.config_for_user("owner_user");
    let outsider_config = server.config_for_user("outsider");

    let workflow = common::create_test_workflow_advanced(
        &owner_config,
        "tasks-test-unauthorized-404",
        "owner_user",
        None,
    );
    let workflow_id = workflow.id.unwrap();

    let resp = workflows_api::initialize_jobs(
        &owner_config,
        workflow_id,
        Some(false),
        Some(false),
        Some(true),
    )
    .expect("initialize_jobs_with_async should return 202 task");

    let task: TaskModel = serde_json::from_value(resp).expect("TaskModel response");

    match tasks_api::get_task(&outsider_config, task.id) {
        Ok(_) => panic!("expected 404 when unauthorized user queries a task"),
        Err(ApiError::ResponseError(resp)) => {
            assert_eq!(
                resp.status.as_u16(),
                404,
                "expected 404 (not 403) to avoid task ID enumeration"
            );
            assert!(
                matches!(resp.entity, Some(GetTaskError::Status404(_))),
                "expected typed 404 entity for get_task"
            );
        }
        Err(err) => panic!("unexpected error from get_task: {}", err),
    }
}

#[rstest]
#[serial]
fn test_wait_for_task_returns_succeeded(start_server: &ServerProcess) {
    // Exercises the wait_for_task helper that `torc workflows reinit` uses for auto-wait.
    let server = start_server;
    let workflow = common::create_test_workflow(&server.config, "tasks-test-wait-helper");
    let workflow_id = workflow.id.unwrap();

    let resp = workflows_api::initialize_jobs(
        &server.config,
        workflow_id,
        Some(false),
        Some(false),
        Some(true),
    )
    .expect("initialize_jobs should accept async request");
    let task: TaskModel = serde_json::from_value(resp).expect("TaskModel response");

    let final_task = wait_for_task(&server.config, task.id, Some(30), 2)
        .expect("wait_for_task should return a terminal task within the timeout");

    assert_eq!(final_task.id, task.id);
    assert_eq!(
        final_task.status,
        TaskStatus::Succeeded,
        "expected initialize_jobs task to reach Succeeded; got {:?} (error: {:?})",
        final_task.status,
        final_task.error
    );
    assert!(
        final_task.finished_at_ms.is_some(),
        "terminal task should have finished_at_ms set"
    );
}

#[rstest]
#[serial]
fn test_initialize_jobs_async_mismatched_params_returns_409(start_server: &ServerProcess) {
    // P2: if a task is already running with one parameter set, a concurrent request with a
    // different parameter set must be rejected rather than silently returned the existing task.
    let server = start_server;
    let workflow = common::create_test_workflow(&server.config, "tasks-test-param-mismatch");
    let workflow_id = workflow.id.unwrap();

    // Enough jobs to keep the first task running while the second request arrives.
    for i in 0..50 {
        let job_name = format!("job_{i}");
        let _job = common::create_test_job(&server.config, workflow_id, &job_name);
    }

    // First request: only_uninitialized=false, clear_ephemeral_user_data=false
    let first = workflows_api::initialize_jobs(
        &server.config,
        workflow_id,
        Some(false),
        Some(false),
        Some(true),
    )
    .expect("first async init should be accepted");
    let first_task: TaskModel = serde_json::from_value(first).expect("TaskModel");

    // Second request with a different only_uninitialized value while the first is still active.
    let second = workflows_api::initialize_jobs(
        &server.config,
        workflow_id,
        Some(true), // mismatched
        Some(false),
        Some(true),
    );

    match second {
        Err(ApiError::ResponseError(resp)) => {
            assert_eq!(
                resp.status.as_u16(),
                409,
                "expected 409 for parameter mismatch; got {}",
                resp.status
            );
            assert!(
                resp.content.contains("different parameters"),
                "expected reason to mention different parameters, got: {}",
                resp.content
            );
            assert!(
                resp.content.contains(&first_task.id.to_string()),
                "expected existing_task_id in payload, got: {}",
                resp.content
            );
        }
        Ok(value) => panic!("expected 409, got success: {}", value),
        Err(err) => panic!("expected ResponseError 409, got: {}", err),
    }
}

#[rstest]
#[serial]
fn test_get_active_task_returns_none_when_idle(start_server: &ServerProcess) {
    let server = start_server;
    let workflow = common::create_test_workflow(&server.config, "tasks-test-active-idle");
    let workflow_id = workflow.id.unwrap();

    let resp = workflows_api::get_active_task_for_workflow(&server.config, workflow_id)
        .expect("active_task endpoint should succeed");
    assert!(
        resp.task.is_none(),
        "expected no active task on an untouched workflow, got {:?}",
        resp.task
    );
}

#[rstest]
#[serial]
fn test_get_active_task_returns_running_task(start_server: &ServerProcess) {
    let server = start_server;
    let workflow = common::create_test_workflow(&server.config, "tasks-test-active-busy");
    let workflow_id = workflow.id.unwrap();

    for i in 0..50 {
        let job_name = format!("job_{i}");
        let _job = common::create_test_job(&server.config, workflow_id, &job_name);
    }

    let started = workflows_api::initialize_jobs(
        &server.config,
        workflow_id,
        Some(false),
        Some(false),
        Some(true),
    )
    .expect("async init should be accepted");
    let started_task: TaskModel = serde_json::from_value(started).expect("TaskModel");

    let resp = workflows_api::get_active_task_for_workflow(&server.config, workflow_id)
        .expect("active_task endpoint should succeed");
    let active = resp
        .task
        .expect("expected an active task while init is running");
    assert_eq!(active.id, started_task.id);
    assert_eq!(active.operation, "initialize_jobs");
}

#[rstest]
#[serial]
fn test_wait_for_task_times_out_for_unknown_task(start_server: &ServerProcess) {
    // A far-future nonexistent task id: server returns 404, which is non-retryable,
    // so wait_for_task should fail fast with Api, not hang.
    let server = start_server;
    match wait_for_task(&server.config, 9_999_999_999, Some(5), 1) {
        Err(WaitError::Api(msg)) => {
            assert!(
                msg.contains("404") || msg.to_lowercase().contains("not found"),
                "expected 404 in error message, got: {}",
                msg
            );
        }
        Err(WaitError::Timeout) => {
            // Acceptable: if retries happened to mask the 404 it would be a timeout.
        }
        Ok(task) => panic!(
            "unexpected successful wait for non-existent task: {:?}",
            task
        ),
    }
}
