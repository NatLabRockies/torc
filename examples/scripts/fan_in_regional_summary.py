#!/usr/bin/env python3
"""Aggregate simulation outputs for a single region.

Usage: fan_in_regional_summary.py <region>
"""

import csv
import glob
import json
import sys

import numpy as np

region = sys.argv[1]

files = sorted(glob.glob(f"output/sim_{region}_*.csv"))
all_values = []
for f in files:
    with open(f) as fh:
        reader = csv.DictReader(fh)
        all_values.extend(float(row["value"]) for row in reader)

arr = np.array(all_values)
summary = {
    "region": region,
    "num_simulations": len(files),
    "total_samples": len(arr),
    "mean": float(arr.mean()),
    "std": float(arr.std()),
    "min": float(arr.min()),
    "max": float(arr.max()),
}

outpath = f"output/summary_{region}.json"
with open(outpath, "w") as f:
    json.dump(summary, f, indent=2)
print(json.dumps(summary, indent=2))
