# Command behavior

`torc <command> --help` is authoritative for flags. This file covers behavior that `--help` does not
state: discovery, prompting, streams, exit codes, and pagination.

## Contents

- [Finding commands](#finding-commands)
- [Interactive prompting](#interactive-prompting)
- [Streams and structured output](#streams-and-structured-output)
- [Exit codes](#exit-codes)
- [Pagination](#pagination)
- [Version compatibility](#version-compatibility)
- [Authentication and TLS](#authentication-and-tls)

## Finding commands

`torc --help` prints grouped headings (Workflow Lifecycle, Workflow Management, Scheduler & Compute,
Analysis & Debugging, Server Administration, Configuration & Utilities), but most subcommand groups
are marked hidden, so the top-level `Commands:` block lists only a couple of entries. The grouped
listing under it is the real map.

Consequences:

- Discover a group's contents with `torc <group> --help` (`torc workflows --help`,
  `torc slurm --help`, `torc remote --help`, ...). These also print grouped headings that list more
  subcommands than the `Commands:` block.
- Some commands exist only in the grouped listing (for example `torc workflows new`,
  `torc workflows execution-plan`, `torc slurm regenerate`, `torc access-groups ...`).
- Lifecycle commands are top level (`create`, `run`, `exec`, `submit`, `status`, `watch`, `recover`,
  `cancel`, `delete`), not under `torc workflows`.

Shell completion covers the hidden entries: `torc completions bash|zsh|fish`.

## Interactive prompting

Many commands take the workflow ID as an optional positional argument. When it is omitted:

- With exactly one non-archived workflow for the user, that workflow is selected silently.
- With several, the command **prints a workflow table to stdout** and reads an ID from stdin.
- On EOF or an invalid entry it exits 1.

Two problems follow for non-interactive use:

1. The selection table is written to stdout, so it corrupts `-f json` output. An agent parsing JSON
   sees a plain-text table instead and `jq` fails.
2. Which workflow gets picked silently depends on how many exist, so a script can operate on the
   wrong workflow.

Always pass the workflow ID explicitly. Where a command takes it as a flag, use `--workflow-id`.

Other prompts and their non-interactive behavior:

| Prompt                                            | Non-interactive escape                                          |
| ------------------------------------------------- | --------------------------------------------------------------- |
| Resource validation warnings on create/run/submit | Refuses to proceed when stdin is not a TTY; use `--skip-checks` |
| Pending `schedule_nodes` review on re-submit      | `--no-prompts`; a non-TTY stdin implies it                      |
| Reset confirmation (`workflows reset-status`)     | `--no-prompts`                                                  |
| `recover` wizard                                  | `--no-prompts`                                                  |
| `jobs reset-status` confirmation                  | `--no-prompts`                                                  |

## Streams and structured output

Default: data on stdout, log messages on stderr. `-f json` gives machine-readable stdout for most
commands, and `-f csv` works for list commands.

The runner commands invert part of this:

- `torc run` and `torc exec` write runner log lines to **stdout** in table format, and to **stderr**
  when `-f json`, so JSON stays parseable. They also always write the same lines to the runner log
  file under the output directory.
- In standalone mode (`-s`), the embedded server's own logs are prefixed `[torc-server]` and go to
  stderr.

```bash
torc -f json status <id> | jq '.jobs_by_status.failed'      # clean JSON on stdout
torc -f csv results list <id> > results.csv                  # spreadsheet export
torc -f json run workflow.yaml 2>run.log                     # JSON out, logs aside
```

`-f csv` is rejected with a clear error, and exit 1, for single-record commands (`jobs get`) and
multi-section reports (`status`, `workflows check-resources`). Use `-f json` for those.

## Exit codes

Torc uses 0 for success and 1 for failure; there is no richer taxonomy. What matters is which
failures are actually reported.

| Command                               | Exit on job failure | Notes                                                   |
| ------------------------------------- | ------------------- | ------------------------------------------------------- |
| `torc run`                            | **0**               | Logs `had_failures=true` but does not propagate it      |
| `torc exec`                           | 1                   | Exits 1 on any failed or terminated job                 |
| `torc watch`                          | 1                   | Also 1 on max retries, stalled recovery, pending_failed |
| `torc create --dry-run`               | 1 on invalid spec   | Fully offline, no server needed                         |
| `torc jobs reset-status --status <s>` | 1 when no match     | Prevents silent no-ops in scripts                       |

So the check after a local run is server state, not `$?`:

```bash
torc run workflow.yaml -o out
failed=$(torc -f json status <id> | jq '.jobs_by_status.failed')
[ "$failed" -eq 0 ] || torc results list <id> --failed
```

Other exit-1 cases: unreachable server, missing record (`jobs get <bad-id>`), unsupported format for
a command, prompt EOF, and validation refusal.

## Pagination

List commands page through the API. `-l/--limit` caps results (default: all), `--offset` is 0-based,
and `--sort-by` with `--reverse-sort` orders them. The CLI paginates transparently: a single API
response is capped at 100,000 records (the server rejects a raw request asking for more), and the
CLI issues as many requests as needed, so a large `-l` value is not an error at the CLI level.

On large workflows, prefer server-side filters (`-s/--status`, `--return-code`, `--failed`,
`-j/--job-id`, `-r/--run-id`, `--compute-node`) over fetching everything and filtering locally.
Unfiltered `list` on a workflow with many thousands of jobs costs several round trips.

`torc jobs list --include-relationships` adds dependency and file/user-data IDs at the cost of extra
queries. Leave it off unless the relationships are needed.

## Version compatibility

The HTTP API carries its own semver, separate from the binary version. The client compares them at
startup and warns on patch/minor drift; a major difference is blocking.

`--skip-version-check` suppresses the check. Use it only to confirm a diagnosis; fix the
installation instead. `torc remote run` performs the same check per worker and reports mismatches
per host.

`torc self update` updates a binary installed by the standalone installer.

## Authentication and TLS

| Mechanism        | Flag                | Environment          |
| ---------------- | ------------------- | -------------------- |
| Server URL       | `--url`             | `TORC_API_URL`       |
| Basic auth       | `--password`        | `TORC_PASSWORD`      |
| Secure prompt    | `--prompt-password` | —                    |
| Cookie/MFA proxy | `--cookie-header`   | `TORC_COOKIE_HEADER` |
| Custom CA        | `--tls-ca-cert`     | `TORC_TLS_CA_CERT`   |
| Skip TLS verify  | `--tls-insecure`    | `TORC_TLS_INSECURE`  |

The username comes from `TORC_USERNAME`, falling back to `USER` / `USERNAME`. It sets workflow
ownership and the basic-auth username, and does not affect Slurm submission, which always uses the
real system user.

`--tls-insecure` disables certificate verification. It belongs in local testing only.
