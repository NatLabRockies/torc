# FileModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **Int64** |  | [optional] [default to nothing]
**name** | **String** |  | [default to nothing]
**path** | **String** |  | [default to nothing]
**sha256** | **String** | SHA-256 of the contents (lowercase hex), recorded with &#x60;st_mtime&#x60;. Same rules as &#x60;size&#x60;. | [optional] [default to nothing]
**size** | **Int64** | Size in bytes, recorded with &#x60;st_mtime&#x60;. Set only for workflows with &#x60;enable_ro_crate&#x60;. Set together with &#x60;sha256&#x60;. When &#x60;st_mtime&#x60; changes without a new &#x60;size&#x60; and &#x60;sha256&#x60;, or the path changes, the server clears both. | [optional] [default to nothing]
**st_mtime** | **Float64** |  | [optional] [default to nothing]
**workflow_id** | **Int64** |  | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


