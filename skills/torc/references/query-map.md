# Query map

Which command answers which question, and what its output actually means.

## Contents

- [Workflows](#workflows)
- [Jobs](#jobs)
- [Results](#results)
- [Structure and dependencies](#structure-and-dependencies)
- [Compute and scheduling](#compute-and-scheduling)
- [Resources](#resources)
- [Files, user data, and metadata](#files-user-data-and-metadata)
- [Export and archive](#export-and-archive)
- [Server and admin](#server-and-admin)

## Workflows

| Question                                 | Command                                                             |
| ---------------------------------------- | ------------------------------------------------------------------- |
| What workflows do I have?                | `torc workflows list`                                               |
| Including other users' or archived ones? | `torc workflows list -a` / `--include-archived` / `--archived-only` |
| Shared with a team?                      | `torc workflows list -g <access-group>`                             |
| Full record for one workflow?            | `torc workflows get <id>`                                           |
| Where does it stand overall?             | `torc status <id>`                                                  |
| Is it finished?                          | `torc workflows is-complete <id>`                                   |

`torc status` returns job counts per status, total execution time, walltime, active and pending
compute node counts, `runtime_blocked_ready_jobs`, `is_complete`, and `is_canceled`.

`torc workflows is-complete` returns just `is_complete` and `is_canceled`, which makes it the cheap
predicate for a wait loop.

## Jobs

| Question                               | Command                                          |
| -------------------------------------- | ------------------------------------------------ |
| All jobs and their current state?      | `torc jobs list <id>`                            |
| Only jobs in a status?                 | `torc jobs list <id> -s failed`                  |
| What is running right now, and where?  | `torc jobs running <id>`                         |
| Everything about one job?              | `torc jobs get <job_id>`                         |
| Which jobs depend on this one?         | `torc jobs list <id> --upstream-job-id <job_id>` |
| Jobs with their resource requirements? | `torc jobs list-resource-requirements <id>`      |
| Jobs with their failure handlers?      | `torc jobs list-failure-handlers <id>`           |

`torc jobs get` includes the exact command, which is what you need to reproduce a failure by hand.

`torc jobs running` adds compute node names and Slurm job IDs, tying a job to the allocation
executing it.

`--include-relationships` on `jobs list` adds `depends_on_job_ids` plus input/output file and
user-data IDs, at the cost of extra queries. Leave it off unless you need the edges.

Table and CSV output on `torc jobs list` supports `-x/--exclude <column>` (repeatable,
case-insensitive) to drop noisy columns such as `command`.

## Results

One result row per job execution attempt.

| Question                         | Command                                                 |
| -------------------------------- | ------------------------------------------------------- |
| Return codes for the latest run? | `torc results list <id>`                                |
| Only failures?                   | `torc results list <id> --failed`                       |
| A specific exit code?            | `torc results list <id> --return-code 137`              |
| One job's history?               | `torc results list <id> -j <job_id> --all-runs`         |
| One run?                         | `torc results list <id> -r <run_id>`                    |
| Everything a node produced?      | `torc results list <id> --compute-node <node_id>`       |
| Where are the log files?         | `torc results list <id> --include-logs -o <output-dir>` |
| One result record?               | `torc results get <result_id>`                          |

Columns include return code, execution time, peak memory, peak CPU percent, completion time, and
status. `--include-logs` switches the command to a JSON report with resolved log paths and warns on
stderr about files it cannot find (usually a wrong `-o`).

Peak memory and CPU are populated only when resource monitoring is enabled; otherwise they read
`0.0MB` / `0.0%`.

## Structure and dependencies

| Question                      | Command                                      |
| ----------------------------- | -------------------------------------------- |
| What will run, in what order? | `torc workflows execution-plan <spec-or-id>` |
| Job-to-job edges?             | `torc job-dependencies job-job <id>`         |
| Job-to-file edges?            | `torc job-dependencies job-file <id>`        |
| Job-to-user-data edges?       | `torc job-dependencies job-user-data <id>`   |

`execution-plan` accepts either a spec path or a workflow ID, so it can preview a graph before
anything is created. Use it to confirm that intended parallelism exists before submitting.

## Compute and scheduling

| Question                             | Command                                             |
| ------------------------------------ | --------------------------------------------------- |
| Which nodes worked on this workflow? | `torc compute-nodes list <id>`                      |
| One node's record?                   | `torc compute-nodes get <node_id>`                  |
| Which allocations were requested?    | `torc scheduled-compute-nodes list <id>`            |
| What ran under one allocation?       | `torc scheduled-compute-nodes list-jobs <sched_id>` |
| Slurm schedulers configured?         | `torc slurm list <id>`                              |
| Slurm accounting for the workflow?   | `torc slurm sacct <id>`                             |
| Per-job sacct stats already stored?  | `torc slurm stats <id>`                             |
| Total node and CPU time consumed?    | `torc slurm usage <id>`                             |

`torc slurm stats` reads the database and needs no cluster access. `torc slurm sacct` calls `sacct`
and must run where Slurm commands work.

## Resources

| Question                                | Command                                                |
| --------------------------------------- | ------------------------------------------------------ |
| Which jobs exceeded their requirements? | `torc workflows check-resources <id>`                  |
| Including failed and terminated jobs?   | `torc workflows check-resources <id> --include-failed` |
| All jobs, not just violations?          | `torc workflows check-resources <id> --all`            |
| Named requirement blocks?               | `torc resource-requirements list <id>`                 |
| Time-series CPU/memory plots?           | `torc plot-resources <db> -o <dir>`                    |

Time-series data lives at
`<output-dir>/resource_utilization/resource_metrics_<hostname>_<workflow_id>_<run_id>.db` and only
exists when the workflow ran with `granularity: time_series`. `plot-resources` accepts several
database paths at once, `-j <ids>` to limit jobs, and `-f json` to emit data instead of HTML.

The SQLite schema is stable enough to query directly: `job_resource_samples` (`job_id`, `timestamp`,
`cpu_percent`, `memory_bytes`, `num_processes`), `job_metadata` (`job_id`, `job_name`), and
`system_resource_samples` for compute-node samples.

## Files, user data, and metadata

| Question                       | Command                             |
| ------------------------------ | ----------------------------------- |
| Tracked files and their paths? | `torc files list <id>`              |
| Stored user data?              | `torc user-data list <id>`          |
| Workflow history as events?    | `torc events list <id>`             |
| Events of one type?            | `torc events list <id> -t <type>`   |
| Most recent event?             | `torc events get-latest-event <id>` |
| Provenance entities?           | `torc ro-crate list <id>`           |
| Provenance document?           | `torc ro-crate export <id>`         |

RO-Crate output exists only for workflows created with `enable_ro_crate: true`.

## Export and archive

```bash
torc workflows export <id> -o backup.json
torc workflows export <id> --include-results --include-events -o full.json
torc workflows import backup.json
torc workflows archive true <id>        # false to unarchive
```

Export is self-contained and portable across servers. Import remaps every entity ID to new
server-assigned IDs and resets job statuses to uninitialized for a fresh start; `--name` overrides
the name and `--skip-results` drops results present in the export. Archiving hides a workflow from
the default `workflows list` without deleting it, which is the right way to retire a workflow you
may still need.

## Server and admin

```bash
torc ping                          # connectivity
torc config show                   # effective configuration
torc admin api-stats               # recent request rate, throughput, status mix
torc admin tail-api                # live inbound requests over SSE
torc admin list-audit-log          # admin raw-SQL audit entries
torc admin sql '<statement>'       # raw SQL, admin only, audited
```

`torc admin sql` requires admin privileges, is audited, and can be disabled server-side
(`server.disable_admin_sql`, `server.disable_admin_sql_writes`). Treat it as the last resort after
the typed commands above.
