"""Test workflow execution"""


from torc.openapi_client.models.job_model import JobModel
from torc.api import create_jobs


def test_add_bulk_jobs(mapped_function_workflow):
    """Test the add_jobs helper function."""
    db, _ = mapped_function_workflow
    api = db.api
    initial_jobs = api.list_jobs(db.workflow.id).items
    initial_count = len(initial_jobs)
    resource_requirements = api.list_resource_requirements(db.workflow.id).items[0]

    jobs = [
        JobModel(
            workflow_id=db.workflow.id,
            name=f"added_job{i}",
            command="python my_script.py",
            resource_requirements_id=resource_requirements.id,
        )
        for i in range(1, 51)
    ]

    added_jobs = create_jobs(api, jobs, max_transfer_size=11)
    assert len(added_jobs) == 50

    final_jobs = api.list_jobs(db.workflow.id).items
    assert len(final_jobs) == initial_count + 50
    added_names = {x.name for x in final_jobs if x.name.startswith("added_job")}
    expected_names = {f"added_job{i}" for i in range(1, 51)}
    assert added_names == expected_names
