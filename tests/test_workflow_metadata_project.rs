mod common;

use common::{ServerProcess, start_server};
use rstest::rstest;
use torc::client::default_api;
use torc::models;

#[rstest]
fn test_create_workflow_with_project_and_metadata(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow model with project and metadata
    let workflow = models::WorkflowModel {
        id: None,
        name: "test_metadata_project_workflow".to_string(),
        user: "test_user".to_string(),
        description: Some("Test workflow with metadata and project".to_string()),
        timestamp: None,
        compute_node_expiration_buffer_seconds: Some(180),
        compute_node_wait_for_new_jobs_seconds: Some(0),
        compute_node_ignore_workflow_completion: Some(false),
        compute_node_wait_for_healthy_database_minutes: Some(20),
        compute_node_min_time_for_new_jobs_seconds: Some(300),
        jobs_sort_method: None,
        resource_monitor_config: None,
        slurm_defaults: None,
        use_pending_failed: Some(false),
        project: Some("test-project".to_string()),
        metadata: Some(r#"{"key":"value","num":42}"#.to_string()),
        status_id: None,
    };

    // Create the workflow
    let created =
        default_api::create_workflow(config, workflow).expect("Failed to create workflow");

    // Verify fields are set
    assert_eq!(created.name, "test_metadata_project_workflow");
    assert_eq!(created.project, Some("test-project".to_string()));
    assert_eq!(
        created.metadata,
        Some(r#"{"key":"value","num":42}"#.to_string())
    );
}

#[rstest]
fn test_create_workflow_without_fields_then_update(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow without project/metadata
    let workflow = models::WorkflowModel {
        id: None,
        name: "test_update_metadata_workflow".to_string(),
        user: "test_user".to_string(),
        description: None,
        timestamp: None,
        compute_node_expiration_buffer_seconds: Some(180),
        compute_node_wait_for_new_jobs_seconds: Some(0),
        compute_node_ignore_workflow_completion: Some(false),
        compute_node_wait_for_healthy_database_minutes: Some(20),
        compute_node_min_time_for_new_jobs_seconds: Some(300),
        jobs_sort_method: None,
        resource_monitor_config: None,
        slurm_defaults: None,
        use_pending_failed: Some(false),
        project: None,
        metadata: None,
        status_id: None,
    };

    let created =
        default_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created.id.unwrap();

    // Verify fields are None initially
    assert_eq!(created.project, None);
    assert_eq!(created.metadata, None);

    // Update with new values
    let mut update = created.clone();
    update.project = Some("updated-project".to_string());
    update.metadata = Some(r#"{"updated":true}"#.to_string());

    let updated = default_api::update_workflow(config, workflow_id, update)
        .expect("Failed to update workflow");

    // Verify fields are updated
    assert_eq!(updated.project, Some("updated-project".to_string()));
    assert_eq!(updated.metadata, Some(r#"{"updated":true}"#.to_string()));
}

#[rstest]
fn test_create_workflow_with_fields_then_change(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow with initial values
    let workflow = models::WorkflowModel {
        id: None,
        name: "test_change_metadata_workflow".to_string(),
        user: "test_user".to_string(),
        description: None,
        timestamp: None,
        compute_node_expiration_buffer_seconds: Some(180),
        compute_node_wait_for_new_jobs_seconds: Some(0),
        compute_node_ignore_workflow_completion: Some(false),
        compute_node_wait_for_healthy_database_minutes: Some(20),
        compute_node_min_time_for_new_jobs_seconds: Some(300),
        jobs_sort_method: None,
        resource_monitor_config: None,
        slurm_defaults: None,
        use_pending_failed: Some(false),
        project: Some("initial-project".to_string()),
        metadata: Some(r#"{"version":"1.0"}"#.to_string()),
        status_id: None,
    };

    let created =
        default_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created.id.unwrap();

    // Verify initial values
    assert_eq!(created.project, Some("initial-project".to_string()));
    assert_eq!(created.metadata, Some(r#"{"version":"1.0"}"#.to_string()));

    // Update with new values
    let mut update = created.clone();
    update.project = Some("changed-project".to_string());
    update.metadata = Some(r#"{"version":"2.0","updated":true}"#.to_string());

    let updated = default_api::update_workflow(config, workflow_id, update)
        .expect("Failed to update workflow");

    // Verify fields are changed
    assert_eq!(updated.project, Some("changed-project".to_string()));
    assert_eq!(
        updated.metadata,
        Some(r#"{"version":"2.0","updated":true}"#.to_string())
    );
}

#[rstest]
fn test_partial_update_preserves_fields(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow with both fields
    let workflow = models::WorkflowModel {
        id: None,
        name: "test_preserve_metadata_workflow".to_string(),
        user: "test_user".to_string(),
        description: None,
        timestamp: None,
        compute_node_expiration_buffer_seconds: Some(180),
        compute_node_wait_for_new_jobs_seconds: Some(0),
        compute_node_ignore_workflow_completion: Some(false),
        compute_node_wait_for_healthy_database_minutes: Some(20),
        compute_node_min_time_for_new_jobs_seconds: Some(300),
        jobs_sort_method: None,
        resource_monitor_config: None,
        slurm_defaults: None,
        use_pending_failed: Some(false),
        project: Some("my-project".to_string()),
        metadata: Some(r#"{"key":"value"}"#.to_string()),
        status_id: None,
    };

    let created =
        default_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created.id.unwrap();

    // Update only project, leaving metadata as None (should preserve existing)
    let mut update = created.clone();
    update.project = Some("new-project".to_string());
    update.metadata = None; // Don't update metadata

    let updated = default_api::update_workflow(config, workflow_id, update)
        .expect("Failed to update workflow");

    // Verify project changed but metadata preserved
    assert_eq!(updated.project, Some("new-project".to_string()));
    assert_eq!(updated.metadata, Some(r#"{"key":"value"}"#.to_string()));
}
