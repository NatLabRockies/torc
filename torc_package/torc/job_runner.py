"""Runs jobs on a compute node"""

import json
import logging
import os
import multiprocessing
import re
import signal
import shutil
import socket
import subprocess
import sys
import time
from datetime import datetime, timedelta
from pathlib import Path

import psutil
from pydantic import BaseModel, ConfigDict  # pylint: disable=no-name-in-module
from resource_monitor.models import (
    ComputeNodeResourceStatConfig,
    ComputeNodeResourceStatResults,
    CompleteProcessesCommand,
    UpdatePidsCommand,
    ShutDownCommand,
    ProcessStatResults,
    ResourceType,
)
from resource_monitor.resource_monitor import run_monitor_async
from resource_monitor.timing.timer_stats import Timer
from torc.swagger_client import DefaultApi
from torc.swagger_client.models.workflow_compute_nodes_model import WorkflowComputeNodesModel
from torc.swagger_client.models.workflow_compute_node_stats_model import (
    WorkflowComputeNodeStatsModel,
)
from torc.swagger_client.models.workflowsworkflowcompute_node_stats_stats import (
    WorkflowsworkflowcomputeNodeStatsStats,
)
from torc.swagger_client.models.edges_name_model import EdgesNameModel
from torc.swagger_client.models.workflow_job_process_stats_model import (
    WorkflowJobProcessStatsModel,
)
from torc.swagger_client.models.key_prepare_jobs_for_submission_model import (
    KeyPrepareJobsForSubmissionModel,
)
from torc.swagger_client.models.workflows_model import WorkflowsModel

import torc.version
from torc.api import send_api_command, iter_documents
from torc.common import JOB_STDIO_DIR, STATS_DIR, timer_stats_collector
from torc.exceptions import InvalidParameter
from torc.utils.cpu_affinity_mask_tracker import CpuAffinityMaskTracker
from torc.utils.filesystem_factory import make_path
from torc.utils.run_command import run_command
from .async_cli_command import AsyncCliCommand
from .common import KiB, MiB, GiB, TiB

JOB_COMPLETION_POLL_INTERVAL = 60

logger = logging.getLogger(__name__)
_g_shutdown = False


class JobRunner:
    """Runs jobs on a compute node"""

    def __init__(
        self,
        api: DefaultApi,
        workflow: WorkflowsModel,
        output_dir: Path,
        job_completion_poll_interval=JOB_COMPLETION_POLL_INTERVAL,
        max_parallel_jobs=None,
        database_poll_interval=600,
        time_limit=None,
        end_time=None,
        resources=None,
        scheduler_config_id=None,
        log_prefix=None,
        cpu_affinity_cpus_per_job=None,
        is_subtask=False,
    ):
        """Constructs a JobRunner.

        Parameters
        ----------
        api : DefaultApi
        output_dir : Path
            Directory for output files
        job_completion_poll_interval : int
            Interval in seconds in which to poll for job completions.
        max_parallel_jobs : int | None
            Maximum number of jobs that can run in parallel. If None (default), rely on resource
            constraints.
        database_poll_interval : int
            Max time in seconds in which the code should poll for job updates in the database.
        end_time : None | datetime
            If None then there is no time limit.
        time_limit : None | str
            ISO 8601 time duration string. If None then there is no time limit.
            Mutually exclusive with end_time.
        resources : None | KeyPrepareJobsForSubmissionModel
            Resources of the compute node. If None, make system calls to check resources.
        scheduler_config_id : str
            ID of the scheduler config used to acquire this compute node.
            If set, use this ID to pull matching jobs. If not set, pull any job that meets the
            resource availability.
        log_prefix : str
            Prefix to use for job-specific log files.
        is_subtask : bool
            Set to True if this is a subtask and multiple instances are running on one node.
        """
        if time_limit is not None and end_time is not None:
            raise Exception("time_limit and end_time are mutually exclusive")

        # TODO: too many inputs and too complex. Needs refactoring.
        self._api = api
        self._workflow = workflow
        self._run_id = send_api_command(api.get_workflows_key_status, workflow.key).run_id
        self._outstanding_jobs = {}
        self._pids = {}
        self._jobs_pending_process_stat_completion = []
        self._hostname = socket.gethostname()
        self._job_stdio_dir = output_dir / JOB_STDIO_DIR
        self._poll_interval = job_completion_poll_interval
        self._max_parallel_jobs = max_parallel_jobs
        self._db_poll_interval = database_poll_interval
        self._output_dir = output_dir
        self._log_prefix = log_prefix
        self._parent_monitor_conn = None
        self._monitor_proc = None
        self._end_time = end_time
        if time_limit is not None:
            self._end_time = datetime.now() + timedelta(seconds=_get_timeout(time_limit))
        if resources is None:
            self._scheduler_config_id = scheduler_config_id
        else:
            self._scheduler_config_id = resources.scheduler_config_id

        self._orig_resources = resources or _get_system_resources()
        if cpu_affinity_cpus_per_job is not None:
            if not hasattr(os, "sched_setaffinity"):
                raise InvalidParameter("This platform does not support sched_setaffinity")

            num_cpus = self._orig_resources.num_cpus
            if cpu_affinity_cpus_per_job > num_cpus:
                raise InvalidParameter(
                    f"{cpu_affinity_cpus_per_job=} cannot be greater than {num_cpus=}"
                )
            self._cpu_tracker = CpuAffinityMaskTracker(num_cpus, cpu_affinity_cpus_per_job)
            num_masks = self._cpu_tracker.get_num_masks()
            if self._max_parallel_jobs is not None and self._max_parallel_jobs < num_masks:
                raise InvalidParameter(f"{max_parallel_jobs=} cannot be less than {num_masks=}")
        else:
            self._cpu_tracker = None

        self._orig_resources.scheduler_config_id = self._scheduler_config_id
        self._resources = KeyPrepareJobsForSubmissionModel(**self._orig_resources.to_dict())
        self._last_db_poll_time = 0
        self._compute_node = None
        self._stats = ComputeNodeResourceStatConfig(
            **(
                api.get_workflows_key_config(
                    self._workflow.key
                ).compute_node_resource_stats.to_dict()
            )
        )
        if is_subtask:
            logger.info("Disable overall compute node stats monitoring for a subtask.")
            self._stats.disable_node_stats()
        self._stats_dir = output_dir / STATS_DIR
        self._job_stdio_dir.mkdir(exist_ok=True)
        self._stats_dir.mkdir(exist_ok=True)

    def __del__(self):
        if self._outstanding_jobs:
            logger.warning(
                "JobRunner destructed with outstanding jobs: %s",
                self._outstanding_jobs.keys(),
            )
        if self._parent_monitor_conn is not None or self._monitor_proc is not None:
            logger.warning("JobRunner destructed without stopping the resource monitor process.")

    def run_worker(self, scheduler=None):
        """Run jobs from a worker process.

        Parameters
        ----------
        scheduler : None | dict
            Scheduler configuration parameters. Used only for logs and events.

        """
        signal.signal(signal.SIGTERM, _sigterm_handler)
        self._log_worker_start_event()
        logger.info("Run worker with resources %s", str(self._resources).replace("\n", " "))
        self._create_compute_node(scheduler)
        if self._stats.is_enabled():
            self._start_resource_monitor()

        try:
            self._run_until_complete()
        finally:
            if self._parent_monitor_conn is not None:
                self._stop_resource_monitor()
            self._complete_compute_node()

    def _run_until_complete(self):
        os.environ["TORC_WORKFLOW_KEY"] = self._workflow.key
        result = send_api_command(self._api.get_workflows_key_is_complete, self._workflow.key)
        short_poll_interval = 3
        last_job_poll_time = 0
        while (
            not _g_shutdown
            and not result.is_complete
            and (self._end_time is None or datetime.now() < self._end_time)
        ):
            cur_time = time.time()
            if cur_time - last_job_poll_time < self._poll_interval:
                # This allows us to detect shutdown on a quicker interval.
                time.sleep(short_poll_interval)
                continue
            last_job_poll_time = cur_time

            num_completed = self._process_completions()
            num_started = 0
            reason_none_started = None
            if (
                num_completed > 0 or self._is_time_to_poll_database() or not self._outstanding_jobs
            ) and (
                self._max_parallel_jobs is None
                or len(self._outstanding_jobs) < self._max_parallel_jobs
            ):
                num_started, reason_none_started = self._run_ready_jobs()

            if num_started == 0 and not self._outstanding_jobs:
                if send_api_command(
                    self._api.get_workflows_key_is_complete, self._workflow.key
                ).is_complete:
                    logger.info("Workflow is complete.")
                else:
                    # TODO: if there is remaining time for this node, consider waiting for new
                    # jobs to become available.
                    logger.info(
                        "No jobs are outstanding on this node and no new jobs are available. "
                        "Reason no jobs started: %s",
                        reason_none_started,
                    )
                break

            if num_started > 0:
                self._update_pids_to_monitor()
            if num_completed > 0:
                self._handle_completed_process_stats()
                self._update_pids_to_monitor()

            time.sleep(short_poll_interval)
            result = send_api_command(self._api.get_workflows_key_is_complete, self._workflow.key)

        schedule_result = send_api_command(
            self._api.post_workflows_key_prepare_jobs_for_scheduling,
            self._workflow.key,
        )
        for scheduler_id in schedule_result.schedulers:
            self._schedule_compute_nodes(scheduler_id)

        if result.is_canceled:
            logger.info("Detected a canceled workflow. Cancel all outstanding jobs and exit.")
            self._cancel_jobs(list(self._outstanding_jobs.values()))

        self._terminate_jobs(list(self._outstanding_jobs.values()))

        self._pids.clear()
        self._handle_completed_process_stats()

    def _schedule_compute_nodes(self, scheduler_id):
        if scheduler_id.startswith("slurm_schedulers"):
            self._schedule_slurm_compute_nodes(scheduler_id)
        else:
            logger.error("Compute node scheduler %s is not supported", scheduler_id)

    def _schedule_slurm_compute_nodes(self, scheduler_id):
        key = scheduler_id.split("/")[1]
        cmd = (
            f"torc -k {self._workflow.key} -u {self._api.api_client.configuration.host} "
            f"hpc slurm schedule-nodes -n 1 "
            f"-o {self._output_dir} -p {self._poll_interval} -s {key}"
        )
        ret = run_command(cmd, num_retries=2)
        if ret == 0:
            logger.info("Scheduled compute nodes with cmd=%s", cmd)
            self._log_worker_schedule_event(scheduler_id)
        else:
            logger.error("Failed to schedule compute nodes: %s", ret)

    def _create_compute_node(self, scheduler):
        compute_node = WorkflowComputeNodesModel(
            hostname=self._hostname,
            pid=os.getpid(),
            start_time=str(datetime.now()),
            resources=self._orig_resources,
            is_active=True,
            scheduler=scheduler or {},
        )
        self._compute_node = send_api_command(
            self._api.post_workflows_workflow_compute_nodes,
            compute_node,
            self._workflow.key,
        )
        logger.info(
            "Running on compute node hostname=%s key=%s",
            self._hostname,
            compute_node.key,
        )

    def _complete_compute_node(self):
        self._compute_node.is_active = False
        self._compute_node.duration_seconds = (
            time.time()
            - datetime.strptime(self._compute_node.start_time, "%Y-%m-%d %H:%M:%S.%f").timestamp()
        )
        try:
            send_api_command(
                self._api.put_workflows_workflow_compute_nodes_key,
                self._compute_node,
                self._workflow.key,
                self._compute_node.key,
            )
        except Exception:  # pylint: disable=broad-exception-caught
            logger.exception("Failed to put_workflows_workflow_compute_nodes_key")

    def _complete_job(self, job, result, status):
        job = send_api_command(
            self._api.post_workflows_workflow_jobs_key_complete_job_status_rev,
            result,
            self._workflow.key,
            job.id,
            status,
            job.rev,
        )
        return job

    def _current_memory_allocation_percentage(self):
        return self._resources.memory_gb / self._orig_resources.memory_gb * 100

    def _decrement_resources(self, job):
        job_resources = send_api_command(
            self._api.get_workflows_workflow_jobs_key_resource_requirements,
            self._workflow.key,
            job.key,
        )
        job_memory_gb = get_memory_gb(job_resources.memory)
        self._resources.num_cpus -= job_resources.num_cpus
        self._resources.num_gpus -= job_resources.num_gpus
        self._resources.memory_gb -= job_memory_gb
        assert self._resources.num_cpus >= 0.0, self._resources.num_cpus
        assert self._resources.num_gpus >= 0.0, self._resources.num_gpus
        assert self._resources.memory_gb >= 0.0, self._resources.memory_gb

    def _increment_resources(self, job):
        job_resources = send_api_command(
            self._api.get_workflows_workflow_jobs_key_resource_requirements,
            self._workflow.key,
            job.key,
        )
        job_memory_gb = get_memory_gb(job_resources.memory)
        self._resources.num_cpus += job_resources.num_cpus
        self._resources.num_gpus += job_resources.num_gpus
        self._resources.memory_gb += job_memory_gb
        assert self._resources.num_cpus <= self._orig_resources.num_cpus, self._resources.num_cpus
        assert self._resources.num_gpus <= self._orig_resources.num_gpus, self._resources.num_gpus
        assert (
            self._resources.memory_gb <= self._orig_resources.memory_gb
        ), self._resources.memory_gb

    def _is_time_to_poll_database(self):
        if (time.time() - self._db_poll_interval) < self._last_db_poll_time:
            return False

        # TODO: needs to be more sophisticated
        # The main point is to provide a way to avoid hundreds of compute nodes unnecessarily
        # asking the database for jobs when it's highly unlikely to get any.
        # It would be better if the database or some middleware could publish events when
        # new jobs are ready to run.
        return self._resources.num_cpus > 0 and self._current_memory_allocation_percentage() > 10

    def _log_worker_start_event(self):
        send_api_command(
            self._api.post_workflows_workflow_events,
            {
                "category": "worker",
                "type": "start",
                "node_name": self._hostname,
                "torc_version": torc.version.__version__,
                "message": f"Started worker {self._hostname}",
            },
            self._workflow.key,
        )

    def _log_worker_schedule_event(self, scheduler_id):
        send_api_command(
            self._api.post_workflows_workflow_events,
            {
                "category": "worker",
                "type": "schedule",
                "node_name": self._hostname,
                "scheduler_id": scheduler_id,
                "message": f"Scheduled compute node(s) for user with {scheduler_id=}",
            },
            self._workflow.key,
        )

    def _log_job_start_event(self, job_key: str):
        send_api_command(
            self._api.post_workflows_workflow_events,
            {
                "category": "job",
                "type": "start",
                "key": job_key,
                "node_name": self._hostname,
                "message": f"Started job {job_key}",
            },
            self._workflow.key,
        )

    def _log_job_complete_event(self, job_key: str, status: str):
        send_api_command(
            self._api.post_workflows_workflow_events,
            {
                "category": "job",
                "type": "complete",
                "key": job_key,
                "status": status,
                "node_name": self._hostname,
                "message": f"Completed job {job_key}",
            },
            self._workflow.key,
        )

    def _process_completions(self):
        done_jobs = []
        for job in self._outstanding_jobs.values():
            if job.is_complete():
                done_jobs.append(job)
                # TODO: check return code first
                self._update_file_info(job)

        for job in done_jobs:
            self._cleanup_job(job, "done")

        if done_jobs:
            logger.info("Found %s completions", len(done_jobs))
        else:
            logger.debug("Found 0 completions")
        return len(done_jobs)

    def _cancel_jobs(self, jobs):
        for job in jobs:
            # Note that the database API service changes job status to canceled.
            job.cancel()
            logger.info("Canceled job key=%s name=%s", job.key, job.db_job.name)

        status = "canceled"
        for job in jobs:
            job.wait_for_completion(status)
            assert job.is_complete()
            job.db_job = send_api_command(
                self._api.get_workflows_workflow_jobs_key,
                self._workflow.key,
                job.key,
            )
            self._cleanup_job(job, status)

    def _terminate_jobs(self, jobs):
        terminated_jobs = []
        for job in jobs:
            if job.db_job.supports_termination:
                job.terminate()
                logger.info("Terminated job key=%s name=%s", job.key, job.db_job.name)
                terminated_jobs.append(job)

        status = "terminated"
        for job in terminated_jobs:
            job.wait_for_completion("terminated")
            assert job.is_complete()
            self._cleanup_job(job, status)

    def _cleanup_job(self, job: AsyncCliCommand, status):
        self._outstanding_jobs.pop(job.key)
        self._increment_resources(job.db_job)
        result = job.get_result(self._run_id)
        self._log_job_complete_event(job.key, status)
        self._complete_job(job.db_job, result, status)
        if self._stats.process:
            self._jobs_pending_process_stat_completion.append(job.key)
            self._pids.pop(job.key)

    def _run_job(self, job: AsyncCliCommand):
        job.run(self._output_dir)
        # The database changes db_job._rev on every update.
        # This reassigns job.db_job in order to stay current.
        job.db_job = send_api_command(
            self._api.put_workflows_workflow_jobs_key_manage_status_change_status_rev,
            self._workflow.key,
            job.key,
            "submitted",
            job.db_job.rev,
        )
        self._outstanding_jobs[job.key] = job
        if self._stats.process:
            self._pids[job.key] = job.pid
        send_api_command(
            self._api.post_workflows_workflow_edges_name,
            EdgesNameModel(
                _from=self._compute_node.id,
                to=job.db_job.id,
            ),
            self._workflow.key,
            "executed",
        )
        logger.debug("Started job %s", job.key)
        self._log_job_start_event(job.key)

    def _run_ready_jobs(self):
        reason_none_started = None
        if self._end_time is not None:
            self._resources.time_limit = convert_end_time_to_duration_str(self._end_time)
        kwargs = {}
        if self._max_parallel_jobs is not None:
            kwargs["limit"] = self._max_parallel_jobs
        ready_jobs = send_api_command(
            self._api.post_workflows_key_prepare_jobs_for_submission,
            self._resources,
            self._workflow.key,
            **kwargs,
        )
        if ready_jobs.jobs:
            logger.info("%s jobs are ready for submission", len(ready_jobs.jobs))
        else:
            reason_none_started = ready_jobs.reason
        for job in ready_jobs.jobs:
            self._run_job(
                AsyncCliCommand(
                    job,
                    log_prefix=self._log_prefix,
                    cpu_affinity_tracker=self._cpu_tracker,
                )
            )
            self._decrement_resources(job)

        self._last_db_poll_time = time.time()
        return len(ready_jobs.jobs), reason_none_started

    def _start_resource_monitor(self):
        self._parent_monitor_conn, child_conn = multiprocessing.Pipe()
        pids = self._pids if self._stats.process else None
        monitor_log_file = self._output_dir / f"monitor_{self._compute_node.key}.log"
        logger.info("Start resource monitor with %s", json.dumps(self._stats.model_dump()))
        if self._stats.monitor_type == "aggregation":
            args = (child_conn, self._stats, pids, monitor_log_file, None)
        elif self._stats.monitor_type == "periodic":
            db_file = self._stats_dir / f"compute_node_{self._compute_node.key}.sqlite"
            args = (child_conn, self._stats, pids, monitor_log_file, db_file)
        else:
            raise Exception(f"Unsupported monitor_type={self._stats.monitor_type}")
        self._monitor_proc = multiprocessing.Process(target=run_monitor_async, args=args)
        self._monitor_proc.start()

    def _stop_resource_monitor(self):
        self._parent_monitor_conn.send(ShutDownCommand(pids=self._pids))
        has_results = False
        for _ in range(30):
            if self._parent_monitor_conn.poll():
                has_results = True
                break
            time.sleep(1)
        if has_results:
            system_results, _ = self._parent_monitor_conn.recv()
            if system_results.results:
                self._post_compute_node_stats(system_results)
        else:
            logger.error("Failed to receive results from resource monitor.")
        self._monitor_proc.join()
        self._parent_monitor_conn = None
        self._monitor_proc = None

    def _handle_completed_process_stats(self):
        if self._stats.process:
            self._parent_monitor_conn.send(
                CompleteProcessesCommand(
                    pids=self._pids,
                    completed_process_keys=self._jobs_pending_process_stat_completion,
                )
            )
            with Timer(timer_stats_collector, "receive_process_stats"):
                results = self._parent_monitor_conn.recv()
            stats = []
            for result in results.results:
                self._post_job_process_stats(result)
                # These json methods let Pydantic run its data type conversions.
                x = json.loads(result.model_dump_json())
                x["job_key"] = x.pop("process_key")
                stats.append(WorkflowsworkflowcomputeNodeStatsStats(**x))
            if stats:
                send_api_command(
                    self._api.post_workflows_workflow_compute_node_stats,
                    WorkflowComputeNodeStatsModel(
                        hostname=self._hostname,
                        stats=stats,
                        timestamp=str(datetime.now()),
                    ),
                    self._workflow.key,
                )
            self._jobs_pending_process_stat_completion.clear()

    def _update_pids_to_monitor(self):
        if self._stats.process:
            self._parent_monitor_conn.send(UpdatePidsCommand(config=self._stats, pids=self._pids))

    def _post_compute_node_stats(self, results: ComputeNodeResourceStatResults):
        res = send_api_command(
            self._api.post_workflows_workflow_compute_node_stats,
            WorkflowComputeNodeStatsModel(
                hostname=self._hostname,
                # These json methods let Pydantic run its data type conversions.
                stats=[
                    WorkflowsworkflowcomputeNodeStatsStats(**json.loads(x.model_dump_json()))
                    for x in results.results
                ],
                timestamp=str(datetime.now()),
            ),
            self._workflow.key,
        )
        send_api_command(
            self._api.post_workflows_workflow_edges_name,
            EdgesNameModel(
                _from=self._compute_node.id,
                to=res.id,
            ),
            self._workflow.key,
            "node_used",
        )

        for result in results.results:
            assert result.resource_type != ResourceType.PROCESS, result

    def _post_job_process_stats(self, result: ProcessStatResults):
        res = send_api_command(
            self._api.post_workflows_workflow_job_process_stats,
            WorkflowJobProcessStatsModel(
                avg_cpu_percent=result.average["cpu_percent"],
                max_cpu_percent=result.maximum["cpu_percent"],
                avg_rss=result.average["rss"],
                max_rss=result.maximum["rss"],
                num_samples=result.num_samples,
                job_key=result.process_key,
                run_id=self._run_id,
                timestamp=str(datetime.now()),
            ),
            self._workflow.key,
        )
        send_api_command(
            self._api.post_workflows_workflow_edges_name,
            EdgesNameModel(
                _from=f"jobs__{self._workflow.key}/{result.process_key}",
                to=res.id,
            ),
            self._workflow.key,
            "process_used",
        )

    def _update_file_info(self, job):
        for file in iter_documents(
            self._api.get_workflows_workflow_files_produced_by_job_key,
            self._workflow.key,
            job.key,
        ):
            path = make_path(file.path)
            if not path.exists():
                logger.warning(
                    "Job %s should have produced file %s, but it does not exist",
                    job.key,
                    file.path,
                )
                continue
            # file.file_hash = compute_file_hash(path)
            file.st_mtime = path.stat().st_mtime
            send_api_command(
                self._api.put_workflows_workflow_files_key,
                file,
                self._workflow.key,
                file.key,
            )


def _get_system_resources():
    return KeyPrepareJobsForSubmissionModel(
        num_cpus=psutil.cpu_count(),
        memory_gb=psutil.virtual_memory().total / GiB,
        num_nodes=1,
        time_limit=None,
        num_gpus=_get_num_gpus(),
    )


def get_memory_gb(memory):
    """Converts a memory defined as a string to GiB.

    Parameters
    ----------
    memory : str
        Memory as string with units, such as '10g'

    Returns
    -------
    int
    """
    return get_memory_in_bytes(memory) / GiB


def get_memory_in_bytes(memory: str):
    """Converts a memory defined as a string to bytes.

    Parameters
    ----------
    memory : str
        Memory as string with units, such as '10g'

    Returns
    -------
    int
    """
    match = re.search(r"^([0-9]+)$", memory)
    if match is not None:
        return int(match.group(1))

    match = re.search(r"^([0-9]+)\s*([kmgtKMGT])$", memory)
    if match is None:
        raise ValueError(f"{memory} is an invalid memory value")

    size = int(match.group(1))
    units = match.group(2).lower()
    if units == "k":
        size *= KiB
    elif units == "m":
        size *= MiB
    elif units == "g":
        size *= GiB
    elif units == "t":
        size *= TiB
    else:
        raise ValueError(f"{units} is an invalid memory unit")

    return size


# This pydantic code will convert ISO 8601 duration strings to timedelta.
class _TimeLimitModel(BaseModel):
    model_config = ConfigDict(ser_json_timedelta="iso8601")

    time_limit: timedelta


def convert_end_time_to_duration_str(end_time: datetime):
    """Convert an end time timestamp to an ISO 8601 duration string, relative to current time."""
    duration = end_time - datetime.now()
    return json.loads(_TimeLimitModel(time_limit=duration).model_dump_json())["time_limit"]


def _get_timeout(time_limit):
    return (
        sys.maxsize
        if time_limit is None
        else _TimeLimitModel(time_limit=time_limit).time_limit.total_seconds()
    )


def _get_num_gpus():
    # Here is example output:
    # nvidia-smi --list-gpus
    # GPU 0: Tesla V100-PCIE-16GB (UUID: GPU-b96a6fce-c5a4-079e-d922-5e9d21b063ce)
    # GPU 1: Tesla V100-PCIE-16GB (UUID: GPU-e57626ea-9c0c-3ceb-06e1-f926467b98ad)

    # TODO: do we need to support other GPUs? Is there a standard way to find them?
    if shutil.which("nvidia-smi") is None:
        return 0

    proc = subprocess.run(["nvidia-smi", "--list-gpus"], stdout=subprocess.PIPE, check=False)
    if proc.returncode == 0:
        gpus = [
            x
            for x in proc.stdout.decode("utf-8").strip().split("\n")
            if x.strip().startswith("GPU")
        ]
        return len(gpus)
    return 0


def _sigterm_handler(signum, frame):  # pylint: disable=unused-argument
    global _g_shutdown  # pylint: disable=global-statement
    logger.info("Detected SIGTERM. Terminate jobs and shutdown.")
    _g_shutdown = True
