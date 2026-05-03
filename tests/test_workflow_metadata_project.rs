mod common;

use common::{ServerProcess, start_server};
use rstest::rstest;
use std::collections::HashMap;
use torc::client::apis;
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
        env: None,
        timestamp: None,
        compute_node_expiration_buffer_seconds: None,
        compute_node_wait_for_new_jobs_seconds: Some(0),
        compute_node_ignore_workflow_completion: Some(false),
        compute_node_wait_for_healthy_database_minutes: Some(20),
        compute_node_min_time_for_new_jobs_seconds: Some(300),
        resource_monitor_config: None,
        slurm_defaults: None,
        use_pending_failed: Some(false),
        enable_ro_crate: None,
        project: Some("test-project".to_string()),
        metadata: Some(r#"{"key":"value","num":42}"#.to_string()),
        slurm_config: None,
        execution_config: None,
    };

    // Create the workflow
    let created =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");

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
        env: None,
        timestamp: None,
        compute_node_expiration_buffer_seconds: None,
        compute_node_wait_for_new_jobs_seconds: Some(0),
        compute_node_ignore_workflow_completion: Some(false),
        compute_node_wait_for_healthy_database_minutes: Some(20),
        compute_node_min_time_for_new_jobs_seconds: Some(300),
        resource_monitor_config: None,
        slurm_defaults: None,
        use_pending_failed: Some(false),
        enable_ro_crate: None,
        project: None,
        metadata: None,
        slurm_config: None,
        execution_config: None,
    };

    let created =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created.id.unwrap();

    // Verify fields are None initially
    assert_eq!(created.project, None);
    assert_eq!(created.metadata, None);

    // Update with new values
    let mut update = created.clone();
    update.project = Some("updated-project".to_string());
    update.metadata = Some(r#"{"updated":true}"#.to_string());

    let updated = apis::workflows_api::update_workflow(config, workflow_id, update)
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
        env: None,
        timestamp: None,
        compute_node_expiration_buffer_seconds: None,
        compute_node_wait_for_new_jobs_seconds: Some(0),
        compute_node_ignore_workflow_completion: Some(false),
        compute_node_wait_for_healthy_database_minutes: Some(20),
        compute_node_min_time_for_new_jobs_seconds: Some(300),
        resource_monitor_config: None,
        slurm_defaults: None,
        use_pending_failed: Some(false),
        enable_ro_crate: None,
        project: Some("initial-project".to_string()),
        metadata: Some(r#"{"version":"1.0"}"#.to_string()),
        slurm_config: None,
        execution_config: None,
    };

    let created =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created.id.unwrap();

    // Verify initial values
    assert_eq!(created.project, Some("initial-project".to_string()));
    assert_eq!(created.metadata, Some(r#"{"version":"1.0"}"#.to_string()));

    // Update with new values
    let mut update = created.clone();
    update.project = Some("changed-project".to_string());
    update.metadata = Some(r#"{"version":"2.0","updated":true}"#.to_string());

    let updated = apis::workflows_api::update_workflow(config, workflow_id, update)
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
        env: None,
        timestamp: None,
        compute_node_expiration_buffer_seconds: None,
        compute_node_wait_for_new_jobs_seconds: Some(0),
        compute_node_ignore_workflow_completion: Some(false),
        compute_node_wait_for_healthy_database_minutes: Some(20),
        compute_node_min_time_for_new_jobs_seconds: Some(300),
        resource_monitor_config: None,
        slurm_defaults: None,
        use_pending_failed: Some(false),
        enable_ro_crate: None,
        project: Some("my-project".to_string()),
        metadata: Some(r#"{"key":"value"}"#.to_string()),
        slurm_config: None,
        execution_config: None,
    };

    let created =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created.id.unwrap();

    // Update only project, leaving metadata as None (should preserve existing)
    let mut update = created.clone();
    update.project = Some("new-project".to_string());
    update.metadata = None; // Don't update metadata

    let updated = apis::workflows_api::update_workflow(config, workflow_id, update)
        .expect("Failed to update workflow");

    // Verify project changed but metadata preserved
    assert_eq!(updated.project, Some("new-project".to_string()));
    assert_eq!(updated.metadata, Some(r#"{"key":"value"}"#.to_string()));
}

#[rstest]
fn test_workflow_env_is_immutable_after_creation(start_server: &ServerProcess) {
    let config = &start_server.config;

    let mut workflow = models::WorkflowModel::new(
        "test_workflow_env_immutable".to_string(),
        "test_user".into(),
    );
    workflow.env = Some(HashMap::from([(
        "LOG_LEVEL".to_string(),
        "info".to_string(),
    )]));

    let created =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created.id.unwrap();

    let mut update = created.clone();
    update.env = Some(HashMap::from([(
        "LOG_LEVEL".to_string(),
        "debug".to_string(),
    )]));

    let result = apis::workflows_api::update_workflow(config, workflow_id, update);
    assert!(result.is_err(), "Updating workflow env should fail");

    let err = result.unwrap_err();
    if let torc::client::apis::Error::ResponseError(response) = &err {
        assert_eq!(
            response.status.as_u16(),
            422,
            "Expected HTTP 422 for immutable env update, got: {}",
            response.status
        );
    } else {
        panic!("Expected ResponseError, got: {:?}", err);
    }

    let err_str = format!("{:?}", err);
    assert!(
        err_str.contains("immutable") || err_str.contains("Cannot modify env"),
        "Error should mention env immutability, got: {}",
        err_str
    );

    let fetched =
        apis::workflows_api::get_workflow(config, workflow_id).expect("Failed to fetch workflow");
    assert_eq!(fetched.env, created.env);
}

#[rstest]
fn test_create_workflow_normalizes_empty_env_in_response(start_server: &ServerProcess) {
    let config = &start_server.config;

    let mut workflow = models::WorkflowModel::new(
        "test_workflow_empty_env_normalized".to_string(),
        "test_user".to_string(),
    );
    workflow.env = Some(HashMap::new());

    let created =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    assert_eq!(created.env, None);

    let fetched = apis::workflows_api::get_workflow(config, created.id.unwrap())
        .expect("Failed to fetch workflow");
    assert_eq!(fetched.env, None);
}

#[rstest]
fn test_create_workflow_rejects_invalid_env_name(start_server: &ServerProcess) {
    let config = &start_server.config;

    let mut workflow = models::WorkflowModel::new(
        "test_workflow_invalid_env_name".to_string(),
        "test_user".to_string(),
    );
    workflow.env = Some(HashMap::from([(
        "BAD-NAME".to_string(),
        "value".to_string(),
    )]));

    let result = apis::workflows_api::create_workflow(config, workflow);
    assert!(
        result.is_err(),
        "Creating workflow with invalid env should fail"
    );

    let err = result.unwrap_err();
    if let torc::client::apis::Error::ResponseError(response) = &err {
        assert_eq!(
            response.status.as_u16(),
            422,
            "Expected HTTP 422 for invalid env name, got: {}",
            response.status
        );
    } else {
        panic!("Expected ResponseError, got: {:?}", err);
    }

    let err_str = format!("{:?}", err);
    assert!(
        err_str.contains("BAD-NAME"),
        "Error should mention invalid env key"
    );
}

#[rstest]
fn test_list_workflows_returns_env(start_server: &ServerProcess) {
    let config = &start_server.config;

    let mut workflow = models::WorkflowModel::new(
        "test_list_workflows_with_env".to_string(),
        "test_user".to_string(),
    );
    workflow.env = Some(HashMap::from([(
        "TORC_TEST_ENV".to_string(),
        "present".to_string(),
    )]));

    let created =
        apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow");
    let workflow_id = created.id.unwrap();

    let response = apis::workflows_api::list_workflows(
        config,
        None,
        None,
        Some("env"),
        None,
        Some("test_list_workflows_with_env"),
        None,
        None,
        Some(false),
    )
    .expect("Failed to list workflows with archived filter");

    let listed = response
        .items
        .into_iter()
        .find(|workflow| workflow.id == Some(workflow_id))
        .expect("Created workflow should be listed");

    assert_eq!(listed.env, created.env);
}
