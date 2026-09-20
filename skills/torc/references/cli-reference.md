# CLI reference

`torc <command> --help` is authoritative for flags and arguments. This file is a lookup index: it
lists every command group so you can find the right command by name, and documents the behavior
`--help` does not state — preconditions, side effects, prompting, and what a command does _not_ do.

Read the entry for a command before running it for the first time in a session.

## Contents

- [Finding commands](#finding-commands)
- [Global options](#global-options)
- [Lifecycle](#lifecycle)
- [workflows](#workflows)
- [jobs](#jobs)
- [results](#results)
- [files and user-data](#files-and-user-data)
- [resource-requirements and failure-handlers](#resource-requirements-and-failure-handlers)
- [slurm](#slurm)
- [hpc](#hpc)
- [remote](#remote)
- [logs](#logs)
- [events](#events)
- [compute-nodes and scheduled-compute-nodes](#compute-nodes-and-scheduled-compute-nodes)
- [job-dependencies](#job-dependencies)
- [access-groups](#access-groups)
- [ro-crate](#ro-crate)
- [admin](#admin)
- [config, tasks, self, completions](#config-tasks-self-completions)

## Finding commands

`torc --help` prints grouped headings, but most subcommand groups are marked hidden, so the
`Commands:` block lists only a couple of entries. The grouped listing below it is the real map, and
`torc <group> --help` is the only reliable inventory for a group.

Consequences: lifecycle verbs are top level (`create`, `run`, `exec`, `submit`, `status`, `watch`,
`recover`, `cancel`, `delete`), not under `torc workflows`; and several commands appear only in the
grouped listing. `torc completions bash|zsh|fish` covers the hidden entries.

## Global options

| Option                                               | Notes                                                           |
| ---------------------------------------------------- | --------------------------------------------------------------- |
| `--url`                                              | Must include `/torc-service/v1`; env `TORC_API_URL`             |
| `-f, --format`                                       | `table` (default), `json`, or `csv`; csv is list-only           |
| `--log-level`                                        | Same argument as env `RUST_LOG`                                 |
| `-s, --standalone`                                   | Ephemeral server for this command                               |
| `--db`                                               | SQLite path for standalone mode (default `torc_output/torc.db`) |
| `--in-memory`                                        | RAM database, Unix only, snapshots to `--db` at exit            |
| `--password`, `--prompt-password`, `--cookie-header` | Authentication                                                  |
| `--tls-ca-cert`, `--tls-insecure`                    | TLS trust                                                       |
| `--skip-version-check`                               | Diagnosis aid only; fix the installation instead                |

`--in-memory` is rejected for anything but `exec` and `run`, because other commands would snapshot
an empty database over your existing data.

## Lifecycle

| Command                    | Behavior not in `--help`                                                     |
| -------------------------- | ---------------------------------------------------------------------------- |
| `torc create <spec>`       | `--dry-run` validates offline with no server and exits non-zero on failure   |
| `torc run <spec-or-id>`    | **Exits 0 even when jobs fail.** Runner logs go to stderr and to a log file  |
| `torc exec`                | Exits 1 on any failed or terminated job, unlike `run`                        |
| `torc submit <spec-or-id>` | Requires a `schedule_nodes` action; fires every pending one                  |
| `torc status <id>`         | Cheapest full picture; `-f csv` is rejected (multi-section report)           |
| `torc watch <id>`          | Blocks until complete; exits 1 on failures without `--recover`               |
| `torc recover <id>`        | Interactive wizard by default; `--no-prompts` for scripts                    |
| `torc cancel <id>`         | Cancels Slurm allocations too; state is preserved and resumable after reinit |
| `torc delete <id>`         | Permanent, cascades to jobs, files, results                                  |
| `torc ping`                | Connectivity only; says nothing about workflows                              |

`create`, `run`, and `submit` all accept `-` to read the spec from stdin, and `run`/`submit` accept
either a spec path or an existing workflow ID.

## workflows

State: `init`, `reinit`, `reset-status`, `is-complete`, `sync-status`, `reconcile`. Query: `list`,
`get`, `execution-plan`, `list-actions`, `update-action`, `delete-action`. Maintenance: `new`,
`update`, `archive`, `check-resources`, `correct-resources`, `diagnose`. Transfer: `export`,
`import`.

| Command             | Behavior not in `--help`                                                                                 |
| ------------------- | -------------------------------------------------------------------------------------------------------- |
| `init`              | Run automatically by `run` and `submit`; `--force` proceeds with missing data                            |
| `reinit`            | **Increments `run_id`**, which log filenames embed. Resets jobs whose inputs changed                     |
| `reset-status`      | Flag is `--failed-only`; without it the whole workflow resets                                            |
| `is-complete`       | Returns only `is_complete` and `is_canceled`; the cheap wait predicate                                   |
| `sync-status`       | Queries `squeue`; failed orphans get return code `-128`                                                  |
| `reconcile`         | Takes `<workflow_id> <run_id>`; `--base-dir` for journals across nodes                                   |
| `list`              | Filters to your user by default; `-a` for all users, `--include-archived`                                |
| `execution-plan`    | Accepts a spec path or an ID, so it previews before anything is created                                  |
| `list-actions`      | Shows action IDs and fire status; the way to see why an action did not fire                              |
| `update-action`     | Partial merge of only the fields you pass; `schedule_nodes` only                                         |
| `new`               | Creates an _empty_ workflow; jobs are added separately                                                   |
| `update`            | Workflow metadata only: `--name`, `--description`, `--owner-user`, `--project`, `--metadata`. Never jobs |
| `archive`           | Argument order is `archive <true\|false> <id>...`                                                        |
| `check-resources`   | **Excludes failed jobs by default**; pass `--include-failed` when diagnosing                             |
| `correct-resources` | Adjusts requirements without resetting or rerunning; downsizes unless `--no-downsize`                    |
| `diagnose`          | Runtime-versus-remaining-walltime packing check, from persisted state only                               |
| `export` / `import` | Import remaps all IDs and resets statuses to uninitialized                                               |

## jobs

`list`, `get`, `running`, `create`, `create-from-file`, `update`, `delete`, `delete-all`,
`reset-status`, `list-resource-requirements`, `list-failure-handlers`.

| Command            | Behavior not in `--help`                                                                                                                                                              |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `list`             | `-s/--status` filters server-side; `--include-relationships` costs extra queries; `-x/--exclude` drops columns from table/CSV output                                                  |
| `get`              | Includes the exact command, which is what you need to reproduce a failure                                                                                                             |
| `running`          | Adds compute node and Slurm job ID, tying a job to its allocation                                                                                                                     |
| `create-from-file` | One command per line, `#` comments skipped; names jobs `job<N>` continuing from the current count, and creates one shared resource requirement                                        |
| `reset-status`     | Three mutually exclusive selectors: IDs, `--status`, `--return-code`. Does **not** reset downstream jobs (reinit does) or bump `run_id`. Exits non-zero when a filter matches nothing |
| `delete-all`       | Destructive; `--no-prompts` to skip confirmation                                                                                                                                      |
| `update`           | Editing a job's command does not rerun it; reset and reinitialize                                                                                                                     |

## results

`list`, `get`, `delete`. One row per execution attempt.

`list` shows only the latest run unless `--all-runs`. Filters: `--failed`, `--return-code`,
`-j/--job-id`, `-r/--run-id`, `-s/--status`, `--compute-node`. `--include-logs` switches the output
to a JSON report with resolved log paths and warns on stderr for files it cannot find, which almost
always means the wrong `-o`.

Peak memory and CPU columns are populated only when resource monitoring is enabled; otherwise they
read zero.

## files and user-data

`files`: `list`, `get`, `create`, `update`, `delete`, `list-required-existing`. `user-data`: `list`,
`get`, `create`, `update`, `delete`, `delete-all`, `list-missing`.

`files list-required-existing` and `user-data list-missing` are the pre-flight checks for "why is
everything blocked": they report inputs that must exist before initialization can mark jobs ready.

Creating files and user data through the CLI is for adjusting an existing workflow; declare them in
the spec instead when authoring.

## resource-requirements and failure-handlers

`resource-requirements`: `list`, `get`, `create`, `update`, `delete`. `failure-handlers`: `list`,
`get` (read-only; declare handlers in the spec).

`resource-requirements update` is the direct lever for packing, since declared values drive
`concurrent_jobs_per_node`. Prefer `torc workflows correct-resources` when adjusting from measured
usage, and see `optimization.md`.

Every workflow gets an auto-created `default` requirement (1 CPU, `1m`, `P0DT1M`) that jobs fall
back to when they name none.

## slurm

Config: `create`, `update`, `list`, `get`, `delete`. Generation: `generate`, `regenerate`.
Execution: `schedule-nodes`. Planning: `plan-allocations`. Diagnostics: `parse-logs`, `sacct`,
`stats`, `usage`.

| Command            | Behavior not in `--help`                                                                                                                                                                                               |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `generate`         | Never edits in place: writes to stdout or `-o`. Submit the generated spec                                                                                                                                              |
| `regenerate`       | For recovery: builds schedulers for uninitialized/ready/blocked jobs, reusing existing scheduler settings as defaults                                                                                                  |
| `schedule-nodes`   | Submits allocations directly. `--suppress-actions` marks pending `schedule_nodes` actions executed first, so the new worker does not also fire them. `--job-prefix` is rejected on a `serialize_allocations` scheduler |
| `plan-allocations` | Runs `sbatch --test-only`; many-small estimate covers only the _first_ allocation                                                                                                                                      |
| `sacct`            | Calls `sacct`, so it needs a node where Slurm commands work                                                                                                                                                            |
| `stats`            | Reads the database instead, so it works anywhere                                                                                                                                                                       |
| `parse-logs`       | Takes an output directory plus `--workflow-id`                                                                                                                                                                         |

Submit from a login node only.

## hpc

`detect`, `list`, `show`, `partitions`, `match`, `generate`.

`detect` matches built-in and custom profiles first, then falls back to querying the live Slurm
cluster; it prints `No known HPC system detected.` only when both fail. `show <name>` and
`partitions [name]` take the profile as a **positional** argument, and the reserved name `slurm`
selects live discovery instead of a stored profile. The commands that build on a profile
(`slurm generate`, `slurm regenerate`, `slurm plan-allocations`) take `--profile <name>` instead.
`match --cpus --memory --walltime [name]` is the direct test of which partition a requirement will
select. `generate` derives a profile snippet from `sinfo`/`scontrol`. See `hpc-profiles.md`.

## remote

`add-workers`, `add-workers-from-file`, `list-workers`, `remove-worker`, `run`, `status`, `stop`,
`collect-logs`, `delete-logs`.

`run` starts detached workers and returns; it does not block until completion. Its `--num-cpus`,
`--memory-gb`, `--num-gpus`, and `--max-parallel-jobs` are forwarded **identically to every
worker**. `stop` is always a forced stop on Windows. See `remote-workers.md`.

## logs

`bundle`, `analyze`.

`bundle -o <output-dir>` must point at the directory used during the run; `--bundle-dir` is where
the tarball goes. `analyze` accepts a tarball or a directory, needs `--workflow-id` when a directory
holds several workflows, and is a pattern matcher: "no errors detected" means no known pattern
matched.

## events

`list`, `monitor`, `get-latest-event`, `create`, `delete`.

`monitor` streams until interrupted or for `-d <minutes>`, with `--level` filtering. `list` queries
stored history and is the better choice after the fact.

## compute-nodes and scheduled-compute-nodes

`compute-nodes`: `list`, `get` — workers that ran jobs. `scheduled-compute-nodes`: `list`, `get`,
`list-jobs` — allocations requested from a scheduler.

`scheduled-compute-nodes list-jobs <sched_id>` maps an allocation to what actually ran on it.

## job-dependencies

`job-job`, `job-file`, `job-user-data`. Read-only edge queries, useful when everything is blocked
and you need to see which edges exist rather than which you intended.

## access-groups

`create`, `delete`, `list`, `get`, `add-user`, `remove-user`, `add-workflow`, `remove-workflow`,
`list-members`, `list-user-groups`, `list-workflow-groups`.

These take **group IDs**, not names: `add-workflow <workflow_id> <group_id>`. The spec's
`access_groups` field takes names instead, so look up the ID with `list` before scripting. Use the
spec field at creation time and these commands for changes afterward.

Access control is only enforced when the server sets `enforce_access_control`.

## ro-crate

`list`, `get`, `create`, `update`, `delete`, `export`, `add-dataset`.

Only meaningful for workflows created with `enable_ro_crate: true`. `export` emits an
`ro-crate-metadata.json` document.

## admin

`reload-auth`, `tail-api`, `api-stats`, `list-audit-log`, `sql`.

`reload-auth` rereads the htpasswd file without restarting. `tail-api` streams requests over SSE and
`api-stats` summarizes rate and status mix, both useful when the server seems slow. `sql` requires
admin rights, is audited, and can be disabled server-side; treat it as a last resort.

## config, tasks, self, completions

`config show|paths|init|validate` — `show` prints the effective merged configuration and is the only
reliable answer to "what will this command use". See `settings.md`.

`tasks wait` blocks on an async task handle, such as the one from `workflows reinit --async`.

`self update` updates a binary installed by the standalone installer.

`completions <shell>` generates completions that include the hidden commands.
