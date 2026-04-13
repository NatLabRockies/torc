#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST, CURRENT_WF_ID used by sourced test_framework.sh
# Test 6: resource_monitoring
#
# Verifies:
#   - Both jobs complete successfully
#   - avg_cpu_percent > 0 for cpu_work
#   - peak_memory_bytes > 0 for memory_work

run_test_resource_monitoring() {
    local wf_id="$1"
    CURRENT_TEST="resource_monitoring"
    CURRENT_WF_ID="$wf_id"
    echo ""
    echo "── Test 6: resource_monitoring (workflow $wf_id) ──"

    # Basic completion
    assert_workflow_complete "$wf_id"
    assert_all_jobs_completed "$wf_id" 2

    # Return codes
    assert_return_code "$wf_id" "cpu_work" "0"
    assert_return_code "$wf_id" "memory_work" "0"

    # Resource monitoring data captured
    assert_avg_cpu_nonzero "$wf_id" "cpu_work"
    assert_peak_memory_nonzero "$wf_id" "memory_work"

    # Also check that results are available in reports
    local results
    results=$(torc --url "$TORC_API_URL" -f json results list "$wf_id" --all-runs 2>/dev/null)
    local result_count
    result_count=$(echo "$results" | jq '.results | length')
    assert_ge "$result_count" "2" "at least 2 results in reports"

    # Check time-series resource metrics DB exists and has data
    assert_resource_metrics_db_has_data "$REPO_ROOT/torc_output" "$wf_id"
}
