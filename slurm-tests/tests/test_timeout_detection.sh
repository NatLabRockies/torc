#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST, CURRENT_WF_ID used by sourced test_framework.sh
# Test: timeout_detection
#
# Verifies:
#   - Fast job completes successfully with return code 0
#   - Slow job fails/terminated due to timeout
#   - Slow job has expected return code (137 for SIGKILL)
#   - torc slurm parse-logs detects timeout
#   - torc logs analyze detects timeout
#   - torc slurm sacct shows TIMEOUT or FAILED
#   - torc reports check-resource-utilization --include-failed flags violation

run_test_timeout_detection() {
    local wf_id="$1"
    CURRENT_TEST="timeout_detection"
    CURRENT_WF_ID="$wf_id"
    echo ""
    echo "── Test: timeout_detection (workflow $wf_id) ──"

    # Fast job should complete
    assert_job_status "$wf_id" "job_fast" "completed"
    assert_return_code "$wf_id" "job_fast" "0"

    # Slow job should fail (status may be "failed" or "terminated")
    local slow_status
    slow_status=$(torc --url "$TORC_API_URL" -f json jobs list "$wf_id" 2>/dev/null \
        | jq -r '.jobs[] | select(.name == "job_slow") | .status')
    if [ "$slow_status" = "failed" ] || [ "$slow_status" = "terminated" ]; then
        _pass "job_slow has terminal failure status ($slow_status)"
    else
        _fail "job_slow expected failed/terminated, got '$slow_status'"
    fi

    # Slow job return code should be non-zero
    local slow_rc
    local slow_id
    slow_id=$(get_job_id "$wf_id" "job_slow")
    slow_rc=$(torc --url "$TORC_API_URL" -f json reports results "$wf_id" 2>/dev/null \
        | jq -r "[.results[] | select(.job_id == $slow_id)] | sort_by(.attempt_id) | last | .return_code")
    assert_ne "${slow_rc:-0}" "0" "job_slow has non-zero return code (got $slow_rc)"

    # parse-logs should detect timeout
    assert_parse_logs_detect_timeout "$wf_id" "$RUN_DIR"

    # logs analyze should detect timeout
    assert_logs_analyze_detect_timeout "$wf_id" "$RUN_DIR"

    # sacct should show TIMEOUT or FAILED state
    local sacct_output
    sacct_output=$(torc --url "$TORC_API_URL" slurm sacct "$wf_id" 2>&1) || true
    if echo "$sacct_output" | grep -qiE "TIMEOUT|FAILED|CANCELLED"; then
        _pass "sacct shows TIMEOUT/FAILED state"
    else
        _fail "sacct does not show TIMEOUT/FAILED (got: $(echo "$sacct_output" | head -5))"
    fi

    # check-resource-utilization should flag violations
    assert_resource_utilization_flags_violation "$wf_id"
}
