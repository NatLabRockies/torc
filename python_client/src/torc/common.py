"""Common definitions in this package."""

import enum
import importlib
import sys
from collections.abc import Callable
from datetime import datetime, timezone
from types import ModuleType

from pydantic import BaseModel, ConfigDict
from rmon.timing.timer_stats import TimerStatsCollector

KiB = 1024
MiB = KiB * KiB
GiB = MiB * KiB
TiB = GiB * KiB
JOB_STDIO_DIR = "job-stdio"

timer_stats_collector = TimerStatsCollector()


class TorcBaseModel(BaseModel):
    """Base model for the torc package."""

    model_config = ConfigDict(
        str_strip_whitespace=True,
        validate_assignment=True,
        validate_default=True,
        extra="forbid",
        use_enum_values=False,
    )


class JobStatus(str, enum.Enum):
    """Defines all job statuses.

    Keep in sync with the JobStatus definition in the torc-service.
    """

    UNINITIALIZED = "uninitialized"
    BLOCKED = "blocked"
    CANCELED = "canceled"
    TERMINATED = "terminated"
    DONE = "done"
    READY = "ready"
    PENDING = "pending"
    RUNNING = "running"
    DISABLED = "disabled"


def check_function(
    module_name: str, func_name: str, module_directory: str | None = None
) -> tuple[ModuleType, Callable]:
    """Check that func_name is importable from module name.

    Parameters
    ----------
    module_name : str
        Name of the module to import.
    func_name : str
        Name of the function to retrieve from the module.
    module_directory : str | None, optional
        Directory to add to sys.path for module import, by default None.

    Returns
    -------
    tuple[ModuleType, Callable]
        The module and function references.

    Raises
    ------
    ValueError
        If the function is not defined in the module.
    """
    import os

    cur_dir = os.getcwd()
    added_cur_dir = False
    try:
        if module_directory is not None:
            sys.path.append(module_directory)
        module = importlib.import_module(module_name)
    except ModuleNotFoundError:
        sys.path.append(cur_dir)
        added_cur_dir = True
        module = importlib.import_module(module_name)
    finally:
        if module_directory is not None:
            sys.path.remove(module_directory)
        if added_cur_dir:
            sys.path.remove(cur_dir)

    func = getattr(module, func_name)
    if func is None:
        msg = f"function={func_name} is not defined in {module_name}"
        raise ValueError(msg)
    return module, func


def convert_timestamp(timestamp: int) -> datetime:
    """Convert a server-side timestamp (UTC milliseconds) to a tz-aware datetime.

    The torc server stores timestamps as UTC; this helper returns a
    timezone-aware ``datetime`` in UTC so callers cannot accidentally mix it
    with a naive local-time value. Use ``.astimezone()`` to render in the
    caller's local timezone for display.

    Parameters
    ----------
    timestamp : int
        Timestamp in UTC milliseconds since the unix epoch.

    Returns
    -------
    datetime
        Timezone-aware datetime in UTC.
    """
    return datetime.fromtimestamp(timestamp / 1000, tz=timezone.utc)
