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
use torc::client::apis::default_api::GetTaskError;
use torc::client::default_api;
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

    let resp = default_api::initialize_jobs_with_async(
        &server.config,
        workflow_id,
        Some(false),
        Some(false),
        Some(true),
        None,
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
        let current = default_api::get_task(&server.config, task.id).expect("get_task should work");
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
fn test_initialize_jobs_async_concurrent_requests_yield_conflict(start_server: &ServerProcess) {
    let server = start_server;
    let workflow = common::create_test_workflow(&server.config, "tasks-test-conflict-workflow");
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
            let result = default_api::initialize_jobs_with_async(
                &config,
                workflow_id,
                Some(false),
                Some(false),
                Some(true),
                None,
            );
            tx.send(result).ok();
        });
    }

    barrier.wait();

    let mut ok_count = 0;
    let mut conflict_count = 0;
    for _ in 0..2 {
        let result = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("thread result");
        match result {
            Ok(value) => {
                let _task: TaskModel =
                    serde_json::from_value(value).expect("initialize_jobs_with_async TaskModel");
                ok_count += 1;
            }
            Err(ApiError::ResponseError(resp)) => {
                if resp.status.as_u16() == 409 {
                    conflict_count += 1;
                } else {
                    panic!("Expected 409 conflict, got {}", resp.status);
                }
            }
            Err(err) => panic!("Unexpected error from initialize_jobs_with_async: {}", err),
        }
    }

    assert_eq!(ok_count, 1, "expected one successful task creation");
    assert_eq!(conflict_count, 1, "expected one conflict response");
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

    let resp = default_api::initialize_jobs_with_async(
        &owner_config,
        workflow_id,
        Some(false),
        Some(false),
        Some(true),
        None,
    )
    .expect("initialize_jobs_with_async should return 202 task");

    let task: TaskModel = serde_json::from_value(resp).expect("TaskModel response");

    match default_api::get_task(&outsider_config, task.id) {
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
