# Configuration Files Tutorial

This tutorial walks you through setting up Torc configuration files to customize your workflows
without specifying options on every command.

## What You'll Learn

- How to create a configuration file
- Configuration file locations and priority
- Using environment variables for configuration
- Common configuration patterns

## Prerequisites

- Torc CLI installed
- Basic familiarity with TOML format

## Step 1: Check Current Configuration

First, let's see what configuration Torc is using:

```bash
torc config paths
```

Output:

```
Configuration file paths (in priority order):

  System:  /etc/torc/config.toml (not found)
  User:    ~/.config/torc/config.toml (not found)
  Local:   torc.toml (not found)

Environment variables (highest priority):
  Use double underscore (__) to separate nested keys:
    TORC_CLIENT__API_URL, TORC_CLIENT__FORMAT, TORC_SERVER__PORT, etc.

No configuration files found. Run 'torc config init --user' to create one.
```

View the effective configuration (defaults):

```bash
torc config show
```

## Step 2: Create a User Configuration File

Create a configuration file in your home directory that applies to all your Torc usage:

```bash
torc config init --user
```

This creates `~/.config/torc/config.toml` (Linux/macOS) or the equivalent on your platform.

## Step 3: Edit the Configuration

Open the configuration file in your editor:

```bash
# Linux/macOS
$EDITOR ~/.config/torc/config.toml

# Or find the path
torc config paths
```

Here's a typical user configuration:

```toml
[client]
# Connect to your team's Torc server
api_url = "http://torc-server.internal:8080/torc-service/v1"

# Default to JSON output for scripting
format = "json"

# Enable debug logging
log_level = "debug"

# Username for authentication
username = "alice"

[client.run]
# Default poll interval for local runs
poll_interval = 10.0

# Default output directory
output_dir = "torc_output"

# Resource limits for local execution
num_cpus = 8
memory_gb = 32.0
num_gpus = 1
```

## Step 4: Validate Your Configuration

After editing, validate the configuration:

```bash
torc config validate
```

Output:

```
Validating configuration...

Loading configuration from:
  - /home/alice/.config/torc/config.toml

Configuration is valid.

Key settings:
  client.api_url = http://torc-server.internal:8080/torc-service/v1
  client.format = json
  server.port = 8080
  dash.port = 8090
```

## Step 5: Create a Project-Local Configuration

For project-specific settings, create a `torc.toml` in your project directory:

```bash
cd ~/myproject
torc config init --local
```

Edit `torc.toml`:

```toml
[client]
# Project-specific server (overrides user config)
api_url = "http://localhost:8080/torc-service/v1"

[client.run]
# Project-specific output directory
output_dir = "results"

# This project needs more memory
memory_gb = 64.0
```

## Step 6: Understanding Priority

Configuration sources are loaded in this order (later sources override earlier):

1. **Built-in defaults** (lowest priority)
2. **System config** (`/etc/torc/config.toml`)
3. **User config** (`~/.config/torc/config.toml`)
4. **Project-local config** (`./torc.toml`)
5. **Environment variables** (`TORC_*`)
6. **CLI arguments** (highest priority)

Example: If you have `api_url` set in your user config but run:

```bash
torc --url http://other-server:8080/torc-service/v1 workflows list
```

The CLI argument takes precedence.

## Step 7: Using Environment Variables

Environment variables are useful for CI/CD pipelines and temporary overrides.

Use double underscore (`__`) to separate nested keys:

```bash
# Override client.api_url
export TORC_CLIENT__API_URL="http://ci-server:8080/torc-service/v1"

# Override client.format
export TORC_CLIENT__FORMAT="json"

# Override server.port
export TORC_SERVER__PORT="9999"

# Verify
torc config show | grep api_url
```

## Step 8: Server Configuration

If you're running `torc-server`, you can configure it too:

```toml
[server]
# Bind to all interfaces
url = "0.0.0.0"
port = 8080

# Use 4 worker threads
threads = 4

# Database location
database = "/var/lib/torc/torc.db"

# Authentication
auth_file = "/etc/torc/htpasswd"
require_auth = true

# Background job processing interval
completion_check_interval_secs = 30.0

# Log level
log_level = "info"

[server.logging]
# Enable file logging
log_dir = "/var/log/torc"
json_logs = true
```

## Step 9: Dashboard Configuration

Configure `torc-dash`:

```toml
[dash]
# Bind address
host = "0.0.0.0"
port = 8090

# API server to connect to
api_url = "http://localhost:8080/torc-service/v1"

# Standalone mode settings
standalone = false
```

## Common Configuration Patterns

### Development Setup

```toml
# ~/.config/torc/config.toml
[client]
api_url = "http://localhost:8080/torc-service/v1"
format = "table"
log_level = "debug"

[client.run]
poll_interval = 2.0
output_dir = "torc_output"
```

### Production Server

```toml
# /etc/torc/config.toml
[server]
url = "0.0.0.0"
port = 8080
threads = 8
database = "/var/lib/torc/production.db"
require_auth = true
auth_file = "/etc/torc/htpasswd"
completion_check_interval_secs = 30.0
log_level = "info"

[server.logging]
log_dir = "/var/log/torc"
json_logs = true
```

### CI/CD Pipeline

```bash
# In CI script
export TORC_CLIENT__API_URL="${CI_TORC_SERVER_URL}"
export TORC_CLIENT__FORMAT="json"

torc run workflow.yaml
```

## Troubleshooting

### Configuration Not Loading

Check which files are being loaded:

```bash
torc config validate
```

### Environment Variables Not Working

Remember to use double underscore (`__`) for nesting:

```bash
# Correct
TORC_CLIENT__API_URL=http://...

# Wrong (single underscore)
TORC_CLIENT_API_URL=http://...
```

### View Effective Configuration

See the merged result of all configuration sources:

```bash
torc config show
```

## Next Steps

- See the [Configuration Reference](../../core/reference/configuration.md) for all available options
- Learn about [Server Deployment](../admin/server-deployment.md) for production setups
- Set up [Authentication](../admin/authentication.md) for secure access
