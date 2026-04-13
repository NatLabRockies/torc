"""Example diamond workflow"""

import getpass
import json
import os
from pathlib import Path

from loguru import logger

from torc import create_jobs, make_api, setup_logging
from torc.api import DefaultApi
from torc.openapi_client import (
    FileModel,
    JobModel,
    ResourceRequirementsModel,
    SlurmSchedulerModel,
    WorkflowModel,
)


TORC_API_URL = os.getenv("TORC_API_URL", "http://localhost:8080/torc-service/v1")
TEST_WORKFLOW = "test_workflow"
PREPROCESS = Path("tests") / "scripts" / "preprocess.py"
POSTPROCESS = Path("tests") / "scripts" / "postprocess.py"
WORK = Path("tests") / "scripts" / "work.py"


def create_workflow(api: DefaultApi) -> WorkflowModel:
    """Create the workflow"""
    workflow = WorkflowModel(
        user=getpass.getuser(),
        name="diamond_workflow",
        description="Example diamond workflow",
    )
    return api.create_workflow(workflow)


def build_workflow(api: DefaultApi, workflow: WorkflowModel):
    """Creates a workflow with implicit job dependencies declared through files."""
    workflow_id = workflow.id
    assert workflow_id is not None
    inputs_file = Path("inputs.json")
    inputs_file.write_text(json.dumps({"val": 5}), encoding="utf-8")

    inputs = api.create_file(
        FileModel(workflow_id=workflow_id, name="inputs", path=str(inputs_file))
    )
    f1 = api.create_file(
        FileModel(workflow_id=workflow_id, name="file1", path="f1.json")
    )
    f2 = api.create_file(
        FileModel(workflow_id=workflow_id, name="file2", path="f2.json")
    )
    f3 = api.create_file(
        FileModel(workflow_id=workflow_id, name="file3", path="f3.json")
    )
    f4 = api.create_file(
        FileModel(workflow_id=workflow_id, name="file4", path="f4.json")
    )
    f5 = api.create_file(
        FileModel(workflow_id=workflow_id, name="file5", path="f5.json")
    )
    preprocess = api.create_file(
        FileModel(workflow_id=workflow_id, name="preprocess", path=str(PREPROCESS))
    )
    work = api.create_file(
        FileModel(workflow_id=workflow_id, name="work", path=str(WORK))
    )
    postprocess = api.create_file(
        FileModel(workflow_id=workflow_id, name="postprocess", path=str(POSTPROCESS))
    )

    small = api.create_resource_requirements(
        ResourceRequirementsModel(
            workflow_id=workflow_id,
            name="small",
            num_cpus=1,
            memory="1g",
            runtime="P0DT1H",
        )
    )
    medium = api.create_resource_requirements(
        ResourceRequirementsModel(
            workflow_id=workflow_id,
            name="medium",
            num_cpus=4,
            memory="8g",
            runtime="P0DT8H",
        )
    )
    large = api.create_resource_requirements(
        ResourceRequirementsModel(
            workflow_id=workflow_id,
            name="large",
            num_cpus=8,
            memory="16g",
            runtime="P0DT12H",
        )
    )
    api.create_slurm_scheduler(
        SlurmSchedulerModel(
            workflow_id=workflow_id,
            name="short",
            account="my_account",
            nodes=1,
            walltime="04:00:00",
        ),
    )

    jobs = [
        JobModel(
            workflow_id=workflow_id,
            name="preprocess",
            command=f"python {preprocess.path} -i {inputs.path} -o {f1.path} -o {f2.path}",
            input_file_ids=[preprocess.id, inputs.id],
            output_file_ids=[f1.id, f2.id],
            resource_requirements_id=small.id,
        ),
        JobModel(
            workflow_id=workflow_id,
            name="work1",
            command=f"python {work.path} -i {f1.path} -o {f3.path}",
            input_file_ids=[work.id, f1.id],
            output_file_ids=[f3.id],
            resource_requirements_id=medium.id,
        ),
        JobModel(
            workflow_id=workflow_id,
            name="work2",
            command=f"python {work.path} -i {f2.path} -o {f4.path}",
            input_file_ids=[work.id, f2.id],
            output_file_ids=[f4.id],
            resource_requirements_id=large.id,
        ),
        JobModel(
            workflow_id=workflow_id,
            name="postprocess",
            command=f"python {postprocess.path} -i {f3.path} -i {f4.path} -o {f5.path}",
            input_file_ids=[postprocess.id, f3.id, f4.id],
            output_file_ids=[f5.id],
            resource_requirements_id=small.id,
        ),
    ]
    create_jobs(api, workflow.id, jobs)

    logger.info("Created workflow {} with {} jobs", workflow.id, len(jobs))


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
