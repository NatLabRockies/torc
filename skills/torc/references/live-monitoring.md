# Live monitoring

Use `torc watch` for blocking monitoring with a meaningful exit status. Use `torc events monitor`
for a live event stream.

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
