#!/bin/bash
# slow_work.sh — Sleeps for a configurable duration (in minutes).
# Used by the timeout_detection test to trigger walltime exceeded.
#
# Usage: slow_work.sh [MINUTES]
#   MINUTES: Duration to sleep (default: 10)

MINUTES="${1:-10}"
SECONDS_TOTAL=$((MINUTES * 60))

echo "Slow job starting at $(date)"
echo "Will sleep for $MINUTES minutes ($SECONDS_TOTAL seconds)"
echo "Running on $(hostname)"

for i in $(seq 1 "$MINUTES"); do
  echo "Minute $i of $MINUTES..."
  sleep 60
done

echo "Slow job completed at $(date)"
