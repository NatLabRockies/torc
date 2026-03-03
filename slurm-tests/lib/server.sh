#!/bin/bash
# server.sh — Helpers for starting, stopping, and health-checking the torc server.
#
# Requires: TORC_BIN, TORC_SERVER_BIN, RUN_DIR to be set before sourcing.

# start_server DB_PATH PORT HOST
#   Starts the torc server with a SQLite database at DB_PATH on PORT.
#   HOST is the hostname/IP the server binds to (must be reachable from compute nodes).
#   Sets SERVER_PID and TORC_API_URL.
#   Enables authentication with two users: "admin" and the current $USER.
start_server() {
  local db_path="$1"
  local port="$2"
  local host="$3"
  local log_file="${RUN_DIR}/server.log"
  local htpasswd_file="${RUN_DIR}/htpasswd"

  # Generate a random password for test users
  if [ -f /usr/share/dict/words ] && command -v shuf &>/dev/null; then
    TORC_TEST_PASSWORD=$(shuf -n3 /usr/share/dict/words | tr '\n' '-' | sed 's/-$//')
  else
    TORC_TEST_PASSWORD=$(head -c 24 /dev/urandom | base64 | tr -dc 'a-zA-Z0-9' | head -c 16)
  fi

  # Create htpasswd file with admin and current user
  echo "Creating auth users (admin, $USER)..."
  "$TORC_HTPASSWD_BIN" add --file "$htpasswd_file" admin --password "$TORC_TEST_PASSWORD"
  "$TORC_HTPASSWD_BIN" add --file "$htpasswd_file" "$USER" --password "$TORC_TEST_PASSWORD"

  # Export password so all torc CLI calls authenticate automatically
  export TORC_PASSWORD="$TORC_TEST_PASSWORD"

  echo "Starting torc server on ${host}:${port} with database $db_path (auth enabled)..."
  DATABASE_URL="sqlite:${db_path}" "$TORC_SERVER_BIN" run \
    --host "$host" -p "$port" \
    --require-auth \
    --auth-file "$htpasswd_file" \
    --admin-user admin \
    --enforce-access-control \
    --completion-check-interval-secs 5 \
    >"$log_file" 2>&1 &
  SERVER_PID=$!

  export TORC_API_URL="http://${host}:${port}/torc-service/v1"

  # Wait for server to become healthy
  wait_for_server "$port" 30
}

# wait_for_server PORT TIMEOUT_SECONDS
#   Polls the server health endpoint until it responds or timeout.
wait_for_server() {
  local port="$1"
  local timeout="${2:-30}"
  local elapsed=0

  while [ "$elapsed" -lt "$timeout" ]; do
    if "$TORC_BIN" ping >/dev/null 2>&1; then
      echo "Server is healthy (port $port)."
      return 0
    fi
    sleep 1
    elapsed=$((elapsed + 1))
  done

  echo "ERROR: Server did not become healthy within ${timeout}s."
  if [ -f "${RUN_DIR}/server.log" ]; then
    echo "Server log (last 20 lines):"
    tail -20 "${RUN_DIR}/server.log"
  fi
  return 1
}

# stop_server
#   Kills the server process if running.
stop_server() {
  if [ -n "${SERVER_PID:-}" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "Stopping server (PID $SERVER_PID)..."
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
    SERVER_PID=""
  fi
}

# is_server_alive
#   Returns 0 if the server process is still running and responsive, 1 otherwise.
is_server_alive() {
  if [ -z "${SERVER_PID:-}" ]; then
    return 1
  fi
  if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    return 1
  fi
  # Also verify the server is responsive
  "$TORC_BIN" ping >/dev/null 2>&1
}

# find_free_port
#   Prints a free TCP port. Falls back to a random port in 10000-60000.
find_free_port() {
  if command -v python3 &>/dev/null; then
    python3 -c "
import socket
s = socket.socket()
s.bind(('', 0))
print(s.getsockname()[1])
s.close()
" 2>/dev/null && return
  fi
  # Fallback: random port
  echo $((RANDOM % 50000 + 10000))
}
