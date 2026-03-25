# EventsApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_event**](EventsApi.md#create_event) | **POST** /events | 
[**delete_event**](EventsApi.md#delete_event) | **DELETE** /events/{id} | 
[**delete_events**](EventsApi.md#delete_events) | **DELETE** /events | 
[**get_event**](EventsApi.md#get_event) | **GET** /events/{id} | 
[**list_events**](EventsApi.md#list_events) | **GET** /events | 
[**update_event**](EventsApi.md#update_event) | **PUT** /events/{id} | 


# **create_event**
> create_event(_api::EventsApi, event_model::EventModel; _mediaType=nothing) -> EventModel, OpenAPI.Clients.ApiResponse <br/>
> create_event(_api::EventsApi, response_stream::Channel, event_model::EventModel; _mediaType=nothing) -> Channel{ EventModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **EventsApi** | API context | 
**event_model** | [**EventModel**](EventModel.md) |  |

### Return type

[**EventModel**](EventModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_event**
> delete_event(_api::EventsApi, id::Int64; _mediaType=nothing) -> EventModel, OpenAPI.Clients.ApiResponse <br/>
> delete_event(_api::EventsApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ EventModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **EventsApi** | API context | 
**id** | **Int64** | ID of the event record. |

### Return type

[**EventModel**](EventModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **delete_events**
> delete_events(_api::EventsApi, workflow_id::Int64; _mediaType=nothing) -> Any, OpenAPI.Clients.ApiResponse <br/>
> delete_events(_api::EventsApi, response_stream::Channel, workflow_id::Int64; _mediaType=nothing) -> Channel{ Any }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **EventsApi** | API context | 
**workflow_id** | **Int64** |  |

### Return type

**Any**

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **get_event**
> get_event(_api::EventsApi, id::Int64; _mediaType=nothing) -> EventModel, OpenAPI.Clients.ApiResponse <br/>
> get_event(_api::EventsApi, response_stream::Channel, id::Int64; _mediaType=nothing) -> Channel{ EventModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **EventsApi** | API context | 
**id** | **Int64** | ID of the event record. |

### Return type

[**EventModel**](EventModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **list_events**
> list_events(_api::EventsApi, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, category=nothing, after_timestamp=nothing, _mediaType=nothing) -> ListEventsResponse, OpenAPI.Clients.ApiResponse <br/>
> list_events(_api::EventsApi, response_stream::Channel, workflow_id::Int64; offset=nothing, limit=nothing, sort_by=nothing, reverse_sort=nothing, category=nothing, after_timestamp=nothing, _mediaType=nothing) -> Channel{ ListEventsResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **EventsApi** | API context | 
**workflow_id** | **Int64** |  |

### Optional Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **offset** | **Int64** |  | [default to nothing]
 **limit** | **Int64** |  | [default to nothing]
 **sort_by** | **String** |  | [default to nothing]
 **reverse_sort** | **Bool** |  | [default to nothing]
 **category** | **String** |  | [default to nothing]
 **after_timestamp** | **Int64** |  | [default to nothing]

### Return type

[**ListEventsResponse**](ListEventsResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **update_event**
> update_event(_api::EventsApi, id::Int64, body::Any; _mediaType=nothing) -> EventModel, OpenAPI.Clients.ApiResponse <br/>
> update_event(_api::EventsApi, response_stream::Channel, id::Int64, body::Any; _mediaType=nothing) -> Channel{ EventModel }, OpenAPI.Clients.ApiResponse



### Required Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **_api** | **EventsApi** | API context | 
**id** | **Int64** | ID of the event. |
**body** | **Any** |  |

### Return type

[**EventModel**](EventModel.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

