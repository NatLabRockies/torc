# SpawnJobModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**cancel_on_blocking_job_failure** | **Bool** |  | [optional] [default to nothing]
**command** | **String** |  | [default to nothing]
**depends_on** | **Vector{String}** | Job names this job depends on (existing jobs or siblings in this batch). | [optional] [default to nothing]
**name** | **String** |  | [default to nothing]
**priority** | **Int64** |  | [optional] [default to nothing]
**resource_requirements** | **String** | Name of an existing resource_requirements record in the workflow. | [optional] [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


