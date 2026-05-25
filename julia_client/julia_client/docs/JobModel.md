# JobModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**attempt_id** | **Int64** |  | [optional] [default to nothing]
**cancel_on_blocking_job_failure** | **Bool** |  | [optional] [default to nothing]
**command** | **String** |  | [default to nothing]
**compute_node_id** | **Int64** | Compute node executing the current attempt. Set by start_job and cleared by complete_job and the reset/retry paths. For completed attempts, the compute node is recorded on the result record. | [optional] [default to nothing]
**depends_on_job_ids** | **Vector{Int64}** |  | [optional] [default to nothing]
**env** | **Dict{String, String}** |  | [optional] [default to nothing]
**failure_handler_id** | **Int64** |  | [optional] [default to nothing]
**id** | **Int64** |  | [optional] [default to nothing]
**input_file_ids** | **Vector{Int64}** |  | [optional] [default to nothing]
**input_user_data_ids** | **Vector{Int64}** |  | [optional] [default to nothing]
**invocation_script** | **String** |  | [optional] [default to nothing]
**name** | **String** |  | [default to nothing]
**origin** | **String** | Provenance marker: NULL for jobs declared at workflow creation, &#x60;\&quot;retry\&quot;&#x60; for jobs resurrected by failure-handler retries, &#x60;\&quot;spawn\&quot;&#x60; for jobs added at runtime by &#x60;spawn_jobs&#x60;. &#x60;torc watch --auto-schedule&#x60; uses this to detect jobs that need unplanned Slurm allocations (deferred &#x60;schedule_nodes&#x60; actions only account for the originally-declared workload). | [optional] [default to nothing]
**output_file_ids** | **Vector{Int64}** |  | [optional] [default to nothing]
**output_user_data_ids** | **Vector{Int64}** |  | [optional] [default to nothing]
**priority** | **Int64** | Scheduling priority; higher values are submitted first. Minimum 0, default 0. | [optional] [default to 0]
**resource_requirements_id** | **Int64** |  | [optional] [default to nothing]
**schedule_compute_nodes** | [***ComputeNodeSchedule**](ComputeNodeSchedule.md) |  | [optional] [default to nothing]
**scheduler_id** | **Int64** |  | [optional] [default to nothing]
**start_time** | **String** | Timestamp when the current attempt began running. Set by start_job and cleared by complete_job and the reset/retry paths. NULL when the job is not running (use &#x60;status&#x60; as the source of truth for \&quot;is running\&quot;). | [optional] [default to nothing]
**status** | [***JobStatus**](JobStatus.md) |  | [optional] [default to nothing]
**supports_termination** | **Bool** |  | [optional] [default to nothing]
**workflow_id** | **Int64** |  | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


