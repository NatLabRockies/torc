mod common;

use common::{
    AccessControlServerProcess, ServerProcess, run_cli_command_with_auth,
    run_cli_command_with_auth_full, run_jobs_cli_command_with_auth,
    run_jobs_cli_command_with_auth_full, start_server, start_server_with_access_control,
};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

use rstest::rstest;
use torc::client::{Configuration, default_api};
use torc::models;

/// Atomic counter for generating unique names in tests
static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Create a workflow with a specific user
fn create_workflow_with_user(
    config: &Configuration,
    name: &str,
    user: &str,
) -> models::WorkflowModel {
    let workflow = models::WorkflowModel::new(name.to_string(), user.to_string());
    default_api::create_workflow(config, workflow).expect("Failed to create workflow")
}

// ============================================================================
// Access Group CRUD Tests
// ============================================================================

#[rstest]
fn test_create_access_group(start_server: &ServerProcess) {
    let config = &start_server.config;

    let group = models::AccessGroupModel {
        id: None,
        name: "test-group".to_string(),
        description: Some("A test access group".to_string()),
        created_at: None,
    };

    let result =
        default_api::create_access_group(config, group).expect("Failed to create access group");

    assert!(result.id.is_some());
    assert_eq!(result.name, "test-group");
    assert_eq!(result.description, Some("A test access group".to_string()));
    assert!(result.created_at.is_some());
}

#[rstest]
fn test_create_access_group_without_description(start_server: &ServerProcess) {
    let config = &start_server.config;

    let group = models::AccessGroupModel {
        id: None,
        name: "group-no-desc".to_string(),
        description: None,
        created_at: None,
    };

    let result = default_api::create_access_group(config, group)
        .expect("Failed to create access group without description");

    assert!(result.id.is_some());
    assert_eq!(result.name, "group-no-desc");
    assert!(result.description.is_none());
}

#[rstest]
fn test_get_access_group(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a group first
    let group = models::AccessGroupModel {
        id: None,
        name: "get-test-group".to_string(),
        description: Some("Group for get test".to_string()),
        created_at: None,
    };

    let created =
        default_api::create_access_group(config, group).expect("Failed to create access group");
    let group_id = created.id.unwrap();

    // Now get it by ID
    let fetched =
        default_api::get_access_group(config, group_id).expect("Failed to get access group");

    assert_eq!(fetched.id, Some(group_id));
    assert_eq!(fetched.name, "get-test-group");
    assert_eq!(fetched.description, Some("Group for get test".to_string()));
}

#[rstest]
fn test_list_access_groups(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create multiple groups
    for i in 0..3 {
        let group = models::AccessGroupModel {
            id: None,
            name: format!("list-group-{}", i),
            description: Some(format!("List test group {}", i)),
            created_at: None,
        };
        default_api::create_access_group(config, group).expect("Failed to create access group");
    }

    // List all groups
    let result =
        default_api::list_access_groups(config, None, None).expect("Failed to list access groups");

    assert!(result.items.len() >= 3);
    assert!(result.total_count >= 3);

    // Verify our groups are in the list
    let names: Vec<&str> = result.items.iter().map(|g| g.name.as_str()).collect();
    assert!(names.contains(&"list-group-0"));
    assert!(names.contains(&"list-group-1"));
    assert!(names.contains(&"list-group-2"));
}

#[rstest]
fn test_list_access_groups_pagination(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create several groups
    for i in 0..5 {
        let group = models::AccessGroupModel {
            id: None,
            name: format!("page-group-{}", i),
            description: None,
            created_at: None,
        };
        let _ = default_api::create_access_group(config, group);
    }

    // Test with limit
    let page1 = default_api::list_access_groups(config, Some(0), Some(2))
        .expect("Failed to list first page");

    assert!(page1.items.len() <= 2);
    assert!(page1.offset == 0);
    assert!(page1.limit == 2);
}

#[rstest]
fn test_delete_access_group(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a group
    let group = models::AccessGroupModel {
        id: None,
        name: "delete-test-group".to_string(),
        description: Some("Group to be deleted".to_string()),
        created_at: None,
    };

    let created =
        default_api::create_access_group(config, group).expect("Failed to create access group");
    let group_id = created.id.unwrap();

    // Delete it
    let deleted =
        default_api::delete_access_group(config, group_id).expect("Failed to delete access group");

    assert_eq!(deleted.id, Some(group_id));
    assert_eq!(deleted.name, "delete-test-group");

    // Verify it's gone (should return an error)
    let result = default_api::get_access_group(config, group_id);
    assert!(result.is_err(), "Deleted group should not be found");
}

// ============================================================================
// User-Group Membership Tests
// ============================================================================

#[rstest]
fn test_add_user_to_group(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a group first
    let group = models::AccessGroupModel {
        id: None,
        name: "membership-test-group".to_string(),
        description: None,
        created_at: None,
    };

    let created =
        default_api::create_access_group(config, group).expect("Failed to create access group");
    let group_id = created.id.unwrap();

    // Add a user to the group
    let membership = models::UserGroupMembershipModel {
        id: None,
        user_name: "alice".to_string(),
        group_id,
        role: "member".to_string(),
        created_at: None,
    };

    let result = default_api::add_user_to_group(config, group_id, membership)
        .expect("Failed to add user to group");

    assert!(result.id.is_some());
    assert_eq!(result.user_name, "alice");
    assert_eq!(result.group_id, group_id);
    assert_eq!(result.role, "member");
}

#[rstest]
fn test_list_group_members(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a group
    let group = models::AccessGroupModel {
        id: None,
        name: "members-list-group".to_string(),
        description: None,
        created_at: None,
    };

    let created =
        default_api::create_access_group(config, group).expect("Failed to create access group");
    let group_id = created.id.unwrap();

    // Add multiple users
    for user in ["bob", "carol", "dave"] {
        let membership = models::UserGroupMembershipModel {
            id: None,
            user_name: user.to_string(),
            group_id,
            role: "member".to_string(),
            created_at: None,
        };
        default_api::add_user_to_group(config, group_id, membership)
            .expect("Failed to add user to group");
    }

    // List members
    let result = default_api::list_group_members(config, group_id, None, None)
        .expect("Failed to list group members");

    assert_eq!(result.items.len(), 3);
    let names: Vec<&str> = result.items.iter().map(|m| m.user_name.as_str()).collect();
    assert!(names.contains(&"bob"));
    assert!(names.contains(&"carol"));
    assert!(names.contains(&"dave"));
}

#[rstest]
fn test_remove_user_from_group(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a group
    let group = models::AccessGroupModel {
        id: None,
        name: "remove-member-group".to_string(),
        description: None,
        created_at: None,
    };

    let created =
        default_api::create_access_group(config, group).expect("Failed to create access group");
    let group_id = created.id.unwrap();

    // Add a user
    let membership = models::UserGroupMembershipModel {
        id: None,
        user_name: "eve".to_string(),
        group_id,
        role: "member".to_string(),
        created_at: None,
    };
    default_api::add_user_to_group(config, group_id, membership)
        .expect("Failed to add user to group");

    // Remove the user
    let removed = default_api::remove_user_from_group(config, group_id, "eve")
        .expect("Failed to remove user from group");

    assert_eq!(removed.user_name, "eve");

    // Verify user is no longer in the group
    let members = default_api::list_group_members(config, group_id, None, None)
        .expect("Failed to list group members");

    let names: Vec<&str> = members.items.iter().map(|m| m.user_name.as_str()).collect();
    assert!(!names.contains(&"eve"));
}

#[rstest]
fn test_list_user_groups(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create multiple groups
    let mut group_ids = Vec::new();
    for i in 0..3 {
        let group = models::AccessGroupModel {
            id: None,
            name: format!("user-groups-test-{}", i),
            description: None,
            created_at: None,
        };

        let created =
            default_api::create_access_group(config, group).expect("Failed to create access group");
        group_ids.push(created.id.unwrap());
    }

    // Add the same user to all groups
    for group_id in &group_ids {
        let membership = models::UserGroupMembershipModel {
            id: None,
            user_name: "multi-group-user".to_string(),
            group_id: *group_id,
            role: "member".to_string(),
            created_at: None,
        };
        default_api::add_user_to_group(config, *group_id, membership)
            .expect("Failed to add user to group");
    }

    // List the user's groups
    let result = default_api::list_user_groups(config, "multi-group-user", None, None)
        .expect("Failed to list user groups");

    assert!(result.items.len() >= 3);
    let names: Vec<&str> = result.items.iter().map(|g| g.name.as_str()).collect();
    assert!(names.contains(&"user-groups-test-0"));
    assert!(names.contains(&"user-groups-test-1"));
    assert!(names.contains(&"user-groups-test-2"));
}

// ============================================================================
// Workflow-Group Association Tests
// ============================================================================

#[rstest]
fn test_add_workflow_to_group(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow
    let workflow = create_workflow_with_user(config, "workflow-for-group", "wf-user");
    let workflow_id = workflow.id.unwrap();

    // Create a group
    let group = models::AccessGroupModel {
        id: None,
        name: "workflow-access-group".to_string(),
        description: None,
        created_at: None,
    };

    let created_group =
        default_api::create_access_group(config, group).expect("Failed to create access group");
    let group_id = created_group.id.unwrap();

    // Add workflow to group
    let association = default_api::add_workflow_to_group(config, workflow_id, group_id)
        .expect("Failed to add workflow to group");

    assert_eq!(association.workflow_id, workflow_id);
    assert_eq!(association.group_id, group_id);
    assert!(association.created_at.is_some());
}

#[rstest]
fn test_list_workflow_groups(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow
    let workflow = create_workflow_with_user(config, "workflow-multi-groups", "wf-user-2");
    let workflow_id = workflow.id.unwrap();

    // Create multiple groups and add workflow to each
    for i in 0..3 {
        let group = models::AccessGroupModel {
            id: None,
            name: format!("wf-group-{}", i),
            description: None,
            created_at: None,
        };

        let created_group =
            default_api::create_access_group(config, group).expect("Failed to create access group");
        let group_id = created_group.id.unwrap();

        default_api::add_workflow_to_group(config, workflow_id, group_id)
            .expect("Failed to add workflow to group");
    }

    // List the workflow's groups
    let result = default_api::list_workflow_groups(config, workflow_id, None, None)
        .expect("Failed to list workflow groups");

    assert!(result.items.len() >= 3);
    let names: Vec<&str> = result.items.iter().map(|g| g.name.as_str()).collect();
    assert!(names.contains(&"wf-group-0"));
    assert!(names.contains(&"wf-group-1"));
    assert!(names.contains(&"wf-group-2"));
}

#[rstest]
fn test_remove_workflow_from_group(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow
    let workflow = create_workflow_with_user(config, "workflow-to-remove", "wf-user-3");
    let workflow_id = workflow.id.unwrap();

    // Create a group
    let group = models::AccessGroupModel {
        id: None,
        name: "removable-wf-group".to_string(),
        description: None,
        created_at: None,
    };

    let created_group =
        default_api::create_access_group(config, group).expect("Failed to create access group");
    let group_id = created_group.id.unwrap();

    // Add workflow to group
    default_api::add_workflow_to_group(config, workflow_id, group_id)
        .expect("Failed to add workflow to group");

    // Remove workflow from group
    let removed = default_api::remove_workflow_from_group(config, workflow_id, group_id)
        .expect("Failed to remove workflow from group");

    assert_eq!(removed.workflow_id, workflow_id);
    assert_eq!(removed.group_id, group_id);

    // Verify the association is gone
    let groups = default_api::list_workflow_groups(config, workflow_id, None, None)
        .expect("Failed to list workflow groups");

    let group_ids: Vec<i64> = groups.items.iter().filter_map(|g| g.id).collect();
    assert!(!group_ids.contains(&group_id));
}

// ============================================================================
// Access Check Tests
// ============================================================================

#[rstest]
fn test_check_workflow_access_owner(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow owned by "owner-user"
    let workflow = create_workflow_with_user(config, "owned-workflow", "owner-user");
    let workflow_id = workflow.id.unwrap();

    // Check that the owner has access
    let result = default_api::check_workflow_access(config, workflow_id, "owner-user")
        .expect("Failed to check workflow access");

    assert!(result.has_access);
    assert_eq!(result.user_name, "owner-user");
    assert_eq!(result.workflow_id, workflow_id);
}

#[rstest]
fn test_check_workflow_access_group_member(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow owned by "creator"
    let workflow = create_workflow_with_user(config, "shared-workflow", "creator");
    let workflow_id = workflow.id.unwrap();

    // Create a group
    let group = models::AccessGroupModel {
        id: None,
        name: "access-check-group".to_string(),
        description: None,
        created_at: None,
    };

    let created_group =
        default_api::create_access_group(config, group).expect("Failed to create access group");
    let group_id = created_group.id.unwrap();

    // Add a user to the group
    let membership = models::UserGroupMembershipModel {
        id: None,
        user_name: "group-member".to_string(),
        group_id,
        role: "member".to_string(),
        created_at: None,
    };
    default_api::add_user_to_group(config, group_id, membership)
        .expect("Failed to add user to group");

    // Initially, group member should NOT have access
    let no_access = default_api::check_workflow_access(config, workflow_id, "group-member")
        .expect("Failed to check workflow access");
    assert!(!no_access.has_access);

    // Add workflow to the group
    default_api::add_workflow_to_group(config, workflow_id, group_id)
        .expect("Failed to add workflow to group");

    // Now the group member should have access
    let has_access = default_api::check_workflow_access(config, workflow_id, "group-member")
        .expect("Failed to check workflow access");
    assert!(has_access.has_access);
    assert_eq!(has_access.user_name, "group-member");
}

#[rstest]
fn test_check_workflow_access_non_member(start_server: &ServerProcess) {
    let config = &start_server.config;

    // Create a workflow
    let workflow = create_workflow_with_user(config, "private-workflow", "private-owner");
    let workflow_id = workflow.id.unwrap();

    // A random user should NOT have access
    let result = default_api::check_workflow_access(config, workflow_id, "random-user")
        .expect("Failed to check workflow access");

    assert!(!result.has_access);
    assert_eq!(result.user_name, "random-user");
}

// ============================================================================
// End-to-End Access Control Enforcement Tests
// ============================================================================
//
// These tests verify that access control is actually ENFORCED when calling
// API endpoints. They require a server started with --enforce-access-control.

/// Generate a unique suffix for test entity names
fn unique_suffix() -> u64 {
    TEST_COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// Helper to set up the two-team scenario:
///
/// - ml-team: alice, bob, shared_user
/// - data-team: carol, dave, shared_user
///
/// Returns (ml_team_id, data_team_id)
fn setup_two_teams(config: &Configuration) -> (i64, i64) {
    let suffix = unique_suffix();

    // Create ML team
    let ml_group = models::AccessGroupModel {
        id: None,
        name: format!("ml-team-{}", suffix),
        description: Some("Machine Learning Team".to_string()),
        created_at: None,
    };
    let ml_team =
        default_api::create_access_group(config, ml_group).expect("Failed to create ML team");
    let ml_team_id = ml_team.id.unwrap();

    // Create Data team
    let data_group = models::AccessGroupModel {
        id: None,
        name: format!("data-team-{}", suffix),
        description: Some("Data Processing Team".to_string()),
        created_at: None,
    };
    let data_team =
        default_api::create_access_group(config, data_group).expect("Failed to create Data team");
    let data_team_id = data_team.id.unwrap();

    // Add users to ML team: alice, bob, shared_user
    for user in ["alice", "bob", "shared_user"] {
        let membership = models::UserGroupMembershipModel {
            id: None,
            user_name: user.to_string(),
            group_id: ml_team_id,
            role: "member".to_string(),
            created_at: None,
        };
        default_api::add_user_to_group(config, ml_team_id, membership)
            .expect("Failed to add user to ML team");
    }

    // Add users to Data team: carol, dave, shared_user
    for user in ["carol", "dave", "shared_user"] {
        let membership = models::UserGroupMembershipModel {
            id: None,
            user_name: user.to_string(),
            group_id: data_team_id,
            role: "member".to_string(),
            created_at: None,
        };
        default_api::add_user_to_group(config, data_team_id, membership)
            .expect("Failed to add user to Data team");
    }

    (ml_team_id, data_team_id)
}

#[rstest]
fn test_enforcement_owner_can_access_own_workflow(
    start_server_with_access_control: &AccessControlServerProcess,
) {
    let config = &start_server_with_access_control.config;

    // Create a workflow owned by "owner_user"
    let workflow = create_workflow_with_user(config, "owner-test-workflow", "owner_user");
    let workflow_id = workflow.id.unwrap();

    // The owner should be able to access their own workflow
    // Note: With access control enabled, this should succeed because ownership grants access
    let result = default_api::check_workflow_access(config, workflow_id, "owner_user")
        .expect("Failed to check access");
    assert!(
        result.has_access,
        "Owner should have access to their own workflow"
    );
}

#[rstest]
fn test_enforcement_non_member_cannot_access_workflow(
    start_server_with_access_control: &AccessControlServerProcess,
) {
    let config = &start_server_with_access_control.config;

    // Create a workflow owned by "owner_user"
    let workflow = create_workflow_with_user(config, "restricted-workflow", "owner_user");
    let workflow_id = workflow.id.unwrap();

    // A user with no access should be denied
    let result = default_api::check_workflow_access(config, workflow_id, "outsider")
        .expect("Failed to check access");
    assert!(
        !result.has_access,
        "Non-member should NOT have access to workflow"
    );
}

#[rstest]
fn test_enforcement_team_member_can_access_shared_workflow(
    start_server_with_access_control: &AccessControlServerProcess,
) {
    let config = &start_server_with_access_control.config;

    // Set up teams
    let (ml_team_id, _data_team_id) = setup_two_teams(config);

    // Create a workflow owned by "workflow_creator"
    let workflow = create_workflow_with_user(config, "ml-shared-workflow", "workflow_creator");
    let workflow_id = workflow.id.unwrap();

    // Initially, alice (ML team member) should NOT have access
    let no_access = default_api::check_workflow_access(config, workflow_id, "alice")
        .expect("Failed to check access");
    assert!(
        !no_access.has_access,
        "Alice should not have access before workflow is shared"
    );

    // Share the workflow with the ML team
    default_api::add_workflow_to_group(config, workflow_id, ml_team_id)
        .expect("Failed to add workflow to ML team");

    // Now alice should have access
    let has_access = default_api::check_workflow_access(config, workflow_id, "alice")
        .expect("Failed to check access");
    assert!(
        has_access.has_access,
        "Alice (ML team member) should have access after workflow is shared with ML team"
    );

    // bob (also ML team member) should also have access
    let bob_access = default_api::check_workflow_access(config, workflow_id, "bob")
        .expect("Failed to check access");
    assert!(
        bob_access.has_access,
        "Bob (ML team member) should have access to ML team workflow"
    );

    // carol (Data team member, NOT ML team) should NOT have access
    let carol_access = default_api::check_workflow_access(config, workflow_id, "carol")
        .expect("Failed to check access");
    assert!(
        !carol_access.has_access,
        "Carol (Data team only) should NOT have access to ML team workflow"
    );
}

#[rstest]
fn test_enforcement_multi_team_member_can_access_both_team_workflows(
    start_server_with_access_control: &AccessControlServerProcess,
) {
    let config = &start_server_with_access_control.config;

    // Set up teams (shared_user is in both teams)
    let (ml_team_id, data_team_id) = setup_two_teams(config);

    // Create an ML workflow
    let ml_workflow = create_workflow_with_user(config, "ml-workflow", "ml_owner");
    let ml_workflow_id = ml_workflow.id.unwrap();

    // Create a Data workflow
    let data_workflow = create_workflow_with_user(config, "data-workflow", "data_owner");
    let data_workflow_id = data_workflow.id.unwrap();

    // Share workflows with respective teams
    default_api::add_workflow_to_group(config, ml_workflow_id, ml_team_id)
        .expect("Failed to share ML workflow");
    default_api::add_workflow_to_group(config, data_workflow_id, data_team_id)
        .expect("Failed to share Data workflow");

    // shared_user should have access to BOTH workflows (member of both teams)
    let ml_access = default_api::check_workflow_access(config, ml_workflow_id, "shared_user")
        .expect("Failed to check ML access");
    assert!(
        ml_access.has_access,
        "shared_user should have access to ML workflow (member of both teams)"
    );

    let data_access = default_api::check_workflow_access(config, data_workflow_id, "shared_user")
        .expect("Failed to check Data access");
    assert!(
        data_access.has_access,
        "shared_user should have access to Data workflow (member of both teams)"
    );

    // alice should only have access to ML workflow
    let alice_ml = default_api::check_workflow_access(config, ml_workflow_id, "alice")
        .expect("Failed to check");
    assert!(
        alice_ml.has_access,
        "alice should have access to ML workflow"
    );

    let alice_data = default_api::check_workflow_access(config, data_workflow_id, "alice")
        .expect("Failed to check");
    assert!(
        !alice_data.has_access,
        "alice should NOT have access to Data workflow"
    );

    // carol should only have access to Data workflow
    let carol_ml = default_api::check_workflow_access(config, ml_workflow_id, "carol")
        .expect("Failed to check");
    assert!(
        !carol_ml.has_access,
        "carol should NOT have access to ML workflow"
    );

    let carol_data = default_api::check_workflow_access(config, data_workflow_id, "carol")
        .expect("Failed to check");
    assert!(
        carol_data.has_access,
        "carol should have access to Data workflow"
    );
}

#[rstest]
fn test_enforcement_revoke_access_removes_permission(
    start_server_with_access_control: &AccessControlServerProcess,
) {
    let config = &start_server_with_access_control.config;

    // Set up teams
    let (ml_team_id, _data_team_id) = setup_two_teams(config);

    // Create and share a workflow
    let workflow = create_workflow_with_user(config, "revoke-test-workflow", "some_owner");
    let workflow_id = workflow.id.unwrap();

    default_api::add_workflow_to_group(config, workflow_id, ml_team_id)
        .expect("Failed to share workflow");

    // Verify alice has access
    let has_access = default_api::check_workflow_access(config, workflow_id, "alice")
        .expect("Failed to check access");
    assert!(has_access.has_access, "alice should have access initially");

    // Revoke access by removing workflow from group
    default_api::remove_workflow_from_group(config, workflow_id, ml_team_id)
        .expect("Failed to remove workflow from group");

    // Verify alice no longer has access
    let no_access = default_api::check_workflow_access(config, workflow_id, "alice")
        .expect("Failed to check access");
    assert!(
        !no_access.has_access,
        "alice should NOT have access after workflow is removed from group"
    );
}

#[rstest]
fn test_enforcement_workflow_shared_with_multiple_groups(
    start_server_with_access_control: &AccessControlServerProcess,
) {
    let config = &start_server_with_access_control.config;

    // Set up teams
    let (ml_team_id, data_team_id) = setup_two_teams(config);

    // Create a workflow
    let workflow = create_workflow_with_user(config, "multi-group-workflow", "creator");
    let workflow_id = workflow.id.unwrap();

    // Share with BOTH teams
    default_api::add_workflow_to_group(config, workflow_id, ml_team_id)
        .expect("Failed to share with ML team");
    default_api::add_workflow_to_group(config, workflow_id, data_team_id)
        .expect("Failed to share with Data team");

    // All team members should have access
    for user in ["alice", "bob", "carol", "dave", "shared_user"] {
        let access = default_api::check_workflow_access(config, workflow_id, user)
            .expect("Failed to check access");
        assert!(
            access.has_access,
            "{} should have access to workflow shared with both teams",
            user
        );
    }

    // An outsider should still not have access
    let outsider = default_api::check_workflow_access(config, workflow_id, "outsider")
        .expect("Failed to check access");
    assert!(
        !outsider.has_access,
        "outsider should NOT have access even when workflow is shared with multiple groups"
    );
}

#[rstest]
fn test_enforcement_remove_user_from_group_revokes_access(
    start_server_with_access_control: &AccessControlServerProcess,
) {
    let config = &start_server_with_access_control.config;

    // Create a group with a user
    let group = models::AccessGroupModel {
        id: None,
        name: format!("user-removal-test-{}", unique_suffix()),
        description: None,
        created_at: None,
    };
    let created_group =
        default_api::create_access_group(config, group).expect("Failed to create group");
    let group_id = created_group.id.unwrap();

    // Add user to group
    let membership = models::UserGroupMembershipModel {
        id: None,
        user_name: "removable_user".to_string(),
        group_id,
        role: "member".to_string(),
        created_at: None,
    };
    default_api::add_user_to_group(config, group_id, membership)
        .expect("Failed to add user to group");

    // Create and share a workflow
    let workflow = create_workflow_with_user(config, "user-removal-workflow", "wf_owner");
    let workflow_id = workflow.id.unwrap();

    default_api::add_workflow_to_group(config, workflow_id, group_id)
        .expect("Failed to share workflow");

    // User should have access
    let has_access = default_api::check_workflow_access(config, workflow_id, "removable_user")
        .expect("Failed to check access");
    assert!(has_access.has_access, "User should have access initially");

    // Remove user from group
    default_api::remove_user_from_group(config, group_id, "removable_user")
        .expect("Failed to remove user from group");

    // User should no longer have access
    let no_access = default_api::check_workflow_access(config, workflow_id, "removable_user")
        .expect("Failed to check access");
    assert!(
        !no_access.has_access,
        "User should NOT have access after being removed from group"
    );
}

// ============================================================================
// API Endpoint Access Control Tests
// ============================================================================
//
// These tests verify that actual API endpoints (get_workflow, get_job, etc.)
// return 403 Forbidden when the user lacks access.

/// Helper to create a configuration with specific basic auth credentials
fn config_with_auth(base_config: &Configuration, username: &str) -> Configuration {
    Configuration {
        base_path: base_config.base_path.clone(),
        user_agent: base_config.user_agent.clone(),
        client: base_config.client.clone(),
        basic_auth: Some((username.to_string(), Some("password".to_string()))),
        oauth_access_token: None,
        bearer_access_token: None,
        api_key: None,
    }
}

/// Helper to create a job for a workflow
fn create_job_for_workflow(
    config: &Configuration,
    workflow_id: i64,
    name: &str,
) -> models::JobModel {
    let job = models::JobModel::new(workflow_id, name.to_string(), "echo test".to_string());
    default_api::create_job(config, job).expect("Failed to create job")
}

/// Helper to check if an error response indicates access denial (HTTP 403 Forbidden).
fn is_access_denied_error<T: std::fmt::Debug>(
    result: &Result<T, torc::client::apis::Error<impl std::fmt::Debug>>,
) -> bool {
    match result {
        Err(torc::client::apis::Error::ResponseError(content)) => {
            // Check for HTTP 403 status code
            content.status.as_u16() == 403
        }
        _ => false,
    }
}

#[rstest]
fn test_get_workflow_returns_error_for_unauthorized_user(
    start_server_with_access_control: &AccessControlServerProcess,
) {
    let config = &start_server_with_access_control.config;

    // Create a workflow owned by "owner"
    let workflow = create_workflow_with_user(config, "api-test-workflow", "owner");
    let workflow_id = workflow.id.unwrap();

    // Create a config with different user credentials
    let unauthorized_config = config_with_auth(config, "unauthorized_user");

    // Try to get the workflow - should fail with access denied
    let result = default_api::get_workflow(&unauthorized_config, workflow_id);

    assert!(
        is_access_denied_error(&result),
        "Expected access denied error, got: {:?}",
        result
    );
}

#[rstest]
fn test_get_job_returns_error_for_unauthorized_user(
    start_server_with_access_control: &AccessControlServerProcess,
) {
    let config = &start_server_with_access_control.config;

    // Create a workflow owned by "owner"
    let workflow = create_workflow_with_user(config, "job-access-test-workflow", "job_owner");
    let workflow_id = workflow.id.unwrap();

    // Create a job in that workflow
    let job = create_job_for_workflow(config, workflow_id, "test-job");
    let job_id = job.id.unwrap();

    // Create a config with different user credentials
    let unauthorized_config = config_with_auth(config, "unauthorized_job_user");

    // Try to get the job - should fail with access denied
    let result = default_api::get_job(&unauthorized_config, job_id);

    assert!(
        is_access_denied_error(&result),
        "Expected access denied error, got: {:?}",
        result
    );
}

#[rstest]
fn test_authorized_user_can_access_shared_workflow_via_api(
    start_server_with_access_control: &AccessControlServerProcess,
) {
    let config = &start_server_with_access_control.config;

    // Set up teams
    let (ml_team_id, _) = setup_two_teams(config);

    // Create a workflow
    let workflow = create_workflow_with_user(config, "shared-api-workflow", "api_owner");
    let workflow_id = workflow.id.unwrap();

    // Share with ML team
    default_api::add_workflow_to_group(config, workflow_id, ml_team_id)
        .expect("Failed to share workflow");

    // Create a job
    let job = create_job_for_workflow(config, workflow_id, "shared-test-job");
    let job_id = job.id.unwrap();

    // Alice (ML team member) should be able to access the workflow
    let alice_config = config_with_auth(config, "alice");
    let workflow_result = default_api::get_workflow(&alice_config, workflow_id);
    assert!(
        workflow_result.is_ok(),
        "Alice should be able to get workflow: {:?}",
        workflow_result.err()
    );

    // Alice should also be able to access the job
    let job_result = default_api::get_job(&alice_config, job_id);
    assert!(
        job_result.is_ok(),
        "Alice should be able to get job: {:?}",
        job_result.err()
    );

    // Carol (Data team, NOT ML team) should NOT be able to access
    let carol_config = config_with_auth(config, "carol");
    let carol_workflow_result = default_api::get_workflow(&carol_config, workflow_id);
    match carol_workflow_result {
        Err(torc::client::apis::Error::ResponseError(content)) => {
            assert_eq!(
                content.status,
                reqwest::StatusCode::FORBIDDEN,
                "Carol should get 403 for workflow"
            );
        }
        Ok(_) => panic!("Carol should NOT be able to access ML team workflow"),
        Err(e) => panic!("Unexpected error for Carol: {:?}", e),
    }

    let carol_job_result = default_api::get_job(&carol_config, job_id);
    match carol_job_result {
        Err(torc::client::apis::Error::ResponseError(content)) => {
            assert_eq!(
                content.status,
                reqwest::StatusCode::FORBIDDEN,
                "Carol should get 403 for job"
            );
        }
        Ok(_) => panic!("Carol should NOT be able to access job in ML team workflow"),
        Err(e) => panic!("Unexpected error for Carol: {:?}", e),
    }
}

#[rstest]
fn test_multi_team_user_can_access_both_workflows_via_api(
    start_server_with_access_control: &AccessControlServerProcess,
) {
    let config = &start_server_with_access_control.config;

    // Set up teams (shared_user is in both)
    let (ml_team_id, data_team_id) = setup_two_teams(config);

    // Create ML workflow and share with ML team
    let ml_workflow = create_workflow_with_user(config, "ml-api-workflow", "ml_api_owner");
    let ml_workflow_id = ml_workflow.id.unwrap();
    default_api::add_workflow_to_group(config, ml_workflow_id, ml_team_id)
        .expect("Failed to share ML workflow");

    // Create Data workflow and share with Data team
    let data_workflow = create_workflow_with_user(config, "data-api-workflow", "data_api_owner");
    let data_workflow_id = data_workflow.id.unwrap();
    default_api::add_workflow_to_group(config, data_workflow_id, data_team_id)
        .expect("Failed to share Data workflow");

    // shared_user should be able to access both
    let shared_config = config_with_auth(config, "shared_user");

    let ml_result = default_api::get_workflow(&shared_config, ml_workflow_id);
    assert!(
        ml_result.is_ok(),
        "shared_user should access ML workflow: {:?}",
        ml_result.err()
    );

    let data_result = default_api::get_workflow(&shared_config, data_workflow_id);
    assert!(
        data_result.is_ok(),
        "shared_user should access Data workflow: {:?}",
        data_result.err()
    );

    // alice (ML only) should only access ML workflow
    let alice_config = config_with_auth(config, "alice");

    let alice_ml = default_api::get_workflow(&alice_config, ml_workflow_id);
    assert!(alice_ml.is_ok(), "Alice should access ML workflow");

    let alice_data = default_api::get_workflow(&alice_config, data_workflow_id);
    match alice_data {
        Err(torc::client::apis::Error::ResponseError(content)) => {
            assert_eq!(content.status, reqwest::StatusCode::FORBIDDEN);
        }
        Ok(_) => panic!("Alice should NOT access Data workflow"),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

// ============================================================================
// Comprehensive End-to-End Access Control Integration Test
// ============================================================================
//
// This test verifies the complete workflow lifecycle with access control:
// - Two access groups with different users
// - Authorized user runs workflow to completion with `torc run`
// - Authorized user inspects results, events, jobs via CLI
// - Authorized user can run reports CLI commands
// - Unauthorized user cannot run the workflow

/// Create a diamond workflow for access control testing.
/// Returns (workflow_id, HashMap of job names to jobs).
fn create_access_control_diamond_workflow(
    config: &Configuration,
    owner: &str,
    work_dir: &std::path::Path,
) -> (i64, std::collections::HashMap<String, models::JobModel>) {
    let suffix = unique_suffix();
    let name = format!("access_control_diamond_workflow_{}", suffix);
    let workflow = models::WorkflowModel::new(name.clone(), owner.to_string());
    let created_workflow =
        default_api::create_workflow(config, workflow).expect("Failed to add workflow");
    let workflow_id = created_workflow.id.unwrap();

    // Create a compute node for this workflow
    let compute_node = models::ComputeNodeModel::new(
        workflow_id,
        "test-host".to_string(),
        std::process::id() as i64,
        chrono::Utc::now().to_rfc3339(),
        8,                   // num_cpus
        16.0,                // memory_gb
        0,                   // num_gpus
        1,                   // num_nodes
        "local".to_string(), // compute_node_type
        None,
    );
    let _ = default_api::create_compute_node(config, compute_node)
        .expect("Failed to create compute node");

    // Create local variables for file paths
    let f1_path = work_dir.join("f1.json").to_string_lossy().to_string();
    let f2_path = work_dir.join("f2.json").to_string_lossy().to_string();
    let f3_path = work_dir.join("f3.json").to_string_lossy().to_string();
    let f4_path = work_dir.join("f4.json").to_string_lossy().to_string();
    let f5_path = work_dir.join("f5.json").to_string_lossy().to_string();
    let f6_path = work_dir.join("f6.json").to_string_lossy().to_string();

    let f1 = default_api::create_file(
        config,
        models::FileModel::new(workflow_id, "f1".to_string(), f1_path.clone()),
    )
    .expect("Failed to add file");
    let f2 = default_api::create_file(
        config,
        models::FileModel::new(workflow_id, "f2".to_string(), f2_path.clone()),
    )
    .expect("Failed to add file");
    let f3 = default_api::create_file(
        config,
        models::FileModel::new(workflow_id, "f3".to_string(), f3_path.clone()),
    )
    .expect("Failed to add file");
    let f4 = default_api::create_file(
        config,
        models::FileModel::new(workflow_id, "f4".to_string(), f4_path.clone()),
    )
    .expect("Failed to add file");
    let f5 = default_api::create_file(
        config,
        models::FileModel::new(workflow_id, "f5".to_string(), f5_path.clone()),
    )
    .expect("Failed to add file");
    let f6 = default_api::create_file(
        config,
        models::FileModel::new(workflow_id, "f6".to_string(), f6_path.clone()),
    )
    .expect("Failed to add file");

    let preprocess_script = "tests/scripts/preprocess.sh";
    let work_script = "tests/scripts/work.sh";
    let postprocess_script = "tests/scripts/postprocess.sh";

    let mut preprocess_pre = models::JobModel::new(
        workflow_id,
        "preprocess".to_string(),
        format!(
            "bash {} -i {} -o {} -o {}",
            preprocess_script, f1_path, f2_path, f3_path
        ),
    );
    let mut work1_pre = models::JobModel::new(
        workflow_id,
        "work1".to_string(),
        format!("bash {} -i {} -o {}", work_script, f2_path, f4_path),
    );
    let mut work2_pre = models::JobModel::new(
        workflow_id,
        "work2".to_string(),
        format!("bash {} -i {} -o {}", work_script, f3_path, f5_path),
    );
    let mut postprocess_pre = models::JobModel::new(
        workflow_id,
        "postprocess".to_string(),
        format!(
            "bash {} -i {} -i {} -o {}",
            postprocess_script, f4_path, f5_path, f6_path
        ),
    );

    preprocess_pre.input_file_ids = Some(vec![f1.id.unwrap()]);
    preprocess_pre.output_file_ids = Some(vec![f2.id.unwrap(), f3.id.unwrap()]);
    work1_pre.input_file_ids = Some(vec![f2.id.unwrap()]);
    work1_pre.output_file_ids = Some(vec![f4.id.unwrap()]);
    work2_pre.input_file_ids = Some(vec![f3.id.unwrap()]);
    work2_pre.output_file_ids = Some(vec![f5.id.unwrap()]);
    postprocess_pre.input_file_ids = Some(vec![f4.id.unwrap(), f5.id.unwrap()]);
    postprocess_pre.output_file_ids = Some(vec![f6.id.unwrap()]);

    let preprocess =
        default_api::create_job(config, preprocess_pre).expect("Failed to add preprocess");
    let work1 = default_api::create_job(config, work1_pre).expect("Failed to add work1");
    let work2 = default_api::create_job(config, work2_pre).expect("Failed to add work2");
    let postprocess =
        default_api::create_job(config, postprocess_pre).expect("Failed to add postprocess");

    let mut jobs = std::collections::HashMap::new();
    jobs.insert("preprocess".to_string(), preprocess);
    jobs.insert("work1".to_string(), work1);
    jobs.insert("work2".to_string(), work2);
    jobs.insert("postprocess".to_string(), postprocess);

    (workflow_id, jobs)
}

/// Comprehensive end-to-end test for access control with workflow execution.
///
/// This test verifies:
/// 1. Two access groups with different users (ml-team and data-team)
/// 2. A valid user (alice, ml-team member) can run a workflow to completion with `torc run`
/// 3. The valid user can inspect results, events, jobs via CLI
/// 4. The valid user can run reports CLI commands
/// 5. An invalid user (carol, data-team only) cannot run the workflow
#[rstest]
fn test_comprehensive_access_control_workflow_execution(
    start_server_with_access_control: &AccessControlServerProcess,
) {
    let config = &start_server_with_access_control.config;
    let password = "password"; // All test users have this password

    // =========================================================================
    // Step 1: Set up two access groups with different users
    // =========================================================================
    let (ml_team_id, _data_team_id) = setup_two_teams(config);

    // =========================================================================
    // Step 2: Create a workflow owned by alice and share it with ml-team
    // =========================================================================
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let work_dir = temp_dir.path().to_path_buf();

    // Create input file
    let input_data = r#"{"data": "initial input", "value": 42}"#;
    fs::write(work_dir.join("f1.json"), input_data).expect("Failed to write f1.json");

    // Create workflow owned by alice (who is an admin and ml-team member)
    let alice_config = config_with_auth(config, "alice");
    let (workflow_id, _jobs) =
        create_access_control_diamond_workflow(&alice_config, "alice", &work_dir);

    // Share workflow with ml-team
    default_api::add_workflow_to_group(config, workflow_id, ml_team_id)
        .expect("Failed to share workflow with ml-team");

    // Verify alice has access
    let alice_access = default_api::check_workflow_access(config, workflow_id, "alice")
        .expect("Failed to check alice's access");
    assert!(
        alice_access.has_access,
        "alice should have access to workflow"
    );

    // Verify carol (data-team only) does NOT have access
    let carol_access = default_api::check_workflow_access(config, workflow_id, "carol")
        .expect("Failed to check carol's access");
    assert!(
        !carol_access.has_access,
        "carol should NOT have access to workflow"
    );

    // =========================================================================
    // Step 3: Valid user (alice) runs workflow to completion with `torc run`
    // =========================================================================
    let workflow_id_str = workflow_id.to_string();
    let work_dir_str = work_dir.to_str().unwrap();
    let run_args: Vec<&str> = vec![
        &workflow_id_str,
        "--output-dir",
        work_dir_str,
        "--poll-interval",
        "0.1",
        "--num-cpus",
        "4",
        "--memory-gb",
        "8.0",
    ];

    run_jobs_cli_command_with_auth(
        &run_args,
        start_server_with_access_control,
        "alice",
        password,
    )
    .expect("alice should be able to run workflow");

    // Verify workflow completion - all jobs should be completed
    let jobs = default_api::list_jobs(
        &alice_config,
        workflow_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("Failed to list jobs");

    for job in jobs.items.unwrap() {
        assert_eq!(
            job.status.unwrap(),
            models::JobStatus::Completed,
            "Job {} should be completed. actual status: {:?}",
            job.name,
            job.status
        );
    }

    // Verify output files exist
    assert!(work_dir.join("f2.json").exists(), "f2.json should exist");
    assert!(work_dir.join("f3.json").exists(), "f3.json should exist");
    assert!(work_dir.join("f4.json").exists(), "f4.json should exist");
    assert!(work_dir.join("f5.json").exists(), "f5.json should exist");
    assert!(work_dir.join("f6.json").exists(), "f6.json should exist");

    // =========================================================================
    // Step 4: Valid user (alice) inspects results, events, jobs via CLI
    // =========================================================================

    // Test jobs list command
    let jobs_output = run_cli_command_with_auth(
        &["jobs", "list", &workflow_id.to_string()],
        start_server_with_access_control,
        "alice",
        password,
    )
    .expect("alice should be able to list jobs");
    assert!(
        jobs_output.contains("preprocess") || jobs_output.contains("work1"),
        "Jobs list should contain job names"
    );

    // Test results list command
    let results_output = run_cli_command_with_auth(
        &["results", "list", &workflow_id.to_string()],
        start_server_with_access_control,
        "alice",
        password,
    )
    .expect("alice should be able to list results");
    // Results should exist and show completion info
    assert!(
        results_output.contains("0") || !results_output.is_empty(),
        "Results list should contain data"
    );

    // Test events list command
    let _events_output = run_cli_command_with_auth(
        &["events", "list", &workflow_id.to_string()],
        start_server_with_access_control,
        "alice",
        password,
    )
    .expect("alice should be able to list events");
    // Events list command should succeed (may be empty if no events were logged)

    // Test files list command
    let files_output = run_cli_command_with_auth(
        &["files", "list", &workflow_id.to_string()],
        start_server_with_access_control,
        "alice",
        password,
    )
    .expect("alice should be able to list files");
    assert!(
        files_output.contains("f1") || files_output.contains("f2"),
        "Files list should contain file names"
    );

    // Test workflow status command
    let status_output = run_cli_command_with_auth(
        &["workflows", "status", &workflow_id.to_string()],
        start_server_with_access_control,
        "alice",
        password,
    )
    .expect("alice should be able to get workflow status");
    // Workflow status shows metadata like run_id, is_canceled - verify we got output
    assert!(
        status_output.contains("Run ID") || status_output.contains("run_id"),
        "Workflow status should show workflow metadata. Got: {}",
        status_output
    );

    // =========================================================================
    // Step 5: Valid user (alice) can run reports CLI commands
    // =========================================================================

    // Test reports commands - these should work for authorized users
    let report_output = run_cli_command_with_auth(
        &["reports", "summary", &workflow_id.to_string()],
        start_server_with_access_control,
        "alice",
        password,
    )
    .expect("alice should be able to run reports summary");
    assert!(!report_output.is_empty(), "Report should produce output");

    // =========================================================================
    // Step 6: Invalid user (carol) cannot run the workflow
    // =========================================================================

    // Create a new workflow for carol to try to access
    let (workflow_id_2, _) =
        create_access_control_diamond_workflow(&alice_config, "alice", &work_dir);

    // Share with ml-team only (carol is in data-team, not ml-team)
    default_api::add_workflow_to_group(config, workflow_id_2, ml_team_id)
        .expect("Failed to share workflow with ml-team");

    // Carol should not be able to run the workflow
    let carol_run_output = run_jobs_cli_command_with_auth_full(
        &[
            &workflow_id_2.to_string(),
            "--output-dir",
            work_dir.to_str().unwrap(),
            "--poll-interval",
            "0.1",
            "--num-cpus",
            "4",
            "--memory-gb",
            "8.0",
        ],
        start_server_with_access_control,
        "carol",
        password,
    );

    // Command should fail with access denied
    assert!(
        !carol_run_output.status.success(),
        "carol should NOT be able to run workflow she doesn't have access to"
    );

    let stderr = String::from_utf8_lossy(&carol_run_output.stderr);
    assert!(
        stderr.contains("403")
            || stderr.contains("Forbidden")
            || stderr.contains("access")
            || stderr.contains("denied")
            || stderr.contains("unauthorized"),
        "Error message should indicate access denial. Got: {}",
        stderr
    );

    // Carol should also not be able to list jobs for the workflow
    let carol_jobs_output = run_cli_command_with_auth_full(
        &["jobs", "list", &workflow_id_2.to_string()],
        start_server_with_access_control,
        "carol",
        password,
    );

    assert!(
        !carol_jobs_output.status.success(),
        "carol should NOT be able to list jobs for workflow she doesn't have access to"
    );

    // =========================================================================
    // Step 7: Verify bob (also ml-team member) can also access the workflow
    // =========================================================================
    let bob_jobs_output = run_cli_command_with_auth(
        &["jobs", "list", &workflow_id.to_string()],
        start_server_with_access_control,
        "bob",
        password,
    )
    .expect("bob (ml-team member) should be able to list jobs");
    assert!(
        bob_jobs_output.contains("preprocess") || bob_jobs_output.contains("work1"),
        "Bob should see job names"
    );

    // =========================================================================
    // Step 8: Verify shared_user (member of both teams) can access workflows
    // shared with either team
    // =========================================================================
    let shared_user_jobs_output = run_cli_command_with_auth(
        &["jobs", "list", &workflow_id.to_string()],
        start_server_with_access_control,
        "shared_user",
        password,
    )
    .expect("shared_user (member of both teams) should be able to list jobs");
    assert!(
        shared_user_jobs_output.contains("preprocess") || shared_user_jobs_output.contains("work1"),
        "shared_user should see job names"
    );
}

/// Test that `workflows list --all-users` with access control returns only workflows
/// the authenticated user can access (owned + group-shared), not all workflows.
#[rstest]
fn test_workflows_list_all_users_with_access_control(
    start_server_with_access_control: &AccessControlServerProcess,
) {
    let admin_config = &start_server_with_access_control.config;
    let password = "password";

    // Create configs for different users
    let wf_user_config = config_with_auth(admin_config, "wf-user");
    let wf_user_2_config = config_with_auth(admin_config, "wf-user-2");
    let wf_user_3_config = config_with_auth(admin_config, "wf-user-3");

    let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);

    // Create workflow A owned by "wf-user"
    let wf_a = create_workflow_with_user(
        &wf_user_config,
        &format!("all-users-test-wf-a-{}", counter),
        "wf-user",
    );
    let wf_a_id = wf_a.id.unwrap();

    // Create workflow B owned by "wf-user-2"
    let wf_b = create_workflow_with_user(
        &wf_user_2_config,
        &format!("all-users-test-wf-b-{}", counter),
        "wf-user-2",
    );
    let wf_b_id = wf_b.id.unwrap();

    // Create workflow C owned by "wf-user-3" (not shared with wf-user)
    let wf_c = create_workflow_with_user(
        &wf_user_3_config,
        &format!("all-users-test-wf-c-{}", counter),
        "wf-user-3",
    );
    let wf_c_id = wf_c.id.unwrap();

    // Create an access group and add wf-user to it (admin can create groups)
    let group = models::AccessGroupModel {
        id: None,
        name: format!("all-users-test-group-{}", counter),
        description: Some("Test group for all-users listing".to_string()),
        created_at: None,
    };
    let created_group =
        default_api::create_access_group(admin_config, group).expect("Failed to create group");
    let group_id = created_group.id.unwrap();

    // Add wf-user to the group
    let membership = models::UserGroupMembershipModel {
        id: None,
        user_name: "wf-user".to_string(),
        group_id,
        role: "member".to_string(),
        created_at: None,
    };
    default_api::add_user_to_group(admin_config, group_id, membership)
        .expect("Failed to add wf-user to group");

    // Share workflow B with the group (so wf-user can access it)
    default_api::add_workflow_to_group(admin_config, wf_b_id, group_id)
        .expect("Failed to share workflow B with group");

    // Verify wf-user can see workflow A (owned) and workflow B (group-shared)
    // but NOT workflow C (no access)
    let output = run_cli_command_with_auth(
        &["--format", "json", "workflows", "list", "--all-users"],
        start_server_with_access_control,
        "wf-user",
        password,
    )
    .expect("Failed to run workflows list --all-users as wf-user");

    let json_output: serde_json::Value =
        serde_json::from_str(&output).expect("Failed to parse JSON output");
    let workflows_array = json_output.as_array().expect("Expected JSON array");

    let found_ids: Vec<i64> = workflows_array
        .iter()
        .filter_map(|w| w.get("id").and_then(|id| id.as_i64()))
        .collect();

    assert!(
        found_ids.contains(&wf_a_id),
        "wf-user should see workflow A (owned), found_ids={:?}",
        found_ids
    );
    assert!(
        found_ids.contains(&wf_b_id),
        "wf-user should see workflow B (group-shared), found_ids={:?}",
        found_ids
    );
    assert!(
        !found_ids.contains(&wf_c_id),
        "wf-user should NOT see workflow C (no access), found_ids={:?}",
        found_ids
    );
}
