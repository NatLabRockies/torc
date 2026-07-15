# Workflow Definition

A **workflow** is a collection of jobs with dependencies. You define workflows in YAML, JSON5, or
JSON files.

## Minimal Example

```yaml
name: hello_world
jobs:
  - name: greet
    command: echo "Hello, World!"
```

That's it. One job, no dependencies.

## Jobs with Dependencies

```yaml
name: two_stage
jobs:
  - name: prepare
    command: ./prepare.sh

  - name: process
    command: ./process.sh
    depends_on: [prepare]
```

The `process` job waits for `prepare` to complete.

## Job Parameterization

Create multiple jobs from a single definition using parameters:

```yaml
name: parameter_sweep
jobs:
  - name: task_{i}
    command: ./run.sh --index {i}
    parameters:
      i: "1:10"
```

This expands to 10 jobs: `task_1`, `task_2`, ..., `task_10`.

### Parameter Formats

| Format          | Example          | Expands To                |
| --------------- | ---------------- | ------------------------- |
| Range           | `"1:5"`          | 1, 2, 3, 4, 5             |
| Range with step | `"0:10:2"`       | 0, 2, 4, 6, 8, 10         |
| List            | `[a, b, c]`      | a, b, c                   |
| Float range     | `"0.0:1.0:0.25"` | 0.0, 0.25, 0.5, 0.75, 1.0 |

Lists are native YAML/JSON sequences; the legacy string form (`"[a,b,c]"`) is still accepted and is
required in KDL specs.

### Format Specifiers

Control how values appear in names:

```yaml
- name: job_{i:03d}      # job_001, job_002, ...
  parameters:
    i: "1:100"

- name: lr_{lr:.4f}      # lr_0.0010, lr_0.0100, ...
  parameters:
    lr: [0.001, 0.01, 0.1]
```

## Resource Requirements

Specify what resources each job needs:

```yaml
name: gpu_workflow

resource_requirements:
  - name: gpu_job
    num_cpus: 8
    num_gpus: 1
    memory: 16g
    runtime: PT2H

jobs:
  - name: train
    command: python train.py
    resource_requirements: gpu_job
```

Resource requirements are used for:

- Local execution: ensuring jobs don't exceed available resources
- HPC/Slurm: requesting appropriate allocations

## Complete Example

```yaml
name: data_pipeline
description: Process data in parallel, then aggregate

resource_requirements:
  - name: worker
    num_cpus: 4
    memory: 8g
    runtime: PT1H

jobs:
  - name: process_{i}
    command: python process.py --chunk {i} --output results/chunk_{i}.json
    resource_requirements: worker
    parameters:
      i: "1:10"

  - name: aggregate
    command: python aggregate.py --input results/ --output final.json
    resource_requirements: worker
    depends_on:
      - process_{i}
    parameters:
      i: "1:10"
```

This creates:

- 10 parallel `process_*` jobs
- 1 `aggregate` job that waits for all 10 to complete

## Failure Recovery Options

Control how Torc handles job failures:

### Default Behavior

By default, jobs that fail without a matching failure handler use `Failed` status:

```yaml
name: my_workflow
jobs:
  - name: task
    command: ./run.sh  # If this fails, status = Failed
```

### AI-Assisted Recovery (Opt-in)

Enable intelligent classification of ambiguous failures:

```yaml
name: ml_training
use_pending_failed: true  # Enable AI-assisted recovery

jobs:
  - name: train_model
    command: python train.py
```

With `use_pending_failed: true`:

- Jobs without matching failure handlers get `PendingFailed` status
- AI agent can analyze stderr and decide whether to retry or fail
- See [AI-Assisted Recovery](../../specialized/fault-tolerance/ai-assisted-recovery.md) for details

## See Also

- [Workflow Specification Formats](../workflows/workflow-formats.md) — Complete syntax reference
- [Job Parameterization](../reference/parameterization.md) — Advanced parameter options
- [Dependency Resolution](./dependencies.md) — How dependencies work
