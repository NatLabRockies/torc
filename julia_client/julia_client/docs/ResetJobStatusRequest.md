# ResetJobStatusRequest


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**dry_run** | **Bool** | Report what would be reset without changing anything. | [optional] [default to nothing]
**job_ids** | **Vector{Int64}** | Jobs to reset. Omit to select every job in the workflow whose status is in &#x60;statuses&#x60;. | [optional] [default to nothing]
**statuses** | [**Vector{JobStatus}**](JobStatus.md) | Statuses a job must be in to be reset. Defaults to the recoverable statuses: failed, terminated, canceled, and pending_failed. | [optional] [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


