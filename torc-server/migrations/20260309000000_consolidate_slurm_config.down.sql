-- Revert: re-add limit_resources and use_srun columns extracted from slurm_config,
-- restore compute_node_expiration_buffer_seconds as NOT NULL with default 60,
-- and drop slurm_config column.

PRAGMA foreign_keys=OFF;

CREATE TABLE workflow_old (
  id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  description TEXT NULL,
  user TEXT NOT NULL,
  timestamp TEXT NOT NULL,
  is_archived INTEGER NOT NULL DEFAULT 0,
  compute_node_expiration_buffer_seconds INTEGER NOT NULL DEFAULT 60,
  compute_node_wait_for_new_jobs_seconds INTEGER NOT NULL DEFAULT 0,
  compute_node_ignore_workflow_completion INTEGER NOT NULL DEFAULT 0,
  compute_node_wait_for_healthy_database_minutes INTEGER NOT NULL DEFAULT 20,
  compute_node_min_time_for_new_jobs_seconds INTEGER NOT NULL DEFAULT 300,
  jobs_sort_method TEXT NOT NULL DEFAULT 'gpus_runtime_memory',
  status_id INTEGER NOT NULL,
  resource_monitor_config TEXT NULL,
  slurm_defaults TEXT NULL,
  use_pending_failed INTEGER NULL,
  limit_resources INTEGER NULL,
  use_srun INTEGER NULL,
  enable_ro_crate INTEGER NULL,
  project TEXT NULL,
  metadata TEXT NULL,
  FOREIGN KEY (status_id) REFERENCES workflow_status(id)
);

INSERT INTO workflow_old (
  id, name, description, user, timestamp, is_archived,
  compute_node_expiration_buffer_seconds,
  compute_node_wait_for_new_jobs_seconds,
  compute_node_ignore_workflow_completion,
  compute_node_wait_for_healthy_database_minutes,
  compute_node_min_time_for_new_jobs_seconds,
  jobs_sort_method, status_id, resource_monitor_config,
  slurm_defaults, use_pending_failed, limit_resources, use_srun,
  enable_ro_crate, project, metadata
)
SELECT
  id, name, description, user, timestamp, is_archived,
  COALESCE(compute_node_expiration_buffer_seconds, 60),
  compute_node_wait_for_new_jobs_seconds,
  compute_node_ignore_workflow_completion,
  compute_node_wait_for_healthy_database_minutes,
  compute_node_min_time_for_new_jobs_seconds,
  jobs_sort_method, status_id, resource_monitor_config,
  slurm_defaults, use_pending_failed,
  CASE WHEN json_extract(slurm_config, '$.limit_resources') IS NOT NULL THEN CASE WHEN json_extract(slurm_config, '$.limit_resources') THEN 1 ELSE 0 END ELSE NULL END,
  CASE WHEN json_extract(slurm_config, '$.use_srun') IS NOT NULL THEN CASE WHEN json_extract(slurm_config, '$.use_srun') THEN 1 ELSE 0 END ELSE NULL END,
  enable_ro_crate, project, metadata
FROM workflow;

DROP TABLE workflow;
ALTER TABLE workflow_old RENAME TO workflow;

CREATE INDEX IF NOT EXISTS idx_workflow_name ON workflow(name);
CREATE INDEX IF NOT EXISTS idx_workflow_user ON workflow(user);
CREATE INDEX IF NOT EXISTS idx_workflow_is_archived ON workflow(is_archived);

PRAGMA foreign_keys=ON;
