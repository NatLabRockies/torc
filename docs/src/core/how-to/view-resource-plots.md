# How to View Resource Utilization Plots

Generate interactive visualizations of CPU and memory usage over time.

## Prerequisites

Enable time series monitoring in your workflow specification:

```yaml
resource_monitor:
  sample_interval_seconds: 2
  jobs:
    enabled: true
    granularity: "time_series"
```

This creates a SQLite database with resource samples at regular intervals.

## Generate Plots

After your workflow completes, generate plots from the collected data:

```bash
torc plot-resources output/resource_utilization/resource_metrics_*.db -o plots/
```

This creates:

- **Individual job plots** — CPU, memory, and process count over time for each job
- **Overview plots** — Comparison across all jobs
- **Summary dashboard** — Bar charts of peak and average usage

## Plot Specific Jobs

Generate plots for only certain jobs:

```bash
torc plot-resources output/resource_utilization/resource_metrics_*.db \
  -o plots/ \
  --job-ids 15,16
```

## View the Plots

Open the generated HTML files in your browser:

```bash
open plots/job_15_resources.html
```

## Query Data Directly

For custom analysis, query the SQLite database:

```bash
sqlite3 -table output/resource_utilization/resource_metrics_1_1.db
```

```sql
-- View samples for a specific job
SELECT timestamp, cpu_percent, memory_bytes
FROM job_resource_samples
WHERE job_id = 1
ORDER BY timestamp;
```

## See Also

- [Resource Monitoring](../monitoring/resource-monitoring.md) — Configuration options
- [Resource Monitoring Database](../reference/resource-monitoring.md) — Database schema reference
