# ListAdminAuditLogResponse


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**count** | **Int64** | Number of entries returned in this page. | [default to nothing]
**has_more** | **Bool** | True when more entries exist beyond this page. | [default to nothing]
**items** | [**Vector{AdminAuditLogEntry}**](AdminAuditLogEntry.md) | Audit-log entries, newest first. | [default to nothing]
**max_limit** | **Int64** | Maximum page size enforced by the server. | [default to nothing]
**offset** | **Int64** | Offset applied to this page. | [default to nothing]
**total_count** | **Int64** | Total number of audit-log entries. | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


