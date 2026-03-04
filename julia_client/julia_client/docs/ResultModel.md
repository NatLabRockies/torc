# ResultModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **Int64** |  | [optional] [default to nothing]
**job_id** | **Int64** | Database ID for the job tied to this result | [default to nothing]
**workflow_id** | **Int64** | Database ID for the workflow tied to this result | [default to nothing]
**run_id** | **Int64** | ID of the workflow run. Incremements on every start and restart. | [default to nothing]
**attempt_id** | **Int64** | Retry attempt number for this result (starts at 1, increments on each retry) | [optional] [default to 1]
**compute_node_id** | **Int64** | Database ID for the compute node that ran this job | [default to nothing]
**return_code** | **Int64** | Code returned by the job. Zero is success; non-zero is a failure. | [default to nothing]
**exec_time_minutes** | **Float64** | Job execution time in minutes | [default to nothing]
**completion_time** | **String** | Timestamp of when the job completed. | [default to nothing]
**status** | [***JobStatus**](JobStatus.md) | Status of the job; managed by torc. | [default to nothing]
**peak_memory_bytes** | **Int64** | Peak memory usage in bytes | [optional] [default to nothing]
**avg_memory_bytes** | **Int64** | Average memory usage in bytes | [optional] [default to nothing]
**peak_cpu_percent** | **Float64** | Peak CPU usage as percentage (can exceed 100% for multi-core) | [optional] [default to nothing]
**avg_cpu_percent** | **Float64** | Average CPU usage as percentage (can exceed 100% for multi-core) | [optional] [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


