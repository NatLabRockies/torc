# Failure analysis

## Contents

- [Read the status first](#read-the-status-first)
- [Return codes](#return-codes)
- [Resource evidence](#resource-evidence)
- [Slurm correlation](#slurm-correlation)
- [Reproducing a failure](#reproducing-a-failure)
- [Common causes and fixes](#common-causes-and-fixes)
- [Unclassified failures](#unclassified-failures)

## Read the status first

The status separates three different situations before you look at any log:

| Status           | Meaning                                                                     |
| ---------------- | --------------------------------------------------------------------------- |
| `failed`         | The job ran and exited non-zero                                             |
| `terminated`     | The system stopped it: walltime, resource limit, node loss                  |
| `canceled`       | It never ran (user cancel, or a blocking job failed with cancel-on-failure) |
| `pending_failed` | Failed with no matching failure-handler rule, awaiting classification       |

```bash
torc status <id>
torc jobs list <id> -s failed
torc results list <id> --failed
```

`results list --failed` selects any non-zero return code, which covers both `failed` and
`terminated`. Note that only the latest run is shown unless you pass `--all-runs`.

## Return codes

| Code | Meaning                        | Where to look                                       |
| ---- | ------------------------------ | --------------------------------------------------- |
| 1    | General error                  | job stderr                                          |
| 2    | Shell misuse                   | job command quoting                                 |
| 126  | Not executable                 | permission bits on the script                       |
| 127  | Command not found              | PATH, module load, or interpreter selection         |
| 137  | SIGKILL (128+9), usually OOM   | `check-resources`, dmesg log, Slurm `OUT_OF_MEMORY` |
| 139  | SIGSEGV (128+11)               | job stderr, core dump, dmesg                        |
| 143  | SIGTERM (128+15)               | runner log; something asked the job to stop         |
| 152  | SIGXCPU (128+24), step timeout | Slurm `TIMEOUT`; job exceeded `srun --time`         |

Two Slurm-specific subtleties:

- **Exit 0 can still mean an unfinished job.** With `srun_termination_signal` configured, a job that
  caught SIGTERM, checkpointed, and exited 0 is recorded `completed` even though its work is
  incomplete. Reinitialize and resubmit to continue from the checkpoint.
- **Exit 152 versus a cancelled step.** Torc sets per-step `--time` so a step times out before its
  allocation expires, turning an ambiguous `CANCELLED` into `TIMEOUT` with code 152.

Group codes to see the shape of a failure quickly:

```bash
torc -f json results list <id> \
  | jq '[.items[].return_code] | group_by(.) | map({code: .[0], n: length})'
```

## Resource evidence

```bash
torc workflows check-resources <id> --include-failed
torc workflows check-resources <id> --all
torc workflows check-resources <id> --min-over-utilization 5
```

`--include-failed` is the flag that matters here: failed and terminated jobs are excluded by
default, which is exactly the wrong default when diagnosing them. The report flags jobs that
exceeded declared memory, CPU, or runtime, with the observed peak.

Peak values come from resource monitoring. With monitoring disabled they read zero, and this
analysis cannot help; enable at least `granularity: summary` for future runs.

`torc results list` shows peak memory and peak CPU per execution, so a job whose peak sits at its
declared memory limit and exited 137 is an OOM with two independent pieces of evidence.

## Slurm correlation

```bash
torc slurm sacct <id>                     # live sacct query, needs Slurm CLI access
torc slurm stats <id>                     # per-job stats already stored in the database
torc slurm usage <id>                     # node and CPU time consumed
torc slurm parse-logs --workflow-id <id> <output-dir>
```

Slurm state confirms what Torc inferred: `OUT_OF_MEMORY` with `MaxRSS` at the limit, `TIMEOUT` for a
step that ran out of time, `NODE_FAIL`, or `PREEMPTED`. Step names follow
`wf<id>_j<job>_r<run>_a<attempt>`, so they map directly onto Torc job results.

`torc slurm stats` works anywhere because it reads the database; `sacct` requires a node where Slurm
commands run.

## Reproducing a failure

```bash
torc jobs get <job_id>                    # exact command, resource requirement, handler, scheduler
torc -f json jobs get <job_id> | jq -r '.command'
```

Run that command by hand in the same environment before changing anything. Remember what the runner
sets that your shell does not: `TORC_WORKFLOW_ID`, `TORC_RUN_ID`, `TORC_JOB_ID`, `TORC_JOB_NAME`,
`TORC_ATTEMPT_ID`, `TORC_API_URL`, `TORC_OUTPUT_DIR`, `TORC_WORKFLOW_SUBMISSION_DIR`, plus the
workflow and job `env` maps and any `invocation_script`.

If it reproduces by hand, it is a payload bug and no amount of Torc configuration will fix it. If it
does not, look at the environment difference: module state, working directory, and the fact that
`env` values are literal strings that never see a shell.

## Common causes and fixes

| Evidence                                              | Cause                                | Fix                                                                |
| ----------------------------------------------------- | ------------------------------------ | ------------------------------------------------------------------ |
| 137 plus peak memory at the limit, or dmesg OOM lines | Out of memory                        | Raise `memory`, or `torc workflows correct-resources`              |
| 152, or Slurm `TIMEOUT`                               | Step exceeded its walltime           | Raise `runtime`/walltime, or handle SIGTERM and checkpoint         |
| 127 with a module-based toolchain                     | Environment not loaded on the node   | Load modules in an `invocation_script` ending in `exec "$@"`       |
| `FileNotFoundError` on a predecessor's output         | Missing or wrong dependency edge     | Declare the file edge instead of relying on ordering               |
| Relative path resolved somewhere unexpected           | Runner CWD is not submission dir     | Use absolute paths or `TORC_WORKFLOW_SUBMISSION_DIR`               |
| `$(...)` or `${VAR:-x}` appearing literally           | `env` values are not shell-evaluated | Move the logic into the command or an `invocation_script`          |
| Job killed locally with `limit_resources` on          | Exceeded declared limits             | Correct the requirement, or set `limit_resources: false` knowingly |
| Only some parameterized jobs fail                     | Data-dependent bug                   | Compare failing and passing inputs; the sweep is not at fault      |

After a resource fix, rerun with the narrowest tool: `torc recover` for Slurm OOM/timeout,
`torc jobs reset-status <ids> --reinit` for a known set. See `rerun-and-recovery.md`.

## Unclassified failures

Jobs in `pending_failed` (workflows with `use_pending_failed: true`) failed with no matching
failure-handler rule. They are waiting on a decision, not on resources.

Options: classify with an AI agent (`torc recover <id> --ai-recovery --ai-agent <cli>`,
experimental), use the MCP tools `list_pending_failed_jobs` and `classify_and_resolve_failures`, or
resolve them manually with `torc workflows reset-status <id> --failed-only`.

`torc watch` exits 1 when only `pending_failed` jobs remain, because it cannot decide for you.
