# FailureHandlersApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_failure_handler**](FailureHandlersApi.md#create_failure_handler) | **POST** /failure_handlers | 
[**delete_failure_handler**](FailureHandlersApi.md#delete_failure_handler) | **DELETE** /failure_handlers/{id} | 
[**get_failure_handler**](FailureHandlersApi.md#get_failure_handler) | **GET** /failure_handlers/{id} | 
[**list_failure_handlers**](FailureHandlersApi.md#list_failure_handlers) | **GET** /workflows/{id}/failure_handlers | 


# **create_failure_handler**
> create_failure_handler(_api::FailureHandlersApi, failure_handler_model::FailureHandlerModel; _mediaType=nothing) -> FailureHandlerModel, OpenAPI.Clients.ApiResponse <br/>
> create_failure_handler(_api::FailureHandlersApi, response_stream::Channel, failure_handler_model::FailureHandlerModel; _mediaType=nothing) -> Channel{ FailureHandlerModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FailureHandlersApi** | API context | 
**failure_handler_model** | [**FailureHandlerModel**](FailureHandlerModel.md) |  |

### Return type

[**FailureHandlerModel**](FailureHandlerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_failure_handler**
> delete_failure_handler(_api::FailureHandlersApi, id::Int64; _mediaType=nothing) -> FailureHandlerModel, OpenAPI.Clients.ApiResponse <br/>
> delete_failure_handler(_api::FailureHandlersApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ FailureHandlerModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FailureHandlersApi** | API context | 
**id** | **Int64** | Failure handler ID |

### Return type

[**FailureHandlerModel**](FailureHandlerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_failure_handler**
> get_failure_handler(_api::FailureHandlersApi, id::Int64; _mediaType=nothing) -> FailureHandlerModel, OpenAPI.Clients.ApiResponse <br/>
> get_failure_handler(_api::FailureHandlersApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ FailureHandlerModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FailureHandlersApi** | API context | 
**id** | **Int64** | Failure handler ID |

### Return type

[**FailureHandlerModel**](FailureHandlerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_failure_handlers**
> list_failure_handlers(_api::FailureHandlersApi, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> ListFailureHandlersResponse, OpenAPI.Clients.ApiResponse <br/>
> list_failure_handlers(_api::FailureHandlersApi, response_stream::Channel, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> Channel{ ListFailureHandlersResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FailureHandlersApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]

### Return type

[**ListFailureHandlersResponse**](ListFailureHandlersResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

