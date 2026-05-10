# Server Deployment

This guide covers deploying and operating the Torc server in production environments, including
logging configuration, daemonization, and service management.

## Server Subcommands

The `torc-server` binary has two main subcommands:

### `torc-server run`

Use `torc-server run` for:

- **HPC login nodes** - Run the server in a tmux session while your jobs are running.
- **Development and testing** - Run the server interactively in a terminal
- **Manual startup** - When you want to control when the server starts and stops
- **Custom deployment** - Integration with external process managers (e.g., supervisord, custom
  scripts)
- **Debugging** - Running with verbose logging to troubleshoot issues

```bash
# Basic usage
torc-server run

# With options
torc-server run --port 8080 --database ./torc.db --log-level debug
torc-server run --completion-check-interval-secs 5
```

### `torc-server service`

Use `torc-server service` for:

- **Production deployment** - Install as a system service that starts on boot
- **Reliability** - Automatic restart on failure
- **Managed lifecycle** - Standard start/stop/status commands
- **Platform integration** - Uses systemd (Linux), launchd (macOS), or Windows Services

```bash
# Install and start as a user service
torc-server service install --user
torc-server service start --user

# Or as a system service (requires root)
sudo torc-server service install
sudo torc-server service start
```

**Which to choose?**

- For **HPC login nodes/development/testing**: Use `torc-server run`
- For **production servers/standalone computers**: Use `torc-server service install`

## Quick Start

### User Service (Development)

For development, install as a user service (no root required):

```bash
# Install with automatic defaults (logs to ~/.torc/logs, db at ~/.torc/torc.db)
torc-server service install --user

# Start the service
torc-server service start --user
```

### System Service (Production)

For production deployment, install as a system service:

```bash
# Install with automatic defaults (logs to /var/log/torc, db at /var/lib/torc/torc.db)
sudo torc-server service install --user

# Start the service
sudo torc-server service start --user
```

The service will automatically start on boot and restart on failure. Logs are automatically
configured to rotate when they reach 10 MiB (keeping 5 files max). See the
[Service Management](#service-management-recommended-for-production) section for customization
options.

## Logging System

Torc-server uses the `tracing` ecosystem for structured, high-performance logging with automatic
size-based file rotation.

### Console Logging (Default)

By default, logs are written to stdout/stderr only:

```bash
torc-server run --log-level info
```

### File Logging with Size-Based Rotation

Enable file logging by specifying a log directory:

```bash
torc-server run --log-dir /var/log/torc
```

This will:

- Write logs to both console and file
- Automatically rotate when log file reaches 10 MiB
- Keep up to 5 rotated log files (torc-server.log, torc-server.log.1, ..., torc-server.log.5)
- Oldest files are automatically deleted when limit is exceeded

### JSON Format Logs

For structured log aggregation (e.g., ELK stack, Splunk):

```bash
torc-server run --log-dir /var/log/torc --json-logs
```

This writes JSON-formatted logs to the file while keeping human-readable logs on console.

### Log Levels

Control verbosity with the `--log-level` flag or `RUST_LOG` environment variable:

```bash
# Available levels: error, warn, info, debug, trace
torc-server run --log-level debug --log-dir /var/log/torc

# Or using environment variable
RUST_LOG=debug torc-server run --log-dir /var/log/torc
```

### Environment Variables

- `TORC_LOG_DIR`: Default log directory
- `RUST_LOG`: Default log level
- `TORC_MAX_REQUEST_BODY_MB`: Override the bulk job upload request-body limit in MiB
- `TORC_SERVER_SNAPSHOT_PATH`: Snapshot output path for in-memory mode (default
  `./torc-server-snapshot.db`). See
  [In-Memory Database with Snapshots](#in-memory-database-with-snapshots-advanced).
- `TORC_SERVER_SNAPSHOT_KEEP`: Number of snapshots to retain (default `5`, minimum `1`)
- `TORC_API_EVENT_BODY_MAX_BYTES`: Per-direction cap on captured request/response bodies surfaced
  through `torc admin tail-api` (default `8192`). See
  [Live API Request Inspection](#live-api-request-inspection).

Example:

```bash
export TORC_LOG_DIR=/var/log/torc
export RUST_LOG=info
export TORC_MAX_REQUEST_BODY_MB=500
torc-server run
```

`TORC_MAX_REQUEST_BODY_MB` applies to `POST /torc-service/v1/bulk_jobs`. Other JSON routes still use
Axum's default `2 MiB` body limit.

## Daemonization (Unix/Linux Only)

Run torc-server as a background daemon:

```bash
torc-server run --daemon --log-dir /var/log/torc
```

**Important:**

- Daemonization is only available on Unix/Linux systems
- When running as daemon, **you must use `--log-dir`** since console output is lost
- The daemon creates a PID file (default: `/var/run/torc-server.pid`)

### Custom PID File Location

```bash
torc-server run --daemon --pid-file /var/run/torc/server.pid --log-dir /var/log/torc
```

### Stopping a Daemon

```bash
# Find the PID
cat /var/run/torc-server.pid

# Kill the process
kill $(cat /var/run/torc-server.pid)

# Or forcefully
kill -9 $(cat /var/run/torc-server.pid)
```

## In-Memory Database with Snapshots (Advanced)

Torc-server supports running entirely from a SQLite in-memory database, with on-demand snapshots to
disk for persistence. This mode is intended for **HPC login and compute nodes where shared
filesystems (Lustre, GPFS, NFS) are intermittently slow**. SQLite running against a stalled shared
filesystem can hang request handlers for tens of seconds; running in memory eliminates that failure
mode entirely.

This is an advanced feature. The trade-off is straightforward: the database lives in RAM, and any
data not yet snapshotted is lost if the process dies. Use it when you understand that trade-off and
want to opt into it explicitly.

### Starting the Server In-Memory

Pass `:memory:` as the database path:

```bash
torc-server run -d ":memory:" -p 8080
```

On startup the server logs a confirmation message describing the snapshot configuration. Internally
the server uses SQLite shared-cache memory mode so all pool connections share a single database;
this is handled automatically — you only need to pass `:memory:`.

### Persisting State with SIGUSR1

To snapshot the in-memory database to disk, send `SIGUSR1` to the server process:

```bash
kill -USR1 $(pgrep -f 'torc-server run')
```

The server uses SQLite's [`VACUUM INTO`](https://sqlite.org/lang_vacuum.html#vacuuminto) to write a
consistent point-in-time copy without blocking writers. The snapshot is written to a `.tmp` sibling
first and then atomically renamed into place, so an interrupted snapshot can never corrupt a prior
one.

This works the same way for on-disk databases — you can use it as a hot-backup mechanism even when
not running in memory.

### Snapshot Rotation

By default the server keeps **5 snapshots**, with the canonical path always pointing at the newest:

```text
./torc-server-snapshot.db      # newest
./torc-server-snapshot.db.1    # previous
./torc-server-snapshot.db.2
./torc-server-snapshot.db.3
./torc-server-snapshot.db.4    # oldest
```

On each `SIGUSR1`, older snapshots are shifted down (`.1` → `.2`, etc.), the oldest is dropped, and
the freshly-written snapshot is renamed into the canonical path. If `TORC_SERVER_SNAPSHOT_KEEP=1`,
no rotation happens — each snapshot simply overwrites the previous one.

### Configuration

Two environment variables control snapshot behavior:

| Variable                    | Default                     | Description                                                                                     |
| --------------------------- | --------------------------- | ----------------------------------------------------------------------------------------------- |
| `TORC_SERVER_SNAPSHOT_PATH` | `./torc-server-snapshot.db` | Output path for the canonical (newest) snapshot. Relative paths resolve against the launch CWD. |
| `TORC_SERVER_SNAPSHOT_KEEP` | `5`                         | Total snapshots retained (canonical + rotated). Minimum `1`.                                    |

```bash
export TORC_SERVER_SNAPSHOT_PATH=/scratch/$USER/torc-snapshots/torc.db
export TORC_SERVER_SNAPSHOT_KEEP=10
torc-server run -d ":memory:" -p 8080
```

Pick a snapshot path on **fast local storage** (e.g. `/tmp`, `/scratch`) — writing snapshots to the
same slow shared filesystem that motivated in-memory mode in the first place defeats the purpose.

### Standalone Mode (`torc --standalone --in-memory`)

For ad-hoc workflows on HPC compute / login nodes, the easier entry point is the standalone client
flag — you do not need to manage `torc-server` directly. Both `exec` (inline commands) and `run`
(workflow spec files) support `--in-memory`:

```bash
# Inline commands
torc -s --in-memory exec -C commands.txt -j 8

# Workflow specification file
torc -s --in-memory run workflow.yaml
```

This launches an ephemeral in-memory `torc-server`, runs the workflow against it, and snapshots the
final database to `./torc_output/torc.db` (or wherever `--db` points) right before shutdown. After
the command returns, the workflow is queryable like any other:

```bash
torc -s results list
torc -s workflows list
torc tui --standalone
```

`--in-memory` is restricted to `exec` and `run` — commands that _create_ workflow state in the same
invocation. It is rejected for read-only commands like `results list` or `workflows list` because
those would snapshot an empty database over your existing `torc_output/torc.db` and destroy prior
data.

Add `--snapshot-interval-seconds <N>` to also snapshot periodically while the workflow is running.
Each snapshot briefly serializes against writes (milliseconds for small DBs, seconds for very large
ones), so prefer larger values — `600` (10 minutes) is a sensible default for high-throughput
workloads:

```bash
torc -s --in-memory --snapshot-interval-seconds 600 run workflow.yaml
```

If the parent process is killed unexpectedly, only state since the last snapshot is lost. Users who
need stronger durability should not opt into `--in-memory` in the first place.

### Restarting from a Snapshot

A snapshot file is a normal SQLite database. To resume from one, copy it into place and start the
server pointing at it:

```bash
cp /scratch/$USER/torc-snapshots/torc.db /tmp/torc-resume.db
torc-server run -d /tmp/torc-resume.db -p 8080
```

If you want to keep running in memory but seed from a prior snapshot, that's not currently supported
— the in-memory database always starts empty.

### When to Use This

Good fits:

- **HPC login/compute nodes** with slow or unreliable shared filesystems
- **Short-lived workflow runs** where you can afford to take a snapshot at the end
- **Performance-sensitive scenarios** where you want to eliminate disk I/O from the hot path

Poor fits:

- Long-running production servers (use a regular on-disk database with backups)
- Multi-day workflows where losing recent state on process death would be costly
- Deployments without operator access to send signals (signals are local-only; there is no remote
  snapshot endpoint)

### Operational Notes

- **Snapshots are signal-driven, not automatic.** Schedule them via cron, a sidecar, or
  workflow-completion hooks if you want periodic captures.
- **The snapshot completes on the server's signal-handler task**, so it does not block HTTP request
  handlers. A snapshot of a small database typically completes in milliseconds; large databases
  scale linearly with size.
- **`SIGUSR1` is Unix-only.** This feature is not available on Windows.
- **Process death loses unsaved data.** Always snapshot before stopping the server if you care about
  the current state. `SIGTERM`/`SIGINT` (graceful shutdown) does **not** automatically snapshot.

## Exporting Filtered Database Copies

`torc-server export` produces a standalone SQLite copy of the live database, optionally filtered to
a subset of workflows. The original workflow and job IDs are preserved verbatim, so log files,
ticket references, and screenshots referring to the production IDs remain interpretable in the
exported copy. The most common use case is **handing a debugging copy to an end user** who does not
have direct access to the production database — for example, so they can analyze their workflows
with [datasight](../tools/datasight.md), `sqlite3`, or another SQL tool without touching production.

```bash
# Hand a single user their workflows
torc-server export --user alice --output alice.db

# Export everything in a project's access group
torc-server export --access-group 7 --output proj-energy.db

# Pull a specific list of workflows (positional)
torc-server export 42 99 314 --output requested.db

# Full unfiltered copy (useful as a hot-backup)
torc-server export --output snapshot.db
```

The filters are mutually exclusive — pick one of `--user` (repeatable), `--access-group`
(repeatable), or positional workflow IDs. Without any filter, the command produces a full copy.

### How it works

1. **Snapshot.** SQLite's [`VACUUM INTO`](https://sqlite.org/lang_vacuum.html#vacuuminto) writes a
   transactionally consistent, defragmented copy of the live database to the output path. This does
   _not_ require quiescing the running server — readers and writers continue normally during the
   snapshot.
2. **Filter.** The output database is reopened with foreign keys enabled, and a single
   `DELETE FROM workflow WHERE id NOT IN (<filter>)` runs. Every per-workflow table has
   `ON DELETE CASCADE` on `workflow_id`, so jobs, files, results, events, ro_crate entities, compute
   nodes, etc. are removed automatically by the cascade chain — for the workflows the filter
   actually deleted.
3. **Sweep orphans (always).** Cascade only fires when the parent row IS deleted, so pre-existing
   orphans in the source DB survive the snapshot. Common sources: a `delete_workflow` code path that
   toggled `PRAGMA foreign_keys = OFF`, or a bare `sqlite3` CLI session (the CLI defaults to
   `foreign_keys = OFF`). The export iteratively runs `PRAGMA foreign_key_check` and deletes every
   reported violation until none remain. `workflow_status` is pruned separately (its back-reference
   column has no FK declared and so is invisible to `foreign_key_check`). This step runs for
   unfiltered exports too — FK violations are data corruption, not fidelity to the source.
4. **Sanitize.** If a filter was applied and `--preserve-access-groups` is not set, the exported
   database has its `user_group_membership` and `access_group` tables emptied. See
   [Access-control sanitization](#access-control-sanitization) below.
5. **Compact.** A final `VACUUM` reclaims the space freed by the deletes (skip with `--no-vacuum`).

If anything in steps 2–5 fails after step 1 has written the snapshot, the partial output file is
removed before the error is reported — a failed export never leaves a half-finished database on
disk.

### Flags

| Flag                       | Effect                                                                                             |
| -------------------------- | -------------------------------------------------------------------------------------------------- |
| `-o`, `--output <PATH>`    | Output SQLite file path (required).                                                                |
| `-d`, `--database <PATH>`  | Source database path. Defaults to `DATABASE_URL`.                                                  |
| `--user <NAME>`            | Keep only workflows owned by this user. Repeatable.                                                |
| `--access-group <ID>`      | Keep only workflows linked to this access-group ID. Repeatable.                                    |
| (positional)               | Keep only these workflow IDs.                                                                      |
| `--overwrite`              | Replace the output file if it already exists.                                                      |
| `--preserve-access-groups` | Keep `access_group` / `user_group_membership` / `workflow_access_group` instead of stripping them. |
| `--no-vacuum`              | Skip the final `VACUUM`. Faster, but the output file retains the source database's allocated size. |

If a filter is specified and matches zero workflows, the command errors out and removes the
partially-written output file rather than producing an empty database.

### Access-control sanitization

By default, `torc-server export` strips three tables from any **filtered** export:

- `user_group_membership` — has no per-workflow scoping, so leaving it intact would leak unrelated
  users' group affiliations.
- `access_group` — group names and descriptions for groups across the whole server.
- `workflow_access_group` — cascades away when `access_group` is emptied.

This is conservative on purpose: there is no straightforward per-workflow filter that wouldn't risk
accidentally leaking entries about other users or groups. If the recipient _is_ authorized to see
the entire access-control state (for example, when handing a full copy to another admin), pass
`--preserve-access-groups` to keep the tables intact.

For unfiltered (full-copy) exports, the access tables are kept as-is regardless — the operator
running the command already has access to everything in the database.

### Recommended workflow for end-user requests

The expected interaction pattern is admin-mediated:

1. End user asks for a copy of workflow `42` (or all workflows under user `alice`, etc.).
2. Admin runs `torc-server export` with the appropriate filter on the server host.
3. Admin reviews the output (`sqlite3 alice.db "SELECT id, user, name FROM workflow"`) and confirms
   it contains only the intended scope.
4. Admin transfers the file to the user.
5. User analyzes the copy locally — IDs match production, so anything that was in their logs or
   tickets continues to make sense.

This avoids needing to grant the user direct filesystem access to the production database, while
still giving them a faithful debugging artifact.

### Notes

- **Live server safe.** `VACUUM INTO` does not block the source server's writers or readers, and the
  export connection participates in SQLite's normal WAL coherency, so the snapshot reflects every
  committed transaction the running server can see.
- **Same-IDs guarantee.** The export preserves all primary keys. By contrast,
  [`torc workflows export`](../../core/workflows/export-import-workflows.md) emits portable JSON
  that loses ID identity on import — use that flow when the recipient cannot get a SQLite file from
  an admin.
- **External files are not bundled.** Only database rows are exported. Files referenced by `path` in
  the `file` table (job inputs, outputs, logs on shared filesystems) are not copied; the recipient
  analyzes metadata only unless those paths are independently shared.

## Complete Example: Production Deployment

```bash
#!/bin/bash
# Production deployment script

# Create required directories
sudo mkdir -p /var/log/torc
sudo mkdir -p /var/run/torc
sudo mkdir -p /var/lib/torc

# Set permissions (adjust as needed)
sudo chown -R torc:torc /var/log/torc
sudo chown -R torc:torc /var/run/torc
sudo chown -R torc:torc /var/lib/torc

# Start server as daemon
torc-server run \
    --daemon \
    --log-dir /var/log/torc \
    --log-level info \
    --json-logs \
    --pid-file /var/run/torc/server.pid \
    --database /var/lib/torc/torc.db \
    --host 0.0.0.0 \
    --port 8080 \
    --threads 8 \
    --auth-file /etc/torc/htpasswd \
    --require-auth
```

## Service Management (Recommended for Production)

### Automatic Installation

The easiest way to install torc-server as a service is using the built-in service management
commands.

#### User Service (No Root Required)

Install as a user service that runs under your user account (recommended for development):

```bash
# Install with defaults (logs to ~/.torc/logs, database at ~/.torc/torc.db)
torc-server service install --user

# Or customize the configuration
torc-server service install --user \
    --log-dir ~/custom/logs \
    --database ~/custom/torc.db \
    --host 0.0.0.0 \
    --port 8080 \
    --threads 4

# Start the user service
torc-server service start --user

# Check status
torc-server service status --user

# Stop the service
torc-server service stop --user

# Uninstall the service
torc-server service uninstall --user
```

**User Service Defaults:**

- Log directory: `~/.torc/logs`
- Database: `~/.torc/torc.db`
- Listen address: `0.0.0.0:8080`
- Worker threads: 4

#### System Service (Requires Root)

Install as a system-wide service (recommended for production):

```bash
# Install with defaults
sudo torc-server service install

# Or customize the configuration
sudo torc-server service install \
    --log-dir /var/log/torc \
    --database /var/lib/torc/torc.db \
    --host 0.0.0.0 \
    --port 8080 \
    --threads 8 \
    --auth-file /etc/torc/htpasswd \
    --require-auth \
    --json-logs

# Start the system service
sudo torc-server service start

# Check status
torc-server service status

# Stop the service
sudo torc-server service stop

# Uninstall the service
sudo torc-server service uninstall
```

**System Service Defaults:**

- Log directory: `/var/log/torc`
- Database: `/var/lib/torc/torc.db`
- Listen address: `0.0.0.0:8080`
- Worker threads: 4

This automatically creates the appropriate service configuration for your platform:

- **Linux**: systemd service (user: `~/.config/systemd/user/`, system: `/etc/systemd/system/`)
- **macOS**: launchd service (user: `~/Library/LaunchAgents/`, system: `/Library/LaunchDaemons/`)
- **Windows**: Windows Service

### Manual Systemd Service (Linux)

Alternatively, you can manually create a systemd service:

```ini
# /etc/systemd/system/torc-server.service
[Unit]
Description=Torc Workflow Orchestration Server
After=network.target

[Service]
Type=simple
User=torc
Group=torc
WorkingDirectory=/var/lib/torc
Environment="RUST_LOG=info"
Environment="TORC_LOG_DIR=/var/log/torc"
ExecStart=/usr/local/bin/torc-server run \
    --log-dir /var/log/torc \
    --json-logs \
    --database /var/lib/torc/torc.db \
    --host 0.0.0.0 \
    --port 8080 \
    --threads 8 \
    --auth-file /etc/torc/htpasswd \
    --require-auth
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
```

Then:

```bash
sudo systemctl daemon-reload
sudo systemctl enable torc-server
sudo systemctl start torc-server
sudo systemctl status torc-server

# View logs
journalctl -u torc-server -f
```

## Managing Users Without Downtime

User credentials can be added, removed, or updated without restarting the server. After modifying
the htpasswd file, reload the credentials:

```bash
# Add or remove users
torc-htpasswd add --file /etc/torc/htpasswd new_user
torc-htpasswd remove --file /etc/torc/htpasswd old_user

# Reload on the running server (admin credentials required)
torc admin reload-auth
```

For Docker/Kubernetes deployments, call `torc admin reload-auth` after updating the htpasswd file
instead of restarting the container. See
[Hot-Reloading Credentials](./authentication.md#hot-reloading-credentials) for details.

## Live API Request Inspection

`torc admin tail-api` streams a structured event for every inbound HTTP request the server
processes, delivered over Server-Sent Events. It is intended for debugging traffic against a running
server without tailing log files: events are network-accessible, structured, and emitted in real
time.

```bash
# Watch all API requests as they arrive
torc admin tail-api

# Include captured request and response bodies (subject to size caps)
torc admin tail-api --include-bodies

# Stream JSON, one event per line, e.g. for piping to jq
torc -f json admin tail-api | jq .
```

Each event includes the HTTP method, path, query string, status code, latency in milliseconds, the
`x-span-id` assigned by the server, and the authenticated user (when one was resolved). Output
fields:

| Field           | Description                                                              |
| --------------- | ------------------------------------------------------------------------ |
| `timestamp_ms`  | Unix epoch milliseconds at which the request finished.                   |
| `method`        | HTTP method (`GET`, `POST`, …).                                          |
| `path`          | URL path (without query string).                                         |
| `query`         | URL query string, when present.                                          |
| `status`        | Final HTTP status code returned to the client.                           |
| `latency_ms`    | Wall-clock duration spent inside the router.                             |
| `request_id`    | Server-assigned `x-span-id` for cross-referencing log lines.             |
| `user`          | Authenticated subject, or advisory client user (see below).              |
| `request_body`  | Captured request body, only when `--include-bodies` is set (see below).  |
| `response_body` | Captured response body, only when `--include-bodies` is set (see below). |

### Body capture

Body capture is **opt-in**. When `--include-bodies` is set, the server captures up to **8 KiB** per
direction and includes truncated UTF-8 text in each event. Override the per-direction display cap
with `TORC_API_EVENT_BODY_MAX_BYTES`.

Independent of the display cap, the server enforces a **1 MiB hard ceiling** on bytes it will buffer
in memory and only captures bodies whose size is advertised up front (via `Content-Length` or the
body's size hint). Larger payloads, chunked uploads with no advertised length, and SSE response
streams (e.g. `/workflows/{id}/events/stream`) are passed through untouched and emitted with no
body. Binary payloads are still recorded — `bytes` reflects the total length — but the `text` field
is omitted so consumers can detect non-UTF-8 content.

`Authorization` and `Cookie` headers are never captured under any setting.

### How the `user` field is populated

When the server is running with `--auth-file`, the `user` field contains the authenticated subject
resolved from Basic Auth. When the server is running **without** `--auth-file`, every request would
otherwise resolve to `anonymous`, which is not very useful for debugging. To improve that, the
`torc` CLI sends an advisory `X-Torc-Client-User` header on every API call, sourced from
`TORC_USERNAME` / `USER` / `USERNAME`. The capture middleware uses this header **only as a
fallback** when no real authentication was resolved.

The advisory header is **trivially spoofable** by any client and is never used for authorization or
workflow-ownership decisions — only for labeling events in this stream. If you need a trustworthy
`user` field, run the server with `--auth-file` and `--require-auth`.

### Performance

When no admin client is connected to the stream, the capture middleware short-circuits — no bodies
are buffered and no event is constructed, so the runtime cost on the request hot path is negligible.
Subscribing one or more admins begins event construction; subscribing _with_ `--include-bodies` is
what triggers body buffering.

### Endpoint

The underlying endpoint is `GET /torc-service/v1/admin/api-events/stream`. It accepts a single
optional query parameter, `include_bodies=true`. Like other admin endpoints (e.g.
`/admin/reload-auth`), it requires standard server authentication but no additional admin role —
restrict access via your htpasswd file or upstream proxy.

## Server Load Stats

For an at-a-glance view of how busy the server is — without streaming individual requests — use
`torc admin api-stats`. The server keeps a 1-hour ring of per-second counters; the CLI fetches an
aggregated snapshot and renders it as a table.

```bash
# Last hour, in 1-minute buckets (default)
torc admin api-stats

# Last 5 minutes, in 30-second buckets
torc admin api-stats --window 300 --interval 30

# Raw JSON for scripts or jq
torc -f json admin api-stats
```

Each row reports request count, requests-per-second, bytes in / bytes out, and a 2xx / 4xx / 5xx
status breakdown for that interval. A trailing `Total:` line sums the entire window.

### Counters

The capture middleware records every request — independent of whether anyone is connected to
`tail-api`, so the ring is always up to date. Bytes are read from the `Content-Length` request and
response headers; chunked or streaming responses (notably the SSE event streams themselves) do not
advertise a length and contribute `0` bytes, although the request itself is still counted. Stats are
in-memory only and reset on server restart.

### Endpoint

The underlying endpoint is `GET /torc-service/v1/admin/api-stats`, with optional `window_seconds`
(default `3600`, capped at the ring size) and `interval_seconds` (default `60`) query parameters.
Buckets are returned newest-first.

## Log Rotation Strategy

The server uses automatic size-based rotation with the following defaults:

- **Max file size**: 10 MiB per file
- **Max files**: 5 rotated files (plus the current log file)
- **Total disk usage**: Maximum of ~50 MiB for all log files

When the current log file reaches 10 MiB, it is automatically rotated:

1. `torc-server.log` → `torc-server.log.1`
2. `torc-server.log.1` → `torc-server.log.2`
3. And so on...
4. Oldest file (`torc-server.log.5`) is deleted

This ensures predictable disk usage without external tools like `logrotate`.

## Timing Instrumentation

For advanced performance monitoring, enable timing instrumentation:

```bash
TORC_TIMING_ENABLED=true torc-server run --log-dir /var/log/torc
```

This adds detailed timing information for all instrumented functions. Note that timing
instrumentation works with both console and file logging.

## Troubleshooting

### Daemon won't start

1. Check permissions on log directory:
   ```bash
   ls -la /var/log/torc
   ```

2. Check if PID file directory exists:
   ```bash
   ls -la /var/run/
   ```

3. Try running in foreground first:
   ```bash
   torc-server run --log-dir /var/log/torc
   ```

### No log files created

1. Verify `--log-dir` is specified
2. Check directory permissions
3. Check disk space: `df -h`

### Logs not rotating

Log rotation happens automatically when a log file reaches 10 MiB. If you need to verify rotation is
working:

1. Check the log directory for numbered files (e.g., `torc-server.log.1`)
2. Monitor disk usage - it should never exceed ~50 MiB for all log files
3. For testing, you can generate large amounts of logs with `--log-level trace`
