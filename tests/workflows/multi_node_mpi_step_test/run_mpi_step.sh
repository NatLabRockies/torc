#!/bin/bash
# Simulates an MPI-style job that uses all allocated nodes.
# Run via srun with --nodes=2 so each task runs on a different node.
#
# This script is invoked by torc via:
#   srun --nodes=2 --ntasks=1 bash run_mpi_step.sh
#
# The environment will contain Slurm multi-node variables if num_nodes=2 is set.

echo "=== Multi-node step job ==="
echo "Hostname: $(hostname)"
echo "SLURM_JOB_NODELIST: ${SLURM_JOB_NODELIST:-not set}"
echo "SLURM_STEP_NODELIST: ${SLURM_STEP_NODELIST:-not set}"
echo "SLURM_NNODES: ${SLURM_NNODES:-not set}"
echo "SLURM_NTASKS: ${SLURM_NTASKS:-not set}"
echo "SLURM_STEP_NUM_NODES: ${SLURM_STEP_NUM_NODES:-not set}"

# Verify we see multiple nodes in the step node list
NODE_COUNT=${SLURM_STEP_NUM_NODES:-${SLURM_NNODES:-1}}
echo "Node count visible to this step: $NODE_COUNT"

if [ "$NODE_COUNT" -lt 2 ]; then
    echo "WARNING: Expected at least 2 nodes in the step, got $NODE_COUNT"
    echo "Check that num_nodes=2 is set in resource requirements"
fi

# Simulate work spanning multiple nodes
echo "Doing multi-node work..."
sleep 2

echo "Multi-node step complete."
