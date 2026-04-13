#!/usr/bin/env python3
"""Compute A^T @ A for a single input matrix (CPU-bound BLAS operation).

Usage: multi_node_matrix_multiply.py <input_path> <output_path> <matrix_size>
"""

import os
import sys

import numpy as np

input_path = sys.argv[1]
output_path = sys.argv[2]
matrix_size = int(sys.argv[3])

num_threads = os.environ.get("OMP_NUM_THREADS", "all")
print(f"Loading input matrix from {input_path} (OMP_NUM_THREADS={num_threads})")

A = np.fromfile(input_path, dtype=np.float64).reshape(matrix_size, matrix_size)
print("Computing A^T @ A (CPU-bound BLAS)...")
result = A.T @ A
result.tofile(output_path)
print(f"Wrote {output_path} ({result.nbytes} bytes)")
