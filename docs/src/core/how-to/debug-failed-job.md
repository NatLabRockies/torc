# How to Debug a Failed Job

Systematically diagnose why a job failed.

## Step 1: Identify the Failed Job

```bash
torc jobs list <workflow_id> --status failed
```

Note the job ID and name.

## Step 2: Check the Exit Code

```bash
torc results get <workflow_id> --job-id <job_id>
```

Common exit codes:

| Code | Meaning                      |
| ---- | ---------------------------- |
| 1    | General error                |
| 2    | Misuse of shell command      |
| 126  | Permission denied            |
| 127  | Command not found            |
| 137  | Killed (SIGKILL) — often OOM |
| 139  | Segmentation fault           |
| 143  | Terminated (SIGTERM)         |

## Step 3: Read the Logs

```bash
# Get log paths
torc results list --include-logs <workflow_id> --job-id <job_id>

# View stderr (usually contains error messages)
cat output/job_stdio/job_wf43_j15_r1_a1.e

# View stdout
cat output/job_stdio/job_wf43_j15_r1_a1.o

# In combined stdio mode, both streams are in a single .log file
cat output/job_stdio/job_wf43_j15_r1_a1.log
```

> **Note:** If `stdio` is configured with `mode: none` or `mode: no_stderr`, log files may not
> exist. See [`StdioConfig`](../reference/workflow-spec.md#stdioconfig) for details.

## Step 4: Check Resource Usage

Did the job exceed its resource limits?

```bash
torc workflows check-resources <workflow_id>
```

Look for:

- **Memory exceeded** — Job was likely OOM-killed (exit code 137)
- **Runtime exceeded** — Job was terminated for running too long

## Step 5: Reproduce Locally

Get the exact command that was run:

```bash
torc jobs get <job_id>
```

Try running it manually to see the error:

```bash
# Copy the command from the output and run it
python process.py --input data.csv
```

## Common Fixes

| Problem           | Solution                                     |
| ----------------- | -------------------------------------------- |
| OOM killed        | Increase `memory` in resource requirements   |
| File not found    | Verify input files exist, check dependencies |
| Permission denied | Check file permissions, execution bits       |
| Timeout           | Increase `runtime` in resource requirements  |

## Step 6: Fix and Retry

After fixing the issue:

```bash
# Reinitialize to reset failed jobs
torc workflows reset-status --failed --reinitialize <workflow_id>

# Run again locally
torc run <workflow_id>
# Or re-submit to Slurm
torc submit <workflow_id>
```

## See Also

- [View Job Logs](./view-job-logs.md) — Finding log files
- [Check Resource Utilization](./check-resource-utilization.md) — Resource analysis
- [Debugging Workflows](../monitoring/debugging.md) — Comprehensive debugging guide
