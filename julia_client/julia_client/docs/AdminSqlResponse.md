# AdminSqlResponse


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**columns** | **Vector{String}** | Column names for SELECT results (empty for write statements). | [default to nothing]
**committed** | **Bool** | True when a write was committed to the database. | [default to nothing]
**rows** | **Vector{Vector{Any}}** | Result rows; each row is a list of JSON-encoded cell values aligned with &#x60;columns&#x60;. | [default to nothing]
**rows_affected** | **Int64** | Number of rows affected by a write statement, when applicable. | [optional] [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


