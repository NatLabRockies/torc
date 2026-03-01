#!/bin/bash
# Single-node work job for parallel dispatch test.
#
# Prints hostname and timing info so verification steps can confirm
# that jobs ran on different nodes and overlapped in time.

JOB_NUM=${1:-?}

echo "=== Job $JOB_NUM starting at $(date +%T) on $(hostname) ==="
echo "SLURM_JOB_ID: ${SLURM_JOB_ID:-not set}"
echo "SLURM_STEP_ID: ${SLURM_STEP_ID:-not set}"
echo "SLURM_STEP_NODELIST: ${SLURM_STEP_NODELIST:-not set}"

# Consume a bit of CPU so sstat can record a non-zero reading
echo "Working..."
for _ in $(seq 1 5); do
    # Busy-wait to generate CPU load
    python3 -c "
import time
t = time.time()
while time.time() - t < 1:
    _ = sum(range(100000))
" 2>/dev/null || sleep 1
done

echo "=== Job $JOB_NUM complete at $(date +%T) on $(hostname) ==="
