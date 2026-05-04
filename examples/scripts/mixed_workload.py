#!/usr/bin/env python3
"""
Mixed workload job that combines CPU-intensive computation with memory allocation
for approximately 60 seconds. Uses only Python standard library.
"""

import time
import sys
import random
import hashlib


def fibonacci(n):
    """Compute the nth Fibonacci number recursively (CPU intensive)."""
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)


def compute_hashes(data):
    """Compute SHA256 hashes of data (CPU intensive)."""
    result = []
    for item in data:
        hash_obj = hashlib.sha256(str(item).encode())
        result.append(hash_obj.hexdigest())
    return result


def mixed_workload(duration_seconds=60):
    """
    Perform a mix of CPU-intensive computation and memory allocation.
    Alternates between computation phases and memory phases.
    """
    print(f"Starting mixed workload for {duration_seconds} seconds...")
    print(f"Job ID: {sys.argv[0]}")

    start_time = time.time()
    data_store = []
    computation_count = 0
    iteration = 0

    while time.time() - start_time < duration_seconds:
        iteration += 1
        elapsed = time.time() - start_time

        # Phase 1: Memory allocation (even iterations)
        if iteration % 2 == 0:
            # Allocate a chunk of random data
            chunk = [random.random() for _ in range(100_000)]
            data_store.append(chunk)

            # Keep only last 10 chunks to limit memory growth
            if len(data_store) > 10:
                data_store.pop(0)

            memory_mb = (sum(len(c) for c in data_store) * 8) / (1024 * 1024)
            print(
                f"[{elapsed:.1f}s] Memory phase: {len(data_store)} chunks, "
                f"~{memory_mb:.1f} MB"
            )

        # Phase 2: CPU computation (odd iterations)
        else:
            # Compute Fibonacci numbers
            fib_results = []
            for n in range(25, 31):  # Fib(25) to Fib(30)
                fib_results.append(fibonacci(n))

            computation_count += len(fib_results)

            # Compute hashes on some stored data
            if data_store:
                chunk_idx = random.randint(0, len(data_store) - 1)
                sample_data = data_store[chunk_idx][:100]
                _ = compute_hashes(sample_data)

            print(
                f"[{elapsed:.1f}s] CPU phase: computed {len(fib_results)} Fibonacci numbers, "
                f"largest: {fib_results[-1]:,}"
            )

        # Small sleep between phases
        time.sleep(0.5)

    elapsed = time.time() - start_time
    total_memory_elements = sum(len(c) for c in data_store)
    memory_mb = (total_memory_elements * 8) / (1024 * 1024)

    print(f"\nCompleted in {elapsed:.2f} seconds")
    print(f"Total iterations: {iteration}")
    print(f"Computation count: {computation_count}")
    print(f"Final data chunks: {len(data_store)}")
    print(f"Final memory usage: ~{memory_mb:.1f} MB")

    return iteration


if __name__ == "__main__":
    try:
        iterations = mixed_workload(60)
        print("\n✓ Mixed workload job completed successfully")
        sys.exit(0)
    except Exception as e:
        print(f"\n✗ Error: {e}", file=sys.stderr)
        sys.exit(1)
