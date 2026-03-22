# AdminResourcesApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_failure_handler**](AdminResourcesApi.md#create_failure_handler) | **POST** /failure_handlers | 
[**create_jobs**](AdminResourcesApi.md#create_jobs) | **POST** /bulk_jobs | 
[**create_resource_requirements**](AdminResourcesApi.md#create_resource_requirements) | **POST** /resource_requirements | 
[**create_slurm_stats**](AdminResourcesApi.md#create_slurm_stats) | **POST** /slurm_stats | 
[**delete_failure_handler**](AdminResourcesApi.md#delete_failure_handler) | **DELETE** /failure_handlers/{id} | 
[**delete_resource_requirement**](AdminResourcesApi.md#delete_resource_requirement) | **DELETE** /resource_requirements/{id} | 
[**delete_resource_requirements**](AdminResourcesApi.md#delete_resource_requirements) | **DELETE** /resource_requirements | 
[**get_failure_handler**](AdminResourcesApi.md#get_failure_handler) | **GET** /failure_handlers/{id} | 
[**get_resource_requirements**](AdminResourcesApi.md#get_resource_requirements) | **GET** /resource_requirements/{id} | 
[**list_failure_handlers**](AdminResourcesApi.md#list_failure_handlers) | **GET** /workflows/{id}/failure_handlers | 
[**list_resource_requirements**](AdminResourcesApi.md#list_resource_requirements) | **GET** /resource_requirements | 
[**list_slurm_stats**](AdminResourcesApi.md#list_slurm_stats) | **GET** /slurm_stats | 
[**update_resource_requirements**](AdminResourcesApi.md#update_resource_requirements) | **PUT** /resource_requirements/{id} | 


# **create_failure_handler**
> create_failure_handler(_api::AdminResourcesApi, failure_handler_model::FailureHandlerModel; _mediaType=nothing) -> FailureHandlerModel, OpenAPI.Clients.ApiResponse <br/>
> create_failure_handler(_api::AdminResourcesApi, response_stream::Channel, failure_handler_model::FailureHandlerModel; _mediaType=nothing) -> Channel{ FailureHandlerModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AdminResourcesApi** | API context | 
**failure_handler_model** | [**FailureHandlerModel**](FailureHandlerModel.md) |  |

### Return type

[**FailureHandlerModel**](FailureHandlerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_jobs**
> create_jobs(_api::AdminResourcesApi, jobs_model::JobsModel; _mediaType=nothing) -> CreateJobsResponse, OpenAPI.Clients.ApiResponse <br/>
> create_jobs(_api::AdminResourcesApi, response_stream::Channel, jobs_model::JobsModel; _mediaType=nothing) -> Channel{ CreateJobsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AdminResourcesApi** | API context | 
**jobs_model** | [**JobsModel**](JobsModel.md) |  |

### Return type

[**CreateJobsResponse**](CreateJobsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_resource_requirements**
> create_resource_requirements(_api::AdminResourcesApi, resource_requirements_model::ResourceRequirementsModel; _mediaType=nothing) -> ResourceRequirementsModel, OpenAPI.Clients.ApiResponse <br/>
> create_resource_requirements(_api::AdminResourcesApi, response_stream::Channel, resource_requirements_model::ResourceRequirementsModel; _mediaType=nothing) -> Channel{ ResourceRequirementsModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AdminResourcesApi** | API context | 
**resource_requirements_model** | [**ResourceRequirementsModel**](ResourceRequirementsModel.md) |  |

### Return type

[**ResourceRequirementsModel**](ResourceRequirementsModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_slurm_stats**
> create_slurm_stats(_api::AdminResourcesApi, slurm_stats_model::SlurmStatsModel; _mediaType=nothing) -> SlurmStatsModel, OpenAPI.Clients.ApiResponse <br/>
> create_slurm_stats(_api::AdminResourcesApi, response_stream::Channel, slurm_stats_model::SlurmStatsModel; _mediaType=nothing) -> Channel{ SlurmStatsModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AdminResourcesApi** | API context | 
**slurm_stats_model** | [**SlurmStatsModel**](SlurmStatsModel.md) |  |

### Return type

[**SlurmStatsModel**](SlurmStatsModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_failure_handler**
> delete_failure_handler(_api::AdminResourcesApi, id::Int64; body=nothing, _mediaType=nothing) -> FailureHandlerModel, OpenAPI.Clients.ApiResponse <br/>
> delete_failure_handler(_api::AdminResourcesApi, response_stream::Channel, id::Int64; body=nothing, _mediaType=nothing) -> Channel{ FailureHandlerModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AdminResourcesApi** | API context | 
**id** | **Int64** | Failure handler ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

[**FailureHandlerModel**](FailureHandlerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_resource_requirement**
> delete_resource_requirement(_api::AdminResourcesApi, id::Int64; body=nothing, _mediaType=nothing) -> ResourceRequirementsModel, OpenAPI.Clients.ApiResponse <br/>
> delete_resource_requirement(_api::AdminResourcesApi, response_stream::Channel, id::Int64; body=nothing, _mediaType=nothing) -> Channel{ ResourceRequirementsModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AdminResourcesApi** | API context | 
**id** | **Int64** | Resource requirements ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

[**ResourceRequirementsModel**](ResourceRequirementsModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_resource_requirements**
> delete_resource_requirements(_api::AdminResourcesApi, workflow_id::Int64; body=nothing, _mediaType=nothing) -> DeleteCountResponse, OpenAPI.Clients.ApiResponse <br/>
> delete_resource_requirements(_api::AdminResourcesApi, response_stream::Channel, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Channel{ DeleteCountResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AdminResourcesApi** | API context | 
**workflow_id** | **Int64** |  |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

[**DeleteCountResponse**](DeleteCountResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_failure_handler**
> get_failure_handler(_api::AdminResourcesApi, id::Int64; _mediaType=nothing) -> FailureHandlerModel, OpenAPI.Clients.ApiResponse <br/>
> get_failure_handler(_api::AdminResourcesApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ FailureHandlerModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AdminResourcesApi** | API context | 
**id** | **Int64** | Failure handler ID |

### Return type

[**FailureHandlerModel**](FailureHandlerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_resource_requirements**
> get_resource_requirements(_api::AdminResourcesApi, id::Int64; _mediaType=nothing) -> ResourceRequirementsModel, OpenAPI.Clients.ApiResponse <br/>
> get_resource_requirements(_api::AdminResourcesApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ResourceRequirementsModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AdminResourcesApi** | API context | 
**id** | **Int64** | Resource requirements ID |

### Return type

[**ResourceRequirementsModel**](ResourceRequirementsModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_failure_handlers**
> list_failure_handlers(_api::AdminResourcesApi, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> ListFailureHandlersResponse, OpenAPI.Clients.ApiResponse <br/>
> list_failure_handlers(_api::AdminResourcesApi, response_stream::Channel, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> Channel{ ListFailureHandlersResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AdminResourcesApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]

### Return type

[**ListFailureHandlersResponse**](ListFailureHandlersResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_resource_requirements**
> list_resource_requirements(_api::AdminResourcesApi, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, job_id=nothing, name=nothing, memory=nothing, num_cpus=nothing, num_gpus=nothing, num_nodes=nothing, runtime=nothing, _mediaType=nothing) -> ListResourceRequirementsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_resource_requirements(_api::AdminResourcesApi, response_stream::Channel, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, job_id=nothing, name=nothing, memory=nothing, num_cpus=nothing, num_gpus=nothing, num_nodes=nothing, runtime=nothing, _mediaType=nothing) -> Channel{ ListResourceRequirementsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AdminResourcesApi** | API context | 
**workflow_id** | **Int64** |  |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to nothing]
 **job_id** | **Int64** |  | [default to nothing]
 **name** | **String** |  | [default to nothing]
 **memory** | **String** |  | [default to nothing]
 **num_cpus** | **Int64** |  | [default to nothing]
 **num_gpus** | **Int64** |  | [default to nothing]
 **num_nodes** | **Int64** |  | [default to nothing]
 **runtime** | **Int64** |  | [default to nothing]

### Return type

[**ListResourceRequirementsResponse**](ListResourceRequirementsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_slurm_stats**
> list_slurm_stats(_api::AdminResourcesApi, workflow_id::Int64; job_id=nothing, offset=nothing, limit=nothing, _mediaType=nothing) -> ListSlurmStatsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_slurm_stats(_api::AdminResourcesApi, response_stream::Channel, workflow_id::Int64; job_id=nothing, offset=nothing, limit=nothing, _mediaType=nothing) -> Channel{ ListSlurmStatsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AdminResourcesApi** | API context | 
**workflow_id** | **Int64** |  |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **job_id** | **Int64** |  | [default to nothing]
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]

### Return type

[**ListSlurmStatsResponse**](ListSlurmStatsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_resource_requirements**
> update_resource_requirements(_api::AdminResourcesApi, id::Int64, resource_requirements_model::ResourceRequirementsModel; _mediaType=nothing) -> ResourceRequirementsModel, OpenAPI.Clients.ApiResponse <br/>
> update_resource_requirements(_api::AdminResourcesApi, response_stream::Channel, id::Int64, resource_requirements_model::ResourceRequirementsModel; _mediaType=nothing) -> Channel{ ResourceRequirementsModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **AdminResourcesApi** | API context | 
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

