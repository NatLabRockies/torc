#!/bin/sh
set -e

# If the first argument is not "torc-server", exec the command directly.
# This allows: docker run image torc workflows list
#              docker run image sh
if [ "$1" != "torc-server" ]; then
  exec "$@"
fi

# Only apply the env-var-based rewrite for "torc-server" with no subcommand
# or with the "run" subcommand. For other subcommands, execute as-is.
# This allows: docker run image torc-server --version
if [ -n "$2" ] && [ "$2" != "run" ]; then
  exec "$@"
fi

# Build the torc-server command from environment variables.
# Shift past "torc-server" and "run" to collect any extra user-provided args.
shift
if [ "$1" = "run" ]; then
  shift
fi

# Auth file is required since --require-auth is always enabled
if [ -z "$TORC_AUTH_FILE" ]; then
  echo "ERROR: TORC_AUTH_FILE environment variable is required." >&2
  echo "Set it to the path of an htpasswd file mounted into the container." >&2
  echo "Example: -e TORC_AUTH_FILE=/data/htpasswd -v ./htpasswd:/data/htpasswd:ro" >&2
  exit 1
fi

# At least one admin user is required for managing access groups
if [ -z "$TORC_ADMIN_USERS" ]; then
  echo "ERROR: TORC_ADMIN_USERS environment variable is required." >&2
  echo "Set it to a comma-separated list of admin usernames." >&2
  echo "Example: -e TORC_ADMIN_USERS=admin or -e TORC_ADMIN_USERS=alice,bob" >&2
  exit 1
fi

set -- torc-server run \
  --host 0.0.0.0 \
  --port "${TORC_PORT:-8080}" \
  --database "${TORC_DATABASE:-/data/torc.db}" \
  --log-dir "${TORC_LOG_DIR:-/data}" \
  --log-level "${TORC_LOG_LEVEL:-info}" \
  --completion-check-interval-secs "${TORC_COMPLETION_CHECK_INTERVAL_SECS:-30}" \
  --require-auth \
  --enforce-access-control \
  --auth-file "${TORC_AUTH_FILE}" \
  "$@"

# Set the number of worker threads (default: 4)
set -- "$@" --threads "${TORC_THREADS:-4}"

# Append --admin-user flags for each comma-separated user in TORC_ADMIN_USERS
if [ -n "$TORC_ADMIN_USERS" ]; then
  OLD_IFS="$IFS"
  IFS=','
  for user in $TORC_ADMIN_USERS; do
    user=$(printf '%s\n' "$user" | xargs)
    if [ -n "$user" ]; then
      set -- "$@" --admin-user "$user"
    fi
  done
  IFS="$OLD_IFS"
fi

exec "$@"
