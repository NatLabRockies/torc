# Using Configuration Files

This guide shows how to set up and use configuration files for Torc components.

## Quick Start

Create a user configuration file:

```bash
torc config init --user
```

Edit the file at `~/.config/torc/config.toml` to set your defaults.

## Configuration File Locations

| Location                     | Purpose              | Priority    |
| ---------------------------- | -------------------- | ----------- |
| `/etc/torc/config.toml`      | System-wide defaults | 1 (lowest)  |
| `~/.config/torc/config.toml` | User preferences     | 2           |
| `./torc.toml`                | Project-specific     | 3           |
| Environment variables        | Runtime overrides    | 4           |
| CLI arguments                | Explicit overrides   | 5 (highest) |

## Available Commands

```bash
# Show configuration file locations
torc config paths

# Show effective (merged) configuration
torc config show

# Show as JSON
torc config show --format json

# Create configuration file
torc config init --user      # User config
torc config init --local     # Project config
torc config init --system    # System config (requires root)

# Validate configuration
torc config validate
```

## Client Configuration

Common client settings:

```toml
[client]
api_url = "http://localhost:8080/torc-service/v1"
format = "table"  # or "json"
log_level = "info"
username = "myuser"

[client.run]
poll_interval = 5.0
output_dir = "torc_output"
max_parallel_jobs = 4
num_cpus = 8
memory_gb = 32.0
num_gpus = 1
```

## Server Configuration

For `torc-server`:

```toml
[server]
url = "0.0.0.0"
port = 8080
threads = 4
database = "/path/to/torc.db"
auth_file = "/path/to/htpasswd"
require_auth = false
completion_check_interval_secs = 30.0
log_level = "info"
https = false

[server.logging]
log_dir = "/var/log/torc"
json_logs = false
```

## Dashboard Configuration

For `torc-dash`:

```toml
[dash]
host = "127.0.0.1"
port = 8090
api_url = "http://localhost:8080/torc-service/v1"
torc_bin = "torc"
torc_server_bin = "torc-server"
standalone = false
server_port = 0
completion_check_interval_secs = 5
```

## Environment Variables

Use environment variables for runtime configuration. Use double underscore (`__`) to separate nested
keys:

```bash
# Client settings
export TORC_CLIENT__API_URL="http://server:8080/torc-service/v1"
export TORC_CLIENT__FORMAT="json"

# Server settings
export TORC_SERVER__PORT="9999"
export TORC_SERVER__THREADS="8"

# Dashboard settings
export TORC_DASH__PORT="8090"
```

## Overriding with CLI Arguments

CLI arguments always take precedence:

```bash
# Uses config file for api_url, but CLI for format
torc --format json workflows list

# CLI url overrides config file
torc --url http://other:8080/torc-service/v1 workflows list
```

## Common Patterns

### Development Environment

```toml
# ~/.config/torc/config.toml
[client]
api_url = "http://localhost:8080/torc-service/v1"
log_level = "debug"

[client.run]
poll_interval = 2.0
```

### Team Shared Server

```toml
# ~/.config/torc/config.toml
[client]
api_url = "http://torc.internal.company.com:8080/torc-service/v1"
username = "developer"
```

### CI/CD Pipeline

```bash
#!/bin/bash
export TORC_CLIENT__API_URL="${CI_TORC_SERVER}"
export TORC_CLIENT__FORMAT="json"

torc run workflow.yaml
result=$(torc status $WORKFLOW_ID | jq -r '.status')
```

### HPC Cluster

```toml
# Project-local torc.toml
[client]
api_url = "http://login-node:8080/torc-service/v1"

[client.run]
num_cpus = 64
memory_gb = 256.0
num_gpus = 8
output_dir = "/scratch/user/workflow_output"
```

## Troubleshooting

**Configuration not applied?**

1. Check which files are loaded: `torc config validate`
2. View effective config: `torc config show`
3. Verify file permissions and syntax

**Environment variable not working?**

Use double underscore for nesting: `TORC_CLIENT__API_URL` (not `TORC_CLIENT_API_URL`)

**Invalid configuration?**

Run validation: `torc config validate`
