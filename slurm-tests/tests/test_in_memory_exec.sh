#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST, CURRENT_WF_ID used by sourced test_framework.sh
# Test: in_memory_exec
#
# Verifies:
#   - Parent (centralized) workflow with the single driver job completes
#   - The driver job exits 0 (it self-validates the standalone in-memory DB)
#   - Driver stdout reports the all-subjobs-completed sentinel, confirming
#     the snapshotted torc.db on the compute node had every subjob in
#     status=completed.

run_test_in_memory_exec() {
    local wf_id="$1"
    CURRENT_TEST="in_memory_exec"
    CURRENT_WF_ID="$wf_id"
    echo ""
    echo "── Test: in_memory_exec (workflow $wf_id) ──"

    assert_workflow_complete "$wf_id"
    assert_all_jobs_completed "$wf_id" 1
    assert_return_code "$wf_id" "in_memory_exec" "0"

    local job_id stdout
    job_id=$(get_job_id "$wf_id" "in_memory_exec")
    stdout=$(get_job_stdout "$wf_id" "$job_id")
    assert_contains "$stdout" "all" "driver script reported all subjobs completed"
    assert_contains "$stdout" "subjobs completed successfully" \
        "driver script printed completion sentinel"
    assert_contains "$stdout" "Snapshotted DB exists" \
        "driver script confirmed on-disk snapshot was created"
}
