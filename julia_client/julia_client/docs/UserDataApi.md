# UserDataApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_user_data**](UserDataApi.md#create_user_data) | **POST** /user_data | 
[**delete_all_user_data**](UserDataApi.md#delete_all_user_data) | **DELETE** /user_data | 
[**delete_user_data**](UserDataApi.md#delete_user_data) | **DELETE** /user_data/{id} | 
[**get_user_data**](UserDataApi.md#get_user_data) | **GET** /user_data/{id} | 
[**list_user_data**](UserDataApi.md#list_user_data) | **GET** /user_data | 
[**update_user_data**](UserDataApi.md#update_user_data) | **PUT** /user_data/{id} | 


# **create_user_data**
> create_user_data(_api::UserDataApi, user_data_model::UserDataModel; consumer_job_id=nothing, producer_job_id=nothing, _mediaType=nothing) -> UserDataModel, OpenAPI.Clients.ApiResponse <br/>
> create_user_data(_api::UserDataApi, response_stream::Channel, user_data_model::UserDataModel; consumer_job_id=nothing, producer_job_id=nothing, _mediaType=nothing) -> Channel{ UserDataModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **UserDataApi** | API context | 
**user_data_model** | [**UserDataModel**](UserDataModel.md) |  |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **consumer_job_id** | **Int64** |  | [default to nothing]
 **producer_job_id** | **Int64** |  | [default to nothing]

### Return type

[**UserDataModel**](UserDataModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_all_user_data**
> delete_all_user_data(_api::UserDataApi, workflow_id::Int64; _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> delete_all_user_data(_api::UserDataApi, response_stream::Channel, workflow_id::Int64; _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **UserDataApi** | API context | 
**workflow_id** | **Int64** |  |

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_user_data**
> delete_user_data(_api::UserDataApi, id::Int64; _mediaType=nothing) -> UserDataModel, OpenAPI.Clients.ApiResponse <br/>
> delete_user_data(_api::UserDataApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ UserDataModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **UserDataApi** | API context | 
**id** | **Int64** | User data record ID |

### Return type

[**UserDataModel**](UserDataModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_user_data**
> get_user_data(_api::UserDataApi, id::Int64; _mediaType=nothing) -> UserDataModel, OpenAPI.Clients.ApiResponse <br/>
> get_user_data(_api::UserDataApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ UserDataModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **UserDataApi** | API context | 
**id** | **Int64** | User data record ID |

### Return type

[**UserDataModel**](UserDataModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_user_data**
> list_user_data(_api::UserDataApi, workflow_id::Int64; consumer_job_id=nothing, producer_job_id=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, name=nothing, is_ephemeral=nothing, _mediaType=nothing) -> ListUserDataResponse, OpenAPI.Clients.ApiResponse <br/>
> list_user_data(_api::UserDataApi, response_stream::Channel, workflow_id::Int64; consumer_job_id=nothing, producer_job_id=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, name=nothing, is_ephemeral=nothing, _mediaType=nothing) -> Channel{ ListUserDataResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **UserDataApi** | API context | 
**workflow_id** | **Int64** |  |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **consumer_job_id** | **Int64** |  | [default to nothing]
 **producer_job_id** | **Int64** |  | [default to nothing]
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to nothing]
 **name** | **String** |  | [default to nothing]
 **is_ephemeral** | **Bool** |  | [default to nothing]

### Return type

[**ListUserDataResponse**](ListUserDataResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_user_data**
> update_user_data(_api::UserDataApi, id::Int64, user_data_model::UserDataModel; _mediaType=nothing) -> UserDataModel, OpenAPI.Clients.ApiResponse <br/>
> update_user_data(_api::UserDataApi, response_stream::Channel, id::Int64, user_data_model::UserDataModel; _mediaType=nothing) -> Channel{ UserDataModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **UserDataApi** | API context | 
**id** | **Int64** | User data record ID |
**user_data_model** | [**UserDataModel**](UserDataModel.md) |  |

### Return type

[**UserDataModel**](UserDataModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

