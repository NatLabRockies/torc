# OutputFileStat


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**file_id** | **Int64** |  | [default to nothing]
**sha256** | **String** | SHA-256 of the contents (lowercase hex). Sent only for workflows with &#x60;enable_ro_crate&#x60;. | [optional] [default to nothing]
**size** | **Int64** | Size in bytes. Sent only for workflows with &#x60;enable_ro_crate&#x60;, together with &#x60;sha256&#x60;. | [optional] [default to nothing]
**st_mtime** | **Float64** |  | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


