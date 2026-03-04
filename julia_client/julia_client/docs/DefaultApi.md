# DefaultApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**add_user_to_group**](DefaultApi.md#add_user_to_group) | **POST** /access_groups/{id}/members | Add a user to an access group.
[**add_workflow_to_group**](DefaultApi.md#add_workflow_to_group) | **POST** /workflows/{id}/access_groups | Grant an access group access to a workflow.
[**cancel_workflow**](DefaultApi.md#cancel_workflow) | **PUT** /workflows/{id}/cancel | Cancel a workflow. Workers will detect the status change and cancel jobs.
[**check_workflow_access**](DefaultApi.md#check_workflow_access) | **GET** /access_check/{workflow_id}/{user_name} | Check if a user can access a workflow.
[**claim_action**](DefaultApi.md#claim_action) | **POST** /workflows/{id}/actions/{action_id}/claim | Atomically claim a workflow action.
[**claim_jobs_based_on_resources**](DefaultApi.md#claim_jobs_based_on_resources) | **POST** /workflows/{id}/claim_jobs_based_on_resources/{limit} | Return jobs that are ready for submission and meet worker resource requirements. Set status to pending.
[**claim_next_jobs**](DefaultApi.md#claim_next_jobs) | **POST** /workflows/{id}/claim_next_jobs | Return user-requested number of jobs that are ready for submission. Sets status to pending.
[**complete_job**](DefaultApi.md#complete_job) | **POST** /jobs/{id}/complete_job/{status}/{run_id} | Complete a job, connect it to a result, and manage side effects.
[**create_access_group**](DefaultApi.md#create_access_group) | **POST** /access_groups | Create a new access group.
[**create_compute_node**](DefaultApi.md#create_compute_node) | **POST** /compute_nodes | Store a compute node.
[**create_event**](DefaultApi.md#create_event) | **POST** /events | Store an event.
[**create_failure_handler**](DefaultApi.md#create_failure_handler) | **POST** /failure_handlers | Create a failure handler.
[**create_file**](DefaultApi.md#create_file) | **POST** /files | Store a file.
[**create_job**](DefaultApi.md#create_job) | **POST** /jobs | Store a job.
[**create_jobs**](DefaultApi.md#create_jobs) | **POST** /bulk_jobs | Create jobs in bulk. Recommended max job count of 10,000.
[**create_local_scheduler**](DefaultApi.md#create_local_scheduler) | **POST** /local_schedulers | Store a local scheduler.
[**create_remote_workers**](DefaultApi.md#create_remote_workers) | **POST** /workflows/{id}/remote_workers | Store remote workers for a workflow.
[**create_resource_requirements**](DefaultApi.md#create_resource_requirements) | **POST** /resource_requirements | Store one resource requirements record.
[**create_result**](DefaultApi.md#create_result) | **POST** /results | Store a job result.
[**create_ro_crate_entity**](DefaultApi.md#create_ro_crate_entity) | **POST** /ro_crate_entities | Create a new RO-Crate entity.
[**create_scheduled_compute_node**](DefaultApi.md#create_scheduled_compute_node) | **POST** /scheduled_compute_nodes | Store a scheduled compute node.
[**create_slurm_scheduler**](DefaultApi.md#create_slurm_scheduler) | **POST** /slurm_schedulers | Store a Slurm compute node configuration.
[**create_slurm_stats**](DefaultApi.md#create_slurm_stats) | **POST** /slurm_stats | Store Slurm accounting stats for a job step.
[**create_user_data**](DefaultApi.md#create_user_data) | **POST** /user_data | Store a user data record.
[**create_workflow**](DefaultApi.md#create_workflow) | **POST** /workflows | Store a workflow.
[**create_workflow_action**](DefaultApi.md#create_workflow_action) | **POST** /workflows/{id}/actions | Create a workflow action.
[**delete_access_group**](DefaultApi.md#delete_access_group) | **DELETE** /access_groups/{id} | Delete an access group.
[**delete_all_user_data**](DefaultApi.md#delete_all_user_data) | **DELETE** /user_data | Delete all user data records for one workflow.
[**delete_compute_node**](DefaultApi.md#delete_compute_node) | **DELETE** /compute_nodes/{id} | Delete a compute node.
[**delete_compute_nodes**](DefaultApi.md#delete_compute_nodes) | **DELETE** /compute_nodes | Delete all compute node records for one workflow.
[**delete_event**](DefaultApi.md#delete_event) | **DELETE** /events/{id} | Delete an event.
[**delete_events**](DefaultApi.md#delete_events) | **DELETE** /events | Delete all events for one workflow.
[**delete_failure_handler**](DefaultApi.md#delete_failure_handler) | **DELETE** /failure_handlers/{id} | Delete a failure handler.
[**delete_file**](DefaultApi.md#delete_file) | **DELETE** /files/{id} | Delete a file.
[**delete_files**](DefaultApi.md#delete_files) | **DELETE** /files | Delete all files for one workflow.
[**delete_job**](DefaultApi.md#delete_job) | **DELETE** /jobs/{id} | Delete a job.
[**delete_jobs**](DefaultApi.md#delete_jobs) | **DELETE** /jobs | Delete all jobs for one workflow.
[**delete_local_scheduler**](DefaultApi.md#delete_local_scheduler) | **DELETE** /local_schedulers/{id} | Delete a local scheduler.
[**delete_local_schedulers**](DefaultApi.md#delete_local_schedulers) | **DELETE** /local_schedulers | Delete all local schedulers for one workflow.
[**delete_remote_worker**](DefaultApi.md#delete_remote_worker) | **DELETE** /workflows/{id}/remote_workers/{worker} | Delete a remote worker from a workflow.
[**delete_resource_requirement**](DefaultApi.md#delete_resource_requirement) | **DELETE** /resource_requirements/{id} | Delete a resource requirements record.
[**delete_resource_requirements**](DefaultApi.md#delete_resource_requirements) | **DELETE** /resource_requirements | Delete all resource requirements records for one workflow.
[**delete_result**](DefaultApi.md#delete_result) | **DELETE** /results/{id} | Delete a job result.
[**delete_results**](DefaultApi.md#delete_results) | **DELETE** /results | Delete all job results for one workflow.
[**delete_ro_crate_entities**](DefaultApi.md#delete_ro_crate_entities) | **DELETE** /workflows/{id}/ro_crate_entities | Delete all RO-Crate entities for a workflow.
[**delete_ro_crate_entity**](DefaultApi.md#delete_ro_crate_entity) | **DELETE** /ro_crate_entities/{id} | Delete an RO-Crate entity.
[**delete_scheduled_compute_node**](DefaultApi.md#delete_scheduled_compute_node) | **DELETE** /scheduled_compute_nodes/{id} | Delete a scheduled compute node.
[**delete_scheduled_compute_nodes**](DefaultApi.md#delete_scheduled_compute_nodes) | **DELETE** /scheduled_compute_nodes | Delete all scheduled compute node records for one workflow.
[**delete_slurm_scheduler**](DefaultApi.md#delete_slurm_scheduler) | **DELETE** /slurm_schedulers/{id} | Delete Slurm compute node configuration.
[**delete_slurm_schedulers**](DefaultApi.md#delete_slurm_schedulers) | **DELETE** /slurm_schedulers | Retrieve all Slurm compute node configurations for one workflow.
[**delete_user_data**](DefaultApi.md#delete_user_data) | **DELETE** /user_data/{id} | Delete a user data record.
[**delete_workflow**](DefaultApi.md#delete_workflow) | **DELETE** /workflows/{id} | Delete a workflow.
[**get_access_group**](DefaultApi.md#get_access_group) | **GET** /access_groups/{id} | Get an access group by ID.
[**get_compute_node**](DefaultApi.md#get_compute_node) | **GET** /compute_nodes/{id} | Retrieve a compute node by ID.
[**get_event**](DefaultApi.md#get_event) | **GET** /events/{id} | Retrieve an event by ID.
[**get_failure_handler**](DefaultApi.md#get_failure_handler) | **GET** /failure_handlers/{id} | Get a failure handler by ID.
[**get_file**](DefaultApi.md#get_file) | **GET** /files/{id} | Retrieve a file.
[**get_job**](DefaultApi.md#get_job) | **GET** /jobs/{id} | Retrieve a job.
[**get_latest_event_timestamp**](DefaultApi.md#get_latest_event_timestamp) | **GET** /workflows/{id}/latest_event_timestamp | Return the timestamp of the latest event in ms since the epoch in UTC.
[**get_local_scheduler**](DefaultApi.md#get_local_scheduler) | **GET** /local_schedulers/{id} | Retrieve a local scheduler.
[**get_pending_actions**](DefaultApi.md#get_pending_actions) | **GET** /workflows/{id}/actions/pending | Get pending workflow actions, optionally filtered by trigger type.
[**get_ready_job_requirements**](DefaultApi.md#get_ready_job_requirements) | **GET** /workflows/{id}/ready_job_requirements | Return the resource requirements for jobs with a status of ready.
[**get_resource_requirements**](DefaultApi.md#get_resource_requirements) | **GET** /resource_requirements/{id} | Retrieve one resource requirements record.
[**get_result**](DefaultApi.md#get_result) | **GET** /results/{id} | Retrieve a job result.
[**get_ro_crate_entity**](DefaultApi.md#get_ro_crate_entity) | **GET** /ro_crate_entities/{id} | Get an RO-Crate entity by ID.
[**get_scheduled_compute_node**](DefaultApi.md#get_scheduled_compute_node) | **GET** /scheduled_compute_nodes/{id} | Retrieve a scheduled compute node.
[**get_slurm_scheduler**](DefaultApi.md#get_slurm_scheduler) | **GET** /slurm_schedulers/{id} | Retrieve a Slurm compute node configuration.
[**get_user_data**](DefaultApi.md#get_user_data) | **GET** /user_data/{id} | Retrieve a user data record.
[**get_version**](DefaultApi.md#get_version) | **GET** /version | Return the version of the service.
[**get_workflow**](DefaultApi.md#get_workflow) | **GET** /workflows/{id} | Retrieve a workflow.
[**get_workflow_actions**](DefaultApi.md#get_workflow_actions) | **GET** /workflows/{id}/actions | Get all workflow actions for a workflow.
[**get_workflow_status**](DefaultApi.md#get_workflow_status) | **GET** /workflows/{id}/status | Return the workflow status.
[**initialize_jobs**](DefaultApi.md#initialize_jobs) | **POST** /workflows/{id}/initialize_jobs | Initialize job relationships based on file and user_data relationships.
[**is_workflow_complete**](DefaultApi.md#is_workflow_complete) | **GET** /workflows/{id}/is_complete | Return true if all jobs in the workflow are complete.
[**is_workflow_uninitialized**](DefaultApi.md#is_workflow_uninitialized) | **GET** /workflows/{id}/is_uninitialized | Return true if all jobs in the workflow are uninitialized or disabled.
[**list_access_groups**](DefaultApi.md#list_access_groups) | **GET** /access_groups | List all access groups.
[**list_compute_nodes**](DefaultApi.md#list_compute_nodes) | **GET** /compute_nodes | Retrieve all compute node records for one workflow.
[**list_events**](DefaultApi.md#list_events) | **GET** /events | Retrieve all events for one workflow.
[**list_failure_handlers**](DefaultApi.md#list_failure_handlers) | **GET** /workflows/{id}/failure_handlers | List failure handlers for a workflow.
[**list_files**](DefaultApi.md#list_files) | **GET** /files | Retrieve all files for one workflow.
[**list_group_members**](DefaultApi.md#list_group_members) | **GET** /access_groups/{id}/members | List members of an access group.
[**list_job_dependencies**](DefaultApi.md#list_job_dependencies) | **GET** /workflows/{id}/job_dependencies | Retrieve job blocking relationships for a workflow.
[**list_job_file_relationships**](DefaultApi.md#list_job_file_relationships) | **GET** /workflows/{id}/job_file_relationships | Retrieve job-file relationships for a workflow.
[**list_job_ids**](DefaultApi.md#list_job_ids) | **GET** /workflows/{id}/job_ids | Retrieve all job IDs for one workflow.
[**list_job_user_data_relationships**](DefaultApi.md#list_job_user_data_relationships) | **GET** /workflows/{id}/job_user_data_relationships | Retrieve job-user_data relationships for a workflow.
[**list_jobs**](DefaultApi.md#list_jobs) | **GET** /jobs | Retrieve all jobs for one workflow.
[**list_local_schedulers**](DefaultApi.md#list_local_schedulers) | **GET** /local_schedulers | Retrieve local schedulers for one workflow.
[**list_missing_user_data**](DefaultApi.md#list_missing_user_data) | **GET** /workflows/{id}/missing_user_data | List missing user data that should exist.
[**list_remote_workers**](DefaultApi.md#list_remote_workers) | **GET** /workflows/{id}/remote_workers | List all remote workers for a workflow.
[**list_required_existing_files**](DefaultApi.md#list_required_existing_files) | **GET** /workflows/{id}/required_existing_files | List files that must exist.
[**list_resource_requirements**](DefaultApi.md#list_resource_requirements) | **GET** /resource_requirements | Retrieve all resource requirements records for one workflow.
[**list_results**](DefaultApi.md#list_results) | **GET** /results | Retrieve all job results for one workflow.
[**list_ro_crate_entities**](DefaultApi.md#list_ro_crate_entities) | **GET** /workflows/{id}/ro_crate_entities | List all RO-Crate entities for a workflow.
[**list_scheduled_compute_nodes**](DefaultApi.md#list_scheduled_compute_nodes) | **GET** /scheduled_compute_nodes | Retrieve scheduled compute node records for one workflow.
[**list_slurm_schedulers**](DefaultApi.md#list_slurm_schedulers) | **GET** /slurm_schedulers | Retrieve a Slurm compute node configuration.
[**list_slurm_stats**](DefaultApi.md#list_slurm_stats) | **GET** /slurm_stats | List Slurm accounting stats.
[**list_user_data**](DefaultApi.md#list_user_data) | **GET** /user_data | Retrieve all user data records for one workflow.
[**list_user_groups**](DefaultApi.md#list_user_groups) | **GET** /users/{user_name}/groups | List groups a user belongs to.
[**list_workflow_groups**](DefaultApi.md#list_workflow_groups) | **GET** /workflows/{id}/access_groups | List access groups that have access to a workflow.
[**list_workflows**](DefaultApi.md#list_workflows) | **GET** /workflows | Retrieve all workflows.
[**manage_status_change**](DefaultApi.md#manage_status_change) | **PUT** /jobs/{id}/manage_status_change/{status}/{run_id} | Change the status of a job and manage side effects.
[**ping**](DefaultApi.md#ping) | **GET** /ping | Check if the service is running.
[**process_changed_job_inputs**](DefaultApi.md#process_changed_job_inputs) | **POST** /workflows/{id}/process_changed_job_inputs | Check for changed job inputs and update status accordingly.
[**remove_user_from_group**](DefaultApi.md#remove_user_from_group) | **DELETE** /access_groups/{id}/members/{user_name} | Remove a user from an access group.
[**remove_workflow_from_group**](DefaultApi.md#remove_workflow_from_group) | **DELETE** /workflows/{id}/access_groups/{group_id} | Revoke an access group&#39;s access to a workflow.
[**reset_job_status**](DefaultApi.md#reset_job_status) | **POST** /workflows/{id}/reset_job_status | Reset status for jobs to uninitialized.
[**reset_workflow_status**](DefaultApi.md#reset_workflow_status) | **POST** /workflows/{id}/reset_status | Reset worklow status.
[**retry_job**](DefaultApi.md#retry_job) | **POST** /jobs/{id}/retry/{run_id} | Retry a failed job.
[**start_job**](DefaultApi.md#start_job) | **PUT** /jobs/{id}/start_job/{run_id}/{compute_node_id} | Start a job and manage side effects.
[**update_compute_node**](DefaultApi.md#update_compute_node) | **PUT** /compute_nodes/{id} | Update a compute node.
[**update_event**](DefaultApi.md#update_event) | **PUT** /events/{id} | Update an event.
[**update_file**](DefaultApi.md#update_file) | **PUT** /files/{id} | Update a file.
[**update_job**](DefaultApi.md#update_job) | **PUT** /jobs/{id} | Update a job.
[**update_local_scheduler**](DefaultApi.md#update_local_scheduler) | **PUT** /local_schedulers/{id} | Update a local scheduler.
[**update_resource_requirements**](DefaultApi.md#update_resource_requirements) | **PUT** /resource_requirements/{id} | Update one resource requirements record.
[**update_result**](DefaultApi.md#update_result) | **PUT** /results/{id} | Update a job result.
[**update_ro_crate_entity**](DefaultApi.md#update_ro_crate_entity) | **PUT** /ro_crate_entities/{id} | Update an RO-Crate entity.
[**update_scheduled_compute_node**](DefaultApi.md#update_scheduled_compute_node) | **PUT** /scheduled_compute_nodes/{id} | Update a scheduled compute node.
[**update_slurm_scheduler**](DefaultApi.md#update_slurm_scheduler) | **PUT** /slurm_schedulers/{id} | Update a Slurm compute node configuration.
[**update_user_data**](DefaultApi.md#update_user_data) | **PUT** /user_data/{id} | Update a user data record.
[**update_workflow**](DefaultApi.md#update_workflow) | **PUT** /workflows/{id} | Update a workflow.
[**update_workflow_status**](DefaultApi.md#update_workflow_status) | **PUT** /workflows/{id}/status | Update the workflow status.


# **add_user_to_group**
> add_user_to_group(_api::DefaultApi, id::Int64, body::UserGroupMembershipModel; _mediaType=nothing) -> UserGroupMembershipModel, OpenAPI.Clients.ApiResponse <br/>
> add_user_to_group(_api::DefaultApi, response_stream::Channel, id::Int64, body::UserGroupMembershipModel; _mediaType=nothing) -> Channel{ UserGroupMembershipModel }, OpenAPI.Clients.ApiResponse

Add a user to an access group.

Add a user to an access group.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the access group |
**body** | [**UserGroupMembershipModel**](UserGroupMembershipModel.md) | User membership to add |

### Return type

[**UserGroupMembershipModel**](UserGroupMembershipModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **add_workflow_to_group**
> add_workflow_to_group(_api::DefaultApi, id::Int64, body::WorkflowAccessGroupModel; _mediaType=nothing) -> WorkflowAccessGroupModel, OpenAPI.Clients.ApiResponse <br/>
> add_workflow_to_group(_api::DefaultApi, response_stream::Channel, id::Int64, body::WorkflowAccessGroupModel; _mediaType=nothing) -> Channel{ WorkflowAccessGroupModel }, OpenAPI.Clients.ApiResponse

Grant an access group access to a workflow.

Grant an access group access to a workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the workflow |
**body** | [**WorkflowAccessGroupModel**](WorkflowAccessGroupModel.md) | Group association to create |

### Return type

[**WorkflowAccessGroupModel**](WorkflowAccessGroupModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **cancel_workflow**
> cancel_workflow(_api::DefaultApi, id::Int64; body=nothing, _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> cancel_workflow(_api::DefaultApi, response_stream::Channel, id::Int64; body=nothing, _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse

Cancel a workflow. Workers will detect the status change and cancel jobs.

Cancel a workflow. Workers will detect the status change and cancel jobs.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **check_workflow_access**
> check_workflow_access(_api::DefaultApi, workflow_id::Int64, user_name::String; _mediaType=nothing) -> AccessCheckResponse, OpenAPI.Clients.ApiResponse <br/>
> check_workflow_access(_api::DefaultApi, response_stream::Channel, workflow_id::Int64, user_name::String; _mediaType=nothing) -> Channel{ AccessCheckResponse }, OpenAPI.Clients.ApiResponse

Check if a user can access a workflow.

Check if a user can access a workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | ID of the workflow |
**user_name** | **String** | Username to check |

### Return type

[**AccessCheckResponse**](AccessCheckResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **claim_action**
> claim_action(_api::DefaultApi, id::Int64, action_id::Int64, body::ClaimActionRequest; _mediaType=nothing) -> ClaimAction200Response, OpenAPI.Clients.ApiResponse <br/>
> claim_action(_api::DefaultApi, response_stream::Channel, id::Int64, action_id::Int64, body::ClaimActionRequest; _mediaType=nothing) -> Channel{ ClaimAction200Response }, OpenAPI.Clients.ApiResponse

Atomically claim a workflow action.

Atomically claim a workflow action for execution by a compute node.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |
**action_id** | **Int64** | Action ID |
**body** | [**ClaimActionRequest**](ClaimActionRequest.md) | Compute node claiming the action |

### Return type

[**ClaimAction200Response**](ClaimAction200Response.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **claim_jobs_based_on_resources**
> claim_jobs_based_on_resources(_api::DefaultApi, id::Int64, limit::Int64, body::ComputeNodesResources; sort_method=nothing, strict_scheduler_match=nothing, _mediaType=nothing) -> ClaimJobsBasedOnResourcesResponse, OpenAPI.Clients.ApiResponse <br/>
> claim_jobs_based_on_resources(_api::DefaultApi, response_stream::Channel, id::Int64, limit::Int64, body::ComputeNodesResources; sort_method=nothing, strict_scheduler_match=nothing, _mediaType=nothing) -> Channel{ ClaimJobsBasedOnResourcesResponse }, OpenAPI.Clients.ApiResponse

Return jobs that are ready for submission and meet worker resource requirements. Set status to pending.

Return jobs that are ready for submission and meet worker resource requirements. Set status to pending.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |
**limit** | **Int64** |  |
**body** | [**ComputeNodesResources**](ComputeNodesResources.md) | Available worker resources. |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **sort_method** | [**JobsSortMethod**](.md) |  | [default to nothing]
 **strict_scheduler_match** | **Bool** | If true, only claim jobs that match the scheduler_id of the worker. If false (default), jobs with a scheduler_id mismatch will be claimed if no matching jobs are available. | [default to false]

### Return type

[**ClaimJobsBasedOnResourcesResponse**](ClaimJobsBasedOnResourcesResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **claim_next_jobs**
> claim_next_jobs(_api::DefaultApi, id::Int64; limit=nothing, body=nothing, _mediaType=nothing) -> ClaimNextJobsResponse, OpenAPI.Clients.ApiResponse <br/>
> claim_next_jobs(_api::DefaultApi, response_stream::Channel, id::Int64; limit=nothing, body=nothing, _mediaType=nothing) -> Channel{ ClaimNextJobsResponse }, OpenAPI.Clients.ApiResponse

Return user-requested number of jobs that are ready for submission. Sets status to pending.

Return user-requested number of jobs that are ready for submission. Sets status to pending.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **limit** | **Int64** |  | [default to 1.0]
 **body** | **Any** |  | 

### Return type

[**ClaimNextJobsResponse**](ClaimNextJobsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **complete_job**
> complete_job(_api::DefaultApi, id::Int64, status::JobStatus, run_id::Int64, body::ResultModel; _mediaType=nothing) -> JobModel, OpenAPI.Clients.ApiResponse <br/>
> complete_job(_api::DefaultApi, response_stream::Channel, id::Int64, status::JobStatus, run_id::Int64, body::ResultModel; _mediaType=nothing) -> Channel{ JobModel }, OpenAPI.Clients.ApiResponse

Complete a job, connect it to a result, and manage side effects.

Complete a job, connect it to a result, and manage side effects.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Job ID |
**status** | [**JobStatus**](.md) | New job status. |
**run_id** | **Int64** | Current job run ID |
**body** | [**ResultModel**](ResultModel.md) | Result of the job. |

### Return type

[**JobModel**](JobModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_access_group**
> create_access_group(_api::DefaultApi, body::AccessGroupModel; _mediaType=nothing) -> AccessGroupModel, OpenAPI.Clients.ApiResponse <br/>
> create_access_group(_api::DefaultApi, response_stream::Channel, body::AccessGroupModel; _mediaType=nothing) -> Channel{ AccessGroupModel }, OpenAPI.Clients.ApiResponse

Create a new access group.

Create a new access group.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**body** | [**AccessGroupModel**](AccessGroupModel.md) | Access group to create |

### Return type

[**AccessGroupModel**](AccessGroupModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_compute_node**
> create_compute_node(_api::DefaultApi, body::ComputeNodeModel; _mediaType=nothing) -> ComputeNodeModel, OpenAPI.Clients.ApiResponse <br/>
> create_compute_node(_api::DefaultApi, response_stream::Channel, body::ComputeNodeModel; _mediaType=nothing) -> Channel{ ComputeNodeModel }, OpenAPI.Clients.ApiResponse

Store a compute node.

Store a compute node.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**body** | [**ComputeNodeModel**](ComputeNodeModel.md) | Compute node |

### Return type

[**ComputeNodeModel**](ComputeNodeModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_event**
> create_event(_api::DefaultApi, body::EventModel; _mediaType=nothing) -> EventModel, OpenAPI.Clients.ApiResponse <br/>
> create_event(_api::DefaultApi, response_stream::Channel, body::EventModel; _mediaType=nothing) -> Channel{ EventModel }, OpenAPI.Clients.ApiResponse

Store an event.

Store an event.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**body** | [**EventModel**](EventModel.md) | Event body |

### Return type

[**EventModel**](EventModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_failure_handler**
> create_failure_handler(_api::DefaultApi, body::FailureHandlerModel; _mediaType=nothing) -> FailureHandlerModel, OpenAPI.Clients.ApiResponse <br/>
> create_failure_handler(_api::DefaultApi, response_stream::Channel, body::FailureHandlerModel; _mediaType=nothing) -> Channel{ FailureHandlerModel }, OpenAPI.Clients.ApiResponse

Create a failure handler.

Create a failure handler with rules for automatic job retry.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**body** | [**FailureHandlerModel**](FailureHandlerModel.md) | Failure handler to create |

### Return type

[**FailureHandlerModel**](FailureHandlerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_file**
> create_file(_api::DefaultApi, body::FileModel; _mediaType=nothing) -> FileModel, OpenAPI.Clients.ApiResponse <br/>
> create_file(_api::DefaultApi, response_stream::Channel, body::FileModel; _mediaType=nothing) -> Channel{ FileModel }, OpenAPI.Clients.ApiResponse

Store a file.

Store a file.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**body** | [**FileModel**](FileModel.md) | file. |

### Return type

[**FileModel**](FileModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_job**
> create_job(_api::DefaultApi, body::JobModel; _mediaType=nothing) -> JobModel, OpenAPI.Clients.ApiResponse <br/>
> create_job(_api::DefaultApi, response_stream::Channel, body::JobModel; _mediaType=nothing) -> Channel{ JobModel }, OpenAPI.Clients.ApiResponse

Store a job.

Store a job.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**body** | [**JobModel**](JobModel.md) | Job body |

### Return type

[**JobModel**](JobModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_jobs**
> create_jobs(_api::DefaultApi, body::JobsModel; _mediaType=nothing) -> CreateJobsResponse, OpenAPI.Clients.ApiResponse <br/>
> create_jobs(_api::DefaultApi, response_stream::Channel, body::JobsModel; _mediaType=nothing) -> Channel{ CreateJobsResponse }, OpenAPI.Clients.ApiResponse

Create jobs in bulk. Recommended max job count of 10,000.

Create jobs in bulk. Recommended max job count of 10,000.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**body** | [**JobsModel**](JobsModel.md) |  |

### Return type

[**CreateJobsResponse**](CreateJobsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_local_scheduler**
> create_local_scheduler(_api::DefaultApi, body::LocalSchedulerModel; _mediaType=nothing) -> LocalSchedulerModel, OpenAPI.Clients.ApiResponse <br/>
> create_local_scheduler(_api::DefaultApi, response_stream::Channel, body::LocalSchedulerModel; _mediaType=nothing) -> Channel{ LocalSchedulerModel }, OpenAPI.Clients.ApiResponse

Store a local scheduler.

Store a local scheduler. table.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**body** | [**LocalSchedulerModel**](LocalSchedulerModel.md) | local compute node configuration. |

### Return type

[**LocalSchedulerModel**](LocalSchedulerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_remote_workers**
> create_remote_workers(_api::DefaultApi, id::Int64, workers::Vector{String}; _mediaType=nothing) -> Vector{RemoteWorkerModel}, OpenAPI.Clients.ApiResponse <br/>
> create_remote_workers(_api::DefaultApi, response_stream::Channel, id::Int64, workers::Vector{String}; _mediaType=nothing) -> Channel{ Vector{RemoteWorkerModel} }, OpenAPI.Clients.ApiResponse

Store remote workers for a workflow.

Store remote workers for a workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |
**workers** | [**Vector{String}**](String.md) | List of remote workers to add |

### Return type

[**Vector{RemoteWorkerModel}**](RemoteWorkerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_resource_requirements**
> create_resource_requirements(_api::DefaultApi, body::ResourceRequirementsModel; _mediaType=nothing) -> ResourceRequirementsModel, OpenAPI.Clients.ApiResponse <br/>
> create_resource_requirements(_api::DefaultApi, response_stream::Channel, body::ResourceRequirementsModel; _mediaType=nothing) -> Channel{ ResourceRequirementsModel }, OpenAPI.Clients.ApiResponse

Store one resource requirements record.

Store one resource requirements definition.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**body** | [**ResourceRequirementsModel**](ResourceRequirementsModel.md) | resource requirements. |

### Return type

[**ResourceRequirementsModel**](ResourceRequirementsModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_result**
> create_result(_api::DefaultApi, body::ResultModel; _mediaType=nothing) -> ResultModel, OpenAPI.Clients.ApiResponse <br/>
> create_result(_api::DefaultApi, response_stream::Channel, body::ResultModel; _mediaType=nothing) -> Channel{ ResultModel }, OpenAPI.Clients.ApiResponse

Store a job result.

Store a job result.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**body** | [**ResultModel**](ResultModel.md) | result. |

### Return type

[**ResultModel**](ResultModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_ro_crate_entity**
> create_ro_crate_entity(_api::DefaultApi, body::RoCrateEntityModel; _mediaType=nothing) -> RoCrateEntityModel, OpenAPI.Clients.ApiResponse <br/>
> create_ro_crate_entity(_api::DefaultApi, response_stream::Channel, body::RoCrateEntityModel; _mediaType=nothing) -> Channel{ RoCrateEntityModel }, OpenAPI.Clients.ApiResponse

Create a new RO-Crate entity.

Create a new RO-Crate entity.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**body** | [**RoCrateEntityModel**](RoCrateEntityModel.md) | RO-Crate entity to create |

### Return type

[**RoCrateEntityModel**](RoCrateEntityModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_scheduled_compute_node**
> create_scheduled_compute_node(_api::DefaultApi, body::ScheduledComputeNodesModel; _mediaType=nothing) -> ScheduledComputeNodesModel, OpenAPI.Clients.ApiResponse <br/>
> create_scheduled_compute_node(_api::DefaultApi, response_stream::Channel, body::ScheduledComputeNodesModel; _mediaType=nothing) -> Channel{ ScheduledComputeNodesModel }, OpenAPI.Clients.ApiResponse

Store a scheduled compute node.

Store a scheduled compute node.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**body** | [**ScheduledComputeNodesModel**](ScheduledComputeNodesModel.md) | scheduled compute node. |

### Return type

[**ScheduledComputeNodesModel**](ScheduledComputeNodesModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_slurm_scheduler**
> create_slurm_scheduler(_api::DefaultApi, body::SlurmSchedulerModel; _mediaType=nothing) -> SlurmSchedulerModel, OpenAPI.Clients.ApiResponse <br/>
> create_slurm_scheduler(_api::DefaultApi, response_stream::Channel, body::SlurmSchedulerModel; _mediaType=nothing) -> Channel{ SlurmSchedulerModel }, OpenAPI.Clients.ApiResponse

Store a Slurm compute node configuration.

Store a Slurm compute node configuration.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**body** | [**SlurmSchedulerModel**](SlurmSchedulerModel.md) | Slurm compute node configuration. |

### Return type

[**SlurmSchedulerModel**](SlurmSchedulerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_slurm_stats**
> create_slurm_stats(_api::DefaultApi, body::SlurmStatsModel; _mediaType=nothing) -> SlurmStatsModel, OpenAPI.Clients.ApiResponse <br/>
> create_slurm_stats(_api::DefaultApi, response_stream::Channel, body::SlurmStatsModel; _mediaType=nothing) -> Channel{ SlurmStatsModel }, OpenAPI.Clients.ApiResponse

Store Slurm accounting stats for a job step.

Store Slurm accounting stats collected via sacct for a job step.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**body** | [**SlurmStatsModel**](SlurmStatsModel.md) | Slurm stats record. |

### Return type

[**SlurmStatsModel**](SlurmStatsModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_user_data**
> create_user_data(_api::DefaultApi, body::UserDataModel; consumer_job_id=nothing, producer_job_id=nothing, _mediaType=nothing) -> UserDataModel, OpenAPI.Clients.ApiResponse <br/>
> create_user_data(_api::DefaultApi, response_stream::Channel, body::UserDataModel; consumer_job_id=nothing, producer_job_id=nothing, _mediaType=nothing) -> Channel{ UserDataModel }, OpenAPI.Clients.ApiResponse

Store a user data record.

Store a user data record.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**body** | [**UserDataModel**](UserDataModel.md) | user data. |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **consumer_job_id** | **Int64** | ID of the job that consumes this user data. | [default to nothing]
 **producer_job_id** | **Int64** | ID of the job that produces this user data. | [default to nothing]

### Return type

[**UserDataModel**](UserDataModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_workflow**
> create_workflow(_api::DefaultApi, body::WorkflowModel; _mediaType=nothing) -> WorkflowModel, OpenAPI.Clients.ApiResponse <br/>
> create_workflow(_api::DefaultApi, response_stream::Channel, body::WorkflowModel; _mediaType=nothing) -> Channel{ WorkflowModel }, OpenAPI.Clients.ApiResponse

Store a workflow.

Store a workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**body** | [**WorkflowModel**](WorkflowModel.md) | Workflow attributes |

### Return type

[**WorkflowModel**](WorkflowModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **create_workflow_action**
> create_workflow_action(_api::DefaultApi, id::Int64, body::WorkflowActionModel; _mediaType=nothing) -> WorkflowActionModel, OpenAPI.Clients.ApiResponse <br/>
> create_workflow_action(_api::DefaultApi, response_stream::Channel, id::Int64, body::WorkflowActionModel; _mediaType=nothing) -> Channel{ WorkflowActionModel }, OpenAPI.Clients.ApiResponse

Create a workflow action.

Create a workflow action.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |
**body** | [**WorkflowActionModel**](WorkflowActionModel.md) | Workflow action to create |

### Return type

[**WorkflowActionModel**](WorkflowActionModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_access_group**
> delete_access_group(_api::DefaultApi, id::Int64; body=nothing, _mediaType=nothing) -> AccessGroupModel, OpenAPI.Clients.ApiResponse <br/>
> delete_access_group(_api::DefaultApi, response_stream::Channel, id::Int64; body=nothing, _mediaType=nothing) -> Channel{ AccessGroupModel }, OpenAPI.Clients.ApiResponse

Delete an access group.

Delete an access group.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the access group |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

[**AccessGroupModel**](AccessGroupModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_all_user_data**
> delete_all_user_data(_api::DefaultApi, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> delete_all_user_data(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse

Delete all user data records for one workflow.

Delete all user data records for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_compute_node**
> delete_compute_node(_api::DefaultApi, id::Int64; body=nothing, _mediaType=nothing) -> ComputeNodeModel, OpenAPI.Clients.ApiResponse <br/>
> delete_compute_node(_api::DefaultApi, response_stream::Channel, id::Int64; body=nothing, _mediaType=nothing) -> Channel{ ComputeNodeModel }, OpenAPI.Clients.ApiResponse

Delete a compute node.

Delete a compute node.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the compute node |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

[**ComputeNodeModel**](ComputeNodeModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_compute_nodes**
> delete_compute_nodes(_api::DefaultApi, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> delete_compute_nodes(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse

Delete all compute node records for one workflow.

Delete all compute node records for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_event**
> delete_event(_api::DefaultApi, id::Int64; body=nothing, _mediaType=nothing) -> EventModel, OpenAPI.Clients.ApiResponse <br/>
> delete_event(_api::DefaultApi, response_stream::Channel, id::Int64; body=nothing, _mediaType=nothing) -> Channel{ EventModel }, OpenAPI.Clients.ApiResponse

Delete an event.

Deletes an event.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the event record. |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

[**EventModel**](EventModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_events**
> delete_events(_api::DefaultApi, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> delete_events(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse

Delete all events for one workflow.

Delete all events for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_failure_handler**
> delete_failure_handler(_api::DefaultApi, id::Int64; body=nothing, _mediaType=nothing) -> FailureHandlerModel, OpenAPI.Clients.ApiResponse <br/>
> delete_failure_handler(_api::DefaultApi, response_stream::Channel, id::Int64; body=nothing, _mediaType=nothing) -> Channel{ FailureHandlerModel }, OpenAPI.Clients.ApiResponse

Delete a failure handler.

Delete a failure handler.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
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

# **delete_file**
> delete_file(_api::DefaultApi, id::Int64; body=nothing, _mediaType=nothing) -> FileModel, OpenAPI.Clients.ApiResponse <br/>
> delete_file(_api::DefaultApi, response_stream::Channel, id::Int64; body=nothing, _mediaType=nothing) -> Channel{ FileModel }, OpenAPI.Clients.ApiResponse

Delete a file.

Delete a file.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the file record. |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

[**FileModel**](FileModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_files**
> delete_files(_api::DefaultApi, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> delete_files(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse

Delete all files for one workflow.

Delete all files for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_job**
> delete_job(_api::DefaultApi, id::Int64; body=nothing, _mediaType=nothing) -> JobModel, OpenAPI.Clients.ApiResponse <br/>
> delete_job(_api::DefaultApi, response_stream::Channel, id::Int64; body=nothing, _mediaType=nothing) -> Channel{ JobModel }, OpenAPI.Clients.ApiResponse

Delete a job.

Delete a job.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Job ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

[**JobModel**](JobModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_jobs**
> delete_jobs(_api::DefaultApi, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> delete_jobs(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse

Delete all jobs for one workflow.

Delete all jobs for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_local_scheduler**
> delete_local_scheduler(_api::DefaultApi, id::Int64; body=nothing, _mediaType=nothing) -> LocalSchedulerModel, OpenAPI.Clients.ApiResponse <br/>
> delete_local_scheduler(_api::DefaultApi, response_stream::Channel, id::Int64; body=nothing, _mediaType=nothing) -> Channel{ LocalSchedulerModel }, OpenAPI.Clients.ApiResponse

Delete a local scheduler.

Delete a local scheduler.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the local compute node configuration record. |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

[**LocalSchedulerModel**](LocalSchedulerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_local_schedulers**
> delete_local_schedulers(_api::DefaultApi, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> delete_local_schedulers(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse

Delete all local schedulers for one workflow.

Delete all local schedulers for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_remote_worker**
> delete_remote_worker(_api::DefaultApi, id::Int64, worker::String; _mediaType=nothing) -> RemoteWorkerModel, OpenAPI.Clients.ApiResponse <br/>
> delete_remote_worker(_api::DefaultApi, response_stream::Channel, id::Int64, worker::String; _mediaType=nothing) -> Channel{ RemoteWorkerModel }, OpenAPI.Clients.ApiResponse

Delete a remote worker from a workflow.

Delete a remote worker from a workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |
**worker** | **String** | Worker address (URL-encoded) |

### Return type

[**RemoteWorkerModel**](RemoteWorkerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_resource_requirement**
> delete_resource_requirement(_api::DefaultApi, id::Int64; body=nothing, _mediaType=nothing) -> ResourceRequirementsModel, OpenAPI.Clients.ApiResponse <br/>
> delete_resource_requirement(_api::DefaultApi, response_stream::Channel, id::Int64; body=nothing, _mediaType=nothing) -> Channel{ ResourceRequirementsModel }, OpenAPI.Clients.ApiResponse

Delete a resource requirements record.

Delete a resource requirements record.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
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
> delete_resource_requirements(_api::DefaultApi, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> delete_resource_requirements(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse

Delete all resource requirements records for one workflow.

Delete all resource requirements records for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_result**
> delete_result(_api::DefaultApi, id::Int64; body=nothing, _mediaType=nothing) -> ResultModel, OpenAPI.Clients.ApiResponse <br/>
> delete_result(_api::DefaultApi, response_stream::Channel, id::Int64; body=nothing, _mediaType=nothing) -> Channel{ ResultModel }, OpenAPI.Clients.ApiResponse

Delete a job result.

Delete a job result.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Results ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

[**ResultModel**](ResultModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_results**
> delete_results(_api::DefaultApi, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> delete_results(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse

Delete all job results for one workflow.

Delete all job results for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_ro_crate_entities**
> delete_ro_crate_entities(_api::DefaultApi, id::Int64; body=nothing, _mediaType=nothing) -> DeleteRoCrateEntities200Response, OpenAPI.Clients.ApiResponse <br/>
> delete_ro_crate_entities(_api::DefaultApi, response_stream::Channel, id::Int64; body=nothing, _mediaType=nothing) -> Channel{ DeleteRoCrateEntities200Response }, OpenAPI.Clients.ApiResponse

Delete all RO-Crate entities for a workflow.

Delete all RO-Crate entities for a workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** |  |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** | Optional request body (ignored) | 

### Return type

[**DeleteRoCrateEntities200Response**](DeleteRoCrateEntities200Response.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_ro_crate_entity**
> delete_ro_crate_entity(_api::DefaultApi, id::Int64; body=nothing, _mediaType=nothing) -> DeleteRoCrateEntity200Response, OpenAPI.Clients.ApiResponse <br/>
> delete_ro_crate_entity(_api::DefaultApi, response_stream::Channel, id::Int64; body=nothing, _mediaType=nothing) -> Channel{ DeleteRoCrateEntity200Response }, OpenAPI.Clients.ApiResponse

Delete an RO-Crate entity.

Delete an RO-Crate entity.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** |  |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** | Optional request body (ignored) | 

### Return type

[**DeleteRoCrateEntity200Response**](DeleteRoCrateEntity200Response.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_scheduled_compute_node**
> delete_scheduled_compute_node(_api::DefaultApi, id::Int64; body=nothing, _mediaType=nothing) -> ScheduledComputeNodesModel, OpenAPI.Clients.ApiResponse <br/>
> delete_scheduled_compute_node(_api::DefaultApi, response_stream::Channel, id::Int64; body=nothing, _mediaType=nothing) -> Channel{ ScheduledComputeNodesModel }, OpenAPI.Clients.ApiResponse

Delete a scheduled compute node.

Delete a scheduled compute node.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Scheduled compute node ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

[**ScheduledComputeNodesModel**](ScheduledComputeNodesModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_scheduled_compute_nodes**
> delete_scheduled_compute_nodes(_api::DefaultApi, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> delete_scheduled_compute_nodes(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse

Delete all scheduled compute node records for one workflow.

Delete all scheduled compute node records for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_slurm_scheduler**
> delete_slurm_scheduler(_api::DefaultApi, id::Int64; body=nothing, _mediaType=nothing) -> SlurmSchedulerModel, OpenAPI.Clients.ApiResponse <br/>
> delete_slurm_scheduler(_api::DefaultApi, response_stream::Channel, id::Int64; body=nothing, _mediaType=nothing) -> Channel{ SlurmSchedulerModel }, OpenAPI.Clients.ApiResponse

Delete Slurm compute node configuration.

Delete Slurm compute node configuration.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Slurm compute node configuration ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

[**SlurmSchedulerModel**](SlurmSchedulerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_slurm_schedulers**
> delete_slurm_schedulers(_api::DefaultApi, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> delete_slurm_schedulers(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; body=nothing, _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse

Retrieve all Slurm compute node configurations for one workflow.

Retrieve all Slurm compute node configurations for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_user_data**
> delete_user_data(_api::DefaultApi, id::Int64; body=nothing, _mediaType=nothing) -> UserDataModel, OpenAPI.Clients.ApiResponse <br/>
> delete_user_data(_api::DefaultApi, response_stream::Channel, id::Int64; body=nothing, _mediaType=nothing) -> Channel{ UserDataModel }, OpenAPI.Clients.ApiResponse

Delete a user data record.

Delete a user data record.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | User data record ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

[**UserDataModel**](UserDataModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_workflow**
> delete_workflow(_api::DefaultApi, id::Int64; body=nothing, _mediaType=nothing) -> WorkflowModel, OpenAPI.Clients.ApiResponse <br/>
> delete_workflow(_api::DefaultApi, response_stream::Channel, id::Int64; body=nothing, _mediaType=nothing) -> Channel{ WorkflowModel }, OpenAPI.Clients.ApiResponse

Delete a workflow.

Delete a workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID. |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

[**WorkflowModel**](WorkflowModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_access_group**
> get_access_group(_api::DefaultApi, id::Int64; _mediaType=nothing) -> AccessGroupModel, OpenAPI.Clients.ApiResponse <br/>
> get_access_group(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ AccessGroupModel }, OpenAPI.Clients.ApiResponse

Get an access group by ID.

Get an access group by ID.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the access group |

### Return type

[**AccessGroupModel**](AccessGroupModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_compute_node**
> get_compute_node(_api::DefaultApi, id::Int64; _mediaType=nothing) -> ComputeNodeModel, OpenAPI.Clients.ApiResponse <br/>
> get_compute_node(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ComputeNodeModel }, OpenAPI.Clients.ApiResponse

Retrieve a compute node by ID.

Retrieve a compute node by ID.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the compute node record |

### Return type

[**ComputeNodeModel**](ComputeNodeModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_event**
> get_event(_api::DefaultApi, id::Int64; _mediaType=nothing) -> EventModel, OpenAPI.Clients.ApiResponse <br/>
> get_event(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ EventModel }, OpenAPI.Clients.ApiResponse

Retrieve an event by ID.

Retrieve an event by ID.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the events record |

### Return type

[**EventModel**](EventModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_failure_handler**
> get_failure_handler(_api::DefaultApi, id::Int64; _mediaType=nothing) -> FailureHandlerModel, OpenAPI.Clients.ApiResponse <br/>
> get_failure_handler(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ FailureHandlerModel }, OpenAPI.Clients.ApiResponse

Get a failure handler by ID.

Retrieve a failure handler by ID.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Failure handler ID |

### Return type

[**FailureHandlerModel**](FailureHandlerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_file**
> get_file(_api::DefaultApi, id::Int64; _mediaType=nothing) -> FileModel, OpenAPI.Clients.ApiResponse <br/>
> get_file(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ FileModel }, OpenAPI.Clients.ApiResponse

Retrieve a file.

Retrieve a file.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the files record |

### Return type

[**FileModel**](FileModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_job**
> get_job(_api::DefaultApi, id::Int64; _mediaType=nothing) -> JobModel, OpenAPI.Clients.ApiResponse <br/>
> get_job(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ JobModel }, OpenAPI.Clients.ApiResponse

Retrieve a job.

Retrieve a job.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the job record |

### Return type

[**JobModel**](JobModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_latest_event_timestamp**
> get_latest_event_timestamp(_api::DefaultApi, id::Int64; _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> get_latest_event_timestamp(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse

Return the timestamp of the latest event in ms since the epoch in UTC.

Return the timestamp of the latest event in ms since the epoch in UTC.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_local_scheduler**
> get_local_scheduler(_api::DefaultApi, id::Int64; _mediaType=nothing) -> LocalSchedulerModel, OpenAPI.Clients.ApiResponse <br/>
> get_local_scheduler(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ LocalSchedulerModel }, OpenAPI.Clients.ApiResponse

Retrieve a local scheduler.

Retrieve a local scheduler.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Scheduler ID |

### Return type

[**LocalSchedulerModel**](LocalSchedulerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_pending_actions**
> get_pending_actions(_api::DefaultApi, id::Int64; trigger_type=nothing, _mediaType=nothing) -> Vector{WorkflowActionModel}, OpenAPI.Clients.ApiResponse <br/>
> get_pending_actions(_api::DefaultApi, response_stream::Channel, id::Int64; trigger_type=nothing, _mediaType=nothing) -> Channel{ Vector{WorkflowActionModel} }, OpenAPI.Clients.ApiResponse

Get pending workflow actions, optionally filtered by trigger type.

Get pending (unexecuted) workflow actions for a workflow, optionally filtered by trigger type.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **trigger_type** | [**Vector{String}**](String.md) | Filter by trigger type (can be specified multiple times) | [default to nothing]

### Return type

[**Vector{WorkflowActionModel}**](WorkflowActionModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_ready_job_requirements**
> get_ready_job_requirements(_api::DefaultApi, id::Int64; scheduler_config_id=nothing, _mediaType=nothing) -> GetReadyJobRequirementsResponse, OpenAPI.Clients.ApiResponse <br/>
> get_ready_job_requirements(_api::DefaultApi, response_stream::Channel, id::Int64; scheduler_config_id=nothing, _mediaType=nothing) -> Channel{ GetReadyJobRequirementsResponse }, OpenAPI.Clients.ApiResponse

Return the resource requirements for jobs with a status of ready.

Return the resource requirements for jobs with a status of ready.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **scheduler_config_id** | **Int64** | Limit output to jobs assigned this scheduler. | [default to nothing]

### Return type

[**GetReadyJobRequirementsResponse**](GetReadyJobRequirementsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_resource_requirements**
> get_resource_requirements(_api::DefaultApi, id::Int64; _mediaType=nothing) -> ResourceRequirementsModel, OpenAPI.Clients.ApiResponse <br/>
> get_resource_requirements(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ResourceRequirementsModel }, OpenAPI.Clients.ApiResponse

Retrieve one resource requirements record.

Retrieve one resource requirements record.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Resource requirements ID |

### Return type

[**ResourceRequirementsModel**](ResourceRequirementsModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_result**
> get_result(_api::DefaultApi, id::Int64; _mediaType=nothing) -> ResultModel, OpenAPI.Clients.ApiResponse <br/>
> get_result(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ResultModel }, OpenAPI.Clients.ApiResponse

Retrieve a job result.

Retrieve a job result.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Results ID |

### Return type

[**ResultModel**](ResultModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_ro_crate_entity**
> get_ro_crate_entity(_api::DefaultApi, id::Int64; _mediaType=nothing) -> RoCrateEntityModel, OpenAPI.Clients.ApiResponse <br/>
> get_ro_crate_entity(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ RoCrateEntityModel }, OpenAPI.Clients.ApiResponse

Get an RO-Crate entity by ID.

Get an RO-Crate entity by ID.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** |  |

### Return type

[**RoCrateEntityModel**](RoCrateEntityModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_scheduled_compute_node**
> get_scheduled_compute_node(_api::DefaultApi, id::Int64; _mediaType=nothing) -> ScheduledComputeNodesModel, OpenAPI.Clients.ApiResponse <br/>
> get_scheduled_compute_node(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ScheduledComputeNodesModel }, OpenAPI.Clients.ApiResponse

Retrieve a scheduled compute node.

Retrieve a scheduled compute node.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the scheduled_compute_nodes record |

### Return type

[**ScheduledComputeNodesModel**](ScheduledComputeNodesModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_slurm_scheduler**
> get_slurm_scheduler(_api::DefaultApi, id::Int64; _mediaType=nothing) -> SlurmSchedulerModel, OpenAPI.Clients.ApiResponse <br/>
> get_slurm_scheduler(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ SlurmSchedulerModel }, OpenAPI.Clients.ApiResponse

Retrieve a Slurm compute node configuration.

Retrieve a Slurm compute node configuration.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Slurm compute node configuration ID |

### Return type

[**SlurmSchedulerModel**](SlurmSchedulerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_user_data**
> get_user_data(_api::DefaultApi, id::Int64; _mediaType=nothing) -> UserDataModel, OpenAPI.Clients.ApiResponse <br/>
> get_user_data(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ UserDataModel }, OpenAPI.Clients.ApiResponse

Retrieve a user data record.

Retrieve a user data record.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | User data record ID |

### Return type

[**UserDataModel**](UserDataModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_version**
> get_version(_api::DefaultApi; _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> get_version(_api::DefaultApi, response_stream::Channel; _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse

Return the version of the service.

Return the version of the service.

### Required Parameters
This endpoint does not need any parameter.

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_workflow**
> get_workflow(_api::DefaultApi, id::Int64; _mediaType=nothing) -> WorkflowModel, OpenAPI.Clients.ApiResponse <br/>
> get_workflow(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ WorkflowModel }, OpenAPI.Clients.ApiResponse

Retrieve a workflow.

Retrieve a workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the workflows record |

### Return type

[**WorkflowModel**](WorkflowModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_workflow_actions**
> get_workflow_actions(_api::DefaultApi, id::Int64; _mediaType=nothing) -> Vector{WorkflowActionModel}, OpenAPI.Clients.ApiResponse <br/>
> get_workflow_actions(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ Vector{WorkflowActionModel} }, OpenAPI.Clients.ApiResponse

Get all workflow actions for a workflow.

Get all workflow actions for a workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |

### Return type

[**Vector{WorkflowActionModel}**](WorkflowActionModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_workflow_status**
> get_workflow_status(_api::DefaultApi, id::Int64; _mediaType=nothing) -> WorkflowStatusModel, OpenAPI.Clients.ApiResponse <br/>
> get_workflow_status(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ WorkflowStatusModel }, OpenAPI.Clients.ApiResponse

Return the workflow status.

Return the workflow status.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
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
> initialize_jobs(_api::DefaultApi, id::Int64; only_uninitialized=nothing, clear_ephemeral_user_data=nothing, body=nothing, _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> initialize_jobs(_api::DefaultApi, response_stream::Channel, id::Int64; only_uninitialized=nothing, clear_ephemeral_user_data=nothing, body=nothing, _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse

Initialize job relationships based on file and user_data relationships.

Initialize job relationships based on file and user_data relationships.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **only_uninitialized** | **Bool** | Only initialize jobs with a status of uninitialized. | [default to false]
 **clear_ephemeral_user_data** | **Bool** | Clear all ephemeral user data. | [default to true]
 **body** | **Any** |  | 

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **is_workflow_complete**
> is_workflow_complete(_api::DefaultApi, id::Int64; _mediaType=nothing) -> IsCompleteResponse, OpenAPI.Clients.ApiResponse <br/>
> is_workflow_complete(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ IsCompleteResponse }, OpenAPI.Clients.ApiResponse

Return true if all jobs in the workflow are complete.

Return true if all jobs in the workflow are complete.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
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
> is_workflow_uninitialized(_api::DefaultApi, id::Int64; _mediaType=nothing) -> IsUninitializedResponse, OpenAPI.Clients.ApiResponse <br/>
> is_workflow_uninitialized(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ IsUninitializedResponse }, OpenAPI.Clients.ApiResponse

Return true if all jobs in the workflow are uninitialized or disabled.

Return true if all jobs in the workflow are uninitialized or disabled.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |

### Return type

[**IsUninitializedResponse**](IsUninitializedResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_access_groups**
> list_access_groups(_api::DefaultApi; offset=nothing, limit=nothing, _mediaType=nothing) -> ListAccessGroupsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_access_groups(_api::DefaultApi, response_stream::Channel; offset=nothing, limit=nothing, _mediaType=nothing) -> Channel{ ListAccessGroupsResponse }, OpenAPI.Clients.ApiResponse

List all access groups.

List all access groups.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to 0]
 **limit** | **Int64** |  | [default to 100]

### Return type

[**ListAccessGroupsResponse**](ListAccessGroupsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_compute_nodes**
> list_compute_nodes(_api::DefaultApi, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, hostname=nothing, is_active=nothing, scheduled_compute_node_id=nothing, _mediaType=nothing) -> ListComputeNodesResponse, OpenAPI.Clients.ApiResponse <br/>
> list_compute_nodes(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, hostname=nothing, is_active=nothing, scheduled_compute_node_id=nothing, _mediaType=nothing) -> Channel{ ListComputeNodesResponse }, OpenAPI.Clients.ApiResponse

Retrieve all compute node records for one workflow.

Retrieve all compute node records for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to 0]
 **limit** | **Int64** |  | [default to 100000]
 **sort_by** | **String** |  | [default to &quot;null&quot;]
 **reverse_sort** | **Bool** |  | [default to false]
 **hostname** | **String** |  | [default to nothing]
 **is_active** | **Bool** |  | [default to nothing]
 **scheduled_compute_node_id** | **Int64** | Filter by scheduled compute node ID (filters compute nodes created by this scheduler) | [default to nothing]

### Return type

[**ListComputeNodesResponse**](ListComputeNodesResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_events**
> list_events(_api::DefaultApi, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, category=nothing, after_timestamp=nothing, _mediaType=nothing) -> ListEventsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_events(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, category=nothing, after_timestamp=nothing, _mediaType=nothing) -> Channel{ ListEventsResponse }, OpenAPI.Clients.ApiResponse

Retrieve all events for one workflow.

Retrieve all events for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to 0]
 **limit** | **Int64** |  | [default to 100000]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to false]
 **category** | **String** |  | [default to nothing]
 **after_timestamp** | **Int64** | Return events after this timestamp (milliseconds since epoch) | [default to nothing]

### Return type

[**ListEventsResponse**](ListEventsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_failure_handlers**
> list_failure_handlers(_api::DefaultApi, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> ListFailureHandlersResponse, OpenAPI.Clients.ApiResponse <br/>
> list_failure_handlers(_api::DefaultApi, response_stream::Channel, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> Channel{ ListFailureHandlersResponse }, OpenAPI.Clients.ApiResponse

List failure handlers for a workflow.

List all failure handlers for a workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to 0]
 **limit** | **Int64** |  | [default to 10000]

### Return type

[**ListFailureHandlersResponse**](ListFailureHandlersResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_files**
> list_files(_api::DefaultApi, workflow_id::Int64; produced_by_job_id=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, name=nothing, path=nothing, is_output=nothing, _mediaType=nothing) -> ListFilesResponse, OpenAPI.Clients.ApiResponse <br/>
> list_files(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; produced_by_job_id=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, name=nothing, path=nothing, is_output=nothing, _mediaType=nothing) -> Channel{ ListFilesResponse }, OpenAPI.Clients.ApiResponse

Retrieve all files for one workflow.

Retrieve all files for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **produced_by_job_id** | **Int64** | Return files produced by a specific job. | [default to nothing]
 **offset** | **Int64** |  | [default to 0]
 **limit** | **Int64** |  | [default to 100000]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to false]
 **name** | **String** |  | [default to nothing]
 **path** | **String** |  | [default to nothing]
 **is_output** | **Bool** | Filter for files that are outputs of jobs (appear in job_output_file table) | [default to nothing]

### Return type

[**ListFilesResponse**](ListFilesResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_group_members**
> list_group_members(_api::DefaultApi, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> ListUserGroupMembershipsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_group_members(_api::DefaultApi, response_stream::Channel, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> Channel{ ListUserGroupMembershipsResponse }, OpenAPI.Clients.ApiResponse

List members of an access group.

List members of an access group.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the access group |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to 0]
 **limit** | **Int64** |  | [default to 100]

### Return type

[**ListUserGroupMembershipsResponse**](ListUserGroupMembershipsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_job_dependencies**
> list_job_dependencies(_api::DefaultApi, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> ListJobDependenciesResponse, OpenAPI.Clients.ApiResponse <br/>
> list_job_dependencies(_api::DefaultApi, response_stream::Channel, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> Channel{ ListJobDependenciesResponse }, OpenAPI.Clients.ApiResponse

Retrieve job blocking relationships for a workflow.

Retrieve all job blocking relationships for one workflow from the job_depends_on table.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to 0]
 **limit** | **Int64** |  | [default to 100000]

### Return type

[**ListJobDependenciesResponse**](ListJobDependenciesResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_job_file_relationships**
> list_job_file_relationships(_api::DefaultApi, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> ListJobFileRelationshipsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_job_file_relationships(_api::DefaultApi, response_stream::Channel, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> Channel{ ListJobFileRelationshipsResponse }, OpenAPI.Clients.ApiResponse

Retrieve job-file relationships for a workflow.

Retrieve all job-file relationships for one workflow from the job_input_file and job_output_file tables.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to 0]
 **limit** | **Int64** |  | [default to 100000]

### Return type

[**ListJobFileRelationshipsResponse**](ListJobFileRelationshipsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_job_ids**
> list_job_ids(_api::DefaultApi, id::Int64; _mediaType=nothing) -> ListJobIdsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_job_ids(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ListJobIdsResponse }, OpenAPI.Clients.ApiResponse

Retrieve all job IDs for one workflow.

Retrieve all job IDs for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
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
> list_job_user_data_relationships(_api::DefaultApi, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> ListJobUserDataRelationshipsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_job_user_data_relationships(_api::DefaultApi, response_stream::Channel, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> Channel{ ListJobUserDataRelationshipsResponse }, OpenAPI.Clients.ApiResponse

Retrieve job-user_data relationships for a workflow.

Retrieve all job-user_data relationships for one workflow from the job_input_user_data and job_output_user_data tables.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to 0]
 **limit** | **Int64** |  | [default to 100000]

### Return type

[**ListJobUserDataRelationshipsResponse**](ListJobUserDataRelationshipsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_jobs**
> list_jobs(_api::DefaultApi, workflow_id::Int64; status=nothing, needs_file_id=nothing, upstream_job_id=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, include_relationships=nothing, active_compute_node_id=nothing, _mediaType=nothing) -> ListJobsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_jobs(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; status=nothing, needs_file_id=nothing, upstream_job_id=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, include_relationships=nothing, active_compute_node_id=nothing, _mediaType=nothing) -> Channel{ ListJobsResponse }, OpenAPI.Clients.ApiResponse

Retrieve all jobs for one workflow.

Retrieve all jobs for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **status** | [**JobStatus**](.md) | Return jobs with this status. | [default to nothing]
 **needs_file_id** | **Int64** | Return jobs that need this file as an input. | [default to nothing]
 **upstream_job_id** | **Int64** | Return jobs that are downstream of this job ID in the workflow graph. | [default to nothing]
 **offset** | **Int64** |  | [default to 0]
 **limit** | **Int64** |  | [default to 100000]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to false]
 **include_relationships** | **Bool** | Include job relationships (depends_on_job_ids, input_file_ids, output_file_ids, input_user_data_ids, output_user_data_ids). Default is false for performance. | [default to false]
 **active_compute_node_id** | **Int64** | Filter jobs by the compute node currently running them. | [default to nothing]

### Return type

[**ListJobsResponse**](ListJobsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_local_schedulers**
> list_local_schedulers(_api::DefaultApi, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, memory=nothing, num_cpus=nothing, _mediaType=nothing) -> ListLocalSchedulersResponse, OpenAPI.Clients.ApiResponse <br/>
> list_local_schedulers(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, memory=nothing, num_cpus=nothing, _mediaType=nothing) -> Channel{ ListLocalSchedulersResponse }, OpenAPI.Clients.ApiResponse

Retrieve local schedulers for one workflow.

Retrieve local schedulers for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to 0]
 **limit** | **Int64** |  | [default to 100000]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to false]
 **memory** | **String** |  | [default to nothing]
 **num_cpus** | **Int64** |  | [default to nothing]

### Return type

[**ListLocalSchedulersResponse**](ListLocalSchedulersResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_missing_user_data**
> list_missing_user_data(_api::DefaultApi, id::Int64; _mediaType=nothing) -> ListMissingUserDataResponse, OpenAPI.Clients.ApiResponse <br/>
> list_missing_user_data(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ListMissingUserDataResponse }, OpenAPI.Clients.ApiResponse

List missing user data that should exist.

List missing user data that should exist.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |

### Return type

[**ListMissingUserDataResponse**](ListMissingUserDataResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_remote_workers**
> list_remote_workers(_api::DefaultApi, id::Int64; _mediaType=nothing) -> Vector{RemoteWorkerModel}, OpenAPI.Clients.ApiResponse <br/>
> list_remote_workers(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ Vector{RemoteWorkerModel} }, OpenAPI.Clients.ApiResponse

List all remote workers for a workflow.

List all remote workers for a workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |

### Return type

[**Vector{RemoteWorkerModel}**](RemoteWorkerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_required_existing_files**
> list_required_existing_files(_api::DefaultApi, id::Int64; _mediaType=nothing) -> ListRequiredExistingFilesResponse, OpenAPI.Clients.ApiResponse <br/>
> list_required_existing_files(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ListRequiredExistingFilesResponse }, OpenAPI.Clients.ApiResponse

List files that must exist.

List files that must exist.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |

### Return type

[**ListRequiredExistingFilesResponse**](ListRequiredExistingFilesResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_resource_requirements**
> list_resource_requirements(_api::DefaultApi, workflow_id::Int64; job_id=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, name=nothing, memory=nothing, num_cpus=nothing, num_gpus=nothing, num_nodes=nothing, runtime=nothing, _mediaType=nothing) -> ListResourceRequirementsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_resource_requirements(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; job_id=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, name=nothing, memory=nothing, num_cpus=nothing, num_gpus=nothing, num_nodes=nothing, runtime=nothing, _mediaType=nothing) -> Channel{ ListResourceRequirementsResponse }, OpenAPI.Clients.ApiResponse

Retrieve all resource requirements records for one workflow.

Retrieve all resource requirements records for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **job_id** | **Int64** | Return the resource requirements for a specific job. | [default to nothing]
 **offset** | **Int64** |  | [default to 0]
 **limit** | **Int64** |  | [default to 100000]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to false]
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

# **list_results**
> list_results(_api::DefaultApi, workflow_id::Int64; job_id=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, run_id=nothing, return_code=nothing, status=nothing, all_runs=nothing, compute_node_id=nothing, _mediaType=nothing) -> ListResultsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_results(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; job_id=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, run_id=nothing, return_code=nothing, status=nothing, all_runs=nothing, compute_node_id=nothing, _mediaType=nothing) -> Channel{ ListResultsResponse }, OpenAPI.Clients.ApiResponse

Retrieve all job results for one workflow.

Retrieve all job results for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **job_id** | **Int64** | Return the results for a specific job. | [default to nothing]
 **offset** | **Int64** |  | [default to 0]
 **limit** | **Int64** |  | [default to 100000]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to false]
 **run_id** | **Int64** |  | [default to nothing]
 **return_code** | **Int64** |  | [default to nothing]
 **status** | [**JobStatus**](.md) |  | [default to nothing]
 **all_runs** | **Bool** | If false (default), only return results in the workflow_result table (current results). If true, return all historical results. | [default to false]
 **compute_node_id** | **Int64** | Filter by compute node ID | [default to nothing]

### Return type

[**ListResultsResponse**](ListResultsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_ro_crate_entities**
> list_ro_crate_entities(_api::DefaultApi, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> ListRoCrateEntitiesResponse, OpenAPI.Clients.ApiResponse <br/>
> list_ro_crate_entities(_api::DefaultApi, response_stream::Channel, id::Int64; offset=nothing, limit=nothing, _mediaType=nothing) -> Channel{ ListRoCrateEntitiesResponse }, OpenAPI.Clients.ApiResponse

List all RO-Crate entities for a workflow.

List all RO-Crate entities for a workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** |  |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to 0]
 **limit** | **Int64** |  | [default to 10000]

### Return type

[**ListRoCrateEntitiesResponse**](ListRoCrateEntitiesResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_scheduled_compute_nodes**
> list_scheduled_compute_nodes(_api::DefaultApi, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, scheduler_id=nothing, scheduler_config_id=nothing, status=nothing, _mediaType=nothing) -> ListScheduledComputeNodesResponse, OpenAPI.Clients.ApiResponse <br/>
> list_scheduled_compute_nodes(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, scheduler_id=nothing, scheduler_config_id=nothing, status=nothing, _mediaType=nothing) -> Channel{ ListScheduledComputeNodesResponse }, OpenAPI.Clients.ApiResponse

Retrieve scheduled compute node records for one workflow.

Retrieve scheduled compute node records for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to 0]
 **limit** | **Int64** |  | [default to 100000]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to false]
 **scheduler_id** | **String** |  | [default to nothing]
 **scheduler_config_id** | **String** |  | [default to nothing]
 **status** | **String** |  | [default to nothing]

### Return type

[**ListScheduledComputeNodesResponse**](ListScheduledComputeNodesResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_slurm_schedulers**
> list_slurm_schedulers(_api::DefaultApi, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, name=nothing, account=nothing, gres=nothing, mem=nothing, nodes=nothing, partition=nothing, qos=nothing, tmp=nothing, walltime=nothing, _mediaType=nothing) -> ListSlurmSchedulersResponse, OpenAPI.Clients.ApiResponse <br/>
> list_slurm_schedulers(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, name=nothing, account=nothing, gres=nothing, mem=nothing, nodes=nothing, partition=nothing, qos=nothing, tmp=nothing, walltime=nothing, _mediaType=nothing) -> Channel{ ListSlurmSchedulersResponse }, OpenAPI.Clients.ApiResponse

Retrieve a Slurm compute node configuration.

Retrieve a Slurm compute node configuration.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to 0]
 **limit** | **Int64** |  | [default to 100000]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to false]
 **name** | **String** |  | [default to nothing]
 **account** | **String** |  | [default to nothing]
 **gres** | **String** |  | [default to nothing]
 **mem** | **String** |  | [default to nothing]
 **nodes** | **Int64** |  | [default to nothing]
 **partition** | **String** |  | [default to nothing]
 **qos** | **String** |  | [default to nothing]
 **tmp** | **String** |  | [default to nothing]
 **walltime** | **String** |  | [default to nothing]

### Return type

[**ListSlurmSchedulersResponse**](ListSlurmSchedulersResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_slurm_stats**
> list_slurm_stats(_api::DefaultApi, workflow_id::Int64; job_id=nothing, offset=nothing, limit=nothing, _mediaType=nothing) -> ListSlurmStatsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_slurm_stats(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; job_id=nothing, offset=nothing, limit=nothing, _mediaType=nothing) -> Channel{ ListSlurmStatsResponse }, OpenAPI.Clients.ApiResponse

List Slurm accounting stats.

Retrieve Slurm accounting stats for a workflow, optionally filtered by job.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **job_id** | **Int64** | Return the stats for a specific job. | [default to nothing]
 **offset** | **Int64** |  | [default to 0]
 **limit** | **Int64** |  | [default to 10000]

### Return type

[**ListSlurmStatsResponse**](ListSlurmStatsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_user_data**
> list_user_data(_api::DefaultApi, workflow_id::Int64; consumer_job_id=nothing, producer_job_id=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, name=nothing, is_ephemeral=nothing, _mediaType=nothing) -> ListUserDataResponse, OpenAPI.Clients.ApiResponse <br/>
> list_user_data(_api::DefaultApi, response_stream::Channel, workflow_id::Int64; consumer_job_id=nothing, producer_job_id=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, name=nothing, is_ephemeral=nothing, _mediaType=nothing) -> Channel{ ListUserDataResponse }, OpenAPI.Clients.ApiResponse

Retrieve all user data records for one workflow.

Retrieve all user data records for one workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**workflow_id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **consumer_job_id** | **Int64** | Return user data records that are consumed by a specific job. | [default to nothing]
 **producer_job_id** | **Int64** | Return user data records that are produced by a specific job. | [default to nothing]
 **offset** | **Int64** |  | [default to 0]
 **limit** | **Int64** |  | [default to 100000]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to false]
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

# **list_user_groups**
> list_user_groups(_api::DefaultApi, user_name::String; offset=nothing, limit=nothing, _mediaType=nothing) -> ListAccessGroupsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_user_groups(_api::DefaultApi, response_stream::Channel, user_name::String; offset=nothing, limit=nothing, _mediaType=nothing) -> Channel{ ListAccessGroupsResponse }, OpenAPI.Clients.ApiResponse

List groups a user belongs to.

List groups a user belongs to.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**user_name** | **String** | Username |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to 0]
 **limit** | **Int64** |  | [default to 100]

### Return type

[**ListAccessGroupsResponse**](ListAccessGroupsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_workflow_groups**
> list_workflow_groups(_api::DefaultApi, id::Int64; _mediaType=nothing) -> ListAccessGroupsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_workflow_groups(_api::DefaultApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ ListAccessGroupsResponse }, OpenAPI.Clients.ApiResponse

List access groups that have access to a workflow.

List access groups that have access to a workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the workflow |

### Return type

[**ListAccessGroupsResponse**](ListAccessGroupsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_workflows**
> list_workflows(_api::DefaultApi; offset=nothing, sort_by=nothing, reverse_sort=nothing, limit=nothing, name=nothing, user=nothing, description=nothing, is_archived=nothing, _mediaType=nothing) -> ListWorkflowsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_workflows(_api::DefaultApi, response_stream::Channel; offset=nothing, sort_by=nothing, reverse_sort=nothing, limit=nothing, name=nothing, user=nothing, description=nothing, is_archived=nothing, _mediaType=nothing) -> Channel{ ListWorkflowsResponse }, OpenAPI.Clients.ApiResponse

Retrieve all workflows.

Retrieve all workflows.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to 0]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to false]
 **limit** | **Int64** |  | [default to 100000]
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

# **manage_status_change**
> manage_status_change(_api::DefaultApi, id::Int64, status::JobStatus, run_id::Int64; body=nothing, _mediaType=nothing) -> JobModel, OpenAPI.Clients.ApiResponse <br/>
> manage_status_change(_api::DefaultApi, response_stream::Channel, id::Int64, status::JobStatus, run_id::Int64; body=nothing, _mediaType=nothing) -> Channel{ JobModel }, OpenAPI.Clients.ApiResponse

Change the status of a job and manage side effects.

Change the status of a job and manage side effects.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Job ID |
**status** | [**JobStatus**](.md) | New job status |
**run_id** | **Int64** | Current job run ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

[**JobModel**](JobModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **ping**
> ping(_api::DefaultApi; _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> ping(_api::DefaultApi, response_stream::Channel; _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse

Check if the service is running.

Check if the service is running.

### Required Parameters
This endpoint does not need any parameter.

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **process_changed_job_inputs**
> process_changed_job_inputs(_api::DefaultApi, id::Int64; dry_run=nothing, body=nothing, _mediaType=nothing) -> ProcessChangedJobInputsResponse, OpenAPI.Clients.ApiResponse <br/>
> process_changed_job_inputs(_api::DefaultApi, response_stream::Channel, id::Int64; dry_run=nothing, body=nothing, _mediaType=nothing) -> Channel{ ProcessChangedJobInputsResponse }, OpenAPI.Clients.ApiResponse

Check for changed job inputs and update status accordingly.

Check for changed job inputs and update status accordingly.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **dry_run** | **Bool** | If true, report changes but do not change the database. | [default to nothing]
 **body** | **Any** |  | 

### Return type

[**ProcessChangedJobInputsResponse**](ProcessChangedJobInputsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **remove_user_from_group**
> remove_user_from_group(_api::DefaultApi, id::Int64, user_name::String; body=nothing, _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> remove_user_from_group(_api::DefaultApi, response_stream::Channel, id::Int64, user_name::String; body=nothing, _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse

Remove a user from an access group.

Remove a user from an access group.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the access group |
**user_name** | **String** | Username to remove |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **remove_workflow_from_group**
> remove_workflow_from_group(_api::DefaultApi, id::Int64, group_id::Int64; body=nothing, _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> remove_workflow_from_group(_api::DefaultApi, response_stream::Channel, id::Int64, group_id::Int64; body=nothing, _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse

Revoke an access group's access to a workflow.

Revoke an access group's access to a workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the workflow |
**group_id** | **Int64** | ID of the access group |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **reset_job_status**
> reset_job_status(_api::DefaultApi, id::Int64; failed_only=nothing, body=nothing, _mediaType=nothing) -> ResetJobStatusResponse, OpenAPI.Clients.ApiResponse <br/>
> reset_job_status(_api::DefaultApi, response_stream::Channel, id::Int64; failed_only=nothing, body=nothing, _mediaType=nothing) -> Channel{ ResetJobStatusResponse }, OpenAPI.Clients.ApiResponse

Reset status for jobs to uninitialized.

Reset status for jobs to uninitialized.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **failed_only** | **Bool** | Only reset failed jobs | [default to false]
 **body** | **Any** |  | 

### Return type

[**ResetJobStatusResponse**](ResetJobStatusResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **reset_workflow_status**
> reset_workflow_status(_api::DefaultApi, id::Int64; force=nothing, body=nothing, _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> reset_workflow_status(_api::DefaultApi, response_stream::Channel, id::Int64; force=nothing, body=nothing, _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse

Reset worklow status.

Reset workflow status.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **force** | **Bool** | If true, ignore active jobs check and reset anyway | [default to false]
 **body** | **Any** |  | 

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **retry_job**
> retry_job(_api::DefaultApi, id::Int64, run_id::Int64; body=nothing, _mediaType=nothing) -> JobModel, OpenAPI.Clients.ApiResponse <br/>
> retry_job(_api::DefaultApi, response_stream::Channel, id::Int64, run_id::Int64; body=nothing, _mediaType=nothing) -> Channel{ JobModel }, OpenAPI.Clients.ApiResponse

Retry a failed job.

Retry a failed job by resetting it to ready status and incrementing attempt_id.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Job ID |
**run_id** | **Int64** | Current workflow run ID |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

[**JobModel**](JobModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **start_job**
> start_job(_api::DefaultApi, id::Int64, run_id::Int64, compute_node_id::Int64; body=nothing, _mediaType=nothing) -> JobModel, OpenAPI.Clients.ApiResponse <br/>
> start_job(_api::DefaultApi, response_stream::Channel, id::Int64, run_id::Int64, compute_node_id::Int64; body=nothing, _mediaType=nothing) -> Channel{ JobModel }, OpenAPI.Clients.ApiResponse

Start a job and manage side effects.

Start a job and manage side effects.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Job ID |
**run_id** | **Int64** | Current job run ID |
**compute_node_id** | **Int64** | Compute node ID that started the job |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **body** | **Any** |  | 

### Return type

[**JobModel**](JobModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_compute_node**
> update_compute_node(_api::DefaultApi, id::Int64, body::ComputeNodeModel; _mediaType=nothing) -> ComputeNodeModel, OpenAPI.Clients.ApiResponse <br/>
> update_compute_node(_api::DefaultApi, response_stream::Channel, id::Int64, body::ComputeNodeModel; _mediaType=nothing) -> Channel{ ComputeNodeModel }, OpenAPI.Clients.ApiResponse

Update a compute node.

Update a compute node.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the compute node. |
**body** | [**ComputeNodeModel**](ComputeNodeModel.md) | Compute node to update in the database. |

### Return type

[**ComputeNodeModel**](ComputeNodeModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_event**
> update_event(_api::DefaultApi, id::Int64, body::EventModel; _mediaType=nothing) -> EventModel, OpenAPI.Clients.ApiResponse <br/>
> update_event(_api::DefaultApi, response_stream::Channel, id::Int64, body::EventModel; _mediaType=nothing) -> Channel{ EventModel }, OpenAPI.Clients.ApiResponse

Update an event.

Update an event.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the event. |
**body** | [**EventModel**](EventModel.md) | event to update in the table. |

### Return type

[**EventModel**](EventModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_file**
> update_file(_api::DefaultApi, id::Int64, body::FileModel; _mediaType=nothing) -> FileModel, OpenAPI.Clients.ApiResponse <br/>
> update_file(_api::DefaultApi, response_stream::Channel, id::Int64, body::FileModel; _mediaType=nothing) -> Channel{ FileModel }, OpenAPI.Clients.ApiResponse

Update a file.

Update a file.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the file. |
**body** | [**FileModel**](FileModel.md) | file to update in the table. |

### Return type

[**FileModel**](FileModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_job**
> update_job(_api::DefaultApi, id::Int64, body::JobModel; _mediaType=nothing) -> JobModel, OpenAPI.Clients.ApiResponse <br/>
> update_job(_api::DefaultApi, response_stream::Channel, id::Int64, body::JobModel; _mediaType=nothing) -> Channel{ JobModel }, OpenAPI.Clients.ApiResponse

Update a job.

Update a job.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | ID of the job. |
**body** | [**JobModel**](JobModel.md) | job to update in the table. |

### Return type

[**JobModel**](JobModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_local_scheduler**
> update_local_scheduler(_api::DefaultApi, id::Int64, body::LocalSchedulerModel; _mediaType=nothing) -> LocalSchedulerModel, OpenAPI.Clients.ApiResponse <br/>
> update_local_scheduler(_api::DefaultApi, response_stream::Channel, id::Int64, body::LocalSchedulerModel; _mediaType=nothing) -> Channel{ LocalSchedulerModel }, OpenAPI.Clients.ApiResponse

Update a local scheduler.

Update a local scheduler.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Scheduler ID |
**body** | [**LocalSchedulerModel**](LocalSchedulerModel.md) | local compute node configuration to update in the table. |

### Return type

[**LocalSchedulerModel**](LocalSchedulerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_resource_requirements**
> update_resource_requirements(_api::DefaultApi, id::Int64, body::ResourceRequirementsModel; _mediaType=nothing) -> ResourceRequirementsModel, OpenAPI.Clients.ApiResponse <br/>
> update_resource_requirements(_api::DefaultApi, response_stream::Channel, id::Int64, body::ResourceRequirementsModel; _mediaType=nothing) -> Channel{ ResourceRequirementsModel }, OpenAPI.Clients.ApiResponse

Update one resource requirements record.

Update one resource requirements record.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Resource requirements ID |
**body** | [**ResourceRequirementsModel**](ResourceRequirementsModel.md) | resource requirements to update in the table. |

### Return type

[**ResourceRequirementsModel**](ResourceRequirementsModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_result**
> update_result(_api::DefaultApi, id::Int64, body::ResultModel; _mediaType=nothing) -> ResultModel, OpenAPI.Clients.ApiResponse <br/>
> update_result(_api::DefaultApi, response_stream::Channel, id::Int64, body::ResultModel; _mediaType=nothing) -> Channel{ ResultModel }, OpenAPI.Clients.ApiResponse

Update a job result.

Update a job result.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Result ID |
**body** | [**ResultModel**](ResultModel.md) | result to update in the table. |

### Return type

[**ResultModel**](ResultModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_ro_crate_entity**
> update_ro_crate_entity(_api::DefaultApi, id::Int64, body::RoCrateEntityModel; _mediaType=nothing) -> RoCrateEntityModel, OpenAPI.Clients.ApiResponse <br/>
> update_ro_crate_entity(_api::DefaultApi, response_stream::Channel, id::Int64, body::RoCrateEntityModel; _mediaType=nothing) -> Channel{ RoCrateEntityModel }, OpenAPI.Clients.ApiResponse

Update an RO-Crate entity.

Update an RO-Crate entity.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** |  |
**body** | [**RoCrateEntityModel**](RoCrateEntityModel.md) | Updated RO-Crate entity |

### Return type

[**RoCrateEntityModel**](RoCrateEntityModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_scheduled_compute_node**
> update_scheduled_compute_node(_api::DefaultApi, id::Int64, body::ScheduledComputeNodesModel; _mediaType=nothing) -> ScheduledComputeNodesModel, OpenAPI.Clients.ApiResponse <br/>
> update_scheduled_compute_node(_api::DefaultApi, response_stream::Channel, id::Int64, body::ScheduledComputeNodesModel; _mediaType=nothing) -> Channel{ ScheduledComputeNodesModel }, OpenAPI.Clients.ApiResponse

Update a scheduled compute node.

Update a scheduled compute node.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Scheduled compute node ID |
**body** | [**ScheduledComputeNodesModel**](ScheduledComputeNodesModel.md) | scheduled compute node to update in the table. |

### Return type

[**ScheduledComputeNodesModel**](ScheduledComputeNodesModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_slurm_scheduler**
> update_slurm_scheduler(_api::DefaultApi, id::Int64, body::SlurmSchedulerModel; _mediaType=nothing) -> SlurmSchedulerModel, OpenAPI.Clients.ApiResponse <br/>
> update_slurm_scheduler(_api::DefaultApi, response_stream::Channel, id::Int64, body::SlurmSchedulerModel; _mediaType=nothing) -> Channel{ SlurmSchedulerModel }, OpenAPI.Clients.ApiResponse

Update a Slurm compute node configuration.

Update a Slurm compute node configuration.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Slurm compute node configuration ID |
**body** | [**SlurmSchedulerModel**](SlurmSchedulerModel.md) | Slurm compute node configuration to update in the table. |

### Return type

[**SlurmSchedulerModel**](SlurmSchedulerModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_user_data**
> update_user_data(_api::DefaultApi, id::Int64, body::UserDataModel; _mediaType=nothing) -> UserDataModel, OpenAPI.Clients.ApiResponse <br/>
> update_user_data(_api::DefaultApi, response_stream::Channel, id::Int64, body::UserDataModel; _mediaType=nothing) -> Channel{ UserDataModel }, OpenAPI.Clients.ApiResponse

Update a user data record.

Update a user data record.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | User data record ID |
**body** | [**UserDataModel**](UserDataModel.md) | user data to update in the table. |

### Return type

[**UserDataModel**](UserDataModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_workflow**
> update_workflow(_api::DefaultApi, id::Int64, body::WorkflowModel; _mediaType=nothing) -> WorkflowModel, OpenAPI.Clients.ApiResponse <br/>
> update_workflow(_api::DefaultApi, response_stream::Channel, id::Int64, body::WorkflowModel; _mediaType=nothing) -> Channel{ WorkflowModel }, OpenAPI.Clients.ApiResponse

Update a workflow.

Update a workflow.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |
**body** | [**WorkflowModel**](WorkflowModel.md) | workflow to update in the table. |

### Return type

[**WorkflowModel**](WorkflowModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_workflow_status**
> update_workflow_status(_api::DefaultApi, id::Int64, body::WorkflowStatusModel; _mediaType=nothing) -> WorkflowStatusModel, OpenAPI.Clients.ApiResponse <br/>
> update_workflow_status(_api::DefaultApi, response_stream::Channel, id::Int64, body::WorkflowStatusModel; _mediaType=nothing) -> Channel{ WorkflowStatusModel }, OpenAPI.Clients.ApiResponse

Update the workflow status.

Update the workflow status.

### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **DefaultApi** | API context | 
**id** | **Int64** | Workflow ID |
**body** | [**WorkflowStatusModel**](WorkflowStatusModel.md) | Updated workflow status |

### Return type

[**WorkflowStatusModel**](WorkflowStatusModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

