# How to Check Resource Utilization

Compare actual resource usage against specified requirements to identify jobs that exceeded their
limits.

## Quick Start

```bash
torc reports check-resource-utilization <workflow_id>
```

Example output:

```
⚠ Found 2 resource over-utilization violations:

Job ID | Job Name    | Resource | Specified | Peak Used | Over-Utilization
-------|-------------|----------|-----------|-----------|------------------
15     | train_model | Memory   | 8.00 GB   | 10.50 GB  | +31.3%
15     | train_model | Runtime  | 2h 0m 0s  | 2h 45m 0s | +37.5%
```

## Show All Jobs

Include jobs that stayed within limits:

```bash
torc reports check-resource-utilization <workflow_id> --all
```

## Check a Specific Run

For workflows that have been reinitialized multiple times:

```bash
torc reports check-resource-utilization <workflow_id> --run-id 2
```

## Automatically Correct Requirements

Use the separate `correct-resources` command to automatically adjust resource allocations based on
actual resource measurements:

```bash
torc workflows correct-resources <workflow_id>
```

This command performs two types of corrections:

### Upscaling (over-utilized resources)

Analyzes completed and failed jobs to detect:

- **Memory violations** — Jobs using more memory than allocated
- **CPU violations** — Jobs using more CPU than allocated
- **Runtime violations** — Jobs running longer than allocated time

### Downsizing (under-utilized resources)

Analyzes successfully completed jobs (return code 0) to detect resources that are significantly
over-allocated. A resource is downsized only when:

- **All** jobs sharing that resource requirement completed successfully
- **All** jobs have peak usage data for that resource type
- The savings exceed minimum thresholds (1 GB for memory, 5 percentage points for CPU, 30 minutes
  for runtime)
- **No** job sharing that resource requirement had a violation

Failed jobs are excluded from downsizing analysis because they may terminate early with
under-reported peak usage.

The command will:

- Calculate new requirements using actual peak usage data
- Apply a 1.2x safety multiplier to each resource (configurable)
- Update the workflow's resource requirements for future runs

Example:

```
Resource Correction Summary:
  Workflow: 5
  Jobs analyzed: 3
  Resource requirements updated: 2
  Upscale:
    Memory corrections: 1
    Runtime corrections: 1
    CPU corrections: 1
  Downscale:
    Memory reductions: 2
    Runtime reductions: 2
    CPU reductions: 0
```

### Preview Changes Without Applying

Use `--dry-run` to see what changes would be made:

```bash
torc workflows correct-resources <workflow_id> --dry-run
```

### Correct Only Specific Jobs

To update only certain jobs (by ID). This filters both upscaling and downsizing:

```bash
torc workflows correct-resources <workflow_id> --job-ids 15,16,18
```

### Disable Downsizing

To only upscale over-utilized resources without reducing over-allocated ones:

```bash
torc workflows correct-resources <workflow_id> --no-downsize
```

### Custom Correction Multipliers

Adjust the safety margins independently (all default to 1.2x):

```bash
torc workflows correct-resources <workflow_id> \
  --memory-multiplier 1.5 \
  --cpu-multiplier 1.3 \
  --runtime-multiplier 1.4
```

## Manual Adjustment

For more control, update your workflow specification with a buffer:

```yaml
resource_requirements:
  - name: training
    memory: 12g       # 10.5 GB peak + 15% buffer
    runtime: PT3H     # 2h 45m actual + buffer
    num_cpus: 7       # Enough for peak CPU usage
```

**Guidelines:**

- Memory: Add 10-20% above peak usage
- Runtime: Add 15-30% above actual duration
- CPU: Round up to accommodate peak percentage (e.g., 501% CPU → 6 cores)

## See Also

- [Resource Monitoring](../monitoring/resource-monitoring.md) — Enable and configure monitoring
- [Resource Requirements Reference](../reference/resources.md) — Specification format
