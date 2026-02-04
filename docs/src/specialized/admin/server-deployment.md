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

Example:

```bash
export TORC_LOG_DIR=/var/log/torc
export RUST_LOG=info
torc-server run
```

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
