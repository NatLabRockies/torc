# RoCrateEntitiesApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_ro_crate_entity**](RoCrateEntitiesApi.md#create_ro_crate_entity) | **POST** /ro_crate_entities | 
[**delete_ro_crate_entities**](RoCrateEntitiesApi.md#delete_ro_crate_entities) | **DELETE** /workflows/{id}/ro_crate_entities | 
[**delete_ro_crate_entity**](RoCrateEntitiesApi.md#delete_ro_crate_entity) | **DELETE** /ro_crate_entities/{id} | 
[**get_ro_crate_entity**](RoCrateEntitiesApi.md#get_ro_crate_entity) | **GET** /ro_crate_entities/{id} | 
[**list_ro_crate_entities**](RoCrateEntitiesApi.md#list_ro_crate_entities) | **GET** /workflows/{id}/ro_crate_entities | 
[**update_ro_crate_entity**](RoCrateEntitiesApi.md#update_ro_crate_entity) | **PUT** /ro_crate_entities/{id} | 


# **create_ro_crate_entity**
> create_ro_crate_entity(_api::RoCrateEntitiesApi, ro_crate_entity_model::RoCrateEntityModel; _mediaType=nothing) -> RoCrateEntityModel, OpenAPI.Clients.ApiResponse <br/>
> create_ro_crate_entity(_api::RoCrateEntitiesApi, response_stream::Channel, ro_crate_entity_model::RoCrateEntityModel; _mediaType=nothing) -> Channel{ RoCrateEntityModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **RoCrateEntitiesApi** | API context | 
**ro_crate_entity_model** | [**RoCrateEntityModel**](RoCrateEntityModel.md) |  |

### Return type

[**RoCrateEntityModel**](RoCrateEntityModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_ro_crate_entities**
> delete_ro_crate_entities(_api::RoCrateEntitiesApi, id::Int64; _mediaType=nothing) -> DeleteRoCrateEntitiesResponse, OpenAPI.Clients.ApiResponse <br/>
> delete_ro_crate_entities(_api::RoCrateEntitiesApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ DeleteRoCrateEntitiesResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **RoCrateEntitiesApi** | API context | 
**id** | **Int64** | Workflow ID |

### Return type

[**DeleteRoCrateEntitiesResponse**](DeleteRoCrateEntitiesResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_ro_crate_entity**
> delete_ro_crate_entity(_api::RoCrateEntitiesApi, id::Int64; _mediaType=nothing) -> MessageResponse, OpenAPI.Clients.ApiResponse <br/>
> delete_ro_crate_entity(_api::RoCrateEntitiesApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ MessageResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **RoCrateEntitiesApi** | API context | 
**id** | **Int64** | Entity ID |

### Return type

[**MessageResponse**](MessageResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_ro_crate_entity**
> get_ro_crate_entity(_api::RoCrateEntitiesApi, id::Int64; _mediaType=nothing) -> RoCrateEntityModel, OpenAPI.Clients.ApiResponse <br/>
> get_ro_crate_entity(_api::RoCrateEntitiesApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ RoCrateEntityModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **RoCrateEntitiesApi** | API context | 
**id** | **Int64** | Entity ID |

### Return type

[**RoCrateEntityModel**](RoCrateEntityModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_ro_crate_entities**
> list_ro_crate_entities(_api::RoCrateEntitiesApi, id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, _mediaType=nothing) -> ListRoCrateEntitiesResponse, OpenAPI.Clients.ApiResponse <br/>
> list_ro_crate_entities(_api::RoCrateEntitiesApi, response_stream::Channel, id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, _mediaType=nothing) -> Channel{ ListRoCrateEntitiesResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **RoCrateEntitiesApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to nothing]

### Return type

[**ListRoCrateEntitiesResponse**](ListRoCrateEntitiesResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_ro_crate_entity**
> update_ro_crate_entity(_api::RoCrateEntitiesApi, id::Int64, ro_crate_entity_model::RoCrateEntityModel; _mediaType=nothing) -> RoCrateEntityModel, OpenAPI.Clients.ApiResponse <br/>
> update_ro_crate_entity(_api::RoCrateEntitiesApi, response_stream::Channel, id::Int64, ro_crate_entity_model::RoCrateEntityModel; _mediaType=nothing) -> Channel{ RoCrateEntityModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **RoCrateEntitiesApi** | API context | 
**id** | **Int64** | Entity ID |
**ro_crate_entity_model** | [**RoCrateEntityModel**](RoCrateEntityModel.md) |  |

### Return type

[**RoCrateEntityModel**](RoCrateEntityModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

