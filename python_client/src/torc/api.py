"""Functions to access the Torc Database API."""

import time
from collections.abc import Callable, Generator
from typing import Any

from loguru import logger
from rmon.timing.timer_stats import Timer

from torc.common import check_function, timer_stats_collector
from torc.exceptions import DatabaseOffline
from torc.openapi_client import (
    AccessControlApi,
    ApiClient,
    ComputeNodesApi,
    EventsApi,
    FailureHandlersApi,
    FilesApi,
    JobsApi,
    LocalSchedulersApi,
    RemoteWorkersApi,
    ResourceRequirementsApi,
    ResultsApi,
    RoCrateEntitiesApi,
    ScheduledComputeNodesApi,
    SlurmSchedulersApi,
    SlurmStatsApi,
    SystemApi,
    UserDataApi,
    WorkflowActionsApi,
    WorkflowsApi,
)
from torc.openapi_client.configuration import Configuration
from torc.openapi_client.models.job_model import JobModel
from torc.openapi_client.models.jobs_model import JobsModel
from torc.openapi_client.models.user_data_model import UserDataModel
from torc.openapi_client.rest import ApiException


class DefaultApi:
    """Backward-compatible aggregate API wrapper over the tag-grouped generated APIs."""

    def __init__(self, api_client: ApiClient):
        self.api_client = api_client
        self._apis = (
            AccessControlApi(api_client),
            ComputeNodesApi(api_client),
            EventsApi(api_client),
            FailureHandlersApi(api_client),
            FilesApi(api_client),
            JobsApi(api_client),
            LocalSchedulersApi(api_client),
            RemoteWorkersApi(api_client),
            ResourceRequirementsApi(api_client),
            ResultsApi(api_client),
            RoCrateEntitiesApi(api_client),
            ScheduledComputeNodesApi(api_client),
            SlurmSchedulersApi(api_client),
            SlurmStatsApi(api_client),
            SystemApi(api_client),
            UserDataApi(api_client),
            WorkflowActionsApi(api_client),
            WorkflowsApi(api_client),
        )

    def __getattr__(self, name: str) -> Any:
        for api in self._apis:
            if hasattr(api, name):
                return getattr(api, name)
        raise AttributeError(name)


def make_api(database_url: str) -> DefaultApi:
    """Instantiate an OpenAPI client object from a database URL.

    Parameters
    ----------
    database_url : str
        URL of the Torc database API.

    Returns
    -------
    DefaultApi
        OpenAPI client for the Torc database.
    """
    configuration = Configuration()
    configuration.host = database_url
    return DefaultApi(ApiClient(configuration))


def wait_for_healthy_database(
    api: DefaultApi, timeout_minutes: float = 20, poll_seconds: float = 60
) -> None:
    """Ping the database until it's responding or timeout_minutes is exceeded.

    Parameters
    ----------
    api : DefaultApi
        OpenAPI client for the Torc database.
    timeout_minutes : float, optional
        Number of minutes to wait for the database to become healthy, by default 20.
    poll_seconds : float, optional
        Number of seconds to wait in between each poll, by default 60.

    Raises
    ------
    DatabaseOffline
        Raised if the timeout is exceeded.
    """
    logger.info(
        "Wait for the database to become healthy: timeout_minutes={}, poll_seconds={}",
        timeout_minutes,
        poll_seconds,
    )
    end = time.time() + timeout_minutes * 60
    while time.time() < end:
        try:
            send_api_command(api.ping)
            logger.info("The database is healthy again.")
            return
        except DatabaseOffline:
            logger.exception("Database is still offline")
        time.sleep(poll_seconds)

    msg = "Timed out waiting for database to become healthy"
    raise DatabaseOffline(msg)


def iter_documents(func: Callable, *args, offset: int = 0, **kwargs) -> Generator[Any, None, None]:
    """Return a generator of documents where the API service employs batching.

    Parameters
    ----------
    func : Callable
        API function to call.
    *args
        Positional arguments to pass to the function.
    offset : int, optional
        Number of documents to skip, by default 0.
    **kwargs
        Keyword arguments to pass to the function.

    Yields
    ------
    Any
        OpenAPI (pydantic) model or dict, depending on what the API function returns.
    """
    if "limit" in kwargs and kwargs["limit"] is None:
        kwargs.pop("limit")
    limit = kwargs.get("limit")

    has_more = True
    docs_received = 0
    while has_more and (limit is None or docs_received < limit):
        result = func(*args, offset=offset, **kwargs)
        yield from result.items
        offset += result.count
        docs_received += result.count
        has_more = result.has_more


def make_job_label(job: JobModel, include_status: bool = False) -> str:
    """Return a user-friendly label for the job for log statements.

    Parameters
    ----------
    job : JobModel
        The job model.
    include_status : bool, optional
        Whether to include the job status in the label, by default False.

    Returns
    -------
    str
        User-friendly label for the job.
    """
    base = f"Job name={job.name} id={job.id}"
    if include_status:
        return f"{base} status={job.status}"
    return base


def send_api_command(
    func: Callable, *args, raise_on_error: bool = True, timeout: float = 120, **kwargs
) -> Any:
    """Send an API command while tracking time, if timer_stats_collector is enabled.

    Parameters
    ----------
    func : Callable
        API function to call.
    *args
        Positional arguments to pass to the function.
    raise_on_error : bool, optional
        Raise an exception if there is an error, by default True.
    timeout : float, optional
        Timeout in seconds, by default 120.
    **kwargs
        Keyword arguments to pass to the function.

    Returns
    -------
    Any
        Result from the API function.

    Raises
    ------
    ApiException
        Raised for errors detected by the server.
    DatabaseOffline
        Raised for all connection errors.
    """
    with Timer(timer_stats_collector, func.__name__):
        try:
            logger.trace("Send API command {}", func.__name__)
            return func(*args, _request_timeout=timeout, **kwargs)
        except ApiException:
            # This covers all errors reported by the server.
            logger.exception("Failed to send API command {}", func.__name__)
            if raise_on_error:
                raise
            logger.info("Exception is ignored.")
            return None
        except Exception as exc:
            # This covers all connection errors. It is likely too risky to try to catch
            # all possible errors from the underlying libraries (OS, urllib3, etc).
            logger.exception("Failed to send API command {}", func.__name__)
            if raise_on_error:
                msg = f"Received exception from API client: {exc=}"
                raise DatabaseOffline(msg) from exc
            logger.info("Exception is ignored.")
            return None


def create_jobs(api: DefaultApi, jobs: list, max_transfer_size: int = 100_000) -> list[JobModel]:
    """Create and add an iterable of jobs to the workflow.

    Parameters
    ----------
    api : DefaultApi
        OpenAPI client for the Torc database.
    jobs : list
        Any iterable of JobModel.
    max_transfer_size : int, optional
        Maximum number of jobs to add per API call, by default 100000.

    Returns
    -------
    list[JobModel]
        List of created jobs. Provided in same order as jobs.
    """
    added_jobs = []
    batch = []
    for job in jobs:
        batch.append(job)
        if len(batch) > max_transfer_size:
            res = send_api_command(api.create_jobs, JobsModel(jobs=batch))
            added_jobs += res.jobs
            batch.clear()

    if batch:
        res = send_api_command(api.create_jobs, JobsModel(jobs=batch))
        added_jobs += res.jobs

    return added_jobs


def map_function_to_jobs(
    api: DefaultApi,
    workflow_id: int,
    module: str,
    func: str,
    params: list[dict],
    postprocess_func: str | None = None,
    module_directory: str | None = None,
    resource_requirements_id: int | None = None,
    scheduler_id: int | None = None,
    start_index: int = 1,
    name_prefix: str = "",
    depends_on_job_ids: list[int] | None = None,
) -> list[JobModel]:
    """Add a job that will call func for each item in params.

    Parameters
    ----------
    api : DefaultApi
        OpenAPI client for the Torc database.
    workflow_id : int
        ID of the workflow.
    module : str
        Name of module that contains func. If it is not available in the Python path,
        specify the parent directory in module_directory.
    func : str
        Name of the function in module to be called.
    params : list[dict]
        Each item in this list will be passed to func. The contents must be
        serializable to JSON.
    postprocess_func : str | None, optional
        Name of the function in module to be called to postprocess all results,
        by default None.
    module_directory : str | None, optional
        Required if module is not importable, by default None.
    resource_requirements_id : int | None, optional
        ID of resource_requirements that should be used by each job, by default None.
    scheduler_id : int | None, optional
        ID of scheduler that should be used by each job, by default None.
    start_index : int, optional
        Starting index to use for job names, by default 1.
    name_prefix : str, optional
        Prepend job names with this prefix, by default "". Names will be the
        index converted to a string.
    depends_on_job_ids : list[int] | None, optional
        Job IDs that should block all jobs created by this function, by default None.

    Returns
    -------
    list[JobModel]
        List of created jobs.
    """
    jobs = []
    output_data_ids = []
    for i, job_params in enumerate(params, start=start_index):
        check_function(module, func, module_directory)
        job_name = f"{name_prefix}{i}"
        data = {
            "module": module,
            "func": func,
            "params": job_params,
            "job_name": job_name,
        }
        if module_directory is not None:
            data["module_directory"] = module_directory
        input_ud = api.create_user_data(
            UserDataModel(workflow_id=workflow_id, name=f"input_{job_name}", data=data)
        )
        output_ud = api.create_user_data(
            UserDataModel(workflow_id=workflow_id, name=f"output_{job_name}", data={})
        )
        assert input_ud.id is not None
        assert output_ud.id is not None
        output_data_ids.append(output_ud.id)
        job = JobModel(
            workflow_id=workflow_id,
            name=job_name,
            command="pytorc run-function",
            input_user_data_ids=[input_ud.id],
            output_user_data_ids=[output_ud.id],
            resource_requirements_id=resource_requirements_id,
            scheduler_id=scheduler_id,
            depends_on_job_ids=depends_on_job_ids,
        )
        jobs.append(job)

    if postprocess_func is not None:
        check_function(module, postprocess_func, module_directory)
        data = {
            "module": module,
            "func": postprocess_func,
        }
        if module_directory is not None:
            data["module_directory"] = module_directory
        input_ud = api.create_user_data(
            UserDataModel(workflow_id=workflow_id, name="input_postprocess", data=data)
        )
        output_ud = api.create_user_data(
            UserDataModel(workflow_id=workflow_id, name="postprocess_result", data=data)
        )
        assert input_ud.id is not None
        assert output_ud.id is not None
        jobs.append(
            JobModel(
                workflow_id=workflow_id,
                name="postprocess",
                command="pytorc run-postprocess",
                input_user_data_ids=[input_ud.id] + output_data_ids,
                output_user_data_ids=[output_ud.id],
                resource_requirements_id=resource_requirements_id,
                scheduler_id=scheduler_id,
            )
        )

    return create_jobs(api, jobs)


def list_model_fields(cls: type) -> list[str]:
    """Return a list of the model's fields.

    Parameters
    ----------
    cls : type
        Pydantic model class.

    Returns
    -------
    list[str]
        List of field names.
    """
    return list(cls.model_json_schema()["properties"].keys())
