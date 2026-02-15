# Tutorial 1: Many Independent Jobs

This tutorial teaches you how to create and run a workflow with many independent parallel jobs using
Torc's parameterization feature.

## Learning Objectives

By the end of this tutorial, you will:

- Understand how to define parameterized jobs that expand into multiple instances
- Learn how Torc executes independent jobs in parallel
- Know how to monitor job execution and view results

## Prerequisites

- Torc server running (see [Installation](../installation.md))
- Basic familiarity with YAML syntax

## Use Cases

This pattern is ideal for:

- **Parameter sweeps**: Testing different configurations
- **Monte Carlo simulations**: Running many independent trials
- **Batch processing**: Processing many files with the same logic
- **Embarrassingly parallel workloads**: Any task that can be split into independent units

## Step 1: Start the Torc Server

First, ensure the Torc server is running:

```console
torc-server run
```

By default, the server listens on port 8080, making the API URL
`http://localhost:8080/torc-service/v1`.

If you use a custom port, set the environment variable:

```console
export TORC_API_URL="http://localhost:8100/torc-service/v1"
```

## Step 2: Create the Workflow Specification

Save the following as `hundred_jobs.yaml`:

```yaml
name: hundred_jobs_parallel
description: 100 independent jobs that can run in parallel

jobs:
  - name: job_{i:03d}
    command: |
      echo "Running job {i}"
      sleep $((RANDOM % 10 + 1))
      echo "Job {i} completed"
    resource_requirements: minimal
    parameters:
      i: "1:100"

resource_requirements:
  - name: minimal
    num_cpus: 1
    num_gpus: 0
    num_nodes: 1
    memory: 1g
    runtime: PT5M
```

### Understanding the Specification

Let's break down the key elements:

- **`name: job_{i:03d}`**: The `{i:03d}` is a parameter placeholder. The `:03d` format specifier
  means "3-digit zero-padded integer", so jobs will be named `job_001`, `job_002`, ..., `job_100`.

- **`parameters: i: "1:100"`**: This defines a parameter `i` that ranges from 1 to 100 (inclusive).
  Torc will create one job for each value.

- **`resource_requirements: minimal`**: Each job uses the "minimal" resource profile defined below.

When Torc processes this specification, it **expands** the single job definition into 100 separate
jobs, each with its own parameter value substituted.

## Step 3: Run the Workflow

Create and run the workflow in one command:

```bash
torc run hundred_jobs.yaml
```

This command:

1. Creates the workflow on the server
2. Expands the parameterized job into 100 individual jobs
3. Initializes the dependency graph (in this case, no dependencies)
4. Starts executing jobs in parallel

You'll see output showing the workflow ID and progress.

## Step 4: Monitor Execution

While the workflow runs, you can monitor progress:

```bash
# Check workflow status
torc workflows status <workflow_id>

# List jobs and their states
torc jobs list <workflow_id>

# Or use the interactive TUI
torc tui
```

Since all 100 jobs are independent (no dependencies between them), Torc will run as many in parallel
as your system resources allow.

## Step 5: View Results

After completion, check the results:

```bash
torc results list <workflow_id>
```

This shows return codes, execution times, and resource usage for each job.

## How It Works

When you run this workflow, Torc:

1. **Expands parameters**: The single job definition becomes 100 jobs (`job_001` through `job_100`)
2. **Marks all as ready**: Since there are no dependencies, all jobs start in the "ready" state
3. **Executes in parallel**: The job runner claims and executes jobs based on available resources
4. **Tracks completion**: Each job's return code and metrics are recorded

The job runner respects the resource requirements you specified. With `num_cpus: 1` per job, if your
machine has 8 CPUs, approximately 8 jobs will run simultaneously.

## What You Learned

In this tutorial, you learned how to:

- ✅ Use parameter expansion (`parameters: i: "1:100"`) to generate multiple jobs from one
  definition
- ✅ Use format specifiers (`{i:03d}`) for consistent naming
- ✅ Run independent parallel jobs with `torc run`
- ✅ Monitor workflow progress and view results

## Example Files

See
[hundred_jobs_parameterized.yaml](https://github.com/NatLabRockies/torc/blob/main/examples/yaml/hundred_jobs_parameterized.yaml)
for a ready-to-run version of this workflow.

## Next Steps

- [Tutorial 2: Diamond Workflow](./diamond.md) - Learn how to create job dependencies using files
- [Tutorial 4: Simple Parameterization](./simple-params.md) - Explore more parameter expansion
  options
- [Multi-Stage Workflows with Barriers](./multi-stage-barrier.md) - Scale to thousands of jobs
  efficiently
