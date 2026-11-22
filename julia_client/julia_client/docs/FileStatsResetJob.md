# FileStatsResetJob


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**job_id** | **Int64** |  | [default to nothing]
**name** | **String** |  | [default to nothing]
**reason** | **String** | &#x60;input_changed&#x60; (an input file changed), &#x60;output_missing&#x60; (an output file of a completed job is missing), or &#x60;downstream&#x60; (depends, directly or transitively, on a reset job). | [default to nothing]
**status** | [***JobStatus**](JobStatus.md) | Status before the reset. | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


