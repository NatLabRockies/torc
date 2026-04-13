#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST, CURRENT_WF_ID used by sourced test_framework.sh
# Test: no_srun_basic
#
# Verifies:
#   - Workflow completes successfully with mode=direct
#   - Both jobs complete with return code 0
#   - avg_cpu_percent > 0 for cpu_work
#   - peak_memory_bytes > 0 for memory_work
#   - Time-series resource metrics DB exists and has sample data

run_test_no_srun_basic() {
    local wf_id="$1"
    CURRENT_TEST="no_srun_basic"
    CURRENT_WF_ID="$wf_id"
    echo ""
    echo "── Test: no_srun_basic (workflow $wf_id) ──"

    # Basic completion
    assert_workflow_complete "$wf_id"
    assert_all_jobs_completed "$wf_id" 2

    # Return codes
    assert_return_code "$wf_id" "cpu_work" "0"
    assert_return_code "$wf_id" "memory_work" "0"

    # Resource monitoring data captured (via process-level sysinfo, not sstat)
    assert_avg_cpu_nonzero "$wf_id" "cpu_work"
    assert_peak_memory_nonzero "$wf_id" "memory_work"

    # Check time-series resource metrics DB exists and has data
    assert_resource_metrics_db_has_data "$REPO_ROOT/torc_output" "$wf_id"
}
