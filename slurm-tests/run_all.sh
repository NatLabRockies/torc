#!/bin/bash
# shellcheck disable=SC1091  # Sourced files resolved at runtime
# run_all.sh — Main entry point for the automated Slurm integration test suite.
#
# Usage:
#   ./slurm-tests/run_all.sh --account myproject --host kl1.hsn.cm.kestrel.hpc.nrel.gov [--partition debug] [--timeout 45]
#
# This script:
#   1. Validates prerequisites (torc, torc-server, jq, sbatch)
#   2. Starts a temporary torc server
#   3. Submits all test workflows
#   4. Polls until all reach terminal state (or timeout)
#   5. Runs verification assertions for each test
#   6. Prints summary and writes results.json
#   7. Cleans up (server process)

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── Parse arguments ───────────────────────────────────────────────────────────

ACCOUNT=""
HOST=""
PARTITION="debug"
TIMEOUT_MINUTES=45
TEST_FILTER=""

usage() {
  echo "Usage: $0 --account ACCOUNT --host HOSTNAME [OPTIONS]"
  echo ""
  echo "Options:"
  echo "  --account   ACCOUNT    Slurm account (required)"
  echo "  --host      HOSTNAME   Server hostname reachable from compute nodes (required)"
  echo "  --partition PARTITION  Slurm partition (default: debug)"
  echo "  --timeout   MINUTES    Max wait time in minutes (default: 45)"
  echo "  --test      PATTERN    Only run tests matching PATTERN (substring match)"
  exit 1
}

while [ $# -gt 0 ]; do
  case "$1" in
  --account)
    ACCOUNT="$2"
    shift 2
    ;;
  --host)
    HOST="$2"
    shift 2
    ;;
  --partition)
    PARTITION="$2"
    shift 2
    ;;
  --timeout)
    TIMEOUT_MINUTES="$2"
    shift 2
    ;;
  --test)
    TEST_FILTER="$2"
    shift 2
    ;;
  -h | --help)
    usage
    ;;
  *)
    echo "Unknown option: $1"
    usage
    ;;
  esac
done

if [ -z "$ACCOUNT" ]; then
  echo "ERROR: --account is required."
  usage
fi

if [ -z "$HOST" ]; then
  echo "ERROR: --host is required."
  echo "Specify a hostname reachable from compute nodes (e.g., the login node's FQDN)."
  usage
fi

TIMEOUT_SECONDS=$((TIMEOUT_MINUTES * 60))

# ── Validate prerequisites ────────────────────────────────────────────────────

echo "Validating prerequisites..."

TORC_BIN=$(command -v torc 2>/dev/null || echo "")
TORC_SERVER_BIN=$(command -v torc-server 2>/dev/null || echo "")
TORC_HTPASSWD_BIN=$(command -v torc-htpasswd 2>/dev/null || echo "")

# Also check in repo target directories
if [ -z "$TORC_BIN" ] && [ -x "$REPO_ROOT/target/release/torc" ]; then
  TORC_BIN="$REPO_ROOT/target/release/torc"
fi
if [ -z "$TORC_SERVER_BIN" ] && [ -x "$REPO_ROOT/target/release/torc-server" ]; then
  TORC_SERVER_BIN="$REPO_ROOT/target/release/torc-server"
fi
if [ -z "$TORC_HTPASSWD_BIN" ] && [ -x "$REPO_ROOT/target/release/torc-htpasswd" ]; then
  TORC_HTPASSWD_BIN="$REPO_ROOT/target/release/torc-htpasswd"
fi

# Override torc command to use the found binary
if [ -n "$TORC_BIN" ]; then
  torc() { "$TORC_BIN" "$@"; }
  export -f torc
fi

missing=()
if [ -z "$TORC_BIN" ]; then missing+=("torc"); fi
if [ -z "$TORC_SERVER_BIN" ]; then missing+=("torc-server"); fi
if [ -z "$TORC_HTPASSWD_BIN" ]; then missing+=("torc-htpasswd"); fi
if ! command -v jq &>/dev/null; then missing+=("jq"); fi
if ! command -v sbatch &>/dev/null; then missing+=("sbatch"); fi

if [ ${#missing[@]} -gt 0 ]; then
  echo "ERROR: Missing prerequisites: ${missing[*]}"
  echo "Ensure torc, torc-server, torc-htpasswd, jq, and Slurm tools are on your PATH."
  exit 1
fi

echo "  torc:           $TORC_BIN"
echo "  torc-server:    $TORC_SERVER_BIN"
echo "  torc-htpasswd:  $TORC_HTPASSWD_BIN"
echo "  jq:             $(command -v jq)"
echo "  sbatch:         $(command -v sbatch)"
echo "  account:        $ACCOUNT"
echo "  host:           $HOST"
echo "  partition:      $PARTITION"
echo "  timeout:        ${TIMEOUT_MINUTES}m"
if [ -n "$TEST_FILTER" ]; then
  echo "  test filter:    $TEST_FILTER"
fi

# ── Source libraries ──────────────────────────────────────────────────────────

# shellcheck source=lib/test_framework.sh
source "$SCRIPT_DIR/lib/test_framework.sh"
# shellcheck source=lib/server.sh
source "$SCRIPT_DIR/lib/server.sh"
# shellcheck source=lib/workflow.sh
source "$SCRIPT_DIR/lib/workflow.sh"

# Source all test scripts
for test_file in "$SCRIPT_DIR/tests"/test_*.sh; do
  # shellcheck source=/dev/null
  source "$test_file"
done

# ── Create run directory ──────────────────────────────────────────────────────

RUN_DIR="$SCRIPT_DIR/output/run_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RUN_DIR"
export RUN_DIR

echo ""
echo "Run directory: $RUN_DIR"

# ── Cleanup trap ──────────────────────────────────────────────────────────────

# shellcheck disable=SC2317,SC2329  # Used via trap
cleanup() {
  echo ""
  echo "Cleaning up..."
  cancel_slurm_jobs
  stop_server
  echo "Server stopped. Logs at: $RUN_DIR/server.log"
}
trap cleanup EXIT

# ── Start server ──────────────────────────────────────────────────────────────

DB_PATH="$RUN_DIR/torc.db"
PORT=$(find_free_port)

start_server "$DB_PATH" "$PORT" "$HOST"

echo ""
echo "Server running on port $PORT (PID $SERVER_PID)"
echo "TORC_API_URL=$TORC_API_URL"

# ── Prepare and submit workflows ─────────────────────────────────────────────

PREP_DIR="$RUN_DIR/prepared_workflows"
mkdir -p "$PREP_DIR"

# All available workflow names, split into two batches to avoid exceeding
# Slurm's limit on outstanding jobs. Batch 1 is submitted first, polled to
# completion, then batch 2 is submitted and polled.
# cancel_workflow is in batch 1 because it requires pre-poll actions.
# sync_status is in batch 2 (also requires pre-poll actions, handled there).
BATCH1_NAMES=(
  single_node_basic
  no_srun_basic
  cancel_workflow
  multi_node_parallel
  multi_node_mpi_step
  job_parallelism
  oom_detection
  resource_monitoring
  failure_recovery
  timeout_detection
)

BATCH2_NAMES=(
  srun_termination_signal
  sync_status
)

# Per-workflow extra arguments for `torc submit`.
# shellcheck disable=SC2034  # Used via ${SUBMIT_EXTRA_ARGS[...]}
declare -A SUBMIT_EXTRA_ARGS
SUBMIT_EXTRA_ARGS[job_parallelism]="--max-parallel-jobs 2"

# Build combined list and apply --test filter
ALL_WORKFLOW_NAMES=("${BATCH1_NAMES[@]}" "${BATCH2_NAMES[@]}")

WORKFLOW_NAMES=()
for name in "${ALL_WORKFLOW_NAMES[@]}"; do
  if [ -z "$TEST_FILTER" ] || [[ "$name" == *"$TEST_FILTER"* ]]; then
    WORKFLOW_NAMES+=("$name")
  fi
done

if [ ${#WORKFLOW_NAMES[@]} -eq 0 ]; then
  echo "ERROR: No tests match filter '$TEST_FILTER'"
  echo "Available tests: ${ALL_WORKFLOW_NAMES[*]}"
  exit 1
fi

echo "Running ${#WORKFLOW_NAMES[@]} test(s): ${WORKFLOW_NAMES[*]}"

declare -A WF_IDS

cd "$REPO_ROOT"

# submit_batch NAMES...
#   Submits all named workflows and records their IDs in WF_IDS.
submit_batch() {
  local names=("$@")
  for name in "${names[@]}"; do
    # Skip if not in the filtered list
    local found=false
    for wn in "${WORKFLOW_NAMES[@]}"; do
      if [ "$wn" = "$name" ]; then found=true; break; fi
    done
    if ! $found; then continue; fi

    echo ""
    echo "Submitting: $name"
    local local_spec="$PREP_DIR/${name}.yaml"
    prepare_workflow_spec \
      "$SCRIPT_DIR/workflows/${name}.yaml" \
      "$ACCOUNT" \
      "$PARTITION" \
      "$local_spec"

    local wf_id
    # shellcheck disable=SC2086  # Intentional word splitting for extra submit args
    wf_id=$(submit_workflow "$local_spec" ${SUBMIT_EXTRA_ARGS[$name]:-})
    if [ -z "$wf_id" ]; then
      echo "FATAL: Failed to submit $name"
      exit 1
    fi
    WF_IDS[$name]=$wf_id
    echo "  -> workflow_id=$wf_id"
  done
}

# collect_batch_ids NAMES...
#   Prints workflow IDs for the given names (only those that were submitted).
collect_batch_ids() {
  local names=("$@")
  for name in "${names[@]}"; do
    if [ -n "${WF_IDS[$name]+x}" ]; then
      echo "${WF_IDS[$name]}"
    fi
  done
}

# run_pre_poll_actions
#   Performs pre-poll actions for workflows that need intervention while running.
run_pre_poll_actions() {
  # Use longer timeout for pre-poll actions — on busy clusters, Slurm
  # allocations can queue for several minutes.
  local PRE_POLL_TIMEOUT=600

  if [ -n "${WF_IDS[cancel_workflow]+x}" ]; then
    echo ""
    echo "Pre-poll: cancel_workflow — waiting for jobs to start running..."
    # Wait for jobs to reach running, but cancel regardless of whether they do.
    # On busy clusters the Slurm allocation may still be queued.
    wait_for_job_status "${WF_IDS[cancel_workflow]}" "running" "$PRE_POLL_TIMEOUT" || \
      echo "  NOTE: Jobs not yet running — canceling workflow anyway."
    echo "  Canceling workflow ${WF_IDS[cancel_workflow]}..."
    torc --url "$TORC_API_URL" -f json workflows cancel "${WF_IDS[cancel_workflow]}" \
      > "$RUN_DIR/cancel_workflow_output.json" 2>"$RUN_DIR/cancel_workflow_stderr.log"
    echo "  Cancel output saved to $RUN_DIR/cancel_workflow_output.json"
  fi

  if [ -n "${WF_IDS[sync_status]+x}" ]; then
    echo ""
    echo "Pre-poll: sync_status — waiting for jobs to start running..."
    if wait_for_job_status "${WF_IDS[sync_status]}" "running" "$PRE_POLL_TIMEOUT"; then
      # Query Slurm job IDs for this workflow from the API
      local sync_slurm_ids
      sync_slurm_ids=$(torc --url "$TORC_API_URL" -f json scheduled-compute-nodes list \
        "${WF_IDS[sync_status]}" 2>/dev/null \
        | jq -r '.scheduled_compute_nodes[].scheduler_id' 2>/dev/null | tr '\n' ' ')
      if [ -n "$sync_slurm_ids" ]; then
        echo "  Externally killing Slurm allocation(s): $sync_slurm_ids"
        # shellcheck disable=SC2086  # Intentional word splitting for multiple Slurm IDs
        scancel $sync_slurm_ids 2>/dev/null || true
        echo "  Waiting 15s for Slurm to fully terminate the allocation..."
        sleep 15
      else
        echo "  WARNING: No Slurm job IDs found for sync_status workflow."
      fi
      echo "  Running sync-status to detect orphaned jobs..."
      torc --url "$TORC_API_URL" -f json workflows sync-status "${WF_IDS[sync_status]}" \
        > "$RUN_DIR/sync_status_output.json" 2>"$RUN_DIR/sync_status_stderr.log"
      echo "  sync-status output saved to $RUN_DIR/sync_status_output.json"
      # Cancel workflow so ready/blocked jobs don't prevent poll_all_workflows from completing
      torc --url "$TORC_API_URL" workflows cancel "${WF_IDS[sync_status]}" > /dev/null 2>&1 || true
    else
      echo "  WARNING: Timed out waiting for sync_status jobs to start running."
      echo "  sync_status test requires running jobs — marking as skipped."
      echo '{"skipped": true, "reason": "jobs never reached running status"}' \
        > "$RUN_DIR/sync_status_output.json"
      # Cancel the workflow so it doesn't block polling
      torc --url "$TORC_API_URL" workflows cancel "${WF_IDS[sync_status]}" > /dev/null 2>&1 || true
    fi
  fi
}

# ── Batch 1: submit, pre-poll actions, wait ──────────────────────────────────

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "  BATCH 1: SUBMITTING WORKFLOWS (up to 10)"
echo "═══════════════════════════════════════════════════════════════"

submit_batch "${BATCH1_NAMES[@]}"

if ! is_server_alive; then
  echo "FATAL: Server died after batch 1 submission. Check $RUN_DIR/server.log"
  exit 1
fi

run_pre_poll_actions

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "  BATCH 1: WAITING FOR WORKFLOWS TO COMPLETE"
echo "═══════════════════════════════════════════════════════════════"

BATCH1_IDS=()
while IFS= read -r id; do
  BATCH1_IDS+=("$id")
done < <(collect_batch_ids "${BATCH1_NAMES[@]}")

if [ ${#BATCH1_IDS[@]} -gt 0 ]; then
  poll_all_workflows "$TIMEOUT_SECONDS" "${BATCH1_IDS[@]}" || true
fi

# ── Batch 2: submit, pre-poll actions, wait ──────────────────────────────────

# Check if any batch 2 workflows passed the filter
BATCH2_FILTERED=()
for name in "${BATCH2_NAMES[@]}"; do
  for wn in "${WORKFLOW_NAMES[@]}"; do
    if [ "$wn" = "$name" ]; then BATCH2_FILTERED+=("$name"); break; fi
  done
done

if [ ${#BATCH2_FILTERED[@]} -gt 0 ]; then
  echo ""
  echo "═══════════════════════════════════════════════════════════════"
  echo "  BATCH 2: SUBMITTING WORKFLOWS (remaining)"
  echo "═══════════════════════════════════════════════════════════════"

  submit_batch "${BATCH2_NAMES[@]}"

  if ! is_server_alive; then
    echo "FATAL: Server died after batch 2 submission. Check $RUN_DIR/server.log"
    exit 1
  fi

  run_pre_poll_actions

  echo ""
  echo "═══════════════════════════════════════════════════════════════"
  echo "  BATCH 2: WAITING FOR WORKFLOWS TO COMPLETE"
  echo "═══════════════════════════════════════════════════════════════"

  BATCH2_IDS=()
  while IFS= read -r id; do
    BATCH2_IDS+=("$id")
  done < <(collect_batch_ids "${BATCH2_NAMES[@]}")

  if [ ${#BATCH2_IDS[@]} -gt 0 ]; then
    poll_all_workflows "$TIMEOUT_SECONDS" "${BATCH2_IDS[@]}" || true
  fi
fi

# Save workflow IDs for reference
for name in "${WORKFLOW_NAMES[@]}"; do
  if [ -n "${WF_IDS[$name]+x}" ]; then
    echo "${WF_IDS[$name]} $name"
  fi
done >"$RUN_DIR/workflow_ids.txt"

# ── Run verifications ─────────────────────────────────────────────────────────

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "  RUNNING VERIFICATIONS"
echo "═══════════════════════════════════════════════════════════════"

# Run test only if the workflow was submitted (respects --test filter)
run_test_if_active() {
  local name="$1"
  local func="run_test_$name"
  if [ -n "${WF_IDS[$name]+x}" ]; then
    "$func" "${WF_IDS[$name]}"
  fi
}

run_test_if_active single_node_basic
run_test_if_active no_srun_basic
run_test_if_active multi_node_parallel
run_test_if_active multi_node_mpi_step
run_test_if_active job_parallelism
run_test_if_active oom_detection
run_test_if_active resource_monitoring
run_test_if_active failure_recovery
run_test_if_active timeout_detection
run_test_if_active srun_termination_signal
run_test_if_active cancel_workflow
run_test_if_active sync_status

# ── Report ────────────────────────────────────────────────────────────────────

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "  RESULTS"
echo "═══════════════════════════════════════════════════════════════"

# Write results.json
write_results_json "$RUN_DIR/results.json"
echo "Results written to: $RUN_DIR/results.json"

# Print summary
print_test_summary
exit_code=$?

# Also save workflow status for debugging
echo ""
echo "Final workflow statuses:"
for name in "${WORKFLOW_NAMES[@]}"; do
  wf_id="${WF_IDS[$name]}"
  status_line=$(torc --url "$TORC_API_URL" -f json workflows status "$wf_id" 2>/dev/null |
    jq -r 'to_entries | map("\(.key)=\(.value)") | join(", ")' 2>/dev/null || echo "unknown")
  echo "  $name ($wf_id): $status_line"
done

exit "$exit_code"
