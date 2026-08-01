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

## Cancel from the TUI

Press `C` on a workflow in `torc tui`. This runs the same cancellation as `torc cancel`, including
the `scancel` of outstanding Slurm allocations, and reports how many were canceled.

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
- [Intelligent Restart](../concepts/intelligent-restart.md) — Rerun affected jobs after editing
  inputs
- [Rerun Failed Jobs](./rerun-failed-jobs.md) — Retry jobs that failed
