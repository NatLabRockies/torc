mod common;

use common::{ServerProcess, create_test_compute_node, create_test_workflow, start_server};
use rstest::rstest;
use std::time::{Duration, Instant};
use torc::client::{apis, workflow_manager::WorkflowManager};
use torc::config::TorcConfig;
use torc::models;
use torc::models::JobStatus;

fn complete(config: &apis::configuration::Configuration, job_id: i64, workflow_id: i64, cn: i64) {
    let result = models::ResultModel::new(
        job_id,
        workflow_id,
        1,
        1,
        cn,
        0,
        0.1,
        chrono::Utc::now().to_rfc3339(),
        JobStatus::Completed,
    );
    apis::jobs_api::complete_job(config, job_id, result.status, 1, result)
        .expect("Failed to complete job");
}

/// Waits for `job_id` to become ready and returns how long it took.
fn wait_until_ready(config: &apis::configuration::Configuration, job_id: i64) -> Duration {
    let start = Instant::now();
    loop {
        let job = apis::jobs_api::get_job(config, job_id).expect("Failed to get job");
        if job.status == Some(JobStatus::Ready) {
            return start.elapsed();
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "job_id={job_id} was not released within 10s (status={:?})",
            job.status
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A completion must release its dependents promptly, without waiting for a periodic timer.
/// The second completion lands inside the server's minimum gap between unblock passes, so it
/// also checks that a completion signaled during that gap is not lost.
#[rstest]
fn test_dependents_released_on_completion(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "test_unblock_on_demand");
    let workflow_id = workflow.id.unwrap();

    let mut ids = Vec::new();
    for i in 0..3 {
        let mut job = models::JobModel::new(workflow_id, format!("job{i}"), "true".to_string());
        if let Some(&prev) = ids.last() {
            job.depends_on_job_ids = Some(vec![prev]);
        }
        ids.push(apis::jobs_api::create_job(config, job).unwrap().id.unwrap());
    }

    let manager = WorkflowManager::new(
        config.clone(),
        TorcConfig::load().unwrap_or_default(),
        workflow,
    );
    manager
        .initialize(true)
        .expect("Failed to initialize workflow");
    let cn = create_test_compute_node(config, workflow_id).id.unwrap();

    complete(config, ids[0], workflow_id, cn);
    let first = wait_until_ready(config, ids[1]);
    complete(config, ids[1], workflow_id, cn);
    let second = wait_until_ready(config, ids[2]);

    // Old default timer was 30 s; on-demand processing is bounded by the 1 s minimum gap.
    assert!(
        first < Duration::from_secs(3),
        "first release took {first:?}"
    );
    assert!(
        second < Duration::from_secs(3),
        "second release took {second:?}"
    );
}
