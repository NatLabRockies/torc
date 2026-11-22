# ApplySchedulerPlanRequest


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**recovery_actions** | [**Vector{SchedulerPlanAction}**](SchedulerPlanAction.md) |  | [optional] [default to nothing]
**schedulers** | [**Vector{SchedulerPlanEntry}**](SchedulerPlanEntry.md) |  | [default to nothing]
**suppress_existing_schedule_actions** | **Bool** | Suppress the workflow&#39;s non-recovery, unexecuted &#x60;schedule_nodes&#x60; actions (persistent ones included) so they cannot start allocations alongside the new schedulers. | [optional] [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


