# WorkflowActionModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **Int64** | Unique identifier for the action | [optional] [default to nothing]
**workflow_id** | **Int64** | ID of the workflow this action belongs to | [default to nothing]
**trigger_type** | **String** | Type of trigger (e.g., on_workflow_start, on_jobs_ready, on_jobs_complete, on_worker_start, on_worker_complete) | [default to nothing]
**action_type** | **String** | Type of action to execute (e.g., run_commands, schedule_nodes) | [default to nothing]
**action_config** | **Any** | Configuration for the action (JSON object) | [default to nothing]
**job_ids** | **Vector{Int64}** | Array of job IDs that this action applies to (for job-specific triggers) | [optional] [default to nothing]
**executed** | **Bool** | Whether the action has been executed | [optional] [default to false]
**executed_at** | **String** | Timestamp when the action was executed | [optional] [default to nothing]
**executed_by** | **Int64** | ID of the compute node that executed the action | [optional] [default to nothing]
**persistent** | **Bool** | Whether the action can be claimed by multiple workers (persistent) or only once (non-persistent) | [optional] [default to false]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)
