# Tutorial 5: Advanced Multi-Dimensional Parameterization

This tutorial teaches you how to create multi-dimensional parameter sweeps—grid searches over
multiple hyperparameters that generate all combinations automatically.

## Learning Objectives

By the end of this tutorial, you will:

- Understand how multiple parameters create a Cartesian product (all combinations)
- Learn to structure complex workflows with data preparation, training, and aggregation stages
- Know how to combine parameterization with explicit dependencies
- See patterns for running large grid searches on HPC systems

## Prerequisites

- Completed [Tutorial 4: Simple Parameterization](./simple-params.md)
- Torc server running
- Understanding of file dependencies

## Multi-Dimensional Parameters: Cartesian Product

When a job has multiple parameters, Torc creates the **Cartesian product**—every combination of
values:

```yaml
parameters:
  lr: "[0.001,0.01]"   # 2 values
  bs: "[16,32]"        # 2 values
```

This generates 2 × 2 = **4 jobs**:

- `lr=0.001, bs=16`
- `lr=0.001, bs=32`
- `lr=0.01, bs=16`
- `lr=0.01, bs=32`

With three parameters:

```yaml
parameters:
  lr: "[0.0001,0.001,0.01]"  # 3 values
  bs: "[16,32,64]"            # 3 values
  opt: "['adam','sgd']"       # 2 values
```

This generates 3 × 3 × 2 = **18 jobs**.

## Step 1: Create the Workflow Specification

Save as `grid_search.yaml`:

```yaml
name: hyperparameter_grid_search
description: 3D grid search over learning rate, batch size, and optimizer

jobs:
  # Data preparation (runs once, no parameters)
  - name: prepare_data
    command: python prepare_data.py --output=/data/processed.pkl
    resource_requirements: data_prep
    output_files:
      - training_data

  # Training jobs (one per parameter combination)
  - name: train_lr{lr:.4f}_bs{bs}_opt{opt}
    command: |
      python train.py \
        --data=/data/processed.pkl \
        --learning-rate={lr} \
        --batch-size={bs} \
        --optimizer={opt} \
        --output=/models/model_lr{lr:.4f}_bs{bs}_opt{opt}.pt \
        --metrics=/results/metrics_lr{lr:.4f}_bs{bs}_opt{opt}.json
    resource_requirements: gpu_training
    input_files:
      - training_data
    output_files:
      - model_lr{lr:.4f}_bs{bs}_opt{opt}
      - metrics_lr{lr:.4f}_bs{bs}_opt{opt}
    parameters:
      lr: "[0.0001,0.001,0.01]"
      bs: "[16,32,64]"
      opt: "['adam','sgd']"

  # Aggregate results (depends on ALL training jobs via file dependencies)
  - name: aggregate_results
    command: |
      python aggregate.py \
        --input-dir=/results \
        --output=/results/summary.csv
    resource_requirements: minimal
    input_files:
      - metrics_lr{lr:.4f}_bs{bs}_opt{opt}
    parameters:
      lr: "[0.0001,0.001,0.01]"
      bs: "[16,32,64]"
      opt: "['adam','sgd']"

  # Find best model (explicit dependency, no parameters)
  - name: select_best_model
    command: |
      python select_best.py \
        --summary=/results/summary.csv \
        --output=/results/best_config.json
    resource_requirements: minimal
    depends_on:
      - aggregate_results

files:
  - name: training_data
    path: /data/processed.pkl

  - name: model_lr{lr:.4f}_bs{bs}_opt{opt}
    path: /models/model_lr{lr:.4f}_bs{bs}_opt{opt}.pt
    parameters:
      lr: "[0.0001,0.001,0.01]"
      bs: "[16,32,64]"
      opt: "['adam','sgd']"

  - name: metrics_lr{lr:.4f}_bs{bs}_opt{opt}
    path: /results/metrics_lr{lr:.4f}_bs{bs}_opt{opt}.json
    parameters:
      lr: "[0.0001,0.001,0.01]"
      bs: "[16,32,64]"
      opt: "['adam','sgd']"

resource_requirements:
  - name: data_prep
    num_cpus: 8
    memory: 32g
    runtime: PT1H

  - name: gpu_training
    num_cpus: 8
    num_gpus: 1
    memory: 16g
    runtime: PT4H

  - name: minimal
    num_cpus: 1
    memory: 2g
    runtime: PT10M
```

### Understanding the Structure

**Four-stage workflow:**

1. **`prepare_data`** (1 job) - No parameters, runs once
2. **`train_*`** (18 jobs) - Parameterized, all depend on `prepare_data`
3. **`aggregate_results`** (1 job) - Has parameters only for file dependency matching
4. **`select_best_model`** (1 job) - Explicit dependency on `aggregate_results`

**Key insight: Why `aggregate_results` has parameters**

The `aggregate_results` job won't expand into multiple jobs (its name has no `{}`). However, it
needs `parameters:` to match the parameterized `input_files`. This tells Torc: "this job depends on
ALL 18 metrics files."

## Step 2: Create and Initialize the Workflow

```bash
WORKFLOW_ID=$(torc create grid_search.yaml -f json | jq -r '.id')
echo "Created workflow: $WORKFLOW_ID"

torc workflows init $WORKFLOW_ID
```

## Step 3: Verify the Expansion

Count the jobs:

```bash
torc jobs list $WORKFLOW_ID -f json | jq '.jobs | length'
```

Expected: **21 jobs** (1 prepare + 18 training + 1 aggregate + 1 select)

List the training jobs:

```bash
torc jobs list $WORKFLOW_ID -f json | jq -r '.jobs[] | select(.name | startswith("train_")) | .name' | sort
```

Output (18 training jobs):

```
train_lr0.0001_bs16_optadam
train_lr0.0001_bs16_optsgd
train_lr0.0001_bs32_optadam
train_lr0.0001_bs32_optsgd
train_lr0.0001_bs64_optadam
train_lr0.0001_bs64_optsgd
train_lr0.0010_bs16_optadam
train_lr0.0010_bs16_optsgd
train_lr0.0010_bs32_optadam
train_lr0.0010_bs32_optsgd
train_lr0.0010_bs64_optadam
train_lr0.0010_bs64_optsgd
train_lr0.0100_bs16_optadam
train_lr0.0100_bs16_optsgd
train_lr0.0100_bs32_optadam
train_lr0.0100_bs32_optsgd
train_lr0.0100_bs64_optadam
train_lr0.0100_bs64_optsgd
```

## Step 4: Examine the Dependency Graph

```bash
torc jobs list $WORKFLOW_ID
```

Initial states:

- `prepare_data`: **ready** (no dependencies)
- All `train_*`: **blocked** (waiting for `training_data` file)
- `aggregate_results`: **blocked** (waiting for all 18 metrics files)
- `select_best_model`: **blocked** (waiting for `aggregate_results`)

## Step 5: Run the Workflow

For local execution:

```bash
torc run $WORKFLOW_ID
```

Execution flow:

1. `prepare_data` runs and produces `training_data`
2. All 18 `train_*` jobs unblock and run in parallel (resource-limited)
3. `aggregate_results` waits for all training jobs, then runs
4. `select_best_model` runs last

## Step 6: Monitor Progress

```bash
# Check status summary
torc status $WORKFLOW_ID

# Watch job completion in real-time
watch -n 10 'torc jobs list-by-status $WORKFLOW_ID'

# Or use the TUI
torc tui
```

## Step 7: Retrieve Results

After completion:

```bash
# View best configuration
cat /results/best_config.json

# View summary of all runs
cat /results/summary.csv
```

## Scaling Considerations

### Job Count Growth

Multi-dimensional parameters grow exponentially:

| Dimensions | Values per Dimension | Total Jobs |
| ---------- | -------------------- | ---------- |
| 1          | 10                   | 10         |
| 2          | 10 × 10              | 100        |
| 3          | 10 × 10 × 10         | 1,000      |
| 4          | 10 × 10 × 10 × 10    | 10,000     |

### Dependency Count

Without barriers, dependencies also grow quickly. In this tutorial:

- 18 training jobs each depend on 1 file = 18 dependencies
- 1 aggregate job depends on 18 files = 18 dependencies
- Total: ~36 dependencies

For larger sweeps (1000+ jobs), consider the [barrier pattern](./multi-stage-barrier.md) to reduce
dependencies from O(n²) to O(n).

## Common Patterns

### Mixing Fixed and Parameterized Jobs

```yaml
jobs:
  # Fixed job (no parameters)
  - name: setup
    command: ./setup.sh

  # Parameterized jobs depend on fixed job
  - name: experiment_{i}
    command: ./run.sh {i}
    depends_on: [setup]
    parameters:
      i: "1:100"
```

### Aggregating Parameterized Results

Use the file dependency pattern shown in this tutorial:

```yaml
- name: aggregate
  input_files:
    - result_{i}    # Matches all parameterized result files
  parameters:
    i: "1:100"      # Same parameters as producer jobs
```

### Nested Parameter Sweeps

For workflows with multiple independent sweeps:

```yaml
jobs:
  # Sweep 1
  - name: sweep1_job_{a}
    parameters:
      a: "1:10"

  # Sweep 2 (independent of sweep 1)
  - name: sweep2_job_{b}
    parameters:
      b: "1:10"
```

## What You Learned

In this tutorial, you learned:

- ✅ How multiple parameters create a Cartesian product of jobs
- ✅ How to structure multi-stage workflows (prep → train → aggregate → select)
- ✅ How to use parameters in file dependencies to collect all outputs
- ✅ How to mix parameterized and non-parameterized jobs
- ✅ Scaling considerations for large grid searches

## Example Files

See these example files for hyperparameter sweep patterns:

- [hyperparameter_sweep.yaml](https://github.com/NatLabRockies/torc/blob/main/examples/yaml/hyperparameter_sweep.yaml) -
  Basic 3×3×2 grid search
- [hyperparameter_sweep_shared_params.yaml](https://github.com/NatLabRockies/torc/blob/main/examples/yaml/hyperparameter_sweep_shared_params.yaml) -
  Grid search with shared parameter definitions

## Next Steps

- [Multi-Stage Workflows with Barriers](./multi-stage-barrier.md) - Essential for scaling to
  thousands of jobs
- [Advanced Slurm Configuration](../../specialized/hpc/slurm.md) - Deploy grid searches on HPC
  clusters
- [Resource Monitoring](../monitoring/resource-monitoring.md) - Track resource usage across your
  sweep
