-- Add missing indexes to improve query performance across the schema.
-- Many tables are routinely queried by workflow_id but lack an index on that column.
-- Junction tables with composite PKs (job_id, x_id) lack reverse-lookup indexes on x_id.

-- ============================================================================
-- Priority 1: High-impact (queried during initialization and dependency resolution)
-- ============================================================================

-- workflow_result: JOIN on result_id in list_results query
CREATE INDEX idx_workflow_result_result_id ON workflow_result(result_id);

-- file: filtered by workflow_id in list_files, delete_files, find_user_required_files,
-- find_job_produced_files, and initialize_jobs
CREATE INDEX idx_file_workflow_id ON file(workflow_id);

-- user_data: filtered by workflow_id in list_user_data, delete_all_user_data,
-- find_missing_user_created_data, find_missing_job_created_data
CREATE INDEX idx_user_data_workflow_id ON user_data(workflow_id);

-- job_input_file: PK is (job_id, file_id) but file_id is used in JOINs throughout
-- files.rs and critically in initialize_jobs for dependency resolution
CREATE INDEX idx_job_input_file_file_id ON job_input_file(file_id);

-- job_output_file: same pattern as job_input_file
CREATE INDEX idx_job_output_file_file_id ON job_output_file(file_id);

-- job_input_user_data: PK is (job_id, user_data_id) but user_data_id is used in
-- reverse-lookup JOINs in list_user_data, find_missing_user_created_data
CREATE INDEX idx_job_input_user_data_user_data_id ON job_input_user_data(user_data_id);

-- job_output_user_data: same pattern as job_input_user_data
CREATE INDEX idx_job_output_user_data_user_data_id ON job_output_user_data(user_data_id);

-- ============================================================================
-- Priority 2: Medium-impact (workflow management operations)
-- ============================================================================

-- workflow_action: queried by (workflow_id, trigger_type, executed) in get_pending_actions,
-- by (workflow_id, executed) in get_unexecuted_actions, and by workflow_id in list/delete
CREATE INDEX idx_workflow_action_pending ON workflow_action(workflow_id, trigger_type, executed);

-- resource_requirements: filtered by workflow_id in list and delete operations
CREATE INDEX idx_resource_requirements_workflow_id ON resource_requirements(workflow_id);

-- compute_node: filtered by workflow_id in list, delete, and get_by_hostname
CREATE INDEX idx_compute_node_workflow_id ON compute_node(workflow_id);

-- scheduled_compute_node: filtered by (workflow_id, status) in reset_workflow_status,
-- and by workflow_id in list/delete operations
CREATE INDEX idx_scheduled_compute_node_workflow_status ON scheduled_compute_node(workflow_id, status);

-- ============================================================================
-- Priority 3: Lower-impact (smaller tables, less frequent queries)
-- ============================================================================

-- local_scheduler: filtered by workflow_id in list and delete operations
CREATE INDEX idx_local_scheduler_workflow_id ON local_scheduler(workflow_id);

-- slurm_scheduler: filtered by workflow_id in list and delete operations
CREATE INDEX idx_slurm_scheduler_workflow_id ON slurm_scheduler(workflow_id);

-- workflow: filtered by user in list_workflows_filtered for multi-user deployments
CREATE INDEX idx_workflow_user ON workflow(user);
