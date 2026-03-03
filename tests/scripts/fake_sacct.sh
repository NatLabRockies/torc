#!/bin/bash

# Fake sacct command for testing
# Simulates Slurm's sacct command

JOBS_FILE="${TMPDIR:-/tmp}/fake_slurm_jobs.txt"

# Check for failure simulation
if [ -n "$TORC_FAKE_SACCT_FAIL" ]; then
    echo "sacct: error: Problem talking to the database" >&2
    exit 1
fi

# Parse arguments
JOB_ID=""
_FORMAT="JobID,JobName%20,state,start,end,Account,Partition%15,QOS"

while [[ $# -gt 0 ]]; do
    case $1 in
        -j)
            JOB_ID="$2"
            shift 2
            ;;
        --format=*)
            _FORMAT="${1#--format=}"
            shift
            ;;
        *)
            shift
            ;;
    esac
done

# The sacct output has a specific format:
# Line 1: Header with column names
# Line 2: Separator line with dashes
# Line 3: Job data (main job entry)
# Line 4: Job step .batch
# Line 5: Job step .extern
# Line 6: Empty line

# Find the job in our jobs file
JOB_DATA=""
if [ -n "$JOB_ID" ]; then
    JOB_DATA=$(grep "^${JOB_ID}|" "$JOBS_FILE" 2>/dev/null | head -1)
fi

if [ -z "$JOB_DATA" ]; then
    # Return empty result with headers
    echo "JobID                 JobName              State      Start                 End           Account    Partition       QOS     "
    echo "-------------------- -------------------- ---------- --------------------- ------------- ---------- --------------- --------"
    exit 0
fi

# Parse job data
IFS='|' read -r job_id name state start end account partition qos <<< "$JOB_DATA"

# Allow environment variable to override job state
if [ -n "$TORC_FAKE_SACCT_STATE" ]; then
    state="$TORC_FAKE_SACCT_STATE"
fi

# Print header
echo "JobID                 JobName              State      Start                 End           Account    Partition       QOS     "
echo "-------------------- -------------------- ---------- --------------------- ------------- ---------- --------------- --------"

# Print job data (main job entry)
printf "%-20s %-20s %-10s %-21s %-13s %-10s %-15s %-8s\n" \
    "$job_id" "$name" "$state" "$start" "$end" "$account" "$partition" "$qos"

# Print batch step
printf "%-20s %-20s %-10s %-21s %-13s %-10s %-15s %-8s\n" \
    "${job_id}.batch" "batch" "$state" "$start" "$end" "" "" ""

# Print extern step
printf "%-20s %-20s %-10s %-21s %-13s %-10s %-15s %-8s\n" \
    "${job_id}.extern" "extern" "$state" "$start" "$end" "" "" ""

# Empty line
echo

exit 0
