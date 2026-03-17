#!/usr/bin/env python3
"""
Memory-intensive job that allocates and manipulates large data structures
for approximately 60 seconds. Uses only Python standard library.
"""

import time
import sys
import random


def allocate_memory(duration_seconds=60):
    """
    Allocate and manipulate large data structures.
    This is memory-intensive with moderate CPU usage.
    """
    print(f"Starting memory-intensive workload for {duration_seconds} seconds...")
    print(f"Job ID: {sys.argv[0]}")

    start_time = time.time()
    data_structures = []
    total_elements = 0
    iteration = 0

    # Allocate memory in chunks over the duration
    chunk_size = 500_000  # 500k integers per chunk
    target_chunks = 20  # Target ~20 chunks (10 million integers total)

    while time.time() - start_time < duration_seconds:
        iteration += 1

        # Allocate a new chunk of data
        if len(data_structures) < target_chunks:
            chunk = [random.randint(0, 1000000) for _ in range(chunk_size)]
            data_structures.append(chunk)
            total_elements += len(chunk)

            elapsed = time.time() - start_time
            memory_mb = (total_elements * 8) / (1024 * 1024)  # Rough estimate
            print(
                f"Iteration {iteration}: Allocated {len(data_structures)} chunks, "
                f"~{memory_mb:.1f} MB, elapsed: {elapsed:.1f}s"
            )
        else:
            # Once we've allocated target memory, do some operations on it
            # to keep memory active
            chunk_idx = random.randint(0, len(data_structures) - 1)
            chunk = data_structures[chunk_idx]

            # Sort the chunk (CPU + memory work)
            chunk.sort()

            # Reverse it
            chunk.reverse()

            # Sum it (to prevent optimization)
            total = sum(chunk[:1000])

            if iteration % 5 == 0:
                elapsed = time.time() - start_time
                print(
                    f"Iteration {iteration}: Working on chunk {chunk_idx}, "
                    f"sample sum: {total}, elapsed: {elapsed:.1f}s"
                )

        # Small sleep to avoid spinning too fast
        time.sleep(0.5)

    elapsed = time.time() - start_time
    memory_mb = (total_elements * 8) / (1024 * 1024)
    print(f"\nCompleted in {elapsed:.2f} seconds")
    print(f"Total chunks allocated: {len(data_structures)}")
    print(f"Total elements: {total_elements:,}")
    print(f"Approximate memory used: {memory_mb:.1f} MB")

    return len(data_structures)


if __name__ == "__main__":
    try:
        chunks = allocate_memory(60)
        print("\n✓ Memory-intensive job completed successfully")
        sys.exit(0)
    except Exception as e:
        print(f"\n✗ Error: {e}", file=sys.stderr)
        sys.exit(1)
