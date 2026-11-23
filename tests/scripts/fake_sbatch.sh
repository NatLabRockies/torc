#!/bin/bash

# Fake sbatch command for testing
# Simulates Slurm's sbatch command

# Read job counter from temp file or initialize to 1000
JOB_COUNTER_FILE="${TORC_FAKE_SLURM_STATE_DIR:-${TMPDIR:-/tmp}}/fake_sbatch_counter.txt"
if [ -f "$JOB_COUNTER_FILE" ]; then
    JOB_ID=$(cat "$JOB_COUNTER_FILE")
else
    JOB_ID=1000
fi

# Increment for next time
echo $((JOB_ID + 1)) > "$JOB_COUNTER_FILE"

# Check for failure simulation
if [ -n "$TORC_FAKE_SBATCH_FAIL" ]; then
    echo "sbatch: error: Batch job submission failed: Invalid account" >&2
    exit 1
fi

# Output the standard sbatch success message
echo "Submitted batch job $JOB_ID"

# Store job info for squeue/sacct to use
JOBS_FILE="${TORC_FAKE_SLURM_STATE_DIR:-${TMPDIR:-/tmp}}/fake_slurm_jobs.txt"
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%S")
# Format: job_id|name|state|start_time|end_time|account|partition|qos
echo "${JOB_ID}|test_job|PENDING|${TIMESTAMP}|Unknown|test_account|debug|normal" >> "$JOBS_FILE"

exit 0
