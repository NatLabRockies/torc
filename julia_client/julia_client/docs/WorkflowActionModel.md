# WorkflowActionModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**action_config** | **Any** |  | [default to nothing]
**action_type** | **String** |  | [default to nothing]
**error** | **String** | Error reported by the claimer when &#x60;state&#x60; is &#x60;failed&#x60;. | [optional] [default to nothing]
**executed** | **Bool** |  | [optional] [default to false]
**executed_at** | **String** |  | [optional] [default to nothing]
**executed_by** | **Int64** |  | [optional] [default to nothing]
**id** | **Int64** |  | [optional] [default to nothing]
**is_recovery** | **Bool** |  | [optional] [default to false]
**job_ids** | **Vector{Int64}** |  | [optional] [default to nothing]
**persistent** | **Bool** |  | [optional] [default to false]
**required_triggers** | **Int64** |  | [optional] [default to 1]
**state** | **String** | Outcome of the action: &#x60;claimed&#x60;, &#x60;succeeded&#x60;, &#x60;failed&#x60;, or &#x60;suppressed&#x60;. Null if it was never claimed (or was claimed through a server older than API 1.4.0). Set by the server; ignored on create. | [optional] [default to nothing]
**trigger_count** | **Int64** |  | [optional] [default to 0]
**trigger_type** | **String** |  | [default to nothing]
**workflow_id** | **Int64** |  | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


