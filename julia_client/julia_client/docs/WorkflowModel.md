# WorkflowModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**compute_node_expiration_buffer_seconds** | **Int64** |  | [optional] [default to nothing]
**compute_node_ignore_workflow_completion** | **Bool** |  | [optional] [default to nothing]
**compute_node_min_time_for_new_jobs_seconds** | **Int64** |  | [optional] [default to nothing]
**compute_node_wait_for_healthy_database_minutes** | **Int64** |  | [optional] [default to nothing]
**compute_node_wait_for_new_jobs_seconds** | **Int64** |  | [optional] [default to nothing]
**description** | **String** |  | [optional] [default to nothing]
**enable_ro_crate** | **Bool** |  | [optional] [default to nothing]
**env** | **Dict{String, String}** |  | [optional] [default to nothing]
**execution_config** | **String** |  | [optional] [default to nothing]
**id** | **Int64** |  | [optional] [default to nothing]
**is_archived** | **Bool** | True when the workflow has been archived. | [optional] [default to nothing]
**is_canceled** | **Bool** | True when a user (or scheduler) has canceled the workflow. | [optional] [default to nothing]
**metadata** | **String** |  | [optional] [default to nothing]
**name** | **String** |  | [default to nothing]
**project** | **String** |  | [optional] [default to nothing]
**resource_monitor_config** | **String** |  | [optional] [default to nothing]
**run_id** | **Int64** | Current run number; incremented on each restart/recovery. | [optional] [default to nothing]
**slurm_config** | **String** |  | [optional] [default to nothing]
**slurm_defaults** | **String** |  | [optional] [default to nothing]
**timestamp** | **String** |  | [optional] [default to nothing]
**use_pending_failed** | **Bool** |  | [optional] [default to nothing]
**user** | **String** |  | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


