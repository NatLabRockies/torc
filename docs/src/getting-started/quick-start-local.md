# Quick Start (Local)

This guide walks you through creating and running your first Torc workflow with local execution.
Jobs run directly on the current machine, making this ideal for testing, development, or non-HPC
environments.

For running workflows on HPC clusters with Slurm, see [Quick Start (HPC)](./quick-start-hpc.md).

## Start the Server

Start a Torc server with a local database. Setting `--completion-check-interval-secs` ensures job
completions are processed quickly (use this for personal servers, not shared deployments).

```console
torc-server run --database torc.db --host localhost --completion-check-interval-secs 5
```

## Test the Connection

In a new terminal, verify the client can connect:

```console
torc workflows list
```

## Create a Workflow

Save this as `workflow.yaml`:

```yaml
name: hello_world
description: Simple hello world workflow

jobs:
  - name: job 1
    command: echo "Hello from torc!"
  - name: job 2
    command: echo "Hello again from torc!"
```

> **Note:** Torc also accepts `.json`, `.json5` and `.kdl` workflow specifications. See
> [Workflow Specification Formats](../core/workflows/workflow-formats.md) for details.

## Run the Workflow

Run jobs locally with a short poll interval for demo purposes:

```console
torc run workflow.yaml --poll-interval 1
```

This creates the workflow, initializes it, and runs all jobs on the current machine.

## View Results

```console
torc results list
```

Or use the TUI for an interactive view:

```console
torc tui
```

## Example: Diamond Workflow

A workflow with fan-out and fan-in dependencies:

```yaml
name: diamond_workflow
description: Example workflow with implicit dependencies

jobs:
  - name: preprocess
    command: "bash tests/scripts/preprocess.sh -i ${files.input.f1} -o ${files.output.f2} -o ${files.output.f3}"

  - name: work1
    command: "bash tests/scripts/work.sh -i ${files.input.f2} -o ${files.output.f4}"

  - name: work2
    command: "bash tests/scripts/work.sh -i ${files.input.f3} -o ${files.output.f5}"

  - name: postprocess
    command: "bash tests/scripts/postprocess.sh -i ${files.input.f4} -i ${files.input.f5} -o ${files.output.f6}"

files:
  - name: f1
    path: f1.json
  - name: f2
    path: f2.json
  - name: f3
    path: f3.json
  - name: f4
    path: f4.json
  - name: f5
    path: f5.json
  - name: f6
    path: f6.json
```

Dependencies are automatically inferred from file inputs/outputs:

- `work1` and `work2` wait for `preprocess` (depend on its output files)
- `postprocess` waits for both `work1` and `work2` to complete

## More Examples

The [examples directory](https://github.com/NREL/torc/tree/main/examples) contains many more
workflow examples in YAML, JSON5, and KDL formats.

## Next Steps

- [Quick Start (HPC)](../specialized/hpc/quick-start-hpc.md) - Run workflows on Slurm clusters
- [Creating Workflows](../core/workflows/creating-workflows.md) - Detailed workflow creation guide
- [Terminal UI](../core/monitoring/tui.md) - Interactive workflow monitoring
