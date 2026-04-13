#!/bin/bash
# shellcheck disable=SC1091  # Sourced files resolved at runtime
# run_all.sh — Launch the Slurm integration suite as a parent Torc workflow.

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── Parse arguments ───────────────────────────────────────────────────────────

ACCOUNT=""
HOST=""
PARTITION="debug"
TIMEOUT_MINUTES=45
TEST_FILTER=""
MAX_PARALLEL_JOBS=4

usage() {
  echo "Usage: $0 --account ACCOUNT --host HOSTNAME [OPTIONS]"
  echo ""
  echo "Options:"
  echo "  --account   ACCOUNT    Slurm account (required)"
  echo "  --host      HOSTNAME   Server hostname reachable from compute nodes (required)"
  echo "  --partition PARTITION  Slurm partition (default: debug)"
  echo "  --timeout   MINUTES    Max wait time in minutes (default: 45)"
  echo "  --max-parallel-jobs N  Parent Torc suite concurrency (default: 4)"
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
  --max-parallel-jobs)
    MAX_PARALLEL_JOBS="$2"
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
echo "  max parallel:   $MAX_PARALLEL_JOBS"
if [ -n "$TEST_FILTER" ]; then
  echo "  test filter:    $TEST_FILTER"
fi

# ── Source libraries ──────────────────────────────────────────────────────────

# shellcheck source=lib/server.sh
source "$SCRIPT_DIR/lib/server.sh"
# shellcheck source=lib/workflow.sh
source "$SCRIPT_DIR/lib/workflow.sh"

# ── Create run directory ──────────────────────────────────────────────────────

RUN_DIR="$SCRIPT_DIR/output/run_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RUN_DIR"
export RUN_DIR

echo ""
echo "Run directory: $RUN_DIR"

ALL_WORKFLOW_NAMES=(
  single_node_basic
  no_srun_basic
  cancel_workflow
  multi_node_parallel
  multi_node_mpi_step
  job_parallelism
  oom_detection
  watch_recover_oom
  resource_monitoring
  failure_recovery
  timeout_detection
  srun_termination_signal
  sync_status
)

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

echo "Selected ${#WORKFLOW_NAMES[@]} test(s): ${WORKFLOW_NAMES[*]}"

# ── Cleanup trap ──────────────────────────────────────────────────────────────

# shellcheck disable=SC2317,SC2329  # Used via trap
cleanup() {
  echo ""
  echo "Cleaning up..."
  if [ -f "$RUN_DIR/workflow_ids/all.txt" ]; then
    while read -r wf_id _; do
      [ -n "$wf_id" ] || continue
      torc --url "$TORC_API_URL" cancel "$wf_id" > /dev/null 2>&1 || true
    done < "$RUN_DIR/workflow_ids/all.txt"
  fi
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

cd "$REPO_ROOT"
mkdir -p "$RUN_DIR/workflow_ids"

PARENT_TEMPLATE="$SCRIPT_DIR/workflows/dogfood_parent.yaml"
PARENT_WORKFLOW="$RUN_DIR/parent_workflow.yaml"
prepare_workflow_spec "$PARENT_TEMPLATE" "$ACCOUNT" "$PARTITION" "$PARENT_WORKFLOW"

export RUN_DIR
export TEST_FILTER
export TORC_BIN
export TORC_API_URL

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "  RUNNING PARENT TORC WORKFLOW"
echo "═══════════════════════════════════════════════════════════════"

set +e
torc --url "$TORC_API_URL" run \
  --max-parallel-jobs "$MAX_PARALLEL_JOBS" \
  -o "$RUN_DIR/parent_output" \
  --time-limit "PT${TIMEOUT_MINUTES}M" \
  "$PARENT_WORKFLOW"
parent_exit=$?
set -e

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "  RESULTS"
echo "═══════════════════════════════════════════════════════════════"

total=0
passed=0
failed=0
skipped=0
failures_json='[]'

for name in "${WORKFLOW_NAMES[@]}"; do
  result_file="$RUN_DIR/results/${name}.json"
  if [ -f "$result_file" ]; then
    total=$((total + $(jq -r '.total // 0' "$result_file")))
    passed=$((passed + $(jq -r '.passed // 0' "$result_file")))
    failed=$((failed + $(jq -r '.failed // 0' "$result_file")))
    skipped=$((skipped + $(jq -r '.skipped // 0' "$result_file")))
    failures_json=$(jq -s '.[0] + .[1]' <(printf '%s\n' "$failures_json") \
      <(jq '.failures // []' "$result_file"))
  else
    failed=$((failed + 1))
    total=$((total + 1))
    failures_json=$(jq -s '.[0] + .[1]' <(printf '%s\n' "$failures_json") \
      <(jq -n --arg msg "[$name] missing results file: $result_file" '[$msg]'))
  fi
done

jq -n \
  --argjson total "$total" \
  --argjson passed "$passed" \
  --argjson failed "$failed" \
  --argjson skipped "$skipped" \
  --argjson failures "$failures_json" \
  '{total: $total, passed: $passed, failed: $failed, skipped: $skipped, failures: $failures}' \
  > "$RUN_DIR/results.json"
echo "Results written to: $RUN_DIR/results.json"

echo "  Total:   $total"
echo "  Passed:  $passed"
echo "  Failed:  $failed"
echo "  Skipped: $skipped"

echo ""
echo "Child workflow IDs:"
if [ -f "$RUN_DIR/workflow_ids/all.txt" ]; then
  cat "$RUN_DIR/workflow_ids/all.txt"
fi

if [ "$failed" -gt 0 ] || [ "$parent_exit" -ne 0 ]; then
  exit 1
fi

exit 0
