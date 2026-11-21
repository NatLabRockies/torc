# ResetJobStatusResponse


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**reset** | **Vector{Int64}** | IDs of the jobs reset (or, for a dry run, that would be reset). Set only when the request has a body. | [optional] [default to nothing]
**reset_type** | **String** |  | [optional] [default to nothing]
**skipped** | [**Vector{ResetJobStatusSkipped}**](ResetJobStatusSkipped.md) | Requested jobs that were not reset, with the reason. Set only when the request has a body. | [optional] [default to nothing]
**status** | **String** |  | [default to nothing]
**updated_count** | **Int64** |  | [default to nothing]
**workflow_id** | **Int64** |  | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


