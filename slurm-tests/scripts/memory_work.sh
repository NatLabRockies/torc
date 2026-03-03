#!/bin/bash
# Controlled memory allocation for resource monitoring test.
# Allocates 500MB and holds it for 30 seconds.

echo "Memory work starting on $(hostname) at $(date)"

python3 << 'EOF'
import time

SIZE_MB = 500
size_bytes = SIZE_MB * 1024 * 1024

print(f"Allocating {SIZE_MB}MB of memory...")
data = bytearray(size_bytes)

# Touch every page to force physical allocation
for i in range(0, len(data), 4096):
    data[i] = 1

print(f"Allocated and touched {SIZE_MB}MB. Holding for 30 seconds...")
time.sleep(30)

print("Memory work complete.")
EOF

echo "Memory work finished at $(date)"
