#!/bin/bash
set -e
mkdir -p /workspace/data /workspace/checkpoints /workspace/models /workspace/results
python3 -c "
import pickle, numpy as np
data = {'X': np.random.rand(50000, 128), 'y': np.random.randint(0, 10, 50000)}
with open('/workspace/data/dataset.pkl', 'wb') as f:
    pickle.dump(data, f)
print('Dataset prepared: 50000 samples, 128 features')
"
