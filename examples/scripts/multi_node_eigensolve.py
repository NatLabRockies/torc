#!/usr/bin/env python3
"""Compute eigenvalue decomposition of averaged product matrices.

Usage: multi_node_eigensolve.py <scratch_dir> <num_matrices> <matrix_size>
"""

import os
import sys

import numpy as np

scratch_dir = sys.argv[1]
num_matrices = int(sys.argv[2])
matrix_size = int(sys.argv[3])

print(f"Loading {num_matrices} product matrices...")
matrices = []
for i in range(num_matrices):
    path = os.path.join(scratch_dir, f"product_{i:02d}.bin")
    matrices.append(
        np.fromfile(path, dtype=np.float64).reshape(matrix_size, matrix_size)
    )

combined = sum(matrices) / len(matrices)
print("Computing eigenvalue decomposition (CPU-intensive)...")
eigenvalues = np.linalg.eigvalsh(combined)

output_path = os.path.join(scratch_dir, "eigenvalues.bin")
eigenvalues.tofile(output_path)
print(f"Computed {len(eigenvalues)} eigenvalues")
