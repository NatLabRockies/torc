#!/bin/bash
set -e
python3 -c "
import numpy as np, json, glob
models = glob.glob('/workspace/models/model_*.pt.npy')
best_loss = float('inf')
best_model = None
for m in models:
    data = np.load(m, allow_pickle=True).item()
    if data['final_loss'] < best_loss:
        best_loss = data['final_loss']
        best_model = m
report = {'best_model': best_model, 'best_loss': best_loss, 'num_models': len(models)}
with open('/workspace/results/evaluation.json', 'w') as f:
    json.dump(report, f, indent=2)
print(json.dumps(report, indent=2))
"
