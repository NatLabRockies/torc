-- ============================================================================
-- DATASETS: First-Class Directory Outputs
-- ============================================================================
-- Issue: #184
-- This migration adds support for directory-based outputs (datasets) that
-- have multiple contributing jobs and aggregate completion semantics.

-- Add workflow-level flag to skip dataset logic when not needed
ALTER TABLE workflow ADD COLUMN has_datasets INTEGER NOT NULL DEFAULT 0;

-- ----------------------------------------------------------------------------
-- dataset: Directory-based output artifacts
-- ----------------------------------------------------------------------------
CREATE TABLE dataset (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    workflow_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    description TEXT,
    hash_mode TEXT NOT NULL DEFAULT 'manifest',  -- 'manifest', 'content', 'none'

    -- Status tracking
    status TEXT NOT NULL DEFAULT 'pending',  -- 'pending', 'finalizing', 'finalized'
    claimed_by_node_id INTEGER,
    claimed_at REAL,

    -- Computed on finalization
    file_count INTEGER,
    total_size_bytes INTEGER,
    manifest_hash TEXT,
    finalized_at REAL,

    FOREIGN KEY (workflow_id) REFERENCES workflow(id) ON DELETE CASCADE,
    FOREIGN KEY (claimed_by_node_id) REFERENCES compute_node(id) ON DELETE SET NULL,
    UNIQUE(workflow_id, name)
);

-- ----------------------------------------------------------------------------
-- job_dataset_output: Jobs that contribute to datasets (many-to-many)
-- ----------------------------------------------------------------------------
CREATE TABLE job_dataset_output (
    job_id INTEGER NOT NULL,
    dataset_id INTEGER NOT NULL,
    workflow_id INTEGER NOT NULL,
    PRIMARY KEY (job_id, dataset_id),
    FOREIGN KEY (job_id) REFERENCES job(id) ON DELETE CASCADE,
    FOREIGN KEY (dataset_id) REFERENCES dataset(id) ON DELETE CASCADE,
    FOREIGN KEY (workflow_id) REFERENCES workflow(id) ON DELETE CASCADE
);

-- ----------------------------------------------------------------------------
-- job_dataset_input: Jobs that depend on datasets (many-to-many)
-- ----------------------------------------------------------------------------
CREATE TABLE job_dataset_input (
    job_id INTEGER NOT NULL,
    dataset_id INTEGER NOT NULL,
    workflow_id INTEGER NOT NULL,
    PRIMARY KEY (job_id, dataset_id),
    FOREIGN KEY (job_id) REFERENCES job(id) ON DELETE CASCADE,
    FOREIGN KEY (dataset_id) REFERENCES dataset(id) ON DELETE CASCADE,
    FOREIGN KEY (workflow_id) REFERENCES workflow(id) ON DELETE CASCADE
);

-- ============================================================================
-- PERFORMANCE INDEXES
-- ============================================================================

-- Index for finding datasets by workflow
CREATE INDEX idx_dataset_workflow_id ON dataset(workflow_id);

-- Index for finding datasets by status (for finalization queries)
CREATE INDEX idx_dataset_status ON dataset(status);

-- Index for finding datasets pending finalization
CREATE INDEX idx_dataset_pending_finalization
ON dataset(workflow_id, status)
WHERE status = 'pending';

-- Index for finding jobs that output to a dataset
CREATE INDEX idx_job_dataset_output_dataset_id ON job_dataset_output(dataset_id);

-- Index for finding jobs that input from a dataset
CREATE INDEX idx_job_dataset_input_dataset_id ON job_dataset_input(dataset_id);

-- Index for workflow-scoped queries on job_dataset_output
CREATE INDEX idx_job_dataset_output_workflow_id ON job_dataset_output(workflow_id);

-- Index for workflow-scoped queries on job_dataset_input
CREATE INDEX idx_job_dataset_input_workflow_id ON job_dataset_input(workflow_id);
