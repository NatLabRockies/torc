# Creating Workflows with the Dashboard Wizard

This tutorial walks you through creating a workflow using the interactive wizard in the Torc
dashboard. The wizard provides a guided, step-by-step interface for building workflows without
writing YAML or JSON files.

## Learning Objectives

By the end of this tutorial, you will:

- Create a multi-job workflow using the dashboard wizard
- Define job dependencies visually
- Configure Slurm schedulers for HPC execution
- Set up workflow actions to automatically schedule nodes
- Understand how the wizard generates workflow specifications

## Prerequisites

- Torc dashboard running (see [Dashboard Deployment](./dashboard-deployment.md))
- Basic understanding of Torc workflows

## Overview

The workflow wizard guides you through five steps:

1. **Basics** - Workflow name and description
2. **Jobs** - Define computational tasks
3. **Schedulers** - Configure Slurm schedulers (optional)
4. **Actions** - Set up automatic node scheduling (optional)
5. **Review** - Preview and create the workflow

## Step 1: Open the Create Workflow Modal

1. Open the Torc dashboard in your browser
2. Click the **Create Workflow** button in the top-right corner
3. Select the **Wizard** tab at the top of the modal

You'll see the wizard interface with step indicators showing your progress.

## Step 2: Configure Basics

Enter the basic workflow information:

- **Workflow Name** (required): A unique identifier for your workflow (e.g., `data-pipeline`)
- **Description** (optional): A brief description of what the workflow does

Click **Next** to proceed.

## Step 3: Add Jobs

This is where you define the computational tasks in your workflow.

### Adding Your First Job

1. Click **+ Add Job**
2. Fill in the job details:
   - **Job Name**: A unique name (e.g., `preprocess`)
   - **Command**: The shell command to execute (e.g., `python preprocess.py`)

### Setting Dependencies

The **Blocked By** field lets you specify which jobs must complete before this job can run:

1. Click the **Blocked By** dropdown
2. Select one or more jobs that must complete first
3. Hold Ctrl/Cmd to select multiple jobs

### Configuring Resources

Choose a resource preset or customize:

- **Small**: 1 CPU, 1GB memory
- **Medium**: 8 CPUs, 50GB memory
- **GPU**: 1 CPU, 10GB memory, 1 GPU
- **Custom**: Specify exact requirements

### Example: Three-Job Pipeline

Let's create a simple pipeline:

**Job 1: preprocess**

- Name: `preprocess`
- Command: `echo "Preprocessing..." && sleep 5`
- Blocked By: (none - this runs first)
- Resources: Small

**Job 2: analyze**

- Name: `analyze`
- Command: `echo "Analyzing..." && sleep 10`
- Blocked By: `preprocess`
- Resources: Medium

**Job 3: report**

- Name: `report`
- Command: `echo "Generating report..." && sleep 3`
- Blocked By: `analyze`
- Resources: Small

Click **Next** when all jobs are configured.

## Step 4: Configure Schedulers (Optional)

If you're running on an HPC system with Slurm, you can define scheduler configurations here. Skip
this step for local execution.

### Adding a Scheduler

1. Click **+ Add Scheduler**
2. Fill in the required fields:
   - **Scheduler Name**: A reference name (e.g., `compute_scheduler`)
   - **Account**: Your Slurm account name

3. Configure optional settings:
   - **Nodes**: Number of nodes to request
   - **Wall Time**: Maximum runtime (HH:MM:SS format)
   - **Partition**: Slurm partition name
   - **QoS**: Quality of service level
   - **GRES**: GPU resources (e.g., `gpu:2`)
   - **Memory**: Memory per node (e.g., `64G`)
   - **Temp Storage**: Local scratch space
   - **Extra Slurm Options**: Additional sbatch flags

### Example: Basic Compute Scheduler

- Scheduler Name: `compute`
- Account: `my_project`
- Nodes: `1`
- Wall Time: `02:00:00`
- Partition: `standard`

### Assigning Jobs to Schedulers

After defining schedulers, you can assign jobs to them:

1. Go back to the **Jobs** step (click **Back**)
2. In each job card, find the **Scheduler** dropdown
3. Select the scheduler to use for that job

Jobs without a scheduler assigned will run locally.

Click **Next** when scheduler configuration is complete.

## Step 5: Configure Actions (Optional)

Actions automatically schedule Slurm nodes when certain events occur. This is useful for dynamic
resource allocation.

### Trigger Types

- **When workflow starts**: Schedule nodes immediately when the workflow begins
- **When jobs become ready**: Schedule nodes when specific jobs are ready to run
- **When jobs complete**: Schedule nodes after specific jobs finish

### Adding an Action

1. Click **+ Add Action**
2. Select the **Trigger** type
3. Select the **Scheduler** to use
4. For job-based triggers, select which **Jobs** trigger the action
5. Set the **Number of Allocations** (how many Slurm jobs to submit)

### Example: Stage-Based Scheduling

For a workflow with setup, compute, and finalize stages:

**Action 1: Setup Stage**

- Trigger: When workflow starts
- Scheduler: `setup_scheduler`
- Allocations: 1

**Action 2: Compute Stage**

- Trigger: When jobs become ready
- Jobs: `compute_job1`, `compute_job2`, `compute_job3`
- Scheduler: `compute_scheduler`
- Allocations: 3

**Action 3: Finalize Stage**

- Trigger: When jobs become ready
- Jobs: `finalize`
- Scheduler: `finalize_scheduler`
- Allocations: 1

Click **Next** to proceed to review.

## Step 6: Review and Create

The review step shows the generated workflow specification in JSON format. This is exactly what will
be submitted to the server.

### Reviewing the Spec

Examine the generated specification to verify:

- All jobs are included with correct names and commands
- Dependencies (`depends_on`) match your intended workflow structure
- Resource requirements are correctly assigned
- Schedulers have the right configuration
- Actions trigger on the expected events

### Creating the Workflow

1. Review the **Options** below the wizard:
   - **Initialize workflow after creation**: Builds the dependency graph (recommended)
   - **Run workflow immediately**: Starts execution right away

2. Click **Create** to submit the workflow

If successful, you'll see a success notification and the workflow will appear in your workflow list.

## Example: Complete Diamond Workflow

Here's how to create a diamond-pattern workflow using the wizard:

```
 preprocess
   /    \
work1   work2
   \    /
postprocess
```

### Jobs Configuration

| Job         | Command            | Blocked By   | Resources |
| ----------- | ------------------ | ------------ | --------- |
| preprocess  | `./preprocess.sh`  | (none)       | Small     |
| work1       | `./work1.sh`       | preprocess   | Medium    |
| work2       | `./work2.sh`       | preprocess   | Medium    |
| postprocess | `./postprocess.sh` | work1, work2 | Small     |

### Generated Spec Preview

The wizard generates a spec like this:

```json
{
  "name": "diamond-workflow",
  "description": "Fan-out and fan-in example",
  "jobs": [
    {
      "name": "preprocess",
      "command": "./preprocess.sh",
      "resource_requirements": "res_1cpu_1g"
    },
    {
      "name": "work1",
      "command": "./work1.sh",
      "depends_on": ["preprocess"],
      "resource_requirements": "res_8cpu_50g"
    },
    {
      "name": "work2",
      "command": "./work2.sh",
      "depends_on": ["preprocess"],
      "resource_requirements": "res_8cpu_50g"
    },
    {
      "name": "postprocess",
      "command": "./postprocess.sh",
      "depends_on": ["work1", "work2"],
      "resource_requirements": "res_1cpu_1g"
    }
  ],
  "resource_requirements": [
    {"name": "res_1cpu_1g", "num_cpus": 1, "memory": "1g", "num_gpus": 0, "num_nodes": 1, "runtime": "PT1H"},
    {"name": "res_8cpu_50g", "num_cpus": 8, "memory": "50g", "num_gpus": 0, "num_nodes": 1, "runtime": "PT1H"}
  ]
}
```

## Using Parameterized Jobs

The wizard supports job parameterization for creating multiple similar jobs:

1. In a job card, find the **Parameters** field
2. Enter parameters in the format: `param_name: "value_spec"`

### Parameter Formats

- **Range**: `i: "1:10"` creates jobs for i=1,2,3,...,10
- **Range with step**: `i: "0:100:10"` creates jobs for i=0,10,20,...,100
- **List**: `dataset: "['train', 'test', 'validation']"`

### Example: Parameterized Processing

- Job Name: `process_{i}`
- Command: `python process.py --index {i}`
- Parameters: `i: "1:5"`

This creates 5 jobs: `process_1` through `process_5`.

## Tips and Best Practices

### Job Naming

- Use descriptive, unique names
- Avoid spaces and special characters
- For parameterized jobs, include the parameter in the name (e.g., `job_{i}`)

### Dependencies

- Keep dependency chains as short as possible
- Use the fan-out/fan-in pattern for parallelism
- Avoid circular dependencies (the server will reject them)

### Schedulers

- Create separate schedulers for different resource needs
- Use descriptive names that indicate the scheduler's purpose
- Set realistic wall times to avoid queue priority penalties

### Actions

- Use `on_workflow_start` for initial resource allocation
- Use `on_jobs_ready` for just-in-time scheduling
- Match allocations to the number of parallel jobs

## What You Learned

In this tutorial, you learned:

- How to navigate the five-step workflow wizard
- How to create jobs with commands, dependencies, and resources
- How to configure Slurm schedulers for HPC execution
- How to set up actions for automatic node scheduling
- How the wizard generates workflow specifications

## Next Steps

- [Diamond Workflow](../../core/tutorials/diamond.md) - Learn about file-based implicit dependencies
- [Simple Parameterization](../../core/tutorials/simple-params.md) - Create parameter sweeps
  programmatically
- [Advanced Slurm Configuration](../hpc/slurm.md) - Manual Slurm scheduler setup
