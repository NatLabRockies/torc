# SpawnJobsRequest


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**jobs** | [**Vector{SpawnJobModel}**](SpawnJobModel.md) | Jobs to add. May be empty (record final state without spawning). | [default to nothing]
**lineage** | **String** | Orchestrator lineage identifier. Defaults to the calling job&#39;s name. The per-lineage spawn counter and state records are keyed on this. | [optional] [default to nothing]
**state** | **Any** |  | [optional] [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


