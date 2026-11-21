# JobCompletionEntry


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**job_id** | **Int64** |  | [default to nothing]
**output_files** | [**Vector{OutputFileStat}**](OutputFileStat.md) | Modification times of the job&#39;s output files, recorded when the job finished. Applied in the same transaction as the completion, and only if it is accepted. A &#x60;file_id&#x60; that is not an output file of this job is skipped; the completion is still applied. | [optional] [default to nothing]
**result** | [***ResultModel**](ResultModel.md) |  | [default to nothing]
**run_id** | **Int64** |  | [default to nothing]
**slurm_stats** | [***SlurmStatsModel**](SlurmStatsModel.md) | Slurm accounting stats for the job step. Stored only if the completion is accepted. Stats whose &#x60;workflow_id&#x60;, &#x60;job_id&#x60;, &#x60;run_id&#x60;, or &#x60;attempt_id&#x60; do not match the completion are skipped; the completion is still applied. | [optional] [default to nothing]
**status** | [***JobStatus**](JobStatus.md) |  | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


