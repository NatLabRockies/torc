"""Example script that monitors a workflow until it finishes.

Unlike ``diamond_workflow.py``, this script does not create a workflow. It polls the
workflow-status API endpoint for an existing workflow and prints a summary of the job
status counts until the workflow is complete (or canceled).

The OpenAPI client is configured with an HTTP basic-auth password loaded from the
``TORC_PASSWORD`` environment variable.

Usage:
    export TORC_PASSWORD=...                # optional, only if the server requires auth
    python monitor_workflow.py <workflow_id> [--poll-seconds 10]
"""

import argparse
import getpass
import os
import sys
import time

from loguru import logger

from torc.api import DefaultApi
from torc.openapi_client import ApiClient
from torc.openapi_client.configuration import Configuration
from torc.openapi_client.rest import ApiException
from torc.loggers import setup_logging


TORC_API_URL = os.getenv("TORC_API_URL", "http://localhost:8080/torc-service/v1")


def make_authenticated_api(database_url: str) -> DefaultApi:
    """Instantiate an OpenAPI client, loading the password from ``TORC_PASSWORD``.

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
    configuration.username = getpass.getuser()
    password = os.getenv("TORC_PASSWORD")
    if password is not None:
        configuration.password = password
        logger.debug("Loaded TORC_PASSWORD from the environment.")
    else:
        logger.debug("TORC_PASSWORD is not set; proceeding without a password.")
    return DefaultApi(ApiClient(configuration))


def format_status_counts(status) -> str:
    """Return a compact, human-readable summary of the non-zero job status counts."""
    counts = status.jobs_by_status
    parts = [
        f"{name}={value}"
        for name, value in (
            ("uninitialized", counts.uninitialized),
            ("blocked", counts.blocked),
            ("ready", counts.ready),
            ("pending", counts.pending),
            ("running", counts.running),
            ("completed", counts.completed),
            ("failed", counts.failed),
            ("canceled", counts.canceled),
            ("terminated", counts.terminated),
            ("disabled", counts.disabled),
            ("pending_failed", counts.pending_failed),
        )
        if value
    ]
    return ", ".join(parts) if parts else "no jobs"


def monitor_workflow(api: DefaultApi, workflow_id: int, poll_seconds: float) -> bool:
    """Poll the workflow-status endpoint until the workflow is complete.

    Parameters
    ----------
    api : DefaultApi
        OpenAPI client for the Torc database.
    workflow_id : int
        ID of the workflow to monitor.
    poll_seconds : float
        Number of seconds to wait between polls.

    Returns
    -------
    bool
        True if the workflow completed, False if it was canceled.
    """
    while True:
        try:
            status = api.get_workflow_status(workflow_id)
        except ApiException as exc:
            if exc.status == 404:
                # The body is empty on a 404, so the generated client raises a confusing
                # deserialization error if we let it propagate. Report the real cause.
                logger.error(
                    "Got HTTP 404 for the workflow-status endpoint. Either workflow_id={} "
                    "does not exist, or the running server predates the "
                    "'/workflows/{{id}}/status' endpoint and needs to be restarted/rebuilt.",
                    workflow_id,
                )
            else:
                logger.error(
                    "Failed to get status for workflow_id={}: HTTP {} {}",
                    workflow_id,
                    exc.status,
                    exc.reason,
                )
            raise
        logger.info(
            "workflow_id={} '{}' total_jobs={} active_compute_nodes={}: {}",
            status.workflow_id,
            status.workflow_name,
            status.total_jobs,
            status.active_compute_nodes,
            format_status_counts(status),
        )
        if status.is_canceled:
            logger.warning("workflow_id={} was canceled.", workflow_id)
            return False
        if status.is_complete:
            logger.info(
                "workflow_id={} is complete. total_exec_time_minutes={:.2f}",
                workflow_id,
                status.total_exec_time_minutes,
            )
            return True
        time.sleep(poll_seconds)


def main():
    """Entry point"""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("workflow_id", type=int, help="ID of the workflow to monitor")
    parser.add_argument(
        "--poll-seconds",
        type=float,
        default=10.0,
        help="Number of seconds to wait between status polls (default: 10)",
    )
    args = parser.parse_args()

    setup_logging()
    api = make_authenticated_api(TORC_API_URL)
    try:
        api.ping()
    except Exception as exc:
        logger.error("Failed to connect to the torc server: {}", exc)
        sys.exit(1)
    completed = monitor_workflow(api, args.workflow_id, args.poll_seconds)
    sys.exit(0 if completed else 1)


if __name__ == "__main__":
    main()
