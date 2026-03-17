# FailureHandlerModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **Int64** | Database ID of this failure handler. | [optional] [default to nothing]
**workflow_id** | **Int64** | Database ID of the workflow this handler is associated with. | [default to nothing]
**name** | **String** | Name of the failure handler | [default to nothing]
**rules** | **String** | JSON array of rule objects. Each rule can have: exit_codes (array of integers), match_all_exit_codes (boolean, matches any non-zero exit code), recovery_script (optional path to script), and max_retries (integer, defaults to 3). | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)
