# FinalizeWorkflowResponse


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**errors** | **Vector{String}** | Problems that block finalization: dependency cycles, dangling references, and execution_config rule violations. | [default to nothing]
**finalized** | **Bool** | True if the workflow is no longer a draft (also true if it never was one). | [default to nothing]
**warnings** | **Vector{String}** | Slurm scheduler node-count mismatches and job resource requirements that don&#39;t fit the Slurm schedulers. They block finalization unless &#x60;force&#x3D;true&#x60;. | [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


