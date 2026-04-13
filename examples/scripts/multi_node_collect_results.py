#!/usr/bin/env python3
"""Collect eigenvalue results and produce a summary report.

Usage: multi_node_collect_results.py <scratch_dir>
"""

import json
import sys

import numpy as np

scratch_dir = sys.argv[1]
eigs = np.fromfile(f"{scratch_dir}/eigenvalues.bin", dtype=np.float64)

report = {
    "num_eigenvalues": len(eigs),
    "min": float(eigs.min()),
    "max": float(eigs.max()),
    "mean": float(eigs.mean()),
    "condition_number": float(eigs.max() / eigs[eigs > 0].min()),
}

output_path = f"{scratch_dir}/report.json"
with open(output_path, "w") as f:
    json.dump(report, f, indent=2)
print(json.dumps(report, indent=2))
