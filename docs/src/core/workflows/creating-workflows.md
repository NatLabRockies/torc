# How to Create Workflows

This guide shows different methods for creating Torc workflows, from the most common (specification
files) to more advanced approaches (CLI, API).

## Using Workflow Specification Files (Recommended)

The easiest way to create workflows is with specification files. Torc supports YAML, JSON5, and KDL
formats.

### Create from a YAML File

```bash
torc create workflow.yaml
```

### Create from JSON5 or KDL

```bash
torc create workflow.json5
torc create workflow.kdl
```

Torc detects the format from the file extension.

### Create and Run in One Step

For quick iteration, combine creation and execution:

```bash
# Create and run locally
torc run workflow.yaml

# Create and submit to Slurm
torc submit workflow.yaml
```

For format syntax and examples, see the [Workflow Specification Formats](./workflow-formats.md)
guide. For a complete reference of all fields, see the
[Workflow Specification Reference](../reference/workflow-spec.md).

## Using the CLI (Step by Step)

For programmatic workflow construction or when you need fine-grained control, create workflows piece
by piece using the CLI.

### Step 1: Create an Empty Workflow

```bash
torc workflows new \
  --name "my_workflow" \
  --description "My test workflow"
```

Output:

```
Successfully created workflow:
  ID: 1
  Name: my_workflow
  User: dthom
  Description: My test workflow
```

Note the workflow ID (1) for subsequent commands.

### Step 2: Add Resource Requirements

```bash
torc resource-requirements create \
  --name "small" \
  --num-cpus 1 \
  --memory "1g" \
  --runtime "PT10M" \
  1  # workflow ID
```

Output:

```
Successfully created resource requirements:
  ID: 2
  Workflow ID: 1
  Name: small
```

### Step 3: Add Files (Optional)

```bash
torc files create \
  --name "input_file" \
  --path "/data/input.txt" \
  1  # workflow ID
```

### Step 4: Add Jobs

```bash
torc jobs create \
  --name "process_data" \
  --command "python process.py" \
  --resource-requirements-id 2 \
  --input-file-ids 1 \
  1  # workflow ID
```

### Step 5: Initialize and Run

```bash
# Initialize the workflow (resolves dependencies)
torc workflows init 1

# Run the workflow
torc run 1
```

## Using the Python API

For complex programmatic workflow construction, use the Python client:

```python
from torc import make_api
from torc.openapi_client import (
    WorkflowModel,
    JobModel,
    ResourceRequirementsModel,
)

# Connect to the server
api = make_api("http://localhost:8080/torc-service/v1")

# Create workflow
workflow = api.create_workflow(WorkflowModel(
    name="my_workflow",
    user="myuser",
    description="Programmatically created workflow",
))

# Add resource requirements
rr = api.create_resource_requirements(ResourceRequirementsModel(
    workflow_id=workflow.id,
    name="small",
    num_cpus=1,
    memory="1g",
    runtime="PT10M",
))

# Add jobs
api.create_job(JobModel(
    workflow_id=workflow.id,
    name="job1",
    command="echo 'Hello World'",
    resource_requirements_id=rr.id,
))

print(f"Created workflow {workflow.id}")
```

For more details, see the
[Map Python Functions](../../specialized/tools/map_python_function_across_workers.md) tutorial.

## Using the Julia API

The Julia client provides similar functionality for programmatic workflow construction:

```julia
using Torc
import Torc: APIClient

# Connect to the server
api = make_api("http://localhost:8080/torc-service/v1")

# Create workflow
workflow = send_api_command(
    api,
    APIClient.create_workflow,
    APIClient.WorkflowModel(;
        name = "my_workflow",
        user = get_user(),
        description = "Programmatically created workflow",
    ),
)

# Add resource requirements
rr = send_api_command(
    api,
    APIClient.create_resource_requirements,
    APIClient.ResourceRequirementsModel(;
        workflow_id = workflow.id,
        name = "small",
        num_cpus = 1,
        memory = "1g",
        runtime = "PT10M",
    ),
)

# Add jobs
send_api_command(
    api,
    APIClient.create_job,
    APIClient.JobModel(;
        workflow_id = workflow.id,
        name = "job1",
        command = "echo 'Hello World'",
        resource_requirements_id = rr.id,
    ),
)

println("Created workflow $(workflow.id)")
```

The Julia client also supports `map_function_to_jobs` for mapping a function across parameters,
similar to the Python client.

## Choosing a Method

| Method                  | Best For                                                     |
| ----------------------- | ------------------------------------------------------------ |
| **Specification files** | Most workflows; declarative, version-controllable            |
| **CLI step-by-step**    | Scripted workflows, testing individual components            |
| **Python API**          | Complex dynamic workflows, integration with Python pipelines |
| **Julia API**           | Complex dynamic workflows, integration with Julia pipelines  |

## Common Tasks

### Validate a Workflow File Without Creating

Use `--dry-run` to validate a workflow specification without creating it on the server:

```bash
torc create --dry-run workflow.yaml
```

Example output:

```
Workflow Validation Results
===========================

Workflow: my_workflow
Description: A sample workflow

Components to be created:
  Jobs: 100 (expanded from 1 parameterized job specs)
  Files: 5
  User data records: 2
  Resource requirements: 2
  Slurm schedulers: 2
  Workflow actions: 3

Submission: Ready for scheduler submission (has on_workflow_start schedule_nodes action)

Validation: PASSED
```

For programmatic use (e.g., in scripts or the dashboard), get JSON output:

```bash
torc -f json workflows create --dry-run workflow.yaml
```

#### What Validation Checks

The dry-run performs comprehensive validation:

**Structural Checks:**

- Valid file format (YAML, JSON5, KDL, or JSON)
- Required fields present
- Parameter expansion (shows expanded job count vs. original spec count)

**Reference Validation:**

- `depends_on` references existing jobs
- `depends_on_regexes` patterns are valid and match at least one job
- `resource_requirements` references exist
- `scheduler` references exist
- `input_files` and `output_files` reference defined files
- `input_user_data` and `output_user_data` reference defined user data
- All regex patterns (`*_regexes` fields) are valid

**Duplicate Detection:**

- Duplicate job names
- Duplicate file names
- Duplicate user data names
- Duplicate resource requirement names
- Duplicate scheduler names

**Dependency Analysis:**

- Circular dependency detection (reports all jobs in the cycle)

**Action Validation:**

- Actions reference existing jobs and schedulers
- `schedule_nodes` actions have required `scheduler` and `scheduler_type`

**Scheduler Configuration:**

- Slurm scheduler node requirements are valid
- Warns about heterogeneous schedulers when jobs lack explicit scheduler assignments (see below)

#### Heterogeneous Scheduler Warning

When you have multiple Slurm schedulers with different resource profiles (memory, GPUs, walltime,
partition) and jobs without explicit scheduler assignments, the validation warns about potential
suboptimal job-to-node matching:

```
Warnings (1):
  - Workflow has 3 schedulers with different memory (mem), walltime but 10 job(s)
    have no explicit scheduler assignment. These jobs can be claimed by any
    compatible runner, which may lead to suboptimal placement on heterogeneous
    schedulers.
```

This warning helps you avoid situations where:

- Long-walltime nodes pull short-runtime jobs
- High-memory nodes pull low-memory jobs
- GPU nodes pull non-GPU jobs

**Solutions:**

1. Assign jobs to specific schedulers using the `scheduler` field on each job
2. Use job `priority` to prefer more important work when multiple jobs are ready
3. Accept flexible placement if any compatible runner is an acceptable target

#### Bypassing Validation

To create a workflow despite validation warnings:

```bash
torc create --skip-checks workflow.yaml
```

Note: This bypasses scheduler node validation checks (which are treated as errors), but does not
bypass all errors. Errors such as missing references or circular dependencies will always prevent
creation.

### List Available Workflows

```bash
torc workflows list
```

### Delete a Workflow

```bash
torc delete <workflow_id>
```

### View Workflow Details

```bash
torc workflows get <workflow_id>
```

## Defining File Dependencies

Jobs often need to read input files and produce output files. Torc can automatically infer job
dependencies from these file relationships using **variable substitution**:

```yaml
files:
  - name: raw_data
    path: /data/raw.csv
  - name: processed_data
    path: /data/processed.csv

jobs:
  - name: preprocess
    command: "python preprocess.py -o ${files.output.raw_data}"

  - name: analyze
    command: "python analyze.py -i ${files.input.raw_data} -o ${files.output.processed_data}"
```

Key concepts:

- **`${files.input.NAME}`** - References a file this job reads (creates a dependency on the job that
  outputs it)
- **`${files.output.NAME}`** - References a file this job writes (satisfies dependencies for
  downstream jobs)

In the example above, `analyze` automatically depends on `preprocess` because it needs `raw_data` as
input, which `preprocess` produces as output.

For a complete walkthrough, see [Tutorial: Diamond Workflow](../tutorials/diamond.md).

## Next Steps

- [Tutorial: Diamond Workflow](../tutorials/diamond.md) - Learn file-based dependencies with the
  fan-out/fan-in pattern
- [Workflow Specification Formats](./workflow-formats.md) - Detailed format reference
- [Workflow Specification Reference](../reference/workflow-spec.md) - Complete field reference for
  all data models
- [Job Parameterization](../reference/parameterization.md) - Generate multiple jobs from templates
- [Tutorial: Many Independent Jobs](../tutorials/many-jobs.md) - Your first workflow
