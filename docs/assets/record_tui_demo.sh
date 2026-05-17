#!/usr/bin/env bash
# Record docs/assets/tui-demo.gif: drives the Torc TUI with VHS while a faked
# "Simulation demo" workflow progresses through three stages (mid-flight ->
# failures appear -> finished).
#
# Environment overrides:
#   TORC_API_URL    Server URL (default: http://localhost:8080/torc-service/v1).
#                   Exported so the torc CLI and TUI hit the same server probed.
#   TORC_DEMO_DB    Path to the SQLite DB the server is writing to
#                   (default: $REPO_ROOT/db/sqlite/dev.db).
#
# Requirements: torc + torc-server, vhs, sqlite3, python3.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DB_PATH="${TORC_DEMO_DB:-$REPO_ROOT/db/sqlite/dev.db}"
SERVER_URL="${TORC_API_URL:-http://localhost:8080/torc-service/v1}"
export TORC_API_URL="$SERVER_URL"
WF_NAME="Simulation demo"
GIF_OUT="$REPO_ROOT/docs/assets/tui-demo.gif"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

for cmd in torc vhs sqlite3 curl python3; do
  command -v "$cmd" >/dev/null || {
    echo "Missing dependency: $cmd" >&2
    exit 1
  }
done

if ! curl -fsS --max-time 3 -o /dev/null "${SERVER_URL}/workflows?limit=1"; then
  echo "torc-server not reachable at ${SERVER_URL}" >&2
  exit 1
fi

[ -f "$DB_PATH" ] || {
  echo "SQLite DB not found at $DB_PATH" >&2
  echo "Override with TORC_DEMO_DB=/path/to/dev.db if the server uses a different file." >&2
  exit 1
}

# ---------- workflow spec ----------
# Heredoc is single-quoted (no shell expansion) so YAML regex backslashes and
# {param} braces pass through unmodified. WF_NAME is sed-substituted afterwards
# so the name lives in exactly one place.
cat >"$TMP/spec.yaml" <<'YAML'
name: __WF_NAME__
description: Parameter sweep across temperature and pressure
project: demo

jobs:
  - name: prepare_inputs
    command: python prepare.py --out=/data/config.xyz
    resource_requirements: small
    output_files:
      - config

  - name: simulate_T{temp}_P{pressure:03d}
    command: ./run_sim --config=/data/config.xyz --T={temp} --P={pressure}
    resource_requirements: simulation
    depends_on:
      - prepare_inputs
    input_files:
      - config
    output_files:
      - result_T{temp}_P{pressure:03d}
    parameters:
      temp: "250:400:50"
      pressure: "1:101:25"

  - name: summarize
    command: python summarize.py --out=/results/phase_diagram.png
    resource_requirements: small
    input_file_regexes:
      - "^result_T\\d+_P\\d+$"

files:
  - name: config
    path: /data/config.xyz

  - name: result_T{temp}_P{pressure:03d}
    path: /data/result_T{temp}_P{pressure:03d}.dat
    parameters:
      temp: "250:400:50"
      pressure: "1:101:25"

resource_requirements:
  - name: small
    num_cpus: 1
    num_gpus: 0
    num_nodes: 1
    memory: 2g
    runtime: PT10M

  - name: simulation
    num_cpus: 8
    num_gpus: 0
    num_nodes: 1
    memory: 30g
    runtime: PT4H
YAML
sed -i.bak "s|__WF_NAME__|$WF_NAME|g" "$TMP/spec.yaml" && rm "$TMP/spec.yaml.bak"

# ---------- wipe any existing "Simulation demo" workflows ----------
old_ids=$(torc -f json workflows list 2>/dev/null | WF_NAME="$WF_NAME" python3 -c '
import json, os, sys
data = json.load(sys.stdin)
if isinstance(data, dict):
    items = data.get("workflows") or data.get("items") or []
else:
    items = data
for w in items:
    if isinstance(w, dict) and w.get("name") == os.environ["WF_NAME"]:
        print(w["id"])
' || true)
if [ -n "$old_ids" ]; then
  # shellcheck disable=SC2086
  torc -f json delete --force $old_ids >/dev/null
fi

# ---------- create & initialize ----------
# torc -f json create emits {"workflow_id": N, "status": "success", ...}.
WF_ID=$(torc -f json create "$TMP/spec.yaml" | python3 -c '
import json, sys
print(json.load(sys.stdin)["workflow_id"])
')
[ -n "$WF_ID" ] || {
  echo "Could not parse workflow id from torc create" >&2
  exit 1
}
echo "Created workflow $WF_ID ($WF_NAME)"

curl -fsS -X POST -H 'content-type: application/json' -d '{}' \
  "${SERVER_URL}/workflows/${WF_ID}/initialize_jobs" >/dev/null

# ---------- stage SQL files ----------
# Stage 1: prepare done, T250 done (with results), T300 running, T350+T400
#          ready, summarize blocked.
# Stage 2: T300 completes, 2 jobs fail (T350_P101 OOM, T400_P101 error).
# Stage 3: everything finishes (20 completed, 2 failed).
# After every result insert, mirror into workflow_result so the list_results
# API (which joins through workflow_result) returns the rows.

cat >"$TMP/stage1.sql" <<SQL
INSERT INTO compute_node
  (workflow_id, hostname, pid, start_time, num_cpus, memory_gb, num_gpus,
   num_nodes, compute_node_type, is_active)
VALUES
  ($WF_ID, 'compute-node-001', 12345, '2026-05-17T10:00:00Z',
   8, 30.0, 0, 1, 'local', 1);

DELETE FROM workflow_result WHERE workflow_id = $WF_ID;
DELETE FROM result WHERE workflow_id = $WF_ID;

UPDATE job SET status = 5 WHERE workflow_id = $WF_ID AND name = 'prepare_inputs';
UPDATE job SET status = 5 WHERE workflow_id = $WF_ID AND name LIKE 'simulate_T250_%';
UPDATE job SET status = 4 WHERE workflow_id = $WF_ID AND name LIKE 'simulate_T300_%';
UPDATE job SET status = 2 WHERE workflow_id = $WF_ID
  AND (name LIKE 'simulate_T350_%' OR name LIKE 'simulate_T400_%');
UPDATE job SET status = 1 WHERE workflow_id = $WF_ID AND name = 'summarize';

INSERT INTO result
  (workflow_id, job_id, run_id, compute_node_id, return_code, exec_time_minutes,
   completion_time, status, peak_memory_bytes, avg_memory_bytes,
   peak_cpu_percent, avg_cpu_percent)
SELECT $WF_ID, j.id, 1,
       (SELECT id FROM compute_node WHERE workflow_id = $WF_ID LIMIT 1),
       0, 2.4, '2026-05-17T10:02:24Z', 5,
       1610612736, 1342177280, 92.3, 78.1
FROM job j WHERE j.workflow_id = $WF_ID AND j.name = 'prepare_inputs';

INSERT INTO result
  (workflow_id, job_id, run_id, compute_node_id, return_code, exec_time_minutes,
   completion_time, status, peak_memory_bytes, avg_memory_bytes,
   peak_cpu_percent, avg_cpu_percent)
SELECT $WF_ID, j.id, 1,
       (SELECT id FROM compute_node WHERE workflow_id = $WF_ID LIMIT 1),
       0, 115.0 + (j.id % 10),
       '2026-05-17T12:00:00Z', 5,
       27500000000 + (j.id % 5) * 500000000,
       21500000000 + (j.id % 5) * 400000000,
       720.0 + (j.id % 8) * 5.0,
       590.0 + (j.id % 8) * 5.0
FROM job j WHERE j.workflow_id = $WF_ID AND j.name LIKE 'simulate_T250_%';

INSERT OR REPLACE INTO workflow_result (workflow_id, job_id, result_id)
SELECT workflow_id, job_id, id FROM result WHERE workflow_id = $WF_ID;
SQL

cat >"$TMP/stage2.sql" <<SQL
UPDATE job SET status = 5 WHERE workflow_id = $WF_ID AND name LIKE 'simulate_T300_%';
UPDATE job SET status = 4 WHERE workflow_id = $WF_ID
  AND name IN ('simulate_T350_P001','simulate_T350_P026',
               'simulate_T350_P051','simulate_T350_P076');
UPDATE job SET status = 6 WHERE workflow_id = $WF_ID AND name = 'simulate_T350_P101';
UPDATE job SET status = 6 WHERE workflow_id = $WF_ID AND name = 'simulate_T400_P101';

INSERT INTO result
  (workflow_id, job_id, run_id, compute_node_id, return_code, exec_time_minutes,
   completion_time, status, peak_memory_bytes, avg_memory_bytes,
   peak_cpu_percent, avg_cpu_percent)
SELECT $WF_ID, j.id, 1,
       (SELECT id FROM compute_node WHERE workflow_id = $WF_ID LIMIT 1),
       0, 118.0 + (j.id % 10),
       '2026-05-17T12:05:00Z', 5,
       27000000000 + (j.id % 5) * 600000000,
       21000000000 + (j.id % 5) * 500000000,
       735.0 + (j.id % 8) * 5.0,
       605.0 + (j.id % 8) * 5.0
FROM job j WHERE j.workflow_id = $WF_ID AND j.name LIKE 'simulate_T300_%';

INSERT INTO result
  (workflow_id, job_id, run_id, compute_node_id, return_code, exec_time_minutes,
   completion_time, status, peak_memory_bytes, avg_memory_bytes,
   peak_cpu_percent, avg_cpu_percent)
SELECT $WF_ID, j.id, 1,
       (SELECT id FROM compute_node WHERE workflow_id = $WF_ID LIMIT 1),
       137, 42.8, '2026-05-17T12:08:00Z', 6,
       32212254720, 28991029248, 798.5, 692.1
FROM job j WHERE j.workflow_id = $WF_ID AND j.name = 'simulate_T350_P101';

INSERT INTO result
  (workflow_id, job_id, run_id, compute_node_id, return_code, exec_time_minutes,
   completion_time, status, peak_memory_bytes, avg_memory_bytes,
   peak_cpu_percent, avg_cpu_percent)
SELECT $WF_ID, j.id, 1,
       (SELECT id FROM compute_node WHERE workflow_id = $WF_ID LIMIT 1),
       1, 87.4, '2026-05-17T12:10:00Z', 6,
       18253611008, 15032385536, 720.1, 580.3
FROM job j WHERE j.workflow_id = $WF_ID AND j.name = 'simulate_T400_P101';

INSERT OR REPLACE INTO workflow_result (workflow_id, job_id, result_id)
SELECT workflow_id, job_id, id FROM result WHERE workflow_id = $WF_ID;
SQL

cat >"$TMP/stage3.sql" <<SQL
UPDATE job SET status = 5 WHERE workflow_id = $WF_ID
  AND name IN ('simulate_T350_P001','simulate_T350_P026',
               'simulate_T350_P051','simulate_T350_P076',
               'simulate_T400_P001','simulate_T400_P026',
               'simulate_T400_P051','simulate_T400_P076');
UPDATE job SET status = 5 WHERE workflow_id = $WF_ID AND name = 'summarize';

INSERT INTO result
  (workflow_id, job_id, run_id, compute_node_id, return_code, exec_time_minutes,
   completion_time, status, peak_memory_bytes, avg_memory_bytes,
   peak_cpu_percent, avg_cpu_percent)
SELECT $WF_ID, j.id, 1,
       (SELECT id FROM compute_node WHERE workflow_id = $WF_ID LIMIT 1),
       0, 120.0 + (j.id % 10),
       '2026-05-17T12:15:00Z', 5,
       26500000000 + (j.id % 5) * 700000000,
       21000000000 + (j.id % 5) * 600000000,
       730.0 + (j.id % 8) * 5.0,
       600.0 + (j.id % 8) * 5.0
FROM job j WHERE j.workflow_id = $WF_ID
  AND j.name IN ('simulate_T350_P001','simulate_T350_P026',
                 'simulate_T350_P051','simulate_T350_P076',
                 'simulate_T400_P001','simulate_T400_P026',
                 'simulate_T400_P051','simulate_T400_P076');

INSERT INTO result
  (workflow_id, job_id, run_id, compute_node_id, return_code, exec_time_minutes,
   completion_time, status, peak_memory_bytes, avg_memory_bytes,
   peak_cpu_percent, avg_cpu_percent)
SELECT $WF_ID, j.id, 1,
       (SELECT id FROM compute_node WHERE workflow_id = $WF_ID LIMIT 1),
       0, 3.1, '2026-05-17T12:18:00Z', 5,
       1932735283, 1610612736, 96.2, 85.4
FROM job j WHERE j.workflow_id = $WF_ID AND j.name = 'summarize';

INSERT OR REPLACE INTO workflow_result (workflow_id, job_id, result_id)
SELECT workflow_id, job_id, id FROM result WHERE workflow_id = $WF_ID;
SQL

# Apply Stage 1 now so the TUI shows mid-flight from frame 1.
sqlite3 "$DB_PATH" <"$TMP/stage1.sql"

# ---------- VHS tape ----------
# Hide block resets to Stage 1 and schedules Stages 2 and 3 in the background.
# `set +m` disables bash's job-monitor mode so backgrounded subshells don't
# print "[1] 12345" / "[1]+ Done" notices onto the TUI. The tape uses
# Left->Enter->Right to force the Jobs view to reload (the `r` key only
# refreshes the workflow list). Results tab is the last stop so its first load
# happens *after* Stage 3 has inserted all 22 result rows (the tab caches data).
#
# VHS's Output directive only accepts bare relative paths (absolute paths fail
# parsing with "Expected file path after output"). We cd into REPO_ROOT before
# running vhs, render as a bare filename there, then move into place.
#
# Paths interpolated into Type "..." commands are single-quoted inside the
# string so a checkout / temp dir with whitespace still produces a valid
# shell command when VHS types it into zsh.

mkdir -p "$(dirname "$GIF_OUT")"
RENDER_NAME="tui-demo.gif"
RENDER_PATH="$REPO_ROOT/$RENDER_NAME"

cat >"$TMP/tape" <<TAPE
Output $RENDER_NAME

Set Shell "bash"
Set FontSize 14
Set Width 1400
Set Height 800
Set Theme "Dracula"
Set TypingSpeed 60ms
Set PlaybackSpeed 1.0

Hide
Set TypingSpeed 10ms
Type "set +m"
Enter
Sleep 50ms
Type "sqlite3 '$DB_PATH' < '$TMP/stage1.sql' >/dev/null 2>&1"
Enter
Sleep 200ms
Type "( sleep 5 && sqlite3 '$DB_PATH' < '$TMP/stage2.sql' ) >/dev/null 2>&1 &"
Enter
Sleep 100ms
Type "( sleep 11 && sqlite3 '$DB_PATH' < '$TMP/stage3.sql' ) >/dev/null 2>&1 &"
Enter
Sleep 100ms
Type "clear"
Enter
Set TypingSpeed 60ms
Show

Type "torc tui"
Enter
Sleep 1.8s

Type "G"
Sleep 300ms
Enter
Sleep 1s

Right
Sleep 400ms
Tab
Sleep 2.2s

Left
Sleep 150ms
Enter
Sleep 150ms
Right
Sleep 3s

Down 8
Sleep 3s

Left
Sleep 150ms
Enter
Sleep 150ms
Right
Sleep 2.5s

Tab@50ms 3
Sleep 800ms
Sleep 7s

Type "q"
Sleep 500ms
TAPE

echo "Recording (this takes ~25s)..."
cd "$REPO_ROOT"
vhs "$TMP/tape" >/dev/null

# Verify the backgrounded stage transitions actually landed before publishing.
# Their stdout/stderr is suppressed inside the VHS shell, so check the DB
# directly — a stale state means a stale GIF.
expected_results=22
expected_completed=20
expected_failed=2
actual_results=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM result WHERE workflow_id = $WF_ID;")
actual_completed=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM job WHERE workflow_id = $WF_ID AND status = 5;")
actual_failed=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM job WHERE workflow_id = $WF_ID AND status = 6;")
if [ "$actual_results" -ne "$expected_results" ] ||
   [ "$actual_completed" -ne "$expected_completed" ] ||
   [ "$actual_failed" -ne "$expected_failed" ]; then
  echo "ERROR: post-recording DB state is wrong — not publishing GIF." >&2
  echo "  results:   got $actual_results,   expected $expected_results" >&2
  echo "  completed: got $actual_completed, expected $expected_completed" >&2
  echo "  failed:    got $actual_failed,    expected $expected_failed" >&2
  echo "  Background stage SQL may have failed silently. The rendered GIF is at" >&2
  echo "  $RENDER_PATH (not moved into docs/assets/)." >&2
  exit 1
fi

mv "$RENDER_PATH" "$GIF_OUT"
echo "Saved: $GIF_OUT"
