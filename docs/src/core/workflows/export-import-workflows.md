# How to Export and Import Workflows

This guide shows how to export workflows to portable JSON files and import them into the same or
different Torc servers. This is useful for:

- **Backup and restore**: Save workflow definitions for disaster recovery
- **Migration**: Move workflows between development, staging, and production environments
- **Sharing**: Share workflow templates with teammates or the community
- **Duplication**: Create copies of workflows for testing or experimentation

## Exporting Workflows

### Basic Export

Export a workflow to a JSON file:

```bash
torc workflows export 123 --output my_workflow.json
```

This creates a self-contained JSON document containing:

- Workflow metadata
- All jobs with their dependencies
- Files and user data
- Resource requirements
- Slurm and local schedulers
- Workflow actions

### Export with Results

Include job results (stdout, stderr, return codes) in the export:

```bash
torc workflows export 123 --output my_workflow.json --include-results
```

### Export with Events

Include workflow events (job status changes, scheduler events):

```bash
torc workflows export 123 --output my_workflow.json --include-events
```

### Export with Everything

Include both results and events:

```bash
torc workflows export 123 --output my_workflow.json --include-results --include-events
```

### Export to Stdout

Omit `--output` to write to stdout (useful for piping):

```bash
torc workflows export 123 > my_workflow.json
```

### JSON Output Format

Use `--format json` for machine-readable output with export statistics:

```bash
torc workflows export 123 --output my_workflow.json --format json
```

Output:

```json
{
  "success": true,
  "workflow_id": 123,
  "workflow_name": "my_workflow",
  "output_file": "my_workflow.json",
  "jobs": 5,
  "files": 3,
  "user_data": 2,
  "results": 0,
  "events": 0
}
```

## Importing Workflows

### Basic Import

Import a workflow from a JSON file:

```bash
torc workflows import my_workflow.json
```

Output:

```
Successfully imported workflow:
  Workflow ID: 456
  Name: my_workflow
  Jobs: 5
  Files: 3
  User data: 2
```

### Import with Custom Name

Override the workflow name during import:

```bash
torc workflows import my_workflow.json --name "new_workflow_name"
```

### Skip Results During Import

If the export includes results but you don't want to import them:

```bash
torc workflows import my_workflow.json --skip-results
```

### Skip Events During Import

If the export includes events but you don't want to import them:

```bash
torc workflows import my_workflow.json --skip-events
```

### JSON Output Format

Use `--format json` for machine-readable output:

```bash
torc workflows import my_workflow.json --format json
```

Output:

```json
{
  "success": true,
  "workflow_id": 456,
  "workflow_name": "my_workflow",
  "jobs": 5,
  "files": 3,
  "user_data": 2
}
```

## How Import Works

### ID Remapping

When importing, all entity IDs are remapped to new IDs assigned by the target server. This ensures
no conflicts with existing workflows. Cross-references between entities (e.g., job dependencies on
files) are automatically updated to use the new IDs.

### Job Status Reset

Imported jobs always start in the `uninitialized` status, regardless of their status in the exported
file. After import, you need to initialize and run the workflow:

```bash
# Initialize the imported workflow
torc workflows init 456

# Run locally
torc run 456

# Or submit to scheduler
torc submit 456
```

### Default Resource Requirements

Each workflow automatically gets a "default" resource requirements entry. During import, the
exported "default" resource requirements are mapped to the new workflow's default entry.

## Export Format

The export format is a versioned JSON document. Here's the structure:

```json
{
  "export_version": "1.0",
  "exported_at": "2024-01-15T10:30:00Z",
  "workflow": { ... },
  "files": [ ... ],
  "user_data": [ ... ],
  "resource_requirements": [ ... ],
  "slurm_schedulers": [ ... ],
  "local_schedulers": [ ... ],
  "jobs": [ ... ],
  "workflow_actions": [ ... ],
  "results": [ ... ],
  "events": [ ... ]
}
```

The `results` and `events` fields are only present when `--include-results` or `--include-events`
are specified.

## Common Workflows

### Backup All Active Workflows

```bash
for id in $(torc workflows list --format json | jq -r '.items[].id'); do
  torc workflows export $id --output "backup_workflow_${id}.json"
done
```

### Migrate to Another Server

```bash
# On source server
torc workflows export 123 --output workflow.json

# On target server (different TORC_API_URL)
export TORC_API_URL="http://new-server:8080/torc-service/v1"
torc workflows import workflow.json
```

### Clone a Workflow for Testing

```bash
# Export existing workflow
torc workflows export 123 --output original.json

# Import as a new workflow with different name
torc workflows import original.json --name "test_copy"
```

## Troubleshooting

### Import Fails with "File not found"

Ensure the export file exists and the path is correct:

```bash
ls -la my_workflow.json
torc workflows import ./my_workflow.json
```

### Import Fails with API Error

Check that:

1. The Torc server is running and accessible
2. You have permission to create workflows
3. The export file is valid JSON (not corrupted)

Validate the export file:

```bash
python -m json.tool my_workflow.json > /dev/null && echo "Valid JSON"
```

### Jobs Not Running After Import

Imported jobs start in `uninitialized` status. You must initialize the workflow:

```bash
torc workflows init 456
```

Then check job status:

```bash
torc jobs list 456
```

Jobs should now show `ready` or `blocked` status depending on their dependencies.
