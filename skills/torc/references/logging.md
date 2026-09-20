# Logging

Torc has three separate logging surfaces: the CLI, the job runner, and the server. They are
configured differently and land in different places.

## Contents

- [CLI logging](#cli-logging)
- [Job runner logging](#job-runner-logging)
- [Server logging](#server-logging)
- [Standalone mode](#standalone-mode)
- [What gets logged](#what-gets-logged)
- [Choosing a level](#choosing-a-level)
- [Recipes](#recipes)

## CLI logging

```bash
torc --log-level debug workflows list
RUST_LOG=debug torc workflows list
```

`--log-level` is declared with `env = "RUST_LOG"`, so the two are the same argument rather than two
independent mechanisms. Precedence: `--log-level` flag, then `RUST_LOG`, then `client.log_level`,
then `info`.

For ordinary commands the logger is `env_logger` with the value used as a filter string, so module
directives work:

```bash
RUST_LOG=torc=debug torc status 123
RUST_LOG=torc::client::apis=trace torc jobs list 123
```

Log records go to stderr; command data goes to stdout. That separation is what makes `-f json`
pipelines safe.

## Job runner logging

`torc run`, `torc exec`, `torc watch`, and the Slurm job runner install their own logger. Where the
lines go depends on which one:

| Command                 | Console     | Log file                                                              |
| ----------------------- | ----------- | --------------------------------------------------------------------- |
| `torc run`, `torc exec` | stderr      | `<output-dir>/job_runner_<hostname>_wf<id>_r<run>.log`                |
| `torc watch`            | stderr      | `<output-dir>/watch_<hostname>_wf<id>.log`                            |
| `torc-slurm-job-runner` | **nothing** | `<output-dir>/job_runner_slurm_wf<id>_sl<slurm>_n<node>_pid<pid>.log` |

- **`run`, `exec` and `watch` duplicate every line** to stderr and to the file. Console output is
  stderr in every format, so `-f json` stdout stays parseable. The runner prints the exact path at
  startup.
- **The Slurm job runner writes to the file only.** Its logger targets the log file and nothing
  else, so do not expect runner output in the allocation's `slurm_output_wf<id>_sl<slurm>.e` -- that
  file gets only whatever the process wrote before the logger was installed, plus output from the
  jobs themselves. When a Slurm allocation looks silent, read `job_runner_slurm_*.log`, not the
  Slurm stderr file.

The filter syntax is the same as for any other command: these loggers call `parse_filters`, so
`RUST_LOG=torc=debug torc run ...` and `--log-level torc::client::job_runner=debug` both work.

`torc tui` installs no logger at all; it is a full-screen application and log output would corrupt
the display.

Job stdout and stderr are separate from all of this and land in `<output-dir>/job_stdio/`, governed
by `execution_config.stdio`. See `log-map.md`.

## Server logging

Console logging is always on. File logging turns on when a directory is configured:

```toml
[server]
log_level = "info"

[server.logging]
log_dir = "/var/log/torc"
json_logs = false
```

Equivalent environment variables: `TORC_SERVER__LOG_LEVEL`, `TORC_SERVER__LOGGING__LOG_DIR` (or
`TORC_LOG_DIR`), `TORC_SERVER__LOGGING__JSON_LOGS`.

Files are written as `torc-server.log` in `log_dir`, rotating at 10 MiB and keeping 5 files, with no
compression. `json_logs = true` emits JSON lines for log shipping.

The server logs one line per initialization stage (migrations, auth, runtime threads, bind address),
which is where to look when it fails to start.

For SQL-level debugging, the server honors `RUST_LOG` filters:

```bash
RUST_LOG=sqlx=debug torc-server run
```

Live request-level views without changing the log level:

```bash
torc admin tail-api          # inbound requests over SSE
torc admin api-stats         # recent rate, throughput, status mix
```

## Standalone mode

With `-s`, the CLI spawns `torc-server` and forwards its output with a `[torc-server]` prefix on
stderr. That child reads the `[server]` configuration; the client's `--log-level` does not apply to
it.

Consequences:

- `torc --log-level warn -s status <id>` still shows `[torc-server] INFO` lines. Quiet them with
  `TORC_SERVER__LOG_LEVEL=warn` or `[server] log_level`.
- Filtering `[torc-server]` out of stderr is safe when you only want client output.
- The standalone server binds `127.0.0.1` on an auto-assigned port and prints the resolved URL,
  which is the value to reuse in the same session.

## What gets logged

Log messages carry `key=value` pairs, so they are greppable and machine-filterable:

```text
Job started workflow_id=1 job_id=1 run_id=1 compute_node_id=1 attempt_id=1 pid=65026
Job completed workflow_id=1 job_id=1 run_id=1 attempt_id=1 return_code=0 status=completed exec_time_s=0.005
Jobs unblocked workflow_id=1 completed_count=1 ready_count=1
Compute node deactivated workflow_id=1 run_id=1 compute_node_id=1 duration_s=30.4
```

The runner's startup line is the single most useful record: it reports client and server versions,
both API versions, detected resources, compute-node rules, poll interval, `claim_backoff_max_secs`,
`max_parallel_jobs`, end time, `strict_scheduler_match`, execution mode, and `limit_resources`.

Repository convention: log messages that reference database records use
`workflow_id=<id> job_id=<id>` so parsing scripts can pick them up. Keep new messages in that form.

## Choosing a level

| Level   | Use for                                                              |
| ------- | -------------------------------------------------------------------- |
| `error` | Quiet automation where only failures matter                          |
| `warn`  | Production runs where warnings should still surface                  |
| `info`  | Default: lifecycle events, job start/completion, allocation activity |
| `debug` | Claim decisions, API interactions, resource evaluation               |
| `trace` | Protocol-level detail; very high volume                              |

`debug` on a long multi-thousand-job run produces large runner logs on a shared filesystem. Prefer
raising the level for a targeted rerun rather than for a full campaign.

## Recipes

```bash
# Debug why a runner is not claiming jobs (module filters work here too)
RUST_LOG=torc::client::job_runner=debug torc run workflow.yaml -o out
grep -E "claim|ready|resource" out/job_runner_*.log

# Debug API interactions for an ordinary command (module filter allowed)
RUST_LOG=torc::client::apis=debug torc jobs list 123

# Capture runner logs without touching stdout
torc run workflow.yaml -o out 2>run.log
jq . <<<"$(torc -f json status 123)"

# Silence the embedded server in standalone mode
TORC_SERVER__LOG_LEVEL=warn torc -s status 123

# Ship server logs as JSON
TORC_SERVER__LOGGING__LOG_DIR=/var/log/torc TORC_SERVER__LOGGING__JSON_LOGS=true torc-server run

# Trace one job across every log file
grep -rE "workflow_id=123.*job_id=456" out/
```
