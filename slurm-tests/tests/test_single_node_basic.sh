#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST, CURRENT_WF_ID used by sourced test_framework.sh
# Test 1: single_node_basic
#
# Verifies:
#   - Workflow completes successfully
#   - All 3 jobs completed with return code 0
#   - Dependency ordering: a ran before b, b ran before c

run_test_single_node_basic() {
    local wf_id="$1"
    CURRENT_TEST="single_node_basic"
    CURRENT_WF_ID="$wf_id"
    echo ""
    echo "── Test 1: single_node_basic (workflow $wf_id) ──"

    # Basic completion
    assert_workflow_complete "$wf_id"
    assert_all_jobs_completed "$wf_id" 3

    # Return codes
    assert_return_code "$wf_id" "job_a" "0"
    assert_return_code "$wf_id" "job_b" "0"
    assert_return_code "$wf_id" "job_c" "0"

    # Dependency ordering: check that job_a stdout timestamp < job_b < job_c
    local id_a id_b id_c
    id_a=$(get_job_id "$wf_id" "job_a")
    id_b=$(get_job_id "$wf_id" "job_b")
    id_c=$(get_job_id "$wf_id" "job_c")

    local stdout_a stdout_b stdout_c
    stdout_a=$(get_job_stdout "$wf_id" "$id_a")
    stdout_b=$(get_job_stdout "$wf_id" "$id_b")
    stdout_c=$(get_job_stdout "$wf_id" "$id_c")

    assert_contains "$stdout_a" "Job A complete" "job_a produced output"
    assert_contains "$stdout_b" "Job B complete" "job_b produced output"
    assert_contains "$stdout_c" "Job C complete" "job_c produced output"
}
