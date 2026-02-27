# How to Add RO-Crate Metadata

Store provenance information about simulation input/output data using
[Research Object Crates (RO-Crate)](https://www.researchobject.org/ro-crate/). Torc lets you attach
JSON-LD metadata entities to a workflow and export them as a valid `ro-crate-metadata.json`
document.

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

Use `Dataset` type with a trailing slash for directory-level entries. This avoids creating one
entity per partition file:

```bash
torc ro-crate create 123 \
  --entity-id "data/partitioned_table/" \
  --type Dataset \
  --metadata '{"name": "Partitioned Table", "encodingFormat": "application/x-parquet"}'
```

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
