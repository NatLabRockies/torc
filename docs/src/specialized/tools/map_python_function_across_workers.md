# Map a Python function to compute nodes

This tutorial will teach you how to build a workflow from Python functions instead of CLI
executables and run on it on an HPC with `Slurm`.

## Pre-requisites

This tutorial requires installation of the python package `torc-client`. Until the latest version is
published at pypi.org, you must clone this repository install the package in a virtual environment.
Use Python 3.11 or later.

```
git clone https://github.com/NatLabRockies/torc
cd torc/python_client
python -m venv .venv
source .venv/bin/activate
pip install -e .
```

## Workflow Description

Let's suppose that your code is in a module called `simulation.py` and looks something like this:

```python
def run(job_name: str, input_params: dict) -> dict:
    """Runs one simulation on a set of input parameters.

    Returns
    -------
    job_name: str
        Name of the job.
    dict
        Result of the simulation.
    """
    return {
        "inputs": input_params,
        "result": 5,
        "output_data_path": f"/projects/my-project/{job_name}",
    }


def postprocess(results: list[dict]) -> dict:
    """Collects the results of the workers and performs post-processing.

    Parameters
    ----------
    results : list[dict]
        Results from each simulation

    Returns
    -------
    dict
        Final result
    """
    total = 0
    paths = []
    for result in results:
        assert "result" in result
        assert "output_data_path" in result
        total += result["result"]
        paths.append(result["output_data_path"])
    return {"total": total, "output_data_paths": paths}
```

You need to run this function on hundreds of sets of input parameters and want torc to help you
scale this work on an HPC.

The recommended procedure for this task is torc's Python API as shown below. The goal is to mimic
the behavior of Python's
[concurrent.futures.ProcessPoolExecutor.map](https://docs.python.org/3/library/concurrent.futures.html#processpoolexecutor)
as much as possible.

Similar functionality is also available with
[Dask](https://docs.dask.org/en/stable/deploying.html?highlight=slurm#deploy-dask-clusters).

### Resource Constraints

- Each function call needs 4 CPUs and 20 GiB of memory.
- The function call takes 1 hour to run.

A compute node with 92 GiB of memory are easiest to acquire but would only be able to run 4 jobs at
a time. The 180 GiB nodes are fewer in number but would use fewer AUs because they would be able to
run 8 jobs at a time.

## Torc Overview

Here is what torc does to solve this problem:

- User creates a workflow in Python.
- User passes a callable function as well as a list of all input parameters that need to be mapped
  to the function.
- For each set of input parameters torc creates a record in the `user_data` table in the database,
  creates a job with a relationship to that record as an input, and creates a placeholder for data
  to be created by that job.
- When torc runs each job it reads the correct input parameters from the database, imports the
  user's function, and then calls it with the input parameters.
- When the function completes, torc stores any returned data in the database.
- When all workers complete torc collects all result data from the database into a list and passes
  that to the postprocess function. It also stores any returned data from that function into the
  database.

## Build the workflow

1. Write a script to create the workflow. Note that you need to correct the `api` URL and the Slurm
   `account`.

```python
import getpass
import os

from torc import make_api, map_function_to_jobs, setup_logging
from torc.openapi_client import (
    DefaultApi,
    ResourceRequirementsModel,
    SlurmSchedulerModel,
    WorkflowModel,
)


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
    api.create_slurm_scheduler(
        SlurmSchedulerModel(
            workflow_id=workflow_id,
            name="short",
            account="my_account",
            mem="180224",
            walltime="04:00:00",
            nodes=1,
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
    print(f"Created workflow with ID {workflow_id} {len(jobs)} jobs.")


def main():
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
```

**Requirements**:

- Your run function should raise an exception if there is a failure. If that happens, torc will
  record a non-zero return code for the job.

- If you want torc to store result data in the database, return it from your run function. **Note**:
  be careful on how much result data you return. If you are using a custom database for one
  workflow, store as much as you want. If you are using a shared server, ensure that you are
  following its administrator's policies. You could consider storing large data in files and only
  storing file paths in the database.

- If you choose to define a postprocess function and want torc to store the final data in the
  database, return it from that function.

- The `params` must be serializable in JSON format because they will be stored in the database.
  Basic types like numbers and strings and lists and dictionaries of those will work fine. If you
  need to store complex, custom types, consider these options:

  - Define data models with [Pydantic](https://docs.pydantic.dev/latest/usage/models/). You can use
    their existing serialization/de-serialization methods or define custom methods.
  - Pickle your data and store the result as a string. Your run function would need to understand
    how to de-serialize it. Note that this has portability limitations. (Please contact the
    developers if you would like to see this happen automatically.)

- Torc must be able to import simulation.py from Python. Here are some options:

  - Put the script in the current directory.
  - Install it in the environment.
  - Specify its parent directory like this:
    `map_function_to_jobs(..., module_directory="my_module")`

```console
python map_function_across_workers.py
```

2. Create the workflow.

```console
python examples/python/map_function_across_workers.py
Created workflow 342 with 4 jobs.
```

3. Run the workflow.

```console
$ torc run 342
```

8. View the result data overall or by job (if your run and postprocess functions return something).
   Note that listing all user-data will return input parameters.

```console
$ torc -f json user-data list 342
```

## Other jobs

You could add "normal" jobs to the workflow as well. For example, you might have preprocessing and
post-processing work to do. You can add those jobs through the API. You could also add multiple
rounds of mapped functions. `map_function_to_jobs` provides a `depends_on_job_ids` parameter to
specify ordering. You could also define job-job relationships through files or user-data as
discussed elsewhere in this documentation.
