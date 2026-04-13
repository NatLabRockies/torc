# SlurmStatsApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_slurm_stats**](SlurmStatsApi.md#create_slurm_stats) | **POST** /slurm_stats | 
[**list_slurm_stats**](SlurmStatsApi.md#list_slurm_stats) | **GET** /slurm_stats | 


# **create_slurm_stats**
> create_slurm_stats(_api::SlurmStatsApi, slurm_stats_model::SlurmStatsModel; _mediaType=nothing) -> SlurmStatsModel, OpenAPI.Clients.ApiResponse <br/>
> create_slurm_stats(_api::SlurmStatsApi, response_stream::Channel, slurm_stats_model::SlurmStatsModel; _mediaType=nothing) -> Channel{ SlurmStatsModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **SlurmStatsApi** | API context | 
**slurm_stats_model** | [**SlurmStatsModel**](SlurmStatsModel.md) |  |

### Return type

[**SlurmStatsModel**](SlurmStatsModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_slurm_stats**
> list_slurm_stats(_api::SlurmStatsApi, workflow_id::Int64; job_id=nothing, run_id=nothing, attempt_id=nothing, offset=nothing, limit=nothing, _mediaType=nothing) -> ListSlurmStatsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_slurm_stats(_api::SlurmStatsApi, response_stream::Channel, workflow_id::Int64; job_id=nothing, run_id=nothing, attempt_id=nothing, offset=nothing, limit=nothing, _mediaType=nothing) -> Channel{ ListSlurmStatsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **SlurmStatsApi** | API context | 
**workflow_id** | **Int64** |  |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **job_id** | **Int64** |  | [default to nothing]
 **run_id** | **Int64** |  | [default to nothing]
 **attempt_id** | **Int64** |  | [default to nothing]
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]

### Return type

[**ListSlurmStatsResponse**](ListSlurmStatsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

