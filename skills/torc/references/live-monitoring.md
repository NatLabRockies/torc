# Live monitoring

Four ways to watch a running workflow. Pick by whether a human is present and whether the
environment is a terminal.

| Interface             | Use when                                           | Interactive |
| --------------------- | -------------------------------------------------- | ----------- |
| `torc watch`          | Unattended monitoring, CI, meaningful exit status  | No          |
| `torc tui`            | A person at a terminal, including over SSH         | Yes         |
| `torc-dash`           | A person with a browser, visual DAG and log viewer | Yes         |
| `torc events monitor` | Streaming a machine-readable event feed            | No          |

## torc watch

```bash
torc watch <id>                     # poll until complete, then report
torc watch <id> -p 30               # poll interval in seconds (default 60)
torc watch <id> --recover           # recover from failures each round
torc watch <id> --recover --auto-schedule
```

This is the right choice for automation: it blocks until the workflow finishes and exits 1 when the
workflow ended with failures and `--recover` is off, when max retries are exceeded, when recovery
cannot progress, or when only unclassified `pending_failed` jobs remain.

`--auto-schedule` regenerates and submits Slurm schedulers when no active or pending scheduler
exists but ready jobs do, and when accumulated retry jobs exceed `--auto-schedule-threshold`
(default 5). It warns when run from a directory other than the recorded submission directory.

Recovery behavior is covered in `rerun-and-recovery.md`.

## torc tui

```bash
torc tui                            # connect to the configured server
torc tui --standalone               # start a torc-server automatically
torc tui --standalone --port 8090 --database /path/to/workflows.db
```

The TUI is built for terminal-over-SSH work on HPC. It streams live job and compute-node events over
SSE and can drive the full lifecycle.

Navigation: arrows move within a table, `←`/`→` switch between the Workflows and Details panes,
`Tab` cycles detail tabs (Summary, Jobs, Running, Results, Files, User Data, Events, Compute Nodes,
Scheduled Nodes, Slurm Stats, DAG), `Enter` loads details, `e` jumps to Events, `?` shows
context-aware help, `r` refreshes, `A` toggles auto-refresh, `q` closes a popup or quits.

Workflow actions on the selected row: `n` new, `i` init, `I` reinit, `R` reset, `x` run, `s` submit,
`W` watch, `V` recover, `v` recover dry-run, `C` cancel, `d` delete. Destructive actions confirm
first; `V`/`v` open a multiplier modal (pre-filled 1.5 memory, 1.5 runtime) where Enter both applies
and confirms.

Job actions in the Jobs tab: `Enter` details, `l` logs with stdout/stderr tabs and `/` search, `C`
cancel (on this tab only; elsewhere `C` cancels the workflow), `t` terminate, `y` retry, `U` reset
to uninitialized, Space toggles selection for a multi-job reset.

Filtering and sorting: `f` opens a filter for the focused pane, `=` filters to the selected row's
value on that pane's primary column, `c` clears it. Active filters appear in the table title. Number
keys sort the focused table by column, left to right, each press cycling none → descending →
ascending: Workflows `1`/`2`/`3` (ID/Name/User), Jobs `1`/`2`/`3` (ID/Name/Status), Results `1`-`8`
(ID, Job ID, Name, Return, Runtime, Completion, Peak Memory, Peak CPU), Compute Nodes `1`-`4`.

An agent driving Torc should not use the TUI. It is a full-screen application with no
non-interactive mode; use the CLI commands instead.

## torc-dash

```bash
torc-dash --standalone                       # starts torc-server and the dashboard
torc-dash                                     # connect to the default API URL
torc-dash --api-url http://myserver:9000/torc-service/v1
```

Default bind is `127.0.0.1:8090`. It offers workflow and job monitoring with SSE updates, spec
upload and run, an interactive DAG, a Debugging tab with a log viewer, and resource plots.

The Debugging tab generates a job-results report with options for the output directory, all runs,
and failed-only, then shows stdout and stderr for the selected job. The output directory must match
the one used during execution.

`torc-dash` ships as a feature-gated binary. Build it with the `dash` feature
(`cargo build --release --features dash`) or install with
`cargo install torc --features "server-bin,mcp-server,dash,slurm-runner"`. Dashboard settings live
in `[dash]` in a config file; see `settings.md`.

## torc events monitor

```bash
torc events monitor <id>
torc events monitor <id> --level warning
torc events monitor <id> -d 30 --filename events.log
torc events list <id> -t job_started
```

`monitor` streams events until interrupted, or for `-d <minutes>`. `--level` filters at `debug`,
`info` (default), `warning`, or `error`, and `--filename` also writes them to a file. `events list`
queries the stored history instead, which is the better choice for post-hoc analysis.

## Resource plots

Time-series monitoring must be enabled in the spec
(`resource_monitor.jobs.granularity: time_series`, or the `compute_node` scope for node-level data).
The database lands under `<output-dir>/resource_utilization/`, one per runner:
`resource_metrics_wf<wf>_h<hostname>_r<run>.db` locally,
`resource_metrics_wf<wf>_sl<slurm>_n<node>_p<pid>.db` on Slurm.

```bash
torc plot-resources <db> -o ./reports
torc plot-resources <db> -j 12,13,14
torc plot-resources <db1> <db2>            # several runners' databases at once
torc plot-resources <db> -f json           # data instead of HTML
```

Output filenames are `job_<id>.html`, `summary.html`, and `system_timeline.html`, optionally
prefixed with `-p`. `resource_monitor.generate_plots: true` produces them automatically when the
runner exits, writing them beside the database in `resource_utilization/` and prefixing each with
the database's own label (`wf<wf>_h<host>_r<run>_job_4.html`) so runs sharing an output directory do
not overwrite each other.

For summary-only workflows there is no time-series database; use `torc workflows check-resources`
and the peak columns in `torc results list` instead.
