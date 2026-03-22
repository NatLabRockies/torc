#!/usr/bin/env bash
#
# Manual test for authentication and access control.
#
# Tests:
#   1. Users can see their own workflows (no group)
#   2. Users CANNOT see other users' private workflows
#   3. Users CAN see workflows shared via team access groups
#   4. Users CANNOT see workflows from teams they're not in
#   5. Unauthenticated / wrong-password requests are rejected
#   6. Direct access by workflow ID respects access control
#   7. Unauthenticated access is rejected
#   8. Direct access by workflow ID respects access control
#   9. Hot-reload of auth credentials (add/remove user, reload, verify)
#
# Setup:
#   Users: alice, bob, carol, dave
#   Teams:
#     ml_team:    alice, bob
#     data_team:  bob, carol       (bob bridges ml and data)
#     infra_team: carol, dave      (carol bridges data and infra)
#
# Prerequisites:
#   - torc-server, torc-htpasswd, and torc binaries on PATH
#     cargo build --release --all-features
#     export PATH="$(pwd)/target/release:$PATH"
#
# Usage:
#   bash tests/workflows/access_control_test/run_test.sh

set -euo pipefail

WORK_DIR="$(mktemp -d)"
PASSWORD="correct horse battery staple"
PORT="${TORC_TEST_PORT:-18321}"
SERVER_PID=""
TORC_URL=""

# ── Helpers ──────────────────────────────────────────────────────────────────

cleanup() {
  echo ""
  echo "Cleaning up..."
  if [ -n "$SERVER_PID" ]; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
    echo "  Stopped server (PID $SERVER_PID)"
  fi
  echo "  Work directory: $WORK_DIR (not removed — inspect if needed)"
}
trap cleanup EXIT

PASS=0
FAIL=0

run_test() {
  local name="$1"
  shift
  echo -n "  $name... "
  if output=$("$@" 2>&1); then
    echo "PASS"
    PASS=$((PASS + 1))
  else
    echo "FAIL"
    echo "    $output" | head -5
    FAIL=$((FAIL + 1))
  fi
}

run_test_expect_fail() {
  local name="$1"
  shift
  echo -n "  $name... "
  if output=$("$@" 2>&1); then
    echo "FAIL (expected failure but got success)"
    echo "    $output" | head -5
    FAIL=$((FAIL + 1))
  else
    echo "PASS"
    PASS=$((PASS + 1))
  fi
}

# Run a torc command as a given user (sets USER env and --password flag).
as_user() {
  local username="$1"
  shift
  env USER="$username" torc --url "$TORC_URL" --password "$PASSWORD" "$@"
}

# Assert that `torc workflows list --all-users` output contains a given workflow name.
# --all-users is needed to see team workflows owned by others (default filters to own).
# We capture output first, then grep, to avoid pipefail issues with transient errors.
assert_workflow_visible() {
  local username="$1"
  local wf_name="$2"
  local listing
  listing=$(as_user "$username" workflows list --all-users 2>&1) || true
  echo "$listing" | grep -q "$wf_name"
}

# Assert that `torc workflows list --all-users` output does NOT contain a given workflow name.
assert_workflow_not_visible() {
  local username="$1"
  local wf_name="$2"
  local listing
  listing=$(as_user "$username" workflows list --all-users 2>&1) || true
  ! echo "$listing" | grep -q "$wf_name"
}

# Extract the "id" field from JSON output.
extract_id() {
  python3 -c "import sys,json; print(json.load(sys.stdin)['id'])"
}

check_command() {
  if ! command -v "$1" &>/dev/null; then
    echo "ERROR: $1 is required but not found on PATH"
    exit 1
  fi
}

# ── Prerequisites ────────────────────────────────────────────────────────────

check_command torc-server
check_command torc-htpasswd
check_command torc
check_command python3

echo "Work directory: $WORK_DIR"
echo ""

# ── Step 1: Create htpasswd file with four users ────────────────────────────

echo "=== Step 1: Creating users ==="
HTPASSWD="$WORK_DIR/torc-passwd"

CURRENT_USER="${USER:-$(whoami)}"
for user in "$CURRENT_USER" alice bob carol dave; do
  torc-htpasswd add --file "$HTPASSWD" "$user" --password "$PASSWORD" --cost 4
  echo "  Created user: $user"
done
echo ""

# ── Step 2: Start the server ────────────────────────────────────────────────

echo "=== Step 2: Starting server ==="

torc-server run \
  --database "$WORK_DIR/torc.db" \
  --log-dir "$WORK_DIR/torc-logs" \
  --auth-file "$HTPASSWD" \
  -c 5 \
  --host localhost \
  --port "$PORT" \
  --enforce-access-control \
  --threads 4 \
  --require-auth \
  --admin-user "$CURRENT_USER" \
  --admin-user alice \
  >"$WORK_DIR/server-stdout.log" 2>&1 &
SERVER_PID=$!
TORC_URL="http://localhost:$PORT/torc-service/v1"

# Wait for server to be ready
echo "  Waiting for server on port $PORT..."
for i in $(seq 1 50); do
  if as_user alice workflows list --limit 1 >/dev/null 2>&1; then
    echo "  Server is ready (PID $SERVER_PID, port $PORT)"
    break
  fi
  if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "ERROR: Server process died. Logs:"
    cat "$WORK_DIR/server-stdout.log" 2>/dev/null || true
    exit 1
  fi
  if [ "$i" -eq 50 ]; then
    echo "ERROR: Server did not start within 10 seconds. Logs:"
    cat "$WORK_DIR/server-stdout.log" 2>/dev/null || true
    exit 1
  fi
  sleep 0.2
done
echo ""

# ── Step 3: Create access groups (alice is admin) ───────────────────────────

echo "=== Step 3: Creating access groups ==="

ML_GROUP_ID=$(as_user alice -f json access-groups create "ml_team" \
  --description "Machine Learning team" | extract_id)
echo "  Created ml_team (group ID $ML_GROUP_ID)"

DATA_GROUP_ID=$(as_user alice -f json access-groups create "data_team" \
  --description "Data Engineering team" | extract_id)
echo "  Created data_team (group ID $DATA_GROUP_ID)"

INFRA_GROUP_ID=$(as_user alice -f json access-groups create "infra_team" \
  --description "Infrastructure team" | extract_id)
echo "  Created infra_team (group ID $INFRA_GROUP_ID)"
echo ""

# ── Step 4: Add users to groups ─────────────────────────────────────────────

echo "=== Step 4: Adding users to groups ==="

# ml_team: alice, bob
as_user alice access-groups add-user "$ML_GROUP_ID" alice --role admin
as_user alice access-groups add-user "$ML_GROUP_ID" bob --role member
echo "  ml_team:    alice (admin), bob (member)"

# data_team: bob, carol
as_user alice access-groups add-user "$DATA_GROUP_ID" bob --role admin
as_user alice access-groups add-user "$DATA_GROUP_ID" carol --role member
echo "  data_team:  bob (admin), carol (member)"

# infra_team: carol, dave
as_user alice access-groups add-user "$INFRA_GROUP_ID" carol --role admin
as_user alice access-groups add-user "$INFRA_GROUP_ID" dave --role member
echo "  infra_team: carol (admin), dave (member)"
echo ""

# ── Step 5: Create workflows ────────────────────────────────────────────────

echo "=== Step 5: Creating workflows ==="

# Team workflows — created by a team member, then assigned to the group.
ML_WF_ID=$(as_user alice -f json workflows new --name ml_experiment | extract_id)
as_user alice access-groups add-workflow "$ML_WF_ID" "$ML_GROUP_ID"
echo "  ml_experiment (ID $ML_WF_ID) → ml_team"

DATA_WF_ID=$(as_user bob -f json workflows new --name data_pipeline | extract_id)
as_user bob access-groups add-workflow "$DATA_WF_ID" "$DATA_GROUP_ID"
echo "  data_pipeline (ID $DATA_WF_ID) → data_team"

INFRA_WF_ID=$(as_user carol -f json workflows new --name infra_deploy | extract_id)
as_user carol access-groups add-workflow "$INFRA_WF_ID" "$INFRA_GROUP_ID"
echo "  infra_deploy (ID $INFRA_WF_ID) → infra_team"

# Private workflows — no group assigned. Only the owner should see them.
ALICE_PRIV_ID=$(as_user alice -f json workflows new --name alice_private | extract_id)
echo "  alice_private (ID $ALICE_PRIV_ID) — no group"

BOB_PRIV_ID=$(as_user bob -f json workflows new --name bob_private | extract_id)
echo "  bob_private (ID $BOB_PRIV_ID) — no group"

CAROL_PRIV_ID=$(as_user carol -f json workflows new --name carol_private | extract_id)
echo "  carol_private (ID $CAROL_PRIV_ID) — no group"

DAVE_PRIV_ID=$(as_user dave -f json workflows new --name dave_private | extract_id)
echo "  dave_private (ID $DAVE_PRIV_ID) — no group"
echo ""

# ── Step 6: Test visibility ─────────────────────────────────────────────────

echo "=== Step 6: Testing workflow visibility ==="
echo ""

# ── 6a: Owners can see their own private workflows ──
echo "  --- Private workflow visibility (owner can see own) ---"
run_test "alice sees alice_private" assert_workflow_visible alice alice_private
run_test "bob sees bob_private" assert_workflow_visible bob bob_private
run_test "carol sees carol_private" assert_workflow_visible carol carol_private
run_test "dave sees dave_private" assert_workflow_visible dave dave_private
echo ""

# ── 6b: Non-owners CANNOT see private workflows ──
echo "  --- Private workflow isolation (others cannot see) ---"
run_test "bob cannot see alice_private" assert_workflow_not_visible bob alice_private
run_test "carol cannot see alice_private" assert_workflow_not_visible carol alice_private
run_test "dave cannot see alice_private" assert_workflow_not_visible dave alice_private

run_test "alice cannot see bob_private" assert_workflow_not_visible alice bob_private
run_test "carol cannot see bob_private" assert_workflow_not_visible carol bob_private
run_test "dave cannot see bob_private" assert_workflow_not_visible dave bob_private

run_test "alice cannot see carol_private" assert_workflow_not_visible alice carol_private
run_test "bob cannot see carol_private" assert_workflow_not_visible bob carol_private
run_test "dave cannot see carol_private" assert_workflow_not_visible dave carol_private

run_test "alice cannot see dave_private" assert_workflow_not_visible alice dave_private
run_test "bob cannot see dave_private" assert_workflow_not_visible bob dave_private
run_test "carol cannot see dave_private" assert_workflow_not_visible carol dave_private
echo ""

# ── 6c: Team members CAN see team workflows ──
echo "  --- Team workflow visibility (members can see) ---"

# ml_team (alice, bob)
run_test "alice sees ml_experiment" assert_workflow_visible alice ml_experiment
run_test "bob sees ml_experiment" assert_workflow_visible bob ml_experiment

# data_team (bob, carol)
run_test "bob sees data_pipeline" assert_workflow_visible bob data_pipeline
run_test "carol sees data_pipeline" assert_workflow_visible carol data_pipeline

# infra_team (carol, dave)
run_test "carol sees infra_deploy" assert_workflow_visible carol infra_deploy
run_test "dave sees infra_deploy" assert_workflow_visible dave infra_deploy
echo ""

# ── 6d: Non-members CANNOT see team workflows ──
echo "  --- Team workflow isolation (non-members cannot see) ---"

# ml_experiment: only alice, bob
run_test "carol cannot see ml_experiment" assert_workflow_not_visible carol ml_experiment
run_test "dave cannot see ml_experiment" assert_workflow_not_visible dave ml_experiment

# data_pipeline: only bob, carol
run_test "alice cannot see data_pipeline" assert_workflow_not_visible alice data_pipeline
run_test "dave cannot see data_pipeline" assert_workflow_not_visible dave data_pipeline

# infra_deploy: only carol, dave
run_test "alice cannot see infra_deploy" assert_workflow_not_visible alice infra_deploy
run_test "bob cannot see infra_deploy" assert_workflow_not_visible bob infra_deploy
echo ""

# ── Step 7: Test unauthenticated access is denied ────────────────────────────

echo "=== Step 7: Testing unauthenticated access ==="
run_test_expect_fail "no credentials rejected" \
  torc --url "$TORC_URL" workflows list --limit 1

run_test_expect_fail "wrong password rejected" \
  env USER=alice torc --url "$TORC_URL" --password "wrong_password" workflows list --limit 1
echo ""

# ── Step 8: Test direct workflow access by ID ────────────────────────────────

echo "=== Step 8: Testing direct access by workflow ID ==="

# Bob should be able to get ml_experiment (he's in ml_team)
run_test "bob can get ml_experiment by ID" \
  as_user bob workflows status "$ML_WF_ID"

# Dave should NOT be able to get ml_experiment
run_test_expect_fail "dave cannot get ml_experiment by ID" \
  as_user dave workflows status "$ML_WF_ID"

# Carol can get data_pipeline (she's in data_team)
run_test "carol can get data_pipeline by ID" \
  as_user carol workflows status "$DATA_WF_ID"

# Alice cannot get infra_deploy (she's not in infra_team)
run_test_expect_fail "alice cannot get infra_deploy by ID" \
  as_user alice workflows status "$INFRA_WF_ID"

# Dave cannot get bob_private by ID
run_test_expect_fail "dave cannot get bob_private by ID" \
  as_user dave workflows status "$BOB_PRIV_ID"
echo ""

# ── Step 9: Test hot-reload of auth credentials ──────────────────────────────

echo "=== Step 9: Testing hot-reload of auth credentials ==="

# Add a new user "eve" to the htpasswd file
torc-htpasswd add --file "$HTPASSWD" --password "$PASSWORD" --cost 4 eve
echo "  Added eve to htpasswd file"

# Eve should be rejected before the server reloads credentials
run_test_expect_fail "eve rejected before reload" \
  env USER=eve torc --url "$TORC_URL" --password "$PASSWORD" workflows list --limit 1

# Admin (alice) triggers a credential reload
run_test "alice (admin) can reload auth" \
  as_user alice admin reload-auth

# Eve should now be able to authenticate
run_test "eve can authenticate after reload" \
  env USER=eve torc --url "$TORC_URL" --password "$PASSWORD" workflows list --limit 1

# Remove eve from the htpasswd file
torc-htpasswd remove --file "$HTPASSWD" eve
echo "  Removed eve from htpasswd file"

# Reload again so the server picks up the removal
run_test "alice (admin) can reload auth again" \
  as_user alice admin reload-auth

# Eve should now be rejected again
run_test_expect_fail "eve rejected after removal and reload" \
  env USER=eve torc --url "$TORC_URL" --password "$PASSWORD" workflows list --limit 1

# Non-admin (dave) should NOT be able to call reload-auth
run_test_expect_fail "non-admin dave cannot reload auth" \
  as_user dave admin reload-auth
echo ""

# ── Summary ──────────────────────────────────────────────────────────────────

echo "================================================================"
echo "Results: $PASS passed, $FAIL failed (out of $((PASS + FAIL)))"
echo "================================================================"
echo ""
echo "Work directory: $WORK_DIR"

if [ "$FAIL" -gt 0 ]; then
  echo ""
  echo "SOME TESTS FAILED — inspect server logs at:"
  echo "  $WORK_DIR/torc-logs/"
  exit 1
fi
