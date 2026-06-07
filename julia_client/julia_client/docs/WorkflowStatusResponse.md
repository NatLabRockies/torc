# WorkflowStatusResponse


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**active_compute_nodes** | **Int64** |  | [default to nothing]
**active_scheduled_nodes** | **Int64** |  | [default to nothing]
**is_canceled** | **Bool** |  | [default to nothing]
**is_complete** | **Bool** |  | [default to nothing]
**jobs_by_status** | [***JobStatusCounts**](JobStatusCounts.md) |  | [default to nothing]
**longest_ready_runtime_seconds** | **Int64** | Longest required runtime (seconds) among ready jobs. Only populated when some ready jobs are runtime-blocked. | [optional] [default to nothing]
**max_allocation_remaining_seconds** | **Int64** | Greatest remaining walltime (seconds) across active walltime-bounded allocations. None when no active allocation reports an end time. | [optional] [default to nothing]
**pending_scheduled_nodes** | **Int64** |  | [default to nothing]
**runtime_blocked_ready_jobs** | **Int64** | Ready jobs whose required runtime exceeds the remaining walltime of every active allocation, so they cannot start until a fresh allocation appears. 0 when there are no walltime-bounded allocations. See &#x60;torc workflows diagnose&#x60;. | [default to nothing]
**total_exec_time_minutes** | **Float64** |  | [default to nothing]
**total_jobs** | **Int64** |  | [default to nothing]
**walltime_seconds** | **Float64** |  | [optional] [default to nothing]
**workflow_id** | **Int64** |  | [default to nothing]
**workflow_name** | **String** |  | [default to nothing]
**workflow_user** | **String** |  | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


