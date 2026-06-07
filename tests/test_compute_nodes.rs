mod common;

use common::{ServerProcess, create_test_compute_node, create_test_workflow, start_server};
use rstest::rstest;
use torc::client::apis;
use torc::models;

fn create_named_compute_node(
    config: &torc::client::Configuration,
    workflow_id: i64,
    host: &str,
    is_active: bool,
) {
    let mut node = models::ComputeNodeModel::new(
        workflow_id,
        host.to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        8,
        16.0,
        0,
        1,
        "local".to_string(),
        None,
    );
    node.is_active = Some(is_active);
    apis::compute_nodes_api::create_compute_node(config, node)
        .expect("Failed to create compute node");
}

/// Server-side filtering by `is_active`, including the total count (the count
/// query previously failed to bind this parameter).
#[rstest]
fn test_list_compute_nodes_active_filter(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "test_compute_nodes_active_filter");
    let workflow_id = workflow.id.unwrap();

    create_named_compute_node(config, workflow_id, "active-host", true);
    create_named_compute_node(config, workflow_id, "inactive-host", false);

    let list = |active: bool| {
        apis::compute_nodes_api::list_compute_nodes(
            config,
            workflow_id,
            None,
            None,
            None,
            None,
            None,
            Some(active),
            None,
        )
        .expect("Failed to list compute nodes by active state")
    };

    let active = list(true);
    assert_eq!(active.total_count, 1);
    assert_eq!(active.items.len(), 1);
    assert_eq!(active.items[0].hostname, "active-host");

    let inactive = list(false);
    assert_eq!(inactive.total_count, 1);
    assert_eq!(inactive.items.len(), 1);
    assert_eq!(inactive.items[0].hostname, "inactive-host");
}

/// Listing compute nodes filtered by hostname matches a substring and returns a
/// correct total count (covers the count-query bindings for the hostname param).
#[rstest]
fn test_list_compute_nodes_hostname_substring(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "test_compute_nodes_hostname_filter");
    let workflow_id = workflow.id.unwrap();

    create_named_compute_node(config, workflow_id, "node-alpha-01", true);
    create_named_compute_node(config, workflow_id, "node-beta-02", true);

    // Substring "alpha" matches only the first node.
    let alpha = apis::compute_nodes_api::list_compute_nodes(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        Some("alpha"),
        None,
        None,
    )
    .expect("Failed to list compute nodes by hostname");
    assert_eq!(alpha.total_count, 1);
    assert_eq!(alpha.items.len(), 1);
    assert_eq!(alpha.items[0].hostname, "node-alpha-01");

    // Substring "node-" matches both.
    let both = apis::compute_nodes_api::list_compute_nodes(
        config,
        workflow_id,
        None,
        None,
        None,
        None,
        Some("node-"),
        None,
        None,
    )
    .expect("Failed to list compute nodes by hostname");
    assert_eq!(both.total_count, 2);
    assert_eq!(both.items.len(), 2);
}

#[rstest]
fn test_compute_node_resource_summary_round_trip(start_server: &ServerProcess) {
    let config = &start_server.config;
    let workflow = create_test_workflow(config, "test_compute_node_resource_summary");
    let workflow_id = workflow.id.unwrap();
    let mut compute_node = create_test_compute_node(config, workflow_id);
    let compute_node_id = compute_node.id.unwrap();

    compute_node.sample_count = Some(3);
    compute_node.peak_cpu_percent = Some(87.5);
    compute_node.avg_cpu_percent = Some(42.25);
    compute_node.peak_memory_bytes = Some(4_294_967_296);
    compute_node.avg_memory_bytes = Some(2_147_483_648);

    let updated =
        apis::compute_nodes_api::update_compute_node(config, compute_node_id, compute_node)
            .expect("Failed to update compute node");
    assert_eq!(updated.sample_count, Some(3));
    assert_eq!(updated.peak_cpu_percent, Some(87.5));
    assert_eq!(updated.avg_cpu_percent, Some(42.25));
    assert_eq!(updated.peak_memory_bytes, Some(4_294_967_296));
    assert_eq!(updated.avg_memory_bytes, Some(2_147_483_648));

    let fetched = apis::compute_nodes_api::get_compute_node(config, compute_node_id)
        .expect("Failed to get compute node");
    assert_eq!(fetched.sample_count, Some(3));
    assert_eq!(fetched.peak_cpu_percent, Some(87.5));
    assert_eq!(fetched.avg_cpu_percent, Some(42.25));
    assert_eq!(fetched.peak_memory_bytes, Some(4_294_967_296));
    assert_eq!(fetched.avg_memory_bytes, Some(2_147_483_648));

    let listed = apis::compute_nodes_api::list_compute_nodes(
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
    .expect("Failed to list compute nodes");
    let listed_node = listed
        .items
        .iter()
        .find(|node| node.id == Some(compute_node_id))
        .expect("Updated compute node missing from list response");
    assert_eq!(listed_node.sample_count, Some(3));
    assert_eq!(listed_node.peak_cpu_percent, Some(87.5));
    assert_eq!(listed_node.avg_cpu_percent, Some(42.25));
    assert_eq!(listed_node.peak_memory_bytes, Some(4_294_967_296));
    assert_eq!(listed_node.avg_memory_bytes, Some(2_147_483_648));
}
