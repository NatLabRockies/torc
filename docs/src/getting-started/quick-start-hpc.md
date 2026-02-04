# Quick Start (HPC)

This guide walks you through running your first Torc workflow on an HPC cluster with Slurm. Jobs are
submitted to Slurm and run on compute nodes.

For local execution (testing, development, or non-HPC environments), see
[Quick Start (Local)](../getting-started/quick-start-local.md).

## Prerequisites

- Access to an HPC cluster with Slurm
- A Slurm account/allocation for submitting jobs
- Torc installed (see [Installation](./installation.md))

## Start the Server

On the login node, start a Torc server with a local database:

**Note**: This uses a specific hostname routable from compute nodes, which may vary across HPC
systems. Adjust as necessary or exclude `--host` to use the default.

```console
torc-server run --database torc.db --host kl1.hsn.cm.kestrel.hpc.nrel.gov --completion-check-interval-secs 5
```

If you get a port conflict, specify a different port with `--port X` or `--port 0` to get a random
port.

> **Note:** For larger deployments, your team may provide a shared Torc server. In that case, skip
> this step and set `TORC_API_URL` to the shared server address.

## Check Your HPC Profile

Torc includes built-in profiles for common HPC systems. Check if your system is detected:

```console
torc hpc detect
```

If detected, you'll see your HPC system name. To see available partitions:

```console
torc hpc partitions <profile-name>
```

> **Note:** If your HPC system isn't detected, see [Custom HPC Profile](./custom-hpc-profile.md) or
> [request built-in support](https://github.com/NREL/torc/issues).

## Create a Workflow with Resource Requirements

Save this as `workflow.yaml`:

```yaml
name: hpc_hello_world
description: Simple HPC workflow

resource_requirements:
  - name: small
    num_cpus: 4
    memory: 8g
    runtime: PT30M

jobs:
  - name: job1
    command: echo "Hello from compute node!" && hostname
    resource_requirements: small

  - name: job2
    command: echo "Hello again!" && hostname
    resource_requirements: small
    depends_on: [job1]
```

Key differences from local workflows:

- **resource_requirements**: Define CPU, memory, and runtime needs
- Jobs reference these requirements by name
- Torc matches requirements to appropriate Slurm partitions

## Submit the Workflow

Submit with your Slurm account:

```console
torc submit-slurm --account <your-account> workflow.yaml
```

Torc will:

1. Detect your HPC system
2. Match job requirements to appropriate partitions
3. Generate Slurm scheduler configurations
4. Create and submit the workflow

## Monitor Progress

Check workflow status:

```console
torc workflows list
torc jobs list <workflow-id>
```

Or use the interactive TUI:

```console
torc tui
```

Check Slurm queue:

```console
squeue --me
```

## View Results

Once jobs complete:

```console
torc results list <workflow-id>
```

Job output is stored in the `output/` directory by default.

## Example: Multi-Stage Pipeline

A more realistic workflow with different resource requirements per stage:

```yaml
name: analysis_pipeline
description: Data processing pipeline

resource_requirements:
  - name: light
    num_cpus: 4
    memory: 8g
    runtime: PT30M

  - name: compute
    num_cpus: 32
    memory: 64g
    runtime: PT2H

  - name: gpu
    num_cpus: 8
    num_gpus: 1
    memory: 32g
    runtime: PT1H

jobs:
  - name: preprocess
    command: python preprocess.py
    resource_requirements: light

  - name: train
    command: python train.py
    resource_requirements: gpu
    depends_on: [preprocess]

  - name: evaluate
    command: python evaluate.py
    resource_requirements: compute
    depends_on: [train]
```

Torc stages resource allocation based on dependencies:

- `preprocess` resources are allocated at workflow start
- `train` resources are allocated when `preprocess` completes
- `evaluate` resources are allocated when `train` completes

This prevents wasting allocation time on resources that aren't needed yet.

## Preview Before Submitting

For production workflows, preview the generated Slurm configuration first:

```console
torc slurm generate --account <your-account> workflow.yaml
```

This shows what schedulers and actions Torc will create without submitting anything.

## Next Steps

- [Slurm Workflows](./slurm-workflows.md) — How Torc manages Slurm
- [Resource Requirements](../../core/reference/resources.md) — All resource options
- [HPC Profiles](./hpc-profiles.md) — Managing HPC configurations
- [Working with Slurm](./slurm.md) — Advanced Slurm configuration
- [Debugging Slurm Workflows](./debugging-slurm.md) — Troubleshooting
