# SlurmSchedulerModel


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**account** | **String** |  | [default to nothing]
**extra** | **String** |  | [optional] [default to nothing]
**gres** | **String** |  | [optional] [default to nothing]
**id** | **Int64** |  | [optional] [default to nothing]
**mem** | **String** |  | [optional] [default to nothing]
**name** | **String** |  | [optional] [default to nothing]
**nodes** | **Int64** |  | [default to nothing]
**ntasks_per_node** | **Int64** |  | [optional] [default to nothing]
**partition** | **String** |  | [optional] [default to nothing]
**qos** | **String** |  | [optional] [default to nothing]
**serialize_allocations** | **Bool** | Run this scheduler&#39;s allocations strictly one at a time.  When set, every allocation submitted for this scheduler shares one Slurm job name and carries &#x60;--dependency&#x3D;singleton&#x60;, so Slurm serializes them. Submit N allocations up front and they chain: each runs until its walltime can no longer fit a ready job, exits, and the next starts. Used for long sequential workflows that outlive any single allocation. | [optional] [default to nothing]
**tmp** | **String** |  | [optional] [default to nothing]
**walltime** | **String** |  | [default to nothing]
**workflow_id** | **Int64** |  | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


