mod common;

use common::{ServerProcess, create_test_compute_node, create_test_workflow, start_server};
use rstest::rstest;
use torc::client::apis;

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
