# TasksApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**get_task**](TasksApi.md#get_task) | **GET** /tasks/{id} | 


# **get_task**
> get_task(_api::TasksApi, id::Int64; _mediaType=nothing) -> TaskModel, OpenAPI.Clients.ApiResponse <br/>
> get_task(_api::TasksApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ TaskModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **TasksApi** | API context | 
**id** | **Int64** | Task ID |

### Return type

[**TaskModel**](TaskModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

