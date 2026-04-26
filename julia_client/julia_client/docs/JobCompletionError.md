# JobCompletionError


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**error_code** | **String** | Machine-readable category for the failure. Stable values: - &#x60;already_complete&#x60; — benign race; the job was already in a   terminal state when the completion arrived. - &#x60;not_found&#x60; — the job_id does not exist. - &#x60;forbidden&#x60; — the caller cannot complete this job. - &#x60;validation&#x60; — input failed a per-completion validation check   (workflow mismatch, run-id mismatch, status not terminal, etc.). - &#x60;internal&#x60; — unmapped server-side failure.  Clients should treat &#x60;already_complete&#x60; as benign and any other value as a real desync that warrants stopping/escalating. | [default to nothing]
**job_id** | **Int64** |  | [default to nothing]
**message** | **String** |  | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


