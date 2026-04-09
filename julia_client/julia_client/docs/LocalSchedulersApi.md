# LocalSchedulersApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_local_scheduler**](LocalSchedulersApi.md#create_local_scheduler) | **POST** /local_schedulers |
[**delete_local_scheduler**](LocalSchedulersApi.md#delete_local_scheduler) | **DELETE** /local_schedulers/{id} |
[**delete_local_schedulers**](LocalSchedulersApi.md#delete_local_schedulers) | **DELETE** /local_schedulers |
[**get_local_scheduler**](LocalSchedulersApi.md#get_local_scheduler) | **GET** /local_schedulers/{id} |
[**list_local_schedulers**](LocalSchedulersApi.md#list_local_schedulers) | **GET** /local_schedulers |
[**update_local_scheduler**](LocalSchedulersApi.md#update_local_scheduler) | **PUT** /local_schedulers/{id} |


# **create_local_scheduler**
> create_local_scheduler(_api::LocalSchedulersApi, local_scheduler_model::LocalSchedulerModel; _mediaType=nothing) -> LocalSchedulerModel, OpenAPI.Clients.ApiResponse <br/>
> create_local_scheduler(_api::LocalSchedulersApi, response_stream::Channel, local_scheduler_model::LocalSchedulerModel; _mediaType=nothing) -> Channel{ LocalSchedulerModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **LocalSchedulersApi** | API context |
**local_scheduler_model** | [**LocalSchedulerModel**](LocalSchedulerModel.md) |  |

### Return type

[**LocalSchedulerModel**](LocalSchedulerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_local_scheduler**
> delete_local_scheduler(_api::LocalSchedulersApi, id::Int64; _mediaType=nothing) -> LocalSchedulerModel, OpenAPI.Clients.ApiResponse <br/>
> delete_local_scheduler(_api::LocalSchedulersApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ LocalSchedulerModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **LocalSchedulersApi** | API context |
**id** | **Int64** | Local scheduler ID |

### Return type

[**LocalSchedulerModel**](LocalSchedulerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_local_schedulers**
> delete_local_schedulers(_api::LocalSchedulersApi, workflow_id::Int64; _mediaType=nothing) -> DeleteCountResponse, OpenAPI.Clients.ApiResponse <br/>
> delete_local_schedulers(_api::LocalSchedulersApi, response_stream::Channel, workflow_id::Int64; _mediaType=nothing) -> Channel{ DeleteCountResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **LocalSchedulersApi** | API context |
**workflow_id** | **Int64** |  |

### Return type

[**DeleteCountResponse**](DeleteCountResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_local_scheduler**
> get_local_scheduler(_api::LocalSchedulersApi, id::Int64; _mediaType=nothing) -> LocalSchedulerModel, OpenAPI.Clients.ApiResponse <br/>
> get_local_scheduler(_api::LocalSchedulersApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ LocalSchedulerModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **LocalSchedulersApi** | API context |
**id** | **Int64** | ID of the local scheduler record |

### Return type

[**LocalSchedulerModel**](LocalSchedulerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_local_schedulers**
> list_local_schedulers(_api::LocalSchedulersApi, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, memory=nothing, num_cpus=nothing, _mediaType=nothing) -> ListLocalSchedulersResponse, OpenAPI.Clients.ApiResponse <br/>
> list_local_schedulers(_api::LocalSchedulersApi, response_stream::Channel, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, memory=nothing, num_cpus=nothing, _mediaType=nothing) -> Channel{ ListLocalSchedulersResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **LocalSchedulersApi** | API context |
**workflow_id** | **Int64** |  |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to nothing]
 **memory** | **String** |  | [default to nothing]
 **num_cpus** | **Int64** |  | [default to nothing]

### Return type

[**ListLocalSchedulersResponse**](ListLocalSchedulersResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_local_scheduler**
> update_local_scheduler(_api::LocalSchedulersApi, id::Int64, local_scheduler_model::LocalSchedulerModel; _mediaType=nothing) -> LocalSchedulerModel, OpenAPI.Clients.ApiResponse <br/>
> update_local_scheduler(_api::LocalSchedulersApi, response_stream::Channel, id::Int64, local_scheduler_model::LocalSchedulerModel; _mediaType=nothing) -> Channel{ LocalSchedulerModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **LocalSchedulersApi** | API context |
**id** | **Int64** | ID of the local scheduler. |
**local_scheduler_model** | [**LocalSchedulerModel**](LocalSchedulerModel.md) |  |

### Return type

[**LocalSchedulerModel**](LocalSchedulerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)
