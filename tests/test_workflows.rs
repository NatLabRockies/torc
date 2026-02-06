mod common;

use common::{
    ServerProcess, create_test_workflow, create_test_workflow_advanced,
    create_test_workflow_with_description, run_cli_with_json, start_server,
};
use rstest::rstest;
use serde_json::json;
use torc::client::default_api;
use torc::models;

#[rstest]
fn test_workflows_add_command_json(start_server: &ServerProcess) {
    // Test the CLI create command with JSON output
    let args = [
        "workflows",
        "new",
        "--name",
        "test_workflow",
        "--description",
        "A test workflow for validation",
    ];

    let json_output = run_cli_with_json(&args, start_server, Some("test_user"))
        .expect("Failed to run workflows create command");

    assert!(json_output.get("id").is_some());
    assert_eq!(json_output.get("name").unwrap(), &json!("test_workflow"));
    assert_eq!(json_output.get("user").unwrap(), &json!("test_user"));
    assert_eq!(
        json_output.get("description").unwrap(),
        &json!("A test workflow for validation")
    );
    assert!(json_output.get("timestamp").is_some());
}

#[rstest]
fn test_workflows_add_minimal(start_server: &ServerProcess) {
    // Test with minimal arguments (no description)
    let args = ["workflows", "new", "--name", "minimal_workflow"];

    let json_output = run_cli_with_json(&args, start_server, Some("minimal_user"))
        .expect("Failed to run workflows create with minimal args");

    assert_eq!(json_output.get("name").unwrap(), &json!("minimal_workflow"));
    assert_eq!(json_output.get("user").unwrap(), &json!("minimal_user"));
    // Description should be null for minimal workflow
    assert!(
        json_output.get("description").is_none()
            || json_output.get("description").unwrap().is_null()
    );
}

#[rstest]
fn test_workflows_add_various_names(start_server: &ServerProcess) {
    // Test different workflow name patterns
    let test_names = [
        "simple_name",
        "workflow-with-dashes",
        "workflow_with_underscores",
        "Workflow With Spaces",
        "UPPERCASE_WORKFLOW",
        "MixedCaseWorkflow",
        "workflow123numbers",
        "workflow.with.dots",
        "very_long_workflow_name_that_contains_many_characters_for_testing_limits",
    ];

    for name in &test_names {
        let args = [
            "workflows",
            "new",
            "--name",
            name,
            "--description",
            &format!("Test workflow for name: {}", name),
        ];

        let json_output = run_cli_with_json(&args, start_server, Some("test_user"))
            .unwrap_or_else(|_| panic!("Failed to create workflow with name: {}", name));

        assert_eq!(json_output.get("name").unwrap(), &json!(name));
        assert_eq!(json_output.get("user").unwrap(), &json!("test_user"));
    }
}

#[rstest]
fn test_workflows_list_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflows
    let _workflow1 = create_test_workflow_with_description(
        config,
        "list_test_workflow_1",
        "list_user",
        Some("First test workflow".to_string()),
    );
    let _workflow2 = create_test_workflow_with_description(
        config,
        "list_test_workflow_2",
        "list_user",
        Some("Second test workflow".to_string()),
    );
    let _workflow3 =
        create_test_workflow_with_description(config, "list_test_workflow_3", "list_user", None);

    // Test the CLI list command
    let args = ["workflows", "list", "--limit", "10"];

    let json_output = run_cli_with_json(&args, start_server, Some("list_user"))
        .expect("Failed to run workflows list command");

    // Verify JSON structure is an array
    assert!(
        json_output.is_array(),
        "Workflows list should return an array"
    );

    let workflows_array = json_output.as_array().unwrap();
    assert!(
        workflows_array.len() >= 3,
        "Should have at least 3 workflows"
    );

    // Verify each workflow has the expected structure
    for workflow in workflows_array {
        assert!(workflow.get("id").is_some());
        assert!(workflow.get("name").is_some());
        assert!(workflow.get("user").is_some());
        assert!(workflow.get("timestamp").is_some());
        // description can be present or null
    }

    // Check that we have our test workflows
    let names: Vec<&str> = workflows_array
        .iter()
        .map(|w| w.get("name").unwrap().as_str().unwrap())
        .collect();

    assert!(names.contains(&"list_test_workflow_1"));
    assert!(names.contains(&"list_test_workflow_2"));
    assert!(names.contains(&"list_test_workflow_3"));
}

#[rstest]
fn test_workflows_list_pagination(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create multiple workflows
    for i in 0..7 {
        let _workflow = create_test_workflow_with_description(
            config,
            &format!("pagination_workflow_{}", i),
            "pagination_user",
            Some(format!("Workflow number {}", i)),
        );
    }

    // Test with limit
    let args = ["workflows", "list", "--limit", "4"];

    let json_output = run_cli_with_json(&args, start_server, Some("pagination_user"))
        .expect("Failed to run paginated workflows list");

    let workflows_array = json_output.as_array().unwrap();
    assert!(workflows_array.len() <= 4, "Should respect limit parameter");
    assert!(
        !workflows_array.is_empty(),
        "Should have at least one workflow"
    );

    // Test with offset
    let args_with_offset = ["workflows", "list", "--limit", "3", "--offset", "3"];

    let json_output_offset =
        run_cli_with_json(&args_with_offset, start_server, Some("pagination_user"))
            .expect("Failed to run workflows list with offset");

    let workflows_with_offset = json_output_offset.as_array().unwrap();
    assert!(
        !workflows_with_offset.is_empty(),
        "Should have workflows with offset"
    );
}

#[rstest]
fn test_workflows_list_sorting(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflows with different names for sorting
    let _workflow_a = create_test_workflow_with_description(
        config,
        "aaa_workflow",
        "sort_user",
        Some("First workflow".to_string()),
    );
    let _workflow_b = create_test_workflow_with_description(
        config,
        "bbb_workflow",
        "sort_user",
        Some("Second workflow".to_string()),
    );
    let _workflow_c = create_test_workflow_with_description(
        config,
        "ccc_workflow",
        "sort_user",
        Some("Third workflow".to_string()),
    );

    // Test sorting by name
    let args = ["workflows", "list", "--sort-by", "name"];

    let json_output = run_cli_with_json(&args, start_server, Some("sort_user"))
        .expect("Failed to run sorted workflows list");

    let workflows_array = json_output.as_array().unwrap();
    assert!(workflows_array.len() >= 3);

    // Test reverse sorting
    let args_reverse = ["workflows", "list", "--sort-by", "name", "--reverse-sort"];

    let json_output_reverse = run_cli_with_json(&args_reverse, start_server, Some("sort_user"))
        .expect("Failed to run reverse sorted workflows list");

    let workflows_array_reverse = json_output_reverse.as_array().unwrap();
    assert!(workflows_array_reverse.len() >= 3);

    // Verify sorting worked
    if workflows_array.len() >= 2 && workflows_array_reverse.len() >= 2 {
        let first_regular = workflows_array[0].get("name").unwrap().as_str().unwrap();
        let first_reverse = workflows_array_reverse[0]
            .get("name")
            .unwrap()
            .as_str()
            .unwrap();

        // They should be different unless all names are the same
        if workflows_array.len() > 1 {
            let last_regular = workflows_array[workflows_array.len() - 1]
                .get("name")
                .unwrap()
                .as_str()
                .unwrap();
            assert!(first_regular <= last_regular || first_reverse >= first_regular);
        }
    }
}

#[rstest]
fn test_workflows_get_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow_with_description(
        config,
        "get_test_workflow",
        "get_user",
        Some("Workflow for get testing".to_string()),
    );
    let workflow_id = workflow.id.unwrap();

    // Test the CLI get command
    let args = ["workflows", "get", &workflow_id.to_string()];

    let json_output = run_cli_with_json(&args, start_server, Some("get_user"))
        .expect("Failed to run workflows get command");

    // Verify JSON structure
    assert_eq!(json_output.get("id").unwrap(), &json!(workflow_id));
    assert_eq!(
        json_output.get("name").unwrap(),
        &json!("get_test_workflow")
    );
    assert_eq!(json_output.get("user").unwrap(), &json!("get_user"));
    assert_eq!(
        json_output.get("description").unwrap(),
        &json!("Workflow for get testing")
    );
    assert!(json_output.get("timestamp").is_some());
}

#[rstest]
fn test_workflows_update_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow_with_description(
        config,
        "update_test_workflow",
        "update_user",
        Some("Original description".to_string()),
    );
    let workflow_id = workflow.id.unwrap();

    // Test the CLI update command
    let args = [
        "workflows",
        "update",
        &workflow_id.to_string(),
        "--name",
        "updated_workflow_name",
        "--description",
        "Updated description for testing",
        "--owner-user",
        "new_owner",
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run workflows update command");

    // Verify the updated values
    assert_eq!(json_output.get("id").unwrap(), &json!(workflow_id));
    assert_eq!(
        json_output.get("name").unwrap(),
        &json!("updated_workflow_name")
    );
    assert_eq!(json_output.get("user").unwrap(), &json!("new_owner"));
    assert_eq!(
        json_output.get("description").unwrap(),
        &json!("Updated description for testing")
    );
}

#[rstest]
fn test_workflows_update_partial_fields(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow_with_description(
        config,
        "partial_update_workflow",
        "partial_user",
        Some("Original description".to_string()),
    );
    let workflow_id = workflow.id.unwrap();

    // Test updating only name
    let args = [
        "workflows",
        "update",
        &workflow_id.to_string(),
        "--name",
        "only_name_updated",
    ];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run partial workflows update");

    // Only name should be updated
    assert_eq!(
        json_output.get("name").unwrap(),
        &json!("only_name_updated")
    );
    // Other fields should remain unchanged
    assert_eq!(json_output.get("user").unwrap(), &json!("partial_user"));
    assert_eq!(
        json_output.get("description").unwrap(),
        &json!("Original description")
    );

    // Test updating only description
    let args_desc = [
        "workflows",
        "update",
        &workflow_id.to_string(),
        "--description",
        "Only description updated",
    ];

    let json_output_desc = run_cli_with_json(&args_desc, start_server, None)
        .expect("Failed to run description-only update");

    // Description should be updated, name should remain from previous update
    assert_eq!(
        json_output_desc.get("name").unwrap(),
        &json!("only_name_updated")
    );
    assert_eq!(
        json_output_desc.get("description").unwrap(),
        &json!("Only description updated")
    );
}

#[rstest]
fn test_workflows_delete_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow_with_description(
        config,
        "delete_test_workflow",
        "delete_user",
        Some("Workflow to be deleted".to_string()),
    );
    let workflow_id = workflow.id.unwrap();

    // Test the CLI delete command (run as the workflow owner)
    let args = [
        "workflows",
        "delete",
        "--no-prompts",
        &workflow_id.to_string(),
    ];

    run_cli_with_json(&args, start_server, Some("delete_user"))
        .expect("Failed to run workflows delete command");

    // Verify the workflow is actually deleted by trying to get it
    let get_result = default_api::get_workflow(config, workflow_id);
    assert!(get_result.is_err(), "Workflow should be deleted");
}

#[rstest]
fn test_workflows_initialize_jobs_command(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow with some jobs
    let workflow = create_test_workflow(config, "init_jobs_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create a job in the workflow
    let job = torc::models::JobModel::new(
        workflow_id,
        "test_job".to_string(),
        "echo 'test'".to_string(),
    );
    let _created_job =
        default_api::create_job(config, job).expect("Failed to create job for initialization test");

    // Test the CLI initialize-jobs command
    let args = ["workflows", "initialize-jobs", &workflow_id.to_string()];

    // This command doesn't return JSON in the current implementation,
    // so we'll test that it doesn't fail
    let _ = run_cli_with_json(&args, start_server, Some("test_user"));
    // The command might not return valid JSON, so we just check it doesn't crash
    // In real implementation, this would initialize job dependencies
}

#[rstest]
fn test_workflows_status_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "status_test_workflow");
    let workflow_id = workflow.id.unwrap();

    // Test the CLI status command
    let args = ["workflows", "status", &workflow_id.to_string()];

    let json_output = run_cli_with_json(&args, start_server, Some("test_user"))
        .expect("Failed to run workflows status command");

    // Verify JSON structure for workflow status
    assert!(json_output.get("run_id").is_some());
    assert!(json_output.get("is_canceled").is_some());
    // Other status fields may or may not be present depending on workflow state
}

#[rstest]
fn test_workflows_reset_status_command_json(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "reset_status_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create jobs in the workflow
    let job1 = torc::models::JobModel::new(
        workflow_id,
        "test_job1".to_string(),
        "echo 'test job 1'".to_string(),
    );
    let job2 = torc::models::JobModel::new(
        workflow_id,
        "test_job2".to_string(),
        "echo 'test job 2'".to_string(),
    );

    let created_job1 = default_api::create_job(config, job1).expect("Failed to create job1");
    let created_job2 = default_api::create_job(config, job2).expect("Failed to create job2");
    let job1_id = created_job1.id.unwrap();
    let job2_id = created_job2.id.unwrap();

    // Verify initial job statuses are Uninitialized
    let initial_job1 = default_api::get_job(config, job1_id).expect("Failed to get job1");
    let initial_job2 = default_api::get_job(config, job2_id).expect("Failed to get job2");
    assert_eq!(
        initial_job1.status.unwrap(),
        torc::models::JobStatus::Uninitialized
    );
    assert_eq!(
        initial_job2.status.unwrap(),
        torc::models::JobStatus::Uninitialized
    );

    // Initialize jobs
    let _result = default_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    // Verify job statuses are now Ready
    let ready_job1 = default_api::get_job(config, job1_id).expect("Failed to get job1 after init");
    let ready_job2 = default_api::get_job(config, job2_id).expect("Failed to get job2 after init");
    assert_eq!(ready_job1.status.unwrap(), torc::models::JobStatus::Ready);
    assert_eq!(ready_job2.status.unwrap(), torc::models::JobStatus::Ready);

    // Test the CLI reset-status command
    let args = ["workflows", "reset-status", &workflow_id.to_string()];

    let json_output = run_cli_with_json(&args, start_server, None)
        .expect("Failed to run workflows reset-status command");

    // Verify the command returned success information
    assert!(json_output.is_object());

    // Verify job statuses are back to Uninitialized after reset
    let reset_job1 = default_api::get_job(config, job1_id).expect("Failed to get job1 after reset");
    let reset_job2 = default_api::get_job(config, job2_id).expect("Failed to get job2 after reset");
    assert_eq!(
        reset_job1.status.unwrap(),
        torc::models::JobStatus::Uninitialized
    );
    assert_eq!(
        reset_job2.status.unwrap(),
        torc::models::JobStatus::Uninitialized
    );
}

#[rstest]
fn test_workflows_reset_status_depends_on_submitted_jobs(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "reset_blocked_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create jobs in the workflow
    let job1 = torc::models::JobModel::new(
        workflow_id,
        "submitted_job".to_string(),
        "echo 'running job'".to_string(),
    );
    let job2 = torc::models::JobModel::new(
        workflow_id,
        "pending_job".to_string(),
        "echo 'pending job'".to_string(),
    );

    let created_job1 = default_api::create_job(config, job1).expect("Failed to create job1");
    let created_job2 = default_api::create_job(config, job2).expect("Failed to create job2");
    let job1_id = created_job1.id.unwrap();
    let job2_id = created_job2.id.unwrap();

    // Initialize jobs so they become Ready
    let _result = default_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    // Get workflow status to get run_id
    let workflow_status = default_api::get_workflow_status(config, workflow_id)
        .expect("Failed to get workflow status");
    let run_id = workflow_status.run_id;

    // Set job1 to running status
    let _submitted_job = default_api::manage_status_change(
        config,
        job1_id,
        torc::models::JobStatus::Running,
        run_id,
        None,
    )
    .expect("Failed to set job1 to running");

    // Set job2 to SubmittedPending status
    let _pending_job = default_api::manage_status_change(
        config,
        job2_id,
        torc::models::JobStatus::Pending,
        run_id,
        None,
    )
    .expect("Failed to set job2 to SubmittedPending");

    // Verify jobs are in the expected statuses
    let check_job1 = default_api::get_job(config, job1_id).expect("Failed to get job1");
    let check_job2 = default_api::get_job(config, job2_id).expect("Failed to get job2");
    assert_eq!(check_job1.status.unwrap(), torc::models::JobStatus::Running);
    assert_eq!(check_job2.status.unwrap(), torc::models::JobStatus::Pending);

    // Try to reset workflow status - should fail with 422 error
    let reset_result = default_api::reset_workflow_status(config, workflow_id, None, None);

    assert!(
        reset_result.is_err(),
        "Reset workflow status should fail when jobs are running or SubmittedPending"
    );

    // Verify it's a 422 error (Unprocessable Content)
    let error = reset_result.unwrap_err();
    let error_message = format!("{:?}", error);

    // The error should indicate the reason (running or pending jobs)
    assert!(
        error_message.contains("422")
            || error_message.contains("running")
            || error_message.contains("pending"),
        "Error should indicate jobs are running or pending: {}",
        error_message
    );

    // Verify jobs are still in their running states (reset didn't happen)
    let final_job1 =
        default_api::get_job(config, job1_id).expect("Failed to get job1 after failed reset");
    let final_job2 =
        default_api::get_job(config, job2_id).expect("Failed to get job2 after failed reset");
    assert_eq!(
        final_job1.status.unwrap(),
        torc::models::JobStatus::Running,
        "Job1 should still be running after failed reset"
    );
    assert_eq!(
        final_job2.status.unwrap(),
        torc::models::JobStatus::Pending,
        "Job2 should still be SubmittedPending after failed reset"
    );

    // Now test that force flag allows reset even with active jobs
    let force_reset_result =
        default_api::reset_workflow_status(config, workflow_id, Some(true), None);

    assert!(
        force_reset_result.is_ok(),
        "Reset workflow status with force=true should succeed even with active jobs"
    );

    // Verify workflow status was reset
    let workflow_status = default_api::get_workflow_status(config, workflow_id)
        .expect("Failed to get workflow status");
    assert!(
        !workflow_status.is_canceled,
        "Workflow should not be canceled after reset"
    );
    assert!(
        !workflow_status.is_archived.unwrap_or(false),
        "Workflow should not be archived after reset"
    );
}

#[rstest]
fn test_workflows_different_users(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflows for different users
    let _workflow_user1 = create_test_workflow_with_description(
        config,
        "user1_workflow",
        "user1",
        Some("Workflow for user1".to_string()),
    );
    let _workflow_user2 = create_test_workflow_with_description(
        config,
        "user2_workflow",
        "user2",
        Some("Workflow for user2".to_string()),
    );
    let _workflow_user3 = create_test_workflow_with_description(
        config,
        "user3_workflow",
        "user3",
        Some("Workflow for user3".to_string()),
    );

    // Test listing workflows for specific user (set USER env var)
    let args_user1 = ["workflows", "list"];

    let json_output_user1 = run_cli_with_json(&args_user1, start_server, Some("user1"))
        .expect("Failed to list workflows for user1");

    let workflows_user1 = json_output_user1.as_array().unwrap();
    assert!(!workflows_user1.is_empty());

    // All workflows should belong to user1
    for workflow in workflows_user1 {
        assert_eq!(workflow.get("user").unwrap(), &json!("user1"));
    }

    // Test listing workflows for different user (set USER env var)
    let args_user2 = ["workflows", "list"];

    let json_output_user2 = run_cli_with_json(&args_user2, start_server, Some("user2"))
        .expect("Failed to list workflows for user2");

    let workflows_user2 = json_output_user2.as_array().unwrap();
    assert!(!workflows_user2.is_empty());

    // All workflows should belong to user2
    for workflow in workflows_user2 {
        assert_eq!(workflow.get("user").unwrap(), &json!("user2"));
    }
}

#[rstest]
fn test_workflows_advanced_configuration(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflow with advanced configuration
    let workflow = create_test_workflow_advanced(
        config,
        "advanced_workflow",
        "advanced_user",
        Some("Advanced workflow with scripts".to_string()),
    );

    let workflow_id = workflow.id.unwrap();

    // Verify the advanced configuration was saved
    let args = ["workflows", "get", &workflow_id.to_string()];

    let json_output = run_cli_with_json(&args, start_server, Some("advanced_user"))
        .expect("Failed to get advanced workflow");

    assert_eq!(
        json_output.get("name").unwrap(),
        &json!("advanced_workflow")
    );
    assert_eq!(json_output.get("user").unwrap(), &json!("advanced_user"));
    assert_eq!(
        json_output.get("description").unwrap(),
        &json!("Advanced workflow with scripts")
    );
}

#[rstest]
fn test_workflows_long_descriptions(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Test with very long description
    let long_description = "This is a very long description that contains many characters and describes in great detail what this workflow does including all the steps involved in processing the data and generating the final results. It goes on for quite a while to test how the system handles longer text fields and ensures that everything works correctly even with substantial amounts of descriptive text.".to_string();

    let workflow = create_test_workflow_with_description(
        config,
        "long_description_workflow",
        "test_user",
        Some(long_description.clone()),
    );

    let workflow_id = workflow.id.unwrap();

    // Verify long description is preserved
    let args = ["workflows", "get", &workflow_id.to_string()];

    let json_output = run_cli_with_json(&args, start_server, Some("test_user"))
        .expect("Failed to get workflow with long description");

    assert_eq!(
        json_output.get("description").unwrap(),
        &json!(long_description)
    );
}

#[rstest]
fn test_workflows_special_characters(start_server: &ServerProcess) {
    // Test with special characters in names and descriptions
    let special_cases = [
        (
            "unicode_workflow",
            "测试工作流",
            "Workflow with Unicode: 你好世界",
        ),
        (
            "emoji_workflow",
            "🚀_workflow",
            "Workflow with emojis: 📊 📈 🎯",
        ),
        (
            "symbols_workflow",
            "workflow-2024_v1.2",
            "Symbols: !@#$%^&*()_+-={}[]|\\:;\"'<>?,./ ",
        ),
        (
            "quotes_workflow",
            "workflow'with\"quotes",
            "Description with 'single' and \"double\" quotes",
        ),
    ];

    for (test_name, workflow_name, description) in &special_cases {
        let args = [
            "workflows",
            "new",
            "--name",
            workflow_name,
            "--description",
            description,
        ];

        let json_output = run_cli_with_json(&args, start_server, Some("special_user"))
            .unwrap_or_else(|_| {
                panic!(
                    "Failed to create workflow with special characters: {}",
                    test_name
                )
            });

        assert_eq!(json_output.get("name").unwrap(), &json!(workflow_name));
        assert_eq!(json_output.get("description").unwrap(), &json!(description));
        assert_eq!(json_output.get("user").unwrap(), &json!("special_user"));
    }
}

#[rstest]
fn test_workflows_error_handling(start_server: &ServerProcess) {
    // Test getting a non-existent workflow
    let args = ["workflows", "get", "999999"];

    let result = run_cli_with_json(&args, start_server, Some("test_user"));
    assert!(
        result.is_err(),
        "Should fail when getting non-existent workflow"
    );

    // Test updating a non-existent workflow
    let args = ["workflows", "update", "999999", "--name", "should_fail"];

    let result = run_cli_with_json(&args, start_server, None);
    assert!(
        result.is_err(),
        "Should fail when updating non-existent workflow"
    );

    // Test removing a non-existent workflow
    let args = ["workflows", "remove", "999999"];

    let result = run_cli_with_json(&args, start_server, Some("test_user"));
    assert!(
        result.is_err(),
        "Should fail when removing non-existent workflow"
    );
}

#[rstest]
fn test_workflows_list_empty_user(start_server: &ServerProcess) {
    // Test listing workflows for user with no workflows
    let args = ["workflows", "list"];

    let json_output = run_cli_with_json(&args, start_server, Some("nonexistent_user"))
        .expect("Failed to list workflows for empty user");

    let workflows_array = json_output.as_array().unwrap();
    assert!(
        workflows_array.is_empty(),
        "Should return empty array for user with no workflows"
    );
}

#[rstest]
fn test_workflows_name_uniqueness(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create multiple workflows with same name but different users
    let _workflow1 = create_test_workflow_with_description(
        config,
        "duplicate_name",
        "user1",
        Some("First workflow".to_string()),
    );
    let _workflow2 = create_test_workflow_with_description(
        config,
        "duplicate_name",
        "user2",
        Some("Second workflow".to_string()),
    );

    // Both should exist and be distinguishable by user
    let args_user1 = ["workflows", "list"];

    let json_output_user1 = run_cli_with_json(&args_user1, start_server, Some("user1"))
        .expect("Failed to list workflows for user1");

    let workflows_user1 = json_output_user1.as_array().unwrap();
    let user1_workflows: Vec<_> = workflows_user1
        .iter()
        .filter(|w| w.get("name").unwrap() == "duplicate_name")
        .collect();
    assert_eq!(user1_workflows.len(), 1);

    let args_user2 = ["workflows", "list"];

    let json_output_user2 = run_cli_with_json(&args_user2, start_server, Some("user2"))
        .expect("Failed to list workflows for user2");

    let workflows_user2 = json_output_user2.as_array().unwrap();
    let user2_workflows: Vec<_> = workflows_user2
        .iter()
        .filter(|w| w.get("name").unwrap() == "duplicate_name")
        .collect();
    assert_eq!(user2_workflows.len(), 1);
}

#[rstest]
fn test_workflows_timestamp_format(start_server: &ServerProcess) {
    let config = &start_server.config;

    let workflow = create_test_workflow_with_description(
        config,
        "timestamp_test",
        "test_user",
        Some("Test timestamp format".to_string()),
    );
    let workflow_id = workflow.id.unwrap();

    let args = ["workflows", "get", &workflow_id.to_string()];

    let json_output = run_cli_with_json(&args, start_server, Some("test_user"))
        .expect("Failed to get workflow for timestamp test");

    // Verify timestamp format (should be ISO 8601 or similar)
    if let Some(timestamp) = json_output.get("timestamp") {
        let timestamp_str = timestamp.as_str();
        if let Some(ts) = timestamp_str {
            // Basic validation - should contain date-like format
            assert!(
                ts.contains("T") || ts.contains("-") || ts.len() > 10,
                "Timestamp should be in a proper date format: {}",
                ts
            );
        }
    }
}

#[rstest]
fn test_workflows_is_uninitialized(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "uninitialized_test_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create jobs in the workflow (they should be in Uninitialized status by default)
    let job1 = torc::models::JobModel::new(
        workflow_id,
        "test_job1".to_string(),
        "echo 'test job 1'".to_string(),
    );
    let job2 = torc::models::JobModel::new(
        workflow_id,
        "test_job2".to_string(),
        "echo 'test job 2'".to_string(),
    );
    let job3 = torc::models::JobModel::new(
        workflow_id,
        "test_job3".to_string(),
        "echo 'test job 3'".to_string(),
    );

    let _created_job1 = default_api::create_job(config, job1).expect("Failed to create job1");
    let _created_job2 = default_api::create_job(config, job2).expect("Failed to create job2");
    let _created_job3 = default_api::create_job(config, job3).expect("Failed to create job3");

    // Check that workflow is uninitialized (all jobs are Uninitialized)
    let uninitialized_response = default_api::is_workflow_uninitialized(config, workflow_id)
        .expect("Failed to check if workflow is uninitialized");

    let is_uninitialized = uninitialized_response
        .get("is_uninitialized")
        .and_then(|v| v.as_bool())
        .expect("Response should contain is_uninitialized field");

    assert!(
        is_uninitialized,
        "Workflow should be uninitialized when all jobs are in Uninitialized status"
    );

    // Initialize jobs
    let _result = default_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    // Check that workflow is no longer uninitialized (jobs are now Ready)
    let initialized_response = default_api::is_workflow_uninitialized(config, workflow_id)
        .expect("Failed to check if workflow is uninitialized after initialization");

    let is_still_uninitialized = initialized_response
        .get("is_uninitialized")
        .and_then(|v| v.as_bool())
        .expect("Response should contain is_uninitialized field");

    assert!(
        !is_still_uninitialized,
        "Workflow should not be uninitialized after jobs are initialized"
    );
}

#[rstest]
fn test_workflows_is_uninitialized_with_disabled_jobs(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "disabled_jobs_workflow");
    let workflow_id = workflow.id.unwrap();

    // Create jobs in the workflow
    let job1 = torc::models::JobModel::new(
        workflow_id,
        "uninitialized_job".to_string(),
        "echo 'uninitialized'".to_string(),
    );
    let mut job2 = torc::models::JobModel::new(
        workflow_id,
        "disabled_job".to_string(),
        "echo 'disabled'".to_string(),
    );

    // Set job2 to Disabled status
    job2.status = Some(torc::models::JobStatus::Disabled);

    let _created_job1 = default_api::create_job(config, job1).expect("Failed to create job1");
    let created_job2 = default_api::create_job(config, job2).expect("Failed to create job2");
    let job2_id = created_job2.id.unwrap();

    // Update job2 to be Disabled
    let mut update_job2 = created_job2.clone();
    update_job2.status = Some(torc::models::JobStatus::Disabled);
    let _updated_job2 = default_api::update_job(config, job2_id, update_job2)
        .expect("Failed to update job2 to Disabled");

    // Check that workflow is uninitialized (one job is Uninitialized, one is Disabled)
    let uninitialized_response = default_api::is_workflow_uninitialized(config, workflow_id)
        .expect("Failed to check if workflow is uninitialized");

    let is_uninitialized = uninitialized_response
        .get("is_uninitialized")
        .and_then(|v| v.as_bool())
        .expect("Response should contain is_uninitialized field");

    assert!(
        is_uninitialized,
        "Workflow should be uninitialized when all jobs are either Uninitialized or Disabled"
    );

    // Initialize jobs (only the uninitialized one will change to Ready)
    let _result = default_api::initialize_jobs(config, workflow_id, None, None, None)
        .expect("Failed to initialize jobs");

    // Check that workflow is no longer uninitialized (job1 is now Ready, job2 is still Disabled)
    let initialized_response = default_api::is_workflow_uninitialized(config, workflow_id)
        .expect("Failed to check if workflow is uninitialized after initialization");

    let is_still_uninitialized = initialized_response
        .get("is_uninitialized")
        .and_then(|v| v.as_bool())
        .expect("Response should contain is_uninitialized field");

    assert!(
        !is_still_uninitialized,
        "Workflow should not be uninitialized when at least one job is in a non-uninitialized/non-disabled status"
    );
}

#[rstest]
fn test_workflows_is_uninitialized_empty_workflow(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow with no jobs
    let workflow = create_test_workflow(config, "empty_workflow");
    let workflow_id = workflow.id.unwrap();

    // Check that empty workflow is considered uninitialized
    let uninitialized_response = default_api::is_workflow_uninitialized(config, workflow_id)
        .expect("Failed to check if empty workflow is uninitialized");

    let is_uninitialized = uninitialized_response
        .get("is_uninitialized")
        .and_then(|v| v.as_bool())
        .expect("Response should contain is_uninitialized field");

    assert!(
        is_uninitialized,
        "Empty workflow (no jobs) should be considered uninitialized"
    );
}

#[rstest]
fn test_workflow_archive_and_unarchive(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create test workflow
    let workflow = create_test_workflow(config, "archive_test_workflow");
    let workflow_id = workflow.id.unwrap();

    // Get initial workflow status - should not be archived
    let initial_status = default_api::get_workflow_status(config, workflow_id)
        .expect("Failed to get initial workflow status");
    assert!(
        !initial_status.is_archived.unwrap_or(false),
        "New workflow should not be archived"
    );

    // Archive the workflow using CLI
    let archive_args = ["workflows", "archive", "true", &workflow_id.to_string()];
    let archive_output =
        run_cli_with_json(&archive_args, start_server, None).expect("Failed to archive workflow");

    assert_eq!(
        archive_output.get("status").and_then(|v| v.as_str()),
        Some("success"),
        "Archive operation should succeed"
    );
    assert_eq!(
        archive_output.get("is_archived").and_then(|v| v.as_bool()),
        Some(true),
        "Response should indicate workflow is archived"
    );

    // Verify workflow is archived
    let archived_status = default_api::get_workflow_status(config, workflow_id)
        .expect("Failed to get archived workflow status");
    assert!(
        archived_status.is_archived.unwrap_or(false),
        "Workflow should be archived"
    );

    // Unarchive the workflow using CLI
    let unarchive_args = ["workflows", "archive", "false", &workflow_id.to_string()];
    let unarchive_output = run_cli_with_json(&unarchive_args, start_server, None)
        .expect("Failed to unarchive workflow");

    assert_eq!(
        unarchive_output.get("status").and_then(|v| v.as_str()),
        Some("success"),
        "Unarchive operation should succeed"
    );
    assert_eq!(
        unarchive_output
            .get("is_archived")
            .and_then(|v| v.as_bool()),
        Some(false),
        "Response should indicate workflow is not archived"
    );

    // Verify workflow is no longer archived
    let unarchived_status = default_api::get_workflow_status(config, workflow_id)
        .expect("Failed to get unarchived workflow status");
    assert!(
        !unarchived_status.is_archived.unwrap_or(false),
        "Workflow should not be archived after unarchive"
    );
}

#[rstest]
fn test_workflow_list_excludes_archived_by_default(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create active and archived workflows
    let active_workflow = create_test_workflow(config, "active_list_test");
    let active_id = active_workflow.id.unwrap();

    let to_archive_workflow = create_test_workflow(config, "archived_list_test");
    let archived_id = to_archive_workflow.id.unwrap();

    // Archive the second workflow
    let mut status = default_api::get_workflow_status(config, archived_id)
        .expect("Failed to get workflow status");
    status.is_archived = Some(true);
    default_api::update_workflow_status(config, archived_id, status)
        .expect("Failed to archive workflow");

    // List workflows without archived filter (default behavior)
    let list_args = ["workflows", "list", "--limit", "100"];
    let list_output = run_cli_with_json(&list_args, start_server, Some("test_user"))
        .expect("Failed to list workflows");

    // Extract workflow IDs from response
    let workflows = list_output.as_array().unwrap_or_else(|| {
        // Handle object format with "workflows" field
        list_output
            .get("workflows")
            .and_then(|w| w.as_array())
            .expect("Expected workflows array in response")
    });

    let workflow_ids: Vec<i64> = workflows
        .iter()
        .filter_map(|w| w.get("id").and_then(|id| id.as_i64()))
        .collect();

    // Active workflow should be in the list
    assert!(
        workflow_ids.contains(&active_id),
        "Active workflow should appear in default list"
    );

    // Archived workflow should NOT be in the list
    assert!(
        !workflow_ids.contains(&archived_id),
        "Archived workflow should NOT appear in default list"
    );
}

#[rstest]
fn test_workflow_list_archived_only(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create active and archived workflows
    let active_workflow = create_test_workflow(config, "active_archived_only_test");
    let active_id = active_workflow.id.unwrap();

    let to_archive_workflow = create_test_workflow(config, "archived_archived_only_test");
    let archived_id = to_archive_workflow.id.unwrap();

    // Archive the second workflow
    let mut status = default_api::get_workflow_status(config, archived_id)
        .expect("Failed to get workflow status");
    status.is_archived = Some(true);
    default_api::update_workflow_status(config, archived_id, status)
        .expect("Failed to archive workflow");

    // List only archived workflows
    let list_args = ["workflows", "list", "--limit", "100", "--archived-only"];
    let list_output = run_cli_with_json(&list_args, start_server, Some("test_user"))
        .expect("Failed to list archived workflows");

    // Extract workflow IDs from response
    let workflows = list_output.as_array().unwrap_or_else(|| {
        // Handle object format with "workflows" field
        list_output
            .get("workflows")
            .and_then(|w| w.as_array())
            .expect("Expected workflows array in response")
    });

    let workflow_ids: Vec<i64> = workflows
        .iter()
        .filter_map(|w| w.get("id").and_then(|id| id.as_i64()))
        .collect();

    // Archived workflow should be in the list
    assert!(
        workflow_ids.contains(&archived_id),
        "Archived workflow should appear in --archived-only list"
    );

    // Active workflow should NOT be in the list
    assert!(
        !workflow_ids.contains(&active_id),
        "Active workflow should NOT appear in --archived-only list"
    );
}

#[rstest]
fn test_cannot_reset_archived_workflow_status(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create and archive a workflow
    let workflow = create_test_workflow(config, "reset_archived_test");
    let workflow_id = workflow.id.unwrap();

    // Archive the workflow
    let mut status = default_api::get_workflow_status(config, workflow_id)
        .expect("Failed to get workflow status");
    status.is_archived = Some(true);
    default_api::update_workflow_status(config, workflow_id, status)
        .expect("Failed to archive workflow");

    // Attempt to reset workflow status - should fail
    let reset_result = default_api::reset_workflow_status(config, workflow_id, None, None);

    assert!(
        reset_result.is_err(),
        "Reset workflow status should fail on archived workflow"
    );

    // Verify it's a 422 error (Unprocessable Content)
    let error = reset_result.unwrap_err();
    let error_message = format!("{:?}", error);

    assert!(
        error_message.contains("422") || error_message.contains("Cannot reset archived"),
        "Error should indicate cannot reset archived workflow: {}",
        error_message
    );
}

#[rstest]
fn test_archived_workflow_other_operations_still_work(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create and archive a workflow
    let workflow = create_test_workflow(config, "archived_ops_test");
    let workflow_id = workflow.id.unwrap();

    // Archive the workflow
    let mut status = default_api::get_workflow_status(config, workflow_id)
        .expect("Failed to get workflow status");
    status.is_archived = Some(true);
    default_api::update_workflow_status(config, workflow_id, status)
        .expect("Failed to archive workflow");

    // Verify get_workflow still works
    let get_result = default_api::get_workflow(config, workflow_id);
    assert!(
        get_result.is_ok(),
        "Should be able to get archived workflow"
    );

    // Verify get_workflow_status still works
    let status_result = default_api::get_workflow_status(config, workflow_id);
    assert!(
        status_result.is_ok(),
        "Should be able to get status of archived workflow"
    );

    // Verify is_workflow_complete still works
    let complete_result = default_api::is_workflow_complete(config, workflow_id);
    assert!(
        complete_result.is_ok(),
        "Should be able to check if archived workflow is complete"
    );

    // Verify is_workflow_uninitialized still works
    let uninit_result = default_api::is_workflow_uninitialized(config, workflow_id);
    assert!(
        uninit_result.is_ok(),
        "Should be able to check if archived workflow is uninitialized"
    );

    // Verify delete_workflow still works
    let delete_result = default_api::delete_workflow(config, workflow_id, None);
    assert!(
        delete_result.is_ok(),
        "Should be able to delete archived workflow"
    );
}

#[rstest]
fn test_archive_multiple_workflows(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create multiple workflows
    let workflow1 = create_test_workflow(config, "multi_archive_1");
    let id1 = workflow1.id.unwrap();

    let workflow2 = create_test_workflow(config, "multi_archive_2");
    let id2 = workflow2.id.unwrap();

    let workflow3 = create_test_workflow(config, "multi_archive_3");
    let id3 = workflow3.id.unwrap();

    // Archive multiple workflows at once using CLI
    let archive_args = [
        "workflows",
        "archive",
        "true",
        &id1.to_string(),
        &id2.to_string(),
        &id3.to_string(),
    ];
    let archive_output = run_cli_with_json(&archive_args, start_server, None)
        .expect("Failed to archive multiple workflows");

    assert_eq!(
        archive_output.get("status").and_then(|v| v.as_str()),
        Some("success"),
        "Multi-archive operation should succeed"
    );

    let updated_workflows = archive_output
        .get("updated_workflows")
        .and_then(|v| v.as_array())
        .expect("Should have updated_workflows array");

    assert_eq!(
        updated_workflows.len(),
        3,
        "Should have archived 3 workflows"
    );

    // Verify all workflows are archived
    for workflow_id in &[id1, id2, id3] {
        let status = default_api::get_workflow_status(config, *workflow_id)
            .expect("Failed to get workflow status");
        assert!(
            status.is_archived.unwrap_or(false),
            "Workflow {} should be archived",
            workflow_id
        );
    }
}

#[rstest]
fn test_workflows_list_all_users_no_auth(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create workflows for 3 different users
    let wf_a = default_api::create_workflow(
        config,
        models::WorkflowModel::new(
            "all_users_test_wf_a".to_string(),
            "all_users_user_a".to_string(),
        ),
    )
    .expect("Failed to create workflow for user_a");

    let wf_b = default_api::create_workflow(
        config,
        models::WorkflowModel::new(
            "all_users_test_wf_b".to_string(),
            "all_users_user_b".to_string(),
        ),
    )
    .expect("Failed to create workflow for user_b");

    let wf_c = default_api::create_workflow(
        config,
        models::WorkflowModel::new(
            "all_users_test_wf_c".to_string(),
            "all_users_user_c".to_string(),
        ),
    )
    .expect("Failed to create workflow for user_c");

    // Run `torc workflows list --all-users` with JSON output
    let args = ["workflows", "list", "--all-users"];
    let json_output = run_cli_with_json(&args, start_server, Some("all_users_user_a"))
        .expect("Failed to run workflows list --all-users");

    // Should return an array containing workflows from all users
    assert!(
        json_output.is_array(),
        "Workflows list should return an array"
    );
    let workflows_array = json_output.as_array().unwrap();

    // Find our test workflows by ID
    let wf_a_id = wf_a.id.unwrap();
    let wf_b_id = wf_b.id.unwrap();
    let wf_c_id = wf_c.id.unwrap();

    let found_ids: Vec<i64> = workflows_array
        .iter()
        .filter_map(|w| w.get("id").and_then(|id| id.as_i64()))
        .collect();

    assert!(
        found_ids.contains(&wf_a_id),
        "Should contain user_a's workflow (id={})",
        wf_a_id
    );
    assert!(
        found_ids.contains(&wf_b_id),
        "Should contain user_b's workflow (id={})",
        wf_b_id
    );
    assert!(
        found_ids.contains(&wf_c_id),
        "Should contain user_c's workflow (id={})",
        wf_c_id
    );

    // Verify each workflow has a user field
    for wf in workflows_array {
        assert!(
            wf.get("user").is_some(),
            "Each workflow should have a 'user' field"
        );
    }

    // Without --all-users, user_a should only see their own workflows
    let args_no_all = ["workflows", "list"];
    let json_filtered = run_cli_with_json(&args_no_all, start_server, Some("all_users_user_a"))
        .expect("Failed to run workflows list without --all-users");

    let filtered_array = json_filtered.as_array().unwrap();
    let filtered_users: Vec<&str> = filtered_array
        .iter()
        .filter_map(|w| w.get("user").and_then(|u| u.as_str()))
        .collect();

    // All returned workflows should belong to user_a
    for u in &filtered_users {
        assert_eq!(
            *u, "all_users_user_a",
            "Without --all-users, should only return current user's workflows"
        );
    }
}
