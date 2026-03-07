# Tutorial: Graceful Job Termination on HPC

This tutorial teaches you how to configure Torc workflows so that long-running jobs receive an early
warning signal before Slurm kills them, giving them time to save progress and exit cleanly.

## Learning Objectives

By the end of this tutorial, you will:

- Understand how `srun_termination_signal` delivers early SIGTERM to running jobs
- Write a Python job that catches SIGTERM and shuts down gracefully
- Use the shutdown-flag pattern to stop a long-running loop cleanly
- Configure a complete Torc workflow with early termination support

## Prerequisites

- Torc server running
- Access to a Slurm cluster
- Basic familiarity with submitting Torc workflows (see
  [Quick Start (HPC/Slurm)](../../getting-started/quick-start-hpc.md))

## Background: Why Graceful Termination Matters

On HPC systems, jobs have a fixed wall-time. When time runs out, Slurm kills the process immediately
with SIGKILL. Any unsaved work—training progress, partial results, intermediate state—is lost.

Torc's `srun_termination_signal` feature tells Slurm to send a catchable signal (SIGTERM) **before**
the hard kill. Your job can trap that signal, finish the current iteration, save a checkpoint, and
exit cleanly.

### Timeline of Events

```mermaid
graph LR
    A["Job starts"] -->|normal execution| B["SIGTERM"]
    B -->|"120 seconds"| C["Step timeout"]

    style A fill:#4a9eff,color:#fff
    style B fill:#e8a735,color:#fff
    style C fill:#d9534f,color:#fff
```

With `srun_termination_signal: "TERM@120"`, your job gets 120 seconds of warning before the srun
step's time limit expires.

## Step 1: Write the Python Job

Save this as `simulate.py`:

```python
#!/usr/bin/env python3
"""Long-running simulation that handles SIGTERM for graceful shutdown."""

import json
import os
import signal
import sys
import time

# --- Shutdown flag -----------------------------------------------------------
# The SIGTERM handler sets this flag. The main loop checks it on every
# iteration and breaks out when it becomes True.
shutdown_requested = False


def handle_sigterm(signum, frame):
    """Set the shutdown flag when SIGTERM is received."""
    global shutdown_requested
    print(f"SIGTERM received (signal {signum}). Will stop after current iteration.")
    shutdown_requested = True


# Register the handler BEFORE doing any work.
signal.signal(signal.SIGTERM, handle_sigterm)

# --- Checkpoint helpers ------------------------------------------------------
CHECKPOINT_PATH = os.environ.get("CHECKPOINT_PATH", "checkpoint.json")


def load_checkpoint():
    """Load the last saved iteration, or start from 0."""
    if os.path.exists(CHECKPOINT_PATH):
        with open(CHECKPOINT_PATH) as f:
            data = json.load(f)
        print(f"Resumed from checkpoint at iteration {data['iteration']}")
        return data["iteration"], data["accumulator"]
    return 0, 0.0


def save_checkpoint(iteration, accumulator):
    """Atomically save progress to disk."""
    tmp = CHECKPOINT_PATH + ".tmp"
    with open(tmp, "w") as f:
        json.dump({"iteration": iteration, "accumulator": accumulator}, f)
    os.replace(tmp, CHECKPOINT_PATH)  # atomic on POSIX
    print(f"Checkpoint saved at iteration {iteration}")


# --- Main loop ---------------------------------------------------------------
def main():
    iteration, accumulator = load_checkpoint()
    total_iterations = 100_000

    print(f"Starting simulation from iteration {iteration}")
    while iteration < total_iterations:
        # Check the shutdown flag at the top of every iteration.
        if shutdown_requested:
            print("Shutdown flag is set. Saving checkpoint and exiting.")
            save_checkpoint(iteration, accumulator)
            sys.exit(0)

        # Simulate one unit of work.
        accumulator += iteration * 0.001
        iteration += 1

        # Periodic progress and checkpoint.
        if iteration % 1000 == 0:
            print(f"Iteration {iteration}/{total_iterations}  accumulator={accumulator:.4f}")
            save_checkpoint(iteration, accumulator)

        time.sleep(0.01)  # simulate compute time

    print(f"Simulation complete. Final accumulator={accumulator:.4f}")
    save_checkpoint(iteration, accumulator)


if __name__ == "__main__":
    main()
```

### Key Design Points

1. **Global shutdown flag.** The signal handler only sets `shutdown_requested = True`. It does no
   I/O and no cleanup—signal handlers should be minimal.

2. **Loop checks the flag.** Every iteration starts with `if shutdown_requested:`. This guarantees
   the current iteration finishes before the job starts saving state.

3. **Atomic checkpoint.** Writing to a `.tmp` file and calling `os.replace()` prevents a corrupted
   checkpoint if the process is killed during the write.

4. **Handler registered early.** `signal.signal(signal.SIGTERM, handle_sigterm)` runs before the
   main loop so the handler is active from the start.

## Step 2: Create the Workflow Specification

Save as `graceful_termination.yaml`:

```yaml
name: graceful_termination_demo
description: Demonstrates early SIGTERM with srun_termination_signal
srun_termination_signal: "TERM@120"

resource_requirements:
  - name: sim_resources
    num_cpus: 2
    num_nodes: 1
    memory: 4g
    runtime: PT2H

jobs:
  - name: simulate
    command: python3 simulate.py
    resource_requirements: sim_resources

slurm_schedulers:
  - name: scheduler
    account: my_project
    partition: standard
    nodes: 1
    walltime: "02:00:00"

actions:
  - trigger_type: on_workflow_start
    action_type: schedule_nodes
    scheduler: scheduler
    scheduler_type: slurm
    num_allocations: 1
```

The `srun_termination_signal: "TERM@120"` is set at the **workflow level**. Torc passes it to every
`srun` invocation as `srun --signal=TERM@120`.

## Step 3: Submit and Run

```bash
torc submit-slurm --account my_project graceful_termination.yaml
```

Or, if you already have schedulers configured in the spec:

```bash
torc submit graceful_termination.yaml
```

## Step 4: Observe the Behavior

Monitor the workflow:

```bash
torc tui
```

When the srun step nears its time limit, you will see in the job's stdout:

```
Iteration 47000/100000  accumulator=1104.4530
SIGTERM received (signal 15). Will stop after current iteration.
Shutdown flag is set. Saving checkpoint and exiting.
Checkpoint saved at iteration 47001
```

The job exits with code 0, so Torc marks it as **completed** rather than terminated or failed.

## Step 5: Resume from Checkpoint

If the simulation didn't finish all iterations, re-submit the workflow. The job will load the
checkpoint and continue:

```bash
torc workflows reinitialize $WORKFLOW_ID
torc workflows submit $WORKFLOW_ID
```

The next run picks up where it left off:

```
Resumed from checkpoint at iteration 47001
Starting simulation from iteration 47001
Iteration 48000/100000  accumulator=1151.4530
...
```

## How It Works Under the Hood

1. **`srun_termination_signal: "TERM@120"`** is stored on the workflow record in the Torc database.

2. When the job runner launches a job inside a Slurm allocation, it builds an `srun` command that
   includes `--signal=TERM@120`.

3. Slurm's step manager sends SIGTERM to the job's process group 120 seconds before `--time`
   expires.

4. Python's signal handler sets `shutdown_requested = True`.

5. The main loop sees the flag, saves the checkpoint, and calls `sys.exit(0)`.

6. Because the exit code is 0, Torc treats this as a successful completion.

## What You Learned

In this tutorial, you learned:

- How to set `srun_termination_signal` in a workflow spec for early warning before timeout
- The shutdown-flag pattern: signal handler sets a flag, main loop checks it each iteration
- How to write atomic checkpoints that survive unexpected kills
- How to resume a job from a checkpoint after re-submission

## Next Steps

- [Automatic Failure Recovery](./automatic-recovery.md) — Configure Torc to automatically retry or
  recover failed jobs
