#!/usr/bin/env python3
"""
CPU-intensive job that computes prime numbers for approximately 60 seconds.
Uses multiprocessing to create 3 worker subprocesses to test process tree monitoring.
Uses only Python standard library.
"""

import time
import sys
import multiprocessing
import os

def is_prime(n):
    """Check if a number is prime using trial division."""
    if n < 2:
        return False
    if n == 2:
        return True
    if n % 2 == 0:
        return False

    # Check odd divisors up to sqrt(n)
    i = 3
    while i * i <= n:
        if n % i == 0:
            return False
        i += 2
    return True

def compute_primes_worker(worker_id, duration_seconds, return_dict):
    """
    Worker function that runs in a subprocess.
    Computes prime numbers for the specified duration.
    """
    pid = os.getpid()
    print(f"Worker {worker_id} (PID {pid}): Starting prime computation for {duration_seconds} seconds...")

    start_time = time.time()
    primes_found = 0
    largest_prime = 0
    n = 2 + worker_id  # Offset starting number to avoid duplicate work

    # Compute primes until time runs out
    while time.time() - start_time < duration_seconds:
        if is_prime(n):
            primes_found += 1
            largest_prime = n

            # Print progress every 1000 primes
            if primes_found % 1000 == 0:
                elapsed = time.time() - start_time
                print(f"Worker {worker_id} (PID {pid}): Found {primes_found} primes in {elapsed:.1f}s, largest: {largest_prime}")

        n += 3  # Skip by 3 to distribute work across workers

    elapsed = time.time() - start_time
    print(f"\nWorker {worker_id} (PID {pid}): Completed in {elapsed:.2f} seconds")
    print(f"Worker {worker_id}: Total primes found: {primes_found}")
    print(f"Worker {worker_id}: Largest prime: {largest_prime}")

    return_dict[worker_id] = {
        'primes_found': primes_found,
        'largest_prime': largest_prime,
        'pid': pid
    }

def main():
    """Main function that spawns 3 worker processes."""
    print(f"Main process PID: {os.getpid()}")
    print(f"Starting CPU-intensive job with 3 worker subprocesses...")

    # Shared dictionary for results (multiprocessing.Manager provides shared memory)
    manager = multiprocessing.Manager()
    return_dict = manager.dict()

    # Create 3 worker processes
    processes = []
    num_workers = 3
    duration_per_worker = 60

    start_time = time.time()

    for i in range(num_workers):
        p = multiprocessing.Process(
            target=compute_primes_worker,
            args=(i, duration_per_worker, return_dict)
        )
        processes.append(p)
        p.start()
        print(f"Started worker {i} with PID {p.pid}")

    # Wait for all workers to complete
    for i, p in enumerate(processes):
        p.join()
        print(f"Worker {i} finished")

    elapsed = time.time() - start_time

    # Aggregate results
    total_primes = sum(r['primes_found'] for r in return_dict.values())
    max_prime = max(r['largest_prime'] for r in return_dict.values())

    print(f"\n{'='*60}")
    print(f"All workers completed in {elapsed:.2f} seconds")
    print(f"Total primes found across all workers: {total_primes}")
    print(f"Largest prime found: {max_prime}")
    print(f"Worker PIDs: {[r['pid'] for r in return_dict.values()]}")
    print(f"{'='*60}")

    return total_primes

if __name__ == "__main__":
    try:
        primes = main()
        print(f"\n✓ CPU-intensive job with multiprocessing completed successfully")
        sys.exit(0)
    except Exception as e:
        print(f"\n✗ Error: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        sys.exit(1)
