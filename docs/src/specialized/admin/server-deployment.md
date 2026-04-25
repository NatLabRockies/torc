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
