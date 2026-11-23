#!/bin/bash

# Fake scancel command for testing
# Simulates Slurm's scancel command

JOBS_FILE="${TORC_FAKE_SLURM_STATE_DIR:-${TMPDIR:-/tmp}}/fake_slurm_jobs.txt"

# Check for failure simulation
if [ -n "$TORC_FAKE_SCANCEL_FAIL" ]; then
    echo "scancel: error: Kill job error on job id $1" >&2
    exit 1
fi

# Get job ID from argument
JOB_ID="$1"

if [ -z "$JOB_ID" ]; then
    echo "scancel: error: No job identification provided" >&2
    exit 1
fi

# Check if job exists
if [ ! -f "$JOBS_FILE" ]; then
    echo "scancel: error: Invalid job id specified" >&2
    exit 1
fi

if ! grep -q "^${JOB_ID}|" "$JOBS_FILE" 2>/dev/null; then
    echo "scancel: error: Invalid job id specified" >&2
    exit 1
fi

# Update job state to CANCELLED in the jobs file
TEMP_FILE="${JOBS_FILE}.tmp"
while IFS='|' read -r job_id name state start end account partition qos; do
    if [ "$job_id" = "$JOB_ID" ]; then
        echo "${job_id}|${name}|CANCELLED|${start}|${end}|${account}|${partition}|${qos}"
    else
        echo "${job_id}|${name}|${state}|${start}|${end}|${account}|${partition}|${qos}"
    fi
done < "$JOBS_FILE" > "$TEMP_FILE"

mv "$TEMP_FILE" "$JOBS_FILE"

exit 0
