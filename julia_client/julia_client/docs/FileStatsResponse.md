# FileStatsResponse


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**changed_files** | [**Vector{FileStatChange}**](FileStatChange.md) |  | [default to nothing]
**missing_required_file_ids** | **Vector{Int64}** | Files in the batch that some job needs, no job produces, and that do not exist. | [default to nothing]
**reset_jobs** | [**Vector{FileStatsResetJob}**](FileStatsResetJob.md) |  | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


