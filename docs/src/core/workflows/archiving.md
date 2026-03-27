# Archiving Workflows

Workflow archiving provides a way to hide completed or inactive workflows from default list views
while preserving all workflow data and execution history. Archived workflows remain fully accessible
but don't clutter everyday workflow management operations.

## Purpose and Motivation

As projects mature and accumulate workflows over time, the list of active workflows can become
difficult to navigate. Archiving addresses this by:

- **Reducing visual clutter** - Completed workflows no longer appear in default list views
- **Preserving historical data** - All workflow data, jobs, results, and logs remain accessible
- **Improving usability** - Users can focus on active workflows without losing access to past work
- **Maintaining audit trails** - Archived workflows can be retrieved for analysis, debugging, or
  compliance

Archiving is particularly useful for:

- Completed experiments that may need future reference
- Successful production runs that serve as historical records
- Development workflows that are no longer active but contain valuable examples
- Workflows from completed projects that need to be retained for documentation

## How It Works

When you archive a workflow, it's marked with an "archived" flag. This flag controls whether the
workflow appears in default list views:

- **Active workflows** (not archived): Appear in standard `workflows list` commands
- **Archived workflows**: Hidden from default lists but accessible with the `--archived-only` flag

The archive status is just metadata - it doesn't affect the workflow's data, results, or any other
functionality.

## Archiving Workflows

Use the `workflows archive` command to archive or unarchive workflows:

```bash
# Archive a specific workflow
torc workflows archive true <workflow_id>

# Archive multiple workflows
torc workflows archive true 123 456 789

# Interactive selection (prompts user to choose)
torc workflows archive true

# With JSON output
torc --format json workflows archive true <workflow_id>
```

The command will output confirmation messages:

```
Successfully archived workflow 123
Successfully archived workflow 456
Successfully archived workflow 789
```

## Unarchiving Workflows

To restore an archived workflow to active status, use the same command with `false`:

```bash
# Unarchive a specific workflow
torc workflows archive false <workflow_id>

# Unarchive multiple workflows
torc workflows archive false 123 456 789

# Interactive selection
torc workflows archive false
```

Output:

```
Successfully unarchived workflow 123
```

## Viewing Workflows

### Default Behavior

By default, the `workflows list` command shows only non-archived workflows:

```bash
# Shows active (non-archived) workflows only
torc workflows list

# Shows active workflows for a specific user
torc workflows list --user alice
```

### Viewing Archived Workflows

Use the `--archived-only` flag to see archived workflows:

```bash
# List only archived workflows for current user
torc workflows list --archived-only
```

### Viewing All Workflows

Use the `--include-archived` flag to see all workflows:

```bash
torc workflows list --include-archived
```

### Accessing Specific Workflows

You can always access a workflow directly by its ID, regardless of archive status:

```bash
# Get details of any workflow (archived or not)
torc workflows get <workflow_id>

# Check workflow status
torc status <workflow_id>
```

## Impact on Workflow Operations

### Operations Restricted on Archived Workflows

Certain workflow operations are not allowed on archived workflows to prevent accidental
modifications:

- ❌ **Status reset**: Cannot use `workflows reset-status` on archived workflows
  - Error message: "Cannot reset archived workflow status. Unarchive the workflow first."
  - To reset status, unarchive the workflow first, then reset

### Interactive Selection Behavior

When commands prompt for interactive workflow selection (when workflow ID is not specified),
archived workflows are excluded by default:

```bash
# These will NOT show archived workflows in the interactive menu
torc-client workflows delete
torc-client workflows status
torc-client workflows initialize
```

This prevents accidentally operating on archived workflows while still allowing explicit access by
ID.

## Archive vs. Delete

Understanding when to archive versus delete workflows:

| Operation   | Data Preserved | Reversible | Use Case                                          |
| ----------- | -------------- | ---------- | ------------------------------------------------- |
| **Archive** | ✅ Yes         | ✅ Yes     | Completed workflows you may reference later       |
| **Delete**  | ❌ No          | ❌ No      | Failed experiments, test workflows, unwanted data |

**Archive when:**

- Workflow completed successfully and may need future reference
- Results should be preserved for reproducibility or compliance
- Workflow represents a milestone or important historical run
- You want to declutter lists but maintain data integrity

**Delete when:**

- Workflow failed and results are not useful
- Workflow was created for testing purposes only
- Data is no longer needed and storage space is a concern
- Workflow contains errors that would confuse future users

## Common Use Cases

### Completed Experiments

After completing an experiment and validating results:

```bash
# Archive the completed experiment
torc-client workflows archive true 123

# Later, if you need to reference it
torc-client workflows get 123
torc-client results list 123
```

### Development Cleanup

Clean up development workflows while preserving examples:

```bash
# Delete test workflows
torc-client workflows delete 301 302 303

# Archive useful development examples
torc-client workflows archive true 304 305
```

### Periodic Maintenance

Regularly archive old workflows to keep lists manageable:

```bash
# List workflows, identify completed ones
torc-client workflows list

# Archive workflows from completed projects
torc workflows archive true 401 402 403 404 405
```

## Best Practices

### When to Archive

1. **After successful completion** - Archive workflows once they've completed successfully and been
   validated
2. **Project milestones** - Archive workflows representing project phases or releases
3. **Regular cleanup** - Establish periodic archiving of workflows older than a certain timeframe
4. **Before major changes** - Archive working versions before making significant modifications

## Summary

Workflow archiving provides a simple, reversible way to hide completed or inactive workflows from
default views while preserving all data and functionality. It's designed for long-term workflow
management in active projects where historical data is valuable but visual clutter is undesirable.

**Key points:**

- Archive workflows with: `torc workflows archive true <id>`
- Unarchive workflows with: `torc workflows archive false <id>`
- Archived workflows are hidden from default lists but remain fully functional
- View archived workflows with: `torc workflows list --archived-only`
- Archiving is reversible and does not affect data storage
- Use archiving for completed workflows; use deletion for unwanted data
