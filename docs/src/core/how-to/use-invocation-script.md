# How to Use Invocation Scripts

Set up the job environment before running the command.

## The Problem

Your jobs need a specific environment (conda, modules, virtualenv) but you don't want to embed
activation commands in every job command.

## The Solution

Use `invocation_script` to wrap job commands with environment setup.

## Step 1: Create a Setup Script

Create a script that sets up your environment and executes the command passed to it:

```bash
#!/bin/bash
# setup.sh - Environment wrapper for jobs

# Load required modules (HPC systems)
module load conda

# Activate the conda environment
conda activate my_env

# Execute the job command (passed as arguments)
"$@"
```

The key is `"$@"` at the end, which runs the job's command with all its arguments.

## Step 2: Reference in Job Definition

Add `invocation_script` to your job:

```yaml
jobs:
  - name: train_model
    command: python train.py --epochs 100 --output model.pt
    invocation_script: bash setup.sh
```

When this job runs, Torc executes:

```bash
bash setup.sh python train.py --epochs 100 --output model.pt
```

The setup script loads conda, activates the environment, then runs `python train.py ...`.

## Complete Example

**setup.sh:**

```bash
#!/bin/bash
module load conda
conda activate ml_env
"$@"
```

**workflow.yaml:**

```yaml
name: ml_training
description: Train models with conda environment

jobs:
  - name: preprocess
    command: python preprocess.py --input data.csv --output processed.pkl
    invocation_script: bash setup.sh

  - name: train
    command: python train.py --data processed.pkl --model model.pt
    invocation_script: bash setup.sh
    depends_on: [preprocess]

  - name: evaluate
    command: python evaluate.py --model model.pt --output results.json
    invocation_script: bash setup.sh
    depends_on: [train]
```

## Common Patterns

### Python Virtual Environment

```bash
#!/bin/bash
source /path/to/venv/bin/activate
"$@"
```

### HPC Module System

```bash
#!/bin/bash
module purge
module load gcc/11.2 cuda/11.8 python/3.10
"$@"
```

### Combined Setup

```bash
#!/bin/bash
# Load modules
module load cuda/11.8

# Activate conda
source /opt/conda/etc/profile.d/conda.sh
conda activate my_env

# Set environment variables
export CUDA_VISIBLE_DEVICES=0

# Run the command
"$@"
```

### Using Apptainer

```bash
#!/bin/bash
apptainer exec /path/to/container.sif "$@"
```

## Tips

- Make the setup script executable: `chmod +x setup.sh`
- Use absolute paths for portability across nodes
- Test the script manually before submitting: `bash setup.sh python --version`
- Different jobs can use different invocation scripts

## See Also

- [Workflow Specification](../reference/workflow-spec.md) - Full JobSpec reference
- [Slurm Workflows](../../specialized/hpc/slurm-workflows.md) - HPC-specific configuration
