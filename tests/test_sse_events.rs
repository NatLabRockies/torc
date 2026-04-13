mod common;

use common::{
    AccessControlServerProcess, ServerProcess, start_server, start_server_with_access_control,
};
use reqwest::StatusCode;
use reqwest::blocking::Client;
use rstest::rstest;
use torc::client::{Configuration, apis};
use torc::models;

/// Helper to create a config with basic auth
fn config_with_auth(base_config: &Configuration, username: &str) -> Configuration {
    Configuration {
        base_path: base_config.base_path.clone(),
        user_agent: base_config.user_agent.clone(),
        client: base_config.client.clone(),
        basic_auth: Some((
            username.to_string(),
            Some("correct horse battery staple".to_string()),
        )),
        oauth_access_token: None,
        bearer_access_token: None,
        api_key: None,
        tls: Default::default(),
        cookie_header: None,
    }
}

/// Create a workflow with a specific user
fn create_workflow_with_user(
    config: &Configuration,
    name: &str,
    user: &str,
) -> models::WorkflowModel {
    let workflow = models::WorkflowModel::new(name.to_string(), user.to_string());
    apis::workflows_api::create_workflow(config, workflow).expect("Failed to create workflow")
}

// ============================================================================
// SSE Events Stream Tests (without access control)
// ============================================================================

#[rstest]
fn test_sse_stream_endpoint_exists(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow
    let workflow = create_workflow_with_user(config, "sse-test-workflow", "test_user");
    let workflow_id = workflow.id.unwrap();

    // Try to connect to SSE endpoint
    let client = Client::new();
    let url = format!(
        "{}/workflows/{}/events/stream",
        config.base_path, workflow_id
    );

    let response = client
        .get(&url)
        .header("Accept", "text/event-stream")
        .timeout(std::time::Duration::from_secs(2))
        .send();

    // The request should succeed (200 OK) - it may timeout on the body read
    // since it's a streaming response, but the initial response should be OK
    match response {
        Ok(resp) => {
            assert_eq!(
                resp.status(),
                StatusCode::OK,
                "SSE endpoint should return 200 OK"
            );
            assert!(
                resp.headers()
                    .get("content-type")
                    .map(|v| v.to_str().unwrap_or(""))
                    .unwrap_or("")
                    .contains("text/event-stream"),
                "Content-Type should be text/event-stream"
            );
        }
        Err(e) => {
            // Timeout is acceptable since we're not reading the stream
            if !e.is_timeout() {
                panic!("Unexpected error connecting to SSE endpoint: {:?}", e);
            }
        }
    }
}

#[rstest]
fn test_sse_stream_404_for_nonexistent_workflow(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Try to connect to SSE endpoint for a non-existent workflow
    let client = Client::new();
    let url = format!("{}/workflows/999999/events/stream", config.base_path);

    let response = client
        .get(&url)
        .header("Accept", "text/event-stream")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .expect("Failed to send request");

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "SSE endpoint should return 404 for non-existent workflow"
    );
}

#[rstest]
fn test_sse_stream_receives_event(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow
    let workflow = create_workflow_with_user(config, "sse-read-workflow", "test_user");
    let workflow_id = workflow.id.unwrap();

    // Start SSE connection asynchronously (in a separate thread)
    let client = Client::new();
    let url = format!(
        "{}/workflows/{}/events/stream",
        config.base_path, workflow_id
    );

    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let response = client
            .get(&url)
            .header("Accept", "text/event-stream")
            .send()
            .expect("Failed to connect to SSE");

        use std::io::BufRead;
        let reader = std::io::BufReader::new(response);
        for line in reader.lines() {
            let line = line.expect("Failed to read line");
            if line.starts_with("event: ") {
                tx.send(line).expect("Failed to send event");
                return; // Exit after first event for this test
            }
        }
    });

    // Trigger an event by creating a compute node
    // Wait a bit for connection to be established
    std::thread::sleep(std::time::Duration::from_millis(500));

    let compute_node = models::ComputeNodeModel::new(
        workflow_id,
        "sse-test-host".to_string(),
        12345,
        chrono::Utc::now().to_rfc3339(),
        4,
        8.0,
        0,
        1,
        "local".to_string(),
        None,
    );
    apis::compute_nodes_api::create_compute_node(config, compute_node)
        .expect("Failed to create compute node");

    // Wait for event
    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(line) => {
            assert!(line.contains("event: compute_node_started"));
        }
        Err(_) => {
            panic!("Timed out waiting for SSE event");
        }
    }
}

// ============================================================================
// SSE Events Stream Tests (with access control)
// ============================================================================

#[rstest]
fn test_sse_stream_authorized_user(start_server_with_access_control: &AccessControlServerProcess) {
    let config = &start_server_with_access_control.config;

    // Create a workflow owned by alice
    let alice_config = config_with_auth(config, "alice");
    let workflow = create_workflow_with_user(&alice_config, "alice-sse-workflow", "alice");
    let workflow_id = workflow.id.unwrap();

    // Alice should be able to access SSE endpoint for her own workflow
    let client = Client::new();
    let url = format!(
        "{}/workflows/{}/events/stream",
        config.base_path, workflow_id
    );

    let response = client
        .get(&url)
        .basic_auth("alice", Some("correct horse battery staple"))
        .header("Accept", "text/event-stream")
        .timeout(std::time::Duration::from_secs(2))
        .send();

    match response {
        Ok(resp) => {
            assert_eq!(
                resp.status(),
                StatusCode::OK,
                "Owner should be able to access SSE endpoint"
            );
        }
        Err(e) => {
            // Timeout is acceptable since we're not reading the stream
            if !e.is_timeout() {
                panic!("Unexpected error for authorized user: {:?}", e);
            }
        }
    }
}

#[rstest]
fn test_sse_stream_unauthorized_user(
    start_server_with_access_control: &AccessControlServerProcess,
) {
    let config = &start_server_with_access_control.config;

    // Create a workflow owned by alice
    let alice_config = config_with_auth(config, "alice");
    let workflow = create_workflow_with_user(&alice_config, "alice-private-workflow", "alice");
    let workflow_id = workflow.id.unwrap();

    // Bob should NOT be able to access SSE endpoint for alice's workflow
    let client = Client::new();
    let url = format!(
        "{}/workflows/{}/events/stream",
        config.base_path, workflow_id
    );

    let response = client
        .get(&url)
        .basic_auth("bob", Some("correct horse battery staple"))
        .header("Accept", "text/event-stream")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .expect("Failed to send request");

    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "Unauthorized user should get 403 Forbidden"
    );
}

#[rstest]
fn test_sse_stream_shared_workflow_access(
    start_server_with_access_control: &AccessControlServerProcess,
) {
    let config = &start_server_with_access_control.config;

    // Create a group and add bob to it
    let group = models::AccessGroupModel {
        id: None,
        name: "sse-test-group".to_string(),
        description: Some("Test group for SSE".to_string()),
        created_at: None,
    };
    let group = apis::access_control_api::create_access_group(config, group)
        .expect("Failed to create group");
    let group_id = group.id.unwrap();

    // Add bob to the group
    let membership = models::UserGroupMembershipModel {
        id: None,
        user_name: "bob".to_string(),
        group_id,
        role: "member".to_string(),
        created_at: None,
    };
    apis::access_control_api::add_user_to_group(config, group_id, membership)
        .expect("Failed to add bob to group");

    // Create a workflow owned by alice
    let alice_config = config_with_auth(config, "alice");
    let workflow = create_workflow_with_user(&alice_config, "shared-sse-workflow", "alice");
    let workflow_id = workflow.id.unwrap();

    // Share the workflow with the group
    apis::access_control_api::add_workflow_to_group(config, workflow_id, group_id)
        .expect("Failed to share workflow");

    // Bob (group member) should now be able to access SSE endpoint
    let client = Client::new();
    let url = format!(
        "{}/workflows/{}/events/stream",
        config.base_path, workflow_id
    );

    let response = client
        .get(&url)
        .basic_auth("bob", Some("correct horse battery staple"))
        .header("Accept", "text/event-stream")
        .timeout(std::time::Duration::from_secs(2))
        .send();

    match response {
        Ok(resp) => {
            assert_eq!(
                resp.status(),
                StatusCode::OK,
                "Group member should be able to access shared workflow's SSE endpoint"
            );
        }
        Err(e) => {
            // Timeout is acceptable since we're not reading the stream
            if !e.is_timeout() {
                panic!("Unexpected error for group member: {:?}", e);
            }
        }
    }

    // Carol (not in group) should still NOT be able to access
    let response = client
        .get(&url)
        .basic_auth("carol", Some("correct horse battery staple"))
        .header("Accept", "text/event-stream")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .expect("Failed to send request");

    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "Non-group member should get 403 Forbidden"
    );
}
