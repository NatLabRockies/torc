# SchedulerPlanAction


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**job_ids** | **Vector{Int64}** | Jobs that gate the action. Must be a subset of that scheduler&#39;s &#x60;job_ids&#x60;. | [default to nothing]
**num_allocations** | **Int64** |  | [default to nothing]
**scheduler_index** | **Int64** | Index into &#x60;schedulers&#x60; of the scheduler this action submits. | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


