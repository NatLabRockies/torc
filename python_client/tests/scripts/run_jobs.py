"""Test script to run jobs."""

import sys
from pathlib import Path

from torc.api import DefaultApi
from torc.openapi_client import ApiClient
from torc.openapi_client.configuration import Configuration

from torc.job_runner import JobRunner
from torc.loggers import setup_logging


if __name__ == "__main__":
    if len(sys.argv) != 5:
        print(f"Usage: python {sys.argv[0]} url workflow_key output_dir time_limit")
        sys.exit(1)

    configuration = Configuration()
    configuration.host = sys.argv[1]
    workflow_key = sys.argv[2]
    time_limit = sys.argv[3]
    output_dir = Path(sys.argv[4])
    job_completion_poll_interval = 0.1
    setup_logging()  # , filename=output_dir / f"{os.getpid()}.log")
    api = DefaultApi(ApiClient(configuration))
    workflow = api.get_workflow(workflow_key)
    runner = JobRunner(
        api,
        workflow,
        output_dir,
        job_completion_poll_interval=job_completion_poll_interval,
        time_limit=time_limit,
    )
    runner.run_worker()
