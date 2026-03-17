# JobFileRelationshipModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**file_id** | **Int64** | The file ID | [default to nothing]
**file_name** | **String** | The name of the file | [default to nothing]
**file_path** | **String** | The path of the file | [default to nothing]
**producer_job_id** | **Int64** | The job that produces this file (null for workflow inputs) | [optional] [default to nothing]
**producer_job_name** | **String** | The name of the job that produces this file | [optional] [default to nothing]
**consumer_job_id** | **Int64** | The job that consumes this file (null for workflow outputs) | [optional] [default to nothing]
**consumer_job_name** | **String** | The name of the job that consumes this file | [optional] [default to nothing]
**workflow_id** | **Int64** | The workflow containing the file and jobs | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)
