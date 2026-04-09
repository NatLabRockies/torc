# SlurmSchedulersApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_slurm_scheduler**](SlurmSchedulersApi.md#create_slurm_scheduler) | **POST** /slurm_schedulers |
[**delete_slurm_scheduler**](SlurmSchedulersApi.md#delete_slurm_scheduler) | **DELETE** /slurm_schedulers/{id} |
[**delete_slurm_schedulers**](SlurmSchedulersApi.md#delete_slurm_schedulers) | **DELETE** /slurm_schedulers |
[**get_slurm_scheduler**](SlurmSchedulersApi.md#get_slurm_scheduler) | **GET** /slurm_schedulers/{id} |
[**list_slurm_schedulers**](SlurmSchedulersApi.md#list_slurm_schedulers) | **GET** /slurm_schedulers |
[**update_slurm_scheduler**](SlurmSchedulersApi.md#update_slurm_scheduler) | **PUT** /slurm_schedulers/{id} |


# **create_slurm_scheduler**
> create_slurm_scheduler(_api::SlurmSchedulersApi, slurm_scheduler_model::SlurmSchedulerModel; _mediaType=nothing) -> SlurmSchedulerModel, OpenAPI.Clients.ApiResponse <br/>
> create_slurm_scheduler(_api::SlurmSchedulersApi, response_stream::Channel, slurm_scheduler_model::SlurmSchedulerModel; _mediaType=nothing) -> Channel{ SlurmSchedulerModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **SlurmSchedulersApi** | API context |
**slurm_scheduler_model** | [**SlurmSchedulerModel**](SlurmSchedulerModel.md) |  |

### Return type

[**SlurmSchedulerModel**](SlurmSchedulerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_slurm_scheduler**
> delete_slurm_scheduler(_api::SlurmSchedulersApi, id::Int64; _mediaType=nothing) -> SlurmSchedulerModel, OpenAPI.Clients.ApiResponse <br/>
> delete_slurm_scheduler(_api::SlurmSchedulersApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ SlurmSchedulerModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **SlurmSchedulersApi** | API context |
**id** | **Int64** | Slurm compute node configuration ID |

### Return type

[**SlurmSchedulerModel**](SlurmSchedulerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_slurm_schedulers**
> delete_slurm_schedulers(_api::SlurmSchedulersApi, workflow_id::Int64; _mediaType=nothing) -> DeleteCountResponse, OpenAPI.Clients.ApiResponse <br/>
> delete_slurm_schedulers(_api::SlurmSchedulersApi, response_stream::Channel, workflow_id::Int64; _mediaType=nothing) -> Channel{ DeleteCountResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **SlurmSchedulersApi** | API context |
**workflow_id** | **Int64** |  |

### Return type

[**DeleteCountResponse**](DeleteCountResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_slurm_scheduler**
> get_slurm_scheduler(_api::SlurmSchedulersApi, id::Int64; _mediaType=nothing) -> SlurmSchedulerModel, OpenAPI.Clients.ApiResponse <br/>
> get_slurm_scheduler(_api::SlurmSchedulersApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ SlurmSchedulerModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **SlurmSchedulersApi** | API context |
**id** | **Int64** | Slurm compute node configuration ID |

### Return type

[**SlurmSchedulerModel**](SlurmSchedulerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_slurm_schedulers**
> list_slurm_schedulers(_api::SlurmSchedulersApi, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, _mediaType=nothing) -> ListSlurmSchedulersResponse, OpenAPI.Clients.ApiResponse <br/>
> list_slurm_schedulers(_api::SlurmSchedulersApi, response_stream::Channel, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, _mediaType=nothing) -> Channel{ ListSlurmSchedulersResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **SlurmSchedulersApi** | API context |
**workflow_id** | **Int64** |  |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to nothing]

### Return type

[**ListSlurmSchedulersResponse**](ListSlurmSchedulersResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_slurm_scheduler**
> update_slurm_scheduler(_api::SlurmSchedulersApi, id::Int64, slurm_scheduler_model::SlurmSchedulerModel; _mediaType=nothing) -> SlurmSchedulerModel, OpenAPI.Clients.ApiResponse <br/>
> update_slurm_scheduler(_api::SlurmSchedulersApi, response_stream::Channel, id::Int64, slurm_scheduler_model::SlurmSchedulerModel; _mediaType=nothing) -> Channel{ SlurmSchedulerModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **SlurmSchedulersApi** | API context |
**id** | **Int64** | Slurm compute node configuration ID |
**slurm_scheduler_model** | [**SlurmSchedulerModel**](SlurmSchedulerModel.md) |  |

### Return type

[**SlurmSchedulerModel**](SlurmSchedulerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)
