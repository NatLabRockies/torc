---
name: torc-inspect
description: >
  Query Torc workflow, job, result, dependency, and resource state for reporting, monitoring, and
  scripting. Use for torc status, workflows list/get, jobs list, results list, events, compute
  nodes, execution plans, dependency queries, JSON/CSV output, jq or Nushell filters, the TUI, the
  web dashboard, resource plots, and workflow export. Do not use for diagnosing failures, which
  belongs to torc-debug.
license: BSD-3-Clause
---

# Torc inspection

The server is the single source of truth. Every question about a workflow is answerable from
persisted state without touching Slurm, SSH, or the filesystem, so start there and reach for
external tools only when server state cannot answer the question.

For anything scripted, add `-f json` and pass the workflow ID explicitly.

## Task router

| Task                                                   | Read                            |
| ------------------------------------------------------ | ------------------------------- |
| Which command answers which question                   | `references/query-map.md`       |
| Parse output with jq or Nushell, build reports and CSV | `references/scripting.md`       |
| Watch a live workflow: TUI, dashboard, events, plots   | `references/live-monitoring.md` |

## Start here

```bash
torc ping                        # server reachable
torc workflows list              # your workflows (add --archived-only or -a for all users' where supported)
torc status <id>                 # job counts by status, exec time, completion state
torc jobs list <id> -s failed    # jobs filtered server-side by status
torc results list <id>           # per-execution return codes and resource peaks
```

`torc status` is the cheapest complete picture: job counts per status, total execution time,
walltime, active and pending compute nodes, and whether the workflow is complete or canceled. Its
JSON form is the right thing to poll in a script.

## Rules that keep inspection correct

- **Pass the workflow ID.** With the ID omitted and several workflows present, commands print a
  selection table to **stdout** and read from stdin. That silently corrupts `-f json` output and
  exits 1 on EOF. With exactly one workflow, one is chosen silently, which is worse in a script.
- **Results are per execution, jobs are current state.** `torc jobs list` shows where a job is now;
  `torc results list` shows what each attempt returned. A job can be `ready` again and still have
  failed results from an earlier attempt.
- **Only the latest run by default.** `results list` reports the workflow's current `run_id`. Use
  `--all-runs` for history, or `-r <run_id>` for one run. `run_id` increments on
  `torc workflows reinit`.
- **Filter on the server.** Prefer `-s/--status`, `--failed`, `--return-code`, `-j/--job-id`,
  `-r/--run-id`, `--compute-node` over fetching everything and filtering locally.
- **`-f csv` is list-only.** Single-record commands (`jobs get`) and multi-section reports
  (`status`, `workflows check-resources`) reject it with exit 1. Use `-f json` there.
- **Data on stdout, logs on stderr** for every command except `torc run` and `torc exec`, which put
  runner logs on stdout unless `-f json` is used.

## Status vocabulary

Job statuses and their stored integers: `uninitialized` 0, `blocked` 1, `ready` 2, `pending` 3,
`running` 4, `completed` 5, `failed` 6, `canceled` 7, `terminated` 8, `disabled` 9,
`pending_failed` 10.

Distinctions that matter when reading counts:

- `blocked` means unmet dependencies; `ready` means claimable now.
- `terminated` means the system stopped the job (walltime, resource limit), not that it exited
  non-zero on its own.
- `canceled` jobs never ran.
- `pending_failed` only appears on workflows with `use_pending_failed: true`: the job failed with no
  matching failure-handler rule and awaits classification.
- Some result and filter surfaces use display names (`Done`, `Completed`) rather than the raw
  status; check the value you actually get before matching on it in a script.

## Output

Report the exact query commands, the workflow ID and run ID the numbers came from, the counts or
records observed, and the interpretation. Distinguish current job state from historical results.
