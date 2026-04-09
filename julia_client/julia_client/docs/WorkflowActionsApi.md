# WorkflowActionsApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**claim_action**](WorkflowActionsApi.md#claim_action) | **POST** /workflows/{id}/actions/{action_id}/claim |
[**create_workflow_action**](WorkflowActionsApi.md#create_workflow_action) | **POST** /workflows/{id}/actions |
[**get_pending_actions**](WorkflowActionsApi.md#get_pending_actions) | **GET** /workflows/{id}/actions/pending |
[**get_workflow_actions**](WorkflowActionsApi.md#get_workflow_actions) | **GET** /workflows/{id}/actions |


# **claim_action**
> claim_action(_api::WorkflowActionsApi, id::Int64, action_id::Int64, claim_action_request::ClaimActionRequest; _mediaType=nothing) -> ClaimActionResponse, OpenAPI.Clients.ApiResponse <br/>
> claim_action(_api::WorkflowActionsApi, response_stream::Channel, id::Int64, action_id::Int64, claim_action_request::ClaimActionRequest; _mediaType=nothing) -> Channel{ ClaimActionResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowActionsApi** | API context |
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

# **create_workflow_action**
> create_workflow_action(_api::WorkflowActionsApi, id::Int64, workflow_action_model::WorkflowActionModel; _mediaType=nothing) -> WorkflowActionModel, OpenAPI.Clients.ApiResponse <br/>
> create_workflow_action(_api::WorkflowActionsApi, response_stream::Channel, id::Int64, workflow_action_model::WorkflowActionModel; _mediaType=nothing) -> Channel{ WorkflowActionModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowActionsApi** | API context |
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

# **get_pending_actions**
> get_pending_actions(_api::WorkflowActionsApi, id::Int64; trigger_type=nothing, _mediaType=nothing) -> Vector{WorkflowActionModel}, OpenAPI.Clients.ApiResponse <br/>
> get_pending_actions(_api::WorkflowActionsApi, response_stream::Channel, id::Int64; trigger_type=nothing, _mediaType=nothing) -> Channel{ Vector{WorkflowActionModel} }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowActionsApi** | API context |
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

# **get_workflow_actions**
> get_workflow_actions(_api::WorkflowActionsApi, id::Int64; _mediaType=nothing) -> Vector{WorkflowActionModel}, OpenAPI.Clients.ApiResponse <br/>
> get_workflow_actions(_api::WorkflowActionsApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ Vector{WorkflowActionModel} }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowActionsApi** | API context |
**id** | **Int64** | Workflow ID |

### Return type

[**Vector{WorkflowActionModel}**](WorkflowActionModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)
