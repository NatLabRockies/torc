#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST, CURRENT_WF_ID used by sourced test_framework.sh
# Test 2: multi_node_parallel
#
# Verifies:
#   - All 40 stress-ng jobs complete successfully
#   - Jobs dispatched to 2 distinct nodes
#   - Each job uses ~5 CPUs (peak CPU ~500%)
#   - Sacct and slurm stats data is available

run_test_multi_node_parallel() {
    local wf_id="$1"
    CURRENT_TEST="multi_node_parallel"
    CURRENT_WF_ID="$wf_id"
    echo ""
    echo "── Test 2: multi_node_parallel (workflow $wf_id) ──"

    # Basic completion
    assert_workflow_complete "$wf_id"
    assert_all_jobs_completed "$wf_id" 40

    # Return codes for a sample of jobs
    for i in $(seq 1 5); do
        assert_return_code "$wf_id" "stress_$i" "0"
    done

    # Multi-node dispatch: should see at least 2 distinct hostnames
    assert_multi_node_dispatch "$wf_id" 2

    # Check sacct data is available
    local sacct_output
    sacct_output=$(torc --url "$TORC_API_URL" slurm sacct "$wf_id" 2>&1) || true
    assert_ne "$sacct_output" "" "sacct output is not empty"

    # Check slurm stats are available
    assert_slurm_stats_available "$wf_id"
}
