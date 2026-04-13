mod common;

use common::{ServerProcess, create_test_workflow, start_server};
use rstest::rstest;
use serde_json::json;
use torc::client::apis;
use torc::models::RoCrateEntityModel;

#[rstest]
fn test_ro_crate_crud(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_ro_crate_crud");
    let workflow_id = workflow.id.unwrap();

    // Create an RO-Crate entity
    let metadata = json!({
        "name": "Simulation Output",
        "description": "Output data from simulation run",
        "encodingFormat": "application/x-parquet"
    });
    let entity = RoCrateEntityModel {
        id: None,
        workflow_id,
        file_id: None,
        entity_id: "data/output.parquet".to_string(),
        entity_type: "File".to_string(),
        metadata: serde_json::to_string(&metadata).unwrap(),
    };

    let created = apis::ro_crate_api::create_ro_crate_entity(config, entity)
        .expect("Failed to create entity");
    assert!(created.id.is_some());
    assert_eq!(created.workflow_id, workflow_id);
    assert_eq!(created.entity_id, "data/output.parquet");
    assert_eq!(created.entity_type, "File");
    let entity_id = created.id.unwrap();

    // Get the entity
    let fetched =
        apis::ro_crate_api::get_ro_crate_entity(config, entity_id).expect("Failed to get entity");
    assert_eq!(fetched.entity_id, "data/output.parquet");
    assert_eq!(fetched.entity_type, "File");
    assert!(fetched.file_id.is_none());

    // Update the entity
    let mut updated = fetched.clone();
    updated.entity_type = "Dataset".to_string();
    let result = apis::ro_crate_api::update_ro_crate_entity(config, entity_id, updated)
        .expect("Failed to update entity");
    assert_eq!(result.entity_type, "Dataset");
    assert_eq!(result.entity_id, "data/output.parquet");

    // List entities
    let list_response =
        apis::ro_crate_api::list_ro_crate_entities(config, workflow_id, None, None, None, None)
            .expect("Failed to list entities");
    let items = list_response.items;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].entity_type, "Dataset");

    // Delete the entity
    apis::ro_crate_api::delete_ro_crate_entity(config, entity_id).expect("Failed to delete entity");

    // Verify it's gone
    let list_response =
        apis::ro_crate_api::list_ro_crate_entities(config, workflow_id, None, None, None, None)
            .expect("Failed to list entities after delete");
    let items = list_response.items;
    assert_eq!(items.len(), 0);
}

#[rstest]
fn test_ro_crate_with_file_id(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_ro_crate_with_file");
    let workflow_id = workflow.id.unwrap();

    // Create a file first
    let file = torc::models::FileModel::new(
        workflow_id,
        "output.csv".to_string(),
        "/tmp/output.csv".to_string(),
    );
    let created_file =
        apis::files_api::create_file(config, file).expect("Failed to create test file");
    let file_id = created_file.id.unwrap();

    // Create an RO-Crate entity linked to the file
    let entity = RoCrateEntityModel {
        id: None,
        workflow_id,
        file_id: Some(file_id),
        entity_id: "output.csv".to_string(),
        entity_type: "File".to_string(),
        metadata: json!({"name": "Output CSV"}).to_string(),
    };

    let created = apis::ro_crate_api::create_ro_crate_entity(config, entity)
        .expect("Failed to create entity");
    assert_eq!(created.file_id, Some(file_id));
}

#[rstest]
fn test_ro_crate_external_entity(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_ro_crate_external");
    let workflow_id = workflow.id.unwrap();

    // Create an external entity (no file_id)
    let entity = RoCrateEntityModel {
        id: None,
        workflow_id,
        file_id: None,
        entity_id: "https://example.com/software/v1.0".to_string(),
        entity_type: "SoftwareApplication".to_string(),
        metadata: json!({
            "name": "My Simulation Software",
            "version": "1.0.0",
            "url": "https://example.com/software"
        })
        .to_string(),
    };

    let created = apis::ro_crate_api::create_ro_crate_entity(config, entity)
        .expect("Failed to create entity");
    assert_eq!(created.entity_id, "https://example.com/software/v1.0");
    assert_eq!(created.entity_type, "SoftwareApplication");
    assert!(created.file_id.is_none());
}

#[rstest]
fn test_ro_crate_bulk_delete(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_ro_crate_bulk_delete");
    let workflow_id = workflow.id.unwrap();

    // Create multiple entities
    for i in 0..3 {
        let entity = RoCrateEntityModel::new(
            workflow_id,
            format!("data/file_{}.csv", i),
            "File".to_string(),
            json!({"name": format!("File {}", i)}).to_string(),
        );
        apis::ro_crate_api::create_ro_crate_entity(config, entity)
            .expect("Failed to create entity");
    }

    // Verify all three exist
    let list =
        apis::ro_crate_api::list_ro_crate_entities(config, workflow_id, None, None, None, None)
            .expect("Failed to list");
    assert_eq!(list.items.len(), 3);

    // Bulk delete all entities for the workflow
    let result = apis::ro_crate_api::delete_ro_crate_entities(config, workflow_id, None)
        .expect("Failed to bulk delete");
    assert_eq!(result.deleted_count, 3);

    // Verify all are gone
    let list =
        apis::ro_crate_api::list_ro_crate_entities(config, workflow_id, None, None, None, None)
            .expect("Failed to list after delete");
    assert_eq!(list.items.len(), 0);
}

#[rstest]
fn test_ro_crate_cascade_delete(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_ro_crate_cascade");
    let workflow_id = workflow.id.unwrap();

    // Create an entity
    let entity = RoCrateEntityModel::new(
        workflow_id,
        "data/result.json".to_string(),
        "File".to_string(),
        json!({"name": "Result"}).to_string(),
    );
    apis::ro_crate_api::create_ro_crate_entity(config, entity).expect("Failed to create entity");

    // Verify it exists
    let list =
        apis::ro_crate_api::list_ro_crate_entities(config, workflow_id, None, None, None, None)
            .expect("Failed to list");
    assert_eq!(list.items.len(), 1);

    // Delete the workflow (should cascade delete RO-Crate entities)
    apis::workflows_api::delete_workflow(config, workflow_id).expect("Failed to delete workflow");

    // The workflow is gone, so listing should fail or return error
    let result =
        apis::ro_crate_api::list_ro_crate_entities(config, workflow_id, None, None, None, None);
    // Either the list returns empty (workflow gone, no entities) or an error
    match result {
        Ok(response) => {
            let items = response.items;
            assert_eq!(items.len(), 0);
        }
        Err(_) => {
            // Expected - workflow no longer exists
        }
    }
}

#[rstest]
fn test_ro_crate_directory_entity(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_ro_crate_directory");
    let workflow_id = workflow.id.unwrap();

    // Create a directory entity for a partitioned dataset
    let entity = RoCrateEntityModel::new(
        workflow_id,
        "data/partitioned_table/".to_string(),
        "Dataset".to_string(),
        json!({
            "name": "Partitioned Table",
            "description": "Hive-partitioned Parquet dataset",
            "encodingFormat": "application/x-parquet"
        })
        .to_string(),
    );

    let created = apis::ro_crate_api::create_ro_crate_entity(config, entity)
        .expect("Failed to create entity");
    assert_eq!(created.entity_id, "data/partitioned_table/");
    assert_eq!(created.entity_type, "Dataset");
}
