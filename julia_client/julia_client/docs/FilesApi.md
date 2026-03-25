# FilesApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_file**](FilesApi.md#create_file) | **POST** /files | 
[**delete_file**](FilesApi.md#delete_file) | **DELETE** /files/{id} | 
[**delete_files**](FilesApi.md#delete_files) | **DELETE** /files | 
[**get_file**](FilesApi.md#get_file) | **GET** /files/{id} | 
[**list_files**](FilesApi.md#list_files) | **GET** /files | 
[**update_file**](FilesApi.md#update_file) | **PUT** /files/{id} | 


# **create_file**
> create_file(_api::FilesApi, file_model::FileModel; _mediaType=nothing) -> FileModel, OpenAPI.Clients.ApiResponse <br/>
> create_file(_api::FilesApi, response_stream::Channel, file_model::FileModel; _mediaType=nothing) -> Channel{ FileModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FilesApi** | API context | 
**file_model** | [**FileModel**](FileModel.md) |  |

### Return type

[**FileModel**](FileModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_file**
> delete_file(_api::FilesApi, id::Int64; _mediaType=nothing) -> FileModel, OpenAPI.Clients.ApiResponse <br/>
> delete_file(_api::FilesApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ FileModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FilesApi** | API context | 
**id** | **Int64** | File ID |

### Return type

[**FileModel**](FileModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_files**
> delete_files(_api::FilesApi, workflow_id::Int64; _mediaType=nothing) -> DeleteCountResponse, OpenAPI.Clients.ApiResponse <br/>
> delete_files(_api::FilesApi, response_stream::Channel, workflow_id::Int64; _mediaType=nothing) -> Channel{ DeleteCountResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FilesApi** | API context | 
**workflow_id** | **Int64** |  |

### Return type

[**DeleteCountResponse**](DeleteCountResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_file**
> get_file(_api::FilesApi, id::Int64; _mediaType=nothing) -> FileModel, OpenAPI.Clients.ApiResponse <br/>
> get_file(_api::FilesApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ FileModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FilesApi** | API context | 
**id** | **Int64** | ID of the file record |

### Return type

[**FileModel**](FileModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_files**
> list_files(_api::FilesApi, workflow_id::Int64; produced_by_job_id=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, name=nothing, path=nothing, is_output=nothing, _mediaType=nothing) -> ListFilesResponse, OpenAPI.Clients.ApiResponse <br/>
> list_files(_api::FilesApi, response_stream::Channel, workflow_id::Int64; produced_by_job_id=nothing, offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, name=nothing, path=nothing, is_output=nothing, _mediaType=nothing) -> Channel{ ListFilesResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FilesApi** | API context | 
**workflow_id** | **Int64** |  |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **produced_by_job_id** | **Int64** |  | [default to nothing]
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to nothing]
 **name** | **String** |  | [default to nothing]
 **path** | **String** |  | [default to nothing]
 **is_output** | **Bool** |  | [default to nothing]

### Return type

[**ListFilesResponse**](ListFilesResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_file**
> update_file(_api::FilesApi, id::Int64, file_model::FileModel; _mediaType=nothing) -> FileModel, OpenAPI.Clients.ApiResponse <br/>
> update_file(_api::FilesApi, response_stream::Channel, id::Int64, file_model::FileModel; _mediaType=nothing) -> Channel{ FileModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **FilesApi** | API context | 
**id** | **Int64** | ID of the file. |
**file_model** | [**FileModel**](FileModel.md) |  |

### Return type

[**FileModel**](FileModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

