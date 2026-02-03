mod common;

use common::{ServerProcess, create_test_workflow, run_cli_command, start_server};
use rstest::rstest;
use std::io::Write;
use tempfile::NamedTempFile;
use torc::client::default_api;

// ============================================================================
// API Tests
// ============================================================================

#[rstest]
fn test_create_remote_workers_api(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "remote_workers_test");
    let workflow_id = workflow.id.unwrap();

    // Add remote workers
    let workers = vec![
        "worker1.example.com".to_string(),
        "alice@worker2.example.com".to_string(),
        "admin@192.168.1.10:2222".to_string(),
    ];

    let created = default_api::create_remote_workers(config, workflow_id, workers.clone())
        .expect("Failed to create remote workers");

    assert_eq!(created.len(), 3, "Should have created 3 workers");

    // Verify each worker was created correctly
    let created_workers: Vec<&str> = created.iter().map(|w| w.worker.as_str()).collect();
    assert!(created_workers.contains(&"worker1.example.com"));
    assert!(created_workers.contains(&"alice@worker2.example.com"));
    assert!(created_workers.contains(&"admin@192.168.1.10:2222"));

    // All workers should have the correct workflow_id
    for worker in &created {
        assert_eq!(worker.workflow_id, workflow_id);
    }
}

#[rstest]
fn test_list_remote_workers_api(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "list_workers_test");
    let workflow_id = workflow.id.unwrap();

    // Initially should have no workers
    let initial_workers =
        default_api::list_remote_workers(config, workflow_id).expect("Failed to list workers");
    assert!(
        initial_workers.is_empty(),
        "Should have no workers initially"
    );

    // Add some workers
    let workers = vec![
        "host1.example.com".to_string(),
        "host2.example.com".to_string(),
    ];
    default_api::create_remote_workers(config, workflow_id, workers)
        .expect("Failed to create workers");

    // List again
    let listed_workers =
        default_api::list_remote_workers(config, workflow_id).expect("Failed to list workers");
    assert_eq!(listed_workers.len(), 2, "Should have 2 workers");
}

#[rstest]
fn test_delete_remote_worker_api(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "delete_worker_test");
    let workflow_id = workflow.id.unwrap();

    // Add workers
    let workers = vec![
        "worker-to-delete.example.com".to_string(),
        "worker-to-keep.example.com".to_string(),
    ];
    default_api::create_remote_workers(config, workflow_id, workers)
        .expect("Failed to create workers");

    // Delete one worker
    let deleted =
        default_api::delete_remote_worker(config, workflow_id, "worker-to-delete.example.com")
            .expect("Failed to delete worker");

    assert_eq!(deleted.worker, "worker-to-delete.example.com");

    // Verify only one worker remains
    let remaining_workers =
        default_api::list_remote_workers(config, workflow_id).expect("Failed to list workers");
    assert_eq!(remaining_workers.len(), 1);
    assert_eq!(remaining_workers[0].worker, "worker-to-keep.example.com");
}

#[rstest]
fn test_create_duplicate_workers_api(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "duplicate_workers_test");
    let workflow_id = workflow.id.unwrap();

    // Add workers
    let workers = vec!["unique-worker.example.com".to_string()];
    let first_create = default_api::create_remote_workers(config, workflow_id, workers.clone())
        .expect("Failed to create workers first time");
    assert_eq!(first_create.len(), 1);

    // Try to add the same worker again - the server silently skips duplicates
    // (uses INSERT OR IGNORE), so it may return empty or the existing worker
    let _second_create = default_api::create_remote_workers(config, workflow_id, workers)
        .expect("Failed to create workers second time");

    // The important thing is that we still only have one worker in the database
    let workers =
        default_api::list_remote_workers(config, workflow_id).expect("Failed to list workers");
    assert_eq!(
        workers.len(),
        1,
        "Should still have only one worker after duplicate add"
    );
}

#[rstest]
fn test_workers_are_workflow_specific(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create two workflows
    let workflow1 = create_test_workflow(config, "workflow_specific_1");
    let workflow1_id = workflow1.id.unwrap();

    let workflow2 = create_test_workflow(config, "workflow_specific_2");
    let workflow2_id = workflow2.id.unwrap();

    // Add workers to workflow1
    let workers1 = vec!["workflow1-worker.example.com".to_string()];
    default_api::create_remote_workers(config, workflow1_id, workers1)
        .expect("Failed to create workers for workflow1");

    // Add different workers to workflow2
    let workers2 = vec!["workflow2-worker.example.com".to_string()];
    default_api::create_remote_workers(config, workflow2_id, workers2)
        .expect("Failed to create workers for workflow2");

    // Verify each workflow has its own workers
    let listed1 = default_api::list_remote_workers(config, workflow1_id)
        .expect("Failed to list workers for workflow1");
    assert_eq!(listed1.len(), 1);
    assert_eq!(listed1[0].worker, "workflow1-worker.example.com");

    let listed2 = default_api::list_remote_workers(config, workflow2_id)
        .expect("Failed to list workers for workflow2");
    assert_eq!(listed2.len(), 1);
    assert_eq!(listed2[0].worker, "workflow2-worker.example.com");
}

#[rstest]
fn test_delete_nonexistent_worker_api(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "delete_nonexistent_test");
    let workflow_id = workflow.id.unwrap();

    // Try to delete a worker that doesn't exist
    let result = default_api::delete_remote_worker(config, workflow_id, "nonexistent.example.com");

    assert!(result.is_err(), "Deleting nonexistent worker should fail");
}

#[rstest]
fn test_various_worker_formats_api(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "worker_formats_test");
    let workflow_id = workflow.id.unwrap();

    // Add workers in various formats
    let workers = vec![
        // Simple hostname
        "simple-host".to_string(),
        // FQDN
        "worker.example.com".to_string(),
        // With username
        "user@host.example.com".to_string(),
        // With port
        "host.example.com:2222".to_string(),
        // With username and port
        "user@host.example.com:2222".to_string(),
        // IPv4
        "192.168.1.100".to_string(),
        // IPv4 with port
        "192.168.1.100:22".to_string(),
        // IPv6 in brackets
        "[::1]".to_string(),
        // IPv6 with port
        "[2001:db8::1]:2222".to_string(),
    ];

    let created = default_api::create_remote_workers(config, workflow_id, workers.clone())
        .expect("Failed to create workers with various formats");

    assert_eq!(
        created.len(),
        workers.len(),
        "All workers should be created"
    );
}

// ============================================================================
// CLI Tests
// ============================================================================

#[rstest]
fn test_cli_add_workers(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "cli_add_workers_test");
    let workflow_id = workflow.id.unwrap();

    // Add workers via CLI (skip SSH check for testing with fake hostnames)
    let args = [
        "remote",
        "add-workers",
        &workflow_id.to_string(),
        "cli-worker1.example.com",
        "cli-worker2.example.com",
        "user@cli-worker3.example.com:2222",
        "--skip-ssh-check",
    ];

    let output =
        run_cli_command(&args, start_server, None).expect("Failed to run add-workers command");

    // Verify output indicates success
    assert!(
        output.contains("Added") || output.contains("worker"),
        "Output should indicate workers were added: {}",
        output
    );

    // Verify via API
    let workers =
        default_api::list_remote_workers(config, workflow_id).expect("Failed to list workers");
    assert_eq!(workers.len(), 3);
}

#[rstest]
fn test_cli_add_workers_from_file(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "cli_add_from_file_test");
    let workflow_id = workflow.id.unwrap();

    // Create a temporary worker file
    let mut worker_file = NamedTempFile::new().expect("Failed to create temp file");
    writeln!(worker_file, "# This is a comment").unwrap();
    writeln!(worker_file, "file-worker1.example.com").unwrap();
    writeln!(worker_file).unwrap(); // Empty line
    writeln!(worker_file, "user@file-worker2.example.com").unwrap();
    writeln!(worker_file, "file-worker3.example.com:2222").unwrap();
    writeln!(worker_file, "# Another comment").unwrap();
    worker_file.flush().unwrap();

    // Add workers via CLI (note: worker_file comes before workflow_id)
    // Skip SSH check for testing with fake hostnames
    let args = [
        "remote",
        "add-workers-from-file",
        worker_file.path().to_str().unwrap(),
        &workflow_id.to_string(),
        "--skip-ssh-check",
    ];

    let output = run_cli_command(&args, start_server, None)
        .expect("Failed to run add-workers-from-file command");

    // Verify output indicates success
    assert!(
        output.contains("Added") || output.contains("worker"),
        "Output should indicate workers were added: {}",
        output
    );

    // Verify via API - should have 3 workers (comments and empty lines ignored)
    let workers =
        default_api::list_remote_workers(config, workflow_id).expect("Failed to list workers");
    assert_eq!(workers.len(), 3);
}

#[rstest]
fn test_cli_list_workers(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "cli_list_workers_test");
    let workflow_id = workflow.id.unwrap();

    // Add workers via API
    let workers = vec![
        "list-worker1.example.com".to_string(),
        "list-worker2.example.com".to_string(),
    ];
    default_api::create_remote_workers(config, workflow_id, workers)
        .expect("Failed to create workers");

    // List workers via CLI
    let args = ["remote", "list-workers", &workflow_id.to_string()];

    let output =
        run_cli_command(&args, start_server, None).expect("Failed to run list-workers command");

    // Verify output contains the workers
    assert!(
        output.contains("list-worker1.example.com"),
        "Output should contain first worker: {}",
        output
    );
    assert!(
        output.contains("list-worker2.example.com"),
        "Output should contain second worker: {}",
        output
    );
}

#[rstest]
fn test_cli_list_workers_empty(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow with no workers
    let workflow = create_test_workflow(config, "cli_list_empty_test");
    let workflow_id = workflow.id.unwrap();

    // List workers via CLI
    let args = ["remote", "list-workers", &workflow_id.to_string()];

    let output =
        run_cli_command(&args, start_server, None).expect("Failed to run list-workers command");

    // Verify output indicates no workers
    assert!(
        output.contains("No") || output.contains("0 total") || output.contains("remote workers"),
        "Output should indicate no workers: {}",
        output
    );
}

#[rstest]
fn test_cli_remove_worker(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "cli_remove_worker_test");
    let workflow_id = workflow.id.unwrap();

    // Add workers via API
    let workers = vec![
        "remove-worker1.example.com".to_string(),
        "remove-worker2.example.com".to_string(),
    ];
    default_api::create_remote_workers(config, workflow_id, workers)
        .expect("Failed to create workers");

    // Remove one worker via CLI (note: worker comes before workflow_id)
    let args = [
        "remote",
        "remove-worker",
        "remove-worker1.example.com",
        &workflow_id.to_string(),
    ];

    let output =
        run_cli_command(&args, start_server, None).expect("Failed to run remove-worker command");

    // Verify output indicates success
    assert!(
        output.contains("Removed") || output.contains("remove-worker1"),
        "Output should indicate worker was removed: {}",
        output
    );

    // Verify via API
    let remaining =
        default_api::list_remote_workers(config, workflow_id).expect("Failed to list workers");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].worker, "remove-worker2.example.com");
}

#[rstest]
fn test_cli_add_workers_with_special_characters(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "cli_special_chars_test");
    let workflow_id = workflow.id.unwrap();

    // Add workers with various special characters in usernames/hostnames
    // Skip SSH check for testing with fake hostnames
    let args = [
        "remote",
        "add-workers",
        &workflow_id.to_string(),
        "user_name@host-name.example.com",
        "user.name@host.name.example.com",
        "--skip-ssh-check",
    ];

    let output = run_cli_command(&args, start_server, None)
        .expect("Failed to add workers with special chars");

    assert!(
        output.contains("Added") || output.contains("worker"),
        "Output should indicate workers were added: {}",
        output
    );

    // Verify via API
    let workers =
        default_api::list_remote_workers(config, workflow_id).expect("Failed to list workers");
    assert_eq!(workers.len(), 2);
}

// ============================================================================
// Edge Cases and Error Handling
// ============================================================================

#[rstest]
fn test_workers_deleted_with_workflow(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "cascade_delete_test");
    let workflow_id = workflow.id.unwrap();

    // Add workers
    let workers = vec!["cascade-worker.example.com".to_string()];
    default_api::create_remote_workers(config, workflow_id, workers)
        .expect("Failed to create workers");

    // Verify workers exist
    let before_delete =
        default_api::list_remote_workers(config, workflow_id).expect("Failed to list workers");
    assert_eq!(before_delete.len(), 1);

    // Delete the workflow
    default_api::delete_workflow(config, workflow_id, None).expect("Failed to delete workflow");

    // Workers should be gone with the workflow (foreign key cascade)
    // Trying to list workers for deleted workflow should fail
    let result = default_api::list_remote_workers(config, workflow_id);
    assert!(
        result.is_err(),
        "Listing workers for deleted workflow should fail"
    );
}

#[rstest]
fn test_add_many_workers(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "many_workers_test");
    let workflow_id = workflow.id.unwrap();

    // Add many workers
    let workers: Vec<String> = (0..50)
        .map(|i| format!("worker{}.example.com", i))
        .collect();

    let created = default_api::create_remote_workers(config, workflow_id, workers.clone())
        .expect("Failed to create many workers");

    assert_eq!(created.len(), 50, "Should create 50 workers");

    // Verify all are listed
    let listed =
        default_api::list_remote_workers(config, workflow_id).expect("Failed to list workers");
    assert_eq!(listed.len(), 50);
}

#[rstest]
fn test_worker_with_ipv6_addresses(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a test workflow
    let workflow = create_test_workflow(config, "ipv6_workers_test");
    let workflow_id = workflow.id.unwrap();

    // Add IPv6 workers
    let workers = vec![
        "[::1]".to_string(),
        "[2001:db8::1]".to_string(),
        "[fe80::1]:22".to_string(),
        "user@[2001:db8::2]:2222".to_string(),
    ];

    let created = default_api::create_remote_workers(config, workflow_id, workers.clone())
        .expect("Failed to create IPv6 workers");

    assert_eq!(created.len(), 4, "Should create 4 IPv6 workers");

    // Verify they can be listed
    let listed =
        default_api::list_remote_workers(config, workflow_id).expect("Failed to list workers");
    assert_eq!(listed.len(), 4);
}

// ============================================================================
// Worker File Parsing Tests (Unit Tests)
// ============================================================================

use torc::client::remote::parse_worker_content;

fn parse_test(content: &str) -> Result<Vec<torc::client::remote::WorkerEntry>, String> {
    parse_worker_content(content, "test.txt")
}

#[rstest]
fn test_parse_simple_hostname() {
    let workers = parse_test("worker1.example.com").unwrap();
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].host, "worker1.example.com");
    assert_eq!(workers[0].user, None);
    assert_eq!(workers[0].port, None);
}

#[rstest]
fn test_parse_with_user() {
    let workers = parse_test("alice@worker1.example.com").unwrap();
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].host, "worker1.example.com");
    assert_eq!(workers[0].user, Some("alice".to_string()));
    assert_eq!(workers[0].port, None);
}

#[rstest]
fn test_parse_with_port() {
    let workers = parse_test("worker1.example.com:2222").unwrap();
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].host, "worker1.example.com");
    assert_eq!(workers[0].user, None);
    assert_eq!(workers[0].port, Some(2222));
}

#[rstest]
fn test_parse_full_format() {
    let workers = parse_test("alice@worker1.example.com:2222").unwrap();
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].host, "worker1.example.com");
    assert_eq!(workers[0].user, Some("alice".to_string()));
    assert_eq!(workers[0].port, Some(2222));
}

#[rstest]
fn test_parse_ipv4() {
    let workers = parse_test("192.168.1.10").unwrap();
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].host, "192.168.1.10");
}

#[rstest]
fn test_parse_ipv4_with_port() {
    let workers = parse_test("192.168.1.10:22").unwrap();
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].host, "192.168.1.10");
    assert_eq!(workers[0].port, Some(22));
}

#[rstest]
fn test_parse_ipv6_bracketed() {
    let workers = parse_test("[::1]").unwrap();
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].host, "::1");
    assert_eq!(workers[0].port, None);
}

#[rstest]
fn test_parse_ipv6_bracketed_with_port() {
    let workers = parse_test("[::1]:2222").unwrap();
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].host, "::1");
    assert_eq!(workers[0].port, Some(2222));
}

#[rstest]
fn test_parse_comments_and_blank_lines() {
    let content = r#"
# This is a comment
worker1.example.com

# Another comment
worker2.example.com
"#;
    let workers = parse_test(content).unwrap();
    assert_eq!(workers.len(), 2);
    assert_eq!(workers[0].host, "worker1.example.com");
    assert_eq!(workers[1].host, "worker2.example.com");
}

#[rstest]
fn test_parse_multiple_workers() {
    let content = r#"
worker1.example.com
alice@worker2.example.com:2222
192.168.1.10
"#;
    let workers = parse_test(content).unwrap();
    assert_eq!(workers.len(), 3);
}

#[rstest]
fn test_parse_empty_file() {
    let result = parse_test("");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("no valid entries"));
}

#[rstest]
fn test_parse_only_comments() {
    let content = r#"
# Comment 1
# Comment 2
"#;
    let result = parse_test(content);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("no valid entries"));
}

#[rstest]
fn test_parse_duplicate_host() {
    let content = r#"
worker1.example.com
alice@worker1.example.com:2222
"#;
    let result = parse_test(content);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Duplicate host"));
}

#[rstest]
fn test_parse_empty_user() {
    let result = parse_test("@worker1.example.com");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Empty username"));
}

#[rstest]
fn test_parse_empty_host() {
    let result = parse_test("alice@");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Empty hostname"));
}

#[rstest]
fn test_parse_invalid_port() {
    let result = parse_test("worker1.example.com:abc");
    // This should be treated as part of the hostname since it's not numeric
    let workers = result.unwrap();
    assert_eq!(workers[0].host, "worker1.example.com:abc");
}

#[rstest]
fn test_parse_port_out_of_range() {
    let result = parse_test("worker1.example.com:99999");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid port"));
}

#[rstest]
fn test_parse_whitespace_trimming() {
    let content = "  worker1.example.com  ";
    let workers = parse_test(content).unwrap();
    assert_eq!(workers[0].host, "worker1.example.com");
}
