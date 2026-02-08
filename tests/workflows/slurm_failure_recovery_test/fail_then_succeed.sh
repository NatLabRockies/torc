#!/bin/bash
#
# This script demonstrates transient failure and recovery behavior.
#
# On the first attempt (ATTEMPT=1), it fails with exit code 42.
# On retry attempts (ATTEMPT=2+), it succeeds.
#
# Exit code 42 is configured in the failure handler to trigger automatic retry.
# The job runner will:
# 1. Detect exit code 42
# 2. Find the matching failure handler rule
# 3. Retry the job automatically (up to 2 times)
# 4. On the second run, the job succeeds

# Get the attempt number from environment or default to 1
ATTEMPT=${TORC_ATTEMPT_ID:-1}
WORK_DIR="${TORC_OUTPUT_DIR:-.}"

echo "Work job 3: Starting (attempt $ATTEMPT)"

# Check if this is the first attempt
if [ "$ATTEMPT" -eq "1" ]; then
    echo "Work job 3: Simulating transient failure (exit code 42)"
    echo "Work job 3: This will be caught by the failure handler and retried automatically"
    exit 42
else
    # On retry, succeed
    echo "Work job 3: Retry attempt $ATTEMPT - succeeding now"
    sleep 1
    echo "Work job 3: Completed successfully (exit code 0)"
    exit 0
fi
