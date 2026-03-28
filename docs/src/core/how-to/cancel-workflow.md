# How to Cancel a Workflow

Stop a running workflow and terminate its jobs.

## Cancel a Workflow

```bash
torc cancel <workflow_id>
```

This:

- Marks the workflow as canceled
- Stops claiming new jobs
- Sends SIGKILL to all running processes
- Sends `scancel` to all active or pending Slurm allocations

## Check Cancellation Status

Verify the workflow was canceled:

```bash
torc status <workflow_id>
```

Or check completion status:

```bash
torc workflows is-complete <workflow_id>
```

Output:

```
Workflow 42 completion status:
  Is Complete: true
  Is Canceled: true
```

## Restart After Cancellation

To resume a canceled workflow:

```bash
# Reinitialize to reset canceled jobs
torc workflows reinit <workflow_id>

# Run again locally
torc run <workflow_id>
# Or submit to scheduler
torc submit <workflow_id>
```

Jobs that completed before cancellation remain completed.

## See Also

- [Track Workflow Status](./track-workflow-status.md) — Monitor workflow progress
- [Workflow Reinitialization](../concepts/reinitialization.md) — Resume after issues
