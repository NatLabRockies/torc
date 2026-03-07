#!/bin/bash
# handle_sigterm.sh — Long-running job that registers a SIGTERM handler.
# Used by the srun_termination_signal test to verify that srun --signal
# delivers SIGTERM before the step is killed.
#
# Usage: handle_sigterm.sh

CHECKPOINT_FILE="${1:-/tmp/torc_sigterm_checkpoint_$$}"

# Register SIGTERM handler (sets flag; loop checks it each iteration)
sigterm_received=false
handle_sigterm() {
    echo "SIGTERM_RECEIVED at $(date)"
    sigterm_received=true
}
trap handle_sigterm SIGTERM

echo "Job starting at $(date)"
echo "Running on $(hostname)"
echo "PID=$$"
echo "SIGTERM handler registered."

# Use background sleep + wait so that SIGTERM interrupts the wait immediately.
# A foreground `sleep 10` blocks signal delivery until it completes, but
# `wait` is interrupted by trapped signals, letting the loop check the flag
# right away.
for i in $(seq 1 60); do
    if [ "$sigterm_received" = true ]; then
        echo "Performing graceful shutdown..."
        echo "Saving checkpoint to $CHECKPOINT_FILE"
        echo "checkpoint_saved" > "$CHECKPOINT_FILE"
        echo "Graceful shutdown complete."
        exit 0
    fi
    echo "Heartbeat $i at $(date)"
    sleep 10 &
    wait $!
done

echo "Job completed normally (should not reach here in this test)."
