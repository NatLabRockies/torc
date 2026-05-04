# JobModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**attempt_id** | **Int64** |  | [optional] [default to nothing]
**cancel_on_blocking_job_failure** | **Bool** |  | [optional] [default to nothing]
**command** | **String** |  | [default to nothing]
**depends_on_job_ids** | **Vector{Int64}** |  | [optional] [default to nothing]
**env** | **Dict{String, String}** |  | [optional] [default to nothing]
**failure_handler_id** | **Int64** |  | [optional] [default to nothing]
**id** | **Int64** |  | [optional] [default to nothing]
**input_file_ids** | **Vector{Int64}** |  | [optional] [default to nothing]
**input_user_data_ids** | **Vector{Int64}** |  | [optional] [default to nothing]
**invocation_script** | **String** |  | [optional] [default to nothing]
**name** | **String** |  | [default to nothing]
**output_file_ids** | **Vector{Int64}** |  | [optional] [default to nothing]
**output_user_data_ids** | **Vector{Int64}** |  | [optional] [default to nothing]
**priority** | **Int64** | Scheduling priority; higher values are submitted first. Minimum 0, default 0. | [optional] [default to 0]
**resource_requirements_id** | **Int64** |  | [optional] [default to nothing]
**schedule_compute_nodes** | [***ComputeNodeSchedule**](ComputeNodeSchedule.md) |  | [optional] [default to nothing]
**scheduler_id** | **Int64** |  | [optional] [default to nothing]
**status** | [***JobStatus**](JobStatus.md) |  | [optional] [default to nothing]
**supports_termination** | **Bool** |  | [optional] [default to nothing]
**workflow_id** | **Int64** |  | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


