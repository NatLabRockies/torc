# RO-Crate Provenance Tracking

Torc supports [Research Object Crate (RO-Crate)](https://www.researchobject.org/ro-crate/), a
community standard for packaging research data with machine-readable metadata. This enables tracking
of data provenance—knowing which jobs produced which outputs, what inputs they consumed, and when
the data was created.

## What is RO-Crate?

RO-Crate is a lightweight approach to packaging research data with JSON-LD metadata. It provides:

- **Standardized metadata format** — Compatible with Schema.org and linked data tools
- **Provenance tracking** — Records how data was produced and transformed
- **Interoperability** — Works with repositories, archives, and other research tools

## How Torc Uses RO-Crate

Torc stores RO-Crate entities per workflow. Each entity describes a file, dataset, software, or
other research object with JSON-LD properties. Entities can be:

1. **Created automatically** when `enable_ro_crate: true` is set on a workflow
2. **Created manually** using the `torc ro-crate create` command
3. **Exported** as a standard `ro-crate-metadata.json` document

## Automatic Entity Generation

When you enable RO-Crate on a workflow, Torc automatically creates metadata entities:

**During workflow initialization:**

- File entities are created for all **input files** (files that exist on disk)
- Entities include MIME type inference, file size, and modification date

**When jobs complete successfully:**

- File entities are created for all **output files**
- CreateAction entities are created for each job (provenance)
- Output files are linked to their producing job via `wasGeneratedBy`

This creates a complete provenance graph linking inputs → jobs → outputs.

### Entity Structure

Automatically generated File entities include:

```json
{
  "@id": "data/output.csv",
  "@type": "File",
  "name": "output.csv",
  "encodingFormat": "text/csv",
  "contentSize": 1024,
  "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  "dateModified": "2024-01-01T00:00:00Z",
  "wasGeneratedBy": { "@id": "#job-42-attempt-1" }
}
```

File entities include SHA256 hashes for integrity verification when the file is readable.

Job provenance is captured as CreateAction entities:

```json
{
  "@id": "#job-42-attempt-1",
  "@type": "CreateAction",
  "name": "process_data",
  "instrument": { "@id": "#workflow-123" },
  "result": [{ "@id": "data/output.csv" }]
}
```

## Enabling Automatic RO-Crate

Add `enable_ro_crate: true` to your workflow specification:

```yaml
name: my_workflow
user: researcher
enable_ro_crate: true

files:
  - name: input_data
    path: data/input.csv  # Must exist on disk when workflow is created

  - name: output_data
    path: data/output.csv  # Will be created by the job

jobs:
  - name: process
    command: python process.py
    input_files: [input_data]
    output_files: [output_data]
```

Torc automatically detects input vs output files by checking if each file exists on disk when the
workflow is created. Files that exist are marked as inputs; files that don't exist are outputs.

After running this workflow:

- `input_data` will have an RO-Crate File entity (created during initialization)
- `output_data` will have an RO-Crate File entity with `wasGeneratedBy` linking to the job
- A CreateAction entity will describe the `process` job execution

## Dataset Entities for Directories

Many workflows produce directory-based outputs rather than single files—for example,
hive-partitioned Parquet datasets with thousands of files. For these, use **Dataset entities**
instead of File entities.

### Why Datasets?

- **Efficiency** — One metadata record instead of thousands of File entities
- **Appropriate granularity** — The directory is the meaningful unit, not individual partition files
- **Integrity verification** — Manifest-based hashing detects changes without reading all file
  contents

### Dataset Structure

Dataset entities include file count, total size, and an optional hash:

```json
{
  "@id": "output/training.parquet/",
  "@type": "Dataset",
  "name": "training_output",
  "description": "Hive-partitioned training results",
  "contentSize": 15032385536,
  "fileCount": 2847,
  "sha256": "a1b2c3...",
  "hashMode": "manifest",
  "encodingFormat": "application/vnd.apache.parquet"
}
```

### Hash Modes

Torc supports three hash modes for datasets:

| Mode       | Description                               | Speed   | Detects                            |
| ---------- | ----------------------------------------- | ------- | ---------------------------------- |
| `manifest` | Hash of sorted (path, size, mtime) list   | Fast    | File additions, deletions, renames |
| `content`  | SHA256 of all file contents (Merkle tree) | Slow    | Any content change                 |
| `none`     | No hash, only file count and size         | Fastest | Nothing (stats only)               |

For large datasets, `manifest` mode provides a good balance—it detects structural changes without
the I/O cost of reading terabytes of data.

### Creating Dataset Entities

Use the `add-dataset` command to create a Dataset entity for a directory:

```bash
torc ro-crate add-dataset \
  --workflow-id 123 \
  --name training_output \
  --path output/training.parquet/ \
  --hash-mode manifest
```

See [How to Add RO-Crate Metadata](../how-to/ro-crate-metadata.md) for detailed usage.

## When to Use RO-Crate

RO-Crate is valuable when you need to:

- **Track data lineage** — Know which jobs produced each output
- **Archive workflows** — Export metadata with your results for long-term storage
- **Share reproducible research** — Provide machine-readable provenance to collaborators
- **Meet compliance requirements** — Document data processing for audits or regulations

## Comparison: Automatic vs Manual

| Feature           | Automatic (`enable_ro_crate`)  | Manual (`torc ro-crate create`) |
| ----------------- | ------------------------------ | ------------------------------- |
| Input files       | Created on initialization      | Must create manually            |
| Output files      | Created on job completion      | Must create manually            |
| Job provenance    | CreateAction entities          | Must create manually            |
| Custom metadata   | Basic (name, type, size, date) | Full control over properties    |
| External entities | Not created                    | Can add software, datasets, etc |

For most workflows, enable automatic generation and add manual entities only for external references
(software versions, related datasets, etc.).

## See Also

- [How to Add RO-Crate Metadata](../how-to/ro-crate-metadata.md) — Step-by-step guide
- [RO-Crate Specification](https://www.researchobject.org/ro-crate/) — Official documentation
