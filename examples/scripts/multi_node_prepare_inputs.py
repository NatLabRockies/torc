#!/usr/bin/env python3
"""Generate random input matrices for the multi-node linear algebra pipeline.

Usage: multi_node_prepare_inputs.py <scratch_dir> <num_matrices> <matrix_size>
"""

import os
import sys

import numpy as np

scratch_dir = sys.argv[1]
num_matrices = int(sys.argv[2])
matrix_size = int(sys.argv[3])

os.makedirs(scratch_dir, exist_ok=True)
for i in range(num_matrices):
    M = np.random.rand(matrix_size, matrix_size)
    path = os.path.join(scratch_dir, f"input_matrix_{i:02d}.bin")
    M.tofile(path)
    print(f"Generated {matrix_size}x{matrix_size} matrix: input_matrix_{i:02d}.bin")
