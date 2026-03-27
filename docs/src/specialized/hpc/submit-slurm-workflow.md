# Submitting Slurm Workflows

Submit a workflow specification to a Slurm-based HPC system with automatic scheduler generation.

## Quick Start

Generate Slurm schedulers and submit in two steps:

```bash
torc slurm generate --account <your-account> workflow.yaml
torc submit workflow.yaml
```

Torc will:

1. Detect your HPC system (e.g., NLR Kestrel)
2. Match job requirements to appropriate partitions
3. Generate Slurm scheduler configurations
4. Submit everything for execution

## Preview Before Submitting

Preview the generated configuration without creating anything:

```bash
torc slurm generate --account <your-account> --dry-run workflow.yaml
```

This shows the Slurm schedulers and workflow actions that would be created.

## Requirements

Your workflow must define resource requirements for jobs:

```yaml
name: my_workflow

resource_requirements:
  - name: standard
    num_cpus: 4
    memory: 8g
    runtime: PT1H

jobs:
  - name: process_data
    command: python process.py
    resource_requirements: standard
```

## Options

```bash
# See all options
torc slurm generate --help
torc submit --help
```

## See Also

- [Slurm Overview](../../specialized/hpc/slurm-workflows.md) — Full Slurm integration guide
- [HPC Profiles](../../specialized/hpc/hpc-profiles.md) — Available HPC system configurations
