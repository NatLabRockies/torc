# SystemApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**get_version**](SystemApi.md#get_version) | **GET** /version | 
[**ping**](SystemApi.md#ping) | **GET** /ping | 


# **get_version**
> get_version(_api::SystemApi; _mediaType=nothing) -> VersionResponse, OpenAPI.Clients.ApiResponse <br/>
> get_version(_api::SystemApi, response_stream::Channel; _mediaType=nothing) -> Channel{ VersionResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters
This endpoint does not need any parameter.

### Return type

[**VersionResponse**](VersionResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

# **ping**
> ping(_api::SystemApi; _mediaType=nothing) -> PingResponse, OpenAPI.Clients.ApiResponse <br/>
> ping(_api::SystemApi, response_stream::Channel; _mediaType=nothing) -> Channel{ PingResponse }, OpenAPI.Clients.ApiResponse



### Required Parameters
This endpoint does not need any parameter.

### Return type

[**PingResponse**](PingResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

