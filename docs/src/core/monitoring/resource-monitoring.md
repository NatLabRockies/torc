# How to Monitor Resource Usage

This guide shows how to track CPU and memory usage of your workflow jobs and identify resource
requirement mismatches.

## Enable Resource Monitoring

Resource monitoring is **enabled by default** for all workflows. To explicitly configure it, add a
`resource_monitor` section to your workflow specification:

```yaml
name: "My Workflow"

resource_monitor:
  enabled: true
  granularity: "summary"       # or "time_series"
  sample_interval_seconds: 10

jobs:
  # ... your jobs
```

To disable monitoring when creating a workflow:

```bash
torc create my_workflow.yaml --no-resource-monitoring
```

## View Summary Metrics

For workflows using summary mode (default), view resource metrics with:

```bash
torc results list <workflow_id>
```

The output includes columns for peak and average CPU and memory usage.

## Check for Resource Violations

Use `check-resource-utilization` to identify jobs that exceeded their specified requirements:

```bash
# Check latest run
torc workflows check-resources <workflow_id>

# Check a specific run
torc workflows check-resources <workflow_id> --run-id <run_id>

# Show all jobs, not just violations
torc workflows check-resources <workflow_id> --all
```

Example output:

```
⚠ Found 3 resource over-utilization violations:

Job ID | Job Name         | Resource | Specified | Peak Used | Over-Utilization
-------|------------------|----------|-----------|-----------|------------------
15     | train_model      | Memory   | 8.00 GB   | 10.50 GB  | +31.3%
15     | train_model      | Runtime  | 2h 0m 0s  | 2h 45m 0s | +37.5%
16     | large_preprocess | CPU      | 800%      | 950.5%    | +18.8%
```

## Adjust Resource Requirements

### Automatic Correction

Use `correct-resources` to automatically fix both over-utilized and under-utilized resources:

```bash
# Preview what would change
torc workflows correct-resources <workflow_id> --dry-run

# Apply corrections (upscale violations + downsize over-allocations)
torc workflows correct-resources <workflow_id>

# Only upscale, don't reduce over-allocated resources
torc workflows correct-resources <workflow_id> --no-downsize
```

The command upscales resources that exceeded their limits and downsizes resources that are
significantly over-allocated. Downsizing only uses successfully completed jobs and requires all jobs
sharing a resource requirement to have peak usage data. See the
[how-to guide](../how-to/check-resource-utilization.md#automatically-correct-requirements) for
details.

### Manual Adjustment

For more control, update your workflow specification directly:

```yaml
# Before: job used 10.5 GB but was allocated 8 GB
resource_requirements:
  - name: training
    memory: 8g
    runtime: PT2H

# After: increased with buffer
resource_requirements:
  - name: training
    memory: 12g       # 10.5 GB peak + 15% buffer
    runtime: PT3H     # 2h 45m actual + buffer
```

**Guidelines for buffers:**

- Memory: Add 10-20% above peak usage
- Runtime: Add 15-30% above actual duration
- CPU: Round up to next core count

## Enable Time Series Monitoring

For detailed resource analysis over time, switch to time series mode:

```yaml
resource_monitor:
  granularity: "time_series"
  sample_interval_seconds: 2
```

This creates a SQLite database with samples at regular intervals.

## Generate Resource Plots

Create interactive visualizations from time series data:

```bash
# Generate all plots
torc plot-resources output/resource_utilization/resource_metrics_*.db \
  -o plots/

# Generate plots for specific jobs
torc plot-resources output/resource_utilization/resource_metrics_*.db \
  -o plots/ \
  --job-ids 15,16
```

The tool generates:

- Individual job plots showing CPU, memory, and process count over time
- Overview plots comparing all jobs
- Summary dashboard with bar charts

## Query Time Series Data Directly

Access the SQLite database for custom analysis:

```bash
sqlite3 -table output/resource_utilization/resource_metrics_1_1.db
```

```sql
-- View samples for a specific job
SELECT job_id, timestamp, cpu_percent, memory_bytes, num_processes
FROM job_resource_samples
WHERE job_id = 1
ORDER BY timestamp;

-- View job metadata
SELECT * FROM job_metadata;
```

## Troubleshooting

### No metrics recorded

- Check that monitoring wasn't disabled with `--no-resource-monitoring`
- Ensure jobs run long enough for at least one sample (default: 5 seconds)

### Time series database not created

- Verify the output directory is writable
- Confirm `granularity: "time_series"` is set in the workflow spec

### Missing child process metrics

- Decrease `sample_interval_seconds` to catch short-lived processes

## Next Steps

- [Resource Monitoring Reference](../reference/resource-monitoring.md) - Configuration options and
  database schema
- [Managing Resources](../reference/resources.md) - Define job resource requirements
