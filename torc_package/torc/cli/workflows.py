"""CLI commands to manage a workflow"""

import getpass
import json
import logging
import math
import sys
from pathlib import Path
from typing import Iterable, Optional

import click
import json5
from torc.openapi_client.models.job_model import JobModel
from torc.openapi_client.models.workflow_model import WorkflowModel
from torc.openapi_client.models.workflow_specification_model import (
    WorkflowSpecificationModel,
)

from torc.api import remove_db_keys, sanitize_workflow, iter_documents, list_model_fields
from torc.exceptions import InvalidWorkflow
from torc.hpc.slurm_interface import SlurmInterface
from torc.openapi_client.api import DefaultApi
from torc.torc_rc import TorcRuntimeConfig
from torc.workflow_manager import WorkflowManager
from .common import (
    check_database_url,
    confirm_change,
    get_workflow_key_from_context,
    get_output_format_from_context,
    get_user_from_context,
    setup_cli_logging,
    parse_filters,
    print_items,
)


logger = logging.getLogger(__name__)


@click.group()
def workflows():
    """Workflow commands"""


@click.command()
@click.argument("workflow_keys", nargs=-1)
@click.pass_obj
@click.pass_context
def cancel(ctx, api: DefaultApi, workflow_keys: tuple[str]) -> None:
    """Cancel one or more workflows."""
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    if not workflow_keys:
        logger.warning("No workflow keys were passed")
        return

    msg = "This command will cancel all specified workflows."
    confirm_change(ctx, msg)

    for key in workflow_keys:
        cancel_workflow(api, key)


@click.command()
@click.pass_obj
@click.pass_context
def cancel_all(ctx, api: DefaultApi) -> None:
    """Cancel all active workflows being run by the current user."""
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    user = get_user_from_context(ctx)
    keys = [x.key for x in iter_documents(api.list_workflows, user=user)]

    final_keys = []
    for key in keys:
        if has_scheduled_compute_nodes(api, key):
            final_keys.append(key)

    if not final_keys:
        logger.info("There are no active workflows to cancel.")
        return

    keys_str = " ".join(final_keys)
    msg = f"This command will cancel these workflow keys: {keys_str}"
    confirm_change(ctx, msg)
    for key in final_keys:
        cancel_workflow(api, key)


@click.command()
@click.option(
    "-U",
    "--update-rc-with-key",
    is_flag=True,
    default=False,
    show_default=True,
    help="Update torc runtime config file with the created workflow key.",
)
@click.option(
    "-d",
    "--description",
    type=str,
    help="Workflow description",
)
@click.option(
    "-k",
    "--key",
    type=str,
    help="Workflow key. Default is to auto-generate",
)
@click.option(
    "-n",
    "--name",
    type=str,
    help="Workflow name",
)
@click.pass_obj
@click.pass_context
def create(
    ctx: click.Context,
    api: DefaultApi,
    update_rc_with_key: bool,
    description: str,
    key: str,
    name: str,
) -> None:
    """Create a new workflow."""
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    workflow = WorkflowModel(
        description=description,
        _key=key,
        name=name,
        user=get_user_from_context(ctx),
    )
    output_format = get_output_format_from_context(ctx)
    workflow = api.add_workflow(workflow)
    if output_format == "text":
        logger.info("Created a workflow with key=%s", workflow.key)
    else:
        print(json.dumps({"key": workflow.key}))
    if update_rc_with_key:
        _update_torc_rc(api, workflow)


@click.command()
@click.argument("filename", type=click.Path(exists=True))
@click.option(
    "-U",
    "--update-rc-with-key",
    is_flag=True,
    default=False,
    show_default=True,
    help="Update torc runtime config file with the created workflow key.",
)
@click.option(
    "-d",
    "--description",
    type=str,
    help="Workflow description",
)
@click.option(
    "-k",
    "--key",
    type=str,
    help="Workflow key. Default is to auto-generate",
)
@click.option(
    "-n",
    "--name",
    type=str,
    help="Workflow name",
)
@click.pass_obj
@click.pass_context
def create_from_commands_file(
    ctx: click.Context,
    api: DefaultApi,
    update_rc_with_key: bool,
    filename: Path,
    description: str,
    key: str,
    name: str,
) -> None:
    """Create a workflow from a text file containing job CLI commands."""
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    output_format = get_output_format_from_context(ctx)
    commands = []
    with open(filename, encoding="utf-8") as f_in:
        for line in f_in:
            line = line.strip()
            if line:
                commands.append(line)
    workflow = WorkflowModel(
        description=description,
        _key=key,
        name=name,
        user=get_user_from_context(ctx),
    )
    workflow = api.add_workflow(workflow)
    assert workflow.key is not None
    if output_format == "text":
        logger.info("Created a workflow from %s with key=%s", filename, workflow.key)
    else:
        print(json.dumps({"filename": filename, "key": workflow.key}))
    for i, command in enumerate(commands, start=1):
        name = str(i)
        api.add_job(workflow.key, JobModel(name=name, command=command))
    if update_rc_with_key:
        _update_torc_rc(api, workflow)


@click.command()
@click.argument("filename", type=click.Path(exists=True), callback=lambda *x: Path(x[2]))
@click.option(
    "-U",
    "--update-rc-with-key",
    is_flag=True,
    default=False,
    show_default=True,
    help="Update torc runtime config file with the created workflow key.",
)
@click.pass_obj
@click.pass_context
def create_from_json_file(
    ctx: click.Context, api: DefaultApi, filename: Path, update_rc_with_key: bool
) -> None:
    """Create a workflow from a JSON/JSON5 file."""
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    workflow = create_workflow_from_json_file(api, filename, user=get_user_from_context(ctx))

    output_format = get_output_format_from_context(ctx)
    if output_format == "text":
        logger.info("Created a workflow from %s with key=%s", filename, workflow.key)
    else:
        print(json.dumps({"filename": str(filename), "key": workflow.key}))
    if update_rc_with_key:
        _update_torc_rc(api, workflow)


@click.command()
@click.argument("workflow_key")
@click.option(
    "-a",
    "--archive",
    help="Set to 'true' to archive the workflow or 'false' to enable it.",
)
@click.option(
    "-d",
    "--description",
    type=str,
    help="Workflow description",
)
@click.option(
    "-n",
    "--name",
    type=str,
    help="Workflow name",
)
@click.pass_obj
@click.pass_context
def modify(
    ctx: click.Context,
    api: DefaultApi,
    workflow_key: str,
    archive: str,
    description: str,
    name: str,
) -> None:
    """Modify the workflow parameters."""
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    workflow = api.get_workflow(workflow_key)
    if archive is not None:
        archive_lowered = archive.lower()
        if archive_lowered not in ("true", "false"):
            logger.error("--archive must be 'true' or 'false': %s", archive)
        workflow.is_archived = True if archive_lowered == "true" else False
    if description is not None:
        workflow.description = description
    if name is not None:
        workflow.name = name
    workflow.user = get_user_from_context(ctx)
    workflow = api.modify_workflow(workflow_key, workflow)
    logger.info("Updated workflow %s", workflow.key)


@click.command()
@click.argument("workflow_keys", nargs=-1)
@click.pass_obj
@click.pass_context
def delete(ctx: click.Context, api: DefaultApi, workflow_keys: tuple[str]):
    """Delete one or more workflows by key."""
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    if not workflow_keys:
        logger.warning("No workflow keys were passed")
        return

    _delete_workflows_with_warning(ctx, api, workflow_keys)


@click.command()
@click.pass_obj
@click.pass_context
def delete_all(ctx: click.Context, api: DefaultApi) -> None:
    """Delete all workflows stored by the user."""
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    user = get_user_from_context(ctx)
    keys = [x.key for x in iter_documents(api.list_workflows, user=user)]
    _delete_workflows_with_warning(ctx, api, keys)


def _delete_workflows_with_warning(
    ctx: click.Context, api: DefaultApi, keys: Iterable[str]
) -> None:
    items = [api.get_workflow(x).to_dict() for x in keys]
    columns = list_model_fields(WorkflowModel)
    columns.remove("_id")
    columns.remove("_rev")
    print_items(
        ctx,
        items,
        "Workflows",
        columns,
        "workflows",
    )
    msg = "This command will delete the workflows above. Continue?"
    confirm_change(ctx, msg)
    current_user = get_user_from_context(ctx)
    for i, key in enumerate(keys):
        user = items[i]["user"]
        if user != current_user:
            msg = f"Workflow {key} was created by {user=}, not {current_user=}. Continue?"
            confirm_change(ctx, msg)

        if has_scheduled_compute_nodes(api, key):
            msg = f"Workflow {key} has pending or running compute nodes. Continue?"
            confirm_change(ctx, msg)
            cancel_workflow(api, key)

        api.remove_workflow(key)
        logger.info("Deleted workflow %s", key)


@click.command()
@click.pass_obj
@click.pass_context
def list_scheduler_configs(ctx: click.Context, api: DefaultApi) -> None:
    """List the scheduler configs in the database."""
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    output_format = get_output_format_from_context(ctx)
    workflow_key = get_workflow_key_from_context(ctx, api)
    items = []
    for scheduler in ("aws_schedulers", "local_schedulers", "slurm_schedulers"):
        method = getattr(api, f"list_{scheduler}")
        for doc in iter_documents(method, workflow_key):
            items.append(doc.id)

    if output_format == "text":
        logger.info("Scheduler configs in workflow %s", workflow_key)
        for item in items:
            print(item)
    else:
        print(json.dumps({"ids": items}))


@click.command(name="list")
@click.option(
    "-A",
    "--only-archived",
    is_flag=True,
    default=False,
    show_default=True,
    help="List only workflows that have been archived.",
)
@click.option(
    "-i",
    "--include-archived",
    is_flag=True,
    default=False,
    show_default=True,
    help="Include archived workflows in the list.",
)
@click.option(
    "-a",
    "--all-users",
    is_flag=True,
    default=False,
    help="List workflows for all users. Default is only for the current user.",
)
@click.option(
    "-f",
    "--filters",
    multiple=True,
    type=str,
    help="Filter the values according to each key=value pair.",
)
@click.option(
    "--sort-by",
    type=str,
    help="Sort results by this column.",
)
@click.option(
    "--reverse-sort",
    is_flag=True,
    default=False,
    show_default=True,
    help="Reverse the sort order if --sort-by is set.",
)
@click.pass_obj
@click.pass_context
def list_workflows(
    ctx: click.Context,
    api: DefaultApi,
    only_archived: bool,
    include_archived: bool,
    all_users: bool,
    filters: tuple[str],
    sort_by: Optional[str],
    reverse_sort: bool,
):
    """List all workflows stored by the user.

    \b
    1. List all workflows for the current user in a table.
       $ torc workflows list
    2. List all workflows in JSON format.
       $ torc -o json workflows list
    3. List only archived workflows.
       $ torc workflows list --only-archived
    4. List all workflows for all users, including archived workflows.
       $ torc workflows list --all-users --include-archived
    """
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    table_title = "Workflows"
    if only_archived and include_archived:
        logger.error("Only one of --only-archived and --include-archived can be set.")
        sys.exit(1)
    _filters = parse_filters(filters)
    if sort_by is not None:
        _filters["sort_by"] = sort_by
        _filters["reverse_sort"] = reverse_sort
    if not include_archived:
        _filters["is_archived"] = only_archived
    if not all_users:
        _filters["user"] = get_user_from_context(ctx)
    items = (x.to_dict() for x in iter_documents(api.list_workflows, **_filters))
    columns = list_model_fields(WorkflowModel)
    columns.remove("_id")
    columns.remove("_rev")
    print_items(
        ctx,
        items,
        table_title,
        columns,
        "workflows",
    )


@click.command()
@click.pass_obj
@click.pass_context
def process_auto_tune_resource_requirements_results(ctx: click.Context, api: DefaultApi) -> None:
    """Process the results of the first round of auto-tuning resource requirements."""
    setup_cli_logging(ctx, __name__)
    workflow_key = get_workflow_key_from_context(ctx, api)
    api.process_auto_tune_resource_requirements_results(workflow_key)
    url = api.api_client.configuration.host
    rr_cmd = f"torc -k {workflow_key} -u {url} resource-requirements list"
    events_cmd = f"torc -k {workflow_key} -u {url} events list -f category=resource_requirements"
    logger.info(
        "Updated resource requirements. Look at current requirements with "
        "\n  '%s'\n and at "
        "changes by reading the events with \n  '%s'\n",
        rr_cmd,
        events_cmd,
    )


@click.command()
@click.option(
    "-c",
    "--num-cpus",
    type=int,
    default=36,
    help="Number of CPUs per node",
    show_default=True,
)
@click.option(
    "-s",
    "--scheduler-config-id",
    type=str,
    help="Limit output to jobs assigned this scheduler config ID. Refer to list-scheduler-configs.",
)
@click.pass_obj
@click.pass_context
def recommend_nodes(
    ctx: click.Context, api: DefaultApi, num_cpus: int, scheduler_config_id: str
) -> None:
    """Recommend compute nodes to schedule."""
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    output_format = get_output_format_from_context(ctx)
    workflow_key = get_workflow_key_from_context(ctx, api)
    if scheduler_config_id is None:
        reqs = api.get_ready_job_requirements(workflow_key)
    else:
        reqs = api.get_ready_job_requirements(
            workflow_key, scheduler_config_id=scheduler_config_id
        )
    if reqs.num_jobs == 0:
        logger.error("No jobs are in the ready state. You may need to run 'torc workflows start'")
        sys.exit(1)

    num_nodes_by_cpus = math.ceil(reqs.num_cpus / num_cpus)
    if output_format == "text":
        print(f"Requirements for jobs in the ready state: \n{reqs}")
        print(f"Based on CPUs, number of required nodes = {num_nodes_by_cpus}")
    else:
        print(
            json.dumps(
                {
                    "ready_job_requirements": reqs.to_dict(),
                    "num_nodes_by_cpus": num_nodes_by_cpus,
                }
            )
        )


@click.command()
@click.argument("workflow_key")
@click.option(
    "-f",
    "--failed-only",
    is_flag=True,
    default=False,
    show_default=True,
    help="Only reset the status of failed jobs.",
)
@click.option(
    "-r",
    "--restart",
    is_flag=True,
    default=False,
    show_default=True,
    help="Send the 'workflows restart' command after resetting status.",
)
@click.pass_obj
@click.pass_context
def reset_status(
    ctx: click.Context, api: DefaultApi, workflow_key: str, failed_only: bool, restart: bool
) -> None:
    """Reset the status of the workflow and all jobs."""
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    workflow = api.get_workflow(workflow_key)
    msg = f"""This command will reset the status of this workflow:
    key: {workflow_key}
    user: {workflow.user}
    name: {workflow.name}
    description: {workflow.description}
"""
    if restart:
        msg += "\nAfter resetting status this command will restart the workflow."
    confirm_change(ctx, msg)
    reset_workflow_status(api, workflow_key)
    reset_workflow_job_status(api, workflow_key, failed_only=failed_only)
    if restart:
        restart_workflow(api, workflow_key)


@click.command()
@click.option(
    "-i",
    "--ignore-missing-data",
    is_flag=True,
    default=False,
    show_default=True,
    help="Ignore checks for missing files and user data documents.",
)
@click.option(
    "--only-uninitialized",
    is_flag=True,
    default=False,
    show_default=True,
    help="Only initialize jobs with a status of uninitialized.",
)
@click.pass_obj
@click.pass_context
def restart(
    ctx: click.Context, api: DefaultApi, ignore_missing_data: bool, only_uninitialized: bool
) -> None:
    """Restart the workflow defined in the database specified by the URL. Resets all jobs with
    a status of canceled, submitted, submitted_pending, and terminated. Does not affect jobs with
    a status of done.
    """
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    workflow_key = get_workflow_key_from_context(ctx, api)
    _exit_if_jobs_are_running(api, workflow_key)
    workflow = api.get_workflow(workflow_key)
    types = "uninitialized" if only_uninitialized else "failed/incomplete"
    msg = f"""This command will restart this workflow and reset {types} job statuses.
    key: {workflow_key}
    user: {workflow.user}
    name: {workflow.name}
    description: {workflow.description}
"""
    confirm_change(ctx, msg)
    restart_workflow(
        api,
        workflow_key,
        ignore_missing_data=ignore_missing_data,
        only_uninitialized=only_uninitialized,
    )


@click.command()
@click.option(
    "-a",
    "--auto-tune-resource-requirements",
    is_flag=True,
    default=False,
    show_default=True,
    help="Setup the workflow such that only one job from each resource group is run in the first "
    "round. Upon completion torc will look at actual resource utilization of those jobs and "
    "apply the results to the resource requirements definitions. When jobs finish, please call "
    "'torc workflows process_auto_tune_resource_requirements_results' to update the requirements.",
)
@click.option(
    "-i",
    "--ignore-missing-data",
    is_flag=True,
    default=False,
    show_default=True,
    help="Ignore checks for missing files and user data documents.",
)
@click.pass_obj
@click.pass_context
def start(
    ctx: click.Context,
    api: DefaultApi,
    auto_tune_resource_requirements: bool,
    ignore_missing_data: bool,
) -> None:
    """Start the workflow defined in the database specified by the URL."""
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    workflow_key = get_workflow_key_from_context(ctx, api)
    _exit_if_jobs_are_running(api, workflow_key)
    done_jobs = api.list_jobs(workflow_key, status="done", limit=1).items
    if done_jobs:
        workflow = api.get_workflow(workflow_key)
        msg = f"""This workflow has one or more jobs with a status of 'done.' This command will
reset all job statuses to 'uninitialized' and then 'ready' or 'blocked.'
    key: {workflow_key}
    user: {workflow.user}
    name: {workflow.name}
    description: {workflow.description}
"""
        confirm_change(ctx, msg)

    try:
        start_workflow(
            api,
            workflow_key,
            auto_tune_resource_requirements=auto_tune_resource_requirements,
            ignore_missing_data=ignore_missing_data,
        )
    except InvalidWorkflow as exc:
        logger.error("Invalid workflow: %s", exc)
        sys.exit(1)


# The functions below exist apart from the CLI functions so that the TUI can call them.


def create_workflow_from_json_file(
    api: DefaultApi, filename: Path, user: str = getpass.getuser()
) -> WorkflowModel:
    """Create a workflow from a JSON/JSON5 file."""
    method = json5.load if filename.suffix == ".json5" else json.load
    with open(filename, "r", encoding="utf-8") as f:
        data = sanitize_workflow(method(f))
    if data.get("user") != user:
        if "user" in data:
            logger.info("Overriding user=%s with %s", data["user"], user)
        data["user"] = user
    spec = WorkflowSpecificationModel(**data)
    return api.add_workflow_specification(spec)


def start_workflow(
    api: DefaultApi,
    workflow_key: str,
    auto_tune_resource_requirements: bool = False,
    ignore_missing_data: bool = False,
) -> None:
    """Starts the workflow."""
    mgr = WorkflowManager(api, workflow_key)
    mgr.start(
        auto_tune_resource_requirements=auto_tune_resource_requirements,
        ignore_missing_data=ignore_missing_data,
    )
    api.add_event(
        workflow_key,
        {
            "category": "workflow",
            "type": "start",
            "key": workflow_key,
            "message": f"Started workflow {workflow_key}",
        },
    )
    # TODO: This could schedule nodes.


def restart_workflow(
    api: DefaultApi,
    workflow_key: str,
    only_uninitialized: bool = False,
    ignore_missing_data: bool = False,
) -> None:
    """Restarts the workflow."""
    mgr = WorkflowManager(api, workflow_key)
    mgr.restart(ignore_missing_data=ignore_missing_data, only_uninitialized=only_uninitialized)
    api.add_event(
        workflow_key,
        {
            "category": "workflow",
            "type": "restart",
            "key": workflow_key,
            "message": f"Restarted workflow {workflow_key}",
        },
    )
    # TODO: This could schedule nodes.


def reset_workflow_status(api: DefaultApi, workflow_key: str) -> None:
    """Resets the status of the workflow."""
    api.reset_workflow_status(workflow_key)
    logger.info("Reset workflow status")
    api.add_event(
        workflow_key,
        {
            "category": "workflow",
            "type": "reset_status",
            "key": workflow_key,
            "message": f"Reset workflow {workflow_key}",
        },
    )


def reset_workflow_job_status(api: DefaultApi, workflow_key: str, failed_only: bool = False):
    """Resets the status of the workflow jobs."""
    api.reset_job_status(workflow_key, failed_only=failed_only)
    logger.info("Reset job status, failed_only=%s", failed_only)
    api.add_event(
        workflow_key,
        {
            "category": "workflow",
            "type": "reset_job_status",
            "key": workflow_key,
            "message": f"Reset workflow {workflow_key} job status",
        },
    )


def cancel_workflow(api: DefaultApi, workflow_key: str) -> None:
    """Cancels the workflow."""
    # TODO: Handling different scheduler types needs to be at a lower level.
    items = api.list_scheduled_compute_nodes(workflow_key).items
    assert items is not None
    for job in items:
        if (
            job.status != "complete"
            and job.scheduler_config_id.split("/")[0].split("__")[0] == "slurm_schedulers"
            and job.scheduler_id is not None
        ):
            assert job.key is not None
            intf = SlurmInterface()
            return_code = intf.cancel_job(job.scheduler_id)
            if return_code == 0:
                job.status = "complete"
                api.modify_scheduled_compute_node(workflow_key, job.key, job)
            # else: Ignore all return codes and try to cancel all jobs.
    api.cancel_workflow(workflow_key)
    logger.info("Canceled workflow %s", workflow_key)
    api.add_event(
        workflow_key,
        {
            "category": "workflow",
            "type": "cancel",
            "key": workflow_key,
            "message": f"Canceled workflow {workflow_key}",
        },
    )


def has_scheduled_compute_nodes(api: DefaultApi, workflow_key: str) -> bool:
    """Returns True if compute nodes are scheduled - could be pending or running."""
    items = api.list_scheduled_compute_nodes(workflow_key).items
    assert items is not None
    for job in items:
        if (
            job.status != "complete"
            and job.scheduler_config_id.split("/")[0].split("__")[0] == "slurm_schedulers"
            and job.scheduler_id is not None
        ):
            return True
    return False


def has_running_jobs(api: DefaultApi, workflow_key: str) -> bool:
    """Returns True if jobs are running."""
    submitted = api.list_jobs(workflow_key, status="submitted", limit=1)
    assert submitted.items is not None
    sub_pend = api.list_jobs(workflow_key, status="submitted_pending", limit=1)
    assert sub_pend.items is not None
    return len(submitted.items) > 0 or len(sub_pend.items) > 0


def _exit_if_jobs_are_running(api: DefaultApi, workflow_key: str) -> None:
    if has_running_jobs(api, workflow_key):
        logger.error(
            "This operation is not allowed on a workflow with 'submitted' jobs. Please allow "
            "the jobs to finish or cancel them."
        )
        sys.exit(1)


@click.command()
@click.option(
    "--sanitize/--no-santize",
    default=True,
    is_flag=True,
    show_default=True,
    help="Remove all database fields from workflow objects.",
)
@click.pass_obj
@click.pass_context
def show(ctx: click.Context, api: DefaultApi, sanitize: bool) -> None:
    """Show the workflow."""
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    workflow_key = get_workflow_key_from_context(ctx, api)
    data = api.get_workflow_specification(workflow_key).to_dict()
    if sanitize:
        sanitize_workflow(data)
    print(json.dumps(data, indent=2))


@click.command()
@click.option(
    "-e",
    "--expiration-buffer",
    type=int,
    help="Set the number of seconds before the expiration time at which torc will terminate jobs.",
)
@click.option(
    "-h",
    "--wait-for-healthy-db",
    type=int,
    help="Set the number of minutes that torc will tolerate an offline database.",
)
@click.option(
    "-i",
    "--ignore-workflow-completion",
    type=str,
    help="Set to 'true' to cause torc to ignore workflow completions and hold onto compute node "
    "allocations indefinitely. Useful for debugging failed jobs. Set to 'false' to revert to "
    "the default behavior.",
)
@click.option(
    "-w",
    "--wait-for-new-jobs",
    type=int,
    help="Set the number of seconds that torc will wait for new jobs before exiting. Does not "
    "apply if the workflow is complete.",
)
@click.pass_obj
@click.pass_context
def set_compute_node_parameters(
    ctx: click.Context,
    api: DefaultApi,
    expiration_buffer: int,
    wait_for_healthy_db: bool,
    ignore_workflow_completion: str,
    wait_for_new_jobs: bool,
) -> None:
    """Set parameters that control how the torc worker app behaves on compute nodes.
    Run 'torc workflows show-config' to see the current values."""
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    workflow_key = get_workflow_key_from_context(ctx, api)
    config = api.get_workflow_config(workflow_key)
    changed = False
    if (
        expiration_buffer is not None
        and expiration_buffer != config.compute_node_expiration_buffer_seconds
    ):
        config.compute_node_expiration_buffer_seconds = expiration_buffer
        changed = True
    if (
        wait_for_healthy_db is not None
        and wait_for_healthy_db != config.compute_node_wait_for_healthy_database_minutes
    ):
        config.compute_node_wait_for_healthy_database_minutes = wait_for_healthy_db
        changed = True
    if ignore_workflow_completion is not None:
        lowered = ignore_workflow_completion.lower()
        if lowered not in ("true", "false"):
            logger.error(
                "Invalid value for ignore_workflow_completion: %s", ignore_workflow_completion
            )
            sys.exit(1)
        val = lowered == "true"
        if val != config.compute_node_ignore_workflow_completion:
            config.compute_node_ignore_workflow_completion = val
            changed = True
    if (
        wait_for_new_jobs is not None
        and wait_for_new_jobs != config.compute_node_wait_for_new_jobs_seconds
    ):
        config.compute_node_wait_for_new_jobs_seconds = wait_for_new_jobs
        changed = True

    if changed:
        config = api.modify_workflow_config(workflow_key, config)
        print(json.dumps(config.to_dict(), indent=2))
    else:
        logger.warning("No parameters were changed")


@click.command()
@click.pass_obj
@click.pass_context
def show_config(ctx: click.Context, api: DefaultApi) -> None:
    """Show the workflow config."""
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    workflow_key = get_workflow_key_from_context(ctx, api)
    config = api.get_workflow_config(workflow_key)
    print(json.dumps(config.to_dict(), indent=2))


@click.command(name="status")
@click.pass_obj
@click.pass_context
def show_status(ctx: click.Context, api: DefaultApi) -> None:
    """Show the workflow status."""
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    workflow_key = get_workflow_key_from_context(ctx, api)
    status = api.get_workflow_status(workflow_key)
    print(json.dumps(status.to_dict(), indent=2))


@click.command()
@click.pass_obj
@click.pass_context
def example(ctx: click.Context, api: DefaultApi) -> None:
    """Show the example workflow."""
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    data = api.get_workflow_specification_example().to_dict()
    sanitize_workflow(data)
    print(json.dumps(data, indent=2))


@click.command()
@click.pass_obj
@click.pass_context
def template(ctx: click.Context, api: DefaultApi) -> None:
    """Show the workflow template."""
    setup_cli_logging(ctx, __name__)
    check_database_url(api)
    data = api.get_workflow_specification_template().to_dict()
    data = remove_db_keys(data)
    data["config"] = remove_db_keys(data["config"])
    data.pop("key", None)
    print(json.dumps(data, indent=2))


def _update_torc_rc(api: DefaultApi, workflow: WorkflowModel) -> None:
    config = TorcRuntimeConfig.load()
    config.workflow_key = workflow.key
    path = config.path()
    logger.info("Updating %s with workflow_key=%s", path, config.workflow_key)
    if config.database_url != api.api_client.configuration.host:
        config.database_url = api.api_client.configuration.host
        logger.info("Updating %s with database_url=%s", path, config.database_url)
    config.dump(path=path)


workflows.add_command(cancel)
workflows.add_command(cancel_all)
workflows.add_command(create)
workflows.add_command(create_from_commands_file)
workflows.add_command(create_from_json_file)
workflows.add_command(modify)
workflows.add_command(delete)
workflows.add_command(delete_all)
workflows.add_command(list_scheduler_configs)
workflows.add_command(list_workflows)
workflows.add_command(process_auto_tune_resource_requirements_results)
workflows.add_command(recommend_nodes)
workflows.add_command(reset_status)
workflows.add_command(restart)
workflows.add_command(set_compute_node_parameters)
workflows.add_command(start)
workflows.add_command(show)
workflows.add_command(show_config)
workflows.add_command(show_status)
workflows.add_command(example)
workflows.add_command(template)
