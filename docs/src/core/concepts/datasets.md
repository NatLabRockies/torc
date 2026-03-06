# Datasets: First-Class Directory Outputs

Datasets are Torc's solution for managing directory-based outputs, particularly useful when multiple
jobs contribute to a shared output directory. Unlike individual files, datasets support a **fan-in**
pattern where many producer jobs write to the same directory, and consumer jobs wait for all
producers to complete before reading.

## When to Use Datasets

Use datasets when your workflow produces:

- **Hive-partitioned data** — Multiple jobs write partitions to a shared Parquet/Delta dataset
- **Distributed training outputs** — Each worker writes checkpoints or results to a common directory
- **Aggregated results** — Many jobs produce files that a final job must process together
- **Large directory trees** — Thousands of files where tracking each individually is impractical

## The Fan-In Pattern

The key difference between files and datasets is the fan-in dependency pattern:

```
train_chunk_0  --+
train_chunk_1  --+--> training_output --> aggregate
train_chunk_2  --+        (dataset)
...            --+
train_chunk_99 --+
```

With files, each job has its own output file. With datasets, multiple jobs can contribute to a
single output, and downstream jobs wait for **all contributors** to complete.

## Dataset Lifecycle

Datasets progress through three states:

| Status       | Description                                        |
| ------------ | -------------------------------------------------- |
| `pending`    | Waiting for all contributing jobs to complete      |
| `finalizing` | Claimed by a runner, computing hash and statistics |
| `finalized`  | Statistics computed, ready for dependent jobs      |

When the last contributing job completes, the job runner automatically:

1. Claims the dataset for finalization
2. Walks the directory to count files and compute total size
3. Computes a hash (based on `hash_mode`)
4. Updates the dataset record with statistics
5. Unblocks any jobs waiting on this dataset

## Hash Modes

Datasets support three hash modes for integrity verification:

| Mode       | What It Hashes                     | Speed   | Detects                            |
| ---------- | ---------------------------------- | ------- | ---------------------------------- |
| `manifest` | Sorted list of (path, size, mtime) | Fast    | File additions, deletions, renames |
| `content`  | SHA256 of all file contents        | Slow    | Any content change                 |
| `none`     | Nothing (file count and size only) | Fastest | Nothing (stats only)               |

For large datasets, `manifest` mode provides a good balance—it detects structural changes without
the I/O cost of reading terabytes of data. The default is `manifest`.

## Defining Datasets

Add a `datasets` section to your workflow specification:

```yaml
name: distributed_training
datasets:
  - name: training_output
    path: output/training.parquet/
    hash_mode: manifest
    description: "Partitioned training results"

jobs:
  - name: train_chunk_{i}
    command: |
      python train.py --partition {i} --output output/training.parquet/partition_{i}/
    parameters:
      i: "0:99"
    output_datasets:
      - training_output

  - name: aggregate
    command: python aggregate.py --input output/training.parquet/
    input_datasets:
      - training_output
```

The 100 `train_chunk_*` jobs all write to `training_output`. The `aggregate` job is blocked until
all 100 jobs complete and the dataset is finalized.

## Variable Substitution

You can reference datasets in job commands using variable substitution:

| Pattern                   | Description                                      |
| ------------------------- | ------------------------------------------------ |
| `${datasets.input.NAME}`  | Dataset path this job reads (creates dependency) |
| `${datasets.output.NAME}` | Dataset path this job writes (marks as producer) |

Example:

```yaml
jobs:
  - name: process
    command: |
      python process.py \
        --input ${datasets.input.raw_data} \
        --output ${datasets.output.processed}
```

## Comparison with Files

| Feature              | Files                  | Datasets                       |
| -------------------- | ---------------------- | ------------------------------ |
| Granularity          | Single file            | Directory of files             |
| Producers            | One job per file       | Multiple jobs per dataset      |
| Dependency pattern   | One-to-one             | Fan-in (many-to-one)           |
| Integrity check      | st_mtime               | Manifest/content hash          |
| Blocking             | Wait for producing job | Wait for all contributing jobs |
| RO-Crate entity type | File                   | Dataset                        |

## RO-Crate Integration

When `enable_ro_crate: true` is set on the workflow, finalized datasets automatically get RO-Crate
entities with:

- `@type: Dataset`
- `contentSize`: total size in bytes
- `fileCount`: number of files
- `sha256`: manifest or content hash
- `hashMode`: the hash mode used

See [RO-Crate Provenance](./ro-crate.md) for more details.

## CLI Commands

Manage datasets with the `torc datasets` commands:

```bash
# List datasets for a workflow
torc datasets list <workflow_id>

# Get dataset details
torc datasets get <dataset_id>

# Manually finalize datasets (for recovery)
torc datasets finalize <workflow_id>

# Finalize with stale claim recovery
torc datasets finalize <workflow_id> --reclaim-stale-after 10m
```

## See Also

- [How to Use Datasets](../how-to/use-datasets.md) — Step-by-step guide
- [Workflow Specification Reference](../reference/workflow-spec.md#datasetspec) — Full schema
- [Dependency Resolution](./dependencies.md) — How dependencies work
