#!/bin/bash
# Simulates an MPI-style job that uses a multi-node allocation.
#
# With start_one_worker_per_node=false (default), a single torc worker runs
# the job via srun --nodes=2 --ntasks=1.  The allocation spans 2 nodes but only
# one task executes.  We verify the allocation size (SLURM_JOB_NUM_NODES) to
# confirm that the scheduler requested the correct number of nodes.

echo "=== Multi-node step job ==="
echo "Hostname: $(hostname)"
echo "SLURM_JOB_NODELIST: ${SLURM_JOB_NODELIST:-not set}"
echo "SLURM_STEP_NODELIST: ${SLURM_STEP_NODELIST:-not set}"
echo "SLURM_NNODES: ${SLURM_NNODES:-not set}"
echo "SLURM_JOB_NUM_NODES: ${SLURM_JOB_NUM_NODES:-not set}"
echo "SLURM_NTASKS: ${SLURM_NTASKS:-not set}"
echo "SLURM_STEP_NUM_NODES: ${SLURM_STEP_NUM_NODES:-not set}"

# Check the allocation node count (not the step node count)
ALLOC_NODES=${SLURM_JOB_NUM_NODES:-${SLURM_NNODES:-1}}
echo "Allocation node count: $ALLOC_NODES"

if [ "$ALLOC_NODES" -lt 2 ]; then
    echo "WARNING: Expected at least 2 nodes in the allocation, got $ALLOC_NODES"
    echo "Check that the Slurm scheduler has nodes=2"
fi

# Simulate work
echo "Doing multi-node work..."
sleep 2

echo "Multi-node step complete."
