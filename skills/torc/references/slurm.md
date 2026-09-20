# Slurm and HPC

## Contents

- [Two submission paths](#two-submission-paths)
- [Discovery before submission](#discovery-before-submission)
- [Generating schedulers](#generating-schedulers)
- [Submitting](#submitting)
- [Allocation sizing](#allocation-sizing)
- [Exit codes and timeouts](#exit-codes-and-timeouts)
- [Login-node discipline](#login-node-discipline)

## Two submission paths

`torc submit` fires the workflow's pending `schedule_nodes` actions. It cannot invent them.

| Spec state                                           | Path                                                           |
| ---------------------------------------------------- | -------------------------------------------------------------- |
| Has `slurm_schedulers` and a `schedule_nodes` action | `torc submit spec.yaml`                                        |
| Has resource requirements but no scheduler config    | `torc slurm generate ... \| torc submit -`                     |
| Has neither                                          | Add resource requirements first; generation has nothing to map |

`torc create --dry-run` tells you which case you are in: it prints either
`Submission: Local execution only (no schedule_nodes action)` or the scheduler and action counts.

## Discovery before submission

```bash
torc hpc detect                      # which built-in profile matches this system
torc hpc list                        # available profiles
torc hpc partitions <profile>        # partitions with CPU/memory/walltime limits
torc hpc match --cpus 32 --memory 64g --walltime 2:00:00
torc hpc generate                    # derive a profile from the live Slurm cluster
```

`torc hpc detect` falls back to live Slurm discovery when no built-in or custom profile matches, and
prints `No known HPC system detected.` only when that also fails. In that case either request
built-in support or define a custom profile under `client.hpc.custom_profiles` (see
`hpc-profiles.md`), then pass `--profile <name>` to `torc slurm generate` explicitly.

`torc slurm plan-allocations <spec>` analyzes the workflow's parallelism against live cluster state
(`sinfo`, `squeue`, and `sbatch --test-only` probes) and recommends whether to use one large
allocation or many small ones. `--offline` skips live queries; `--skip-test-only` skips the probes.
How to read its output is in `optimization.md`.

## Generating schedulers

```bash
torc slurm generate --account <acct> workflow.yaml -o generated.yaml
torc create --dry-run generated.yaml
```

`generate` never edits the input in place. It writes the augmented spec to stdout, or to
`-o <file>`. Submit the generated output, not the original.

Options that change the shape of the result:

| Flag                               | Effect                                                                         |
| ---------------------------------- | ------------------------------------------------------------------------------ |
| `--profile <name>`                 | Skip detection and target a specific HPC profile                               |
| `--group-by partition` (default)   | Jobs mapping to the same partition share one scheduler, sized by the group max |
| `--group-by resource-requirements` | One scheduler per named requirement, isolating distinct resource profiles      |
| `--single-allocation`              | One allocation with all nodes (1xN) instead of one allocation per node (Nx1)   |
| `--walltime-strategy`              | `max-job-runtime` (default, x `--walltime-multiplier`) or `max-partition-time` |
| `--no-actions`                     | Generate schedulers without `schedule_nodes` actions                           |
| `--overwrite`                      | Replace schedulers already in the spec                                         |
| `--dry-run`                        | Show what would be generated without writing                                   |

Generation is heuristic. Review the generated schedulers, actions, and walltimes for any workflow
with unusual dependency structure before submitting production work.

Be careful with the default grouping: merging requirements that share a partition sizes the whole
group from the **maximum** of each dimension, so one large or slow job inflates every allocation in
the group. Fewer schedulers is not the same as fewer allocations. `optimization.md` shows a measured
case where the default produces 52 allocations and `--group-by resource-requirements` produces 6 for
the same work.

Keep the generated spec separate from the source spec, and regenerate rather than hand-editing when
the source changes. Use the direct pipe only when the output needs no review.

## Submitting

```bash
torc submit generated.yaml -o /scratch/$USER/torc-output
torc submit 123 --no-prompts                # existing workflow, non-interactive
torc slurm generate --account acct workflow.yaml | torc submit -
```

- `--no-prompts` skips the review prompt for pending `schedule_nodes` actions on re-submission and
  fires them with their configured allocation counts. A non-TTY stdin implies it.
- `-o`/`--output-dir` must be on a filesystem the compute nodes can write to. Note that the default
  is the relative path `torc_output`, resolved against the submitting directory.
- A successful `sbatch` is not a successful workflow. Follow with `torc status` or
  `torc watch <id>`.

`torc slurm schedule-nodes <id>` submits allocations for an already-configured scheduler without
going through actions. Useful flags: `-n/--num-hpc-jobs`, `-m/--max-parallel-jobs`,
`--start-one-worker-per-node` for direct-mode single-node jobs sharing a multi-node allocation, and
`--keep-submission-scripts` when you need to inspect the generated sbatch script.

Both `torc slurm schedule-nodes` and `torc watch` warn when invoked from a directory other than the
recorded submission directory, because relative output paths are the usual cause of lost outputs.

## Allocation sizing

Packing, walltime, and allocation count all follow arithmetically from resource requirements, and
getting them wrong is the main source of wasted node-hours. `optimization.md` is the reference for
that work: node sharing, splitting by memory and runtime, jobs-per-node versus more nodes, walltime
strategy, `1 x N` versus `N x 1`, and chained allocations.

The minimum to know here:

- `Nx1` (default) lets jobs start as individual nodes become available and tolerates node failures;
  `--single-allocation` needs every node simultaneously.
- Keep an allocation alive across a lull with `compute_node_wait_for_new_jobs_seconds`, and hold it
  past workflow completion with `compute_node_ignore_workflow_completion`.
- `serialize_allocations` chains a scheduler's allocations one at a time and requires a single fixed
  Slurm job name, so `--job-prefix` is rejected for that scheduler.

A ready job will not start on a running allocation whose remaining walltime is shorter than the
job's `runtime`, because it would be killed mid-run, so packing degrades silently as allocations
age:

```bash
torc workflows diagnose <id>
torc -f json workflows diagnose <id>
```

This runs entirely off persisted server state, needs no Slurm access, and reports free
CPU/memory/GPU against remaining walltime. Run it whenever ready jobs are not starting despite
active allocations.

## Exit codes and timeouts

Torc sets per-step walltime with `srun --time` so a step times out before its allocation expires,
turning an ambiguous `CANCELLED` into a distinguishable `TIMEOUT`.

| Scenario                 | Exit | Slurm state     | Torc status | Fix                                                        |
| ------------------------ | ---- | --------------- | ----------- | ---------------------------------------------------------- |
| Out of memory            | 137  | `OUT_OF_MEMORY` | failed      | Raise `memory`, or `torc workflows correct-resources`      |
| Timeout, SIGTERM handled | 0    | `COMPLETED`     | completed   | Reinit and resubmit to continue from the checkpoint        |
| Timeout, SIGKILL         | 152  | `TIMEOUT`       | terminated  | Add a SIGTERM handler, raise runtime, or lengthen walltime |

The graceful case is the surprising one: a job that caught SIGTERM, checkpointed, and exited 0 is
recorded as completed even though its work is unfinished. Reinitialize and resubmit to continue.

Accounting and log inspection:

```bash
torc slurm sacct <id>                 # sacct summary for the workflow's allocations
torc slurm stats <id>                 # per-job sacct stats stored in the database
torc slurm usage <id>                 # total node and CPU time consumed
torc slurm parse-logs --workflow-id <id> <output-dir>
```

## Login-node discipline

Login nodes are for inspection, module discovery, Git and worktree setup, spec validation, and
submission. Do not run builds, installs, solvers, model runs, or payload smoke tests there. Move
compute into a job, an `invocation_script`, or an interactive allocation.

## Re-running part of a Slurm workflow

Because generated schedulers are tied to their jobs via `on_jobs_ready`, resetting a subset re-arms
only those actions:

```bash
torc jobs reset-status <id1> <id2> --reinit
torc submit <workflow_id>
```

A partial reinitialize leaves an `on_workflow_start` `schedule_nodes` action suppressed, so
`torc submit` cannot re-fire it; only a full `torc workflows init` re-arms it. That is why
`on_jobs_ready` is the better trigger. Full rerun semantics live in `rerun-and-recovery.md`.
