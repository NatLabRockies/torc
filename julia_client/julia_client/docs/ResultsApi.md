# ResultsApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_result**](ResultsApi.md#create_result) | **POST** /results | 
[**delete_result**](ResultsApi.md#delete_result) | **DELETE** /results/{id} | 
[**delete_results**](ResultsApi.md#delete_results) | **DELETE** /results | 
[**get_result**](ResultsApi.md#get_result) | **GET** /results/{id} | 
[**list_results**](ResultsApi.md#list_results) | **GET** /results | 
[**update_result**](ResultsApi.md#update_result) | **PUT** /results/{id} | 


# **create_result**
> create_result(_api::ResultsApi, result_model::ResultModel; _mediaType=nothing) -> ResultModel, OpenAPI.Clients.ApiResponse <br/>
> create_result(_api::ResultsApi, response_stream::Channel, result_model::ResultModel; _mediaType=nothing) -> Channel{ ResultModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ResultsApi** | API context | 
**result_model** | [**ResultModel**](ResultModel.md) |  |

### Return type

[**ResultModel**](ResultModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_result**
> delete_result(_api::ResultsApi, id::Int64; _mediaType=nothing) -> ResultModel, OpenAPI.Clients.ApiResponse <br/>
> delete_result(_api::ResultsApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ResultModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ResultsApi** | API context | 
**id** | **Int64** | Results ID |

### Return type

[**ResultModel**](ResultModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_results**
> delete_results(_api::ResultsApi, workflow_id::Int64; _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> delete_results(_api::ResultsApi, response_stream::Channel, workflow_id::Int64; _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ResultsApi** | API context | 
**workflow_id** | **Int64** |  |

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_result**
> get_result(_api::ResultsApi, id::Int64; _mediaType=nothing) -> ResultModel, OpenAPI.Clients.ApiResponse <br/>
> get_result(_api::ResultsApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ResultModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ResultsApi** | API context | 
**id** | **Int64** | Results ID |

### Return type

[**ResultModel**](ResultModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_results**
> list_results(_api::ResultsApi, workflow_id::Int64; job_id=nothing, run_id=nothing, return_code=nothing, status=nothing, compute_node_id=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, all_runs=nothing, ids=nothing, statuses=nothing, _mediaType=nothing) -> ListResultsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_results(_api::ResultsApi, response_stream::Channel, workflow_id::Int64; job_id=nothing, run_id=nothing, return_code=nothing, status=nothing, compute_node_id=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, all_runs=nothing, ids=nothing, statuses=nothing, _mediaType=nothing) -> Channel{ ListResultsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ResultsApi** | API context | 
**workflow_id** | **Int64** |  |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **job_id** | **Int64** |  | [default to nothing]
 **run_id** | **Int64** |  | [default to nothing]
 **return_code** | **Int64** |  | [default to nothing]
 **status** | [**JobStatus**](.md) |  | [default to nothing]
 **compute_node_id** | **Int64** |  | [default to nothing]
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to nothing]
 **all_runs** | **Bool** |  | [default to nothing]
 **ids** | [**Vector{Int64}**](Int64.md) | Comma-separated job IDs: restrict to results of these jobs, scoped to &#x60;workflow_id&#x60;. At most &#x60;MAX_RECORD_TRANSFER_COUNT&#x60; IDs; more returns 422. | [default to nothing]
 **statuses** | [**Vector{JobStatus}**](JobStatus.md) | Comma-separated statuses (&#x60;statuses&#x3D;failed,terminated&#x60;): match any of them. Mutually exclusive with &#x60;status&#x60; (both given returns 422). | [default to nothing]

### Return type

[**ListResultsResponse**](ListResultsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_result**
> update_result(_api::ResultsApi, id::Int64, result_model::ResultModel; _mediaType=nothing) -> ResultModel, OpenAPI.Clients.ApiResponse <br/>
> update_result(_api::ResultsApi, response_stream::Channel, id::Int64, result_model::ResultModel; _mediaType=nothing) -> Channel{ ResultModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **ResultsApi** | API context | 
**id** | **Int64** | Result ID |
**result_model** | [**ResultModel**](ResultModel.md) |  |

### Return type

[**ResultModel**](ResultModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

