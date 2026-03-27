#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST, CURRENT_WF_ID used by sourced test_framework.sh
# Test: watch_recover_oom
#
# Verifies:
#   - `torc watch --recover` observes the initial OOM failure
#   - The failed job is retried automatically
#   - The workflow finishes successfully after recovery

run_test_watch_recover_oom() {
    local wf_id="$1"
    CURRENT_TEST="watch_recover_oom"
    CURRENT_WF_ID="$wf_id"
    echo ""
    echo "── Test: watch_recover_oom (workflow $wf_id) ──"

    assert_workflow_complete "$wf_id"
    assert_all_jobs_completed "$wf_id" 2
    assert_return_code "$wf_id" "normal_job" "0"
    assert_return_code "$wf_id" "recoverable_oom_job" "0"

    local oom_job_id
    local attempts
    oom_job_id=$(get_job_id "$wf_id" "recoverable_oom_job")
    attempts=$(torc --url "$TORC_API_URL" -f json results list "$wf_id" --all-runs 2>/dev/null \
        | jq -r "[.results[] | select(.job_id == $oom_job_id)] | length")
    assert_ge "${attempts:-0}" "2" "recoverable_oom_job was retried after OOM"

    local watch_log
    watch_log=$(cat "$RUN_DIR/test_logs/${TEST_NAME}.log" 2>/dev/null || true)
    assert_contains "$watch_log" "Attempting automatic recovery" "watch entered recovery mode"
    assert_contains "$watch_log" "Applied fixes: 1 OOM, 0 timeout" "watch applied OOM recovery heuristic"
    assert_contains "$watch_log" "Recovery initiated. Resuming monitoring" "watch resumed after recovery"
}
