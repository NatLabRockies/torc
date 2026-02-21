# AdminApi

All URIs are relative to *http://localhost/torc-service/v1*

Method | HTTP request | Description
------------- | ------------- | -------------
[**reload_auth**](AdminApi.md#reload_auth) | **POST** /admin/reload-auth | Reload auth credentials from htpasswd file


# **reload_auth**
> reload_auth(_api::AdminApi; _mediaType=nothing) -> ReloadAuth200Response, OpenAPI.Clients.ApiResponse <br/>
> reload_auth(_api::AdminApi, response_stream::Channel; _mediaType=nothing) -> Channel{ ReloadAuth200Response }, OpenAPI.Clients.ApiResponse

Reload auth credentials from htpasswd file

Re-reads the htpasswd authentication file from disk without restarting the server. This is useful for adding/removing users or changing passwords. Only admin users can call this endpoint. The credential cache is cleared after reload.

### Required Parameters
This endpoint does not need any parameter.

### Return type

[**ReloadAuth200Response**](ReloadAuth200Response.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

