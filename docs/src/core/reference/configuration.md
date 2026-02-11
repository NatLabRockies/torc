# Configuration Reference

Complete reference for Torc configuration options.

## Configuration Sources

Torc loads configuration from multiple sources in this order (later sources override earlier):

1. **Built-in defaults** (lowest priority)
2. **System config**: `/etc/torc/config.toml`
3. **User config**: `~/.config/torc/config.toml` (platform-dependent)
4. **Project config**: `./torc.toml`
5. **Environment variables**: `TORC_*` prefix
6. **CLI arguments** (highest priority)

## Configuration Commands

```bash
torc config show              # Show effective configuration
torc config show --format json # Show as JSON
torc config paths             # Show configuration file locations
torc config init --user       # Create user config file
torc config init --local      # Create project config file
torc config init --system     # Create system config file
torc config validate          # Validate current configuration
```

## Client Configuration

Settings for the `torc` CLI.

### `[client]` Section

| Option      | Type   | Default                                 | Description                                          |
| ----------- | ------ | --------------------------------------- | ---------------------------------------------------- |
| `api_url`   | string | `http://localhost:8080/torc-service/v1` | Torc server API URL                                  |
| `format`    | string | `table`                                 | Output format: `table` or `json`                     |
| `log_level` | string | `info`                                  | Log level: `error`, `warn`, `info`, `debug`, `trace` |
| `username`  | string | (none)                                  | Username for basic authentication                    |

### `[client.run]` Section

Settings for `torc run` command.

| Option              | Type  | Default  | Description                                         |
| ------------------- | ----- | -------- | --------------------------------------------------- |
| `poll_interval`     | float | `5.0`    | Job completion poll interval (seconds)              |
| `output_dir`        | path  | `output` | Output directory for job logs                       |
| `max_parallel_jobs` | int   | (none)   | Maximum parallel jobs (overrides resource-based)    |
| `num_cpus`          | int   | (none)   | Available CPUs for resource-based scheduling        |
| `memory_gb`         | float | (none)   | Available memory (GB) for resource-based scheduling |
| `num_gpus`          | int   | (none)   | Available GPUs for resource-based scheduling        |

### `[client.tls]` Section

Settings for client-side TLS when connecting to an HTTPS server.

| Option     | Type   | Default | Description                                  |
| ---------- | ------ | ------- | -------------------------------------------- |
| `ca_cert`  | string | (none)  | Path to PEM-encoded CA certificate to trust  |
| `insecure` | bool   | `false` | Skip certificate verification (testing only) |

### Example

```toml
[client]
api_url = "https://torc.hpc.nrel.gov:8080/torc-service/v1"
format = "table"
log_level = "info"
username = "myuser"

[client.tls]
ca_cert = "/etc/pki/tls/certs/corporate-ca.pem"
# insecure = false

[client.run]
poll_interval = 5.0
output_dir = "output"
max_parallel_jobs = 4
num_cpus = 8
memory_gb = 32.0
num_gpus = 1
```

### `[client.hpc]` Section

Settings for HPC profile system (used by `torc hpc` and `torc slurm` commands).

| Option              | Type  | Default | Description                                 |
| ------------------- | ----- | ------- | ------------------------------------------- |
| `profile_overrides` | table | `{}`    | Override settings for built-in HPC profiles |
| `custom_profiles`   | table | `{}`    | Define custom HPC profiles                  |

### `[client.hpc.profile_overrides.<profile>]` Section

Override settings for built-in profiles (e.g., `kestrel`).

| Option            | Type   | Default | Description                            |
| ----------------- | ------ | ------- | -------------------------------------- |
| `default_account` | string | (none)  | Default Slurm account for this profile |

### `[client.hpc.custom_profiles.<name>]` Section

Define a custom HPC profile.

| Option            | Type   | Required | Description                                       |
| ----------------- | ------ | -------- | ------------------------------------------------- |
| `display_name`    | string | No       | Human-readable name                               |
| `description`     | string | No       | Profile description                               |
| `detect_env_var`  | string | No       | Environment variable for detection (`NAME=value`) |
| `detect_hostname` | string | No       | Regex pattern for hostname detection              |
| `default_account` | string | No       | Default Slurm account                             |
| `partitions`      | array  | Yes      | List of partition configurations                  |

### `[[client.hpc.custom_profiles.<name>.partitions]]` Section

Define partitions for a custom profile.

| Option                      | Type   | Required | Description                            |
| --------------------------- | ------ | -------- | -------------------------------------- |
| `name`                      | string | Yes      | Partition name                         |
| `cpus_per_node`             | int    | Yes      | CPU cores per node                     |
| `memory_mb`                 | int    | Yes      | Memory per node in MB                  |
| `max_walltime_secs`         | int    | Yes      | Maximum walltime in seconds            |
| `gpus_per_node`             | int    | No       | GPUs per node                          |
| `gpu_type`                  | string | No       | GPU model (e.g., "H100")               |
| `shared`                    | bool   | No       | Whether partition supports shared jobs |
| `min_nodes`                 | int    | No       | Minimum required nodes                 |
| `requires_explicit_request` | bool   | No       | Must be explicitly requested           |

### HPC Example

```toml
[client.hpc.profile_overrides.kestrel]
default_account = "my_default_account"

[client.hpc.custom_profiles.mycluster]
display_name = "My Research Cluster"
description = "Internal research HPC system"
detect_env_var = "MY_CLUSTER=research"
default_account = "default_project"

[[client.hpc.custom_profiles.mycluster.partitions]]
name = "compute"
cpus_per_node = 64
memory_mb = 256000
max_walltime_secs = 172800
shared = false

[[client.hpc.custom_profiles.mycluster.partitions]]
name = "gpu"
cpus_per_node = 32
memory_mb = 128000
max_walltime_secs = 86400
gpus_per_node = 4
gpu_type = "A100"
shared = false
```

## Server Configuration

Settings for `torc-server`.

### `[server]` Section

| Option                           | Type         | Default     | Description                                             |
| -------------------------------- | ------------ | ----------- | ------------------------------------------------------- |
| `log_level`                      | string       | `info`      | Log level                                               |
| `https`                          | bool         | `false`     | Enable HTTPS                                            |
| `url`                            | string       | `localhost` | Hostname/IP to bind to                                  |
| `port`                           | int          | `8080`      | Port to listen on                                       |
| `threads`                        | int          | `1`         | Number of worker threads                                |
| `database`                       | string       | (none)      | SQLite database path (falls back to `DATABASE_URL` env) |
| `auth_file`                      | string       | (none)      | Path to htpasswd file                                   |
| `require_auth`                   | bool         | `false`     | Require authentication for all requests                 |
| `enforce_access_control`         | bool         | `false`     | Enforce access control based on workflow ownership      |
| `admin_users`                    | string array | `[]`        | Users to add to the admin group                         |
| `completion_check_interval_secs` | float        | `30.0`      | Background job processing interval                      |

### `[server.logging]` Section

| Option      | Type | Default | Description                                    |
| ----------- | ---- | ------- | ---------------------------------------------- |
| `log_dir`   | path | (none)  | Directory for log files (enables file logging) |
| `json_logs` | bool | `false` | Use JSON format for log files                  |

### Example

```toml
[server]
url = "0.0.0.0"
port = 8080
threads = 4
database = "/var/lib/torc/torc.db"
auth_file = "/etc/torc/htpasswd"
require_auth = true
enforce_access_control = true
admin_users = ["alice", "bob"]
completion_check_interval_secs = 30.0
log_level = "info"
https = false

[server.logging]
log_dir = "/var/log/torc"
json_logs = false
```

## Dashboard Configuration

Settings for `torc-dash`.

### `[dash]` Section

| Option                           | Type   | Default                                 | Description                                 |
| -------------------------------- | ------ | --------------------------------------- | ------------------------------------------- |
| `host`                           | string | `127.0.0.1`                             | Hostname/IP to bind to                      |
| `port`                           | int    | `8090`                                  | Port to listen on                           |
| `api_url`                        | string | `http://localhost:8080/torc-service/v1` | Torc server API URL                         |
| `torc_bin`                       | string | `torc`                                  | Path to torc CLI binary                     |
| `torc_server_bin`                | string | `torc-server`                           | Path to torc-server binary                  |
| `standalone`                     | bool   | `false`                                 | Auto-start torc-server                      |
| `server_port`                    | int    | `0`                                     | Server port for standalone mode (0 = auto)  |
| `database`                       | string | (none)                                  | Database path for standalone mode           |
| `completion_check_interval_secs` | int    | `5`                                     | Completion check interval (standalone mode) |

### Example

```toml
[dash]
host = "0.0.0.0"
port = 8090
api_url = "http://localhost:8080/torc-service/v1"
torc_bin = "/usr/local/bin/torc"
torc_server_bin = "/usr/local/bin/torc-server"
standalone = true
server_port = 0
completion_check_interval_secs = 5
```

## Environment Variables

Environment variables use double underscore (`__`) to separate nested keys.

### Client Variables

| Variable                              | Maps To                        |
| ------------------------------------- | ------------------------------ |
| `TORC_CLIENT__API_URL`                | `client.api_url`               |
| `TORC_CLIENT__FORMAT`                 | `client.format`                |
| `TORC_CLIENT__LOG_LEVEL`              | `client.log_level`             |
| `TORC_CLIENT__USERNAME`               | `client.username`              |
| `TORC_CLIENT__RUN__POLL_INTERVAL`     | `client.run.poll_interval`     |
| `TORC_CLIENT__RUN__OUTPUT_DIR`        | `client.run.output_dir`        |
| `TORC_CLIENT__RUN__MAX_PARALLEL_JOBS` | `client.run.max_parallel_jobs` |
| `TORC_CLIENT__RUN__NUM_CPUS`          | `client.run.num_cpus`          |
| `TORC_CLIENT__RUN__MEMORY_GB`         | `client.run.memory_gb`         |
| `TORC_CLIENT__RUN__NUM_GPUS`          | `client.run.num_gpus`          |
| `TORC_CLIENT__TLS__CA_CERT`           | `client.tls.ca_cert`           |
| `TORC_CLIENT__TLS__INSECURE`          | `client.tls.insecure`          |

### Server Variables

| Variable                                      | Maps To                                 |
| --------------------------------------------- | --------------------------------------- |
| `TORC_SERVER__URL`                            | `server.url`                            |
| `TORC_SERVER__PORT`                           | `server.port`                           |
| `TORC_SERVER__THREADS`                        | `server.threads`                        |
| `TORC_SERVER__DATABASE`                       | `server.database`                       |
| `TORC_SERVER__AUTH_FILE`                      | `server.auth_file`                      |
| `TORC_SERVER__REQUIRE_AUTH`                   | `server.require_auth`                   |
| `TORC_SERVER__ENFORCE_ACCESS_CONTROL`         | `server.enforce_access_control`         |
| `TORC_SERVER__LOG_LEVEL`                      | `server.log_level`                      |
| `TORC_SERVER__COMPLETION_CHECK_INTERVAL_SECS` | `server.completion_check_interval_secs` |
| `TORC_SERVER__LOGGING__LOG_DIR`               | `server.logging.log_dir`                |
| `TORC_SERVER__LOGGING__JSON_LOGS`             | `server.logging.json_logs`              |

### Dashboard Variables

| Variable                | Maps To           |
| ----------------------- | ----------------- |
| `TORC_DASH__HOST`       | `dash.host`       |
| `TORC_DASH__PORT`       | `dash.port`       |
| `TORC_DASH__API_URL`    | `dash.api_url`    |
| `TORC_DASH__STANDALONE` | `dash.standalone` |

### Legacy Variables

These environment variables are still supported directly by clap:

| Variable                              | Component | Description                             |
| ------------------------------------- | --------- | --------------------------------------- |
| `TORC_API_URL`                        | Client    | Server API URL (CLI only)               |
| `TORC_PASSWORD`                       | Client    | Authentication password (CLI only)      |
| `TORC_TLS_CA_CERT`                    | Client    | PEM-encoded CA certificate path         |
| `TORC_TLS_INSECURE`                   | Client    | Skip certificate verification           |
| `TORC_AUTH_FILE`                      | Server    | htpasswd file path                      |
| `TORC_LOG_DIR`                        | Server    | Log directory                           |
| `TORC_COMPLETION_CHECK_INTERVAL_SECS` | Server    | Completion check interval               |
| `TORC_ADMIN_USERS`                    | Server    | Comma-separated list of admin usernames |
| `DATABASE_URL`                        | Server    | SQLite database URL                     |
| `RUST_LOG`                            | All       | Log level filter                        |

## Complete Example

```toml
# ~/.config/torc/config.toml

[client]
api_url = "https://torc.hpc.nrel.gov:8080/torc-service/v1"
format = "table"
log_level = "info"
username = "developer"

[client.tls]
ca_cert = "/etc/pki/tls/certs/corporate-ca.pem"
# insecure = false

[client.run]
poll_interval = 5.0
output_dir = "output"
num_cpus = 8
memory_gb = 32.0
num_gpus = 1

[server]
log_level = "info"
https = false
url = "localhost"
port = 8080
threads = 4
database = "/var/lib/torc/torc.db"
auth_file = "/etc/torc/htpasswd"
require_auth = true
enforce_access_control = true
admin_users = ["alice", "bob"]
completion_check_interval_secs = 30.0

[server.logging]
log_dir = "/var/log/torc"
json_logs = false

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

## See Also

- [Configuration Files How-To](../../specialized/admin/configuration-files.md)
- [Configuration Tutorial](../../specialized/tools/configuration.md)
- [Server Deployment](../../specialized/admin/server-deployment.md)
