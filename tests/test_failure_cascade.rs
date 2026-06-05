mod common;

use common::{
    ServerProcess, create_test_compute_node, create_test_resource_requirements,
    create_test_workflow, start_server,
};
use rstest::rstest;
use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant};
use torc::client::apis;
use torc::models::{self, JobStatus};

/// Fetch the current status of every job in the workflow, keyed by job name.
fn job_statuses(
    config: &torc::client::Configuration,
    workflow_id: i64,
) -> HashMap<String, JobStatus> {
    let jobs = apis::jobs_api::list_jobs(
        config,
        workflow_id,
        None, // status
        None, // offset
        None, // limit
        None, // sort_by
        None, // reverse_sort
        None, // job_ids
        None, // resource_requirements_id
        None, // include_relationships
        None, // active_compute_node_id
        None, // origin_is_set
        None, // name
        None, // command
    )
    .expect("Failed to list jobs");

    jobs.items
        .into_iter()
        .map(|j| (j.name.clone(), j.status.unwrap()))
        .collect()
}

/// Poll until every named job reaches a terminal status (or the timeout elapses).
/// The cascade runs in the server's background unblock task, so we cannot assert
/// synchronously right after completing the failing job.
fn wait_for_terminal(
    config: &torc::client::Configuration,
    workflow_id: i64,
    names: &[&str],
) -> HashMap<String, JobStatus> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let statuses = job_statuses(config, workflow_id);
        let all_terminal = names.iter().all(|name| {
            statuses
                .get(*name)
                .map(|s| s.is_complete())
                .unwrap_or(false)
        });
        if all_terminal || Instant::now() >= deadline {
            return statuses;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Create a linear dependency chain A -> B -> C -> D using file dependencies.
/// Each job consumes the file produced by the previous one, so B depends on A,
/// C depends on B, and D depends on C.
fn create_chain_workflow(
    config: &torc::client::Configuration,
    name: &str,
) -> (i64, HashMap<String, i64>) {
    let workflow = create_test_workflow(config, name);
    let workflow_id = workflow.id.unwrap();

    let resource_req =
        create_test_resource_requirements(config, workflow_id, "chain", 1, 0, 1, "1g", "P0DT1H");
    let rr_id = resource_req.id.unwrap();

    // Files linking each stage of the chain.
    let file_names = ["f_ab", "f_bc", "f_cd"];
    let mut file_ids = Vec::new();
    for fname in file_names {
        let file = apis::files_api::create_file(
            config,
            models::FileModel::new(workflow_id, fname.to_string(), format!("{fname}.txt")),
        )
        .expect("Failed to create file");
        file_ids.push(file.id.unwrap());
    }

    // job_a -> f_ab; job_b consumes f_ab -> f_bc; job_c consumes f_bc -> f_cd;
    // job_d consumes f_cd.
    let specs: [(&str, Option<i64>, Option<i64>); 4] = [
        ("job_a", None, Some(file_ids[0])),
        ("job_b", Some(file_ids[0]), Some(file_ids[1])),
        ("job_c", Some(file_ids[1]), Some(file_ids[2])),
        ("job_d", Some(file_ids[2]), None),
    ];

    let mut job_ids = HashMap::new();
    for (job_name, input, output) in specs {
        let mut job = models::JobModel::new(
            workflow_id,
            job_name.to_string(),
            format!("echo '{job_name}'"),
        );
        job.resource_requirements_id = Some(rr_id);
        job.input_file_ids = input.map(|id| vec![id]);
        job.output_file_ids = output.map(|id| vec![id]);
        let created = apis::jobs_api::create_job(config, job).expect("Failed to create job");
        job_ids.insert(job_name.to_string(), created.id.unwrap());
    }

    apis::workflows_api::initialize_jobs(config, workflow_id, Some(false), Some(false), None)
        .expect("Failed to initialize jobs");

    (workflow_id, job_ids)
}

/// Regression test for transitive failure cancellation.
///
/// In a chain A -> B -> C -> D, when A fails, the cancellation must propagate all
/// the way down the chain: B, C, and D should all be canceled. Previously only B
/// (the direct dependent of the failed job) was canceled, while C and D became
/// ready and ran, because the cascade query required a failed dependency to have
/// a `result` row with a non-zero return code -- and canceled jobs have no result
/// row.
#[rstest]
fn test_failure_cancellation_is_transitive(start_server: &ServerProcess) {
    let config = &start_server.config;

    let (workflow_id, job_ids) = create_chain_workflow(config, "test_cascade_chain");

    let workflow =
        apis::workflows_api::get_workflow(config, workflow_id).expect("Failed to get workflow");
    let run_id = workflow.run_id.unwrap_or(0);

    let compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    // Only job_a is ready initially; run it and fail it.
    let job_a_id = job_ids["job_a"];
    apis::jobs_api::manage_status_change(config, job_a_id, JobStatus::Running, run_id)
        .expect("Failed to set job_a running");

    let result = models::ResultModel::new(
        job_a_id,
        workflow_id,
        run_id,
        1, // attempt_id
        compute_node_id,
        1,   // return_code (failure)
        0.1, // exec_time_minutes
        "2020-01-01T00:00:00Z".to_string(),
        JobStatus::Failed,
    );
    apis::jobs_api::complete_job(config, job_a_id, JobStatus::Failed, run_id, result)
        .expect("Failed to complete job_a as failed");

    let statuses = wait_for_terminal(config, workflow_id, &["job_b", "job_c", "job_d"]);

    assert_eq!(
        statuses.get("job_a"),
        Some(&JobStatus::Failed),
        "job_a should be failed"
    );
    assert_eq!(
        statuses.get("job_b"),
        Some(&JobStatus::Canceled),
        "job_b (direct dependent of failed job_a) should be canceled"
    );
    assert_eq!(
        statuses.get("job_c"),
        Some(&JobStatus::Canceled),
        "job_c should be canceled transitively (depends on canceled job_b)"
    );
    assert_eq!(
        statuses.get("job_d"),
        Some(&JobStatus::Canceled),
        "job_d should be canceled transitively (depends on canceled job_c)"
    );

    // No result rows should exist for the canceled jobs -- only job_a produced a result.
    let results = apis::results_api::list_results(
        config,
        workflow_id,
        None, // job_id
        None, // run_id
        None, // return_code
        None, // status
        None, // compute_node_id
        None, // offset
        None, // limit
        None, // sort_by
        None, // reverse_sort
        None, // all_runs
    )
    .expect("Failed to list results");
    assert_eq!(
        results.items.len(),
        1,
        "only job_a should have a result row; canceled jobs must not run"
    );
}
