mod common;

use common::{ServerProcess, create_test_workflow, start_server};
use rstest::rstest;
use torc::client::apis;
use torc::models;

/// Helper function to create a test Slurm scheduler
fn create_test_slurm_scheduler(
    config: &torc::client::Configuration,
    workflow_id: i64,
    name: &str,
) -> models::SlurmSchedulerModel {
    let scheduler = models::SlurmSchedulerModel {
        id: None,
        workflow_id,
        name: Some(name.to_string()),
        account: "test_account".to_string(),
        gres: Some("gpu:2".to_string()),
        mem: Some("32G".to_string()),
        nodes: 2,
        ntasks_per_node: None,
        partition: Some("test_partition".to_string()),
        qos: Some("normal".to_string()),
        tmp: Some("100G".to_string()),
        walltime: "04:00:00".to_string(),
        extra: None,
    };
    apis::slurm_schedulers_api::create_slurm_scheduler(config, scheduler)
        .expect("Failed to create test Slurm scheduler")
}

#[rstest]
fn test_create_scheduled_compute_node(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow and scheduler
    let workflow = create_test_workflow(config, "test_scheduled_nodes_workflow");
    let workflow_id = workflow.id.unwrap();

    let scheduler = create_test_slurm_scheduler(config, workflow_id, "test_scheduler");
    let scheduler_config_id = scheduler.id.unwrap();

    // Create a scheduled compute node
    let node = models::ScheduledComputeNodesModel::new(
        workflow_id,
        12345, // scheduler_id (e.g., Slurm job ID)
        scheduler_config_id,
        "slurm".to_string(),
        "pending".to_string(),
    );

    let created = apis::scheduled_compute_nodes_api::create_scheduled_compute_node(config, node)
        .expect("Failed to create scheduled compute node");

    assert!(created.id.is_some());
    assert_eq!(created.workflow_id, workflow_id);
    assert_eq!(created.scheduler_id, 12345);
    assert_eq!(created.scheduler_config_id, scheduler_config_id);
    assert_eq!(created.status, "pending");
}

#[rstest]
fn test_get_scheduled_compute_node(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow and scheduler
    let workflow = create_test_workflow(config, "test_get_scheduled_node_workflow");
    let workflow_id = workflow.id.unwrap();

    let scheduler = create_test_slurm_scheduler(config, workflow_id, "test_scheduler");
    let scheduler_config_id = scheduler.id.unwrap();

    // Create a scheduled compute node
    let node = models::ScheduledComputeNodesModel::new(
        workflow_id,
        54321,
        scheduler_config_id,
        "slurm".to_string(),
        "pending".to_string(),
    );

    let created = apis::scheduled_compute_nodes_api::create_scheduled_compute_node(config, node)
        .expect("Failed to create scheduled compute node");
    let node_id = created.id.unwrap();

    // Get the scheduled compute node
    let retrieved = apis::scheduled_compute_nodes_api::get_scheduled_compute_node(config, node_id)
        .expect("Failed to get scheduled compute node");

    assert_eq!(retrieved.id, Some(node_id));
    assert_eq!(retrieved.workflow_id, workflow_id);
    assert_eq!(retrieved.scheduler_id, 54321);
    assert_eq!(retrieved.scheduler_config_id, scheduler_config_id);
    assert_eq!(retrieved.status, "pending");
}

#[rstest]
fn test_update_scheduled_compute_node(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow and scheduler
    let workflow = create_test_workflow(config, "test_update_scheduled_node_workflow");
    let workflow_id = workflow.id.unwrap();

    let scheduler = create_test_slurm_scheduler(config, workflow_id, "test_scheduler");
    let scheduler_config_id = scheduler.id.unwrap();

    // Create a scheduled compute node with "pending" status
    let node = models::ScheduledComputeNodesModel::new(
        workflow_id,
        98765,
        scheduler_config_id,
        "slurm".to_string(),
        "pending".to_string(),
    );

    let created = apis::scheduled_compute_nodes_api::create_scheduled_compute_node(config, node)
        .expect("Failed to create scheduled compute node");
    let node_id = created.id.unwrap();

    // Update status to "active"
    let mut updated_node = created.clone();
    updated_node.status = "active".to_string();

    let updated = apis::scheduled_compute_nodes_api::update_scheduled_compute_node(
        config,
        node_id,
        updated_node,
    )
    .expect("Failed to update scheduled compute node");

    assert_eq!(updated.id, Some(node_id));
    assert_eq!(updated.status, "active");

    // Verify the update by fetching again
    let retrieved = apis::scheduled_compute_nodes_api::get_scheduled_compute_node(config, node_id)
        .expect("Failed to get updated scheduled compute node");

    assert_eq!(retrieved.status, "active");

    // Update status to "complete"
    let mut final_node = updated.clone();
    final_node.status = "complete".to_string();

    let final_updated = apis::scheduled_compute_nodes_api::update_scheduled_compute_node(
        config, node_id, final_node,
    )
    .expect("Failed to update scheduled compute node to complete");

    assert_eq!(final_updated.status, "complete");
}

#[rstest]
fn test_list_scheduled_compute_nodes(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow and scheduler
    let workflow = create_test_workflow(config, "test_list_scheduled_nodes_workflow");
    let workflow_id = workflow.id.unwrap();

    let scheduler = create_test_slurm_scheduler(config, workflow_id, "test_scheduler");
    let scheduler_config_id = scheduler.id.unwrap();

    // Create multiple scheduled compute nodes
    let node1 = models::ScheduledComputeNodesModel::new(
        workflow_id,
        11111,
        scheduler_config_id,
        "slurm".to_string(),
        "pending".to_string(),
    );
    let node2 = models::ScheduledComputeNodesModel::new(
        workflow_id,
        22222,
        scheduler_config_id,
        "slurm".to_string(),
        "active".to_string(),
    );
    let node3 = models::ScheduledComputeNodesModel::new(
        workflow_id,
        33333,
        scheduler_config_id,
        "slurm".to_string(),
        "complete".to_string(),
    );

    apis::scheduled_compute_nodes_api::create_scheduled_compute_node(config, node1)
        .expect("Failed to create node1");
    apis::scheduled_compute_nodes_api::create_scheduled_compute_node(config, node2)
        .expect("Failed to create node2");
    apis::scheduled_compute_nodes_api::create_scheduled_compute_node(config, node3)
        .expect("Failed to create node3");

    // List all scheduled compute nodes for this workflow
    let response = apis::scheduled_compute_nodes_api::list_scheduled_compute_nodes(
        config,
        workflow_id,
        None, // offset
        None, // limit
        None, // sort_by
        None, // reverse_sort
        None, // scheduler_id
        None, // scheduler_config_id
        None, // status
    )
    .expect("Failed to list scheduled compute nodes");

    let items = response.items;
    assert_eq!(items.len(), 3);

    // Verify all nodes are present
    let scheduler_ids: Vec<i64> = items.iter().map(|n| n.scheduler_id).collect();
    assert!(scheduler_ids.contains(&11111));
    assert!(scheduler_ids.contains(&22222));
    assert!(scheduler_ids.contains(&33333));
}

#[rstest]
fn test_list_scheduled_compute_nodes_filter_by_scheduler_id(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow and scheduler
    let workflow = create_test_workflow(config, "test_filter_scheduler_id_workflow");
    let workflow_id = workflow.id.unwrap();

    let scheduler = create_test_slurm_scheduler(config, workflow_id, "test_scheduler");
    let scheduler_config_id = scheduler.id.unwrap();

    // Create scheduled compute nodes with different scheduler IDs
    let node1 = models::ScheduledComputeNodesModel::new(
        workflow_id,
        99999,
        scheduler_config_id,
        "slurm".to_string(),
        "pending".to_string(),
    );
    let node2 = models::ScheduledComputeNodesModel::new(
        workflow_id,
        88888,
        scheduler_config_id,
        "slurm".to_string(),
        "active".to_string(),
    );

    apis::scheduled_compute_nodes_api::create_scheduled_compute_node(config, node1)
        .expect("Failed to create node1");
    apis::scheduled_compute_nodes_api::create_scheduled_compute_node(config, node2)
        .expect("Failed to create node2");

    // Filter by scheduler_id
    let response = apis::scheduled_compute_nodes_api::list_scheduled_compute_nodes(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        Some("99999"), // scheduler_id filter
        None,
        None,
    )
    .expect("Failed to list scheduled compute nodes with filter");

    let items = response.items;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].scheduler_id, 99999);
}

#[rstest]
fn test_list_scheduled_compute_nodes_filter_by_status(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow and scheduler
    let workflow = create_test_workflow(config, "test_filter_status_workflow");
    let workflow_id = workflow.id.unwrap();

    let scheduler = create_test_slurm_scheduler(config, workflow_id, "test_scheduler");
    let scheduler_config_id = scheduler.id.unwrap();

    // Create scheduled compute nodes with different statuses
    let node1 = models::ScheduledComputeNodesModel::new(
        workflow_id,
        77777,
        scheduler_config_id,
        "slurm".to_string(),
        "pending".to_string(),
    );
    let node2 = models::ScheduledComputeNodesModel::new(
        workflow_id,
        66666,
        scheduler_config_id,
        "slurm".to_string(),
        "active".to_string(),
    );
    let node3 = models::ScheduledComputeNodesModel::new(
        workflow_id,
        55555,
        scheduler_config_id,
        "slurm".to_string(),
        "active".to_string(),
    );

    apis::scheduled_compute_nodes_api::create_scheduled_compute_node(config, node1)
        .expect("Failed to create node1");
    apis::scheduled_compute_nodes_api::create_scheduled_compute_node(config, node2)
        .expect("Failed to create node2");
    apis::scheduled_compute_nodes_api::create_scheduled_compute_node(config, node3)
        .expect("Failed to create node3");

    // Filter by status "active"
    let response = apis::scheduled_compute_nodes_api::list_scheduled_compute_nodes(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        Some("active"), // status filter
    )
    .expect("Failed to list scheduled compute nodes with status filter");

    let items = response.items;
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|n| n.status == "active"));
}

#[rstest]
fn test_delete_scheduled_compute_node(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow and scheduler
    let workflow = create_test_workflow(config, "test_delete_scheduled_node_workflow");
    let workflow_id = workflow.id.unwrap();

    let scheduler = create_test_slurm_scheduler(config, workflow_id, "test_scheduler");
    let scheduler_config_id = scheduler.id.unwrap();

    // Create a scheduled compute node
    let node = models::ScheduledComputeNodesModel::new(
        workflow_id,
        44444,
        scheduler_config_id,
        "slurm".to_string(),
        "pending".to_string(),
    );

    let created = apis::scheduled_compute_nodes_api::create_scheduled_compute_node(config, node)
        .expect("Failed to create scheduled compute node");
    let node_id = created.id.unwrap();

    // Delete the scheduled compute node
    let deleted = apis::scheduled_compute_nodes_api::delete_scheduled_compute_node(config, node_id)
        .expect("Failed to delete scheduled compute node");

    assert_eq!(deleted.id, Some(node_id));

    // Verify it's deleted by trying to get it (should fail or return not found)
    let get_result = apis::scheduled_compute_nodes_api::get_scheduled_compute_node(config, node_id);
    assert!(
        get_result.is_err(),
        "Expected error when getting deleted node"
    );
}

#[rstest]
fn test_scheduled_compute_node_status_transitions(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow and scheduler
    let workflow = create_test_workflow(config, "test_status_transitions_workflow");
    let workflow_id = workflow.id.unwrap();

    let scheduler = create_test_slurm_scheduler(config, workflow_id, "test_scheduler");
    let scheduler_config_id = scheduler.id.unwrap();

    // Create a scheduled compute node and test status transitions
    let node = models::ScheduledComputeNodesModel::new(
        workflow_id,
        13579,
        scheduler_config_id,
        "slurm".to_string(),
        "pending".to_string(),
    );

    let created = apis::scheduled_compute_nodes_api::create_scheduled_compute_node(config, node)
        .expect("Failed to create scheduled compute node");
    let node_id = created.id.unwrap();

    // Transition: pending -> active
    let mut updated_node = created.clone();
    updated_node.status = "active".to_string();
    let active_node = apis::scheduled_compute_nodes_api::update_scheduled_compute_node(
        config,
        node_id,
        updated_node,
    )
    .expect("Failed to update to active");
    assert_eq!(active_node.status, "active");

    // Transition: active -> complete
    let mut final_node = active_node.clone();
    final_node.status = "complete".to_string();
    let complete_node = apis::scheduled_compute_nodes_api::update_scheduled_compute_node(
        config, node_id, final_node,
    )
    .expect("Failed to update to complete");
    assert_eq!(complete_node.status, "complete");
}
