# Access Groups

Torc supports team-based access control through access groups. This allows teams to share access to
workflows while restricting access from other teams.

## Overview

Access groups provide:

- **Team-based access control** - Share workflows with specific groups
- **Role-based membership** - Users can be members or admins of groups
- **Multiple group membership** - Users can belong to multiple groups
- **Workflow sharing** - Associate workflows with one or more groups

## Concepts

### Admin Group

The **admin** group is a special system group that controls who can create and manage access groups.
Admin group membership is managed via server configuration, not through the CLI.

- Only members of the admin group can create, delete, or modify access groups
- The admin group is created automatically on server startup
- Admin users are specified via `--admin-user` CLI flag or `admin_users` config option
- The admin group cannot be deleted or have its membership modified via the API

### Access Groups

An access group is a named collection of users who share access to workflows. Groups have:

- **Name** - A unique identifier for the group
- **Description** - Optional description of the group's purpose

### Memberships

Users are added to groups as members. Each membership has:

- **User name** - The username being added
- **Role** - Either "member" or "admin" (for future use)

### Workflow Access

Workflows can be associated with multiple groups. When a workflow is associated with a group, all
members of that group can access the workflow.

## Access Rules

Access to a workflow is granted if any of these conditions are met:

1. **Ownership** - The user created the workflow
2. **Group membership** - The user belongs to a group that has access to the workflow

## CLI Commands

### Group Management

**Note:** Creating, deleting, and modifying access groups requires admin access. Only users who are
members of the admin group can perform these operations.

```bash
# Create a new group (admin only)
torc access-groups create "data-science" --description "Data science team"

# List all groups
torc access-groups list

# Get a specific group
torc access-groups get 1

# Delete a group (admin only)
torc access-groups delete 1
```

### Membership Management

**Note:** Adding and removing users from groups requires admin access or group admin role.

```bash
# Add a user to a group (admin or group admin only)
torc access-groups add-user 1 alice --role member

# List members of a group
torc access-groups list-members 1

# Remove a user from a group (admin or group admin only)
torc access-groups remove-user 1 alice

# List groups a user belongs to
torc access-groups list-user-groups alice
```

### Workflow Access

**Note:** Adding and removing workflows from groups requires workflow ownership or admin access.

```bash
# Add a workflow to a group (owner or admin only)
torc access-groups add-workflow 42 1

# List groups that have access to a workflow
torc access-groups list-workflow-groups 42

# Remove a workflow from a group (owner or admin only)
torc access-groups remove-workflow 42 1
```

## Common Workflows

### Setting Up a Team

As an admin user:

```bash
# 1. Create the team group (requires admin access)
torc access-groups create "ml-team" --description "Machine learning team"
# Output: Successfully created access group:
#   ID: 1
#   Name: ml-team
#   Description: Machine learning team

# 2. Add team members (requires admin access)
torc access-groups add-user 1 alice
torc access-groups add-user 1 bob
```

### Sharing a Workflow with a Team

```bash
# 1. Create a workflow (using any method)
torc create examples/sample_workflow.yaml
# Output: Created workflow 42

# 2. Add the workflow to the team's group
torc access-groups add-workflow 42 1
# Now all members of ml-team (group 1) can access workflow 42
```

### Multi-Team Access

A workflow can be shared with multiple teams:

```bash
# Share with data science team (group 1)
torc access-groups add-workflow 42 1

# Also share with DevOps team (group 2)
torc access-groups add-workflow 42 2

# Both teams can now access the workflow
```

### Checking Group Membership

```bash
# List all members in a group
torc access-groups list-members 1

# List all groups a user belongs to
torc access-groups list-user-groups alice

# List all groups with access to a workflow
torc access-groups list-workflow-groups 42
```

## JSON Output

All commands support JSON output format for scripting:

```bash
# List groups in JSON format
torc access-groups list --format json

# Get group details in JSON
torc access-groups get 1 --format json
```

## Database Schema

Access groups use three tables:

### `access_group`

| Column      | Type    | Description                                     |
| ----------- | ------- | ----------------------------------------------- |
| id          | INTEGER | Primary key                                     |
| name        | TEXT    | Unique group name                               |
| description | TEXT    | Optional description                            |
| is_system   | INTEGER | 1 if system group (cannot be deleted), 0 if not |
| created_at  | TEXT    | Timestamp of creation                           |

### `user_group_membership`

| Column     | Type    | Description                      |
| ---------- | ------- | -------------------------------- |
| id         | INTEGER | Primary key                      |
| user_name  | TEXT    | Username of the member           |
| group_id   | INTEGER | Foreign key to access_group      |
| role       | TEXT    | Role in the group (member/admin) |
| created_at | TEXT    | Timestamp of membership creation |

### `workflow_access_group`

| Column      | Type    | Description                 |
| ----------- | ------- | --------------------------- |
| workflow_id | INTEGER | Foreign key to workflow     |
| group_id    | INTEGER | Foreign key to access_group |
| created_at  | TEXT    | Timestamp of association    |

## Enabling Access Control Enforcement

By default, access groups are not enforced - all authenticated users can access all workflows. To
enable enforcement, start the server with the `--enforce-access-control` flag:

```bash
torc-server run --enforce-access-control --auth-file /path/to/htpasswd
```

When enforcement is enabled:

- Users can only access workflows they own or have group access to
- Anonymous access is denied
- API requests to inaccessible workflows return a 403 Forbidden error
- Only admin group members can create and manage access groups

The enforcement setting can also be configured in the torc configuration file:

```toml
[server]
enforce_access_control = true
```

## Configuring Admin Users

Admin users have permission to create, delete, and modify access groups. Configure admin users via:

### CLI Flag

```bash
torc-server run --admin-user alice --admin-user bob --enforce-access-control
```

### Environment Variable

```bash
export TORC_ADMIN_USERS="alice,bob"
torc-server run --enforce-access-control
```

### Configuration File

```toml
[server]
admin_users = ["alice", "bob"]
enforce_access_control = true
```

On server startup, the admin group is automatically created or updated to include the configured
users. The admin group is a system group that cannot be deleted or modified via the API.

## Future Enhancements

- **Group admin role** - Users with the "admin" role in a group can manage that group's membership
