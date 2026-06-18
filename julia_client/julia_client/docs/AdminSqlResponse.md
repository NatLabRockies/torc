# AdminSqlResponse


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**columns** | **Vector{String}** | Column names in result order, defining how &#x60;items&#x60; is displayed (empty for write statements). Names the query repeats are suffixed (&#x60;id&#x60;, &#x60;id_2&#x60;, ...) so each item&#39;s keys stay unique. | [default to nothing]
**committed** | **Bool** | True when a write was committed to the database. | [default to nothing]
**items** | **Vector{Dict{String, Any}}** | Result rows as objects keyed by &#x60;columns&#x60; (empty for write statements). | [default to nothing]
**rows_affected** | **Int64** | Number of rows affected by a write statement, when applicable. | [optional] [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


