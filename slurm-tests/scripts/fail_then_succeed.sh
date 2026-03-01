#!/bin/bash
# Demonstrates transient failure and recovery behavior.
#
# On the first attempt (ATTEMPT=1), fails with exit code 42.
# On retry attempts (ATTEMPT=2+), succeeds.
#
# Exit code 42 is configured in the failure handler to trigger automatic retry.

# Get the attempt number from environment or default to 1
ATTEMPT=${TORC_ATTEMPT_ID:-1}

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
