# JobDependencyModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**job_id** | **Int64** | The job that is blocked | [default to nothing]
**job_name** | **String** | The name of the job that is blocked | [default to nothing]
**depends_on_job_id** | **Int64** | The job that must complete first | [default to nothing]
**depends_on_job_name** | **String** | The name of the job that must complete first | [default to nothing]
**workflow_id** | **Int64** | The workflow containing both jobs | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)
