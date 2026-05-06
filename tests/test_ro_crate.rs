mod common;

use common::{ServerProcess, create_test_workflow, start_server};
use rstest::rstest;
use serde_json::json;
use torc::client::apis;
use torc::client::apis::Error;
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
    let list_response = apis::ro_crate_api::list_ro_crate_entities(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list entities");
    let items = list_response.items;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].entity_type, "Dataset");

    // Delete the entity
    apis::ro_crate_api::delete_ro_crate_entity(config, entity_id).expect("Failed to delete entity");

    // Verify it's gone
    let list_response = apis::ro_crate_api::list_ro_crate_entities(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
    )
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
fn test_ro_crate_list_filters(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_ro_crate_list_filters");
    let workflow_id = workflow.id.unwrap();

    let file = torc::models::FileModel::new(
        workflow_id,
        "filtered.csv".to_string(),
        "/tmp/filtered.csv".to_string(),
    );
    let created_file =
        apis::files_api::create_file(config, file).expect("Failed to create test file");
    let file_id = created_file.id.unwrap();

    let file_entity = RoCrateEntityModel {
        id: None,
        workflow_id,
        file_id: Some(file_id),
        entity_id: "filtered.csv".to_string(),
        entity_type: "File".to_string(),
        metadata: json!({"name": "Filtered CSV"}).to_string(),
    };
    apis::ro_crate_api::create_ro_crate_entity(config, file_entity)
        .expect("Failed to create file entity");

    let software_entity = RoCrateEntityModel {
        id: None,
        workflow_id,
        file_id: None,
        entity_id: "#software-test-run-id-1".to_string(),
        entity_type: "SoftwareApplication".to_string(),
        metadata: json!({"name": "Test Software"}).to_string(),
    };
    apis::ro_crate_api::create_ro_crate_entity(config, software_entity)
        .expect("Failed to create software entity");

    let by_file = apis::ro_crate_api::list_ro_crate_entities_with_filters(
        config,
        workflow_id,
        Some(0),
        Some(10),
        Some(file_id),
        None,
        None,
        None,
    )
    .expect("Failed to list filtered by file_id");
    assert_eq!(by_file.items.len(), 1);
    assert_eq!(by_file.count, 1);
    assert_eq!(by_file.total_count, 1);
    assert_eq!(by_file.items[0].file_id, Some(file_id));

    let by_entity = apis::ro_crate_api::list_ro_crate_entities_with_filters(
        config,
        workflow_id,
        Some(0),
        Some(10),
        None,
        Some("#software-test-run-id-1"),
        None,
        None,
    )
    .expect("Failed to list filtered by entity_id");
    assert_eq!(by_entity.items.len(), 1);
    assert_eq!(by_entity.items[0].entity_id, "#software-test-run-id-1");

    let by_both = apis::ro_crate_api::list_ro_crate_entities_with_filters(
        config,
        workflow_id,
        Some(0),
        Some(10),
        Some(file_id),
        Some("filtered.csv"),
        None,
        None,
    )
    .expect("Failed to list filtered by file_id and entity_id");
    assert_eq!(by_both.items.len(), 1);
    assert_eq!(by_both.total_count, 1);
    assert!(!by_both.has_more);

    let empty = apis::ro_crate_api::list_ro_crate_entities_with_filters(
        config,
        workflow_id,
        Some(0),
        Some(10),
        Some(file_id),
        Some("does-not-match"),
        None,
        None,
    )
    .expect("Failed to list filtered with non-matching combined filters");
    assert!(empty.items.is_empty());
    assert_eq!(empty.count, 0);
    assert_eq!(empty.total_count, 0);
    assert!(!empty.has_more);

    let paged = apis::ro_crate_api::list_ro_crate_entities_with_filters(
        config,
        workflow_id,
        Some(0),
        Some(1),
        None,
        None,
        Some("entity_id"),
        Some(false),
    )
    .expect("Failed to list paged entities");
    assert_eq!(paged.items.len(), 1);
    assert_eq!(paged.count, 1);
    assert_eq!(paged.total_count, 2);
    assert!(paged.has_more);

    let paged_offset = apis::ro_crate_api::list_ro_crate_entities_with_filters(
        config,
        workflow_id,
        Some(1),
        Some(1),
        None,
        None,
        Some("entity_id"),
        Some(false),
    )
    .expect("Failed to list second page of entities");
    assert_eq!(paged_offset.items.len(), 1);
    assert_eq!(paged_offset.count, 1);
    assert_eq!(paged_offset.total_count, 2);
    assert!(!paged_offset.has_more);

    let missing_by_file =
        apis::ro_crate_api::find_ro_crate_entity_by_file_id(config, workflow_id, file_id + 1)
            .expect("Failed to lookup missing file_id");
    assert!(missing_by_file.is_none());

    let missing_by_entity = apis::ro_crate_api::find_ro_crate_entity_by_entity_id(
        config,
        workflow_id,
        "#software-missing",
    )
    .expect("Failed to lookup missing entity_id");
    assert!(missing_by_entity.is_none());

    let found_by_file =
        apis::ro_crate_api::find_ro_crate_entity_by_file_id(config, workflow_id, file_id)
            .expect("Failed to lookup by file_id");
    assert_eq!(
        found_by_file
            .expect("Expected entity for file_id")
            .entity_id,
        "filtered.csv"
    );

    let found_by_entity = apis::ro_crate_api::find_ro_crate_entity_by_entity_id(
        config,
        workflow_id,
        "#software-test-run-id-1",
    )
    .expect("Failed to lookup by entity_id");
    assert_eq!(
        found_by_entity
            .expect("Expected entity for entity_id")
            .entity_type,
        "SoftwareApplication"
    );
}

#[rstest]
fn test_ro_crate_rejects_duplicate_workflow_file_link(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow(config, "test_ro_crate_duplicate_file_link");
    let workflow_id = workflow.id.unwrap();

    let file = torc::models::FileModel::new(
        workflow_id,
        "duplicate.csv".to_string(),
        "/tmp/duplicate.csv".to_string(),
    );
    let created_file =
        apis::files_api::create_file(config, file).expect("Failed to create test file");
    let file_id = created_file.id.unwrap();

    let first = RoCrateEntityModel {
        id: None,
        workflow_id,
        file_id: Some(file_id),
        entity_id: "duplicate.csv".to_string(),
        entity_type: "File".to_string(),
        metadata: json!({"name": "First"}).to_string(),
    };
    apis::ro_crate_api::create_ro_crate_entity(config, first)
        .expect("Failed to create first entity");

    let duplicate = RoCrateEntityModel {
        id: None,
        workflow_id,
        file_id: Some(file_id),
        entity_id: "duplicate-second.csv".to_string(),
        entity_type: "File".to_string(),
        metadata: json!({"name": "Second"}).to_string(),
    };
    let result = apis::ro_crate_api::create_ro_crate_entity(config, duplicate);
    match result {
        Err(Error::ResponseError(content)) => {
            assert_eq!(content.status.as_u16(), 422);
            assert!(
                content
                    .content
                    .contains("RO-Crate entity already exists for this workflow/file link"),
                "unexpected error payload: {}",
                content.content
            );
        }
        other => panic!("Expected 422 duplicate rejection, got: {:?}", other),
    }
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
    let list = apis::ro_crate_api::list_ro_crate_entities(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list");
    assert_eq!(list.items.len(), 3);

    // Bulk delete all entities for the workflow
    let result = apis::ro_crate_api::delete_ro_crate_entities(config, workflow_id, None)
        .expect("Failed to bulk delete");
    assert_eq!(result.deleted_count, 3);

    // Verify all are gone
    let list = apis::ro_crate_api::list_ro_crate_entities(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
    )
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
    let list = apis::ro_crate_api::list_ro_crate_entities(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list");
    assert_eq!(list.items.len(), 1);

    // Delete the workflow (should cascade delete RO-Crate entities)
    apis::workflows_api::delete_workflow(config, workflow_id).expect("Failed to delete workflow");

    // The workflow is gone, so listing should fail or return error
    let result = apis::ro_crate_api::list_ro_crate_entities(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
    );
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
