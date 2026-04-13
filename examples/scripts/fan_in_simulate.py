#!/usr/bin/env python3
"""Run a stochastic simulation for a given region and seed.

Usage: fan_in_simulate.py <region> <seed>
"""

import csv
import json
import random
import sys

region = sys.argv[1]
seed = int(sys.argv[2])

random.seed(seed)
with open("output/config.json") as f:
    cfg = json.load(f)

results = [random.gauss(0, 1) for _ in range(cfg["timesteps"])]

outpath = f"output/sim_{region}_{seed:03d}.csv"
with open(outpath, "w", newline="") as f:
    w = csv.writer(f)
    w.writerow(["timestep", "value"])
    for i, v in enumerate(results):
        w.writerow([i, v])

print(f"Wrote seed {seed:03d} for region {region}: {cfg['timesteps']} timesteps")
