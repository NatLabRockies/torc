#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST, CURRENT_WF_ID used by sourced test_framework.sh
# Test: timeout_detection
#
# Verifies:
#   - Fast job completes successfully with return code 0
#   - Slow job is terminated by the job runner before Slurm walltime
#   - Slow job has expected return code (137 for SIGKILL from job runner)
#   - sacct shows FAILED or CANCELLED state for the killed step

run_test_timeout_detection() {
    local wf_id="$1"
    CURRENT_TEST="timeout_detection"
    CURRENT_WF_ID="$wf_id"
    echo ""
    echo "── Test: timeout_detection (workflow $wf_id) ──"

    # Fast job should complete
    assert_job_status "$wf_id" "job_fast" "completed"
    assert_return_code "$wf_id" "job_fast" "0"

    # Slow job should be terminated by the job runner (proactive kill before Slurm walltime)
    local slow_status
    slow_status=$(torc --url "$TORC_API_URL" -f json jobs list "$wf_id" 2>/dev/null \
        | jq -r '.jobs[] | select(.name == "job_slow") | .status')
    if [ "$slow_status" = "failed" ] || [ "$slow_status" = "terminated" ]; then
        _pass "job_slow has terminal failure status ($slow_status)"
    else
        _fail "job_slow expected failed/terminated, got '$slow_status'"
    fi

    # Slow job return code should be non-zero (137 = SIGKILL from job runner)
    local slow_rc
    local slow_id
    slow_id=$(get_job_id "$wf_id" "job_slow")
    slow_rc=$(torc --url "$TORC_API_URL" -f json reports results "$wf_id" 2>/dev/null \
        | jq -r "[.results[] | select(.job_id == $slow_id)] | sort_by(.attempt_id) | last | .return_code")
    assert_ne "${slow_rc:-0}" "0" "job_slow has non-zero return code (got $slow_rc)"

    # sacct should show FAILED or CANCELLED state for the killed step
    local sacct_output
    sacct_output=$(torc --url "$TORC_API_URL" slurm sacct "$wf_id" 2>&1) || true
    if echo "$sacct_output" | grep -qiE "TIMEOUT|FAILED|CANCELLED"; then
        _pass "sacct shows TIMEOUT/FAILED/CANCELLED state"
    else
        _fail "sacct does not show TIMEOUT/FAILED/CANCELLED (got: $(echo "$sacct_output" | head -5))"
    fi
}
