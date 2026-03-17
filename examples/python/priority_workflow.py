"""Example showing per-job scheduling priority.

Jobs with higher priority values are claimed by workers first.
When multiple jobs are ready simultaneously and worker capacity is
limited, priority determines execution order.
"""

import getpass
import os

from torc import create_jobs, make_api, setup_logging
from torc.openapi_client import DefaultApi, JobModel, ResourceRequirementsModel, WorkflowModel

TORC_API_URL = os.getenv("TORC_API_URL", "http://localhost:8080/torc-service/v1")


def build_workflow(api: DefaultApi, workflow: WorkflowModel) -> None:
    workflow_id = workflow.id
    assert workflow_id is not None

    small = api.create_resource_requirements(
        ResourceRequirementsModel(
            workflow_id=workflow_id,
            name="small",
            num_cpus=1,
            memory="2g",
            runtime="PT30M",
        )
    )
    medium = api.create_resource_requirements(
        ResourceRequirementsModel(
            workflow_id=workflow_id,
            name="medium",
            num_cpus=4,
            memory="8g",
            runtime="PT1H",
        )
    )

    # preprocess has no dependencies and the highest priority — it runs first.
    preprocess = api.create_job(
        JobModel(
            workflow_id=workflow_id,
            name="preprocess",
            command="python preprocess.py --input raw.csv --output clean.csv",
            priority=10,
            resource_requirements_id=small.id,
        )
    )

    # These three jobs become ready at the same time once preprocess completes.
    # Workers claim them in descending priority order.
    jobs = [
        JobModel(
            workflow_id=workflow_id,
            name="critical_analysis",
            command="python analyze.py --mode critical --input clean.csv",
            priority=5,  # claimed first among the three
            depends_on_job_ids=[preprocess.id],
            resource_requirements_id=medium.id,
        ),
        JobModel(
            workflow_id=workflow_id,
            name="normal_analysis",
            command="python analyze.py --mode normal --input clean.csv",
            priority=3,  # claimed second
            depends_on_job_ids=[preprocess.id],
            resource_requirements_id=medium.id,
        ),
        JobModel(
            workflow_id=workflow_id,
            name="background_report",
            command="python report.py --input clean.csv",
            priority=1,  # claimed last
            depends_on_job_ids=[preprocess.id],
            resource_requirements_id=small.id,
        ),
    ]
    created = create_jobs(api, workflow_id, jobs)

    # Summary waits for all three; priority defaults to 0.
    api.create_job(
        JobModel(
            workflow_id=workflow_id,
            name="summary",
            command="python summarize.py",
            depends_on_job_ids=[j.id for j in created],
            resource_requirements_id=small.id,
        )
    )


def main() -> None:
    setup_logging()
    api = make_api(TORC_API_URL)
    workflow = api.create_workflow(
        WorkflowModel(
            user=getpass.getuser(),
            name="priority_demo",
            description="Demonstrates per-job scheduling priority",
        )
    )
    try:
        build_workflow(api, workflow)
    except Exception:
        api.delete_workflow(workflow.id)
        raise


if __name__ == "__main__":
    main()
