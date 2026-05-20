# SpawnJobsResponse


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**iteration** | **Int64** | This lineage&#39;s spawn-iteration counter after the call. | [default to nothing]
**spawned_job_ids** | **Vector{Int64}** | IDs of the spawned jobs. On a fresh call this is the IDs of the newly inserted jobs; on an idempotent replay (same names already exist) it is the IDs of those pre-existing jobs in the order they appear in the request. Empty only when the request&#39;s &#x60;jobs&#x60; array is empty (e.g. a final-state convergence call). | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


