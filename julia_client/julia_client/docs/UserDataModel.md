# UserDataModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **Int64** |  | [optional] [default to nothing]
**workflow_id** | **Int64** | Database ID of the workflow this record is associated with. | [default to nothing]
**is_ephemeral** | **Bool** | The data will only exist for the duration of one run. Torc will clear it before starting new runs. | [optional] [default to false]
**name** | **String** | Name of the data object | [default to nothing]
**data** | **Any** | User-defined data | [optional] [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)
