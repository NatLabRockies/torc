# Rerun Failed Jobs

When jobs in a workflow fail, you have several options for retrying them depending on your execution
environment and how much automation you want.

> **Looking to rerun jobs after editing an input file?** That's a different operation — see
> [Intelligent Restart](../concepts/intelligent-restart.md).

## Slurm Workflows: `torc recover`

For Slurm workflows, `torc recover` is the comprehensive option. It diagnoses each failure (OOM,
timeout, unknown), adjusts resource requirements, resets the failed jobs, reinitializes the
workflow, and resubmits Slurm allocations:

```bash
# Preview what recovery would do
torc recover <workflow_id> --dry-run

# Interactive recovery wizard (default)
torc recover <workflow_id>

# Non-interactive recovery (for scripts/CI)
torc recover <workflow_id> --no-prompts
```

For continuous monitoring with auto-recovery, use `torc watch --recover` instead — it polls until
the workflow completes and re-runs recovery on each round of failures.

See [Automatic Failure Recovery](../../specialized/fault-tolerance/automatic-recovery.md) for the
full guide.

## Local Workflows: `torc workflows reset-status`

For local (non-Slurm) workflows, or when you just want to retry without resource adjustment:

```bash
# Reset only failed jobs to ready and rerun
torc workflows reset-status <workflow_id> --failed-only --reinitialize

# Or reset failed jobs without reinitializing (e.g. transient infrastructure issue)
torc workflows reset-status <workflow_id> --failed-only
```

Then resume execution with `torc run <workflow_id>` (local) or `torc submit <workflow_id>` (Slurm).

## Choosing the Right Tool

| Scenario                                                  | Use                                                                               |
| --------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Slurm workflow with OOM/timeout failures                  | `torc recover`                                                                    |
| Slurm workflow, want continuous self-healing              | `torc watch --recover`                                                            |
| Local workflow with failures                              | `torc workflows reset-status --failed-only`                                       |
| Want to retry without changing resource allocations       | `torc workflows reset-status --failed-only`                                       |
| Workflow ran fine but inputs changed                      | [Intelligent Restart](../concepts/intelligent-restart.md)                         |
| Need AI-driven classification of unfamiliar failure modes | [AI-Assisted Recovery](../../specialized/fault-tolerance/ai-assisted-recovery.md) |

## See Also

- [Automatic Failure Recovery](../../specialized/fault-tolerance/automatic-recovery.md) — Full guide
  to `torc recover` and `torc watch --recover`
- [AI-Assisted Recovery](../../specialized/fault-tolerance/ai-assisted-recovery.md) — Classify
  unknown failures with an AI agent
- [Configurable Failure Handlers](../../specialized/fault-tolerance/failure-handlers.md) — Per-job
  retry logic configured in the workflow spec
- [Debug a Failed Job](./debug-failed-job.md) — Investigate why a job failed
- [Intelligent Restart](../concepts/intelligent-restart.md) — Rerun affected jobs after editing
  inputs
