# JobNameRefs


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**depends_on** | **Vector{String}** | Names of jobs this job depends on. | [optional] [default to nothing]
**depends_on_regexes** | **Vector{String}** | Regexes over job names; every matching job becomes a dependency. | [optional] [default to nothing]
**failure_handler** | **String** | Name of the failure handler. | [optional] [default to nothing]
**input_file_regexes** | **Vector{String}** | Regexes over file names for input files. | [optional] [default to nothing]
**input_files** | **Vector{String}** | Names of input files. | [optional] [default to nothing]
**input_user_data** | **Vector{String}** | Names of input user_data records. | [optional] [default to nothing]
**input_user_data_regexes** | **Vector{String}** | Regexes over user_data names for input user_data. | [optional] [default to nothing]
**output_file_regexes** | **Vector{String}** | Regexes over file names for output files. | [optional] [default to nothing]
**output_files** | **Vector{String}** | Names of output files. | [optional] [default to nothing]
**output_user_data** | **Vector{String}** | Names of output user_data records. | [optional] [default to nothing]
**output_user_data_regexes** | **Vector{String}** | Regexes over user_data names for output user_data. | [optional] [default to nothing]
**resource_requirements** | **String** | Name of the resource requirements record. Takes precedence over the \&quot;default\&quot; fallback. | [optional] [default to nothing]
**scheduler** | **String** | Name of the Slurm scheduler. | [optional] [default to nothing]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


