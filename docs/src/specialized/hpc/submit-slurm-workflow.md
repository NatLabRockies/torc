# How to Submit a Workflow to Slurm

Submit a workflow specification to a Slurm-based HPC system with automatic scheduler generation.

## Quick Start

```bash
torc submit-slurm --account <your-account> workflow.yaml
```

Torc will:

1. Detect your HPC system (e.g., NLR Kestrel)
2. Match job requirements to appropriate partitions
3. Generate Slurm scheduler configurations
4. Submit everything for execution

## Preview Before Submitting

Always preview the generated configuration first:

```bash
torc slurm generate --account <your-account> workflow.yaml
```

This shows the Slurm schedulers and workflow actions that would be created without submitting.

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
torc submit-slurm --help
```

## See Also

- [Slurm Workflows](../../specialized/hpc/slurm-workflows.md) — Full Slurm integration guide
- [HPC Profiles](../../specialized/hpc/hpc-profiles.md) — Available HPC system configurations
