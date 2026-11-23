# ApplySchedulerPlanResponse


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**actions** | [**Vector{AppliedSchedulerAction}**](AppliedSchedulerAction.md) | Created recovery actions, in the order of the request&#39;s &#x60;recovery_actions&#x60;. A recovery action whose gating jobs were all skipped is not created. | [default to nothing]
**scheduler_ids** | **Vector{Int64}** | Created scheduler IDs, in the order of the request&#39;s &#x60;schedulers&#x60;. | [default to nothing]
**skipped_job_ids** | **Vector{Int64}** | Planned jobs left out because they were no longer uninitialized, blocked, or ready when the plan was applied (e.g. a running allocation claimed them after planning). | [optional] [default to nothing]
**suppressed_action_ids** | **Vector{Int64}** |  | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


