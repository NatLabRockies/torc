# Quick Start (Remote Workers)

This guide walks you through running a Torc workflow on multiple remote machines via SSH. Jobs are
distributed across workers without requiring an HPC scheduler like Slurm.

For local execution, see [Quick Start (Local)](../getting-started/quick-start-local.md). For
HPC/Slurm execution, see [Quick Start (HPC)](../getting-started/quick-start-hpc.md).

## Prerequisites

- SSH key-based authentication to all remote machines (no password prompts)
- Torc installed on all machines with **matching versions**
- Torc server accessible from all machines

## Start the Server

Start a Torc server that's accessible from the remote machines. This typically means binding to a
network interface (not just localhost):

```console
torc-server run --database torc.db --url 0.0.0.0 --port 8080
```

## Create a Worker File

Create a file listing the remote machines. Each line contains one machine in the format
`[user@]hostname[:port]`:

```text
# workers.txt
worker1.example.com
alice@worker2.example.com
admin@192.168.1.10:2222
```

Lines starting with `#` are comments. Empty lines are ignored.

## Create a Workflow

Save this as `workflow.yaml`:

```yaml
name: distributed_hello
description: Distributed hello world workflow

jobs:
  - name: job 1
    command: echo "Hello from $(hostname)!"
  - name: job 2
    command: echo "Hello again from $(hostname)!"
  - name: job 3
    command: echo "And once more from $(hostname)!"
```

## Create the Workflow on the Server

```console
torc workflows create workflow.yaml
```

Note the workflow ID in the output.

## Run Workers on Remote Machines

Start workers on all remote machines. Each worker will poll for available jobs and execute them:

```console
torc remote run --workers workers.txt <workflow-id> --poll-interval 5
```

This will:

1. Check SSH connectivity to all machines
2. Verify all machines have the same torc version
3. Start a worker process on each machine (detached via `nohup`)
4. Report which workers started successfully

## Check Worker Status

Monitor which workers are still running:

```console
torc remote status <workflow-id>
```

## View Workflow Progress

Check job status from any machine:

```console
torc jobs list <workflow-id>
```

Or use the interactive TUI:

```console
torc tui
```

## Collect Logs

After the workflow completes, collect logs from all workers:

```console
torc remote collect-logs <workflow-id> --local-output-dir ./logs
```

This creates a tarball for each worker containing:

- Worker logs: `torc_worker_<workflow_id>.log`
- Job stdout/stderr: `job_stdio/job_*.o` and `job_stdio/job_*.e`
- Resource utilization data (if enabled): `resource_utilization/resource_metrics_*.db`

## Stop Workers

If you need to stop workers before the workflow completes:

```console
torc remote stop <workflow-id>
```

Add `--force` to send SIGKILL instead of SIGTERM.

## Next Steps

- [Remote Workers Guide](./remote-workers.md) - Detailed configuration and troubleshooting
- [Creating Workflows](../../core/workflows/creating-workflows.md) - Workflow specification format
- [Resource Monitoring](../../core/monitoring/resource-monitoring.md) - Track CPU/memory usage per
  job
