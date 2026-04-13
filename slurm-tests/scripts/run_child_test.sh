#!/bin/bash
# shellcheck disable=SC1091

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SLURM_TESTS_DIR="$REPO_ROOT/slurm-tests"

TEST_NAME="${1:?test name is required}"
ACCOUNT="${2:?account is required}"
PARTITION="${3:?partition is required}"

if [ -n "${TEST_FILTER:-}" ] && [[ "$TEST_NAME" != *"$TEST_FILTER"* ]]; then
  echo "Skipping $TEST_NAME because TEST_FILTER=$TEST_FILTER"
  exit 0
fi

if [ -z "${RUN_DIR:-}" ]; then
  echo "RUN_DIR must be set" >&2
  exit 1
fi

if [ -z "${TORC_API_URL:-}" ]; then
  echo "TORC_API_URL must be set" >&2
  exit 1
fi

if [ -z "${TORC_BIN:-}" ]; then
  TORC_BIN="$(command -v torc 2>/dev/null || true)"
fi
if [ -n "${TORC_BIN:-}" ]; then
  torc() { "$TORC_BIN" "$@"; }
  export -f torc
fi

source "$SLURM_TESTS_DIR/lib/test_framework.sh"
source "$SLURM_TESTS_DIR/lib/workflow.sh"

for test_file in "$SLURM_TESTS_DIR/tests"/test_*.sh; do
  # shellcheck source=/dev/null
  source "$test_file"
done

mkdir -p \
  "$RUN_DIR/prepared_workflows" \
  "$RUN_DIR/results" \
  "$RUN_DIR/workflow_ids" \
  "$RUN_DIR/test_logs"

append_workflow_id() {
  local wf_id="$1"
  printf "%s %s\n" "$wf_id" "$TEST_NAME" >> "$RUN_DIR/workflow_ids/all.txt"
  printf "%s\n" "$wf_id" > "$RUN_DIR/workflow_ids/${TEST_NAME}.txt"
}

run_pre_poll_actions() {
  local wf_id="$1"
  local pre_poll_timeout=600

  case "$TEST_NAME" in
  cancel_workflow)
    echo "Pre-poll: waiting for running jobs before canceling workflow $wf_id"
    wait_for_job_status "$wf_id" "running" "$pre_poll_timeout" || \
      echo "Jobs not yet running, canceling workflow anyway"
    torc --url "$TORC_API_URL" -f json cancel "$wf_id" \
      > "$RUN_DIR/cancel_workflow_output.json" \
      2> "$RUN_DIR/cancel_workflow_stderr.log"
    ;;
  sync_status)
    echo "Pre-poll: waiting for running jobs before external scancel for workflow $wf_id"
    if wait_for_job_status "$wf_id" "running" "$pre_poll_timeout"; then
      local sync_slurm_ids
      sync_slurm_ids=$(torc --url "$TORC_API_URL" -f json scheduled-compute-nodes list "$wf_id" \
        2>/dev/null \
        | jq -r '.scheduled_compute_nodes[].scheduler_id' 2>/dev/null \
        | tr '\n' ' ')
      if [ -n "$sync_slurm_ids" ]; then
        echo "Externally killing Slurm allocation(s): $sync_slurm_ids"
        # shellcheck disable=SC2086
        scancel $sync_slurm_ids 2>/dev/null || true
        sleep 15
      fi
      torc --url "$TORC_API_URL" -f json workflows sync-status "$wf_id" \
        > "$RUN_DIR/sync_status_output.json" \
        2> "$RUN_DIR/sync_status_stderr.log"
      torc --url "$TORC_API_URL" cancel "$wf_id" > /dev/null 2>&1 || true
    else
      printf '%s\n' \
        '{"skipped": true, "reason": "jobs never reached running status"}' \
        > "$RUN_DIR/sync_status_output.json"
      torc --url "$TORC_API_URL" cancel "$wf_id" > /dev/null 2>&1 || true
    fi
    ;;
  esac
}

watch_child_workflow() {
  local wf_id="$1"
  local watch_args=(--url "$TORC_API_URL" watch "$wf_id" --poll-interval 10)

  if [ "$TEST_NAME" = "failure_recovery" ]; then
    watch_args+=(--auto-schedule --partition "$PARTITION")
  elif [ "$TEST_NAME" = "watch_recover_oom" ]; then
    watch_args+=(--recover --memory-multiplier 2.0 --partition "$PARTITION")
  fi

  "${TORC_BIN:-torc}" "${watch_args[@]}" || true
}

run_verification() {
  local wf_id="$1"
  local func="run_test_${TEST_NAME}"
  if ! declare -F "$func" >/dev/null 2>&1; then
    echo "Missing test function: $func" >&2
    return 1
  fi
  "$func" "$wf_id"
}

main() {
  local template="$SLURM_TESTS_DIR/workflows/${TEST_NAME}.yaml"
  local prepared="$RUN_DIR/prepared_workflows/${TEST_NAME}.yaml"
  local child_log="$RUN_DIR/test_logs/${TEST_NAME}.log"
  local result_json="$RUN_DIR/results/${TEST_NAME}.json"
  local submit_extra=()

  if [ ! -f "$template" ]; then
    echo "Missing child workflow template: $template" >&2
    exit 1
  fi

  prepare_workflow_spec "$template" "$ACCOUNT" "$PARTITION" "$prepared"

  if [ "$TEST_NAME" = "job_parallelism" ]; then
    submit_extra+=(--max-parallel-jobs 2)
  fi

  {
    echo "=== Running $TEST_NAME ==="
    echo "Prepared workflow: $prepared"
    # shellcheck disable=SC2086
    local wf_id
    wf_id=$(submit_workflow "$prepared" "${submit_extra[@]}")
    echo "Child workflow id: $wf_id"
    append_workflow_id "$wf_id"

    run_pre_poll_actions "$wf_id"
    watch_child_workflow "$wf_id"
    run_verification "$wf_id"
    write_results_json "$result_json"
    print_test_summary
  } 2>&1 | tee "$child_log"
}

main
