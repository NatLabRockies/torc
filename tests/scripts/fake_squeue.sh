#!/bin/bash

# Fake squeue command for testing
# Simulates Slurm's squeue command

JOBS_FILE="${TMPDIR:-/tmp}/fake_slurm_jobs.txt"

# Create empty jobs file if it doesn't exist
if [ ! -f "$JOBS_FILE" ]; then
    touch "$JOBS_FILE"
fi

# Check for failure simulation
if [ -n "$TORC_FAKE_SQUEUE_FAIL" ]; then
    echo "squeue: error: Connection refused" >&2
    exit 1
fi

# Parse arguments
USER=""
FORMAT=""
HEADER=true
JOB_ID=""

while [[ $# -gt 0 ]]; do
    case $1 in
        -u)
            USER="$2"
            shift 2
            ;;
        --Format)
            FORMAT="$2"
            shift 2
            ;;
        -h)
            HEADER=false
            shift
            ;;
        -j)
            JOB_ID="$2"
            shift 2
            ;;
        --format=*)
            # Handle --format='%20e' or similar
            FORMAT="${1#--format=}"
            FORMAT="${FORMAT//\'/}"  # Remove quotes
            shift
            ;;
        *)
            shift
            ;;
    esac
done

# Handle different format strings
if [[ "$FORMAT" == *"%"* ]]; then
    # Special format like '%20e' for end time or '%5D %500N' for nodes
    if [[ "$FORMAT" == *"%e"* ]]; then
        # End time format
        if [ -z "$HEADER" ]; then
            echo "END_TIME"
        fi
        # Return a future end time
        FUTURE_TIME=$(date -u -v+10d +"%Y-%m-%dT%H:%M:%S" 2>/dev/null || date -u -d "+10 days" +"%Y-%m-%dT%H:%M:%S")
        echo "$FUTURE_TIME"
    elif [[ "$FORMAT" == *"%D"* && "$FORMAT" == *"%N"* ]]; then
        # Node list format
        if [ -n "$TORC_FAKE_SQUEUE_NODES" ]; then
            echo "$TORC_FAKE_SQUEUE_NODES"
        else
            echo "1 node001"
        fi
    fi
    exit 0
fi

# Standard format handling
IFS=',' read -ra FIELDS <<< "$FORMAT"

# Print header if requested
if [ "$HEADER" = true ]; then
    for i in "${!FIELDS[@]}"; do
        if [ "$i" -gt 0 ]; then
            echo -n " "
        fi
        echo -n "${FIELDS[$i]^^}"
    done
    echo
fi

# Filter and print jobs
# shellcheck disable=SC2094  # grep only reads JOBS_FILE, no write conflict
while IFS='|' read -r job_id name state _start _end _account _partition _qos; do
    # Skip empty lines
    [ -z "$job_id" ] && continue

    # Filter by user if specified
    if [ -n "$USER" ]; then
        # In this fake implementation, we accept all users
        :
    fi

    # Filter by job ID if specified
    if [ -n "$JOB_ID" ] && [ "$job_id" != "$JOB_ID" ]; then
        continue
    fi

    # Check for "Invalid job id" case
    if [ -n "$JOB_ID" ] && [ "$job_id" != "$JOB_ID" ]; then
        if ! grep -q "^${JOB_ID}|" "$JOBS_FILE" 2>/dev/null; then
            echo "squeue: error: Invalid job id specified" >&2
            exit 1
        fi
    fi

    # Allow environment variable to override job state
    if [ -n "$TORC_FAKE_SQUEUE_STATE" ]; then
        state="$TORC_FAKE_SQUEUE_STATE"
    fi

    # Print requested fields
    for i in "${!FIELDS[@]}"; do
        if [ "$i" -gt 0 ]; then
            echo -n " "
        fi
        field="${FIELDS[$i]}"
        case "$field" in
            jobid)
                echo -n "$job_id"
                ;;
            name)
                # Pad name to 20 characters to match typical squeue output
                printf "%-20s" "$name"
                ;;
            state)
                echo -n "$state"
                ;;
            *)
                echo -n "N/A"
                ;;
        esac
    done
    echo
done < "$JOBS_FILE"

exit 0
