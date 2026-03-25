# ResourceRequirementsApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_resource_requirements**](ResourceRequirementsApi.md#create_resource_requirements) | **POST** /resource_requirements | 
[**delete_resource_requirement**](ResourceRequirementsApi.md#delete_resource_requirement) | **DELETE** /resource_requirements/{id} | 
[**delete_resource_requirements**](ResourceRequirementsApi.md#delete_resource_requirements) | **DELETE** /resource_requirements | 
[**get_resource_requirements**](ResourceRequirementsApi.md#get_resource_requirements) | **GET** /resource_requirements/{id} | 
[**list_resource_requirements**](ResourceRequirementsApi.md#list_resource_requirements) | **GET** /resource_requirements | 
[**update_resource_requirements**](ResourceRequirementsApi.md#update_resource_requirements) | **PUT** /resource_requirements/{id} | 


# **create_resource_requirements**
> create_resource_requirements(_api::ResourceRequirementsApi, resource_requirements_model::ResourceRequirementsModel; _mediaType=nothing) -> ResourceRequirementsModel, OpenAPI.Clients.ApiResponse <br/>
> create_resource_requirements(_api::ResourceRequirementsApi, response_stream::Channel, resource_requirements_model::ResourceRequirementsModel; _mediaType=nothing) -> Channel{ ResourceRequirementsModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ResourceRequirementsApi** | API context | 
**resource_requirements_model** | [**ResourceRequirementsModel**](ResourceRequirementsModel.md) |  |

### Return type

[**ResourceRequirementsModel**](ResourceRequirementsModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_resource_requirement**
> delete_resource_requirement(_api::ResourceRequirementsApi, id::Int64; _mediaType=nothing) -> ResourceRequirementsModel, OpenAPI.Clients.ApiResponse <br/>
> delete_resource_requirement(_api::ResourceRequirementsApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ResourceRequirementsModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ResourceRequirementsApi** | API context | 
**id** | **Int64** | Resource requirements ID |

### Return type

[**ResourceRequirementsModel**](ResourceRequirementsModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_resource_requirements**
> delete_resource_requirements(_api::ResourceRequirementsApi, workflow_id::Int64; _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> delete_resource_requirements(_api::ResourceRequirementsApi, response_stream::Channel, workflow_id::Int64; _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ResourceRequirementsApi** | API context | 
**workflow_id** | **Int64** |  |

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_resource_requirements**
> get_resource_requirements(_api::ResourceRequirementsApi, id::Int64; _mediaType=nothing) -> ResourceRequirementsModel, OpenAPI.Clients.ApiResponse <br/>
> get_resource_requirements(_api::ResourceRequirementsApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ResourceRequirementsModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ResourceRequirementsApi** | API context | 
**id** | **Int64** | Resource requirements ID |

### Return type

[**ResourceRequirementsModel**](ResourceRequirementsModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_resource_requirements**
> list_resource_requirements(_api::ResourceRequirementsApi, workflow_id::Int64; job_id=nothing, name=nothing, memory=nothing, num_cpus=nothing, num_gpus=nothing, num_nodes=nothing, runtime=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, _mediaType=nothing) -> ListResourceRequirementsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_resource_requirements(_api::ResourceRequirementsApi, response_stream::Channel, workflow_id::Int64; job_id=nothing, name=nothing, memory=nothing, num_cpus=nothing, num_gpus=nothing, num_nodes=nothing, runtime=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, _mediaType=nothing) -> Channel{ ListResourceRequirementsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ResourceRequirementsApi** | API context | 
**workflow_id** | **Int64** |  |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **job_id** | **Int64** |  | [default to nothing]
 **name** | **String** |  | [default to nothing]
 **memory** | **String** |  | [default to nothing]
 **num_cpus** | **Int64** |  | [default to nothing]
 **num_gpus** | **Int64** |  | [default to nothing]
 **num_nodes** | **Int64** |  | [default to nothing]
 **runtime** | **Int64** |  | [default to nothing]
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to nothing]

### Return type

[**ListResourceRequirementsResponse**](ListResourceRequirementsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_resource_requirements**
> update_resource_requirements(_api::ResourceRequirementsApi, id::Int64, resource_requirements_model::ResourceRequirementsModel; _mediaType=nothing) -> ResourceRequirementsModel, OpenAPI.Clients.ApiResponse <br/>
> update_resource_requirements(_api::ResourceRequirementsApi, response_stream::Channel, id::Int64, resource_requirements_model::ResourceRequirementsModel; _mediaType=nothing) -> Channel{ ResourceRequirementsModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ResourceRequirementsApi** | API context | 
**id** | **Int64** | Resource requirements ID |
**resource_requirements_model** | [**ResourceRequirementsModel**](ResourceRequirementsModel.md) |  |

### Return type

[**ResourceRequirementsModel**](ResourceRequirementsModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

