# Tutorial: Graceful Job Termination on HPC

This tutorial teaches you how to configure Torc workflows so that long-running jobs receive an early
warning signal before Slurm kills them, giving them time to save progress and automatically resume
from the last checkpoint.

## Learning Objectives

By the end of this tutorial, you will:

- Understand how `srun_termination_signal` delivers early SIGTERM to running jobs
- Write a Python job that catches SIGTERM and shuts down gracefully
- Use the shutdown-flag pattern to stop a long-running loop cleanly
- Use a non-zero exit code with a failure handler so Torc automatically retries the job
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
exit with a dedicated exit code that tells Torc to retry automatically.

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

# Exit code that signals "checkpointed, please retry".
# Must match the exit_codes in the workflow's failure_handler.
EXIT_CHECKPOINT = 42

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
            sys.exit(EXIT_CHECKPOINT)

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

1. **Non-zero exit code.** When SIGTERM arrives before the simulation finishes, the script exits
   with code 42 (`EXIT_CHECKPOINT`). This tells Torc the job did not complete successfully, so the
   failure handler can automatically schedule a retry. Exit code 0 is reserved for genuine
   completion.

2. **Global shutdown flag.** The signal handler only sets `shutdown_requested = True`. It does no
   I/O and no cleanup—signal handlers should be minimal.

3. **Loop checks the flag.** Every iteration starts with `if shutdown_requested:`. This guarantees
   the current iteration finishes before the job starts saving state.

4. **Atomic checkpoint.** Writing to a `.tmp` file and calling `os.replace()` prevents a corrupted
   checkpoint if the process is killed during the write.

5. **Handler registered early.** `signal.signal(signal.SIGTERM, handle_sigterm)` runs before the
   main loop so the handler is active from the start.

## Step 2: Create the Workflow Specification

Save as `graceful_termination.yaml`:

```yaml
name: graceful_termination_demo
description: Demonstrates early SIGTERM with automatic checkpoint-and-retry

slurm_config:
  srun_termination_signal: "TERM@120"

failure_handlers:
  - name: checkpoint_retry
    rules:
      # Exit code 42 means "checkpointed, please retry"
      - exit_codes: [42]
        max_retries: 100

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
    failure_handler: checkpoint_retry

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

Three pieces work together here:

- **`slurm_config.srun_termination_signal: "TERM@120"`** tells Slurm to send SIGTERM 120 seconds
  before the step time limit. Torc passes this to every `srun` invocation as
  `srun --signal=TERM@120`.

- **`failure_handlers`** with exit code 42 tells Torc to automatically retry the job whenever it
  exits with code 42. Each retry picks up from the last checkpoint, so the simulation makes progress
  across multiple Slurm allocations.

- **`max_retries`** controls how many checkpoint cycles the job is allowed. Set this high enough to
  cover the worst case: if each allocation provides ~2 hours of compute and the total job needs ~40
  hours, you need at least 20 retries. Setting it generously (e.g., 100) is safe — the job exits
  with code 0 once it finishes, so unused retries cost nothing.

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

The job exits with code 42, so Torc marks it as **failed** and the failure handler automatically
schedules a retry. The next attempt loads the checkpoint and continues:

```
Resumed from checkpoint at iteration 47001
Starting simulation from iteration 47001
Iteration 48000/100000  accumulator=1151.4530
...
```

This cycle repeats until the simulation finishes all iterations and exits with code 0, at which
point Torc marks the job as **completed**.

Note that each retry doesn't need to finish the entire remaining work — it only needs to make
**some** forward progress before the next checkpoint. This is the expected behavior for long-running
jobs whose total compute time exceeds a single Slurm allocation. The job spreads its work across as
many allocations as needed, and `max_retries` just needs to be high enough to cover the total number
of checkpoint cycles.

## How It Works Under the Hood

1. **`slurm_config.srun_termination_signal: "TERM@120"`** is stored on the workflow record in the
   Torc database.

2. When the job runner launches a job inside a Slurm allocation, it builds an `srun` command that
   includes `--signal=TERM@120`.

3. Slurm's step manager sends SIGTERM to the job's process group 120 seconds before `--time`
   expires.

4. Python's signal handler sets `shutdown_requested = True`.

5. The main loop sees the flag, saves the checkpoint, and calls `sys.exit(42)`.

6. Torc sees exit code 42 (non-zero), marks the job as failed, and consults the failure handler.

7. The failure handler matches exit code 42 and schedules a retry (up to `max_retries` times).

8. On retry, the script calls `load_checkpoint()` and resumes from where it left off.

9. When all iterations finish, the script exits with code 0 and Torc marks it as completed.

## What You Learned

In this tutorial, you learned:

- How to set `srun_termination_signal` in a workflow spec for early warning before timeout
- The shutdown-flag pattern: signal handler sets a flag, main loop checks it each iteration
- How to write atomic checkpoints that survive unexpected kills
- How to use a dedicated exit code with a failure handler for automatic checkpoint-and-retry

## Next Steps

- [Automatic Failure Recovery](./automatic-recovery.md) — Configure Torc to automatically retry or
  recover failed jobs
