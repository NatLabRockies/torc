# Working with Logs

Torc provides tools for bundling and analyzing workflow logs. These are useful for:

- **Sharing logs** with colleagues for help debugging
- **Archiving** completed workflow logs for later reference
- **Scanning for errors** across all log files at once

## Log File Overview

Torc generates several types of log files during workflow execution:

| Log Type     | Path Pattern                                  | Contents                          |
| ------------ | --------------------------------------------- | --------------------------------- |
| Job stdout   | `output/job_stdio/job_wf<id>_j<job>_r<run>.o` | Standard output from job commands |
| Job stderr   | `output/job_stdio/job_wf<id>_j<job>_r<run>.e` | Error output, stack traces        |
| Job runner   | `output/job_runner_*.log`                     | Torc job runner internal logs     |
| Slurm stdout | `output/slurm_output_wf<id>_sl<slurm_id>.o`   | Slurm job allocation output       |
| Slurm stderr | `output/slurm_output_wf<id>_sl<slurm_id>.e`   | Slurm-specific errors             |
| Slurm env    | `output/slurm_env_*.log`                      | Slurm environment variables       |
| dmesg        | `output/dmesg_slurm_*.log`                    | Kernel messages (on failure)      |

For detailed information about log file contents, see [Debugging Workflows](debugging.md) and
[Debugging Slurm Workflows](debugging-slurm.md).

## Bundling Logs

The `torc logs bundle` command packages all logs for a workflow into a compressed tarball:

```bash
# Bundle all logs for a workflow
torc logs bundle <workflow_id>

# Specify custom output directory (where logs are located)
torc logs bundle <workflow_id> --output-dir /path/to/output

# Save bundle to a specific directory
torc logs bundle <workflow_id> --bundle-dir /path/to/bundles
```

This creates a `wf<id>.tar.gz` file containing:

- All job stdout/stderr files (`job_wf*_j*_r*.o/e`)
- Job runner logs (`job_runner_*.log`)
- Slurm output files (`slurm_output_wf*_sl*.o/e`)
- Slurm environment logs (`slurm_env_wf*_sl*.log`)
- dmesg logs (`dmesg_slurm_wf*_sl*.log`)
- Bundle metadata (workflow info, collection timestamp)

### Example: Sharing Logs

```bash
# Bundle workflow logs
torc logs bundle 123 --bundle-dir ./bundles

# Share the bundle
ls ./bundles/
# wf123.tar.gz

# Recipient can extract and analyze
tar -xzf wf123.tar.gz
torc logs analyze wf123/
```

## Analyzing Logs

The `torc logs analyze` command scans log files for known error patterns:

```bash
# Analyze a log bundle tarball
torc logs analyze wf123.tar.gz

# Analyze a log directory directly (auto-detects workflow if only one present)
torc logs analyze output/

# Analyze a directory with multiple workflows (specify which one)
torc logs analyze output/ --workflow-id 123
```

### Detected Error Patterns

The analyzer scans for common failure patterns including:

**Memory Errors:**

- Out of memory, OOM kills
- `std::bad_alloc` (C++)
- `MemoryError` (Python)

**Slurm Errors:**

- Time limit exceeded
- Node failures
- Preemption

**GPU/CUDA Errors:**

- CUDA out of memory
- GPU memory exceeded

**Crashes:**

- Segmentation faults
- Bus errors
- Signal kills

**Python Errors:**

- Tracebacks
- Import errors

**File System Errors:**

- No space left on device
- Permission denied

**Network Errors:**

- Connection refused/timed out

### Example Output

```
Log Analysis Results
====================

Analyzing: output/

Files with detected errors:

  output/job_stdio/job_wf123_j456_r1_a1.e
    Line 42: MemoryError: Unable to allocate 8.00 GiB
    Severity: critical
    Type: Python Memory Error

  output/slurm_output_wf123_sl789.e
    Line 15: slurmstepd: error: Detected 1 oom-kill event(s)
    Severity: critical
    Type: Out of Memory (OOM) Kill

Summary:
  Total files scanned: 24
  Files with errors: 2
  Error types found: MemoryError, OOM Kill
```

### Excluding Files

Environment variable files (`slurm_env_*.log`) are automatically excluded from error analysis since
they contain configuration data, not error logs.

## Workflow: Bundle and Share

A common pattern when asking for help:

```bash
# 1. Bundle the workflow logs
torc logs bundle <workflow_id>

# 2. Analyze locally first to understand the issue
torc logs analyze wf<id>.tar.gz

# 3. Share the bundle with your colleague/support
#    They can extract and analyze:
tar -xzf wf<id>.tar.gz
torc logs analyze wf<id>/
```

## Related Commands

- **`torc reports results`**: Generate JSON report with all log file paths
- **`torc results list`**: View summary table of job return codes
- **`torc slurm parse-logs`**: Parse Slurm logs for error patterns (Slurm-specific)
- **`torc slurm sacct`**: Collect Slurm accounting data

## See Also

- [Debugging Workflows](debugging.md) — General debugging workflow and log file details
- [Debugging Slurm Workflows](debugging-slurm.md) — Slurm-specific debugging tools
