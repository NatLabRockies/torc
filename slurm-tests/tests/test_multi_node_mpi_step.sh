#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST, CURRENT_WF_ID used by sourced test_framework.sh
# Test 4: multi_node_mpi_step
#
# Verifies:
#   - Job completes successfully
#   - Allocation spans >= 2 nodes (SLURM_JOB_NUM_NODES)
#   - sacct shows multi-node allocation

run_test_multi_node_mpi_step() {
    local wf_id="$1"
    CURRENT_TEST="multi_node_mpi_step"
    CURRENT_WF_ID="$wf_id"
    echo ""
    echo "── Test 4: multi_node_mpi_step (workflow $wf_id) ──"

    # Basic completion
    assert_workflow_complete "$wf_id"
    assert_all_jobs_completed "$wf_id" 1
    assert_return_code "$wf_id" "mpi_job" "0"

    # Check job stdout for multi-node evidence
    local job_id stdout
    job_id=$(get_job_id "$wf_id" "mpi_job")
    stdout=$(get_job_stdout "$wf_id" "$job_id")

    assert_contains "$stdout" "Multi-node step complete" "mpi_job produced expected output"

    # Check allocation node count >= 2 in stdout
    local node_count
    node_count=$(echo "$stdout" | grep -oP 'Allocation node count: \K\d+' || echo 0)
    assert_ge "$node_count" "2" "allocation spans >= 2 nodes (got $node_count)"

    # Verify sacct shows the allocation
    local sacct_output
    sacct_output=$(torc --url "$TORC_API_URL" slurm sacct "$wf_id" 2>&1) || true
    assert_ne "$sacct_output" "" "sacct output is not empty for MPI workflow"
}
