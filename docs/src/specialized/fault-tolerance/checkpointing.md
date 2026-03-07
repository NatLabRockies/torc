# How to Checkpoint a Job During Wall-Time Timeout

When running jobs on HPC systems like Slurm, your job may be terminated when it exceeds its
configured runtime. Torc supports **graceful termination**, allowing jobs to save checkpoints before
exiting. This guide explains how to configure your jobs to handle wall-time timeouts gracefully.

## Overview

Torc wraps each job with `srun --time=<remaining_minutes>`, where `remaining_minutes` is the time
left in the Slurm allocation. This ensures Slurm enforces a per-step walltime and produces a clean
`TIMEOUT` state when time runs out. When a job exceeds its step time limit:

1. Slurm sends **SIGTERM** to the job process
2. The job has `KillWait` seconds (typically 30s, configured in `slurm.conf`) to save state
3. After the grace period, Slurm sends **SIGKILL** to force termination
4. Torc detects the `TIMEOUT` state via sacct and reports return code 152

Jobs that handle SIGTERM can catch the signal and perform cleanup operations like saving
checkpoints, flushing buffers, or releasing resources.

### Early Warning with `srun_termination_signal`

By default, jobs only get `KillWait` seconds (typically 30s) between SIGTERM and SIGKILL. For jobs
that need more time to checkpoint (e.g., saving a large model to disk), set
`srun_termination_signal` in your workflow spec:

```yaml
name: my_workflow
srun_termination_signal: "TERM@120"
```

This passes `srun --signal=TERM@120`, which sends SIGTERM **120 seconds before** the step time
limit. The job then has 120 seconds to save its checkpoint before the normal SIGTERM/SIGKILL
sequence at the time limit.

## Writing a Job That Handles SIGTERM

Your job script must catch SIGTERM and save its state. Here's a Python example:

```python
import signal
import sys
import pickle

# Global state
checkpoint_path = "/scratch/checkpoints/model.pkl"
model_state = None

def save_checkpoint():
    """Save current model state to disk."""
    print("Saving checkpoint...")
    with open(checkpoint_path, 'wb') as f:
        pickle.dump(model_state, f)
    print(f"Checkpoint saved to {checkpoint_path}")

def handle_sigterm(signum, frame):
    """Handle SIGTERM by saving checkpoint and exiting."""
    print("Received SIGTERM - saving checkpoint before exit")
    save_checkpoint()
    sys.exit(0)  # Exit cleanly after saving

# Register the signal handler
signal.signal(signal.SIGTERM, handle_sigterm)

# Main training loop
def train():
    global model_state
    for epoch in range(1000):
        # Training logic here...
        model_state = {"epoch": epoch, "weights": [...]}

        # Optionally save periodic checkpoints
        if epoch % 100 == 0:
            save_checkpoint()

if __name__ == "__main__":
    train()
```

### Bash Script Example

For shell scripts, use `trap` to catch SIGTERM:

```bash
#!/bin/bash

CHECKPOINT_FILE="/scratch/checkpoints/progress.txt"

# Function to save checkpoint
save_checkpoint() {
    echo "Saving checkpoint at iteration $ITERATION"
    echo "$ITERATION" > "$CHECKPOINT_FILE"
}

# Trap SIGTERM and save checkpoint
trap 'save_checkpoint; exit 0' SIGTERM

# Load checkpoint if exists
if [ -f "$CHECKPOINT_FILE" ]; then
    ITERATION=$(cat "$CHECKPOINT_FILE")
    echo "Resuming from iteration $ITERATION"
else
    ITERATION=0
fi

# Main loop
while [ $ITERATION -lt 1000 ]; do
    # Do work...
    ITERATION=$((ITERATION + 1))
    sleep 1
done
```

### Complete Workflow Example

```yaml
name: ml_training_workflow
user: researcher
srun_termination_signal: "TERM@120"

jobs:
  - name: preprocess
    command: python preprocess.py

  - name: train_model
    command: python train.py --checkpoint-dir /scratch/checkpoints
    depends_on:
      - preprocess
    resource_requirements:
      num_cpus: 8
      memory: 32g
      num_gpus: 1
      runtime: PT4H

  - name: evaluate
    command: python evaluate.py
    depends_on:
      - train_model

slurm_schedulers:
  - name: gpu_scheduler
    account: my_project
    partition: gpu
    nodes: 1
    walltime: "04:00:00"

actions:
  - trigger_type: on_workflow_start
    action_type: schedule_nodes
    scheduler: gpu_scheduler
    scheduler_type: slurm
    num_allocations: 1
```

With `srun_termination_signal: "TERM@120"`, Slurm sends SIGTERM 120 seconds before the allocation
ends, giving `train_model` time to save a checkpoint before the hard kill.

## Restarting After Termination

When a job is terminated due to wall-time, it will have status `terminated`. To continue the
workflow:

1. **Re-submit the workflow** to allocate new compute time:
   ```bash
   torc workflows submit $WORKFLOW_ID
   ```

2. **Reinitialize terminated jobs** to make them ready again:
   ```bash
   torc workflows reinitialize $WORKFLOW_ID
   ```

Your job script should detect existing checkpoints and resume from where it left off.

## Best Practices

### 1. Verify Checkpoint Integrity

Add validation to ensure checkpoints are complete:

```python
def save_checkpoint():
    temp_path = checkpoint_path + ".tmp"
    with open(temp_path, 'wb') as f:
        pickle.dump(model_state, f)
    # Atomic rename ensures complete checkpoint
    os.rename(temp_path, checkpoint_path)
```

### 2. Handle Multiple Termination Signals

Some systems send multiple signals. Ensure your handler is idempotent:

```python
checkpoint_saved = False

def handle_sigterm(signum, frame):
    global checkpoint_saved
    if not checkpoint_saved:
        save_checkpoint()
        checkpoint_saved = True
    sys.exit(0)
```

### 3. Test Locally

Test your SIGTERM handling locally before running on the cluster:

```bash
# Start your job
python train.py &
PID=$!

# Wait a bit, then send SIGTERM
sleep 10
kill -TERM $PID

# Verify checkpoint was saved
ls -la /scratch/checkpoints/
```

## Troubleshooting

### Job Killed Without Checkpointing

**Symptoms:** Job status is `terminated` but no checkpoint was saved.

**Causes:**

- Signal handler not registered before training started
- Checkpoint save took longer than the `KillWait` grace period

**Solutions:**

- Register signal handlers early in your script
- Ask your Slurm admin to increase `KillWait` if needed

### Checkpoint File Corrupted

**Symptoms:** Job fails to load checkpoint on restart.

**Causes:**

- Job was killed during checkpoint write
- Disk space exhausted

**Solutions:**

- Use atomic file operations (write to temp, then rename)
- Check available disk space before checkpointing
- Implement checkpoint validation on load

### Job Doesn't Receive SIGTERM

**Symptoms:** Job runs until hard kill with no graceful shutdown.

**Causes:**

- Job running in a subprocess that doesn't forward signals
- Container or wrapper script intercepting signals

**Solutions:**

- Use `exec` in wrapper scripts to replace the shell
- Configure signal forwarding in containers
- Run the job directly without wrapper scripts

## See Also

- [Advanced Slurm Configuration](./slurm.md) - Manual Slurm scheduler setup
- [Managing Resources](./resources.md) - Resource requirements configuration
- [Debugging Workflows](./debugging.md) - Troubleshooting workflow issues
