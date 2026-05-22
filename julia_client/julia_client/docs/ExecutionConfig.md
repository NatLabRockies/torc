# ExecutionConfig


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**enable_cpu_bind** | **Bool** | When true, allow Slurm to bind tasks to specific CPU cores (slurm mode only). | [optional] [default to nothing]
**job_stdio_overrides** | [**Dict{String, StdioConfig}**](StdioConfig.md) | Per-job stdio overrides keyed by job name. Populated during workflow creation from per-job &#x60;stdio&#x60; fields in the spec. | [optional] [default to nothing]
**limit_resources** | **Bool** | When true (default), monitor memory/CPU usage and kill jobs that exceed their resource requirements (OOM enforcement). Only applies in direct mode. | [optional] [default to nothing]
**mode** | [***ExecutionMode**](ExecutionMode.md) | Execution mode: direct (default), slurm, or auto. | [optional] [default to nothing]
**oom_exit_code** | **Int64** | Exit code to use when a job is OOM-killed (direct mode only). Default: 137 (128 + SIGKILL &#x3D; 128 + 9). | [optional] [default to nothing]
**sigkill_headroom_seconds** | **Int64** | Seconds before end_time to send SIGKILL (direct mode) or set srun --time (slurm mode). Default: 60. | [optional] [default to nothing]
**sigterm_lead_seconds** | **Int64** | Seconds before SIGKILL to send the termination signal (direct mode only). Default: 30. | [optional] [default to nothing]
**srun_mpi** | **String** | MPI launcher mode for the outer &#x60;srun&#x60; used to launch one job runner per allocated node. | [optional] [default to nothing]
**srun_termination_signal** | **String** | Signal specification for srun steps, passed as &#x60;srun --signal&#x3D;&lt;value&gt;&#x60; (slurm mode only). | [optional] [default to nothing]
**staggered_start** | **Bool** | Enable staggered startup for job runners to mitigate thundering herd. | [optional] [default to nothing]
**stdio** | [***StdioConfig**](StdioConfig.md) | Workflow-level default for stdout/stderr capture. | [optional] [default to nothing]
**termination_signal** | **String** | Signal to send before SIGKILL for graceful termination (direct mode only). Default: \&quot;SIGTERM\&quot;. | [optional] [default to nothing]
**timeout_exit_code** | **Int64** | Exit code to use when a job times out. Default: 152 (matches Slurm&#39;s TIMEOUT exit code). | [optional] [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


