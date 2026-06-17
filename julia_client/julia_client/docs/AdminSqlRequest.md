# AdminSqlRequest


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**allow_full_table** | **Bool** | Permit an unqualified UPDATE/DELETE (no WHERE clause). Ignored on the read-only path. | [optional] [default to nothing]
**dry_run** | **Bool** | Write path only: run inside a transaction, report rows affected, then roll back instead of committing (preview). | [optional] [default to nothing]
**limit** | **Int64** | Maximum number of SELECT result rows to return. Defaults to and is capped at 10,000 (the standard list cap); values above the cap are clamped. | [optional] [default to nothing]
**sql** | **String** | The single SQL statement to execute. | [default to nothing]
**write** | **Bool** | Opt into the write path. When false (default) the statement runs on a read-only connection, so any write fails at the SQLite layer. | [optional] [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


