# Datasets Implementation Plan

Issue: #184 - Datasets: First-Class Directory Outputs

## Overview

Add first-class support for directory-based outputs (datasets) to Torc. Unlike individual files,
datasets:

- Have multiple contributing jobs (fan-in pattern)
- Are "complete" when all contributors finish
- Use manifest-based hashing for integrity (not per-file hashing)

## Phase 1: Database Schema & Server Foundation

### Database Migration

```sql
-- Add workflow-level flag to skip dataset logic when not needed
ALTER TABLE workflows ADD COLUMN has_datasets BOOLEAN NOT NULL DEFAULT FALSE;

-- Main datasets table
CREATE TABLE datasets (
    id INTEGER PRIMARY KEY,
    workflow_id INTEGER NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    description TEXT,
    hash_mode TEXT NOT NULL DEFAULT 'manifest',  -- 'manifest', 'content', 'none'

    -- Status tracking
    status TEXT NOT NULL DEFAULT 'pending',  -- 'pending', 'finalizing', 'finalized'
    claimed_by_node_id INTEGER REFERENCES compute_nodes(id),
    claimed_at REAL,

    -- Computed on finalization
    file_count INTEGER,
    total_size_bytes INTEGER,
    manifest_hash TEXT,
    finalized_at REAL,

    UNIQUE(workflow_id, name)
);

-- Track which jobs contribute to which datasets (output)
CREATE TABLE job_dataset_outputs (
    job_id INTEGER NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    dataset_id INTEGER NOT NULL REFERENCES datasets(id) ON DELETE CASCADE,
    PRIMARY KEY (job_id, dataset_id)
);

-- Track which jobs depend on which datasets (input)
CREATE TABLE job_dataset_inputs (
    job_id INTEGER NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    dataset_id INTEGER NOT NULL REFERENCES datasets(id) ON DELETE CASCADE,
    PRIMARY KEY (job_id, dataset_id)
);

CREATE INDEX idx_datasets_workflow_id ON datasets(workflow_id);
CREATE INDEX idx_datasets_status ON datasets(status);
CREATE INDEX idx_job_dataset_outputs_dataset_id ON job_dataset_outputs(dataset_id);
CREATE INDEX idx_job_dataset_inputs_dataset_id ON job_dataset_inputs(dataset_id);
```

### Server Models

```rust
pub struct DatasetModel {
    pub id: Option<i64>,
    pub workflow_id: i64,
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub hash_mode: HashMode,
    pub status: DatasetStatus,
    pub claimed_by_node_id: Option<i64>,
    pub claimed_at: Option<f64>,
    pub file_count: Option<i64>,
    pub total_size_bytes: Option<i64>,
    pub manifest_hash: Option<String>,
    pub finalized_at: Option<f64>,
}

pub enum DatasetStatus {
    Pending,              // Contributors still running
    Finalizing,           // Claimed by a runner, computing hash
    Finalized,            // Hash computed, ready for RO-Crate
}

pub enum HashMode {
    Manifest,  // Hash sorted (path, size, mtime) tuples - fast
    Content,   // SHA256 of all file contents - thorough
    None,      // No hash, just count/size - fastest
}
```

### Server API Endpoints

| Method | Endpoint                   | Description                          |
| ------ | -------------------------- | ------------------------------------ |
| POST   | `/workflows/{id}/datasets` | Create dataset                       |
| GET    | `/workflows/{id}/datasets` | List datasets                        |
| GET    | `/datasets/{id}`           | Get dataset details                  |
| PUT    | `/datasets/{id}`           | Update dataset                       |
| DELETE | `/datasets/{id}`           | Delete dataset                       |
| POST   | `/datasets/{id}/finalize`  | Finalize dataset (set hash, counts)  |
| POST   | `/datasets/reclaim-stale`  | Reclaim datasets stuck in finalizing |

### Extend complete_job Response

```rust
pub struct CompleteJobResponse {
    pub success: bool,
    pub datasets_to_finalize: Vec<DatasetFinalizationTask>,
}

pub struct DatasetFinalizationTask {
    pub dataset_id: i64,
    pub name: String,
    pub path: String,
    pub hash_mode: HashMode,
}
```

Server logic in `complete_job`:

```rust
// Only check datasets if workflow uses them
if workflow.has_datasets {
    datasets_to_finalize = claim_completed_datasets(job_id, compute_node_id)?;
} else {
    datasets_to_finalize = vec![];
}
```

Atomic claim query:

```sql
BEGIN IMMEDIATE TRANSACTION;

UPDATE datasets
SET status = 'finalizing',
    claimed_by_node_id = ?,
    claimed_at = ?
WHERE id IN (
    SELECT d.id FROM datasets d
    JOIN job_dataset_outputs jdo ON d.id = jdo.dataset_id
    WHERE jdo.job_id = ?  -- completing job
      AND d.status = 'pending'
      AND NOT EXISTS (
          SELECT 1 FROM job_dataset_outputs jdo2
          JOIN jobs j ON jdo2.job_id = j.id
          WHERE jdo2.dataset_id = d.id
            AND j.status NOT IN ('completed', 'disabled')
      )
)
RETURNING id, name, path, hash_mode;

COMMIT;
```

---

## Phase 2: Workflow Spec & Reference Parsing

### Spec Structures

```rust
pub struct DatasetSpec {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub hash_mode: Option<HashMode>,  // None = inherit from workflow default
}

// Add to WorkflowSpec
pub struct WorkflowSpec {
    // ... existing fields ...
    pub datasets: Option<Vec<DatasetSpec>>,
    pub default_hash_mode: Option<HashMode>,  // Workflow-level default
}
```

### Reference Syntax

```yaml
datasets:
  - name: training_output
    path: output/training.parquet/

jobs:
  - name: train_chunk_{i}
    command: >
      python train.py
        --output ${datasets.output.training_output}/chunk_{i}/
    parameters:
      i: "0:99"

  - name: aggregate
    command: >
      python aggregate.py
        --input ${datasets.input.training_output}
```

Reference resolution:

- `${datasets.output.X}` - Record in `job_dataset_outputs`, returns path
- `${datasets.input.X}` - Record in `job_dataset_inputs`, returns path

### Hash Mode Resolution Order

1. Dataset-level `hash_mode` (explicit override)
2. Workflow-level `default_hash_mode`
3. System default: `manifest`

### Workflow Creation

In `create_workflow_from_spec()`:

1. Create dataset records
2. Set `workflow.has_datasets = true` if any datasets defined
3. Parse job commands for `${datasets.*}` references
4. Populate `job_dataset_outputs` and `job_dataset_inputs`

---

## Phase 3: Dependency Resolution

### Blocking Semantics

A job with `${datasets.input.X}` is blocked until ALL jobs with `${datasets.output.X}` are complete.

```
train_chunk_0  --+
train_chunk_1  --+--> training_output --> aggregate
train_chunk_2  --+
...            --+
```

### Extend initialize_jobs

Modify the server's `initialize_jobs` to consider dataset dependencies:

```sql
-- Job is blocked if any input dataset has incomplete contributors
SELECT j.id
FROM jobs j
JOIN job_dataset_inputs jdi ON j.id = jdi.job_id
JOIN job_dataset_outputs jdo ON jdi.dataset_id = jdo.dataset_id
JOIN jobs contributor ON jdo.job_id = contributor.id
WHERE j.workflow_id = ?
  AND contributor.status NOT IN ('completed', 'disabled')
GROUP BY j.id;
```

### Extend unblock_jobs_waiting_for

When a job completes, check if this unblocks any dataset-dependent jobs:

```sql
-- Find jobs waiting on datasets that are now complete
SELECT DISTINCT j.id
FROM jobs j
JOIN job_dataset_inputs jdi ON j.id = jdi.job_id
WHERE j.status = 'blocked'
  AND NOT EXISTS (
      -- No incomplete contributors to any input dataset
      SELECT 1 FROM job_dataset_inputs jdi2
      JOIN job_dataset_outputs jdo ON jdi2.dataset_id = jdo.dataset_id
      JOIN jobs contributor ON jdo.job_id = contributor.id
      WHERE jdi2.job_id = j.id
        AND contributor.status NOT IN ('completed', 'disabled')
  );
```

---

## Phase 4: Dataset Finalization

### Job Runner Integration

```rust
// In job_runner after job completes:
let response = complete_job(&config, job_id, ...)?;

for task in response.datasets_to_finalize {
    info!("Finalizing dataset {} at {}", task.name, task.path);

    let result = compute_dataset_hash(&task.path, task.hash_mode)?;

    finalize_dataset(&config, task.dataset_id, DatasetFinalization {
        file_count: result.file_count,
        total_size_bytes: result.total_size_bytes,
        manifest_hash: result.hash,
    })?;
}
```

### Hash Computation

```rust
pub struct DatasetHashResult {
    pub file_count: i64,
    pub total_size_bytes: i64,
    pub hash: Option<String>,
}

pub fn compute_dataset_hash(path: &Path, mode: HashMode) -> Result<DatasetHashResult> {
    let mut file_count = 0i64;
    let mut total_size = 0i64;
    let mut entries: Vec<(String, u64, f64)> = Vec::new();  // (path, size, mtime)

    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let metadata = entry.metadata()?;
            file_count += 1;
            total_size += metadata.len() as i64;

            if mode != HashMode::None {
                let rel_path = entry.path().strip_prefix(path)?;
                let mtime = metadata.modified()?.duration_since(UNIX_EPOCH)?.as_secs_f64();
                entries.push((rel_path.to_string_lossy().into(), metadata.len(), mtime));
            }
        }
    }

    let hash = match mode {
        HashMode::Manifest => {
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let manifest: String = entries.iter()
                .map(|(p, s, m)| format!("{}|{}|{:.6}", p, s, m))
                .collect::<Vec<_>>()
                .join("\n");
            Some(sha256_hex(&manifest))
        }
        HashMode::Content => {
            // Compute SHA256 of each file, then hash the sorted list
            let mut file_hashes: Vec<(String, String)> = Vec::new();
            for (rel_path, _, _) in &entries {
                let full_path = path.join(rel_path);
                let hash = sha256_file(&full_path)?;
                file_hashes.push((rel_path.clone(), hash));
            }
            file_hashes.sort_by(|a, b| a.0.cmp(&b.0));
            let combined: String = file_hashes.iter()
                .map(|(p, h)| format!("{}|{}", p, h))
                .collect::<Vec<_>>()
                .join("\n");
            Some(sha256_hex(&combined))
        }
        HashMode::None => None,
    };

    Ok(DatasetHashResult { file_count, total_size_bytes: total_size, hash })
}
```

### Stale Claim Recovery

For runners that crash mid-finalization:

```bash
torc datasets finalize [workflow_id] [--reclaim-stale-after 10m]
```

```sql
-- Reclaim stale claims
UPDATE datasets
SET status = 'pending', claimed_by_node_id = NULL, claimed_at = NULL
WHERE status = 'finalizing'
  AND claimed_at < ?;  -- now - timeout

-- Then finalize all pending datasets
SELECT * FROM datasets WHERE status = 'pending' AND workflow_id = ?;
```

---

## Phase 5: RO-Crate Integration

### Dataset Entity

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
  "wasGeneratedBy": [
    { "@id": "#job-1-attempt-1" },
    { "@id": "#job-2-attempt-1" }
  ]
}
```

### Minimal Trait Abstraction

```rust
/// Common behavior for workflow artifacts (files and datasets)
pub trait WorkflowArtifact {
    fn id(&self) -> i64;
    fn workflow_id(&self) -> i64;
    fn name(&self) -> &str;
    fn path(&self) -> &str;

    /// Check if the artifact exists on the filesystem
    fn exists(&self) -> bool;

    /// Get the artifact type for RO-Crate
    fn ro_crate_type(&self) -> &'static str;
}

impl WorkflowArtifact for FileModel {
    fn ro_crate_type(&self) -> &'static str { "File" }
    // ...
}

impl WorkflowArtifact for DatasetModel {
    fn ro_crate_type(&self) -> &'static str { "Dataset" }
    // ...
}
```

The trait is intentionally minimal - blocking logic remains separate due to different semantics.

---

## Phase 6: CLI & Documentation

### CLI Commands

```bash
# List datasets
torc datasets list <workflow_id>

# Get dataset details
torc datasets get <dataset_id>

# Manual finalization (backup for crashed runners)
torc datasets finalize [workflow_id] [--reclaim-stale-after 10m]

# Finalize specific dataset
torc datasets finalize --dataset-id <id>
```

### Documentation Updates

Update `/docs/src/` with the following:

**Core Concepts** (`docs/src/core-concepts/datasets.md`):

- What datasets are and how they differ from files
- Fan-in pattern: multiple jobs contributing to one dataset
- Dataset lifecycle: pending → finalizing → finalized
- Hash modes: manifest vs content vs none

**How-To Guides** (`docs/src/how-to/`):

- `use-datasets.md` - Define datasets in workflow specs, reference syntax
- `finalize-datasets.md` - Manual finalization, recovering from crashes

**Tutorials** (`docs/src/tutorials/`):

- `distributed-training-with-datasets.md` - End-to-end tutorial:
  - Parameterized jobs writing to shared dataset
  - Aggregation job consuming dataset
  - RO-Crate export with dataset entities

**Reference Updates**:

- Update `workflow-spec.md` with `datasets` section and `${datasets.*}` syntax
- Update `cli-reference.md` with `torc datasets` commands

---

## Phase 7: Testing & Examples

### Integration Tests (`/tests/`)

Create `tests/test_datasets.rs`:

```rust
// Test cases:
#[test] fn test_create_dataset()
#[test] fn test_dataset_dependency_blocking()
#[test] fn test_dataset_unblocking_on_contributor_completion()
#[test] fn test_atomic_dataset_claim_on_complete_job()
#[test] fn test_dataset_finalization()
#[test] fn test_stale_claim_recovery()
#[test] fn test_hash_mode_manifest()
#[test] fn test_hash_mode_content()
#[test] fn test_hash_mode_none()
#[test] fn test_dataset_spec_parsing()
#[test] fn test_dataset_reference_resolution()
#[test] fn test_workflow_has_datasets_flag()
```

### Slurm Tests (`/slurm-tests/`)

Create `slurm-tests/test_datasets_slurm.sh`:

- Multi-node workflow with dataset outputs
- Verify atomic claiming works across Slurm jobs
- Test finalization by whichever job completes the dataset
- Verify RO-Crate export includes dataset entities

### Example Workflow Specs (`/examples/`)

Provide examples in all supported formats:

| File                                | Description                            |
| ----------------------------------- | -------------------------------------- |
| `dataset_fanin.yaml`                | Basic fan-in pattern (YAML)            |
| `dataset_fanin.json`                | Same example in JSON                   |
| `dataset_fanin.json5`               | Same example in JSON5                  |
| `dataset_fanin.kdl`                 | Same example in KDL                    |
| `dataset_hash_modes.yaml`           | Demonstrates all three hash modes      |
| `distributed_training_dataset.yaml` | Full training pipeline with datasets   |
| `dataset_with_ro_crate.yaml`        | Dataset workflow with RO-Crate enabled |

### Example: Basic Fan-In (`examples/dataset_fanin.yaml`)

```yaml
name: dataset_fanin_example
description: "Demonstrates fan-in pattern with datasets"
default_hash_mode: manifest

datasets:
  - name: partitioned_output
    path: output/partitioned/
    description: "Partitioned results from parallel jobs"

jobs:
  - name: process_chunk_{i}
    command: |
      mkdir -p output/partitioned/chunk_{i}
      echo "Result from chunk {i}" > output/partitioned/chunk_{i}/result.txt
    parameters:
      i: "0:4"
    output_datasets:
      - partitioned_output

  - name: aggregate
    command: |
      cat output/partitioned/*/result.txt > output/summary.txt
    input_datasets:
      - partitioned_output
```

---

## Implementation Order

```
Phase 1: Database Schema & Server Foundation
    - Migration, models, basic CRUD endpoints
    - Extend complete_job response
    - has_datasets workflow flag

Phase 2: Workflow Spec & Reference Parsing
    - DatasetSpec parsing
    - ${datasets.*} reference resolution
    - Junction table population

Phase 3: Dependency Resolution
    - Extend initialize_jobs for dataset dependencies
    - Extend unblock_jobs_waiting_for

Phase 4: Dataset Finalization
    - Hash computation (manifest/content/none)
    - Job runner integration
    - Stale claim recovery

Phase 5: RO-Crate Integration
    - Dataset entities in export
    - WorkflowArtifact trait

Phase 6: CLI & Documentation
    - torc datasets commands
    - Core concepts documentation
    - How-to guides
    - Tutorial: distributed training with datasets

Phase 7: Testing & Examples
    - Integration tests in /tests/test_datasets.rs
    - Slurm test in /slurm-tests/test_datasets_slurm.sh
    - Example specs in all formats (YAML, JSON, JSON5, KDL)
```

---

## Estimated Scope

| Phase     | Description           | Lines/Files |
| --------- | --------------------- | ----------- |
| Phase 1   | DB + API              | ~500        |
| Phase 2   | Spec parsing          | ~350        |
| Phase 3   | Dependency resolution | ~300        |
| Phase 4   | Finalization          | ~250        |
| Phase 5   | RO-Crate              | ~150        |
| Phase 6   | CLI/Docs              | ~400        |
| Phase 7   | Testing/Examples      | ~600        |
| **Total** |                       | ~2550       |

---

## Open Questions

1. **External input datasets** - Should datasets support `st_mtime` equivalent for pre-existing
   directories? (Probably: check directory exists at initialization)

2. **Partial re-runs** - If some contributors are re-run, should the dataset be re-finalized? (Yes:
   reset to `pending` when any contributor resets)

3. **Dataset deletion cleanup** - Should `torc datasets finalize` optionally delete the directory
   contents? (Probably not in v1)
