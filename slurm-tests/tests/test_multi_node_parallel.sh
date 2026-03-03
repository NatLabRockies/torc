#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST, CURRENT_WF_ID used by sourced test_framework.sh
# Test 2: multi_node_parallel
#
# Verifies:
#   - All 6 jobs complete successfully
#   - Jobs dispatched to 2 distinct nodes
#   - At least one job has peak CPU > 0 (sacct captured data)

run_test_multi_node_parallel() {
    local wf_id="$1"
    CURRENT_TEST="multi_node_parallel"
    CURRENT_WF_ID="$wf_id"
    echo ""
    echo "── Test 2: multi_node_parallel (workflow $wf_id) ──"

    # Basic completion
    assert_workflow_complete "$wf_id"
    assert_all_jobs_completed "$wf_id" 6

    # Return codes for all jobs
    for i in $(seq 1 6); do
        assert_return_code "$wf_id" "work_$i" "0"
    done

    # Multi-node dispatch: should see at least 2 distinct hostnames
    assert_multi_node_dispatch "$wf_id" 2

    # Resource monitoring: at least one job should have peak_cpu > 0
    # Not all jobs may have non-zero peak_cpu (short-lived srun steps may finish
    # before sacct captures accounting data), so check the workflow as a whole.
    assert_any_peak_cpu_nonzero "$wf_id"

    # Check sacct data is available
    local sacct_output
    sacct_output=$(torc --url "$TORC_API_URL" slurm sacct "$wf_id" 2>&1) || true
    assert_ne "$sacct_output" "" "sacct output is not empty"

    # Check slurm stats are available
    assert_slurm_stats_available "$wf_id"
}
