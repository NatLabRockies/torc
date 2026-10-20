# Command behavior

`cli-reference.md` is the per-command index. This file covers the cross-cutting mechanics that apply
to every command: prompting, streams, exit codes, pagination, versioning, and auth.

## Contents

- [Interactive prompting](#interactive-prompting)
- [Streams and structured output](#streams-and-structured-output)
- [Exit codes](#exit-codes)
- [Pagination](#pagination)
- [Version compatibility](#version-compatibility)
- [Authentication and TLS](#authentication-and-tls)

## Interactive prompting

Many commands take the workflow ID as an optional positional argument. When it is omitted:

- **Without a TTY on stdin the command exits 1** with
  `Error: a workflow ID is required when stdin is not a terminal`. It never prompts and never
  guesses which workflow you meant.
- On a TTY with exactly one non-archived workflow for the user, that workflow is selected silently.
- On a TTY with several, the command prints a workflow table **on stderr** and reads an ID from
  stdin. On EOF or an invalid entry it exits 1.

The stdout stream is never used for the table, so `-f json` stays parseable. The remaining hazard is
the silent single-workflow selection on an interactive terminal: which workflow gets picked depends
on how many exist, so an interactive script can still operate on the wrong one.

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

stdout carries only the command's actual result: tables, JSON, CSV, and result text. Log output,
prompts, warnings, hints, notes, and progress messages all go to stderr. This holds for every
command and every format, so `-f json` stdout is always a single parseable document.

- `torc run`, `torc exec`, and `torc watch` write runner log lines to **stderr** regardless of `-f`,
  and write the same lines to a log file under the output directory.
- Progress lines such as `Created workflow N` from `run`/`submit`/`exec` with a spec go to stderr;
  `torc create` prints its `Created workflow N` result on stdout.
- In standalone mode (`-s`), the embedded server's own logs are prefixed `[torc-server]` and go to
  stderr.

```bash
torc -f json status <id> | jq '.jobs_by_status.failed'      # clean JSON on stdout
torc -f csv results list <id> > results.csv                  # spreadsheet export
torc run workflow.yaml -o out 2>run.log                      # runner logs aside
```

`-f csv` is rejected with a clear error, and exit 1, for single-record commands (`jobs get`) and
multi-section reports (`status`, `workflows check-resources`). Use `-f json` for those.

## Exit codes

Torc uses 0 for success and 1 for failure; there is no richer taxonomy. The thing to understand is
**what each command's exit status is a statement about**.

| Command                               | Exit on job failure | What the status means                                     |
| ------------------------------------- | ------------------- | --------------------------------------------------------- |
| `torc run`                            | **0**               | Whether the _runner_ worked, not whether the workload did |
| `torc exec`                           | 1                   | Whether every inline command succeeded                    |
| `torc watch`                          | 1                   | Also 1 on max retries, stalled recovery, pending_failed   |
| `torc create --dry-run`               | 1 on invalid spec   | Fully offline, no server needed                           |
| `torc jobs reset-status --status <s>` | 1 when no match     | Prevents silent no-ops in scripts                         |

`torc run` exiting 0 with failed jobs is deliberate, not an oversight. A workflow is a long-lived
graph in which some failures are expected and are handled by failure handlers, recovery, or a later
rerun; the runner reports that it claimed, executed, and recorded jobs without itself failing, and
it logs `had_failures=true`. Job outcomes live in server state, which outlasts the process. Do not
"fix" this by wrapping `torc run` in a status check that treats it as a bug.

`torc exec` is the opposite case on purpose: it is a batch-of-commands tool in the mould of GNU
Parallel, where the exit status is the whole point, so it exits 1 if any job failed or was
terminated. Prefer `exec` in CI when you want `$?` to mean something.

Either way, the authoritative check after a run is server state, not `$?`:

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
