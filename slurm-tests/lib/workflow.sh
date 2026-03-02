#!/bin/bash
# workflow.sh — Helpers for submitting and monitoring torc workflows.
#
# Requires: TORC_API_URL to be set before calling these functions.

# Global array of Slurm job IDs created during this test run.
# Used by cancel_slurm_jobs to clean up on exit.
SLURM_JOB_IDS=()

# submit_workflow SPEC_FILE
#   Submits a workflow from a spec file and prints the workflow ID.
#   Slurm job IDs from the submission are tracked in SLURM_JOB_IDS.
submit_workflow() {
    local spec_file="$1"
    local stderr_file
    stderr_file=$(mktemp)
    local output
    output=$(torc --url "$TORC_API_URL" -f json submit "$spec_file" 2>"$stderr_file") || {
        echo "ERROR: Failed to submit workflow from $spec_file" >&2
        echo "Output: $output" >&2
        echo "Stderr: $(cat "$stderr_file")" >&2
        rm -f "$stderr_file"
        return 1
    }
    # Capture any Slurm job IDs from stderr (format: "with ID=12345")
    while IFS= read -r slurm_id; do
        SLURM_JOB_IDS+=("$slurm_id")
    done < <(grep -oP 'with ID=\K\d+' "$stderr_file" || true)
    rm -f "$stderr_file"
    local wf_id
    # Try JSON format: {"workflow_id": 123}
    wf_id=$(echo "$output" | grep -oP '"workflow_id"\s*:\s*\K\d+' | head -1)
    if [ -z "$wf_id" ]; then
        # Try plain text: "Created workflow 123"
        wf_id=$(echo "$output" | grep -oP 'Created workflow \K\d+' | head -1)
    fi
    if [ -z "$wf_id" ]; then
        echo "ERROR: Could not parse workflow ID from submit output" >&2
        echo "Output: $output" >&2
        return 1
    fi
    echo "$wf_id"
}

# cancel_slurm_jobs
#   Cancels all Slurm jobs tracked in SLURM_JOB_IDS.
cancel_slurm_jobs() {
    if [ ${#SLURM_JOB_IDS[@]} -eq 0 ]; then
        return 0
    fi
    echo "Canceling ${#SLURM_JOB_IDS[@]} Slurm job(s): ${SLURM_JOB_IDS[*]}"
    scancel "${SLURM_JOB_IDS[@]}" 2>/dev/null || true
}

# is_workflow_terminal WF_ID
#   Returns 0 if the workflow is in a terminal state (complete or canceled), 1 otherwise.
is_workflow_terminal() {
    local wf_id="$1"
    local result
    result=$(torc --url "$TORC_API_URL" -f json workflows is-complete "$wf_id" 2>/dev/null) || return 1
    local is_complete is_canceled
    is_complete=$(echo "$result" | jq -r '.is_complete // false')
    is_canceled=$(echo "$result" | jq -r '.is_canceled // false')
    [ "$is_complete" = "true" ] || [ "$is_canceled" = "true" ]
}

# workflow_job_summary WF_ID
#   Prints a compact summary of job counts by status (e.g., "completed=3 running=2 ready=1").
workflow_job_summary() {
    local wf_id="$1"
    torc --url "$TORC_API_URL" -f json reports summary "$wf_id" 2>/dev/null \
        | jq -r '.jobs_by_status | to_entries | map(select(.value > 0)) | map("\(.key)=\(.value)") | join(" ")' \
        2>/dev/null || echo "unknown"
}

# poll_workflow WF_ID TIMEOUT_SECONDS [POLL_INTERVAL]
#   Polls until the workflow reaches a terminal state or times out.
#   Returns 0 if complete, 1 if timed out.
poll_workflow() {
    local wf_id="$1"
    local timeout="$2"
    local interval="${3:-10}"
    local elapsed=0

    echo "Polling workflow $wf_id (timeout: ${timeout}s, interval: ${interval}s)..."
    while [ "$elapsed" -lt "$timeout" ]; do
        if is_workflow_terminal "$wf_id"; then
            echo "Workflow $wf_id reached terminal state after ${elapsed}s."
            return 0
        fi
        sleep "$interval"
        elapsed=$((elapsed + interval))
        # Print progress every 60 seconds
        if [ $((elapsed % 60)) -eq 0 ]; then
            echo "  [${elapsed}s] workflow $wf_id: $(workflow_job_summary "$wf_id")"
        fi
    done

    echo "WARNING: Workflow $wf_id timed out after ${timeout}s."
    echo "  $(workflow_job_summary "$wf_id")"
    return 1
}

# poll_all_workflows TIMEOUT_SECONDS WF_IDS...
#   Polls until all listed workflows reach terminal state or timeout.
#   Returns 0 if all complete, 1 if any timed out.
poll_all_workflows() {
    local timeout="$1"
    shift
    local wf_ids=("$@")
    local interval=10
    local elapsed=0
    local all_done

    echo "Polling ${#wf_ids[@]} workflows (timeout: ${timeout}s)..."
    while [ "$elapsed" -lt "$timeout" ]; do
        all_done=true
        for wf_id in "${wf_ids[@]}"; do
            if ! is_workflow_terminal "$wf_id"; then
                all_done=false
                break
            fi
        done

        if $all_done; then
            echo "All workflows reached terminal state after ${elapsed}s."
            return 0
        fi

        sleep "$interval"
        elapsed=$((elapsed + interval))

        # Print progress every 60 seconds
        if [ $((elapsed % 60)) -eq 0 ]; then
            echo "  [${elapsed}s] Still waiting..."
            for wf_id in "${wf_ids[@]}"; do
                echo "    workflow $wf_id: $(workflow_job_summary "$wf_id")"
            done
        fi
    done

    echo "WARNING: Not all workflows completed within ${timeout}s."
    for wf_id in "${wf_ids[@]}"; do
        if ! is_workflow_terminal "$wf_id"; then
            echo "  workflow $wf_id still running: $(workflow_job_summary "$wf_id")"
        fi
    done
    return 1
}

# get_job_id WF_ID JOB_NAME
#   Returns the numeric job ID for a named job.
get_job_id() {
    local wf_id="$1" job_name="$2"
    torc --url "$TORC_API_URL" -f json jobs list "$wf_id" 2>/dev/null \
        | jq -r ".jobs[] | select(.name == \"$job_name\") | .id"
}

# get_job_stdout WF_ID JOB_ID
#   Returns the stdout of a job.
get_job_stdout() {
    local wf_id="$1" job_id="$2"
    torc --url "$TORC_API_URL" logs stdout "$wf_id" "$job_id" 2>/dev/null || true
}

# get_job_stderr WF_ID JOB_ID
#   Returns the stderr of a job.
get_job_stderr() {
    local wf_id="$1" job_id="$2"
    torc --url "$TORC_API_URL" logs stderr "$wf_id" "$job_id" 2>/dev/null || true
}

# prepare_workflow_spec TEMPLATE ACCOUNT PARTITION OUTPUT_FILE
#   Substitutes placeholders in a workflow template and writes to output.
prepare_workflow_spec() {
    local template="$1"
    local account="$2"
    local partition="$3"
    local output="$4"

    sed -e "s|PLACEHOLDER_ACCOUNT|$account|g" \
        -e "s|PLACEHOLDER_PARTITION|$partition|g" \
        "$template" > "$output"
}
