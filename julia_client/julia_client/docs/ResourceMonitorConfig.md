# ResourceMonitorConfig


## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**compute_node** | [***ComputeNodeMonitorConfig**](ComputeNodeMonitorConfig.md) |  | [optional] [default to nothing]
**enabled** | **Bool** | Deprecated compatibility field. Use &#x60;jobs.enabled&#x60; for new workflow specs. | [optional] [default to false]
**flush_interval_seconds** | **Int64** | How often buffered time-series samples are flushed to SQLite, in seconds. | [optional] [default to 300]
**generate_plots** | **Bool** |  | [optional] [default to false]
**granularity** | [***MonitorGranularity**](MonitorGranularity.md) | Deprecated compatibility field. Use &#x60;jobs.granularity&#x60; for new workflow specs. | [optional] [default to nothing]
**jobs** | [***JobMonitorConfig**](JobMonitorConfig.md) |  | [optional] [default to nothing]
**live_monitoring** | **Bool** | Checkpoint the time-series database after every flush so readers on other hosts (e.g. torc-dash live plots over a shared filesystem) see new samples. Off by default: each checkpoint adds a small write and fsync to the main database file. | [optional] [default to false]
**sample_interval_seconds** | **Int64** |  | [optional] [default to 10]


[[Back to Model list]](../README.md#models) [[Back to API list]](../README.md#api-endpoints) [[Back to README]](../README.md)


