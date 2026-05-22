"""torc package"""

import warnings
from importlib import metadata

from torc.api import (
    create_jobs,
    iter_documents,
    make_api,
    make_job_label,
    map_function_to_jobs,
    send_api_command,
)
from torc.loggers import setup_logging
from torc.openapi_client.models.spawn_job_model import SpawnJobModel
from torc.openapi_client.models.spawn_jobs_response import SpawnJobsResponse
from torc.orchestrator import Orchestrator


__version__ = metadata.metadata("torc-client")["Version"]

warnings.filterwarnings("once", category=DeprecationWarning)


__all__ = (
    "Orchestrator",
    "SpawnJobModel",
    "SpawnJobsResponse",
    "create_jobs",
    "iter_documents",
    "make_api",
    "make_job_label",
    "map_function_to_jobs",
    "send_api_command",
    "setup_logging",
)
