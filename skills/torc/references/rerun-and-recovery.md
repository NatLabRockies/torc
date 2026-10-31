# Rerunning and recovery

Pick the narrowest tool that matches the reason for rerunning. Reset semantics differ in what they
touch and whether they bump the run ID.

## Contents

- [Choosing a tool](#choosing-a-tool)
- [Inputs changed: reinit](#inputs-changed-reinit)
- [Specific jobs: jobs reset-status](#specific-jobs-jobs-reset-status)
- [All failures: workflows reset-status](#all-failures-workflows-reset-status)
- [Resource failures: recover](#resource-failures-recover)
- [Continuous recovery: watch](#continuous-recovery-watch)
- [Proactive right-sizing](#proactive-right-sizing)
- [Run IDs and logs](#run-ids-and-logs)

## Choosing a tool

| Reason for rerunning                                   | Tool                                                |
| ------------------------------------------------------ | --------------------------------------------------- |
| An input file or user data changed                     | `torc workflows reinit`                             |
| A known set of jobs must rerun                         | `torc jobs reset-status <ids> --reinit`             |
| Every job in a status or with a return code must rerun | `torc jobs reset-status --status` / `--return-code` |
| Every unsuccessful job, no resource change wanted      | `torc workflows reset-status --failed-only`         |
| Slurm failures from OOM or timeout                     | `torc recover`                                      |
| Unattended monitoring with self-healing                | `torc watch --recover`                              |
| Requirements are wrong but nothing needs rerunning     | `torc workflows correct-resources`                  |

Most of these support `--dry-run`; use it first where it exists. The exceptions are
`torc workflows reset-status` and `torc watch --recover`, which have no preview mode -- reset-status
gates on a confirmation prompt instead (`--no-prompts` to skip), and `watch --recover` starts
applying recovery immediately, so preview with `torc recover <id> --dry-run` before handing the
workflow to `watch`.

## Inputs changed: reinit

`torc workflows reinit` is change detection, not failure recovery. It resets jobs that are canceled,
submitting, pending, or terminated, plus completed jobs whose inputs changed, then propagates the
reset downstream.

```bash
torc workflows reinit <id> --dry-run
torc workflows reinit <id>
torc workflows reinit <id> --force        # proceed despite missing data
torc workflows reinit <id> --async        # returns a task handle; wait with torc tasks wait
```

Detection sources: tracked file modification times, changed user data, missing output files, and
changed job definitions. A file whose mtime moved marks its consumers stale, and their consumers in
turn.

`torc workflows init` is the first-time equivalent, run automatically by `torc run` and
`torc submit`. `--force` lets it proceed when input data is missing.

## Specific jobs: jobs reset-status

Selects jobs one of three mutually exclusive ways:

```bash
torc jobs reset-status 101 102 --dry-run                              # explicit IDs (same workflow)
torc jobs reset-status --status terminated,canceled,failed --workflow-id <id>
torc jobs reset-status --return-code 42 --workflow-id <id>            # latest result's exit code
```

Semantics that differ from the workflow-level reset:

- Only the selected jobs are reset. Downstream dependents are listed but not reset until you run
  `torc workflows reinit`, which resets them transitively.
- The workflow `run_id` is not bumped and workflow state is not reset. `reinit` does that once.
  `--reinit` folds both steps into one command.
- A filter that matches nothing exits non-zero with an error, so a wrong assumption fails loudly in
  a script instead of silently doing nothing.
- Resetting a completed job discards its results; the command warns first.
- `--force` bypasses two guards: the no-active-workers check (compute nodes and Slurm allocations)
  and the rejection of jobs currently Running or Pending.

```bash
torc jobs reset-status 101 102 --reinit
torc run <id>                    # or: torc submit <id>
```

The workflow need not be complete, and the command can be run repeatedly as long as no workers are
active.

## All failures: workflows reset-status

```bash
torc workflows reset-status <id> --failed-only              # only failed jobs
torc workflows reset-status <id> --failed-only --reinitialize
torc workflows reset-status <id>                            # every job, full rerun
torc workflows reset-status <id> --force --no-prompts        # ignore active-job check, no prompt
```

The flag is `--failed-only` and the reinit flag is `-r`/`--reinitialize`. Without `--failed-only`
this resets the entire workflow.

`--failed-only` is broader than its name: it resets every job in `failed`, `canceled`, `terminated`,
or `pending_failed`, because status is the source of truth for "did not succeed" and canceled or
terminated jobs may have no result record at all. If you want strictly the jobs whose status is
`failed`, use `torc jobs reset-status --status failed --workflow-id <id>` instead.

Then resume with `torc run <id>` locally or `torc submit <id>` on Slurm.

## Resource failures: recover

`torc recover` is the Slurm path. It cleans up orphans, diagnoses each failure (OOM, timeout,
unknown), adjusts resource requirements, resets the failed jobs, reinitializes, and resubmits
allocations.

```bash
torc recover <id> --dry-run       # diagnose and show proposed changes only
torc recover <id>                 # interactive wizard (default)
torc recover <id> --no-prompts    # apply heuristics automatically
```

Defaults: memory x1.5 for OOM, runtime x1.5 for timeout. Jobs with unknown failure causes are
skipped, because retrying rarely fixes a script or data bug; `--retry-unknown` overrides that, and
`--recovery-hook '<cmd>'` runs custom logic before resetting those jobs (the workflow ID arrives as
an argument and as `TORC_WORKFLOW_ID`).

If `recover` reports active Slurm allocations that `squeue` does not show, clear the stale state
first:

```bash
torc workflows sync-status <id> --dry-run
torc workflows sync-status <id>
```

That marks orphaned running jobs as failed and removes allocations whose Slurm job is gone.

Jobs in `pending_failed` (workflows with `use_pending_failed: true`) need classification, not a
resource bump. `--ai-recovery` with `--ai-agent` invokes an agent CLI to classify them; without it,
reset them manually.

## Continuous recovery: watch

```bash
torc watch <id>                                  # poll until complete, then report
torc watch <id> --recover                        # recover on each round of failures
torc watch <id> --recover --auto-schedule        # also submit allocations as jobs become ready
torc watch <id> --recover -m 3                   # cap recovery attempts
```

`watch` delegates to the same recovery path as `torc recover`, always non-interactive. Its exit
status is meaningful: it exits 1 when the workflow finished with failures and `--recover` is off,
when max retries are exceeded, when recovery cannot make progress, and when only unclassified
`pending_failed` jobs remain. Multipliers are `--memory-multiplier` (1.5) and `--runtime-multiplier`
(1.5), the same defaults `torc recover` and the MCP `recover_workflow` tool use.

`watch` warns when run from a directory other than the recorded submission directory.

## Proactive right-sizing

`torc workflows correct-resources` updates resource requirements from observed usage without
resetting or rerunning anything:

```bash
torc workflows correct-resources <id> --dry-run
torc workflows correct-resources <id>
torc workflows correct-resources <id> --job-ids 12,13 --no-downsize
```

Multipliers default to 1.2 for memory, CPU, and runtime. It both upsizes violations and downsizes
over-allocations, so pass `--no-downsize` when the workflow will grow. Pair with
`torc workflows check-resources <id> --include-failed` to see the evidence first.

## Run IDs and logs

`reinit` bumps the workflow `run_id`, and `run_id` is embedded in every log filename
(`job_wf<id>_j<job>_r<run>_a<attempt>.o`). Collect or read the logs you need **before**
reinitializing, or query them later with `torc results list <id> --all-runs`.

Retries within a run increment `attempt_id` instead, so failure-handler retries keep their own logs.
