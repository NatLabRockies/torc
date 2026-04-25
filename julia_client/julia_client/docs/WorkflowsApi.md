# WorkflowsApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**batch_complete_jobs**](WorkflowsApi.md#batch_complete_jobs) | **POST** /workflows/{id}/batch_complete_jobs | 
[**cancel_workflow**](WorkflowsApi.md#cancel_workflow) | **PUT** /workflows/{id}/cancel | 
[**claim_jobs_based_on_resources**](WorkflowsApi.md#claim_jobs_based_on_resources) | **POST** /workflows/{id}/claim_jobs_based_on_resources/{limit} | 
[**claim_next_jobs**](WorkflowsApi.md#claim_next_jobs) | **POST** /workflows/{id}/claim_next_jobs | 
[**create_workflow**](WorkflowsApi.md#create_workflow) | **POST** /workflows | 
[**delete_workflow**](WorkflowsApi.md#delete_workflow) | **DELETE** /workflows/{id} | 
[**get_active_task_for_workflow**](WorkflowsApi.md#get_active_task_for_workflow) | **GET** /workflows/{id}/active_task | 
[**get_ready_job_requirements**](WorkflowsApi.md#get_ready_job_requirements) | **GET** /workflows/{id}/ready_job_requirements | 
[**get_workflow**](WorkflowsApi.md#get_workflow) | **GET** /workflows/{id} | 
[**get_workflow_status**](WorkflowsApi.md#get_workflow_status) | **GET** /workflows/{id}/status | 
[**initialize_jobs**](WorkflowsApi.md#initialize_jobs) | **POST** /workflows/{id}/initialize_jobs | 
[**is_workflow_complete**](WorkflowsApi.md#is_workflow_complete) | **GET** /workflows/{id}/is_complete | 
[**is_workflow_uninitialized**](WorkflowsApi.md#is_workflow_uninitialized) | **GET** /workflows/{id}/is_uninitialized | 
[**list_job_dependencies**](WorkflowsApi.md#list_job_dependencies) | **GET** /workflows/{id}/job_dependencies | 
[**list_job_file_relationships**](WorkflowsApi.md#list_job_file_relationships) | **GET** /workflows/{id}/job_file_relationships | 
[**list_job_ids**](WorkflowsApi.md#list_job_ids) | **GET** /workflows/{id}/job_ids | 
[**list_job_user_data_relationships**](WorkflowsApi.md#list_job_user_data_relationships) | **GET** /workflows/{id}/job_user_data_relationships | 
[**list_missing_user_data**](WorkflowsApi.md#list_missing_user_data) | **GET** /workflows/{id}/missing_user_data | 
[**list_required_existing_files**](WorkflowsApi.md#list_required_existing_files) | **GET** /workflows/{id}/required_existing_files | 
[**list_workflows**](WorkflowsApi.md#list_workflows) | **GET** /workflows | 
[**process_changed_job_inputs**](WorkflowsApi.md#process_changed_job_inputs) | **POST** /workflows/{id}/process_changed_job_inputs | 
[**reset_job_status**](WorkflowsApi.md#reset_job_status) | **POST** /workflows/{id}/reset_job_status | 
[**reset_workflow_status**](WorkflowsApi.md#reset_workflow_status) | **POST** /workflows/{id}/reset_status | 
[**update_workflow**](WorkflowsApi.md#update_workflow) | **PUT** /workflows/{id} | 
[**update_workflow_status**](WorkflowsApi.md#update_workflow_status) | **PUT** /workflows/{id}/status | 


# **batch_complete_jobs**
> batch_complete_jobs(_api::WorkflowsApi, id::Int64, batch_complete_jobs_request::BatchCompleteJobsRequest; _mediaType=nothing) -> BatchCompleteJobsResponse, OpenAPI.Clients.ApiResponse <br/>
> batch_complete_jobs(_api::WorkflowsApi, response_stream::Channel, id::Int64, batch_complete_jobs_request::BatchCompleteJobsRequest; _mediaType=nothing) -> Channel{ BatchCompleteJobsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |
**batch_complete_jobs_request** | [**BatchCompleteJobsRequest**](BatchCompleteJobsRequest.md) |  |

### Return type

[**BatchCompleteJobsResponse**](BatchCompleteJobsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **cancel_workflow**
> cancel_workflow(_api::WorkflowsApi, id::Int64; _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> cancel_workflow(_api::WorkflowsApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **claim_jobs_based_on_resources**
> claim_jobs_based_on_resources(_api::WorkflowsApi, id::Int64, limit::Int64, compute_nodes_resources::ComputeNodesResources; strict_scheduler_match=nothing, _mediaType=nothing) -> ClaimJobsBasedOnResources, OpenAPI.Clients.ApiResponse <br/>
> claim_jobs_based_on_resources(_api::WorkflowsApi, response_stream::Channel, id::Int64, limit::Int64, compute_nodes_resources::ComputeNodesResources; strict_scheduler_match=nothing, _mediaType=nothing) -> Channel{ ClaimJobsBasedOnResources }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |
**limit** | **Int64** | Maximum number of jobs to claim |
**compute_nodes_resources** | [**ComputeNodesResources**](ComputeNodesResources.md) |  |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **strict_scheduler_match** | **Bool** |  | [default to nothing]

### Return type

[**ClaimJobsBasedOnResources**](ClaimJobsBasedOnResources.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **claim_next_jobs**
> claim_next_jobs(_api::WorkflowsApi, id::Int64; limit=nothing, _mediaType=nothing) -> ClaimNextJobsResponse, OpenAPI.Clients.ApiResponse <br/>
> claim_next_jobs(_api::WorkflowsApi, response_stream::Channel, id::Int64; limit=nothing, _mediaType=nothing) -> Channel{ ClaimNextJobsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **limit** | **Int64** |  | [default to nothing]

### Return type

[**ClaimNextJobsResponse**](ClaimNextJobsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_workflow**
> create_workflow(_api::WorkflowsApi, workflow_model::WorkflowModel; _mediaType=nothing) -> WorkflowModel, OpenAPI.Clients.ApiResponse <br/>
> create_workflow(_api::WorkflowsApi, response_stream::Channel, workflow_model::WorkflowModel; _mediaType=nothing) -> Channel{ WorkflowModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**workflow_model** | [**WorkflowModel**](WorkflowModel.md) |  |

### Return type

[**WorkflowModel**](WorkflowModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_workflow**
> delete_workflow(_api::WorkflowsApi, id::Int64; _mediaType=nothing) -> WorkflowModel, OpenAPI.Clients.ApiResponse <br/>
> delete_workflow(_api::WorkflowsApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ WorkflowModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |

### Return type

[**WorkflowModel**](WorkflowModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_active_task_for_workflow**
> get_active_task_for_workflow(_api::WorkflowsApi, id::Int64; _mediaType=nothing) -> ActiveTaskResponse, OpenAPI.Clients.ApiResponse <br/>
> get_active_task_for_workflow(_api::WorkflowsApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ActiveTaskResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |

### Return type

[**ActiveTaskResponse**](ActiveTaskResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_ready_job_requirements**
> get_ready_job_requirements(_api::WorkflowsApi, id::Int64; scheduler_config_id=nothing, _mediaType=nothing) -> GetReadyJobRequirementsResponse, OpenAPI.Clients.ApiResponse <br/>
> get_ready_job_requirements(_api::WorkflowsApi, response_stream::Channel, id::Int64; scheduler_config_id=nothing, _mediaType=nothing) -> Channel{ GetReadyJobRequirementsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **scheduler_config_id** | **Int64** |  | [default to nothing]

### Return type

[**GetReadyJobRequirementsResponse**](GetReadyJobRequirementsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_workflow**
> get_workflow(_api::WorkflowsApi, id::Int64; _mediaType=nothing) -> WorkflowModel, OpenAPI.Clients.ApiResponse <br/>
> get_workflow(_api::WorkflowsApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ WorkflowModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |

### Return type

[**WorkflowModel**](WorkflowModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_workflow_status**
> get_workflow_status(_api::WorkflowsApi, id::Int64; _mediaType=nothing) -> WorkflowStatusModel, OpenAPI.Clients.ApiResponse <br/>
> get_workflow_status(_api::WorkflowsApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ WorkflowStatusModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |

### Return type

[**WorkflowStatusModel**](WorkflowStatusModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **initialize_jobs**
> initialize_jobs(_api::WorkflowsApi, id::Int64; only_uninitialized=nothing, clear_ephemeral_user_data=nothing, async=nothing, _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> initialize_jobs(_api::WorkflowsApi, response_stream::Channel, id::Int64; only_uninitialized=nothing, clear_ephemeral_user_data=nothing, async=nothing, _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **only_uninitialized** | **Bool** |  | [default to nothing]
 **clear_ephemeral_user_data** | **Bool** |  | [default to nothing]
 **async** | **Bool** |  | [default to nothing]

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **is_workflow_complete**
> is_workflow_complete(_api::WorkflowsApi, id::Int64; _mediaType=nothing) -> IsCompleteResponse, OpenAPI.Clients.ApiResponse <br/>
> is_workflow_complete(_api::WorkflowsApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ IsCompleteResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |

### Return type

[**IsCompleteResponse**](IsCompleteResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **is_workflow_uninitialized**
> is_workflow_uninitialized(_api::WorkflowsApi, id::Int64; _mediaType=nothing) -> IsUninitializedResponse, OpenAPI.Clients.ApiResponse <br/>
> is_workflow_uninitialized(_api::WorkflowsApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ IsUninitializedResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |

### Return type

[**IsUninitializedResponse**](IsUninitializedResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_job_dependencies**
> list_job_dependencies(_api::WorkflowsApi, id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, _mediaType=nothing) -> ListJobDependenciesResponse, OpenAPI.Clients.ApiResponse <br/>
> list_job_dependencies(_api::WorkflowsApi, response_stream::Channel, id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, _mediaType=nothing) -> Channel{ ListJobDependenciesResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to nothing]

### Return type

[**ListJobDependenciesResponse**](ListJobDependenciesResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_job_file_relationships**
> list_job_file_relationships(_api::WorkflowsApi, id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, _mediaType=nothing) -> ListJobFileRelationshipsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_job_file_relationships(_api::WorkflowsApi, response_stream::Channel, id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, _mediaType=nothing) -> Channel{ ListJobFileRelationshipsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to nothing]

### Return type

[**ListJobFileRelationshipsResponse**](ListJobFileRelationshipsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_job_ids**
> list_job_ids(_api::WorkflowsApi, id::Int64; _mediaType=nothing) -> ListJobIdsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_job_ids(_api::WorkflowsApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ListJobIdsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |

### Return type

[**ListJobIdsResponse**](ListJobIdsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_job_user_data_relationships**
> list_job_user_data_relationships(_api::WorkflowsApi, id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, _mediaType=nothing) -> ListJobUserDataRelationshipsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_job_user_data_relationships(_api::WorkflowsApi, response_stream::Channel, id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, _mediaType=nothing) -> Channel{ ListJobUserDataRelationshipsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to nothing]

### Return type

[**ListJobUserDataRelationshipsResponse**](ListJobUserDataRelationshipsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_missing_user_data**
> list_missing_user_data(_api::WorkflowsApi, id::Int64; _mediaType=nothing) -> ListMissingUserDataResponse, OpenAPI.Clients.ApiResponse <br/>
> list_missing_user_data(_api::WorkflowsApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ListMissingUserDataResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |

### Return type

[**ListMissingUserDataResponse**](ListMissingUserDataResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_required_existing_files**
> list_required_existing_files(_api::WorkflowsApi, id::Int64; _mediaType=nothing) -> ListRequiredExistingFilesResponse, OpenAPI.Clients.ApiResponse <br/>
> list_required_existing_files(_api::WorkflowsApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ListRequiredExistingFilesResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |

### Return type

[**ListRequiredExistingFilesResponse**](ListRequiredExistingFilesResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_workflows**
> list_workflows(_api::WorkflowsApi; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, name=nothing, user=nothing, description=nothing, is_archived=nothing, _mediaType=nothing) -> ListWorkflowsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_workflows(_api::WorkflowsApi, response_stream::Channel; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, name=nothing, user=nothing, description=nothing, is_archived=nothing, _mediaType=nothing) -> Channel{ ListWorkflowsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to nothing]
 **name** | **String** |  | [default to nothing]
 **user** | **String** |  | [default to nothing]
 **description** | **String** |  | [default to nothing]
 **is_archived** | **Bool** |  | [default to nothing]

### Return type

[**ListWorkflowsResponse**](ListWorkflowsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **process_changed_job_inputs**
> process_changed_job_inputs(_api::WorkflowsApi, id::Int64; dry_run=nothing, _mediaType=nothing) -> ProcessChangedJobInputsResponse, OpenAPI.Clients.ApiResponse <br/>
> process_changed_job_inputs(_api::WorkflowsApi, response_stream::Channel, id::Int64; dry_run=nothing, _mediaType=nothing) -> Channel{ ProcessChangedJobInputsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **dry_run** | **Bool** |  | [default to nothing]

### Return type

[**ProcessChangedJobInputsResponse**](ProcessChangedJobInputsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **reset_job_status**
> reset_job_status(_api::WorkflowsApi, id::Int64; failed_only=nothing, _mediaType=nothing) -> ResetJobStatusResponse, OpenAPI.Clients.ApiResponse <br/>
> reset_job_status(_api::WorkflowsApi, response_stream::Channel, id::Int64; failed_only=nothing, _mediaType=nothing) -> Channel{ ResetJobStatusResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **failed_only** | **Bool** |  | [default to nothing]

### Return type

[**ResetJobStatusResponse**](ResetJobStatusResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **reset_workflow_status**
> reset_workflow_status(_api::WorkflowsApi, id::Int64; force=nothing, _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> reset_workflow_status(_api::WorkflowsApi, response_stream::Channel, id::Int64; force=nothing, _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **force** | **Bool** |  | [default to nothing]

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_workflow**
> update_workflow(_api::WorkflowsApi, id::Int64, workflow_model::WorkflowModel; _mediaType=nothing) -> WorkflowModel, OpenAPI.Clients.ApiResponse <br/>
> update_workflow(_api::WorkflowsApi, response_stream::Channel, id::Int64, workflow_model::WorkflowModel; _mediaType=nothing) -> Channel{ WorkflowModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |
**workflow_model** | [**WorkflowModel**](WorkflowModel.md) |  |

### Return type

[**WorkflowModel**](WorkflowModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_workflow_status**
> update_workflow_status(_api::WorkflowsApi, id::Int64, workflow_status_model::WorkflowStatusModel; _mediaType=nothing) -> WorkflowStatusModel, OpenAPI.Clients.ApiResponse <br/>
> update_workflow_status(_api::WorkflowsApi, response_stream::Channel, id::Int64, workflow_status_model::WorkflowStatusModel; _mediaType=nothing) -> Channel{ WorkflowStatusModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **WorkflowsApi** | API context | 
**id** | **Int64** | Workflow ID |
**workflow_status_model** | [**WorkflowStatusModel**](WorkflowStatusModel.md) |  |

### Return type

[**WorkflowStatusModel**](WorkflowStatusModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

