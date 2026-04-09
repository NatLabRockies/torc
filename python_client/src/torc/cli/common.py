"""Common functions for CLI commands."""

import os
import sys
from pathlib import Path
from typing import Any

from loguru import logger


def check_output_path(path: Path, force: bool) -> None:
    """Ensure that the parameter path does not exist.

    Parameters
    ----------
    path : Path
        Path to check.
    force : bool
        If True and the path exists, delete it.
    """
    if path.exists():
        if force:
            path.unlink()
        else:
            msg = (
                f"{path} already exists. Choose a different name or pass --force to overwrite it."
            )
            print(msg, file=sys.stderr)
            sys.exit(1)


def get_job_env_vars() -> dict[str, Any]:
    """Return the environment variables set by torc for a job.

    Returns
    -------
    dict[str, Any]
        Dictionary containing url, workflow_id, and job_id.
    """
    env_vars: dict[str, Any] = {}
    url = os.getenv("TORC_API_URL")
    if url is None:
        logger.error("This command can only be called from the torc worker application.")
        sys.exit(1)
    env_vars["url"] = url

    workflow_id_str = os.getenv("TORC_WORKFLOW_ID")
    if workflow_id_str is None:
        logger.error("This command can only be called from the torc worker application.")
        sys.exit(1)
    workflow_id = int(workflow_id_str)
    env_vars["workflow_id"] = workflow_id

    job_id_str = os.getenv("TORC_JOB_ID")
    if job_id_str is None:
        logger.error("This command can only be called from the torc worker application.")
        sys.exit(1)
    job_id = int(job_id_str)
    env_vars["job_id"] = job_id

    return env_vars
