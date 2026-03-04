# RoCrateEntityModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **Int64** | Database ID of this record | [optional] [default to nothing]
**workflow_id** | **Int64** | Database ID of the workflow this entity belongs to | [default to nothing]
**file_id** | **Int64** | Optional link to a file record | [optional] [default to nothing]
**entity_id** | **String** | The JSON-LD @id for this entity (e.g., \&quot;data/output.parquet\&quot;, \&quot;#job-42-attempt-1\&quot;) | [default to nothing]
**entity_type** | **String** | The Schema.org @type (e.g., \&quot;File\&quot;, \&quot;Dataset\&quot;, \&quot;SoftwareApplication\&quot;, \&quot;CreateAction\&quot;) | [default to nothing]
**metadata** | **String** | Full JSON-LD metadata object as a JSON string | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


