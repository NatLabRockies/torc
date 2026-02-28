-- Add Slurm accounting (sacct) fields to the result table.
-- These fields are populated when a job runs inside a Slurm allocation (SLURM_JOB_ID is set).
-- sacct is called once after each job step exits; values are NULL for local (non-Slurm) runs.
ALTER TABLE result ADD COLUMN sacct_max_rss_bytes INTEGER NULL;
ALTER TABLE result ADD COLUMN sacct_max_disk_read_bytes INTEGER NULL;
ALTER TABLE result ADD COLUMN sacct_max_disk_write_bytes INTEGER NULL;
ALTER TABLE result ADD COLUMN sacct_ave_cpu_seconds REAL NULL;
