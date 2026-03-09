# Resource Requirements Reference

Technical reference for job resource specifications and allocation strategies.

## Resource Requirements Fields

| Field       | Type    | Required | Default | Description                                                       |
| ----------- | ------- | -------- | ------- | ----------------------------------------------------------------- |
| `name`      | string  | Yes      | —       | Identifier to reference from jobs                                 |
| `num_cpus`  | integer | No       | `1`     | Number of CPU cores                                               |
| `num_gpus`  | integer | No       | `0`     | Number of GPUs                                                    |
| `num_nodes` | integer | No       | `1`     | Number of nodes for the job (`sbatch --nodes` and `srun --nodes`) |
| `memory`    | string  | No       | `1m`    | Memory allocation (see format below)                              |
| `runtime`   | string  | No       | `PT1H`  | Maximum runtime (ISO 8601 duration)                               |

### Example

```yaml
resource_requirements:
  - name: small
    num_cpus: 2
    num_gpus: 0
    num_nodes: 1
    memory: 4g
    runtime: PT30M

  - name: large
    num_cpus: 16
    num_gpus: 2
    num_nodes: 1
    memory: 128g
    runtime: PT8H

  - name: mpi_job       # multi-node MPI or Julia Distributed.jl
    num_cpus: 32
    num_nodes: 4        # allocates and spans 4 nodes
    memory: 128g
    runtime: PT8H
```

### `num_nodes`

The `num_nodes` field controls both the Slurm allocation size (`sbatch --nodes`) and the number of
nodes each job step spans (`srun --nodes`).

For most jobs the value is `1` (default). Set it to a larger value for multi-node jobs such as MPI
or Julia `Distributed.jl`.

See [Multi-Node Jobs](../../specialized/hpc/multi-node-jobs.md) for detailed examples and guidance.

## Memory Format

String format with unit suffix:

| Suffix | Unit      | Example |
| ------ | --------- | ------- |
| `k`    | Kilobytes | `512k`  |
| `m`    | Megabytes | `512m`  |
| `g`    | Gigabytes | `16g`   |

Examples:

```yaml
memory: 512m    # 512 MB
memory: 1g      # 1 GB
memory: 16g     # 16 GB
```

## Runtime Format

ISO 8601 duration format:

| Format   | Description    | Example              |
| -------- | -------------- | -------------------- |
| `PTnM`   | Minutes        | `PT30M` (30 minutes) |
| `PTnH`   | Hours          | `PT2H` (2 hours)     |
| `PnD`    | Days           | `P1D` (1 day)        |
| `PnDTnH` | Days and hours | `P1DT12H` (1.5 days) |

Examples:

```yaml
runtime: PT10M      # 10 minutes
runtime: PT4H       # 4 hours
runtime: P1D        # 1 day
runtime: P1DT12H    # 1 day, 12 hours
```

## Job Allocation Strategies

### Resource-Based Allocation (Default)

The server considers each job's resource requirements and only returns jobs that fit within
available compute node resources.

**Behavior:**

- Considers CPU, memory, and GPU requirements
- Prevents resource over-subscription
- Enables efficient packing of heterogeneous workloads

**Configuration:** Run without `--max-parallel-jobs`:

```bash
torc run $WORKFLOW_ID
```

### Queue-Based Allocation

The server returns the next N ready jobs regardless of resource requirements.

**Behavior:**

- Ignores job resource requirements
- Only limits concurrent job count
- Simpler and faster (no resource calculation)

**Configuration:** Run with `--max-parallel-jobs`:

```bash
torc run $WORKFLOW_ID --max-parallel-jobs 10
```

**Use cases:**

- Homogeneous workloads where all jobs need similar resources
- Simple task queues
- When resource tracking overhead is not wanted

## Resource Tracking

When using resource-based allocation, the job runner tracks:

| Resource | Description                            |
| -------- | -------------------------------------- |
| CPUs     | Number of CPU cores in use             |
| Memory   | Total memory allocated to running jobs |
| GPUs     | Number of GPUs in use                  |
| Nodes    | Number of jobs running per node        |

Jobs are only started when sufficient resources are available.
