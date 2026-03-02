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

# Also check in repo target directories
if [ -z "$TORC_BIN" ] && [ -x "$REPO_ROOT/target/release/torc" ]; then
  TORC_BIN="$REPO_ROOT/target/release/torc"
fi
if [ -z "$TORC_SERVER_BIN" ] && [ -x "$REPO_ROOT/target/release/torc-server" ]; then
  TORC_SERVER_BIN="$REPO_ROOT/target/release/torc-server"
fi

# Override torc command to use the found binary
if [ -n "$TORC_BIN" ]; then
  torc() { "$TORC_BIN" "$@"; }
  export -f torc
fi

missing=()
if [ -z "$TORC_BIN" ]; then missing+=("torc"); fi
if [ -z "$TORC_SERVER_BIN" ]; then missing+=("torc-server"); fi
if ! command -v jq &>/dev/null; then missing+=("jq"); fi
if ! command -v sbatch &>/dev/null; then missing+=("sbatch"); fi

if [ ${#missing[@]} -gt 0 ]; then
  echo "ERROR: Missing prerequisites: ${missing[*]}"
  echo "Ensure torc, torc-server, jq, and Slurm tools are on your PATH."
  exit 1
fi

echo "  torc:        $TORC_BIN"
echo "  torc-server: $TORC_SERVER_BIN"
echo "  jq:          $(command -v jq)"
echo "  sbatch:      $(command -v sbatch)"
echo "  account:     $ACCOUNT"
echo "  host:        $HOST"
echo "  partition:    $PARTITION"
echo "  timeout:      ${TIMEOUT_MINUTES}m"
if [ -n "$TEST_FILTER" ]; then
  echo "  test filter: $TEST_FILTER"
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

# shellcheck disable=SC2329  # Used via trap
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

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "  SUBMITTING WORKFLOWS"
echo "═══════════════════════════════════════════════════════════════"

PREP_DIR="$RUN_DIR/prepared_workflows"
mkdir -p "$PREP_DIR"

# All available workflow names
ALL_WORKFLOW_NAMES=(
  single_node_basic
  multi_node_parallel
  multi_node_mpi_step
  oom_detection
  resource_monitoring
  failure_recovery
  timeout_detection
)

# Filter workflow names if --test was specified
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

for name in "${WORKFLOW_NAMES[@]}"; do
  echo ""
  echo "Submitting: $name"
  local_spec="$PREP_DIR/${name}.yaml"
  prepare_workflow_spec \
    "$SCRIPT_DIR/workflows/${name}.yaml" \
    "$ACCOUNT" \
    "$PARTITION" \
    "$local_spec"

  wf_id=$(submit_workflow "$local_spec")
  if [ -z "$wf_id" ]; then
    echo "FATAL: Failed to submit $name"
    exit 1
  fi
  WF_IDS[$name]=$wf_id
  echo "  -> workflow_id=$wf_id"
done

echo ""
echo "All workflows submitted:"
for name in "${WORKFLOW_NAMES[@]}"; do
  echo "  $name: ${WF_IDS[$name]}"
done

# Save workflow IDs for reference
for name in "${WORKFLOW_NAMES[@]}"; do
  echo "${WF_IDS[$name]} $name"
done >"$RUN_DIR/workflow_ids.txt"

# ── Validate server is still alive ───────────────────────────────────────────

if ! is_server_alive; then
  echo "FATAL: Server died after submission phase. Check $RUN_DIR/server.log"
  exit 1
fi

# ── Poll until all complete ───────────────────────────────────────────────────

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "  WAITING FOR WORKFLOWS TO COMPLETE"
echo "═══════════════════════════════════════════════════════════════"

ALL_IDS=()
for name in "${WORKFLOW_NAMES[@]}"; do
  ALL_IDS+=("${WF_IDS[$name]}")
done

poll_all_workflows "$TIMEOUT_SECONDS" "${ALL_IDS[@]}" || true

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
run_test_if_active multi_node_parallel
run_test_if_active multi_node_mpi_step
run_test_if_active oom_detection
run_test_if_active resource_monitoring
run_test_if_active failure_recovery
run_test_if_active timeout_detection

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
