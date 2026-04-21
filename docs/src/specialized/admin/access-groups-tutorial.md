# Tutorial: Team-Based Access Control with Access Groups

This tutorial walks you through setting up team-based access control so that workflows can be shared
within teams while remaining private from other users.

## Learning Objectives

By the end of this tutorial, you will:

- Understand how Torc's access control system works
- Set up authentication with htpasswd files
- Create access groups and add team members
- Share workflows with specific teams
- Enable access control enforcement on the server

## Prerequisites

- Torc server and CLI installed
- Basic familiarity with the command line
- Administrative access to start/restart the server

## Scenario

You're setting up Torc for an organization with two teams:

- **ML Team**: Alice and Bob work on machine learning workflows
- **Data Team**: Carol and Dave work on data processing workflows

Each team should only be able to see and manage their own workflows, but some workflows may need to
be shared between teams.

## Step 1: Create an htpasswd File

First, create an htpasswd file with user credentials. Torc uses bcrypt-hashed passwords for
security.

```bash
# Create the htpasswd directory
mkdir -p /etc/torc

# Add users using torc-htpasswd utility
torc-htpasswd -c /etc/torc/htpasswd alice
# Enter password when prompted

torc-htpasswd /etc/torc/htpasswd bob
torc-htpasswd /etc/torc/htpasswd carol
torc-htpasswd /etc/torc/htpasswd dave
```

Verify the file was created:

```bash
cat /etc/torc/htpasswd
```

Expected output (hashes will differ):

```
alice:$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN82lfIptSLnyJqRZaJ/K
bob:$2b$12$...
carol:$2b$12$...
dave:$2b$12$...
```

## Step 2: Start the Server with Authentication and Access Control

Start the server with authentication required, access control enforcement enabled, and Alice as an
admin user:

```bash
torc-server run \
  --database /var/lib/torc/torc.db \
  --auth-file /etc/torc/htpasswd \
  --require-auth \
  --enforce-access-control \
  --admin-user alice
```

You should see:

```
INFO  Starting torc-server version=0.8.0 (abc1234)
INFO  Loaded 4 users from htpasswd file
INFO  Authentication is REQUIRED for all requests
INFO  Access control is ENABLED - users can only access their own workflows and workflows shared via access groups
INFO  Admin users configured: ["alice"]
INFO  Listening on localhost:8080
```

**Note:** The `--admin-user` flag specifies users who can create and manage access groups. Only
admin users can create, delete, or modify groups.

## Step 3: Configure CLI Authentication

Set up credentials for each user. In a new terminal:

```bash
# Set the API URL
export TORC_API_URL="http://localhost:8080/torc-service/v1"

# Set credentials for Alice
read -s TORC_PASSWORD && export TORC_PASSWORD
# It will prompt you for the password with displaying it.

# Verify connection
torc ping
```

Expected output:

```json
{ "status": "ok" }
```

## Step 4: Create Access Groups

As Alice (who is an admin user), create the two team groups:

```bash
# Create the ML team group (requires admin access)
torc access-groups create "ml-team" --description "Machine Learning Team"
```

Output:

```
Successfully created access group:
  ID: 1
  Name: ml-team
  Description: Machine Learning Team
```

```bash
# Create the Data team group
torc access-groups create "data-team" --description "Data Processing Team"
```

Output:

```
Successfully created access group:
  ID: 2
  Name: data-team
  Description: Data Processing Team
```

List the groups to verify:

```bash
torc access-groups list
```

Output:

```
╭────┬────────────┬─────────────────────────╮
│ ID │ Name       │ Description             │
├────┼────────────┼─────────────────────────┤
│ 1  │ ml-team    │ Machine Learning Team   │
│ 2  │ data-team  │ Data Processing Team    │
╰────┴────────────┴─────────────────────────╯
```

## Step 5: Add Team Members

Add users to their respective teams:

```bash
# Add Alice and Bob to the ML team
torc access-groups add-user 1 alice
torc access-groups add-user 1 bob

# Add Carol and Dave to the Data team
torc access-groups add-user 2 carol
torc access-groups add-user 2 dave
```

Verify team membership:

```bash
# List ML team members
torc access-groups list-members 1
```

Output:

```
╭───────────┬────────╮
│ User Name │ Role   │
├───────────┼────────┤
│ alice     │ member │
│ bob       │ member │
╰───────────┴────────╯
```

```bash
# Check which groups Alice belongs to
torc access-groups list-user-groups alice
```

Output:

```
╭────┬─────────┬───────────────────────╮
│ ID │ Name    │ Description           │
├────┼─────────┼───────────────────────┤
│ 1  │ ml-team │ Machine Learning Team │
╰────┴─────────┴───────────────────────╯
```

## Step 6: Create Workflows as Different Users

Now let's create workflows and see how access control works.

### As Alice (ML Team)

```bash
export TORC_PASSWORD="alice_password"

# Create a workflow
cat > /tmp/ml_training.yaml << 'EOF'
name: ml_training_workflow
description: Train a machine learning model

jobs:
  - name: train_model
    command: echo "Training model..."
    resource_requirements: small

resource_requirements:
  - name: small
    num_cpus: 1
    memory: 1g
    runtime: PT10M
EOF

WORKFLOW_ID=$(torc workflows create /tmp/ml_training.yaml -f json | jq -r '.id')
echo "Alice created workflow: $WORKFLOW_ID"
```

### As Carol (Data Team)

```bash
export TORC_PASSWORD="carol_password"

# Create a different workflow
cat > /tmp/data_pipeline.yaml << 'EOF'
name: data_pipeline
description: Process incoming data

jobs:
  - name: process_data
    command: echo "Processing data..."
    resource_requirements: small

resource_requirements:
  - name: small
    num_cpus: 1
    memory: 1g
    runtime: PT10M
EOF

torc workflows create /tmp/data_pipeline.yaml
```

## Step 7: Observe Access Control in Action

### Carol Cannot Access Alice's Workflow

Still as Carol, try to access Alice's workflow:

```bash
# Try to get Alice's workflow (assuming ID 1)
torc workflows get 1
```

Output:

```json
{
  "error": "Forbidden",
  "message": "User 'carol' does not have access to workflow 1"
}
```

### Carol Can Only See Her Own Workflows

```bash
torc workflows list
```

Output:

```
╭────┬───────────────┬─────────────────────────┬───────╮
│ ID │ Name          │ Description             │ User  │
├────┼───────────────┼─────────────────────────┼───────┤
│ 2  │ data_pipeline │ Process incoming data   │ carol │
╰────┴───────────────┴─────────────────────────┴───────╯
```

Carol only sees her own workflow, not Alice's.

## Step 8: Share a Workflow with Another Team

Sometimes workflows need to be shared between teams. Alice can share her workflow with the Data
team.

### As Alice, Share the Workflow

```bash
export TORC_PASSWORD="alice_password"

# Share workflow 1 with the data team (group 2)
torc access-groups add-workflow 1 2

echo "Shared workflow 1 with data-team"
```

### Verify the Sharing

```bash
# List groups that have access to workflow 1
torc access-groups list-workflow-groups 1
```

Output:

```
╭────┬────────────┬─────────────────────────╮
│ ID │ Name       │ Description             │
├────┼────────────┼─────────────────────────┤
│ 2  │ data-team  │ Data Processing Team    │
╰────┴────────────┴─────────────────────────╯
```

### Carol Can Now Access the Shared Workflow

```bash
export TORC_PASSWORD="carol_password"

# Now Carol can access the workflow
torc workflows get 1
```

Output:

```
╭────────────────────────────────────────┬────────────────────────────╮
│ Field                                  │ Value                      │
├────────────────────────────────────────┼────────────────────────────┤
│ ID                                     │ 1                          │
│ Name                                   │ ml_training_workflow       │
│ User                                   │ alice                      │
│ Description                            │ Train a machine learning   │
│                                        │ model                      │
╰────────────────────────────────────────┴────────────────────────────╯
```

Carol can now see and interact with Alice's workflow because she's a member of the data-team, which
has been granted access.

## Step 9: Revoke Access

If you need to remove access:

```bash
export TORC_PASSWORD="alice_password"

# Remove the data team's access to workflow 1
torc access-groups remove-workflow 1 2

echo "Revoked data-team access to workflow 1"
```

Now Carol can no longer access the workflow.

## Access Control Summary

Here's how access is determined:

```
Can user access workflow?
├── Is user the workflow owner? → YES → ALLOWED
├── Is user in a group with access to this workflow? → YES → ALLOWED
└── Otherwise → DENIED
```

### Access Rules

1. **Ownership**: Users always have access to workflows they created
2. **Group Membership**: Users have access to workflows shared with any group they belong to
3. **No Inheritance**: Access is explicit—being in one group doesn't grant access to another group's
   workflows

## Configuration Reference

### Server Flags

| Flag                       | Description                               |
| -------------------------- | ----------------------------------------- |
| `--auth-file`              | Path to htpasswd file                     |
| `--require-auth`           | Require authentication for all requests   |
| `--enforce-access-control` | Enable access control enforcement         |
| `--admin-user`             | Add user to admin group (can be repeated) |

### Configuration File

You can also configure these in `config.toml`:

```toml
[server]
auth_file = "/etc/torc/htpasswd"
require_auth = true
enforce_access_control = true
admin_users = ["alice", "bob"]
```

## Troubleshooting

### "Anonymous access not allowed"

This error appears when:

- No credentials are provided
- `--require-auth` is enabled

Solution: Set the `TORC_PASSWORD` environment variable.

### "User is not a system administrator"

This error appears when trying to create, delete, or modify access groups without admin privileges.

Solution: Either:

1. Add the user to the admin group in the server configuration using `--admin-user` or `admin_users`
   in config.toml
2. Use an account that is already an admin

### "User does not have access to workflow"

This error appears when:

- The user is not the workflow owner
- The user is not in any group with access to the workflow
- `--enforce-access-control` is enabled

Solution: Either the workflow owner needs to share it with a group the user belongs to, or add the
user to an appropriate group.

### Authentication Working but Access Control Not Enforced

Check that `--enforce-access-control` flag is set when starting the server.

## What You Learned

In this tutorial, you learned:

- How to create an htpasswd file with user credentials
- How to start the server with authentication and access control
- How to create and manage access groups
- How to add users to groups
- How to share workflows with teams
- How access control decisions are made

## Next Steps

- Learn about [Configuration Files](./configuration.md) to set up persistent configuration
- Explore [Server Deployment](./server-deployment.md) for production setups
- See the [Access Groups Reference](./access-groups.md) for all available commands
