# AccessControlApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**add_user_to_group**](AccessControlApi.md#add_user_to_group) | **POST** /access_groups/{id}/members | 
[**add_workflow_to_group**](AccessControlApi.md#add_workflow_to_group) | **POST** /workflows/{id}/access_groups/{group_id} | 
[**check_workflow_access**](AccessControlApi.md#check_workflow_access) | **GET** /access_check/{workflow_id}/{user_name} | 
[**create_access_group**](AccessControlApi.md#create_access_group) | **POST** /access_groups | 
[**delete_access_group**](AccessControlApi.md#delete_access_group) | **DELETE** /access_groups/{id} | 
[**get_access_group**](AccessControlApi.md#get_access_group) | **GET** /access_groups/{id} | 
[**list_access_groups**](AccessControlApi.md#list_access_groups) | **GET** /access_groups | 
[**list_group_members**](AccessControlApi.md#list_group_members) | **GET** /access_groups/{id}/members | 
[**list_user_groups**](AccessControlApi.md#list_user_groups) | **GET** /users/{user_name}/groups | 
[**list_workflow_groups**](AccessControlApi.md#list_workflow_groups) | **GET** /workflows/{id}/access_groups | 
[**reload_auth**](AccessControlApi.md#reload_auth) | **POST** /admin/reload-auth | 
[**remove_user_from_group**](AccessControlApi.md#remove_user_from_group) | **DELETE** /access_groups/{id}/members/{user_name} | 
[**remove_workflow_from_group**](AccessControlApi.md#remove_workflow_from_group) | **DELETE** /workflows/{id}/access_groups/{group_id} | 


# **add_user_to_group**
> add_user_to_group(_api::AccessControlApi, id::Int64, user_group_membership_model::UserGroupMembershipModel; _mediaType=nothing) -> UserGroupMembershipModel, OpenAPI.Clients.ApiResponse <br/>
> add_user_to_group(_api::AccessControlApi, response_stream::Channel, id::Int64, user_group_membership_model::UserGroupMembershipModel; _mediaType=nothing) -> Channel{ UserGroupMembershipModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AccessControlApi** | API context | 
**id** | **Int64** | Access group ID |
**user_group_membership_model** | [**UserGroupMembershipModel**](UserGroupMembershipModel.md) |  |

### Return type

[**UserGroupMembershipModel**](UserGroupMembershipModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **add_workflow_to_group**
> add_workflow_to_group(_api::AccessControlApi, id::Int64, group_id::Int64; _mediaType=nothing) -> WorkflowAccessGroupModel, OpenAPI.Clients.ApiResponse <br/>
> add_workflow_to_group(_api::AccessControlApi, response_stream::Channel, id::Int64, group_id::Int64; _mediaType=nothing) -> Channel{ WorkflowAccessGroupModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AccessControlApi** | API context | 
**id** | **Int64** | Workflow ID |
**group_id** | **Int64** | Access group ID |

### Return type

[**WorkflowAccessGroupModel**](WorkflowAccessGroupModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **check_workflow_access**
> check_workflow_access(_api::AccessControlApi, workflow_id::Int64, user_name::String; _mediaType=nothing) -> AccessCheckResponse, OpenAPI.Clients.ApiResponse <br/>
> check_workflow_access(_api::AccessControlApi, response_stream::Channel, workflow_id::Int64, user_name::String; _mediaType=nothing) -> Channel{ AccessCheckResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AccessControlApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |
**user_name** | **String** | Username |

### Return type

[**AccessCheckResponse**](AccessCheckResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_access_group**
> create_access_group(_api::AccessControlApi, access_group_model::AccessGroupModel; _mediaType=nothing) -> AccessGroupModel, OpenAPI.Clients.ApiResponse <br/>
> create_access_group(_api::AccessControlApi, response_stream::Channel, access_group_model::AccessGroupModel; _mediaType=nothing) -> Channel{ AccessGroupModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AccessControlApi** | API context | 
**access_group_model** | [**AccessGroupModel**](AccessGroupModel.md) |  |

### Return type

[**AccessGroupModel**](AccessGroupModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_access_group**
> delete_access_group(_api::AccessControlApi, id::Int64; _mediaType=nothing) -> AccessGroupModel, OpenAPI.Clients.ApiResponse <br/>
> delete_access_group(_api::AccessControlApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ AccessGroupModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AccessControlApi** | API context | 
**id** | **Int64** | Access group ID |

### Return type

[**AccessGroupModel**](AccessGroupModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_access_group**
> get_access_group(_api::AccessControlApi, id::Int64; _mediaType=nothing) -> AccessGroupModel, OpenAPI.Clients.ApiResponse <br/>
> get_access_group(_api::AccessControlApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ AccessGroupModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AccessControlApi** | API context | 
**id** | **Int64** | Access group ID |

### Return type

[**AccessGroupModel**](AccessGroupModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_access_groups**
> list_access_groups(_api::AccessControlApi; offset=nothing, limit=nothing, _mediaType=nothing) -> ListAccessGroupsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_access_groups(_api::AccessControlApi, response_stream::Channel; offset=nothing, limit=nothing, _mediaType=nothing) -> Channel{ ListAccessGroupsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AccessControlApi** | API context | 

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]

### Return type

[**ListAccessGroupsResponse**](ListAccessGroupsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_group_members**
> list_group_members(_api::AccessControlApi, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> ListUserGroupMembershipsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_group_members(_api::AccessControlApi, response_stream::Channel, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> Channel{ ListUserGroupMembershipsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AccessControlApi** | API context | 
**id** | **Int64** | Access group ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]

### Return type

[**ListUserGroupMembershipsResponse**](ListUserGroupMembershipsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_user_groups**
> list_user_groups(_api::AccessControlApi, user_name::String; offset=nothing, limit=nothing, _mediaType=nothing) -> ListAccessGroupsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_user_groups(_api::AccessControlApi, response_stream::Channel, user_name::String; offset=nothing, limit=nothing, _mediaType=nothing) -> Channel{ ListAccessGroupsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AccessControlApi** | API context | 
**user_name** | **String** | Username |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]

### Return type

[**ListAccessGroupsResponse**](ListAccessGroupsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_workflow_groups**
> list_workflow_groups(_api::AccessControlApi, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> ListAccessGroupsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_workflow_groups(_api::AccessControlApi, response_stream::Channel, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> Channel{ ListAccessGroupsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AccessControlApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]

### Return type

[**ListAccessGroupsResponse**](ListAccessGroupsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **reload_auth**
> reload_auth(_api::AccessControlApi; _mediaType=nothing) -> ReloadAuthResponse, OpenAPI.Clients.ApiResponse <br/>
> reload_auth(_api::AccessControlApi, response_stream::Channel; _mediaType=nothing) -> Channel{ ReloadAuthResponse }, OpenAPI.Clients.ApiResponse



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

# **remove_user_from_group**
> remove_user_from_group(_api::AccessControlApi, id::Int64, user_name::String; _mediaType=nothing) -> UserGroupMembershipModel, OpenAPI.Clients.ApiResponse <br/>
> remove_user_from_group(_api::AccessControlApi, response_stream::Channel, id::Int64, user_name::String; _mediaType=nothing) -> Channel{ UserGroupMembershipModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AccessControlApi** | API context | 
**id** | **Int64** | Access group ID |
**user_name** | **String** | Username |

### Return type

[**UserGroupMembershipModel**](UserGroupMembershipModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **remove_workflow_from_group**
> remove_workflow_from_group(_api::AccessControlApi, id::Int64, group_id::Int64; _mediaType=nothing) -> WorkflowAccessGroupModel, OpenAPI.Clients.ApiResponse <br/>
> remove_workflow_from_group(_api::AccessControlApi, response_stream::Channel, id::Int64, group_id::Int64; _mediaType=nothing) -> Channel{ WorkflowAccessGroupModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AccessControlApi** | API context | 
**id** | **Int64** | Workflow ID |
**group_id** | **Int64** | Access group ID |

### Return type

[**WorkflowAccessGroupModel**](WorkflowAccessGroupModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

