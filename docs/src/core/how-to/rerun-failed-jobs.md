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

## Selective Job Reset: `torc jobs reset-status`

When you need to rerun only specific jobs (not all failed ones), use `torc jobs reset-status`. This
is useful when you know exactly which jobs need to be rerun without resetting the whole workflow.

Unlike `torc workflows reset-status`, this command:

- Resets only the explicitly named job IDs. Downstream dependents are **not** reset by this command
  — it lists them for you, and they are reset transitively when you run `torc workflows reinit` (a
  rerun job produces new outputs that consumers must consume again).
- Does **not** bump the workflow `run_id` or reset workflow state — you follow up with
  `torc workflows reinit` once, which does the run_id bump exactly once.
- All supplied job IDs must belong to the same workflow (hard error otherwise).

```bash
# Preview what would be reset (no changes applied)
torc jobs reset-status 101 102 --dry-run

# Reset and reinitialize in one step, then run
torc jobs reset-status 101 102 --reinit
torc run <workflow_id>      # local execution
# or: torc submit <workflow_id>   # Slurm

# Reset and rerun (manual two-step flow)
torc jobs reset-status 101 102 --no-prompts
torc workflows reinit <workflow_id>
torc run <workflow_id>

# Override quiescence check (e.g. workflow still technically running)
torc jobs reset-status 101 --force

# JSON output for scripting
torc -f json jobs reset-status 101 102 --no-prompts
```

The `--force` flag bypasses two checks: (1) the workflow quiescence check (complete/no active
workers), and (2) the active-status guard (jobs in Running or Pending are normally rejected).

## Choosing the Right Tool

| Scenario                                                  | Use                                                                               |
| --------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Slurm workflow with OOM/timeout failures                  | `torc recover`                                                                    |
| Slurm workflow, want continuous self-healing              | `torc watch --recover`                                                            |
| Local workflow with failures                              | `torc workflows reset-status --failed-only`                                       |
| Want to retry without changing resource allocations       | `torc workflows reset-status --failed-only`                                       |
| Rerun only specific known jobs                            | `torc jobs reset-status <id>...`                                                  |
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
