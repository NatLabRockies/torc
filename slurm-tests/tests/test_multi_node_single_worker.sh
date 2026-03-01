#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST used by sourced test_framework.sh
# Test 3: multi_node_single_worker
#
# Verifies:
#   - All 4 jobs complete successfully
#   - All jobs run on the SAME node (single worker, start_one_worker_per_node not set)

run_test_multi_node_single_worker() {
    local wf_id="$1"
    CURRENT_TEST="multi_node_single_worker"
    echo ""
    echo "── Test 3: multi_node_single_worker (workflow $wf_id) ──"

    # Basic completion
    assert_workflow_complete "$wf_id"
    assert_all_jobs_completed "$wf_id" 4

    # Return codes
    for i in $(seq 1 4); do
        assert_return_code "$wf_id" "work_$i" "0"
    done

    # All jobs should run on the same node (single worker)
    local jobs_json hostnames
    jobs_json=$(torc --url "$TORC_API_URL" -f json jobs list "$wf_id" 2>/dev/null)
    hostnames=""
    while IFS= read -r job_id; do
        local stdout host
        stdout=$(get_job_stdout "$wf_id" "$job_id")
        host=$(echo "$stdout" | grep -oP 'on \K\S+' | head -1)
        if [ -n "$host" ]; then
            hostnames="$hostnames $host"
        fi
    done < <(echo "$jobs_json" | jq -r '.[].id')

    local unique_count
    unique_count=$(echo "$hostnames" | tr ' ' '\n' | sort -u | grep -c . || echo 0)
    assert_eq "$unique_count" "1" "all jobs ran on the same node (single worker mode)"
}
