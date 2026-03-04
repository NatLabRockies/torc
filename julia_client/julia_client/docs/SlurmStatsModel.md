# SlurmStatsModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **Int64** | Database ID for the record | [optional] [default to nothing]
**workflow_id** | **Int64** | Database ID for the workflow | [default to nothing]
**job_id** | **Int64** | Database ID for the job | [default to nothing]
**run_id** | **Int64** | ID of the workflow run | [default to nothing]
**attempt_id** | **Int64** | Retry attempt number (starts at 1) | [default to nothing]
**slurm_job_id** | **String** | Slurm allocation ID (from SLURM_JOB_ID env var) | [optional] [default to nothing]
**max_rss_bytes** | **Int64** | Max resident set size in bytes (from sacct MaxRSS) | [optional] [default to nothing]
**max_vm_size_bytes** | **Int64** | Max virtual memory size in bytes (from sacct MaxVMSize) | [optional] [default to nothing]
**max_disk_read_bytes** | **Int64** | Max disk read in bytes (from sacct MaxDiskRead) | [optional] [default to nothing]
**max_disk_write_bytes** | **Int64** | Max disk write in bytes (from sacct MaxDiskWrite) | [optional] [default to nothing]
**ave_cpu_seconds** | **Float64** | Average CPU time in seconds (from sacct AveCPU) | [optional] [default to nothing]
**node_list** | **String** | Node(s) on which the step ran (from sacct NodeList) | [optional] [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


