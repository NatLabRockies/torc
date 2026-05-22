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
**dynamic_jobs** | [***DynamicJobsConfig**](DynamicJobsConfig.md) | Dynamic job spawning configuration. Mirrors the workflow-spec &#x60;dynamic_jobs&#x60; section identically. Runtime-immutable after workflow creation. | [optional] [default to nothing]
**enable_ro_crate** | **Bool** |  | [optional] [default to nothing]
**env** | **Dict{String, String}** |  | [optional] [default to nothing]
**execution_config** | [***ExecutionConfig**](ExecutionConfig.md) |  | [optional] [default to nothing]
**id** | **Int64** |  | [optional] [default to nothing]
**is_archived** | **Bool** | True when the workflow has been archived. Read-only on the API: set via &#x60;POST /workflows/{id}/archive&#x60;, cleared via &#x60;POST /workflows/{id}/reset_status&#x60;. Values supplied to create/update workflow endpoints are ignored. | [optional] [readonly] [default to nothing]
**is_canceled** | **Bool** | True when a user (or scheduler) has canceled the workflow. Read-only on the API: set via &#x60;POST /workflows/{id}/cancel&#x60;, cleared via &#x60;POST /workflows/{id}/reset_status&#x60;. Values supplied to create/update workflow endpoints are ignored. | [optional] [readonly] [default to nothing]
**metadata** | **Dict{String, Any}** |  | [optional] [default to nothing]
**name** | **String** |  | [default to nothing]
**project** | **String** |  | [optional] [default to nothing]
**resource_monitor_config** | [***ResourceMonitorConfig**](ResourceMonitorConfig.md) |  | [optional] [default to nothing]
**run_id** | **Int64** | Current run number; incremented on each restart/recovery. Read-only on the API: incremented as a side effect of &#x60;POST /workflows/{id}/reset_status&#x60;. Values supplied to create/update workflow endpoints are ignored. | [optional] [readonly] [default to nothing]
**slurm_defaults** | **Dict{String, Any}** |  | [optional] [default to nothing]
**timestamp** | **String** |  | [optional] [default to nothing]
**use_pending_failed** | **Bool** |  | [optional] [default to nothing]
**user** | **String** |  | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


