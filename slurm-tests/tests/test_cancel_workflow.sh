#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST, CURRENT_WF_ID used by sourced test_framework.sh
# Test: cancel_workflow
#
# Verifies:
#   - Cancel command returns success status
#   - At least one Slurm job was canceled
#   - Workflow is marked as canceled
#   - No jobs remain in "running" status

run_test_cancel_workflow() {
    local wf_id="$1"
    CURRENT_TEST="cancel_workflow"
    CURRENT_WF_ID="$wf_id"
    echo ""
    echo "── Test: cancel_workflow (workflow $wf_id) ──"

    # Read the cancel output captured during pre-poll actions
    local cancel_output_file="$RUN_DIR/cancel_workflow_output.json"
    if [ ! -f "$cancel_output_file" ]; then
        _fail "cancel output file not found: $cancel_output_file"
        return
    fi

    local cancel_output
    cancel_output=$(cat "$cancel_output_file")

    # Cancel status should be "success" or "partial_success"
    local cancel_status
    cancel_status=$(echo "$cancel_output" | jq -r '.status // "unknown"')
    if [ "$cancel_status" = "success" ] || [ "$cancel_status" = "partial_success" ]; then
        _pass "cancel status is '$cancel_status'"
    else
        _fail "cancel status expected success/partial_success, got '$cancel_status'"
    fi

    # At least one Slurm job should have been canceled
    local canceled_count
    canceled_count=$(echo "$cancel_output" | jq '.canceled_slurm_jobs | length')
    assert_gt "$canceled_count" "0" "at least 1 Slurm job canceled (got $canceled_count)"

    # Workflow should be marked as canceled
    assert_workflow_canceled "$wf_id"

    # No jobs should remain in "running" status
    local running_count
    running_count=$(torc --url "$TORC_API_URL" -f json jobs list "$wf_id" 2>/dev/null \
        | jq '[.jobs[] | select(.status == "running")] | length')
    assert_eq "$running_count" "0" "no jobs remain in running status"
}
