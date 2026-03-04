#!/bin/bash
# Sustained CPU load for resource monitoring test.
# Generates ~100% CPU usage on 1 core for 30 seconds.

echo "CPU work starting on $(hostname) at $(date)"

python3 << 'EOF'
import time

print("Generating sustained CPU load for 30 seconds...")
end_time = time.time() + 30
iterations = 0
while time.time() < end_time:
    # Busy-wait: compute-intensive loop
    _ = sum(range(100000))
    iterations += 1

print(f"CPU work complete: {iterations} iterations")
EOF

echo "CPU work finished at $(date)"
