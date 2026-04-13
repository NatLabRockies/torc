# ScheduledComputeNodesApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_scheduled_compute_node**](ScheduledComputeNodesApi.md#create_scheduled_compute_node) | **POST** /scheduled_compute_nodes | 
[**delete_scheduled_compute_node**](ScheduledComputeNodesApi.md#delete_scheduled_compute_node) | **DELETE** /scheduled_compute_nodes/{id} | 
[**delete_scheduled_compute_nodes**](ScheduledComputeNodesApi.md#delete_scheduled_compute_nodes) | **DELETE** /scheduled_compute_nodes | 
[**get_scheduled_compute_node**](ScheduledComputeNodesApi.md#get_scheduled_compute_node) | **GET** /scheduled_compute_nodes/{id} | 
[**list_scheduled_compute_nodes**](ScheduledComputeNodesApi.md#list_scheduled_compute_nodes) | **GET** /scheduled_compute_nodes | 
[**update_scheduled_compute_node**](ScheduledComputeNodesApi.md#update_scheduled_compute_node) | **PUT** /scheduled_compute_nodes/{id} | 


# **create_scheduled_compute_node**
> create_scheduled_compute_node(_api::ScheduledComputeNodesApi, scheduled_compute_nodes_model::ScheduledComputeNodesModel; _mediaType=nothing) -> ScheduledComputeNodesModel, OpenAPI.Clients.ApiResponse <br/>
> create_scheduled_compute_node(_api::ScheduledComputeNodesApi, response_stream::Channel, scheduled_compute_nodes_model::ScheduledComputeNodesModel; _mediaType=nothing) -> Channel{ ScheduledComputeNodesModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ScheduledComputeNodesApi** | API context | 
**scheduled_compute_nodes_model** | [**ScheduledComputeNodesModel**](ScheduledComputeNodesModel.md) |  |

### Return type

[**ScheduledComputeNodesModel**](ScheduledComputeNodesModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_scheduled_compute_node**
> delete_scheduled_compute_node(_api::ScheduledComputeNodesApi, id::Int64; _mediaType=nothing) -> ScheduledComputeNodesModel, OpenAPI.Clients.ApiResponse <br/>
> delete_scheduled_compute_node(_api::ScheduledComputeNodesApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ScheduledComputeNodesModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ScheduledComputeNodesApi** | API context | 
**id** | **Int64** | Scheduled compute node ID |

### Return type

[**ScheduledComputeNodesModel**](ScheduledComputeNodesModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_scheduled_compute_nodes**
> delete_scheduled_compute_nodes(_api::ScheduledComputeNodesApi, workflow_id::Int64; _mediaType=nothing) -> DeleteCountResponse, OpenAPI.Clients.ApiResponse <br/>
> delete_scheduled_compute_nodes(_api::ScheduledComputeNodesApi, response_stream::Channel, workflow_id::Int64; _mediaType=nothing) -> Channel{ DeleteCountResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ScheduledComputeNodesApi** | API context | 
**workflow_id** | **Int64** |  |

### Return type

[**DeleteCountResponse**](DeleteCountResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_scheduled_compute_node**
> get_scheduled_compute_node(_api::ScheduledComputeNodesApi, id::Int64; _mediaType=nothing) -> ScheduledComputeNodesModel, OpenAPI.Clients.ApiResponse <br/>
> get_scheduled_compute_node(_api::ScheduledComputeNodesApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ScheduledComputeNodesModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ScheduledComputeNodesApi** | API context | 
**id** | **Int64** | ID of the scheduled_compute_nodes record |

### Return type

[**ScheduledComputeNodesModel**](ScheduledComputeNodesModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_scheduled_compute_nodes**
> list_scheduled_compute_nodes(_api::ScheduledComputeNodesApi, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, scheduler_id=nothing, scheduler_config_id=nothing, status=nothing, _mediaType=nothing) -> ListScheduledComputeNodesResponse, OpenAPI.Clients.ApiResponse <br/>
> list_scheduled_compute_nodes(_api::ScheduledComputeNodesApi, response_stream::Channel, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, scheduler_id=nothing, scheduler_config_id=nothing, status=nothing, _mediaType=nothing) -> Channel{ ListScheduledComputeNodesResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ScheduledComputeNodesApi** | API context | 
**workflow_id** | **Int64** |  |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to nothing]
 **scheduler_id** | **String** |  | [default to nothing]
 **scheduler_config_id** | **String** |  | [default to nothing]
 **status** | **String** |  | [default to nothing]

### Return type

[**ListScheduledComputeNodesResponse**](ListScheduledComputeNodesResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_scheduled_compute_node**
> update_scheduled_compute_node(_api::ScheduledComputeNodesApi, id::Int64, scheduled_compute_nodes_model::ScheduledComputeNodesModel; _mediaType=nothing) -> ScheduledComputeNodesModel, OpenAPI.Clients.ApiResponse <br/>
> update_scheduled_compute_node(_api::ScheduledComputeNodesApi, response_stream::Channel, id::Int64, scheduled_compute_nodes_model::ScheduledComputeNodesModel; _mediaType=nothing) -> Channel{ ScheduledComputeNodesModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ScheduledComputeNodesApi** | API context | 
**id** | **Int64** | Scheduled compute node ID |
**scheduled_compute_nodes_model** | [**ScheduledComputeNodesModel**](ScheduledComputeNodesModel.md) |  |

### Return type

[**ScheduledComputeNodesModel**](ScheduledComputeNodesModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

