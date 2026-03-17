# SlurmSchedulerModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **Int64** |  | [optional] [default to nothing]
**workflow_id** | **Int64** | Database ID of the workflow this scheduler is associated with. | [default to nothing]
**name** | **String** | Name of the scheduler | [optional] [default to nothing]
**account** | **String** | Slurm account ID | [default to nothing]
**gres** | **String** | Generic resource requirement | [optional] [default to nothing]
**mem** | **String** | Compute node memory requirement | [optional] [default to nothing]
**nodes** | **Int64** | Number of nodes for the Slurm allocation | [default to nothing]
**ntasks_per_node** | **Int64** | Number of tasks to invoke on each node | [optional] [default to nothing]
**partition** | **String** | Compute node partition; likely not necessary because Slurm should optimize it. | [optional] [default to nothing]
**qos** | **String** | Priority of Slurm job | [optional] [default to "normal"]
**tmp** | **String** | Compute node local storage size requirement | [optional] [default to nothing]
**walltime** | **String** | Slurm runtime requirement, e.g., 04:00:00 | [default to nothing]
**extra** | **String** | Extra Slurm parameters that torc will append to the sbatch command | [optional] [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)
