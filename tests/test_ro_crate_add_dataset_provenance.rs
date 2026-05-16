//! Integration tests for `torc ro-crate add-dataset --job-id`.
//!
//! These verify the provenance wiring: that a CreateAction entity is created
//! (or updated) for each producing job, that the dataset is added to the
//! CreateAction's `result`, and that the dataset's `prov:wasGeneratedBy`
//! references those CreateAction entities.

mod common;

use common::{ServerProcess, run_cli_command, start_server};
use rstest::rstest;
use std::fs;
use std::path::Path;
use torc::client::apis;
use torc::models;

/// Create a workflow with one job (no tracked files needed for these tests).
/// Returns (workflow_id, job_id).
fn create_workflow_with_job(config: &torc::client::Configuration, job_name: &str) -> (i64, i64) {
    let workflow = models::WorkflowModel::new(
        "test_add_dataset_provenance".to_string(),
        "test_user".to_string(),
    );
    let created_workflow =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created_workflow.id.unwrap();

    let job = models::JobModel::new(workflow_id, job_name.to_string(), "echo hi".to_string());
    let created_job = apis::jobs_api::create_job(config, job).expect("Failed to create job");
    let job_id = created_job.id.unwrap();

    (workflow_id, job_id)
}

/// Create a directory with a couple of files to register as a dataset.
fn make_dataset_dir(base: &Path) -> String {
    let dir = base.join("ds");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("part-0.parquet"), b"abc").unwrap();
    fs::write(dir.join("part-1.parquet"), b"defg").unwrap();
    dir.to_string_lossy().to_string()
}

fn list_entities(
    config: &torc::client::Configuration,
    workflow_id: i64,
) -> Vec<models::RoCrateEntityModel> {
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
    .expect("Failed to list RO-Crate entities")
    .items
}

#[rstest]
fn test_add_dataset_creates_create_action_for_job(start_server: &ServerProcess) {
    let config = &start_server.config;
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

    let (workflow_id, job_id) = create_workflow_with_job(config, "produce_dataset");
    let dataset_path = make_dataset_dir(temp_dir.path());

    // The job never ran, so no CreateAction exists yet. add-dataset must create
    // one and wire the bidirectional provenance link.
    let workflow_id_str = workflow_id.to_string();
    let job_id_str = job_id.to_string();
    run_cli_command(
        &[
            "ro-crate",
            "add-dataset",
            &workflow_id_str,
            "--name",
            "training_output",
            "--path",
            &dataset_path,
            "-j",
            &job_id_str,
        ],
        start_server,
        None,
    )
    .expect("add-dataset command failed");

    let entities = list_entities(config, workflow_id);
    let expected_action_id = format!("#job-{}-attempt-1", job_id);
    let dataset_entity_id = format!("{}/", dataset_path);

    // A CreateAction entity must exist for the producing job.
    let action = entities
        .iter()
        .find(|e| e.entity_id == expected_action_id)
        .unwrap_or_else(|| {
            panic!(
                "Expected a CreateAction entity '{}'. Found: {:?}",
                expected_action_id,
                entities.iter().map(|e| &e.entity_id).collect::<Vec<_>>()
            )
        });
    assert_eq!(action.entity_type, "CreateAction");

    let action_meta: serde_json::Value =
        serde_json::from_str(&action.metadata).expect("CreateAction metadata is not valid JSON");
    assert_eq!(action_meta["@type"][0], "CreateAction");
    // The dataset must appear in the CreateAction's result array.
    let result = action_meta["result"]
        .as_array()
        .expect("CreateAction should have a result array");
    assert!(
        result
            .iter()
            .any(|r| r["@id"] == serde_json::json!(dataset_entity_id)),
        "CreateAction result should reference the dataset. result={:?}",
        result
    );

    // The Dataset entity must point back at the CreateAction.
    let dataset = entities
        .iter()
        .find(|e| e.entity_id == dataset_entity_id)
        .expect("Dataset entity should exist");
    assert_eq!(dataset.entity_type, "Dataset");
    let dataset_meta: serde_json::Value =
        serde_json::from_str(&dataset.metadata).expect("Dataset metadata is not valid JSON");
    assert_eq!(
        dataset_meta["prov:wasGeneratedBy"]["@id"],
        serde_json::json!(expected_action_id),
        "Dataset prov:wasGeneratedBy should reference the CreateAction"
    );
}

#[rstest]
fn test_add_dataset_multiple_jobs_creates_actions(start_server: &ServerProcess) {
    let config = &start_server.config;
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

    let (workflow_id, job1) = create_workflow_with_job(config, "writer_a");
    // Add a second job to the same workflow.
    let job2 = {
        let j = models::JobModel::new(workflow_id, "writer_b".to_string(), "echo hi".to_string());
        apis::jobs_api::create_job(config, j)
            .expect("Failed to create job2")
            .id
            .unwrap()
    };
    let dataset_path = make_dataset_dir(temp_dir.path());

    let workflow_id_str = workflow_id.to_string();
    let job1_str = job1.to_string();
    let job2_str = job2.to_string();
    run_cli_command(
        &[
            "ro-crate",
            "add-dataset",
            &workflow_id_str,
            "--name",
            "shared_output",
            "--path",
            &dataset_path,
            "-j",
            &job1_str,
            "-j",
            &job2_str,
        ],
        start_server,
        None,
    )
    .expect("add-dataset command failed");

    let entities = list_entities(config, workflow_id);
    let action1 = format!("#job-{}-attempt-1", job1);
    let action2 = format!("#job-{}-attempt-1", job2);
    let dataset_entity_id = format!("{}/", dataset_path);

    for action_id in [&action1, &action2] {
        assert!(
            entities
                .iter()
                .any(|e| &e.entity_id == action_id && e.entity_type == "CreateAction"),
            "Expected CreateAction entity '{}'",
            action_id
        );
    }

    let dataset = entities
        .iter()
        .find(|e| e.entity_id == dataset_entity_id)
        .expect("Dataset entity should exist");
    let dataset_meta: serde_json::Value =
        serde_json::from_str(&dataset.metadata).expect("Dataset metadata is not valid JSON");
    let generated_by = dataset_meta["prov:wasGeneratedBy"]
        .as_array()
        .expect("prov:wasGeneratedBy should be an array for multiple jobs");
    let referenced: Vec<&str> = generated_by
        .iter()
        .filter_map(|v| v["@id"].as_str())
        .collect();
    assert!(referenced.contains(&action1.as_str()));
    assert!(referenced.contains(&action2.as_str()));
}

#[rstest]
fn test_add_dataset_updates_existing_create_action(start_server: &ServerProcess) {
    let config = &start_server.config;
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

    let (workflow_id, job_id) = create_workflow_with_job(config, "ran_already");
    let dataset_path = make_dataset_dir(temp_dir.path());
    let action_id = format!("#job-{}-attempt-1", job_id);

    // Simulate a CreateAction that already exists from a prior run with a
    // tracked output file in its result array.
    let existing_meta = serde_json::json!({
        "@id": action_id,
        "@type": ["CreateAction", "prov:Activity"],
        "name": "ran_already",
        "result": [{ "@id": "output/existing.json" }]
    });
    let existing = models::RoCrateEntityModel::new(
        workflow_id,
        action_id.clone(),
        "CreateAction".to_string(),
        existing_meta.to_string(),
    );
    apis::ro_crate_entities_api::create_ro_crate_entity(config, existing)
        .expect("Failed to pre-create CreateAction");

    let workflow_id_str = workflow_id.to_string();
    let job_id_str = job_id.to_string();
    run_cli_command(
        &[
            "ro-crate",
            "add-dataset",
            &workflow_id_str,
            "--name",
            "training_output",
            "--path",
            &dataset_path,
            "-j",
            &job_id_str,
        ],
        start_server,
        None,
    )
    .expect("add-dataset command failed");

    let entities = list_entities(config, workflow_id);
    let dataset_entity_id = format!("{}/", dataset_path);

    // Still exactly one CreateAction for the job (updated, not duplicated).
    let actions: Vec<_> = entities
        .iter()
        .filter(|e| e.entity_id == action_id)
        .collect();
    assert_eq!(
        actions.len(),
        1,
        "CreateAction should be updated in place, not duplicated"
    );

    let action_meta: serde_json::Value =
        serde_json::from_str(&actions[0].metadata).expect("CreateAction metadata invalid");
    let result = action_meta["result"]
        .as_array()
        .expect("result should be an array");
    let result_ids: Vec<&str> = result.iter().filter_map(|r| r["@id"].as_str()).collect();
    // The pre-existing tracked output is preserved...
    assert!(
        result_ids.contains(&"output/existing.json"),
        "Existing tracked output should be preserved. result={:?}",
        result_ids
    );
    // ...and the dataset is appended.
    assert!(
        result_ids.contains(&dataset_entity_id.as_str()),
        "Dataset should be appended to existing result. result={:?}",
        result_ids
    );
}
