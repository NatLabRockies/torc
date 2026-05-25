# Submitting Slurm Workflows

Submit a workflow specification to a Slurm-based HPC system with automatic scheduler generation.

## Quick Start

Generate Slurm schedulers and submit in a single pipeline. `torc slurm generate` writes the
augmented spec to stdout, and `torc submit -` reads it from stdin:

```bash
torc slurm generate --account <your-account> workflow.yaml | torc submit -
```

Prefer two explicit steps? Save the generated spec to a file first, then submit that file:

```bash
torc slurm generate --account <your-account> -o workflow_with_slurm.yaml workflow.yaml
torc submit workflow_with_slurm.yaml
```

> Note: `torc slurm generate` does not modify the input file in place — it emits the augmented spec
> to stdout (or to `-o <file>`). Submit the generated output, not the original `workflow.yaml`.

Either way, torc will:

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
