# RemoteWorkersApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_remote_workers**](RemoteWorkersApi.md#create_remote_workers) | **POST** /workflows/{id}/remote_workers |
[**delete_remote_worker**](RemoteWorkersApi.md#delete_remote_worker) | **DELETE** /workflows/{id}/remote_workers/{worker} |
[**list_remote_workers**](RemoteWorkersApi.md#list_remote_workers) | **GET** /workflows/{id}/remote_workers |


# **create_remote_workers**
> create_remote_workers(_api::RemoteWorkersApi, id::Int64, request_body::Vector{String}; _mediaType=nothing) -> Vector{RemoteWorkerModel}, OpenAPI.Clients.ApiResponse <br/>
> create_remote_workers(_api::RemoteWorkersApi, response_stream::Channel, id::Int64, request_body::Vector{String}; _mediaType=nothing) -> Channel{ Vector{RemoteWorkerModel} }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **RemoteWorkersApi** | API context |
**id** | **Int64** | Workflow ID |
**request_body** | [**Vector{String}**](String.md) |  |

### Return type

[**Vector{RemoteWorkerModel}**](RemoteWorkerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_remote_worker**
> delete_remote_worker(_api::RemoteWorkersApi, id::Int64, worker::String; _mediaType=nothing) -> RemoteWorkerModel, OpenAPI.Clients.ApiResponse <br/>
> delete_remote_worker(_api::RemoteWorkersApi, response_stream::Channel, id::Int64, worker::String; _mediaType=nothing) -> Channel{ RemoteWorkerModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **RemoteWorkersApi** | API context |
**id** | **Int64** | Workflow ID |
**worker** | **String** | Worker address |

### Return type

[**RemoteWorkerModel**](RemoteWorkerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_remote_workers**
> list_remote_workers(_api::RemoteWorkersApi, id::Int64; _mediaType=nothing) -> Vector{RemoteWorkerModel}, OpenAPI.Clients.ApiResponse <br/>
> list_remote_workers(_api::RemoteWorkersApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ Vector{RemoteWorkerModel} }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **RemoteWorkersApi** | API context |
**id** | **Int64** | Workflow ID |

### Return type

[**Vector{RemoteWorkerModel}**](RemoteWorkerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)
