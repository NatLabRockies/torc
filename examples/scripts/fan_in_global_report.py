#!/usr/bin/env python3
"""Aggregate all regional summaries into a global report.

Usage: fan_in_global_report.py
"""

import glob
import json

summaries = []
for f in sorted(glob.glob("output/summary_*.json")):
    with open(f) as fh:
        summaries.append(json.load(fh))

report = {
    "regions": {s["region"]: s for s in summaries},
    "global_mean": sum(s["mean"] for s in summaries) / len(summaries),
    "total_simulations": sum(s["num_simulations"] for s in summaries),
}

with open("output/global_report.json", "w") as f:
    json.dump(report, f, indent=2)
print(json.dumps(report, indent=2))
