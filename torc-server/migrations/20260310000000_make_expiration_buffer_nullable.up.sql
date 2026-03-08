-- Make compute_node_expiration_buffer_seconds nullable and set existing values to NULL.
-- This field is deprecated and should only be set explicitly by the user.
-- Also remove individual Slurm columns (limit_resources, use_srun, srun_termination_signal,
-- enable_cpu_bind) and merge their values into the slurm_config JSON blob.

-- SQLite doesn't support ALTER COLUMN, so we need to recreate the table.
-- However, since SQLite 3.35.0+ supports DROP COLUMN but not ALTER COLUMN nullability,
-- we use the rename-recreate pattern.

PRAGMA foreign_keys=OFF;

CREATE TABLE workflow_new (
  id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  description TEXT NULL,
  user TEXT NOT NULL,
  timestamp TEXT NOT NULL,
  is_archived INTEGER NOT NULL DEFAULT 0,
  compute_node_expiration_buffer_seconds INTEGER NULL,
  compute_node_wait_for_new_jobs_seconds INTEGER NOT NULL DEFAULT 0,
  compute_node_ignore_workflow_completion INTEGER NOT NULL DEFAULT 0,
  compute_node_wait_for_healthy_database_minutes INTEGER NOT NULL DEFAULT 20,
  compute_node_min_time_for_new_jobs_seconds INTEGER NOT NULL DEFAULT 300,
  jobs_sort_method TEXT NOT NULL DEFAULT 'gpus_runtime_memory',
  status_id INTEGER NOT NULL,
  resource_monitor_config TEXT NULL,
  slurm_defaults TEXT NULL,
  use_pending_failed INTEGER NULL,
  enable_ro_crate INTEGER NULL,
  project TEXT NULL,
  metadata TEXT NULL,
  slurm_config TEXT NULL,
  FOREIGN KEY (status_id) REFERENCES workflow_status(id)
);

INSERT INTO workflow_new (
  id, name, description, user, timestamp, is_archived,
  compute_node_expiration_buffer_seconds,
  compute_node_wait_for_new_jobs_seconds,
  compute_node_ignore_workflow_completion,
  compute_node_wait_for_healthy_database_minutes,
  compute_node_min_time_for_new_jobs_seconds,
  jobs_sort_method, status_id, resource_monitor_config,
  slurm_defaults, use_pending_failed,
  enable_ro_crate, project, metadata,
  slurm_config
)
SELECT
  id, name, description, user, timestamp, is_archived,
  NULL,
  compute_node_wait_for_new_jobs_seconds,
  compute_node_ignore_workflow_completion,
  compute_node_wait_for_healthy_database_minutes,
  compute_node_min_time_for_new_jobs_seconds,
  jobs_sort_method, status_id, resource_monitor_config,
  slurm_defaults, use_pending_failed,
  enable_ro_crate, project, metadata,
  CASE
    WHEN slurm_config IS NOT NULL THEN slurm_config
    WHEN limit_resources IS NOT NULL OR use_srun IS NOT NULL
         OR srun_termination_signal IS NOT NULL OR enable_cpu_bind IS NOT NULL THEN
      json_object(
        'limit_resources', CASE WHEN limit_resources = 1 THEN json('true') WHEN limit_resources = 0 THEN json('false') ELSE NULL END,
        'use_srun', CASE WHEN use_srun = 1 THEN json('true') WHEN use_srun = 0 THEN json('false') ELSE NULL END,
        'srun_termination_signal', srun_termination_signal,
        'enable_cpu_bind', CASE WHEN enable_cpu_bind = 1 THEN json('true') WHEN enable_cpu_bind = 0 THEN json('false') ELSE NULL END
      )
    ELSE NULL
  END as slurm_config
FROM workflow;

DROP TABLE workflow;
ALTER TABLE workflow_new RENAME TO workflow;

-- Recreate indexes
CREATE INDEX IF NOT EXISTS idx_workflow_name ON workflow(name);
CREATE INDEX IF NOT EXISTS idx_workflow_user ON workflow(user);
CREATE INDEX IF NOT EXISTS idx_workflow_is_archived ON workflow(is_archived);

PRAGMA foreign_keys=ON;
