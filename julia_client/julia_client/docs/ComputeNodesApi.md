# ComputeNodesApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_compute_node**](ComputeNodesApi.md#create_compute_node) | **POST** /compute_nodes |
[**delete_compute_node**](ComputeNodesApi.md#delete_compute_node) | **DELETE** /compute_nodes/{id} |
[**delete_compute_nodes**](ComputeNodesApi.md#delete_compute_nodes) | **DELETE** /compute_nodes |
[**get_compute_node**](ComputeNodesApi.md#get_compute_node) | **GET** /compute_nodes/{id} |
[**list_compute_nodes**](ComputeNodesApi.md#list_compute_nodes) | **GET** /compute_nodes |
[**update_compute_node**](ComputeNodesApi.md#update_compute_node) | **PUT** /compute_nodes/{id} |


# **create_compute_node**
> create_compute_node(_api::ComputeNodesApi, compute_node_model::ComputeNodeModel; _mediaType=nothing) -> ComputeNodeModel, OpenAPI.Clients.ApiResponse <br/>
> create_compute_node(_api::ComputeNodesApi, response_stream::Channel, compute_node_model::ComputeNodeModel; _mediaType=nothing) -> Channel{ ComputeNodeModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ComputeNodesApi** | API context |
**compute_node_model** | [**ComputeNodeModel**](ComputeNodeModel.md) |  |

### Return type

[**ComputeNodeModel**](ComputeNodeModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_compute_node**
> delete_compute_node(_api::ComputeNodesApi, id::Int64; _mediaType=nothing) -> ComputeNodeModel, OpenAPI.Clients.ApiResponse <br/>
> delete_compute_node(_api::ComputeNodesApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ComputeNodeModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ComputeNodesApi** | API context |
**id** | **Int64** | Compute node ID |

### Return type

[**ComputeNodeModel**](ComputeNodeModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_compute_nodes**
> delete_compute_nodes(_api::ComputeNodesApi, workflow_id::Int64; _mediaType=nothing) -> DeleteCountResponse, OpenAPI.Clients.ApiResponse <br/>
> delete_compute_nodes(_api::ComputeNodesApi, response_stream::Channel, workflow_id::Int64; _mediaType=nothing) -> Channel{ DeleteCountResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ComputeNodesApi** | API context |
**workflow_id** | **Int64** |  |

### Return type

[**DeleteCountResponse**](DeleteCountResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_compute_node**
> get_compute_node(_api::ComputeNodesApi, id::Int64; _mediaType=nothing) -> ComputeNodeModel, OpenAPI.Clients.ApiResponse <br/>
> get_compute_node(_api::ComputeNodesApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ComputeNodeModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ComputeNodesApi** | API context |
**id** | **Int64** | ID of the compute node record |

### Return type

[**ComputeNodeModel**](ComputeNodeModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_compute_nodes**
> list_compute_nodes(_api::ComputeNodesApi, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, hostname=nothing, is_active=nothing, scheduled_compute_node_id=nothing, _mediaType=nothing) -> ListComputeNodesResponse, OpenAPI.Clients.ApiResponse <br/>
> list_compute_nodes(_api::ComputeNodesApi, response_stream::Channel, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, hostname=nothing, is_active=nothing, scheduled_compute_node_id=nothing, _mediaType=nothing) -> Channel{ ListComputeNodesResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ComputeNodesApi** | API context |
**workflow_id** | **Int64** |  |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to nothing]
 **hostname** | **String** |  | [default to nothing]
 **is_active** | **Bool** |  | [default to nothing]
 **scheduled_compute_node_id** | **Int64** |  | [default to nothing]

### Return type

[**ListComputeNodesResponse**](ListComputeNodesResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_compute_node**
> update_compute_node(_api::ComputeNodesApi, id::Int64, compute_node_model::ComputeNodeModel; _mediaType=nothing) -> ComputeNodeModel, OpenAPI.Clients.ApiResponse <br/>
> update_compute_node(_api::ComputeNodesApi, response_stream::Channel, id::Int64, compute_node_model::ComputeNodeModel; _mediaType=nothing) -> Channel{ ComputeNodeModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ComputeNodesApi** | API context |
**id** | **Int64** | ID of the compute node. |
**compute_node_model** | [**ComputeNodeModel**](ComputeNodeModel.md) |  |

### Return type

[**ComputeNodeModel**](ComputeNodeModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)
