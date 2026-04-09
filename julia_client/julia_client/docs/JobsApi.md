# JobsApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**complete_job**](JobsApi.md#complete_job) | **POST** /jobs/{id}/complete_job/{status}/{run_id} |
[**create_job**](JobsApi.md#create_job) | **POST** /jobs |
[**create_jobs**](JobsApi.md#create_jobs) | **POST** /bulk_jobs |
[**delete_job**](JobsApi.md#delete_job) | **DELETE** /jobs/{id} |
[**delete_jobs**](JobsApi.md#delete_jobs) | **DELETE** /jobs |
[**get_job**](JobsApi.md#get_job) | **GET** /jobs/{id} |
[**list_jobs**](JobsApi.md#list_jobs) | **GET** /jobs |
[**manage_status_change**](JobsApi.md#manage_status_change) | **PUT** /jobs/{id}/manage_status_change/{status}/{run_id} |
[**retry_job**](JobsApi.md#retry_job) | **POST** /jobs/{id}/retry/{run_id} |
[**start_job**](JobsApi.md#start_job) | **PUT** /jobs/{id}/start_job/{run_id}/{compute_node_id} |
[**update_job**](JobsApi.md#update_job) | **PUT** /jobs/{id} |


# **complete_job**
> complete_job(_api::JobsApi, id::Int64, status::JobStatus, run_id::Int64, result_model::ResultModel; _mediaType=nothing) -> JobModel, OpenAPI.Clients.ApiResponse <br/>
> complete_job(_api::JobsApi, response_stream::Channel, id::Int64, status::JobStatus, run_id::Int64, result_model::ResultModel; _mediaType=nothing) -> Channel{ JobModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **JobsApi** | API context |
**id** | **Int64** | Job ID |
**status** | [**JobStatus**](.md) | New job status. |
**run_id** | **Int64** | Current job run ID |
**result_model** | [**ResultModel**](ResultModel.md) |  |

### Return type

[**JobModel**](JobModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_job**
> create_job(_api::JobsApi, job_model::JobModel; _mediaType=nothing) -> JobModel, OpenAPI.Clients.ApiResponse <br/>
> create_job(_api::JobsApi, response_stream::Channel, job_model::JobModel; _mediaType=nothing) -> Channel{ JobModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **JobsApi** | API context |
**job_model** | [**JobModel**](JobModel.md) |  |

### Return type

[**JobModel**](JobModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_jobs**
> create_jobs(_api::JobsApi, jobs_model::JobsModel; _mediaType=nothing) -> CreateJobsResponse, OpenAPI.Clients.ApiResponse <br/>
> create_jobs(_api::JobsApi, response_stream::Channel, jobs_model::JobsModel; _mediaType=nothing) -> Channel{ CreateJobsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **JobsApi** | API context |
**jobs_model** | [**JobsModel**](JobsModel.md) |  |

### Return type

[**CreateJobsResponse**](CreateJobsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_job**
> delete_job(_api::JobsApi, id::Int64; _mediaType=nothing) -> JobModel, OpenAPI.Clients.ApiResponse <br/>
> delete_job(_api::JobsApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ JobModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **JobsApi** | API context |
**id** | **Int64** | Job ID |

### Return type

[**JobModel**](JobModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_jobs**
> delete_jobs(_api::JobsApi, workflow_id::Int64; _mediaType=nothing) -> DeleteCountResponse, OpenAPI.Clients.ApiResponse <br/>
> delete_jobs(_api::JobsApi, response_stream::Channel, workflow_id::Int64; _mediaType=nothing) -> Channel{ DeleteCountResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **JobsApi** | API context |
**workflow_id** | **Int64** |  |

### Return type

[**DeleteCountResponse**](DeleteCountResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_job**
> get_job(_api::JobsApi, id::Int64; _mediaType=nothing) -> JobModel, OpenAPI.Clients.ApiResponse <br/>
> get_job(_api::JobsApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ JobModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **JobsApi** | API context |
**id** | **Int64** | ID of the job record |

### Return type

[**JobModel**](JobModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_jobs**
> list_jobs(_api::JobsApi, workflow_id::Int64; status=nothing, needs_file_id=nothing, upstream_job_id=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, include_relationships=nothing, active_compute_node_id=nothing, _mediaType=nothing) -> ListJobsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_jobs(_api::JobsApi, response_stream::Channel, workflow_id::Int64; status=nothing, needs_file_id=nothing, upstream_job_id=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, include_relationships=nothing, active_compute_node_id=nothing, _mediaType=nothing) -> Channel{ ListJobsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **JobsApi** | API context |
**workflow_id** | **Int64** |  |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **status** | [**JobStatus**](.md) |  | [default to nothing]
 **needs_file_id** | **Int64** |  | [default to nothing]
 **upstream_job_id** | **Int64** |  | [default to nothing]
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to nothing]
 **include_relationships** | **Bool** |  | [default to nothing]
 **active_compute_node_id** | **Int64** |  | [default to nothing]

### Return type

[**ListJobsResponse**](ListJobsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **manage_status_change**
> manage_status_change(_api::JobsApi, id::Int64, status::JobStatus, run_id::Int64; _mediaType=nothing) -> JobModel, OpenAPI.Clients.ApiResponse <br/>
> manage_status_change(_api::JobsApi, response_stream::Channel, id::Int64, status::JobStatus, run_id::Int64; _mediaType=nothing) -> Channel{ JobModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **JobsApi** | API context |
**id** | **Int64** | Job ID |
**status** | [**JobStatus**](.md) | New job status |
**run_id** | **Int64** | Current job run ID |

### Return type

[**JobModel**](JobModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **retry_job**
> retry_job(_api::JobsApi, id::Int64, run_id::Int64, max_retries::Int64; _mediaType=nothing) -> JobModel, OpenAPI.Clients.ApiResponse <br/>
> retry_job(_api::JobsApi, response_stream::Channel, id::Int64, run_id::Int64, max_retries::Int64; _mediaType=nothing) -> Channel{ JobModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **JobsApi** | API context |
**id** | **Int64** | Job ID |
**run_id** | **Int64** | Current workflow run ID |
**max_retries** | **Int64** |  |

### Return type

[**JobModel**](JobModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **start_job**
> start_job(_api::JobsApi, id::Int64, run_id::Int64, compute_node_id::Int64; _mediaType=nothing) -> JobModel, OpenAPI.Clients.ApiResponse <br/>
> start_job(_api::JobsApi, response_stream::Channel, id::Int64, run_id::Int64, compute_node_id::Int64; _mediaType=nothing) -> Channel{ JobModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **JobsApi** | API context |
**id** | **Int64** | Job ID |
**run_id** | **Int64** | Current job run ID |
**compute_node_id** | **Int64** | Compute node ID that started the job |

### Return type

[**JobModel**](JobModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_job**
> update_job(_api::JobsApi, id::Int64, job_model::JobModel; _mediaType=nothing) -> JobModel, OpenAPI.Clients.ApiResponse <br/>
> update_job(_api::JobsApi, response_stream::Channel, id::Int64, job_model::JobModel; _mediaType=nothing) -> Channel{ JobModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **JobsApi** | API context |
**id** | **Int64** | ID of the job. |
**job_model** | [**JobModel**](JobModel.md) |  |

### Return type

[**JobModel**](JobModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)
