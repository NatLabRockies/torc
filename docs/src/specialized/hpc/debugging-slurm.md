# Debugging Slurm Workflows

When running workflows on Slurm clusters, Torc provides additional debugging tools specifically
designed for Slurm environments. This guide covers Slurm-specific debugging techniques and tools.

For general debugging concepts and tools that apply to all workflows, see
[Debugging Workflows](debugging.md).

## Overview

Slurm workflows generate additional log files beyond the standard job logs:

- **Slurm stdout/stderr**: Output from Slurm's perspective (job allocation, environment setup)
- **Slurm environment logs**: All SLURM environment variables captured at job runner startup
- **dmesg logs**: Kernel message buffer captured when the Slurm job runner exits

These logs help diagnose issues specific to the cluster environment, such as resource allocation
failures, node problems, and system-level errors.

## Slurm Log File Structure

For jobs executed via Slurm scheduler (`compute_node_type: "slurm"`), the debug report includes
these additional log paths:

```json
{
  "job_stdout": "torc_output/job_stdio/job_wf1_j456_r1.o",
  "job_stderr": "torc_output/job_stdio/job_wf1_j456_r1.e",
  "job_runner_log": "torc_output/job_runner_slurm_wf1_sl12345_n0_pid67890.log",
  "slurm_stdout": "torc_output/slurm_output_wf1_sl12345.o",
  "slurm_stderr": "torc_output/slurm_output_wf1_sl12345.e",
  "slurm_env_log": "torc_output/slurm_env_wf1_sl12345_n0_pid67890.log",
  "dmesg_log": "torc_output/dmesg_slurm_wf1_sl12345_n0_pid67890.log"
}
```

All Slurm log files include the workflow ID (`wf<id>`) prefix, making it easy to identify and
collect logs for a specific workflow.

### Log File Descriptions

1. **slurm_stdout** (`torc_output/slurm_output_wf<workflow_id>_sl<slurm_job_id>.o`):
   - Standard output from Slurm's perspective
   - Includes Slurm environment setup, job allocation info
   - **Use for**: Debugging Slurm job submission issues

2. **slurm_stderr** (`torc_output/slurm_output_wf<workflow_id>_sl<slurm_job_id>.e`):
   - Standard error from Slurm's perspective
   - Contains Slurm-specific errors (allocation failures, node issues)
   - **Use for**: Investigating Slurm scheduler problems

3. **job_runner_log** (`torc_output/job_runner_slurm_wf<id>_sl<slurm_job_id>_n<node>_pid<pid>.log`):
   - Log output from the Torc Slurm job runner process
   - Contains job execution details, status updates, and runner-level errors
   - **Use for**: Debugging job runner issues, understanding job execution flow

4. **slurm_env_log** (`torc_output/slurm_env_wf<id>_sl<slurm_job_id>_n<node_id>_pid<task_pid>.log`):
   - All SLURM environment variables captured at job runner startup
   - Contains job allocation details, resource limits, node assignments
   - **Use for**: Verifying Slurm job configuration, debugging resource allocation issues

5. **dmesg_log** (`torc_output/dmesg_slurm_wf<id>_sl<slurm_job_id>_n<node_id>_pid<task_pid>.log`):
   - Kernel message buffer captured when the Slurm job runner exits (only on failure)
   - Contains system-level events: OOM killer activity, hardware errors, kernel panics
   - **Use for**: Investigating job failures caused by system-level issues (e.g., out-of-memory
     kills, hardware failures)

**Note**: All Slurm log files include the workflow ID, Slurm job ID, node ID, and task PID in the
filename for easy filtering and correlation with Slurm's own logs.

## Parsing Slurm Log Files for Errors

The `torc slurm parse-logs` command scans Slurm stdout/stderr log files for known error patterns and
correlates them with affected Torc jobs:

```bash
# Parse logs for a specific workflow
torc slurm parse-logs <workflow_id>

# Specify custom output directory
torc slurm parse-logs <workflow_id> --output-dir /path/to/torc_output

# Output as JSON for programmatic processing
torc slurm parse-logs <workflow_id> --format json
```

### Detected Error Patterns

The command detects common Slurm failure patterns including:

**Memory Errors:**

- `out of memory`, `oom-kill`, `cannot allocate memory`
- `memory cgroup out of memory`, `Exceeded job memory limit`
- `task/cgroup: .*: Killed`
- `std::bad_alloc` (C++), `MemoryError` (Python)

**Slurm-Specific Errors:**

- `slurmstepd: error:`, `srun: error:`
- `DUE TO TIME LIMIT`, `DUE TO PREEMPTION`
- `NODE_FAIL`, `FAILED`, `CANCELLED`
- `Exceeded.*step.*limit`

**GPU/CUDA Errors:**

- `CUDA out of memory`, `CUDA error`, `GPU memory.*exceeded`

**Signal/Crash Errors:**

- `Segmentation fault`, `SIGSEGV`
- `Bus error`, `SIGBUS`
- `killed by signal`, `core dumped`

**Python Errors:**

- `Traceback (most recent call last)`
- `ModuleNotFoundError`, `ImportError`

**File System Errors:**

- `No space left on device`, `Disk quota exceeded`
- `Read-only file system`, `Permission denied`

**Network Errors:**

- `Connection refused`, `Connection timed out`, `Network is unreachable`

### Example Output

**Table format:**

```
Slurm Log Analysis Results
==========================

Found 2 error(s) in log files:

╭─────────────────────────────┬──────────────┬──────┬─────────────────────────────┬──────────┬──────────────────────────────╮
│ File                        │ Slurm Job ID │ Line │ Pattern                     │ Severity │ Affected Torc Jobs           │
├─────────────────────────────┼──────────────┼──────┼─────────────────────────────┼──────────┼──────────────────────────────┤
│ slurm_output_sl12345.e      │ 12345        │ 42   │ Out of Memory (OOM) Kill    │ critical │ process_data (ID: 456)       │
│ slurm_output_sl12346.e      │ 12346        │ 15   │ CUDA out of memory          │ error    │ train_model (ID: 789)        │
╰─────────────────────────────┴──────────────┴──────┴─────────────────────────────┴──────────┴──────────────────────────────╯
```

## Viewing Slurm Accounting Data

The `torc slurm sacct` command displays a summary of Slurm job accounting data for all scheduled
compute nodes in a workflow:

```bash
# Display sacct summary table for a workflow
torc slurm sacct <workflow_id>

# Also save full JSON files for detailed analysis
torc slurm sacct <workflow_id> --save-json --output-dir /path/to/torc_output

# Output as JSON for programmatic processing
torc slurm sacct <workflow_id> --format json
```

### Summary Table Fields

The command displays a summary table with key metrics:

- **Slurm Job**: The Slurm job ID
- **Job Step**: Name of the job step (e.g., "worker_1", "batch")
- **State**: Job state (COMPLETED, FAILED, TIMEOUT, OUT_OF_MEMORY, etc.)
- **Exit Code**: Exit code of the job step
- **Elapsed**: Wall clock time for the job step
- **Max RSS**: Maximum resident set size (memory usage)
- **CPU Time**: Total CPU time consumed
- **Nodes**: Compute nodes used

### Example Output

```
Slurm Accounting Summary for Workflow 123

╭────────────┬───────────┬───────────┬───────────┬─────────┬─────────┬──────────┬─────────╮
│ Slurm Job  │ Job Step  │ State     │ Exit Code │ Elapsed │ Max RSS │ CPU Time │ Nodes   │
├────────────┼───────────┼───────────┼───────────┼─────────┼─────────┼──────────┼─────────┤
│ 12345      │ worker_1  │ COMPLETED │ 0         │ 2h 15m  │ 4.5GB   │ 4h 30m   │ node01  │
│ 12345      │ batch     │ COMPLETED │ 0         │ 2h 16m  │ 128.0MB │ 1m 30s   │ node01  │
│ 12346      │ worker_1  │ FAILED    │ 1         │ 45m 30s │ 8.2GB   │ 1h 30m   │ node02  │
╰────────────┴───────────┴───────────┴───────────┴─────────┴─────────┴──────────┴─────────╯

Total: 3 job steps
```

### Saving Full JSON Output

Use `--save-json` to save full sacct JSON output to files for detailed analysis:

```bash
torc slurm sacct 123 --save-json --output-dir torc_output
# Creates: torc_output/sacct_12345.json, torc_output/sacct_12346.json, etc.
```

## Viewing Slurm Logs in torc-dash

The torc-dash web interface provides two ways to view Slurm logs:

### Debugging Tab - Slurm Log Analysis

The Debugging tab includes a "Slurm Log Analysis" section:

1. Navigate to the **Debugging** tab
2. Find the **Slurm Log Analysis** section
3. Enter the output directory path (default: `torc_output`)
4. Click **Analyze Slurm Logs**

The results show all detected errors with their Slurm job IDs, line numbers, error patterns,
severity levels, and affected Torc jobs.

### Debugging Tab - Slurm Accounting Data

The Debugging tab also includes a "Slurm Accounting Data" section:

1. Navigate to the **Debugging** tab
2. Find the **Slurm Accounting Data** section
3. Click **Collect sacct Data**

This displays a summary table showing job state, exit codes, elapsed time, memory usage (Max RSS),
CPU time, and nodes for all Slurm job steps. The table helps quickly identify failed jobs and
resource usage patterns.

### Scheduled Nodes Tab - View Slurm Logs

You can view individual Slurm job logs directly from the Details view:

1. Select a workflow
2. Go to the **Details** tab
3. Switch to the **Scheduled Nodes** sub-tab
4. Find a Slurm scheduled node in the table
5. Click the **View Logs** button in the Logs column

This opens a modal with tabs for viewing the Slurm job's stdout and stderr files.

## Viewing Slurm Logs in the TUI

The `torc tui` terminal interface also supports Slurm log viewing:

1. Launch the TUI: `torc tui`
2. Select a workflow and press Enter to load details
3. Press Tab to switch to the **Scheduled Nodes** tab
4. Navigate to a Slurm scheduled node using arrow keys
5. Press `l` to view the Slurm job's logs

The log viewer shows:

- **stdout tab**: Slurm job standard output (`slurm_output_wf<id>_sl<slurm_job_id>.o`)
- **stderr tab**: Slurm job standard error (`slurm_output_wf<id>_sl<slurm_job_id>.e`)

Use Tab to switch between stdout/stderr, arrow keys to scroll, `/` to search, and `q` to close.

## Debugging Slurm Job Failures

When a Slurm job fails, follow this debugging workflow:

1. **Parse logs for known errors:**
   ```bash
   torc slurm parse-logs <workflow_id>
   ```

2. **If OOM or resource issues are detected, collect sacct data:**
   ```bash
   torc slurm sacct <workflow_id>
   cat torc_output/sacct_<slurm_job_id>.json | jq '.jobs[].steps[].tres.requested'
   ```

3. **View the specific Slurm log files:**
   - Use torc-dash: Details → Scheduled Nodes → View Logs
   - Or use TUI: Scheduled Nodes tab → press `l`
   - Or directly: `cat torc_output/slurm_output_wf<workflow_id>_sl<slurm_job_id>.e`

4. **Check the job's own stderr for application errors:**
   ```bash
   torc reports results <workflow_id> > report.json
   jq -r '.results[] | select(.return_code != 0) | .job_stderr' report.json | xargs cat
   ```

5. **Review dmesg logs for system-level issues:**
   ```bash
   cat torc_output/dmesg_slurm_wf<workflow_id>_sl<slurm_job_id>_*.log
   ```

## Orphaned Jobs and Status Synchronization

When a Slurm allocation terminates unexpectedly (e.g., due to timeout, node failure, or admin
intervention), jobs may become "orphaned" - stuck in "running" status in Torc's database even though
no process is actually executing them.

### Detecting Orphaned Jobs

Common signs of orphaned jobs:

- Jobs remain in "running" status long after the Slurm allocation ended
- `torc recover` reports "there are active Slurm allocations" but `squeue` shows none
- Workflow appears stuck but no Slurm jobs are actually running

### Synchronizing Status with Slurm

The `torc workflows sync-status` command detects and fixes orphaned jobs by checking the actual
Slurm state:

```bash
# Preview what would be cleaned up (recommended first)
torc workflows sync-status <workflow_id> --dry-run

# Clean up orphaned jobs
torc workflows sync-status <workflow_id>

# Get JSON output for scripting
torc -f json workflows sync-status <workflow_id>
```

This command:

1. Checks each "active" scheduled compute node against `squeue`
2. If Slurm reports the job is no longer running, marks associated Torc jobs as failed
3. Updates scheduled compute node status to "complete"
4. Also handles "pending" allocations that were cancelled before starting

### Example Output

```
Synchronizing job statuses for workflow 42...

Cleaned up orphaned jobs:
  - 3 job(s) from terminated Slurm allocations
  - 1 pending allocation(s) that no longer exist in Slurm

Affected jobs:
  - Job 107 (train_model_7): Allocation terminated (Slurm job 12345)
  - Job 112 (train_model_12): Allocation terminated (Slurm job 12345)
  - Job 123 (train_model_23): Allocation terminated (Slurm job 12345)

Total: 3 job(s) marked as failed

You can now run `torc recover 42` to retry failed jobs.
```

### Automatic Cleanup in Recovery

The `torc recover` command automatically performs orphan detection as its first step, so you
typically don't need to run `sync-status` manually before recovery. However, `sync-status` is useful
when:

- You want to clean up orphaned jobs without triggering a full recovery
- You want to preview what `recover` would clean up (using `--dry-run`)
- You're debugging why `recover` reports active allocations

## Common Slurm Issues and Solutions

### Out of Memory (OOM) Kills

**Symptoms:**

- `torc slurm parse-logs` shows "Out of Memory (OOM) Kill"
- Job exits with signal 9 (SIGKILL)
- dmesg log shows "oom-kill" entries

**Solutions:**

- Increase memory request in job specification
- Check `torc slurm sacct` output for actual memory usage (Max RSS)
- Consider splitting job into smaller chunks

### Time Limit Exceeded

**Symptoms:**

- `torc slurm parse-logs` shows "DUE TO TIME LIMIT"
- Job state in sacct shows "TIMEOUT"

**Solutions:**

- Increase runtime in job specification
- Check if job is stuck (review stdout for progress)
- Consider optimizing the job or splitting into phases

### Node Failures

**Symptoms:**

- `torc slurm parse-logs` shows "NODE_FAIL"
- Job may have completed partially

**Solutions:**

- Reinitialize workflow to retry failed jobs
- Check cluster status with `sinfo`
- Review dmesg logs for hardware issues

### GPU/CUDA Errors

**Symptoms:**

- `torc slurm parse-logs` shows "CUDA out of memory" or "CUDA error"

**Solutions:**

- Reduce batch size or model size
- Check GPU memory with `nvidia-smi` in job script
- Ensure correct CUDA version is loaded

## Related Commands

- **`torc slurm parse-logs`**: Parse Slurm logs for known error patterns
- **`torc slurm sacct`**: Collect Slurm accounting data for workflow jobs
- **`torc workflows sync-status`**: Detect and fix orphaned jobs from terminated Slurm allocations
- **`torc reports results`**: Generate debug report with all log file paths
- **`torc results list`**: View summary of job results in table format
- **`torc-dash`**: Launch web interface with Slurm log viewing
- **`torc tui`**: Launch terminal UI with Slurm log viewing

## See Also

- [Debugging Workflows](debugging.md) — General debugging tools and workflows
- [Working with Logs](working-with-logs.md) — Bundling and analyzing logs
