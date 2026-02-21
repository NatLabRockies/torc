# Access Control Test

Manual test for authentication and workflow access control via access groups.

## What It Tests

1. **Private workflows** — only the owner can see them; no other user has access
2. **Team workflows** — members of the assigned access group can see the workflow (via
   `--all-users`)
3. **Cross-team isolation** — users NOT in a group cannot see that group's workflows
4. **Unauthenticated access** — requests without valid credentials are rejected
5. **Direct ID access** — access control applies to `workflows status <id>` too, not just listing

**Note:** `workflows list` defaults to showing only the current user's own workflows. The test uses
`--all-users` to verify that access control correctly includes/excludes team workflows from other
owners.

## Setup

**Users:** alice, bob, carol, dave

**Teams (overlapping membership):**

| Team       | Members     |
| ---------- | ----------- |
| ml_team    | alice, bob  |
| data_team  | bob, carol  |
| infra_team | carol, dave |

Bob bridges ml_team and data_team. Carol bridges data_team and infra_team.

**Workflows:**

| Workflow      | Owner | Access Group | Visible To  |
| ------------- | ----- | ------------ | ----------- |
| ml_experiment | alice | ml_team      | alice, bob  |
| data_pipeline | bob   | data_team    | bob, carol  |
| infra_deploy  | carol | infra_team   | carol, dave |
| alice_private | alice | (none)       | alice only  |
| bob_private   | bob   | (none)       | bob only    |
| carol_private | carol | (none)       | carol only  |
| dave_private  | dave  | (none)       | dave only   |

## Prerequisites

Build the binaries:

```bash
cargo build --release --all-features
export PATH="$(pwd)/target/release:$PATH"
```

## Usage

```bash
bash tests/workflows/access_control_test/run_test.sh
```

The script will:

1. Create a temporary directory with htpasswd file and database
2. Start `torc-server` with `--require-auth --enforce-access-control`
3. Create users, groups, and workflows
4. Run ~41 assertions checking visibility rules and credential hot-reload
5. Print results and stop the server

Use `TORC_TEST_PORT=9090` to set a specific port (defaults to 18321).

## Expected Output

All tests should pass. If any fail, inspect the server logs printed at the end.
