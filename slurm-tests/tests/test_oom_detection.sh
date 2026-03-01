#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST used by sourced test_framework.sh
# Test 5: oom_detection
#
# Verifies:
#   - Normal job completes successfully
#   - OOM job fails with non-zero return code
#   - torc slurm parse-logs detects OOM
#   - torc logs analyze detects OOM
#   - torc slurm sacct shows OUT_OF_MEMORY or FAILED
#   - torc reports check-resource-utilization --include-failed flags violation

run_test_oom_detection() {
    local wf_id="$1"
    CURRENT_TEST="oom_detection"
    echo ""
    echo "── Test 5: oom_detection (workflow $wf_id) ──"

    # Normal job should complete
    assert_job_status "$wf_id" "normal_job" "completed"
    assert_return_code "$wf_id" "normal_job" "0"

    # OOM job should fail (status may be "failed" or "terminated")
    local oom_status
    oom_status=$(torc --url "$TORC_API_URL" -f json jobs list "$wf_id" 2>/dev/null \
        | jq -r '.[] | select(.name == "oom_job") | .status')
    if [ "$oom_status" = "failed" ] || [ "$oom_status" = "terminated" ]; then
        _pass "oom_job has terminal failure status ($oom_status)"
    else
        _fail "oom_job expected failed/terminated, got '$oom_status'"
    fi

    # OOM job return code should be non-zero
    local oom_rc
    local oom_id
    oom_id=$(get_job_id "$wf_id" "oom_job")
    oom_rc=$(torc --url "$TORC_API_URL" -f json reports results "$wf_id" 2>/dev/null \
        | jq -r ".[] | select(.job_id == $oom_id) | .return_code" | tail -1)
    assert_ne "${oom_rc:-0}" "0" "oom_job has non-zero return code (got $oom_rc)"

    # parse-logs should detect OOM
    assert_parse_logs_detect_oom "$wf_id" "$RUN_DIR"

    # logs analyze should detect OOM
    assert_logs_analyze_detect_oom "$wf_id" "$RUN_DIR"

    # sacct should show OOM or FAILED state
    local sacct_output
    sacct_output=$(torc --url "$TORC_API_URL" slurm sacct "$wf_id" 2>&1) || true
    if echo "$sacct_output" | grep -qiE "OUT_OF_MEMORY|FAILED|OOM"; then
        _pass "sacct shows OOM/FAILED state"
    else
        _fail "sacct does not show OOM/FAILED (got: $(echo "$sacct_output" | head -5))"
    fi

    # check-resource-utilization should flag violations
    assert_resource_utilization_flags_violation "$wf_id"
}
