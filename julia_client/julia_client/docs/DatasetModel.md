# DatasetModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **Int64** |  | [optional] [default to nothing]
**workflow_id** | **Int64** | Database ID of the workflow this dataset belongs to. | [default to nothing]
**name** | **String** | User-defined name of the dataset. | [default to nothing]
**path** | **String** | Path to the dataset directory. | [default to nothing]
**description** | **String** | Optional description of the dataset. | [optional] [default to nothing]
**hash_mode** | **String** | Hash mode for integrity verification (manifest, content, none). | [optional] [default to "manifest"]
**status** | **String** | Dataset status (pending, finalizing, finalized). | [optional] [default to "pending"]
**claimed_by_node_id** | **Int64** | ID of the compute node that claimed this dataset for finalization. | [optional] [default to nothing]
**claimed_at** | **Float64** | Unix timestamp when the dataset was claimed. | [optional] [default to nothing]
**file_count** | **Int64** | Number of files in the dataset (set after finalization). | [optional] [default to nothing]
**total_size_bytes** | **Int64** | Total size of all files in bytes (set after finalization). | [optional] [default to nothing]
**manifest_hash** | **String** | Hash of the dataset contents (set after finalization). | [optional] [default to nothing]
**finalized_at** | **Float64** | Unix timestamp when the dataset was finalized. | [optional] [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


