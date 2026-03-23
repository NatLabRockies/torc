# FinalSurfacesApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**claim_action**](FinalSurfacesApi.md#claim_action) | **POST** /workflows/{id}/actions/{action_id}/claim | 
[**create_remote_workers**](FinalSurfacesApi.md#create_remote_workers) | **POST** /workflows/{id}/remote_workers | 
[**create_ro_crate_entity**](FinalSurfacesApi.md#create_ro_crate_entity) | **POST** /ro_crate_entities | 
[**create_workflow_action**](FinalSurfacesApi.md#create_workflow_action) | **POST** /workflows/{id}/actions | 
[**delete_remote_worker**](FinalSurfacesApi.md#delete_remote_worker) | **DELETE** /workflows/{id}/remote_workers/{worker} | 
[**delete_ro_crate_entities**](FinalSurfacesApi.md#delete_ro_crate_entities) | **DELETE** /workflows/{id}/ro_crate_entities | 
[**delete_ro_crate_entity**](FinalSurfacesApi.md#delete_ro_crate_entity) | **DELETE** /ro_crate_entities/{id} | 
[**get_pending_actions**](FinalSurfacesApi.md#get_pending_actions) | **GET** /workflows/{id}/actions/pending | 
[**get_ro_crate_entity**](FinalSurfacesApi.md#get_ro_crate_entity) | **GET** /ro_crate_entities/{id} | 
[**get_workflow_actions**](FinalSurfacesApi.md#get_workflow_actions) | **GET** /workflows/{id}/actions | 
[**list_remote_workers**](FinalSurfacesApi.md#list_remote_workers) | **GET** /workflows/{id}/remote_workers | 
[**list_ro_crate_entities**](FinalSurfacesApi.md#list_ro_crate_entities) | **GET** /workflows/{id}/ro_crate_entities | 
[**reload_auth**](FinalSurfacesApi.md#reload_auth) | **POST** /admin/reload-auth | 
[**update_ro_crate_entity**](FinalSurfacesApi.md#update_ro_crate_entity) | **PUT** /ro_crate_entities/{id} | 


# **claim_action**
> claim_action(_api::FinalSurfacesApi, id::Int64, action_id::Int64, claim_action_request::ClaimActionRequest; _mediaType=nothing) -> ClaimActionResponse, OpenAPI.Clients.ApiResponse <br/>
> claim_action(_api::FinalSurfacesApi, response_stream::Channel, id::Int64, action_id::Int64, claim_action_request::ClaimActionRequest; _mediaType=nothing) -> Channel{ ClaimActionResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FinalSurfacesApi** | API context | 
**id** | **Int64** | Workflow ID |
**action_id** | **Int64** | Action ID |
**claim_action_request** | [**ClaimActionRequest**](ClaimActionRequest.md) |  |

### Return type

[**ClaimActionResponse**](ClaimActionResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_remote_workers**
> create_remote_workers(_api::FinalSurfacesApi, id::Int64, request_body::Vector{String}; _mediaType=nothing) -> Vector{RemoteWorkerModel}, OpenAPI.Clients.ApiResponse <br/>
> create_remote_workers(_api::FinalSurfacesApi, response_stream::Channel, id::Int64, request_body::Vector{String}; _mediaType=nothing) -> Channel{ Vector{RemoteWorkerModel} }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FinalSurfacesApi** | API context | 
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

# **create_ro_crate_entity**
> create_ro_crate_entity(_api::FinalSurfacesApi, ro_crate_entity_model::RoCrateEntityModel; _mediaType=nothing) -> RoCrateEntityModel, OpenAPI.Clients.ApiResponse <br/>
> create_ro_crate_entity(_api::FinalSurfacesApi, response_stream::Channel, ro_crate_entity_model::RoCrateEntityModel; _mediaType=nothing) -> Channel{ RoCrateEntityModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FinalSurfacesApi** | API context | 
**ro_crate_entity_model** | [**RoCrateEntityModel**](RoCrateEntityModel.md) |  |

### Return type

[**RoCrateEntityModel**](RoCrateEntityModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_workflow_action**
> create_workflow_action(_api::FinalSurfacesApi, id::Int64, workflow_action_model::WorkflowActionModel; _mediaType=nothing) -> WorkflowActionModel, OpenAPI.Clients.ApiResponse <br/>
> create_workflow_action(_api::FinalSurfacesApi, response_stream::Channel, id::Int64, workflow_action_model::WorkflowActionModel; _mediaType=nothing) -> Channel{ WorkflowActionModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FinalSurfacesApi** | API context | 
**id** | **Int64** | Workflow ID |
**workflow_action_model** | [**WorkflowActionModel**](WorkflowActionModel.md) |  |

### Return type

[**WorkflowActionModel**](WorkflowActionModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_remote_worker**
> delete_remote_worker(_api::FinalSurfacesApi, id::Int64, worker::String; _mediaType=nothing) -> RemoteWorkerModel, OpenAPI.Clients.ApiResponse <br/>
> delete_remote_worker(_api::FinalSurfacesApi, response_stream::Channel, id::Int64, worker::String; _mediaType=nothing) -> Channel{ RemoteWorkerModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FinalSurfacesApi** | API context | 
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

# **delete_ro_crate_entities**
> delete_ro_crate_entities(_api::FinalSurfacesApi, id::Int64; body=nothing, _mediaType=nothing) -> DeleteRoCrateEntitiesResponse, OpenAPI.Clients.ApiResponse <br/>
> delete_ro_crate_entities(_api::FinalSurfacesApi, response_stream::Channel, id::Int64; body=nothing, _mediaType=nothing) -> Channel{ DeleteRoCrateEntitiesResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FinalSurfacesApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

[**DeleteRoCrateEntitiesResponse**](DeleteRoCrateEntitiesResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_ro_crate_entity**
> delete_ro_crate_entity(_api::FinalSurfacesApi, id::Int64; _mediaType=nothing) -> MessageResponse, OpenAPI.Clients.ApiResponse <br/>
> delete_ro_crate_entity(_api::FinalSurfacesApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ MessageResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FinalSurfacesApi** | API context | 
**id** | **Int64** | Entity ID |

### Return type

[**MessageResponse**](MessageResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_pending_actions**
> get_pending_actions(_api::FinalSurfacesApi, id::Int64; trigger_type=nothing, _mediaType=nothing) -> Vector{WorkflowActionModel}, OpenAPI.Clients.ApiResponse <br/>
> get_pending_actions(_api::FinalSurfacesApi, response_stream::Channel, id::Int64; trigger_type=nothing, _mediaType=nothing) -> Channel{ Vector{WorkflowActionModel} }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FinalSurfacesApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **trigger_type** | [**Vector{String}**](String.md) |  | [default to nothing]

### Return type

[**Vector{WorkflowActionModel}**](WorkflowActionModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_ro_crate_entity**
> get_ro_crate_entity(_api::FinalSurfacesApi, id::Int64; _mediaType=nothing) -> RoCrateEntityModel, OpenAPI.Clients.ApiResponse <br/>
> get_ro_crate_entity(_api::FinalSurfacesApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ RoCrateEntityModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FinalSurfacesApi** | API context | 
**id** | **Int64** | Entity ID |

### Return type

[**RoCrateEntityModel**](RoCrateEntityModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_workflow_actions**
> get_workflow_actions(_api::FinalSurfacesApi, id::Int64; _mediaType=nothing) -> Vector{WorkflowActionModel}, OpenAPI.Clients.ApiResponse <br/>
> get_workflow_actions(_api::FinalSurfacesApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ Vector{WorkflowActionModel} }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FinalSurfacesApi** | API context | 
**id** | **Int64** | Workflow ID |

### Return type

[**Vector{WorkflowActionModel}**](WorkflowActionModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_remote_workers**
> list_remote_workers(_api::FinalSurfacesApi, id::Int64; _mediaType=nothing) -> Vector{RemoteWorkerModel}, OpenAPI.Clients.ApiResponse <br/>
> list_remote_workers(_api::FinalSurfacesApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ Vector{RemoteWorkerModel} }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FinalSurfacesApi** | API context | 
**id** | **Int64** | Workflow ID |

### Return type

[**Vector{RemoteWorkerModel}**](RemoteWorkerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_ro_crate_entities**
> list_ro_crate_entities(_api::FinalSurfacesApi, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> ListRoCrateEntitiesResponse, OpenAPI.Clients.ApiResponse <br/>
> list_ro_crate_entities(_api::FinalSurfacesApi, response_stream::Channel, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> Channel{ ListRoCrateEntitiesResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FinalSurfacesApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]

### Return type

[**ListRoCrateEntitiesResponse**](ListRoCrateEntitiesResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **reload_auth**
> reload_auth(_api::FinalSurfacesApi; _mediaType=nothing) -> ReloadAuthResponse, OpenAPI.Clients.ApiResponse <br/>
> reload_auth(_api::FinalSurfacesApi, response_stream::Channel; _mediaType=nothing) -> Channel{ ReloadAuthResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters
This endpoint does not need any parameter.

### Return type

[**ReloadAuthResponse**](ReloadAuthResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_ro_crate_entity**
> update_ro_crate_entity(_api::FinalSurfacesApi, id::Int64, ro_crate_entity_model::RoCrateEntityModel; _mediaType=nothing) -> RoCrateEntityModel, OpenAPI.Clients.ApiResponse <br/>
> update_ro_crate_entity(_api::FinalSurfacesApi, response_stream::Channel, id::Int64, ro_crate_entity_model::RoCrateEntityModel; _mediaType=nothing) -> Channel{ RoCrateEntityModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FinalSurfacesApi** | API context | 
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

