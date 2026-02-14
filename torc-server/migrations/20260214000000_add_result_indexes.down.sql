-- Priority 1
DROP INDEX IF EXISTS idx_workflow_result_result_id;
DROP INDEX IF EXISTS idx_file_workflow_id;
DROP INDEX IF EXISTS idx_user_data_workflow_id;
DROP INDEX IF EXISTS idx_job_input_file_file_id;
DROP INDEX IF EXISTS idx_job_output_file_file_id;
DROP INDEX IF EXISTS idx_job_input_user_data_user_data_id;
DROP INDEX IF EXISTS idx_job_output_user_data_user_data_id;

-- Priority 2
DROP INDEX IF EXISTS idx_workflow_action_pending;
DROP INDEX IF EXISTS idx_resource_requirements_workflow_id;
DROP INDEX IF EXISTS idx_compute_node_workflow_id;
DROP INDEX IF EXISTS idx_scheduled_compute_node_workflow_status;

-- Priority 3
DROP INDEX IF EXISTS idx_local_scheduler_workflow_id;
DROP INDEX IF EXISTS idx_slurm_scheduler_workflow_id;
DROP INDEX IF EXISTS idx_workflow_user;
