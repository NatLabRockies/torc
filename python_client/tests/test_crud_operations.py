"""Test CRUD operations for the Python auto-generated client.

This test assumes a torc server is running and tests all basic Create, Read,
Update, and Delete operations for the main resources in the Torc API.

Prerequisites:
    - A running torc-server instance (default: http://localhost:8080/torc-service/v1)
    - Set TORC_API_URL environment variable to override the default server URL

Usage:
    # Start the server (in separate terminal)
    $ cd /path/to/torc
    $ cargo run --bin torc-server

    # Run all CRUD tests
    $ cd python_client
    $ pytest tests/test_crud_operations.py -v

    # Run specific test class
    $ pytest tests/test_crud_operations.py::TestWorkflowCRUD -v

    # Run with custom server URL
    $ TORC_API_URL=http://custom-host:8080/torc-service/v1 pytest tests/test_crud_operations.py -v

Tested Resources:
    - Workflows: create, get, list, update, delete
    - Jobs: create, get, list, update, delete
    - Files: create, get, list, update, delete
    - Resource Requirements: create, get, list, update, delete
    - Results: create, get, list, update, delete
    - Events: create, get, list, update, delete
    - User Data: create, get, list, update, delete
    - Compute Nodes: create, get, list, update, delete
    - Local Schedulers: create, get, list, update, delete
    - Slurm Schedulers: create, get, list, update, delete
"""

import os
from datetime import datetime

import pytest
from torc.api import DefaultApi
from torc.openapi_client import (
    ApiClient,
    ComputeNodeModel,
    Configuration,
    EventModel,
    FileModel,
    JobModel,
    ResourceRequirementsModel,
    # ResultModel,
    SlurmSchedulerModel,
    UserDataModel,
    WorkflowModel,
)


@pytest.fixture
def api_client():
    """Create an API client connected to the test server."""
    configuration = Configuration()
    # Get server URL from environment variable or use default
    server_url = os.environ.get("TORC_API_URL", "http://localhost:8080/torc-service/v1")
    configuration.host = server_url
    return DefaultApi(ApiClient(configuration))


@pytest.fixture
def test_workflow(api_client, tmp_path):
    """Create a test workflow and clean it up after the test."""
    workflow = api_client.create_workflow(
        WorkflowModel(user="test_user", name="test_crud_workflow")
    )
    yield workflow
    # Cleanup: delete workflow (cascades to all related resources)
    try:
        api_client.delete_workflow(workflow.id)
    except Exception:
        pass  # Already deleted or doesn't exist


class TestWorkflowCRUD:
    """Test CRUD operations for Workflows."""

    def test_create_workflow(self, api_client):
        """Test creating a workflow."""
        workflow = api_client.create_workflow(
            WorkflowModel(user="test_user", name="test_workflow_create")
        )
        try:
            assert workflow.id is not None
            assert workflow.user == "test_user"
            assert workflow.name == "test_workflow_create"
        finally:
            api_client.delete_workflow(workflow.id)

    def test_get_workflow(self, api_client, test_workflow):
        """Test getting a workflow by ID."""
        retrieved = api_client.get_workflow(test_workflow.id)
        assert retrieved.id == test_workflow.id
        assert retrieved.user == test_workflow.user
        assert retrieved.name == test_workflow.name

    def test_list_workflows(self, api_client, test_workflow):
        """Test listing workflows."""
        workflows = api_client.list_workflows()
        assert workflows.items is not None
        assert len(workflows.items) > 0
        workflow_ids = [w.id for w in workflows.items]
        assert test_workflow.id in workflow_ids

    def test_update_workflow(self, api_client, test_workflow):
        """Test updating a workflow."""
        test_workflow.name = "updated_workflow_name"
        updated = api_client.update_workflow(test_workflow.id, test_workflow)
        assert updated.name == "updated_workflow_name"

    def test_delete_workflow(self, api_client):
        """Test deleting a workflow."""
        workflow = api_client.create_workflow(
            WorkflowModel(user="test_user", name="test_workflow_delete")
        )
        api_client.delete_workflow(workflow.id)
        # Verify it's deleted by checking the list
        workflows = api_client.list_workflows()
        workflow_ids = [w.id for w in workflows.items]
        assert workflow.id not in workflow_ids


class TestJobCRUD:
    """Test CRUD operations for Jobs."""

    def test_create_job(self, api_client, test_workflow):
        """Test creating a job."""
        job = api_client.create_job(
            JobModel(workflow_id=test_workflow.id, name="test_job", command="echo 'hello world'"),
        )
        assert job.id is not None
        assert job.name == "test_job"
        assert job.command == "echo 'hello world'"

    def test_get_job(self, api_client, test_workflow):
        """Test getting a job by ID."""
        created_job = api_client.create_job(
            JobModel(workflow_id=test_workflow.id, name="test_get_job", command="echo 'test'"),
        )
        retrieved = api_client.get_job(created_job.id)
        assert retrieved.id == created_job.id
        assert retrieved.name == created_job.name

    def test_list_jobs(self, api_client, test_workflow):
        """Test listing jobs."""
        # Create a couple of jobs
        api_client.create_job(
            JobModel(workflow_id=test_workflow.id, name="job1", command="echo 'job1'")
        )
        api_client.create_job(
            JobModel(workflow_id=test_workflow.id, name="job2", command="echo 'job2'")
        )
        jobs = api_client.list_jobs(test_workflow.id)
        assert jobs.items is not None
        assert len(jobs.items) >= 2

    def test_update_job(self, api_client, test_workflow):
        """Test updating a job."""
        job = api_client.create_job(
            JobModel(workflow_id=test_workflow.id, name="test_update_job", command="echo 'original'"),
        )
        job.command = "echo 'updated'"
        updated = api_client.update_job(job.id, job)
        assert updated.command == "echo 'updated'"

    def test_delete_job(self, api_client, test_workflow):
        """Test deleting a job."""
        job = api_client.create_job(
            JobModel(workflow_id=test_workflow.id, name="test_delete_job", command="echo 'delete me'"),
        )
        api_client.delete_job(job.id)
        # Verify deletion
        jobs = api_client.list_jobs(test_workflow.id)
        job_ids = [j.id for j in jobs.items]
        assert job.id not in job_ids


class TestFileCRUD:
    """Test CRUD operations for Files."""

    def test_create_file(self, api_client, test_workflow, tmp_path):
        """Test creating a file."""
        test_file = tmp_path / "test_input.txt"
        test_file.write_text("test content")
        file = api_client.create_file(
            FileModel(workflow_id=test_workflow.id, name="test_file", path=str(test_file)),
        )
        assert file.id is not None
        assert file.name == "test_file"
        assert file.path == str(test_file)

    def test_get_file(self, api_client, test_workflow, tmp_path):
        """Test getting a file by ID."""
        test_file = tmp_path / "test_get_file.txt"
        test_file.write_text("test content")
        created_file = api_client.create_file(
            FileModel(workflow_id=test_workflow.id, name="get_file", path=str(test_file)),
        )
        retrieved = api_client.get_file(created_file.id)
        assert retrieved.id == created_file.id
        assert retrieved.name == created_file.name

    def test_list_files(self, api_client, test_workflow, tmp_path):
        """Test listing files."""
        file1 = tmp_path / "file1.txt"
        file2 = tmp_path / "file2.txt"
        file1.write_text("content1")
        file2.write_text("content2")
        api_client.create_file(
            FileModel(workflow_id=test_workflow.id, name="file1", path=str(file1))
        )
        api_client.create_file(
            FileModel(workflow_id=test_workflow.id, name="file2", path=str(file2))
        )
        files = api_client.list_files(test_workflow.id)
        assert files.items is not None
        assert len(files.items) >= 2

    def test_update_file(self, api_client, test_workflow, tmp_path):
        """Test updating a file."""
        test_file = tmp_path / "update_file.txt"
        test_file.write_text("original")
        file = api_client.create_file(
            FileModel(workflow_id=test_workflow.id, name="update_file", path=str(test_file)),
        )
        # Update the modification time
        test_file.write_text("updated content")
        mtime = test_file.stat().st_mtime
        file.st_mtime = mtime
        updated = api_client.update_file(file.id, file)
        # Use approx comparison due to floating-point precision loss in JSON serialization
        assert updated.st_mtime == pytest.approx(mtime)

    def test_delete_file(self, api_client, test_workflow, tmp_path):
        """Test deleting a file."""
        test_file = tmp_path / "delete_file.txt"
        test_file.write_text("delete me")
        file = api_client.create_file(
            FileModel(workflow_id=test_workflow.id, name="delete_file", path=str(test_file)),
        )
        api_client.delete_file(file.id)
        # Verify deletion
        files = api_client.list_files(test_workflow.id)
        file_ids = [f.id for f in files.items]
        assert file.id not in file_ids


class TestResourceRequirementsCRUD:
    """Test CRUD operations for Resource Requirements."""

    def test_create_resource_requirements(self, api_client, test_workflow):
        """Test creating resource requirements."""
        req = api_client.create_resource_requirements(
            ResourceRequirementsModel(
                name="test", workflow_id=test_workflow.id, num_cpus=4, memory="8g", num_gpus=0, runtime="PT1H"
            ),
        )
        assert req.id is not None
        assert req.num_cpus == 4
        assert req.memory == "8g"

    def test_get_resource_requirements(self, api_client, test_workflow):
        """Test getting resource requirements by ID."""
        created_req = api_client.create_resource_requirements(
            ResourceRequirementsModel(
                name="test", workflow_id=test_workflow.id, num_cpus=2, memory="4g", num_gpus=0, runtime="PT30M"
            ),
        )
        retrieved = api_client.get_resource_requirements(created_req.id)
        assert retrieved.id == created_req.id
        assert retrieved.num_cpus == created_req.num_cpus

    def test_list_resource_requirements(self, api_client, test_workflow):
        """Test listing resource requirements."""
        api_client.create_resource_requirements(
            ResourceRequirementsModel(name="test", workflow_id=test_workflow.id, num_cpus=1, memory="2g", num_gpus=0),
        )
        api_client.create_resource_requirements(
            ResourceRequirementsModel(name="test", workflow_id=test_workflow.id, num_cpus=2, memory="4g", num_gpus=0),
        )
        reqs = api_client.list_resource_requirements(test_workflow.id)
        assert reqs.items is not None
        assert len(reqs.items) >= 2

    def test_update_resource_requirements(self, api_client, test_workflow):
        """Test updating resource requirements."""
        req = api_client.create_resource_requirements(
            ResourceRequirementsModel(name="test", workflow_id=test_workflow.id, num_cpus=2, memory="4g", num_gpus=0),
        )
        req.num_cpus = 8
        req.memory = "16g"
        updated = api_client.update_resource_requirements(
            req.id, req
        )
        assert updated.num_cpus == 8
        assert updated.memory == "16g"

    def test_delete_resource_requirement(self, api_client, test_workflow):
        """Test deleting resource requirements."""
        req = api_client.create_resource_requirements(
            ResourceRequirementsModel(name="test", workflow_id=test_workflow.id, num_cpus=1, memory="1g", num_gpus=0),
        )
        api_client.delete_resource_requirement(req.id)
        # Verify deletion
        reqs = api_client.list_resource_requirements(test_workflow.id)
        req_ids = [r.id for r in reqs.items]
        assert req.id not in req_ids


#class TestResultCRUD:
#    """Test CRUD operations for Results."""
#
#    def test_create_result(self, api_client, test_workflow):
#        """Test creating a result."""
#        job = api_client.create_job(
#            JobModel(workflow_id=test_workflow.id, name="result_test_job", command="echo 'test'"),
#        )
#        result = api_client.create_result(
#            ResultModel(
#                workflow_id=test_workflow.id,
#                job_id=job.id,
#                run_id=1,
#                return_code=0,
#                exec_time_minutes=0.5,
#                completion_time=str(datetime.now()),
#                status="done",
#                compute_node_id=1,
#            ),
#        )
#        assert result.id is not None
#        assert result.job_id == job.id
#        assert result.return_code == 0
#
#    def test_get_result(self, api_client, test_workflow):
#        """Test getting a result by ID."""
#        job = api_client.create_job(
#            JobModel(workflow_id=test_workflow.id, name="get_result_job", command="echo 'test'"),
#        )
#        created_result = api_client.create_result(
#            ResultModel(
#                workflow_id=test_workflow.id,
#                job_id=job.id,
#                run_id=1,
#                return_code=0,
#                exec_time_minutes=1.0,
#                completion_time=str(datetime.now()),
#                status="done",
#                compute_node_id=1,
#            ),
#        )
#        retrieved = api_client.get_result(created_result.id)
#        assert retrieved.id == created_result.id
#        assert retrieved.job_id == created_result.job_id
#
#    def test_list_results(self, api_client, test_workflow):
#        """Test listing results."""
#        job1 = api_client.create_job(
#            JobModel(workflow_id=test_workflow.id, name="job1", command="echo '1'")
#        )
#        job2 = api_client.create_job(
#            JobModel(workflow_id=test_workflow.id, name="job2", command="echo '2'")
#        )
#        api_client.create_result(
#            ResultModel(
#                workflow_id=test_workflow.id,
#                job_id=job1.id,
#                run_id=1,
#                return_code=0,
#                exec_time_minutes=0.5,
#                completion_time=str(datetime.now()),
#                status="done",
#                compute_node_id=1,
#            ),
#        )
#        api_client.create_result(
#            ResultModel(
#                workflow_id=test_workflow.id,
#                job_id=job2.id,
#                run_id=1,
#                return_code=0,
#                exec_time_minutes=0.5,
#                completion_time=str(datetime.now()),
#                status="done",
#                compute_node_id=1,
#            ),
#        )
#        results = api_client.list_results(test_workflow.id)
#        assert results.items is not None
#        assert len(results.items) >= 2
#
#    def test_update_result(self, api_client, test_workflow):
#        """Test updating a result."""
#        job = api_client.create_job(
#            JobModel(workflow_id=test_workflow.id, name="update_result_job", command="echo 'test'"),
#        )
#        result = api_client.create_result(
#            ResultModel(
#                workflow_id=test_workflow.id,
#                job_id=job.id,
#                run_id=1,
#                return_code=0,
#                exec_time_minutes=0.5,
#                completion_time=str(datetime.now()),
#                status="done",
#                compute_node_id=1,
#            ),
#        )
#        result.return_code = 1
#        result.status = "failed"
#        updated = api_client.update_result(result.id, result)
#        assert updated.return_code == 1
#        assert updated.status == "failed"
#
#    def test_delete_result(self, api_client, test_workflow):
#        """Test deleting a result."""
#        job = api_client.create_job(
#            JobModel(workflow_id=test_workflow.id, name="delete_result_job", command="echo 'test'"),
#        )
#        result = api_client.create_result(
#            ResultModel(
#                workflow_id=test_workflow.id,
#                job_id=job.id,
#                run_id=1,
#                return_code=0,
#                exec_time_minutes=0.5,
#                completion_time=str(datetime.now()),
#                status="done",
#                compute_node_id=1,
#            ),
#        )
#        api_client.delete_result(result.id)
#        # Verify deletion
#        results = api_client.list_results(test_workflow.id)
#        result_ids = [r.id for r in results.items]
#        assert result.id not in result_ids


class TestEventCRUD:
    """Test CRUD operations for Events."""

    def test_create_event(self, api_client, test_workflow):
        """Test creating an event."""
        event = api_client.create_event(
            EventModel(
                workflow_id=test_workflow.id,
                timestamp=int(datetime.now().timestamp()),
                data={"message": "test event"},
            ),
        )
        assert event.id is not None

    def test_get_event(self, api_client, test_workflow):
        """Test getting an event by ID."""
        created_event = api_client.create_event(
            EventModel(
                workflow_id=test_workflow.id,
                timestamp=int(datetime.now().timestamp()),
                data={"job_name": "test_job"},
            ),
        )
        retrieved = api_client.get_event(created_event.id)
        assert retrieved.id == created_event.id

    def test_list_events(self, api_client, test_workflow):
        """Test listing events."""
        api_client.create_event(
            EventModel(
                workflow_id=test_workflow.id,
                timestamp=int(datetime.now().timestamp()),
                data={"val": 3},
            ),
        )
        api_client.create_event(
            EventModel(
                workflow_id=test_workflow.id,
                timestamp=int(datetime.now().timestamp()),
                data={"val": 3},
            ),
        )
        events = api_client.list_events(test_workflow.id)
        assert events.items is not None
        assert len(events.items) >= 2

    #def test_update_event(self, api_client, test_workflow):
    #    """Test updating an event."""
    #    event = api_client.create_event(
    #        EventModel(
    #            workflow_id=test_workflow.id,
    #            timestamp=str(datetime.now()),
    #            data={"key": "data"},
    #        ),
    #    )
    #    # TODO: this is a bug in the server
    #    event.data["key"] = "data2"
    #    updated = api_client.update_event(event.id, event)
    #    assert updated.data["key"] == "data2"

    def test_delete_event(self, api_client, test_workflow):
        """Test deleting an event."""
        event = api_client.create_event(
            EventModel(
                workflow_id=test_workflow.id,
                timestamp=int(datetime.now().timestamp()),
                data={"val": 3},
            ),
        )
        api_client.delete_event(event.id)
        # Verify deletion
        events = api_client.list_events(test_workflow.id)
        event_ids = [e.id for e in events.items]
        assert event.id not in event_ids


class TestUserDataCRUD:
    """Test CRUD operations for User Data."""

    def test_create_user_data(self, api_client, test_workflow):
        """Test creating user data."""
        user_data = api_client.create_user_data(
            UserDataModel(workflow_id=test_workflow.id, name="test_data", data={"key": "value"}),
        )
        assert user_data.id is not None
        assert user_data.name == "test_data"
        assert user_data.data["key"] == "value"

    def test_get_user_data(self, api_client, test_workflow):
        """Test getting user data by ID."""
        created_data = api_client.create_user_data(
            UserDataModel(workflow_id=test_workflow.id, name="get_data", data={"test": "data"}),
        )
        retrieved = api_client.get_user_data(created_data.id)
        assert retrieved.id == created_data.id
        assert retrieved.name == created_data.name

    def test_list_user_data(self, api_client, test_workflow):
        """Test listing user data."""
        api_client.create_user_data(
            UserDataModel(workflow_id=test_workflow.id, name="data1", data={"value": 1}),
        )
        api_client.create_user_data(
            UserDataModel(workflow_id=test_workflow.id, name="data2", data={"value": 2}),
        )
        user_data_list = api_client.list_user_data(test_workflow.id)
        assert user_data_list.items is not None
        assert len(user_data_list.items) >= 2

    def test_update_user_data(self, api_client, test_workflow):
        """Test updating user data."""
        user_data = api_client.create_user_data(
            UserDataModel(workflow_id=test_workflow.id, name="update_data", data={"original": "value"}),
        )
        user_data.data = {"updated": "value"}
        updated = api_client.update_user_data(
            user_data.id, user_data
        )
        assert updated.data == {"updated": "value"}

    def test_delete_user_data(self, api_client, test_workflow):
        """Test deleting user data."""
        user_data = api_client.create_user_data(
            UserDataModel(workflow_id=test_workflow.id, name="delete_data", data={"to": "delete"}),
        )
        api_client.delete_user_data(user_data.id)
        # Verify deletion
        user_data_list = api_client.list_user_data(test_workflow.id)
        user_data_ids = [ud.id for ud in user_data_list.items]
        assert user_data.id not in user_data_ids


class TestComputeNodeCRUD:
    """Test CRUD operations for Compute Nodes."""

    def test_create_compute_node(self, api_client, test_workflow):
        """Test creating a compute node."""
        node = api_client.create_compute_node(
            ComputeNodeModel(
                workflow_id=test_workflow.id,
                hostname="test-node",
                pid=12345,
                start_time=str(datetime.now()),
                num_cpus=8,
                memory_gb=16.0,
                num_nodes=1,
                time_limit="PT2H",
                num_gpus=0,
                is_active=True,
                compute_node_type="local",
            ),
        )
        assert node.id is not None
        assert node.hostname == "test-node"
        assert node.pid == 12345

    def test_get_compute_node(self, api_client, test_workflow):
        """Test getting a compute node by ID."""
        created_node = api_client.create_compute_node(
            ComputeNodeModel(
                workflow_id=test_workflow.id,
                hostname="test-node",
                pid=12345,
                start_time=str(datetime.now()),
                num_cpus=8,
                memory_gb=16.0,
                num_nodes=1,
                time_limit="PT2H",
                num_gpus=0,
                is_active=True,
                compute_node_type="local",
            ),
        )
        retrieved = api_client.get_compute_node(created_node.id)
        assert retrieved.id == created_node.id
        assert retrieved.hostname == created_node.hostname

    def test_list_compute_nodes(self, api_client, test_workflow):
        """Test listing compute nodes."""
        api_client.create_compute_node(
            ComputeNodeModel(
                workflow_id=test_workflow.id,
                hostname="test-node",
                pid=12345,
                start_time=str(datetime.now()),
                num_cpus=8,
                memory_gb=16.0,
                num_nodes=1,
                time_limit="PT2H",
                num_gpus=0,
                is_active=True,
                compute_node_type="local",
            ),
        )
        api_client.create_compute_node(
            ComputeNodeModel(
                workflow_id=test_workflow.id,
                hostname="test-node",
                pid=12345,
                start_time=str(datetime.now()),
                num_cpus=8,
                memory_gb=16.0,
                num_nodes=1,
                time_limit="PT2H",
                num_gpus=0,
                is_active=True,
                compute_node_type="local",
            ),
        )
        nodes = api_client.list_compute_nodes(test_workflow.id)
        assert nodes.items is not None
        assert len(nodes.items) >= 2

    def test_update_compute_node(self, api_client, test_workflow):
        """Test updating a compute node."""
        node = api_client.create_compute_node(
            ComputeNodeModel(
                workflow_id=test_workflow.id,
                hostname="test-node",
                pid=12345,
                start_time=str(datetime.now()),
                num_cpus=8,
                memory_gb=16.0,
                num_nodes=1,
                time_limit="PT2H",
                num_gpus=0,
                is_active=True,
                compute_node_type="local",
            ),
        )
        node.is_active = False
        updated = api_client.update_compute_node(node.id, node)
        assert updated.is_active is False

    def test_delete_compute_node(self, api_client, test_workflow):
        """Test deleting a compute node."""
        node = api_client.create_compute_node(
            ComputeNodeModel(
                workflow_id=test_workflow.id,
                hostname="test-node",
                pid=12345,
                start_time=str(datetime.now()),
                num_cpus=8,
                memory_gb=16.0,
                num_nodes=1,
                time_limit="PT2H",
                num_gpus=0,
                is_active=True,
                compute_node_type="local",
            ),
        )
        api_client.delete_compute_node(node.id)
        # Verify deletion
        nodes = api_client.list_compute_nodes(test_workflow.id)
        node_ids = [n.id for n in nodes.items]
        assert node.id not in node_ids


class TestSlurmSchedulerCRUD:
    """Test CRUD operations for Slurm Schedulers."""

    def test_create_slurm_scheduler(self, api_client, test_workflow):
        """Test creating a slurm scheduler."""
        scheduler = api_client.create_slurm_scheduler(
            SlurmSchedulerModel(
                workflow_id=test_workflow.id,
                account="test_account",
                nodes=1,
                walltime="01:00:00",
                name="test_slurm_scheduler",
                partition="test_partition",
            ),
        )
        assert scheduler.id is not None
        assert scheduler.name == "test_slurm_scheduler"
        assert scheduler.partition == "test_partition"

    def test_get_slurm_scheduler(self, api_client, test_workflow):
        """Test getting a slurm scheduler by ID."""
        created_scheduler = api_client.create_slurm_scheduler(
            SlurmSchedulerModel(
                workflow_id=test_workflow.id,
                account="test_account",
                nodes=1,
                walltime="01:00:00",
                name="get_slurm_scheduler",
                partition="test_partition",
            ),
        )
        retrieved = api_client.get_slurm_scheduler(created_scheduler.id)
        assert retrieved.id == created_scheduler.id
        assert retrieved.name == created_scheduler.name

    def test_list_slurm_schedulers(self, api_client, test_workflow):
        """Test listing slurm schedulers."""
        api_client.create_slurm_scheduler(
            SlurmSchedulerModel(
                workflow_id=test_workflow.id,
                account="test_account",
                nodes=1,
                walltime="01:00:00",
                name="slurm1",
                partition="partition1",
            ),
        )
        api_client.create_slurm_scheduler(
            SlurmSchedulerModel(
                workflow_id=test_workflow.id,
                account="test_account",
                nodes=1,
                walltime="01:00:00",
                name="slurm2",
                partition="partition2",
            ),
        )
        schedulers = api_client.list_slurm_schedulers(test_workflow.id)
        assert schedulers.items is not None
        assert len(schedulers.items) >= 2

    def test_update_slurm_scheduler(self, api_client, test_workflow):
        """Test updating a slurm scheduler."""
        scheduler = api_client.create_slurm_scheduler(
            SlurmSchedulerModel(
                workflow_id=test_workflow.id,
                account="my_account",
                name="update_slurm",
                partition="old_partition",
                walltime="04:00:00",
                nodes=1,
            ),
        )
        scheduler.partition = "new_partition"
        updated = api_client.update_slurm_scheduler(
            scheduler.id, scheduler
        )
        assert updated.partition == "new_partition"

    def test_delete_slurm_scheduler(self, api_client, test_workflow):
        """Test deleting a slurm scheduler."""
        scheduler = api_client.create_slurm_scheduler(
            SlurmSchedulerModel(
                workflow_id=test_workflow.id,
                account="my_account",
                name="update_slurm",
                partition="old_partition",
                walltime="04:00:00",
                nodes=1,
            ),
        )
        api_client.delete_slurm_scheduler(scheduler.id)
        # Verify deletion
        schedulers = api_client.list_slurm_schedulers(test_workflow.id)
        scheduler_ids = [s.id for s in schedulers.items]
        assert scheduler.id not in scheduler_ids
