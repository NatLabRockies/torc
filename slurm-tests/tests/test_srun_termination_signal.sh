#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST, CURRENT_WF_ID used by sourced test_framework.sh
# Test: srun_termination_signal
#
# Verifies:
#   - Job received SIGTERM before being killed (handler wrote "SIGTERM_RECEIVED")
#   - Job exited via the signal handler (return code 0 from graceful exit)
#   - Workflow completed (the only job exited cleanly via handler)

run_test_srun_termination_signal() {
    local wf_id="$1"
    CURRENT_TEST="srun_termination_signal"
    CURRENT_WF_ID="$wf_id"
    echo ""
    echo "── Test: srun_termination_signal (workflow $wf_id) ──"

    # The job should have completed (handler calls exit 0)
    assert_job_status "$wf_id" "sigterm_job" "completed"
    assert_return_code "$wf_id" "sigterm_job" "0"

    # Verify the SIGTERM handler fired by checking stdout
    local job_id stdout
    job_id=$(get_job_id "$wf_id" "sigterm_job")
    stdout=$(get_job_stdout "$wf_id" "$job_id")

    assert_contains "$stdout" "SIGTERM handler registered" "job registered SIGTERM handler"
    assert_contains "$stdout" "SIGTERM_RECEIVED" "job received SIGTERM from srun --signal"
    assert_contains "$stdout" "Graceful shutdown complete" "job performed graceful shutdown"

    # Workflow should be complete since the job exited 0
    assert_workflow_complete "$wf_id"
}
