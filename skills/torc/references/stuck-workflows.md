# Stuck workflows

Nothing is failing, but nothing is progressing either. Work from the server's own account of the
world outward.

## Contents

- [Read the counts](#read-the-counts)
- [Everything blocked](#everything-blocked)
- [Ready but never claimed](#ready-but-never-claimed)
- [Ready jobs on live allocations that will not start](#ready-jobs-on-live-allocations-that-will-not-start)
- [Stuck in running or pending](#stuck-in-running-or-pending)
- [No results at all](#no-results-at-all)
- [Cancel and clean up](#cancel-and-clean-up)

## Read the counts

```bash
torc status <id>
torc -f json status <id> | jq '{jobs_by_status, active_compute_nodes, active_scheduled_nodes, pending_scheduled_nodes, runtime_blocked_ready_jobs, is_complete, is_canceled}'
```

The pattern in these fields names the problem:

| Pattern                                                     | Section                                                                               |
| ----------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| Everything `blocked`, nothing `ready`                       | [Everything blocked](#everything-blocked)                                             |
| `ready` > 0, `active_compute_nodes` = 0                     | [Ready but never claimed](#ready-but-never-claimed)                                   |
| `ready` > 0, nodes active, `runtime_blocked_ready_jobs` > 0 | [Ready jobs that will not start](#ready-jobs-on-live-allocations-that-will-not-start) |
| `running`/`pending` > 0 with no active nodes                | [Stuck in running or pending](#stuck-in-running-or-pending)                           |
| Everything `uninitialized`                                  | Workflow was never initialized: `torc workflows init <id>`                            |

## Everything blocked

`blocked` means unmet dependencies. Either a predecessor has not completed, or the graph is not the
one you intended.

```bash
torc workflows execution-plan <id>          # what should run and in what order
torc job-dependencies job-job <id>          # explicit edges
torc job-dependencies job-file <id>         # file-derived edges
torc files list-required-existing <id>      # inputs that must exist before init
```

Frequent causes:

- A regex in `depends_on_regexes` or `input_file_regexes` matches more than intended, adding edges
  you did not plan.
- A required input file does not exist, so nothing becomes ready. Initialize with `--force` only if
  you accept the missing data.
- A cycle or an inverted edge: `execution-plan` on the spec, before creating, shows this
  immediately.

Validate a suspect graph offline: `torc create --dry-run <spec>` and
`torc workflows execution-plan <spec>`.

## Ready but never claimed

Ready jobs need a runner. `active_compute_nodes` at 0 means nobody is polling.

| Mode           | Check                                                            |
| -------------- | ---------------------------------------------------------------- |
| Local          | Is `torc run <id>` actually running?                             |
| Slurm          | `torc scheduled-compute-nodes list <id>`, then `squeue -u $USER` |
| Remote workers | `torc remote status <id>`                                        |

For Slurm, `pending_scheduled_nodes` > 0 means allocations are queued but not yet started, which is
a queue-wait, not a Torc problem. Zero scheduled nodes with ready jobs means no `schedule_nodes`
action fired: check `torc workflows list-actions <id>` and resubmit.

For remote workers, a worker that started and died immediately almost always could not reach the
server. See `connectivity.md`.

If runners are alive but claim nothing, compare each ready job's requirements against what the
runner detected. The runner's startup log line records its resources, `max_parallel_jobs`, and
execution mode. A job asking for more CPU, memory, or GPUs than a worker has will never be claimed
by that worker.

## Ready jobs on live allocations that will not start

A ready job will not start on an allocation whose remaining walltime is shorter than the job's
declared `runtime`, because Torc refuses to start work that would be killed mid-run. Packing
degrades silently as allocations age.

```bash
torc workflows diagnose <id>
torc -f json workflows diagnose <id>
```

This reports free CPU, memory, and GPU against remaining walltime, entirely from persisted state, so
it works without Slurm access. `runtime_blocked_ready_jobs` in `torc status` is the same signal.

Fixes: submit more allocations (`torc submit <id>` again, or
`torc slurm schedule-nodes <id> -n <count>`), request longer walltime for the next round
(`--walltime-strategy max-partition-time`), or lower the affected jobs' `runtime` when the declared
value is unrealistically conservative.

## Stuck in running or pending

Torc marks a job running when a runner claims it. If the runner or its allocation dies ungracefully,
nothing updates the record, so the job stays `running` forever.

```bash
torc workflows sync-status <id> --dry-run     # safe, read-only preview
torc workflows sync-status <id>               # apply
```

This queries Slurm via `squeue`, fails jobs whose allocation has ended, and clears pending
allocations whose Slurm job is no longer queued. Orphaned jobs are recorded with return code `-128`,
which is how you recognize them later in `torc results list`.

Run it when `torc recover` reports active Slurm allocations that `squeue` does not show, when jobs
appear stuck after an allocation ended, or before recovery to clear stale state.

`torc jobs running <id>` shows which node and Slurm job each running job claims, which is what to
cross-check against `squeue`.

After cleanup, recover or reset:

```bash
torc recover <id> --dry-run
torc workflows reset-status <id> --failed-only --reinitialize
```

`--force` on the reset commands bypasses the active-worker guard. Only use it after confirming no
runner is alive; otherwise two runners can execute the same job.

## No results at all

Results appear only after a job execution completes. An empty result set means execution never
finished, not that results were lost.

Check in order: was the workflow initialized (`torc status`), did any runner attach
(`active_compute_nodes`, `torc compute-nodes list <id>`), and did a runner log get written
(`<output-dir>/job_runner_*.log`). A runner log that exists but shows no claims points back to
[Ready but never claimed](#ready-but-never-claimed).

If results existed before and are gone now, a `reinit` bumped `run_id`; use `--all-runs`.

## Cancel and clean up

```bash
torc cancel <id>                  # cancels the workflow and its Slurm allocations
torc workflows reinit <id>        # then resume from where it stopped
```

Cancel preserves all state; the workflow can be resumed after reinitialization. It also cancels the
associated Slurm jobs, so it is the correct way to stop an HPC workflow rather than `scancel` by
hand.
