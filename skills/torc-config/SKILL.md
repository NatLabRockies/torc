---
name: torc-config
description: >
  Configure Torc's client, server, and dashboard settings and control its logging. Use for
  torc.toml or config.toml files, torc config show/paths/init/validate, TORC_* environment
  variables, api_url and authentication defaults, run and Slurm defaults, HPC profiles and custom
  clusters, log levels, RUST_LOG, server file logging, and JSON logs. Do not use for workflow spec
  fields, which belong to torc-workflows.
license: BSD-3-Clause
---

# Torc configuration and logging

Torc reads layered TOML files plus `TORC_*` environment variables, and CLI flags override both.
Everything a flag can set has a config-file equivalent, so put stable choices in a file and keep
flags for the exceptions.

Inspect before editing. `torc config show` prints the effective merged result, which is the only
reliable answer to "what will this command actually use".

## Task router

| Task                                                 | Read                         |
| ---------------------------------------------------- | ---------------------------- |
| Every setting, section by section, with defaults     | `references/settings.md`     |
| Log levels, log destinations, and log file locations | `references/logging.md`      |
| Define or override an HPC profile for a cluster      | `references/hpc-profiles.md` |

## Precedence

Later sources win:

1. Built-in defaults
2. `/etc/torc/config.toml` (system)
3. The user config: `~/.config/torc/config.toml` on Linux,
   `~/Library/Application Support/torc/config.toml` on macOS
4. `./torc.toml` (project-local, resolved against the current directory)
5. `TORC_*` environment variables
6. CLI flags

```bash
torc config show                  # effective merged configuration (TOML)
torc config show -f json          # same, as JSON
torc config paths                 # which files exist and are being read
torc config init --user           # write a commented default file
torc config init --local          # ./torc.toml
torc config validate              # check the current configuration
```

`torc config paths` also confirms whether a file is being picked up at all, which is the first check
when a setting seems ignored.

Project-local `./torc.toml` is resolved against the current working directory, so the same command
can behave differently from another directory. That is convenient for per-project defaults and a
trap when a job runs elsewhere.

## Environment variables

Two families exist, and both work:

- Structured, mirroring the file layout with `__` between levels: `TORC_CLIENT__API_URL`,
  `TORC_CLIENT__RUN__OUTPUT_DIR`, `TORC_SERVER__PORT`, `TORC_SERVER__LOGGING__LOG_DIR`,
  `TORC_DASH__HOST`.
- Direct variables read by the CLI and server: `TORC_API_URL`, `TORC_PASSWORD`, `TORC_USERNAME`,
  `TORC_COOKIE_HEADER`, `TORC_TLS_CA_CERT`, `TORC_TLS_INSECURE`, `TORC_SERVER_BIN`,
  `TORC_AUTH_FILE`, `TORC_LOG_DIR`, `TORC_ADMIN_USERS`, `TORC_COMPLETION_CHECK_INTERVAL_SECS`,
  `TORC_MAX_REQUEST_BODY_MB`, `DATABASE_URL`, `RUST_LOG`.

Torc also sets variables **into** jobs at execution time (`TORC_WORKFLOW_ID`, `TORC_JOB_ID`,
`TORC_OUTPUT_DIR`, and so on). Those are outputs, not configuration inputs; setting them yourself
does not reconfigure anything.

## Three components, one file

A single config file can carry all three sections; each binary reads its own.

| Section    | Read by       | Typical settings                               |
| ---------- | ------------- | ---------------------------------------------- |
| `[client]` | `torc`        | `api_url`, `format`, `log_level`, run defaults |
| `[server]` | `torc-server` | `host`, `port`, `database`, auth, logging      |
| `[dash]`   | `torc-dash`   | `host`, `port`, `api_url`, standalone behavior |

A server-side change (`require_auth`, `enforce_access_control`, `admin_users`) does nothing until
`torc-server` restarts, except the htpasswd file, which `torc admin reload-auth` rereads live.

## Logging in one screen

```bash
torc --log-level debug <command>          # this invocation
RUST_LOG=debug torc <command>             # same, via environment
```

Four things regularly surprise people:

- `--log-level` and `RUST_LOG` are the **same clap argument**, so `RUST_LOG` sets the CLI's
  `--log-level` value rather than acting as an independent filter.
- `torc run`, `torc exec`, `torc watch`, and `torc tui` install their own logger, and it accepts
  only a bare level (`error`, `warn`, `info`, `debug`, `trace`). A target filter such as
  `RUST_LOG=torc=debug` is rejected with `Invalid log level ... defaulting to 'info'`. Set
  `RUST_LOG` to a module directive only for commands that use the default logger.
- `torc run` and `torc exec` write log lines to **stdout** in table format (stderr with `-f json`),
  and always mirror them into the runner log file.
- In standalone mode the embedded server's logs are prefixed `[torc-server]`, arrive on stderr, and
  follow the `[server]` configuration, not the client's `--log-level`.

Default client level is `info` (`client.log_level`). Full detail, including server file logging and
log rotation, is in `references/logging.md`.

## Output

Report the file or variable changed, the effective value confirmed with `torc config show`, which
component reads it, and whether a restart is needed.
