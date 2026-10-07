mod common;

use common::{
    ServerProcess, create_test_resource_requirements, run_jobs_cli_command, start_server,
};
use rstest::rstest;
use std::collections::HashSet;
use torc::client::Configuration;
use torc::client::apis;
use torc::models;

/// Create a workflow with two unlabeled jobs and two jobs labeled `windows`.
/// Returns the workflow ID.
fn create_labeled_workflow(config: &Configuration, name: &str) -> i64 {
    let workflow = models::WorkflowModel::new(name.to_string(), "test_user".to_string());
    let workflow_id = apis::workflows_api::create_workflow(config, workflow)
        .expect("Failed to create workflow")
        .id
        .unwrap();
    let rr = create_test_resource_requirements(config, workflow_id, "small", 1, 0, 1, "1g", "PT1M");

    for (job_name, label) in [
        ("plain_1", None),
        ("plain_2", None),
        ("windows_1", Some("windows")),
        ("windows_2", Some("windows")),
    ] {
        let mut job = models::JobModel::new(
            workflow_id,
            job_name.to_string(),
            format!("echo {job_name}"),
        );
        job.resource_requirements_id = rr.id;
        job.label = label.map(str::to_string);
        let created = apis::jobs_api::create_job(config, job).expect("Failed to create job");
        assert_eq!(created.label.as_deref(), label);
    }

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");
    workflow_id
}

fn names(jobs: Option<Vec<models::JobModel>>) -> HashSet<String> {
    jobs.unwrap_or_default()
        .into_iter()
        .map(|job| job.name)
        .collect()
}

fn set(items: &[&str]) -> HashSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[rstest]
fn test_claim_next_jobs_filters_by_label(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_labeled_workflow(config, "label_claim_next_jobs");
    let claim = |label: Option<&str>| {
        let response = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(10), label)
            .expect("claim_next_jobs should succeed");
        names(response.jobs)
    };

    // A label that no job has claims nothing, even though jobs are ready.
    assert!(claim(Some("gpu")).is_empty());
    // A labeled runner claims only jobs with its label.
    assert_eq!(claim(Some("windows")), set(&["windows_1", "windows_2"]));
    // An unlabeled runner claims only unlabeled jobs.
    assert_eq!(claim(None), set(&["plain_1", "plain_2"]));
}

#[rstest]
fn test_claim_jobs_based_on_resources_filters_by_label(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_labeled_workflow(config, "label_claim_resources");
    let claim = |label: Option<&str>| {
        let mut resources = models::ComputeNodesResources::new(8, 16.0, 0, 1);
        resources.label = label.map(str::to_string);
        let response = apis::workflows_api::claim_jobs_based_on_resources(
            config,
            workflow_id,
            10,
            resources,
            None,
        )
        .expect("claim_jobs_based_on_resources should succeed");
        names(response.jobs)
    };

    assert!(claim(Some("gpu")).is_empty());
    assert_eq!(claim(Some("windows")), set(&["windows_1", "windows_2"]));
    assert_eq!(claim(None), set(&["plain_1", "plain_2"]));
}

#[rstest]
fn test_update_job_sets_label(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_labeled_workflow(config, "label_update");
    let response = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(1), None)
        .expect("claim_next_jobs should succeed");
    let mut job = response.jobs.unwrap().remove(0);
    assert_eq!(job.label, None);

    let job_id = job.id.unwrap();
    job.label = Some("windows".to_string());
    apis::jobs_api::update_job(config, job_id, job).expect("Failed to update job");
    let updated = apis::jobs_api::get_job(config, job_id).expect("Failed to get job");
    assert_eq!(updated.label.as_deref(), Some("windows"));
}

#[rstest]
fn test_update_job_clears_label(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_labeled_workflow(config, "label_clear");
    let response =
        apis::workflows_api::claim_next_jobs(config, workflow_id, Some(1), Some("windows"))
            .expect("claim_next_jobs should succeed");
    let mut job = response.jobs.unwrap().remove(0);
    let job_id = job.id.unwrap();

    // Sending the job back unchanged keeps its label; an empty label clears it.
    apis::jobs_api::update_job(config, job_id, job.clone()).expect("Failed to update job");
    let unchanged = apis::jobs_api::get_job(config, job_id).expect("Failed to get job");
    assert_eq!(unchanged.label.as_deref(), Some("windows"));

    job.label = Some(String::new());
    apis::jobs_api::update_job(config, job_id, job).expect("Failed to update job");
    let cleared = apis::jobs_api::get_job(config, job_id).expect("Failed to get job");
    assert_eq!(cleared.label, None);
}

/// Labels match exactly, so an empty or whitespace-padded label would strand the job.
#[rstest]
fn test_invalid_labels_are_rejected(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow_id = create_labeled_workflow(config, "label_invalid");

    for bad in ["", " ", "windows ", " windows"] {
        let mut job =
            models::JobModel::new(workflow_id, format!("bad_{bad:?}"), "echo".to_string());
        job.label = Some(bad.to_string());
        assert!(
            apis::jobs_api::create_job(config, job).is_err(),
            "label {bad:?} should be rejected on create"
        );
    }

    let response = apis::workflows_api::claim_next_jobs(config, workflow_id, Some(1), None)
        .expect("claim_next_jobs should succeed");
    let mut job = response.jobs.unwrap().remove(0);
    let job_id = job.id.unwrap();
    job.label = Some("windows ".to_string());
    assert!(apis::jobs_api::update_job(config, job_id, job).is_err());
}

/// A persistent runner exits once no unfinished jobs with its label remain, even though
/// the workflow is not complete (an unlabeled job that nobody runs stays `ready`).
#[rstest]
fn test_persistent_runner_exits_when_its_label_is_done(start_server: &ServerProcess) {
    let config = &start_server.config;
    let mut workflow =
        models::WorkflowModel::new("label_persistent_exit".to_string(), "test_user".to_string());
    workflow.compute_node_wait_for_new_jobs_seconds = Some(1);
    let workflow_id = apis::workflows_api::create_workflow(config, workflow)
        .expect("Failed to create workflow")
        .id
        .unwrap();
    let rr = create_test_resource_requirements(config, workflow_id, "small", 1, 0, 1, "1g", "PT1M");

    let mut ids = Vec::new();
    for (name, label) in [("plain", None), ("windows", Some("windows"))] {
        let mut job = models::JobModel::new(workflow_id, name.to_string(), "echo hi".to_string());
        job.resource_requirements_id = rr.id;
        job.label = label.map(str::to_string);
        ids.push(
            apis::jobs_api::create_job(config, job)
                .expect("Failed to create job")
                .id
                .unwrap(),
        );
    }
    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let workflow_id_str = workflow_id.to_string();
    // Hangs here if the runner waits for the whole workflow to complete.
    run_jobs_cli_command(
        &[
            workflow_id_str.as_str(),
            "--output-dir",
            temp_dir.path().to_str().unwrap(),
            "--poll-interval",
            "0.2",
            "--label",
            "windows",
            "--persistent",
        ],
        start_server,
    )
    .expect("Persistent labeled runner failed");

    let status = |id| apis::jobs_api::get_job(config, id).unwrap().status;
    assert_eq!(status(ids[0]), Some(models::JobStatus::Ready));
    assert_eq!(status(ids[1]), Some(models::JobStatus::Completed));

    // The runner's compute node records the label it was started with.
    let nodes = apis::compute_nodes_api::list_compute_nodes(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list compute nodes")
    .items;
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].label.as_deref(), Some("windows"));
}

/// An unlabeled runner and a persistent labeled runner share one workflow. The labeled job
/// depends on the unlabeled one, which runs longer than the workflow's idle timeout. Without
/// `--persistent` the labeled runner would give up before its job became ready.
#[rstest]
fn test_persistent_labeled_runner_waits_for_its_job(start_server: &ServerProcess) {
    let config = &start_server.config;
    let mut workflow =
        models::WorkflowModel::new("label_persistent".to_string(), "test_user".to_string());
    workflow.compute_node_wait_for_new_jobs_seconds = Some(1);
    let workflow_id = apis::workflows_api::create_workflow(config, workflow)
        .expect("Failed to create workflow")
        .id
        .unwrap();
    let rr = create_test_resource_requirements(config, workflow_id, "small", 1, 0, 1, "1g", "PT1M");

    let mut first = models::JobModel::new(workflow_id, "plain".to_string(), "sleep 4".to_string());
    first.resource_requirements_id = rr.id;
    let first_id = apis::jobs_api::create_job(config, first)
        .expect("Failed to create job")
        .id
        .unwrap();

    let mut second = models::JobModel::new(
        workflow_id,
        "windows".to_string(),
        "echo windows".to_string(),
    );
    second.resource_requirements_id = rr.id;
    second.label = Some("windows".to_string());
    second.depends_on_job_ids = Some(vec![first_id]);
    let second_id = apis::jobs_api::create_job(config, second)
        .expect("Failed to create job")
        .id
        .unwrap();

    apis::workflows_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let output_dir = temp_dir.path().to_str().unwrap();
    let workflow_id_str = workflow_id.to_string();
    let common_args = [
        workflow_id_str.as_str(),
        "--output-dir",
        output_dir,
        "--poll-interval",
        "0.2",
    ];

    std::thread::scope(|scope| {
        let labeled = scope.spawn(|| {
            let mut args = common_args.to_vec();
            args.extend(["--label", "windows", "--persistent"]);
            run_jobs_cli_command(&args, start_server).map_err(|e| e.to_string())
        });
        let mut args = common_args.to_vec();
        args.extend(["--max-parallel-jobs", "1"]);
        run_jobs_cli_command(&args, start_server).expect("Unlabeled runner failed");
        labeled
            .join()
            .unwrap()
            .expect("Persistent labeled runner failed");
    });

    for job_id in [first_id, second_id] {
        let job = apis::jobs_api::get_job(config, job_id).expect("Failed to get job");
        assert_eq!(job.status, Some(models::JobStatus::Completed));
    }
}
