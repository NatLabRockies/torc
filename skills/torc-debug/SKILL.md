---
name: torc-debug
description: >
  Diagnose failed, stuck, orphaned, or unscheduled Torc jobs and workflows. Use for non-zero return
  codes, OOM or timeout kills, missing or empty logs, jobs stuck in running or pending, ready jobs
  that never start, empty results, log bundling and analysis, Slurm sacct correlation, offline-drain
  reconciliation, and server connectivity failures. Do not use for authoring specs or routine state
  queries.
license: BSD-3-Clause
---

# Torc debugging

Diagnose from server state first, then logs, then the cluster. Most apparent Torc failures are one
of four things: a job that exited non-zero, a job the system killed for resources, state that no
longer matches reality, or a runner that never got work.

Never conclude a cause from a single symptom. Confirm with the return code, the log content, and the
resource evidence.

## Triage

Start with the symptom, not the tool.

| Symptom                                   | First command                                                | Then read                        |
| ----------------------------------------- | ------------------------------------------------------------ | -------------------------------- |
| Jobs report `failed`                      | `torc results list <id> --failed`                            | `references/failure-analysis.md` |
| Jobs report `terminated`                  | `torc results list <id> --failed` (look for 137/152)         | `references/failure-analysis.md` |
| Cannot find or read logs                  | `torc results list <id> --include-logs -o <output-dir>`      | `references/log-map.md`          |
| Jobs stuck `running` with no allocation   | `torc workflows sync-status <id> --dry-run`                  | `references/stuck-workflows.md`  |
| Ready jobs never start despite free nodes | `torc workflows diagnose <id>`                               | `references/stuck-workflows.md`  |
| Everything `blocked`, nothing ready       | `torc status <id>` then `torc job-dependencies job-job <id>` | `references/stuck-workflows.md`  |
| No results at all                         | `torc status <id>`                                           | `references/stuck-workflows.md`  |
| CLI cannot reach the server               | `torc ping`                                                  | `references/connectivity.md`     |
| Results missing after a server outage     | `torc workflows reconcile <id> <run_id>`                     | `references/connectivity.md`     |

## Standard investigation

```bash
torc status <id>                                        # what the server believes
torc results list <id> --failed                         # exit codes for failures
torc results list <id> --failed --include-logs -o <out> # resolved log paths, as JSON
torc workflows check-resources <id> --include-failed    # OOM/timeout evidence
torc logs analyze <out> --workflow-id <id>              # pattern scan across all logs
torc jobs get <job_id>                                  # exact command to reproduce
```

Five checks, in this order, answer most cases:

1. **Status counts.** `failed` versus `terminated` versus `canceled` already separates "the job
   errored" from "the system stopped it" from "it never ran".
2. **Return code.** 137 means SIGKILL, usually OOM. 152 means the step hit its walltime. 127 means
   the command was not found, which is almost always a PATH or module problem, not a bug in the
   code.
3. **stderr.** Read the actual error before theorizing. `--include-logs` gives the resolved path.
4. **Resource evidence.** `check-resources --include-failed` shows whether the job exceeded its
   declared memory, CPU, or runtime.
5. **Reproduce.** `torc jobs get <job_id>` prints the exact command. Run it by hand in the same
   environment before changing the workflow.

## Traps

- **`torc run` exits 0 with failed jobs.** Never infer success from the exit status of a local run.
  Check `torc status` or `torc results list --failed`. `torc exec` and `torc watch` do exit
  non-zero.
- **`-o` must match the run.** Log-reading commands default to `torc_output`. Point them at the
  directory actually used, or every path warns as missing.
- **`reinit` bumps `run_id`, and logs are per run.** Collect logs before reinitializing, or use
  `--all-runs`.
- **`delete_on_success` and `stdio` modes remove logs by design.** An absent `.o`/`.e` may be
  configuration, not loss.
- **Retries write new files.** `attempt_id` is in the filename, so compare the attempt you mean.
- **A successful `sbatch` is not a successful job.** Slurm accepting an allocation says nothing
  about the payload.
- **Omitting the workflow ID prints a selection table on stdout** and breaks `-f json` parsing; it
  exits 1 on EOF.

## Reporting a diagnosis

State the symptom, the commands run, the return codes and log excerpts that identify the cause, the
resource evidence, and the specific fix. Distinguish what you observed from what you inferred, and
say so plainly when logs were missing or inconclusive.

For sharing with someone else, bundle the evidence:

```bash
torc logs bundle <id> -o <output-dir> --bundle-dir ./bundles
torc logs analyze ./bundles/wf<id>.tar.gz
```
