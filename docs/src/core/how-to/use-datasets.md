# How to Use Datasets

This guide shows how to define and use datasets for directory-based outputs in your workflows.

## Basic Usage

### 1. Define a Dataset

Add a `datasets` section to your workflow spec:

```yaml
name: my_workflow
datasets:
  - name: output_data
    path: output/results/
    hash_mode: manifest
```

### 2. Mark Jobs as Producers

Jobs that write to the dataset use `output_datasets`:

```yaml
jobs:
  - name: producer
    command: python generate.py --output output/results/
    output_datasets:
      - output_data
```

### 3. Mark Jobs as Consumers

Jobs that read from the dataset use `input_datasets`:

```yaml
jobs:
  - name: consumer
    command: python aggregate.py --input output/results/
    input_datasets:
      - output_data
```

The consumer job will be blocked until all producer jobs complete and the dataset is finalized.

## Complete Example: Fan-In Pattern

This example shows 5 producer jobs writing to a shared dataset:

```yaml
name: fanin_example
description: "Demonstrates the fan-in pattern with datasets"

datasets:
  - name: partitioned_output
    path: output/partitions/
    hash_mode: manifest
    description: "Combined output from all workers"

jobs:
  - name: worker_{i}
    command: |
      mkdir -p output/partitions/
      echo "Result from worker {i}" > output/partitions/part_{i}.txt
    parameters:
      i: "0:4"
    output_datasets:
      - partitioned_output

  - name: combine
    command: |
      cat output/partitions/*.txt > output/combined.txt
      echo "Combined $(ls output/partitions/*.txt | wc -l) files"
    input_datasets:
      - partitioned_output
```

Run with:

```bash
torc run fanin_example.yaml
```

The `combine` job waits for all 5 workers to complete before running.

## Using Variable Substitution

Reference dataset paths in commands using `${datasets.input.NAME}` or `${datasets.output.NAME}`:

```yaml
datasets:
  - name: training_data
    path: data/training/

jobs:
  - name: preprocess
    command: |
      python preprocess.py \
        --input raw_data/ \
        --output ${datasets.output.training_data}
    output_datasets:
      - training_data

  - name: train
    command: python train.py --data ${datasets.input.training_data}
    input_datasets:
      - training_data
```

## Hash Modes

Choose a hash mode based on your needs:

### Manifest Mode (Default)

Fast integrity checking based on file metadata:

```yaml
datasets:
  - name: results
    path: output/
    hash_mode: manifest  # Hash of (path, size, mtime) tuples
```

### Content Mode

Thorough integrity checking by hashing file contents:

```yaml
datasets:
  - name: critical_data
    path: output/
    hash_mode: content  # SHA256 of all file contents
```

### No Hash

Skip integrity checking entirely:

```yaml
datasets:
  - name: logs
    path: logs/
    hash_mode: none  # Only track file count and size
```

## Multiple Datasets

Workflows can have multiple datasets with different purposes:

```yaml
datasets:
  - name: intermediate
    path: scratch/intermediate/
    hash_mode: none  # Fast, don't need integrity for temp data

  - name: final_output
    path: output/final/
    hash_mode: manifest  # Track integrity of final results

jobs:
  - name: stage1_{i}
    command: python stage1.py --part {i}
    parameters:
      i: "0:9"
    output_datasets:
      - intermediate

  - name: stage2_{i}
    command: python stage2.py --part {i}
    parameters:
      i: "0:9"
    input_datasets:
      - intermediate
    output_datasets:
      - final_output

  - name: finalize
    command: python finalize.py
    input_datasets:
      - final_output
```

## Manual Finalization

If a job runner crashes during finalization, you can manually finalize datasets:

```bash
# Finalize all pending datasets for a workflow
torc datasets finalize <workflow_id>

# Reclaim stale claims (stuck in 'finalizing' for too long)
torc datasets finalize <workflow_id> --reclaim-stale-after 10m

# Finalize a specific dataset
torc datasets finalize --dataset-id <dataset_id>

# Preview without making changes
torc datasets finalize <workflow_id> --dry-run
```

## Checking Dataset Status

View dataset information with:

```bash
# List all datasets for a workflow
torc datasets list <workflow_id>

# Filter by status
torc datasets list <workflow_id> --status pending
torc datasets list <workflow_id> --status finalized

# Get details for a specific dataset
torc datasets get <dataset_id>
```

## Troubleshooting

### Consumer job stays blocked

Check that all producer jobs have completed:

```bash
torc jobs list <workflow_id> --status completed
```

Verify the dataset status:

```bash
torc datasets list <workflow_id>
```

If the dataset is stuck in `finalizing`, try manual finalization:

```bash
torc datasets finalize <workflow_id> --reclaim-stale-after 5m
```

### Dataset path doesn't exist

Ensure producer jobs create the directory before writing:

```yaml
jobs:
  - name: producer
    command: |
      mkdir -p output/results/
      python generate.py --output output/results/
```

## See Also

- [Datasets Concept](../concepts/datasets.md) — Understanding datasets
- [Workflow Specification](../reference/workflow-spec.md#datasetspec) — Full schema reference
- [Dependency Resolution](../concepts/dependencies.md) — How dependencies work
