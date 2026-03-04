# ResourceRequirementsModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **Int64** | Database ID of this record. | [optional] [default to nothing]
**workflow_id** | **Int64** | Database ID of the workflow this record is associated with. | [default to nothing]
**name** | **String** | Name of the resource requirements | [default to nothing]
**num_cpus** | **Int64** | Number of CPUs required by a job | [optional] [default to 1]
**num_gpus** | **Int64** | Number of GPUs required by a job | [optional] [default to 0]
**num_nodes** | **Int64** | Number of nodes required by a job (allocation size for sbatch) | [optional] [default to 1]
**step_nodes** | **Int64** | Number of nodes each srun step spans. Distinct from num_nodes (allocation size). Defaults to 1. Set to num_nodes for MPI or Julia Distributed.jl jobs. | [optional] [default to 1]
**memory** | **String** | Amount of memory required by a job, e.g., 20g | [optional] [default to "1m"]
**runtime** | **String** | Maximum runtime for a job | [optional] [default to "P0DT1M"]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


