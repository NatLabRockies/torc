# How to Add RO-Crate Metadata

Store provenance information about simulation input/output data using
[Research Object Crates (RO-Crate)](https://www.researchobject.org/ro-crate/). Torc lets you attach
JSON-LD metadata entities to a workflow and export them as a valid `ro-crate-metadata.json`
document.

## Automatic Entity Generation

The easiest way to add RO-Crate metadata is to enable automatic generation. Set
`enable_ro_crate: true` in your workflow specification:

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
workflow is created. Files that exist get their modification time recorded.

When automatic generation is enabled:

- **Input files** (files that exist on disk) get File entities created during workflow
  initialization
- **Output files** get File entities with provenance (`wasGeneratedBy`) created when jobs complete
- **Jobs** get CreateAction entities linking to their output files

After running the workflow, export the metadata:

```bash
torc ro-crate export 123 -o ro-crate-metadata.json
```

The exported document includes complete provenance:

```json
{
  "@id": "data/output.csv",
  "@type": "File",
  "name": "output.csv",
  "encodingFormat": "text/csv",
  "wasGeneratedBy": { "@id": "#job-1-attempt-1" }
}
```

## Manual Entity Creation

For additional metadata (external software, custom properties), use manual commands.

## Quick Start

```bash
# Add an entity describing an output file
torc ro-crate create 123 \
  --entity-id "data/output.parquet" \
  --type File \
  --metadata '{"name": "Simulation Output", "encodingFormat": "application/x-parquet"}'

# Export all entities as an RO-Crate metadata document
torc ro-crate export 123 -o ro-crate-metadata.json
```

## Core Concepts

Each RO-Crate entity has:

| Field       | Description                                                                   |
| ----------- | ----------------------------------------------------------------------------- |
| `entity_id` | The JSON-LD `@id` (e.g., `"data/output.parquet"`, a URL)                      |
| `type`      | The Schema.org `@type` (e.g., `"File"`, `"Dataset"`, `"SoftwareApplication"`) |
| `metadata`  | A JSON string containing additional JSON-LD properties                        |
| `file_id`   | Optional link to a Torc file record                                           |

Entities are stored per-workflow. The `export` command assembles them into a complete RO-Crate
document with the required metadata descriptor and root dataset.

## Creating Entities

### File entity

Describe a single output file:

```bash
torc ro-crate create 123 \
  --entity-id "results/summary.csv" \
  --type File \
  --metadata '{"name": "Summary", "encodingFormat": "text/csv"}'
```

### Directory entity (Hive-partitioned data)

For directories with many files (like hive-partitioned Parquet datasets), use the `add-dataset`
command instead of creating entities manually. This automatically computes file count, total size,
and an integrity hash:

```bash
torc ro-crate add-dataset 123 \
  --name partitioned_table \
  --path data/partitioned_table/ \
  --hash-mode manifest \
  --encoding-format "application/vnd.apache.parquet"
```

See [Adding Dataset Entities](#adding-dataset-entities) below for full details.

### External software entity

Record which software produced the data (no `--file-id` needed):

```bash
torc ro-crate create 123 \
  --entity-id "https://example.com/simulation/v2.1" \
  --type SoftwareApplication \
  --metadata '{"name": "My Simulation", "version": "2.1.0"}'
```

### Link to a Torc file record

If the entity corresponds to a Torc file, link them with `--file-id`:

```bash
torc ro-crate create 123 \
  --entity-id "output.csv" \
  --type File \
  --file-id 42 \
  --metadata '{"name": "Output CSV"}'
```

### Read metadata from stdin

For large metadata objects, pipe from a file:

```bash
torc ro-crate create 123 \
  --entity-id "data/model.h5" \
  --type File \
  --metadata -  < metadata.json
```

## Adding Dataset Entities

For directory-based outputs (like hive-partitioned Parquet datasets), the `add-dataset` command
creates a Dataset entity with computed statistics and integrity hash.

### Basic Usage

```bash
torc ro-crate add-dataset 123 \
  --name training_output \
  --path output/training.parquet/
```

This walks the directory, counts files, sums sizes, computes a manifest hash, and creates:

```json
{
  "@id": "output/training.parquet/",
  "@type": "Dataset",
  "name": "training_output",
  "contentSize": 15032385536,
  "fileCount": 2847,
  "sha256": "7cbcd407fae0631505a1fe289356ee07c8825e41e9441fafca44c001bd6ce75d",
  "hashMode": "manifest"
}
```

### Hash Modes

Choose a hash mode based on your needs:

```bash
# Manifest hash (default) - fast, detects structural changes
torc ro-crate add-dataset 123 --name output --path data/ --hash-mode manifest

# Content hash - thorough but slow, detects any content change
torc ro-crate add-dataset 123 --name output --path data/ --hash-mode content

# No hash - fastest, only counts files and sizes
torc ro-crate add-dataset 123 --name output --path data/ --hash-mode none
```

| Mode       | What it hashes                     | When to use                          |
| ---------- | ---------------------------------- | ------------------------------------ |
| `manifest` | Sorted list of (path, size, mtime) | Large datasets, structural integrity |
| `content`  | All file contents (Merkle tree)    | Small datasets, content verification |
| `none`     | Nothing                            | Very large datasets, stats only      |

### Parallel Processing

For large directories, use multiple threads to speed up content hashing:

```bash
# Use 8 threads for content hashing
torc ro-crate add-dataset 123 \
  --name training_output \
  --path output/training.parquet/ \
  --hash-mode content \
  --threads 8
```

By default, the command uses all available CPU cores. The `--threads` option is most useful for
`content` mode where file I/O is the bottleneck.

### Full Example

```bash
torc ro-crate add-dataset 123 \
  --name simulation_results \
  --path output/results.parquet/ \
  --hash-mode manifest \
  --description "Hive-partitioned simulation output with 100 partitions" \
  --encoding-format "application/vnd.apache.parquet"
```

Output:

```
Computing dataset statistics for: output/results.parquet/ (using 8 threads)
  Files: 2847, Size: 15032385536 bytes
  Hash (manifest): 7cbcd407fae0631505a1fe289356ee07c8825e41e9441fafca44c001bd6ce75d
Created RO-Crate Dataset entity with ID: 42
```

### When to Use add-dataset vs create

| Scenario                       | Command       |
| ------------------------------ | ------------- |
| Directory with many files      | `add-dataset` |
| Need file count and total size | `add-dataset` |
| Need integrity hash            | `add-dataset` |
| Single file                    | `create`      |
| External URL or software       | `create`      |
| Custom metadata only           | `create`      |

## Listing and Viewing Entities

```bash
# List all entities for a workflow
torc ro-crate list 123

# Get a specific entity with full metadata
torc ro-crate get 1

# JSON output for scripting
torc -f json ro-crate list 123
```

## Updating Entities

Update individual fields of an existing entity:

```bash
# Change the type
torc ro-crate update 1 --type Dataset

# Update metadata
torc ro-crate update 1 --metadata '{"name": "Updated Name"}'

# Unlink from a file (set file_id to 0)
torc ro-crate update 1 --file-id 0
```

## Deleting Entities

```bash
# Delete a single entity
torc ro-crate delete 1
```

Entities are also automatically deleted when their parent workflow is deleted (cascade delete).

## Exporting an RO-Crate Document

The `export` command assembles all entities into a valid
[RO-Crate 1.1](https://w3id.org/ro/crate/1.1) metadata document:

```bash
# Write to file
torc ro-crate export 123 -o ro-crate-metadata.json

# Write to stdout
torc ro-crate export 123
```

The exported document has this structure:

```json
{
  "@context": "https://w3id.org/ro/crate/1.1/context",
  "@graph": [
    {
      "@id": "ro-crate-metadata.json",
      "@type": "CreativeWork",
      "about": {"@id": "./"},
      "conformsTo": {"@id": "https://w3id.org/ro/crate/1.1"}
    },
    {
      "@id": "./",
      "@type": "Dataset",
      "name": "my_workflow",
      "hasPart": [
        {"@id": "data/output.parquet"},
        {"@id": "https://example.com/simulation/v2.1"}
      ]
    },
    {
      "@id": "data/output.parquet",
      "@type": "File",
      "name": "Simulation Output",
      "encodingFormat": "application/x-parquet"
    },
    {
      "@id": "https://example.com/simulation/v2.1",
      "@type": "SoftwareApplication",
      "name": "My Simulation",
      "version": "2.1.0"
    }
  ]
}
```

The `@id` and `@type` fields are always set from the entity record, overriding any values in the
metadata JSON.

## Workflow Export/Import

RO-Crate entities are included in workflow exports (`torc workflows export`) and restored during
imports (`torc workflows import`). File ID links are remapped automatically.

## See Also

- [RO-Crate Provenance Tracking](../concepts/ro-crate.md) — Concept overview
- [RO-Crate Specification](https://www.researchobject.org/ro-crate/) — Official documentation
