#!/bin/bash
# Allocate 1GB of memory and hold it briefly.
# Used by the job_parallelism test to verify memory tracking
# works for jobs without explicit resource requirements.

JOB_NUM=${1:-?}

echo "=== Memory job $JOB_NUM starting at $(date +%T) on $(hostname) ==="

python3 << 'EOF'
import time

SIZE_MB = 1024
size_bytes = SIZE_MB * 1024 * 1024

print(f"Allocating {SIZE_MB}MB of memory...")
data = bytearray(size_bytes)

# Touch every page to force physical allocation
for i in range(0, len(data), 4096):
    data[i] = 1

print(f"Allocated and touched {SIZE_MB}MB. Holding for 10 seconds...")
time.sleep(10)

print("Memory work complete.")
EOF

echo "=== Memory job $JOB_NUM complete at $(date +%T) on $(hostname) ==="
