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

Instead of manually adjusting requirements, use the `--correct` flag to automatically increase
resource allocations based on actual usage:

```bash
torc reports check-resource-utilization <workflow_id> --correct
```

This will:

- Detect all resource over-utilization violations (memory, CPU, runtime)
- Calculate new requirements with a 1.2x multiplier for safety margin
- Update the workflow's resource requirements immediately

Example:

```
⚠ Found 1 resource over-utilization violations:
Job 15 (train_model): Memory over-utilization detected, peak 10.5 GB → allocating 12.6 GB (1.2x)
✓ Updated 1 resource requirements
```

### Preview Changes Without Applying

Use `--dry-run` to see what changes would be made:

```bash
torc reports check-resource-utilization <workflow_id> --correct --dry-run
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
