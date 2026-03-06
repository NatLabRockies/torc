mod common;

use common::{ServerProcess, create_test_workflow, start_server};
use rstest::rstest;
use torc::client::apis::default_api;
use torc::models::{DatasetModel, DatasetStatus, HashMode};

/// Helper to create a test dataset
fn create_test_dataset(
    config: &torc::client::apis::configuration::Configuration,
    workflow_id: i64,
    name: &str,
    path: &str,
) -> DatasetModel {
    let dataset = DatasetModel {
        id: None,
        workflow_id,
        name: name.to_string(),
        path: path.to_string(),
        description: Some(format!("Test dataset: {}", name)),
        status: DatasetStatus::Pending,
        hash_mode: HashMode::Manifest,
        file_count: None,
        total_size_bytes: None,
        manifest_hash: None,
        claimed_by_node_id: None,
        claimed_at: None,
        finalized_at: None,
    };
    default_api::create_dataset(config, dataset).expect("Failed to create dataset")
}

// =============================================================================
// Dataset CRUD Tests
// =============================================================================

#[rstest]
fn test_dataset_create(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "dataset_create_workflow");
    let workflow_id = workflow.id.unwrap();

    let dataset = DatasetModel {
        id: None,
        workflow_id,
        name: "output_dataset".to_string(),
        path: "/data/outputs/results".to_string(),
        description: Some("Test output dataset".to_string()),
        status: DatasetStatus::Pending,
        hash_mode: HashMode::Manifest,
        file_count: None,
        total_size_bytes: None,
        manifest_hash: None,
        claimed_by_node_id: None,
        claimed_at: None,
        finalized_at: None,
    };

    let created = default_api::create_dataset(config, dataset).expect("Failed to create dataset");

    assert!(created.id.is_some());
    assert_eq!(created.workflow_id, workflow_id);
    assert_eq!(created.name, "output_dataset");
    assert_eq!(created.path, "/data/outputs/results");
    assert_eq!(created.status, DatasetStatus::Pending);
    assert_eq!(created.hash_mode, HashMode::Manifest);
}

#[rstest]
fn test_dataset_create_with_different_hash_modes(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "dataset_hash_modes_workflow");
    let workflow_id = workflow.id.unwrap();

    let hash_modes = [
        (HashMode::Manifest, "manifest_dataset"),
        (HashMode::Content, "content_dataset"),
        (HashMode::None, "none_dataset"),
    ];

    for (hash_mode, name) in hash_modes {
        let dataset = DatasetModel {
            id: None,
            workflow_id,
            name: name.to_string(),
            path: format!("/data/{}", name),
            description: None,
            status: DatasetStatus::Pending,
            hash_mode,
            file_count: None,
            total_size_bytes: None,
            manifest_hash: None,
            claimed_by_node_id: None,
            claimed_at: None,
            finalized_at: None,
        };

        let created =
            default_api::create_dataset(config, dataset).expect("Failed to create dataset");

        assert_eq!(created.hash_mode, hash_mode);
    }
}

#[rstest]
fn test_dataset_with_description(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "dataset_description_workflow");
    let workflow_id = workflow.id.unwrap();

    let dataset = DatasetModel {
        id: None,
        workflow_id,
        name: "described_dataset".to_string(),
        path: "/data/described".to_string(),
        description: Some("This is a detailed description of the dataset".to_string()),
        status: DatasetStatus::Pending,
        hash_mode: HashMode::Manifest,
        file_count: None,
        total_size_bytes: None,
        manifest_hash: None,
        claimed_by_node_id: None,
        claimed_at: None,
        finalized_at: None,
    };

    let created = default_api::create_dataset(config, dataset).expect("Failed to create dataset");

    assert_eq!(
        created.description,
        Some("This is a detailed description of the dataset".to_string())
    );
}

#[rstest]
fn test_dataset_multiple_per_workflow(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "dataset_multi_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create multiple datasets for the same workflow
    let dataset1 = create_test_dataset(config, workflow_id, "dataset_1", "/data/output_1");
    let dataset2 = create_test_dataset(config, workflow_id, "dataset_2", "/data/output_2");
    let dataset3 = create_test_dataset(config, workflow_id, "dataset_3", "/data/output_3");

    // Verify they all have unique IDs
    assert_ne!(dataset1.id, dataset2.id);
    assert_ne!(dataset2.id, dataset3.id);
    assert_ne!(dataset1.id, dataset3.id);

    // Verify they're all for the same workflow
    assert_eq!(dataset1.workflow_id, workflow_id);
    assert_eq!(dataset2.workflow_id, workflow_id);
    assert_eq!(dataset3.workflow_id, workflow_id);
}

// =============================================================================
// NOTE: The following tests are commented out because they require HTTP routing
// handlers that haven't been fully implemented yet in routing.rs:
//
// - GET /datasets/{id} - get a single dataset
// - GET /workflows/{id}/datasets - list datasets for a workflow
// - POST /datasets/{id}/finalize - finalize a dataset
// - DELETE /datasets/{id} - delete a dataset
// - DELETE /workflows/{id}/datasets - delete all datasets for a workflow
//
// The API trait methods exist and are implemented in http_server.rs, but the
// routing dispatch code needs to be added to routing.rs.
//
// TODO: Implement routing handlers for these endpoints, then uncomment tests.
// =============================================================================

// #[rstest]
// fn test_dataset_get(start_server: &ServerProcess) { ... }
//
// #[rstest]
// fn test_dataset_list(start_server: &ServerProcess) { ... }
//
// #[rstest]
// fn test_dataset_list_by_status(start_server: &ServerProcess) { ... }
//
// #[rstest]
// fn test_dataset_fan_in_dependency(start_server: &ServerProcess) { ... }
//
// #[rstest]
// fn test_dataset_finalization_unblocks_consumer(start_server: &ServerProcess) { ... }
//
// #[rstest]
// fn test_datasets_list_cli(start_server: &ServerProcess) { ... }
//
// #[rstest]
// fn test_datasets_get_cli(start_server: &ServerProcess) { ... }
