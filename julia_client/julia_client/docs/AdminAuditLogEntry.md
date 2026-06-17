# AdminAuditLogEntry


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**allow_full_table** | **Bool** | True when the full-table guard was overridden for this statement. | [default to nothing]
**committed** | **Bool** | True when the write was committed to the database. | [default to nothing]
**error** | **String** | Error message captured for a failed statement, when applicable. | [optional] [default to nothing]
**id** | **Int64** | Auto-increment row id. | [default to nothing]
**is_write** | **Bool** | True for write-path statements (all audited rows are writes). | [default to nothing]
**rows_affected** | **Int64** | Rows affected by the statement, when known. | [optional] [default to nothing]
**sql_text** | **String** | The SQL statement text. | [default to nothing]
**success** | **Bool** | True when the statement executed without error. | [default to nothing]
**timestamp** | **Int64** | Execution time in milliseconds since the Unix epoch. | [default to nothing]
**user_name** | **String** | User that executed the statement. | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


