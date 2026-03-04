#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST, CURRENT_WF_ID used by sourced test_framework.sh
# Test: sync_status
#
# Verifies:
#   - sync-status detects orphaned jobs (slurm_jobs_failed > 0)
#   - At least one job has status "failed"
#   - At least one job has return code -128 (ORPHANED_JOB_RETURN_CODE)
#   - Workflow is canceled (we cancel after sync-status to ensure terminal state)

run_test_sync_status() {
    local wf_id="$1"
    CURRENT_TEST="sync_status"
    CURRENT_WF_ID="$wf_id"
    echo ""
    echo "── Test: sync_status (workflow $wf_id) ──"

    # Read the sync-status output captured during pre-poll actions
    local sync_output_file="$RUN_DIR/sync_status_output.json"
    if [ ! -f "$sync_output_file" ]; then
        _fail "sync-status output file not found: $sync_output_file"
        return
    fi

    local sync_output
    sync_output=$(cat "$sync_output_file")

    # If jobs never reached running, this test cannot produce meaningful results
    local skipped
    skipped=$(echo "$sync_output" | jq -r '.skipped // false')
    if [ "$skipped" = "true" ]; then
        local reason
        reason=$(echo "$sync_output" | jq -r '.reason // "unknown"')
        _skip "sync_status test skipped: $reason"
        return
    fi

    # sync-status should detect orphaned jobs from terminated Slurm allocations
    local slurm_jobs_failed
    slurm_jobs_failed=$(echo "$sync_output" | jq '.slurm_jobs_failed // 0')
    assert_gt "$slurm_jobs_failed" "0" \
        "sync-status detected orphaned Slurm jobs (slurm_jobs_failed=$slurm_jobs_failed)"

    # At least one job should now be in "failed" status
    local failed_count
    failed_count=$(torc --url "$TORC_API_URL" -f json jobs list "$wf_id" 2>/dev/null \
        | jq '[.jobs[] | select(.status == "failed")] | length')
    assert_gt "$failed_count" "0" "at least 1 job has status 'failed' (got $failed_count)"

    # At least one job should have return code -128 (ORPHANED_JOB_RETURN_CODE)
    local orphan_rc_count
    orphan_rc_count=$(torc --url "$TORC_API_URL" -f json reports results "$wf_id" 2>/dev/null \
        | jq '[.results[] | select(.return_code == -128)] | length')
    assert_gt "$orphan_rc_count" "0" \
        "at least 1 job has return code -128 (got $orphan_rc_count)"

    # Workflow should be canceled (we cancel after sync-status to make it terminal)
    assert_workflow_canceled "$wf_id"
}
