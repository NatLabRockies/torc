# WorkflowModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **Int64** |  | [optional] [default to nothing]
**name** | **String** | Name of the workflow | [default to nothing]
**user** | **String** | User that created the workflow | [default to nothing]
**description** | **String** | Description of the workflow | [optional] [default to nothing]
**timestamp** | **String** | Timestamp of workflow creation | [optional] [default to nothing]
**project** | **String** | Project name or identifier for grouping workflows | [optional] [default to nothing]
**metadata** | **String** | Arbitrary metadata as JSON string | [optional] [default to nothing]
**compute_node_expiration_buffer_seconds** | **Int64** | Inform all compute nodes to shut down this number of seconds before the expiration time. This allows torc to send SIGTERM to all job processes and set all statuses to terminated. Increase the time in cases where the job processes handle SIGTERM and need more time to gracefully shut down. Set the value to 0 to maximize the time given to jobs. If not set, take the database&#39;s default value of 90 seconds. | [optional] [default to 180]
**compute_node_wait_for_new_jobs_seconds** | **Int64** | Inform all compute nodes to wait for new jobs for this time period before exiting. Does not apply if the workflow is complete. Default must be &gt;&#x3D; completion_check_interval_secs + job_completion_poll_interval to avoid exiting before dependent jobs are unblocked. | [optional] [default to 90]
**compute_node_ignore_workflow_completion** | **Bool** | Inform all compute nodes to ignore workflow completions and hold onto allocations indefinitely. Useful for debugging failed jobs and possibly dynamic workflows where jobs get added after starting. | [optional] [default to false]
**compute_node_wait_for_healthy_database_minutes** | **Int64** | Inform all compute nodes to wait this number of minutes if the database becomes unresponsive. | [optional] [default to 20]
**compute_node_min_time_for_new_jobs_seconds** | **Int64** | Minimum remaining walltime (in seconds) required before a compute node will request new jobs. If the remaining time is less than this value, the compute node will stop requesting new jobs and wait for running jobs to complete. This prevents starting jobs that won&#39;t have enough time to complete. Default is 300 seconds (5 minutes). | [optional] [default to 300]
**jobs_sort_method** | [***JobsSortMethod**](JobsSortMethod.md) |  | [optional] [default to nothing]
**resource_monitor_config** | **String** | Resource monitoring configuration as JSON string | [optional] [default to nothing]
**slurm_defaults** | **String** | Default Slurm parameters to apply to all schedulers as JSON string | [optional] [default to nothing]
**use_pending_failed** | **Bool** | Use PendingFailed status for failed jobs (enables AI-assisted recovery) | [optional] [default to false]
**status_id** | **Int64** |  | [optional] [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


