# Prioritize Jobs

Use job `priority` when some ready jobs should run before others.

This is useful when:

- A small number of urgent jobs should be claimed first
- Expensive compute should be reserved for the most important ready work
- Background or cleanup jobs should run only after higher-value jobs have been claimed

## How Priority Works

- Every job has a `priority`
- Higher values are claimed first
- The default is `0`
- Priority affects both queue-depth claims and resource-based claims
- A high-priority job still must fit the requesting runner's resources to be claimed

If two ready jobs have the same priority, Torc uses a stable tie-breaker.

## YAML Example

```yaml
name: priority_example

resource_requirements:
  - name: cpu_small
    num_cpus: 2
    memory: 4g
    num_gpus: 0
    runtime: PT30M

jobs:
  - name: urgent_report
    command: python make_report.py
    resource_requirements: cpu_small
    priority: 100

  - name: normal_batch
    command: python run_batch.py
    resource_requirements: cpu_small
    priority: 10

  - name: cleanup
    command: ./cleanup.sh
    resource_requirements: cpu_small
    priority: 0
```

## JSON Example

```json
{
  "name": "priority_example",
  "jobs": [
    {
      "name": "urgent_report",
      "command": "python make_report.py",
      "priority": 100
    },
    {
      "name": "normal_batch",
      "command": "python run_batch.py",
      "priority": 10
    }
  ]
}
```

## KDL Example

```kdl
name "priority_example"

jobs {
  job {
    name "urgent_report"
    command "python make_report.py"
    priority 100
  }

  job {
    name "normal_batch"
    command "python run_batch.py"
    priority 10
  }
}
```

## Priority and Resources

Priority does not override resource limits.

Example:

- `train_large_model` has priority `100` but needs 8 GPUs
- `small_analysis` has priority `20` and needs 1 CPU
- A runner with 1 CPU and 0 GPUs requests work

In that case, Torc skips `train_large_model` because it does not fit and can still return
`small_analysis`.

## Priority and Scheduler Assignment

Priority decides which eligible jobs are considered first. It does not force a job onto a specific
scheduler or cluster.

If a job must run on a specific scheduler, set its `scheduler` field. Use `priority` to order jobs
within that eligible set.

## Recommendations

- Use larger gaps like `100`, `50`, `10`, `0` so future adjustments are easy
- Reserve very high priorities for urgent or user-facing work
- Keep most jobs at `0` unless you need explicit ordering
- Use `scheduler` for placement constraints and `priority` for ordering
