#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST, CURRENT_WF_ID used by sourced test_framework.sh
# Test: timeout_detection
#
# Verifies:
#   - Fast job completes successfully with return code 0
#   - Slow job is terminated by the job runner when runtime is exceeded
#   - Job runner log contains the "End time reached" termination message

run_test_timeout_detection() {
    local wf_id="$1"
    CURRENT_TEST="timeout_detection"
    CURRENT_WF_ID="$wf_id"
    echo ""
    echo "── Test: timeout_detection (workflow $wf_id) ──"

    # Fast job should complete
    assert_job_status "$wf_id" "job_fast" "completed"
    assert_return_code "$wf_id" "job_fast" "0"

    # Slow job should be terminated (job runner kills it when runtime is exceeded)
    assert_job_status "$wf_id" "job_slow" "terminated"

    # Job runner log should contain the termination message
    # Slurm job runner logs are written to torc_output/ as job_runner_slurm_wf<ID>_*.log
    # Filter by workflow ID to avoid false positives from other workflows in the same run.
    local runner_log_found=false
    for log_file in "$REPO_ROOT"/torc_output/job_runner_*wf"${wf_id}"_*.log; do
        if [ -f "$log_file" ] && grep -q "End time reached" "$log_file" 2>/dev/null; then
            runner_log_found=true
            break
        fi
    done
    if [ "$runner_log_found" = true ]; then
        _pass "job runner log contains 'End time reached' termination message"
    else
        _fail "job runner log does not contain 'End time reached' termination message"
    fi
}
