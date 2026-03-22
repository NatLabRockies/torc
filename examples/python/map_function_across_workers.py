import getpass
import os

from torc import make_api, map_function_to_jobs, setup_logging
from torc.api import DefaultApi
from torc.openapi_client import ResourceRequirementsModel, WorkflowModel


TORC_API_URL = os.getenv("TORC_API_URL", "http://localhost:8080/torc-service/v1")


def create_workflow(api: DefaultApi) -> WorkflowModel:
    """Create the workflow"""
    workflow = WorkflowModel(
        user=getpass.getuser(),
        name="map_function_workflow",
        description="Example workflow that maps a function across workers",
    )
    return api.create_workflow(workflow)


def build_workflow(api: DefaultApi, workflow: WorkflowModel):
    """Creates a workflow with implicit job dependencies declared through files."""
    workflow_id = workflow.id
    assert workflow_id is not None
    params = [
        {"input1": 1, "input2": 2, "input3": 3},
        {"input1": 4, "input2": 5, "input3": 6},
        {"input1": 7, "input2": 8, "input3": 9},
    ]
    assert workflow.id is not None
    rr = api.create_resource_requirements(
        ResourceRequirementsModel(
            workflow_id=workflow_id,
            name="medium",
            num_cpus=4,
            memory="20g",
            runtime="P0DT1H",
        ),
    )
    jobs = map_function_to_jobs(
        api,
        workflow_id,
        "simulation",
        "run",
        params,
        resource_requirements_id=rr.id,
        # Note that this is optional.
        postprocess_func="postprocess",
    )
    print(f"Created workflow with ID {workflow_id} with {len(jobs)} jobs.")


def main():
    """Entry point"""
    setup_logging()
    api = make_api(TORC_API_URL)
    workflow = create_workflow(api)
    try:
        build_workflow(api, workflow)
    except Exception:
        api.delete_workflow(workflow.id)
        raise


if __name__ == "__main__":
    main()
