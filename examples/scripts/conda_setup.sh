#!/bin/bash
# conda_setup.sh - Environment wrapper for Python jobs
#
# This script sets up a conda environment before running the job command.
# Usage: bash conda_setup.sh <command> [args...]
#
# Example:
#   bash conda_setup.sh python train.py --epochs 100
#
# In a workflow, reference it with invocation_script:
#   jobs:
#     - name: train
#       command: python train.py --epochs 100
#       invocation_script: bash scripts/conda_setup.sh

# Load conda module (common on HPC systems)
# Uncomment and modify for your system:
# module load conda

# Source conda (if not automatically available)
# source /opt/conda/etc/profile.d/conda.sh

# Activate the environment
conda activate my_env

# Execute the job command with all arguments
"$@"
