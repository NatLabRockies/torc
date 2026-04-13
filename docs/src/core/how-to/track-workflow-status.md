# How to Track Workflow Status

Monitor a running workflow's progress using the CLI, TUI, or dashboard.

## Quick Status Check

```bash
torc status <workflow_id>
```

Example output:

```
Workflow 42: data_pipeline

Jobs by Status:
  Completed:  45
  Running:     5
  Ready:      10
  Blocked:    40
```

## Continuous Monitoring

Watch status update every 10 seconds:

```bash
watch -n 10 torc status <workflow_id>
```

## Interactive TUI

Launch the terminal UI for a visual dashboard:

```bash
torc tui
```

The TUI shows:

- Job status breakdown with progress bars
- Running job details
- Failed job information
- Real-time updates

## List Individual Jobs

View job-level status:

```bash
# All jobs
torc jobs list <workflow_id>

# Filter by status
torc jobs list <workflow_id> --status running
torc jobs list <workflow_id> --status failed
```

## Check Completion

Verify if a workflow has finished:

```bash
torc workflows is-complete <workflow_id>
```

For scripting:

```bash
if torc -f json workflows is-complete "$WORKFLOW_ID" | jq -e '.is_complete' > /dev/null; then
    echo "Workflow complete"
fi
```

## See Also

- [Terminal UI (TUI)](../monitoring/tui.md) — Interactive monitoring
- [Web Dashboard](../monitoring/dashboard.md) — Visual workflow management
- [Workflow Reports](../monitoring/workflow-reports.md) — Generate summary reports
