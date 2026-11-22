# WorkflowActionsApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**claim_action**](WorkflowActionsApi.md#claim_action) | **POST** /workflows/{id}/actions/{action_id}/claim | 
[**create_workflow_action**](WorkflowActionsApi.md#create_workflow_action) | **POST** /workflows/{id}/actions | 
[**delete_workflow_action**](WorkflowActionsApi.md#delete_workflow_action) | **DELETE** /workflows/{id}/actions/{action_id} | 
[**get_pending_actions**](WorkflowActionsApi.md#get_pending_actions) | **GET** /workflows/{id}/actions/pending | 
[**get_workflow_actions**](WorkflowActionsApi.md#get_workflow_actions) | **GET** /workflows/{id}/actions | 
[**report_action_result**](WorkflowActionsApi.md#report_action_result) | **POST** /workflows/{id}/actions/{action_id}/result | 
[**reset_action**](WorkflowActionsApi.md#reset_action) | **POST** /workflows/{id}/actions/{action_id}/reset | 
[**suppress_action**](WorkflowActionsApi.md#suppress_action) | **POST** /workflows/{id}/actions/{action_id}/suppress | 
[**update_workflow_action**](WorkflowActionsApi.md#update_workflow_action) | **PUT** /workflows/{id}/actions/{action_id} | 


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

# **delete_workflow_action**
> delete_workflow_action(_api::WorkflowActionsApi, id::Int64, action_id::Int64; _mediaType=nothing) -> MessageResponse, OpenAPI.Clients.ApiResponse <br/>
> delete_workflow_action(_api::WorkflowActionsApi, response_stream::Channel, id::Int64, action_id::Int64; _mediaType=nothing) -> Channel{ MessageResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowActionsApi** | API context | 
**id** | **Int64** | Workflow ID |
**action_id** | **Int64** | Action ID |

### Return type

[**MessageResponse**](MessageResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
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

# **report_action_result**
> report_action_result(_api::WorkflowActionsApi, id::Int64, action_id::Int64, workflow_action_result_request::WorkflowActionResultRequest; _mediaType=nothing) -> WorkflowActionModel, OpenAPI.Clients.ApiResponse <br/>
> report_action_result(_api::WorkflowActionsApi, response_stream::Channel, id::Int64, action_id::Int64, workflow_action_result_request::WorkflowActionResultRequest; _mediaType=nothing) -> Channel{ WorkflowActionModel }, OpenAPI.Clients.ApiResponse



Record the outcome of a claimed action: state becomes succeeded, or failed with the given error. For a persistent action this is the latest outcome. 409 if the action has not been claimed (or was suppressed); 422 if error is set with success = true.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowActionsApi** | API context | 
**id** | **Int64** | Workflow ID |
**action_id** | **Int64** | Action ID |
**workflow_action_result_request** | [**WorkflowActionResultRequest**](WorkflowActionResultRequest.md) |  |

### Return type

[**WorkflowActionModel**](WorkflowActionModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **reset_action**
> reset_action(_api::WorkflowActionsApi, id::Int64, action_id::Int64; _mediaType=nothing) -> WorkflowActionModel, OpenAPI.Clients.ApiResponse <br/>
> reset_action(_api::WorkflowActionsApi, response_stream::Channel, id::Int64, action_id::Int64; _mediaType=nothing) -> Channel{ WorkflowActionModel }, OpenAPI.Clients.ApiResponse



Re-arm an action that is failed, suppressed, or stuck in claimed: clears executed, executed_at, executed_by, state, and error so it can be claimed again. A no-op for an action that was never claimed; 409 for a succeeded action.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowActionsApi** | API context | 
**id** | **Int64** | Workflow ID |
**action_id** | **Int64** | Action ID |

### Return type

[**WorkflowActionModel**](WorkflowActionModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **suppress_action**
> suppress_action(_api::WorkflowActionsApi, id::Int64, action_id::Int64; _mediaType=nothing) -> WorkflowActionModel, OpenAPI.Clients.ApiResponse <br/>
> suppress_action(_api::WorkflowActionsApi, response_stream::Channel, id::Int64, action_id::Int64; _mediaType=nothing) -> Channel{ WorkflowActionModel }, OpenAPI.Clients.ApiResponse



Mark an action suppressed (executed = true, state = suppressed) so it is never claimed, without executing it. Idempotent. A persistent action can be suppressed in any state; a non-persistent action only while unclaimed (409 if it is claimed, succeeded, or failed).

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowActionsApi** | API context | 
**id** | **Int64** | Workflow ID |
**action_id** | **Int64** | Action ID |

### Return type

[**WorkflowActionModel**](WorkflowActionModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_workflow_action**
> update_workflow_action(_api::WorkflowActionsApi, id::Int64, action_id::Int64, body::Any; _mediaType=nothing) -> WorkflowActionModel, OpenAPI.Clients.ApiResponse <br/>
> update_workflow_action(_api::WorkflowActionsApi, response_stream::Channel, id::Int64, action_id::Int64, body::Any; _mediaType=nothing) -> Channel{ WorkflowActionModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowActionsApi** | API context | 
**id** | **Int64** | Workflow ID |
**action_id** | **Int64** | Action ID |
**body** | **Any** |  |

### Return type

[**WorkflowActionModel**](WorkflowActionModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

