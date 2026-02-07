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

This analyzes both completed and failed jobs to detect:

- **Memory violations** — Jobs using more memory than allocated
- **CPU violations** — Jobs using more CPU than allocated
- **Runtime violations** — Jobs running longer than allocated time

The command will:

- Calculate new requirements using actual peak usage data
- Apply a 1.2x safety multiplier to each resource
- Update the workflow's resource requirements for future runs

Example:

```
Analyzing and correcting resource requirements for workflow 5
✓ Resource requirements updated successfully

Corrections applied:
  memory_training: 8g → 10g (+25.0%)
  cpu_training: 4 → 5 cores (+25.0%)
  runtime_training: PT2H → PT2H30M (+25.0%)
```

### Preview Changes Without Applying

Use `--dry-run` to see what changes would be made:

```bash
torc workflows correct-resources <workflow_id> --dry-run
```

### Correct Only Specific Jobs

To update only certain jobs (by ID):

```bash
torc workflows correct-resources <workflow_id> --job-ids 15,16,18
```

### Custom Correction Multiplier

Adjust the safety margin (default 1.2x):

```bash
torc workflows correct-resources <workflow_id> --memory-multiplier 1.5 --runtime-multiplier 1.4
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
