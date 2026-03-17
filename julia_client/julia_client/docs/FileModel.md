# FileModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **Int64** |  | [optional] [default to nothing]
**workflow_id** | **Int64** | Database ID of the workflow this record is associated with. | [default to nothing]
**name** | **String** | User-defined name of the file (not necessarily the filename) | [default to nothing]
**path** | **String** | Path to the file; can be relative to the execution directory. | [default to nothing]
**st_mtime** | **Float64** | Timestamp of when the file was last modified | [optional] [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)
