//! Access group management commands for team-based access control

use crate::client::apis;
use crate::client::apis::configuration::Configuration;
use crate::client::commands::output::{print_if_json, print_json_wrapped};
use crate::client::commands::table_format::display_table_with_count;
use crate::models::{AccessGroupModel, UserGroupMembershipModel};
use tabled::Tabled;

// ============================================================================
// Table display types
// ============================================================================

#[derive(Tabled)]
struct GroupTableRow {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Description")]
    description: String,
    #[tabled(rename = "Created")]
    created_at: String,
}

#[derive(Tabled)]
struct MemberTableRow {
    #[tabled(rename = "User")]
    user_name: String,
    #[tabled(rename = "Role")]
    role: String,
    #[tabled(rename = "Added")]
    created_at: String,
}

// ============================================================================
// CLI Command definitions
// ============================================================================

#[derive(clap::Subcommand)]
pub enum AccessGroupCommands {
    /// Create a new access group
    Create {
        /// Name of the group
        #[arg()]
        name: String,
        /// Description of the group
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Get details of an access group
    Get {
        /// ID of the group
        #[arg()]
        id: i64,
    },
    /// List all access groups
    List {
        /// Maximum number of groups to return
        #[arg(short, long, default_value = "100")]
        limit: i64,
        /// Offset for pagination
        #[arg(short, long, default_value = "0")]
        offset: i64,
    },
    /// Delete an access group
    Delete {
        /// ID of the group to delete
        #[arg()]
        id: i64,
    },
    /// Add a user to a group
    AddUser {
        /// ID of the group
        #[arg()]
        group_id: i64,
        /// Username to add
        #[arg()]
        user_name: String,
        /// Role in the group (admin or member)
        #[arg(short, long, default_value = "member")]
        role: String,
    },
    /// Remove a user from a group
    RemoveUser {
        /// ID of the group
        #[arg()]
        group_id: i64,
        /// Username to remove
        #[arg()]
        user_name: String,
    },
    /// List members of a group
    ListMembers {
        /// ID of the group
        #[arg()]
        group_id: i64,
        /// Maximum number of members to return
        #[arg(short, long, default_value = "100")]
        limit: i64,
        /// Offset for pagination
        #[arg(short, long, default_value = "0")]
        offset: i64,
    },
    /// List groups a user belongs to
    ListUserGroups {
        /// Username
        #[arg()]
        user_name: String,
        /// Maximum number of groups to return
        #[arg(short, long, default_value = "100")]
        limit: i64,
        /// Offset for pagination
        #[arg(short, long, default_value = "0")]
        offset: i64,
    },
    /// Add a workflow to a group (grant group access)
    AddWorkflow {
        /// ID of the workflow
        #[arg()]
        workflow_id: i64,
        /// ID of the group
        #[arg()]
        group_id: i64,
    },
    /// Remove a workflow from a group (revoke group access)
    RemoveWorkflow {
        /// ID of the workflow
        #[arg()]
        workflow_id: i64,
        /// ID of the group
        #[arg()]
        group_id: i64,
    },
    /// List groups that have access to a workflow
    ListWorkflowGroups {
        /// ID of the workflow
        #[arg()]
        workflow_id: i64,
    },
}

// ============================================================================
// Command handler
// ============================================================================

pub fn handle_access_group_commands(
    config: &Configuration,
    command: &AccessGroupCommands,
    format: &str,
) {
    match command {
        AccessGroupCommands::Create { name, description } => {
            let mut group = AccessGroupModel::new(name.clone());
            group.description = description.clone();

            match apis::access_control_api::create_access_group(config, group) {
                Ok(group) => {
                    if print_if_json(format, &group, "group") {
                        // JSON was printed
                    } else {
                        println!("Successfully created access group:");
                        println!("  ID: {}", group.id.unwrap_or(-1));
                        println!("  Name: {}", group.name);
                        if let Some(desc) = &group.description {
                            println!("  Description: {}", desc);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error creating access group: {}", e);
                    std::process::exit(1);
                }
            }
        }
        AccessGroupCommands::Get { id } => {
            match apis::access_control_api::get_access_group(config, *id) {
                Ok(group) => {
                    if print_if_json(format, &group, "group") {
                        // JSON was printed
                    } else {
                        println!("Access group:");
                        println!("  ID: {}", group.id.unwrap_or(-1));
                        println!("  Name: {}", group.name);
                        if let Some(desc) = &group.description {
                            println!("  Description: {}", desc);
                        }
                        if let Some(created) = &group.created_at {
                            println!("  Created: {}", created);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error getting access group: {}", e);
                    std::process::exit(1);
                }
            }
        }
        AccessGroupCommands::List { limit, offset } => {
            match apis::access_control_api::list_access_groups(config, Some(*offset), Some(*limit))
            {
                Ok(response) => {
                    if format == "json" {
                        print_json_wrapped("groups", &response.items, "groups");
                    } else if response.items.is_empty() {
                        println!("No access groups found");
                    } else {
                        let rows: Vec<GroupTableRow> = response
                            .items
                            .iter()
                            .map(|g| GroupTableRow {
                                id: g.id.unwrap_or(-1),
                                name: g.name.clone(),
                                description: g.description.clone().unwrap_or_default(),
                                created_at: g.created_at.clone().unwrap_or_default(),
                            })
                            .collect();
                        display_table_with_count(&rows, "access groups");
                    }
                }
                Err(e) => {
                    eprintln!("Error listing access groups: {}", e);
                    std::process::exit(1);
                }
            }
        }
        AccessGroupCommands::Delete { id } => {
            match apis::access_control_api::delete_access_group(config, *id) {
                Ok(group) => {
                    if print_if_json(format, &group, "group") {
                        // JSON was printed
                    } else {
                        println!("Successfully deleted access group:");
                        println!("  ID: {}", group.id.unwrap_or(-1));
                        println!("  Name: {}", group.name);
                    }
                }
                Err(e) => {
                    eprintln!("Error deleting access group: {}", e);
                    std::process::exit(1);
                }
            }
        }
        AccessGroupCommands::AddUser {
            group_id,
            user_name,
            role,
        } => {
            let mut membership = UserGroupMembershipModel::new(user_name.clone(), *group_id);
            membership.role = role.clone();

            match apis::access_control_api::add_user_to_group(config, *group_id, membership) {
                Ok(membership) => {
                    if print_if_json(format, &membership, "membership") {
                        // JSON was printed
                    } else {
                        println!("Successfully added user to group:");
                        println!("  User: {}", membership.user_name);
                        println!("  Group ID: {}", membership.group_id);
                        println!("  Role: {}", membership.role);
                    }
                }
                Err(e) => {
                    eprintln!("Error adding user to group: {}", e);
                    std::process::exit(1);
                }
            }
        }
        AccessGroupCommands::RemoveUser {
            group_id,
            user_name,
        } => match apis::access_control_api::remove_user_from_group(config, *group_id, user_name) {
            Ok(_membership) => {
                if format == "json" {
                    println!("{{\"success\": true}}");
                } else {
                    println!(
                        "Successfully removed user '{}' from group {}",
                        user_name, group_id
                    );
                }
            }
            Err(e) => {
                eprintln!("Error removing user from group: {}", e);
                std::process::exit(1);
            }
        },
        AccessGroupCommands::ListMembers {
            group_id,
            limit,
            offset,
        } => {
            match apis::access_control_api::list_group_members(
                config,
                *group_id,
                Some(*offset),
                Some(*limit),
            ) {
                Ok(response) => {
                    if format == "json" {
                        print_json_wrapped("members", &response.items, "members");
                    } else if response.items.is_empty() {
                        println!("No members found in group {}", group_id);
                    } else {
                        println!("Members of group {}:", group_id);
                        let rows: Vec<MemberTableRow> = response
                            .items
                            .iter()
                            .map(|m| MemberTableRow {
                                user_name: m.user_name.clone(),
                                role: m.role.clone(),
                                created_at: m.created_at.clone().unwrap_or_default(),
                            })
                            .collect();
                        display_table_with_count(&rows, "members");
                    }
                }
                Err(e) => {
                    eprintln!("Error listing group members: {}", e);
                    std::process::exit(1);
                }
            }
        }
        AccessGroupCommands::ListUserGroups {
            user_name,
            limit,
            offset,
        } => match apis::access_control_api::list_user_groups(
            config,
            user_name,
            Some(*offset),
            Some(*limit),
        ) {
            Ok(response) => {
                if format == "json" {
                    print_json_wrapped("groups", &response.items, "groups");
                } else if response.items.is_empty() {
                    println!("User '{}' is not a member of any groups", user_name);
                } else {
                    println!("Groups for user '{}':", user_name);
                    let rows: Vec<GroupTableRow> = response
                        .items
                        .iter()
                        .map(|g| GroupTableRow {
                            id: g.id.unwrap_or(-1),
                            name: g.name.clone(),
                            description: g.description.clone().unwrap_or_default(),
                            created_at: g.created_at.clone().unwrap_or_default(),
                        })
                        .collect();
                    display_table_with_count(&rows, "groups");
                }
            }
            Err(e) => {
                eprintln!("Error listing user groups: {}", e);
                std::process::exit(1);
            }
        },
        AccessGroupCommands::AddWorkflow {
            workflow_id,
            group_id,
        } => match apis::access_control_api::add_workflow_to_group(config, *workflow_id, *group_id)
        {
            Ok(association) => {
                if print_if_json(format, &association, "association") {
                    // JSON was printed
                } else {
                    println!("Successfully added workflow to group:");
                    println!("  Workflow ID: {}", association.workflow_id);
                    println!("  Group ID: {}", association.group_id);
                }
            }
            Err(e) => {
                eprintln!("Error adding workflow to group: {}", e);
                std::process::exit(1);
            }
        },
        AccessGroupCommands::RemoveWorkflow {
            workflow_id,
            group_id,
        } => match apis::access_control_api::remove_workflow_from_group(
            config,
            *workflow_id,
            *group_id,
        ) {
            Ok(_association) => {
                if format == "json" {
                    println!("{{\"success\": true}}");
                } else {
                    println!(
                        "Successfully removed workflow {} from group {}",
                        workflow_id, group_id
                    );
                }
            }
            Err(e) => {
                eprintln!("Error removing workflow from group: {}", e);
                std::process::exit(1);
            }
        },
        AccessGroupCommands::ListWorkflowGroups { workflow_id } => {
            match apis::access_control_api::list_workflow_groups(config, *workflow_id, None, None) {
                Ok(response) => {
                    if format == "json" {
                        print_json_wrapped("groups", &response.items, "groups");
                    } else if response.items.is_empty() {
                        println!("Workflow {} is not associated with any groups", workflow_id);
                    } else {
                        println!("Groups with access to workflow {}:", workflow_id);
                        let rows: Vec<GroupTableRow> = response
                            .items
                            .iter()
                            .map(|g| GroupTableRow {
                                id: g.id.unwrap_or(-1),
                                name: g.name.clone(),
                                description: g.description.clone().unwrap_or_default(),
                                created_at: g.created_at.clone().unwrap_or_default(),
                            })
                            .collect();
                        display_table_with_count(&rows, "groups");
                    }
                }
                Err(e) => {
                    eprintln!("Error listing workflow groups: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
