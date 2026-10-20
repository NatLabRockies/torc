# Settings reference

Section-by-section defaults, verified against `src/config/` in this repository. `torc config show`
prints the effective merged values; `torc config init --user` writes a commented template.

`docs/src/core/reference/configuration.md` covers the same ground in prose. Where the two disagree,
`torc config show` wins.

## Contents

- [client](#client)
- [client.run](#clientrun)
- [client.offline](#clientoffline)
- [client.slurm](#clientslurm)
- [client.watch](#clientwatch)
- [client.tls](#clienttls)
- [client.hpc](#clienthpc)
- [server](#server)
- [server.logging](#serverlogging)
- [dash](#dash)
- [Environment variable mapping](#environment-variable-mapping)
- [Worked example](#worked-example)

## client

| Option      | Default                                 | Notes                                     |
| ----------- | --------------------------------------- | ----------------------------------------- |
| `api_url`   | `http://localhost:8080/torc-service/v1` | Must include the `/torc-service/v1` path  |
| `format`    | `table`                                 | `table`, `json`, or `csv`                 |
| `log_level` | `info`                                  | `error`, `warn`, `info`, `debug`, `trace` |

`--url` and `TORC_API_URL` override `api_url`. `-f` overrides `format` only when explicitly passed,
since clap's own default is also `table`.

## client.run

Defaults for `torc run`.

| Option                   | Default       | Notes                                                           |
| ------------------------ | ------------- | --------------------------------------------------------------- |
| `poll_interval`          | `5.0`         | Seconds between job-completion polls                            |
| `claim_backoff_max_secs` | `300.0`       | Cap on adaptive idle backoff; set to `poll_interval` to disable |
| `max_parallel_jobs`      | unset         | Unset means resource-based packing                              |
| `output_dir`             | `torc_output` | Where logs and artifacts go                                     |
| `num_cpus`               | unset         | Unset means detect                                              |
| `memory_gb`              | unset         | Unset means detect                                              |
| `num_gpus`               | unset         | Unset means detect                                              |

The backoff doubles from `poll_interval` toward `claim_backoff_max_secs` after an iteration with no
completions and no claims, and resets on any progress. Raise it to reduce server load from many idle
runners; lower it when a runner must pick up work promptly.

Setting `num_cpus` / `memory_gb` / `num_gpus` below the machine's real capacity is how you keep a
runner from taking over a shared workstation.

## client.offline

Behavior when a runner loses the server.

| Option                     | Default | Notes                                             |
| -------------------------- | ------- | ------------------------------------------------- |
| `enabled`                  | `true`  | Drain and journal instead of killing running jobs |
| `drain_ping_interval_secs` | `120`   | How often to check whether the server recovered   |

While draining, the runner stops claiming, lets running jobs finish, and journals results under
`<output-dir>/offline_journal/`. Replay them with `torc workflows reconcile <id> <run_id>`. With
`enabled = false` the runner kills running jobs and exits.

## client.slurm

| Option                    | Default | Notes                                                            |
| ------------------------- | ------- | ---------------------------------------------------------------- |
| `poll_interval`           | `30`    | Seconds, for Slurm job runners                                   |
| `keep_submission_scripts` | `false` | Keep generated sbatch scripts; useful when debugging submission  |
| `strict_scheduler_match`  | `false` | When true, a worker claims only jobs matching its `scheduler_id` |

With `strict_scheduler_match = false`, a worker whose matching queue is empty will claim jobs
belonging to another scheduler. That maximizes utilization and can also run a job on an allocation
sized for something else; set it true when resource profiles must stay isolated.

## client.watch

**This section is currently inert.** The struct exists and `torc config show` prints it, but nothing
outside `src/config/client.rs` reads it: `torc watch` takes its poll interval from the clap default
of **60 seconds** and its retry count from `--max-retries`, with no config-file fallback. Setting
these values changes nothing today.

| Option                  | Value in config            | What actually applies                            |
| ----------------------- | -------------------------- | ------------------------------------------------ |
| `poll_interval`         | `30`                       | Ignored; `torc watch -p` defaults to `60`        |
| `max_retries`           | `3`                        | Ignored; `--max-retries` is unlimited when unset |
| `retry_cooldown`        | `60`                       | Ignored                                          |
| `model`                 | `claude-sonnet-4-20250514` | Ignored                                          |
| `rate_limit_per_minute` | `10`                       | Ignored                                          |
| `cache_path`            | unset                      | Ignored                                          |
| `audit_log_path`        | unset                      | Ignored                                          |
| `api_key`               | unset                      | Ignored; use `ANTHROPIC_API_KEY`                 |

Pass the flags on the command line instead, and do not report a `[client.watch]` value as the
effective setting.

## client.tls

| Option     | Default | Notes                                       |
| ---------- | ------- | ------------------------------------------- |
| `ca_cert`  | unset   | PEM CA to trust for HTTPS servers           |
| `insecure` | `false` | Skip certificate verification; testing only |

## client.hpc

| Option              | Default | Notes                              |
| ------------------- | ------- | ---------------------------------- |
| `default_account`   | unset   | Applies to all profiles            |
| `profile_overrides` | `{}`    | Per-profile overrides of built-ins |
| `custom_profiles`   | `{}`    | User-defined clusters              |

See `hpc-profiles.md` for the profile and partition schema.

## server

Read by `torc-server`.

| Option                           | Default   | Notes                                            |
| -------------------------------- | --------- | ------------------------------------------------ |
| `host`                           | `0.0.0.0` | Bind address; accepts `url` as an alias          |
| `port`                           | `8080`    |                                                  |
| `threads`                        | `1`       | Worker threads                                   |
| `database`                       | unset     | SQLite path; falls back to `DATABASE_URL`        |
| `log_level`                      | `info`    |                                                  |
| `https`                          | `false`   | With `tls_cert` and `tls_key`                    |
| `tls_cert` / `tls_key`           | unset     | PEM paths                                        |
| `auth_file`                      | unset     | htpasswd file                                    |
| `require_auth`                   | `false`   | Require authentication on all requests           |
| `credential_cache_ttl_secs`      | `60`      | Avoids repeated bcrypt verification; 0 disables  |
| `enforce_access_control`         | `false`   | Enforce ownership and group membership           |
| `admin_users`                    | `[]`      | Added to the admin group at startup              |
| `completion_check_interval_secs` | `30.0`    | Background completion processing interval        |
| `disable_admin_sql`              | `false`   | Disable `torc admin sql` entirely                |
| `disable_admin_sql_writes`       | `false`   | Read-only raw SQL; ignored when the above is set |

The default bind is `0.0.0.0`, which is reachable from other machines as soon as the port is open.
Confirm the effective value with `torc config show`.

Enabling `require_auth` without distributing credentials locks out every client, including running
job runners. `completion_check_interval_secs` bounds how quickly downstream jobs unblock after a
completion, so a large value makes a workflow look stalled.

## server.logging

| Option      | Default | Notes                                       |
| ----------- | ------- | ------------------------------------------- |
| `log_dir`   | unset   | Setting it enables rotating file logging    |
| `json_logs` | `false` | JSON lines instead of human-readable output |

Files are written as `torc-server.log` in `log_dir`, rotating at 10 MiB and keeping 5 files.

## dash

| Option                           | Default                                 | Notes                                    |
| -------------------------------- | --------------------------------------- | ---------------------------------------- |
| `host`                           | `127.0.0.1`                             | Dashboard bind address                   |
| `port`                           | `8090`                                  |                                          |
| `api_url`                        | `http://localhost:8080/torc-service/v1` | Server the dashboard queries             |
| `torc_bin`                       | `torc`                                  | CLI used for execution features          |
| `torc_server_bin`                | `torc-server`                           | Binary used in standalone mode           |
| `standalone`                     | `false`                                 | Auto-start a server                      |
| `server_port`                    | `0`                                     | 0 means auto-detect a free port          |
| `server_host`                    | `0.0.0.0`                               | Bind address for the auto-started server |
| `database`                       | unset                                   | Database for standalone mode             |
| `socket`                         | unset (Unix only)                       | UNIX socket instead of TCP               |
| `completion_check_interval_secs` | `5`                                     | Standalone-mode completion interval      |

The dashboard binds loopback by default while its managed server binds `0.0.0.0`. To expose the
dashboard itself, set `host = "0.0.0.0"` deliberately and put authentication in front of it.

## Environment variable mapping

Structured form mirrors the file layout with `__` between levels:

```bash
export TORC_CLIENT__API_URL="https://torc.example.gov:8443/torc-service/v1"
export TORC_CLIENT__RUN__OUTPUT_DIR="/scratch/$USER/torc_output"
export TORC_CLIENT__RUN__MAX_PARALLEL_JOBS=4
export TORC_SERVER__PORT=9000
export TORC_SERVER__LOGGING__LOG_DIR=/var/log/torc
export TORC_DASH__HOST=0.0.0.0
```

Direct variables also work and are what most users reach for: `TORC_API_URL`, `TORC_PASSWORD`,
`TORC_USERNAME`, `TORC_COOKIE_HEADER`, `TORC_TLS_CA_CERT`, `TORC_TLS_INSECURE`, `TORC_SERVER_BIN`,
`TORC_AUTH_FILE`, `TORC_LOG_DIR`, `TORC_ADMIN_USERS`, `TORC_COMPLETION_CHECK_INTERVAL_SECS`,
`TORC_MAX_REQUEST_BODY_MB`, `DATABASE_URL`, `RUST_LOG`.

## Worked example

A user config that points at a shared HTTPS server and keeps run output on scratch:

```toml
# ~/.config/torc/config.toml
[client]
api_url = "https://torc.hpc.example.gov:8443/torc-service/v1"
format = "table"
log_level = "info"

[client.tls]
ca_cert = "/etc/pki/tls/certs/internal-ca.pem"

[client.run]
output_dir = "/scratch/alice/torc_output"
poll_interval = 5.0
claim_backoff_max_secs = 300.0

[client.slurm]
keep_submission_scripts = true

[client.hpc]
default_account = "my_project"
```

A project-local `./torc.toml` beside a workflow, overriding only what differs:

```toml
[client.run]
output_dir = "runs/current"
max_parallel_jobs = 2
```

Verify the merge, do not assume it:

```bash
torc config paths
torc config show | head -30
torc config validate
```
