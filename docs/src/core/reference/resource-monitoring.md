# Resource Monitoring Reference

Technical reference for Torc's resource monitoring system.

## Configuration Options

The `resource_monitor` section in workflow specifications accepts the following fields:

| Field                     | Type    | Default     | Description                      |
| ------------------------- | ------- | ----------- | -------------------------------- |
| `enabled`                 | boolean | `true`      | Enable or disable monitoring     |
| `granularity`             | string  | `"summary"` | `"summary"` or `"time_series"`   |
| `sample_interval_seconds` | integer | `5`         | Seconds between resource samples |
| `generate_plots`          | boolean | `false`     | Reserved for future use          |

### Granularity Modes

**Summary mode** (`"summary"`):

- Stores only peak and average values per job
- Metrics stored in the main database results table
- Minimal storage overhead

**Time series mode** (`"time_series"`):

- Stores samples at regular intervals
- Creates separate SQLite database per workflow run
- Database location:
  `<output_dir>/resource_utilization/resource_metrics_<hostname>_<workflow_id>_<run_id>.db`

### Sample Interval Guidelines

| Job Duration | Recommended Interval |
| ------------ | -------------------- |
| < 1 hour     | 1-2 seconds          |
| 1-4 hours    | 5 seconds (default)  |
| > 4 hours    | 10-30 seconds        |

## Time Series Database Schema

### `job_resource_samples` Table

| Column          | Type    | Description                      |
| --------------- | ------- | -------------------------------- |
| `id`            | INTEGER | Primary key                      |
| `job_id`        | INTEGER | Torc job ID                      |
| `timestamp`     | REAL    | Unix timestamp                   |
| `cpu_percent`   | REAL    | CPU utilization percentage       |
| `memory_bytes`  | INTEGER | Memory usage in bytes            |
| `num_processes` | INTEGER | Process count including children |

### `job_metadata` Table

| Column     | Type    | Description              |
| ---------- | ------- | ------------------------ |
| `job_id`   | INTEGER | Primary key, Torc job ID |
| `job_name` | TEXT    | Human-readable job name  |

## Summary Metrics in Results

When using summary mode, the following fields are added to job results:

| Field              | Type  | Description                     |
| ------------------ | ----- | ------------------------------- |
| `peak_cpu_percent` | float | Maximum CPU percentage observed |
| `avg_cpu_percent`  | float | Average CPU percentage          |
| `peak_memory_gb`   | float | Maximum memory in GB            |
| `avg_memory_gb`    | float | Average memory in GB            |

## check-resource-utilization JSON Output

When using `--format json`:

```json
{
  "workflow_id": 123,
  "run_id": null,
  "total_results": 10,
  "over_utilization_count": 3,
  "violations": [
    {
      "job_id": 15,
      "job_name": "train_model",
      "resource_type": "Memory",
      "specified": "8.00 GB",
      "peak_used": "10.50 GB",
      "over_utilization": "+31.3%"
    }
  ]
}
```

| Field                    | Description                                              |
| ------------------------ | -------------------------------------------------------- |
| `workflow_id`            | Workflow being analyzed                                  |
| `run_id`                 | Specific run ID if provided, otherwise `null` for latest |
| `total_results`          | Total number of completed jobs analyzed                  |
| `over_utilization_count` | Number of violations found                               |
| `violations`             | Array of violation details                               |

### Violation Object

| Field              | Description                             |
| ------------------ | --------------------------------------- |
| `job_id`           | Job ID with violation                   |
| `job_name`         | Human-readable job name                 |
| `resource_type`    | `"Memory"`, `"CPU"`, or `"Runtime"`     |
| `specified`        | Resource requirement from workflow spec |
| `peak_used`        | Actual peak usage observed              |
| `over_utilization` | Percentage over/under specification     |

## correct-resources JSON Output

When using `torc -f json workflows correct-resources`:

```json
{
  "status": "success",
  "workflow_id": 123,
  "dry_run": false,
  "no_downsize": false,
  "memory_multiplier": 1.2,
  "cpu_multiplier": 1.2,
  "runtime_multiplier": 1.2,
  "resource_requirements_updated": 2,
  "jobs_analyzed": 5,
  "memory_corrections": 1,
  "runtime_corrections": 1,
  "cpu_corrections": 1,
  "downsize_memory_corrections": 2,
  "downsize_runtime_corrections": 2,
  "downsize_cpu_corrections": 0,
  "adjustments": [
    {
      "resource_requirements_id": 10,
      "direction": "upscale",
      "job_ids": [15],
      "job_names": ["train_model"],
      "memory_adjusted": true,
      "original_memory": "8g",
      "new_memory": "13g",
      "max_peak_memory_bytes": 10500000000
    },
    {
      "resource_requirements_id": 11,
      "direction": "downscale",
      "job_ids": [20, 21],
      "job_names": ["preprocess_a", "preprocess_b"],
      "memory_adjusted": true,
      "original_memory": "32g",
      "new_memory": "3g",
      "max_peak_memory_bytes": 2147483648,
      "runtime_adjusted": true,
      "original_runtime": "PT4H",
      "new_runtime": "PT12M"
    }
  ]
}
```

### Top-Level Fields

| Field                           | Description                                          |
| ------------------------------- | ---------------------------------------------------- |
| `memory_multiplier`             | Memory safety multiplier used                        |
| `cpu_multiplier`                | CPU safety multiplier used                           |
| `runtime_multiplier`            | Runtime safety multiplier used                       |
| `resource_requirements_updated` | Number of resource requirements changed              |
| `jobs_analyzed`                 | Number of jobs with violations analyzed              |
| `memory_corrections`            | Jobs affected by memory upscaling                    |
| `runtime_corrections`           | Jobs affected by runtime upscaling                   |
| `cpu_corrections`               | Jobs affected by CPU upscaling                       |
| `downsize_memory_corrections`   | Jobs affected by memory downsizing                   |
| `downsize_runtime_corrections`  | Jobs affected by runtime downsizing                  |
| `downsize_cpu_corrections`      | Jobs affected by CPU downsizing                      |
| `adjustments`                   | Array of per-resource-requirement adjustment details |

### Adjustment Object

| Field                      | Description                                        |
| -------------------------- | -------------------------------------------------- |
| `resource_requirements_id` | ID of the resource requirement being adjusted      |
| `direction`                | `"upscale"` or `"downscale"`                       |
| `job_ids`                  | Job IDs sharing this resource requirement          |
| `job_names`                | Human-readable job names                           |
| `memory_adjusted`          | Whether memory was changed                         |
| `original_memory`          | Previous memory setting (if adjusted)              |
| `new_memory`               | New memory setting (if adjusted)                   |
| `max_peak_memory_bytes`    | Maximum peak memory observed across jobs           |
| `runtime_adjusted`         | Whether runtime was changed                        |
| `original_runtime`         | Previous runtime setting (if adjusted)             |
| `new_runtime`              | New runtime setting (if adjusted)                  |
| `cpu_adjusted`             | Whether CPU count was changed (omitted when false) |
| `original_cpus`            | Previous CPU count (if adjusted)                   |
| `new_cpus`                 | New CPU count (if adjusted)                        |
| `max_peak_cpu_percent`     | Maximum peak CPU percentage observed across jobs   |

## plot-resources Output Files

| File                                 | Description                                      |
| ------------------------------------ | ------------------------------------------------ |
| `resource_plot_job_<id>.html`        | Per-job timeline with CPU, memory, process count |
| `resource_plot_cpu_all_jobs.html`    | CPU comparison across all jobs                   |
| `resource_plot_memory_all_jobs.html` | Memory comparison across all jobs                |
| `resource_plot_summary.html`         | Bar chart dashboard of peak vs average           |

All plots are self-contained HTML files using Plotly.js with:

- Interactive hover tooltips
- Zoom and pan controls
- Legend toggling
- Export options (PNG, SVG)

## Monitored Metrics

| Metric         | Unit  | Description                               |
| -------------- | ----- | ----------------------------------------- |
| CPU percentage | %     | Total CPU utilization across all cores    |
| Memory usage   | bytes | Resident memory consumption               |
| Process count  | count | Number of processes in job's process tree |

### Process Tree Tracking

The monitoring system automatically tracks child processes spawned by jobs. When a job creates
worker processes (e.g., Python multiprocessing), all descendants are included in the aggregated
metrics.

## Slurm Accounting Stats

When running inside a Slurm allocation, Torc calls `sacct` after each job step completes and stores
the results in the `slurm_stats` table. These complement the sysinfo-based metrics above with
Slurm-native cgroup measurements.

### Fields

| Field                  | sacct source   | Description                           |
| ---------------------- | -------------- | ------------------------------------- |
| `max_rss_bytes`        | `MaxRSS`       | Peak resident-set size (from cgroups) |
| `max_vm_size_bytes`    | `MaxVMSize`    | Peak virtual memory size              |
| `max_disk_read_bytes`  | `MaxDiskRead`  | Peak disk read bytes                  |
| `max_disk_write_bytes` | `MaxDiskWrite` | Peak disk write bytes                 |
| `ave_cpu_seconds`      | `AveCPU`       | Average CPU time in seconds           |
| `node_list`            | `NodeList`     | Nodes used by the job step            |

Additional identifying fields stored per record: `workflow_id`, `job_id`, `run_id`, `attempt_id`,
`slurm_job_id`.

Fields are `null` when:

- The job ran locally (no `SLURM_JOB_ID` in the environment)
- `sacct` is not available on the node
- The step was not found in the Slurm accounting database at collection time

### Viewing Stats

```bash
torc slurm stats <workflow_id>
torc slurm stats <workflow_id> --job-id <job_id>
torc -f json slurm stats <workflow_id>
```

## Performance Characteristics

- Single background monitoring thread regardless of job count
- Typical overhead: <1% CPU even with 1-second sampling
- Uses native OS APIs via the `sysinfo` crate
- Non-blocking async design
