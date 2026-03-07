#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST, CURRENT_WF_ID used by sourced test_framework.sh
# Test: timeout_detection
#
# Verifies:
#   - Fast job completes successfully with return code 0
#   - Slow job is terminated by srun --time when it exceeds the allocation walltime
#   - Slow job has return code 152 (sacct State=TIMEOUT)

run_test_timeout_detection() {
    local wf_id="$1"
    CURRENT_TEST="timeout_detection"
    CURRENT_WF_ID="$wf_id"
    echo ""
    echo "── Test: timeout_detection (workflow $wf_id) ──"

    # Fast job should complete
    assert_job_status "$wf_id" "job_fast" "completed"
    assert_return_code "$wf_id" "job_fast" "0"

    # Slow job should be terminated by srun --time (sacct State=TIMEOUT → terminated)
    assert_job_status "$wf_id" "job_slow" "terminated"
    assert_return_code "$wf_id" "job_slow" "152"
}
