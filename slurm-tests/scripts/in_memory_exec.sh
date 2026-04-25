#!/bin/bash
# in_memory_exec.sh — Drive `torc -s --in-memory exec` on a Slurm compute node.
#
# Exercises the in-memory standalone database with periodic snapshots
# (the example from docs/src/core/how-to/run-inline-commands.md). Verifies
# that the snapshotted on-disk torc.db contains every subjob in the
# completed state after the standalone server exits.

set -Eeuo pipefail

JOB_COUNT="${IN_MEMORY_JOB_COUNT:-12}"
PARALLELISM="${IN_MEMORY_PARALLELISM:-4}"
SNAPSHOT_INTERVAL="${IN_MEMORY_SNAPSHOT_INTERVAL:-5}"
PER_JOB_SLEEP="${IN_MEMORY_PER_JOB_SLEEP:-3}"

WORKDIR="${SLURM_SUBMIT_DIR:-$PWD}/in_memory_exec_run_${SLURM_JOB_ID:-local}"
rm -rf "$WORKDIR"
mkdir -p "$WORKDIR"
cd "$WORKDIR"

echo "in_memory_exec: host=$(hostname) workdir=$WORKDIR"
echo "  jobs=$JOB_COUNT parallelism=$PARALLELISM snapshot-interval=${SNAPSHOT_INTERVAL}s sleep=${PER_JOB_SLEEP}s"

if ! command -v torc >/dev/null 2>&1; then
  echo "ERROR: torc not on PATH on $(hostname)" >&2
  exit 1
fi

if ! command -v sqlite3 >/dev/null 2>&1; then
  echo "ERROR: sqlite3 not on PATH on $(hostname)" >&2
  exit 1
fi

COMMANDS_FILE="$WORKDIR/commands.txt"
: > "$COMMANDS_FILE"
for i in $(seq 1 "$JOB_COUNT"); do
  # shellcheck disable=SC2016  # $(hostname) is intentionally evaluated when the subjob runs.
  printf 'bash -c "echo subjob %d on \\$(hostname); sleep %d"\n' \
    "$i" "$PER_JOB_SLEEP" >> "$COMMANDS_FILE"
done
echo "Wrote $JOB_COUNT commands to $COMMANDS_FILE"

# Run from a clean output dir so the standalone server's torc.db lives here.
OUTPUT_DIR="$WORKDIR/torc_output"
rm -rf "$OUTPUT_DIR"

set +e
torc -s --in-memory --snapshot-interval-seconds "$SNAPSHOT_INTERVAL" \
  exec \
  -n in_memory_exec_smoke \
  --description "Slurm in-memory exec smoke test" \
  -C "$COMMANDS_FILE" \
  -j "$PARALLELISM" \
  -o "$OUTPUT_DIR"
exec_rc=$?
set -e

echo "torc exec exit code: $exec_rc"
if [ "$exec_rc" -ne 0 ]; then
  echo "ERROR: torc -s --in-memory exec failed" >&2
  exit "$exec_rc"
fi

DB_PATH="$OUTPUT_DIR/torc.db"
if [ ! -f "$DB_PATH" ]; then
  echo "ERROR: expected snapshotted DB at $DB_PATH was not created" >&2
  exit 1
fi
echo "Snapshotted DB exists: $DB_PATH ($(stat -c%s "$DB_PATH" 2>/dev/null \
  || stat -f%z "$DB_PATH") bytes)"

# Validate every subjob completed (status=5) in the on-disk DB.
total=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM job;")
completed=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM job WHERE status = 5;")
echo "DB job counts: total=$total completed=$completed"

if [ "$total" -ne "$JOB_COUNT" ]; then
  echo "ERROR: expected $JOB_COUNT jobs in DB, found $total" >&2
  exit 1
fi

if [ "$completed" -ne "$JOB_COUNT" ]; then
  echo "ERROR: expected all $JOB_COUNT jobs completed, found $completed" >&2
  sqlite3 "$DB_PATH" "SELECT id, name, status FROM job;" >&2 || true
  exit 1
fi

echo "in_memory_exec: all $JOB_COUNT subjobs completed successfully"
