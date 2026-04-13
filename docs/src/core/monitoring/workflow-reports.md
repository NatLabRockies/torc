# How to Generate Workflow Reports

This guide shows how to check workflow completion status and generate summary reports.

## Check if a Workflow is Complete

Before generating reports, verify that your workflow has finished:

```bash
torc workflows is-complete <workflow_id>
```

If you omit the workflow ID, you'll be prompted to select from your workflows:

```bash
torc workflows is-complete
```

Example output:

```
Workflow 42 completion status:
  Is Complete: true
  Is Canceled: false
  Needs Completion Script: false
```

For JSON output:

```bash
torc -f json workflows is-complete <workflow_id>
```

## Generate a Workflow Summary

Once a workflow is complete, generate a summary report:

```bash
torc status <workflow_id>
```

If you omit the workflow ID, you'll be prompted to select from your workflows:

```bash
torc status
```

Example output:

```
Workflow Summary
================

Workflow ID: 42
Name: data_processing_pipeline
User: jsmith

Job Status (total: 100):
  Completed:     95 ✓
  Failed:        5 ✗

Total Execution Time: 2h 30m 15s
Walltime:             3h 15m 42s
```

If all jobs succeeded:

```
Workflow Summary
================

Workflow ID: 42
Name: simulation_run
User: jsmith

Job Status (total: 50):
  Completed:     50 ✓

Total Execution Time: 45m 30s

✓ All jobs completed successfully!
```

Only non-zero status counts are displayed.

### Continuous Monitoring

This command can be very convenient, but be mindful of your workflow size (number of jobs) and
network load if you are using a shared server.

```bash
watch -n 10 torc status <workflow_id>
```

### JSON Output

This is useful for scripts:

```bash
torc -f json reports summary <workflow_id>
```

```json
{
  "workflow_id": 42,
  "workflow_name": "data_processing_pipeline",
  "workflow_user": "jsmith",
  "total_jobs": 100,
  "jobs_by_status": {
    "uninitialized": 0,
    "blocked": 0,
    "ready": 0,
    "pending": 0,
    "running": 0,
    "completed": 95,
    "failed": 5,
    "canceled": 0,
    "terminated": 0,
    "disabled": 0
  },
  "total_exec_time_minutes": 150.25,
  "total_exec_time_formatted": "2h 30m 15s",
  "walltime_seconds": 11742.0,
  "walltime_formatted": "3h 15m 42s"
}
```

## Use in Scripts

Combine these commands in automation scripts:

```bash
#!/bin/bash
WORKFLOW_ID=$1

# Check completion status
if torc -f json workflows is-complete "$WORKFLOW_ID" | jq -e '.is_complete' > /dev/null; then
    echo "Workflow complete, generating summary..."
    torc -f json reports summary "$WORKFLOW_ID" > "summary_${WORKFLOW_ID}.json"
else
    echo "Workflow not yet complete"
    exit 1
fi
```

## Check Resource Utilization

After a workflow completes, check if any jobs exceeded their resource limits:

```bash
torc workflows check-resources <workflow_id>
```

Example output when jobs stayed within limits:

```
Resource Utilization Report for Workflow 42
===========================================

All 50 jobs completed within resource limits.
```

Example output when jobs exceeded limits:

```
Resource Utilization Report for Workflow 42
===========================================

Jobs exceeding resource limits:

Job ID  Name           Memory Limit  Peak Memory  Status
------  -------------  ------------  -----------  ------
123     train_model_1  16g           18.2g        EXCEEDED
124     train_model_2  16g           17.8g        EXCEEDED

Recommendation: Increase memory allocation for affected jobs.
```

This helps identify jobs that may have been killed due to out-of-memory conditions or that are at
risk of failure in future runs.

## Related Commands

- `torc status <id>` - View current job status counts
- `torc results list <id>` - List individual job results
- `torc workflows check-resources <id>` - Check for resource violations
- `torc results list --include-logs <id>` - Generate detailed results with log file paths

## Next Steps

- [Resource Monitoring](./resource-monitoring.md) - Track CPU and memory usage
- [Debugging Workflows](./debugging.md) - Troubleshoot failed jobs
