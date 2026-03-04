#!/bin/bash
# Script that allocates a specified amount of memory in GB.
#
# Usage: allocate_memory.sh <size_in_gb>
#
# Designed to trigger OOM when the allocation exceeds Slurm's cgroup memory limit.

set -e

SIZE_GB=${1:-30}
SIZE_MB=$((SIZE_GB * 1024))

echo "Job starting: $(hostname)"
echo "Requested memory allocation: ${SIZE_GB}GB (${SIZE_MB}MB)"
echo "Start time: $(date)"

# Check available memory
if [ -f /proc/meminfo ]; then
    AVAILABLE_MB=$(grep MemAvailable /proc/meminfo | awk '{print int($2/1024)}')
    echo "Available memory: ${AVAILABLE_MB}MB"
fi

# Allocate memory using Python (reliable for triggering OOM)
echo "Allocating ${SIZE_GB}GB of memory..."

python3 << EOF
import sys
import time

size_gb = ${SIZE_GB}
size_bytes = size_gb * 1024 * 1024 * 1024

print(f"Attempting to allocate {size_gb}GB ({size_bytes:,} bytes)")

try:
    data = bytearray(size_bytes)
    # Touch the memory to ensure it's actually allocated
    for i in range(0, len(data), 4096):
        data[i] = 1
    print(f"Successfully allocated and touched {size_gb}GB of memory")

    # Hold the memory for a bit to simulate work
    print("Holding memory for 10 seconds to simulate work...")
    time.sleep(10)

    print("Work completed successfully!")
except MemoryError as e:
    print(f"MemoryError: Failed to allocate {size_gb}GB - {e}")
    sys.exit(137)  # Exit code for OOM
EOF

echo "Job completed: $(date)"
