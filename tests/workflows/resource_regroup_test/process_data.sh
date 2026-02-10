#!/bin/bash
# Simulates data processing for a given chunk index.
# Memory usage varies per chunk based on input data characteristics.
#
# Usage: process_data.sh <chunk_index>

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CHUNK_INDEX=${1:?Usage: process_data.sh <chunk_index>}
HOLD_SECONDS=15

# Read workload configuration
CONFIG_FILE="${SCRIPT_DIR}/workloads.conf"
if [ ! -f "$CONFIG_FILE" ]; then
    echo "Error: workload config not found: $CONFIG_FILE" >&2
    exit 1
fi

SIZE_MB=$(grep "^${CHUNK_INDEX} " "$CONFIG_FILE" | awk '{print $2}')
if [ -z "$SIZE_MB" ]; then
    echo "Error: no config for chunk index ${CHUNK_INDEX}" >&2
    exit 1
fi

echo "Processing chunk ${CHUNK_INDEX} on $(hostname) at $(date)"

python3 << EOF
import time

size_mb = ${SIZE_MB}
size_bytes = size_mb * 1024 * 1024
hold = ${HOLD_SECONDS}

data = bytearray(size_bytes)
for i in range(0, len(data), 4096):
    data[i] = 1
print(f"Processing chunk ${CHUNK_INDEX}: allocated working set, holding for {hold}s...")
time.sleep(hold)
print("Chunk processing complete")
EOF

echo "Chunk ${CHUNK_INDEX} finished at $(date)"
