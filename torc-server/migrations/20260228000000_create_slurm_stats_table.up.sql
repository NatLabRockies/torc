-- Create slurm_stats table for per-step Slurm accounting data collected via sacct.
-- Keyed by (workflow_id, job_id, run_id, attempt_id) to match job result identity.
-- Only populated when torc runs inside a Slurm allocation (SLURM_JOB_ID is set).
CREATE TABLE slurm_stats (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    workflow_id INTEGER NOT NULL,
    job_id INTEGER NOT NULL,
    run_id INTEGER NOT NULL,
    attempt_id INTEGER NOT NULL DEFAULT 1,
    slurm_job_id TEXT NULL,
    max_rss_bytes INTEGER NULL,
    max_vm_size_bytes INTEGER NULL,
    max_disk_read_bytes INTEGER NULL,
    max_disk_write_bytes INTEGER NULL,
    ave_cpu_seconds REAL NULL,
    node_list TEXT NULL,
    FOREIGN KEY (workflow_id) REFERENCES workflow(id) ON DELETE CASCADE,
    FOREIGN KEY (job_id) REFERENCES job(id) ON DELETE CASCADE,
    UNIQUE(workflow_id, job_id, run_id, attempt_id)
);

CREATE INDEX idx_slurm_stats_workflow_job ON slurm_stats(workflow_id, job_id);
