# JobUserDataRelationshipModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**user_data_id** | **Int64** | The user_data ID | [default to nothing]
**user_data_name** | **String** | The name of the user_data | [default to nothing]
**producer_job_id** | **Int64** | The job that produces this user_data (null for workflow inputs) | [optional] [default to nothing]
**producer_job_name** | **String** | The name of the job that produces this user_data | [optional] [default to nothing]
**consumer_job_id** | **Int64** | The job that consumes this user_data (null for workflow outputs) | [optional] [default to nothing]
**consumer_job_name** | **String** | The name of the job that consumes this user_data | [optional] [default to nothing]
**workflow_id** | **Int64** | The workflow containing the user_data and jobs | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)
