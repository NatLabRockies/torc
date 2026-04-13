mod common;

use common::{ServerProcess, create_test_workflow, delete_all_workflows, start_server};
use rstest::rstest;
use serial_test::serial;

use torc::client::commands::select_workflow_interactively;

#[rstest]
#[serial(workflow_delete)]
fn test_select_workflow_interactively_single_workflow(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Clean up any existing workflows to ensure test isolation
    delete_all_workflows(config).expect("Failed to clean up existing workflows");

    let workflow = create_test_workflow(config, "single_test_workflow");
    let workflow_id = workflow.id.unwrap();
    let user = workflow.user.clone();

    // Test that the function auto-selects when there's only one workflow
    let selected_id = select_workflow_interactively(config, &user)
        .expect("Should successfully auto-select single workflow");

    assert_eq!(selected_id, workflow_id);
}

#[rstest]
#[serial(workflow_delete)]
fn test_select_workflow_interactively_user_isolation(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Clean up any existing workflows to ensure test isolation
    delete_all_workflows(config).expect("Failed to clean up existing workflows");

    // Create workflows with explicitly different users
    let user1 = "test_user_1".to_string();
    let user2 = "test_user_2".to_string();

    let workflow1 = torc::models::WorkflowModel::new("user1_workflow".to_string(), user1.clone());
    let created_workflow1 = torc::client::apis::workflows_api::create_workflow(config, workflow1)
        .expect("Failed to create workflow for user1");

    let workflow2 = torc::models::WorkflowModel::new("user2_workflow".to_string(), user2.clone());
    let created_workflow2 = torc::client::apis::workflows_api::create_workflow(config, workflow2)
        .expect("Failed to create workflow for user2");

    // Test that user1 only sees their own workflow (and gets auto-selected)
    let selected_id1 = select_workflow_interactively(config, &user1)
        .expect("Should successfully auto-select workflow for user1");

    assert_eq!(selected_id1, created_workflow1.id.unwrap());

    // Verify that listing workflows for user1 only returns user1's workflows
    let list_response = torc::client::apis::workflows_api::list_workflows(
        config,
        None,     // offset
        Some(50), // limit
        None,     // sort_by
        None,     // reverse_sort
        None,     // name filter
        Some(&user1),
        None, // description filter
        None, // is_archived filter
    )
    .expect("Should list workflows for user1");

    let user1_workflows = list_response.items;

    // User1 should see at least their workflow
    assert!(
        !user1_workflows.is_empty(),
        "User1 should have at least one workflow"
    );

    // All workflows returned should belong to user1
    for workflow in user1_workflows {
        assert_eq!(
            workflow.user, user1,
            "All returned workflows should belong to user1"
        );
    }

    // Test that user2 also gets auto-selected for their single workflow
    let selected_id2 = select_workflow_interactively(config, &user2)
        .expect("Should successfully auto-select workflow for user2");

    assert_eq!(selected_id2, created_workflow2.id.unwrap());

    // Verify that listing workflows for user2 only returns user2's workflows
    let list_response2 = torc::client::apis::workflows_api::list_workflows(
        config,
        None,     // offset
        Some(50), // limit
        None,     // sort_by
        None,     // reverse_sort
        None,     // name filter
        Some(&user2),
        None, // description filter
        None, // is_archived filter
    )
    .expect("Should list workflows for user2");

    let user2_workflows = list_response2.items;

    // User2 should see at least their workflow
    assert!(
        !user2_workflows.is_empty(),
        "User2 should have at least one workflow"
    );

    // All workflows returned should belong to user2
    for workflow in user2_workflows {
        assert_eq!(
            workflow.user, user2,
            "All returned workflows should belong to user2"
        );
    }
}

#[rstest]
#[serial(workflow_delete)]
fn test_delete_all_workflows(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Clean up any existing workflows first to start with a clean slate
    delete_all_workflows(config).expect("Failed to clean up existing workflows");

    // Create multiple workflows with different users
    let user1 = "delete_test_user_1".to_string();
    let user2 = "delete_test_user_2".to_string();
    let user3 = "delete_test_user_3".to_string();

    let workflow1 =
        torc::models::WorkflowModel::new("delete_test_workflow_1".to_string(), user1.clone());
    let _created_workflow1 = torc::client::apis::workflows_api::create_workflow(config, workflow1)
        .expect("Failed to create workflow1");

    let workflow2 =
        torc::models::WorkflowModel::new("delete_test_workflow_2".to_string(), user2.clone());
    let _created_workflow2 = torc::client::apis::workflows_api::create_workflow(config, workflow2)
        .expect("Failed to create workflow2");

    let workflow3 =
        torc::models::WorkflowModel::new("delete_test_workflow_3".to_string(), user3.clone());
    let _created_workflow3 = torc::client::apis::workflows_api::create_workflow(config, workflow3)
        .expect("Failed to create workflow3");

    // Verify workflows were created - should have at least our 3 workflows
    let list_response_before = torc::client::apis::workflows_api::list_workflows(
        config, None, None, None, None, None, None, None, None,
    )
    .expect("Should list workflows before deletion");

    let workflows_before = list_response_before.items;
    assert!(
        workflows_before.len() >= 3,
        "Should have at least 3 workflows before deletion"
    );

    // Find our created workflows in the list
    let our_workflow_ids: Vec<i64> = workflows_before
        .iter()
        .filter(|w| w.user == user1 || w.user == user2 || w.user == user3)
        .filter_map(|w| w.id)
        .collect();
    assert_eq!(
        our_workflow_ids.len(),
        3,
        "Should find exactly 3 workflows we created"
    );

    // Delete all workflows
    delete_all_workflows(config).expect("Should successfully delete all workflows");

    // Verify all workflows were deleted
    let list_response_after = torc::client::apis::workflows_api::list_workflows(
        config, None, None, None, None, None, None, None, None,
    )
    .expect("Should list workflows after deletion");

    let workflows_after = list_response_after.items;
    assert!(
        workflows_after.is_empty(),
        "Should have no workflows after deletion"
    );

    // Verify that each user has no workflows
    for user in [&user1, &user2, &user3] {
        let user_workflows = torc::client::apis::workflows_api::list_workflows(
            config,
            None,
            None,
            None,
            None,
            None,
            Some(user),
            None,
            None,
        )
        .expect("Should list user workflows after deletion");

        let user_workflow_list = user_workflows.items;
        assert!(
            user_workflow_list.is_empty(),
            "User {} should have no workflows after deletion",
            user
        );
    }
}

#[rstest]
#[serial(workflow_delete)]
fn test_delete_all_workflows_strict_success_criteria(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Clean up any existing workflows first to start with a clean slate
    delete_all_workflows(config).expect("Failed to clean up existing workflows");

    // Create a workflow that we'll be able to delete
    let user1 = "strict_test_user_1".to_string();
    let workflow1 =
        torc::models::WorkflowModel::new("strict_deletable_workflow".to_string(), user1.clone());
    let _created_workflow1 = torc::client::apis::workflows_api::create_workflow(config, workflow1)
        .expect("Failed to create workflow1");

    // Create another workflow
    let user2 = "strict_test_user_2".to_string();
    let workflow2 = torc::models::WorkflowModel::new(
        "strict_another_deletable_workflow".to_string(),
        user2.clone(),
    );
    let _created_workflow2 = torc::client::apis::workflows_api::create_workflow(config, workflow2)
        .expect("Failed to create workflow2");

    // Verify workflows were created
    let list_response_before = torc::client::apis::workflows_api::list_workflows(
        config, None, None, None, None, None, None, None, None,
    )
    .expect("Should list workflows before deletion test");

    let workflows_before = list_response_before.items;
    assert!(
        workflows_before.len() >= 2,
        "Should have at least 2 workflows before test"
    );

    // Test that delete_all_workflows succeeds when all deletions work
    let result = delete_all_workflows(config);
    assert!(
        result.is_ok(),
        "delete_all_workflows should succeed when all deletions work: {:?}",
        result
    );

    // Verify all workflows were actually deleted
    let list_response_after = torc::client::apis::workflows_api::list_workflows(
        config, None, None, None, None, None, None, None, None,
    )
    .expect("Should list workflows after successful deletion");

    let workflows_after = list_response_after.items;
    assert!(
        workflows_after.is_empty(),
        "Should have no workflows after successful deletion"
    );
}
