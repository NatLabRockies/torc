#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST, CURRENT_WF_ID used by sourced test_framework.sh
# Test 5: oom_detection
#
# Verifies:
#   - Normal job completes successfully
#   - OOM job fails with non-zero return code
#   - torc slurm parse-logs detects OOM
#   - torc logs analyze detects OOM
#   - torc slurm sacct shows OUT_OF_MEMORY or FAILED
#   - torc workflows check-resources --include-failed flags violation

run_test_oom_detection() {
    local wf_id="$1"
    CURRENT_TEST="oom_detection"
    CURRENT_WF_ID="$wf_id"
    echo ""
    echo "── Test 5: oom_detection (workflow $wf_id) ──"

    # Normal job should complete
    assert_job_status "$wf_id" "normal_job" "completed"
    assert_return_code "$wf_id" "normal_job" "0"

    # OOM job should fail (status may be "failed" or "terminated")
    local oom_status
    oom_status=$(torc --url "$TORC_API_URL" -f json jobs list "$wf_id" 2>/dev/null \
        | jq -r '.jobs[] | select(.name == "oom_job") | .status')
    if [ "$oom_status" = "failed" ] || [ "$oom_status" = "terminated" ]; then
        _pass "oom_job has terminal failure status ($oom_status)"
    else
        _fail "oom_job expected failed/terminated, got '$oom_status'"
    fi

    # OOM job return code should be 137 (SIGKILL = 128+9)
    local oom_rc
    local oom_id
    oom_id=$(get_job_id "$wf_id" "oom_job")
    oom_rc=$(torc --url "$TORC_API_URL" -f json results list "$wf_id" --all-runs 2>/dev/null \
        | jq -r "[.results[] | select(.job_id == $oom_id)] | sort_by(.attempt_id) | last | .return_code")
    assert_ne "${oom_rc:-0}" "0" "oom_job has non-zero return code (got $oom_rc)"
    # srun may report exit code 1 for OOM kills instead of 137 (SIGKILL)
    if [ "${oom_rc:-0}" = "137" ] || [ "${oom_rc:-0}" = "1" ]; then
        _pass "oom_job return code indicates OOM ($oom_rc)"
    else
        _fail "oom_job return code expected 1 or 137, got '${oom_rc:-0}'"
    fi

    # parse-logs should detect OOM
    assert_parse_logs_detect_oom "$wf_id" "$REPO_ROOT/torc_output"

    # logs analyze should detect OOM
    assert_logs_analyze_detect_oom "$wf_id" "$REPO_ROOT/torc_output"

    # sacct should show OOM state
    assert_sacct_job_state "$wf_id" "OUT_OF_MEMORY"

    # check-resources should flag violations
    assert_resource_utilization_flags_violation "$wf_id"
}
