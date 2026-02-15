"""pytest fixtures"""

import os
from pathlib import Path

import pytest

from torc import map_function_to_jobs
from torc.openapi_client import (
    ApiClient,
    Configuration,
    DefaultApi,
    WorkflowModel,
)
from torc.loggers import setup_logging
from torc.tests.database_interface import DatabaseInterface


TEST_WORKFLOW = "test_workflow"
PREPROCESS = Path("tests") / "scripts" / "preprocess.py"
POSTPROCESS = Path("tests") / "scripts" / "postprocess.py"
WORK = Path("tests") / "scripts" / "work.py"
PREPROCESS_UD = Path("tests") / "scripts" / "preprocess_ud.py"
POSTPROCESS_UD = Path("tests") / "scripts" / "postprocess_ud.py"
WORK_UD = Path("tests") / "scripts" / "work_ud.py"
INVALID = Path("tests") / "scripts" / "invalid.py"
NOOP = Path("tests") / "scripts" / "noop.py"
RC_JOB = Path("tests") / "scripts" / "resource_consumption.py"
CREATE_RESOURCE_JOB = Path("tests") / "scripts" / "create_resource.py"
USE_RESOURCE_JOB = Path("tests") / "scripts" / "use_resource.py"
SLEEP_JOB = Path("tests") / "scripts" / "sleep.py"

DEFAULT_API_URL = "http://localhost:8080/torc-service/v1"


def _initialize_api():
    setup_logging()
    configuration = Configuration()
    configuration.host = os.getenv("TORC_API_URL", DEFAULT_API_URL)
    return DefaultApi(ApiClient(configuration))


@pytest.fixture
def mapped_function_workflow(tmp_path):
    """Creates a workflow out of a function mapped to jobs."""
    api = _initialize_api()
    output_dir = tmp_path / "torc_output"
    output_dir.mkdir()
    workflow = api.create_workflow(WorkflowModel(user="test", name="test_workflow"))
    params = [{"val": i} for i in range(5)]
    map_function_to_jobs(
        api,
        workflow.id,
        "mapped_function",
        "run",
        params,
        module_directory="tests/scripts",
        postprocess_func="postprocess",
    )
    db = DatabaseInterface(api, workflow)
    yield db, output_dir
    api.delete_workflow(workflow.id)
