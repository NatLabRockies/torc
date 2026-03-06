#!/bin/bash
# shellcheck disable=SC2034  # CURRENT_TEST, CURRENT_WF_ID used by sourced test_framework.sh
# Test: datasets
#
# Verifies:
#   - Workflow completes successfully
#   - All 4 jobs completed (3 producers + 1 consumer)
#   - Consumer job ran AFTER all producer jobs
#   - Consumer job saw all 3 partition files

run_test_datasets() {
    local wf_id="$1"
    CURRENT_TEST="datasets"
    CURRENT_WF_ID="$wf_id"
    echo ""
    echo "── Test: datasets (workflow $wf_id) ──"

    # Basic completion
    assert_workflow_complete "$wf_id"
    assert_all_jobs_completed "$wf_id" 4

    # Return codes
    assert_return_code "$wf_id" "producer_0" "0"
    assert_return_code "$wf_id" "producer_1" "0"
    assert_return_code "$wf_id" "producer_2" "0"
    assert_return_code "$wf_id" "consumer" "0"

    # Get job IDs
    local id_p0 id_p1 id_p2 id_consumer
    id_p0=$(get_job_id "$wf_id" "producer_0")
    id_p1=$(get_job_id "$wf_id" "producer_1")
    id_p2=$(get_job_id "$wf_id" "producer_2")
    id_consumer=$(get_job_id "$wf_id" "consumer")

    # Check consumer output shows all 3 partition files
    local stdout_consumer
    stdout_consumer=$(get_job_stdout "$wf_id" "$id_consumer")

    assert_contains "$stdout_consumer" "Consumer complete" "consumer produced output"
    assert_contains "$stdout_consumer" "Found 3 partition files" "consumer saw all 3 partitions"

    # Verify dependency ordering: consumer ran after all producers
    # We check this by verifying the consumer saw the partition files
    assert_contains "$stdout_consumer" "part_0.txt" "consumer saw part_0.txt"
    assert_contains "$stdout_consumer" "part_1.txt" "consumer saw part_1.txt"
    assert_contains "$stdout_consumer" "part_2.txt" "consumer saw part_2.txt"

    # Check dataset status (if torc datasets command is available)
    local dataset_list
    dataset_list=$(torc --url "$TORC_API_URL" -f json datasets list "$wf_id" 2>/dev/null) || true
    if [ -n "$dataset_list" ] && [ "$dataset_list" != "null" ]; then
        local dataset_status
        dataset_status=$(echo "$dataset_list" | jq -r '.datasets[0].status // "unknown"')
        assert_eq "$dataset_status" "finalized" "dataset is finalized"
    else
        echo "  SKIP: torc datasets list not available or returned no data"
    fi
}
