#!/bin/bash
# Recovery hook script for recovery_hook_test workflow
#
# This script is called by `torc watch --recovery-hook` when jobs fail
# with unknown causes (not OOM or timeout).
#
# It receives the workflow ID as:
# - Argument: $1
# - Environment variable: $TORC_WORKFLOW_ID
#
# This script creates the missing file that work_3 requires.

set -e

WORKFLOW_ID="${1:-$TORC_WORKFLOW_ID}"

if [ -z "$WORKFLOW_ID" ]; then
  echo "ERROR: No workflow ID provided"
  exit 1
fi

echo "Recovery hook running for workflow $WORKFLOW_ID"

# Find failed jobs to understand what went wrong
echo "Checking failed jobs..."
FAILED_JOBS=$(torc jobs list "$WORKFLOW_ID" --status failed -f json 2>/dev/null || echo "[]")
echo "Failed jobs: $FAILED_JOBS"

# Create the output directory if it doesn't exist
# The output directory is typically "torc_output" relative to where torc watch is run
OUTPUT_DIR="${TORC_OUTPUT_DIR:-output}"
mkdir -p "$OUTPUT_DIR"

# Create the required file that work_3 needs
REQUIRED_FILE="$OUTPUT_DIR/required_input.txt"
echo "Creating required file: $REQUIRED_FILE"
echo "This file was created by the recovery hook at $(date)" >"$REQUIRED_FILE"

echo "Recovery hook completed successfully"
echo "The missing file has been created. work_3 should succeed on retry."
