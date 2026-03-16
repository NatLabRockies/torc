# How to View Job Logs

Find and read the stdout/stderr output from job execution.

## Find Log File Paths

```bash
torc reports results <workflow_id>
torc reports results <workflow_id> --job-id 15
```

Output includes:

```
{
  "job_id": 15,
  "job_name": "work_2",
  "status": "Failed",
  "run_id": 1,
  "return_code": 137,
  "completion_time": "2026-01-06T20:30:00.200Z",
  "exec_time_minutes": 0.09313333333333332,
  "compute_node_id": 47,
  "job_stdout": "output/job_stdio/job_wf43_j15_r1_a1.o",
  "job_stderr": "output/job_stdio/job_wf43_j15_r1_a1.e",
  "compute_node_type": "slurm"
},
```

## Read Logs Directly

Once you have the path, view the logs:

```bash
# View stdout
cat output/job_stdio/job_wf43_j15_r1_a1.o

# View stderr
cat output/job_stdio/job_wf43_j15_r1_a1.e

# Follow logs in real-time (for running jobs)
tail -f output/job_stdio/job_wf43_j15_r1_a1.*
```

## Default Log Location

By default, logs are stored in the output directory:

```
output/
└── job_stdio/
    ├── job_wf<id>_j<job>_r<run>_a<attempt>.o    # stdout (separate mode)
    ├── job_wf<id>_j<job>_r<run>_a<attempt>.e    # stderr (separate mode)
    ├── job_wf<id>_j<job>_r<run>_a<attempt>.log  # combined mode
```

The output directory can be configured via the run/submit CLI options.

> **Note:** The files present depend on the
> [`stdio` configuration](../reference/workflow-spec.md#stdioconfig). In `combined` mode, stdout and
> stderr are merged into a single `.log` file. Modes like `no_stdout`, `no_stderr`, or `none`
> suppress some or all files. If `delete_on_success` is enabled, files are removed after successful
> job completion.

## View Logs for Failed Jobs

Quickly find logs for failed jobs:

```bash
# Get failed job IDs
torc jobs list <workflow_id> --status failed

# Then view each job's logs
torc reports results <workflow_id> --job-id <failed_job_id>
```

## View Logs in TUI or Dashboard

You can also view job logs interactively:

- **TUI** — Run `torc tui` and select a job to view its stdout/stderr in the interface. See
  [Terminal UI](../monitoring/tui.md).
- **Dashboard** — The web dashboard displays job logs when you click on a job. See
  [Web Dashboard](../monitoring/dashboard.md).

## See Also

- [Working with Logs](../monitoring/working-with-logs.md) — Log configuration and management
- [Debug a Failed Job](./debug-failed-job.md) — Full debugging workflow
