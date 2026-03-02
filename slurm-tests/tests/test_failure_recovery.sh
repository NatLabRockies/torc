#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST, CURRENT_WF_ID used by sourced test_framework.sh
# Test 7: failure_recovery
#
# Verifies:
#   - work_3 was retried (attempt 2 succeeded)
#   - All 7 jobs eventually complete
#   - postprocess ran after all work jobs

run_test_failure_recovery() {
    local wf_id="$1"
    CURRENT_TEST="failure_recovery"
    CURRENT_WF_ID="$wf_id"
    echo ""
    echo "── Test 7: failure_recovery (workflow $wf_id) ──"

    # All jobs should complete (including retried work_3)
    assert_workflow_complete "$wf_id"
    assert_all_jobs_completed "$wf_id" 7

    # Return codes: all final results should be 0
    assert_return_code "$wf_id" "preprocess" "0"
    assert_return_code "$wf_id" "work_1" "0"
    assert_return_code "$wf_id" "work_2" "0"
    assert_return_code "$wf_id" "work_3" "0"
    assert_return_code "$wf_id" "work_4" "0"
    assert_return_code "$wf_id" "work_5" "0"
    assert_return_code "$wf_id" "postprocess" "0"

    # work_3 should have been retried: check logs for attempt evidence
    local work3_id stdout_work3
    work3_id=$(get_job_id "$wf_id" "work_3")
    stdout_work3=$(get_job_stdout "$wf_id" "$work3_id")
    assert_contains "$stdout_work3" "Completed successfully" "work_3 eventually succeeded"

    # Check that the results show work_3 had multiple attempts
    local results work3_results_count
    results=$(torc --url "$TORC_API_URL" -f json reports results "$wf_id" 2>/dev/null)
    work3_results_count=$(echo "$results" | jq "[.results[] | select(.job_id == $work3_id)] | length")
    assert_ge "$work3_results_count" "2" "work_3 has >= 2 result entries (retry evidence)"

    # postprocess should contain completion message
    local post_id stdout_post
    post_id=$(get_job_id "$wf_id" "postprocess")
    stdout_post=$(get_job_stdout "$wf_id" "$post_id")
    assert_contains "$stdout_post" "Postprocess: Completed successfully" "postprocess ran successfully"
}
